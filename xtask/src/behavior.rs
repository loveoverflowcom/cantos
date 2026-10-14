//! Development-only executable behavior models. Catalog and Quint own selection/semantics;
//! the existing sequential pipeline invokes one bounded, evidence-retaining primitive.
use std::path::Path;

use crate::{command::CommandSpec, workspace::Workspace};

pub const CONFORMANCE_SCRIPT: &str = "scripts/verify_behavior_conformance.py";
pub const OBSERVER: &str = "services/cantos-api/src/contexts/media/quint_conformance_tests.rs";

pub const SCRIPT: &str = "scripts/verify_behavior.py";

pub fn command(
    workspace: &Workspace,
    capability: Option<&str>,
    setup: bool,
    conformance: bool,
) -> CommandSpec {
    let mut command = CommandSpec::new("python3", workspace.root())
        .args([SCRIPT])
        .arg_if(setup, "--setup")
        .arg_if(conformance, "--conformance");
    if let Some(capability) = capability {
        command = command.args(["--capability", capability]);
    }
    command
}

pub fn probe_command(root: &Path, runtime_only: bool) -> CommandSpec {
    CommandSpec::new("python3", root)
        .args([SCRIPT, "--probe"])
        .arg_if(runtime_only, "--runtime-only")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_only_the_narrow_existing_model_primitive() {
        let workspace = Workspace::discover().unwrap();
        assert_eq!(command(&workspace, None, false, false).args, [SCRIPT]);
        assert_eq!(
            command(
                &workspace,
                Some("cantos.media.private-audio-admission"),
                true,
                false
            )
            .args,
            [
                SCRIPT,
                "--setup",
                "--capability",
                "cantos.media.private-audio-admission"
            ]
        );
        assert_eq!(
            probe_command(workspace.root(), false).args,
            [SCRIPT, "--probe"]
        );
        assert_eq!(
            probe_command(workspace.root(), true).args,
            [SCRIPT, "--probe", "--runtime-only"]
        );
    }
}
