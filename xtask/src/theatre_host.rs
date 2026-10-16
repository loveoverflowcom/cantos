//! The production Theatre Android host. Unlike `theatre-demo`, it composes `:shared` only and
//! has no mock, sample or diagnostic fallback. Its one command builds an unsigned, unshrunk
//! release APK and AAB and inspects the actual bytes; it never signs, installs or distributes.

use std::{path::Path, process::ExitCode};

use anyhow::Result;
use clap::Subcommand;

use crate::{
    command::{CommandSpec, Runner},
    config::THEATRE_MOBILE_DIR,
    pipeline::{self, Step},
    report::{Report, Status},
    theatre_demo::{self, ANDROID_BUILD_TOOLS},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

pub const APPLICATION_ID: &str = "com.cantos.theatre";
const LAUNCHER: &str = "com.cantos.theatre.app.TheatreActivity";
const APK: &str =
    "androidProductionApp/build/outputs/apk/release/androidProductionApp-release-unsigned.apk";
const AAB: &str =
    "androidProductionApp/build/outputs/bundle/release/androidProductionApp-release.aab";
const REPORT: &str = "androidProductionApp/build/reports/release-boundary/artifact-inspection.txt";
/// The sandbox's own compiled Android classes and resources form the forbidden inventory.
const SANDBOX_CLASSES: &str = "preview-sandbox/build/tmp/kotlin-classes/release";
const SANDBOX_RESOURCES: &str = "preview-sandbox/src/commonMain/composeResources";
const COMPOSE_RESOURCE_PACKAGE: &str = "com.cantos.theatre.resources";
/// Production classes whose absence would make a finding-free inspection vacuous.
const REQUIRED_CLASSES: [&str; 3] = [
    "Lcom/cantos/theatre/app/TheatreActivity;",
    "Lcom/cantos/theatre/app/AppContainer;",
    "Lcom/cantos/theatre/features/launch/presentation/CantosListeningUnavailableKt;",
];
/// androidx.core declares this signature permission in every app; any other needs review.
const ALLOWED_PERMISSIONS: [&str; 1] =
    ["com.cantos.theatre.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION"];

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum HostAction {
    /// Build the production Android host's unsigned release APK and AAB and inspect release
    /// classpaths, DEX classes, resources and manifest for sandbox, mock and test content.
    AndroidRelease,
}

pub fn tools() -> Vec<Tool> {
    vec![
        Tool::Gradle,
        Tool::Java,
        Tool::TheatreDemoGradleProject,
        Tool::Python3,
    ]
}

/// The production host and the sandbox inventory it is checked against must both exist.
pub fn prerequisite(project: &Path, sdk: Option<&Path>) -> Result<(), &'static str> {
    if !project
        .join("androidProductionApp/build.gradle.kts")
        .is_file()
        || !project
            .join("androidProductionApp/src/main/AndroidManifest.xml")
            .is_file()
        || !project.join("shared/src/androidMain/kotlin").is_dir()
        || !project.join(SANDBOX_RESOURCES).is_dir()
    {
        return Err("production Android host, shared Android sources or the sandbox resource inventory are missing");
    }
    theatre_demo::android_sdk_prerequisite(sdk)?;
    if sdk.and_then(Path::to_str).is_none() {
        return Err("configured Android SDK path must be UTF-8 to pass to the inspector");
    }
    Ok(())
}

/// Builds only the production host's release outputs. Its `preBuild` also enforces the
/// resolved release-classpath boundary; the sandbox is compiled solely as an inventory.
pub fn gradle_release_command(workspace: &Workspace) -> CommandSpec {
    theatre_demo::gradle_command(workspace).args([
        "-Pcantos.android=true",
        "-Pandroid.builder.sdkDownload=false",
        ":androidProductionApp:assertReleaseDependencyBoundary",
        ":androidProductionApp:assembleRelease",
        ":androidProductionApp:bundleRelease",
        ":preview-sandbox:compileReleaseKotlinAndroid",
    ])
}

pub fn inspection_command(workspace: &Workspace, sdk: &str) -> CommandSpec {
    let mobile = |path: &str| format!("{THEATRE_MOBILE_DIR}/{path}");
    let mut args = vec![
        "scripts/inspect_android_release.py".to_owned(),
        "--apk".to_owned(),
        mobile(APK),
        "--aab".to_owned(),
        mobile(AAB),
        "--aapt2".to_owned(),
        format!("{sdk}/{ANDROID_BUILD_TOOLS}/aapt2"),
        "--application-id".to_owned(),
        APPLICATION_ID.to_owned(),
        "--launcher".to_owned(),
        LAUNCHER.to_owned(),
        "--compose-resource-package".to_owned(),
        COMPOSE_RESOURCE_PACKAGE.to_owned(),
        "--forbidden-classes".to_owned(),
        mobile(SANDBOX_CLASSES),
        "--forbidden-resources".to_owned(),
        mobile(SANDBOX_RESOURCES),
        "--report".to_owned(),
        mobile(REPORT),
    ];
    for class in REQUIRED_CLASSES {
        args.extend(["--require-class".to_owned(), class.to_owned()]);
    }
    for permission in ALLOWED_PERMISSIONS {
        args.extend(["--allowed-permission".to_owned(), permission.to_owned()]);
    }
    CommandSpec::new("python3", workspace.root()).args(args)
}

fn unsupported(reason: &str) -> ExitCode {
    let mut report = Report::new("theatre-host");
    report.record(
        "Theatre production Android release",
        Status::Unsupported,
        reason,
    );
    report.print();
    report.exit_code()
}

pub fn execute(
    workspace: &Workspace,
    action: HostAction,
    tool_probe: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    match action {
        HostAction::AndroidRelease => {
            let sdk = theatre_demo::android_sdk_from_environment();
            if let Err(reason) = prerequisite(&workspace.path(THEATRE_MOBILE_DIR), sdk.as_deref()) {
                return Ok(unsupported(reason));
            }
            // The prerequisite has accepted an absolute UTF-8 SDK path.
            let sdk = sdk.as_deref().and_then(Path::to_str).unwrap_or_default();
            let steps = vec![
                Step::new(
                    "Theatre production Android release build",
                    &tools(),
                    vec![gradle_release_command(workspace)],
                ),
                Step::new(
                    "Theatre production Android release inspection",
                    &[Tool::Python3],
                    vec![inspection_command(workspace, sdk)],
                ),
            ];
            let report = pipeline::execute("theatre-host", &steps, tool_probe, runner);
            report.print();
            Ok(report.exit_code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::testing::RecordingRunner, tools::testing::FakeTools};
    use std::path::PathBuf;

    fn workspace() -> Workspace {
        Workspace::discover().unwrap()
    }

    fn sdk_fixture(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-theatre-host-{name}-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("platforms/android-35")).unwrap();
        std::fs::create_dir_all(dir.join(ANDROID_BUILD_TOOLS)).unwrap();
        std::fs::write(dir.join("platforms/android-35/android.jar"), "").unwrap();
        for tool in ["aapt2", "d8"] {
            let path = dir.join(ANDROID_BUILD_TOOLS).join(tool);
            std::fs::write(&path, "fixture executable; never executed").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        dir
    }

    #[test]
    fn release_build_names_only_production_outputs_and_the_sandbox_inventory() {
        let workspace = workspace();
        let command = gradle_release_command(&workspace);
        assert_eq!(command.program, "gradle");
        assert!(command.env.is_empty());
        assert!(command.args.ends_with(&[
            "-Pcantos.android=true".to_owned(),
            "-Pandroid.builder.sdkDownload=false".to_owned(),
            ":androidProductionApp:assertReleaseDependencyBoundary".to_owned(),
            ":androidProductionApp:assembleRelease".to_owned(),
            ":androidProductionApp:bundleRelease".to_owned(),
            ":preview-sandbox:compileReleaseKotlinAndroid".to_owned(),
        ]));
        assert!(command
            .args
            .contains(&"-Dorg.gradle.java.installations.auto-download=false".to_owned()));
        // Exactly these tasks: never the demo host, signing, installation or a debug variant.
        let tasks: Vec<_> = command
            .args
            .iter()
            .filter(|arg| arg.starts_with(':'))
            .collect();
        assert_eq!(tasks.len(), 4);
        assert!(!tasks.iter().any(|task| task.starts_with(":androidApp:")
            || task.contains("Debug")
            || task.contains("install")
            || task.contains("sign")));
    }

    #[test]
    fn inspection_reads_the_built_artifacts_against_the_sandbox_inventory() {
        let workspace = workspace();
        let command = inspection_command(&workspace, "/sdk with space");
        assert_eq!(command.program, "python3");
        assert_eq!(command.args[0], "scripts/inspect_android_release.py");
        let value = |flag: &str| -> Vec<&str> {
            command
                .args
                .windows(2)
                .filter(|pair| pair[0] == flag)
                .map(|pair| pair[1].as_str())
                .collect()
        };
        assert_eq!(value("--apk"), [format!("{THEATRE_MOBILE_DIR}/{APK}")]);
        assert_eq!(value("--aab"), [format!("{THEATRE_MOBILE_DIR}/{AAB}")]);
        assert_eq!(
            value("--aapt2"),
            ["/sdk with space/build-tools/35.0.0/aapt2"]
        );
        assert_eq!(value("--application-id"), [APPLICATION_ID]);
        assert_eq!(
            value("--forbidden-classes"),
            [format!("{THEATRE_MOBILE_DIR}/{SANDBOX_CLASSES}")]
        );
        assert_eq!(
            value("--forbidden-resources"),
            [format!("{THEATRE_MOBILE_DIR}/{SANDBOX_RESOURCES}")]
        );
        assert_eq!(value("--require-class"), REQUIRED_CLASSES);
        assert_eq!(value("--allowed-permission"), ALLOWED_PERMISSIONS);
        assert!(!value("--allowed-permission").contains(&"android.permission.INTERNET"));
    }

    #[test]
    fn missing_host_or_sdk_is_unsupported_and_runs_nothing() {
        let dir = std::env::temp_dir().join(format!("xtask-theatre-host-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sdk = sdk_fixture("sdk");
        assert!(prerequisite(&dir, Some(&sdk))
            .unwrap_err()
            .contains("production Android host"));
        std::fs::create_dir_all(dir.join("androidProductionApp/src/main")).unwrap();
        std::fs::create_dir_all(dir.join("shared/src/androidMain/kotlin")).unwrap();
        std::fs::write(dir.join("androidProductionApp/build.gradle.kts"), "").unwrap();
        std::fs::write(
            dir.join("androidProductionApp/src/main/AndroidManifest.xml"),
            "",
        )
        .unwrap();
        // The forbidden inventory is required: inspecting without it would be vacuous.
        assert!(prerequisite(&dir, Some(&sdk))
            .unwrap_err()
            .contains("inventory"));
        std::fs::create_dir_all(dir.join(SANDBOX_RESOURCES)).unwrap();
        assert!(prerequisite(&dir, None)
            .unwrap_err()
            .contains("ANDROID_HOME"));
        assert!(prerequisite(&dir, Some(Path::new("relative-sdk")))
            .unwrap_err()
            .contains("absolute"));
        assert_eq!(prerequisite(&dir, Some(&sdk)), Ok(()));
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::remove_dir_all(sdk).unwrap();
    }

    #[test]
    fn every_missing_tool_prevents_build_and_inspection() {
        let workspace = workspace();
        let steps = |sdk: &str| {
            vec![
                Step::new("build", &tools(), vec![gradle_release_command(&workspace)]),
                Step::new(
                    "inspect",
                    &[Tool::Python3],
                    vec![inspection_command(&workspace, sdk)],
                ),
            ]
        };
        for tool in tools() {
            let mut runner = RecordingRunner::default();
            let report = pipeline::execute(
                "theatre-host",
                &steps("/sdk"),
                &FakeTools::without(&[tool]),
                &mut runner,
            );
            assert_eq!(report.exit_code(), ExitCode::FAILURE);
            assert!(runner.commands.is_empty());
        }
        let mut runner = RecordingRunner {
            fail_on: Some("gradle".into()),
            ..RecordingRunner::default()
        };
        let report = pipeline::execute(
            "theatre-host",
            &steps("/sdk"),
            &FakeTools::all(),
            &mut runner,
        );
        assert_eq!(report.exit_code(), ExitCode::FAILURE);
        // A failed build never lets the inspector read stale artifacts.
        assert_eq!(runner.commands.len(), 1);
        assert_eq!(runner.commands[0].program, "gradle");
    }
}
