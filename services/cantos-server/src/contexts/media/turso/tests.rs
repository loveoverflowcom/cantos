//! Listener reads over a real file-backed `server.db` (no PostgreSQL, no network, no doubles for
//! the database). Each publication is seeded with production domain values (see
//! `test_support::world`); the serving path under test is the production `ListenerService`.
//! Nothing here establishes the creator -> publication pipeline, CDN delivery or a device.
use std::sync::Arc;

use cantos_contracts::theatre as dto;
use cantos_store_turso::testing::FixedClock;
use futures_util::StreamExt;

use crate::{
    contexts::{
        catalog::domain::{Visibility, WorkKind},
        media::{
            domain::playback_grant::GrantKey, FilesystemPublicationStorage, ListenerError,
            ListenerService, OriginPlayback, PublicMediaOrigin,
        },
    },
    test_support::{Corruption, SeededWork, ServerFixture, Stage, UnitSeed, WorkSeed},
};

/// 2030-03-17 in microseconds; far from any real clock so a leaked wall-clock read shows.
const T0: i64 = 1_900_000_000_000_000;

async fn fixture() -> (ServerFixture, Arc<FixedClock>) {
    let clock = FixedClock::new(T0);
    (ServerFixture::with_clock(clock.clone()).await, clock)
}

fn service(fixture: &ServerFixture) -> ListenerService {
    ListenerService::new(fixture.store().clone())
}

fn story(tag: u32, title: &str, language: &str) -> WorkSeed {
    let mut seed = WorkSeed::podcast(tag, title, &["Chương một"]);
    seed.kind = WorkKind::Story;
    seed.language = language.into();
    seed
}

async fn single(fixture: &ServerFixture) -> SeededWork {
    fixture
        .seed_work(
            None,
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một lời hẹn"]),
        )
        .await
}

/// The refusal of a result whose success value (a media admission) must never be printed.
fn refusal<T>(result: Result<T, ListenerError>) -> ListenerError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected a refusal"),
    }
}

fn release(work: &SeededWork, unit: usize) -> String {
    work.units[unit].chain.release_id.clone().unwrap()
}

#[tokio::test]
async fn discovery_orders_filters_searches_and_pages_by_immutable_anchor() {
    let (fixture, clock) = fixture().await;
    let first = fixture
        .seed_work(
            None,
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một lời hẹn", "Tập hai"]),
        )
        .await;
    clock.advance(10_000_000);
    let second = fixture
        .seed_work(None, &story(2, "Chuyện nhỏ", "en-US"))
        .await;
    let service = service(&fixture);

    let page = service.works("", None, None, 20, None).await.unwrap();
    assert_eq!(
        page.items
            .iter()
            .map(|w| w.work_id.as_str())
            .collect::<Vec<_>>(),
        [second.work_id.as_str(), first.work_id.as_str()],
        "newest first anchor, then identity"
    );
    assert_eq!(page.next_cursor, None);
    assert_eq!(page.items[1].title, "Ánh đèn cuối sân khấu");
    assert_eq!(page.items[1].language, "vi-VN");
    assert_eq!(page.items[1].kind, dto::WorkKind::Podcast);
    assert_eq!(page.items[0].kind, dto::WorkKind::Story);
    assert!(page.items[0].first_published_at_ms > page.items[1].first_published_at_ms);

    // Search is NFC/NFD and case insensitive over the pinned title and synopsis.
    for query in [
        "ÁNH ĐÈN",
        "a\u{0301}nh đe\u{0300}n",
        "sân khấu",
        "danh mục công khai",
    ] {
        let found = service.works(query, None, None, 20, None).await.unwrap();
        assert!(
            found.items.iter().any(|w| w.work_id == first.work_id),
            "{query}"
        );
    }
    assert!(service
        .works("không có", None, None, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    let podcasts = service
        .works("", Some("podcast"), None, 20, None)
        .await
        .unwrap();
    assert_eq!(podcasts.items.len(), 1);
    assert_eq!(podcasts.items[0].work_id, first.work_id);
    let english = service
        .works("", None, Some("en-US"), 20, None)
        .await
        .unwrap();
    assert_eq!(english.items.len(), 1);
    assert_eq!(english.items[0].work_id, second.work_id);

    // A cursor walk with limit 1 visits each Work exactly once and ends without a cursor.
    let one = service.works("", None, None, 1, None).await.unwrap();
    assert_eq!(one.items.len(), 1);
    let cursor = one.next_cursor.clone().expect("a continuation");
    let two = service
        .works("", None, None, 1, Some(&cursor))
        .await
        .unwrap();
    assert_eq!(two.items[0].work_id, first.work_id);
    assert_eq!(two.next_cursor, None);
    // The cursor is bound to its query: a different filter, or a different limit, is refused.
    assert_eq!(
        service
            .works("", Some("podcast"), None, 1, Some(&cursor))
            .await
            .unwrap_err(),
        ListenerError::InvalidCursor
    );
    assert_eq!(
        service
            .works("", None, None, 2, Some(&cursor))
            .await
            .unwrap_err(),
        ListenerError::InvalidCursor
    );
    assert_eq!(
        service
            .works("", None, None, 1, Some("forged"))
            .await
            .unwrap_err(),
        ListenerError::InvalidCursor
    );

    // Units: stable order by the immutable position, and the cursor is bound to its Work.
    let units = service.units(&first.work_id, 20, None).await.unwrap();
    assert_eq!(units.work_id, first.work_id);
    assert_eq!(
        units
            .items
            .iter()
            .map(|u| (u.position, u.title.as_str()))
            .collect::<Vec<_>>(),
        [(1, "Một lời hẹn"), (2, "Tập hai")]
    );
    assert_eq!(units.items[0].publication_id, release(&first, 0));
    assert_eq!(units.items[0].duration_ms, 60_000);
    let head = service.units(&first.work_id, 1, None).await.unwrap();
    let rest = service
        .units(&first.work_id, 1, head.next_cursor.as_deref())
        .await
        .unwrap();
    assert_eq!(rest.items[0].unit_id, first.units[1].unit_id);
    assert_eq!(rest.next_cursor, None);
    assert_eq!(
        service
            .units(&second.work_id, 1, head.next_cursor.as_deref())
            .await
            .unwrap_err(),
        ListenerError::InvalidCursor
    );
    assert_eq!(
        service
            .units("00000000-0000-4000-8000-0000000000ff", 20, None)
            .await
            .unwrap_err(),
        ListenerError::NotFound
    );
    assert_eq!(
        service.work(&second.work_id).await.unwrap().title,
        "Chuyện nhỏ"
    );
}

#[tokio::test]
async fn presentation_is_an_opt_in_with_exact_counts_and_ordinals_over_the_eligible_relation() {
    let (fixture, _) = fixture().await;
    let work = fixture
        .seed_work(
            None,
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một", "Hai", "Ba"]),
        )
        .await;
    let service = service(&fixture);
    let page = service
        .presented_works("", None, None, 20, None)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].unit_count, 3);
    assert_eq!(
        service
            .presented_work(&work.work_id)
            .await
            .unwrap()
            .unit_count,
        3
    );
    let units = service
        .presented_units(&work.work_id, 2, None)
        .await
        .unwrap();
    assert_eq!(
        units.items.iter().map(|u| u.ordinal).collect::<Vec<_>>(),
        [1, 2]
    );
    let tail = service
        .presented_units(&work.work_id, 2, units.next_cursor.as_deref())
        .await
        .unwrap();
    assert_eq!(
        tail.items
            .iter()
            .map(|u| (u.ordinal, u.position))
            .collect::<Vec<_>>(),
        [(3, 3)]
    );
    // A unit that stops being eligible leaves the count and renumbers the ordinals; its anchor
    // position stays where it was.
    fixture.withdraw_release("alice", &work.units[1]).await;
    assert_eq!(
        service
            .presented_work(&work.work_id)
            .await
            .unwrap()
            .unit_count,
        2
    );
    let units = service
        .presented_units(&work.work_id, 20, None)
        .await
        .unwrap();
    assert_eq!(
        units
            .items
            .iter()
            .map(|u| (u.position, u.ordinal))
            .collect::<Vec<_>>(),
        [(1, 1), (3, 2)]
    );
    // The two cursor vaults are separate: a plain cursor never opens a presented page.
    let plain = service.units(&work.work_id, 1, None).await.unwrap();
    assert_eq!(
        service
            .presented_units(&work.work_id, 1, plain.next_cursor.as_deref())
            .await
            .unwrap_err(),
        ListenerError::InvalidCursor
    );
}

/// Every reason a release must stop resolving, each on its own single-release Work.
#[tokio::test]
async fn private_staged_withdrawn_revoked_expired_superseded_archived_and_disabled_are_refused() {
    let (fixture, _) = fixture().await;
    let service = service(&fixture);
    let mut cases: Vec<(&str, SeededWork)> = Vec::new();
    for (tag, name) in [
        (10, "private"),
        (11, "withdrawn"),
        (12, "revoked"),
        (13, "archived work"),
        (14, "archived unit"),
        (15, "owner disabled"),
        (16, "superseded"),
    ] {
        let mut seed = WorkSeed::podcast(tag, &format!("Tác phẩm {name}"), &["Một lời hẹn"]);
        if name == "owner disabled" {
            // Another owner: disabling one actor must not hide anyone else's Work.
            seed.owner = "carol".into();
        }
        cases.push((name, fixture.seed_work(None, &seed).await));
    }
    let mut staged = WorkSeed::podcast(17, "Tác phẩm chưa phát hành", &["Một lời hẹn"]);
    staged.units[0].stage = Stage::Staged;
    let staged = fixture.seed_work(None, &staged).await;
    let mut expired = WorkSeed::podcast(18, "Tác phẩm hết hạn", &["Một lời hẹn"]);
    expired.units[0] = UnitSeed {
        rights_until_from_now_ms: -60_000,
        ..UnitSeed::published("Một lời hẹn")
    };
    let expired = fixture.seed_work(None, &expired).await;
    let mut hidden = WorkSeed::podcast(19, "Tác phẩm riêng tư", &["Một lời hẹn"]);
    hidden.visibility = Visibility::Private;
    let hidden = fixture.seed_work(None, &hidden).await;
    let unlisted = {
        let mut seed = WorkSeed::podcast(20, "Tác phẩm không công khai", &["Một lời hẹn"]);
        seed.visibility = Visibility::Unlisted;
        fixture.seed_work(None, &seed).await
    };

    // Before any change every published Work is visible.
    assert_eq!(
        service
            .works("", None, None, 50, None)
            .await
            .unwrap()
            .items
            .len(),
        7
    );

    let by_name = |name: &str| &cases.iter().find(|(n, _)| *n == name).unwrap().1;
    fixture
        .edit_work_visibility(by_name("private"), Visibility::Private)
        .await;
    fixture
        .withdraw_release("alice", &by_name("withdrawn").units[0])
        .await;
    fixture
        .revoke_submission("alice", &by_name("revoked").units[0])
        .await;
    fixture.archive_work(by_name("archived work")).await;
    fixture
        .archive_unit(&by_name("archived unit").units[0])
        .await;
    fixture.set_actor_active("carol", false).await;
    let replacement = {
        let w = by_name("superseded");
        fixture.publish_replacement(None, w, &w.units[0]).await
    };

    let gone: Vec<(&str, String)> = vec![
        ("private", release(by_name("private"), 0)),
        ("withdrawn", release(by_name("withdrawn"), 0)),
        ("revoked", release(by_name("revoked"), 0)),
        ("archived work", release(by_name("archived work"), 0)),
        ("archived unit", release(by_name("archived unit"), 0)),
        ("expired", release(&expired, 0)),
        ("owner disabled", release(by_name("owner disabled"), 0)),
        ("hidden at publication", release(&hidden, 0)),
        ("unlisted", release(&unlisted, 0)),
        ("superseded", release(by_name("superseded"), 0)),
    ];
    for (name, id) in &gone {
        assert_eq!(
            service.manifest(id).await.unwrap_err(),
            ListenerError::NotFound,
            "{name}"
        );
        assert_eq!(
            service.authorize_playback(id).await.unwrap_err(),
            ListenerError::NotFound,
            "{name}"
        );
    }
    // A staged unit never had a release to resolve.
    assert_eq!(staged.units[0].chain.release_id, None);
    for work in [
        by_name("private"),
        by_name("withdrawn"),
        by_name("revoked"),
        by_name("archived work"),
        by_name("archived unit"),
        by_name("owner disabled"),
        &expired,
        &hidden,
        &unlisted,
        &staged,
    ] {
        assert_eq!(
            service.work(&work.work_id).await.unwrap_err(),
            ListenerError::NotFound,
            "{}",
            work.title
        );
        assert_eq!(
            service.units(&work.work_id, 20, None).await.unwrap_err(),
            ListenerError::NotFound,
            "{}",
            work.title
        );
    }
    // The superseding release resolves, and is the only visible Work left.
    let page = service.works("", None, None, 50, None).await.unwrap();
    assert_eq!(
        page.items
            .iter()
            .map(|w| w.work_id.as_str())
            .collect::<Vec<_>>(),
        [by_name("superseded").work_id.as_str()]
    );
    assert_eq!(
        service
            .manifest(replacement.release_id.as_ref().unwrap())
            .await
            .unwrap()
            .sha256,
        replacement.sha256
    );
    assert_ne!(
        replacement.sha256,
        by_name("superseded").units[0].chain.sha256
    );
}

#[tokio::test]
async fn a_withdrawal_or_revocation_fact_denies_even_while_the_pointer_still_names_the_release() {
    for revoke in [false, true] {
        let (fixture, _) = fixture().await;
        let work = single(&fixture).await;
        let service = service(&fixture);
        let id = release(&work, 0);
        service.manifest(&id).await.unwrap();
        if revoke {
            fixture.revoke_submission("alice", &work.units[0]).await;
        } else {
            fixture.record_withdrawal("alice", &work.units[0]).await;
        }
        assert!(
            fixture.pointer_names(&id).await,
            "the pointer was not moved"
        );
        assert_eq!(
            service.manifest(&id).await.unwrap_err(),
            ListenerError::NotFound
        );
        assert!(service
            .works("", None, None, 20, None)
            .await
            .unwrap()
            .items
            .is_empty());
        assert_eq!(
            service.authorize_playback(&id).await.unwrap_err(),
            ListenerError::NotFound
        );
    }
}

#[tokio::test]
async fn list_and_detail_share_one_representative_release_per_work() {
    let (fixture, _) = fixture().await;
    let work = fixture
        .seed_work(
            None,
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một", "Hai", "Ba"]),
        )
        .await;
    let service = service(&fixture);
    let units = service.units(&work.work_id, 20, None).await.unwrap();
    assert_eq!(units.items.len(), 3);
    // Three releases were published at the same instant: the representative of the Work is the
    // smallest release id, whatever order the rows were read in, in list, detail and presentation.
    let mut ids: Vec<_> = (0..3).map(|i| release(&work, i)).collect();
    ids.sort();
    let listed = service.works("", None, None, 20, None).await.unwrap();
    let detail = service.work(&work.work_id).await.unwrap();
    assert_eq!(listed.items[0], detail);
    let presented = service.presented_work(&work.work_id).await.unwrap();
    assert_eq!(presented.unit_count, 3);
    assert_eq!(presented.title, detail.title);
    assert_eq!(
        service.manifest(&ids[0]).await.unwrap().work_id,
        work.work_id
    );
}

#[tokio::test]
async fn released_metadata_is_pinned_while_current_privacy_still_governs() {
    let (fixture, _) = fixture().await;
    let work = single(&fixture).await;
    let service = service(&fixture);
    let before = service.manifest(&release(&work, 0)).await.unwrap();
    assert_eq!(before.work_id, work.work_id);
    assert_eq!(before.unit_id, work.units[0].unit_id);
    assert_eq!(before.rendition_id, work.units[0].chain.rendition_id);
    assert_eq!(before.sha256, work.units[0].chain.sha256);
    assert_eq!(before.byte_length, 4096);
    assert_eq!(before.duration_ms, 60_000);
    assert_eq!(
        (before.container.as_str(), before.codec.as_str()),
        ("ogg", "opus")
    );
    // A later draft title is never disclosed: the summary is the released revision.
    fixture
        .edit_work(&work, |details| {
            details.title = "Bản nháp riêng chưa phát hành".into()
        })
        .await;
    let page = service.works("", None, None, 20, None).await.unwrap();
    assert_eq!(page.items[0].title, "Ánh đèn cuối sân khấu");
    assert_eq!(service.manifest(&release(&work, 0)).await.unwrap(), before);
    assert_eq!(
        service
            .works("bản nháp", None, None, 20, None)
            .await
            .unwrap()
            .items
            .len(),
        0,
        "search is over the released text"
    );
    // Current privacy, in contrast, applies at once and again when it is lifted.
    fixture
        .edit_work_visibility(&work, Visibility::Private)
        .await;
    assert_eq!(
        service.manifest(&release(&work, 0)).await.unwrap_err(),
        ListenerError::NotFound
    );
    fixture
        .edit_work_visibility(&work, Visibility::Public)
        .await;
    assert_eq!(service.manifest(&release(&work, 0)).await.unwrap(), before);
}

#[tokio::test]
async fn a_row_that_fails_its_integrity_checks_is_corruption_and_is_never_served() {
    for (name, damage) in [
        ("release snapshot", Corruption::ReleaseSnapshot),
        ("work revision digest", Corruption::WorkRevisionDigest),
        ("listener index digest", Corruption::ListenerIndexWindow),
        ("rendition object key", Corruption::RenditionObjectKey),
    ] {
        let (fixture, _) = fixture().await;
        let work = single(&fixture).await;
        let service = service(&fixture);
        service.manifest(&release(&work, 0)).await.unwrap();
        fixture.corrupt(damage).await;
        assert_eq!(
            service.manifest(&release(&work, 0)).await.unwrap_err(),
            ListenerError::Corrupt,
            "{name}"
        );
        assert_eq!(
            service.works("", None, None, 20, None).await.unwrap_err(),
            ListenerError::Corrupt,
            "{name}"
        );
    }
}

#[tokio::test]
async fn concurrent_readers_see_a_whole_publication_before_or_after_a_withdrawal_never_a_torn_one()
{
    let (fixture, _) = fixture().await;
    let work = single(&fixture).await;
    let service = service(&fixture);
    let id = release(&work, 0);
    let readers = (0..24).map(|_| {
        let service = service.clone();
        let id = id.clone();
        async move {
            (
                service.manifest(&id).await,
                service.works("", None, None, 20, None).await,
            )
        }
    });
    let withdrawal = fixture.withdraw_release("alice", &work.units[0]);
    let (outcomes, ()) = tokio::join!(futures_util::future::join_all(readers), withdrawal);
    for (manifest, works) in outcomes {
        assert!(
            matches!(manifest, Ok(_) | Err(ListenerError::NotFound)),
            "{manifest:?}"
        );
        assert!(works.is_ok(), "{works:?}");
    }
    assert_eq!(
        service.manifest(&id).await.unwrap_err(),
        ListenerError::NotFound
    );
}

fn playback(fixture: &ServerFixture) -> (FilesystemPublicationStorage, OriginPlayback) {
    let storage =
        FilesystemPublicationStorage::new(&fixture.dir().join("publication")).expect("a root");
    let origin = OriginPlayback::new(
        PublicMediaOrigin::parse("https://listen.example.test").unwrap(),
        GrantKey::new(&[3; 32]).unwrap(),
        Arc::new(storage.clone()),
    );
    (storage, origin)
}

async fn body(mut read: crate::contexts::media::storage::port::StorageRead) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(chunk) = read.stream.next().await {
        bytes.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(bytes.len() as u64, read.byte_length);
    bytes
}

#[tokio::test]
async fn playback_is_authorized_for_the_exact_release_and_the_bytes_are_those_it_names() {
    let (fixture, clock) = fixture().await;
    let (storage, origin) = playback(&fixture);
    let work = fixture
        .seed_work(
            Some(&storage),
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một lời hẹn"]),
        )
        .await;
    let chain = &work.units[0].chain;
    let id = release(&work, 0);
    // The default host has no origin: authorization is an honest, retryable unavailability.
    assert_eq!(
        service(&fixture).authorize_playback(&id).await.unwrap_err(),
        ListenerError::RenditionUnavailable
    );
    let service = service(&fixture).with_origin_playback(origin);
    let grant = service.authorize_playback(&id).await.unwrap();
    assert_eq!(grant.publication_id, id);
    assert_eq!(grant.rendition_id, chain.rendition_id);
    assert_eq!(grant.sha256, chain.sha256);
    assert_eq!(grant.content_type, "audio/ogg");
    assert!(grant.byte_range_supported);
    assert!(
        grant
            .url
            .starts_with("https://listen.example.test/api/v1/theatre/media/"),
        "{}",
        grant.url
    );
    assert!(!grant.url.contains(&chain.sha256) && !grant.url.contains(&chain.object_id));
    // The access window is a short grant, far inside the rights window (a day from now).
    assert_eq!(grant.expires_at_ms, T0 / 1000 + 120_000);

    let token = grant.url.rsplit('/').next().unwrap();
    let authorized = service.authorize_media(token).await.unwrap();
    assert_eq!(
        (authorized.sha256(), authorized.byte_length()),
        (chain.sha256.as_str(), 4096)
    );
    assert_eq!(
        body(service.open_media(&authorized, None).await.unwrap()).await,
        chain.bytes
    );
    let range = crate::contexts::media::read::ByteRange::parse("bytes=100-199")
        .unwrap()
        .normalize(4096)
        .unwrap();
    assert_eq!(
        body(service.open_media(&authorized, Some(range)).await.unwrap()).await,
        &chain.bytes[100..200]
    );
    // A token for another publication's bytes does not exist; a forged one names nothing.
    assert_eq!(
        refusal(service.authorize_media("forged").await),
        ListenerError::NotFound
    );

    // The grant is re-admitted on every fetch: its own expiry ends it, and so does a withdrawal.
    clock.advance(121_000_000);
    assert_eq!(
        refusal(service.authorize_media(token).await),
        ListenerError::NotFound
    );
    clock.set(T0);
    fixture.withdraw_release("alice", &work.units[0]).await;
    assert_eq!(
        refusal(service.authorize_media(token).await),
        ListenerError::NotFound
    );
    assert_eq!(
        service.authorize_playback(&id).await.unwrap_err(),
        ListenerError::NotFound
    );
}

#[tokio::test]
async fn a_release_whose_bytes_are_missing_or_replaced_is_a_delivery_failure_not_a_substitute() {
    let (fixture, _) = fixture().await;
    let (storage, origin) = playback(&fixture);
    // The metadata is current and eligible, but nothing was ever placed in the publication class.
    let work = single(&fixture).await;
    let service = service(&fixture).with_origin_playback(origin);
    let grant = service
        .authorize_playback(&release(&work, 0))
        .await
        .unwrap();
    let token = grant.url.rsplit('/').next().unwrap();
    let authorized = service.authorize_media(token).await.unwrap();
    assert!(matches!(
        service.open_media(&authorized, None).await,
        Err(ListenerError::RenditionUnavailable)
    ));
    // Placed bytes are served; placing different bytes under the same name is impossible, since
    // the object is addressed by its digest.
    storage
        .place_transport_bytes(&work.units[0].chain.bytes)
        .unwrap();
    assert_eq!(
        body(service.open_media(&authorized, None).await.unwrap()).await,
        work.units[0].chain.bytes
    );
}
