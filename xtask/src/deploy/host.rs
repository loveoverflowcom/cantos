//! The local-effect boundary of a deploy run: the clock, Git identity, the configuration files,
//! the private per-run directory, the scan of the built bundle and atomic receipt replacement. The
//! coordinator talks to the [`Host`] trait; [`SystemHost`] is the real implementation and tests use
//! fakes. Neutral file, digest and Git primitives live in [`crate::run_files`].

use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use sha2::{Digest, Sha256, Sha384};

use super::{
    bundle::{
        base64, FileFacts, Irregular, Irregularity, Scan, HEAD_BYTES, INDEX_LIMIT, MAX_FILES,
    },
    hosting_config::{CONFIG_DIR, FIREBASERC, FIREBASE_JSON},
    receipt::Receipt,
};
use crate::{
    run_files::{self, hex, RunDir, SourceIdentity},
    tools::{SystemTools, Tool},
    workspace::Workspace,
};

/// Where every run keeps its private files. Ignored by Git (`/artifacts/`).
pub const RUNS_DIR: &str = "artifacts/hosting";
/// A configuration file larger than this is not a Firebase configuration.
const CONFIG_LIMIT: u64 = 1024 * 1024;
const MAX_DEPTH: usize = 16;

/// The tracked policy text and, when present, the operator's `.firebaserc` text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFiles {
    pub policy: String,
    pub firebaserc: Option<String>,
}

/// Read the configuration beside `infra/firebase/hosting`. Reading changes nothing.
pub fn read_config_files(workspace: &Workspace) -> Result<ConfigFiles, String> {
    let directory = workspace.path(CONFIG_DIR);
    let policy = read_text(&directory.join(FIREBASE_JSON))
        .map_err(|why| format!("{CONFIG_DIR}/{FIREBASE_JSON}: {why}"))?
        .ok_or_else(|| format!("{CONFIG_DIR}/{FIREBASE_JSON} does not exist"))?;
    let firebaserc = read_text(&directory.join(FIREBASERC))
        .map_err(|why| format!("{CONFIG_DIR}/{FIREBASERC}: {why}"))?;
    Ok(ConfigFiles { policy, firebaserc })
}

/// `Ok(None)` for an absent file; a bounded, UTF-8 read otherwise.
fn read_text(path: &Path) -> Result<Option<String>, String> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.kind().to_string()),
    };
    if !file.metadata().map_err(|e| e.kind().to_string())?.is_file() {
        return Err("not a regular file".to_owned());
    }
    let mut bytes = Vec::new();
    file.take(CONFIG_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.kind().to_string())?;
    if bytes.len() as u64 > CONFIG_LIMIT {
        return Err("larger than 1 MiB".to_owned());
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "not UTF-8".to_owned())
}

pub trait Host {
    fn now(&mut self) -> String;
    fn source_identity(&mut self) -> Result<SourceIdentity, String>;
    /// Names (never values) of inherited `TRUNK_*` environment variables, which could redirect
    /// the build.
    fn trunk_overrides(&self) -> Vec<String>;
    /// Raw nonsecret build input. The coordinator requires and validates it before any run.
    fn api_origin(&self) -> Result<Option<String>, String>;
    fn read_config(&mut self) -> Result<ConfigFiles, String>;
    /// First-line versions of the installed Cargo and Trunk, when they report one.
    fn tool_versions(&mut self) -> (Option<String>, Option<String>);
    /// Create a new unique private directory; never reuses or overwrites a previous run.
    fn create_run(&mut self) -> Result<RunDir, String>;
    /// Create a new private (owner-only) file in the run directory.
    fn write_private(&mut self, run: &RunDir, name: &str, bytes: &[u8]) -> Result<PathBuf, String>;
    /// Walk the built bundle once, never following a link, and describe every entry.
    fn scan_bundle(&mut self, directory: &Path) -> Result<Scan, String>;
    fn hash_file(&mut self, path: &Path) -> Result<(String, u64), String>;
    /// Atomically replace the run's receipt.
    fn persist(&mut self, run: &RunDir, receipt: &Receipt) -> Result<PathBuf, String>;
    /// Wait between read-only smoke attempts.
    fn pause(&mut self, duration: Duration);
}

pub struct SystemHost {
    root: PathBuf,
    workspace: Workspace,
    tools: SystemTools,
}

impl SystemHost {
    pub fn new(workspace: &Workspace) -> Self {
        Self {
            root: workspace.root().to_path_buf(),
            workspace: workspace.clone(),
            tools: SystemTools::new(workspace.root()),
        }
    }
}

impl Host for SystemHost {
    fn now(&mut self) -> String {
        run_files::now()
    }

    fn source_identity(&mut self) -> Result<SourceIdentity, String> {
        run_files::source_identity(&self.root)
    }

    fn trunk_overrides(&self) -> Vec<String> {
        trunk_override_names(std::env::vars_os().map(|(name, _)| name))
    }

    fn api_origin(&self) -> Result<Option<String>, String> {
        match std::env::var("CANTOS_API_ORIGIN") {
            Ok(value) => Ok(Some(value)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(_) => Err("invalid CANTOS_API_ORIGIN".into()),
        }
    }

    fn read_config(&mut self) -> Result<ConfigFiles, String> {
        read_config_files(&self.workspace)
    }

    fn tool_versions(&mut self) -> (Option<String>, Option<String>) {
        (
            self.tools.version(Tool::Cargo),
            self.tools.version(Tool::Trunk),
        )
    }

    fn create_run(&mut self) -> Result<RunDir, String> {
        run_files::create_run(&self.root, RUNS_DIR, &["smoke"])
    }

    fn write_private(&mut self, run: &RunDir, name: &str, bytes: &[u8]) -> Result<PathBuf, String> {
        run_files::write_private(run, name, bytes)
    }

    fn scan_bundle(&mut self, directory: &Path) -> Result<Scan, String> {
        scan_directory(directory)
    }

    fn hash_file(&mut self, path: &Path) -> Result<(String, u64), String> {
        run_files::hash_path(path)
    }

    fn persist(&mut self, run: &RunDir, receipt: &Receipt) -> Result<PathBuf, String> {
        run_files::persist_atomic(run, "receipt.json", &receipt.to_json())
    }

    fn pause(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// The `TRUNK_*` variables among `names`, sorted. Trunk binds its options to such variables
/// (`TRUNK_BUILD_DIST`, `TRUNK_BUILD_PUBLIC_URL`, ...), so one set in the caller's shell could
/// redirect where the bundle is written or how it is built.
pub fn trunk_override_names(names: impl Iterator<Item = std::ffi::OsString>) -> Vec<String> {
    let mut found: Vec<String> = names
        .filter_map(|name| name.into_string().ok())
        .filter(|name| name.starts_with("TRUNK_"))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Walk `root` and describe every entry. Symbolic links, devices, FIFOs and unreadable entries are
/// recorded as irregular rather than followed or skipped, and files are opened without following a
/// link so that a swap after the check cannot redirect the read.
pub fn scan_directory(root: &Path) -> Result<Scan, String> {
    let meta = std::fs::symlink_metadata(root).map_err(|error| {
        format!(
            "the build output {} is unreadable ({})",
            root.display(),
            error.kind()
        )
    })?;
    if !meta.is_dir() {
        return Err(format!(
            "the build output {} is not a directory",
            root.display()
        ));
    }
    let mut scan = Scan::default();
    walk(root, "", 0, &mut scan);
    scan.files.sort_by(|a, b| a.path.cmp(&b.path));
    scan.irregular.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(scan)
}

fn walk(directory: &Path, prefix: &str, depth: usize, scan: &mut Scan) {
    let irregular = |scan: &mut Scan, path: String, kind: Irregular| {
        scan.irregular.push(Irregularity { path, kind });
    };
    if depth > MAX_DEPTH {
        irregular(
            scan,
            prefix.trim_end_matches('/').to_owned(),
            Irregular::TooDeep,
        );
        return;
    }
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            irregular(
                scan,
                prefix.trim_end_matches('/').to_owned(),
                Irregular::Unreadable(error.kind().to_string()),
            );
            return;
        }
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if scan.files.len() + scan.irregular.len() > MAX_FILES {
            scan.truncated = true;
            return;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            irregular(
                scan,
                format!("{prefix}{}", name.to_string_lossy()),
                Irregular::NonUtf8Name,
            );
            continue;
        };
        let path = format!("{prefix}{name}");
        let full = entry.path();
        let meta = match std::fs::symlink_metadata(&full) {
            Ok(meta) => meta,
            Err(error) => {
                irregular(scan, path, Irregular::Unreadable(error.kind().to_string()));
                continue;
            }
        };
        let kind = meta.file_type();
        if kind.is_symlink() {
            irregular(scan, path, Irregular::Symlink);
        } else if kind.is_dir() {
            walk(&full, &format!("{path}/"), depth + 1, scan);
        } else if kind.is_file() {
            match read_file(&full, &path, scan) {
                Ok(facts) => scan.files.push(facts),
                Err(kind) => irregular(scan, path, kind),
            }
        } else {
            irregular(scan, path, Irregular::Special);
        }
    }
}

fn open_without_following(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A FIFO swapped in after the metadata check must not block the open.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    options.open(path)
}

fn read_file(path: &Path, relative: &str, scan: &mut Scan) -> Result<FileFacts, Irregular> {
    let unreadable = |error: std::io::Error| Irregular::Unreadable(error.kind().to_string());
    let mut file = open_without_following(path).map_err(|error| {
        // `O_NOFOLLOW` turns a swapped-in link into ELOOP.
        if error.raw_os_error() == Some(libc_eloop()) {
            Irregular::Symlink
        } else {
            unreadable(error)
        }
    })?;
    if !file.metadata().map_err(unreadable)?.is_file() {
        return Err(Irregular::Special);
    }
    let keep_whole = relative == "index.html";
    let mut sha256 = Sha256::new();
    let mut sha384 = Sha384::new();
    let mut head = Vec::with_capacity(HEAD_BYTES);
    let mut whole = Vec::new();
    let mut buffer = vec![0u8; 1 << 16];
    let mut size = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(unreadable)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        let chunk = &buffer[..read];
        sha256.update(chunk);
        sha384.update(chunk);
        if head.len() < HEAD_BYTES {
            head.extend_from_slice(&chunk[..chunk.len().min(HEAD_BYTES - head.len())]);
        }
        if keep_whole && whole.len() as u64 <= INDEX_LIMIT {
            whole.extend_from_slice(chunk);
        }
        // Past the per-file bound the digest is partial; the size alone already refuses the file.
        if size > super::bundle::MAX_FILE_BYTES {
            break;
        }
    }
    if keep_whole {
        if whole.len() as u64 > INDEX_LIMIT {
            scan.index_oversized = true;
        } else {
            scan.index_html = Some(whole);
        }
    }
    Ok(FileFacts {
        path: relative.to_owned(),
        size,
        sha256: hex(&sha256.finalize()),
        sha384: base64(&sha384.finalize()),
        head,
    })
}

#[cfg(unix)]
fn libc_eloop() -> i32 {
    libc::ELOOP
}

#[cfg(not(unix))]
fn libc_eloop() -> i32 {
    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::bundle::{self, sha384_base64};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-deploy-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_scan_describes_every_file_with_digests_head_and_the_whole_index() {
        let root = temp("scan");
        write(&root, "index.html", b"<!doctype html><title>Cantos</title>");
        write(&root, "assets/a b/ứng dụng.svg", b"<svg/>");
        let wasm = b"\0asm\x01\0\0\0 and more bytes after the magic";
        write(&root, "app.wasm", wasm);
        let wasm_bytes: &[u8] = wasm;
        let scan = scan_directory(&root).unwrap();
        let paths: Vec<_> = scan.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["app.wasm", "assets/a b/ứng dụng.svg", "index.html"]);
        assert!(scan.irregular.is_empty() && !scan.truncated && !scan.index_oversized);
        let wasm = &scan.files[0];
        assert_eq!(wasm.size, 39);
        assert!(wasm.head.starts_with(b"\0asm"));
        assert_eq!(wasm.sha256, crate::run_files::sha256_hex(wasm_bytes));
        assert_eq!(
            scan.files[2].sha384,
            sha384_base64(b"<!doctype html><title>Cantos</title>")
        );
        assert_eq!(
            scan.index_html.as_deref(),
            Some(&b"<!doctype html><title>Cantos</title>"[..])
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn links_special_files_and_unreadable_entries_are_irregular_never_followed() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp("irregular");
        let outside = temp("outside");
        write(&outside, "secret.txt", b"do not upload");
        write(&root, "index.html", b"x");
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked-dir")).unwrap();
        let fifo = root.join("pipe");
        let name = std::ffi::CString::new(std::os::unix::ffi::OsStrExt::as_bytes(fifo.as_os_str()))
            .unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        write(&root, "locked/file.txt", b"x");
        std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o000))
            .unwrap();
        let scan = scan_directory(&root).unwrap();
        std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let kinds: Vec<(&str, &Irregular)> = scan
            .irregular
            .iter()
            .map(|item| (item.path.as_str(), &item.kind))
            .collect();
        assert!(
            kinds.contains(&("link.txt", &Irregular::Symlink)),
            "{kinds:?}"
        );
        assert!(
            kinds.contains(&("linked-dir", &Irregular::Symlink)),
            "{kinds:?}"
        );
        assert!(kinds.contains(&("pipe", &Irregular::Special)), "{kinds:?}");
        assert!(
            kinds
                .iter()
                .any(|(path, kind)| *path == "locked" && matches!(kind, Irregular::Unreadable(_))),
            "{kinds:?}"
        );
        // The target of the links is never read into the scan.
        assert!(scan.files.iter().all(|file| file.path == "index.html"));
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_link_swapped_in_after_the_check_is_refused_by_the_open() {
        let root = temp("swap");
        let outside = temp("swap-outside");
        write(&outside, "secret.txt", b"do not upload");
        std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("late.txt")).unwrap();
        let mut scan = Scan::default();
        let result = read_file(&root.join("late.txt"), "late.txt", &mut scan);
        assert_eq!(result, Err(Irregular::Symlink));
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(outside).unwrap();
    }

    #[test]
    fn a_root_that_is_not_a_directory_is_an_error_not_an_empty_bundle() {
        let root = temp("not-dir");
        write(&root, "file", b"x");
        assert!(scan_directory(&root.join("file"))
            .unwrap_err()
            .contains("not a directory"));
        assert!(scan_directory(&root.join("missing"))
            .unwrap_err()
            .contains("unreadable"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn too_many_entries_and_too_deep_trees_are_flagged() {
        let root = temp("many");
        for index in 0..MAX_FILES + 5 {
            write(&root, &format!("f{index:04}.txt"), b"x");
        }
        let scan = scan_directory(&root).unwrap();
        assert!(scan.truncated);
        assert!(scan.files.len() <= MAX_FILES + 1);
        std::fs::remove_dir_all(&root).unwrap();

        let deep = temp("deep");
        let nested = (0..=MAX_DEPTH + 1)
            .map(|i| format!("d{i}"))
            .collect::<Vec<_>>()
            .join("/");
        write(&deep, &format!("{nested}/leaf.txt"), b"x");
        let scan = scan_directory(&deep).unwrap();
        assert!(scan
            .irregular
            .iter()
            .any(|item| item.kind == Irregular::TooDeep));
        std::fs::remove_dir_all(deep).unwrap();
    }

    #[test]
    fn an_oversized_index_is_reported_instead_of_buffered() {
        let root = temp("big-index");
        write(&root, "index.html", &vec![b'a'; INDEX_LIMIT as usize + 10]);
        let scan = scan_directory(&root).unwrap();
        assert!(scan.index_oversized);
        assert_eq!(scan.index_html, None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_scan_feeds_the_validator_and_a_changed_byte_changes_the_manifest() {
        let root = temp("end-to-end");
        let js = b"export default function init(){}";
        let wasm = b"\0asm\x01\0\0\0";
        write(&root, "cantos-theatre-aa.js", js);
        write(&root, "cantos-theatre-aa_bg.wasm", wasm);
        let index = format!(
            "<!doctype html><title>Cantos</title><script type=module>import '/cantos-theatre-aa.js'; init('/cantos-theatre-aa_bg.wasm')</script><link rel=modulepreload href=/cantos-theatre-aa.js integrity=\"sha384-{}\">",
            sha384_base64(js)
        );
        write(&root, "index.html", index.as_bytes());
        write(&root, bundle::THEATRE_VERIFY_EMAIL_ENTRY, index.as_bytes());
        let first =
            bundle::validate(crate::config::App::Theatre, &scan_directory(&root).unwrap()).unwrap();
        write(&root, "cantos-theatre-aa_bg.wasm", b"\0asm\x01\0\0\x01");
        let second =
            bundle::validate(crate::config::App::Theatre, &scan_directory(&root).unwrap()).unwrap();
        assert_ne!(first.digest, second.digest);
        // A tampered script no longer matches the integrity the page pins.
        write(
            &root,
            "cantos-theatre-aa.js",
            b"export default function init(){ steal() }",
        );
        assert!(
            bundle::validate(crate::config::App::Theatre, &scan_directory(&root).unwrap()).is_err()
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_trunk_prefixed_variable_names_are_reported_and_never_their_values() {
        let names = [
            "PATH",
            "TRUNK_BUILD_DIST",
            "TRUNK_BUILD_PUBLIC_URL",
            "TRUNK",
            "XTRUNK_A",
            "TRUNK_BUILD_DIST",
        ];
        let found = trunk_override_names(names.iter().map(std::ffi::OsString::from));
        assert_eq!(found, ["TRUNK_BUILD_DIST", "TRUNK_BUILD_PUBLIC_URL"]);
    }

    #[test]
    fn config_reads_are_bounded_utf8_regular_files_and_absence_is_not_an_error() {
        let root = temp("config");
        assert_eq!(read_text(&root.join("none")), Ok(None));
        write(&root, "ok.json", b"{}");
        assert_eq!(read_text(&root.join("ok.json")), Ok(Some("{}".into())));
        write(&root, "bad.json", &[0xFF, 0xFE]);
        assert_eq!(read_text(&root.join("bad.json")), Err("not UTF-8".into()));
        write(&root, "big.json", &vec![b' '; CONFIG_LIMIT as usize + 1]);
        assert_eq!(
            read_text(&root.join("big.json")),
            Err("larger than 1 MiB".into())
        );
        assert_eq!(read_text(&root), Err("not a regular file".into()));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_tracked_policy_is_readable_and_the_operator_mapping_is_optional() {
        let workspace = Workspace::discover().unwrap();
        let files = read_config_files(&workspace).unwrap();
        assert!(files.policy.contains("\"hosting\""));
    }

    #[test]
    fn runs_live_under_the_ignored_artifacts_tree_with_a_smoke_directory() {
        let root = temp("run");
        let workspace_less = run_files::create_run(&root, RUNS_DIR, &["smoke"]).unwrap();
        assert!(workspace_less
            .path
            .starts_with(root.join("artifacts/hosting")));
        assert!(workspace_less.path.join("smoke").is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }
}

// Hosting is public HTTPS; unlike the local Web proxy, it cannot use loopback HTTP.
pub(super) fn checked_api_origin(value: Option<&str>) -> Result<String, String> {
    let value = value.ok_or_else(|| {
        "CANTOS_API_ORIGIN is required for Firebase Hosting; set an exact HTTPS API origin"
            .to_owned()
    })?;
    let valid = url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.origin().ascii_serialization() == value
    });
    if valid {
        Ok(value.into())
    } else {
        Err("invalid CANTOS_API_ORIGIN".into())
    }
}
#[cfg(test)]
mod api_origin_tests {
    use super::checked_api_origin;
    #[test]
    fn hosting_api_origin_is_required_before_static_release() {
        assert_eq!(
            checked_api_origin(None),
            Err(
                "CANTOS_API_ORIGIN is required for Firebase Hosting; set an exact HTTPS API origin"
                    .into()
            )
        );
    }

    #[test]
    fn hosting_api_origin_is_nonsecret_exact_https() {
        assert_eq!(
            checked_api_origin(Some("https://api.example.org")),
            Ok("https://api.example.org".into())
        );
        assert_eq!(
            checked_api_origin(Some("https://api.example.org:8443")),
            Ok("https://api.example.org:8443".into())
        );
        for value in [
            "",
            "http://127.0.0.1:8080",
            "https://API.example.org",
            "https://api.example.org/",
            "https://secret@api.example.org",
            "https://user:secret@api.example.org",
            "https://api.example.org/api",
            "https://api.example.org?token=secret",
            "https://api.example.org#fragment",
            " https://api.example.org",
            "https://api.example.org\n",
        ] {
            assert_eq!(
                checked_api_origin(Some(value)),
                Err("invalid CANTOS_API_ORIGIN".into())
            );
        }
    }
}
