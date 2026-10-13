//! External tool detection. A missing tool makes a step UNSUPPORTED before anything runs;
//! xtask never installs toolchains, SDKs or databases.

use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{
    config::{GRADLE_VERSION, UI_CMP_ANDROID_DIR, UI_CMP_DIR, WASM_TARGET},
    theatre_demo,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tool {
    Cargo,
    Rustfmt,
    Clippy,
    Wasm32Target,
    Trunk,
    Gradle,
    Java,
    CmpGradleProject,
    CmpAndroidGradleProject,
    TheatreDemoGradleProject,
    Python3,
    Git,
    Initdb,
    PgCtl,
    Psql,
    Ffmpeg,
    Ffprobe,
    AndroidCli,
    Kani,
}

impl Tool {
    pub const ALL: [Tool; 19] = [
        Tool::Cargo,
        Tool::Rustfmt,
        Tool::Clippy,
        Tool::Wasm32Target,
        Tool::Trunk,
        Tool::Gradle,
        Tool::Java,
        Tool::CmpGradleProject,
        Tool::CmpAndroidGradleProject,
        Tool::TheatreDemoGradleProject,
        Tool::Python3,
        Tool::Git,
        Tool::Initdb,
        Tool::PgCtl,
        Tool::Psql,
        Tool::Ffmpeg,
        Tool::Ffprobe,
        Tool::AndroidCli,
        Tool::Kani,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Cargo => "cargo",
            Tool::Rustfmt => "rustfmt",
            Tool::Clippy => "clippy",
            Tool::Wasm32Target => WASM_TARGET,
            Tool::Trunk => "trunk",
            Tool::Gradle => "gradle",
            Tool::Java => "java",
            Tool::CmpGradleProject => "CMP Gradle project",
            Tool::CmpAndroidGradleProject => "CMP Android preview Gradle project",
            Tool::TheatreDemoGradleProject => "Theatre mock Gradle project",
            Tool::Python3 => "python3",
            Tool::Git => "git",
            Tool::Initdb => "initdb",
            Tool::PgCtl => "pg_ctl",
            Tool::Psql => "psql",
            Tool::Ffmpeg => "ffmpeg",
            Tool::Ffprobe => "ffprobe",
            Tool::AndroidCli => "android",
            Tool::Kani => "Kani 0.68.0 / CBMC 6.11.0",
        }
    }

    /// How a developer provides the tool; xtask itself never does.
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Cargo => "install Rust 1.87 with rustup",
            Tool::Rustfmt => "rustup component add rustfmt",
            Tool::Clippy => "rustup component add clippy",
            Tool::Wasm32Target => "rustup target add wasm32-unknown-unknown",
            Tool::Trunk => "cargo install trunk --version 0.21.14 --locked",
            Tool::Gradle => "put installed Gradle 8.14.3 on PATH (no wrapper download)",
            Tool::Java => {
                "put a compatible JDK 17 or newer on PATH (the CMP project targets Java 17)"
            }
            Tool::CmpGradleProject => {
                "settings.gradle(.kts) and build.gradle(.kts) must exist in packages/ui-system/cmp"
            }
            Tool::CmpAndroidGradleProject => {
                "settings.gradle(.kts) and build.gradle(.kts) must exist in packages/ui-system/cmp/android-preview"
            }
            Tool::TheatreDemoGradleProject => {
                "the isolated root/shared Gradle project and common/desktop sources must exist in apps/cantos-theatre/mobile"
            }
            Tool::Kani => "prepare the official Kani 0.68.0 bundle with its matching nightly and CBMC 6.11.0 separately; set KANI_HOME; xtask never installs or runs the installer proxy",
            Tool::Python3 => "install Python 3",
            Tool::Git => "install Git",
            Tool::Ffmpeg | Tool::Ffprobe => {
                "put trusted installed ffmpeg and ffprobe executables on PATH; xtask never installs them"
            }
            Tool::AndroidCli => "install the official Android CLI; Compose preview needs Android Studio Quail 2+ with the selected Android project open",
            Tool::Initdb | Tool::PgCtl | Tool::Psql => {
                "put the PostgreSQL 17 server binaries directory on PATH"
            }
        }
    }

    fn version_command(self) -> Option<(&'static str, &'static [&'static str])> {
        match self {
            Tool::Cargo => Some(("cargo", &["--version"])),
            Tool::Rustfmt => Some(("cargo", &["fmt", "--version"])),
            Tool::Clippy => Some(("cargo", &["clippy", "--version"])),
            Tool::Wasm32Target | Tool::Kani => None,
            Tool::Trunk => Some(("trunk", &["--version"])),
            Tool::Gradle => Some(("gradle", &["--no-daemon", "--version"])),
            Tool::Java => Some(("java", &["-version"])),
            Tool::CmpGradleProject => None,
            Tool::CmpAndroidGradleProject => None,
            Tool::TheatreDemoGradleProject => None,
            Tool::Python3 => Some(("python3", &["--version"])),
            Tool::Git => Some(("git", &["--version"])),
            Tool::Initdb => Some(("initdb", &["--version"])),
            Tool::PgCtl => Some(("pg_ctl", &["--version"])),
            Tool::Psql => Some(("psql", &["--version"])),
            Tool::Ffmpeg => Some(("ffmpeg", &["-version"])),
            Tool::Ffprobe => Some(("ffprobe", &["-version"])),
            Tool::AndroidCli => Some(("android", &["--version"])),
        }
    }
}

/// Answers whether a tool is usable here. Injected so plans and executors are testable.
pub trait ToolProbe {
    fn available(&self, tool: Tool) -> bool;

    fn missing(&self, tools: &[Tool]) -> Vec<Tool> {
        tools
            .iter()
            .copied()
            .filter(|tool| !self.available(*tool))
            .collect()
    }
}

/// Probes the real environment from the workspace root (so rustup overrides apply) and caches.
pub struct SystemTools {
    root: PathBuf,
    cache: RefCell<BTreeMap<Tool, bool>>,
}

impl SystemTools {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            cache: RefCell::new(BTreeMap::new()),
        }
    }

    /// First line of the tool's version output, when it can be obtained.
    pub fn version(&self, tool: Tool) -> Option<String> {
        if matches!(
            tool,
            Tool::Wasm32Target
                | Tool::CmpGradleProject
                | Tool::CmpAndroidGradleProject
                | Tool::TheatreDemoGradleProject
        ) {
            return self.available(tool).then(|| "available".to_owned());
        }
        if tool == Tool::Kani {
            let output = crate::kani::probe_command(&self.root)
                .to_command()
                .stdin(Stdio::null())
                .output()
                .ok()?;
            return output.status.success().then(|| {
                String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .replace('\n', "; ")
            });
        }
        let (program, args) = tool.version_command()?;
        let output = Command::new(program)
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        reported_version(tool, &output.stdout, &output.stderr)
    }

    /// `rustc --version` as selected for the workspace (rustup overrides included).
    pub fn rustc_version(&self) -> Option<String> {
        let output = Command::new("rustc")
            .arg("--version")
            .current_dir(&self.root)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    fn detect(&self, tool: Tool) -> bool {
        match tool {
            Tool::Wasm32Target => self.wasm_target_installed(),
            Tool::CmpGradleProject => cmp_gradle_project_exists(&self.root.join(UI_CMP_DIR)),
            Tool::CmpAndroidGradleProject => {
                cmp_gradle_project_exists(&self.root.join(UI_CMP_ANDROID_DIR))
            }
            Tool::TheatreDemoGradleProject => {
                theatre_demo::project_exists(&self.root.join(crate::config::THEATRE_MOBILE_DIR))
            }
            Tool::Gradle => self
                .version(tool)
                .is_some_and(|version| gradle_version_supported(&version)),
            Tool::Java => self
                .version(tool)
                .is_some_and(|version| java_version_supported(&version)),
            Tool::Rustfmt | Tool::Clippy | Tool::Ffmpeg | Tool::Ffprobe | Tool::Kani => {
                self.version(tool).is_some()
            }
            other => find_on_path(other.label()).is_some(),
        }
    }

    fn wasm_target_installed(&self) -> bool {
        let Ok(output) = Command::new("rustc")
            .args(["--print", "sysroot"])
            .current_dir(&self.root)
            .output()
        else {
            return false;
        };
        let sysroot = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        output.status.success()
            && Path::new(&sysroot)
                .join("lib/rustlib")
                .join(WASM_TARGET)
                .is_dir()
    }
}

impl ToolProbe for SystemTools {
    fn available(&self, tool: Tool) -> bool {
        if let Some(known) = self.cache.borrow().get(&tool) {
            return *known;
        }
        let found = self.detect(tool);
        self.cache.borrow_mut().insert(tool, found);
        found
    }
}

/// Select only reportable tool output; Java startup diagnostics can contain environment values.
fn reported_version(tool: Tool, stdout: &[u8], stderr: &[u8]) -> Option<String> {
    if tool == Tool::Java {
        // JAVA_TOOL_OPTIONS and JDK_JAVA_OPTIONS notices precede the actual version. Never
        // return those diagnostics: doctor must report the version without exposing their values.
        return [stdout, stderr].into_iter().find_map(|bytes| {
            String::from_utf8_lossy(bytes)
                .lines()
                .map(str::trim)
                .find(|line| {
                    line.starts_with("openjdk version \"") || line.starts_with("java version \"")
                })
                .map(str::to_owned)
        });
    }
    let text = String::from_utf8_lossy(if stdout.is_empty() { stderr } else { stdout });
    if tool == Tool::Gradle {
        return text
            .lines()
            .find(|line| line.starts_with("Gradle "))
            .map(str::to_owned);
    }
    text.lines().next().map(|line| line.trim().to_owned())
}

/// The desktop preview emits Java 17 bytecode, so an older JVM cannot run it.
fn java_version_supported(version: &str) -> bool {
    version
        .split('"')
        .nth(1)
        .and_then(|number| number.split('.').next())
        .and_then(|major| major.parse::<u32>().ok())
        .is_some_and(|major| major >= 17)
}

/// A package-local preview must be a real Gradle project, not a reserved README.
pub fn cmp_gradle_project_exists(dir: &Path) -> bool {
    ["settings.gradle.kts", "settings.gradle"]
        .iter()
        .any(|file| dir.join(file).is_file())
        && ["build.gradle.kts", "build.gradle"]
            .iter()
            .any(|file| dir.join(file).is_file())
}

/// The package pins the reviewed Kotlin 2.2.20 / Compose 1.9 Gradle 8.14.3 version.
fn gradle_version_supported(version: &str) -> bool {
    version.strip_prefix("Gradle ") == Some(GRADLE_VERSION)
}

/// Locates an executable on `PATH` without running it.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
pub mod testing {
    use super::*;
    use std::collections::BTreeSet;

    /// A fixed set of available tools.
    pub struct FakeTools(pub BTreeSet<Tool>);

    impl FakeTools {
        pub fn all() -> Self {
            Self(Tool::ALL.into_iter().collect())
        }

        pub fn without(tools: &[Tool]) -> Self {
            let mut set: BTreeSet<Tool> = Tool::ALL.into_iter().collect();
            for tool in tools {
                set.remove(tool);
            }
            Self(set)
        }
    }

    impl ToolProbe for FakeTools {
        fn available(&self, tool: Tool) -> bool {
            self.0.contains(&tool)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_version_selection_skips_startup_diagnostics_without_reporting_them() {
        let output = b"Picked up JAVA_TOOL_OPTIONS: -Djava.awt.headless=true\nopenjdk version \"21.0.12.1\" 2026-08-18\nOpenJDK Runtime Environment\n";
        let selected = reported_version(Tool::Java, b"", output).unwrap();
        assert_eq!(selected, r#"openjdk version "21.0.12.1" 2026-08-18"#);
        assert!(java_version_supported(&selected));
    }

    #[test]
    fn java_version_selection_checks_both_streams_and_rejects_diagnostics_only() {
        let preamble = b"Picked up JDK_JAVA_OPTIONS: -Djava.awt.headless=true\n";
        let version = b"java version \"17.0.16\" 2025-07-15\n";
        assert_eq!(
            reported_version(Tool::Java, preamble, version).as_deref(),
            Some(r#"java version "17.0.16" 2025-07-15"#)
        );
        assert_eq!(
            reported_version(Tool::Java, version, preamble).as_deref(),
            Some(r#"java version "17.0.16" 2025-07-15"#)
        );
        assert_eq!(reported_version(Tool::Java, preamble, preamble), None);
    }

    #[test]
    fn the_desktop_preview_needs_at_least_java_17_bytecode_support() {
        for version in [
            r#"openjdk version "17.0.16" 2025-07-15"#,
            r#"java version "21.0.1""#,
        ] {
            assert!(java_version_supported(version), "{version}");
        }
        for version in [
            r#"openjdk version "11.0.25""#,
            r#"java version "1.8.0_422""#,
            "",
            "java broken",
        ] {
            assert!(!java_version_supported(version), "{version}");
        }
    }

    #[test]
    fn gradle_versions_are_bounded_and_preview_versions_are_rejected() {
        assert!(gradle_version_supported("Gradle 8.14.3"));
        for version in [
            "Gradle 8.14.2",
            "Gradle 8.14.4",
            "Gradle 8.15",
            "Gradle 8.20.0",
            "Gradle 8.14",
            "Gradle 9.0.0",
            "Gradle 8.14.3-rc-1",
            "gradle 8.14.3",
            "",
            "Gradle broken",
        ] {
            assert!(!gradle_version_supported(version), "{version}");
        }
    }

    #[test]
    fn cmp_project_detection_needs_both_settings_and_build_files() {
        let dir = std::env::temp_dir().join(format!("xtask-cmp-project-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!cmp_gradle_project_exists(&dir));
        std::fs::write(dir.join("README.md"), "guide only").unwrap();
        assert!(!cmp_gradle_project_exists(&dir));
        std::fs::write(
            dir.join("settings.gradle.kts"),
            "rootProject.name = \"test\"",
        )
        .unwrap();
        assert!(!cmp_gradle_project_exists(&dir));
        std::fs::write(dir.join("build.gradle.kts"), "plugins {}").unwrap();
        assert!(cmp_gradle_project_exists(&dir));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn finds_programs_on_path_and_rejects_unknown_ones() {
        assert!(find_on_path("sh").is_some());
        assert!(find_on_path("cantos-no-such-program").is_none());
    }

    #[test]
    fn missing_lists_only_unavailable_tools() {
        let probe = testing::FakeTools::without(&[Tool::Trunk, Tool::Psql]);
        assert_eq!(
            probe.missing(&[Tool::Cargo, Tool::Trunk, Tool::Psql]),
            vec![Tool::Trunk, Tool::Psql]
        );
    }

    #[test]
    fn media_tools_use_their_supported_version_flags_and_keep_separate_prerequisites() {
        assert_eq!(
            Tool::Ffmpeg.version_command(),
            Some(("ffmpeg", &["-version"][..]))
        );
        assert_eq!(
            Tool::Ffprobe.version_command(),
            Some(("ffprobe", &["-version"][..]))
        );
        let probe = testing::FakeTools::without(&[Tool::Ffprobe]);
        assert_eq!(
            probe.missing(&[Tool::Cargo, Tool::Ffmpeg, Tool::Ffprobe]),
            [Tool::Ffprobe]
        );
    }
}
