//! Startup-only selected Firebase composition. No credential creation, login or remote probes.
use super::{
    application::FirebaseEnrollmentService, postgres::PostgresFirebaseEnrollmentRepository,
    runtime::FirebaseEnrollmentRuntime,
};
use crate::platform::{
    postgres::Store,
    theatre_registration::{
        config::{peer_policy, FIELDS as COMMON_FIELDS, TRUSTED_PROXIES},
        domain::{RegistrationError, RegistrationPolicy},
        firebase_action::{ApiKey, FirebaseActionClient, FirebaseProjectId},
        firebase_bootstrap::{
            FirebaseBootstrapClient, FirebaseBootstrapConfig, FirebaseMailboxProvider,
        },
        kanidm::{KanidmConfig, KanidmIssuer},
        mail_config::SmtpConfig,
        runtime::AttemptPolicy,
    },
};
use std::{collections::BTreeMap, io::Read, path::Path, sync::Arc};
const PREFIX: &str = "CANTOS_THEATRE_REGISTRATION_FIREBASE_";
#[cfg(test)]
const PREFIX_COMMON: &str = "CANTOS_THEATRE_REGISTRATION_";
// ADMIN_ACCESS_TOKEN is retired: a hand-supplied bearer expires in about an hour. The API mints
// its own short-lived token from SERVICE_ACCOUNT_FILE, so a static token is refused, never mixed.
const UNSUPPORTED: &[&str] = &[
    "TENANT_ID",
    "EMULATOR_HOST",
    "ENDPOINT",
    "ACTION_ORIGIN",
    "ADMIN_ACCESS_TOKEN",
];
const FIELDS: &[&str] = &[
    "PROJECT_ID",
    "API_KEY",
    "SERVICE_ACCOUNT_FILE",
    "PROJECT_BINDING_APPROVED",
    "ACTION_HANDLER_APPROVED",
    "PROVIDER_ACCESS_APPROVED",
];

pub(crate) fn has_inputs(lookup: &mut impl FnMut(&str) -> Option<String>) -> bool {
    FIELDS
        .iter()
        .chain(UNSUPPORTED.iter())
        .any(|name| lookup(&format!("{PREFIX}{name}")).is_some())
}

pub fn from_lookup(
    store: Store,
    web_origin: &str,
    mut lookup: impl FnMut(&str) -> Option<String>,
) -> Result<Option<FirebaseEnrollmentRuntime>, RegistrationError> {
    if UNSUPPORTED
        .iter()
        .any(|name| lookup(&format!("{PREFIX}{name}")).is_some())
        || lookup("FIREBASE_AUTH_EMULATOR_HOST").is_some()
    {
        return Err(RegistrationError::Configuration);
    }
    let fields: BTreeMap<_, _> = FIELDS
        .iter()
        .filter_map(|name| lookup(&format!("{PREFIX}{name}")).map(|v| (*name, v)))
        .collect();
    let common: BTreeMap<_, _> = COMMON_FIELDS
        .iter()
        .filter_map(|name| {
            lookup(&format!("CANTOS_THEATRE_REGISTRATION_{name}")).map(|v| (*name, v))
        })
        .collect();
    // Firebase is a proof provider, never an SMTP Mailer or an automatic fallback.
    if SmtpConfig::from_lookup(&mut lookup)
        .map_err(|_| RegistrationError::Configuration)?
        .is_some()
    {
        return Err(RegistrationError::Configuration);
    }
    let proxies = lookup(TRUSTED_PROXIES);
    if common.get("ENABLED").map(String::as_str) != Some("true") {
        if common.get("ENABLED").map(String::as_str) == Some("false")
            && common.len() == 1
            && fields.is_empty()
            && proxies.is_none()
        {
            return Ok(None);
        }
        return Err(RegistrationError::Configuration);
    }
    let value = |name| {
        common
            .get(name)
            .map(String::as_str)
            .ok_or(RegistrationError::Configuration)
    };
    let firebase = |name| {
        fields
            .get(name)
            .map(String::as_str)
            .ok_or(RegistrationError::Configuration)
    };
    let number = |name| {
        value(name)?
            .parse::<u32>()
            .map_err(|_| RegistrationError::Configuration)
    };
    for field in [
        "PROJECT_BINDING_APPROVED",
        "ACTION_HANDLER_APPROVED",
        "PROVIDER_ACCESS_APPROVED",
    ] {
        if firebase(field)? != "true" {
            return Err(RegistrationError::Configuration);
        }
    }
    if value("POLICY_APPROVED")? != "true" || value("ISSUER_ACCESS_APPROVED")? != "true" {
        return Err(RegistrationError::Configuration);
    }
    let peer_policy = peer_policy(value("PEER_POLICY")?, proxies.as_deref())?;
    let issuer = lookup("CANTOS_THEATRE_OIDC_ISSUER").ok_or(RegistrationError::Configuration)?;
    if !store.theatre_identity().is_some_and(|identity| {
        identity.registration_issuer_matches(&issuer) && identity.web_origin() == web_origin
    }) {
        return Err(RegistrationError::Configuration);
    }
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
    let project = firebase("PROJECT_ID")?;
    let key = firebase("API_KEY")?;
    let actions = FirebaseActionClient::new(
        FirebaseProjectId::parse(project).map_err(|_| RegistrationError::Configuration)?,
        ApiKey::parse(key).map_err(|_| RegistrationError::Configuration)?,
    )
    .map_err(|_| RegistrationError::Configuration)?;
    // Read only this operator-supplied existing secret. Never print a path/body or write keys.
    let secret = read_existing_secret(firebase("SERVICE_ACCOUNT_FILE")?)?;
    let bootstrap = FirebaseBootstrapClient::new(
        FirebaseBootstrapConfig::parse(project, key, &secret)
            .map_err(|_| RegistrationError::Configuration)?,
    )
    .map_err(|_| RegistrationError::Configuration)?;
    let mailbox = FirebaseMailboxProvider::new(bootstrap, web_origin)
        .map_err(|_| RegistrationError::Configuration)?;
    let issuer_adapter = KanidmIssuer::new(
        KanidmConfig::new(
            &issuer,
            value("TOKEN")?.to_owned(),
            number("SETUP_SECONDS")?,
        )?
        .with_trust(store.theatre_identity().and_then(|v| v.issuer_trust())),
    )?;
    let repository = PostgresFirebaseEnrollmentRepository::new(store, &issuer, project)?;
    let service = FirebaseEnrollmentService::new(
        Arc::new(repository),
        Arc::new(mailbox),
        Arc::new(actions),
        Arc::new(issuer_adapter),
        project,
        policy,
        &issuer,
    )?;
    FirebaseEnrollmentRuntime::new(
        Arc::new(service),
        web_origin,
        network_key,
        number("MAX_INFLIGHT")?,
        attempts,
    )
    .map(|runtime| Some(runtime.with_peer_policy(peer_policy)))
}

fn read_existing_secret(value: &str) -> Result<Vec<u8>, RegistrationError> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(RegistrationError::Configuration);
    }
    let file = std::fs::File::open(path).map_err(|_| RegistrationError::Configuration)?;
    if !file
        .metadata()
        .map_err(|_| RegistrationError::Configuration)?
        .is_file()
    {
        return Err(RegistrationError::Configuration);
    }
    let mut bytes = Vec::new();
    file.take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|_| RegistrationError::Configuration)?;
    if bytes.is_empty() || bytes.len() > 65_536 {
        return Err(RegistrationError::Configuration);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> Store {
        Store::new(
            crate::platform::postgres::local_config("postgresql://fixture@127.0.0.1:9/fixture")
                .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn retired_static_bearer_counts_as_an_input_and_is_refused_alone() {
        let lookup = |name: &str| {
            (name == format!("{PREFIX}ADMIN_ACCESS_TOKEN"))
                .then(|| "synthetic-admin-bearer".to_owned())
        };
        assert!(has_inputs(&mut |name| lookup(name)));
        assert!(!FIELDS.contains(&"ADMIN_ACCESS_TOKEN"));
        assert!(from_lookup(store(), "https://cantos.example", lookup).is_err());
    }
    #[tokio::test]
    async fn selected_firebase_requires_explicit_enable_and_refuses_every_partial_or_smtp_fallback()
    {
        assert!(from_lookup(store(), "https://cantos.example", |_| None).is_err());
        assert!(from_lookup(store(), "https://cantos.example", |name| (name
            == "CANTOS_THEATRE_REGISTRATION_ENABLED")
            .then(|| "false".into()))
        .unwrap()
        .is_none());
        for field in FIELDS {
            assert!(
                from_lookup(store(), "https://cantos.example", |name| (name
                    == format!("{PREFIX}{field}"))
                .then(|| "fixture".into()))
                .is_err(),
                "{field}"
            );
        }
        assert!(super::super::super::config::from_lookup(
            store(),
            "https://cantos.example",
            |name| (name == format!("{PREFIX}PROJECT_ID")).then(|| "cantos".into())
        )
        .is_err());
        assert!(super::super::super::config::from_lookup(
            store(),
            "https://cantos.example",
            |name| (name == "CANTOS_THEATRE_REGISTRATION_PROVIDER").then(|| "automatic".into())
        )
        .is_err());
    }
    #[test]
    fn secret_loader_refuses_relative_missing_and_non_file_without_disclosure() {
        for value in [
            "relative-secret.json",
            "/nonexistent/cantos-fixture-key",
            "/tmp",
        ] {
            assert_eq!(
                read_existing_secret(value),
                Err(RegistrationError::Configuration)
            );
        }
    }
    #[tokio::test]
    async fn complete_explicit_firebase_configuration_composes_without_network_and_requires_every_field(
    ) {
        use crate::platform::theatre_identity::TheatreIdentity;
        let secret_path = std::env::temp_dir().join(format!(
            "cantos-synthetic-account-{}.json",
            uuid::Uuid::new_v4()
        ));
        let secret =
            crate::platform::theatre_registration::firebase_bootstrap::tests::service_account();
        std::fs::write(&secret_path, serde_json::to_vec(&secret).unwrap()).unwrap();
        struct Remove(std::path::PathBuf);
        impl Drop for Remove {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _cleanup = Remove(secret_path.clone());
        let mut fields = BTreeMap::<String, String>::new();
        for (name, value) in [
            ("ENABLED", "true"),
            ("PROVIDER", "firebase"),
            ("POLICY_APPROVED", "true"),
            ("ISSUER_ACCESS_APPROVED", "true"),
            ("PEER_POLICY", "direct_peer"),
            ("TOKEN", "synthetic-kanidm-token"),
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
            ("FIREBASE_PROJECT_ID", "candidate-project"),
            ("FIREBASE_API_KEY", "synthetic-key"),
            ("FIREBASE_PROJECT_BINDING_APPROVED", "true"),
            ("FIREBASE_ACTION_HANDLER_APPROVED", "true"),
            ("FIREBASE_PROVIDER_ACCESS_APPROVED", "true"),
        ] {
            fields.insert(
                format!("CANTOS_THEATRE_REGISTRATION_{name}"),
                value.to_owned(),
            );
        }
        fields.insert(
            format!("{PREFIX}SERVICE_ACCOUNT_FILE"),
            secret_path.to_str().unwrap().to_owned(),
        );
        fields.insert(
            "CANTOS_THEATRE_OIDC_ISSUER".into(),
            "https://idm.example/oauth2/openid/cantos-theatre-web-bff".into(),
        );
        let identity_store = || {
            store().with_theatre_identity(
                TheatreIdentity::new(
                    "https://idm.example/oauth2/openid/cantos-theatre-web-bff",
                    "https://cantos.example",
                    "synthetic-oidc-secret".into(),
                )
                .unwrap(),
            )
        };
        let composed = super::super::super::config::from_lookup(
            identity_store(),
            "https://cantos.example",
            |name| fields.get(name).cloned(),
        )
        .unwrap();
        assert!(matches!(
            composed,
            Some(super::super::super::config::RegistrationMode::Firebase(_))
        ));
        for field in FIELDS {
            let mut missing = fields.clone();
            missing.remove(&format!("{PREFIX}{field}"));
            assert!(
                super::super::super::config::from_lookup(
                    identity_store(),
                    "https://cantos.example",
                    |name| missing.get(name).cloned()
                )
                .is_err(),
                "{field}"
            );
        }
        // A proxy list is evidence only with `trusted_proxy`, and `trusted_proxy` needs the list.
        let peer = |policy: &str, proxies: Option<&str>| {
            let mut candidate = fields.clone();
            candidate.insert(format!("{PREFIX_COMMON}PEER_POLICY"), policy.into());
            if let Some(list) = proxies {
                candidate.insert(format!("{PREFIX_COMMON}TRUSTED_PROXIES"), list.into());
            }
            super::super::super::config::from_lookup(
                identity_store(),
                "https://cantos.example",
                |name| candidate.get(name).cloned(),
            )
            .is_ok()
        };
        assert!(peer("trusted_proxy", Some("10.89.0.0/24")));
        assert!(!peer("trusted_proxy", None));
        assert!(!peer("trusted_proxy", Some("0.0.0.0/0")));
        assert!(!peer("trusted_proxy", Some("edge.internal/24")));
        assert!(!peer("direct_peer", Some("10.89.0.0/24")));
        assert!(!peer("x_forwarded_for", Some("10.89.0.0/24")));
        assert!(
            super::super::super::config::from_lookup(store(), "https://cantos.example", |name| {
                (name == "CANTOS_THEATRE_REGISTRATION_TRUSTED_PROXIES")
                    .then(|| "10.89.0.0/24".to_owned())
            })
            .is_err(),
            "an orphan proxy list never leaves registration quietly disabled"
        );
        for (field, value) in [
            (
                "CANTOS_THEATRE_REGISTRATION_FIREBASE_PROJECT_ID",
                "different-project",
            ),
            (
                "CANTOS_THEATRE_REGISTRATION_FIREBASE_PROJECT_BINDING_APPROVED",
                "false",
            ),
            (
                "CANTOS_THEATRE_REGISTRATION_FIREBASE_TENANT_ID",
                "unsupported",
            ),
            // The retired static bearer is refused, never mixed with the minting path.
            (
                "CANTOS_THEATRE_REGISTRATION_FIREBASE_ADMIN_ACCESS_TOKEN",
                "synthetic-admin-bearer",
            ),
            ("FIREBASE_AUTH_EMULATOR_HOST", "127.0.0.1:9099"),
            ("CANTOS_THEATRE_REGISTRATION_SMTP_HOST", "127.0.0.1"),
        ] {
            let mut invalid = fields.clone();
            invalid.insert(field.into(), value.into());
            assert!(
                super::super::super::config::from_lookup(
                    identity_store(),
                    "https://cantos.example",
                    |name| invalid.get(name).cloned()
                )
                .is_err(),
                "{field}"
            );
        }
    }
}
