//! Everything the Pages flow may touch on this machine: files, the environment (by name), the
//! clock and the run directory. The flow sees it only through [`Host`], so every decision is
//! testable with a fake and the real implementation stays a thin shell.

use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::Value;

use super::super::{bundle::Scan, host as firebase_host};
use super::{
    artifact, policy,
    wrangler::{BINARY, WRANGLER_DIR},
};
use crate::{run_files, workspace::Workspace};

pub const RUNS_DIR: &str = "artifacts/cloudflare";
const FILE_LIMIT: u64 = 1024 * 1024;

/// The two files that make up a Web release; anything else in the directory is refused.
pub const ARCHIVE_SUFFIX: &str = ".tar.gz";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub id: String,
    pub path: PathBuf,
}

impl Run {
    pub fn public(&self) -> PathBuf {
        self.path.join("public")
    }

    pub fn output_file(&self) -> PathBuf {
        self.path.join("out").join("wrangler.ndjson")
    }
}

pub trait Host {
    fn now(&mut self) -> String;
    /// `Some` text of the private mapping, or `None` when the file does not exist.
    fn read_mapping(&mut self) -> Result<Option<String>, String>;
    /// `(headers, 404.html, optional _redirects)` of one app's tracked policy.
    fn read_policy(&mut self, target: &str) -> Result<policy::PolicyFiles, String>;
    /// Whether a non-empty `CLOUDFLARE_API_TOKEN` is set. The value is never read into a record.
    fn api_token_present(&self) -> bool;
    /// The ambient `CLOUDFLARE_ACCOUNT_ID`, which is not secret, so a conflict is detectable.
    fn ambient_account(&self) -> Option<String>;
    fn wrangler_binary(&self) -> PathBuf;
    fn wrangler_installed(&self) -> bool;
    /// Names of the entries in the artifact directory.
    fn list_artifact(&mut self, directory: &Path) -> Result<Vec<String>, String>;
    fn read_manifest(&mut self, directory: &Path) -> Result<String, String>;
    fn archive_facts(&mut self, path: &Path) -> Result<(String, u64), String>;
    fn create_run(&mut self) -> Result<Run, String>;
    /// Configuration that would make Wrangler build or bundle something other than the staged
    /// static directory: a config file in the run or any ancestor, `functions/`, `.env*`.
    fn wrangler_hazards(&mut self, run: &Run) -> Vec<String>;
    fn extract(
        &mut self,
        archive: &Path,
        run: &Run,
    ) -> Result<artifact::Extracted, artifact::ArtifactError>;
    fn add_file(&mut self, run: &Run, name: &str, bytes: &[u8]) -> Result<(), String>;
    fn scan_bundle(&mut self, run: &Run) -> Result<Scan, String>;
    fn hash_file(&mut self, path: &Path) -> Result<(String, u64), String>;
    fn read_output_file(&mut self, run: &Run) -> Option<String>;
    fn persist(&mut self, run: &Run, receipt: &Value) -> Result<PathBuf, String>;
    fn pause(&mut self, duration: Duration);
}

pub struct SystemHost {
    root: PathBuf,
}

impl SystemHost {
    pub fn new(workspace: &Workspace) -> Self {
        Self {
            root: workspace.root().to_path_buf(),
        }
    }
}

fn read_text(path: &Path) -> Result<Option<String>, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {}", path.display(), error.kind())),
    };
    let metadata = file
        .metadata()
        .map_err(|error| format!("{}: {}", path.display(), error.kind()))?;
    if !metadata.is_file() {
        return Err(format!("{}: not a regular file", path.display()));
    }
    let mut bytes = Vec::new();
    file.take(FILE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("{}: {}", path.display(), error.kind()))?;
    if bytes.len() as u64 > FILE_LIMIT {
        return Err(format!("{}: larger than 1 MiB", path.display()));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| format!("{}: not UTF-8", path.display()))
}

/// The tracked policy and mapping files, relative to the repository root and never the caller's
/// working directory.
pub fn read_policy_files(root: &Path, target: &str) -> Result<policy::PolicyFiles, String> {
    let directory = root.join(policy::POLICY_DIR).join(target);
    let required = |name: &str| {
        read_text(&directory.join(name))?
            .ok_or_else(|| format!("{}/{target}/{name} is missing", policy::POLICY_DIR))
    };
    Ok(policy::PolicyFiles {
        headers: required("_headers")?,
        not_found: required("404.html")?,
        redirects: read_text(&directory.join("_redirects"))?,
    })
}

impl Host for SystemHost {
    fn now(&mut self) -> String {
        run_files::now()
    }

    fn read_mapping(&mut self) -> Result<Option<String>, String> {
        read_text(&self.root.join(policy::MAPPING_FILE))
    }

    fn read_policy(&mut self, target: &str) -> Result<policy::PolicyFiles, String> {
        read_policy_files(&self.root, target)
    }

    fn api_token_present(&self) -> bool {
        std::env::var_os("CLOUDFLARE_API_TOKEN").is_some_and(|value| !value.is_empty())
    }

    fn ambient_account(&self) -> Option<String> {
        std::env::var("CLOUDFLARE_ACCOUNT_ID")
            .ok()
            .filter(|value| !value.is_empty())
    }

    fn wrangler_binary(&self) -> PathBuf {
        self.root.join(BINARY)
    }

    fn wrangler_installed(&self) -> bool {
        self.root.join(BINARY).is_file()
    }

    fn list_artifact(&mut self, directory: &Path) -> Result<Vec<String>, String> {
        let metadata = std::fs::symlink_metadata(directory)
            .map_err(|error| format!("{}: {}", directory.display(), error.kind()))?;
        if !metadata.is_dir() {
            return Err(format!(
                "{}: not a directory (and never followed through a link)",
                directory.display()
            ));
        }
        let mut names = Vec::new();
        for entry in std::fs::read_dir(directory).map_err(|error| error.kind().to_string())? {
            let entry = entry.map_err(|error| error.kind().to_string())?;
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        Ok(names)
    }

    fn read_manifest(&mut self, directory: &Path) -> Result<String, String> {
        let path = directory.join(artifact::MANIFEST_FILE);
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|error| format!("manifest.json: {}", error.kind()))?;
        if !metadata.is_file() {
            return Err("manifest.json is not a regular file".to_owned());
        }
        read_text(&path)?.ok_or_else(|| "manifest.json is missing".to_owned())
    }

    fn archive_facts(&mut self, path: &Path) -> Result<(String, u64), String> {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| format!("{}: {}", path.display(), error.kind()))?;
        if !metadata.is_file() {
            return Err(format!("{}: not a regular file", path.display()));
        }
        run_files::hash_path(path)
    }

    fn create_run(&mut self) -> Result<Run, String> {
        let run = run_files::create_run(&self.root, RUNS_DIR, &["public", "out", "smoke"])?;
        Ok(Run {
            id: run.id,
            path: run.path,
        })
    }

    fn wrangler_hazards(&mut self, run: &Run) -> Vec<String> {
        const CONFIG_NAMES: [&str; 4] = [
            "wrangler.toml",
            "wrangler.json",
            "wrangler.jsonc",
            ".wrangler",
        ];
        let mut found = Vec::new();
        for ancestor in run.path.ancestors() {
            for name in CONFIG_NAMES {
                if ancestor.join(name).exists() {
                    found.push(format!("{}/{name}", ancestor.display()));
                }
            }
        }
        for name in [
            "functions",
            ".env",
            ".dev.vars",
            "_worker.js",
            "_routes.json",
        ] {
            if run.path.join(name).exists() {
                found.push(format!("{}/{name}", run.path.display()));
            }
        }
        // `.env.*` files are read from the working directory too.
        if let Ok(entries) = std::fs::read_dir(&run.path) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(".env.") || name.starts_with(".dev.vars.") {
                    found.push(format!("{}/{name}", run.path.display()));
                }
            }
        }
        found
    }

    fn extract(
        &mut self,
        archive: &Path,
        run: &Run,
    ) -> Result<artifact::Extracted, artifact::ArtifactError> {
        let file = std::fs::File::open(archive).map_err(|error| {
            artifact::ArtifactError::Io(format!("{}: {}", archive.display(), error.kind()))
        })?;
        artifact::extract(file, &run.public())
    }

    fn add_file(&mut self, run: &Run, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = run.public().join(name);
        let mut file = run_files::create_private(&path)
            .map_err(|error| format!("{name}: {}", error.kind()))?;
        std::io::Write::write_all(&mut file, bytes)
            .map_err(|error| format!("{name}: {}", error.kind()))?;
        file.sync_all()
            .map_err(|error| format!("{name}: {}", error.kind()))
    }

    fn scan_bundle(&mut self, run: &Run) -> Result<Scan, String> {
        firebase_host::scan_directory(&run.public())
    }

    fn hash_file(&mut self, path: &Path) -> Result<(String, u64), String> {
        run_files::hash_path(path)
    }

    fn read_output_file(&mut self, run: &Run) -> Option<String> {
        read_text(&run.output_file()).ok().flatten()
    }

    fn persist(&mut self, run: &Run, receipt: &Value) -> Result<PathBuf, String> {
        let mut bytes = serde_json::to_vec_pretty(receipt).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        run_files::persist_atomic(
            &run_files::RunDir {
                id: run.id.clone(),
                path: run.path.clone(),
            },
            "receipt.json",
            &bytes,
        )
    }

    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Where the tracked lockfile that pins Wrangler lives, for messages.
pub fn wrangler_directory() -> &'static str {
    WRANGLER_DIR
}
