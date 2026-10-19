//! Offline mobile toolchain probes and the shared Gradle invocation. These are neutral primitives
//! used by the Desktop Preview, the production Theatre host and the retired-lane refusals; they
//! never download, install, select or configure a toolchain.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{command::CommandSpec, config::THEATRE_MOBILE_DIR, workspace::Workspace};

const KOTLIN_NATIVE_VERSION: &str = "2.2.20";
const ANDROID_PLATFORM: &str = "platforms/android-35/android.jar";
pub const ANDROID_BUILD_TOOLS: &str = "build-tools/35.0.0";

/// The offline Gradle invocation shared by the demo and production-host commands.
pub fn gradle_command(workspace: &Workspace) -> CommandSpec {
    CommandSpec::new("gradle", workspace.root()).args([
        "--project-dir",
        THEATRE_MOBILE_DIR,
        "--no-daemon",
        "-Dorg.gradle.java.installations.auto-download=false",
        "--max-workers",
        "2",
        "-Pkotlin.compiler.execution.strategy=in-process",
        "--no-configuration-cache",
    ])
}

/// The already installed Android SDK named by the environment, if any; never downloaded.
pub fn android_sdk_from_environment() -> Option<PathBuf> {
    std::env::var_os("ANDROID_HOME")
        .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
        .map(PathBuf::from)
}

pub(crate) fn native_compiler_prerequisite(compiler_home: &Path) -> Result<(), &'static str> {
    if !compiler_home.is_absolute() || compiler_home.to_str().is_none() {
        return Err("--kotlin-native-home must be an absolute UTF-8 path to an installed compiler");
    }
    if !local_executable(&compiler_home.join("bin/konanc")) {
        return Err("installed Kotlin/Native compiler is missing executable bin/konanc; xtask never downloads it");
    }
    if !compiler_home
        .join("klib/common/stdlib/default/manifest")
        .is_file()
    {
        return Err("installed Kotlin/Native compiler has no stdlib manifest");
    }
    // The stdlib manifest records its build compiler, which can be an RC even in a stable
    // distribution. Ask the installed compiler for its distribution version without a target.
    let output = Command::new(compiler_home.join("bin/konanc"))
        .arg("-version")
        .output()
        .map_err(|_| "installed Kotlin/Native compiler could not report its version")?;
    if !output.status.success() || !pinned_native_version(&output.stdout, &output.stderr) {
        return Err("ios-build requires an already installed Kotlin/Native 2.2.20 compiler");
    }
    Ok(())
}

fn pinned_native_version(stdout: &[u8], stderr: &[u8]) -> bool {
    [stdout, stderr].into_iter().find_map(|bytes| {
        String::from_utf8_lossy(bytes).lines().find_map(|line| {
            line.trim()
                .strip_prefix("Kotlin/Native:")
                .map(|version| version.trim() == KOTLIN_NATIVE_VERSION)
        })
    }) == Some(true)
}

pub fn local_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// An already installed, absolute SDK with platform 35 and build tools 35.0.0; never downloaded.
pub fn android_sdk_prerequisite(sdk: Option<&Path>) -> Result<(), &'static str> {
    let sdk =
        sdk.ok_or("set ANDROID_HOME or ANDROID_SDK_ROOT to an already installed Android SDK")?;
    if !sdk.is_absolute() {
        return Err("configured Android SDK must be an absolute path, independent of the caller's directory");
    }
    if !sdk.is_dir() {
        return Err("configured Android SDK directory does not exist; xtask never installs an SDK");
    }
    if !sdk.join(ANDROID_PLATFORM).is_file()
        || !local_executable(&sdk.join(ANDROID_BUILD_TOOLS).join("aapt2"))
        || !local_executable(&sdk.join(ANDROID_BUILD_TOOLS).join("d8"))
    {
        return Err("requires installed Android SDK platform 35 and build tools 35.0.0; SDK downloading is disabled");
    }
    Ok(())
}

/// A valid simulator SDK distinguishes full Xcode from command-line tools alone. This probe
/// reads installed tool output only and cannot install or configure Xcode.
pub(crate) fn ios_sdk_available() -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    Command::new("xcrun")
        .args(["--sdk", "iphonesimulator", "--show-sdk-path"])
        .output()
        .ok()
        .is_some_and(|output| {
            output.status.success()
                && Path::new(String::from_utf8_lossy(&output.stdout).trim()).is_dir()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn executable_fixture(path: &Path) {
        std::fs::write(path, "fixture executable; never executed").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    #[test]
    fn gradle_runs_offline_without_toolchain_provisioning_or_a_wrapper() {
        let workspace = Workspace::discover().unwrap();
        let command = gradle_command(&workspace);
        assert_eq!(command.program, "gradle");
        assert_eq!(
            command.display(workspace.root()),
            "gradle --project-dir apps/cantos-theatre/mobile --no-daemon -Dorg.gradle.java.installations.auto-download=false --max-workers 2 -Pkotlin.compiler.execution.strategy=in-process --no-configuration-cache"
        );
        assert!(command.env.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn native_probe_uses_distribution_version_instead_of_stdlib_build_version() {
        let dir = std::env::temp_dir().join(format!("xtask-theatre-native-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        assert!(native_compiler_prerequisite(&dir)
            .unwrap_err()
            .contains("bin/konanc"));
        let compiler = dir.join("bin/konanc");
        executable_fixture(&compiler);
        let manifest_dir = dir.join("klib/common/stdlib/default");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(
            manifest_dir.join("manifest"),
            "compiler_version=2.2.20-RC3-275\n",
        )
        .unwrap();
        std::fs::write(
            &compiler,
            "#!/bin/sh\nprintf '%s\\n' 'Kotlin/Native: 2.4.20'\n",
        )
        .unwrap();
        assert!(native_compiler_prerequisite(&dir)
            .unwrap_err()
            .contains("2.2.20"));
        std::fs::write(
            &compiler,
            "#!/bin/sh\nprintf '%s\\n' 'Kotlin/Native: 2.2.20'\n",
        )
        .unwrap();
        assert_eq!(native_compiler_prerequisite(&dir), Ok(()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn native_distribution_version_parser_checks_both_streams_and_exact_pin() {
        for (stdout, stderr) in [
            (b"Kotlin/Native: 2.2.20\n".as_slice(), b"".as_slice()),
            (
                b"".as_slice(),
                b"info: kotlinc-native 2.2.20 (JRE 21)\nKotlin/Native: 2.2.20\n".as_slice(),
            ),
        ] {
            assert!(pinned_native_version(stdout, stderr));
        }
        for line in [
            "Kotlin/Native: 2.4.20",
            "Kotlin/Native: 2.2.20-dev-1",
            "compiler_version=2.2.20",
            "",
        ] {
            assert!(!pinned_native_version(line.as_bytes(), b""));
        }
    }

    #[test]
    fn android_missing_project_or_sdk_never_passes_prerequisites() {
        let dir =
            std::env::temp_dir().join(format!("xtask-theatre-android-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(android_sdk_prerequisite(None)
            .unwrap_err()
            .contains("ANDROID_HOME"));
        assert!(android_sdk_prerequisite(Some(&dir.join("absent-sdk")))
            .unwrap_err()
            .contains("does not exist"));
        assert!(android_sdk_prerequisite(Some(&dir))
            .unwrap_err()
            .contains("platform 35"));
        std::fs::create_dir_all(dir.join("platforms/android-35")).unwrap();
        std::fs::create_dir_all(dir.join(ANDROID_BUILD_TOOLS)).unwrap();
        std::fs::write(dir.join(ANDROID_PLATFORM), "").unwrap();
        executable_fixture(&dir.join(ANDROID_BUILD_TOOLS).join("aapt2"));
        executable_fixture(&dir.join(ANDROID_BUILD_TOOLS).join("d8"));
        assert_eq!(android_sdk_prerequisite(Some(&dir)), Ok(()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
