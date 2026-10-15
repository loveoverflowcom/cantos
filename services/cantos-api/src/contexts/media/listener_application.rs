//! Listener use cases: no Studio token, session cookie, editorial access or production work.
use super::{
    domain::listening::{after_work, DiscoveryQuery, ListenerDomainError},
    listener_cursor::{CursorVault, Keyset},
    listener_delivery::{PlaybackDelivery, PlaybackPermit, UnavailablePlayback},
    listener_projection::{publication_manifest, unit_summary, work_summary},
    postgres::listener::{PgRead, ReadItem},
};
use crate::platform::{
    ids::canonical_uuid,
    postgres::{Store, StoreError},
};
use cantos_contracts::theatre as dto;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerError {
    InvalidRequest,
    InvalidCursor,
    NotFound,
    RenditionUnavailable,
    Unavailable,
    Corrupt,
}
impl From<StoreError> for ListenerError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::CorruptRevision => Self::Corrupt,
            _ => Self::Unavailable,
        }
    }
}
impl From<ListenerDomainError> for ListenerError {
    fn from(e: ListenerDomainError) -> Self {
        match e {
            ListenerDomainError::InvalidQuery | ListenerDomainError::InvalidLimit => {
                Self::InvalidRequest
            }
            ListenerDomainError::BindingMismatch | ListenerDomainError::InvalidTime => {
                Self::Corrupt
            }
        }
    }
}
fn clock() -> Result<i64, ListenerError> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ListenerError::Unavailable)?
            .as_millis(),
    )
    .map_err(|_| ListenerError::Unavailable)
}
fn id(raw: &str) -> Result<String, ListenerError> {
    canonical_uuid(raw)
        .filter(|v| v == raw)
        .ok_or(ListenerError::InvalidRequest)
}
pub(super) enum ReadRequest {
    Work {
        work: String,
    },
    Works {
        query: DiscoveryQuery,
        after: Option<Keyset>,
    },
    Units {
        work: String,
        after: Option<Keyset>,
        limit: usize,
    },
    Publication {
        id: String,
    },
}
pub(super) struct ReadSnapshot {
    pub now: i64,
    pub items: Vec<ReadItem>,
    pub next_key: Option<Keyset>,
}
pub(super) type ReadFuture =
    Pin<Box<dyn Future<Output = Result<ReadSnapshot, ListenerError>> + Send + 'static>>;
pub(super) trait ListenerRead: Send + Sync {
    fn read(&self, request: ReadRequest) -> ReadFuture;
}
#[derive(Clone)]
pub struct ListenerService {
    read: Arc<dyn ListenerRead>,
    cursors: Arc<CursorVault>,
    delivery: Arc<dyn PlaybackDelivery>,
}

/// Named external-read double for the crate-root transport contract fixture only.
/// No runtime switch or public constructor of publication authority is created.
#[cfg(test)]
#[derive(Clone, Copy, Default)]
pub(crate) enum PlaybackFixtureEligibility {
    #[default]
    Current,
    Withdrawn,
    Revoked,
    Superseded,
    OwnerDisabled,
}

#[cfg(test)]
impl ListenerService {
    pub(crate) fn playback_contract_fixture(
        delivery: Arc<dyn PlaybackDelivery>,
        sha256: String,
        byte_length: u64,
        eligibility: Arc<std::sync::Mutex<PlaybackFixtureEligibility>>,
        now: Arc<std::sync::atomic::AtomicI64>,
    ) -> Self {
        Self {
            read: Arc::new(tests::FixtureRead {
                sha256,
                byte_length,
                eligibility,
                now,
            }),
            cursors: Arc::new(CursorVault::default()),
            delivery,
        }
    }
}
impl ListenerService {
    pub fn new(store: Store) -> Self {
        Self {
            read: Arc::new(PgRead(store)),
            cursors: Arc::new(CursorVault::default()),
            delivery: Arc::new(UnavailablePlayback),
        }
    }
    pub fn with_playback_delivery(mut self, delivery: Arc<dyn PlaybackDelivery>) -> Self {
        self.delivery = delivery;
        self
    }
    pub async fn works(
        &self,
        search: &str,
        kind: Option<&str>,
        language: Option<&str>,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::WorkPage, ListenerError> {
        let query = DiscoveryQuery::new(search, kind, language, limit)?;
        let after = cursor
            .map(|v| self.cursors.read(v, &query, clock()?))
            .transpose()?;
        if after
            .as_ref()
            .is_some_and(|k| !matches!(k, Keyset::Works { .. }))
        {
            return Err(ListenerError::InvalidCursor);
        }
        let snapshot = self
            .read
            .read(ReadRequest::Works {
                query: query.clone(),
                after: after.clone(),
            })
            .await?;
        let mut items = Vec::new();
        for r in snapshot.items {
            if !query.matches(&r.publication.discoverable()) {
                return Err(ListenerError::Corrupt);
            }
            let summary = work_summary(&r.publication, r.first_at);
            if let Some(Keyset::Works { time, id: after_id }) = &after {
                let current = uuid::Uuid::parse_str(&summary.work_id)
                    .map_err(|_| ListenerError::Corrupt)?
                    .as_u128();
                let old = uuid::Uuid::parse_str(after_id)
                    .map_err(|_| ListenerError::Corrupt)?
                    .as_u128();
                if !after_work(r.first_at, current, *time, old) {
                    return Err(ListenerError::Corrupt);
                }
            }
            items.push(summary);
        }
        items.truncate(query.limit());
        let next_cursor = snapshot
            .next_key
            .map(|key| self.cursors.issue(&query, key, clock()?))
            .transpose()?;
        Ok(dto::WorkPage { items, next_cursor })
    }
    pub async fn work(&self, work: &str) -> Result<dto::WorkSummary, ListenerError> {
        let work = id(work)?;
        let snapshot = self.read.read(ReadRequest::Work { work }).await?;
        snapshot
            .items
            .first()
            .map(|r| work_summary(&r.publication, r.first_at))
            .ok_or(ListenerError::NotFound)
    }
    pub async fn units(
        &self,
        work: &str,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::UnitPage, ListenerError> {
        let work = id(work)?;
        let query = DiscoveryQuery::new("", None, None, limit)?;
        let after = cursor
            .map(|v| self.cursors.read(v, &query, clock()?))
            .transpose()?;
        if after
            .as_ref()
            .is_some_and(|k| !matches!(k,Keyset::Units{work:w,..} if w==&work))
        {
            return Err(ListenerError::InvalidCursor);
        }
        let snapshot = self
            .read
            .read(ReadRequest::Units {
                work: work.clone(),
                after,
                limit: query.limit(),
            })
            .await?;
        if snapshot.items.is_empty() && snapshot.next_key.is_none() && cursor.is_none() {
            return Err(ListenerError::NotFound);
        }
        let mut items: Vec<_> = snapshot
            .items
            .into_iter()
            .map(|r| unit_summary(&r.publication, r.position))
            .collect();
        items.truncate(query.limit());
        let next_cursor = snapshot
            .next_key
            .map(|key| self.cursors.issue(&query, key, clock()?))
            .transpose()?;
        Ok(dto::UnitPage {
            work_id: work,
            items,
            next_cursor,
        })
    }
    async fn resolve(&self, publication: &str) -> Result<(ReadItem, i64), ListenerError> {
        let snapshot = self
            .read
            .read(ReadRequest::Publication {
                id: id(publication)?,
            })
            .await?;
        let p = snapshot
            .items
            .into_iter()
            .next()
            .ok_or(ListenerError::NotFound)?;
        if p.publication.release().document().id != publication {
            return Err(ListenerError::Corrupt);
        }
        Ok((p, snapshot.now))
    }
    pub async fn manifest(
        &self,
        publication: &str,
    ) -> Result<dto::PublicationManifest, ListenerError> {
        let (item, _) = self.resolve(publication).await?;
        Ok(publication_manifest(&item.publication))
    }
    pub async fn authorize_playback(
        &self,
        publication: &str,
    ) -> Result<dto::PlaybackAuthorization, ListenerError> {
        let (initial, now) = self.resolve(publication).await?;
        let d = initial.publication.release().document();
        let m = &d.manifest;
        let until = now
            .checked_add(120_000)
            .ok_or(ListenerError::Corrupt)?
            .min(m.distribution_clearance.request.valid_until);
        let permit = PlaybackPermit {
            publication_id: d.id.clone(),
            rendition_id: m.rendition_id.clone(),
            manifest_digest: m.digest.clone(),
            sha256: m.media.sha256.clone(),
            object_id: m.delivery_object_id.clone(),
            byte_length: m.media.byte_length,
            valid_until: until,
        };
        let grant = self.delivery.grant(permit.clone()).await?;
        // Re-read from a fresh PG transaction after external signing. A stale snapshot cannot grant.
        let (fresh, fresh_now) = self.resolve(publication).await?;
        if publication_manifest(&fresh.publication) != publication_manifest(&initial.publication)
            || grant.permit != permit
            || grant.expires <= fresh_now
            || grant.expires > until
            || !grant.range
        {
            return Err(ListenerError::RenditionUnavailable);
        }
        Ok(dto::PlaybackAuthorization {
            publication_id: d.id.clone(),
            rendition_id: m.rendition_id.clone(),
            sha256: m.media.sha256.clone(),
            url: grant.url,
            expires_at_ms: grant.expires,
            content_type: "audio/ogg".into(),
            byte_range_supported: true,
        })
    }
}
#[cfg(test)]
pub(super) mod tests {
    use super::super::{
        domain::{
            listening::{ListenerFacts, ListenerPublication},
            publication::{decide, AudioRelease, ReleaseId},
        },
        listener_delivery::{MediaGrant, PlaybackDeliveryFuture},
    };
    use super::*;
    use crate::contexts::catalog::domain::{NewWork, Visibility, Work};
    use std::sync::Mutex;
    fn item() -> ReadItem {
        item_with_media(
            &"a".repeat(64),
            100,
            PlaybackFixtureEligibility::Current,
            5000,
        )
        .unwrap()
    }
    pub(super) fn item_with_media(
        sha256: &str,
        byte_length: u64,
        eligibility: PlaybackFixtureEligibility,
        now: i64,
    ) -> Option<ReadItem> {
        let mut f =
            super::super::domain::publication::tests::fixture_with_media(sha256, byte_length);
        let mut details = f.work.details().clone();
        details.visibility = Visibility::Public;
        f.work = Work::new(NewWork {
            id: f.work.id().clone(),
            kind: f.work.kind(),
            owner_id: f.work.owner_id().clone(),
            details,
            at: 1000,
        })
        .unwrap();
        let r = AudioRelease::publish(
            ReleaseId::parse("80000000-0000-4000-8000-000000000008").unwrap(),
            &f.actor,
            5000,
            None,
            decide(&f.facts()).unwrap(),
        )
        .unwrap();
        let p = ListenerPublication::resolve(ListenerFacts {
            release: &r,
            pinned_work: &f.work,
            pinned_unit: &f.unit,
            current_work: &f.work,
            current_unit: &f.unit,
            rights: Some(&f.rights),
            approval: Some(&f.approval),
            active: !matches!(eligibility, PlaybackFixtureEligibility::Superseded),
            owner_active: !matches!(eligibility, PlaybackFixtureEligibility::OwnerDisabled),
            withdrawn: matches!(eligibility, PlaybackFixtureEligibility::Withdrawn),
            submission_revoked: matches!(eligibility, PlaybackFixtureEligibility::Revoked),
            now,
        })
        .unwrap()?;
        Some(ReadItem {
            publication: p,
            first_at: 5000,
            position: 1,
        })
    }

    pub(super) struct FixtureRead {
        pub(super) sha256: String,
        pub(super) byte_length: u64,
        pub(super) eligibility: Arc<Mutex<PlaybackFixtureEligibility>>,
        pub(super) now: Arc<std::sync::atomic::AtomicI64>,
    }
    impl ListenerRead for FixtureRead {
        fn read(&self, _request: ReadRequest) -> ReadFuture {
            let now = self.now.load(std::sync::atomic::Ordering::SeqCst);
            let item = item_with_media(
                &self.sha256,
                self.byte_length,
                *self.eligibility.lock().unwrap(),
                now,
            );
            Box::pin(async move {
                Ok(ReadSnapshot {
                    now,
                    items: item.into_iter().collect(),
                    next_key: None,
                })
            })
        }
    }
    struct SequencedRead(Mutex<Vec<bool>>);
    impl ListenerRead for SequencedRead {
        fn read(&self, _request: ReadRequest) -> ReadFuture {
            let visible = self.0.lock().unwrap().remove(0);
            Box::pin(async move {
                Ok(ReadSnapshot {
                    now: 5000,
                    items: if visible { vec![item()] } else { vec![] },
                    next_key: None,
                })
            })
        }
    }
    struct NamedDelivery {
        expires: i64,
        range: bool,
    }
    impl PlaybackDelivery for NamedDelivery {
        fn grant(&self, permit: PlaybackPermit) -> PlaybackDeliveryFuture {
            assert_eq!(permit.publication_object_id(), "publication-fixture");
            let expires = self.expires;
            let range = self.range;
            Box::pin(async move {
                MediaGrant::verified_for(
                    &permit,
                    "https://synthetic.invalid/approved-media".into(),
                    expires,
                    range,
                )
            })
        }
    }
    fn service(sequence: Vec<bool>, delivery: Arc<dyn PlaybackDelivery>) -> ListenerService {
        ListenerService {
            read: Arc::new(SequencedRead(Mutex::new(sequence))),
            cursors: Arc::new(CursorVault::default()),
            delivery,
        }
    }
    const ID: &str = "80000000-0000-4000-8000-000000000008";
    #[tokio::test]
    async fn grant_rechecks_withdrawal_and_never_uses_stale_read() {
        let s = service(
            vec![true, false],
            Arc::new(NamedDelivery {
                expires: 9000,
                range: true,
            }),
        );
        assert_eq!(
            s.authorize_playback(ID).await.unwrap_err(),
            ListenerError::NotFound
        );
    }
    #[tokio::test]
    async fn expiry_window_and_range_are_enforced_after_io() {
        for (expires, range) in [(5000, true), (10001, true), (9000, false)] {
            let s = service(vec![true, true], Arc::new(NamedDelivery { expires, range }));
            assert_eq!(
                s.authorize_playback(ID).await.unwrap_err(),
                ListenerError::RenditionUnavailable
            );
        }
        let s = service(
            vec![true, true],
            Arc::new(NamedDelivery {
                expires: 9000,
                range: true,
            }),
        );
        let result = s.authorize_playback(ID).await.unwrap();
        assert_eq!(result.sha256, "a".repeat(64));
        assert_eq!(result.expires_at_ms, 9000);
    }
    #[tokio::test]
    async fn default_delivery_is_explicitly_unavailable() {
        let s = service(vec![true], Arc::new(UnavailablePlayback));
        assert_eq!(
            s.authorize_playback(ID).await.unwrap_err(),
            ListenerError::RenditionUnavailable
        );
    }

    struct ForeignGrant(usize);
    impl PlaybackDelivery for ForeignGrant {
        fn grant(&self, mut permit: PlaybackPermit) -> PlaybackDeliveryFuture {
            // Model an adapter replaying a grant issued for another immutable permit.
            match self.0 {
                0 => permit.publication_id = "90000000-0000-4000-8000-000000000009".into(),
                1 => permit.rendition_id = "90000000-0000-4000-8000-000000000009".into(),
                2 => permit.manifest_digest = "other-manifest-version".into(),
                3 => permit.sha256 = "b".repeat(64),
                4 => permit.object_id = "other-publication-object".into(),
                5 => permit.byte_length = 101,
                6 => permit.valid_until = 9999,
                _ => unreachable!("finite test mutation domain"),
            }
            Box::pin(async move {
                MediaGrant::verified_for(
                    &permit,
                    "https://synthetic.invalid/approved-media".into(),
                    9000,
                    true,
                )
            })
        }
    }

    #[tokio::test]
    async fn adapter_replay_cannot_substitute_any_field_of_the_sealed_permit() {
        for dimension in 0..7 {
            let service = service(vec![true, true], Arc::new(ForeignGrant(dimension)));
            assert_eq!(
                service.authorize_playback(ID).await.unwrap_err(),
                ListenerError::RenditionUnavailable,
                "identity dimension {dimension}"
            );
        }
    }
    /// Returns the resolved fixture under a listing anchor that differs from its Studio position.
    struct AnchoredRead(u32);
    impl ListenerRead for AnchoredRead {
        fn read(&self, _request: ReadRequest) -> ReadFuture {
            let position = self.0;
            Box::pin(async move {
                let mut anchored = item();
                anchored.position = position;
                Ok(ReadSnapshot {
                    now: 5000,
                    items: vec![anchored],
                    next_key: None,
                })
            })
        }
    }
    #[tokio::test]
    async fn discovery_projects_resolved_publications_with_listing_anchors() {
        let s = ListenerService {
            read: Arc::new(AnchoredRead(9)),
            cursors: Arc::new(CursorVault::default()),
            delivery: Arc::new(UnavailablePlayback),
        };
        let work_id = item().publication.work().id().as_str().to_owned();
        let page = s
            .works("ÁNH ĐÈN", Some("podcast"), Some("vi-VN"), 20, None)
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].work_id, work_id);
        assert_eq!(page.items[0].kind, dto::WorkKind::Podcast);
        assert_eq!(page.items[0].title, "Ánh đèn cuối sân khấu");
        assert_eq!(page.items[0].first_published_at_ms, 5000);
        assert_eq!(s.work(&work_id).await.unwrap(), page.items[0]);
        let units = s.units(&work_id, 20, None).await.unwrap();
        assert_eq!(units.items.len(), 1);
        assert_eq!(units.items[0].position, 9);
        assert_eq!(units.items[0].kind, dto::UnitKind::Episode);
        assert_eq!(units.items[0].publication_id, ID);
        // A read adapter that returns a publication the query excludes is corrupt, not a result.
        for (search, kind, language) in [
            ("không có", None, None),
            ("", Some("novel"), None),
            ("", None, Some("en-US")),
        ] {
            assert_eq!(
                s.works(search, kind, language, 20, None).await.unwrap_err(),
                ListenerError::Corrupt
            );
        }
    }

    /// Models eligibility loss after a bounded scan, then a fresh read of the remaining units.
    struct UnitWalkRead {
        remaining_visible: bool,
        requests: Mutex<Vec<Option<Keyset>>>,
    }
    impl ListenerRead for UnitWalkRead {
        fn read(&self, request: ReadRequest) -> ReadFuture {
            let ReadRequest::Units { work, after, limit } = request else {
                panic!("unit walk must not read another discovery route");
            };
            assert_eq!(work, item().publication.work().id().as_str());
            assert_eq!(limit, 1);
            self.requests.lock().unwrap().push(after.clone());
            let remaining_visible = self.remaining_visible;
            Box::pin(async move {
                let mut remaining = item();
                remaining.position = 9;
                let scanned = Keyset::Units {
                    work,
                    position: 7,
                    id: "70000000-0000-4000-8000-000000000007".into(),
                };
                let (items, next_key) = match after {
                    None => (vec![], Some(scanned)),
                    Some(key) => {
                        assert_eq!(key, scanned);
                        (
                            remaining_visible.then_some(remaining).into_iter().collect(),
                            None,
                        )
                    }
                };
                Ok(ReadSnapshot {
                    now: 5000,
                    items,
                    next_key,
                })
            })
        }
    }

    #[tokio::test]
    async fn empty_first_unit_page_preserves_continuation_and_cursor_scope() {
        for remaining_visible in [false, true] {
            let read = Arc::new(UnitWalkRead {
                remaining_visible,
                requests: Mutex::new(vec![]),
            });
            let s = ListenerService {
                read: read.clone(),
                cursors: Arc::new(CursorVault::default()),
                delivery: Arc::new(UnavailablePlayback),
            };
            let work = item().publication.work().id().as_str().to_owned();
            let first = s.units(&work, 1, None).await.unwrap();
            assert_eq!(first.work_id, work);
            assert!(first.items.is_empty());
            let cursor = first.next_cursor.unwrap();
            for (other_work, limit) in [
                ("90000000-0000-4000-8000-000000000009", 1),
                (work.as_str(), 2),
            ] {
                assert_eq!(
                    s.units(other_work, limit, Some(&cursor)).await.unwrap_err(),
                    ListenerError::InvalidCursor
                );
            }
            assert_eq!(
                s.works("", None, None, 1, Some(&cursor)).await.unwrap_err(),
                ListenerError::InvalidCursor
            );
            assert_eq!(read.requests.lock().unwrap().as_slice(), &[None]);

            let next = s.units(&work, 1, Some(&cursor)).await.unwrap();
            assert_eq!(next.work_id, work);
            assert_eq!(next.next_cursor, None);
            let expected = if remaining_visible {
                vec![unit_summary(&item().publication, 9)]
            } else {
                vec![]
            };
            assert_eq!(next.items, expected);
            assert_eq!(read.requests.lock().unwrap().len(), 2);
        }
    }

    #[tokio::test]
    async fn empty_catalog_and_private_or_absent_release_are_distinct() {
        let s = service(vec![false, false, false], Arc::new(UnavailablePlayback));
        assert_eq!(
            s.works("", None, None, 20, None).await.unwrap(),
            dto::WorkPage {
                items: vec![],
                next_cursor: None
            }
        );
        assert_eq!(s.manifest(ID).await.unwrap_err(), ListenerError::NotFound);
        let work = item().publication.work().id().as_str().to_owned();
        assert_eq!(
            s.units(&work, 1, None).await.unwrap_err(),
            ListenerError::NotFound
        );
    }
}
