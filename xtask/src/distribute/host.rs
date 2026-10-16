//! The local-effect boundary of a distribution run: clock, Git identity, the Android build
//! output, the private per-run directory and atomic receipt replacement. The coordinator talks to
//! the [`Host`] trait; [`SystemHost`] is the real implementation and tests use fakes.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

use super::{
    apk::{output_directory, OutputError, APK_LIMIT},
    receipt::Receipt,
    request::{Policy, NOTES_LIMIT},
};
use crate::{
    config::THEATRE_MOBILE_DIR,
    tools::{SystemTools, Tool},
    workspace::Workspace,
};

/// Where every run keeps its private files. Ignored by Git (`/artifacts/`).
pub const RUNS_DIR: &str = "artifacts/distributions";

/// Read only a regular notes file, with one extra byte to detect an oversized input.
/// A Unix FIFO must not block on open before its descriptor can be rejected.
pub fn read_notes_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "release notes must be a regular file",
        ));
    }
    read_notes_prefix(file)
}

fn read_notes_prefix(reader: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(NOTES_LIMIT + 1);
    reader.take((NOTES_LIMIT + 1) as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunDir {
    pub id: String,
    pub path: PathBuf,
}

/// A run-owned copy of the exact bytes that were inspected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    pub path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub commit: String,
    pub dirty: bool,
    pub content_fingerprint: Option<String>,
}

pub trait Host {
    /// An already-installed Android SDK, if one is configured.
    fn android_sdk(&self) -> Option<PathBuf>;
    fn now(&mut self) -> String;
    fn source_identity(&mut self) -> Result<SourceIdentity, String>;
    /// First-line versions of the installed Gradle and JDK, when they report one.
    fn tool_versions(&mut self) -> (Option<String>, Option<String>);
    /// Create a new unique private directory; never reuses or overwrites a previous run.
    fn create_run(&mut self) -> Result<RunDir, String>;
    fn read_output_metadata(&mut self, policy: Policy) -> Result<Vec<u8>, OutputError>;
    /// Copy the declared output into the run directory, hashing the bytes as they are copied.
    fn stage_output(
        &mut self,
        policy: Policy,
        run: &RunDir,
        file_name: &str,
    ) -> Result<Staged, OutputError>;
    /// Create a new private (owner-only) file in the run directory.
    fn write_private(&mut self, run: &RunDir, name: &str, bytes: &[u8]) -> Result<PathBuf, String>;
    fn hash_file(&mut self, path: &Path) -> Result<(String, u64), String>;
    /// Atomically replace the run's receipt.
    fn persist(&mut self, run: &RunDir, receipt: &Receipt) -> Result<PathBuf, String>;
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

fn compact(timestamp: &str) -> String {
    timestamp
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'T' || *c == 'Z')
        .collect()
}

pub struct SystemHost {
    root: PathBuf,
    tools: SystemTools,
}

impl SystemHost {
    pub fn new(workspace: &Workspace) -> Self {
        Self {
            root: workspace.root().to_path_buf(),
            tools: SystemTools::new(workspace.root()),
        }
    }

    fn git(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
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

    fn mobile_dir(&self) -> PathBuf {
        self.root.join(THEATRE_MOBILE_DIR)
    }
}

impl Host for SystemHost {
    fn android_sdk(&self) -> Option<PathBuf> {
        crate::theatre_demo::android_sdk_from_environment()
    }

    fn now(&mut self) -> String {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        utc_timestamp(seconds)
    }

    fn source_identity(&mut self) -> Result<SourceIdentity, String> {
        let head = String::from_utf8_lossy(&self.git(&["rev-parse", "HEAD"])?)
            .trim()
            .to_owned();
        if head.len() < 40 || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("git did not report a commit".to_owned());
        }
        let status = self.git(&["status", "--porcelain=v1", "-z", "--untracked-files=all"])?;
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
        digest.update(self.git(&["diff", "HEAD", "--binary", "--no-ext-diff"])?);
        digest.update(b"\0untracked\0");
        let listing = self.git(&["ls-files", "--others", "--exclude-standard", "-z"])?;
        for name in listing
            .split(|byte| *byte == 0)
            .filter(|name| !name.is_empty())
        {
            let relative = String::from_utf8_lossy(name).into_owned();
            digest.update(name);
            digest.update([0]);
            let path = self.root.join(&relative);
            if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file()) {
                digest.update(
                    self.hash_file(&path)
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

    fn tool_versions(&mut self) -> (Option<String>, Option<String>) {
        (
            self.tools.version(Tool::Gradle),
            self.tools.version(Tool::Java),
        )
    }

    fn create_run(&mut self) -> Result<RunDir, String> {
        let base = self.root.join(RUNS_DIR);
        std::fs::create_dir_all(&base)
            .map_err(|error| format!("{}: {}", RUNS_DIR, error.kind()))?;
        let stamp = compact(&self.now());
        for attempt in 0..32u32 {
            let id = format!("{stamp}-{}-{attempt}", std::process::id());
            let path = base.join(&id);
            match private_dir(&path) {
                Ok(()) => {
                    private_dir(&path.join("artifact"))
                        .map_err(|error| format!("{}: {}", path.display(), error.kind()))?;
                    return Ok(RunDir { id, path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("{}: {}", path.display(), error.kind())),
            }
        }
        Err("could not allocate a unique run directory".to_owned())
    }

    fn read_output_metadata(&mut self, policy: Policy) -> Result<Vec<u8>, OutputError> {
        let path = output_directory(&self.mobile_dir(), policy).join("output-metadata.json");
        match std::fs::read(&path) {
            Ok(bytes) => Ok(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(OutputError::MetadataMissing)
            }
            Err(error) => Err(OutputError::MetadataUnreadable(error.kind().to_string())),
        }
    }

    fn stage_output(
        &mut self,
        policy: Policy,
        run: &RunDir,
        file_name: &str,
    ) -> Result<Staged, OutputError> {
        let source = output_directory(&self.mobile_dir(), policy).join(file_name);
        let meta = match std::fs::symlink_metadata(&source) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(OutputError::FileMissing(file_name.to_owned()))
            }
            Err(error) => return Err(OutputError::Io(error.kind().to_string())),
        };
        if !meta.is_file() {
            return Err(OutputError::NotRegularFile(file_name.to_owned()));
        }
        if meta.len() == 0 {
            return Err(OutputError::Empty(file_name.to_owned()));
        }
        if meta.len() > APK_LIMIT {
            return Err(OutputError::TooLarge(meta.len()));
        }
        let destination = run.path.join("artifact").join(file_name);
        let io = |error: std::io::Error| OutputError::Io(error.kind().to_string());
        let mut input = std::fs::File::open(&source).map_err(io)?;
        let mut output = create_private(&destination).map_err(io)?;
        let mut digest = Sha256::new();
        let mut buffer = vec![0u8; 1 << 16];
        let mut size = 0u64;
        loop {
            let read = input.read(&mut buffer).map_err(io)?;
            if read == 0 {
                break;
            }
            size += read as u64;
            if size > APK_LIMIT {
                return Err(OutputError::TooLarge(size));
            }
            digest.update(&buffer[..read]);
            output.write_all(&buffer[..read]).map_err(io)?;
        }
        output.sync_all().map_err(io)?;
        drop(output);
        make_read_only(&destination).map_err(io)?;
        Ok(Staged {
            path: destination,
            sha256: hex(&digest.finalize()),
            size_bytes: size,
        })
    }

    fn write_private(&mut self, run: &RunDir, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        let path = run.path.join(name);
        let mut file =
            create_private(&path).map_err(|error| format!("{name}: {}", error.kind()))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("{name}: {}", error.kind()))?;
        Ok(path)
    }

    fn hash_file(&mut self, path: &Path) -> Result<(String, u64), String> {
        hash_path(path)
    }

    fn persist(&mut self, run: &RunDir, receipt: &Receipt) -> Result<PathBuf, String> {
        let temporary = run.path.join(".receipt.json.tmp");
        let target = run.path.join("receipt.json");
        let write = || -> std::io::Result<()> {
            let mut file = open_private(&temporary, true)?;
            file.write_all(&receipt.to_json())?;
            file.sync_all()?;
            drop(file);
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
}

fn hash_path(path: &Path) -> Result<(String, u64), String> {
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

#[cfg(unix)]
fn private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn private_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir(path)
}

fn open_private(path: &Path, truncate: bool) -> std::io::Result<std::fs::File> {
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

fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    open_private(path, false)
}

fn make_read_only(path: &Path) -> std::io::Result<()> {
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
    use crate::distribute::request::{AppId, AppIdSource, LiveRequest, NotesChoice, Recipients};

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-distribute-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A host over a throwaway tree, with the mobile output directory pre-populated.
    fn host(root: &Path) -> SystemHost {
        SystemHost {
            root: root.to_path_buf(),
            tools: SystemTools::new(root),
        }
    }

    fn output_dir(root: &Path) -> PathBuf {
        let dir = output_directory(&root.join(THEATRE_MOBILE_DIR), Policy::DemoApk);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn live() -> LiveRequest {
        LiveRequest {
            policy: Policy::DemoApk,
            app_id: AppId::parse("1:000000000000:android:0123456789abcdef").unwrap(),
            app_id_source: AppIdSource::Flag,
            recipients: Recipients::new(&["qa".to_owned()], &[]).unwrap(),
            notes: NotesChoice::Generated,
            expected_cert: None,
        }
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
    fn notes_reads_stop_after_the_oversize_sentinel_even_without_eof() {
        let mut reader = std::io::repeat(b'x');
        assert_eq!(
            read_notes_prefix(&mut reader).unwrap(),
            vec![b'x'; NOTES_LIMIT + 1]
        );
        let mut cursor = std::io::Cursor::new(vec![b'x'; NOTES_LIMIT * 4]);
        assert_eq!(read_notes_prefix(&mut cursor).unwrap().len(), NOTES_LIMIT + 1);
        assert_eq!(cursor.position(), (NOTES_LIMIT + 1) as u64);
    }

    #[test]
    fn notes_file_reads_preserve_the_limit_and_reject_non_regular_inputs() {
        let root = temp("notes");
        let path = root.join("notes.txt");
        for size in [NOTES_LIMIT, NOTES_LIMIT + 1, NOTES_LIMIT * 4] {
            std::fs::write(&path, vec![b'x'; size]).unwrap();
            assert_eq!(
                read_notes_file(&path).unwrap(),
                vec![b'x'; size.min(NOTES_LIMIT + 1)]
            );
        }
        assert!(read_notes_file(&root).is_err());
        #[cfg(unix)]
        {
            let link = root.join("notes-link.txt");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert_eq!(read_notes_file(&link).unwrap(), vec![b'x'; NOTES_LIMIT + 1]);
            assert_eq!(
                read_notes_file(Path::new("/dev/zero")).unwrap_err().kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn notes_fifo_is_rejected_without_waiting_for_a_writer() {
        use std::{
            ffi::CString,
            os::unix::ffi::OsStrExt,
            process::Child,
            thread,
            time::{Duration, Instant},
        };

        const CHILD_ROOT: &str = "CANTOS_DISTRIBUTE_NOTES_FIFO_TEST_ROOT";
        const TEST_NAME: &str =
            "distribute::host::tests::notes_fifo_is_rejected_without_waiting_for_a_writer";
        if let Some(root) = std::env::var_os(CHILD_ROOT) {
            let root = PathBuf::from(root);
            assert_eq!(
                read_notes_file(&root.join("notes.fifo")).unwrap_err().kind(),
                std::io::ErrorKind::InvalidInput
            );
            std::fs::write(root.join("rejected"), b"InvalidInput").unwrap();
            return;
        }

        struct FifoReader {
            root: PathBuf,
            child: Option<Child>,
        }
        impl Drop for FifoReader {
            fn drop(&mut self) {
                if let Some(child) = &mut self.child {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }

        let mut reader = FifoReader {
            root: temp("notes-fifo"),
            child: None,
        };
        let path = reader.root.join("notes.fifo");
        let name = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        reader.child = Some(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST_NAME, "--nocapture"])
                .env(CHILD_ROOT, &reader.root)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = reader.child.as_mut().unwrap().try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "notes FIFO reader waited for a writer"
            );
            thread::sleep(Duration::from_millis(10));
        };
        reader.child = None;
        assert!(status.success(), "notes FIFO reader failed: {status}");
        assert_eq!(
            std::fs::read(reader.root.join("rejected")).unwrap(),
            b"InvalidInput"
        );
    }

    #[test]
    fn runs_get_unique_private_directories_and_never_share_a_receipt() {
        let root = temp("runs");
        let mut host = host(&root);
        let first = host.create_run().unwrap();
        let second = host.create_run().unwrap();
        assert_ne!(first.id, second.id);
        assert_ne!(first.path, second.path);
        assert!(first.path.starts_with(root.join(RUNS_DIR)));
        assert!(first.path.join("artifact").is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&first.path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
        let mut one = Receipt::new(&first.id, "t", &live());
        one.phase = "complete";
        host.persist(&first, &one).unwrap();
        assert!(first.path.join("receipt.json").is_file());
        assert!(!second.path.join("receipt.json").exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn receipt_replacement_is_atomic_and_leaves_no_temporary_file() {
        let root = temp("atomic");
        let mut host = host(&root);
        let run = host.create_run().unwrap();
        let mut receipt = Receipt::new(&run.id, "t", &live());
        host.persist(&run, &receipt).unwrap();
        receipt.mark_intent();
        let path = host.persist(&run, &receipt).unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("\"phase\": \"intent\""));
        assert!(!run.path.join(".receipt.json.tmp").exists());
        // A removed run directory makes persistence fail with a typed message, not a panic.
        std::fs::remove_dir_all(&run.path).unwrap();
        assert!(host.persist(&run, &receipt).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn staging_copies_exact_bytes_read_only_and_hashes_them() {
        let root = temp("stage");
        let dir = output_dir(&root);
        std::fs::write(dir.join("a b.apk"), b"PK-demo-bytes").unwrap();
        let mut host = host(&root);
        let run = host.create_run().unwrap();
        let staged = host.stage_output(Policy::DemoApk, &run, "a b.apk").unwrap();
        assert_eq!(staged.size_bytes, 13);
        assert_eq!(staged.sha256, sha256_hex(b"PK-demo-bytes"));
        assert_eq!(std::fs::read(&staged.path).unwrap(), b"PK-demo-bytes");
        assert!(staged.path.starts_with(&run.path));
        assert!(std::fs::metadata(&staged.path)
            .unwrap()
            .permissions()
            .readonly());
        assert_eq!(host.hash_file(&staged.path).unwrap().0, staged.sha256);
        // Changing the source afterwards cannot alter the inspected copy.
        std::fs::write(dir.join("a b.apk"), b"tampered").unwrap();
        assert_eq!(host.hash_file(&staged.path).unwrap().0, staged.sha256);
        // A second stage of the same name in the same run is refused, not overwritten.
        assert!(host.stage_output(Policy::DemoApk, &run, "a b.apk").is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn staging_rejects_missing_empty_and_non_regular_outputs() {
        let root = temp("stage-bad");
        let dir = output_dir(&root);
        std::fs::write(dir.join("empty.apk"), b"").unwrap();
        std::fs::create_dir(dir.join("dir.apk")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("empty.apk"), dir.join("link.apk")).unwrap();
        let mut host = host(&root);
        let run = host.create_run().unwrap();
        assert_eq!(
            host.stage_output(Policy::DemoApk, &run, "missing.apk"),
            Err(OutputError::FileMissing("missing.apk".into()))
        );
        assert_eq!(
            host.stage_output(Policy::DemoApk, &run, "empty.apk"),
            Err(OutputError::Empty("empty.apk".into()))
        );
        assert_eq!(
            host.stage_output(Policy::DemoApk, &run, "dir.apk"),
            Err(OutputError::NotRegularFile("dir.apk".into()))
        );
        #[cfg(unix)]
        assert_eq!(
            host.stage_output(Policy::DemoApk, &run, "link.apk"),
            Err(OutputError::NotRegularFile("link.apk".into()))
        );
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn output_metadata_distinguishes_absent_from_unreadable() {
        let root = temp("metadata");
        let mut host = host(&root);
        assert_eq!(
            host.read_output_metadata(Policy::DemoApk),
            Err(OutputError::MetadataMissing)
        );
        let dir = output_dir(&root);
        std::fs::write(dir.join("output-metadata.json"), b"{}").unwrap();
        assert_eq!(host.read_output_metadata(Policy::DemoApk).unwrap(), b"{}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn private_files_are_created_once_with_owner_only_access() {
        let root = temp("private");
        let mut host = host(&root);
        let run = host.create_run().unwrap();
        let path = host
            .write_private(&run, "testers.txt", b"x@example.com\n")
            .unwrap();
        assert!(host.write_private(&run, "testers.txt", b"y").is_err());
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
        let mut host = SystemHost::new(&workspace);
        if let Ok(identity) = host.source_identity() {
            assert_eq!(identity.commit.len(), 40);
            assert_eq!(identity.dirty, identity.content_fingerprint.is_some());
        }
    }
}
