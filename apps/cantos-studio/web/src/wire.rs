//! Representation choice for the negotiated revision routes (head, pinned revision, save and
//! validation); see `docs/architecture/wire-codec.md`. Pure, so host tests cover it; `api.rs`
//! only moves these bytes and headers through `fetch`.
//!
//! Requests declare a representation and responses are decoded by their own `Content-Type`:
//! errors are always JSON, and an older or intermediate server may answer JSON to a Protobuf
//! request. A request is sent exactly once; no failure is retried in the other representation,
//! because a write may already have committed (the editor reconciles its own operation).
use crate::contract::{
    codec::{self as wire, WireFormat},
    protobuf::ProtobufDto,
    ApiError, ErrorCode,
};
use serde::{de::DeserializeOwned, Serialize};

/// The single client switch for the four pilot routes. The opt-in build feature exercises the
/// new binary path; the shipped default remains JSON until ADR 0023's compatibility, browser,
/// PostgreSQL and measurement activation gates pass. Rebuild without the feature for rollback.
#[cfg(not(feature = "revision-protobuf"))]
pub const REVISION_CODEC: WireFormat = WireFormat::Json;
#[cfg(feature = "revision-protobuf")]
pub const REVISION_CODEC: WireFormat = WireFormat::Protobuf;

/// Binary requests also admit a JSON success at lower quality. This is server negotiation,
/// never a second request or a client-side fallback after an uncertain write.
pub const fn accept(codec: WireFormat) -> &'static str {
    match codec {
        WireFormat::Json => "application/json",
        WireFormat::MessagePack => "application/msgpack, application/json;q=0.5",
        WireFormat::Protobuf => "application/protobuf, application/json;q=0.5",
    }
}

/// A request body that cannot be encoded was never sent: no write can have happened.
// Inlining lets the constant `REVISION_CODEC` prune unused binary branches. Bundle costs
// are measured separately for the shipped JSON and explicit Protobuf pilot builds.
#[inline(always)]
pub fn encode_body<T: Serialize + ProtobufDto>(
    codec: WireFormat,
    value: &T,
) -> Result<Vec<u8>, ApiError> {
    wire::encode_pilot(codec, value).map_err(|_| unavailable())
}

/// Decodes one response by its actual `Content-Type`. A binary success is read only when the
/// client offered that binary representation, keeping unexpected responses fail-closed. A success
/// that cannot be decoded is `Unavailable`: the write may have committed, so the outcome stays
/// ambiguous and the caller reconciles the same operation instead of assuming failure.
#[inline(always)]
pub fn decode_response<T: DeserializeOwned + ProtobufDto>(
    requested: WireFormat,
    ok: bool,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<T, ApiError> {
    let format = match content_type {
        // A missing type keeps the historical JSON reading of these routes.
        None => Some(WireFormat::Json),
        Some(value) => WireFormat::from_content_type(value),
    };
    match (ok, format, requested) {
        (true, Some(WireFormat::Json), _) => {
            wire::decode(WireFormat::Json, body).map_err(|_| unavailable())
        }
        (true, Some(WireFormat::MessagePack), WireFormat::MessagePack) => {
            wire::decode_pilot(WireFormat::MessagePack, body).map_err(|_| unavailable())
        }
        (true, Some(WireFormat::Protobuf), WireFormat::Protobuf) => {
            wire::decode_pilot(WireFormat::Protobuf, body).map_err(|_| unavailable())
        }
        (false, Some(WireFormat::Json), _) => {
            Err(wire::decode(WireFormat::Json, body).unwrap_or_else(|_| unavailable()))
        }
        _ => Err(unavailable()),
    }
}

fn unavailable() -> ApiError {
    ApiError {
        code: ErrorCode::Unavailable,
        current_revision: None,
        issues: vec![],
    }
}

#[cfg(test)]
mod tests {
    use crate::contract::{FieldIssue, RevisionResponse, SaveRevisionRequest};

    use super::*;

    fn revision() -> RevisionResponse {
        serde_json::from_str(include_str!(
            "../../../../services/cantos-server/contracts/studio/editorial/v1/fixtures/revision.json"
        ))
        .unwrap()
    }

    fn stale() -> &'static [u8] {
        include_bytes!("../../../../services/cantos-server/contracts/studio/editorial/v1/fixtures/stale-revision.json")
    }

    #[test]
    fn json_rollback_request_bytes_match_the_previous_transport() {
        #[cfg(not(feature = "revision-protobuf"))]
        assert_eq!(REVISION_CODEC, WireFormat::Json);
        #[cfg(feature = "revision-protobuf")]
        assert_eq!(REVISION_CODEC, WireFormat::Protobuf);
        assert_eq!(accept(WireFormat::Json), "application/json");
        let request = SaveRevisionRequest {
            expected_revision: 1,
            operation_id: "00000000-0000-4000-8000-000000000004".into(),
            script_json: "{\"title\":\"Ngày mai\"}".into(),
        };
        // gloo-net's `.json()` sent `serde_json::to_string`; the JSON plan sends the same bytes.
        assert_eq!(
            encode_body(WireFormat::Json, &request).unwrap(),
            serde_json::to_string(&request).unwrap().into_bytes()
        );
        assert_eq!(
            encode_body(WireFormat::MessagePack, &request).unwrap()[0],
            0x83,
            "a named map"
        );
    }

    #[test]
    fn msgpack_requests_still_accept_a_json_answer_at_lower_quality() {
        assert_eq!(
            accept(WireFormat::MessagePack),
            "application/msgpack, application/json;q=0.5"
        );
    }

    #[test]
    fn responses_are_decoded_by_their_own_content_type() {
        let value = revision();
        let json = wire::encode(WireFormat::Json, &value).unwrap();
        let msgpack = wire::encode(WireFormat::MessagePack, &value).unwrap();
        for requested in [
            WireFormat::Json,
            WireFormat::MessagePack,
            WireFormat::Protobuf,
        ] {
            for content_type in [
                None,
                Some("application/json"),
                Some("application/json; charset=utf-8"),
            ] {
                assert_eq!(
                    decode_response::<RevisionResponse>(requested, true, content_type, &json),
                    Ok(value.clone())
                );
            }
        }
        assert_eq!(
            decode_response::<RevisionResponse>(
                WireFormat::MessagePack,
                true,
                Some("application/msgpack"),
                &msgpack
            ),
            Ok(value)
        );
    }

    #[test]
    fn errors_are_json_whatever_was_requested() {
        for requested in [
            WireFormat::Json,
            WireFormat::MessagePack,
            WireFormat::Protobuf,
        ] {
            let error = decode_response::<RevisionResponse>(
                requested,
                false,
                Some("application/json"),
                stale(),
            )
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::StaleRevision);
            assert_eq!(error.current_revision, Some(2));
        }
        let refused = br#"{"code":"invalid_request","current_revision":null,"issues":[{"path":"accept","rule":"not_acceptable"}]}"#;
        assert_eq!(
            decode_response::<RevisionResponse>(
                WireFormat::MessagePack,
                false,
                Some("application/json"),
                refused
            )
            .unwrap_err()
            .issues,
            vec![FieldIssue {
                path: "accept".into(),
                rule: "not_acceptable".into()
            }]
        );
    }

    #[test]
    fn an_undecodable_success_is_ambiguous_and_never_a_definitive_failure() {
        let msgpack = wire::encode(WireFormat::MessagePack, &revision()).unwrap();
        let json = wire::encode(WireFormat::Json, &revision()).unwrap();
        for (requested, ok, content_type, body) in [
            // Truncated after a possible commit.
            (
                WireFormat::MessagePack,
                true,
                Some("application/msgpack"),
                &msgpack[..10],
            ),
            (
                WireFormat::Json,
                true,
                Some("application/json"),
                &json[..10],
            ),
            // A MessagePack body this JSON build never asked for.
            (
                WireFormat::Json,
                true,
                Some("application/msgpack"),
                &msgpack[..],
            ),
            // A proxy page, an unknown type, a mislabelled body.
            (
                WireFormat::MessagePack,
                true,
                Some("text/html"),
                b"<html>".as_slice(),
            ),
            (
                WireFormat::MessagePack,
                true,
                Some("application/msgpack"),
                &json[..],
            ),
            (
                WireFormat::MessagePack,
                false,
                Some("text/html"),
                b"<html>".as_slice(),
            ),
            (
                WireFormat::MessagePack,
                false,
                Some("application/msgpack"),
                &msgpack[..],
            ),
            (
                WireFormat::Json,
                false,
                Some("application/json"),
                b"{".as_slice(),
            ),
        ] {
            assert_eq!(
                decode_response::<RevisionResponse>(requested, ok, content_type, body),
                Err(unavailable()),
                "{requested:?} {ok} {content_type:?}"
            );
        }
    }
}

#[cfg(test)]
#[path = "wire/protobuf_tests.rs"]
mod protobuf_tests;
