//! Representation negotiation for the routes listed in `docs/architecture/wire-codec.md`.
//!
//! The pure decisions over header text come first and are tested without a server. The two
//! extractors and [`respond`] only move bytes. Extractors run before the handler body, so a
//! refused `Content-Type`, `Accept` or binary body is answered before the store is touched:
//! a representation failure can never follow a committed write. Errors stay JSON `ApiError`s
//! with the existing codes for every client.
use axum::{
    body::Bytes,
    extract::{FromRequest, FromRequestParts, Request},
    http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use cantos_contracts::{
    protobuf::ProtobufDto,
    wire::{self, WireFormat},
    ApiError, ErrorCode, FieldIssue,
};
use serde::{de::DeserializeOwned, Serialize};

use super::error_response;
use crate::platform::postgres::StoreError;

/// `Content-Type` positively named a representation these routes cannot decode.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct UnsupportedMediaType;

/// `Accept` listed media ranges, and none of them allows a supported representation.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct NotAcceptable;

/// The request body decoder. An absent or empty header takes the JSON path, so the JSON
/// extractor keeps answering exactly as before (400 `invalid_request`).
pub(super) fn request_format(
    content_type: Option<&str>,
) -> Result<WireFormat, UnsupportedMediaType> {
    match content_type {
        None => Ok(WireFormat::Json),
        Some(value) if wire::media_type_essence(value).is_empty() => Ok(WireFormat::Json),
        Some(value) => WireFormat::from_content_type(value).ok_or(UnsupportedMediaType),
    }
}

/// JSON wins absent, ambiguous and equal preferences. Protobuf wins a strict binary
/// preference, including an equal binary-only tie with legacy MessagePack. The most specific
/// matching media range sets each quality (RFC 9110 § 12.5.1).
pub(super) fn response_format(accept: Option<&str>) -> Result<WireFormat, NotAcceptable> {
    select_response_format(accept, true, true)
}

/// Catalog currently offers only JSON; a higher binary preference does not exclude
/// JSON when the client still allows it. Keep the existing header parser and quality rules.
pub(super) fn json_response_format(accept: Option<&str>) -> Result<WireFormat, NotAcceptable> {
    select_response_format(accept, false, false)
}

/// New artwork metadata offers JSON and Protobuf only. Reuse the frozen header parser
/// without widening legacy MessagePack or existing Catalog route capabilities.
pub(super) fn artwork_response_format(accept: Option<&str>) -> Result<WireFormat, NotAcceptable> {
    select_response_format(accept, false, true)
}

pub(super) fn artwork_request_format(
    content_type: Option<&str>,
) -> Result<WireFormat, UnsupportedMediaType> {
    match request_format(content_type)? {
        WireFormat::MessagePack => Err(UnsupportedMediaType),
        format => Ok(format),
    }
}

fn select_response_format(
    accept: Option<&str>,
    messagepack_allowed: bool,
    protobuf_allowed: bool,
) -> Result<WireFormat, NotAcceptable> {
    let ranges: Vec<MediaRange> = accept
        .into_iter()
        .flat_map(|value| value.split(','))
        .filter_map(MediaRange::parse)
        .collect();
    if ranges.is_empty() {
        return Ok(WireFormat::Json);
    }
    let json = quality(&ranges, "json");
    let msgpack = if messagepack_allowed {
        quality(&ranges, "msgpack")
    } else {
        0
    };
    let protobuf = if protobuf_allowed {
        quality(&ranges, "protobuf")
    } else {
        0
    };
    if json == 0 && msgpack == 0 && protobuf == 0 {
        Err(NotAcceptable)
    } else if protobuf > json && protobuf >= msgpack {
        Ok(WireFormat::Protobuf)
    } else if msgpack > json {
        Ok(WireFormat::MessagePack)
    } else {
        Ok(WireFormat::Json)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct MediaRange {
    kind: String,
    subtype: String,
    /// Thousandths: `q=0.5` is 500.
    quality: u16,
}

impl MediaRange {
    /// `None` for a malformed range, which is then ignored like any unknown token.
    fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split(';');
        let essence = parts.next()?.trim().to_ascii_lowercase();
        let (kind, subtype) = essence.split_once('/')?;
        let token = |part: &str| !part.is_empty() && !part.contains(char::is_whitespace);
        if !token(kind) || !token(subtype) || (kind == "*" && subtype != "*") {
            return None;
        }
        let mut quality = 1000;
        let protobuf = kind == "application"
            && matches!(
                subtype,
                "protobuf" | "x-protobuf" | "protobuf+json" | "x-protobuf+json"
            );
        let mut protobuf_type = essence.clone();
        let mut quality_seen = false;
        let mut invalid_parameters = false;
        let mut invalid_quality = false;
        for parameter in parts {
            let Some((name, value)) = parameter.split_once('=') else {
                if protobuf {
                    invalid_parameters = true;
                    continue;
                }
                return None;
            };
            if name.trim().eq_ignore_ascii_case("q") {
                invalid_parameters |= protobuf && quality_seen;
                quality_seen = true;
                match parse_quality(value.trim()) {
                    Some(parsed) => quality = parsed,
                    None if protobuf => invalid_quality = true,
                    None => return None,
                }
            } else if protobuf {
                protobuf_type.push(';');
                protobuf_type.push_str(parameter);
            }
        }
        // Keep an invalid protobuf range as an explicit exclusion. Otherwise a wildcard
        // could resurrect a version or encoding the client explicitly asked us to refuse.
        if protobuf
            && (invalid_parameters
                || WireFormat::from_content_type(&protobuf_type) != Some(WireFormat::Protobuf))
        {
            quality = 0;
        } else if invalid_quality {
            // A lone malformed q keeps the historical ignored-range behavior. Complete
            // protobuf parameter inspection first, so it cannot hide an explicit exclusion.
            return None;
        }
        Some(Self {
            kind: kind.to_owned(),
            subtype: subtype.to_owned(),
            quality,
        })
    }

    /// How specifically this range names `application/<subtype>`; `None` when it does not.
    fn specificity(&self, subtype: &str) -> Option<u8> {
        match (self.kind.as_str(), self.subtype.as_str()) {
            ("*", "*") => Some(1),
            ("application", "*") => Some(2),
            ("application", "x-protobuf" | "protobuf+json" | "x-protobuf+json")
                if subtype == "protobuf" =>
            {
                Some(3)
            }
            ("application", named) if named == subtype => Some(3),
            _ => None,
        }
    }
}

/// RFC 9110 qvalue: `0`–`1` with at most three decimals.
fn parse_quality(text: &str) -> Option<u16> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if fraction.len() > 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let thousandths: u16 = format!("{fraction:0<3}").parse().ok()?;
    match whole {
        "0" => Some(thousandths),
        "1" if thousandths == 0 => Some(1000),
        _ => None,
    }
}

/// The quality of the most specific range matching `application/<subtype>`; 0 when none does.
fn quality(ranges: &[MediaRange], subtype: &str) -> u16 {
    ranges
        .iter()
        .filter_map(|range| Some((range.specificity(subtype)?, range.quality)))
        .max()
        .map_or(0, |(_, quality)| quality)
}

fn refusal(status: StatusCode, header: &str, rule: &str) -> Response {
    let body = ApiError {
        code: ErrorCode::InvalidRequest,
        current_revision: None,
        issues: vec![FieldIssue {
            path: header.into(),
            rule: rule.into(),
        }],
    };
    (status, Json(body)).into_response()
}

fn vary_on_accept(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("accept"));
    response
}

/// The response representation for this request, decided before the handler runs.
pub(super) struct Accepts(pub(super) WireFormat);

impl<S: Send + Sync> FromRequestParts<S> for Accepts {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Response> {
        let accept = accept_header(&parts.headers);
        response_format(accept.as_deref())
            .map(Self)
            .map_err(|NotAcceptable| {
                vary_on_accept(refusal(
                    StatusCode::NOT_ACCEPTABLE,
                    "accept",
                    "not_acceptable",
                ))
            })
    }
}

/// Repeated `Accept` fields form one comma-separated list; a non-ASCII field is ignored.
pub(super) fn accept_header(headers: &HeaderMap) -> Option<String> {
    let values: Vec<&str> = headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect();
    (!values.is_empty()).then(|| values.join(","))
}

/// A pilot DTO decoded from JSON, legacy MessagePack or Protobuf before the handler runs.
pub(super) struct WireBody<T>(pub(super) T);

impl<T, S> FromRequest<S> for WireBody<T>
where
    T: DeserializeOwned + ProtobufDto + Send,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(request: Request, state: &S) -> Result<Self, Response> {
        let content_type = request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok());
        let invalid = || error_response(StoreError::InvalidRequest);
        match request_format(content_type) {
            Err(UnsupportedMediaType) => Err(refusal(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "content-type",
                "unsupported_media_type",
            )),
            Ok(WireFormat::Json) => Json::<T>::from_request(request, state)
                .await
                .map(|Json(value)| Self(value))
                .map_err(|_| invalid()),
            Ok(format @ (WireFormat::MessagePack | WireFormat::Protobuf)) => {
                let bytes = Bytes::from_request(request, state)
                    .await
                    .map_err(|_| invalid())?;
                wire::decode_pilot(format, &bytes)
                    .map(Self)
                    .map_err(|_| invalid())
            }
        }
    }
}

/// A successful result in the negotiated representation. JSON goes through `axum::Json`, the
/// same bytes these routes returned before negotiation existed.
pub(super) fn respond<T: Serialize + ProtobufDto>(format: WireFormat, value: T) -> Response {
    let response = match format {
        WireFormat::Json => Json(value).into_response(),
        WireFormat::MessagePack | WireFormat::Protobuf => {
            match wire::encode_pilot(format, &value) {
                Ok(body) => (
                    [(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static(format.media_type()),
                    )],
                    body,
                )
                    .into_response(),
                // The write may already be committed, so this must stay an ambiguous outcome that
                // the client reconciles with the same operation, never a definitive failure.
                Err(_) => return vary_on_accept(error_response(StoreError::Unavailable)),
            }
        }
    };
    vary_on_accept(response)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use axum::{
        body::Body,
        extract::{DefaultBodyLimit, State},
        routing::post,
        Router,
    };
    use cantos_contracts::SaveRevisionRequest;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    #[test]
    fn artwork_negotiation_is_json_default_and_explicit_protobuf_only() {
        use WireFormat::{Json as J, Protobuf as P};
        for (accept, expected) in [
            (None, Ok(J)),
            (Some("*/*"), Ok(J)),
            (Some("application/protobuf"), Ok(P)),
            (Some("application/msgpack"), Err(NotAcceptable)),
            (Some("application/msgpack,application/json;q=0.1"), Ok(J)),
            (
                Some("application/msgpack,application/protobuf;q=0.1"),
                Ok(P),
            ),
            (Some("application/protobuf,application/json"), Ok(J)),
            (Some("application/json;q=0,*/*"), Ok(P)),
            (
                Some("application/protobuf;q=0,application/json;q=0,*/*"),
                Err(NotAcceptable),
            ),
            (
                Some("application/protobuf;version=2,application/json;q=0,*/*"),
                Err(NotAcceptable),
            ),
            (Some("application/protobuf+json"), Err(NotAcceptable)),
        ] {
            assert_eq!(artwork_response_format(accept), expected, "{accept:?}");
        }
        for header in [
            "application/msgpack",
            "Application/Msgpack;x=1",
            "application/x-msgpack",
        ] {
            assert_eq!(
                artwork_request_format(Some(header)),
                Err(UnsupportedMediaType)
            );
        }
        assert_eq!(artwork_request_format(None), Ok(J));
        assert_eq!(artwork_request_format(Some("application/json")), Ok(J));
        assert_eq!(
            artwork_request_format(Some("application/x-protobuf;version=1")),
            Ok(P)
        );
    }

    #[test]
    fn json_only_routes_respect_json_quality_even_when_msgpack_is_preferred() {
        for (accept, allowed) in [
            (None, true),
            (Some("*/*"), true),
            (Some("application/msgpack,application/json;q=0.1"), true),
            (Some("application/json;q=0,*/*"), false),
            (Some("application/msgpack"), false),
            (Some("application/json;q=0"), false),
        ] {
            assert_eq!(json_response_format(accept).is_ok(), allowed, "{accept:?}");
        }
    }

    #[test]
    fn content_type_selects_the_decoder_and_refuses_only_positive_mismatches() {
        for (value, expected) in [
            (None, Ok(WireFormat::Json)),
            (Some(""), Ok(WireFormat::Json)),
            (Some(" ; charset=utf-8"), Ok(WireFormat::Json)),
            (Some("application/json"), Ok(WireFormat::Json)),
            (
                Some("application/json; charset=utf-8"),
                Ok(WireFormat::Json),
            ),
            (Some("application/merge-patch+json"), Ok(WireFormat::Json)),
            (Some("application/msgpack"), Ok(WireFormat::MessagePack)),
            (
                Some("Application/MsgPack; x=1"),
                Ok(WireFormat::MessagePack),
            ),
            (Some("application/x-msgpack"), Err(UnsupportedMediaType)),
            (Some("text/plain"), Err(UnsupportedMediaType)),
            (Some("multipart/form-data"), Err(UnsupportedMediaType)),
            (
                Some("application/x-www-form-urlencoded"),
                Err(UnsupportedMediaType),
            ),
        ] {
            assert_eq!(request_format(value), expected, "{value:?}");
        }
    }

    #[test]
    fn accept_keeps_json_unless_msgpack_is_strictly_preferred() {
        use WireFormat::{Json as J, MessagePack as M, Protobuf as P};
        for (accept, expected) in [
            (None, Ok(J)),
            (Some(""), Ok(J)),
            (Some(" , ,"), Ok(J)),
            (Some("garbage"), Ok(J)),
            (Some("*/*"), Ok(J)),
            (Some("application/*"), Ok(J)),
            (Some("application/json"), Ok(J)),
            (
                Some("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
                Ok(J),
            ),
            (Some("application/msgpack, application/json"), Ok(J)),
            (Some("application/json, application/msgpack"), Ok(J)),
            (Some("application/msgpack"), Ok(M)),
            (Some("APPLICATION/MSGPACK; charset=binary"), Ok(M)),
            (Some("application/msgpack, application/json;q=0.5"), Ok(M)),
            (
                Some("application/json;q=0.2, application/msgpack;q=0.8"),
                Ok(M),
            ),
            (Some("application/msgpack, */*;q=0.1"), Ok(M)),
            // The most specific range wins for each representation.
            (Some("application/json;q=0, */*"), Ok(P)),
            (Some("application/msgpack;q=0, */*"), Ok(J)),
            (
                Some("application/*;q=0.5, application/msgpack;q=0.4"),
                Ok(J),
            ),
            (
                Some("application/msgpack;q=0.4, application/json;q=0.2, application/json;q=0.9"),
                Ok(J),
            ),
            // An invalid quality voids its range; nothing valid left means no preference.
            (Some("application/msgpack;q=2"), Ok(J)),
            (Some("application/msgpack;q=0.0001"), Ok(J)),
            (Some("application/msgpack;q"), Ok(J)),
            (Some("text/*"), Err(NotAcceptable)),
            (Some("text/html"), Err(NotAcceptable)),
            (Some("application/x-msgpack"), Err(NotAcceptable)),
            (
                Some("application/cbor, application/xml"),
                Err(NotAcceptable),
            ),
            (Some("application/msgpack;q=0"), Err(NotAcceptable)),
            (
                Some("application/json;q=0, application/msgpack;q=0.000"),
                Err(NotAcceptable),
            ),
            (Some("*/*;q=0"), Err(NotAcceptable)),
        ] {
            assert_eq!(response_format(accept), expected, "{accept:?}");
        }
    }

    #[test]
    fn protobuf_headers_follow_the_frozen_parameter_and_quality_policy() {
        use WireFormat::{Json as J, MessagePack as M, Protobuf as P};
        for (header, expected) in [
            ("application/protobuf", Ok(P)),
            (
                " APPLICATION/X-PROTOBUF ; ENCODING=\"Binary\"; VERSION=\"1\" ",
                Ok(P),
            ),
            (
                "application/protobuf; charset=utf-8",
                Err(UnsupportedMediaType),
            ),
            ("application/protobuf; version=2", Err(UnsupportedMediaType)),
            (
                "application/protobuf; encoding=json",
                Err(UnsupportedMediaType),
            ),
            (
                "application/protobuf; version=1; version=1",
                Err(UnsupportedMediaType),
            ),
            ("application/protobuf; q=1", Err(UnsupportedMediaType)),
            ("application/protobuf+json", Err(UnsupportedMediaType)),
        ] {
            assert_eq!(request_format(Some(header)), expected, "{header}");
        }
        for (header, expected) in [
            ("application/protobuf", Ok(P)),
            (
                "application/x-protobuf; encoding=\"binary\"; version=1",
                Ok(P),
            ),
            ("application/protobuf,application/json", Ok(J)),
            ("application/msgpack,application/protobuf", Ok(P)),
            (
                "application/protobuf;q=0.4,application/msgpack;q=0.8",
                Ok(M),
            ),
            ("application/protobuf;q=0.8,application/json;q=0.7", Ok(P)),
            ("application/protobuf;q=0,*/*", Ok(J)),
            ("application/x-protobuf;q=0,application/json;q=0,*/*", Ok(M)),
            (
                "application/protobuf;q=0,application/json;q=0,application/msgpack;q=0,*/*",
                Err(NotAcceptable),
            ),
            ("application/protobuf;version=2", Err(NotAcceptable)),
            ("application/protobuf;encoding=json", Err(NotAcceptable)),
            ("application/protobuf;charset=binary", Err(NotAcceptable)),
            (
                "application/protobuf;version=1;version=1",
                Err(NotAcceptable),
            ),
            ("application/protobuf;q=1;q=1", Err(NotAcceptable)),
            ("application/protobuf+json", Err(NotAcceptable)),
            (
                "application/protobuf;version=2,application/json;q=0.1",
                Ok(J),
            ),
            (
                "application/protobuf;version=2,application/json;q=0,application/msgpack;q=0,*/*",
                Err(NotAcceptable),
            ),
            ("application/protobuf;q=2", Ok(J)),
        ] {
            assert_eq!(response_format(Some(header)), expected, "{header}");
        }
        for header in [
            "application/protobuf",
            "application/x-protobuf",
            "application/protobuf;version=1",
        ] {
            assert_eq!(json_response_format(Some(header)), Err(NotAcceptable));
        }
        assert_eq!(
            json_response_format(Some("application/protobuf,application/json;q=0.1")),
            Ok(J)
        );
    }

    #[tokio::test]
    async fn protobuf_hostile_and_oversized_envelopes_never_reach_the_handler() {
        let (router, calls) = probe(8 * 1024 * 1024);
        let valid = wire::encode_pilot(WireFormat::Protobuf, &save_request()).unwrap();
        let mut duplicate = valid.clone();
        duplicate.extend_from_slice(&[0x08, 0x03]);
        let mut unknown = valid.clone();
        unknown.extend_from_slice(&[0x20, 0x00]);
        let mut noncanonical = vec![0x08, 0x83, 0x00];
        noncanonical.extend_from_slice(&valid[2..]);
        for body in [
            vec![],
            vec![0x00],
            vec![
                0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02,
            ],
            vec![0x12, 0x01, 0xff],
            vec![0x1a, 0xff, 0xff, 0xff, 0xff, 0x07],
            vec![0x0a, 0x00],
            valid[..valid.len() - 1].to_vec(),
            duplicate,
            unknown,
            noncanonical,
            vec![0; 8 * 1024 * 1024 + 1],
        ] {
            let (status, headers, bytes) = send(
                &router,
                Some("application/protobuf"),
                Some("application/protobuf"),
                body,
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(headers[header::CONTENT_TYPE], "application/json");
            assert_eq!(
                api_error(&bytes),
                ApiError {
                    code: ErrorCode::InvalidRequest,
                    current_revision: None,
                    issues: vec![]
                }
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        // Refusals are final; a later explicit valid request reaches the handler exactly once.
        let (status, headers, bytes) = send(
            &router,
            Some("application/x-protobuf;version=1"),
            Some("application/x-protobuf"),
            valid,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers[header::CONTENT_TYPE], "application/protobuf");
        assert_eq!(
            wire::decode_pilot::<SaveRevisionRequest>(WireFormat::Protobuf, &bytes).unwrap(),
            save_request()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn quality_values_follow_rfc_9110_syntax() {
        for (text, expected) in [
            ("0", Some(0)),
            ("0.", Some(0)),
            ("0.5", Some(500)),
            ("0.25", Some(250)),
            ("0.125", Some(125)),
            ("1", Some(1000)),
            ("1.0", Some(1000)),
            ("1.000", Some(1000)),
            ("1.001", None),
            ("0.1234", None),
            ("0.5a", None),
            ("-0", None),
            ("2", None),
            ("", None),
            (".5", None),
        ] {
            assert_eq!(parse_quality(text), expected, "{text}");
        }
    }

    #[test]
    fn malformed_quality_cannot_hide_an_explicit_protobuf_exclusion() {
        for range in [
            "application/protobuf;version=2;q=bad",
            "application/protobuf;encoding=json;q=2",
            "application/protobuf;version=1;version=1;q=bad",
            "application/protobuf;q=0;q=bad",
            "application/x-protobuf;q=bad;q=0",
            "application/protobuf;unknown=x;q=bad",
            "application/protobuf+json;q=bad",
            "application/x-protobuf+json;q=2",
        ] {
            assert_eq!(response_format(Some(range)), Err(NotAcceptable), "{range}");
            let excluded = format!("{range},application/json;q=0,application/msgpack;q=0,*/*");
            assert_eq!(
                response_format(Some(&excluded)),
                Err(NotAcceptable),
                "{range}"
            );
            let json_allowed =
                format!("{range},application/json;q=0.5,application/msgpack;q=0,*/*");
            assert_eq!(
                response_format(Some(&json_allowed)),
                Ok(WireFormat::Json),
                "{range}"
            );
        }
        // Preserve the baseline's tolerance for a lone malformed quality with no other
        // unsupported protobuf parameter or duplicate. JSON/MessagePack handling is unchanged.
        assert_eq!(
            response_format(Some("application/protobuf;q=2")),
            Ok(WireFormat::Json)
        );
    }

    #[test]
    fn repeated_accept_fields_form_one_list() {
        let mut headers = HeaderMap::new();
        headers.append(
            header::ACCEPT,
            HeaderValue::from_static("application/json;q=0.1"),
        );
        headers.append(
            header::ACCEPT,
            HeaderValue::from_static("application/msgpack"),
        );
        assert_eq!(
            response_format(accept_header(&headers).as_deref()),
            Ok(WireFormat::MessagePack)
        );
        assert_eq!(accept_header(&HeaderMap::new()), None);
    }

    /// A route shaped like the save route; the counter proves which requests reached it.
    fn probe(limit: usize) -> (Router, Arc<AtomicUsize>) {
        async fn handler(
            State(calls): State<Arc<AtomicUsize>>,
            Accepts(format): Accepts,
            WireBody(request): WireBody<SaveRevisionRequest>,
        ) -> Response {
            calls.fetch_add(1, Ordering::SeqCst);
            respond(format, request)
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let router = Router::new()
            .route("/probe", post(handler))
            .layer(DefaultBodyLimit::max(limit))
            .with_state(calls.clone());
        (router, calls)
    }

    fn save_request() -> SaveRevisionRequest {
        SaveRevisionRequest {
            expected_revision: 3,
            operation_id: "00000000-0000-4000-8000-000000000009".into(),
            script_json: "{\"title\":\"Vọng Đài — “Tiếng vọng” 🎭\"}".into(),
        }
    }

    async fn send(
        router: &Router,
        content_type: Option<&str>,
        accept: Option<&str>,
        body: Vec<u8>,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut request = axum::http::Request::post("/probe");
        if let Some(value) = content_type {
            request = request.header(header::CONTENT_TYPE, value);
        }
        if let Some(value) = accept {
            request = request.header(header::ACCEPT, value);
        }
        let response = router
            .clone()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, headers, bytes.to_vec())
    }

    fn api_error(bytes: &[u8]) -> ApiError {
        serde_json::from_slice(bytes).unwrap()
    }

    #[tokio::test]
    async fn malformed_quality_with_unsupported_protobuf_version_refuses_before_handler() {
        let (router, calls) = probe(64 * 1024);
        let request = save_request();
        let (status, headers, bytes) = send(
            &router,
            Some("application/protobuf"),
            Some("application/protobuf;version=2;q=bad,application/json;q=0,application/msgpack;q=0,*/*"),
            wire::encode_pilot(WireFormat::Protobuf, &request).unwrap(),
        ).await;
        assert_eq!(status, StatusCode::NOT_ACCEPTABLE);
        assert_eq!(headers[header::CONTENT_TYPE], "application/json");
        assert_eq!(headers[header::VARY], "accept");
        assert_eq!(
            api_error(&bytes),
            ApiError {
                code: ErrorCode::InvalidRequest,
                current_revision: None,
                issues: vec![FieldIssue {
                    path: "accept".into(),
                    rule: "not_acceptable".into()
                }],
            }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn every_request_and_response_codec_pair_carries_the_same_dto() {
        let (router, calls) = probe(1024 * 1024);
        let request = save_request();
        for sent in [
            WireFormat::Json,
            WireFormat::MessagePack,
            WireFormat::Protobuf,
        ] {
            for wanted in [
                WireFormat::Json,
                WireFormat::MessagePack,
                WireFormat::Protobuf,
            ] {
                let body = wire::encode_pilot(sent, &request).unwrap();
                let (status, headers, bytes) = send(
                    &router,
                    Some(sent.media_type()),
                    Some(wanted.media_type()),
                    body,
                )
                .await;
                assert_eq!(status, StatusCode::OK);
                assert_eq!(headers[header::CONTENT_TYPE], wanted.media_type());
                assert_eq!(headers[header::VARY], "accept");
                let echoed: SaveRevisionRequest = wire::decode_pilot(wanted, &bytes).unwrap();
                assert_eq!(echoed, request, "{sent:?} → {wanted:?}");
            }
        }
        assert_eq!(calls.load(Ordering::SeqCst), 9);
    }

    #[tokio::test]
    async fn a_json_response_is_byte_identical_to_axum_json() {
        let (router, _) = probe(1024 * 1024);
        let request = save_request();
        let body = serde_json::to_vec(&request).unwrap();
        for accept in [None, Some("*/*"), Some("application/json")] {
            let (status, headers, bytes) =
                send(&router, Some("application/json"), accept, body.clone()).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(headers[header::CONTENT_TYPE], "application/json");
            assert_eq!(bytes, body, "{accept:?}");
        }
    }

    #[tokio::test]
    async fn refused_representations_never_reach_the_handler_and_stay_json_errors() {
        let (router, calls) = probe(64 * 1024);
        let msgpack = wire::encode(WireFormat::MessagePack, &save_request()).unwrap();
        let mut trailing = msgpack.clone();
        trailing.push(0xc0);
        let json = serde_json::to_vec(&save_request()).unwrap();
        let (bad, msgpack_type) = (StatusCode::BAD_REQUEST, Some("application/msgpack"));
        let cases = [
            (
                Some("text/plain"),
                None,
                msgpack.clone(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
            (
                msgpack_type,
                Some("text/html"),
                msgpack.clone(),
                StatusCode::NOT_ACCEPTABLE,
            ),
            (
                msgpack_type,
                None,
                msgpack[..msgpack.len() - 1].to_vec(),
                bad,
            ),
            (msgpack_type, None, trailing, bad),
            (msgpack_type, None, json.clone(), bad),
            (Some("application/json"), None, msgpack.clone(), bad),
            // No Content-Type keeps the JSON extractor's existing refusal.
            (None, None, json, bad),
            (
                Some("application/protobuf;version=2"),
                Some("application/protobuf"),
                msgpack.clone(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
            (
                Some("application/protobuf"),
                Some("application/protobuf;version=2"),
                msgpack.clone(),
                StatusCode::NOT_ACCEPTABLE,
            ),
            // Accept refusal precedes Content-Type refusal and byte decoding.
            (
                Some("application/protobuf;version=2"),
                Some("text/html"),
                msgpack.clone(),
                StatusCode::NOT_ACCEPTABLE,
            ),
            (msgpack_type, None, vec![0x81; 64 * 1024 + 1], bad),
        ];
        for (content_type, accept, body, status) in cases {
            let (actual, headers, bytes) = send(&router, content_type, accept, body).await;
            assert_eq!(actual, status, "{content_type:?} {accept:?}");
            assert_eq!(headers[header::CONTENT_TYPE], "application/json");
            let error = api_error(&bytes);
            assert_eq!(error.code, ErrorCode::InvalidRequest);
            assert_eq!(error.current_revision, None);
            let issue = |path: &str, rule: &str| FieldIssue {
                path: path.into(),
                rule: rule.into(),
            };
            let expected = match status {
                StatusCode::UNSUPPORTED_MEDIA_TYPE => {
                    vec![issue("content-type", "unsupported_media_type")]
                }
                StatusCode::NOT_ACCEPTABLE => vec![issue("accept", "not_acceptable")],
                _ => vec![],
            };
            assert_eq!(error.issues, expected);
            assert_eq!(
                headers.contains_key(header::VARY),
                status == StatusCode::NOT_ACCEPTABLE
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_failed_binary_encoding_is_reported_as_an_ambiguous_outcome() {
        struct Unencodable;
        impl Serialize for Unencodable {
            fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("synthetic failure"))
            }
        }
        impl ProtobufDto for Unencodable {
            fn encode_protobuf(&self) -> Result<Vec<u8>, wire::EncodeError> {
                wire::encode(WireFormat::Json, self)
            }
            fn decode_protobuf(_: &[u8]) -> Result<Self, wire::DecodeError> {
                Err(wire::DecodeError::Shape)
            }
        }
        for format in [WireFormat::MessagePack, WireFormat::Protobuf] {
            let response = respond(format, Unencodable);
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            assert_eq!(response.headers()[header::VARY], "accept");
        }
    }
}
