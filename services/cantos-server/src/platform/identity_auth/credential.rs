//! Opaque credential syntax for sessions issued from a verified Identity fact. A tag routes a
//! value to its verifier; it never establishes authority (the session row does).
//!
//! The `ci1_` family is distinct from the retired provider-bound `ca1_`/`cr1_` credentials, which
//! no verifier accepts any more.
use crate::platform::bearer::client::Client;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Theatre,
    Studio,
}

/// How a session is carried. Web always means an HttpOnly cookie, native an opaque Bearer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Web(Surface),
    Native,
}

impl Channel {
    pub fn surface(self) -> Surface {
        match self {
            Self::Web(surface) => surface,
            Self::Native => Surface::Theatre,
        }
    }

    /// The existing registration whose `SessionPolicy` this channel reuses; no TTL is invented.
    pub(crate) fn policy_client(self) -> Client {
        match self {
            Self::Web(Surface::Theatre) => Client::TheatreWeb,
            Self::Web(Surface::Studio) => Client::Studio,
            Self::Native => Client::TheatreNative,
        }
    }

    pub(crate) fn stored(self) -> &'static str {
        match self {
            Self::Web(Surface::Theatre) => "theatre-web",
            Self::Web(Surface::Studio) => "studio-web",
            Self::Native => "theatre-native",
        }
    }

    pub(crate) fn access_prefix(self) -> &'static str {
        match self {
            Self::Web(Surface::Theatre) => "ci1_t_",
            Self::Web(Surface::Studio) => "ci1_s_",
            Self::Native => "ci1_n_",
        }
    }
}

const REFRESH_PREFIX: &str = "ci1_r_";

pub fn secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}

/// Which channel an access value claims, by exact tag plus exact secret syntax.
pub fn access_channel(value: &str) -> Option<Channel> {
    [
        Channel::Web(Surface::Theatre),
        Channel::Web(Surface::Studio),
        Channel::Native,
    ]
    .into_iter()
    .find(|channel| {
        value
            .strip_prefix(channel.access_prefix())
            .is_some_and(secret)
    })
}

pub fn is_refresh(value: &str) -> bool {
    value.strip_prefix(REFRESH_PREFIX).is_some_and(secret)
}

/// Any value in this family, used to route before touching a legacy verifier.
pub fn is_identity_credential(value: &str) -> bool {
    access_channel(value).is_some() || is_refresh(value)
}

pub(crate) fn random_secret() -> Option<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|v| format!("{v:02x}")).collect())
}

/// Fresh access (and, for native, refresh) secrets. Hashing is the storage owner's job.
pub struct Secrets {
    pub access: String,
    pub refresh: Option<String>,
}

impl Secrets {
    pub fn new(channel: Channel) -> Option<Self> {
        let access = format!("{}{}", channel.access_prefix(), random_secret()?);
        let refresh = match channel {
            Channel::Native => Some(format!("{REFRESH_PREFIX}{}", random_secret()?)),
            Channel::Web(_) => None,
        };
        Some(Self { access, refresh })
    }
}

pub fn cookie_name(surface: Surface, secure: bool) -> &'static str {
    match (surface, secure) {
        (Surface::Theatre, true) => "__Secure-cantos_id_theatre",
        (Surface::Theatre, false) => "cantos_id_theatre",
        (Surface::Studio, true) => "__Secure-cantos_id_studio",
        (Surface::Studio, false) => "cantos_id_studio",
    }
}

fn cookie_path(surface: Surface) -> &'static str {
    match surface {
        Surface::Theatre => "/api/v1/theatre",
        Surface::Studio => "/api/v1/studio",
    }
}

/// Host-only (no Domain), HttpOnly, per-surface path, Strict. `Secure` follows the configured
/// Web origin scheme; `max_age` 0 with an empty value clears it.
pub fn set_cookie(surface: Surface, secure: bool, value: &str, max_age: u64) -> String {
    format!(
        "{}={value}; HttpOnly; SameSite=Strict; Path={}; Max-Age={max_age}{}",
        cookie_name(surface, secure),
        cookie_path(surface),
        if secure { "; Secure" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_exact_and_routing_never_crosses_the_legacy_family() {
        for (channel, prefix) in [
            (Channel::Web(Surface::Theatre), "ci1_t_"),
            (Channel::Web(Surface::Studio), "ci1_s_"),
            (Channel::Native, "ci1_n_"),
        ] {
            let value = format!("{prefix}{}", "a".repeat(64));
            assert_eq!(access_channel(&value), Some(channel));
            assert!(is_identity_credential(&value));
        }
        let refresh = format!("ci1_r_{}", "b".repeat(64));
        assert!(is_refresh(&refresh));
        assert!(access_channel(&refresh).is_none());
        // A provider-bound credential is never in this family and vice versa.
        for legacy in [
            format!("ca1_n_{}", "a".repeat(64)),
            format!("cr1_{}", "a".repeat(64)),
        ] {
            assert!(!is_identity_credential(&legacy));
        }
        for bad in [
            "".to_owned(),
            "ci1_n_".to_owned(),
            format!("ci1_n_{}", "A".repeat(64)),
            format!("ci1_n_{}", "a".repeat(63)),
            format!("ci1_n_{}", "a".repeat(65)),
            format!("ci1_x_{}", "a".repeat(64)),
            format!(" ci1_n_{}", "a".repeat(64)),
            format!("ci1_r_{}", "a".repeat(65)),
        ] {
            assert!(!is_identity_credential(&bad), "{bad}");
        }
    }

    #[test]
    fn generated_secrets_have_the_exact_shape_and_are_distinct() {
        let web = Secrets::new(Channel::Web(Surface::Studio)).unwrap();
        assert_eq!(
            access_channel(&web.access),
            Some(Channel::Web(Surface::Studio))
        );
        assert!(web.refresh.is_none());
        let native = Secrets::new(Channel::Native).unwrap();
        assert_eq!(access_channel(&native.access), Some(Channel::Native));
        assert!(is_refresh(native.refresh.as_deref().unwrap()));
        assert_ne!(web.access, native.access);
    }

    #[test]
    fn cookies_are_host_only_http_only_strict_and_path_scoped_per_surface() {
        let theatre = set_cookie(Surface::Theatre, true, "v", 600);
        assert_eq!(
            theatre,
            "__Secure-cantos_id_theatre=v; HttpOnly; SameSite=Strict; Path=/api/v1/theatre; Max-Age=600; Secure"
        );
        let studio = set_cookie(Surface::Studio, false, "", 0);
        assert_eq!(
            studio,
            "cantos_id_studio=; HttpOnly; SameSite=Strict; Path=/api/v1/studio; Max-Age=0"
        );
        assert!(!theatre.contains("Domain"));
        assert_ne!(
            cookie_name(Surface::Theatre, true),
            cookie_name(Surface::Studio, true)
        );
    }
}
