use prost::Message;
use serde_json::Value;
mod wire {
    include!(concat!(env!("OUT_DIR"), "/cantos.studio.wire.v1.rs"));
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
    println!("PASS prost 0.13.5 schema oracle: {} vectors", cases.len());
}
