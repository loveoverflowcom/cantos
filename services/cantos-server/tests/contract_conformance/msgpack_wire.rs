//! The MessagePack representation of the contract DTOs, checked against goldens written by the
//! independent `scripts/reference_msgpack.py` encoder and against the JSON fixtures.
use std::fmt::Debug;

use cantos_server::contract::{
    codec::{decode, encode, DecodeError, WireFormat, MAX_MSGPACK_DEPTH},
    studio::{
        ApiError, ErrorCode, HistoryResponse, ProductionCatalogResponse, ReviewResponse,
        RevisionResponse, SaveProductionRightsRequest, SaveProductionSettingsRequest,
        SaveRevisionRequest, ScriptValidationResponse, ValidateScriptRequest,
    },
};
use serde::{de::DeserializeOwned, de::IgnoredAny, Serialize};
use serde_json::Value;

use WireFormat::{Json, MessagePack};

const GOLDENS: &str =
    include_str!("../../contracts/studio/editorial/v1/fixtures/msgpack-goldens.json");
const SAVE: &str = include_str!("../../contracts/studio/editorial/v1/fixtures/save-revision.json");

fn studio_fixture(name: &str) -> &'static str {
    match name {
        "revision.json" => {
            include_str!("../../contracts/studio/editorial/v1/fixtures/revision.json")
        }
        "save-revision.json" => SAVE,
        "validate-script.json" => {
            include_str!("../../contracts/studio/editorial/v1/fixtures/validate-script.json")
        }
        "validation.json" => {
            include_str!("../../contracts/studio/editorial/v1/fixtures/validation.json")
        }
        "history.json" => include_str!("../../contracts/studio/editorial/v1/fixtures/history.json"),
        "review.json" => include_str!("../../contracts/studio/editorial/v1/fixtures/review.json"),
        other => panic!("golden names an unknown fixture: {other}"),
    }
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
        .collect()
}

fn json_value(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

/// JSON fixture → DTO → MessagePack → DTO → JSON, returning the MessagePack bytes.
fn through_both<T>(json: &str) -> Vec<u8>
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let from_json: T = decode(Json, json.as_bytes()).unwrap();
    let msgpack = encode(MessagePack, &from_json).unwrap();
    let from_msgpack: T = decode(MessagePack, &msgpack).unwrap();
    assert_eq!(from_msgpack, from_json);
    assert_eq!(
        json_value(&encode(Json, &from_msgpack).unwrap()),
        json_value(json.as_bytes())
    );
    msgpack
}

fn agree_with_golden<T>(json: &str, golden: &[u8])
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    assert_eq!(through_both::<T>(json), golden);
    assert_eq!(
        decode::<T>(MessagePack, golden).unwrap(),
        decode::<T>(Json, json.as_bytes()).unwrap()
    );
}

#[test]
fn independent_goldens_equal_rust_bytes_and_decode_to_the_json_fixture_values() {
    let goldens = json_value(GOLDENS.as_bytes());
    assert_eq!(goldens["media_type"], "application/msgpack");
    let cases = goldens["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for case in cases {
        let json = studio_fixture(case["fixture"].as_str().unwrap());
        let golden = hex(case["msgpack_hex"].as_str().unwrap());
        match case["definition"].as_str().unwrap() {
            "Revision" => agree_with_golden::<RevisionResponse>(json, &golden),
            "SaveRevision" => agree_with_golden::<SaveRevisionRequest>(json, &golden),
            "ValidateScript" => agree_with_golden::<ValidateScriptRequest>(json, &golden),
            "ScriptValidation" => agree_with_golden::<ScriptValidationResponse>(json, &golden),
            "History" => agree_with_golden::<HistoryResponse>(json, &golden),
            "Review" => agree_with_golden::<ReviewResponse>(json, &golden),
            other => panic!("golden names an unknown definition: {other}"),
        }
    }
}

#[test]
fn other_contract_fixtures_survive_json_to_msgpack_to_json_including_tagged_enums() {
    through_both::<ApiError>(include_str!(
        "../../contracts/studio/editorial/v1/fixtures/stale-revision.json"
    ));
    through_both::<ProductionCatalogResponse>(include_str!(
        "../../contracts/studio/production/v1/fixtures/catalog-response.json"
    ));
    through_both::<SaveProductionRightsRequest>(include_str!(
        "../../contracts/studio/production/v1/fixtures/rights-request.json"
    ));
    through_both::<SaveProductionSettingsRequest>(include_str!(
        "../../contracts/studio/production/v1/fixtures/settings-request.json"
    ));
}

#[test]
fn every_struct_level_is_a_map_keyed_by_field_name() {
    let history: HistoryResponse = decode(Json, studio_fixture("history.json").as_bytes()).unwrap();
    let as_value: Value = decode(MessagePack, &encode(MessagePack, &history).unwrap()).unwrap();
    let keys = |value: &Value| {
        let mut keys: Vec<_> = value.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        keys
    };
    assert_eq!(keys(&as_value), ["next_after", "reviews", "revisions"]);
    assert_eq!(
        keys(&as_value["revisions"][0]),
        [
            "accepted_at",
            "accepted_by",
            "content_digest",
            "export_digest",
            "revision",
            "script_id"
        ]
    );
    assert_eq!(keys(&as_value["reviews"][0]).len(), 8);
}

/// A tiny independent builder: fixmap with fixstr keys and caller-encoded values.
fn map(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    assert!(entries.len() < 16);
    let mut bytes = vec![0x80 | entries.len() as u8];
    for (key, value) in entries {
        assert!(key.len() < 32);
        bytes.push(0xa0 | key.len() as u8);
        bytes.extend_from_slice(key.as_bytes());
        bytes.extend_from_slice(value);
    }
    bytes
}

fn scalar<T: Serialize>(value: T) -> Vec<u8> {
    encode(MessagePack, &value).unwrap()
}

fn save_request() -> SaveRevisionRequest {
    decode(Json, SAVE.as_bytes()).unwrap()
}

#[test]
fn key_order_is_free_but_positional_encoders_are_refused() {
    let save = save_request();
    let reordered = map(&[
        ("script_json", scalar(&save.script_json)),
        ("operation_id", scalar(&save.operation_id)),
        ("expected_revision", scalar(save.expected_revision)),
    ]);
    assert_eq!(
        decode::<SaveRevisionRequest>(MessagePack, &reordered).unwrap(),
        save
    );
    let positional = rmp_serde::to_vec(&save).unwrap();
    assert_eq!(positional[0], 0x93, "rmp_serde::to_vec writes a fixarray");
    assert_eq!(
        decode::<SaveRevisionRequest>(MessagePack, &positional),
        Err(DecodeError::Shape)
    );
}

#[test]
fn absent_and_null_optionals_both_mean_none_and_encoding_always_writes_null() {
    let error = ApiError {
        code: ErrorCode::NotFound,
        current_revision: None,
        issues: vec![],
    };
    let written: Value = decode(MessagePack, &encode(MessagePack, &error).unwrap()).unwrap();
    assert_eq!(written["current_revision"], Value::Null);
    assert!(written
        .as_object()
        .unwrap()
        .contains_key("current_revision"));
    let without_key = map(&[("code", scalar("not_found")), ("issues", vec![0x90])]);
    let with_null = map(&[
        ("code", scalar("not_found")),
        ("current_revision", vec![0xc0]),
        ("issues", vec![0x90]),
    ]);
    for bytes in [without_key, with_null] {
        assert_eq!(decode::<ApiError>(MessagePack, &bytes).unwrap(), error);
    }
    for json in [
        r#"{"code":"not_found","issues":[]}"#,
        r#"{"code":"not_found","current_revision":null,"issues":[]}"#,
    ] {
        assert_eq!(decode::<ApiError>(Json, json.as_bytes()).unwrap(), error);
    }
    let history: HistoryResponse = decode(Json, studio_fixture("history.json").as_bytes()).unwrap();
    assert_eq!(history.next_after, None);
    let written: Value = decode(MessagePack, &encode(MessagePack, &history).unwrap()).unwrap();
    assert_eq!(written["next_after"], Value::Null);
}

#[test]
fn unknown_missing_duplicate_and_mistyped_fields_are_refused_in_both_representations() {
    let save = save_request();
    let base = || {
        vec![
            ("expected_revision", scalar(save.expected_revision)),
            ("operation_id", scalar(&save.operation_id)),
            ("script_json", scalar(&save.script_json)),
        ]
    };
    let mut unknown = base();
    unknown.push(("actor_id", scalar("owner")));
    let mut missing = base();
    missing.remove(1);
    let mut duplicate = base();
    duplicate.push(("expected_revision", scalar(2u64)));
    let mut mistyped = base();
    mistyped[0].1 = scalar("1");
    let mut negative = base();
    negative[0].1 = scalar(-1i64);
    for (case, entries) in [
        ("unknown", unknown),
        ("missing", missing),
        ("duplicate", duplicate),
        ("mistyped", mistyped),
        ("negative", negative),
    ] {
        let bytes = map(&entries);
        assert_eq!(
            decode::<SaveRevisionRequest>(MessagePack, &bytes),
            Err(DecodeError::Shape),
            "{case}"
        );
        let json = serde_json::to_vec(&rmp_serde::from_slice::<Value>(&bytes).unwrap()).unwrap();
        if case != "duplicate" {
            assert_eq!(
                decode::<SaveRevisionRequest>(Json, &json),
                Err(DecodeError::Shape),
                "{case}"
            );
        }
    }
    let duplicate_json = br#"{"expected_revision":1,"expected_revision":2,"operation_id":"00000000-0000-4000-8000-000000000004","script_json":"{}"}"#;
    assert_eq!(
        decode::<SaveRevisionRequest>(Json, duplicate_json),
        Err(DecodeError::Shape)
    );
}

#[test]
fn every_strict_prefix_of_a_golden_is_reported_as_truncated() {
    let golden = encode(MessagePack, &save_request()).unwrap();
    for length in 0..golden.len() {
        assert_eq!(
            decode::<SaveRevisionRequest>(MessagePack, &golden[..length]),
            Err(DecodeError::Truncated),
            "prefix {length}"
        );
    }
    assert_eq!(
        decode::<SaveRevisionRequest>(Json, &SAVE.as_bytes()[..SAVE.len() / 2]),
        Err(DecodeError::Truncated)
    );
}

#[test]
fn bytes_after_one_complete_value_are_refused() {
    let golden = encode(MessagePack, &save_request()).unwrap();
    for trailer in [vec![0xc0], vec![0xc1], vec![0x80], golden.clone()] {
        let mut bytes = golden.clone();
        bytes.extend_from_slice(&trailer);
        assert_eq!(
            decode::<SaveRevisionRequest>(MessagePack, &bytes),
            Err(DecodeError::TrailingBytes),
            "{trailer:02x?}"
        );
    }
    let mut json = SAVE.as_bytes().to_vec();
    json.extend_from_slice(b" x");
    assert_eq!(
        decode::<SaveRevisionRequest>(Json, &json),
        Err(DecodeError::Malformed)
    );
}

#[test]
fn invalid_utf8_is_refused_rather_than_replaced() {
    let bytes = map(&[
        ("expected_revision", scalar(1u64)),
        (
            "operation_id",
            scalar("00000000-0000-4000-8000-000000000004"),
        ),
        ("script_json", vec![0xa3, b'{', 0xff, b'}']),
    ]);
    assert_eq!(
        decode::<SaveRevisionRequest>(MessagePack, &bytes),
        Err(DecodeError::InvalidUtf8)
    );
    assert_eq!(
        decode::<ScriptValidationResponse>(MessagePack, &[0xc1]),
        Err(DecodeError::Shape),
        "a reserved marker at the top level is not a map"
    );
}

fn nested_arrays_in_a_map(arrays: usize) -> Vec<u8> {
    let mut bytes = vec![0x81, 0xa4, b'j', b'u', b'n', b'k'];
    bytes.extend(std::iter::repeat_n(0x91, arrays));
    bytes.push(0xc0);
    bytes
}

#[test]
fn nesting_is_bounded_before_recursion_can_exhaust_the_stack() {
    // The top-level map counts: a value nests fewer than MAX_MSGPACK_DEPTH arrays and maps.
    let deepest = nested_arrays_in_a_map(MAX_MSGPACK_DEPTH - 2);
    assert_eq!(decode::<IgnoredAny>(MessagePack, &deepest), Ok(IgnoredAny));
    for arrays in [MAX_MSGPACK_DEPTH - 1, 100_000] {
        assert_eq!(
            decode::<IgnoredAny>(MessagePack, &nested_arrays_in_a_map(arrays)),
            Err(DecodeError::TooDeep),
            "{arrays}"
        );
    }
}

#[test]
fn declared_lengths_beyond_the_input_fail_without_allocating_for_them() {
    let huge_string = map(&[
        ("expected_revision", scalar(1u64)),
        ("operation_id", vec![0xdb, 0xff, 0xff, 0xff, 0xff, b'a']),
    ]);
    let huge_array = map(&[("issues", vec![0xdd, 0xff, 0xff, 0xff, 0xff, 0x80])]);
    let huge_map = vec![0xdf, 0xff, 0xff, 0xff, 0xff];
    assert_eq!(
        decode::<SaveRevisionRequest>(MessagePack, &huge_string),
        Err(DecodeError::Truncated)
    );
    assert_eq!(
        decode::<ScriptValidationResponse>(MessagePack, &huge_array),
        Err(DecodeError::Shape),
        "serde caps preallocation at 1 MiB, so a 4 Gi-entry header cannot force its size"
    );
    assert_eq!(
        decode::<ScriptValidationResponse>(MessagePack, &huge_array[..huge_array.len() - 1]),
        Err(DecodeError::Truncated)
    );
    assert_eq!(
        decode::<ScriptValidationResponse>(MessagePack, &huge_map),
        Err(DecodeError::Truncated)
    );
}

#[test]
fn integer_and_string_formats_follow_the_specification_boundaries() {
    // The same table pins scripts/reference_msgpack.py, so both encoders agree with the spec.
    for (value, expected) in [
        (0u64, "00"),
        (127, "7f"),
        (128, "cc80"),
        (255, "ccff"),
        (256, "cd0100"),
        (65535, "cdffff"),
        (65536, "ce00010000"),
        (u64::from(u32::MAX), "ceffffffff"),
        (u64::from(u32::MAX) + 1, "cf0000000100000000"),
        (i64::MAX as u64, "cf7fffffffffffffff"),
    ] {
        assert_eq!(scalar(value), hex(expected), "{value}");
    }
    for (length, header) in [
        (31usize, "bf"),
        (32, "d920"),
        (255, "d9ff"),
        (256, "da0100"),
        (65535, "daffff"),
        (65536, "db00010000"),
    ] {
        let bytes = scalar("x".repeat(length));
        let header = hex(header);
        assert_eq!(&bytes[..header.len()], header, "{length}");
        assert_eq!(bytes.len(), header.len() + length);
    }
}

#[test]
fn unicode_text_crosses_both_representations_byte_for_byte() {
    let decomposed = "Nga\u{300}y";
    let composed = "Ngày";
    assert_ne!(decomposed, composed);
    for text in [
        decomposed,
        composed,
        "“Đi thôi!” — anh nói \"khẽ\"; lối C:\\kịch\\cảnh-1 🎭",
        "dòng 1\r\n\tdòng 2\u{2028}\u{2029}",
        "\u{feff}\u{200b}\u{0}",
        "",
    ] {
        let request = SaveRevisionRequest {
            script_json: text.to_owned(),
            ..save_request()
        };
        for format in [Json, MessagePack] {
            let decoded: SaveRevisionRequest =
                decode(format, &encode(format, &request).unwrap()).unwrap();
            assert_eq!(
                decoded.script_json.as_bytes(),
                text.as_bytes(),
                "{format:?}"
            );
        }
    }
}

fn container_depth(value: &Value) -> usize {
    match value {
        Value::Array(items) => 1 + items.iter().map(container_depth).max().unwrap_or(0),
        Value::Object(fields) => 1 + fields.values().map(container_depth).max().unwrap_or(0),
        _ => 0,
    }
}

#[test]
fn contract_fixtures_nest_well_below_the_decoder_bound() {
    let deepest = [
        studio_fixture("revision.json"),
        studio_fixture("history.json"),
        studio_fixture("save-revision.json"),
        include_str!("../../contracts/studio/production/v1/fixtures/catalog-response.json"),
        include_str!("../../contracts/studio/production/v1/fixtures/rights-request.json"),
        include_str!("../../contracts/studio/production/v1/fixtures/settings-request.json"),
    ]
    .iter()
    .map(|json| container_depth(&json_value(json.as_bytes())))
    .max()
    .unwrap();
    assert!(
        deepest + 4 < MAX_MSGPACK_DEPTH,
        "deepest fixture nests {deepest} containers; review MAX_MSGPACK_DEPTH"
    );
}
