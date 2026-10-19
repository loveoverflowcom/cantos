//! Anonymous reads of one release: its manifest and a temporary playback authorization. No
//! identity cookie, account token or Studio credential is sent, and the media bytes themselves
//! are never fetched here: the audio element streams the returned URL directly, as given.
//! Decoding is pure and strict over the shared DTOs.
use super::public_work::{classify_error, is_json};
use crate::application::{media_driver::AuthorizationError, public_work::CatalogReadError};
use cantos_contracts::{
    routes::THEATRE_BASE,
    theatre::{PlaybackAuthorization, PublicationManifest},
};
use uuid::Uuid;

/// Both answers are small; a body beyond this is not the contract's.
const MAX_BODY_BYTES: usize = 65_536;

pub fn manifest_path(publication: Uuid) -> String {
    format!("{THEATRE_BASE}/publications/{}", publication.hyphenated())
}

pub fn authorization_path(publication: Uuid) -> String {
    format!(
        "{THEATRE_BASE}/publications/{}/playback-authorizations",
        publication.hyphenated()
    )
}

/// A success body is bounded JSON; anything else breaks the contract.
fn json_body<'a>(content_type: Option<&str>, body: &'a [u8]) -> Result<&'a [u8], CatalogReadError> {
    if body.len() > MAX_BODY_BYTES || !is_json(content_type) {
        return Err(CatalogReadError::Protocol);
    }
    Ok(body)
}

/// Other statuses use the shared classification; a cursor error cannot occur on these routes.
fn failure(status: u16, content_type: Option<&str>, body: &[u8]) -> CatalogReadError {
    match classify_error(status, content_type, body) {
        CatalogReadError::InvalidCursor => CatalogReadError::Protocol,
        error => error,
    }
}

pub fn decode_manifest(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<PublicationManifest, CatalogReadError> {
    if status != 200 {
        return Err(failure(status, content_type, body));
    }
    serde_json::from_slice(json_body(content_type, body)?).map_err(|_| CatalogReadError::Protocol)
}

pub fn decode_authorization(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<PlaybackAuthorization, AuthorizationError> {
    let error = if status == 200 {
        match json_body(content_type, body)
            .and_then(|json| serde_json::from_slice(json).map_err(|_| CatalogReadError::Protocol))
        {
            Ok(authorization) => return Ok(authorization),
            Err(error) => error,
        }
    } else {
        failure(status, content_type, body)
    };
    Err(match error {
        CatalogReadError::NotFound => AuthorizationError::NotFound,
        CatalogReadError::Unavailable => AuthorizationError::Unavailable,
        CatalogReadError::Protocol | CatalogReadError::InvalidCursor => {
            AuthorizationError::Protocol
        }
    })
}

#[cfg(target_arch = "wasm32")]
async fn send(
    post: bool,
    path: &str,
    controller: web_sys::AbortController,
) -> Result<(u16, Option<String>, Vec<u8>), CatalogReadError> {
    use gloo_net::http::Request;
    use web_sys::RequestCredentials;

    // Held until the body is read; dropping it clears the timer and aborts the request.
    let deadline =
        super::browser::deadline(controller).map_err(|_| CatalogReadError::Unavailable)?;
    let signal = deadline.controller.signal();
    let url = crate::api_endpoint::endpoint(path).map_err(|_| CatalogReadError::Unavailable)?;
    let request = if post {
        Request::post(&url)
    } else {
        Request::get(&url)
    };
    let response = request
        .credentials(RequestCredentials::Omit)
        .header("Accept", "application/json")
        .abort_signal(Some(&signal))
        .send()
        .await
        .map_err(|_| CatalogReadError::Unavailable)?;
    let declared = response
        .headers()
        .get("content-length")
        .and_then(|value| value.trim().parse::<usize>().ok());
    if declared.is_some_and(|length| length > MAX_BODY_BYTES) {
        return Err(CatalogReadError::Protocol);
    }
    let content_type = response.headers().get("content-type");
    let body = response
        .binary()
        .await
        .map_err(|_| CatalogReadError::Unavailable)?;
    Ok((response.status(), content_type, body))
}

#[cfg(target_arch = "wasm32")]
pub async fn load_manifest(
    publication: Uuid,
    controller: web_sys::AbortController,
) -> Result<PublicationManifest, CatalogReadError> {
    let (status, content_type, body) = send(false, &manifest_path(publication), controller).await?;
    decode_manifest(status, content_type.as_deref(), &body)
}

/// The body is empty: the contract accepts no body or `{}`, and an empty POST needs no
/// preflight when the API is another origin.
#[cfg(target_arch = "wasm32")]
pub async fn authorize(
    publication: Uuid,
    controller: web_sys::AbortController,
) -> Result<PlaybackAuthorization, AuthorizationError> {
    match send(true, &authorization_path(publication), controller).await {
        Ok((status, content_type, body)) => {
            decode_authorization(status, content_type.as_deref(), &body)
        }
        Err(CatalogReadError::Unavailable) => Err(AuthorizationError::Unavailable),
        Err(_) => Err(AuthorizationError::Protocol),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/manifest.json"
    );
    const NOT_FOUND: &str = include_str!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/not_found.json"
    );
    const UNAVAILABLE: &str = include_str!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/unavailable.json"
    );
    const RENDITION_UNAVAILABLE: &str = include_str!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/rendition_unavailable.json"
    );
    const INVALID_REQUEST: &str = include_str!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/invalid_request.json"
    );
    const JSON: Option<&str> = Some("application/json; charset=utf-8");
    const PUBLICATION: &str = "80000000-0000-4000-8000-000000000008";

    fn authorization() -> PlaybackAuthorization {
        PlaybackAuthorization {
            publication_id: PUBLICATION.into(),
            rendition_id: "60000000-0000-4000-8000-000000000006".into(),
            sha256: "b".repeat(64),
            url: "https://localhost:8443/api/v1/theatre/media/pg1_x".into(),
            expires_at_ms: 120_000,
            content_type: "audio/ogg".into(),
            byte_range_supported: true,
        }
    }

    #[test]
    fn paths_use_the_theatre_group_and_the_canonical_publication_id() {
        let id = Uuid::parse_str(PUBLICATION).unwrap();
        assert_eq!(
            manifest_path(id),
            "/api/v1/theatre/publications/80000000-0000-4000-8000-000000000008"
        );
        assert_eq!(
            authorization_path(id),
            "/api/v1/theatre/publications/80000000-0000-4000-8000-000000000008/playback-authorizations"
        );
    }

    #[test]
    fn the_shared_manifest_fixture_decodes_and_unknown_fields_are_refused() {
        let manifest = decode_manifest(200, JSON, MANIFEST.as_bytes()).unwrap();
        assert_eq!(manifest.publication_id, PUBLICATION);
        let mut extra: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
        extra["signed_url"] = serde_json::json!("https://example.invalid/a");
        for (kind, body) in [
            (None, MANIFEST.as_bytes().to_vec()),
            (Some("text/html"), MANIFEST.as_bytes().to_vec()),
            (JSON, serde_json::to_vec(&extra).unwrap()),
            (JSON, b"{}".to_vec()),
            (JSON, vec![b' '; MAX_BODY_BYTES + 1]),
        ] {
            assert_eq!(
                decode_manifest(200, kind, &body),
                Err(CatalogReadError::Protocol)
            );
        }
    }

    #[test]
    fn an_authorization_round_trips_its_contract_fields_exactly() {
        let body = serde_json::to_vec(&authorization()).unwrap();
        assert_eq!(decode_authorization(200, JSON, &body), Ok(authorization()));
        let mut unknown: serde_json::Value = serde_json::from_slice(&body).unwrap();
        unknown["account_token"] = serde_json::json!("secret");
        assert_eq!(
            decode_authorization(200, JSON, &serde_json::to_vec(&unknown).unwrap()),
            Err(AuthorizationError::Protocol)
        );
        assert_eq!(
            decode_authorization(200, Some("text/plain"), &body),
            Err(AuthorizationError::Protocol)
        );
    }

    #[test]
    fn contract_errors_classify_exactly_for_both_routes() {
        let cases = [
            (404, NOT_FOUND, AuthorizationError::NotFound),
            (503, UNAVAILABLE, AuthorizationError::Unavailable),
            (503, RENDITION_UNAVAILABLE, AuthorizationError::Unavailable),
            (
                500,
                r#"{"code":"unavailable","retryable":false}"#,
                AuthorizationError::Protocol,
            ),
            (400, INVALID_REQUEST, AuthorizationError::Protocol),
            (503, NOT_FOUND, AuthorizationError::Protocol),
            // An uncoded answer (proxy page, outage, wrong host) is never "withdrawn".
            (
                404,
                "<html>Not Found</html>",
                AuthorizationError::Unavailable,
            ),
            (502, "", AuthorizationError::Unavailable),
            (401, "{}", AuthorizationError::Unavailable),
        ];
        for (status, body, expected) in cases {
            assert_eq!(
                decode_authorization(status, JSON, body.as_bytes()),
                Err(expected),
                "{status} {body}"
            );
        }
        assert_eq!(
            decode_manifest(404, JSON, NOT_FOUND.as_bytes()),
            Err(CatalogReadError::NotFound)
        );
        assert_eq!(
            decode_manifest(404, Some("text/html"), NOT_FOUND.as_bytes()),
            Err(CatalogReadError::Unavailable)
        );
    }
}
