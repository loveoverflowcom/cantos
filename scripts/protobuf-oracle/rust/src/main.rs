use prost::Message;
use serde_json::Value;
mod wire {
    include!(concat!(env!("OUT_DIR"), "/cantos.studio.wire.v1.rs"));
}
mod artwork {
    include!(concat!(env!("OUT_DIR"), "/cantos.studio.artwork.v1.rs"));
}
fn text(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}
fn number(value: &Value, field: &str) -> Option<u64> {
    value.get(field).and_then(Value::as_u64)
}
fn check<M: Message + Default + PartialEq + std::fmt::Debug>(message: M, expected: &[u8]) {
    assert_eq!(message.encode_to_vec(), expected);
    assert_eq!(M::decode(expected).unwrap(), message);
}
fn main() {
    let generated = include_str!(concat!(env!("OUT_DIR"), "/cantos.studio.wire.v1.rs"));
    let reviewed = include_str!("../../../../packages/contracts/api/src/protobuf/generated.rs");
    assert_eq!(
        generated, reviewed,
        "generated prost wire drift: regenerate from reviewed schema, never edit generated types"
    );
    let artwork_generated = include_str!(concat!(env!("OUT_DIR"), "/cantos.studio.artwork.v1.rs"));
    let artwork_reviewed =
        include_str!("../../../../packages/contracts/api/src/protobuf/generated_artwork.rs");
    assert_eq!(
        artwork_generated, artwork_reviewed,
        "generated artwork prost wire drift"
    );
    let path = std::env::args().nth(1).expect("goldens path");
    let corpus: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    for case in cases {
        let value = &case["value"];
        let hex = case["protobuf_hex"].as_str().unwrap();
        let bytes: Vec<u8> = hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        match case["message"].as_str().unwrap() {
            "ValidateScriptRequest" => check(
                wire::ValidateScriptRequest {
                    script_json: text(value, "script_json"),
                },
                &bytes,
            ),
            "SaveRevisionRequest" => check(
                wire::SaveRevisionRequest {
                    expected_revision: number(value, "expected_revision"),
                    operation_id: text(value, "operation_id"),
                    script_json: text(value, "script_json"),
                },
                &bytes,
            ),
            "RevisionResponse" => check(
                wire::RevisionResponse {
                    script_id: text(value, "script_id"),
                    revision: number(value, "revision"),
                    accepted_by: text(value, "accepted_by"),
                    accepted_at: text(value, "accepted_at"),
                    content_digest: text(value, "content_digest"),
                    export_digest: text(value, "export_digest"),
                    script_json: text(value, "script_json"),
                },
                &bytes,
            ),
            "ScriptValidationResponse" => check(
                wire::ScriptValidationResponse {
                    issues: value["issues"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|issue| wire::FieldIssue {
                            path: text(issue, "path"),
                            rule: text(issue, "rule"),
                        })
                        .collect(),
                },
                &bytes,
            ),
            other => panic!("unknown message {other}"),
        }
    }
    let artwork_path = std::env::args()
        .nth(2)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../packages/contracts/fixtures/artwork/v1/goldens.json")
        });
    let artwork_corpus: Value =
        serde_json::from_slice(&std::fs::read(artwork_path).unwrap()).unwrap();
    let artwork_cases = artwork_corpus["cases"].as_array().unwrap();
    for case in artwork_cases {
        let value = case.get("wire_value").unwrap_or(&case["value"]);
        let bytes = from_hex(case["protobuf_hex"].as_str().unwrap());
        match case["message"].as_str().unwrap() {
            "ImageDeclarations" => check(declarations(value), &bytes),
            "CreateImageIntentRequest" => check(create_intent(value), &bytes),
            "ImageIntentDocument" => check(intent(value), &bytes),
            "ImageVariantMetadata" => check(variant(value), &bytes),
            "ImageAssetDocument" => check(asset(value), &bytes),
            "ImageIntentResponse" => check(
                artwork::ImageIntentResponse {
                    intent: nested(value, "intent", intent),
                    asset: nested(value, "asset", asset),
                    state: enumeration(value, "state"),
                },
                &bytes,
            ),
            "WorkArtworkSelection" => check(selection(value), &bytes),
            "ClearArtwork" => check(artwork::ClearArtwork {}, &bytes),
            "SetWorkArtworkRequest" => {
                let change = if value.get("clear_artwork").is_some_and(|arm| !arm.is_null())
                    || value.get("artwork").is_some_and(Value::is_null)
                {
                    Some(artwork::set_work_artwork_request::Change::ClearArtwork(
                        artwork::ClearArtwork {},
                    ))
                } else {
                    nested(value, "artwork", selection)
                        .map(artwork::set_work_artwork_request::Change::Artwork)
                };
                check(
                    artwork::SetWorkArtworkRequest {
                        operation_id: text(value, "operation_id"),
                        expected_work_revision: number(value, "expected_work_revision"),
                        change,
                    },
                    &bytes,
                );
            }
            "WorkArtworkResponse" => check(
                artwork::WorkArtworkResponse {
                    work_id: text(value, "work_id"),
                    work_revision: number(value, "work_revision"),
                    artwork: nested(value, "artwork", selection),
                    changed_at_ms: number(value, "changed_at_ms"),
                },
                &bytes,
            ),
            other => panic!("unknown artwork message {other}"),
        }
    }
    println!(
        "PASS prost 0.13.5 schema oracle: {} pilot + {} artwork vectors",
        cases.len(),
        artwork_cases.len()
    );
}

fn from_hex(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn nested<T>(value: &Value, field: &str, convert: fn(&Value) -> T) -> Option<T> {
    value
        .get(field)
        .filter(|value| !value.is_null())
        .map(convert)
}
fn enumeration(value: &Value, field: &str) -> Option<i32> {
    let value = value.get(field)?;
    if let Some(number) = value.as_i64() {
        return Some(i32::try_from(number).expect("reviewed enum value"));
    }
    value.as_str().map(|name| match name {
        "image/png" | "pending" => 1,
        "image/jpeg" | "admitted" => 2,
        "expired" => 3,
        other => panic!("unknown enum value {other}"),
    })
}
fn declarations(value: &Value) -> artwork::ImageDeclarations {
    artwork::ImageDeclarations {
        provenance: text(value, "provenance"),
        rights_refs: value
            .get("rights_refs")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .map(|value| value.as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default(),
    }
}
fn create_intent(value: &Value) -> artwork::CreateImageIntentRequest {
    artwork::CreateImageIntentRequest {
        operation_id: text(value, "operation_id"),
        id: text(value, "id"),
        expected_sha256: text(value, "expected_sha256"),
        expected_byte_length: number(value, "expected_byte_length"),
        content_type: enumeration(value, "content_type"),
        declarations: nested(value, "declarations", declarations),
    }
}
fn intent(value: &Value) -> artwork::ImageIntentDocument {
    artwork::ImageIntentDocument {
        schema_version: text(value, "schema_version"),
        id: text(value, "id"),
        work_id: text(value, "work_id"),
        owner_id: text(value, "owner_id"),
        operation_id: text(value, "operation_id"),
        request: nested(value, "request", create_intent),
        created_at_ms: number(value, "created_at_ms"),
        expires_at_ms: number(value, "expires_at_ms"),
        digest: text(value, "digest"),
        profile_id: text(value, "profile_id"),
    }
}
fn variant(value: &Value) -> artwork::ImageVariantMetadata {
    artwork::ImageVariantMetadata {
        sha256: text(value, "sha256"),
        byte_length: number(value, "byte_length"),
        content_type: enumeration(value, "content_type"),
        width_px: number(value, "width_px").map(|value| u32::try_from(value).unwrap()),
        height_px: number(value, "height_px").map(|value| u32::try_from(value).unwrap()),
        profile_id: text(value, "profile_id"),
    }
}
fn asset(value: &Value) -> artwork::ImageAssetDocument {
    artwork::ImageAssetDocument {
        schema_version: text(value, "schema_version"),
        id: text(value, "id"),
        intent_id: text(value, "intent_id"),
        work_id: text(value, "work_id"),
        owner_id: text(value, "owner_id"),
        verified_media: nested(value, "verified_media", variant),
        declarations: nested(value, "declarations", declarations),
        admitted_at_ms: number(value, "admitted_at_ms"),
        digest: text(value, "digest"),
    }
}
fn selection(value: &Value) -> artwork::WorkArtworkSelection {
    artwork::WorkArtworkSelection {
        image_asset_id: text(value, "image_asset_id"),
        image_asset_digest: text(value, "image_asset_digest"),
        alt_text: text(value, "alt_text"),
    }
}
