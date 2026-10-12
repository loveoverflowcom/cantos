use super::*;
use crate::contexts::catalog::{content::wire, domain::content::*};
use crate::contexts::production::domain::brief::rights_digest;
const WORK: &str = "10000000-0000-4000-8000-000000000001";
const UNIT: &str = "20000000-0000-4000-8000-000000000002";
const CONTENT: &str = "30000000-0000-4000-8000-000000000003";
const BRIEF: &str = "40000000-0000-4000-8000-000000000004";
const ID: &str = "50000000-0000-4000-8000-000000000005";
const OP: &str = "60000000-0000-4000-8000-000000000006";
fn brief() -> ProductionBrief {
    let content = ContentRevision::new(ContentRevisionFacts {
        id: ContentRevisionId::parse(CONTENT).unwrap(),
        work_id: WorkId::parse(WORK).unwrap(),
        unit_id: ContentUnitId::parse(UNIT).unwrap(),
        revision: 1,
        details: ContentDetails::new(
            "vi-VN".into(),
            ContentBody::Recording,
            vec![],
            "Synthetic recording fixture".into(),
        )
        .unwrap(),
        recorded_by: OwnerId::parse("alice").unwrap(),
        recorded_at: 1000,
    })
    .unwrap();
    let request: dto::CreateProductionBriefRequest = serde_json::from_str(include_str!(
        "../../../../../packages/contracts/fixtures/briefs/v1/create-recording.json"
    ))
    .unwrap();
    ProductionBrief::admit(
        dto::ProductionBriefDocument {
            schema_version: "1".into(),
            id: BRIEF.into(),
            version: 1,
            work: dto::BriefWork {
                id: WORK.into(),
                owner_id: "alice".into(),
                kind: "podcast".into(),
                title: "Tập thử âm thanh".into(),
                rights_refs: vec!["synthetic-rights".into()],
                rights_digest: rights_digest(&["synthetic-rights".into()]).unwrap(),
            },
            content: wire::response(&content).unwrap(),
            instructions: dto::ProductionInstructions::Recording {},
            guidance: request.guidance,
            created_by: "alice".into(),
            created_at: 2000,
            digest: String::new(),
        },
        &content,
    )
    .unwrap()
}
fn request() -> dto::CreateAudioSubmissionRequest {
    dto::CreateAudioSubmissionRequest {
        operation_id: OP.into(),
        id: ID.into(),
        binding: Binding::from_brief(&brief()).unwrap().raw().clone(),
        expected_sha256: "a".repeat(64),
        expected_byte_length: 100,
        declarations: dto::ProducerDeclarations {
            producer: "Synthetic producer".into(),
            method: "recording".into(),
            provenance: "Original owned test signal".into(),
            rights_refs: vec!["synthetic-rights".into()],
            quality_report: "NOT_RUN listening".into(),
        },
        upload_token: "d".repeat(64),
        expires_at: 5000,
    }
}
fn submission() -> AudioSubmission {
    AudioSubmission::create(
        &request(),
        &brief(),
        &OwnerId::parse("alice").unwrap(),
        3000,
    )
    .unwrap()
}
fn rendition() -> AudioRendition {
    AudioRendition::create(
        &submission(),
        dto::AudioMediaMetadata {
            sha256: "a".repeat(64),
            byte_length: 100,
            container: "ogg".into(),
            codec: "opus".into(),
            channels: 1,
            sample_rate_hz: 48000,
            duration_ms: 1000,
            audio_bitrate_bps: 24000,
            profile_id: "opus-mono-24-v1".into(),
        },
        AudioRenditionId::parse(OP).unwrap(),
        4000,
    )
    .unwrap()
}
fn review(kind: dto::AudioReviewKind) -> dto::AudioReviewDocument {
    let r = rendition();
    record_review(
        dto::AudioReviewDocument {
            id: ID.into(),
            rendition_id: r.document().id.clone(),
            operation_id: OP.into(),
            sequence: 1,
            reviewer_id: "alice".into(),
            reviewed_at: 4500,
            request: dto::CreateAudioReviewRequest {
                operation_id: OP.into(),
                rendition_digest: r.document().digest.clone(),
                sha256: "a".repeat(64),
                kind,
                decision: dto::AudioReviewDecision::Approved,
                notes: "Synthetic approval; no listening claim".into(),
                evidence_refs: vec!["fixture-only".into()],
            },
            digest: String::new(),
        },
        &r,
    )
    .unwrap()
}
#[test]
fn every_binding_field_and_rights_declaration_is_exact() {
    let base = request();
    for index in 0..8 {
        let mut r = base.clone();
        match index {
            0 => r.binding.work_id = ID.into(),
            1 => r.binding.unit_id = ID.into(),
            2 => r.binding.content_revision_id = ID.into(),
            3 => r.binding.content_revision_digest = format!("cc-r1:sha256:{}", "b".repeat(64)),
            4 => r.binding.brief_id = ID.into(),
            5 => r.binding.brief_version += 1,
            6 => r.binding.brief_digest = format!("pb-r1:sha256:{}", "b".repeat(64)),
            7 => r.binding.profile_id = "opus-mono-40-v1".into(),
            _ => unreachable!(),
        }
        assert_eq!(
            AudioSubmission::create(&r, &brief(), &OwnerId::parse("alice").unwrap(), 3000),
            Err(AudioError::BindingMismatch)
        );
    }
    let mut r = base;
    r.declarations.rights_refs.clear();
    assert_eq!(
        AudioSubmission::create(&r, &brief(), &OwnerId::parse("alice").unwrap(), 3000),
        Err(AudioError::InvalidDeclaration)
    );
}
#[test]
fn upload_decision_has_exact_digest_revocation_expiry_stale_and_replay_errors() {
    let s = submission();
    let hash = "a".repeat(64);
    for (now, revoked, current, digest, length, expected) in [
        (
            3000,
            false,
            true,
            hash.as_str(),
            100,
            Ok(AdmissionDecision::Inspect),
        ),
        (
            5000,
            false,
            true,
            hash.as_str(),
            100,
            Err(AudioError::PermissionExpired),
        ),
        (
            3000,
            true,
            true,
            hash.as_str(),
            100,
            Err(AudioError::PermissionRevoked),
        ),
        (
            3000,
            false,
            false,
            hash.as_str(),
            100,
            Err(AudioError::StaleBrief),
        ),
        (
            3000,
            false,
            true,
            "bad",
            100,
            Err(AudioError::DigestMismatch),
        ),
        (
            3000,
            false,
            true,
            hash.as_str(),
            101,
            Err(AudioError::DigestMismatch),
        ),
    ] {
        assert_eq!(
            decide_admission(&s, now, revoked, current, digest, length, None),
            expected
        );
    }
    assert_eq!(
        decide_admission(&s, 4500, false, true, &hash, 100, Some(&rendition())),
        Ok(AdmissionDecision::Replay)
    );
}
#[test]
fn settled_records_reject_tamper_and_review_operation_id_split() {
    let mut raw = submission().document().clone();
    raw.declarations.method = "other".into();
    assert_eq!(
        AudioSubmission::restore(raw),
        Err(AudioError::InvalidDigest)
    );
    let mut r = rendition().document().clone();
    r.verified_media.channels = 2;
    assert_eq!(
        AudioRendition::restore(r, &submission()),
        Err(AudioError::BindingMismatch)
    );
    let mut r = review(dto::AudioReviewKind::Qc);
    r.operation_id = ID.into();
    assert_eq!(
        record_review(r, &rendition()),
        Err(AudioError::BindingMismatch)
    );
}
#[test]
fn json_escape_expansion_cannot_create_an_unpersistable_review_proof() {
    let mut raw = review(dto::AudioReviewKind::Qc);
    raw.request.notes = "\"".repeat(4096);
    raw.request.evidence_refs = (0..64)
        .map(|n| format!("{n:02}{}", "\"".repeat(254)))
        .collect();
    assert_eq!(
        record_review(raw, &rendition()),
        Err(AudioError::InvalidSize)
    );
}

#[test]
fn gate_matches_independent_truth_table_and_never_implies_publication() {
    use dto::{AudioReleaseBlocker as B, AudioReviewKind as K};
    for bits in 0..128 {
        let current = bits & 1 == 0;
        let revoked = bits & 2 != 0;
        let rights = bits & 4 == 0;
        let expired = bits & 64 != 0;
        let reviews = [K::Qc, K::Rights, K::Editorial]
            .into_iter()
            .enumerate()
            .filter(|(i, _)| bits & (8 << i) == 0)
            .map(|(_, k)| review(k))
            .collect::<Vec<_>>();
        let actual = release_blockers(current, revoked, expired, rights, &reviews, None, (1, 1));
        let expected = [
            (!current, B::StaleBrief),
            (revoked, B::UploadPermissionRevoked),
            (expired, B::UploadPermissionExpired),
            (!rights, B::RightsUnrecorded),
            (bits & 8 != 0, B::QcNotApproved),
            (bits & 16 != 0, B::RightsNotApproved),
            (bits & 32 != 0, B::EditorialNotApproved),
            (true, B::PublicDeliveryNotImplemented),
        ]
        .into_iter()
        .filter(|(bad, _)| *bad)
        .map(|(_, b)| b)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
        assert_eq!(actual, expected, "bits={bits}");
    }
}
#[test]
fn latest_rejection_withdrawal_and_changed_candidate_are_blockers() {
    use dto::{AudioReleaseBlocker as B, AudioReviewDecision as D, AudioReviewKind as K};
    let mut reviews = vec![review(K::Qc), review(K::Rights), review(K::Editorial)];
    let r = rendition();
    let mut c = record_candidate(
        dto::ReleaseCandidateDocument {
            id: ID.into(),
            rendition_id: r.document().id.clone(),
            rendition_digest: r.document().digest.clone(),
            binding: r.document().binding.clone(),
            sha256: "a".repeat(64),
            work_metadata_revision: 1,
            unit_metadata_revision: 1,
            review_ids: vec![],
            created_by: "alice".into(),
            created_at: 4500,
            operation_id: OP.into(),
            digest: String::new(),
        },
        &r,
    )
    .unwrap();
    assert_eq!(
        release_blockers(true, false, false, true, &reviews, Some(&c), (2, 1)),
        vec![
            B::CandidateReviewsChanged,
            B::MetadataChanged,
            B::PublicDeliveryNotImplemented
        ]
    );
    c.review_ids = latest_reviews(&reviews)
        .iter()
        .map(|r| r.id.clone())
        .collect();
    for decision in [D::Rejected, D::Withdrawn] {
        let mut later = review(K::Qc);
        later.sequence = 2;
        later.id = OP.into();
        later.request.decision = decision;
        reviews.push(later);
        assert!(
            release_blockers(true, false, false, true, &reviews, Some(&c), (1, 1))
                .contains(&B::QcNotApproved)
        );
        reviews.pop();
    }
}
#[test]
fn explicit_state_event_matrix_rejects_terminal_admission() {
    use dto::AudioSubmissionState as S;
    use AudioError::InvalidTransition as E;
    let expected = [
        [
            Ok(S::Admitted),
            Ok(S::Revoked),
            Ok(S::Expired),
            Ok(S::Stale),
        ],
        [Ok(S::Admitted), Ok(S::Revoked), Err(E), Ok(S::Stale)],
        [Err(E), Ok(S::Revoked), Err(E), Ok(S::Stale)],
        [Err(E), Ok(S::Revoked), Ok(S::Expired), Ok(S::Stale)],
        [Err(E), Ok(S::Stale), Err(E), Ok(S::Stale)],
    ];
    for (row, state) in [
        S::AwaitingBytes,
        S::Admitted,
        S::Revoked,
        S::Expired,
        S::Stale,
    ]
    .into_iter()
    .enumerate()
    {
        for (column, event) in [
            SubmissionEvent::Admit,
            SubmissionEvent::Revoke,
            SubmissionEvent::Expire,
            SubmissionEvent::Invalidate,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                transition(state, event),
                expected[row][column],
                "{state:?} {event:?}"
            );
        }
    }
}
