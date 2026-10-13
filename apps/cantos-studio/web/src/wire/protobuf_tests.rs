//! Host evidence for the real pilot adapter; browser fetch/cancellation is a separate gate.
use cantos_contracts::{
    FieldIssue, RevisionResponse, SaveRevisionRequest, ScriptValidationResponse,
    ValidateScriptRequest,
};
use serde_json::Value;

use super::*;
use crate::editor::{Editor, Status};

fn revision() -> RevisionResponse {
    serde_json::from_str(include_str!(
        "../../../../../packages/contracts/fixtures/studio/v1/revision.json"
    ))
    .unwrap()
}

fn stale() -> &'static [u8] {
    include_bytes!("../../../../../packages/contracts/fixtures/studio/v1/stale-revision.json")
}

fn golden_cases() -> Vec<Value> {
    serde_json::from_str::<Value>(include_str!(
        "../../../../../packages/contracts/fixtures/protobuf/v1/goldens.json"
    ))
    .unwrap()["cases"]
        .as_array()
        .unwrap()
        .clone()
}

fn golden_bytes(case: &Value) -> Vec<u8> {
    case["protobuf_hex"]
        .as_str()
        .unwrap()
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn check_golden<T>(case: &Value)
where
    T: Serialize + DeserializeOwned + ProtobufDto + PartialEq + std::fmt::Debug,
{
    let bytes = golden_bytes(case);
    let actual = decode_response::<T>(
        WireFormat::Protobuf,
        true,
        Some("application/protobuf"),
        &bytes,
    );
    if case["application_valid"] == false {
        assert_eq!(actual, Err(unavailable()), "{}", case["name"]);
        return;
    }
    let expected: T = serde_json::from_value(case["value"].clone()).unwrap();
    assert_eq!(actual, Ok(expected), "{}", case["name"]);
    let value: T = serde_json::from_value(case["value"].clone()).unwrap();
    assert_eq!(
        encode_body(WireFormat::Protobuf, &value).unwrap(),
        bytes,
        "independent Python bytes: {}",
        case["name"]
    );
}

#[test]
fn all_four_pilot_dtos_match_independent_goldens_with_presence_and_exact_text() {
    let cases = golden_cases();
    assert_eq!(cases.len(), 13);
    for case in &cases {
        match case["message"].as_str().unwrap() {
            "ValidateScriptRequest" => check_golden::<ValidateScriptRequest>(case),
            "SaveRevisionRequest" => check_golden::<SaveRevisionRequest>(case),
            "RevisionResponse" => check_golden::<RevisionResponse>(case),
            "ScriptValidationResponse" => check_golden::<ScriptValidationResponse>(case),
            unknown => panic!("unreviewed pilot golden: {unknown}"),
        }
    }
}

#[test]
fn protobuf_offers_json_and_actual_json_response_header_selects_json() {
    assert_eq!(
        accept(WireFormat::Protobuf),
        "application/protobuf, application/json;q=0.5"
    );
    let expected = revision();
    let json = wire::encode(WireFormat::Json, &expected).unwrap();
    assert_eq!(
        decode_response::<RevisionResponse>(
            WireFormat::Protobuf,
            true,
            Some("application/json; charset=utf-8"),
            &json,
        ),
        Ok(expected)
    );
}

#[test]
fn valid_protobuf_response_aliases_and_parameters_decode_without_sniffing() {
    let expected = revision();
    let bytes = encode_body(WireFormat::Protobuf, &expected).unwrap();
    for header in [
        "application/protobuf",
        "application/x-protobuf",
        "Application/Protobuf; encoding=binary; version=1",
        "application/protobuf; version=\"1\"; encoding=\"binary\"",
    ] {
        assert_eq!(
            decode_response::<RevisionResponse>(WireFormat::Protobuf, true, Some(header), &bytes),
            Ok(expected.clone()),
            "{header}"
        );
    }
}

#[test]
fn mislabeled_invalid_or_unoffered_binary_successes_stay_ambiguous() {
    let expected = revision();
    let bytes = encode_body(WireFormat::Protobuf, &expected).unwrap();
    let json = wire::encode(WireFormat::Json, &expected).unwrap();
    for header in [
        None,
        Some("application/json"),
        Some("application/protobuf+json"),
        Some("application/protobuf; charset=utf-8"),
        Some("application/protobuf; version=2"),
        Some("application/protobuf; encoding=json"),
        Some("application/protobuf; version=1; version=1"),
        Some("application/protobuf; q=1"),
        Some("text/html"),
    ] {
        assert_eq!(
            decode_response::<RevisionResponse>(WireFormat::Protobuf, true, header, &bytes),
            Err(unavailable()),
            "{header:?}"
        );
    }
    assert_eq!(
        decode_response::<RevisionResponse>(
            WireFormat::Protobuf,
            true,
            Some("application/protobuf"),
            &json,
        ),
        Err(unavailable())
    );
    // The JSON rollback build did not offer a binary response. Retain historical fail-closed
    // behavior rather than sniffing bytes, linking an unsolicited codec or sending a retry.
    for requested in [WireFormat::Json, WireFormat::MessagePack] {
        assert_eq!(
            decode_response::<RevisionResponse>(
                requested,
                true,
                Some("application/protobuf"),
                &bytes,
            ),
            Err(unavailable())
        );
    }
}

#[test]
fn every_truncated_revision_and_unknown_trailer_is_ambiguous() {
    let bytes = encode_body(WireFormat::Protobuf, &revision()).unwrap();
    for end in 0..bytes.len() {
        assert_eq!(
            decode_response::<RevisionResponse>(
                WireFormat::Protobuf,
                true,
                Some("application/protobuf"),
                &bytes[..end],
            ),
            Err(unavailable()),
            "prefix {end}"
        );
    }
    let mut trailing = bytes;
    trailing.extend_from_slice(&[0x78, 0]); // reserved/unknown tag 15
    assert_eq!(
        decode_response::<RevisionResponse>(
            WireFormat::Protobuf,
            true,
            Some("application/protobuf"),
            &trailing,
        ),
        Err(unavailable())
    );
}

#[test]
fn json_error_envelope_remains_exact_on_a_protobuf_request() {
    for code in [
        "stale_revision",
        "unauthenticated",
        "forbidden",
        "operation_reused",
    ] {
        let bytes = format!(
            "{{\"code\":\"{code}\",\"current_revision\":2,\"issues\":[{{\"path\":\"script_json\",\"rule\":\"required\"}}]}}"
        );
        let expected: ApiError = serde_json::from_str(&bytes).unwrap();
        assert_eq!(
            decode_response::<RevisionResponse>(
                WireFormat::Protobuf,
                false,
                Some("application/json"),
                bytes.as_bytes(),
            ),
            Err(expected)
        );
    }
    assert_eq!(
        decode_response::<RevisionResponse>(
            WireFormat::Protobuf,
            false,
            Some("application/json"),
            b"{",
        ),
        Err(unavailable())
    );
    // A binary error is not a valid ApiError, even if its body is JSON.
    assert_eq!(
        decode_response::<RevisionResponse>(
            WireFormat::Protobuf,
            false,
            Some("application/protobuf"),
            stale(),
        ),
        Err(unavailable())
    );
}

#[test]
fn malformed_save_response_keeps_exact_intent_and_newer_typing_until_explicit_reconciliation() {
    let initial = Editor {
        actor: "alice".into(),
        script: "script".into(),
        draft: "Ngày mai 🎭 \"khẽ\"".into(),
        base: 7,
        ..Editor::default()
    };
    let (saving, intent) = initial.start_save("same-operation".into()).unwrap();
    let newer = saving.edit("newer typing".into());
    assert!(newer.start_save("extra-post".into()).is_none());
    let error = decode_response::<RevisionResponse>(
        WireFormat::Protobuf,
        true,
        Some("application/protobuf"),
        &[0xff],
    )
    .unwrap_err();
    let unresolved = newer.failed(&error);
    assert_eq!(unresolved.status, Status::Unavailable);
    assert!(unresolved.ambiguous);
    assert!(!unresolved.busy);
    assert_eq!(unresolved.pending, Some(intent.clone()));
    assert_eq!(unresolved.draft, "newer typing");
    assert_eq!(unresolved.base, 7);
    assert!(unresolved.start_save("extra-post".into()).is_none());
    assert!(unresolved.signed_in("bob".into()).retry().is_none());
    let (_, retry) = unresolved.retry().unwrap();
    assert_eq!(retry, intent);
}

#[test]
fn protobuf_validation_preserves_order_and_empty_present_issue_fields() {
    let expected = ScriptValidationResponse {
        issues: vec![
            FieldIssue {
                path: String::new(),
                rule: String::new(),
            },
            FieldIssue {
                path: "episode.🎭".into(),
                rule: "required".into(),
            },
        ],
    };
    let bytes = encode_body(WireFormat::Protobuf, &expected).unwrap();
    assert_eq!(
        decode_response::<ScriptValidationResponse>(
            WireFormat::Protobuf,
            true,
            Some("application/protobuf"),
            &bytes,
        ),
        Ok(expected)
    );
    assert_eq!(
        decode_response::<ScriptValidationResponse>(
            WireFormat::Protobuf,
            true,
            Some("application/protobuf"),
            &[],
        ),
        Ok(ScriptValidationResponse { issues: vec![] })
    );
}
