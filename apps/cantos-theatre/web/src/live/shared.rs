//! Authenticated shared reads reuse strict Discovery shapes; they never retry public routes.
use crate::contract::discovery::PlaybackAuthorization;

pub fn path(public: &str) -> Option<String> {
    public
        .strip_prefix("/api/v1/theatre/")
        .map(|suffix| format!("/api/v1/theatre/shared/{suffix}"))
}

/// A credentialed audio URL is accepted only on the configured API origin and shared media path.
/// Public URLs, redirects, query capabilities and off-origin hosts never receive Theatre cookies.
pub fn media_is_bound(authorization: &PlaybackAuthorization, api: &str) -> bool {
    let (Ok(actual), Ok(expected)) = (url::Url::parse(&authorization.url), url::Url::parse(api))
    else {
        return false;
    };
    actual.scheme() == "https"
        && actual.origin() == expected.origin()
        && actual.username().is_empty()
        && actual.password().is_none()
        && actual.fragment().is_none()
        && actual.query().is_none()
        && actual
            .path()
            .strip_prefix("/api/v1/theatre/shared/media/")
            .is_some_and(|token| {
                !token.is_empty()
                    && token
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            })
}

pub fn auth_error(
    error: cantos_web_auth::Failure,
) -> crate::application::public_work::CatalogReadError {
    use crate::application::{errors::ErrorKind, public_work::CatalogReadError as Error};
    use cantos_web_auth::Failure;
    match error {
        Failure::SignedOut | Failure::Changed | Failure::AccountMismatch => {
            Error::Diagnostic(ErrorKind::Unauthenticated)
        }
        Failure::Forbidden => Error::Diagnostic(ErrorKind::Forbidden),
        Failure::Unavailable | Failure::CleanupPending | Failure::RateLimited(_) => {
            Error::Unavailable
        }
        Failure::NotFound | Failure::Invalid | Failure::InvalidCredentials | Failure::Protocol => {
            Error::Protocol
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub mod browser {
    use super::*;
    use crate::application::shared_access::ClaimReceipt;
    use crate::application::{account::RequestFence, public_work::CatalogReadError};
    use crate::application::{
        public_discovery::{DiscoveryReadError, DiscoveryTicket},
        public_work::{PageTicket, UNIT_PAGE_LIMIT},
    };
    use crate::contract::discovery::{
        MetadataUnitPage, MetadataWorkPage, MetadataWorkSummary, PublicationManifest,
    };
    use gloo_net::http::Request;
    use web_sys::AbortController;

    #[derive(Clone, Default)]
    pub struct Requests(std::rc::Rc<std::cell::RefCell<Vec<AbortController>>>);
    impl Requests {
        pub fn start(&self) -> Result<AbortController, CatalogReadError> {
            let controller = AbortController::new().map_err(|_| CatalogReadError::Unavailable)?;
            self.0.borrow_mut().retain(|old| !old.signal().aborted());
            self.0.borrow_mut().push(controller.clone());
            Ok(controller)
        }
        pub fn abort(&self) {
            for controller in self.0.borrow_mut().drain(..) {
                controller.abort();
            }
        }
    }
    async fn send(
        path: &str,
        fence: &RequestFence,
        body: Option<String>,
        controller: AbortController,
    ) -> Result<(u16, Option<String>, Vec<u8>), CatalogReadError> {
        let expected = fence.session();
        let credential = crate::api_endpoint::auth()
            .map_err(|_| CatalogReadError::Unavailable)?
            .access(Some(expected))
            .await
            .map_err(auth_error)?;
        let url = crate::api_endpoint::endpoint(path).map_err(|_| CatalogReadError::Protocol)?;
        let deadline = crate::live::browser::deadline(controller)
            .map_err(|_| CatalogReadError::Unavailable)?;
        let request = credential
            .apply(if body.is_some() {
                Request::post(&url)
            } else {
                Request::get(&url)
            })
            .header("Accept", "application/json")
            .abort_signal(Some(&deadline.controller.signal()));
        let request = match body {
            Some(body) => request
                .header("Content-Type", "application/json")
                .body(body),
            None => request.build(),
        }
        .map_err(|_| CatalogReadError::Protocol)?;
        let response = request
            .send()
            .await
            .map_err(|_| CatalogReadError::Unavailable)?;
        credential.receive(&response).map_err(auth_error)?;
        let status = response.status();
        if status == 200
            && response
                .headers()
                .get("x-cantos-theatre-session")
                .as_deref()
                != Some(expected)
        {
            return Err(CatalogReadError::Protocol);
        }
        if response
            .headers()
            .get("content-length")
            .and_then(|s| s.parse::<usize>().ok())
            .is_some_and(|n| n > 2_097_152)
        {
            return Err(CatalogReadError::Protocol);
        }
        let content_type = response.headers().get("content-type");
        let bytes = response
            .binary()
            .await
            .map_err(|_| CatalogReadError::Unavailable)?;
        credential.check().map_err(auth_error)?;
        if bytes.len() > 2_097_152 {
            return Err(CatalogReadError::Protocol);
        }
        Ok((status, content_type, bytes))
    }
    pub async fn work(
        id: crate::core::public_ids::PublicWorkId,
        fence: &RequestFence,
        controller: AbortController,
    ) -> Result<MetadataWorkSummary, CatalogReadError> {
        let path = format!("/api/v1/theatre/shared/works/{id}");
        let (status, content, bytes) = send(&path, fence, None, controller).await?;
        let work = crate::live::public_work::decode_work(status, content.as_deref(), &bytes, id)?;
        if work.cover.is_some() {
            return Err(CatalogReadError::Protocol);
        }
        Ok(work)
    }
    pub async fn units(
        ticket: &PageTicket,
        fence: &RequestFence,
        controller: AbortController,
    ) -> Result<MetadataUnitPage, CatalogReadError> {
        let mut path = format!(
            "/api/v1/theatre/shared/works/{}/units?limit={UNIT_PAGE_LIMIT}",
            ticket.work()
        );
        if let Some(cursor) = ticket.cursor() {
            path.push_str("&cursor=");
            path.extend(url::form_urlencoded::byte_serialize(cursor.as_bytes()));
        }
        let (status, content, bytes) = send(&path, fence, None, controller).await?;
        crate::live::public_work::decode_units(
            status,
            content.as_deref(),
            &bytes,
            ticket.work(),
            ticket.kind(),
            ticket.cursor().is_some(),
        )
    }
    pub async fn catalog(
        ticket: &DiscoveryTicket,
        fence: &RequestFence,
        controller: AbortController,
    ) -> Result<MetadataWorkPage, DiscoveryReadError> {
        let Some(path) = super::path(&crate::live::discovery::works_path(
            ticket.query(),
            ticket.cursor(),
        )) else {
            return Err(DiscoveryReadError::Protocol);
        };
        let (status, content, bytes) =
            send(&path, fence, None, controller)
                .await
                .map_err(|e| match e {
                    CatalogReadError::Diagnostic(kind) => DiscoveryReadError::Diagnostic(kind),
                    _ => DiscoveryReadError::Unavailable,
                })?;
        let page = crate::live::discovery::decode_response(
            status,
            content.as_deref(),
            &bytes,
            ticket.cursor().is_some(),
        )?;
        if page.items.iter().any(|w| w.cover.is_some()) {
            return Err(DiscoveryReadError::Protocol);
        }
        Ok(page)
    }
    pub async fn manifest(
        publication: uuid::Uuid,
        fence: &RequestFence,
        controller: AbortController,
    ) -> Result<PublicationManifest, CatalogReadError> {
        let path = format!("/api/v1/theatre/shared/publications/{publication}");
        let (status, content, bytes) = send(&path, fence, None, controller).await?;
        crate::live::public_media::decode_manifest(status, content.as_deref(), &bytes)
    }
    pub async fn authorize(
        publication: uuid::Uuid,
        fence: &RequestFence,
        controller: AbortController,
    ) -> Result<PlaybackAuthorization, crate::application::media_driver::AuthorizationError> {
        use crate::application::media_driver::AuthorizationError;
        let path =
            format!("/api/v1/theatre/shared/publications/{publication}/playback-authorizations");
        let (status, content, bytes) = send(&path, fence, Some("{}".into()), controller)
            .await
            .map_err(|e| match e {
                CatalogReadError::Diagnostic(kind) => AuthorizationError::Diagnostic(kind),
                CatalogReadError::Protocol => AuthorizationError::Protocol,
                _ => AuthorizationError::Unavailable,
            })?;
        let authorization =
            crate::live::public_media::decode_authorization(status, content.as_deref(), &bytes)?;
        let api = crate::api_endpoint::endpoint("/api/v1/theatre")
            .map_err(|_| AuthorizationError::Protocol)?;
        if !media_is_bound(&authorization, &api) {
            return Err(AuthorizationError::Protocol);
        }
        Ok(authorization)
    }
    pub async fn claim(
        operation: &str,
        fence: &RequestFence,
        controller: AbortController,
    ) -> Result<ClaimReceipt, bool> {
        let body = serde_json::json!({"operation_id":operation}).to_string();
        let (status, content, bytes) = send(
            "/api/v1/theatre/shared/claim",
            fence,
            Some(body),
            controller,
        )
        .await
        .map_err(|_| true)?;
        if status != 200 {
            return Err(status >= 500);
        }
        if !crate::live::public_work::is_json(content.as_deref()) || bytes.len() > 65536 {
            return Err(true);
        }
        serde_json::from_slice(&bytes).map_err(|_| true)
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::Requests;
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct Requests {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_audio_requires_exact_origin_and_path_without_url_credential_fallback() {
        let mut auth = PlaybackAuthorization {
            publication_id: uuid::Uuid::from_u128(1).to_string(),
            rendition_id: uuid::Uuid::from_u128(2).to_string(),
            sha256: "a".repeat(64),
            url: String::new(),
            expires_at_ms: 1000,
            content_type: "audio/ogg".into(),
            byte_range_supported: true,
        };
        auth.url = "https://api.example.org/api/v1/theatre/shared/media/pg1_bound".into();
        assert!(media_is_bound(
            &auth,
            "https://api.example.org/api/v1/theatre"
        ));
        for url in [
            "https://api.example.org.evil.test/api/v1/theatre/shared/media/x",
            "https://api.example.org/api/v1/theatre/media/x",
            "https://api.example.org/api/v1/theatre/shared/media/x?token=x",
            "https://other.example.org/api/v1/theatre/shared/media/x",
            "https://api.example.org/api/v1/theatre/shared/media/x/y",
        ] {
            auth.url = url.into();
            assert!(!media_is_bound(&auth, "https://api.example.org"));
        }
    }
    #[test]
    fn claim_decodes_the_actual_receipt_and_refuses_a_client_supplied_actor_or_email() {
        let fixture = include_str!("../../../../../services/cantos-server/contracts/theatre/viewer-access/v1/fixtures/claimed.json");
        let receipt: crate::application::shared_access::ClaimReceipt =
            serde_json::from_str(fixture).unwrap();
        assert_eq!(receipt.grants.len(), 1);
        let mut extra: serde_json::Value = serde_json::from_str(fixture).unwrap();
        extra["email"] = "private@example.org".into();
        assert!(
            serde_json::from_value::<crate::application::shared_access::ClaimReceipt>(extra)
                .is_err()
        );
    }
}
