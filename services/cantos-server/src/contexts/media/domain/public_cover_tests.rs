use crate::contract::studio as dto;

use super::*;
use crate::contexts::catalog::domain::{
    NewWork, OwnerId, Visibility, Work, WorkDetails, WorkId, WorkKind,
};
use crate::contexts::media::domain::{
    artwork::{decide_artwork_change, ArtworkDecision, SetWorkArtwork},
    image::{ImageIntent, ImageMetadata, PrepareImage},
};

const WORK: &str = "00000000-0000-0000-0000-000000000001";
const OPERATION: &str = "00000000-0000-0000-0000-000000000002";
const INTENT_A: &str = "00000000-0000-0000-0000-000000000003";
const INTENT_B: &str = "00000000-0000-0000-0000-000000000004";

fn work() -> Work {
    Work::new(NewWork {
        id: WorkId::parse(WORK).unwrap(),
        kind: WorkKind::Story,
        owner_id: OwnerId::parse("owner-sentinel").unwrap(),
        details: WorkDetails {
            title: "Ánh đèn cuối sân khấu".into(),
            synopsis: String::new(),
            language: "vi".into(),
            visibility: Visibility::Public,
            rights_refs: vec!["work-rights".into()],
        },
        at: 100,
    })
    .unwrap()
}

fn asset(work: &Work, intent: &str, fill: char, at: i64) -> ImageAsset {
    let sha256 = fill.to_string().repeat(64);
    let intent = ImageIntent::create(
        PrepareImage::admit(&dto::CreateImageIntentRequest {
            operation_id: intent.into(),
            id: intent.into(),
            expected_sha256: sha256.clone(),
            expected_byte_length: 67,
            content_type: dto::ImageMime::Png,
            declarations: dto::ImageDeclarations {
                provenance: "private-provenance-sentinel".into(),
                rights_refs: vec!["private-rights-sentinel".into()],
            },
        })
        .unwrap(),
        work.id(),
        work.owner_id(),
        at,
    )
    .unwrap();
    ImageAsset::create(
        &intent,
        ImageMetadata::new(sha256, 67, ImageMime::Png, 2, 3).unwrap(),
        at + 1,
    )
    .unwrap()
}

/// Apply one owner change through the real artwork decision, returning the touched Work.
fn change(work: &Work, selected: Option<&ImageAsset>, at: i64) -> (Work, ArtworkChange) {
    let request = SetWorkArtwork::admit(&dto::SetWorkArtworkRequest {
        operation_id: OPERATION.into(),
        expected_work_revision: work.revision(),
        artwork: selected.map(|asset| dto::WorkArtworkSelection {
            image_asset_id: asset.id().as_str().into(),
            image_asset_digest: asset.digest().into(),
            alt_text: "Bìa tập truyện".into(),
        }),
    })
    .unwrap();
    match decide_artwork_change(work, &request, selected, None, at).unwrap() {
        ArtworkDecision::Apply(mutation) => (mutation.work().clone(), mutation.change().clone()),
        ArtworkDecision::Replay => panic!("a first operation applies"),
    }
}

/// w1 has no cover; w2 pins A; w3 is the owner's private replacement B; w4 clears it.
struct History {
    revisions: [Work; 4],
    a: ImageAsset,
    b: ImageAsset,
    changes: Vec<ArtworkChange>,
}
fn history() -> History {
    let w1 = work();
    let a = asset(&w1, INTENT_A, 'a', 110);
    let (w2, set_a) = change(&w1, Some(&a), 200);
    let b = asset(&w2, INTENT_B, 'b', 210);
    let (w3, set_b) = change(&w2, Some(&b), 300);
    let (w4, clear) = change(&w3, None, 400);
    History {
        revisions: [w1, w2, w3, w4],
        a,
        b,
        changes: vec![set_a, set_b, clear],
    }
}

fn cleared(asset: &ImageAsset, valid_until_ms: i64) -> CoverDisclosure<'_> {
    CoverDisclosure::Cleared {
        image_asset_digest: asset.digest(),
        valid_until_ms,
    }
}

fn project(
    pinned: &Work,
    h: &History,
    asset: Option<&ImageAsset>,
    disclosure: CoverDisclosure<'_>,
    now_ms: i64,
) -> CoverProjection {
    project_at(
        pinned,
        CoverFacts {
            changes: &h.changes,
            asset,
            disclosure,
            now_ms,
        },
    )
}

fn shown(projection: CoverProjection) -> PublicCover {
    match projection {
        CoverProjection::Shown(cover) => cover,
        CoverProjection::Fallback(reason) => panic!("expected a cover, got {reason:?}"),
    }
}

#[test]
fn private_replacement_after_publication_never_changes_the_release_pinned_cover() {
    let h = history();
    let [_, w2, w3, w4] = &h.revisions;
    // Release pinned at w2 keeps A although B and then a clear are newer private heads.
    let cover = shown(project(w2, &h, Some(&h.a), cleared(&h.a, 1000), 500));
    assert_eq!(cover.image_asset_id(), h.a.id().as_str());
    assert_eq!(cover.image_asset_digest(), h.a.digest());
    assert_eq!(cover.sha256(), "a".repeat(64));
    assert_eq!(cover.alt_text(), "Bìa tập truyện");
    assert_eq!(
        (cover.content_type(), cover.width_px(), cover.height_px()),
        (ImageMime::Png, 2, 3)
    );
    assert_eq!(cover.byte_length(), 67);
    // A later release pinned at w3 publishes B only with B's own clearance.
    let b = shown(project(w3, &h, Some(&h.b), cleared(&h.b, 1000), 500));
    assert_eq!(b.image_asset_digest(), h.b.digest());
    assert_eq!(
        project(w3, &h, Some(&h.b), cleared(&h.a, 1000), 500),
        CoverProjection::Fallback(CoverFallback::Corrupt)
    );
    // A release pinned after the explicit clear has no cover, whatever the shell supplies.
    for asset in [None, Some(&h.a), Some(&h.b)] {
        assert_eq!(
            project(w4, &h, asset, cleared(&h.b, 1000), 500),
            CoverProjection::Fallback(CoverFallback::NoArtwork)
        );
    }
}

#[test]
fn cover_never_predates_its_first_pinned_selection() {
    let h = history();
    assert_eq!(
        project(&h.revisions[0], &h, Some(&h.a), cleared(&h.a, 1000), 500),
        CoverProjection::Fallback(CoverFallback::NoArtwork)
    );
}

#[test]
fn public_display_requires_an_explicit_current_same_image_decision() {
    let h = history();
    let w2 = &h.revisions[1];
    for (disclosure, now, expected) in [
        (CoverDisclosure::Undecided, 500, CoverFallback::Undisclosed),
        (
            CoverDisclosure::Revoked {
                image_asset_digest: h.a.digest(),
            },
            500,
            CoverFallback::Revoked,
        ),
        (
            CoverDisclosure::Revoked {
                image_asset_digest: h.b.digest(),
            },
            500,
            CoverFallback::Corrupt,
        ),
        (cleared(&h.b, 1000), 500, CoverFallback::Corrupt),
        (cleared(&h.a, 500), 500, CoverFallback::Expired),
        (cleared(&h.a, 1000), -1, CoverFallback::Corrupt),
    ] {
        assert_eq!(
            project(w2, &h, Some(&h.a), disclosure, now),
            CoverProjection::Fallback(expected),
            "{disclosure:?} at {now}"
        );
    }
    shown(project(w2, &h, Some(&h.a), cleared(&h.a, 500), 499));
}

#[test]
fn missing_foreign_or_inconsistent_cover_facts_fall_back_without_failing() {
    let h = history();
    let w2 = &h.revisions[1];
    for asset in [None, Some(&h.b)] {
        assert_eq!(
            project(w2, &h, asset, cleared(&h.a, 1000), 500),
            CoverProjection::Fallback(CoverFallback::Corrupt)
        );
    }
    let duplicated = [h.changes[0].clone(), h.changes[0].clone()];
    assert_eq!(
        project_at(
            w2,
            CoverFacts {
                changes: &duplicated,
                asset: Some(&h.a),
                disclosure: cleared(&h.a, 1000),
                now_ms: 500,
            },
        ),
        CoverProjection::Fallback(CoverFallback::Corrupt)
    );
    // An asset owned by another Work cannot be bound even with an identical digest claim.
    let mut other = work();
    other = Work::new(NewWork {
        id: WorkId::parse("00000000-0000-0000-0000-000000000009").unwrap(),
        kind: other.kind(),
        owner_id: other.owner_id().clone(),
        details: other.details().clone(),
        at: 100,
    })
    .unwrap();
    let foreign = asset(&other, INTENT_A, 'a', 110);
    assert_eq!(
        project(w2, &h, Some(&foreign), cleared(&foreign, 1000), 500),
        CoverProjection::Fallback(CoverFallback::Corrupt)
    );
}

#[test]
fn public_cover_exposes_no_private_provenance_rights_or_owner() {
    let h = history();
    let cover = shown(project(
        &h.revisions[1],
        &h,
        Some(&h.a),
        cleared(&h.a, 1000),
        500,
    ));
    // The asset ID (which equals its admitted intent ID) is the public identity; the owner,
    // provenance and rights evidence of that admission are not.
    let rendered = format!("{cover:?}");
    for private in [
        "private-provenance-sentinel",
        "private-rights-sentinel",
        "owner-sentinel",
        "work-rights",
    ] {
        assert!(!rendered.contains(private), "{private}");
    }
}

#[test]
fn only_an_eligible_listener_publication_can_request_a_cover() {
    use crate::contexts::media::domain::{
        listening::ListenerFacts,
        publication::{decide, tests::fixture, AudioRelease, ReleaseId},
    };
    let mut f = fixture();
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
    let release = AudioRelease::publish(
        ReleaseId::parse("80000000-0000-4000-8000-000000000008").unwrap(),
        &f.actor,
        5000,
        None,
        decide(&f.facts()).unwrap(),
    )
    .unwrap();
    let publication = ListenerPublication::resolve(ListenerFacts {
        release: &release,
        pinned_work: &f.work,
        pinned_unit: &f.unit,
        current_work: &f.work,
        current_unit: &f.unit,
        rights: Some(&f.rights),
        approval: Some(&f.approval),
        active: true,
        owner_active: true,
        withdrawn: false,
        submission_revoked: false,
        now: 5000,
    })
    .unwrap()
    .unwrap();
    // The fixture release pins Work revision 1, before any artwork history exists.
    assert_eq!(
        project_public_cover(
            &publication,
            CoverFacts {
                changes: &[],
                asset: None,
                disclosure: CoverDisclosure::Undecided,
                now_ms: 5000,
            },
        ),
        CoverProjection::Fallback(CoverFallback::NoArtwork)
    );
}

/// Bounded exhaustive oracle over the finite kernel domain plus clock boundaries.
#[test]
fn cover_outcome_matches_an_independent_precedence_table() {
    let clocks = [i64::MIN, -1, 0, 4, 5, 6, i64::MAX];
    let mut decisions = vec![Decision::Undecided];
    for same_image in [false, true] {
        decisions.push(Decision::Revoked { same_image });
        for valid_until_ms in clocks {
            decisions.push(Decision::Cleared {
                same_image,
                valid_until_ms,
            });
        }
    }
    let mut shown = 0;
    for pinned in [Pinned::NoArtwork, Pinned::Corrupt, Pinned::Bound] {
        for decision in &decisions {
            for now in clocks {
                let expected = match (pinned, *decision) {
                    (Pinned::Corrupt, _) => Err(CoverFallback::Corrupt),
                    (Pinned::NoArtwork, _) => Err(CoverFallback::NoArtwork),
                    _ if now < 0 => Err(CoverFallback::Corrupt),
                    (_, Decision::Undecided) => Err(CoverFallback::Undisclosed),
                    (_, Decision::Revoked { same_image: true }) => Err(CoverFallback::Revoked),
                    (
                        _,
                        Decision::Revoked { .. }
                        | Decision::Cleared {
                            same_image: false, ..
                        },
                    ) => Err(CoverFallback::Corrupt),
                    (_, Decision::Cleared { valid_until_ms, .. }) if now >= valid_until_ms => {
                        Err(CoverFallback::Expired)
                    }
                    _ => Ok(()),
                };
                let actual = cover_outcome(pinned, *decision, now);
                assert_eq!(actual, expected, "{pinned:?} {decision:?} {now}");
                shown += usize::from(actual.is_ok());
            }
        }
    }
    // Non-vacuity: a cover is shown only for same-image clearances valid at now.
    assert_eq!(shown, 10);
}

// The registered fetch markers below are checked by tests/public_cover_behavior_binding.rs.
// They link native regressions to modeled scenarios; no trace replay/conformance is implied.
/// Quint run: cantos.media.public-cover-projection.run.fetch-current-shown
#[test]
fn cover_fetch_requires_the_exact_currently_shown_sha256() {
    let h = history();
    let w2 = &h.revisions[1];
    let (a, b) = ("a".repeat(64), "b".repeat(64));

    let undisclosed = project(w2, &h, Some(&h.a), CoverDisclosure::Undecided, 500);
    assert_eq!(
        undisclosed,
        CoverProjection::Fallback(CoverFallback::Undisclosed)
    );
    assert_eq!(admit_cover_fetch(&undisclosed, &a), None);

    let shown_a = project(w2, &h, Some(&h.a), cleared(&h.a, 1000), 500);
    let before = shown_a.clone();
    let admitted = admit_cover_fetch(&shown_a, &a).unwrap();
    assert_eq!(admitted.sha256(), a);
    assert_eq!(admitted.image_asset_id(), h.a.id().as_str());
    assert_eq!(admitted.image_asset_digest(), h.a.digest());
    // Malformed byte identities are native partitions, outside the model's two-image bound.
    for requested in [
        b,
        String::new(),
        "A".repeat(64),
        "g".repeat(64),
        a[..63].into(),
        "a".repeat(65),
    ] {
        assert_eq!(admit_cover_fetch(&shown_a, &requested), None, "{requested}");
    }
    assert_eq!(
        shown_a, before,
        "fetch admission cannot mutate the projection"
    );
}

/// Quint run: cantos.media.public-cover-projection.run.fetch-private-replacement
#[test]
fn cover_fetch_keeps_the_release_pinned_image_after_private_replacement_and_clear() {
    let h = history();
    let w2 = &h.revisions[1];
    let (a, b) = ("a".repeat(64), "b".repeat(64));
    // Complete history includes replacement B and a later clear; the release still pins A.
    let shown_a = project(w2, &h, Some(&h.a), cleared(&h.a, 1000), 500);
    assert_eq!(
        admit_cover_fetch(&shown_a, &a).map(PublicCover::sha256),
        Some(a.as_str())
    );
    assert_eq!(admit_cover_fetch(&shown_a, &b), None);

    // Even a current disclosure of private B is not authority for either image at A's pin.
    let wrong_disclosure = project(w2, &h, Some(&h.a), cleared(&h.b, 1000), 500);
    assert_eq!(
        wrong_disclosure,
        CoverProjection::Fallback(CoverFallback::Corrupt)
    );
    assert_eq!(admit_cover_fetch(&wrong_disclosure, &a), None);
    assert_eq!(admit_cover_fetch(&wrong_disclosure, &b), None);
}

/// Quint run: cantos.media.public-cover-projection.run.fetch-stale-republish
#[test]
fn cover_fetch_rejects_the_old_image_after_republish_pins_the_replacement() {
    let h = history();
    let [_, w2, w3, _] = &h.revisions;
    let (a, b) = ("a".repeat(64), "b".repeat(64));
    let shown_a = project(w2, &h, Some(&h.a), cleared(&h.a, 1000), 500);
    assert_eq!(
        admit_cover_fetch(&shown_a, &a).map(PublicCover::sha256),
        Some(a.as_str())
    );
    let shown_b = project(w3, &h, Some(&h.b), cleared(&h.b, 1000), 500);
    assert_eq!(admit_cover_fetch(&shown_b, &a), None);
    let admitted = admit_cover_fetch(&shown_b, &b).unwrap();
    assert_eq!(admitted.sha256(), b);
    assert_eq!(admitted.image_asset_id(), h.b.id().as_str());
    assert_eq!(admitted.image_asset_digest(), h.b.digest());
}

/// Quint run: cantos.media.public-cover-projection.run.fetch-revoked
#[test]
fn cover_fetch_rejects_the_previously_shown_image_after_revocation() {
    let h = history();
    let w2 = &h.revisions[1];
    let a = "a".repeat(64);
    let shown_a = project(w2, &h, Some(&h.a), cleared(&h.a, 1000), 500);
    assert_eq!(
        admit_cover_fetch(&shown_a, &a).map(PublicCover::sha256),
        Some(a.as_str())
    );
    let revoked = project(
        w2,
        &h,
        Some(&h.a),
        CoverDisclosure::Revoked {
            image_asset_digest: h.a.digest(),
        },
        500,
    );
    assert_eq!(revoked, CoverProjection::Fallback(CoverFallback::Revoked));
    assert_eq!(admit_cover_fetch(&revoked, &a), None);
}

/// Quint run: cantos.media.public-cover-projection.run.fetch-restored-disclosure
#[test]
fn cover_fetch_restores_the_same_image_when_its_current_disclosure_is_cleared_again() {
    let h = history();
    let w2 = &h.revisions[1];
    let (a, b) = ("a".repeat(64), "b".repeat(64));

    let initially_shown = project(w2, &h, Some(&h.a), cleared(&h.a, 1000), 500);
    let initial_cover = admit_cover_fetch(&initially_shown, &a).unwrap().clone();
    assert_eq!(initial_cover.sha256(), a);
    assert_eq!(initial_cover.image_asset_id(), h.a.id().as_str());
    assert_eq!(initial_cover.image_asset_digest(), h.a.digest());
    assert_eq!(admit_cover_fetch(&initially_shown, &b), None);

    let revoked = project(
        w2,
        &h,
        Some(&h.a),
        CoverDisclosure::Revoked {
            image_asset_digest: h.a.digest(),
        },
        501,
    );
    assert_eq!(revoked, CoverProjection::Fallback(CoverFallback::Revoked));
    assert_eq!(admit_cover_fetch(&revoked, &a), None);
    assert_eq!(admit_cover_fetch(&revoked, &b), None);

    let restored = project(w2, &h, Some(&h.a), cleared(&h.a, 1200), 502);
    assert_eq!(restored, CoverProjection::Shown(initial_cover.clone()));
    assert_eq!(admit_cover_fetch(&restored, &a), Some(&initial_cover));
    assert_eq!(admit_cover_fetch(&restored, &b), None);
}

/// Quint run: cantos.media.public-cover-projection.run.fetch-expired
#[test]
fn cover_fetch_rejects_the_previously_shown_image_at_the_expiry_boundary() {
    let h = history();
    let w2 = &h.revisions[1];
    let a = "a".repeat(64);
    let before_expiry = project(w2, &h, Some(&h.a), cleared(&h.a, 500), 499);
    assert_eq!(
        admit_cover_fetch(&before_expiry, &a).map(PublicCover::sha256),
        Some(a.as_str())
    );
    let expired = project(w2, &h, Some(&h.a), cleared(&h.a, 500), 500);
    assert_eq!(expired, CoverProjection::Fallback(CoverFallback::Expired));
    assert_eq!(admit_cover_fetch(&expired, &a), None);
}

/// Quint run: cantos.media.public-cover-projection.run.fetch-withdrawn
#[test]
fn cover_fetch_loses_its_publication_authority_after_withdrawal() {
    use crate::contexts::media::domain::{
        lifecycle::ReleaseCandidate,
        listening::ListenerFacts,
        publication::{
            decide, tests::fixture, AudioRelease, DistributionClearance, PublicationApproval,
            ReleaseId,
        },
    };

    let mut f = fixture();
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
    let image = asset(&f.work, INTENT_A, 'a', 1100);
    let (pinned, set_image) = change(&f.work, Some(&image), 1200);
    f.work = pinned;
    // Bind the existing synthetic audio fixture to the real artwork metadata revision,
    // rebuilding every dependent digest through its production constructor.
    let mut candidate = f.candidate.document().clone();
    candidate.work_metadata_revision = f.work.revision();
    f.candidate = ReleaseCandidate::record(candidate, &f.r).unwrap();
    let mut rights = f.rights.document().clone();
    rights.request.candidate_digest = f.candidate.document().digest.clone();
    f.rights = DistributionClearance::record(rights, &f.candidate).unwrap();
    let mut approval = f.approval.document().clone();
    approval.request.candidate_digest = f.candidate.document().digest.clone();
    approval.request.clearance_digest = f.rights.document().digest.clone();
    f.approval = PublicationApproval::record(approval, &f.candidate).unwrap();
    let release = AudioRelease::publish(
        ReleaseId::parse("80000000-0000-4000-8000-000000000008").unwrap(),
        &f.actor,
        5000,
        None,
        decide(&f.facts()).unwrap(),
    )
    .unwrap();

    let resolve = |withdrawn| {
        ListenerPublication::resolve(ListenerFacts {
            release: &release,
            pinned_work: &f.work,
            pinned_unit: &f.unit,
            current_work: &f.work,
            current_unit: &f.unit,
            rights: Some(&f.rights),
            approval: Some(&f.approval),
            active: true,
            owner_active: true,
            withdrawn,
            submission_revoked: false,
            now: 5000,
        })
        .unwrap()
    };
    let changes = [set_image];
    let project = |publication: &ListenerPublication| {
        project_public_cover(
            publication,
            CoverFacts {
                changes: &changes,
                asset: Some(&image),
                disclosure: cleared(&image, 9000),
                now_ms: 5000,
            },
        )
    };
    let a = "a".repeat(64);
    let eligible = resolve(false).unwrap();
    assert_eq!(eligible.work().revision(), 2);
    assert_eq!(eligible.release().document().id, release.document().id);
    let shown_a = project(&eligible);
    assert_eq!(
        admit_cover_fetch(&shown_a, &a).map(PublicCover::sha256),
        Some(a.as_str())
    );

    // A fresh eligibility read yields no witness; no public projection can be requested.
    // This exercises the native eligibility boundary, not an invented HTTP/storage shell.
    let withdrawn = resolve(true);
    assert!(withdrawn.is_none());
    let after_withdrawal = withdrawn.as_ref().map(project);
    assert_eq!(after_withdrawal, None);
    assert_eq!(
        after_withdrawal
            .as_ref()
            .and_then(|projection| admit_cover_fetch(projection, &a)),
        None
    );
}
