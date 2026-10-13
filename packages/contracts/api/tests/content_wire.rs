//! Codec-level Content coverage. Content routes remain JSON-only; using MessagePack here
//! checks the shared DTO representation, not route support or semantic domain admission.
use std::fmt::Debug;

use cantos_contracts::{
    wire::{decode, encode, DecodeError, WireFormat},
    AppendContentRequest, ContentBody, ContentRevisionResponse, UnitPatchDto,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

const TEXT: &str = include_str!("../../fixtures/content/v1/text-request.json");
const RECORDING: &str = include_str!("../../fixtures/content/v1/recording-request.json");
const REVISION: &str = include_str!("../../fixtures/content/v1/text-revision.json");
const FORMATS: [WireFormat; 2] = [WireFormat::Json, WireFormat::MessagePack];

fn round_trip<T>(value: &T)
where
    T: DeserializeOwned + Serialize + PartialEq + Debug,
{
    for format in FORMATS {
        assert_eq!(
            decode::<T>(format, &encode(format, value).unwrap()).unwrap(),
            *value
        );
    }
}

#[test]
fn current_content_fixtures_keep_values_in_both_representations() {
    for fixture in [TEXT, RECORDING] {
        let value: AppendContentRequest = decode(WireFormat::Json, fixture.as_bytes()).unwrap();
        round_trip(&value);
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            serde_json::from_str::<Value>(fixture).unwrap()
        );
    }
    let value: ContentRevisionResponse = decode(WireFormat::Json, REVISION.as_bytes()).unwrap();
    round_trip(&value);
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        serde_json::from_str::<Value>(REVISION).unwrap()
    );
}

#[test]
fn raw_numeric_limits_round_trip_without_claiming_domain_admission() {
    let mut request: AppendContentRequest = decode(WireFormat::Json, TEXT.as_bytes()).unwrap();
    for revision in [0, 1, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX] {
        request.expected_revision = revision;
        request.expected_work_revision = revision;
        round_trip(&request);
    }
    let mut response: ContentRevisionResponse =
        decode(WireFormat::Json, REVISION.as_bytes()).unwrap();
    for timestamp in [i64::MIN, -1, 0, 1, i64::MAX] {
        response.recorded_at = timestamp;
        response.revision = u64::MAX;
        round_trip(&response);
    }
}

#[test]
fn negative_fractional_and_out_of_range_unsigned_wire_numbers_are_shape_errors() {
    let mut value: Value = serde_json::from_str(TEXT).unwrap();
    for number in [json!(-1), json!(1.5), json!(1.0)] {
        value["expected_revision"] = number;
        for format in FORMATS {
            assert_eq!(
                decode::<AppendContentRequest>(format, &encode(format, &value).unwrap()),
                Err(DecodeError::Shape)
            );
        }
    }
    let raw = serde_json::to_string(&serde_json::from_str::<Value>(TEXT).unwrap()).unwrap();
    let needle = "\"expected_revision\":0";
    assert!(raw.contains(needle));
    let overflow = raw.replace(needle, "\"expected_revision\":18446744073709551616");
    assert_eq!(
        decode::<AppendContentRequest>(WireFormat::Json, overflow.as_bytes()),
        Err(DecodeError::Shape)
    );
}

#[test]
fn signed_timestamp_overflow_is_a_shape_error() {
    let mut value: Value = serde_json::from_str(REVISION).unwrap();
    value["recorded_at"] = json!(u64::MAX);
    for format in FORMATS {
        assert_eq!(
            decode::<ContentRevisionResponse>(format, &encode(format, &value).unwrap()),
            Err(DecodeError::Shape)
        );
    }
    let raw = serde_json::to_string(&serde_json::from_str::<Value>(REVISION).unwrap()).unwrap();
    for token in ["9223372036854775808", "-9223372036854775809"] {
        let overflow = raw.replace("\"recorded_at\":1000", &format!("\"recorded_at\":{token}"));
        assert_eq!(
            decode::<ContentRevisionResponse>(WireFormat::Json, overflow.as_bytes()),
            Err(DecodeError::Shape)
        );
    }
}

#[test]
fn unknown_fields_at_each_content_object_cannot_forge_authority() {
    for path in ["", "/details", "/details/body"] {
        let mut value: Value = serde_json::from_str(TEXT).unwrap();
        value.pointer_mut(path).unwrap()["approved"] = json!(true);
        for format in FORMATS {
            assert_eq!(
                decode::<AppendContentRequest>(format, &encode(format, &value).unwrap()),
                Err(DecodeError::Shape),
                "{path} {format:?}"
            );
        }
    }
    let mut value: Value = serde_json::from_str(REVISION).unwrap();
    value["details"]["body"] = json!({"kind":"script","source":null,"script":{"id":"raw-id","revision":1,"export_digest":"raw-digest","latest":true}});
    for format in FORMATS {
        assert_eq!(
            decode::<ContentRevisionResponse>(format, &encode(format, &value).unwrap()),
            Err(DecodeError::Shape)
        );
    }
}

#[test]
fn all_body_variants_preserve_raw_references_and_reject_unknown_kind() {
    for value in [
        json!({"kind":"recording"}),
        json!({"kind":"text","text":"Nga\u{0300}y\r\n\u{0000}","source":null}),
        json!({"kind":"source","source":{"id":"raw-id","sha256":"raw-hash"}}),
        json!({"kind":"script","script":{"id":"raw-id","revision":0,"export_digest":"raw-digest"},"source":null}),
    ] {
        let body: ContentBody = serde_json::from_value(value.clone()).unwrap();
        round_trip(&body);
        assert_eq!(serde_json::to_value(body).unwrap(), value);
    }
    let value = json!({"kind":"publication"});
    for format in FORMATS {
        assert_eq!(
            decode::<ContentBody>(format, &encode(format, &value).unwrap()),
            Err(DecodeError::Shape)
        );
    }
}

#[test]
fn raw_content_decoding_does_not_normalize_or_validate_domain_text_and_identity() {
    let mut value: Value = serde_json::from_str(TEXT).unwrap();
    value["id"] = json!("");
    value["details"]["language"] = json!("not a language");
    value["details"]["body"]["text"] = json!("Nga\u{0300}y\r\n\u{0000}");
    value["details"]["rights_refs"] = json!(["duplicate", "duplicate"]);
    let raw: AppendContentRequest = serde_json::from_value(value.clone()).unwrap();
    round_trip(&raw);
    assert_eq!(serde_json::to_value(raw).unwrap(), value);
}

#[test]
fn every_strict_prefix_is_truncated_or_an_incomplete_utf8_sequence() {
    let request: AppendContentRequest = decode(WireFormat::Json, TEXT.as_bytes()).unwrap();
    for format in FORMATS {
        let bytes = encode(format, &request).unwrap();
        for length in 0..bytes.len() {
            let expected = match format {
                WireFormat::Json if std::str::from_utf8(&bytes[..length]).is_err() => {
                    DecodeError::InvalidUtf8
                }
                WireFormat::Json | WireFormat::MessagePack => DecodeError::Truncated,
            };
            assert_eq!(
                decode::<AppendContentRequest>(format, &bytes[..length]),
                Err(expected),
                "{format:?} prefix {length}"
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(b'x');
        let expected = match format {
            WireFormat::Json => DecodeError::Malformed,
            WireFormat::MessagePack => DecodeError::TrailingBytes,
        };
        assert_eq!(
            decode::<AppendContentRequest>(format, &trailing),
            Err(expected)
        );
    }
}

#[test]
fn patch_missing_clear_and_replace_survive_both_codecs() {
    for value in [
        json!({}),
        json!({"source":null}),
        json!({"source":{"id":"raw-id","sha256":"raw-hash"}}),
    ] {
        let patch: UnitPatchDto = serde_json::from_value(value.clone()).unwrap();
        round_trip(&patch);
        assert_eq!(serde_json::to_value(patch).unwrap(), value);
    }
}
