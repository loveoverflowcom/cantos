//! Studio Web's own byte representations of its contract DTOs: JSON and MessagePack carry the
//! same values, and the editorial pilot roots also have a Protobuf representation.
//!
//! This module owns how this client turns a DTO into bytes and back. It does not decide which
//! route uses which representation; route support, `Content-Type`/`Accept` negotiation and error
//! policy are specified in `docs/architecture/wire-codec.md` (the request choice is in
//! `crate::wire`). The Server keeps its own codec for the same canonical goldens.
//!
//! - MessagePack is always a map keyed by field name, never a positional array, so an added,
//!   reordered or optional field cannot shift the meaning of another, and `deny_unknown_fields`
//!   rejects unknown keys in both representations alike.
//! - Decoding reads exactly one value from an already-buffered slice: nesting is bounded, a
//!   length prefix larger than the remaining input fails before anything is allocated for it,
//!   and trailing bytes are refused, matching JSON's rejection of trailing characters.
use std::io;

use serde::{
    de::{DeserializeOwned, IgnoredAny},
    Deserialize, Serialize,
};

pub const PROTOBUF_MEDIA_TYPE: &str = "application/protobuf";

pub const JSON_MEDIA_TYPE: &str = "application/json";
/// The only MessagePack media type Cantos reads or writes. Aliases such as
/// `application/x-msgpack` are refused until a consumer needs one.
pub const MSGPACK_MEDIA_TYPE: &str = "application/msgpack";

/// Nesting bound for MessagePack input: a value may nest fewer arrays and maps than this. Contract
/// DTOs nest far less (a test pins the margin); unknown fields are skipped recursively, so without
/// a bound a small hostile body could exhaust the stack (rmp-serde's own default is 1024).
pub const MAX_MSGPACK_DEPTH: usize = 16;

/// Baseline encodings and the schema-reviewed Studio pilot representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireFormat {
    Json,
    /// Named-map MessagePack.
    MessagePack,
    /// Reviewed versioned Studio metadata; use its explicit codec composition.
    Protobuf,
}

impl WireFormat {
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Json => JSON_MEDIA_TYPE,
            Self::MessagePack => MSGPACK_MEDIA_TYPE,
            Self::Protobuf => PROTOBUF_MEDIA_TYPE,
        }
    }

    /// Classifies actual `Content-Type`: JSON and legacy MessagePack keep their baseline
    /// rules; Protobuf accepts only reviewed RFC 9996 binary parameters and x-protobuf
    /// alias. ProtoJSON is deliberately refused; arbitrary +json remains baseline JSON.
    pub fn from_content_type(value: &str) -> Option<Self> {
        let essence = media_type_essence(value);
        if essence == PROTOBUF_MEDIA_TYPE || essence == "application/x-protobuf" {
            return protobuf_parameters_valid(value).then_some(Self::Protobuf);
        }
        if essence == "application/protobuf+json" || essence == "application/x-protobuf+json" {
            return None;
        }
        if essence == MSGPACK_MEDIA_TYPE {
            Some(Self::MessagePack)
        } else if essence == JSON_MEDIA_TYPE
            || (essence.starts_with("application/") && essence.ends_with("+json"))
        {
            Some(Self::Json)
        } else {
            None
        }
    }
}

/// RFC 9996 binary parameters, deliberately strict for the reviewed schema pilot.
fn protobuf_parameters_valid(value: &str) -> bool {
    let mut encoding_seen = false;
    let mut version_seen = false;
    for parameter in value.split(';').skip(1) {
        let Some((name, raw)) = parameter.split_once('=') else {
            return false;
        };
        let raw = raw.trim();
        let parameter_value = if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
            &raw[1..raw.len() - 1]
        } else {
            raw
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "encoding" if !encoding_seen && parameter_value.eq_ignore_ascii_case("binary") => {
                encoding_seen = true
            }
            "version" if !version_seen && parameter_value == "1" => version_seen = true,
            _ => return false,
        }
    }
    true
}

/// `type/subtype` in ASCII lowercase, without parameters or surrounding whitespace.
pub fn media_type_essence(value: &str) -> String {
    value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

/// Why bytes are not exactly one value of the expected DTO.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The input ended inside a value.
    Truncated,
    /// Buffered protobuf bytes, field count or repeated issues exceeded its bound.
    TooLarge,
    /// MessagePack nesting exceeded [`MAX_MSGPACK_DEPTH`].
    TooDeep,
    /// A complete MessagePack value was followed by more bytes.
    TrailingBytes,
    /// The JSON body or a MessagePack string was not valid UTF-8.
    InvalidUtf8,
    /// Well-formed input that is not this DTO: a wrong type, a missing, duplicate or unknown
    /// field, or an out-of-range number.
    Shape,
    /// Input that is not well formed: JSON syntax, or a MessagePack marker no DTO uses.
    Malformed,
}

/// A DTO could not be serialized; no bytes were produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodeError;

#[inline]
pub fn encode<T: Serialize + ?Sized>(
    format: WireFormat,
    value: &T,
) -> Result<Vec<u8>, EncodeError> {
    match format {
        WireFormat::Json => serde_json::to_vec(value).map_err(|_| EncodeError),
        WireFormat::MessagePack => rmp_serde::to_vec_named(value).map_err(|_| EncodeError),
        WireFormat::Protobuf => Err(EncodeError),
    }
}

/// Decodes exactly one DTO. MessagePack input must be a map at the top level: every contract DTO
/// is a struct, and a top-level array is what a positional encoder writes.
#[inline]
pub fn decode<T: DeserializeOwned>(format: WireFormat, bytes: &[u8]) -> Result<T, DecodeError> {
    match format {
        WireFormat::Json => decode_json(bytes),
        WireFormat::MessagePack => decode_msgpack(bytes),
        WireFormat::Protobuf => Err(DecodeError::Shape),
    }
}

/// Only schema-reviewed pilot DTOs may use Protobuf; generic Serde helpers stay unchanged.
#[inline]
pub fn encode_pilot<T: Serialize + super::protobuf::ProtobufDto + ?Sized>(
    format: WireFormat,
    value: &T,
) -> Result<Vec<u8>, EncodeError> {
    match format {
        WireFormat::Protobuf => super::protobuf::encode(value),
        WireFormat::Json | WireFormat::MessagePack => encode(format, value),
    }
}

/// Decode a pilot by the actual negotiated representation, preserving baseline JSON semantics.
#[inline]
pub fn decode_pilot<T: DeserializeOwned + super::protobuf::ProtobufDto>(
    format: WireFormat,
    bytes: &[u8],
) -> Result<T, DecodeError> {
    match format {
        WireFormat::Protobuf => super::protobuf::decode(bytes),
        WireFormat::Json | WireFormat::MessagePack => decode(format, bytes),
    }
}

/// Text first, then `from_str`: the same parser instantiation a browser client already links
/// for `Response::json`, and the same refusal of invalid UTF-8 anywhere in the body.
fn decode_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    let text = std::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
    serde_json::from_str(text).map_err(json_error)
}

fn decode_msgpack<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    match bytes.first() {
        None => return Err(DecodeError::Truncated),
        Some(0x80..=0x8f | 0xde | 0xdf) => {}
        Some(_) => return Err(DecodeError::Shape),
    }
    let mut deserializer = rmp_serde::Deserializer::from_read_ref(bytes);
    deserializer.set_max_depth(MAX_MSGPACK_DEPTH);
    let value = T::deserialize(&mut deserializer).map_err(msgpack_error)?;
    // rmp-serde stops after one value. Reading a second one must hit the end of the input
    // immediately; anything else, including garbage, means trailing bytes.
    match IgnoredAny::deserialize(&mut deserializer) {
        Err(rmp_serde::decode::Error::InvalidMarkerRead(error))
            if error.kind() == io::ErrorKind::UnexpectedEof =>
        {
            Ok(value)
        }
        _ => Err(DecodeError::TrailingBytes),
    }
}

fn msgpack_error(error: rmp_serde::decode::Error) -> DecodeError {
    use rmp_serde::decode::Error;
    match error {
        Error::InvalidMarkerRead(error) | Error::InvalidDataRead(error)
            if error.kind() == io::ErrorKind::UnexpectedEof =>
        {
            DecodeError::Truncated
        }
        Error::DepthLimitExceeded => DecodeError::TooDeep,
        Error::Utf8Error(_) => DecodeError::InvalidUtf8,
        Error::TypeMismatch(rmp::Marker::Reserved) => DecodeError::Malformed,
        Error::TypeMismatch(_)
        | Error::OutOfRange
        | Error::LengthMismatch(_)
        | Error::Uncategorized(_)
        | Error::Syntax(_) => DecodeError::Shape,
        Error::InvalidMarkerRead(_) | Error::InvalidDataRead(_) => DecodeError::Malformed,
    }
}

fn json_error(error: serde_json::Error) -> DecodeError {
    match error.classify() {
        serde_json::error::Category::Eof => DecodeError::Truncated,
        serde_json::error::Category::Data => DecodeError::Shape,
        serde_json::error::Category::Syntax | serde_json::error::Category::Io => {
            DecodeError::Malformed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_classification_ignores_parameters_and_case_and_refuses_aliases() {
        for (value, expected) in [
            ("application/json", Some(WireFormat::Json)),
            ("Application/JSON; charset=utf-8", Some(WireFormat::Json)),
            ("application/merge-patch+json", Some(WireFormat::Json)),
            (" application/msgpack ", Some(WireFormat::MessagePack)),
            (
                "APPLICATION/MSGPACK; charset=binary",
                Some(WireFormat::MessagePack),
            ),
            ("application/x-msgpack", None),
            ("application/vnd.msgpack", None),
            ("application/vnd.cantos+msgpack", None),
            ("text/json", None),
            ("text/plain", None),
            ("", None),
        ] {
            assert_eq!(WireFormat::from_content_type(value), expected, "{value}");
        }
    }

    #[test]
    fn media_types_round_trip_through_classification() {
        for format in [
            WireFormat::Json,
            WireFormat::MessagePack,
            WireFormat::Protobuf,
        ] {
            assert_eq!(
                WireFormat::from_content_type(format.media_type()),
                Some(format)
            );
        }
    }
}
