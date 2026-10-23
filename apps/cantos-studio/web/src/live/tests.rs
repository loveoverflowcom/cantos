use super::*;
use crate::contract::*;
const ID: &str = "10000000-0000-4000-8000-000000000001";

#[test]
fn only_owner_authentication_denials_invalidate_not_producer_or_permission_denial() {
    let rendition: AudioRenditionDocument = serde_json::from_slice(include_bytes!(
        "../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json"
    ))
    .unwrap();
    let producer = Command::Upload {
        binding: rendition.binding,
        sha256: rendition.verified_media.sha256,
        submission: rendition.submission_id,
        credential: UploadCredential::new("a".repeat(64)).unwrap(),
        bytes: std::sync::Arc::new(vec![0]),
    };
    let owner = Command::Works { cursor: None };
    let denied = ClientError::http(
        401,
        br#"{"code":"unauthenticated","current_revision":null,"issues":[]}"#,
    );
    assert_eq!(
        denied.owner_session_denial(&owner).unwrap().code,
        ErrorCode::Unauthenticated
    );
    assert_eq!(denied.owner_session_denial(&producer), None);
    let changed = ClientError::http(
        409,
        br#"{"code":"unauthenticated","current_revision":null,"issues":[]}"#,
    );
    assert_eq!(
        changed.owner_session_denial(&owner).unwrap().code,
        ErrorCode::Unauthenticated
    );
    assert_eq!(changed.owner_session_denial(&producer), None);
    for status in [401, 403, 409, 503] {
        assert_eq!(
            ClientError::http(
                status,
                br#"{"code":"forbidden","current_revision":null,"issues":[]}"#
            )
            .owner_session_denial(&owner),
            None
        );
    }
    // An unknown Upload's reconciliation is an owner-cookie read, unlike direct producer PUT.
    let state = RequestState::default().context("alice", ID).unwrap();
    let (sending, ticket) = state.begin(producer).unwrap();
    let unknown = sending.finish(ticket, Err(ClientError::Transport));
    let (reconciling, ticket, owner_read) = unknown.begin_reconcile().unwrap();
    assert_eq!(owner_read.credentials(), Credentials::OwnerSession);
    assert_eq!(
        denied.owner_session_denial(&owner_read).unwrap().code,
        ErrorCode::Unauthenticated
    );
    let denied_receipt = reconciling.finish(ticket, Err(denied.clone()));
    assert!(denied_receipt.pending().is_some());
    assert_eq!(denied_receipt.actor(), "alice");
    assert_eq!(
        ClientError::http(
            403,
            br#"{"code":"forbidden","current_revision":null,"issues":[]}"#
        )
        .owner_session_denial(&owner),
        None
    );
    assert_eq!(ClientError::Transport.owner_session_denial(&owner), None);
}

#[test]
fn interrupted_write_retries_the_exact_snapshot_without_false_success() {
    let command = Command::CreateWork(
        serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json"
        ))
        .unwrap(),
    );
    let state = RequestState::default().context("owner", "works").unwrap();
    let (sending, ticket) = state.begin(command.clone()).unwrap();
    let failed = sending.finish(ticket, Err(ClientError::Transport));
    assert_eq!(failed.phase(), Phase::Ambiguous);
    assert!(failed.result().is_none());
    assert_eq!(
        failed.retry().unwrap().0.pending().unwrap().json().unwrap(),
        command.json().unwrap()
    );
}
#[test]
fn late_response_after_cancel_navigation_or_session_switch_is_ignored() {
    let state = RequestState::default().context("owner", ID).unwrap();
    let (sending, ticket) = state.begin(Command::Work { id: ID.into() }).unwrap();
    let cancelled = sending.cancel().unwrap();
    assert_eq!(
        cancelled
            .finish(ticket, Err(ClientError::Transport))
            .phase(),
        Phase::Idle
    );
    let changed = sending.context("another-owner", ID).unwrap();
    assert_eq!(
        changed.finish(ticket, Err(ClientError::Transport)).phase(),
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
        Credentials::OwnerSession
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
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json"
        ))
        .unwrap(),
    );
    let (pending, ticket) = RequestState::default()
        .context("owner", "works")
        .unwrap()
        .begin(command)
        .unwrap();
    let unchanged = pending.input_failed();
    assert_eq!(unchanged.phase(), Phase::Pending);
    assert!(unchanged.accepts(ticket));
    let ambiguous = pending.cancel().unwrap();
    let unchanged = ambiguous.input_failed();
    assert_eq!(unchanged.phase(), Phase::Ambiguous);
    assert!(unchanged.retry().is_some());
}

#[test]
fn original_unknown_outcome_survives_retry_auth_denial_and_reauthentication() {
    let command = Command::CreateWork(
        serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json"
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
    assert_eq!(denied.phase(), Phase::Ambiguous);
    assert_eq!(
        denied.pending().as_ref().unwrap().json().unwrap(),
        command.json().unwrap()
    );
    let other = denied.context("other", "epoch2").unwrap();
    assert!(!other.can_retry());
    let owner = other.context("owner", "epoch3").unwrap();
    assert!(owner.can_retry());
    assert_eq!(
        owner.pending().unwrap().json().unwrap(),
        command.json().unwrap()
    );
}

#[test]
fn creation_of_a_different_work_clears_all_unit_dependent_facts() {
    let mutation: CatalogMutationResponse = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/mutation.json"
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
        "../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json"
    );
    let rendition: AudioRenditionDocument = serde_json::from_slice(raw).unwrap();
    let command = Command::Upload {
        binding: rendition.binding.clone(),
        sha256: rendition.verified_media.sha256.clone(),
        submission: rendition.submission_id.clone(),
        credential: UploadCredential::new("a".repeat(64)).unwrap(),
        bytes: std::sync::Arc::new(vec![0; rendition.verified_media.byte_length as usize]),
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
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json"
        ))
        .unwrap(),
    );
    let response: CatalogMutationResponse = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/mutation.json"
    ))
    .unwrap();
    let (sending, ticket) = RequestState::default()
        .context("another-actor", "works")
        .unwrap()
        .begin(command)
        .unwrap();
    let refused = sending.finish(ticket, Ok(Reply::Mutation(response)));
    assert_eq!(refused.phase(), Phase::Ambiguous);
    assert_eq!(refused.error(), Some(&ClientError::SessionChanged));
    assert!(refused.result().is_none());
    assert!(refused.pending().is_some());
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
    assert_eq!(failed.phase(), Phase::Failed);
}

fn private_status_with_rendition() -> AudioSubmissionResponse {
    let request: CreateAudioSubmissionRequest = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/prepare-recording.json"
    ))
    .unwrap();
    let rendition: AudioRenditionDocument = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json"
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
    assert_eq!(read.credentials(), Credentials::OwnerSession);
    assert_eq!(read.method(), "GET");
    let reconciled = checking.finish(ticket, Ok(Reply::Submission(Box::new(status.clone()))));
    assert_eq!(reconciled.phase(), Phase::Ready);
    assert!(reconciled.pending().is_none());
    let mut mismatch = status;
    mismatch.rendition.as_mut().unwrap().verified_media.sha256 = "other".into();
    let refused = checking.finish(ticket, Ok(Reply::Submission(Box::new(mismatch))));
    assert_eq!(refused.phase(), Phase::Ambiguous);
    assert!(refused.pending().is_some());
    assert_eq!(refused.error(), Some(&ClientError::ReceiptNotEstablished));
}

#[test]
fn a_well_formed_reply_for_another_work_never_becomes_ready() {
    let response: CatalogMutationResponse = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/mutation.json"
    ))
    .unwrap();
    let (pending, ticket) = RequestState::default()
        .context("owner", "works")
        .unwrap()
        .begin(Command::Work { id: ID.into() })
        .unwrap();
    let mut other = response.work;
    other.id = "99999999-0000-4000-8000-000000000001".into();
    let rejected = pending.finish(ticket, Ok(Reply::Work(other)));
    assert_eq!(rejected.phase(), Phase::Failed);
    assert_eq!(rejected.error(), Some(&ClientError::ReplyMismatch));
    assert!(rejected.result().is_none());
}

#[test]
fn prepare_acknowledgement_cannot_arm_a_different_upload_intent() {
    let response = private_status_with_rendition();
    let mut request: CreateAudioSubmissionRequest = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/prepare-recording.json"
    ))
    .unwrap();
    request.binding = response.submission.binding.clone();
    request.expected_sha256 = response.submission.expected_sha256.clone();
    request.expected_byte_length = response.submission.expected_byte_length;
    let command = Command::Prepare {
        unit: request.binding.unit_id.clone(),
        request: request.clone(),
    };
    let (pending, ticket) = RequestState::default()
        .context("owner", "unit")
        .unwrap()
        .begin(command.clone())
        .unwrap();
    let mut other = response;
    other.submission.operation_id = "99999999-0000-4000-8000-000000000001".into();
    let rejected = pending.finish(ticket, Ok(Reply::Submission(Box::new(other))));
    assert_eq!(rejected.phase(), Phase::Ambiguous);
    assert_eq!(rejected.error(), Some(&ClientError::ReplyMismatch));
    assert!(rejected.result().is_none());
    assert_eq!(
        rejected.pending().unwrap().json().unwrap(),
        command.json().unwrap()
    );
}

#[test]
fn every_client_failure_has_one_completion_and_never_an_empty_success() {
    let create: CreateWorkRequest = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/create-story.json"
    ))
    .unwrap();
    let cases = [
        (ClientError::InvalidInput, Phase::Failed),
        (ClientError::Cancelled, Phase::Ambiguous),
        (ClientError::SessionChanged, Phase::Ambiguous),
        (ClientError::ReceiptNotEstablished, Phase::Ambiguous),
        (ClientError::ReplyMismatch, Phase::Ambiguous),
        (ClientError::Transport, Phase::Ambiguous),
        (ClientError::RateLimited { seconds: 30 }, Phase::Ambiguous),
        (ClientError::Protocol { status: 200 }, Phase::Ambiguous),
        (
            ClientError::http(
                503,
                br#"{"code":"unavailable","current_revision":null,"issues":[]}"#,
            ),
            Phase::Ambiguous,
        ),
        (
            ClientError::http(
                409,
                br#"{"code":"stale_revision","current_revision":8,"issues":[]}"#,
            ),
            Phase::Failed,
        ),
    ];
    for (error, write_phase) in cases {
        for command in [
            Command::Works { cursor: None },
            Command::CreateWork(create.clone()),
        ] {
            let is_write = command.is_write();
            let initial = RequestState::default().context("owner", "works").unwrap();
            assert_eq!(initial.completion(), None);
            let (pending, ticket) = initial.begin(command.clone()).unwrap();
            assert_eq!(pending.completion(), None);
            assert!(pending.begin(command.clone()).is_none());
            let failed = pending.finish(ticket, Err(error.clone()));
            let expected_phase = if is_write { write_phase } else { Phase::Failed };
            assert_eq!(failed.phase(), expected_phase);
            assert_eq!(failed.completion(), Some(Err(&error)));
            assert_eq!(failed.error(), Some(&error));
            assert_eq!(failed.result(), None);
            assert!(!failed.is_empty());
            assert_eq!(
                failed.pending().is_some(),
                expected_phase == Phase::Ambiguous
            );
            if expected_phase == Phase::Ambiguous {
                assert_eq!(
                    failed.retry().unwrap().0.pending().unwrap().json(),
                    command.json()
                );
            }
        }
    }
}

#[test]
fn late_success_and_failure_cannot_replace_a_newer_completed_read() {
    let initial = RequestState::default().context("owner", "works").unwrap();
    let (old, old_ticket) = initial.begin(Command::Works { cursor: None }).unwrap();
    let (new, new_ticket) = old
        .cancel()
        .unwrap()
        .begin(Command::Works { cursor: None })
        .unwrap();
    let reply = Reply::Works(CatalogPage {
        items: vec![],
        next_cursor: None,
    });
    let completed = new.finish(new_ticket, Ok(reply.clone()));
    assert_eq!(completed.completion(), Some(Ok(&reply)));
    for stale_result in [Ok(reply.clone()), Err(ClientError::Transport)] {
        let unchanged = completed.finish(old_ticket, stale_result);
        assert_eq!(unchanged.completion(), Some(Ok(&reply)));
        assert_eq!(unchanged.phase(), Phase::Ready);
        assert!(unchanged.pending().is_none());
    }
    let (refreshing, _) = completed.begin(Command::Works { cursor: None }).unwrap();
    assert_eq!(refreshing.completion(), None);
    assert_eq!(refreshing.result(), None);
}

#[test]
fn content_and_brief_replies_bind_exact_requested_unit_and_revision() {
    let content: ContentRevisionResponse = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/content/v1/fixtures/text-revision.json"
    ))
    .unwrap();
    let exact = Command::ContentRevision {
        unit: content.unit_id.clone(),
        revision: content.id.clone(),
    };
    assert_eq!(exact.check_reply(&Reply::Content(content.clone())), Ok(()));
    let mut wrong_revision = content.clone();
    wrong_revision.id = ID.into();
    assert_eq!(
        exact.check_reply(&Reply::Content(wrong_revision.clone())),
        Err(ClientError::ReplyMismatch)
    );
    assert_eq!(
        exact.decode(200, &serde_json::to_vec(&wrong_revision).unwrap()),
        Err(ClientError::Protocol { status: 200 })
    );
    let mut wrong_unit = content.clone();
    wrong_unit.unit_id = ID.into();
    assert_eq!(
        Command::ContentHead {
            unit: content.unit_id.clone()
        }
        .check_reply(&Reply::Content(wrong_unit)),
        Err(ClientError::ReplyMismatch)
    );
    assert_eq!(
        exact.check_reply(&Reply::Revoked),
        Err(ClientError::ReplyMismatch)
    );

    let brief: ProductionBriefDocument = serde_json::from_str(include_str!(
        "../../../../../services/cantos-server/contracts/studio/briefs/v1/fixtures/narration-document.json"
    ))
    .unwrap();
    let command = Command::Brief {
        unit: brief.content.unit_id.clone(),
        version: brief.version,
    };
    let reply = ProductionBriefResponse {
        brief,
        freshness: BriefFreshness::Current,
    };
    assert_eq!(
        command.check_reply(&Reply::Brief(Box::new(reply.clone()))),
        Ok(())
    );
    let mut old_version = reply;
    old_version.brief.version += 1;
    assert_eq!(
        command.check_reply(&Reply::Brief(Box::new(old_version))),
        Err(ClientError::ReplyMismatch)
    );
}

#[test]
fn upload_receipts_require_exact_binding_hash_size_and_submission() {
    let status = private_status_with_rendition();
    let rendition = status.rendition.unwrap();
    let command = Command::Upload {
        binding: rendition.binding.clone(),
        sha256: rendition.verified_media.sha256.clone(),
        submission: rendition.submission_id.clone(),
        credential: UploadCredential::new("a".repeat(64)).unwrap(),
        bytes: std::sync::Arc::new(vec![0; rendition.verified_media.byte_length as usize]),
    };
    assert_eq!(
        command.check_reply(&Reply::Rendition(Box::new(rendition.clone()))),
        Ok(())
    );
    let mut changes = vec![rendition.clone(); 4];
    changes[0].submission_id = ID.into();
    changes[1].binding.unit_id = ID.into();
    changes[2].verified_media.sha256 = "b".repeat(64);
    changes[3].verified_media.byte_length += 1;
    for changed in changes {
        assert_eq!(
            command.check_reply(&Reply::Rendition(Box::new(changed.clone()))),
            Err(ClientError::ReplyMismatch)
        );
        assert_eq!(
            command.decode(200, &serde_json::to_vec(&changed).unwrap()),
            Err(ClientError::Protocol { status: 200 })
        );
    }
}

#[test]
fn submission_reads_reject_internally_misbound_renditions() {
    let status = private_status_with_rendition();
    let command = Command::Submission {
        unit: status.submission.binding.unit_id.clone(),
        submission: status.submission.id.clone(),
    };
    assert_eq!(
        command.check_reply(&Reply::Submission(Box::new(status.clone()))),
        Ok(())
    );
    let mut changes = vec![status.clone(); 4];
    changes[0].rendition.as_mut().unwrap().submission_id = ID.into();
    changes[1].rendition.as_mut().unwrap().binding.unit_id = ID.into();
    changes[2].rendition.as_mut().unwrap().verified_media.sha256 = "b".repeat(64);
    changes[3]
        .rendition
        .as_mut()
        .unwrap()
        .verified_media
        .byte_length += 1;
    for changed in changes {
        assert_eq!(
            command.check_reply(&Reply::Submission(Box::new(changed))),
            Err(ClientError::ReplyMismatch)
        );
    }
}

#[test]
fn transport_policy_owner_requests_use_the_cookie_session_and_only_producer_routes_carry_a_bearer()
{
    let rendition: AudioRenditionDocument = serde_json::from_slice(include_bytes!(
        "../../../../../services/cantos-server/contracts/studio/audio-lifecycle/v1/fixtures/rendition-response.json"
    ))
    .unwrap();
    let token = UploadCredential::new("b".repeat(64)).unwrap();
    let producer = [
        Command::ProducerStatus {
            submission: ID.into(),
            credential: token.clone(),
        },
        Command::Upload {
            binding: rendition.binding,
            sha256: rendition.verified_media.sha256,
            submission: rendition.submission_id,
            credential: token,
            bytes: std::sync::Arc::new(vec![0]),
        },
    ];
    let owner = [
        Command::Works { cursor: None },
        Command::Work { id: ID.into() },
        Command::Units {
            work: ID.into(),
            cursor: None,
        },
        Command::ContentHead { unit: ID.into() },
        Command::Artwork { work: ID.into() },
        Command::Delegations {
            work: ID.into(),
            after: None,
        },
        Command::Revoke {
            unit: ID.into(),
            submission: ID.into(),
        },
        Command::RevokeDelegation {
            work: ID.into(),
            grant: ID.into(),
        },
    ];
    for command in &owner {
        // credentials: include + the session fence; never an Authorization header.
        assert_eq!(command.credentials(), Credentials::OwnerSession);
        assert!(command.credentials().includes_cookie());
        assert!(command.producer_credential().is_none());
    }
    for command in &producer {
        // credentials: omit; the producer capability is its own Bearer and never meets the cookie.
        assert_eq!(command.credentials(), Credentials::Omit);
        assert!(!command.credentials().includes_cookie());
        assert!(command.producer_credential().is_some());
    }
}

#[test]
fn a_session_fence_mismatch_is_recognized_only_on_a_409_unauthenticated_reply() {
    let body = |code| {
        serde_json::to_vec(&ApiError {
            code,
            current_revision: None,
            issues: vec![],
        })
        .unwrap()
    };
    assert!(ClientError::http(409, &body(ErrorCode::Unauthenticated)).is_session_fence_mismatch());
    for (status, code) in [
        (401, ErrorCode::Unauthenticated),
        (409, ErrorCode::StaleRevision),
        (409, ErrorCode::CatalogConflict),
        (403, ErrorCode::Forbidden),
    ] {
        assert!(!ClientError::http(status, &body(code)).is_session_fence_mismatch());
    }
    assert!(!ClientError::Transport.is_session_fence_mismatch());
    assert!(!ClientError::SessionChanged.is_session_fence_mismatch());
}

#[test]
fn identity_failures_project_onto_commands_without_signing_out_by_accident() {
    use cantos_web_auth::Failure;
    let code = |error: ClientError| match error {
        ClientError::Http { status, error } => Some((status, error.code)),
        _ => None,
    };
    // 401 is signed out; 403 stays forbidden and is never turned into an expired session.
    assert_eq!(
        code(ClientError::from_identity(Failure::SignedOut)),
        Some((401, ErrorCode::Unauthenticated))
    );
    assert_eq!(
        code(ClientError::from_identity(Failure::Forbidden)),
        Some((403, ErrorCode::Forbidden))
    );
    // A 429 hold keeps its wait, is an unknown outcome for a write and never reads as signed out.
    let held = ClientError::from_identity(Failure::RateLimited(42));
    assert_eq!(held, ClientError::RateLimited { seconds: 42 });
    assert!(held.ambiguous());
    assert!(!held.is_session_fence_mismatch());
    for unknown in [
        Failure::Unavailable,
        Failure::Protocol,
        Failure::CleanupPending,
        Failure::NotFound,
    ] {
        assert_eq!(ClientError::from_identity(unknown), ClientError::Transport);
    }
    for moved in [Failure::Changed, Failure::AccountMismatch] {
        assert_eq!(
            ClientError::from_identity(moved),
            ClientError::SessionChanged
        );
    }
}

#[test]
fn historical_content_never_rewinds_the_head_or_editor_binding() {
    let content = |id: &str, revision: u64, script: &str| ContentRevisionResponse {
        id: id.into(),
        work_id: ID.into(),
        unit_id: ID.into(),
        revision,
        details: ContentDetails {
            language: "vi-VN".into(),
            body: ContentBody::Script {
                script: CatalogScriptRef {
                    id: script.into(),
                    revision,
                    export_digest: "digest".into(),
                },
                source: None,
            },
            rights_refs: vec![],
            provenance_note: "Tác giả".into(),
        },
        content_digest: "digest".into(),
        revision_digest: "digest".into(),
        recorded_by: "alice".into(),
        recorded_at: 0,
    };
    let unit = CatalogContentUnit {
        id: ID.into(),
        work_id: ID.into(),
        kind: "chapter".into(),
        details: UnitDetailsDto {
            title: "Chương một".into(),
            description: String::new(),
            source: None,
            script: None,
            content_digest: None,
        },
        position: 0,
        lifecycle: "draft".into(),
        revision: 1,
        created_at: 0,
        updated_at: 0,
    };
    let facts = Facts {
        unit: Some(unit),
        ..Facts::default()
    };
    let head = content("head", 4, "current-script");
    let current = facts.apply_command(
        &Command::ContentHead { unit: ID.into() },
        Reply::Content(head.clone()),
    );
    assert_eq!(current.content_base, Some(4));
    assert_eq!(
        current
            .editor_script()
            .as_ref()
            .map(|script| script.id.as_str()),
        Some("current-script")
    );
    let inspected = current.apply_command(
        &Command::ContentRevision {
            unit: ID.into(),
            revision: "old".into(),
        },
        Reply::Content(content("old", 1, "historical-script")),
    );
    assert_eq!(inspected.content.as_ref().unwrap().revision, 1);
    assert_eq!(inspected.content_base, Some(4));
    assert_eq!(inspected.content_head, Some(head));
    assert_eq!(
        inspected
            .editor_script()
            .as_ref()
            .map(|script| script.id.as_str()),
        Some("current-script")
    );
}
