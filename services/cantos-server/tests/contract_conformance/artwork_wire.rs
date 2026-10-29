//! Independent Python/prost/Kotlin artwork metadata corpus and exact presence regression tests.
use cantos_server::contract::{
    codec::{self as wire, DecodeError, EncodeError, WireFormat},
    studio::protobuf::{self, ProtobufDto, MAX_ARTWORK_PROTOBUF_BYTES, MAX_ARTWORK_RIGHTS_REFS},
    studio::{
        CreateImageIntentRequest, ImageAssetDocument, ImageDeclarations, ImageIntentDocument,
        ImageIntentResponse, ImageIntentState, ImageMime, ImageVariantMetadata,
        SetWorkArtworkRequest, WorkArtworkResponse, WorkArtworkSelection,
    },
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::fmt::Debug;
const GOLDENS: &str = include_str!("../../contracts/studio/artwork/v1/fixtures/goldens.json");
const HOSTILE: &str = include_str!("../../contracts/studio/artwork/v1/fixtures/hostile.json");
const SCHEMA: &str = include_str!("../../contracts/studio/artwork/v1/artwork.schema.json");
fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn check<T: DeserializeOwned + Serialize + ProtobufDto + PartialEq + Debug>(case: &Value) {
    let encoded = bytes(case["protobuf_hex"].as_str().unwrap());
    if !case["application_valid"].as_bool().unwrap() {
        assert_eq!(
            protobuf::decode::<T>(&encoded),
            Err(DecodeError::Shape),
            "{}",
            case["name"]
        );
        assert!(
            serde_json::from_value::<T>(case["value"].clone()).is_err(),
            "{}",
            case["name"]
        );
        return;
    }
    let expected: T = serde_json::from_value(case["value"].clone()).unwrap();
    assert_eq!(
        protobuf::encode(&expected),
        Ok(encoded.clone()),
        "{}",
        case["name"]
    );
    assert_eq!(
        protobuf::decode::<T>(&encoded),
        Ok(expected),
        "{}",
        case["name"]
    );
    // Both directions preserve the same raw application intent; nullable response
    // absence serializes explicit null, without inventing accepted facts or time.
    for format in [WireFormat::Json, WireFormat::Protobuf] {
        let expected: T = serde_json::from_value(case["value"].clone()).unwrap();
        let encoded = wire::encode_artwork(format, &expected).unwrap();
        assert_eq!(wire::decode_artwork::<T>(format, &encoded), Ok(expected));
    }
}
#[test]
fn independent_artwork_goldens_preserve_raw_json_and_protobuf_intent() {
    let corpus: Value = serde_json::from_str(GOLDENS).unwrap();
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 40);
    for case in corpus["cases"].as_array().unwrap() {
        match case["message"].as_str().unwrap() {
            "ImageDeclarations" => check::<ImageDeclarations>(case),
            "CreateImageIntentRequest" => check::<CreateImageIntentRequest>(case),
            "ImageIntentDocument" => check::<ImageIntentDocument>(case),
            "ImageVariantMetadata" => check::<ImageVariantMetadata>(case),
            "ImageAssetDocument" => check::<ImageAssetDocument>(case),
            "ImageIntentResponse" => check::<ImageIntentResponse>(case),
            "WorkArtworkSelection" => check::<WorkArtworkSelection>(case),
            "SetWorkArtworkRequest" => check::<SetWorkArtworkRequest>(case),
            "WorkArtworkResponse" => check::<WorkArtworkResponse>(case),
            // Empty clear is an internal oneof arm, never a public standalone DTO.
            "ClearArtwork" => assert!(case["protobuf_hex"].as_str().unwrap().is_empty()),
            unknown => panic!("unknown corpus message {unknown}"),
        }
    }
}
fn decode_case(message: &str, input: &[u8]) -> Result<(), DecodeError> {
    match message {
        "ImageDeclarations" => protobuf::decode::<ImageDeclarations>(input).map(|_| ()),
        "CreateImageIntentRequest" => {
            protobuf::decode::<CreateImageIntentRequest>(input).map(|_| ())
        }
        "ImageIntentDocument" => protobuf::decode::<ImageIntentDocument>(input).map(|_| ()),
        "ImageVariantMetadata" => protobuf::decode::<ImageVariantMetadata>(input).map(|_| ()),
        "ImageAssetDocument" => protobuf::decode::<ImageAssetDocument>(input).map(|_| ()),
        "ImageIntentResponse" => protobuf::decode::<ImageIntentResponse>(input).map(|_| ()),
        "WorkArtworkSelection" => protobuf::decode::<WorkArtworkSelection>(input).map(|_| ()),
        "SetWorkArtworkRequest" => protobuf::decode::<SetWorkArtworkRequest>(input).map(|_| ()),
        "WorkArtworkResponse" => protobuf::decode::<WorkArtworkResponse>(input).map(|_| ()),
        unknown => panic!("unknown hostile message {unknown}"),
    }
}
#[test]
fn independent_hostile_vectors_fail_with_exact_bounded_categories() {
    let corpus: Value = serde_json::from_str(HOSTILE).unwrap();
    for case in corpus["cases"].as_array().unwrap() {
        let expected = case["error"].as_str().map(|error| match error {
            "Shape" => DecodeError::Shape,
            "Malformed" => DecodeError::Malformed,
            "Truncated" => DecodeError::Truncated,
            "InvalidUtf8" => DecodeError::InvalidUtf8,
            "TooLarge" => DecodeError::TooLarge,
            "TooDeep" => DecodeError::TooDeep,
            unknown => panic!("unknown category {unknown}"),
        });
        assert_eq!(
            decode_case(
                case["message"].as_str().unwrap(),
                &bytes(case["protobuf_hex"].as_str().unwrap())
            ),
            expected.map_or(Ok(()), Err),
            "{}",
            case["name"]
        );
    }
    // Presence belongs to the adapter after the complete bounded prescan. A
    // missing nested provenance must not hide malformed trailing envelope data.
    let missing_child = [10, 0, 18, 0, 26, 0, 32, 0, 40, 1, 50, 0];
    assert_eq!(
        protobuf::decode::<CreateImageIntentRequest>(&missing_child),
        Err(DecodeError::Shape)
    );
    let mut malformed = missing_child.to_vec();
    malformed.push(0);
    assert_eq!(
        protobuf::decode::<CreateImageIntentRequest>(&malformed),
        Err(DecodeError::Malformed)
    );
}
fn fixture(name: &str) -> Value {
    let corpus: Value = serde_json::from_str(GOLDENS).unwrap();
    corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()["value"]
        .clone()
}
#[test]
fn json_set_requires_explicit_null_or_complete_selection() {
    let base = fixture("clear-artwork");
    let expected: SetWorkArtworkRequest = serde_json::from_value(base.clone()).unwrap();
    assert_eq!(expected.artwork, None);
    assert_eq!(
        serde_json::to_value(&expected).unwrap()["artwork"],
        Value::Null
    );
    let mut missing = base.clone();
    missing.as_object_mut().unwrap().remove("artwork");
    assert!(serde_json::from_value::<SetWorkArtworkRequest>(missing)
        .unwrap_err()
        .to_string()
        .contains("missing field `artwork`"));
    for wrong in [json!(false), json!([]), json!({}), json!("clear")] {
        let mut value = base.clone();
        value["artwork"] = wrong;
        assert!(serde_json::from_value::<SetWorkArtworkRequest>(value).is_err());
    }
    let duplicate =
        "{\"operation_id\":\"x\",\"expected_work_revision\":1,\"artwork\":null,\"artwork\":null}";
    assert_eq!(
        wire::decode_artwork::<SetWorkArtworkRequest>(WireFormat::Json, duplicate.as_bytes()),
        Err(DecodeError::Shape)
    );
}
#[test]
fn every_json_required_scalar_and_nested_value_fails_when_missing_or_null() {
    let cases = [
        ("declarations-unicode", "ImageDeclarations"),
        ("create-png", "CreateImageIntentRequest"),
        ("intent-document", "ImageIntentDocument"),
        ("variant-png", "ImageVariantMetadata"),
        ("asset-document", "ImageAssetDocument"),
        ("intent-pending-no-asset", "ImageIntentResponse"),
        ("selection-unicode", "WorkArtworkSelection"),
        ("set-artwork", "SetWorkArtworkRequest"),
        ("work-selected", "WorkArtworkResponse"),
    ];
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    for (name, message) in cases {
        let base = fixture(name);
        let validate = |value: Value| -> bool {
            match message {
                "ImageDeclarations" => serde_json::from_value::<ImageDeclarations>(value).is_err(),
                "CreateImageIntentRequest" => {
                    serde_json::from_value::<CreateImageIntentRequest>(value).is_err()
                }
                "ImageIntentDocument" => {
                    serde_json::from_value::<ImageIntentDocument>(value).is_err()
                }
                "ImageVariantMetadata" => {
                    serde_json::from_value::<ImageVariantMetadata>(value).is_err()
                }
                "ImageAssetDocument" => {
                    serde_json::from_value::<ImageAssetDocument>(value).is_err()
                }
                "ImageIntentResponse" => {
                    serde_json::from_value::<ImageIntentResponse>(value).is_err()
                }
                "WorkArtworkSelection" => {
                    serde_json::from_value::<WorkArtworkSelection>(value).is_err()
                }
                "SetWorkArtworkRequest" => {
                    serde_json::from_value::<SetWorkArtworkRequest>(value).is_err()
                }
                "WorkArtworkResponse" => {
                    serde_json::from_value::<WorkArtworkResponse>(value).is_err()
                }
                _ => unreachable!(),
            }
        };
        for field in schema["$defs"][message]["required"].as_array().unwrap() {
            let field = field.as_str().unwrap();
            let mut value = base.clone();
            value.as_object_mut().unwrap().remove(field);
            assert!(validate(value), "missing {message}.{field}");
            if message != "SetWorkArtworkRequest" || field != "artwork" {
                let mut value = base.clone();
                value[field] = Value::Null;
                assert!(validate(value), "null {message}.{field}");
            }
        }
        let mut value = base;
        value["public_url"] = json!("https://untrusted.invalid/cover");
        assert!(validate(value), "unknown {message} field");
    }
}
#[test]
fn schema_accepts_semantic_examples_and_refuses_invalid_intent_and_private_authority() {
    let schema: Value = serde_json::from_str(SCHEMA).unwrap();
    for (name, message) in [
        ("create-png", "CreateImageIntentRequest"),
        ("intent-document", "ImageIntentDocument"),
        ("asset-document", "ImageAssetDocument"),
        ("set-artwork", "SetWorkArtworkRequest"),
        ("clear-artwork", "SetWorkArtworkRequest"),
        ("work-selected", "WorkArtworkResponse"),
    ] {
        let mut root = schema.clone();
        root["$ref"] = json!(format!("#/$defs/{message}"));
        let validator = jsonschema::validator_for(&root).unwrap();
        assert!(validator.is_valid(&fixture(name)), "schema {name}");
    }
    let mut root = schema;
    root["$ref"] = json!("#/$defs/SetWorkArtworkRequest");
    let validator = jsonschema::validator_for(&root).unwrap();
    let mut missing = fixture("clear-artwork");
    missing.as_object_mut().unwrap().remove("artwork");
    assert!(!validator.is_valid(&missing));
}
#[test]
fn json_and_protobuf_enum_boundaries_are_closed_without_implicit_defaults() {
    for value in [json!("image/gif"), json!(0), Value::Null] {
        assert!(serde_json::from_value::<ImageMime>(value).is_err());
    }
    for value in [json!("accepted"), json!(0), Value::Null] {
        assert!(serde_json::from_value::<ImageIntentState>(value).is_err());
    }
    let raw: CreateImageIntentRequest =
        serde_json::from_value(fixture("create-full-uint64")).unwrap();
    assert_eq!(raw.expected_byte_length, u64::MAX);
    let raw: ImageVariantMetadata = serde_json::from_value(fixture("variant-full-uint32")).unwrap();
    assert_eq!(raw.width_px, u32::MAX);
}
#[test]
fn artwork_envelope_and_rights_limits_apply_without_changing_legacy_formats() {
    let boundary = ImageDeclarations {
        provenance: "x".repeat(MAX_ARTWORK_PROTOBUF_BYTES - 4),
        rights_refs: vec![],
    };
    let encoded = protobuf::encode(&boundary).unwrap();
    assert_eq!(encoded.len(), MAX_ARTWORK_PROTOBUF_BYTES);
    assert_eq!(
        protobuf::decode::<ImageDeclarations>(&encoded),
        Ok(boundary)
    );
    assert_eq!(
        protobuf::decode::<ImageDeclarations>(&vec![0; MAX_ARTWORK_PROTOBUF_BYTES + 1]),
        Err(DecodeError::TooLarge)
    );
    let allowed = ImageDeclarations {
        provenance: String::new(),
        rights_refs: vec![String::new(); MAX_ARTWORK_RIGHTS_REFS],
    };
    let encoded = protobuf::encode(&allowed).unwrap();
    assert_eq!(protobuf::decode::<ImageDeclarations>(&encoded), Ok(allowed));
    let mut wrong_wire_after_maximum = encoded;
    wrong_wire_after_maximum.extend([16, 0]);
    assert_eq!(
        protobuf::decode::<ImageDeclarations>(&wrong_wire_after_maximum),
        Err(DecodeError::Shape)
    );
    let too_many = ImageDeclarations {
        provenance: String::new(),
        rights_refs: vec![String::new(); MAX_ARTWORK_RIGHTS_REFS + 1],
    };
    assert_eq!(protobuf::encode(&too_many), Err(EncodeError));
    assert_eq!(
        protobuf::encode(&ImageDeclarations {
            provenance: "x".repeat(MAX_ARTWORK_PROTOBUF_BYTES),
            rights_refs: vec![]
        }),
        Err(EncodeError)
    );
    let request: SetWorkArtworkRequest = serde_json::from_value(fixture("clear-artwork")).unwrap();
    assert_eq!(
        wire::encode_artwork(WireFormat::MessagePack, &request),
        Err(EncodeError)
    );
    assert_eq!(
        wire::decode_artwork::<SetWorkArtworkRequest>(WireFormat::MessagePack, &[0x80]),
        Err(DecodeError::Shape)
    );
    assert_eq!(
        wire::decode_artwork::<SetWorkArtworkRequest>(
            WireFormat::Json,
            &vec![b' '; MAX_ARTWORK_PROTOBUF_BYTES + 1]
        ),
        Err(DecodeError::TooLarge)
    );
    // Existing generic JSON/MessagePack codec remains available with its unchanged semantics.
    let encoded = wire::encode(WireFormat::MessagePack, &request).unwrap();
    assert_eq!(
        wire::decode::<SetWorkArtworkRequest>(WireFormat::MessagePack, &encoded),
        Ok(request)
    );
}
#[test]
fn arbitrary_short_artwork_wire_bytes_never_panic() {
    for first in 0..=255_u8 {
        for second in 0..=255_u8 {
            let input = [first, second];
            for message in [
                "CreateImageIntentRequest",
                "ImageIntentResponse",
                "SetWorkArtworkRequest",
                "ImageVariantMetadata",
                "WorkArtworkResponse",
            ] {
                let _ = decode_case(message, &input);
            }
        }
    }
}

fn scalar_varint(input: &[u8], cursor: &mut usize) -> u64 {
    let mut value = 0;
    let mut shift = 0;
    loop {
        let byte = input[*cursor];
        *cursor += 1;
        value |= u64::from(byte & 127) << shift;
        if byte & 128 == 0 {
            return value;
        }
        shift += 7;
    }
}
fn omit_field(input: &[u8], omitted: u64) -> Vec<u8> {
    let mut result = vec![];
    let mut cursor = 0;
    while cursor < input.len() {
        let start = cursor;
        let key = scalar_varint(input, &mut cursor);
        match key & 7 {
            0 => {
                scalar_varint(input, &mut cursor);
            }
            2 => {
                let length = scalar_varint(input, &mut cursor);
                cursor += usize::try_from(length).unwrap();
            }
            other => panic!("unexpected oracle wire type {other}"),
        }
        if key >> 3 != omitted {
            result.extend_from_slice(&input[start..cursor]);
        }
    }
    result
}
#[test]
fn every_protobuf_required_scalar_and_message_retains_presence() {
    let corpus: Value = serde_json::from_str(GOLDENS).unwrap();
    for (name, message, fields) in [
        ("declarations-unicode", "ImageDeclarations", &[1][..]),
        (
            "create-png",
            "CreateImageIntentRequest",
            &[1, 2, 3, 4, 5, 6][..],
        ),
        (
            "intent-document",
            "ImageIntentDocument",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10][..],
        ),
        (
            "variant-png",
            "ImageVariantMetadata",
            &[1, 2, 3, 4, 5, 6][..],
        ),
        (
            "asset-document",
            "ImageAssetDocument",
            &[1, 2, 3, 4, 5, 6, 7, 8, 9][..],
        ),
        (
            "intent-admitted-with-asset",
            "ImageIntentResponse",
            &[1, 3][..],
        ),
        ("selection-unicode", "WorkArtworkSelection", &[1, 2, 3][..]),
        ("set-artwork", "SetWorkArtworkRequest", &[1, 2, 3][..]),
        ("clear-artwork", "SetWorkArtworkRequest", &[1, 2, 4][..]),
        ("work-selected", "WorkArtworkResponse", &[1, 2][..]),
    ] {
        let case = corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap();
        let input = bytes(case["protobuf_hex"].as_str().unwrap());
        for field in fields {
            assert_eq!(
                decode_case(message, &omit_field(&input, *field)),
                Err(DecodeError::Shape),
                "{message} field {field}"
            );
        }
    }
}
