//! Anonymous public Work detail and unit-page reads. No identity cookie, credential, write,
//! playback authorization or player effect. Decoding is pure and strict over the shared DTOs.
use crate::contract::discovery::{
    ListenerError, ListenerErrorCode, MetadataUnitPage, MetadataWorkSummary, UnitKind,
};
use crate::{
    application::public_work::{validate_page, validate_work, CatalogReadError, UNIT_PAGE_LIMIT},
    core::public_ids::PublicWorkId,
};
use cantos_web_auth::address::THEATRE_BASE;

// One legal Work: 512-byte title, 16 KiB description and short language, with worst-case
// `\u` escaping (6×) plus framing.
const MAX_WORK_BYTES: usize = 262_144;
// Twenty legal units, each with a 16 KiB description and 512-byte title, with JSON escaping.
const MAX_PAGE_BYTES: usize = 2_097_152;
const MAX_ERROR_BYTES: usize = 4_096;

/// Work and Units use the single generation representation, with explicit nullable playback.
/// The strict v1 DTOs are never extended.
pub fn work_path(work: PublicWorkId) -> String {
    format!("{THEATRE_BASE}/works/{work}")
}

/// The cursor is opaque: it is only form-encoded, never parsed or persisted.
pub fn units_path(work: PublicWorkId, cursor: Option<&str>) -> String {
    let mut path = format!("{THEATRE_BASE}/works/{work}/units?limit={UNIT_PAGE_LIMIT}");
    if let Some(cursor) = cursor {
        path.push_str("&cursor=");
        path.extend(url::form_urlencoded::byte_serialize(cursor.as_bytes()));
    }
    path
}

pub(super) fn is_json(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    })
}

/// Only the exact contract pairing of HTTP status, code and retryability is trusted. An uncoded
/// answer (proxy page, outage) is unavailable, never `not_found`.
pub(super) fn classify_error(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
) -> CatalogReadError {
    use crate::application::errors::ErrorKind;
    let diagnostic = match status {
        401 => Some(ErrorKind::Unauthenticated),
        403 => Some(ErrorKind::Forbidden),
        408 | 504 => Some(ErrorKind::Timeout),
        _ => None,
    };
    if let Some(kind) = diagnostic {
        return CatalogReadError::Diagnostic(kind);
    }
    let coded = (is_json(content_type) && body.len() <= MAX_ERROR_BYTES)
        .then(|| serde_json::from_slice::<ListenerError>(body).ok())
        .flatten();
    match (status, coded.map(|error| (error.code, error.retryable))) {
        (404, Some((ListenerErrorCode::NotFound, false))) => CatalogReadError::NotFound,
        (400, Some((ListenerErrorCode::InvalidCursor, false))) => CatalogReadError::InvalidCursor,
        (
            503,
            Some((ListenerErrorCode::Unavailable | ListenerErrorCode::RenditionUnavailable, true)),
        ) => CatalogReadError::Unavailable,
        // Stored corruption is not retryable: never blame the connection or invite a retry loop.
        (500, Some((ListenerErrorCode::Unavailable, false))) => CatalogReadError::Protocol,
        (_, Some(_)) => CatalogReadError::Protocol,
        (_, None) => CatalogReadError::Unavailable,
    }
}

pub fn decode_work(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
    expected: PublicWorkId,
) -> Result<MetadataWorkSummary, CatalogReadError> {
    if status != 200 {
        return Err(match classify_error(status, content_type, body) {
            // The detail read sends no cursor.
            CatalogReadError::InvalidCursor => CatalogReadError::Protocol,
            error => error,
        });
    }
    if body.len() > MAX_WORK_BYTES || !is_json(content_type) {
        return Err(CatalogReadError::Protocol);
    }
    let work: MetadataWorkSummary =
        serde_json::from_slice(body).map_err(|_| CatalogReadError::Protocol)?;
    if !validate_work(&work, expected) {
        return Err(CatalogReadError::Protocol);
    }
    Ok(work)
}

pub fn decode_units(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
    work: PublicWorkId,
    kind: UnitKind,
    cursor_sent: bool,
) -> Result<MetadataUnitPage, CatalogReadError> {
    if status != 200 {
        return match classify_error(status, content_type, body) {
            // Page one carries no cursor, so it cannot be an invalid cursor.
            CatalogReadError::InvalidCursor if !cursor_sent => Err(CatalogReadError::Protocol),
            error => Err(error),
        };
    }
    if body.len() > MAX_PAGE_BYTES || !is_json(content_type) {
        return Err(CatalogReadError::Protocol);
    }
    let page: MetadataUnitPage =
        serde_json::from_slice(body).map_err(|_| CatalogReadError::Protocol)?;
    if !validate_page(&page, work, kind) {
        return Err(CatalogReadError::Protocol);
    }
    Ok(page)
}

#[cfg(target_arch = "wasm32")]
pub(super) async fn get(
    path: &str,
    limit: usize,
) -> Result<(u16, Option<String>, Vec<u8>), CatalogReadError> {
    use gloo_net::http::Request;
    use web_sys::{AbortController, RequestCredentials};

    let controller = AbortController::new().map_err(|_| {
        CatalogReadError::Diagnostic(crate::application::errors::ErrorKind::Network)
    })?;
    // Held until the body is read; dropping it clears the timer and aborts the request.
    let deadline = super::browser::deadline(controller).map_err(|_| {
        CatalogReadError::Diagnostic(crate::application::errors::ErrorKind::Network)
    })?;
    let signal = deadline.controller.signal();
    let response = Request::get(&crate::api_endpoint::endpoint(path).map_err(|_| {
        CatalogReadError::Diagnostic(crate::application::errors::ErrorKind::Configuration)
    })?)
    .credentials(RequestCredentials::Omit)
    .header("Accept", "application/json")
    .abort_signal(Some(&signal))
    .send()
    .await
    .map_err(|_| CatalogReadError::Diagnostic(deadline.failure_kind()))?;
    let declared = response
        .headers()
        .get("content-length")
        .and_then(|value| value.trim().parse::<usize>().ok());
    if declared.is_some_and(|length| length > limit) {
        return Err(CatalogReadError::Protocol);
    }
    let content_type = response.headers().get("content-type");
    let body = response
        .binary()
        .await
        .map_err(|_| CatalogReadError::Diagnostic(deadline.failure_kind()))?;
    Ok((response.status(), content_type, body))
}

#[cfg(target_arch = "wasm32")]
pub async fn load_work(work: PublicWorkId) -> Result<MetadataWorkSummary, CatalogReadError> {
    let (status, content_type, body) = get(&work_path(work), MAX_WORK_BYTES).await?;
    decode_work(status, content_type.as_deref(), &body, work)
}

#[cfg(target_arch = "wasm32")]
pub async fn load_units(
    ticket: &crate::application::public_work::PageTicket,
) -> Result<MetadataUnitPage, CatalogReadError> {
    let (status, content_type, body) =
        get(&units_path(ticket.work(), ticket.cursor()), MAX_PAGE_BYTES).await?;
    decode_units(
        status,
        content_type.as_deref(),
        &body,
        ticket.work(),
        ticket.kind(),
        ticket.cursor().is_some(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: &str = "10000000-0000-4000-8000-000000000001";
    const WORKS: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/covered_works.json"
    );
    const UNITS: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/presented_units.json"
    );
    const CONTINUATION: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/units_empty_continuation.json"
    );
    const END_EMPTY: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/units_end_empty.json"
    );
    const REPUBLISHED: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/units_republished.json"
    );
    const NOT_FOUND: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/not_found.json"
    );
    const INVALID_CURSOR: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/invalid_cursor.json"
    );
    const INVALID_REQUEST: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/invalid_request.json"
    );
    const UNAVAILABLE: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/unavailable.json"
    );
    const RENDITION_UNAVAILABLE: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/rendition_unavailable.json"
    );
    const JSON: Option<&str> = Some("application/json; charset=utf-8");

    fn work() -> PublicWorkId {
        PublicWorkId::parse(WORK).unwrap()
    }
    fn work_body() -> Vec<u8> {
        let page: serde_json::Value = serde_json::from_str(WORKS).unwrap();
        serde_json::to_vec(&page["items"][0]).unwrap()
    }
    /// The shared continuation/republish fixtures predate `ordinal`; rank their rows in list
    /// order as the presentation capability does. Anything that is not such a page is untouched.
    fn ranked(body: &str) -> String {
        match serde_json::from_str::<serde_json::Value>(body) {
            Ok(mut value) if value["items"].is_array() => {
                for (index, item) in value["items"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .enumerate()
                {
                    if item.get("ordinal").is_none() {
                        item["ordinal"] = serde_json::json!(index + 1);
                    }
                }
                value.to_string()
            }
            _ => body.to_owned(),
        }
    }
    fn units(
        status: u16,
        body: &str,
        cursor_sent: bool,
    ) -> Result<MetadataUnitPage, CatalogReadError> {
        decode_units(
            status,
            JSON,
            ranked(body).as_bytes(),
            work(),
            UnitKind::Chapter,
            cursor_sent,
        )
    }

    #[test]
    fn paths_use_the_theatre_group_canonical_id_and_an_encoded_opaque_cursor() {
        assert_eq!(
            work_path(work()),
            "/api/v1/theatre/works/10000000-0000-4000-8000-000000000001"
        );
        assert_eq!(
            units_path(work(), None),
            "/api/v1/theatre/works/10000000-0000-4000-8000-000000000001/units?limit=20"
        );
        assert!(units_path(work(), Some("lc1_a+b/c=&d")).ends_with("&cursor=lc1_a%2Bb%2Fc%3D%26d"));
    }

    #[test]
    fn shared_work_fixture_decodes_and_must_match_the_route_identity() {
        let decoded = decode_work(200, JSON, &work_body(), work()).unwrap();
        assert_eq!(decoded.title, "Ánh đèn cuối sân khấu");
        let other = PublicWorkId::parse("10000000-0000-4000-8000-000000000002").unwrap();
        assert_eq!(
            decode_work(200, JSON, &work_body(), other),
            Err(CatalogReadError::Protocol)
        );
        let mut extra: serde_json::Value = serde_json::from_slice(&work_body()).unwrap();
        extra["cover_url"] = serde_json::json!("https://example.invalid/c.png");
        for (kind, body) in [
            (None, work_body()),
            (Some("text/html"), work_body()),
            (JSON, serde_json::to_vec(&extra).unwrap()),
            (JSON, b"{}".to_vec()),
            (JSON, vec![b' '; MAX_WORK_BYTES + 1]),
        ] {
            assert_eq!(
                decode_work(200, kind, &body, work()),
                Err(CatalogReadError::Protocol)
            );
        }
    }

    #[test]
    fn the_detail_cover_is_required_nullable_and_bound_to_the_route_work() {
        let mut value: serde_json::Value = serde_json::from_slice(&work_body()).unwrap();
        assert!(decode_work(200, JSON, &work_body(), work())
            .unwrap()
            .cover
            .is_some());
        value["cover"] = serde_json::Value::Null;
        let no_cover = serde_json::to_vec(&value).unwrap();
        assert!(decode_work(200, JSON, &no_cover, work())
            .unwrap()
            .cover
            .is_none());
        value.as_object_mut().unwrap().remove("cover");
        let missing = serde_json::to_vec(&value).unwrap();
        // A cover of the right shape for a different Work is not this Work's cover.
        let mut foreign: serde_json::Value = serde_json::from_slice(&work_body()).unwrap();
        let url = foreign["cover"]["url"]
            .as_str()
            .unwrap()
            .replace("0001/cover", "0002/cover");
        foreign["cover"]["url"] = url.into();
        let foreign = serde_json::to_vec(&foreign).unwrap();
        // The presentation=3 detail has no `cover` key at all.
        for body in [missing, foreign] {
            assert_eq!(
                decode_work(200, JSON, &body, work()),
                Err(CatalogReadError::Protocol)
            );
        }
    }

    #[test]
    fn contract_errors_classify_exactly_and_uncoded_answers_are_never_not_found() {
        let cases = [
            (404, NOT_FOUND, CatalogReadError::NotFound),
            (503, UNAVAILABLE, CatalogReadError::Unavailable),
            (503, RENDITION_UNAVAILABLE, CatalogReadError::Unavailable),
            (
                500,
                r#"{"code":"unavailable","retryable":false}"#,
                CatalogReadError::Protocol,
            ),
            (400, INVALID_REQUEST, CatalogReadError::Protocol),
            (400, INVALID_CURSOR, CatalogReadError::Protocol),
            (200, NOT_FOUND, CatalogReadError::Protocol),
            (503, NOT_FOUND, CatalogReadError::Protocol),
            (404, "<html>Not Found</html>", CatalogReadError::Unavailable),
            (502, "", CatalogReadError::Unavailable),
            (
                401,
                "{}",
                CatalogReadError::Diagnostic(
                    crate::application::errors::ErrorKind::Unauthenticated,
                ),
            ),
            (
                403,
                "{}",
                CatalogReadError::Diagnostic(crate::application::errors::ErrorKind::Forbidden),
            ),
            (
                504,
                "",
                CatalogReadError::Diagnostic(crate::application::errors::ErrorKind::Timeout),
            ),
        ];
        for (status, body, expected) in cases {
            assert_eq!(
                decode_work(status, JSON, body.as_bytes(), work()),
                Err(expected),
                "{status} {body}"
            );
        }
        assert_eq!(
            decode_work(404, Some("text/html"), NOT_FOUND.as_bytes(), work()),
            Err(CatalogReadError::Unavailable)
        );
    }

    #[test]
    fn shared_unit_fixtures_decode_including_empty_continuation_and_version_change() {
        let first = units(200, UNITS, false).unwrap();
        assert_eq!(first.items[0].title, "Một tiếng chuông trong sương");
        assert_eq!(first.next_cursor, None);
        let continuation = units(200, CONTINUATION, true).unwrap();
        assert!(continuation.items.is_empty() && continuation.next_cursor.is_some());
        let end = units(200, END_EMPTY, true).unwrap();
        assert!(end.items.is_empty() && end.next_cursor.is_none());
        let republished = units(200, REPUBLISHED, false).unwrap();
        assert_eq!(republished.items[0].unit_id, first.items[0].unit_id);
        assert_ne!(republished.items[0].playback, first.items[0].playback);
    }

    #[test]
    fn invalid_cursor_is_only_meaningful_for_a_cursor_request() {
        assert_eq!(
            units(400, INVALID_CURSOR, true),
            Err(CatalogReadError::InvalidCursor)
        );
        assert_eq!(
            units(400, INVALID_CURSOR, false),
            Err(CatalogReadError::Protocol)
        );
        assert_eq!(units(404, NOT_FOUND, true), Err(CatalogReadError::NotFound));
        assert_eq!(
            units(503, UNAVAILABLE, true),
            Err(CatalogReadError::Unavailable)
        );
    }

    #[test]
    fn wrong_kind_identity_duplicate_or_oversized_unit_pages_are_rejected() {
        assert_eq!(
            decode_units(
                200,
                JSON,
                UNITS.as_bytes(),
                work(),
                UnitKind::Episode,
                false
            ),
            Err(CatalogReadError::Protocol),
            "a story's units are chapters"
        );
        let valid: serde_json::Value = serde_json::from_str(UNITS).unwrap();
        let mut no_audio = valid.clone();
        no_audio["items"][0]["playback"] = serde_json::Value::Null;
        assert!(
            units(200, &serde_json::to_string(&no_audio).unwrap(), false)
                .unwrap()
                .items[0]
                .playback
                .is_none()
        );
        let mut missing = valid.clone();
        missing["items"][0]
            .as_object_mut()
            .unwrap()
            .remove("playback");
        assert_eq!(
            units(200, &serde_json::to_string(&missing).unwrap(), false),
            Err(CatalogReadError::Protocol)
        );
        let mutate = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut value = valid.clone();
            edit(&mut value);
            serde_json::to_string(&value).unwrap()
        };
        for bad in [
            mutate(&|v| v["work_id"] = serde_json::json!("10000000-0000-4000-8000-000000000002")),
            mutate(&|v| v["items"][0]["unit_id"] = serde_json::json!("unit-1")),
            mutate(&|v| v["items"][0]["playback"]["publication_id"] = serde_json::json!("")),
            mutate(&|v| v["items"][0]["title"] = serde_json::json!("  ")),
            mutate(&|v| v["items"][0]["availability"] = serde_json::json!("withdrawn")),
            mutate(&|v| {
                let item = v["items"][0].clone();
                v["items"].as_array_mut().unwrap().push(item);
            }),
        ] {
            assert_eq!(
                units(200, &bad, false),
                Err(CatalogReadError::Protocol),
                "{bad}"
            );
        }
        let oversized = " ".repeat(MAX_PAGE_BYTES + 1);
        assert_eq!(
            units(200, &oversized, false),
            Err(CatalogReadError::Protocol)
        );
    }
}
