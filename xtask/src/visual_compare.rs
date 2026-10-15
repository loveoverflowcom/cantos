//! Optional local PNG comparison; no app dependency, baseline approval or network setup.

use std::{io::ErrorKind, path::PathBuf, process::ExitCode};

use anyhow::{bail, Result};
use clap::Args;

use crate::{
    command::CommandSpec,
    process::{Supervisor, Wait},
    report::{Report, Status},
    workspace::Workspace,
};

#[derive(Debug, Args)]
pub struct VisualCompareArgs {
    #[arg(long, required_unless_present_any = ["self_test", "spike_pairs"], conflicts_with_all = ["self_test", "spike_pairs"])]
    pub reference: Option<PathBuf>,
    #[arg(long, required_unless_present_any = ["self_test", "spike_pairs"], conflicts_with_all = ["self_test", "spike_pairs"])]
    pub actual: Option<PathBuf>,
    #[arg(long, required_unless_present_any = ["self_test", "spike_pairs"], conflicts_with_all = ["self_test", "spike_pairs"])]
    pub scenario: Option<String>,
    /// Optional provenance and reviewed masks; absent fields remain unknown.
    #[arg(long, conflicts_with_all = ["self_test", "spike_pairs"])]
    pub metadata: Option<PathBuf>,
    /// Fresh directory beneath artifacts/ui-evidence; never overwrites an earlier run.
    #[arg(long, conflicts_with_all = ["self_test", "spike_pairs"])]
    pub output: Option<PathBuf>,
    /// Maximum premultiplied RGBA channel delta ignored (0 means exact).
    #[arg(long, default_value_t = 0)]
    pub threshold: u8,
    /// Adjacent occupied tiles form regions; every changed pixel is still counted.
    #[arg(long, default_value_t = 16, value_parser = clap::value_parser!(u16).range(1..=128))]
    pub tile_size: u16,
    /// Interpreter with the pinned optional Pillow QA dependency; never installed implicitly.
    #[arg(long, default_value = "python3")]
    pub python: String,
    /// Run the standard-library core tests and optional installed-Pillow adapter tests.
    #[arg(long)]
    pub self_test: bool,
    /// Optional A/B/C/D benchmark manifest; requires explicitly installed isolated npm tools.
    #[arg(long, conflicts_with = "self_test")]
    pub spike_pairs: Option<PathBuf>,
}

fn command(workspace: &Workspace, args: &VisualCompareArgs) -> CommandSpec {
    let mut spec = CommandSpec::new(&args.python, workspace.root());
    if args.self_test {
        return spec.args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_visual_compare.py",
        ]);
    }
    if let Some(pairs) = &args.spike_pairs {
        return spec.args([
            "scripts/visual_compare_spike.py".to_owned(),
            pairs.display().to_string(),
        ]);
    }
    spec = spec.args([
        "scripts/visual_compare.py".to_owned(),
        "--threshold".to_owned(),
        args.threshold.to_string(),
        "--tile-size".to_owned(),
        args.tile_size.to_string(),
    ]);
    for (flag, path) in [
        ("--reference", &args.reference),
        ("--actual", &args.actual),
        ("--metadata", &args.metadata),
        ("--output", &args.output),
    ] {
        if let Some(path) = path {
            spec = spec.args([flag.to_owned(), path.display().to_string()]);
        }
    }
    if let Some(scenario) = &args.scenario {
        spec = spec.args(["--scenario", scenario]);
    }
    spec
}

pub fn run(workspace: &Workspace, args: &VisualCompareArgs) -> Result<ExitCode> {
    let spec = command(workspace, args);
    println!("    $ {}", spec.display(workspace.root()));
    let mut supervisor = Supervisor::new()?;
    if let Err(error) = supervisor.spawn("visual-compare", &spec) {
        if error.chain().any(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == ErrorKind::NotFound)
        }) {
            let mut report = Report::new("visual-compare");
            report.record("interpreter", Status::Unsupported, error.to_string());
            report.print();
            return Ok(report.exit_code());
        }
        return Err(error);
    }
    let outcome = supervisor.wait()?;
    supervisor.shutdown();
    let Wait::Exited { status, .. } = outcome else {
        bail!("visual comparison interrupted");
    };
    let mut report = Report::new("visual-compare");
    let (result, detail) = match status.code() {
        Some(0) => (
            Status::Pass,
            "tool completed; pixel results do not approve UX or a baseline",
        ),
        Some(3) => (
            Status::Unsupported,
            "optional comparator/spike dependency unavailable; no install attempted",
        ),
        _ => (Status::Fail, "comparison/test failed; see diagnostic above"),
    };
    let operation = if args.self_test {
        "self-test"
    } else if args.spike_pairs.is_some() {
        "spike"
    } else {
        "comparison"
    };
    report.record(operation, result, detail);
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Task};
    use clap::Parser;

    #[test]
    fn comparison_requires_a_pair_and_forwards_literal_paths() {
        assert!(Cli::try_parse_from(["cargo xtask", "visual-compare"]).is_err());
        let cli = Cli::try_parse_from([
            "cargo xtask",
            "visual-compare",
            "--reference",
            "a b.png",
            "--actual",
            "$(x).png",
            "--scenario",
            "player-vi",
            "--threshold",
            "8",
        ])
        .unwrap();
        let Task::VisualCompare(args) = cli.command else {
            panic!("wrong task")
        };
        let spec = command(&Workspace::discover().unwrap(), &args);
        assert!(spec
            .args
            .windows(2)
            .any(|w| w == ["--reference", "a b.png"]));
        assert!(spec.args.windows(2).any(|w| w == ["--actual", "$(x).png"]));
        assert!(spec.args.windows(2).any(|w| w == ["--threshold", "8"]));
    }

    #[test]
    fn self_test_needs_no_images_or_pillow_setup() {
        let cli = Cli::try_parse_from(["cargo xtask", "visual-compare", "--self-test"]).unwrap();
        let Task::VisualCompare(args) = cli.command else {
            panic!("wrong task")
        };
        assert_eq!(
            command(&Workspace::discover().unwrap(), &args).args[0],
            "-m"
        );
        assert!(Cli::try_parse_from([
            "cargo xtask",
            "visual-compare",
            "--self-test",
            "--actual",
            "x.png",
        ])
        .is_err());
    }

    #[test]
    fn missing_interpreter_is_refused_without_running_a_comparison() {
        let cli = Cli::try_parse_from([
            "cargo xtask",
            "visual-compare",
            "--self-test",
            "--python",
            "/cantos-nonexistent-visual-compare-test/python",
        ])
        .unwrap();
        let Task::VisualCompare(args) = cli.command else {
            panic!("wrong task")
        };
        assert_eq!(
            run(&Workspace::discover().unwrap(), &args).unwrap(),
            ExitCode::FAILURE
        );
    }
}
