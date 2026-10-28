//! Orchestration only: Server owns the versioned profile plan, schema, HTTP and oracles.
use super::{sample, SeedOptions};
use crate::{
    build::Refusal,
    command::{CommandSpec, Runner},
    report::{Report, Status},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};
use anyhow::{anyhow, ensure, Result};
use clap::{Args, ValueEnum};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, process::ExitCode};

const REGISTRY: &str =
    include_str!("../../../services/cantos-server/tests/fixtures/local-datasets/registry.json");
const DATASET: &str = "cantos-local-datasets-v1";
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Action {
    List,
    Plan,
    Create,
    Verify,
    Serve,
    Archive,
}
impl Action {
    fn name(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Plan => "plan",
            Self::Create => "create",
            Self::Verify => "verify",
            Self::Serve => "serve",
            Self::Archive => "archive-check",
        }
    }
}
#[derive(Args, Debug, Default, Clone, PartialEq, Eq)]
pub struct Options {
    /// Dataset lifecycle operation; defaults to list. Only for `seed dataset`.
    #[arg(long, value_enum)]
    pub action: Option<Action>,
    /// Name printed by `seed dataset --action list`.
    #[arg(long)]
    pub profile: Option<String>,
    /// Dedicated absolute output directory outside Git; never an existing user database.
    #[arg(long, value_name = "DIR")]
    pub dataset_root: Option<PathBuf>,
    /// Deterministic namespace seed (default 568); never a source of credentials.
    #[arg(long)]
    pub seed: Option<u64>,
    /// Only the selected profile's listed cardinalities are admitted.
    #[arg(long)]
    pub size: Option<usize>,
}
#[derive(Debug)]
struct Plan {
    action: Action,
    selection: Value,
    root: Option<PathBuf>,
}
fn plan(options: &SeedOptions) -> Result<Plan> {
    ensure!(
        !options.is_retired()
            && !options.reset
            && options.sample_input.is_none()
            && options.sample_root.is_none()
            && options.sample_media_source.is_none()
            && !options.sample_serve,
        "dataset and Theatre/sample/retired flags cannot be combined"
    );
    let o = &options.dataset;
    let action = o.action.unwrap_or(Action::List);
    let registry: Value = serde_json::from_str(REGISTRY)?;
    if action == Action::List {
        ensure!(
            o.profile.is_none() && o.dataset_root.is_none() && o.seed.is_none() && o.size.is_none(),
            "list takes no dataset selection"
        );
        return Ok(Plan {
            action,
            selection: registry,
            root: None,
        });
    }
    let name = o
        .profile
        .as_deref()
        .ok_or_else(|| anyhow!("--profile is required"))?;
    let profile = registry["profiles"]
        .as_array()
        .ok_or_else(|| anyhow!("invalid registry"))?
        .iter()
        .find(|p| p["name"] == name)
        .ok_or_else(|| anyhow!("unknown profile; use --action list"))?;
    let size = o.size.unwrap_or(
        profile["default_size"]
            .as_u64()
            .ok_or_else(|| anyhow!("invalid size"))? as usize,
    );
    ensure!(
        profile["sizes"]
            .as_array()
            .is_some_and(|sizes| sizes.contains(&json!(size))),
        "size is outside this profile's explicit bounds"
    );
    let selection = json!({"profile":name,"seed":o.seed.unwrap_or(568),"size":size});
    let root = if action == Action::Plan {
        None
    } else {
        let path = o
            .dataset_root
            .as_deref()
            .ok_or_else(|| anyhow!("--dataset-root is required"))?;
        // Resolve no symlink component, including parents. Do not canonicalize an alias into authority.
        for ancestor in path.ancestors() {
            ensure!(
                !fs::symlink_metadata(ancestor).is_ok_and(|m| m.file_type().is_symlink()),
                "dataset paths cannot contain symbolic links"
            );
            ensure!(
                ancestor == path
                    || (!ancestor.join("sample-owner.json").exists()
                        && !ancestor.join("server.db").exists()
                        && !ancestor.join("identity.db").exists()),
                "dataset root overlaps another database or dataset root"
            );
        }
        let root = sample::destination(path)?;
        if root.exists() {
            let marker_path = root.join("sample-owner.json");
            let metadata = fs::symlink_metadata(&marker_path)
                .map_err(|_| anyhow!("existing root has no dataset ownership marker"))?;
            ensure!(
                metadata.file_type().is_file() && metadata.len() <= 65536,
                "dataset marker must be a bounded regular file"
            );
            let marker: Value = serde_json::from_slice(&fs::read(marker_path)?)?;
            ensure!(
                marker["dataset_id"] == DATASET
                    && marker["root"].as_str() == root.to_str()
                    && marker["selection"] == selection,
                "foreign root or changed dataset selection; no command started"
            );
        } else {
            ensure!(action == Action::Create, "dataset root does not exist");
        }
        Some(root)
    };
    Ok(Plan {
        action,
        selection,
        root,
    })
}
fn command(workspace: &Workspace, plan: &Plan) -> CommandSpec {
    CommandSpec::new("cargo", workspace.root())
        .args([
            "test",
            "--locked",
            "-p",
            "cantos-server",
            "--test",
            "local_dataset_turso",
            "dataset_cli",
            "--",
            "--ignored",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("CANTOS_DATASET_SELECTION", plan.selection.to_string())
        .env("CANTOS_DATASET_ACTION", plan.action.name())
        .env(
            "CANTOS_DATASET_ROOT",
            plan.root
                .as_ref()
                .map_or(String::new(), |p| p.display().to_string()),
        )
        .unset("CANTOS_DATABASE_URL")
        .unset("CANTOS_SEED_API_ORIGIN")
        .unset("CANTOS_SEED_STUDIO_TOKEN")
}
pub(super) fn execute(
    workspace: &Workspace,
    options: &SeedOptions,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<std::result::Result<ExitCode, Refusal>> {
    let plan = match plan(options) {
        Ok(p) => p,
        Err(e) => return Ok(Err(Refusal::Usage(e.to_string()))),
    };
    if matches!(plan.action, Action::List | Action::Plan) {
        println!("{}", serde_json::to_string_pretty(&plan.selection)?);
        return Ok(Ok(ExitCode::SUCCESS));
    }
    // The owning fixture is Unix-only. Never turn a platform-gated, zero-test invocation
    // into a successful persistent dataset operation.
    if !cfg!(unix) {
        return Ok(Err(Refusal::Unsupported {
            subject: "localhost dataset".into(),
            reason: "the Server dataset fixture requires a Unix host; no command started".into(),
        }));
    }
    let root = plan
        .root
        .as_ref()
        .ok_or_else(|| anyhow!("dataset root missing"))?;
    let _operation = sample::operation_lock(root)?;
    let mut report = Report::new("localhost dataset");
    if !tools.missing(&[Tool::Cargo]).is_empty() {
        report.record(
            "Server dataset fixture",
            Status::Unsupported,
            "cargo unavailable; dataset untouched",
        );
    } else {
        match runner.run(&command(workspace, &plan)) {
            Ok(()) => {
                if plan.action == Action::Archive {
                    match sample::archive_owned(root, DATASET) {
                        Ok(path) => report.record(
                            "archive dataset",
                            Status::Pass,
                            format!("preserved all files in {}", path.display()),
                        ),
                        Err(e) => report.record("archive dataset", Status::Fail, e.to_string()),
                    }
                } else {
                    report.record("Server dataset fixture",Status::Pass,format!("{}; inspect manifest/report for separate capability and browser status",root.display()));
                }
            }
            Err(e) => report.record("Server dataset fixture", Status::Fail, e.to_string()),
        }
    }
    report.print();
    Ok(Ok(report.exit_code()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreign_overlapping_symlinked_and_changed_roots_refuse_before_effects() {
        let parent = std::env::temp_dir().join(format!(
            "cantos-dataset-plan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&parent).unwrap();
        let parent = parent.canonicalize().unwrap();
        let root = parent.join("data");
        let mut options = SeedOptions {
            dataset: Options {
                action: Some(Action::Verify),
                profile: Some("empty".into()),
                dataset_root: Some(root.clone()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(plan(&options)
            .unwrap_err()
            .to_string()
            .contains("does not exist"));
        options.dataset.action = Some(Action::Create);
        assert_eq!(plan(&options).unwrap().root, Some(root.clone()));
        assert!(!root.exists());
        fs::create_dir(&root).unwrap();
        fs::write(root.join("retain"), "user evidence").unwrap();
        assert!(plan(&options)
            .unwrap_err()
            .to_string()
            .contains("no dataset ownership"));
        fs::write(root.join("sample-owner.json"), serde_json::to_vec(&json!({"dataset_id":DATASET,"root":root,"selection":{"profile":"empty","seed":568,"size":0}})).unwrap()).unwrap();
        assert!(plan(&options).is_ok());
        options.dataset.seed = Some(569);
        assert!(plan(&options)
            .unwrap_err()
            .to_string()
            .contains("changed dataset selection"));
        options.dataset.seed = None;
        options.dataset.dataset_root = Some(root.join("nested"));
        assert!(plan(&options).unwrap_err().to_string().contains("overlaps"));
        #[cfg(unix)]
        {
            let alias = parent.join("alias");
            std::os::unix::fs::symlink(&root, &alias).unwrap();
            options.dataset.dataset_root = Some(alias);
            assert!(plan(&options)
                .unwrap_err()
                .to_string()
                .contains("symbolic links"));
        }
        assert_eq!(
            fs::read_to_string(root.join("retain")).unwrap(),
            "user evidence"
        );
        assert!(!parent.join(".data.sample-owner").exists());
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn list_plan_and_heavy_sizes_are_explicit_and_bounded_before_effects() {
        assert_eq!(plan(&SeedOptions::default()).unwrap().action, Action::List);
        let mut o = SeedOptions {
            dataset: Options {
                action: Some(Action::Plan),
                profile: Some("scale-local".into()),
                size: Some(10000),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(plan(&o).unwrap().selection["size"], 10000);
        o.dataset.size = Some(10001);
        assert_eq!(
            plan(&o).unwrap_err().to_string(),
            "size is outside this profile's explicit bounds"
        );
        o.dataset.size = None;
        o.sample_serve = true;
        assert!(plan(&o)
            .unwrap_err()
            .to_string()
            .contains("cannot be combined"));
        o.sample_serve = false;
        o.dataset.action = Some(Action::Create);
        assert_eq!(
            plan(&o).unwrap_err().to_string(),
            "--dataset-root is required"
        );
    }
    #[test]
    fn command_selects_only_ignored_server_fixture_and_no_environment_target() {
        let p = Plan {
            action: Action::Create,
            selection: json!({"profile":"empty","seed":568,"size":0}),
            root: Some("/isolated/dataset".into()),
        };
        let spec = command(&Workspace::discover().unwrap(), &p);
        assert!(spec.args.contains(&"--ignored".into()) && spec.args.contains(&"--exact".into()));
        assert!(spec.args.contains(&"local_dataset_turso".into()));
        assert!(!spec
            .args
            .iter()
            .any(|a| a.contains("identity") || a.contains("deploy")));
    }
}
