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
        claim: &artifact::ArchiveClaim,
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
        asset_links: read_text(&directory.join(policy::ASSET_LINKS))?,
        apple_app_site_association: read_text(&directory.join(policy::APPLE_APP_SITE_ASSOCIATION))?,
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
        claim: &artifact::ArchiveClaim,
    ) -> Result<artifact::Extracted, artifact::ArtifactError> {
        let file = std::fs::File::open(archive).map_err(|error| {
            artifact::ArtifactError::Io(format!("{}: {}", archive.display(), error.kind()))
        })?;
        artifact::extract_verified(file, &run.public(), claim)
    }

    fn add_file(&mut self, run: &Run, name: &str, bytes: &[u8]) -> Result<(), String> {
        let path = run.public().join(name);
        // Association files live under `.well-known/`, which the extracted release never has.
        // Create it here as a real directory; an existing symlink or file is refused.
        if let Some(parent) = path.parent().filter(|parent| *parent != run.public()) {
            match std::fs::symlink_metadata(parent) {
                Ok(meta) if meta.is_dir() => {}
                Ok(_) => return Err(format!("{name}: its directory is not a plain directory")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    std::fs::create_dir(parent)
                        .map_err(|error| format!("{name}: {}", error.kind()))?;
                }
                Err(error) => return Err(format!("{name}: {}", error.kind())),
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn archive(body: &[u8]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, "index.html", body)
            .unwrap();
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::none());
        encoder.write_all(&builder.into_inner().unwrap()).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn association_files_get_a_plain_well_known_directory_and_never_follow_a_link() {
        let workspace = Workspace::discover().unwrap();
        let mut host = SystemHost::new(&workspace);
        let root =
            std::env::temp_dir().join(format!("cantos-cf-well-known-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let run = Run {
            id: "well-known-regression".into(),
            path: root.clone(),
        };
        std::fs::create_dir_all(run.public()).unwrap();
        for name in [policy::ASSET_LINKS, policy::APPLE_APP_SITE_ASSOCIATION] {
            host.add_file(&run, name, b"{}").unwrap();
            assert_eq!(std::fs::read(run.public().join(name)).unwrap(), b"{}");
        }
        let meta = std::fs::symlink_metadata(run.public().join(".well-known")).unwrap();
        assert!(meta.is_dir() && !meta.file_type().is_symlink());
        // A planted link in place of the directory is refused, and nothing is written through it.
        let outside = root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::remove_dir_all(run.public().join(".well-known")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, run.public().join(".well-known")).unwrap();
            assert!(host.add_file(&run, policy::ASSET_LINKS, b"{}").is_err());
            assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn archive_replaced_after_verification_is_refused_before_bundle_validation() {
        let workspace = Workspace::discover().unwrap();
        let mut host = SystemHost::new(&workspace);
        let root =
            std::env::temp_dir().join(format!("cantos-cf-replaced-archive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let run = Run {
            id: "replacement-regression".into(),
            path: root.clone(),
        };
        std::fs::create_dir_all(run.public()).unwrap();
        let path = root.join("theatre-web.tar.gz");
        let original = archive(b"original");
        let replacement = archive(b"tampered");
        assert_eq!(original.len(), replacement.len());
        std::fs::write(&path, original).unwrap();
        let (sha256, bytes) = host.archive_facts(&path).unwrap();
        let claim = artifact::ArchiveClaim {
            path: "theatre-web.tar.gz".into(),
            sha256,
            bytes,
        };
        assert_eq!(
            artifact::verify_archive(&claim, &claim.sha256, claim.bytes),
            Ok(())
        );
        std::fs::write(&path, replacement).unwrap();

        let result = host.extract(&path, &run, &claim);
        std::fs::remove_dir_all(root).unwrap();
        assert_eq!(
            result,
            Err(artifact::ArtifactError::ArchiveMismatch(
                "SHA-256 differs from the manifest".into()
            ))
        );
    }
}
