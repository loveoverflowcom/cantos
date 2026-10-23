use cantos_server::contract::studio::{
    ImportBlockKind, ImportCueKind, ImportFormat, ImportOutcome, ImportRequest, ImportResponse,
};
use serde_json::{json, Value};

// Synthetic Vietnamese content and hand-authored wire expectations; no parser generates these.
const REQUEST: &str = r#"{
  "metadata": {
    "operation_id": "00000000-0000-4000-8000-000000000002",
    "file_name": "Chương một.txt", "format": "txt", "reference": "Synthetic contract sample",
    "rights_holder": null, "permission_evidence": null, "usage_scope": "Private editorial review"
  },
  "original_bytes": [77,198,176,97,32,107,104,225,186,189,32,114,198,161,105,46,13,10]
}"#;

const PARSED: &str = r#"{
  "id": "src_00000000000040008000000000000002", "imported_by": "creator",
  "recorded_at": "2026-10-10T08:00:00.000001Z",
  "sha256": "a1ec7f966650df77800c32c97ea7e6938614fb6a2d7c2a3aa2c573807defbc93", "byte_len": 18,
  "metadata": {
    "operation_id": "00000000-0000-4000-8000-000000000002",
    "file_name": "Chương một.txt", "format": "txt", "reference": "Synthetic contract sample",
    "rights_holder": null, "permission_evidence": null, "usage_scope": "Private editorial review"
  },
  "outcome": {
    "status": "parsed", "extraction": {
      "extractor_version": "cantos-import-1",
      "blocks": [{"index":0,"kind":"paragraph","text":"Mưa khẽ rơi.","speaker":null,"scene":null,"cue_kind":null}],
      "warnings": [{"code":"unresolved_speaker","block":0}], "script_json": null
    }
  },
  "original_text": "Mưa khẽ rơi.\r\n"
}"#;

const FAILED: &str = r#"{
  "id": "src_00000000000040008000000000000003", "imported_by": "creator",
  "recorded_at": "2026-10-10T08:00:00.000002Z",
  "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "byte_len": 0,
  "metadata": {
    "operation_id": "00000000-0000-4000-8000-000000000003",
    "file_name": "empty.docx", "format": "docx", "reference": "Synthetic empty source",
    "rights_holder": null, "permission_evidence": null, "usage_scope": null
  },
  "outcome": {"status":"failed","error":{"code":"empty_source","offset":null}},
  "original_text": null
}"#;

#[test]
fn consumer_decodes_independent_original_and_both_receipt_branches() {
    let request: ImportRequest = serde_json::from_str(REQUEST).unwrap();
    assert_eq!(request.original_bytes, "Mưa khẽ rơi.\r\n".as_bytes());
    assert_eq!(request.metadata.format, ImportFormat::Txt);
    assert_eq!(request.metadata.rights_holder, None);
    assert_eq!(request.metadata.permission_evidence, None);
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::from_str::<Value>(REQUEST).unwrap()
    );
    for fixture in [PARSED, FAILED] {
        let receipt: ImportResponse = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            serde_json::to_value(&receipt).unwrap(),
            serde_json::from_str::<Value>(fixture).unwrap()
        );
        match receipt.outcome {
            ImportOutcome::Parsed { extraction } => {
                assert_eq!(extraction.blocks[0].kind, ImportBlockKind::Paragraph);
                assert_eq!(extraction.blocks[0].speaker, None);
                assert_eq!(extraction.script_json, None);
                assert_eq!(receipt.original_text.as_deref(), Some("Mưa khẽ rơi.\r\n"));
            }
            ImportOutcome::Failed { error } => {
                assert_eq!(error.code, "empty_source");
                assert_eq!(error.offset, None);
                assert_eq!(receipt.original_text, None);
            }
        }
    }
}

#[test]
fn closed_wire_spellings_have_no_fallback_variants() {
    for (spelling, format) in [
        ("txt", ImportFormat::Txt),
        ("markdown", ImportFormat::Markdown),
        ("docx", ImportFormat::Docx),
        ("script_ir", ImportFormat::ScriptIr),
    ] {
        assert_eq!(
            serde_json::from_value::<ImportFormat>(json!(spelling)).unwrap(),
            format
        );
        assert_eq!(serde_json::to_value(format).unwrap(), json!(spelling));
    }
    for (spelling, kind) in [
        ("paragraph", ImportBlockKind::Paragraph),
        ("dialogue", ImportBlockKind::Dialogue),
        ("narration", ImportBlockKind::Narration),
        ("scene_cue", ImportBlockKind::SceneCue),
        ("sound_cue", ImportBlockKind::SoundCue),
    ] {
        assert_eq!(
            serde_json::from_value::<ImportBlockKind>(json!(spelling)).unwrap(),
            kind
        );
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(spelling));
    }
    for (spelling, kind) in [
        ("ambience", ImportCueKind::Ambience),
        ("music", ImportCueKind::Music),
        ("sfx", ImportCueKind::Sfx),
    ] {
        assert_eq!(
            serde_json::from_value::<ImportCueKind>(json!(spelling)).unwrap(),
            kind
        );
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(spelling));
    }
    let error = serde_json::from_value::<ImportFormat>(json!("pdf")).unwrap_err();
    assert!(error.to_string().contains("unknown variant `pdf`"));
    let error = serde_json::from_value::<ImportBlockKind>(json!("ready_dialogue")).unwrap_err();
    assert!(error
        .to_string()
        .contains("unknown variant `ready_dialogue`"));
    let error = serde_json::from_value::<ImportCueKind>(json!("voice")).unwrap_err();
    assert!(error.to_string().contains("unknown variant `voice`"));
    let mut receipt: Value = serde_json::from_str(PARSED).unwrap();
    receipt["outcome"]["extraction"]["blocks"][0]["cue_kind"] = json!("voice");
    let error = serde_json::from_value::<ImportResponse>(receipt).unwrap_err();
    assert!(error.to_string().contains("unknown variant `voice`"));
}

#[test]
fn request_rejects_extra_fields_and_non_byte_values() {
    for pointer in ["", "/metadata"] {
        let mut request: Value = serde_json::from_str(REQUEST).unwrap();
        request.pointer_mut(pointer).unwrap()["forged_owner"] = json!("another_actor");
        let error = serde_json::from_value::<ImportRequest>(request).unwrap_err();
        assert!(error.to_string().contains("unknown field `forged_owner`"));
    }
    for byte in [json!(-1), json!(256), json!(1.5), json!("77"), Value::Null] {
        let mut request: Value = serde_json::from_str(REQUEST).unwrap();
        request["original_bytes"] = json!([byte]);
        let error = serde_json::from_value::<ImportRequest>(request).unwrap_err();
        assert!(
            error.to_string().contains("invalid type")
                || error.to_string().contains("invalid value")
        );
    }
    for field in ["metadata", "original_bytes"] {
        let mut request: Value = serde_json::from_str(REQUEST).unwrap();
        request.as_object_mut().unwrap().remove(field);
        let error = serde_json::from_value::<ImportRequest>(request).unwrap_err();
        assert!(error
            .to_string()
            .contains(&format!("missing field `{field}`")));
    }
}

#[test]
fn receipt_rejects_unknown_fields_at_every_object_boundary() {
    for (fixture, pointer) in [
        (PARSED, ""),
        (PARSED, "/metadata"),
        (PARSED, "/outcome"),
        (PARSED, "/outcome/extraction"),
        (PARSED, "/outcome/extraction/blocks/0"),
        (PARSED, "/outcome/extraction/warnings/0"),
        (FAILED, "/outcome/error"),
    ] {
        let mut receipt: Value = serde_json::from_str(fixture).unwrap();
        receipt.pointer_mut(pointer).unwrap()["publication_ready"] = json!(true);
        let error = serde_json::from_value::<ImportResponse>(receipt).unwrap_err();
        assert!(error
            .to_string()
            .contains("unknown field `publication_ready`"));
    }
}

#[test]
fn tagged_outcomes_require_their_branch_and_refuse_mixed_receipts() {
    for outcome in [
        json!({"status":"ready"}),
        json!({"status":"parsed","error":{"code":"bad","offset":null}}),
        json!({"status":"failed","extraction":{"blocks":[]}}),
        json!({"status":"failed","error":{"code":"bad","offset":null},"extraction":{}}),
    ] {
        let error = serde_json::from_value::<ImportOutcome>(outcome).unwrap_err();
        assert!(
            error.to_string().contains("unknown variant")
                || error.to_string().contains("unknown field")
                || error.to_string().contains("missing field")
        );
    }
}

#[test]
fn nullable_absence_keeps_unknown_facts_and_serialization_is_explicit() {
    let mut request: Value = serde_json::from_str(REQUEST).unwrap();
    for field in ["rights_holder", "permission_evidence", "usage_scope"] {
        request["metadata"].as_object_mut().unwrap().remove(field);
    }
    let request: ImportRequest = serde_json::from_value(request).unwrap();
    assert_eq!(request.metadata.rights_holder, None);
    let encoded = serde_json::to_value(request).unwrap();
    for field in ["rights_holder", "permission_evidence", "usage_scope"] {
        assert_eq!(encoded["metadata"][field], Value::Null);
    }
    let parsed: ImportOutcome = serde_json::from_value(json!({
        "status":"parsed",
        "extraction":{"extractor_version":"cantos-import-1","blocks":[{"index":0,"kind":"dialogue","text":"— Ai đó?"}],"warnings":[{"code":"unresolved_speaker"}]}
    })).unwrap();
    let ImportOutcome::Parsed { extraction } = parsed else {
        panic!("parsed branch required")
    };
    assert_eq!(extraction.blocks[0].speaker, None);
    assert_eq!(extraction.blocks[0].scene, None);
    assert_eq!(extraction.blocks[0].cue_kind, None);
    assert_eq!(extraction.warnings[0].block, None);
    assert_eq!(extraction.script_json, None);
}
