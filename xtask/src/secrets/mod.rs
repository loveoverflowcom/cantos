//! Explicit, operator-only Google Cloud Secret Manager commands (issue #446).
//!
//! `inventory` validates the tracked nonsecret mapping offline. `status` reads version metadata.
//! `import` creates a listed secret if absent and adds one immutable version from an approved
//! owner-only file or piped stdin. The runbook is docs/deployment/google-secret-manager.md.
//!
//! The pieces are separated by what they may touch:
//! - [`inventory`] parses and validates the tracked mapping (pure);
//! - [`request`] resolves flags and checks payload metadata and shape (pure);
//! - [`gcloud`] builds argv and interprets gcloud's answers (pure);
//! - [`host`] reads the one approved input, runs gcloud and appends the ignored ledger;
//! - [`flow`] orders the remote steps and stops at the first failure.
//!
//! # Acceptance status
//!
//! Tested against a scripted gcloud with synthetic payloads only. Not accepted, each
//! `ACCEPTANCE(NOT_RUN)`: a real create/add/list against project `cantos-a348b` (no authorized
//! gcloud credential was available), gcloud's exact stderr wording across releases, IAM grants,
//! CI workload identity and any runtime/VPS binding.
//!
//! Never invoked by build, run, verify, preflight or ci. There is no HTTP endpoint, no bulk or
//! environment import, no generated value and no payload read path.

mod flow;
mod gcloud;
mod host;
mod inventory;
mod request;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{build::Refusal, workspace::Workspace};
use inventory::{Environment, Inventory, INVENTORY_FILE};

const SECRETS_HELP: &str = "\
Operator-only Secret Manager commands for the project named in infra/google-secret-manager/inventory.toml.

  inventory   validate and print the tracked names, consumers and pins (offline)
  status      list version metadata of inventoried secrets (read-only; never reads a payload)
  import      create a listed secret if absent, then add ONE immutable version (a REMOTE WRITE)

Examples:
  cargo xtask secrets inventory
  cargo xtask secrets status --project cantos-a348b --account operator@example.com --environment prod
  # Review first: no gcloud, no network, and the input file is stat'ed but never opened.
  cargo xtask secrets import --project cantos-a348b --account operator@example.com \\
    --secret cantos-prod-server-playback-key --file /home/operator/private/playback.key \\
    --expect-latest none --dry-run

xtask never logs in, enables APIs, changes IAM or billing, generates a value, reads a payload
back or touches a host. The account must already hold a gcloud credential
(`gcloud auth login` by the operator); a browser Console session is not one.
See docs/deployment/google-secret-manager.md.";

#[derive(Debug, Args)]
#[command(after_long_help = SECRETS_HELP)]
pub struct SecretsArgs {
    #[command(subcommand)]
    pub action: SecretsAction,
}

#[derive(Debug, Subcommand)]
pub enum SecretsAction {
    /// Validate the tracked inventory and print names only (offline).
    Inventory,
    /// Read version metadata for inventoried secrets (read-only).
    Status(StatusArgs),
    /// Create a listed secret if absent and add one version (a remote write).
    Import(ImportArgs),
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Google Cloud project ID; must equal the inventory's project.
    #[arg(long, value_name = "PROJECT_ID")]
    pub project: String,
    /// gcloud account to act as (required; the active configuration is never used).
    #[arg(long, value_name = "EMAIL")]
    pub account: String,
    /// Restrict to one environment: dev, prod or ops.
    #[arg(long, value_name = "ENVIRONMENT")]
    pub environment: Option<String>,
    /// Print the read-only plan; starts no tool.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Google Cloud project ID; must equal the inventory's project.
    #[arg(long, value_name = "PROJECT_ID")]
    pub project: String,
    /// gcloud account to act as (required; the active configuration is never used).
    #[arg(long, value_name = "EMAIL")]
    pub account: String,
    /// Secret ID; must be listed in the inventory.
    #[arg(long, value_name = "SECRET_ID")]
    pub secret: String,
    /// Absolute path of an owner-only (0600/0400) file outside the checkout.
    #[arg(long, value_name = "PATH")]
    pub file: Option<String>,
    /// Read the value from piped stdin (refuses an interactive terminal).
    #[arg(long)]
    pub stdin: bool,
    /// `none` or the latest version number you observed; a mismatch refuses the write.
    #[arg(long, value_name = "none|N")]
    pub expect_latest: String,
    /// Print the plan only: no gcloud, no network, no ledger; the file is stat'ed, not opened.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(workspace: &Workspace, args: &SecretsArgs) -> Result<ExitCode> {
    let inventory = match std::fs::read_to_string(workspace.path(INVENTORY_FILE))
        .map_err(|_| format!("{INVENTORY_FILE} cannot be read"))
        .and_then(|text| inventory::parse(&text).map_err(|e| e.to_string()))
    {
        Ok(inventory) => inventory,
        Err(why) => return Ok(crate::refuse(Refusal::Usage(why))),
    };
    match &args.action {
        SecretsAction::Inventory => {
            print!("{}", inventory.render());
            Ok(ExitCode::SUCCESS)
        }
        SecretsAction::Status(status) => run_status(&inventory, status, &mut host::SystemGcloud),
        SecretsAction::Import(import) => {
            run_import(workspace, &inventory, import, &mut host::SystemGcloud)
        }
    }
}

fn select<'a>(
    inventory: &'a Inventory,
    environment: Option<&str>,
) -> Result<Vec<&'a inventory::Secret>, String> {
    let wanted = match environment {
        None => None,
        Some("dev") => Some(Environment::Dev),
        Some("prod") => Some(Environment::Prod),
        Some("ops") => Some(Environment::Ops),
        Some(other) => return Err(format!("--environment {other:?} must be dev, prod or ops")),
    };
    Ok(inventory
        .secrets
        .iter()
        .filter(|s| wanted.is_none_or(|w| s.environment == w))
        .collect())
}

fn run_status(
    inventory: &Inventory,
    args: &StatusArgs,
    gcloud: &mut dyn flow::Gcloud,
) -> Result<ExitCode> {
    let checked = request::check_project(&args.project, inventory)
        .and_then(|()| {
            if request::valid_account(&args.account) {
                Ok(())
            } else {
                Err("--account must be the explicit gcloud account email to act as".into())
            }
        })
        .and_then(|()| select(inventory, args.environment.as_deref()));
    let secrets = match checked {
        Ok(secrets) => secrets,
        Err(why) => return Ok(crate::refuse(Refusal::Usage(why))),
    };
    if args.dry_run {
        println!(
            "plan: read-only version metadata for {} secret(s); no payload access",
            secrets.len()
        );
        println!("  {}", gcloud::display(&gcloud::auth_list(&args.account)));
        for secret in &secrets {
            println!(
                "  {}",
                gcloud::display(&gcloud::versions_list(
                    &secret.id,
                    &inventory.project,
                    &args.account
                ))
            );
        }
        return Ok(ExitCode::SUCCESS);
    }
    let report = flow::status(inventory, &secrets, &args.account, gcloud);
    report.print();
    Ok(report.exit_code())
}

fn run_import(
    workspace: &Workspace,
    inventory: &Inventory,
    args: &ImportArgs,
    gcloud: &mut dyn flow::Gcloud,
) -> Result<ExitCode> {
    let raw = request::RawImport {
        project: args.project.clone(),
        account: args.account.clone(),
        secret: args.secret.clone(),
        file: args.file.clone(),
        stdin: args.stdin,
        expect_latest: args.expect_latest.clone(),
        dry_run: args.dry_run,
    };
    let request = match request::resolve(&raw, inventory) {
        Ok(request) => request,
        Err(why) => return Ok(crate::refuse(Refusal::Usage(why))),
    };
    if request.dry_run {
        if let request::Source::File(path) = &request.source {
            let checked = host::file_facts(path, workspace.root())
                .and_then(|facts| request::check_file(facts, request.secret.format));
            if let Err(why) = checked {
                return Ok(crate::refuse(Refusal::Usage(why.into())));
            }
        }
        print!("{}", plan_text(&request));
        return Ok(ExitCode::SUCCESS);
    }
    let payload = match host::read_payload(&request.source, request.secret.format, workspace.root())
    {
        Ok(payload) => payload,
        Err(why) => return Ok(crate::refuse(Refusal::Usage(why))),
    };
    let mut ledger = host::FileLedger::new(workspace.root());
    let mut report = flow::import(
        &request,
        &payload,
        flow::Ports {
            gcloud,
            ledger: &mut ledger,
        },
    );
    drop(payload);
    if ledger.failed {
        report.note(format!(
            "the local ledger {} could not be written; record the outcome above by hand",
            host::LEDGER_FILE
        ));
    }
    report.print();
    Ok(report.exit_code())
}

/// The reviewable plan: names, scope and every remote effect, never the input's bytes or size.
fn plan_text(request: &request::Import<'_>) -> String {
    let secret = request.secret;
    let (id, project, account) = (&secret.id, &request.project, &request.account);
    let input = match &request.source {
        request::Source::File(_) => {
            "an owner-only file outside the checkout (metadata checked, not opened)"
        }
        request::Source::Stdin => "piped stdin (not read during a dry run)",
    };
    let mut out = format!(
        "plan: import into {id} ({} / {} / {}:{})\n  project   {project}\n  account   {account}\n  input     {input}\n  format    {}\n  fence     latest must be {}\n",
        secret.environment.as_str(),
        secret.consumer.as_str(),
        match secret.delivery {
            inventory::Delivery::Env => "env",
            inventory::Delivery::File => "file",
        },
        secret.contract,
        secret.format.as_str(),
        request.expect.describe(),
    );
    for (label, args, effect) in [
        ("account", gcloud::auth_list(account), "local, read-only"),
        ("get", gcloud::describe(id, project, account), "read-only"),
        (
            "create",
            gcloud::create(secret, project, account),
            "REMOTE WRITE, only if absent, no payload",
        ),
        (
            "fence",
            gcloud::versions_list(id, project, account),
            "read-only",
        ),
        (
            "add",
            gcloud::add_version(id, project, account),
            "REMOTE WRITE, payload on stdin",
        ),
    ] {
        out.push_str(&format!(
            "  {label:<8} [{effect}] {}\n",
            gcloud::display(&args)
        ));
    }
    out.push_str("  ledger    artifacts/secrets/import-ledger.ndjson (ignored; names, version and result only)\n");
    out.push_str(
        "Not done by this command: IAM grants, runtime binding, pin updates, deploy or restart.\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use flow::testing::Scripted;

    fn inventory() -> Inventory {
        let workspace = Workspace::discover().unwrap();
        inventory::parse(&std::fs::read_to_string(workspace.path(INVENTORY_FILE)).unwrap()).unwrap()
    }

    fn import_args(file: Option<String>, dry_run: bool) -> ImportArgs {
        ImportArgs {
            project: "cantos-a348b".into(),
            account: "operator@example.test".into(),
            secret: "cantos-prod-server-playback-key".into(),
            file,
            stdin: false,
            expect_latest: "none".into(),
            dry_run,
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_dry_run_starts_no_gcloud_writes_no_ledger_and_never_prints_the_input() {
        use std::os::unix::fs::PermissionsExt;
        let workspace = Workspace::discover().unwrap();
        let inventory = inventory();
        let dir = std::env::temp_dir().join(format!("cantos-secrets-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("synthetic-playback.key");
        std::fs::write(&path, [3u8; 32]).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let ledger_before = workspace.path(host::LEDGER_FILE).exists();
        let mut gcloud = Scripted::default();
        let code = run_import(
            &workspace,
            &inventory,
            &import_args(Some(path.display().to_string()), true),
            &mut gcloud,
        )
        .unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(gcloud.calls.is_empty());
        assert_eq!(workspace.path(host::LEDGER_FILE).exists(), ledger_before);

        let request = request::resolve(
            &request::RawImport {
                project: "cantos-a348b".into(),
                account: "operator@example.test".into(),
                secret: "cantos-prod-server-playback-key".into(),
                file: Some(path.display().to_string()),
                stdin: false,
                expect_latest: "none".into(),
                dry_run: true,
            },
            &inventory,
        )
        .unwrap();
        let plan = plan_text(&request);
        assert!(plan.contains("--data-file=-"), "{plan}");
        assert!(plan.contains("REMOTE WRITE, only if absent"), "{plan}");
        assert!(
            !plan.contains("synthetic-playback.key"),
            "the private path stays out of the plan"
        );

        // A world-readable input is refused even in a dry run.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let code = run_import(
            &workspace,
            &inventory,
            &import_args(Some(path.display().to_string()), true),
            &mut gcloud,
        )
        .unwrap();
        assert_ne!(code, ExitCode::SUCCESS);
        assert!(gcloud.calls.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_wrong_project_is_refused_before_any_tool_for_every_command() {
        let workspace = Workspace::discover().unwrap();
        let inventory = inventory();
        let mut gcloud = Scripted::default();
        let mut args = import_args(Some("/nonexistent/x".into()), false);
        args.project = "cantos-other-1".into();
        assert_ne!(
            run_import(&workspace, &inventory, &args, &mut gcloud).unwrap(),
            ExitCode::SUCCESS
        );
        let status = StatusArgs {
            project: "cantos-other-1".into(),
            account: "operator@example.test".into(),
            environment: None,
            dry_run: false,
        };
        assert_ne!(
            run_status(&inventory, &status, &mut gcloud).unwrap(),
            ExitCode::SUCCESS
        );
        let bad_env = StatusArgs {
            project: "cantos-a348b".into(),
            environment: Some("staging".into()),
            ..status
        };
        assert_ne!(
            run_status(&inventory, &bad_env, &mut gcloud).unwrap(),
            ExitCode::SUCCESS
        );
        assert!(gcloud.calls.is_empty());
    }

    #[test]
    fn a_real_import_with_an_unreadable_input_refuses_before_gcloud() {
        let workspace = Workspace::discover().unwrap();
        let inventory = inventory();
        let mut gcloud = Scripted::default();
        let args = import_args(Some("/nonexistent/cantos-synthetic".into()), false);
        assert_ne!(
            run_import(&workspace, &inventory, &args, &mut gcloud).unwrap(),
            ExitCode::SUCCESS
        );
        assert!(gcloud.calls.is_empty());
    }

    #[test]
    fn environment_selection_partitions_the_inventory() {
        let inventory = inventory();
        let all = select(&inventory, None).unwrap().len();
        let parts: usize = ["dev", "prod", "ops"]
            .iter()
            .map(|e| select(&inventory, Some(e)).unwrap().len())
            .sum();
        assert_eq!(all, parts);
        assert!(select(&inventory, Some("prod"))
            .unwrap()
            .iter()
            .all(|s| s.id.starts_with("cantos-prod-")));
    }
}
