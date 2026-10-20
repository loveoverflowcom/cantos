//! Trusted registration-to-policy/admission mapping, outside the lifetime and credential cores.
use super::core::SessionPolicy;
use crate::platform::oidc::config::{
    STUDIO_CLIENT_ID, THEATRE_NATIVE_CLIENT_ID, THEATRE_WEB_CLIENT_ID,
};

// Checked at compile time; invalid server constants cannot fall back to another policy.
const STUDIO_POLICY: SessionPolicy = match SessionPolicy::new(120, 300, 600) {
    Ok(policy) => policy,
    Err(_) => panic!("invalid Studio session policy"),
};
const THEATRE_POLICY: SessionPolicy = match SessionPolicy::new(120, 600, 900) {
    Ok(policy) => policy,
    Err(_) => panic!("invalid Theatre session policy"),
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Client {
    Studio,
    TheatreWeb,
    TheatreNative,
}

impl Client {
    pub fn name(self) -> &'static str {
        match self {
            Self::Studio => STUDIO_CLIENT_ID,
            Self::TheatreWeb => THEATRE_WEB_CLIENT_ID,
            Self::TheatreNative => THEATRE_NATIVE_CLIENT_ID,
        }
    }
    pub fn surface(self) -> &'static str {
        match self {
            Self::Studio => "studio",
            Self::TheatreWeb | Self::TheatreNative => "theatre",
        }
    }
    pub(crate) fn policy(self) -> SessionPolicy {
        match self {
            Self::Studio => STUDIO_POLICY,
            Self::TheatreWeb | Self::TheatreNative => THEATRE_POLICY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::bearer::core::{issue, Lifetime};

    #[test]
    fn trusted_registrations_keep_exact_names_surfaces_and_lifetimes() {
        for (client, name, surface, idle, absolute) in [
            (Client::Studio, "cantos-studio-web-bff", "studio", 300, 600),
            (
                Client::TheatreWeb,
                "cantos-theatre-web-bff",
                "theatre",
                600,
                900,
            ),
            (
                Client::TheatreNative,
                "cantos-theatre-native",
                "theatre",
                600,
                900,
            ),
        ] {
            assert_eq!(client.name(), name);
            assert_eq!(client.surface(), surface);
            let policy = client.policy();
            assert_eq!(policy.access_seconds(), 120);
            assert_eq!(policy.idle_seconds(), idle);
            assert_eq!(policy.absolute_seconds(), absolute);
        }
        for (client, expected) in [
            (
                Client::Studio,
                Lifetime {
                    access: 220,
                    idle: 400,
                    absolute: 700,
                },
            ),
            (
                Client::TheatreWeb,
                Lifetime {
                    access: 220,
                    idle: 700,
                    absolute: 1000,
                },
            ),
            (
                Client::TheatreNative,
                Lifetime {
                    access: 220,
                    idle: 700,
                    absolute: 1000,
                },
            ),
        ] {
            assert_eq!(issue(client.policy(), 100, 2000, 2000), Ok(expected));
        }
    }

    #[test]
    fn every_mismatched_registration_is_refused_before_provider_construction() {
        use crate::platform::{
            bearer::BearerIdentity,
            oidc::{config::OidcConfig, core::AuthError},
        };
        for client in [Client::Studio, Client::TheatreWeb, Client::TheatreNative] {
            for other in [Client::Studio, Client::TheatreWeb, Client::TheatreNative] {
                if client == other {
                    continue;
                }
                let config = match other {
                    Client::Studio => OidcConfig::new(
                        "https://id.example/oauth2/openid/cantos-studio-web-bff",
                        "https://studio.example",
                        "secret".into(),
                    ),
                    Client::TheatreWeb => OidcConfig::theatre_web(
                        "https://id.example/oauth2/openid/cantos-theatre-web-bff",
                        "https://theatre.example",
                        "secret".into(),
                    ),
                    Client::TheatreNative => OidcConfig::theatre_native(
                        "https://id.example/oauth2/openid/cantos-theatre-native",
                    ),
                }
                .unwrap();
                assert!(matches!(
                    BearerIdentity::new(config, client),
                    Err(AuthError::Configuration)
                ));
            }
        }
    }
}
