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
            content_type: ImageMime::Png,
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
            content_type: ImageMime::Png,
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
        assert_eq!(file_bounds("image/png", size), Ok(ImageMime::Png));
        assert_eq!(file_bounds("image/jpeg", size), Ok(ImageMime::Jpeg));
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
    assert_eq!(c.credentials(), crate::live::Credentials::SameOrigin);
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
        mime: ImageMime::Png,
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
