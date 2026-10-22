//! Explicit host composition. No discovery, account creation, SMTP connection or bootstrap.
use super::{
    application::RegistrationService,
    domain::{PeerPolicy, RegistrationError, RegistrationPolicy, TrustedProxies},
    kanidm::{KanidmConfig, KanidmIssuer},
    mail::SmtpMailer,
    mail_config::SmtpConfig,
    postgres::PostgresRegistrationRepository,
    runtime::{AttemptPolicy, RegistrationRuntime},
};
use crate::platform::store::Store;
use std::{collections::BTreeMap, sync::Arc};

const PREFIX: &str = "CANTOS_THEATRE_REGISTRATION_";
pub(super) const FIELDS: &[&str] = &[
    "ENABLED",
    "POLICY_APPROVED",
    "ISSUER_ACCESS_APPROVED",
    "PEER_POLICY",
    "TOKEN",
    "NETWORK_KEY",
    "VERIFICATION_SECONDS",
    "RESEND_SECONDS",
    "WINDOW_SECONDS",
    "MAX_GLOBAL_REQUESTS",
    "MAX_NETWORK_REQUESTS",
    "SETUP_SECONDS",
    "MAX_INFLIGHT",
    "ATTEMPT_WINDOW_SECONDS",
    "ATTEMPT_MAX_GLOBAL",
    "ATTEMPT_MAX_NETWORK",
];

/// Optional inputs shared by both providers: required with `PEER_POLICY=trusted_proxy`, refused
/// with `direct_peer`, and never a reason to leave a disabled registration half-configured.
pub(super) const TRUSTED_PROXIES: &str = "CANTOS_THEATRE_REGISTRATION_TRUSTED_PROXIES";

/// `direct_peer` (no proxy) or `trusted_proxy` plus the exact proxy networks. No default and no
/// header trust without an explicit network list.
pub(super) fn peer_policy(
    policy: &str,
    proxies: Option<&str>,
) -> Result<PeerPolicy, RegistrationError> {
    match (policy, proxies) {
        ("direct_peer", None) => Ok(PeerPolicy::DirectPeer),
        ("trusted_proxy", Some(list)) => Ok(PeerPolicy::TrustedProxy(TrustedProxies::parse(list)?)),
        _ => Err(RegistrationError::Configuration),
    }
}

/// Approval fields are operator declarations, never proof of effective issuer privilege or
/// delivery. The repository has no credential/default policy bootstrap. Live acceptance and
/// approval for the exact service principal, grants and SMTP settings remain separate.
pub enum RegistrationMode {
    Smtp(RegistrationRuntime),
    Firebase(super::firebase_enrollment::runtime::FirebaseEnrollmentRuntime),
}

/// Select exactly one configured provider; orphan inputs never activate a fallback.
pub fn from_lookup(
    store: Store,
    web_origin: &str,
    mut lookup: impl FnMut(&str) -> Option<String>,
) -> Result<Option<RegistrationMode>, RegistrationError> {
    match lookup("CANTOS_THEATRE_REGISTRATION_PROVIDER").as_deref() {
        None | Some("smtp") => {
            if super::firebase_enrollment::config::has_inputs(&mut lookup) {
                return Err(RegistrationError::Configuration);
            }
            smtp_from_lookup(store, web_origin, lookup)
                .map(|runtime| runtime.map(RegistrationMode::Smtp))
        }
        Some("firebase") => {
            super::firebase_enrollment::config::from_lookup(store, web_origin, lookup)
                .map(|runtime| runtime.map(RegistrationMode::Firebase))
        }
        _ => Err(RegistrationError::Configuration),
    }
}

fn smtp_from_lookup(
    store: Store,
    web_origin: &str,
    mut lookup: impl FnMut(&str) -> Option<String>,
) -> Result<Option<RegistrationRuntime>, RegistrationError> {
    let fields: BTreeMap<_, _> = FIELDS
        .iter()
        .filter_map(|name| lookup(&format!("{PREFIX}{name}")).map(|value| (*name, value)))
        .collect();
    let smtp =
        SmtpConfig::from_lookup(&mut lookup).map_err(|_| RegistrationError::Configuration)?;
    let proxies = lookup(TRUSTED_PROXIES);
    let enabled = fields.get("ENABLED").map(String::as_str);
    if enabled != Some("true") {
        if matches!(enabled, None | Some("false"))
            && fields.keys().all(|key| *key == "ENABLED")
            && smtp.is_none()
            && proxies.is_none()
        {
            return Ok(None);
        }
        return Err(RegistrationError::Configuration);
    }
    let value = |name| {
        fields
            .get(name)
            .map(String::as_str)
            .ok_or(RegistrationError::Configuration)
    };
    if value("POLICY_APPROVED")? != "true"
        || value("ISSUER_ACCESS_APPROVED")? != "true"
        || store.theatre_identity().is_none()
    {
        return Err(RegistrationError::Configuration);
    }
    let peer_policy = peer_policy(value("PEER_POLICY")?, proxies.as_deref())?;
    let number = |name| {
        value(name)?
            .parse::<u32>()
            .map_err(|_| RegistrationError::Configuration)
    };
    let policy = RegistrationPolicy::new(
        number("VERIFICATION_SECONDS")?,
        number("RESEND_SECONDS")?,
        number("WINDOW_SECONDS")?,
        number("MAX_GLOBAL_REQUESTS")?,
        number("MAX_NETWORK_REQUESTS")?,
    )?;
    let attempts = AttemptPolicy::new(
        number("ATTEMPT_WINDOW_SECONDS")?,
        number("ATTEMPT_MAX_GLOBAL")?,
        number("ATTEMPT_MAX_NETWORK")?,
    )?;
    let encoded = value("NETWORK_KEY")?;
    if encoded.len() != 64
        || !encoded
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
    {
        return Err(RegistrationError::Configuration);
    }
    let mut network_key = [0; 32];
    for (index, byte) in network_key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16)
            .map_err(|_| RegistrationError::Configuration)?;
    }
    let issuer = lookup("CANTOS_THEATRE_OIDC_ISSUER").ok_or(RegistrationError::Configuration)?;
    if !store.theatre_identity().is_some_and(|identity| {
        identity.registration_issuer_matches(&issuer) && identity.web_origin() == web_origin
    }) {
        return Err(RegistrationError::Configuration);
    }
    let smtp = smtp.ok_or(RegistrationError::Configuration)?;
    if smtp.is_loopback_fixture() && lookup("CANTOS_ENV").as_deref() != Some("development") {
        return Err(RegistrationError::Configuration);
    }
    let issuer_adapter = KanidmIssuer::new(
        KanidmConfig::new(
            &issuer,
            value("TOKEN")?.to_owned(),
            number("SETUP_SECONDS")?,
        )?
        .with_trust(store.theatre_identity().and_then(|v| v.issuer_trust())),
    )?;
    let mailer = SmtpMailer::new(smtp).map_err(|_| RegistrationError::Configuration)?;
    // Snapshot before attaching the runtime avoids a Store -> service -> Store Arc cycle.
    let repository = PostgresRegistrationRepository::new(store, &issuer)?;
    let service = RegistrationService::new(
        Arc::new(repository),
        Arc::new(issuer_adapter),
        Arc::new(mailer),
        web_origin,
        policy,
    )?;
    RegistrationRuntime::new(
        Arc::new(service),
        web_origin,
        network_key,
        number("MAX_INFLIGHT")?,
        attempts,
    )
    .map(|runtime| Some(runtime.with_peer_policy(peer_policy)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{postgres::local_config, theatre_identity::TheatreIdentity};
    fn store(identity: bool) -> Store {
        let store =
            Store::new(local_config("postgresql://fixture@127.0.0.1:9/fixture").unwrap()).unwrap();
        if identity {
            store.with_theatre_identity(
                TheatreIdentity::new(
                    "https://idm.example/oauth2/openid/cantos-theatre-web-bff",
                    "https://cantos.example",
                    "isolated-fixture-secret".into(),
                )
                .unwrap(),
            )
        } else {
            store
        }
    }
    fn candidate() -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        for (key, value) in [
            ("ENABLED", "true"),
            ("POLICY_APPROVED", "true"),
            ("ISSUER_ACCESS_APPROVED", "true"),
            ("PEER_POLICY", "direct_peer"),
            ("TOKEN", "isolated-fixture-bearer"),
            (
                "NETWORK_KEY",
                "1111111111111111111111111111111111111111111111111111111111111111",
            ),
            ("VERIFICATION_SECONDS", "600"),
            ("RESEND_SECONDS", "60"),
            ("WINDOW_SECONDS", "3600"),
            ("MAX_GLOBAL_REQUESTS", "100"),
            ("MAX_NETWORK_REQUESTS", "10"),
            ("SETUP_SECONDS", "600"),
            ("MAX_INFLIGHT", "4"),
            ("ATTEMPT_WINDOW_SECONDS", "60"),
            ("ATTEMPT_MAX_GLOBAL", "100"),
            ("ATTEMPT_MAX_NETWORK", "10"),
            ("SMTP_HOST", "127.0.0.1"),
            ("SMTP_PORT", "2525"),
            ("SMTP_TLS", "loopback"),
            ("SMTP_FROM", "cantos@example.test"),
        ] {
            values.insert(format!("{PREFIX}{key}"), value.into());
        }
        values.insert(
            "CANTOS_THEATRE_OIDC_ISSUER".into(),
            "https://idm.example/oauth2/openid/cantos-theatre-web-bff".into(),
        );
        values.insert("CANTOS_ENV".into(), "development".into());
        values
    }
    #[tokio::test]
    async fn absent_is_disabled_and_orphan_inputs_fail_closed() {
        assert!(
            from_lookup(store(false), "https://cantos.example", |_| None)
                .unwrap()
                .is_none()
        );
        for name in FIELDS.iter().filter(|v| **v != "ENABLED") {
            assert!(
                from_lookup(store(false), "https://cantos.example", |key| (key
                    == format!("{PREFIX}{name}"))
                .then(|| "fixture".into()))
                .is_err()
            );
        }
        assert!(
            from_lookup(store(false), "https://cantos.example", |key| key
                .ends_with("SMTP_HOST")
                .then(|| "127.0.0.1".into()))
            .is_err()
        );
    }
    #[tokio::test]
    async fn complete_candidate_composes_without_dialing_and_requires_each_explicit_field() {
        let values = candidate();
        assert!(
            from_lookup(store(true), "https://cantos.example", |key| values
                .get(key)
                .cloned())
            .unwrap()
            .is_some()
        );
        for name in FIELDS {
            let mut absent = values.clone();
            absent.remove(&format!("{PREFIX}{name}"));
            assert!(
                from_lookup(store(true), "https://cantos.example", |key| absent
                    .get(key)
                    .cloned())
                .is_err(),
                "{name}"
            );
        }
        assert!(
            from_lookup(store(false), "https://cantos.example", |key| values
                .get(key)
                .cloned())
            .is_err()
        );
    }
    #[tokio::test]
    async fn smtp_trusts_forwarded_headers_only_for_an_explicit_proxy_network_list() {
        let compose = |policy: &str, proxies: Option<&str>| {
            let mut values = candidate();
            values.insert(format!("{PREFIX}PEER_POLICY"), policy.into());
            if let Some(list) = proxies {
                values.insert(format!("{PREFIX}TRUSTED_PROXIES"), list.into());
            }
            from_lookup(store(true), "https://cantos.example", |key| {
                values.get(key).cloned()
            })
            .is_ok()
        };
        assert!(compose("trusted_proxy", Some("10.89.0.0/24,fd00::/64")));
        assert!(!compose("trusted_proxy", None));
        assert!(!compose("trusted_proxy", Some("0.0.0.0/0")));
        assert!(!compose("direct_peer", Some("10.89.0.0/24")));
        assert!(compose("direct_peer", None));
    }
    #[tokio::test]
    async fn no_defaults_approval_bypass_proxy_trust_or_plaintext_remote_activation() {
        for (name, value) in [
            ("POLICY_APPROVED", "false"),
            ("ISSUER_ACCESS_APPROVED", "false"),
            ("PEER_POLICY", "x_forwarded_for"),
            (
                "NETWORK_KEY",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            ("MAX_INFLIGHT", "33"),
            ("VERIFICATION_SECONDS", "0"),
            ("SETUP_SECONDS", "1"),
        ] {
            let mut values = candidate();
            values.insert(format!("{PREFIX}{name}"), value.into());
            assert!(
                from_lookup(store(true), "https://cantos.example", |key| values
                    .get(key)
                    .cloned())
                .is_err(),
                "{name}"
            );
        }
        let mut wrong_issuer = candidate();
        wrong_issuer.insert(
            "CANTOS_THEATRE_OIDC_ISSUER".into(),
            "https://other.example/oauth2/openid/cantos-theatre-web-bff".into(),
        );
        assert!(
            from_lookup(store(true), "https://cantos.example", |key| wrong_issuer
                .get(key)
                .cloned())
            .is_err()
        );
        let values = candidate();
        assert!(
            from_lookup(store(true), "https://other.example", |key| values
                .get(key)
                .cloned())
            .is_err()
        );
        let mut values = candidate();
        values.insert("CANTOS_ENV".into(), "production".into());
        assert!(
            from_lookup(store(true), "https://cantos.example", |key| values
                .get(key)
                .cloned())
            .is_err()
        );
    }
}
