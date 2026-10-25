//! Orders the remote steps and stops at the first failure. Each port is a trait so the whole
//! sequence runs against a scripted gcloud in tests; nothing here touches a real project.
//!
//! Import: account → secret (get, else create metadata) → version fence → add version.
//! Status: account → per-secret version listing and pin check (read-only).

use super::{
    gcloud::{self, Failure},
    inventory::{Inventory, Secret},
    request::{Expect, Import},
};
use crate::report::{Report, Status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    NotInstalled,
    /// `code` is `None` when the process was killed by a signal.
    Exited {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
}

pub trait Gcloud {
    fn run(&mut self, args: &[String], stdin: Option<&[u8]>) -> Invocation;
}

/// One sanitized line per remote write attempt: no payload, hash, size or path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub project: String,
    pub secret: String,
    pub account: String,
    pub action: &'static str,
    pub result: &'static str,
    pub version: Option<u64>,
}

pub trait Ledger {
    fn append(&mut self, entry: &LedgerEntry);
}

enum Ran {
    Ok(String),
    Failed(Failure),
    /// Killed or ended without an exit status: treat like an uncertain transport failure.
    Interrupted,
    NotInstalled,
}

fn run(gcloud: &mut dyn Gcloud, args: &[String], stdin: Option<&[u8]>) -> Ran {
    match gcloud.run(args, stdin) {
        Invocation::NotInstalled => Ran::NotInstalled,
        Invocation::Exited {
            code: Some(0),
            stdout,
            ..
        } => Ran::Ok(stdout),
        Invocation::Exited { code: None, .. } => Ran::Interrupted,
        Invocation::Exited { stderr, .. } => Ran::Failed(gcloud::classify(&stderr)),
    }
}

const NOT_INSTALLED: &str =
    "gcloud is not installed or not on PATH; xtask never installs it or logs in";

/// The account must already hold a gcloud credential. A browser Console session is not one.
fn check_account(gcloud: &mut dyn Gcloud, account: &str, report: &mut Report) -> bool {
    let (status, detail) = match run(gcloud, &gcloud::auth_list(account), None) {
        Ran::NotInstalled => (Status::Unsupported, NOT_INSTALLED.to_owned()),
        Ran::Ok(stdout) if stdout.lines().any(|line| line.trim() == account) => {
            (Status::Pass, format!("{account} holds a gcloud credential"))
        }
        Ran::Ok(_) => (
            Status::Fail,
            format!("{account} has no gcloud credential; the operator runs `gcloud auth login {account}` themself"),
        ),
        Ran::Failed(failure) => (Status::Fail, failure.explain().to_owned()),
        Ran::Interrupted => (Status::Fail, "gcloud auth list was interrupted".to_owned()),
    };
    let passed = status == Status::Pass;
    report.record("account", status, detail);
    passed
}

pub struct Ports<'a> {
    pub gcloud: &'a mut dyn Gcloud,
    pub ledger: &'a mut dyn Ledger,
}

pub fn import(request: &Import<'_>, payload: &[u8], ports: Ports<'_>) -> Report {
    let mut report = Report::new(format!("secrets import {}", request.secret.id));
    let Ports { gcloud, ledger } = ports;
    let entry = |action, result, version| LedgerEntry {
        project: request.project.clone(),
        secret: request.secret.id.clone(),
        account: request.account.clone(),
        action,
        result,
        version,
    };
    let rest = ["secret", "fence", "add-version"];
    let skip = |report: &mut Report, from: usize| {
        for name in &rest[from..] {
            report.record(*name, Status::NotRun, "stopped by an earlier step");
        }
    };
    if !check_account(gcloud, &request.account, &mut report) {
        skip(&mut report, 0);
        return report;
    }
    let (id, project, account) = (&request.secret.id, &request.project, &request.account);

    // 1. The secret's metadata: reuse it, or create it (metadata only, no payload).
    match run(gcloud, &gcloud::describe(id, project, account), None) {
        Ran::Ok(stdout) if gcloud::names_secret(&stdout, id) => {
            report.record("secret", Status::Pass, "exists");
        }
        Ran::Ok(_) => {
            report.record(
                "secret",
                Status::Fail,
                "gcloud describe returned unexpected output",
            );
            skip(&mut report, 1);
            return report;
        }
        Ran::Failed(Failure::NotFound) => {
            match run(
                gcloud,
                &gcloud::create(request.secret, project, account),
                None,
            ) {
                Ran::Ok(_) => {
                    ledger.append(&entry("create", "created", None));
                    report.record(
                        "secret",
                        Status::Pass,
                        "created (automatic replication, no payload)",
                    );
                }
                Ran::Failed(Failure::AlreadyExists) => {
                    report.record(
                        "secret",
                        Status::Pass,
                        "exists (created concurrently or by an earlier uncertain attempt)",
                    );
                }
                Ran::Failed(failure) if failure.definite() => {
                    ledger.append(&entry("create", "rejected", None));
                    report.record(
                        "secret",
                        Status::Fail,
                        format!("create refused: {}", failure.explain()),
                    );
                    skip(&mut report, 1);
                    return report;
                }
                Ran::Failed(_) | Ran::Interrupted | Ran::NotInstalled => {
                    ledger.append(&entry("create", "uncertain", None));
                    report.record("secret", Status::Fail, "create outcome UNCERTAIN (metadata only, no payload sent); rerun --dry-run, then this command, which reuses an existing secret");
                    skip(&mut report, 1);
                    return report;
                }
            }
        }
        Ran::Failed(failure) => {
            report.record("secret", Status::Fail, failure.explain());
            skip(&mut report, 1);
            return report;
        }
        Ran::Interrupted | Ran::NotInstalled => {
            report.record("secret", Status::Fail, "gcloud describe was interrupted");
            skip(&mut report, 1);
            return report;
        }
    }

    // 2. The fence: the highest version ever issued must be the one the operator expects.
    let observed = match run(gcloud, &gcloud::versions_list(id, project, account), None) {
        Ran::Ok(stdout) => gcloud::parse_versions(&stdout, id).map(|v| gcloud::latest(&v)),
        Ran::Failed(failure) => {
            report.record(
                "fence",
                Status::Fail,
                format!("cannot list versions: {}", failure.explain()),
            );
            skip(&mut report, 2);
            return report;
        }
        Ran::Interrupted | Ran::NotInstalled => None,
    };
    let Some(observed) = observed else {
        report.record(
            "fence",
            Status::Fail,
            "version listing was unreadable; nothing was added",
        );
        skip(&mut report, 2);
        return report;
    };
    let observed = Expect::of(observed);
    if observed != request.expect {
        report.record(
            "fence",
            Status::Fail,
            format!(
                "the secret has {} but --expect-latest said {}; another or an earlier uncertain write may have landed. Reconcile with `cargo xtask secrets status` before adding a version",
                observed.describe(),
                request.expect.describe()
            ),
        );
        skip(&mut report, 2);
        return report;
    }
    report.record(
        "fence",
        Status::Pass,
        format!("latest is {} as expected", observed.describe()),
    );

    // 3. Add exactly one immutable version. Never retried here.
    match run(
        gcloud,
        &gcloud::add_version(id, project, account),
        Some(payload),
    ) {
        Ran::Ok(stdout) => match gcloud::parse_added(&stdout, id) {
            Some(version) => {
                ledger.append(&entry("add-version", "added", Some(version)));
                report.record(
                    "add-version",
                    Status::Pass,
                    format!("added v{version} (IMPORTED)"),
                );
                let next = match request.expect {
                    Expect::NoVersion => 1,
                    Expect::Latest(v) => v + 1,
                };
                if version != next {
                    report.note(format!("expected v{next} but Secret Manager issued v{version}: another writer was active; review the version list"));
                }
            }
            None => {
                ledger.append(&entry("add-version", "uncertain", None));
                report.record("add-version", Status::Fail, uncertain(request.expect));
            }
        },
        Ran::Failed(failure) if failure.definite() => {
            ledger.append(&entry("add-version", "rejected", None));
            report.record(
                "add-version",
                Status::Fail,
                format!("refused: {}", failure.explain()),
            );
        }
        Ran::Failed(_) | Ran::Interrupted | Ran::NotInstalled => {
            ledger.append(&entry("add-version", "uncertain", None));
            report.record("add-version", Status::Fail, uncertain(request.expect));
        }
    }
    report.note("IMPORTED is not VERIFIED_ACCESS (the runtime reader's own read) or RUNTIME_BOUND (a reviewed numeric pin installed on the host); both remain NOT_RUN here");
    if request.secret.rotation_required {
        report.note("ROTATION_REQUIRED: this value left its protected store; rotate it at its provider and import the replacement");
    }
    report
}

fn uncertain(expect: Expect) -> String {
    format!(
        "outcome UNCERTAIN: do not retry blindly. Run `cargo xtask secrets status` and compare the latest version with {}; if a new version exists it is this upload",
        expect.describe()
    )
}

/// Read-only: which inventoried secrets exist, their latest version and the pin's state.
pub fn status(
    inventory: &Inventory,
    secrets: &[&Secret],
    account: &str,
    gcloud: &mut dyn Gcloud,
) -> Report {
    let mut report = Report::new(format!("secrets status {}", inventory.project));
    if !check_account(gcloud, account, &mut report) {
        return report;
    }
    for secret in secrets {
        let args = gcloud::versions_list(&secret.id, &inventory.project, account);
        let (status, detail) = match run(gcloud, &args, None) {
            Ran::Ok(stdout) => match gcloud::parse_versions(&stdout, &secret.id) {
                Some(versions) => observe(secret, &versions),
                None => (Status::Fail, "unreadable version listing".to_owned()),
            },
            Ran::Failed(Failure::NotFound) => (Status::Pass, missing(secret)),
            Ran::Failed(failure) => (Status::Fail, failure.explain().to_owned()),
            Ran::Interrupted | Ran::NotInstalled => (Status::Fail, "interrupted".to_owned()),
        };
        report.record(secret.id.clone(), status, detail);
    }
    report.note("metadata only: no payload was accessed. VERIFIED_ACCESS and RUNTIME_BOUND need the consumer's own identity and host; NOT_RUN here");
    report
}

fn missing(secret: &Secret) -> String {
    match secret.pinned_version {
        Some(pin) => format!("MISSING, yet pinned to v{pin}"),
        None => "MISSING".to_owned(),
    }
}

fn observe(secret: &Secret, versions: &[gcloud::Version]) -> (Status, String) {
    let Some(latest) = gcloud::latest(versions) else {
        return (
            Status::Pass,
            "MISSING (secret exists without versions)".to_owned(),
        );
    };
    let enabled = versions.iter().filter(|v| v.state == "ENABLED").count();
    let mut detail = format!("IMPORTED latest v{latest}, {enabled} enabled");
    if secret.rotation_required {
        detail.push_str(", ROTATION_REQUIRED");
    }
    match secret.pinned_version {
        None => detail.push_str(", unpinned"),
        Some(pin) => match versions.iter().find(|v| v.number == pin) {
            Some(v) if v.state == "ENABLED" => detail.push_str(&format!(", pinned v{pin} ENABLED")),
            Some(v) => {
                return (
                    Status::Fail,
                    format!("{detail}, pinned v{pin} is {}", v.state),
                );
            }
            None => {
                return (
                    Status::Fail,
                    format!("{detail}, pinned v{pin} does not exist"),
                )
            }
        },
    }
    (Status::Pass, detail)
}

#[cfg(test)]
pub mod testing {
    use super::*;

    /// A scripted gcloud: answers in order and records argv plus whether stdin carried bytes.
    #[derive(Default)]
    pub struct Scripted {
        pub answers: Vec<Invocation>,
        pub calls: Vec<(Vec<String>, Option<Vec<u8>>)>,
    }

    impl Scripted {
        pub fn ok(mut self, stdout: &str) -> Self {
            self.answers.push(Invocation::Exited {
                code: Some(0),
                stdout: stdout.into(),
                stderr: String::new(),
            });
            self
        }
        pub fn fail(mut self, stderr: &str) -> Self {
            self.answers.push(Invocation::Exited {
                code: Some(1),
                stdout: String::new(),
                stderr: stderr.into(),
            });
            self
        }
        pub fn killed(mut self) -> Self {
            self.answers.push(Invocation::Exited {
                code: None,
                stdout: String::new(),
                stderr: String::new(),
            });
            self
        }
    }

    impl Gcloud for Scripted {
        fn run(&mut self, args: &[String], stdin: Option<&[u8]>) -> Invocation {
            self.calls.push((args.to_vec(), stdin.map(<[u8]>::to_vec)));
            assert!(!self.answers.is_empty(), "unexpected gcloud call {args:?}");
            self.answers.remove(0)
        }
    }

    #[derive(Default)]
    pub struct Recorded(pub Vec<LedgerEntry>);

    impl Ledger for Recorded {
        fn append(&mut self, entry: &LedgerEntry) {
            self.0.push(entry.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{testing::*, *};
    use crate::secrets::{inventory, request};

    const ACCOUNT: &str = "operator@example.test";
    const ID: &str = "cantos-dev-identity-smtp-password";
    const SYNTHETIC: &[u8] = b"synthetic-not-a-real-password";

    fn inventory() -> Inventory {
        let root = env!("CARGO_MANIFEST_DIR");
        let text =
            std::fs::read_to_string(format!("{root}/../{}", inventory::INVENTORY_FILE)).unwrap();
        inventory::parse(&text).unwrap()
    }

    fn import_request<'a>(inventory: &'a Inventory, expect: &str) -> Import<'a> {
        request::resolve(
            &request::RawImport {
                project: "cantos-a348b".into(),
                account: ACCOUNT.into(),
                secret: ID.into(),
                file: None,
                stdin: true,
                expect_latest: expect.into(),
                dry_run: false,
            },
            inventory,
        )
        .unwrap()
    }

    fn secret_json() -> String {
        format!(r#"{{"name":"projects/123/secrets/{ID}"}}"#)
    }

    fn versions(numbers: &[u64]) -> String {
        let items: Vec<String> = numbers
            .iter()
            .map(|n| {
                format!(r#"{{"name":"projects/123/secrets/{ID}/versions/{n}","state":"ENABLED"}}"#)
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    fn added(n: u64) -> String {
        format!(r#"{{"name":"projects/123/secrets/{ID}/versions/{n}","state":"ENABLED"}}"#)
    }

    fn statuses(report: &Report) -> Vec<(String, Status)> {
        report
            .outcomes
            .iter()
            .map(|o| (o.name.clone(), o.status))
            .collect()
    }

    fn printed(report: &Report) -> String {
        format!("{report:?}")
    }

    #[test]
    fn a_missing_secret_is_created_then_receives_its_first_version_over_stdin() {
        let inventory = inventory();
        let request = import_request(&inventory, "none");
        let mut gcloud = Scripted::default()
            .ok(&format!("{ACCOUNT}\n"))
            .fail("ERROR: NOT_FOUND: Secret not found")
            .ok(&secret_json())
            .ok("[]")
            .ok(&added(1));
        let mut ledger = Recorded::default();
        let report = import(
            &request,
            SYNTHETIC,
            Ports {
                gcloud: &mut gcloud,
                ledger: &mut ledger,
            },
        );
        assert!(report.succeeded(), "{report:?}");
        let verbs: Vec<&str> = gcloud.calls.iter().map(|(a, _)| a[1].as_str()).collect();
        assert_eq!(
            verbs,
            ["list", "describe", "create", "versions", "versions"]
        );
        // Only the add-version call carries bytes; they are never in argv.
        for (args, stdin) in &gcloud.calls[..4] {
            assert!(stdin.is_none(), "{args:?}");
        }
        assert_eq!(gcloud.calls[4].1.as_deref(), Some(SYNTHETIC));
        for (args, _) in &gcloud.calls {
            assert!(
                !args.iter().any(|a| a.contains("synthetic-not")),
                "{args:?}"
            );
        }
        assert_eq!(
            ledger
                .0
                .iter()
                .map(|e| (e.action, e.result, e.version))
                .collect::<Vec<_>>(),
            [
                ("create", "created", None),
                ("add-version", "added", Some(1))
            ]
        );
        assert!(!printed(&report).contains("synthetic-not"));
    }

    #[test]
    fn an_existing_secret_gets_a_new_version_when_the_fence_matches() {
        let inventory = inventory();
        let request = import_request(&inventory, "2");
        let mut gcloud = Scripted::default()
            .ok(ACCOUNT)
            .ok(&secret_json())
            .ok(&versions(&[2, 1]))
            .ok(&added(3));
        let mut ledger = Recorded::default();
        let report = import(
            &request,
            SYNTHETIC,
            Ports {
                gcloud: &mut gcloud,
                ledger: &mut ledger,
            },
        );
        assert!(report.succeeded(), "{report:?}");
        assert!(report.notes.iter().any(|n| n.contains("VERIFIED_ACCESS")));
        assert_eq!(ledger.0.len(), 1);
    }

    #[test]
    fn a_fence_mismatch_refuses_without_writing() {
        let inventory = inventory();
        let request = import_request(&inventory, "none");
        let mut gcloud = Scripted::default()
            .ok(ACCOUNT)
            .ok(&secret_json())
            .ok(&versions(&[1]));
        let mut ledger = Recorded::default();
        let report = import(
            &request,
            SYNTHETIC,
            Ports {
                gcloud: &mut gcloud,
                ledger: &mut ledger,
            },
        );
        assert_eq!(
            statuses(&report),
            [
                ("account".into(), Status::Pass),
                ("secret".into(), Status::Pass),
                ("fence".into(), Status::Fail),
                ("add-version".into(), Status::NotRun),
            ]
        );
        assert!(gcloud.calls.iter().all(|(_, stdin)| stdin.is_none()));
        assert!(ledger.0.is_empty());
    }

    #[test]
    fn missing_iam_or_api_fails_closed_before_any_payload_is_sent() {
        let inventory = inventory();
        for stderr in [
            "PERMISSION_DENIED: Permission 'secretmanager.secrets.get' denied (or it may not exist)",
            "PERMISSION_DENIED: API has not been used in project 1. reason: SERVICE_DISABLED",
        ] {
            let request = import_request(&inventory, "none");
            let mut gcloud = Scripted::default().ok(ACCOUNT).fail(stderr);
            let mut ledger = Recorded::default();
            let report = import(&request, SYNTHETIC, Ports { gcloud: &mut gcloud, ledger: &mut ledger });
            assert!(!report.succeeded());
            assert_eq!(gcloud.calls.len(), 2, "no create after a denial");
            assert!(!printed(&report).contains("secretmanager.secrets.get"), "stderr is withheld");
        }
        // Denied on add: definite, recorded as rejected.
        let request = import_request(&inventory, "none");
        let mut gcloud = Scripted::default()
            .ok(ACCOUNT)
            .ok(&secret_json())
            .ok("[]")
            .fail("PERMISSION_DENIED: Permission 'secretmanager.versions.add' denied");
        let mut ledger = Recorded::default();
        let report = import(
            &request,
            SYNTHETIC,
            Ports {
                gcloud: &mut gcloud,
                ledger: &mut ledger,
            },
        );
        assert!(!report.succeeded());
        assert_eq!(ledger.0[0].result, "rejected");
    }

    #[test]
    fn an_uncertain_add_is_reported_once_and_never_retried() {
        let inventory = inventory();
        for gcloud in [
            Scripted::default()
                .ok(ACCOUNT)
                .ok(&secret_json())
                .ok("[]")
                .fail("ConnectionError: timed out"),
            Scripted::default()
                .ok(ACCOUNT)
                .ok(&secret_json())
                .ok("[]")
                .killed(),
            Scripted::default()
                .ok(ACCOUNT)
                .ok(&secret_json())
                .ok("[]")
                .ok("garbled"),
        ] {
            let mut gcloud = gcloud;
            let request = import_request(&inventory, "none");
            let mut ledger = Recorded::default();
            let report = import(
                &request,
                SYNTHETIC,
                Ports {
                    gcloud: &mut gcloud,
                    ledger: &mut ledger,
                },
            );
            assert!(!report.succeeded());
            assert_eq!(gcloud.calls.len(), 4, "exactly one add attempt");
            assert_eq!(ledger.0[0].result, "uncertain");
            assert!(printed(&report).contains("UNCERTAIN"));
        }
    }

    #[test]
    fn an_uncredentialed_account_or_missing_gcloud_stops_everything() {
        let inventory = inventory();
        let request = import_request(&inventory, "none");
        let mut gcloud = Scripted::default().ok("someone-else@example.test\n");
        let mut ledger = Recorded::default();
        let report = import(
            &request,
            SYNTHETIC,
            Ports {
                gcloud: &mut gcloud,
                ledger: &mut ledger,
            },
        );
        assert_eq!(gcloud.calls.len(), 1);
        assert!(!report.succeeded());
        let mut gcloud = Scripted {
            answers: vec![Invocation::NotInstalled],
            ..Default::default()
        };
        let report = import(
            &request,
            SYNTHETIC,
            Ports {
                gcloud: &mut gcloud,
                ledger: &mut ledger,
            },
        );
        assert_eq!(report.outcomes[0].status, Status::Unsupported);
        assert!(ledger.0.is_empty());
    }

    #[test]
    fn status_reports_missing_imported_and_bad_pins_without_payload_access() {
        let mut inventory = inventory();
        let ids = [
            "cantos-prod-server-playback-key",
            "cantos-prod-identity-smtp-password",
            "cantos-ops-cloudflare-api-token",
        ];
        inventory.secrets.retain(|s| ids.contains(&s.id.as_str()));
        inventory
            .secrets
            .iter_mut()
            .find(|s| s.id == ids[2])
            .unwrap()
            .pinned_version = Some(4);
        let selected: Vec<&Secret> = inventory.secrets.iter().collect();
        let listing = |id: &str, states: &[(u64, &str)]| {
            let items: Vec<String> = states
                .iter()
                .map(|(n, s)| {
                    format!(r#"{{"name":"projects/1/secrets/{id}/versions/{n}","state":"{s}"}}"#)
                })
                .collect();
            format!("[{}]", items.join(","))
        };
        // Inventory order: identity smtp, playback key, cloudflare token.
        let mut gcloud = Scripted::default()
            .ok(ACCOUNT)
            .fail("NOT_FOUND")
            .ok(&listing(ids[0], &[(1, "ENABLED")]))
            .ok(&listing(ids[2], &[(4, "DISABLED"), (5, "ENABLED")]));
        let report = status(&inventory, &selected, ACCOUNT, &mut gcloud);
        let details: Vec<(Status, &str)> = report
            .outcomes
            .iter()
            .map(|o| (o.status, o.detail.as_str()))
            .collect();
        assert_eq!(details[1], (Status::Pass, "MISSING"));
        assert_eq!(
            details[2],
            (Status::Pass, "IMPORTED latest v1, 1 enabled, unpinned")
        );
        assert_eq!(details[3].0, Status::Fail);
        assert!(details[3].1.contains("pinned v4 is DISABLED"));
        assert!(gcloud
            .calls
            .iter()
            .all(|(args, stdin)| stdin.is_none() && !args.contains(&"access".to_owned())));
    }
}
