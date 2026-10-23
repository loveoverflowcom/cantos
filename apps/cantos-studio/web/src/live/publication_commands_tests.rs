//! Publication transport correlation and exact replay, without a Server or delivery fake.
use super::*;
use crate::contract::*;

const UNIT: &str = "20000000-0000-4000-8000-000000000002";
const CANDIDATE: &str = "60000000-0000-4000-8000-000000000006";
const RELEASE: &str = "90000000-0000-4000-8000-000000000001";
const OTHER: &str = "90000000-0000-4000-8000-000000000009";
const OWNER: &str = "fixture-owner";

macro_rules! fixture {
    ($path:literal) => {
        include_str!(concat!(
            "../../../../../services/cantos-server/contracts/studio/publication/v1/fixtures/",
            $path
        ))
    };
}
fn rights_request() -> RecordDistributionRightsRequest {
    serde_json::from_str(fixture!("clearance.json")).unwrap()
}
fn approval_request() -> RecordPublicationApprovalRequest {
    serde_json::from_str(fixture!("approval.json")).unwrap()
}
fn publish_request() -> PublishAudioReleaseRequest {
    serde_json::from_str(fixture!("publish.json")).unwrap()
}
fn clearance() -> DistributionClearanceDocument {
    DistributionClearanceDocument {
        id: "80000000-0000-4000-8000-000000000005".into(),
        candidate_id: CANDIDATE.into(),
        actor_id: OWNER.into(),
        sequence: 1,
        recorded_at: 2000,
        request: rights_request(),
        digest: approval_request().clearance_digest,
    }
}
fn approval() -> PublicationApprovalDocument {
    PublicationApprovalDocument {
        id: "80000000-0000-4000-8000-000000000006".into(),
        candidate_id: CANDIDATE.into(),
        actor_id: OWNER.into(),
        sequence: 1,
        recorded_at: 3000,
        request: approval_request(),
        digest: format!("pa-r1:sha256:{}", "c".repeat(64)),
    }
}
fn readiness() -> PublicationReadinessResponse {
    PublicationReadinessResponse {
        candidate_id: CANDIDATE.into(),
        candidate_digest: publish_request().candidate_digest,
        pointer_version: 0,
        active_release_id: None,
        blockers: vec![PublicationBlocker::DeliveryUnavailable],
        clearance: Some(clearance()),
        approval: Some(approval()),
    }
}
fn release_response() -> ActiveAudioReleaseResponse {
    let rendition: AudioRenditionDocument = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json"
    )).unwrap();
    ActiveAudioReleaseResponse {
        pointer_version: 1,
        release: Some(AudioReleaseDocument {
            id: RELEASE.into(),
            published_by: OWNER.into(),
            published_at: 4000,
            supersedes: None,
            manifest: ReleaseManifestDocument {
                schema_version: "1.0.0".into(),
                work_id: rendition.binding.work_id.clone(),
                unit_id: UNIT.into(),
                work_metadata_revision: 2,
                unit_metadata_revision: 1,
                work_title: "Truyện tổng hợp".into(),
                unit_title: "Chương một".into(),
                language: "vi-VN".into(),
                rights_refs: vec!["synthetic-rights".into()],
                candidate_id: CANDIDATE.into(),
                candidate_digest: publish_request().candidate_digest,
                rendition_id: rendition.id,
                rendition_digest: rendition.digest,
                binding: rendition.binding,
                media: rendition.verified_media,
                review_evidence: vec![],
                distribution_clearance: clearance(),
                publication_approval: approval(),
                delivery_object_id: "publication-fixture-identity".into(),
                retention: ReleaseRetentionPolicy::RetainUntilRightsExpiryOrWithdrawal,
                digest: format!("rm-r1:sha256:{}", "d".repeat(64)),
            },
            digest: format!("rl-r1:sha256:{}", "e".repeat(64)),
        }),
    }
}
fn publish() -> Command {
    Command::Publish {
        unit: UNIT.into(),
        candidate: CANDIDATE.into(),
        request: publish_request(),
    }
}
fn read() -> Command {
    Command::Readiness {
        unit: UNIT.into(),
        candidate: CANDIDATE.into(),
        candidate_digest: publish_request().candidate_digest,
    }
}
fn encoded<T: serde::Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

#[test]
fn publication_routes_use_exact_studio_owner_paths_methods_and_statuses() {
    let cases = [
        (
            read(),
            "GET",
            format!("release-candidates/{CANDIDATE}/readiness"),
            200,
            encoded(&readiness()),
        ),
        (
            Command::RecordDistributionRights {
                unit: UNIT.into(),
                candidate: CANDIDATE.into(),
                request: rights_request(),
            },
            "POST",
            format!("release-candidates/{CANDIDATE}/distribution-rights"),
            201,
            encoded(&clearance()),
        ),
        (
            Command::RecordPublicationApproval {
                unit: UNIT.into(),
                candidate: CANDIDATE.into(),
                request: approval_request(),
            },
            "POST",
            format!("release-candidates/{CANDIDATE}/publication-approvals"),
            201,
            encoded(&approval()),
        ),
        (
            publish(),
            "POST",
            format!("release-candidates/{CANDIDATE}/publish"),
            201,
            encoded(&release_response()),
        ),
        (
            Command::ActiveRelease { unit: UNIT.into() },
            "GET",
            "publication/active".into(),
            200,
            encoded(&release_response()),
        ),
        (
            Command::Withdraw {
                unit: UNIT.into(),
                request: serde_json::from_str(fixture!("withdraw.json")).unwrap(),
            },
            "POST",
            "publication/withdraw".into(),
            200,
            encoded(&AudioReleaseWithdrawalResponse {
                pointer_version: 2,
                withdrawn_release_id: RELEASE.into(),
            }),
        ),
    ];
    for (command, method, suffix, expected_status, body) in cases {
        assert_eq!(
            command.path().unwrap(),
            format!("/api/v1/studio/content-units/{UNIT}/{suffix}")
        );
        assert_eq!(command.method(), method);
        assert_eq!(command.credentials(), Credentials::OwnerSession);
        assert!(command.producer_credential().is_none());
        assert_eq!(command.is_write(), method == "POST");
        assert_eq!(command.json().unwrap().is_some(), method == "POST");
        assert!(command.decode(expected_status, &body).is_ok());
        let wrong_status = if expected_status == 200 { 201 } else { 200 };
        assert_eq!(
            command.decode(wrong_status, &body),
            Err(ClientError::Protocol {
                status: wrong_status
            })
        );
    }
    let unsafe_path = Command::ActiveRelease {
        unit: "../theatre".into(),
    };
    assert_eq!(unsafe_path.path(), Err(ClientError::InvalidInput));
}

#[test]
fn advisory_readiness_keeps_stale_approval_and_every_server_blocker() {
    let mut status = readiness();
    status.approval.as_mut().unwrap().request.clearance_digest = "older-clearance".into();
    status.blockers = vec![
        PublicationBlocker::ApprovalStale,
        PublicationBlocker::DeliveryUnavailable,
    ];
    assert_eq!(
        read().decode(200, &encoded(&status)),
        Ok(Reply::Readiness(Box::new(status.clone())))
    );
    status.candidate_id = OTHER.into();
    assert_eq!(
        read().check_reply(&Reply::Readiness(Box::new(status.clone()))),
        Err(ClientError::ReplyMismatch)
    );
    status.candidate_id = CANDIDATE.into();
    status.clearance.as_mut().unwrap().candidate_id = OTHER.into();
    assert_eq!(
        read().check_reply(&Reply::Readiness(Box::new(status))),
        Err(ClientError::ReplyMismatch)
    );
}

#[test]
fn rights_and_approval_acknowledgements_pin_the_exact_operation_and_request() {
    let mut request = rights_request();
    request.rights_refs = vec!["z-rights".into(), "a-rights".into()];
    request.evidence_refs = vec!["z-evidence".into(), "a-evidence".into()];
    let command = Command::RecordDistributionRights {
        unit: UNIT.into(),
        candidate: CANDIDATE.into(),
        request: request.clone(),
    };
    let mut record = clearance();
    record.request = request;
    record.request.rights_refs.reverse();
    record.request.evidence_refs.reverse();
    assert_eq!(
        command.check_reply(&Reply::DistributionRights(record.clone())),
        Ok(())
    );
    record.request.operation_id = OTHER.into();
    assert_eq!(
        command.check_reply(&Reply::DistributionRights(record)),
        Err(ClientError::ReplyMismatch)
    );

    let command = Command::RecordPublicationApproval {
        unit: UNIT.into(),
        candidate: CANDIDATE.into(),
        request: approval_request(),
    };
    let mut record = approval();
    assert_eq!(
        command.check_reply(&Reply::PublicationApproval(record.clone())),
        Ok(())
    );
    record.request.clearance_digest = "different-clearance".into();
    assert_eq!(
        command.check_reply(&Reply::PublicationApproval(record)),
        Err(ClientError::ReplyMismatch)
    );
}

#[test]
fn publication_write_acknowledgements_cannot_cross_an_actor() {
    let replies = [
        (
            Command::RecordDistributionRights {
                unit: UNIT.into(),
                candidate: CANDIDATE.into(),
                request: rights_request(),
            },
            Reply::DistributionRights(clearance()),
        ),
        (
            Command::RecordPublicationApproval {
                unit: UNIT.into(),
                candidate: CANDIDATE.into(),
                request: approval_request(),
            },
            Reply::PublicationApproval(approval()),
        ),
        (
            publish(),
            Reply::ActiveRelease(Box::new(release_response())),
        ),
    ];
    for (command, reply) in replies {
        let state = RequestState::default()
            .context("other-owner", "workspace")
            .unwrap();
        let (sending, ticket) = state.begin(command).unwrap();
        let settled = sending.finish(ticket, Ok(reply));
        assert_eq!(settled.phase(), Phase::Ambiguous);
        assert_eq!(settled.error(), Some(&ClientError::SessionChanged));
        assert!(settled.result().is_none());
    }
}

#[test]
fn publish_response_requires_the_next_pointer_and_exact_release_binding() {
    let valid = release_response();
    assert_eq!(
        publish().check_reply(&Reply::ActiveRelease(Box::new(valid.clone()))),
        Ok(())
    );
    let mut wrong_pointer = valid.clone();
    wrong_pointer.pointer_version = 2;
    let mut empty = valid.clone();
    empty.release = None;
    let mut wrong_unit = valid.clone();
    wrong_unit.release.as_mut().unwrap().manifest.unit_id = OTHER.into();
    let mut wrong_digest = valid.clone();
    wrong_digest
        .release
        .as_mut()
        .unwrap()
        .manifest
        .candidate_digest = "another-candidate".into();
    let mut wrong_clearance = valid.clone();
    wrong_clearance
        .release
        .as_mut()
        .unwrap()
        .manifest
        .publication_approval
        .request
        .clearance_digest = "another-clearance".into();
    let mut wrong_prior = valid;
    wrong_prior.release.as_mut().unwrap().supersedes = Some(OTHER.into());
    for response in [
        wrong_pointer,
        empty,
        wrong_unit,
        wrong_digest,
        wrong_clearance,
        wrong_prior,
    ] {
        assert_eq!(
            publish().check_reply(&Reply::ActiveRelease(Box::new(response))),
            Err(ClientError::ReplyMismatch)
        );
    }
}

#[test]
fn lost_publish_replays_its_original_operation_instead_of_reading_the_current_pointer() {
    let command = publish();
    let expected_json = command.json().unwrap();
    let state = RequestState::default().context(OWNER, "workspace").unwrap();
    let (sending, ticket) = state.begin(command).unwrap();
    let unknown = sending.finish(ticket, Err(ClientError::Transport));
    assert_eq!(unknown.phase(), Phase::Ambiguous);
    assert!(!unknown.can_reconcile());
    assert!(unknown
        .begin(Command::ActiveRelease { unit: UNIT.into() })
        .is_none());
    let (replay, ticket) = unknown.retry().unwrap();
    assert_eq!(replay.pending().unwrap().json().unwrap(), expected_json);
    // A durable replay can return its original version-one receipt after later withdrawal.
    let settled = replay.finish(
        ticket,
        Ok(Reply::ActiveRelease(Box::new(release_response()))),
    );
    assert_eq!(settled.phase(), Phase::Ready);
    assert_eq!(
        settled.result(),
        Some(&Reply::ActiveRelease(Box::new(release_response())))
    );
    let switched = unknown.context("other-owner", "workspace").unwrap();
    assert!(!switched.can_retry());
}

#[test]
fn active_release_absence_is_explicit_and_present_release_must_match_the_unit() {
    let command = Command::ActiveRelease { unit: UNIT.into() };
    let absent = ActiveAudioReleaseResponse {
        pointer_version: 4,
        release: None,
    };
    assert_eq!(
        command.decode(200, &encoded(&absent)),
        Ok(Reply::ActiveRelease(Box::new(absent)))
    );
    let mut response = release_response();
    response.release.as_mut().unwrap().manifest.binding.unit_id = OTHER.into();
    assert_eq!(
        command.check_reply(&Reply::ActiveRelease(Box::new(response))),
        Err(ClientError::ReplyMismatch)
    );
}

#[test]
fn withdrawal_receipt_identifies_the_tombstone_without_claiming_an_empty_active_pointer() {
    let command = Command::Withdraw {
        unit: UNIT.into(),
        request: serde_json::from_str(fixture!("withdraw.json")).unwrap(),
    };
    for version in [1, 2] {
        assert_eq!(
            command.check_reply(&Reply::Withdrawal(AudioReleaseWithdrawalResponse {
                pointer_version: version,
                withdrawn_release_id: RELEASE.into()
            })),
            Ok(())
        );
    }
    for receipt in [
        AudioReleaseWithdrawalResponse {
            pointer_version: 3,
            withdrawn_release_id: RELEASE.into(),
        },
        AudioReleaseWithdrawalResponse {
            pointer_version: 2,
            withdrawn_release_id: OTHER.into(),
        },
    ] {
        assert_eq!(
            command.check_reply(&Reply::Withdrawal(receipt)),
            Err(ClientError::ReplyMismatch)
        );
    }
}
