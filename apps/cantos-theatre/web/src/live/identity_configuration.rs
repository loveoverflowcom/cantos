//! Public same-origin capability read. It cannot create an account or authorize Studio.
#[cfg(any(target_arch = "wasm32", test))]
use crate::application::registration::ConfigurationFailure;
#[cfg(any(target_arch = "wasm32", test))]
use cantos_contracts::theatre_identity::Configuration;

#[cfg(any(target_arch = "wasm32", test))]
fn decode(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<Configuration, ConfigurationFailure> {
    if status != 200 {
        return Err(ConfigurationFailure::Unavailable);
    }
    let json = content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
    });
    if !json || body.len() > 8192 {
        return Err(ConfigurationFailure::Protocol);
    }
    serde_json::from_slice(body).map_err(|_| ConfigurationFailure::Protocol)
}

#[cfg(target_arch = "wasm32")]
pub async fn load() -> Result<Configuration, ConfigurationFailure> {
    use gloo_net::http::Request;
    use web_sys::{AbortController, RequestCredentials};

    let controller = AbortController::new().map_err(|_| ConfigurationFailure::Unavailable)?;
    let deadline =
        super::browser::deadline(controller).map_err(|_| ConfigurationFailure::Unavailable)?;
    let signal = deadline.controller.signal();
    let response = Request::get(
        &crate::api_endpoint::endpoint("/api/v1/theatre/auth/configuration")
            .map_err(|_| ConfigurationFailure::Unavailable)?,
    )
    .credentials(RequestCredentials::Omit)
    .redirect(web_sys::RequestRedirect::Error)
    .abort_signal(Some(&signal))
    .header("Accept", "application/json")
    .send()
    .await
    .map_err(|_| ConfigurationFailure::Unavailable)?;
    let status = response.status();
    let content_type = response.headers().get("content-type");
    let body = response
        .binary()
        .await
        .map_err(|_| ConfigurationFailure::Unavailable)?;
    decode(status, content_type.as_deref(), &body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_contracts::theatre_identity::{
        LoginMethod, Registration, RegistrationUnavailableReason,
    };

    #[test]
    fn capability_requires_a_bounded_strict_json_response_and_never_falls_back() {
        let body = br#"{"login":"oidc","registration":{"status":"unavailable","reason":"provider_unsupported"}}"#;
        assert_eq!(
            decode(200, Some("application/json; charset=utf-8"), body),
            Ok(Configuration {
                login: LoginMethod::Oidc,
                registration: Registration::Unavailable {
                    reason: RegistrationUnavailableReason::ProviderUnsupported
                },
            })
        );
        for status in [401, 403, 404, 500, 503] {
            assert_eq!(
                decode(status, Some("application/json"), body),
                Err(ConfigurationFailure::Unavailable)
            );
        }
        for content_type in [None, Some("text/html"), Some("application/jsonp")] {
            assert_eq!(
                decode(200, content_type, body),
                Err(ConfigurationFailure::Protocol)
            );
        }
        for body in [
            br#"{}"#.as_slice(),
            br#"{"login":"oidc","registration":{"status":"complete"}}"#,
            br#"{"login":"oidc","registration":{"status":"unavailable","reason":"unknown"}}"#,
            &[b' '; 8193],
        ] {
            assert_eq!(
                decode(200, Some("application/json"), body),
                Err(ConfigurationFailure::Protocol)
            );
        }
    }
}
