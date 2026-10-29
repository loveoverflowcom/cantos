use super::*;
use crate::live::{ClientError, Command, Phase, Reply, RequestState};
fn intent() -> ImageIntentDocument {
    ImageIntentDocument {
        schema_version: "1.0.0".into(),
        id: "00000000-0000-4000-8000-000000000002".into(),
        work_id: "00000000-0000-4000-8000-000000000001".into(),
        owner_id: "alice".into(),
        operation_id: "00000000-0000-4000-8000-000000000003".into(),
        request: CreateImageIntentRequest {
            operation_id: "00000000-0000-4000-8000-000000000003".into(),
            id: "00000000-0000-4000-8000-000000000002".into(),
            expected_sha256: "ab".repeat(32),
            expected_byte_length: 1,
            content_type: ImageMime::Avif,
            declarations: ImageDeclarations {
                provenance: "Original fixture".into(),
                rights_refs: vec!["rights_1".into()],
            },
        },
        created_at_ms: 0,
        expires_at_ms: 1_800_000,
        digest: "intent-digest".into(),
        profile_id: "cover-image-v1".into(),
    }
}
fn asset() -> ImageAssetDocument {
    let i = intent();
    ImageAssetDocument {
        schema_version: i.schema_version,
        id: "00000000-0000-4000-8000-000000000004".into(),
        intent_id: i.id,
        work_id: i.work_id,
        owner_id: i.owner_id,
        verified_media: ImageVariantMetadata {
            sha256: i.request.expected_sha256,
            byte_length: 1,
            content_type: ImageMime::Avif,
            width_px: 1,
            height_px: 1,
            profile_id: "cover-image-v1".into(),
        },
        declarations: i.request.declarations,
        admitted_at_ms: 4,
        digest: "asset-digest".into(),
    }
}
#[test]
fn utf8_limits_and_forbidden_alt_characters_are_exact() {
    let mut d = Draft {
        alt: "ế".repeat(341),
        provenance: "ế".repeat(1365),
        rights: "rights_1".into(),
    };
    assert_eq!(d.alt(), Ok("ế".repeat(341)));
    assert!(d.declarations().is_ok());
    d.alt.push('ế');
    assert_eq!(d.alt(), Err(Problem::Alt));
    d.provenance.push('ế');
    assert_eq!(d.declarations(), Err(Problem::Provenance));
    for c in [
        '\n', '\r', '\t', '\0', '\u{007f}', '\u{0085}', '\u{2028}', '\u{2029}',
    ] {
        d.alt = format!("Ảnh{c}bìa");
        assert_eq!(d.alt(), Err(Problem::Alt));
    }
    d.alt = "  ".into();
    assert_eq!(d.alt(), Err(Problem::Alt));
}
#[test]
fn declarations_reject_duplicates_and_canonicalize_without_erasing_source() {
    let mut d = Draft {
        provenance: "  Bản vẽ gốc\nCó\tquyền  ".into(),
        rights: "b, a".into(),
        alt: "Ảnh bìa".into(),
    };
    assert_eq!(
        d.declarations(),
        Ok(ImageDeclarations {
            provenance: d.provenance.clone(),
            rights_refs: vec!["a".into(), "b".into()]
        })
    );
    for bad in ["", "a,a", "a,", "UPPER", "a/b", "a.b", &"a".repeat(65)] {
        d.rights = bad.into();
        assert_eq!(d.declarations(), Err(Problem::Rights));
    }
    d.rights = (0..64)
        .map(|n| format!("rights_{n}"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(d.declarations().is_ok());
    d.rights.push_str(",extra");
    assert_eq!(d.declarations(), Err(Problem::Rights));
}
#[test]
fn file_bounds_include_exact_limits_and_reject_overflow_dimensions() {
    for size in [1, MAX_BYTES] {
        assert_eq!(file_bounds("image/png", size), Err(Problem::File));
        assert_eq!(file_bounds("image/jpeg", size), Err(Problem::File));
        assert_eq!(file_bounds("image/avif", size), Ok(ImageMime::Avif));
    }
    for size in [0, MAX_BYTES + 1, u64::MAX] {
        assert_eq!(file_bounds("image/png", size), Err(Problem::File));
    }
    assert_eq!(file_bounds("image/svg+xml", 10), Err(Problem::File));
    assert_eq!(dimensions(4096, 4096), Ok(()));
    assert_eq!(dimensions(1, 1), Ok(()));
    for (w, h) in [(0, 1), (1, 0), (4097, 1), (1, 4097), (u32::MAX, u32::MAX)] {
        assert_eq!(dimensions(w, h), Err(Problem::Dimensions));
    }
}
#[test]
fn unknown_prepare_reads_exact_identity_and_cannot_start_another_write() {
    let i = intent();
    let command = Command::PrepareImage {
        work: i.work_id.clone(),
        request: i.request.clone(),
    };
    let initial = RequestState::default().context("alice", "work").unwrap();
    let (pending, ticket) = initial.begin(command.clone()).unwrap();
    let unknown = pending.finish(ticket, Err(ClientError::Transport));
    assert_eq!(unknown.phase(), Phase::Ambiguous);
    assert!(unknown.begin(command).is_none());
    let (reading, ticket, read) = unknown.begin_reconcile().unwrap();
    assert_eq!(read.method(), "GET");
    assert!(read.path().unwrap().ends_with(&i.id));
    let mut wrong = i.clone();
    wrong.request.declarations.provenance = "other".into();
    let still_unknown = reading.finish(
        ticket,
        Ok(Reply::ImageIntent(ImageIntentResponse {
            intent: wrong,
            asset: None,
            state: ImageIntentState::Pending,
        })),
    );
    assert_eq!(still_unknown.phase(), Phase::Ambiguous);
    assert_eq!(
        still_unknown.error(),
        Some(&ClientError::ReceiptNotEstablished)
    );
    let good = reading.finish(
        ticket,
        Ok(Reply::ImageIntent(ImageIntentResponse {
            intent: i,
            asset: None,
            state: ImageIntentState::Pending,
        })),
    );
    assert_eq!(good.phase(), Phase::Ready);
}
#[test]
fn upload_reconciliation_requires_immutable_asset_with_exact_owner_and_bytes() {
    let i = intent();
    let command = Command::UploadImage {
        work: i.work_id.clone(),
        intent: i.clone(),
        bytes: Arc::new(vec![0]),
    };
    let pending = ImageIntentResponse {
        intent: i.clone(),
        asset: None,
        state: ImageIntentState::Pending,
    };
    assert!(!command.reconciles(&Reply::ImageIntent(pending), "alice"));
    let good = ImageIntentResponse {
        intent: i,
        asset: Some(asset()),
        state: ImageIntentState::Admitted,
    };
    assert!(command.reconciles(&Reply::ImageIntent(good.clone()), "alice"));
    assert!(!command.reconciles(&Reply::ImageIntent(good.clone()), "bob"));
    let mut wrong = good;
    wrong.asset.as_mut().unwrap().verified_media.sha256 = "cd".repeat(32);
    assert!(!command.reconciles(&Reply::ImageIntent(wrong), "alice"));
}
#[test]
fn lost_selection_response_replays_original_cas_and_explicit_clear_is_null() {
    let i = intent();
    let request = SetWorkArtworkRequest {
        operation_id: i.operation_id,
        expected_work_revision: 7,
        artwork: None,
    };
    let command = Command::SetArtwork {
        work: i.work_id.clone(),
        request: request.clone(),
    };
    assert_eq!(command.method(), "PATCH");
    assert!(command
        .json()
        .unwrap()
        .unwrap()
        .contains("\"artwork\":null"));
    assert!(command.reconciliation_read().is_none());
    let initial = RequestState::default().context("alice", "work").unwrap();
    let (pending, ticket) = initial.begin(command.clone()).unwrap();
    let unknown = pending.finish(ticket, Err(ClientError::Transport));
    let (replay, ticket) = unknown.retry().unwrap();
    assert_eq!(
        replay.pending().unwrap().json().unwrap(),
        command.json().unwrap()
    );
    let response = WorkArtworkResponse {
        work_id: i.work_id,
        work_revision: 8,
        artwork: None,
        changed_at_ms: Some(100),
    };
    assert_eq!(
        replay.finish(ticket, Ok(Reply::Artwork(response))).phase(),
        Phase::Ready
    );
    let switched = unknown.context("bob", "work").unwrap();
    assert!(!switched.can_retry());
}
#[test]
fn conflict_and_late_reply_do_not_advance_cover_or_lose_draft() {
    let i = intent();
    let initial = RequestState::default().context("alice", "work").unwrap();
    let command = Command::SetArtwork {
        work: i.work_id.clone(),
        request: SetWorkArtworkRequest {
            operation_id: i.operation_id,
            expected_work_revision: 7,
            artwork: None,
        },
    };
    let (pending, ticket) = initial.begin(command).unwrap();
    let error = ClientError::Http {
        status: 409,
        error: ApiError {
            code: ErrorCode::StaleRevision,
            current_revision: Some(9),
            issues: vec![],
        },
    };
    assert_eq!(
        pending.finish(ticket, Err(error.clone())).error(),
        Some(&error)
    );
    let interrupted = pending.cancel().unwrap();
    let r = Reply::Artwork(WorkArtworkResponse {
        work_id: i.work_id,
        work_revision: 8,
        artwork: None,
        changed_at_ms: Some(0),
    });
    assert_eq!(interrupted.finish(ticket, Ok(r)).phase(), Phase::Ambiguous);
    let d = Draft {
        alt: "Ảnh đang thay".into(),
        ..Draft::default()
    };
    assert_eq!(d.alt(), Ok("Ảnh đang thay".into()));
    let model = Projection {
        stage: Stage::Conflict,
        asset: Some(asset()),
        ..Projection::default()
    };
    let refreshed = model.apply(&Reply::Artwork(WorkArtworkResponse {
        work_id: intent().work_id,
        work_revision: 9,
        artwork: None,
        changed_at_ms: None,
    }));
    assert_eq!(refreshed.asset, model.asset);
}

#[test]
fn wrong_work_alt_or_revision_cannot_acknowledge_a_cover_change() {
    let a = asset();
    let selection = WorkArtworkSelection {
        image_asset_id: a.id,
        image_asset_digest: a.digest,
        alt_text: "Ánh đèn".into(),
    };
    let c = Command::SetArtwork {
        work: a.work_id.clone(),
        request: SetWorkArtworkRequest {
            operation_id: intent().operation_id,
            expected_work_revision: 4,
            artwork: Some(selection.clone()),
        },
    };
    let good = WorkArtworkResponse {
        work_id: a.work_id,
        work_revision: 5,
        artwork: Some(selection),
        changed_at_ms: Some(10),
    };
    assert_eq!(c.check_reply(&Reply::Artwork(good.clone())), Ok(()));
    let mut wrong = good.clone();
    wrong.work_id = "00000000-0000-4000-8000-000000000099".into();
    assert_eq!(
        c.check_reply(&Reply::Artwork(wrong)),
        Err(ClientError::ReplyMismatch)
    );
    let mut wrong = good.clone();
    wrong.work_revision = 6;
    assert_eq!(
        c.check_reply(&Reply::Artwork(wrong)),
        Err(ClientError::ReplyMismatch)
    );
    let mut wrong = good;
    wrong.artwork.as_mut().unwrap().alt_text = "Other".into();
    assert_eq!(
        c.check_reply(&Reply::Artwork(wrong)),
        Err(ClientError::ReplyMismatch)
    );
}

#[test]
fn prepare_payload_does_not_send_alt_and_admission_does_not_change_the_cover() {
    let i = intent();
    let c = Command::PrepareImage {
        work: i.work_id.clone(),
        request: i.request.clone(),
    };
    assert_eq!(c.credentials(), crate::live::Credentials::OwnerSession);
    let payload = c.json().unwrap().unwrap();
    assert!(!payload.contains("alt"));
    let current = WorkArtworkResponse {
        work_id: i.work_id,
        work_revision: 7,
        artwork: None,
        changed_at_ms: None,
    };
    let p = Projection {
        current: Some(current.clone()),
        ..Projection::default()
    };
    let admitted = p.apply(&Reply::ImageAdmitted(asset()));
    assert_eq!(admitted.current, Some(current));
    assert_eq!(admitted.stage, Stage::Admitted);
}

#[test]
fn artwork_success_status_must_match_its_contract() {
    let i = intent();
    let command = Command::PrepareImage {
        work: i.work_id.clone(),
        request: i.request.clone(),
    };
    let body = serde_json::to_vec(&i).unwrap();
    assert_eq!(
        command.decode(200, &body),
        Err(ClientError::Protocol { status: 200 })
    );
    assert_eq!(command.decode(201, &body), Ok(Reply::ImagePrepared(i)));
}

#[test]
fn invalid_replacement_retains_prior_bytes_but_cannot_upload_them_as_the_new_pick() {
    let local = LocalImage {
        name: "prior.png".into(),
        bytes: Arc::new(vec![0]),
        sha256: "ab".repeat(32),
        mime: ImageMime::Avif,
        width: 1,
        height: 1,
        url: "blob:fixture".into(),
    };
    let mut projection = Projection {
        local: Some(local.clone()),
        ..Projection::default()
    };
    assert!(projection.file_ready());
    for problem in [Problem::File, Problem::Dimensions, Problem::Decode] {
        projection.problem = Some(problem);
        assert!(!projection.file_ready());
        assert_eq!(projection.local, Some(local.clone()));
    }
    projection.problem = None;
    assert!(projection.file_ready());
}

fn local_image() -> LocalImage {
    LocalImage {
        name: "prior.png".into(),
        bytes: Arc::new(vec![0]),
        sha256: intent().request.expected_sha256,
        mime: ImageMime::Avif,
        width: 1,
        height: 1,
        url: "blob:local-fixture".into(),
    }
}

fn artwork_reply(revision: u64) -> Reply {
    Reply::Artwork(WorkArtworkResponse {
        work_id: intent().work_id,
        work_revision: revision,
        artwork: None,
        changed_at_ms: None,
    })
}

#[test]
fn exact_expiry_settles_unknown_upload_without_admitting_or_losing_local_bytes() {
    let i = intent();
    let local = local_image();
    let command = Command::UploadImage {
        work: i.work_id.clone(),
        intent: i.clone(),
        bytes: local.bytes.clone(),
    };
    let initial = RequestState::default().context("alice", "work").unwrap();
    let (pending, ticket) = initial.begin(command).unwrap();
    let unknown = pending.finish(ticket, Err(ClientError::Transport));
    let (reading, ticket, read) = unknown.begin_reconcile().unwrap();
    assert_eq!(
        read.path(),
        Command::ImageIntent {
            work: i.work_id.clone(),
            intent: i.id.clone(),
        }
        .path()
    );
    let expired = Reply::ImageIntent(ImageIntentResponse {
        intent: i.clone(),
        asset: None,
        state: ImageIntentState::Expired,
    });
    let settled = reading.finish(ticket, Ok(expired.clone()));
    assert_eq!(settled.phase(), Phase::Ready);
    assert_eq!(settled.result(), Some(&expired));
    assert!(settled.pending().is_none());
    assert!(!settled.can_reconcile());
    assert!(!settled.can_retry());
    let model = Projection {
        local: Some(local.clone()),
        stage: Stage::Unknown,
        intent: Some(i.clone()),
        ..Projection::default()
    };
    let resolved = model.apply(settled.result().unwrap());
    assert_eq!(resolved.stage, Stage::Expired);
    assert_eq!(resolved.local, Some(local));
    assert_eq!(resolved.intent, Some(i.clone()));
    assert_eq!(resolved.asset, None);
    assert!(resolved.file_ready());
    assert!(resolved.can_prepare());
    assert!(!resolved.can_upload());
    let (refreshing_state, refresh_ticket) = settled
        .begin(Command::Artwork {
            work: i.work_id.clone(),
        })
        .unwrap();
    let mut refreshing = resolved;
    refreshing.start_request(Stage::Loading, None);
    let refreshed_state = refreshing_state.finish(refresh_ticket, Ok(artwork_reply(8)));
    assert_eq!(refreshed_state.phase(), Phase::Ready);
    let refreshed = refreshing.apply(refreshed_state.result().unwrap());
    assert_eq!(refreshed.intent_state, Some(ImageIntentState::Expired));
    assert!(refreshed.can_prepare());
    assert!(!refreshed.can_upload());
    let mut new_intent = i;
    new_intent.id = "00000000-0000-4000-8000-000000000098".into();
    new_intent.operation_id = "00000000-0000-4000-8000-000000000099".into();
    new_intent.request.id = new_intent.id.clone();
    new_intent.request.operation_id = new_intent.operation_id.clone();
    let (preparing, ticket) = refreshed_state
        .begin(Command::PrepareImage {
            work: new_intent.work_id.clone(),
            request: new_intent.request.clone(),
        })
        .unwrap();
    let prepared = preparing.finish(ticket, Ok(Reply::ImagePrepared(new_intent.clone())));
    assert_eq!(prepared.phase(), Phase::Ready);
    let prepared = refreshed.apply(prepared.result().unwrap());
    assert_eq!(prepared.intent, Some(new_intent));
    assert_eq!(prepared.intent_state, Some(ImageIntentState::Pending));
    assert!(!prepared.can_prepare());
    assert!(prepared.can_upload());
}

#[test]
fn unknown_upload_rejects_pending_foreign_and_inconsistent_terminal_receipts() {
    let i = intent();
    let command = Command::UploadImage {
        work: i.work_id.clone(),
        intent: i.clone(),
        bytes: Arc::new(vec![0]),
    };
    let initial = RequestState::default().context("alice", "work").unwrap();
    let (pending, ticket) = initial.begin(command).unwrap();
    let unknown = pending.finish(ticket, Err(ClientError::Transport));
    let (reading, ticket, _) = unknown.begin_reconcile().unwrap();
    let expired = ImageIntentResponse {
        intent: i,
        asset: None,
        state: ImageIntentState::Expired,
    };
    let mut cases = Vec::new();
    let mut pending = expired.clone();
    pending.state = ImageIntentState::Pending;
    cases.push(pending);
    let mut missing_asset = expired.clone();
    missing_asset.state = ImageIntentState::Admitted;
    cases.push(missing_asset);
    let mut expired_asset = expired.clone();
    expired_asset.asset = Some(asset());
    cases.push(expired_asset);
    let mut foreign = expired.clone();
    foreign.intent.owner_id = "bob".into();
    cases.push(foreign);
    let mut foreign = expired.clone();
    foreign.intent.work_id = "other-work".into();
    cases.push(foreign);
    let mut foreign = expired.clone();
    foreign.intent.id = "other-intent".into();
    cases.push(foreign);
    let mut changed = expired.clone();
    changed.intent.request.expected_sha256 = "cd".repeat(32);
    cases.push(changed);
    let mut changed = expired.clone();
    changed.intent.request.expected_byte_length = 2;
    cases.push(changed);
    let mut changed = expired.clone();
    changed.intent.request.content_type = ImageMime::Jpeg;
    cases.push(changed);
    let mut changed = expired.clone();
    changed.intent.request.declarations.provenance = "Changed".into();
    cases.push(changed);
    let mut changed = expired.clone();
    changed.intent.operation_id = "other-operation".into();
    cases.push(changed);
    let mut changed = expired.clone();
    changed.intent.digest = "changed-digest".into();
    cases.push(changed);
    let mut changed = expired;
    changed.intent.expires_at_ms += 1;
    cases.push(changed);
    for status in cases {
        let unresolved = reading.finish(ticket, Ok(Reply::ImageIntent(status)));
        assert_eq!(unresolved.phase(), Phase::Ambiguous);
        assert_eq!(
            unresolved.error(),
            Some(&ClientError::ReceiptNotEstablished)
        );
        assert!(unresolved.can_reconcile());
        assert!(unresolved
            .begin(Command::Artwork {
                work: intent().work_id
            })
            .is_none());
    }
}

#[test]
fn admitted_receipt_still_settles_upload_when_observed_after_intent_expiry() {
    // No client clock classifies expiry: a server-admitted receipt remains authoritative.
    let i = intent();
    let initial = RequestState::default().context("alice", "work").unwrap();
    let (pending, ticket) = initial
        .begin(Command::UploadImage {
            work: i.work_id.clone(),
            intent: i.clone(),
            bytes: Arc::new(vec![0]),
        })
        .unwrap();
    let unknown = pending.finish(ticket, Err(ClientError::Transport));
    let (reading, ticket, _) = unknown.begin_reconcile().unwrap();
    let receipt = Reply::ImageIntent(ImageIntentResponse {
        intent: i,
        asset: Some(asset()),
        state: ImageIntentState::Admitted,
    });
    let settled = reading.finish(ticket, Ok(receipt.clone()));
    assert_eq!(settled.phase(), Phase::Ready);
    let model = Projection::default().apply(settled.result().unwrap());
    assert_eq!(model.stage, Stage::Admitted);
    assert_eq!(model.asset, Some(asset()));
    assert_eq!(settled.result(), Some(&receipt));
}

#[test]
fn invalid_file_guard_survives_request_start_replies_and_unrelated_form_errors() {
    for problem in [Problem::File, Problem::Dimensions, Problem::Decode] {
        for prepared in [false, true] {
            let local = local_image();
            let mut model = Projection {
                local: Some(local.clone()),
                intent: prepared.then(intent),
                problem: Some(problem),
                ..Projection::default()
            };
            model.start_request(Stage::Loading, None);
            assert_eq!(model.problem, Some(problem));
            for reply in [
                artwork_reply(7),
                Reply::ImagePrepared(intent()),
                Reply::ImageAdmitted(asset()),
                Reply::ImageIntent(ImageIntentResponse {
                    intent: intent(),
                    asset: None,
                    state: ImageIntentState::Expired,
                }),
            ] {
                let mut next = model.apply(&reply);
                next.report_form_problem(Problem::Alt);
                assert_eq!(next.problem, Some(problem));
                assert!(!next.file_ready());
                assert!(!next.can_prepare());
                assert!(!next.can_upload());
                assert_eq!(next.local, Some(local.clone()));
            }
        }
    }
}

#[test]
fn request_start_clears_only_correctable_non_file_errors() {
    for problem in [
        None,
        Some(Problem::Alt),
        Some(Problem::Rights),
        Some(Problem::Provenance),
    ] {
        let mut model = Projection {
            problem,
            ..Projection::default()
        };
        model.start_request(Stage::Preparing, Some("operation".into()));
        assert_eq!(model.problem, None);
        assert_eq!(model.stage, Stage::Preparing);
        assert_eq!(model.operation.as_deref(), Some("operation"));
        model.report_form_problem(Problem::Rights);
        assert_eq!(model.problem, Some(Problem::Rights));
    }
}

#[test]
fn transient_stage_cannot_substitute_for_the_server_intent_fact() {
    for stage in [
        Stage::Prepared,
        Stage::Loading,
        Stage::Replacement,
        Stage::Expired,
    ] {
        for state in [
            None,
            Some(ImageIntentState::Pending),
            Some(ImageIntentState::Admitted),
            Some(ImageIntentState::Expired),
        ] {
            let model = Projection {
                stage,
                local: Some(local_image()),
                intent: Some(intent()),
                intent_state: state,
                ..Projection::default()
            };
            assert_eq!(
                model.can_prepare(),
                state == Some(ImageIntentState::Expired)
            );
            assert_eq!(model.can_upload(), state == Some(ImageIntentState::Pending));
        }
    }
}

#[test]
fn valid_replacement_or_explicit_discard_clears_file_guard_and_old_intent() {
    for problem in [Problem::File, Problem::Dimensions, Problem::Decode] {
        let old = local_image();
        let original = Projection {
            local: Some(old.clone()),
            intent: Some(intent()),
            intent_state: Some(ImageIntentState::Admitted),
            asset: Some(asset()),
            problem: Some(problem),
            ..Projection::default()
        };
        let mut changed = original.clone();
        let replacement = LocalImage {
            name: "valid-new.png".into(),
            ..old.clone()
        };
        assert_eq!(
            changed.replace_local(replacement.clone()),
            Some(old.clone())
        );
        assert_eq!(changed.local, Some(replacement));
        assert_eq!(changed.problem, None);
        assert_eq!(changed.intent, None);
        assert_eq!(changed.intent_state, None);
        assert_eq!(changed.asset, None);
        assert_eq!(changed.stage, Stage::Replacement);
        assert!(changed.file_ready());
        let mut discarded = original;
        assert_eq!(discarded.discard_local(), Some(old));
        assert_eq!(discarded.discard_local(), None);
        assert_eq!(discarded.problem, None);
        assert_eq!(discarded.intent, None);
        assert_eq!(discarded.intent_state, None);
        assert_eq!(discarded.asset, None);
        assert_eq!(discarded.local, None);
        assert_eq!(discarded.stage, Stage::Empty);
        assert!(!discarded.file_ready());
    }
}

#[test]
fn server_replies_preserve_preview_handle_until_lifecycle_retires_it_once() {
    let selected = WorkArtworkSelection {
        image_asset_id: asset().id,
        image_asset_digest: asset().digest,
        alt_text: "Ảnh bìa hiện tại".into(),
    };
    let mut model = Projection {
        current: Some(WorkArtworkResponse {
            work_id: intent().work_id,
            work_revision: 1,
            artwork: Some(selected.clone()),
            changed_at_ms: None,
        }),
        current_url: Some("blob:old-private".into()),
        current_media: Some(asset().verified_media),
        ..Projection::default()
    };
    let mut retired = Vec::new();
    for n in 1..=5 {
        let mut reply = artwork_reply(n);
        if let Reply::Artwork(current) = &mut reply {
            current.artwork = match n {
                1 | 2 => Some(selected.clone()), // Refresh, including an unchanged response.
                3 => Some(WorkArtworkSelection {
                    image_asset_id: "00000000-0000-4000-8000-000000000099".into(),
                    ..selected.clone()
                }), // Confirmed replacement.
                _ => None,                       // Clear and repeated cleared refresh.
            };
        }
        model = model.apply(&reply);
        let expected = if n == 1 {
            "blob:old-private".into()
        } else {
            format!("blob:private-{}", n - 1)
        };
        assert_eq!(model.current_url.as_ref(), Some(&expected));
        retired.push(model.take_current_preview().unwrap());
        assert_eq!(model.current_url, None);
        assert_eq!(model.current_media, None);
        assert_eq!(model.take_current_preview(), None);
        model.current_url = Some(format!("blob:private-{n}"));
        model.current_media = Some(asset().verified_media);
    }
    // Disposal also receives the last handle even if a reply precedes the preview effect.
    model = model.apply(&artwork_reply(6));
    retired.push(model.take_current_preview().unwrap());
    assert_eq!(
        retired,
        vec![
            "blob:old-private",
            "blob:private-1",
            "blob:private-2",
            "blob:private-3",
            "blob:private-4",
            "blob:private-5",
        ]
    );
}

#[test]
fn artwork_named_facts_preserve_literal_braces_and_localize_units() {
    let name = "Lời hẹn {time}.png";
    assert_eq!(
        copy::media_facts(1, 1, 1, "image/png", crate::localization::Locale::English),
        "1 byte · 1 × 1 px · image/png"
    );
    assert_eq!(
        copy::transferred_bytes(0, 1, crate::localization::Locale::English),
        "0 / 1 byte"
    );
    assert_eq!(
        copy::file_facts(
            name,
            128,
            320,
            240,
            "image/png",
            crate::localization::Locale::Vietnamese
        ),
        "Lời hẹn {time}.png · 128 byte · 320 × 240 px · image/png"
    );
    assert_eq!(
        copy::file_facts(
            name,
            128,
            320,
            240,
            "image/png",
            crate::localization::Locale::English
        ),
        "Lời hẹn {time}.png · 128 bytes · 320 × 240 px · image/png"
    );
    assert_eq!(
        copy::media_facts(
            128,
            320,
            240,
            "image/png",
            crate::localization::Locale::Vietnamese
        ),
        "128 byte · 320 × 240 px · image/png"
    );
    assert_eq!(
        copy::media_facts(
            128,
            320,
            240,
            "image/png",
            crate::localization::Locale::English
        ),
        "128 bytes · 320 × 240 px · image/png"
    );
    assert_eq!(
        copy::transferred_bytes(64, 128, crate::localization::Locale::Vietnamese),
        "64 / 128 byte"
    );
    assert_eq!(
        copy::transferred_bytes(64, 128, crate::localization::Locale::English),
        "64 / 128 bytes"
    );
}

#[test]
fn artwork_locale_projection_updates_labels_errors_and_accessible_names_without_editing_draft() {
    use leptos::prelude::*;
    let owner = Owner::new();
    owner.with(|| {
        let original = Draft { alt: "Lời hẹn {time}".into(), provenance: "Bản vẽ gốc".into(), rights: "rights_1".into() };
        let draft = RwSignal::new(original.clone());
        let locale = RwSignal::new(crate::localization::Locale::Vietnamese);
        let model = Projection {
            local: Some(LocalImage { name: "Lời hẹn {time}.png".into(), ..local_image() }),
            problem: Some(Problem::Alt),
            transferred: Some(1),
            ..Projection::default()
        };
        for (en, title, label, error, action) in [
            (crate::localization::Locale::Vietnamese, "Ảnh bìa của tác phẩm", "Mô tả thay thế", "Mô tả không được trống", "Bỏ chọn tệp local"),
            (crate::localization::Locale::English, "Work cover artwork", "Alternative description", "Alt text must be nonblank", "Discard local selection"),
        ] {
            locale.set(en);
            let model = model.clone();
            let html = view! { <form::ArtworkForm model=Signal::derive(move ||model.clone()) draft=draft
                locale=Signal::derive(move ||locale.get()) owner=Signal::derive(||true) blocked=Signal::derive(||false)
                on_intent=Callback::new(|_|{}) on_file=Callback::new(|_|{}) clear_open=RwSignal::new(false)/>
            }.to_html();
            for expected in [title, label, error, action, "Lời hẹn {time}", "Bản vẽ gốc", "rights_1"] {
                assert!(html.contains(expected), "missing {expected}: {html}");
            }
            assert!(html.contains("for=\"artwork-alt\""), "{html}");
            assert!(html.contains("aria-labelledby=\"artwork-title\""), "{html}");
            assert!(html.contains("1 / 1 byte"), "{html}");
            assert_eq!(draft.get_untracked(), original);
        }
    });
}

#[test]
fn ancillary_artwork_edits_revert_and_exact_selection_acknowledgement_preserves_later_input() {
    let mut current = Draft::default();
    assert!(!current.is_dirty(false));
    current.alt = "Bìa mới".into();
    assert!(current.is_dirty(false));
    current.alt.clear();
    assert!(!current.is_dirty(false));
    assert!(current.is_dirty(true));
    current.provenance = "Original fixture".into();
    current.rights = "rights_1".into();
    current.alt = "Bìa mới".into();
    let submitted = current.clone();
    current.alt = "Later alternative".into();
    let later = current.clone();
    assert!(!current.acknowledge_selection(&submitted, true));
    assert_eq!(current, later);
    current = submitted.clone();
    assert!(!current.acknowledge_selection(&submitted, false));
    assert_eq!(current, submitted);
    assert!(current.acknowledge_selection(&submitted, true));
    assert_eq!(current, Draft::default());
    assert!(!current.is_dirty(false));
}

#[test]
fn browser_preview_refuses_legacy_bytes_even_when_the_file_claims_avif() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!("../../../../../../services/cantos-server/src/contexts/media/image_intake/fixtures/legacy-rejection.json")).unwrap();
    for value in vectors.as_object().unwrap().values() {
        let hex = value.as_str().unwrap();
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(avif_bytes(&bytes), Err(Problem::File));
    }
    assert_eq!(avif_bytes(include_bytes!("../../../../../../services/cantos-server/src/contexts/media/image_intake/fixtures/ffmpeg-yuv420p-2x3.avif")), Ok(()));
}

fn selected_artwork() -> WorkArtworkSelection {
    WorkArtworkSelection {
        image_asset_id: asset().id,
        image_asset_digest: asset().digest,
        alt_text: "Ảnh bìa hiện tại".into(),
    }
}

fn with_selection(selection: Option<WorkArtworkSelection>) -> Projection {
    let Reply::Artwork(mut current) = artwork_reply(1) else {
        unreachable!()
    };
    current.artwork = selection;
    Projection {
        current: Some(current),
        stage: Stage::Current,
        ..Projection::default()
    }
}

#[test]
fn cover_view_claims_empty_only_after_a_confirmed_read() {
    // Nothing read yet, a refused read and a non-owner are never "no cover selected".
    assert_eq!(Projection::default().cover_view(true), CoverView::Loading);
    let refused = Projection {
        read_failed: true,
        stage: Stage::Forbidden,
        ..Projection::default()
    };
    assert_eq!(refused.cover_view(true), CoverView::Unavailable);
    assert_eq!(
        Projection::default().cover_view(false),
        CoverView::Unavailable
    );
    // A later successful read clears the failure and may then say empty.
    let confirmed = refused.apply(&artwork_reply(2));
    assert!(!confirmed.read_failed);
    assert_eq!(confirmed.cover_view(true), CoverView::Empty);
    assert_eq!(with_selection(None).cover_view(true), CoverView::Empty);
}

#[test]
fn cover_view_separates_selected_cover_preview_states() {
    let mut model = with_selection(Some(selected_artwork()));
    assert_eq!(model.cover_view(true), CoverView::PreviewLoading);
    model.preview_failed = true;
    assert_eq!(model.cover_view(true), CoverView::PreviewUnavailable);
    model.preview_failed = false;
    assert_eq!(model.cover_view(false), CoverView::PreviewUnavailable);
    model.current_url = Some("blob:private".into());
    assert_eq!(model.cover_view(true), CoverView::Shown);
    // Retiring the preview handle also retires a stale failure so the next fetch starts clean.
    model.preview_failed = true;
    assert_eq!(
        model.take_current_preview().as_deref(),
        Some("blob:private")
    );
    assert!(!model.preview_failed);
}

#[test]
fn current_cover_figure_renders_one_distinct_state_per_view() {
    use leptos::prelude::*;
    let owner = Owner::new();
    owner.with(|| {
        let render = |model: Projection, is_owner: bool, locale| {
            view! { <form::ArtworkForm model=Signal::derive(move ||model.clone()) draft=RwSignal::new(Draft::default())
                locale=Signal::derive(move ||locale) owner=Signal::derive(move ||is_owner) blocked=Signal::derive(||false)
                on_intent=Callback::new(|_|{}) on_file=Callback::new(|_|{}) clear_open=RwSignal::new(false)/>
            }.to_html()
        };
        let vi = crate::localization::Locale::Vietnamese;
        let en = crate::localization::Locale::English;
        let refused = Projection { read_failed: true, stage: Stage::Forbidden, ..Projection::default() };
        let mut shown = with_selection(Some(selected_artwork()));
        shown.current_url = Some("blob:private".into());
        let mut failed = with_selection(Some(selected_artwork()));
        failed.preview_failed = true;
        for (model, is_owner, locale, state, text) in [
            (Projection::default(), true, vi, "loading", "Đang tải ảnh bìa hiện tại"),
            (refused.clone(), true, vi, "unavailable", "nên chưa thể coi là trống"),
            (refused, false, en, "unavailable", "owner access required"),
            (with_selection(None), true, en, "empty", "No cover selected"),
            (with_selection(Some(selected_artwork())), true, vi, "preview-loading", "Đang tải ảnh riêng tư"),
            (failed, true, en, "preview-unavailable", "Private preview unavailable"),
            (shown, true, en, "shown", "Ảnh bìa hiện tại"),
        ] {
            let html = render(model, is_owner, locale);
            assert!(html.contains(&format!("data-state=\"{state}\"")), "{state}: {html}");
            assert!(html.contains(text), "{state} missing {text}: {html}");
            // Neither the figure nor the status badge may claim an empty cover before a confirmed read.
            let claims_empty = html.contains("No cover selected") || html.contains("Chưa có ảnh bìa");
            assert_eq!(claims_empty, state == "empty", "{state}: {html}");
        }
    });
}
