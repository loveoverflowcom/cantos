use cantos_contracts::{ImportCueKind, ImportFormat, ImportOutcome, ImportRequest, ImportResponse};
use cantos_server::contexts::ingestion::extract;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn schema(name: &str) -> jsonschema::Validator {
    let root: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/manuscript/v1.schema.json"
    ))
    .unwrap();
    jsonschema::validator_for(&json!({
        "$schema":"https://json-schema.org/draft/2020-12/schema",
        "$ref":format!("#/$defs/{name}"),
        "$defs":root["$defs"]
    }))
    .unwrap()
}

fn request() -> Value {
    // Independent exact original vector; deliberately retain CRLF and Vietnamese bytes.
    json!({
        "metadata":{
            "operation_id":"00000000-0000-4000-8000-000000000002",
            "file_name":"Chương một.txt","format":"txt","reference":"Synthetic contract sample",
            "rights_holder":null,"permission_evidence":null,"usage_scope":"Private editorial review"
        },
        "original_bytes":[77,198,176,97,32,107,104,225,186,189,32,114,198,161,105,46,13,10]
    })
}

fn receipt() -> Value {
    // Reviewed literal output shape; never generated from the parser or a saved receipt.
    json!({
        "id":"src_00000000000040008000000000000002","imported_by":"creator",
        "recorded_at":"2026-10-10T08:00:00.000001Z",
        "sha256":"a1ec7f966650df77800c32c97ea7e6938614fb6a2d7c2a3aa2c573807defbc93","byte_len":18,
        "metadata":{
            "operation_id":"00000000-0000-4000-8000-000000000002",
            "file_name":"Chương một.txt","format":"txt","reference":"Synthetic contract sample",
            "rights_holder":null,"permission_evidence":null,"usage_scope":"Private editorial review"
        },
        "outcome":{
            "status":"parsed","extraction":{
                "extractor_version":"cantos-import-1",
                "blocks":[{"index":0,"kind":"paragraph","text":"Mưa khẽ rơi.","speaker":null,"scene":null,"cue_kind":null}],
                "warnings":[{"code":"unresolved_speaker","block":0}],"script_json":null
            }
        },
        "original_text":"Mưa khẽ rơi.\r\n"
    })
}

#[test]
fn independent_request_and_receipt_match_schema_without_changing_source_bytes() {
    let request_value = request();
    schema("ImportRequest").validate(&request_value).unwrap();
    let request: ImportRequest = serde_json::from_value(request_value).unwrap();
    let response_value = receipt();
    schema("ImportResponse").validate(&response_value).unwrap();
    let response: ImportResponse = serde_json::from_value(response_value).unwrap();
    assert_eq!(request.original_bytes, "Mưa khẽ rơi.\r\n".as_bytes());
    assert_eq!(
        response.original_text.unwrap().as_bytes(),
        request.original_bytes
    );
    assert_eq!(response.byte_len, 18);
    assert_eq!(
        response.sha256,
        format!("{:x}", Sha256::digest(&request.original_bytes))
    );
    assert_eq!(response.metadata, request.metadata);
    let ImportOutcome::Parsed { extraction } = response.outcome else {
        panic!("fixture must represent parsed source")
    };
    assert_eq!(extraction.blocks[0].text, "Mưa khẽ rơi.");
    assert_eq!(extraction.blocks[0].speaker, None);
    assert_eq!(extraction.script_json, None);
}

#[test]
fn real_extractor_branches_conform_to_the_published_outcome_shape() {
    let validator = schema("ImportOutcome");
    for (bytes, format, expected_status) in [
        ("Mưa khẽ rơi.\r\n".as_bytes(), ImportFormat::Txt, "parsed"),
        (
            "# Cảnh 1\n\nAn: Tôi đã về.\n".as_bytes(),
            ImportFormat::Markdown,
            "parsed",
        ),
        (&b""[..], ImportFormat::Txt, "failed"),
        (&b"\xff"[..], ImportFormat::Txt, "failed"),
        (&b"not a ZIP archive"[..], ImportFormat::Docx, "failed"),
        (&b"{}"[..], ImportFormat::ScriptIr, "failed"),
    ] {
        let outcome = serde_json::to_value(extract(bytes, format)).unwrap();
        assert_eq!(outcome["status"], json!(expected_status));
        validator.validate(&outcome).unwrap();
        let decoded: ImportOutcome = serde_json::from_value(outcome.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), outcome);
    }
}

#[test]
fn unknown_fields_fail_in_schema_and_consumer_at_each_object_boundary() {
    let response_validator = schema("ImportResponse");
    for pointer in [
        "",
        "/metadata",
        "/outcome",
        "/outcome/extraction",
        "/outcome/extraction/blocks/0",
        "/outcome/extraction/warnings/0",
    ] {
        let mut changed = receipt();
        changed.pointer_mut(pointer).unwrap()["forged"] = json!(true);
        assert!(!response_validator.is_valid(&changed), "pointer {pointer}");
        let error = serde_json::from_value::<ImportResponse>(changed).unwrap_err();
        assert!(error.to_string().contains("unknown field `forged`"));
    }
    let mut failed = receipt();
    failed["outcome"] =
        json!({"status":"failed","error":{"code":"invalid_utf8","offset":0,"forged":true}});
    assert!(!response_validator.is_valid(&failed));
    let error = serde_json::from_value::<ImportResponse>(failed).unwrap_err();
    assert!(error.to_string().contains("unknown field `forged`"));
    let request_validator = schema("ImportRequest");
    for pointer in ["", "/metadata"] {
        let mut changed = request();
        changed.pointer_mut(pointer).unwrap()["forged"] = json!(true);
        assert!(!request_validator.is_valid(&changed));
        let error = serde_json::from_value::<ImportRequest>(changed).unwrap_err();
        assert!(error.to_string().contains("unknown field `forged`"));
    }
}

#[test]
fn request_schema_bounds_bytes_and_metadata_without_claiming_byte_string_validation() {
    let validator = schema("ImportRequest");
    for bytes in [json!([]), json!([0, 255])] {
        let mut value = request();
        value["original_bytes"] = bytes;
        validator.validate(&value).unwrap();
        serde_json::from_value::<ImportRequest>(value).unwrap();
    }
    for byte in [json!(-1), json!(256), json!(1.5), json!("77"), Value::Null] {
        let mut value = request();
        value["original_bytes"] = json!([byte]);
        assert!(!validator.is_valid(&value));
        serde_json::from_value::<ImportRequest>(value).unwrap_err();
    }
    let mut value = request();
    value["original_bytes"] = Value::Array(vec![json!(0); 1_048_576]);
    validator.validate(&value).unwrap();
    value["original_bytes"]
        .as_array_mut()
        .unwrap()
        .push(json!(0));
    assert!(!validator.is_valid(&value));
    for (field, limit) in [
        ("file_name", 255),
        ("reference", 2048),
        ("rights_holder", 2048),
        ("permission_evidence", 2048),
        ("usage_scope", 2048),
    ] {
        let mut value = request();
        value["metadata"][field] = json!("a".repeat(limit));
        validator.validate(&value).unwrap();
        value["metadata"][field] = json!("a".repeat(limit + 1));
        assert!(!validator.is_valid(&value), "field {field}");
        for invalid in ["", "a\u{0000}", "a\n", "a\u{0085}"] {
            value["metadata"][field] = json!(invalid);
            assert!(!validator.is_valid(&value), "field {field}");
        }
    }
    // JSON Schema counts scalars, whereas the backend must also enforce UTF-8 bytes.
    let mut multibyte = request();
    multibyte["metadata"]["file_name"] = json!("ế".repeat(255));
    validator.validate(&multibyte).unwrap();
    assert_eq!(
        multibyte["metadata"]["file_name"].as_str().unwrap().len(),
        765
    );
}

#[test]
fn schema_closes_identity_format_block_cue_and_outcome_variants() {
    let validator = schema("ImportResponse");
    for (pointer, invalid) in [
        ("/id", json!("src_../../private")),
        ("/imported_by", json!("another actor")),
        ("/recorded_at", json!("2026-10-10")),
        ("/sha256", json!("a".repeat(63))),
        ("/byte_len", json!(1_048_577)),
        (
            "/metadata/operation_id",
            json!("00000000-0000-4000-8000-00000000000A"),
        ),
        ("/metadata/format", json!("pdf")),
        ("/outcome/status", json!("ready")),
        ("/outcome/extraction/blocks/0/index", json!(-1)),
        (
            "/outcome/extraction/blocks/0/kind",
            json!("guessed_speaker"),
        ),
        ("/outcome/extraction/blocks/0/cue_kind", json!("voice")),
    ] {
        let mut changed = receipt();
        *changed.pointer_mut(pointer).unwrap() = invalid;
        assert!(!validator.is_valid(&changed), "pointer {pointer}");
    }
    for (cue, expected) in [
        (json!(null), None),
        (json!("ambience"), Some(ImportCueKind::Ambience)),
        (json!("music"), Some(ImportCueKind::Music)),
        (json!("sfx"), Some(ImportCueKind::Sfx)),
    ] {
        let mut changed = receipt();
        changed["outcome"]["extraction"]["blocks"][0]["cue_kind"] = cue;
        validator.validate(&changed).unwrap();
        let decoded: ImportResponse = serde_json::from_value(changed).unwrap();
        let ImportOutcome::Parsed { extraction } = decoded.outcome else {
            panic!("parsed receipt required")
        };
        assert_eq!(extraction.blocks[0].cue_kind, expected);
    }
    let mut unknown_cue = receipt();
    unknown_cue["outcome"]["extraction"]["blocks"][0]["cue_kind"] = json!("voice");
    assert!(!validator.is_valid(&unknown_cue));
    let error = serde_json::from_value::<ImportResponse>(unknown_cue).unwrap_err();
    assert!(error.to_string().contains("unknown variant `voice`"));
    for outcome in [
        json!({"status":"parsed"}),
        json!({"status":"failed"}),
        json!({"status":"parsed","extraction":receipt()["outcome"]["extraction"],"error":{"code":"bad"}}),
        json!({"status":"failed","error":{"code":"bad"},"extraction":{}}),
    ] {
        let mut changed = receipt();
        changed["outcome"] = outcome;
        assert!(!validator.is_valid(&changed));
        serde_json::from_value::<ImportResponse>(changed).unwrap_err();
    }
}

#[test]
fn nullable_omission_does_not_create_an_owner_right_or_resolved_speaker() {
    let mut value = receipt();
    value.as_object_mut().unwrap().remove("original_text");
    for field in ["rights_holder", "permission_evidence", "usage_scope"] {
        value["metadata"].as_object_mut().unwrap().remove(field);
    }
    value["outcome"]["extraction"]
        .as_object_mut()
        .unwrap()
        .remove("script_json");
    for field in ["speaker", "scene", "cue_kind"] {
        value["outcome"]["extraction"]["blocks"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
    }
    value["outcome"]["extraction"]["warnings"][0]
        .as_object_mut()
        .unwrap()
        .remove("block");
    schema("ImportResponse").validate(&value).unwrap();
    let response: ImportResponse = serde_json::from_value(value).unwrap();
    assert_eq!(response.metadata.rights_holder, None);
    assert_eq!(response.metadata.permission_evidence, None);
    assert_eq!(response.original_text, None);
    let ImportOutcome::Parsed { extraction } = response.outcome else {
        panic!("parsed response required")
    };
    assert_eq!(extraction.script_json, None);
    assert_eq!(extraction.blocks[0].speaker, None);
    assert_eq!(extraction.blocks[0].scene, None);
    assert_eq!(extraction.blocks[0].cue_kind, None);
    assert_eq!(extraction.warnings[0].block, None);
}
