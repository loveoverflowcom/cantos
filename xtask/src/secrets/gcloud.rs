//! The `gcloud` invocations (Secret Manager v1 `projects.secrets.create`, `.addVersion`, `.get`
//! and `versions.list`) as pure argv, plus the interpretation of what gcloud reports.
//!
//! Every call names `--project` and `--account` explicitly, so neither gcloud's active
//! configuration nor ambient credentials choose the target. Payload bytes travel only on stdin
//! (`--data-file=-`): never argv, a temporary file or a log. gcloud's stderr is classified and
//! then dropped; it is never printed, because provider errors may echo request context.

use serde_json::Value;

use super::inventory::Secret;

pub const PROGRAM: &str = "gcloud";

/// Environment for every gcloud child: no interactive prompt can block or ask for input.
pub const ENV: [(&str, &str); 1] = [("CLOUDSDK_CORE_DISABLE_PROMPTS", "1")];

fn scoped(mut args: Vec<String>, project: &str, account: &str) -> Vec<String> {
    args.push(format!("--project={project}"));
    args.push(format!("--account={account}"));
    args
}

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}

/// Lists credentialed accounts filtered to exactly this one (read-only, local).
pub fn auth_list(account: &str) -> Vec<String> {
    vec![
        "auth".into(),
        "list".into(),
        format!("--filter-account={account}"),
        "--format=value(account)".into(),
    ]
}

pub fn describe(id: &str, project: &str, account: &str) -> Vec<String> {
    let mut args = words(&["secrets", "describe", id]);
    args.push("--format=json".into());
    scoped(args, project, account)
}

/// Creates metadata only (no payload). Labels are nonsecret and help an operator audit scope.
pub fn create(secret: &Secret, project: &str, account: &str) -> Vec<String> {
    let mut args = words(&["secrets", "create", &secret.id]);
    args.push("--replication-policy=automatic".into());
    args.push(format!(
        "--labels=managed-by=cantos-xtask,environment={},consumer={}",
        secret.environment.as_str(),
        secret.consumer.as_str()
    ));
    args.push("--format=json".into());
    scoped(args, project, account)
}

/// Lists every version in every state (enabled, disabled, destroyed): the fence needs the
/// highest number ever issued, not only the usable ones.
pub fn versions_list(id: &str, project: &str, account: &str) -> Vec<String> {
    let mut args = words(&["secrets", "versions", "list", id]);
    args.push("--format=json".into());
    scoped(args, project, account)
}

/// Adds one immutable version whose bytes are read from stdin.
pub fn add_version(id: &str, project: &str, account: &str) -> Vec<String> {
    let mut args = words(&["secrets", "versions", "add", id]);
    args.push("--data-file=-".into());
    args.push("--format=json".into());
    scoped(args, project, account)
}

pub fn display(args: &[String]) -> String {
    let mut line = ENV
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(" ");
    line.push(' ');
    line.push_str(PROGRAM);
    for arg in args {
        line.push(' ');
        line.push_str(arg);
    }
    line
}

/// What a finished gcloud run means, from its exit code and stderr only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    NotFound,
    AlreadyExists,
    /// Secret Manager API or billing is not enabled for the project.
    ProjectNotReady,
    PermissionDenied,
    AuthRequired,
    InvalidArgument,
    /// Connection, timeout or availability failure: the request may or may not have arrived.
    Transport,
    Unknown,
}

impl Failure {
    /// The server definitely rejected the request; nothing was written.
    pub fn definite(self) -> bool {
        !matches!(self, Failure::Transport | Failure::Unknown)
    }

    pub fn explain(self) -> &'static str {
        match self {
            Failure::NotFound => "NOT_FOUND",
            Failure::AlreadyExists => "ALREADY_EXISTS",
            Failure::ProjectNotReady => {
                "the Secret Manager API or billing is not enabled for this project (owner action)"
            }
            Failure::PermissionDenied => {
                "PERMISSION_DENIED: the account lacks the Secret Manager role this step needs"
            }
            Failure::AuthRequired => {
                "the account has no usable gcloud credential; the operator runs `gcloud auth login` themself"
            }
            Failure::InvalidArgument => "INVALID_ARGUMENT",
            Failure::Transport => "transport failure or timeout",
            Failure::Unknown => "unrecognized gcloud failure (details withheld)",
        }
    }
}

pub fn classify(stderr: &str) -> Failure {
    let has = |needles: &[&str]| needles.iter().any(|n| stderr.contains(n));
    if has(&[
        "SERVICE_DISABLED",
        "has not been used in project",
        "BILLING_DISABLED",
        "billing to be enabled",
    ]) {
        Failure::ProjectNotReady
    } else if has(&[
        "PERMISSION_DENIED",
        "does not have permission",
        "Permission denied on resource",
    ]) {
        // Before NOT_FOUND: a denial may add "or it may not exist", which must never read as
        // "absent, so create it".
        Failure::PermissionDenied
    } else if has(&["NOT_FOUND", "was not found"]) {
        Failure::NotFound
    } else if has(&["ALREADY_EXISTS", "already exists"]) {
        Failure::AlreadyExists
    } else if has(&[
        "UNAUTHENTICATED",
        "Reauthentication",
        "do not currently have an active account",
        "invalid_grant",
        "There was a problem refreshing your current auth tokens",
    ]) {
        Failure::AuthRequired
    } else if has(&["INVALID_ARGUMENT"]) {
        Failure::InvalidArgument
    } else if has(&[
        "DEADLINE_EXCEEDED",
        "UNAVAILABLE",
        "Unable to connect",
        "timed out",
        "Connection reset",
        "ConnectionError",
        "ServerNotFoundError",
    ]) {
        Failure::Transport
    } else {
        Failure::Unknown
    }
}

/// `projects/<number>/secrets/<id>/versions/<n>` → `n`, when the secret ID matches.
fn version_of(name: &str, id: &str) -> Option<u64> {
    let (secret, version) = name.rsplit_once("/versions/")?;
    if !secret.ends_with(&format!("/secrets/{id}")) {
        return None;
    }
    version.parse().ok().filter(|v| *v > 0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub number: u64,
    pub state: String,
}

/// Parses `versions list --format=json`. Malformed output is an error, never "no versions".
pub fn parse_versions(stdout: &str, id: &str) -> Option<Vec<Version>> {
    let Value::Array(items) = serde_json::from_str::<Value>(stdout).ok()? else {
        return None;
    };
    items
        .iter()
        .map(|item| {
            Some(Version {
                number: version_of(item.get("name")?.as_str()?, id)?,
                state: item.get("state")?.as_str()?.to_owned(),
            })
        })
        .collect()
}

pub fn latest(versions: &[Version]) -> Option<u64> {
    versions.iter().map(|v| v.number).max()
}

/// Parses `versions add --format=json`: the new version's number.
pub fn parse_added(stdout: &str, id: &str) -> Option<u64> {
    let value: Value = serde_json::from_str(stdout).ok()?;
    version_of(value.get("name")?.as_str()?, id)
}

/// `secrets describe` / `create` output names this exact secret.
pub fn names_secret(stdout: &str, id: &str) -> bool {
    serde_json::from_str::<Value>(stdout)
        .ok()
        .and_then(|value| value.get("name")?.as_str().map(str::to_owned))
        .is_some_and(|name| {
            name.ends_with(&format!("/secrets/{id}")) && name.starts_with("projects/")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::inventory::{Consumer, Delivery, Environment, Format};

    fn secret() -> Secret {
        Secret {
            id: "cantos-prod-server-playback-key".into(),
            environment: Environment::Prod,
            consumer: Consumer::CantosServer,
            contract: "CANTOS_PLAYBACK_KEY_FILE".into(),
            delivery: Delivery::File,
            format: Format::Bytes32,
            pinned_version: None,
            rotation_required: false,
        }
    }

    #[test]
    fn every_remote_call_names_project_and_account_and_never_a_payload_path() {
        let s = secret();
        for args in [
            describe(&s.id, "cantos-a348b", "op@example.test"),
            create(&s, "cantos-a348b", "op@example.test"),
            versions_list(&s.id, "cantos-a348b", "op@example.test"),
            add_version(&s.id, "cantos-a348b", "op@example.test"),
        ] {
            assert!(
                args.contains(&"--project=cantos-a348b".to_owned()),
                "{args:?}"
            );
            assert!(
                args.contains(&"--account=op@example.test".to_owned()),
                "{args:?}"
            );
            assert!(!args.iter().any(|a| a.contains("latest")), "{args:?}");
        }
        let add = add_version(&s.id, "cantos-a348b", "op@example.test");
        assert!(add.contains(&"--data-file=-".to_owned()));
        let create = create(&s, "cantos-a348b", "op@example.test");
        assert!(create.contains(&"--replication-policy=automatic".to_owned()));
        assert!(create.iter().any(
            |a| a == "--labels=managed-by=cantos-xtask,environment=prod,consumer=cantos-server"
        ));
        assert!(display(&add)
            .starts_with("CLOUDSDK_CORE_DISABLE_PROMPTS=1 gcloud secrets versions add"));
    }

    #[test]
    fn failures_are_classified_and_only_transport_or_unknown_are_uncertain() {
        let cases = [
            ("ERROR: (gcloud.secrets.describe) NOT_FOUND: Secret [x] not found", Failure::NotFound),
            ("ERROR: ALREADY_EXISTS: Secret [x] already exists.", Failure::AlreadyExists),
            ("PERMISSION_DENIED: Secret Manager API has not been used in project 1 before or it is disabled. reason: SERVICE_DISABLED", Failure::ProjectNotReady),
            ("PERMISSION_DENIED: Permission 'secretmanager.versions.add' denied", Failure::PermissionDenied),
            ("ERROR: (gcloud.secrets.list) You do not currently have an active account selected.", Failure::AuthRequired),
            ("ERROR: There was a problem refreshing your current auth tokens: invalid_grant", Failure::AuthRequired),
            ("INVALID_ARGUMENT: payload too large", Failure::InvalidArgument),
            ("ERROR: gcloud crashed (ConnectionError): HTTPSConnectionPool timed out", Failure::Transport),
            ("something new", Failure::Unknown),
        ];
        for (stderr, expected) in cases {
            assert_eq!(classify(stderr), expected, "{stderr}");
        }
        assert!(Failure::PermissionDenied.definite());
        assert!(Failure::NotFound.definite());
        assert!(!Failure::Transport.definite());
        assert!(!Failure::Unknown.definite());
    }

    #[test]
    fn version_output_is_parsed_strictly() {
        let id = "cantos-prod-server-playback-key";
        let listed = r#"[
          {"name":"projects/123/secrets/cantos-prod-server-playback-key/versions/2","state":"ENABLED"},
          {"name":"projects/123/secrets/cantos-prod-server-playback-key/versions/1","state":"DESTROYED"}
        ]"#;
        let versions = parse_versions(listed, id).unwrap();
        assert_eq!(latest(&versions), Some(2));
        assert_eq!(parse_versions("[]", id), Some(vec![]));
        assert_eq!(latest(&[]), None);
        assert!(parse_versions("not json", id).is_none());
        assert!(parse_versions(
            r#"[{"name":"projects/1/secrets/other/versions/9","state":"ENABLED"}]"#,
            id
        )
        .is_none());
        assert_eq!(
            parse_added(
                r#"{"name":"projects/123/secrets/cantos-prod-server-playback-key/versions/3","state":"ENABLED"}"#,
                id
            ),
            Some(3)
        );
        assert_eq!(parse_added("{}", id), None);
        assert!(names_secret(
            r#"{"name":"projects/123/secrets/cantos-prod-server-playback-key"}"#,
            id
        ));
        assert!(!names_secret(
            r#"{"name":"projects/123/secrets/other"}"#,
            id
        ));
    }
}
