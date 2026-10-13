//! Operator plan and explicit read-only metadata check. No grants/secret/bootstrap mutation.
use crate::{
    command::{CommandSpec, Runner},
    workspace::Workspace,
};
use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use std::process::ExitCode;

const CLIENT: &str = "cantos-studio-web-bff";
#[derive(Debug, Args)]
pub struct KanidmArgs {
    #[command(subcommand)]
    pub action: Action,
}
#[derive(Debug, Subcommand)]
pub enum Action {
    /// Print an operator reconciliation plan, without credentials, network or mutation.
    Reconcile {
        #[arg(long)]
        issuer: String,
        #[arg(long)]
        web_origin: String,
        #[arg(long)]
        group: String,
        #[arg(long, required = true)]
        dry_run: bool,
    },
    /// Read discovery/JWKS only. Does not authenticate users or establish production readiness.
    Verify {
        #[arg(long)]
        issuer: String,
        #[arg(long)]
        web_origin: String,
        #[arg(long)]
        dry_run: bool,
    },
}

fn validate(issuer: &str, web: &str) -> Result<()> {
    let provider = url::Url::parse(issuer).map_err(|_| anyhow::anyhow!("invalid issuer URL"))?;
    let origin = url::Url::parse(web).map_err(|_| anyhow::anyhow!("invalid web origin"))?;
    for url in [&provider, &origin] {
        let local = matches!(url.host_str(), Some("127.0.0.1" | "[::1]"));
        if (url.scheme() != "https" && !(url.scheme() == "http" && local))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.host_str().is_none()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("explicit HTTPS or literal loopback URLs without credentials, query or fragment required");
        }
    }
    if provider.path() != format!("/oauth2/openid/{CLIENT}")
        || provider.as_str() != issuer
        || origin.origin().ascii_serialization() != web
    {
        bail!("exact Studio issuer/client path and canonical web origin required");
    }
    Ok(())
}

fn plan(web: &str, group: &str) -> Result<Vec<Vec<String>>> {
    if group.is_empty()
        || group.len() > 64
        || !group
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        bail!("explicit operator group must be a bounded identifier");
    }
    let command = |args: &[&str]| args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    Ok(vec![
        command(&[
            "kanidm",
            "system",
            "oauth2",
            "create",
            CLIENT,
            "Cantos Studio",
            web,
        ]),
        command(&[
            "kanidm",
            "system",
            "oauth2",
            "add-redirect-url",
            CLIENT,
            &format!("{web}/api/v1/studio/auth/callback"),
        ]),
        command(&[
            "kanidm",
            "system",
            "oauth2",
            "update-scope-map",
            CLIENT,
            group,
            "openid",
        ]),
    ])
}
pub fn run(workspace: &Workspace, args: &KanidmArgs, runner: &mut impl Runner) -> Result<ExitCode> {
    match &args.action {
        Action::Reconcile {
            issuer,
            web_origin,
            group,
            dry_run,
        } => {
            validate(issuer, web_origin)?;
            if !dry_run {
                bail!("reconciliation is plan-only; live mutation requires operator authorization");
            }
            for command in plan(web_origin, group)? {
                println!("{command:?}");
            }
            println!("NOT_RUN Kanidm reconciliation: operator plan only; inspect existing exact callbacks, PKCE enforcement, ES256 and group grants before any mutation");
            Ok(ExitCode::SUCCESS)
        }
        Action::Verify {
            issuer,
            web_origin,
            dry_run,
        } => {
            validate(issuer, web_origin)?;
            let command = CommandSpec::new("cargo", workspace.root()).args([
                "run",
                "--locked",
                "--package",
                "cantos-api",
                "--bin",
                "cantos-oidc-metadata",
                "--",
                issuer,
                web_origin,
            ]);
            if *dry_run {
                println!("NOT_RUN Kanidm metadata verification: {command:?}");
                return Ok(ExitCode::SUCCESS);
            }
            runner.run(&command)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dry_runs_execute_nothing_and_verify_has_one_read_only_primitive() {
        use crate::command::testing::RecordingRunner;
        let workspace = Workspace::discover().unwrap();
        let issuer = "https://idm.example/oauth2/openid/cantos-studio-web-bff";
        let mut runner = RecordingRunner::default();
        for action in [
            Action::Reconcile {
                issuer: issuer.into(),
                web_origin: "https://studio.example".into(),
                group: "cantos_creators".into(),
                dry_run: true,
            },
            Action::Verify {
                issuer: issuer.into(),
                web_origin: "https://studio.example".into(),
                dry_run: true,
            },
        ] {
            assert_eq!(
                run(&workspace, &KanidmArgs { action }, &mut runner).unwrap(),
                ExitCode::SUCCESS
            );
        }
        assert!(runner.commands.is_empty());
        let action = Action::Verify {
            issuer: issuer.into(),
            web_origin: "https://studio.example".into(),
            dry_run: false,
        };
        assert_eq!(
            run(&workspace, &KanidmArgs { action }, &mut runner).unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(
            runner.commands,
            vec![CommandSpec::new("cargo", workspace.root()).args([
                "run",
                "--locked",
                "--package",
                "cantos-api",
                "--bin",
                "cantos-oidc-metadata",
                "--",
                issuer,
                "https://studio.example"
            ])]
        );
        assert!(validate(
            "https://user:secret@idm.example/oauth2/openid/cantos-studio-web-bff",
            "https://studio.example"
        )
        .is_err());
        assert!(validate(issuer, "https://studio.example/unexpected-path").is_err());
    }
    #[test]
    fn reconciliation_plan_is_studio_only_exact_callback_and_openid() {
        let commands = plan("https://studio.example", "cantos_creators").unwrap();
        assert_eq!(
            commands[1],
            vec![
                "kanidm",
                "system",
                "oauth2",
                "add-redirect-url",
                CLIENT,
                "https://studio.example/api/v1/studio/auth/callback"
            ]
        );
        assert_eq!(commands[2].last().unwrap(), "openid");
        assert!(plan("https://studio.example", "x; injected").is_err());
        assert!(validate(
            "http://remote/oauth2/openid/cantos-studio-web-bff",
            "https://studio.example"
        )
        .is_err());
    }
}
