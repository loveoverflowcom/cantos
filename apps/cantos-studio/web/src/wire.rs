//! Representation choice for the negotiated revision routes (head, pinned revision, save and
//! validation); see `docs/architecture/wire-codec.md`. Pure, so host tests cover it; `api.rs`
//! only moves these bytes and headers through `fetch`.
//!
//! Requests declare a representation and responses are decoded by their own `Content-Type`:
//! errors are always JSON, and an older or intermediate server may answer JSON to a MessagePack
//! request. A request is sent exactly once; no failure is retried in the other representation,
//! because a write may already have committed (the editor reconciles its own operation).
use cantos_contracts::{
    wire::{self, WireFormat},
    ApiError, ErrorCode,
};
use serde::{de::DeserializeOwned, Serialize};

/// The single client switch for the negotiated routes. JSON stays the default until the
/// measurements in `docs/evidence/messagepack-wire.md` justify MessagePack; the server serves
/// both, so setting this back to `Json` is the whole client rollback.
pub const REVISION_CODEC: WireFormat = WireFormat::Json;

/// `Accept` for a request sent with `codec`. MessagePack still admits JSON at a lower quality,
/// so a server or route without MessagePack answers JSON instead of refusing.
pub const fn accept(codec: WireFormat) -> &'static str {
    match codec {
        WireFormat::Json => "application/json",
        WireFormat::MessagePack => "application/msgpack, application/json;q=0.5",
    }
}

/// A request body that cannot be encoded was never sent: no write can have happened.
// Both functions are inlined so the constant `REVISION_CODEC` prunes the unused codec branch
// from the WASM bundle; the evidence record measures that the JSON build links no decoder.
#[inline(always)]
pub fn encode_body<T: Serialize>(codec: WireFormat, value: &T) -> Result<Vec<u8>, ApiError> {
    wire::encode(codec, value).map_err(|_| unavailable())
}

/// Decodes one response by its actual `Content-Type`. MessagePack is read only when this client
/// asked for it, so a JSON build never links the MessagePack decoder. A success that cannot be
/// decoded is `Unavailable`: the write may have committed, so the outcome stays ambiguous and
/// the caller reconciles the same operation instead of assuming failure.
#[inline(always)]
pub fn decode_response<T: DeserializeOwned>(
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
            wire::decode(WireFormat::MessagePack, body).map_err(|_| unavailable())
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
    use cantos_contracts::{FieldIssue, RevisionResponse, SaveRevisionRequest};

    use super::*;

    fn revision() -> RevisionResponse {
        serde_json::from_str(include_str!(
            "../../../../packages/contracts/fixtures/studio/v1/revision.json"
        ))
        .unwrap()
    }

    fn stale() -> &'static [u8] {
        include_bytes!("../../../../packages/contracts/fixtures/studio/v1/stale-revision.json")
    }

    #[test]
    fn json_stays_the_default_and_its_request_bytes_match_the_previous_transport() {
        assert_eq!(REVISION_CODEC, WireFormat::Json);
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
        for requested in [WireFormat::Json, WireFormat::MessagePack] {
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
        for requested in [WireFormat::Json, WireFormat::MessagePack] {
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
