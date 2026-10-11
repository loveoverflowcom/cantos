//! External tool detection. A missing tool makes a step UNSUPPORTED before anything runs;
//! xtask never installs toolchains, SDKs or databases.

use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::config::WASM_TARGET;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tool {
    Cargo,
    Rustfmt,
    Clippy,
    Wasm32Target,
    Trunk,
    Python3,
    Git,
    Initdb,
    PgCtl,
    Psql,
}

impl Tool {
    pub const ALL: [Tool; 10] = [
        Tool::Cargo,
        Tool::Rustfmt,
        Tool::Clippy,
        Tool::Wasm32Target,
        Tool::Trunk,
        Tool::Python3,
        Tool::Git,
        Tool::Initdb,
        Tool::PgCtl,
        Tool::Psql,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Cargo => "cargo",
            Tool::Rustfmt => "rustfmt",
            Tool::Clippy => "clippy",
            Tool::Wasm32Target => WASM_TARGET,
            Tool::Trunk => "trunk",
            Tool::Python3 => "python3",
            Tool::Git => "git",
            Tool::Initdb => "initdb",
            Tool::PgCtl => "pg_ctl",
            Tool::Psql => "psql",
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
            Tool::Python3 => "install Python 3",
            Tool::Git => "install Git",
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
            Tool::Wasm32Target => None,
            Tool::Trunk => Some(("trunk", &["--version"])),
            Tool::Python3 => Some(("python3", &["--version"])),
            Tool::Git => Some(("git", &["--version"])),
            Tool::Initdb => Some(("initdb", &["--version"])),
            Tool::PgCtl => Some(("pg_ctl", &["--version"])),
            Tool::Psql => Some(("psql", &["--version"])),
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
        if tool == Tool::Wasm32Target {
            return self.available(tool).then(|| "installed".to_owned());
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
        let text = if output.stdout.is_empty() {
            output.stderr
        } else {
            output.stdout
        };
        String::from_utf8_lossy(&text)
            .lines()
            .next()
            .map(|line| line.trim().to_owned())
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
            Tool::Rustfmt | Tool::Clippy => self.version(tool).is_some(),
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
}
