//! Trusted registration-to-policy mapping, outside the lifetime core.
use super::core::SessionPolicy;

// Stable registration names of the three session purposes (Identity v1 tags its audit and
// policy lookups with them); they name no external provider client.
const STUDIO_CLIENT_ID: &str = "cantos-studio-web-bff";
const THEATRE_WEB_CLIENT_ID: &str = "cantos-theatre-web-bff";
const THEATRE_NATIVE_CLIENT_ID: &str = "cantos-theatre-native";

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
}
