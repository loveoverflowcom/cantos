//! Local run files shared by the explicit remote-write commands (`distribute`, `deploy`): the
//! clock, SHA-256, owner-only per-run directories under the ignored `artifacts/` tree, atomic
//! record replacement and the Git source identity. Nothing here knows about Android, Firebase or
//! Hosting; the callers own what a run records and what it may reach.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunDir {
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub commit: String,
    pub dirty: bool,
    pub content_fingerprint: Option<String>,
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// RFC 3339 UTC timestamp for Unix seconds (proleptic Gregorian, no leap seconds).
pub fn utc_timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let remainder = seconds % 86_400;
    // Howard Hinnant's civil-from-days.
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        remainder / 3_600,
        remainder % 3_600 / 60,
        remainder % 60
    )
}

pub fn now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    utc_timestamp(seconds)
}

fn compact(timestamp: &str) -> String {
    timestamp
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T' || *c == 'Z')
        .collect()
}

/// Create a new unique private directory `<root>/<base>/<run id>` (plus the named private
/// subdirectories); never reuses or overwrites a previous run.
pub fn create_run(root: &Path, base: &str, subdirectories: &[&str]) -> Result<RunDir, String> {
    let base_dir = root.join(base);
    std::fs::create_dir_all(&base_dir).map_err(|error| format!("{base}: {}", error.kind()))?;
    let stamp = compact(&now());
    for attempt in 0..32u32 {
        let id = format!("{stamp}-{}-{attempt}", std::process::id());
        let path = base_dir.join(&id);
        match private_dir(&path) {
            Ok(()) => {
                for name in subdirectories {
                    private_dir(&path.join(name))
                        .map_err(|error| format!("{}: {}", path.display(), error.kind()))?;
                }
                return Ok(RunDir { id, path });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("{}: {}", path.display(), error.kind())),
        }
    }
    Err("could not allocate a unique run directory".to_owned())
}

/// Create a new private (owner-only) file in the run directory; an existing name is refused.
pub fn write_private(run: &RunDir, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let path = run.path.join(name);
    let mut file = create_private(&path).map_err(|error| format!("{name}: {}", error.kind()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("{name}: {}", error.kind()))?;
    Ok(path)
}

/// Atomically replace `<run>/<file>` through a private temporary sibling.
pub fn persist_atomic(run: &RunDir, file: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let temporary = run.path.join(format!(".{file}.tmp"));
    let target = run.path.join(file);
    let write = || -> std::io::Result<()> {
        let mut output = open_private(&temporary, true)?;
        output.write_all(bytes)?;
        output.sync_all()?;
        drop(output);
        std::fs::rename(&temporary, &target)?;
        // Make the rename durable where the platform allows opening a directory.
        if let Ok(directory) = std::fs::File::open(&run.path) {
            let _ = directory.sync_all();
        }
        Ok(())
    };
    write().map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!("{}: {}", target.display(), error.kind())
    })?;
    Ok(target)
}

pub fn hash_path(path: &Path) -> Result<(String, u64), String> {
    let mut file = std::fs::File::open(path).map_err(|error| error.kind().to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut size = 0u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| error.kind().to_string())?;
        if read == 0 {
            return Ok((hex(&digest.finalize()), size));
        }
        size += read as u64;
        digest.update(&buffer[..read]);
    }
}

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("could not run git: {}", error.kind()))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!("`git {}` failed", args.join(" ")))
    }
}

/// The checked-out commit, and for a dirty tree a digest of its status, tracked diff and
/// untracked files, so two dirty builds of different content never share an identity.
pub fn source_identity(root: &Path) -> Result<SourceIdentity, String> {
    let head = String::from_utf8_lossy(&git(root, &["rev-parse", "HEAD"])?)
        .trim()
        .to_owned();
    if head.len() < 40 || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("git did not report a commit".to_owned());
    }
    let status = git(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    if status.is_empty() {
        return Ok(SourceIdentity {
            commit: head,
            dirty: false,
            content_fingerprint: None,
        });
    }
    let mut digest = Sha256::new();
    digest.update(b"status\0");
    digest.update(&status);
    digest.update(b"\0diff\0");
    digest.update(git(root, &["diff", "HEAD", "--binary", "--no-ext-diff"])?);
    digest.update(b"\0untracked\0");
    let listing = git(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    for name in listing
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        let relative = String::from_utf8_lossy(name).into_owned();
        digest.update(name);
        digest.update([0]);
        let path = root.join(&relative);
        if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file()) {
            digest.update(
                hash_path(&path)
                    .map_err(|why| format!("could not fingerprint {relative}: {why}"))?
                    .0,
            );
        }
        digest.update([0]);
    }
    Ok(SourceIdentity {
        commit: head,
        dirty: true,
        content_fingerprint: Some(hex(&digest.finalize())),
    })
}

#[cfg(unix)]
pub fn private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
pub fn private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir(path)
}

pub fn open_private(path: &Path, truncate: bool) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    if truncate {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

pub fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    open_private(path, false)
}

pub fn make_read_only(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o400))
    }
    #[cfg(not(unix))]
    {
        let mut permissions = std::fs::metadata(path)?.permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(path, permissions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Workspace;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-run-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn timestamps_are_rfc3339_utc() {
        assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_timestamp(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_timestamp(1_781_000_000), "2026-06-09T10:13:20Z");
        assert_eq!(utc_timestamp(1_791_935_999), "2026-10-13T23:59:59Z");
        assert_eq!(compact("2026-10-15T01:02:03Z"), "20261015T010203Z");
    }

    #[test]
    fn digests_are_standard_sha256() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn runs_get_unique_private_directories_under_the_requested_base() {
        let root = temp("runs");
        let first = create_run(&root, "artifacts/example", &["artifact"]).unwrap();
        let second = create_run(&root, "artifacts/example", &["artifact"]).unwrap();
        assert_ne!(first.id, second.id);
        assert_ne!(first.path, second.path);
        assert!(first.path.starts_with(root.join("artifacts/example")));
        assert!(first.path.join("artifact").is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&first.path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
        persist_atomic(&first, "receipt.json", b"{}").unwrap();
        assert!(first.path.join("receipt.json").is_file());
        assert!(!second.path.join("receipt.json").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn replacement_is_atomic_and_leaves_no_temporary_file() {
        let root = temp("atomic");
        let run = create_run(&root, "artifacts/example", &[]).unwrap();
        persist_atomic(&run, "receipt.json", b"one").unwrap();
        let path = persist_atomic(&run, "receipt.json", b"two").unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"two");
        assert!(!run.path.join(".receipt.json.tmp").exists());
        // A removed run directory makes persistence fail with a typed message, not a panic.
        std::fs::remove_dir_all(&run.path).unwrap();
        assert!(persist_atomic(&run, "receipt.json", b"three").is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn private_files_are_created_once_with_owner_only_access() {
        let root = temp("private");
        let run = create_run(&root, "artifacts/example", &[]).unwrap();
        let path = write_private(&run, "testers.txt", b"x@example.com\n").unwrap();
        assert!(write_private(&run, "testers.txt", b"y").is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn source_identity_of_this_checkout_is_a_commit_with_a_dirty_indicator() {
        let workspace = Workspace::discover().unwrap();
        if let Ok(identity) = source_identity(workspace.root()) {
            assert_eq!(identity.commit.len(), 40);
            assert_eq!(identity.dirty, identity.content_fingerprint.is_some());
        }
    }
}
