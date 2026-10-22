//! `cargo xtask preflight`: the developer safety gate before pushing. The default `handoff`
//! profile is that gate; `--profile local` is a lighter DEV/CHECKPOINT subset of it.

mod changes;
mod graph;
mod select;

use std::{collections::BTreeSet, process::ExitCode};

use anyhow::Result;
use clap::ValueEnum;

use crate::{
    command::Runner,
    gate::{self, Gate, GateKind},
    kani, pipeline,
    tools::ToolProbe,
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PreflightProfile {
    /// The complete developer safety gate (current behavior).
    Handoff,
    /// Static repository checks, format, affected Rust lint/tests and WASM lint only.
    Local,
}

/// Printed with every local result so it cannot be read as a handoff preflight.
const LOCAL_SCOPE: &str = "Full handoff, database integration, platform runtime and formal verification are not established by this profile.";

pub struct PreflightOptions {
    pub profile: PreflightProfile,
    pub kani: bool,
    pub base: String,
    pub changed_only: bool,
    pub skip: BTreeSet<GateKind>,
    pub dry_run: bool,
}

impl PreflightOptions {
    /// Rejects flag combinations the selected profile cannot honor.
    pub fn validate(&self) -> Result<(), String> {
        if self.profile != PreflightProfile::Local {
            return Ok(());
        }
        if !self.changed_only {
            return Err("--profile local plans from the Git diff and requires --changed-only; run `cargo xtask preflight --profile local --changed-only --base REF`".into());
        }
        if self.kani {
            return Err("--profile local never runs Kani; drop --kani, or use `cargo xtask preflight --changed-only --kani` or `cargo xtask verify kani`".into());
        }
        Ok(())
    }
}

pub fn run(
    workspace: &Workspace,
    options: &PreflightOptions,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    options.validate().map_err(anyhow::Error::msg)?;
    let local = options.profile == PreflightProfile::Local;
    println!("Cantos preflight");
    if local {
        println!("  profile:       local (DEV/CHECKPOINT subset; not a handoff preflight)");
    }
    let changed = if options.changed_only {
        let paths = changes::changed_paths(workspace.root(), &options.base)?;
        println!("  base:          {}", options.base);
        println!("  changed paths: {}", paths.len());
        Some(paths)
    } else {
        None
    };
    let graph = match changed {
        Some(_) => graph::PackageGraph::load(workspace.root())?,
        None => graph::PackageGraph::default(),
    };
    let mut selection = match &changed {
        Some(paths) if local => select::select_local(paths, &graph),
        _ => select::select_with_kani(changed.as_deref(), &graph, options.kani),
    };
    for gate in &mut selection.gates {
        if let Gate::Repository { base } | Gate::RepositoryStatic { base } = gate {
            *base = Some(options.base.clone());
        }
    }
    match &selection.broadened {
        Some(reason) if local => println!("  impact:        every local check ({reason})"),
        Some(reason) => println!("  impact:        every gate ({reason})"),
        None if selection.affected.is_empty() => println!("  impact:        no Cargo package"),
        None => println!(
            "  impact:        {}",
            selection
                .affected
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }

    let mut steps = gate::steps(&selection.gates, workspace);
    for (step, gate) in steps.iter_mut().zip(&selection.gates) {
        step.skipped = options.skip.contains(&gate.kind());
    }
    println!("  plan:");
    pipeline::print_plan(&steps, workspace.root());
    let deferred: Vec<String> = selection.deferred.iter().map(Gate::name).collect();
    if local {
        println!(
            "  outside the local profile (deferred to handoff preflight or separate acceptance):"
        );
        if deferred.is_empty() {
            println!("  - none for this diff; this is still not a handoff run");
        }
        for name in &deferred {
            println!("  - {name}");
        }
    }
    for note in &selection.notes {
        println!("  note: {note}");
    }
    if options.dry_run {
        if local {
            println!("\nDry run: nothing was executed (every selected step is NOT_RUN).");
            println!("{LOCAL_SCOPE}");
        } else {
            println!("\nDry run: nothing was executed (every gate is NOT_RUN).");
        }
        return Ok(ExitCode::SUCCESS);
    }

    if selection
        .gates
        .iter()
        .any(|gate| matches!(gate, Gate::Kani(_)))
        && !options.skip.contains(&GateKind::Kani)
    {
        if let Err(error) = kani::prepare_configuration(workspace, &mut steps) {
            return Ok(kani::configuration_refusal("preflight", &steps, error));
        }
    }
    let title = if local {
        "local checkpoint"
    } else {
        "preflight"
    };
    let mut report = pipeline::execute(title, &steps, tools, runner);
    for note in &selection.notes {
        report.note(note.clone());
    }
    if local {
        if !deferred.is_empty() {
            report.note(format!(
                "outside the local profile, not run: {}",
                deferred.join(", ")
            ));
        }
        report.note(if report.succeeded() {
            format!("Local checkpoint checks completed. {LOCAL_SCOPE}")
        } else {
            LOCAL_SCOPE.to_owned()
        });
    }
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        command::testing::RecordingRunner,
        tools::{testing::FakeTools, Tool},
    };
    use std::{fs, process::Command, time::SystemTime};

    /// A one-package Git fixture whose `integration-base` tag is the comparison base.
    fn fixture_repository(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "xtask-preflight-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("xtask/src")).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=['xtask']\n").unwrap();
        fs::write(
            root.join("xtask/Cargo.toml"),
            "[package]\nname='fixture'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        fs::write(root.join("xtask/src/main.rs"), "fn main() {}\n").unwrap();
        // `cargo metadata` may write a lock file; it must not become a changed path.
        fs::write(root.join(".gitignore"), "Cargo.lock\ntarget/\n").unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec!["commit", "-qm", "base"],
            vec!["tag", "integration-base"],
        ] {
            let output = Command::new("git")
                .args([
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                ])
                .args(args)
                .current_dir(&root)
                .output()
                .unwrap();
            assert!(output.status.success(), "{output:?}");
        }
        root
    }

    fn options(profile: PreflightProfile, changed_only: bool) -> PreflightOptions {
        PreflightOptions {
            profile,
            kani: false,
            base: "integration-base".into(),
            changed_only,
            skip: BTreeSet::new(),
            dry_run: false,
        }
    }

    fn lines(root: &std::path::Path, runner: &RecordingRunner) -> Vec<String> {
        runner
            .commands
            .iter()
            .map(|command| command.display(root))
            .collect()
    }

    /// Every tool a local plan never needs: integration, runtime, platform and formal tooling.
    const CAMPAIGN_TOOLS: [Tool; 11] = [
        Tool::Trunk,
        Tool::Gradle,
        Tool::Java,
        Tool::CmpGradleProject,
        Tool::CmpAndroidGradleProject,
        Tool::TheatrePreviewGradleProject,
        Tool::Node,
        Tool::Ffmpeg,
        Tool::Ffprobe,
        Tool::Kani,
        Tool::Quint,
    ];

    #[test]
    fn repository_localization_uses_the_preflight_base_in_both_modes() {
        let root = fixture_repository("base");
        fs::write(root.join("README.md"), "Changed documentation.\n").unwrap();
        let workspace = Workspace::fixture(&root).unwrap();
        for changed_only in [false, true] {
            let options = options(PreflightProfile::Handoff, changed_only);
            let mut runner = RecordingRunner::default();
            assert_eq!(
                run(&workspace, &options, &FakeTools::all(), &mut runner).unwrap(),
                ExitCode::SUCCESS
            );
            let check = runner
                .commands
                .iter()
                .find(|command| {
                    command
                        .args
                        .first()
                        .is_some_and(|arg| arg == "scripts/check_repository.py")
                })
                .unwrap();
            assert_eq!(
                check.args,
                ["scripts/check_repository.py", "--base", "integration-base"],
                "changed_only={changed_only}"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_documentation_runs_static_checks_without_campaign_prerequisites() {
        let root = fixture_repository("local-docs");
        fs::write(root.join("README.md"), "Changed documentation.\n").unwrap();
        fs::write(root.join("xtask/README.md"), "Tooling documentation.\n").unwrap();
        let workspace = Workspace::fixture(&root).unwrap();
        let mut runner = RecordingRunner::default();
        let code = run(
            &workspace,
            &options(PreflightProfile::Local, true),
            &FakeTools::without(&CAMPAIGN_TOOLS),
            &mut runner,
        )
        .unwrap();
        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(
            lines(&root, &runner),
            [
                "cargo run --quiet --locked -p xtask -- services validate",
                "python3 scripts/check_repository.py --base integration-base",
                "git diff --check",
                "git diff --cached --check",
                "cargo fmt --all -- --check",
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_tooling_changes_run_package_checks_and_real_failures_fail() {
        let root = fixture_repository("local-tooling");
        fs::write(root.join("xtask/src/main.rs"), "fn main() { let _ = 1; }\n").unwrap();
        let workspace = Workspace::fixture(&root).unwrap();
        let mut runner = RecordingRunner::default();
        let local = options(PreflightProfile::Local, true);
        assert_eq!(
            run(
                &workspace,
                &local,
                &FakeTools::without(&CAMPAIGN_TOOLS),
                &mut runner
            )
            .unwrap(),
            ExitCode::SUCCESS
        );
        let planned = lines(&root, &runner);
        assert!(planned.contains(&"python3 -m unittest discover -s scripts -p 'test_*.py'".into()));
        assert!(planned
            .contains(&"cargo clippy -p fixture --all-targets --locked -- -D warnings".into()));
        assert_eq!(planned.last().unwrap(), "cargo test -p fixture --locked");

        let mut failing = RecordingRunner {
            fail_on: Some("clippy".into()),
            ..Default::default()
        };
        assert_eq!(
            run(&workspace, &local, &FakeTools::all(), &mut failing).unwrap(),
            ExitCode::FAILURE
        );
        assert!(lines(&root, &failing)
            .last()
            .unwrap()
            .starts_with("cargo clippy -p fixture"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_profile_refuses_a_missing_base_and_conflicting_flags() {
        let root = fixture_repository("local-refusal");
        let workspace = Workspace::fixture(&root).unwrap();
        let mut missing = options(PreflightProfile::Local, true);
        missing.base = "no-such-base".into();
        let mut runner = RecordingRunner::default();
        let error = run(&workspace, &missing, &FakeTools::all(), &mut runner).unwrap_err();
        assert!(error.to_string().contains("does not exist"), "{error}");
        assert!(runner.commands.is_empty());

        let without_diff = options(PreflightProfile::Local, false);
        assert!(without_diff
            .validate()
            .unwrap_err()
            .contains("--changed-only"));
        assert!(run(&workspace, &without_diff, &FakeTools::all(), &mut runner).is_err());
        let mut with_kani = options(PreflightProfile::Local, true);
        with_kani.kani = true;
        assert!(with_kani.validate().unwrap_err().contains("Kani"));
        assert!(runner.commands.is_empty());
        let mut handoff = options(PreflightProfile::Handoff, false);
        handoff.kani = true;
        assert_eq!(handoff.validate(), Ok(()));
        fs::remove_dir_all(root).unwrap();
    }
}
