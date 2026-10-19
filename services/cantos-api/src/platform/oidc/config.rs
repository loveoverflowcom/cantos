use super::{core::AuthError, trust::IssuerTrust};
const STUDIO_CLIENT_ID: &str = "cantos-studio-web-bff";
pub const THEATRE_WEB_CLIENT_ID: &str = "cantos-theatre-web-bff";
pub const THEATRE_NATIVE_CLIENT_ID: &str = "cantos-theatre-native";
pub const THEATRE_NATIVE_REDIRECT: &str = "com.cantos.theatre:/oauth2/callback";
/// Exact Android debug callback. It is admitted only by explicit operator opt-in.
pub const THEATRE_NATIVE_DEVELOPMENT_REDIRECT: &str = "com.cantos.theatre.dev:/oauth2/callback";
use reqwest::Url;

#[derive(Clone)]
pub(crate) enum ClientAuthentication {
    Confidential(String),
    PublicNative,
}

/// Operator-owned exact configuration. No environment, headers or discovery can choose a client.
#[derive(Clone)]
pub struct OidcConfig {
    pub(crate) client_id: &'static str,
    pub(crate) issuer: String,
    pub(crate) origin: Url,
    pub(crate) web_origin: String,
    pub(crate) redirect: String,
    /// Operator-admitted extra native callback; never derived from a request.
    pub(crate) development_redirect: Option<&'static str>,
    pub(crate) authentication: ClientAuthentication,
    pub(crate) secure: bool,
    pub(crate) trust: Option<IssuerTrust>,
}

impl OidcConfig {
    pub(crate) fn theatre_native(issuer: &str) -> Result<Self, AuthError> {
        let provider = checked_url(issuer)?;
        if provider.path() != format!("/oauth2/openid/{THEATRE_NATIVE_CLIENT_ID}") {
            return Err(AuthError::Configuration);
        }
        let origin = provider.join("/").map_err(|_| AuthError::Configuration)?;
        Ok(Self {
            client_id: THEATRE_NATIVE_CLIENT_ID,
            issuer: issuer.into(),
            origin,
            web_origin: String::new(),
            redirect: THEATRE_NATIVE_REDIRECT.into(),
            development_redirect: None,
            authentication: ClientAuthentication::PublicNative,
            secure: true,
            trust: None,
        })
    }

    /// Admits the exact development callback for this native client. No other URI is possible.
    pub(crate) fn with_development_callback(mut self) -> Self {
        self.development_redirect = Some(THEATRE_NATIVE_DEVELOPMENT_REDIRECT);
        self
    }
    /// Resolves a closed profile to its exact operator-admitted callback.
    /// An unadmitted profile is a configuration refusal; it never falls back to production.
    pub(crate) fn native_redirect(
        &self,
        profile: Option<cantos_contracts::bearer::CallbackProfile>,
    ) -> Result<&str, AuthError> {
        use cantos_contracts::bearer::CallbackProfile;
        match profile.unwrap_or(CallbackProfile::Production) {
            CallbackProfile::Production => Ok(&self.redirect),
            CallbackProfile::Development => {
                self.development_redirect.ok_or(AuthError::Configuration)
            }
        }
    }
    /// Every callback currently admitted for flow consumption, production first.
    pub(crate) fn admitted_native_redirects(&self) -> Vec<&str> {
        std::iter::once(self.redirect.as_str())
            .chain(self.development_redirect)
            .collect()
    }
    pub(crate) fn is_public_native(&self) -> bool {
        matches!(self.authentication, ClientAuthentication::PublicNative)
    }
    /// HTTPS with normal certificate validation; local HTTP is limited to literal loopback.
    /// The host mode (development or production) is decided separately in `platform::host`.
    pub fn new(issuer: &str, web_origin: &str, secret: String) -> Result<Self, AuthError> {
        Self::for_web(issuer, web_origin, secret, STUDIO_CLIENT_ID, "studio")
    }
    pub(crate) fn theatre_web(
        issuer: &str,
        web_origin: &str,
        secret: String,
    ) -> Result<Self, AuthError> {
        Self::for_web(issuer, web_origin, secret, THEATRE_WEB_CLIENT_ID, "theatre")
    }
    fn for_web(
        issuer: &str,
        web_origin: &str,
        secret: String,
        client_id: &'static str,
        surface: &str,
    ) -> Result<Self, AuthError> {
        let provider = checked_url(issuer)?;
        let web = checked_url(web_origin)?;
        if provider.path() != format!("/oauth2/openid/{client_id}")
            || !matches!(web.path(), "" | "/")
            || web_origin.ends_with('/')
            || secret.is_empty()
            || secret.len() > 4096
            || secret.chars().any(char::is_control)
        {
            return Err(AuthError::Configuration);
        }
        let origin = provider.join("/").map_err(|_| AuthError::Configuration)?;
        Ok(Self {
            client_id,
            issuer: issuer.into(),
            origin,
            web_origin: web_origin.into(),
            redirect: format!("{web_origin}/api/v1/{surface}/auth/callback"),
            development_redirect: None,
            authentication: ClientAuthentication::Confidential(secret),
            secure: web.scheme() == "https",
            trust: None,
        })
    }

    /// A browser served by static Hosting returns through the API BFF, then to the fixed Web
    /// origin. This never changes the issuer, client, state/nonce or cookie policy.
    pub fn with_api_callback_origin(mut self, api_origin: &str) -> Result<Self, AuthError> {
        let api = checked_url(api_origin)?;
        if api.origin().ascii_serialization() != api_origin
            || api.scheme() != if self.secure { "https" } else { "http" }
        {
            return Err(AuthError::Configuration);
        }
        let surface = match self.client_id {
            STUDIO_CLIENT_ID => "studio",
            THEATRE_WEB_CLIENT_ID => "theatre",
            _ => return Err(AuthError::Configuration),
        };
        self.redirect = format!("{api_origin}/api/v1/{surface}/auth/callback");
        Ok(self)
    }

    /// Private anchors only make sense for an HTTPS issuer; unused trust is a configuration error.
    pub fn with_trust(self, trust: IssuerTrust) -> Result<Self, AuthError> {
        if self.origin.scheme() != "https" {
            return Err(AuthError::Configuration);
        }
        Ok(Self {
            trust: Some(trust),
            ..self
        })
    }

    pub(crate) fn trust(&self) -> Option<&IssuerTrust> {
        self.trust.as_ref()
    }

    pub(crate) fn endpoint(&self, value: &str) -> Result<Url, AuthError> {
        let url = checked_url(value).map_err(|_| AuthError::InvalidMetadata)?;
        if url.origin() != self.origin.origin() {
            return Err(AuthError::InvalidMetadata);
        }
        Ok(url)
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub(crate) fn web_return_origin(&self) -> &str {
        if self
            .redirect
            .starts_with(&format!("{}/api/v1/", self.web_origin))
        {
            ""
        } else {
            &self.web_origin
        }
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect
    }
}

fn checked_url(value: &str) -> Result<Url, AuthError> {
    let url = Url::parse(value).map_err(|_| AuthError::Configuration)?;
    let local = matches!(url.host_str(), Some("127.0.0.1" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && local))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
    {
        return Err(AuthError::Configuration);
    }
    // A URL parser must not silently change the exact issuer spelling (host/path/dot segments).
    let normalized = url.as_str().trim_end_matches('/');
    if normalized != value.trim_end_matches('/') {
        return Err(AuthError::Configuration);
    }
    Ok(url)
}

#[cfg(test)]
mod api_origin_tests {
    use super::*;
    #[test]
    fn api_callback_is_pinned_separately_from_the_web_return_origin() {
        let config = OidcConfig::new(
            "https://auth.example.org/oauth2/openid/cantos-studio-web-bff",
            "https://studio.example.org",
            "secret".into(),
        )
        .unwrap()
        .with_api_callback_origin("https://api.example.org")
        .unwrap();
        assert_eq!(
            config.redirect_uri(),
            "https://api.example.org/api/v1/studio/auth/callback"
        );
        assert_eq!(config.web_origin, "https://studio.example.org");
        assert!(config.secure);
        assert_eq!(config.web_return_origin(), "https://studio.example.org");
        let theatre = OidcConfig::theatre_web(
            "https://auth.example.org/oauth2/openid/cantos-theatre-web-bff",
            "https://listen.example.org",
            "secret".into(),
        )
        .unwrap()
        .with_api_callback_origin("https://api.example.org")
        .unwrap();
        assert_eq!(
            theatre.redirect_uri(),
            "https://api.example.org/api/v1/theatre/auth/callback"
        );
    }
    #[test]
    fn native_callbacks_are_a_closed_operator_admitted_set_without_fallback() {
        use cantos_contracts::bearer::CallbackProfile::{Development, Production};
        let issuer = "https://auth.example.org/oauth2/openid/cantos-theatre-native";
        let default = OidcConfig::theatre_native(issuer).unwrap();
        assert_eq!(
            default.native_redirect(None),
            Ok("com.cantos.theatre:/oauth2/callback")
        );
        assert_eq!(
            default.native_redirect(Some(Production)),
            Ok("com.cantos.theatre:/oauth2/callback")
        );
        assert!(matches!(
            default.native_redirect(Some(Development)),
            Err(AuthError::Configuration)
        ));
        assert_eq!(
            default.admitted_native_redirects(),
            ["com.cantos.theatre:/oauth2/callback"]
        );
        let admitted = default.with_development_callback();
        assert_eq!(
            admitted.native_redirect(Some(Development)),
            Ok("com.cantos.theatre.dev:/oauth2/callback")
        );
        assert_eq!(
            admitted.native_redirect(None),
            Ok("com.cantos.theatre:/oauth2/callback")
        );
        assert_eq!(
            admitted.admitted_native_redirects(),
            [
                "com.cantos.theatre:/oauth2/callback",
                "com.cantos.theatre.dev:/oauth2/callback"
            ]
        );
    }
    #[test]
    fn callback_profile_wire_names_are_closed_and_reject_uris() {
        use cantos_contracts::bearer::{CallbackProfile, FlowRequest};
        let base =
            |extra: &str| format!(r#"{{"state":"s","nonce":"n","code_challenge":"c"{extra}}}"#);
        let parse = |extra: &str| serde_json::from_str::<FlowRequest>(&base(extra));
        assert_eq!(parse("").unwrap().callback_profile, None);
        assert_eq!(
            parse(r#","callback_profile":"development""#)
                .unwrap()
                .callback_profile,
            Some(CallbackProfile::Development)
        );
        for bad in [
            r#","callback_profile":"com.cantos.theatre.dev:/oauth2/callback""#,
            r#","callback_profile":"com.cantos.evil""#,
            r#","callback_profile":"Development""#,
            r#","callback_profile":null,"redirect_uri":"x""#,
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn callback_config_refuses_origin_normalization_downgrade_and_native_reuse() {
        for origin in [
            "https://api.example.org/",
            "https://API.example.org",
            "https://api.example.org?next=evil",
            "https://api.example.org/evil",
            "http://127.0.0.1:8080",
        ] {
            let config = OidcConfig::new(
                "https://auth.example.org/oauth2/openid/cantos-studio-web-bff",
                "https://studio.example.org",
                "secret".into(),
            )
            .unwrap();
            assert!(
                matches!(
                    config.with_api_callback_origin(origin),
                    Err(AuthError::Configuration)
                ),
                "{origin}"
            );
        }
        let native = OidcConfig::theatre_native(
            "https://auth.example.org/oauth2/openid/cantos-theatre-native",
        )
        .unwrap();
        assert!(matches!(
            native.with_api_callback_origin("https://api.example.org"),
            Err(AuthError::Configuration)
        ));
    }
}
