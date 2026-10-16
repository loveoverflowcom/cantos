//! Workspace root discovery. Every command resolves paths from this root, never from the
//! caller's current directory.

use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};

#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// The root is the parent of this crate's manifest directory, fixed at compile time, so
    /// `cargo xtask` behaves the same from any subdirectory.
    pub fn discover() -> Result<Self> {
        Self::from_xtask_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
    }

    fn from_xtask_dir(xtask_dir: &Path) -> Result<Self> {
        let root = xtask_dir
            .parent()
            .context("the xtask crate must live directly inside the workspace root")?;
        let manifest = root.join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .with_context(|| format!("could not read {}", manifest.display()))?;
        ensure!(
            text.lines().any(|line| line.trim() == "[workspace]"),
            "{} is not a Cargo workspace manifest",
            manifest.display()
        );
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    #[cfg(test)]
    pub(crate) fn fixture(root: &Path) -> Result<Self> {
        Self::from_xtask_dir(&root.join("xtask"))
    }

    /// Repository-relative display form of `path`, or the path itself when outside the root.
    pub fn relative(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(relative) if relative.as_os_str().is_empty() => ".".to_owned(),
            Ok(relative) => relative.display().to_string(),
            Err(_) => path.display().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_the_cantos_root_independently_of_the_current_directory() {
        let workspace = Workspace::discover().unwrap();
        assert!(workspace.path("services/cantos-api/Cargo.toml").is_file());
        assert!(workspace.path("xtask/Cargo.toml").is_file());
        assert_eq!(workspace.relative(workspace.root()), ".");
        assert_eq!(
            workspace.relative(&workspace.path("apps/cantos-theatre/web")),
            "apps/cantos-theatre/web"
        );
    }

    #[test]
    fn rejects_a_directory_whose_parent_is_not_a_workspace() {
        let error = Workspace::from_xtask_dir(Path::new("/")).unwrap_err();
        assert!(error.to_string().contains("workspace root"), "{error}");
    }
}
