//! Command execution abstraction. Plans are built from immutable [`CommandSpec`] values and
//! executed through a [`Runner`], so planning is testable without starting processes.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Context, Result};

/// One external program invocation with an explicit working directory and environment delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    /// Inherited variables the child must not see.
    pub env_remove: Vec<String>,
    /// Keys of `env` whose values are credentials: shown as `<redacted>` in every rendering.
    pub secret_keys: Vec<String>,
}

impl CommandSpec {
    pub fn new(program: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: cwd.into(),
            env: Vec::new(),
            env_remove: Vec::new(),
            secret_keys: Vec::new(),
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn arg_if(self, condition: bool, arg: &str) -> Self {
        if condition {
            self.args([arg])
        } else {
            self
        }
    }

    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// A variable carrying a credential: passed to the child like `env`, but never rendered.
    pub fn env_secret(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        self.secret_keys.push(key.clone());
        self.env.push((key, value.into()));
        self
    }

    /// Hide an inherited variable from the child, for settings that must never come from the
    /// ambient environment.
    pub fn unset(mut self, key: impl Into<String>) -> Self {
        self.env_remove.push(key.into());
        self
    }

    /// Shell-like rendering for logs and plans: `(in dir) [env -u KEY] KEY=value program args`.
    pub fn display(&self, root: &Path) -> String {
        let mut parts = Vec::new();
        if self.cwd != root {
            let dir = self
                .cwd
                .strip_prefix(root)
                .unwrap_or(&self.cwd)
                .display()
                .to_string();
            parts.push(format!("(in {dir})"));
        }
        if !self.env_remove.is_empty() {
            parts.push("env".to_owned());
            parts.extend(self.env_remove.iter().map(|key| format!("-u {key}")));
        }
        for (key, value) in &self.env {
            if self.secret_keys.contains(key) {
                parts.push(format!("{key}=<redacted>"));
            } else {
                parts.push(format!("{key}={}", quote(value)));
            }
        }
        let program = Path::new(&self.program)
            .strip_prefix(root)
            .map(|relative| relative.display().to_string())
            .unwrap_or_else(|_| self.program.clone());
        parts.push(program);
        parts.extend(self.args.iter().map(|arg| quote(arg)));
        parts.join(" ")
    }

    pub fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args).current_dir(&self.cwd);
        for key in &self.env_remove {
            command.env_remove(key);
        }
        for (key, value) in &self.env {
            command.env(key, value);
        }
        command
    }
}

fn quote(value: &str) -> String {
    let plain = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@+,%".contains(c));
    if plain {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', r"'\''"))
    }
}

/// Executes a command to completion; implementations decide how output is handled.
pub trait Runner {
    fn run(&mut self, spec: &CommandSpec) -> Result<()>;
}

/// Runs commands with inherited stdio, in the caller's process group, so Ctrl-C stops both.
pub struct SystemRunner {
    root: PathBuf,
}

impl SystemRunner {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl Runner for SystemRunner {
    fn run(&mut self, spec: &CommandSpec) -> Result<()> {
        let shown = spec.display(&self.root);
        println!("    $ {shown}");
        let status = spec
            .to_command()
            .status()
            .with_context(|| format!("could not start `{}`", spec.program))?;
        if status.success() {
            Ok(())
        } else {
            bail!("`{shown}` exited with {status}")
        }
    }
}

#[cfg(test)]
pub mod testing {
    use super::*;

    /// Records commands instead of running them; fails the first command matching `fail_on`.
    #[derive(Default)]
    pub struct RecordingRunner {
        pub commands: Vec<CommandSpec>,
        pub fail_on: Option<String>,
    }

    impl Runner for RecordingRunner {
        fn run(&mut self, spec: &CommandSpec) -> Result<()> {
            self.commands.push(spec.clone());
            let line = format!("{} {}", spec.program, spec.args.join(" "));
            match &self.fail_on {
                Some(pattern) if line.contains(pattern.as_str()) => bail!("simulated failure"),
                _ => Ok(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_shows_directory_environment_and_quotes() {
        let root = Path::new("/repo");
        let spec = CommandSpec::new("trunk", "/repo/apps/cantos-theatre/web")
            .env("NO_COLOR", "true")
            .args(["build", "--locked"])
            .arg_if(true, "--release")
            .arg_if(false, "--never");
        assert_eq!(
            spec.display(root),
            "(in apps/cantos-theatre/web) NO_COLOR=true trunk build --locked --release"
        );
        let python = CommandSpec::new("python3", "/repo").args(["-p", "test_*.py", "it's"]);
        assert_eq!(python.display(root), r"python3 -p 'test_*.py' 'it'\''s'");
        let binary = CommandSpec::new("/repo/target/debug/cantos-server", "/repo");
        assert_eq!(binary.display(root), "target/debug/cantos-server");
    }

    #[test]
    fn secret_environment_reaches_the_child_but_is_never_rendered() {
        let spec = CommandSpec::new("cargo", "/repo")
            .env("PLAIN", "visible")
            .env_secret("TOKEN", "do-not-print");
        let shown = spec.display(Path::new("/repo"));
        assert_eq!(shown, "PLAIN=visible TOKEN=<redacted> cargo");
        assert!(!format!("{shown} {:?}", spec.secret_keys).contains("do-not-print"));
        assert!(spec
            .env
            .iter()
            .any(|(key, value)| key == "TOKEN" && value == "do-not-print"));
    }

    #[test]
    fn system_runner_reports_the_exit_status() {
        let root = std::env::temp_dir();
        let mut runner = SystemRunner::new(&root);
        runner
            .run(&CommandSpec::new("sh", &root).args(["-c", "exit 0"]))
            .unwrap();
        let error = runner
            .run(&CommandSpec::new("sh", &root).args(["-c", "exit 3"]))
            .unwrap_err();
        assert!(error.to_string().contains("exit"), "{error}");
        let missing = runner
            .run(&CommandSpec::new("cantos-no-such-program", &root))
            .unwrap_err();
        assert!(missing.to_string().contains("could not start"), "{missing}");
    }
}
