//! Independent Python wire-spec corpus, also executed by Kotlin 2.2.20 serializers.
use cantos_server::contract::{
    codec::{self as wire, DecodeError, EncodeError, WireFormat},
    studio::protobuf::{self, ProtobufDto, MAX_PROTOBUF_BYTES, MAX_PROTOBUF_ISSUES},
    studio::{
        FieldIssue, RevisionResponse, SaveRevisionRequest, ScriptValidationResponse,
        ValidateScriptRequest,
    },
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::fmt::Debug;
const GOLDENS: &str =
    include_str!("../../contracts/studio/editorial/v1/fixtures/protobuf/goldens.json");
fn from_hex(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn check<T: DeserializeOwned + Serialize + ProtobufDto + PartialEq + Debug>(case: &Value) {
    let bytes = from_hex(case["protobuf_hex"].as_str().unwrap());
    if !case["application_valid"].as_bool().unwrap() {
        assert_eq!(protobuf::decode::<T>(&bytes), Err(DecodeError::Shape));
        assert!(serde_json::from_value::<T>(case["value"].clone()).is_err());
        return;
    }
    let expected: T = serde_json::from_value(case["value"].clone()).unwrap();
    assert_eq!(
        protobuf::encode(&expected),
        Ok(bytes.clone()),
        "{}",
        case["name"]
    );
    assert_eq!(
        protobuf::decode::<T>(&bytes),
        Ok(expected),
        "{}",
        case["name"]
    );
    for format in [
        WireFormat::Json,
        WireFormat::MessagePack,
        WireFormat::Protobuf,
    ] {
        let expected: T = serde_json::from_value(case["value"].clone()).unwrap();
        let encoded = wire::encode_pilot(format, &expected).unwrap();
        assert_eq!(wire::decode_pilot::<T>(format, &encoded), Ok(expected));
    }
}
#[test]
fn independent_rust_kotlin_goldens_preserve_exact_json_values() {
    let corpus: Value = serde_json::from_str(GOLDENS).unwrap();
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 13);
    for case in corpus["cases"].as_array().unwrap() {
        match case["message"].as_str().unwrap() {
            "ValidateScriptRequest" => check::<ValidateScriptRequest>(case),
            "SaveRevisionRequest" => check::<SaveRevisionRequest>(case),
            "RevisionResponse" => check::<RevisionResponse>(case),
            "ScriptValidationResponse" => check::<ScriptValidationResponse>(case),
            unknown => panic!("unknown corpus message {unknown}"),
        }
    }
}
#[test]
fn missing_required_scalars_never_manufacture_defaults() {
    assert_eq!(
        protobuf::decode::<SaveRevisionRequest>(&[0x08, 0]),
        Err(DecodeError::Shape)
    );
    assert_eq!(
        protobuf::decode::<RevisionResponse>(&[]),
        Err(DecodeError::Shape)
    );
    assert_eq!(
        protobuf::decode::<ValidateScriptRequest>(&[]),
        Err(DecodeError::Shape)
    );
    assert_eq!(
        protobuf::decode::<ValidateScriptRequest>(&[0x0a, 0]),
        Ok(ValidateScriptRequest {
            script_json: String::new()
        })
    );
    assert_eq!(
        protobuf::decode::<ScriptValidationResponse>(&[]),
        Ok(ScriptValidationResponse { issues: vec![] })
    );
    assert_eq!(
        protobuf::decode::<ScriptValidationResponse>(&[0x0a, 0]),
        Err(DecodeError::Shape)
    );
}
#[test]
fn hostile_inputs_have_exact_bounded_typed_errors() {
    for (bytes, expected) in [
        (vec![0], DecodeError::Malformed),
        (vec![0x0a], DecodeError::Truncated),
        (vec![0x0a, 10, b'a'], DecodeError::Truncated),
        (vec![0x0a, 1, 0xff], DecodeError::InvalidUtf8),
        (vec![0x0a, 0, 0x0a, 0], DecodeError::Shape),
        (vec![0x12, 0], DecodeError::Shape),
        (vec![0x0a, 0, 0x12, 0], DecodeError::Shape),
        (vec![0x08, 0], DecodeError::Shape),
        (vec![0x0b, 0x0c], DecodeError::Shape),
        (vec![0x0a, 0x80, 0], DecodeError::Malformed),
        (vec![0x8a, 0, 0], DecodeError::Malformed),
        (vec![0x0a, 0, 0xff], DecodeError::Truncated),
        (vec![0x0a, 0, 0], DecodeError::Malformed),
        (vec![0x0a, 0, 0x08, 0], DecodeError::Shape),
        (vec![0x0a, 0x0a, b'{'], DecodeError::Truncated),
    ] {
        assert_eq!(
            protobuf::decode::<ValidateScriptRequest>(&bytes),
            Err(expected),
            "{bytes:?}"
        );
    }
    let mut overflow = vec![8];
    overflow.extend([255; 9]);
    overflow.push(2);
    assert_eq!(
        protobuf::decode::<SaveRevisionRequest>(&overflow),
        Err(DecodeError::Malformed)
    );
    let mut endless = vec![8];
    endless.extend([128; 10]);
    assert_eq!(
        protobuf::decode::<SaveRevisionRequest>(&endless),
        Err(DecodeError::Malformed)
    );
    // An overflowing tag, huge length and nested duplicate have no untrusted allocation.
    let mut huge_tag = vec![255; 9];
    huge_tag.push(1);
    assert_eq!(
        protobuf::decode::<ValidateScriptRequest>(&huge_tag),
        Err(DecodeError::Malformed)
    );
    let mut huge_length = vec![10];
    huge_length.extend([255; 9]);
    huge_length.push(1);
    assert_eq!(
        protobuf::decode::<ValidateScriptRequest>(&huge_length),
        Err(DecodeError::Truncated)
    );
    assert_eq!(
        protobuf::decode::<ScriptValidationResponse>(&[10, 6, 10, 0, 10, 0, 18, 0]),
        Err(DecodeError::Shape)
    );
    assert_eq!(
        protobuf::decode::<ScriptValidationResponse>(&[10, 4, 10, 0, 18, 0, 18, 0]),
        Err(DecodeError::Shape)
    );
}
#[test]
fn every_strict_prefix_of_a_scalar_message_fails_without_panicking() {
    let value = ValidateScriptRequest {
        script_json: "Ánh đèn 😀 e\u{301}".into(),
    };
    let bytes = protobuf::encode(&value).unwrap();
    for length in 0..bytes.len() {
        let expected = if length == 0 {
            DecodeError::Shape
        } else {
            DecodeError::Truncated
        };
        assert_eq!(
            protobuf::decode::<ValidateScriptRequest>(&bytes[..length]),
            Err(expected),
            "prefix {length}"
        );
    }
}
#[test]
fn fields_may_be_reordered_but_not_duplicated_and_repeated_order_is_exact() {
    let bytes = from_hex("1a0012000800");
    assert_eq!(
        protobuf::decode::<SaveRevisionRequest>(&bytes),
        Ok(SaveRevisionRequest {
            expected_revision: 0,
            operation_id: "".into(),
            script_json: "".into()
        })
    );
    // Unframed repeated-only message concatenation is exactly one ordered list, not a terminator.
    let issue = [10, 4, 10, 0, 18, 0];
    let mut bytes = issue.to_vec();
    bytes.extend(issue);
    assert_eq!(
        protobuf::decode::<ScriptValidationResponse>(&bytes),
        Ok(ScriptValidationResponse {
            issues: vec![
                FieldIssue {
                    path: "".into(),
                    rule: "".into()
                };
                2
            ]
        })
    );
    let mut duplicate = from_hex("080012001a00");
    duplicate.extend([8, 0]);
    assert_eq!(
        protobuf::decode::<SaveRevisionRequest>(&duplicate),
        Err(DecodeError::Shape)
    );
}
#[test]
fn byte_and_item_limits_fail_before_runtime_decoding() {
    // Four length-varint bytes and one tag byte make this exactly the envelope bound.
    let boundary = ValidateScriptRequest {
        script_json: "x".repeat(MAX_PROTOBUF_BYTES - 5),
    };
    let encoded_boundary = protobuf::encode(&boundary).unwrap();
    assert_eq!(encoded_boundary.len(), MAX_PROTOBUF_BYTES);
    assert_eq!(
        protobuf::decode::<ValidateScriptRequest>(&encoded_boundary),
        Ok(boundary)
    );
    assert_eq!(
        protobuf::decode::<ValidateScriptRequest>(&vec![0; MAX_PROTOBUF_BYTES + 1]),
        Err(DecodeError::TooLarge)
    );
    let issue = [10, 4, 10, 0, 18, 0];
    let allowed = issue.repeat(MAX_PROTOBUF_ISSUES);
    let value = protobuf::decode::<ScriptValidationResponse>(&allowed).unwrap();
    assert_eq!(value.issues.len(), MAX_PROTOBUF_ISSUES);
    assert_eq!(protobuf::encode(&value).unwrap(), allowed);
    assert_eq!(
        protobuf::decode::<ScriptValidationResponse>(&issue.repeat(MAX_PROTOBUF_ISSUES + 1)),
        Err(DecodeError::TooLarge)
    );
    let mut wrong_wire_after_maximum = issue.repeat(MAX_PROTOBUF_ISSUES);
    wrong_wire_after_maximum.extend([8, 0]);
    assert_eq!(
        protobuf::decode::<ScriptValidationResponse>(&wrong_wire_after_maximum),
        Err(DecodeError::Shape)
    );
    let mut truncated_after_maximum = issue.repeat(MAX_PROTOBUF_ISSUES);
    truncated_after_maximum.extend([10, 1]);
    assert_eq!(
        protobuf::decode::<ScriptValidationResponse>(&truncated_after_maximum),
        Err(DecodeError::Truncated)
    );
    let excessive = ScriptValidationResponse {
        issues: vec![
            FieldIssue {
                path: "".into(),
                rule: "".into()
            };
            MAX_PROTOBUF_ISSUES + 1
        ],
    };
    assert_eq!(protobuf::encode(&excessive), Err(EncodeError));
    assert_eq!(
        protobuf::encode(&ValidateScriptRequest {
            script_json: "x".repeat(MAX_PROTOBUF_BYTES)
        }),
        Err(EncodeError)
    );
}
#[test]
fn arbitrary_short_hostile_bytes_never_panic() {
    for first in 0..=255_u8 {
        for second in 0..=255_u8 {
            let bytes = [first, second];
            let _ = protobuf::decode::<ValidateScriptRequest>(&bytes);
            let _ = protobuf::decode::<SaveRevisionRequest>(&bytes);
            let _ = protobuf::decode::<RevisionResponse>(&bytes);
            let _ = protobuf::decode::<ScriptValidationResponse>(&bytes);
        }
    }
}
#[test]
fn protobuf_media_parameters_are_rfc9996_binary_only() {
    for accepted in [
        "application/protobuf",
        "Application/Protobuf; encoding=BINARY; version=1",
        "application/x-protobuf",
        "application/protobuf;version=\"1\";encoding=\"binary\"",
    ] {
        assert_eq!(
            WireFormat::from_content_type(accepted),
            Some(WireFormat::Protobuf),
            "{accepted}"
        );
    }
    for refused in [
        "application/protobuf+json",
        "application/x-protobuf+json",
        "application/x-protobuffer",
        "application/protobuf;charset=utf-8",
        "application/protobuf;version=2",
        "application/protobuf;encoding=json",
        "application/protobuf;encoding=binary;encoding=binary",
        "application/protobuf;version=1;version=1",
        "application/protobuf;foo=bar",
        "application/protobuf;",
        "application/protobuf;q=1",
    ] {
        assert_eq!(WireFormat::from_content_type(refused), None, "{refused}");
    }
    assert_eq!(WireFormat::Protobuf.media_type(), "application/protobuf");
    assert_eq!(
        wire::encode(
            WireFormat::Protobuf,
            &ValidateScriptRequest {
                script_json: "".into()
            }
        ),
        Err(EncodeError)
    );
    assert_eq!(
        wire::decode::<ValidateScriptRequest>(WireFormat::Protobuf, &[10, 0]),
        Err(DecodeError::Shape)
    );
}

#[test]
fn shared_kotlin_hostile_admission_corpus_has_exact_categories() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../contracts/studio/editorial/v1/fixtures/protobuf/hostile.json"
    ))
    .unwrap();
    for case in corpus["cases"].as_array().unwrap() {
        let bytes = from_hex(case["protobuf_hex"].as_str().unwrap());
        let result = match case["message"].as_str().unwrap() {
            "ValidateScriptRequest" => {
                protobuf::decode::<ValidateScriptRequest>(&bytes).map(|_| ())
            }
            "SaveRevisionRequest" => protobuf::decode::<SaveRevisionRequest>(&bytes).map(|_| ()),
            "ScriptValidationResponse" => {
                protobuf::decode::<ScriptValidationResponse>(&bytes).map(|_| ())
            }
            unknown => panic!("unknown corpus message {unknown}"),
        };
        let expected = case["error"].as_str().map(|error| match error {
            "Shape" => DecodeError::Shape,
            "Malformed" => DecodeError::Malformed,
            "Truncated" => DecodeError::Truncated,
            "InvalidUtf8" => DecodeError::InvalidUtf8,
            unknown => panic!("unknown expected error {unknown}"),
        });
        assert_eq!(result, expected.map_or(Ok(()), Err), "{}", case["name"]);
    }
}
