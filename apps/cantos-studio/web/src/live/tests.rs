use super::*;
use cantos_contracts::*;
const ID: &str = "10000000-0000-4000-8000-000000000001";

#[test]
fn interrupted_write_retries_the_exact_snapshot_without_false_success() {
    let command = Command::CreateWork(
        serde_json::from_str(include_str!(
            "../../../../../packages/contracts/fixtures/catalog/v1/create-story.json"
        ))
        .unwrap(),
    );
    let state = RequestState::default().context("owner", "works").unwrap();
    let (sending, ticket) = state.begin(command.clone()).unwrap();
    let failed = sending.finish(ticket, Err(ClientError::Transport));
    assert_eq!(failed.phase, Phase::Ambiguous);
    assert!(failed.result.is_none());
    assert_eq!(
        failed.retry().unwrap().0.pending.unwrap().json().unwrap(),
        command.json().unwrap()
    );
}
#[test]
fn late_response_after_cancel_navigation_or_session_switch_is_ignored() {
    let state = RequestState::default().context("owner", ID).unwrap();
    let (sending, ticket) = state.begin(Command::Work { id: ID.into() }).unwrap();
    let cancelled = sending.cancel().unwrap();
    assert_eq!(
        cancelled.finish(ticket, Err(ClientError::Transport)).phase,
        Phase::Idle
    );
    let changed = sending.context("another-owner", ID).unwrap();
    assert_eq!(
        changed.finish(ticket, Err(ClientError::Transport)).phase,
        Phase::Idle
    );
}
#[test]
fn errors_keep_exact_status_and_current_revision() {
    let error = ClientError::http(
        409,
        br#"{"code":"stale_revision","current_revision":8,"issues":[]}"#,
    );
    assert_eq!(
        error,
        ClientError::Http {
            status: 409,
            error: ApiError {
                code: ErrorCode::StaleRevision,
                current_revision: Some(8),
                issues: vec![]
            }
        }
    );
    assert!(!error.ambiguous());
    assert_eq!(
        ClientError::http(503, b"bad proxy response"),
        ClientError::Protocol { status: 503 }
    );
}
#[test]
fn paths_reject_noncanonical_ids_and_cursor_injection() {
    assert_eq!(
        Command::Work {
            id: "../producer".into()
        }
        .path(),
        Err(ClientError::InvalidInput)
    );
    assert_eq!(
        Command::Works {
            cursor: Some("a&limit=99".into())
        }
        .path(),
        Err(ClientError::InvalidInput)
    );
    assert_eq!(
        Command::Work { id: ID.into() }.path().unwrap(),
        format!("/api/v1/studio/works/{ID}")
    );
}
#[test]
fn upload_is_bounded_credential_separated_and_uses_only_derived_paths() {
    assert_eq!(
        UploadCredential::new("invalid".into()).err(),
        Some(ClientError::InvalidInput)
    );
    assert_eq!(admit_audio_size(0), Err(ClientError::InvalidInput));
    assert_eq!(
        admit_audio_size(MAX_AUDIO_BYTES + 1),
        Err(ClientError::InvalidInput)
    );
    assert_eq!(admit_audio_size(MAX_AUDIO_BYTES), Ok(()));
    let token = UploadCredential::new("a".repeat(64)).unwrap();
    assert!(!format!("{token:?}").contains(&"a".repeat(64)));
    let command = Command::ProducerStatus {
        submission: ID.into(),
        credential: token,
    };
    assert_eq!(command.credentials(), Credentials::Omit);
    assert_eq!(
        command.path().unwrap(),
        format!("/api/v1/studio/producer/audio-submissions/{ID}")
    );
    assert_eq!(
        Command::Work { id: ID.into() }.credentials(),
        Credentials::SameOrigin
    );
}
#[test]
fn all_four_explicit_profiles_and_no_default_are_admitted() {
    for profile in [
        "opus-mono-20-v1",
        "opus-mono-24-v1",
        "opus-mono-32-v1",
        "opus-mono-40-v1",
    ] {
        assert!(valid_profile(profile));
    }
    for profile in ["", "aac", "opus-mono-48-v1"] {
        assert!(!valid_profile(profile));
    }
}

#[test]
fn async_local_input_failure_does_not_corrupt_pending_or_ambiguous_write() {
    let command = Command::CreateWork(
        serde_json::from_str(include_str!(
            "../../../../../packages/contracts/fixtures/catalog/v1/create-story.json"
        ))
        .unwrap(),
    );
    let (pending, ticket) = RequestState::default()
        .context("owner", "works")
        .unwrap()
        .begin(command)
        .unwrap();
    let unchanged = pending.input_failed();
    assert_eq!(unchanged.phase, Phase::Pending);
    assert!(unchanged.accepts(ticket));
    let ambiguous = pending.cancel().unwrap();
    let unchanged = ambiguous.input_failed();
    assert_eq!(unchanged.phase, Phase::Ambiguous);
    assert!(unchanged.retry().is_some());
}

#[test]
fn original_unknown_outcome_survives_retry_auth_denial_and_reauthentication() {
    let command = Command::CreateWork(
        serde_json::from_str(include_str!(
            "../../../../../packages/contracts/fixtures/catalog/v1/create-story.json"
        ))
        .unwrap(),
    );
    let (sending, ticket) = RequestState::default()
        .context("owner", "epoch1")
        .unwrap()
        .begin(command.clone())
        .unwrap();
    let ambiguous = sending.finish(ticket, Err(ClientError::Transport));
    let (retry, ticket) = ambiguous.retry().unwrap();
    let denied = retry.finish(
        ticket,
        Err(ClientError::http(
            401,
            br#"{"code":"unauthenticated","current_revision":null,"issues":[]}"#,
        )),
    );
    assert_eq!(denied.phase, Phase::Ambiguous);
    assert_eq!(
        denied.pending.as_ref().unwrap().json().unwrap(),
        command.json().unwrap()
    );
    let other = denied.context("other", "epoch2").unwrap();
    assert!(!other.can_retry());
    let owner = other.context("owner", "epoch3").unwrap();
    assert!(owner.can_retry());
    assert_eq!(
        owner.pending.unwrap().json().unwrap(),
        command.json().unwrap()
    );
}

#[test]
fn creation_of_a_different_work_clears_all_unit_dependent_facts() {
    let mutation: CatalogMutationResponse = serde_json::from_str(include_str!(
        "../../../../../packages/contracts/fixtures/catalog/v1/mutation.json"
    ))
    .unwrap();
    let initial = Facts::default().apply(Reply::Mutation(mutation.clone()));
    let initial = Facts {
        unit: mutation.units.first().cloned(),
        content_base: Some(4),
        ..initial
    };
    let mut other = mutation;
    other.work.id = "99999999-0000-4000-8000-000000000001".into();
    other.units.clear();
    let updated = initial.apply(Reply::Mutation(other.clone()));
    assert_eq!(updated.work.unwrap().id, other.work.id);
    assert!(updated.unit.is_none());
    assert!(updated.units.is_empty());
    assert!(updated.content_base.is_none());
}

#[test]
fn upload_decodes_the_actual_rendition_dto_not_the_owner_status_dto() {
    let raw = include_bytes!(
        "../../../../../packages/contracts/fixtures/audio-lifecycle/v1/rendition-response.json"
    );
    let rendition: AudioRenditionDocument = serde_json::from_slice(raw).unwrap();
    let command = Command::Upload {
        binding: rendition.binding.clone(),
        sha256: rendition.verified_media.sha256.clone(),
        submission: rendition.submission_id.clone(),
        credential: UploadCredential::new("a".repeat(64)).unwrap(),
        bytes: std::sync::Arc::new(vec![0]),
    };
    assert_eq!(
        command.decode(200, raw).unwrap(),
        Reply::Rendition(Box::new(rendition))
    );
    assert_eq!(
        command.decode(200, b"{}"),
        Err(ClientError::Protocol { status: 200 })
    );
}

#[test]
fn cross_tab_cookie_change_cannot_acknowledge_another_actors_write() {
    let command = Command::CreateWork(
        serde_json::from_str(include_str!(
            "../../../../../packages/contracts/fixtures/catalog/v1/create-story.json"
        ))
        .unwrap(),
    );
    let response: CatalogMutationResponse = serde_json::from_str(include_str!(
        "../../../../../packages/contracts/fixtures/catalog/v1/mutation.json"
    ))
    .unwrap();
    let (sending, ticket) = RequestState::default()
        .context("another-actor", "works")
        .unwrap()
        .begin(command)
        .unwrap();
    let refused = sending.finish(ticket, Ok(Reply::Mutation(response)));
    assert_eq!(refused.phase, Phase::Ambiguous);
    assert_eq!(refused.error, Some(ClientError::SessionChanged));
    assert!(refused.result.is_none());
    assert!(refused.pending.is_some());
}

#[test]
fn session_exchange_blocks_new_commands_before_any_http_effect() {
    let state = RequestState::default().context("owner", "works").unwrap();
    assert!(state
        .begin_in_session(Command::Works { cursor: None }, true)
        .is_none());
    assert!(state
        .begin_in_session(Command::Works { cursor: None }, false)
        .is_some());
}

#[test]
fn only_actual_204_acknowledges_upload_revocation() {
    let command = Command::Revoke {
        unit: ID.into(),
        submission: ID.into(),
    };
    assert_eq!(command.decode(204, b""), Ok(Reply::Revoked));
    assert_eq!(
        command.decode(200, b"{}"),
        Err(ClientError::Protocol { status: 200 })
    );
}

#[test]
fn empty_page_and_failed_read_are_distinct_and_neither_implies_a_save() {
    let (pending, ticket) = RequestState::default()
        .context("owner", "works")
        .unwrap()
        .begin(Command::Works { cursor: None })
        .unwrap();
    let empty = pending.finish(
        ticket,
        Ok(Reply::Works(CatalogPage {
            items: vec![],
            next_cursor: None,
        })),
    );
    assert!(empty.is_empty());
    let failed = pending.finish(ticket, Err(ClientError::Transport));
    assert!(!failed.is_empty());
    assert_eq!(failed.phase, Phase::Failed);
}

fn private_status_with_rendition() -> AudioSubmissionResponse {
    let request: CreateAudioSubmissionRequest = serde_json::from_str(include_str!(
        "../../../../../packages/contracts/fixtures/audio-lifecycle/v1/prepare-recording.json"
    ))
    .unwrap();
    let rendition: AudioRenditionDocument = serde_json::from_str(include_str!(
        "../../../../../packages/contracts/fixtures/audio-lifecycle/v1/rendition-response.json"
    ))
    .unwrap();
    AudioSubmissionResponse {
        submission: AudioSubmissionDocument {
            schema_version: "1.0.0".into(),
            id: request.id,
            operation_id: request.operation_id,
            binding: rendition.binding.clone(),
            expected_sha256: rendition.verified_media.sha256.clone(),
            expected_byte_length: rendition.verified_media.byte_length,
            declarations: request.declarations,
            owner_id: "owner".into(),
            created_at: 0,
            expires_at: request.expires_at,
            upload_path: "derived-by-server".into(),
            digest: "synthetic".into(),
        },
        state: AudioSubmissionState::Revoked,
        rendition: Some(rendition),
        reviews: vec![],
    }
}
#[test]
fn unknown_upload_is_reconciled_by_exact_owner_receipt_after_bearer_revocation() {
    let status = private_status_with_rendition();
    let command = Command::Upload {
        binding: status.submission.binding.clone(),
        sha256: status.submission.expected_sha256.clone(),
        submission: status.submission.id.clone(),
        credential: UploadCredential::new("a".repeat(64)).unwrap(),
        bytes: std::sync::Arc::new(vec![0; status.submission.expected_byte_length as usize]),
    };
    let (pending, ticket) = RequestState::default()
        .context("owner", "unit")
        .unwrap()
        .begin(command)
        .unwrap();
    let unknown = pending.finish(ticket, Err(ClientError::Transport));
    assert!(unknown.can_reconcile());
    let (checking, ticket, read) = unknown.begin_reconcile().unwrap();
    assert_eq!(read.credentials(), Credentials::SameOrigin);
    assert_eq!(read.method(), "GET");
    let reconciled = checking.finish(ticket, Ok(Reply::Submission(Box::new(status.clone()))));
    assert_eq!(reconciled.phase, Phase::Ready);
    assert!(reconciled.pending.is_none());
    let mut mismatch = status;
    mismatch.rendition.as_mut().unwrap().verified_media.sha256 = "other".into();
    let refused = checking.finish(ticket, Ok(Reply::Submission(Box::new(mismatch))));
    assert_eq!(refused.phase, Phase::Ambiguous);
    assert!(refused.pending.is_some());
    assert_eq!(refused.error, Some(ClientError::ReceiptNotEstablished));
}
