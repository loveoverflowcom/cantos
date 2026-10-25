//! The tracked, nonsecret inventory (`infra/google-secret-manager/inventory.toml`), parsed and
//! validated as a pure value. Every later step only accepts a secret through [`Inventory`], so an
//! unlisted ID, another project or an ops secret bound to an application cannot reach `gcloud`.

use std::{collections::BTreeSet, fmt};

use serde::Deserialize;

pub const INVENTORY_FILE: &str = "infra/google-secret-manager/inventory.toml";

/// Secret Manager's own payload limit.
pub const MAX_PAYLOAD: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Environment {
    Dev,
    Prod,
    Ops,
}

impl Environment {
    pub fn as_str(self) -> &'static str {
        match self {
            Environment::Dev => "dev",
            Environment::Prod => "prod",
            Environment::Ops => "ops",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Consumer {
    CantosServer,
    CantosIdentity,
    CiDeploy,
    Operator,
}

impl Consumer {
    pub fn as_str(self) -> &'static str {
        match self {
            Consumer::CantosServer => "cantos-server",
            Consumer::CantosIdentity => "cantos-identity",
            Consumer::CiDeploy => "ci-deploy",
            Consumer::Operator => "operator",
        }
    }

    fn is_application(self) -> bool {
        matches!(self, Consumer::CantosServer | Consumer::CantosIdentity)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Delivery {
    Env,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    Text,
    Bytes32,
    ServiceAccountJson,
    Opaque,
}

impl Format {
    pub fn as_str(self) -> &'static str {
        match self {
            Format::Text => "text",
            Format::Bytes32 => "bytes32",
            Format::ServiceAccountJson => "service-account-json",
            Format::Opaque => "opaque",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Secret {
    pub id: String,
    pub environment: Environment,
    pub consumer: Consumer,
    pub contract: String,
    pub delivery: Delivery,
    pub format: Format,
    #[serde(default)]
    pub pinned_version: Option<u64>,
    #[serde(default)]
    pub rotation_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    schema_version: u32,
    project: String,
    replication: String,
    #[serde(rename = "secret")]
    secrets: Vec<Secret>,
}

/// A validated inventory: one project, unique IDs, no cross-environment binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub project: String,
    pub secrets: Vec<Secret>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryError(String);

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{INVENTORY_FILE}: {}", self.0)
    }
}

fn invalid(message: impl Into<String>) -> InventoryError {
    InventoryError(message.into())
}

/// Google project IDs: 6–30 characters, lowercase letter first, letters/digits/hyphens, no
/// trailing hyphen.
pub fn valid_project(project: &str) -> bool {
    let bytes = project.as_bytes();
    (6..=30).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        && !project.ends_with('-')
}

/// Secret Manager secret IDs: 1–255 of `[A-Za-z0-9_-]`.
pub fn valid_secret_id(id: &str) -> bool {
    (1..=255).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn parse(text: &str) -> Result<Inventory, InventoryError> {
    let raw: Raw = toml::from_str(text).map_err(|error| invalid(error.message().to_owned()))?;
    if raw.schema_version != 1 {
        return Err(invalid("schema_version must be 1"));
    }
    if !valid_project(&raw.project) {
        return Err(invalid("project is not a valid Google Cloud project ID"));
    }
    if raw.replication != "automatic" {
        return Err(invalid(
            "replication must be `automatic` unless an owner policy selects a region",
        ));
    }
    if raw.secrets.is_empty() {
        return Err(invalid("no [[secret]] entries"));
    }
    let mut seen = BTreeSet::new();
    for secret in &raw.secrets {
        validate(secret)?;
        if !seen.insert(secret.id.as_str()) {
            return Err(invalid(format!("duplicate secret id {}", secret.id)));
        }
    }
    Ok(Inventory {
        project: raw.project,
        secrets: raw.secrets,
    })
}

fn validate(secret: &Secret) -> Result<(), InventoryError> {
    let id = &secret.id;
    if !valid_secret_id(id) {
        return Err(invalid(format!("{id:?} is not a valid secret ID")));
    }
    let prefix = format!("cantos-{}-", secret.environment.as_str());
    if !id.starts_with(&prefix) {
        return Err(invalid(format!(
            "{id} must start with {prefix} to match its environment"
        )));
    }
    if secret.contract.trim().is_empty() {
        return Err(invalid(format!("{id} names no consumer contract")));
    }
    // Ops credentials (deploy tokens, signing keys, SSH) are never readable by an application, and
    // an application secret is never handed to a deploy job or a generic operator role.
    match (secret.environment, secret.consumer.is_application()) {
        (Environment::Ops, true) => {
            return Err(invalid(format!(
                "{id} is an ops secret and cannot be bound to {}",
                secret.consumer.as_str()
            )))
        }
        (Environment::Dev | Environment::Prod, false) => {
            return Err(invalid(format!(
            "{id} is a {} runtime secret; its consumer must be cantos-server or cantos-identity",
            secret.environment.as_str()
        )))
        }
        _ => {}
    }
    if matches!(secret.format, Format::Bytes32 | Format::ServiceAccountJson)
        && secret.delivery != Delivery::File
    {
        return Err(invalid(format!(
            "{id} is {} and must be delivered as a private file",
            secret.format.as_str()
        )));
    }
    if secret.pinned_version == Some(0) {
        return Err(invalid(format!("{id} pinned_version must be positive")));
    }
    if secret.pinned_version.is_some() && secret.environment == Environment::Dev {
        return Err(invalid(format!(
            "{id} is a development secret; pins are reviewed production/ops bindings"
        )));
    }
    Ok(())
}

impl Inventory {
    pub fn find(&self, id: &str) -> Option<&Secret> {
        self.secrets.iter().find(|secret| secret.id == id)
    }

    /// A reviewable table of names only: never values, sizes or hashes.
    pub fn render(&self) -> String {
        let mut out = format!("project {}\n", self.project);
        for secret in &self.secrets {
            let delivery = match secret.delivery {
                Delivery::Env => "env",
                Delivery::File => "file",
            };
            let pin = secret
                .pinned_version
                .map_or_else(|| "unpinned".to_owned(), |v| format!("pinned v{v}"));
            let rotation = if secret.rotation_required {
                ", ROTATION_REQUIRED"
            } else {
                ""
            };
            out.push_str(&format!(
                "  {:<46} {:<4} {:<15} {delivery}:{} ({}, {pin}{rotation})\n",
                secret.id,
                secret.environment.as_str(),
                secret.consumer.as_str(),
                secret.contract,
                secret.format.as_str(),
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str =
        "schema_version = 1\nproject = \"cantos-a348b\"\nreplication = \"automatic\"\n";

    fn with(secret: &str) -> Result<Inventory, InventoryError> {
        parse(&format!("{HEADER}\n[[secret]]\n{secret}"))
    }

    const IDENTITY_KEY: &str = "id = \"cantos-prod-identity-firebase-envelope-key\"\nenvironment = \"prod\"\nconsumer = \"cantos-identity\"\ncontract = \"CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE\"\ndelivery = \"file\"\nformat = \"bytes32\"\n";

    #[test]
    fn the_tracked_inventory_is_valid_and_targets_the_owner_selected_project() {
        let root = env!("CARGO_MANIFEST_DIR");
        let text = std::fs::read_to_string(format!("{root}/../{INVENTORY_FILE}")).unwrap();
        let inventory = parse(&text).unwrap();
        assert_eq!(inventory.project, "cantos-a348b");
        // Each file contract of the native owners is delivered as a file, never an env value.
        for contract in [
            "CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE",
            "CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE",
            "CANTOS_PLAYBACK_KEY_FILE",
        ] {
            let secret = inventory
                .secrets
                .iter()
                .find(|s| s.contract == contract && s.environment == Environment::Prod)
                .unwrap_or_else(|| panic!("{contract} is inventoried"));
            assert_eq!(secret.delivery, Delivery::File, "{contract}");
        }
        // The SSH key is ops-only and readable by the operator role alone.
        let ssh = inventory.find("cantos-ops-vps-ssh-private-key").unwrap();
        assert_eq!(ssh.consumer, Consumer::Operator);
        assert!(inventory
            .render()
            .contains("cantos-prod-server-playback-key"));
    }

    #[test]
    fn a_valid_entry_parses() {
        let inventory = with(IDENTITY_KEY).unwrap();
        assert_eq!(inventory.secrets.len(), 1);
        assert!(inventory
            .find("cantos-prod-identity-firebase-envelope-key")
            .is_some());
        assert!(inventory.find("cantos-prod-unlisted").is_none());
    }

    #[test]
    fn cross_environment_bindings_are_refused() {
        let ops_to_app = IDENTITY_KEY
            .replace("cantos-prod-", "cantos-ops-")
            .replace("\"prod\"", "\"ops\"");
        assert!(with(&ops_to_app).unwrap_err().0.contains("ops secret"));
        let prod_to_ci = IDENTITY_KEY.replace("cantos-identity\"", "ci-deploy\"");
        assert!(with(&prod_to_ci).unwrap_err().0.contains("runtime secret"));
        let mislabelled = IDENTITY_KEY.replace("\"prod\"", "\"dev\"");
        assert!(with(&mislabelled)
            .unwrap_err()
            .0
            .contains("must start with"));
    }

    #[test]
    fn file_contracts_cannot_be_delivered_as_environment_values() {
        let env = IDENTITY_KEY.replace("\"file\"", "\"env\"");
        assert!(with(&env).unwrap_err().0.contains("private file"));
    }

    #[test]
    fn pins_are_positive_numbers_never_latest() {
        assert!(with(&format!("{IDENTITY_KEY}pinned_version = 3\n")).is_ok());
        assert!(with(&format!("{IDENTITY_KEY}pinned_version = 0\n")).is_err());
        assert!(with(&format!("{IDENTITY_KEY}pinned_version = \"latest\"\n")).is_err());
        assert!(with(&format!("{IDENTITY_KEY}pinned_version = -1\n")).is_err());
    }

    #[test]
    fn unknown_fields_values_and_duplicates_are_refused() {
        assert!(with(&format!("{IDENTITY_KEY}value = \"x\"\n")).is_err());
        let twice = format!("{HEADER}\n[[secret]]\n{IDENTITY_KEY}\n[[secret]]\n{IDENTITY_KEY}");
        assert!(parse(&twice).unwrap_err().0.contains("duplicate"));
        let other = with(IDENTITY_KEY).unwrap();
        assert_eq!(other.project, "cantos-a348b");
        assert!(parse(&format!(
            "{}\n[[secret]]\n{IDENTITY_KEY}",
            HEADER.replace("cantos-a348b", "Cantos_Prod")
        ))
        .is_err());
        assert!(parse(&format!(
            "{}\n[[secret]]\n{IDENTITY_KEY}",
            HEADER.replace("automatic", "user-managed")
        ))
        .is_err());
    }

    #[test]
    fn identifiers_follow_google_rules() {
        assert!(valid_project("cantos-a348b"));
        assert!(!valid_project("cantos-"));
        assert!(!valid_project("1cantos"));
        assert!(!valid_project("short"));
        assert!(valid_secret_id("cantos-prod-server-playback-key"));
        assert!(!valid_secret_id("cantos/prod"));
        assert!(!valid_secret_id(""));
        assert!(!valid_secret_id(&"a".repeat(256)));
    }
}
