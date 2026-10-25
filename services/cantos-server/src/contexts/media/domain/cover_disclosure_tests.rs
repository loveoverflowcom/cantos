use crate::contract::studio as dto;

use super::*;
use crate::contexts::catalog::domain::{NewWork, Visibility, WorkDetails, WorkKind};

const WORK: &str = "00000000-0000-0000-0000-000000000001";
const OTHER_WORK: &str = "00000000-0000-0000-0000-000000000009";
const OPERATION: &str = "00000000-0000-0000-0000-000000000002";

fn digest(fill: char) -> String {
    format!("ia-r1:sha256:{}", fill.to_string().repeat(64))
}

fn owned_work(id: &str) -> Work {
    Work::new(NewWork {
        id: WorkId::parse(id).unwrap(),
        kind: WorkKind::Story,
        owner_id: OwnerId::parse("owner").unwrap(),
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

fn raw(kind: dto::CoverDisclosureKind, until: Option<u64>) -> dto::RecordCoverDisclosureRequest {
    dto::RecordCoverDisclosureRequest {
        operation_id: OPERATION.into(),
        image_asset_digest: digest('a'),
        decision: kind,
        valid_until_ms: until,
    }
}

fn facts<'a>(
    work: &'a Work,
    prior: Option<&'a CoverDisclosureRecord>,
    image_bound: bool,
) -> DisclosureFacts<'a> {
    DisclosureFacts {
        work,
        prior,
        image_bound,
        next_sequence: 4,
        now_ms: 1_000,
    }
}

fn applied(decision: CoverDisclosureDecision) -> CoverDisclosureRecord {
    match decision {
        CoverDisclosureDecision::Apply(record) => record,
        CoverDisclosureDecision::Replay(_) => panic!("a first operation applies"),
    }
}

#[test]
fn admission_requires_exact_digest_and_decision_shape() {
    use dto::CoverDisclosureKind::{Cleared, Revoked};
    assert!(RecordCoverDisclosure::admit(&raw(Cleared, Some(2_000))).is_ok());
    assert!(RecordCoverDisclosure::admit(&raw(Revoked, None)).is_ok());
    for (request, expected) in [
        // A clearance is never perpetual by default; a revocation carries no window.
        (raw(Cleared, None), CoverDisclosureError::InvalidDecision),
        (
            raw(Revoked, Some(2_000)),
            CoverDisclosureError::InvalidDecision,
        ),
        (raw(Cleared, Some(0)), CoverDisclosureError::InvalidExpiry),
        (
            raw(Cleared, Some(9_007_199_254_740_992)),
            CoverDisclosureError::InvalidExpiry,
        ),
        (
            raw(Cleared, Some(u64::MAX)),
            CoverDisclosureError::InvalidExpiry,
        ),
    ] {
        assert_eq!(RecordCoverDisclosure::admit(&request), Err(expected));
    }
    for bad in [
        "a".repeat(64),
        format!("wa-r1:sha256:{}", "a".repeat(64)),
        format!("ia-r1:sha256:{}", "A".repeat(64)),
        format!("ia-r1:sha256:{}", "a".repeat(63)),
    ] {
        let mut request = raw(Revoked, None);
        request.image_asset_digest = bad;
        assert_eq!(
            RecordCoverDisclosure::admit(&request),
            Err(ImageError::InvalidDigest.into())
        );
    }
    let mut request = raw(Revoked, None);
    request.operation_id = "not-a-uuid".into();
    assert_eq!(
        RecordCoverDisclosure::admit(&request),
        Err(ImageError::InvalidOperationId.into())
    );
}

#[test]
fn clearance_expiry_is_strictly_after_recording_and_binding_is_required() {
    let work = owned_work(WORK);
    for (until, expected) in [
        (999, Err(CoverDisclosureError::InvalidExpiry)),
        (1_000, Err(CoverDisclosureError::InvalidExpiry)),
        (1_001, Ok(())),
    ] {
        let request =
            RecordCoverDisclosure::admit(&raw(dto::CoverDisclosureKind::Cleared, Some(until)))
                .unwrap();
        assert_eq!(
            decide_cover_disclosure(&request, facts(&work, None, true)).map(|_| ()),
            expected,
            "{until}"
        );
    }
    let revoke =
        RecordCoverDisclosure::admit(&raw(dto::CoverDisclosureKind::Revoked, None)).unwrap();
    assert_eq!(
        decide_cover_disclosure(&revoke, facts(&work, None, false)),
        Err(CoverDisclosureError::UnknownImage)
    );
    let record = applied(decide_cover_disclosure(&revoke, facts(&work, None, true)).unwrap());
    assert_eq!(
        (
            record.sequence(),
            record.recorded_at_ms(),
            record.owner_id().as_str()
        ),
        (4, 1_000, "owner")
    );
    assert_eq!(
        record.as_cover_disclosure(),
        crate::contexts::media::domain::public_cover::CoverDisclosure::Revoked {
            image_asset_digest: &digest('a')
        }
    );
}

#[test]
fn same_operation_replays_and_a_reused_operation_is_refused() {
    let work = owned_work(WORK);
    let request =
        RecordCoverDisclosure::admit(&raw(dto::CoverDisclosureKind::Cleared, Some(5_000))).unwrap();
    let record = applied(decide_cover_disclosure(&request, facts(&work, None, true)).unwrap());
    // Replay happens before time and binding checks: a lost response is recoverable later.
    let mut late = facts(&work, Some(&record), false);
    late.now_ms = 9_000;
    assert_eq!(
        decide_cover_disclosure(&request, late),
        Ok(CoverDisclosureDecision::Replay(record.clone()))
    );
    let other =
        RecordCoverDisclosure::admit(&raw(dto::CoverDisclosureKind::Revoked, None)).unwrap();
    assert_eq!(
        decide_cover_disclosure(&other, facts(&work, Some(&record), true)),
        Err(ImageError::OperationReused.into())
    );
    // The same request body for another Work has another fingerprint.
    let foreign = owned_work(OTHER_WORK);
    assert_eq!(
        decide_cover_disclosure(&request, facts(&foreign, Some(&record), true)),
        Err(ImageError::OperationReused.into())
    );
}

#[test]
fn stored_rows_restore_only_consistent_decisions() {
    let stored = |decision: &str, until: Option<i64>| StoredDisclosure {
        work_id: WORK.into(),
        sequence: 1,
        owner_id: "owner".into(),
        image_asset_digest: digest('a'),
        decision: decision.into(),
        valid_until_ms: until,
        recorded_at_ms: 1_000,
        operation_id: OPERATION.into(),
        fingerprint: format!("image-operation1:sha256:{}", "b".repeat(64)),
    };
    assert!(CoverDisclosureRecord::restore(stored("cleared", Some(1_001))).is_ok());
    assert!(CoverDisclosureRecord::restore(stored("revoked", None)).is_ok());
    for (decision, until) in [
        ("cleared", None),
        ("cleared", Some(1_000)),
        ("revoked", Some(2_000)),
        ("withdrawn", None),
    ] {
        assert_eq!(
            CoverDisclosureRecord::restore(stored(decision, until)),
            Err(CoverDisclosureError::Corrupt),
            "{decision} {until:?}"
        );
    }
    let mut zero = stored("revoked", None);
    zero.sequence = 0;
    assert_eq!(
        CoverDisclosureRecord::restore(zero),
        Err(CoverDisclosureError::Corrupt)
    );
}
