//! The only part that touches the machine: the approved input, the gcloud child and the local
//! ignored ledger. It reads one named file or stdin, never the environment's values.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use super::{
    flow::{Gcloud, Invocation, Ledger, LedgerEntry},
    gcloud,
    inventory::{Format, MAX_PAYLOAD},
    request::{self, FileFacts, Source},
};

pub const LEDGER_FILE: &str = "artifacts/secrets/import-ledger.ndjson";

/// Metadata only: `lstat` plus ownership, mode and location. The content is not opened.
pub fn file_facts(path: &Path, checkout: &Path) -> Result<FileFacts, &'static str> {
    let link = std::fs::symlink_metadata(path).map_err(|_| "the input file cannot be read")?;
    let inside_checkout = match (
        path.parent().map(std::fs::canonicalize),
        std::fs::canonicalize(checkout),
    ) {
        (Some(Ok(parent)), Ok(root)) => parent.starts_with(root),
        _ => true,
    };
    #[cfg(unix)]
    let (owned_by_caller, mode) = {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: geteuid has no preconditions and cannot fail.
        (
            link.uid() == unsafe { libc::geteuid() },
            link.mode() & 0o7777,
        )
    };
    #[cfg(not(unix))]
    let (owned_by_caller, mode) = (false, 0o777);
    Ok(FileFacts {
        symlink: link.file_type().is_symlink(),
        regular: link.file_type().is_file(),
        owned_by_caller,
        mode,
        len: link.len(),
        inside_checkout,
    })
}

/// Reads the approved bytes once, bounded, and validates them for the secret's format. The same
/// buffer is what gcloud receives, so there is no window to swap the file after checking it.
pub fn read_payload(source: &Source, format: Format, checkout: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    match source {
        Source::File(path) => {
            request::check_file(file_facts(path, checkout)?, format)?;
            let mut options = std::fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW);
            }
            let file = options
                .open(path)
                .map_err(|_| "the input file cannot be opened".to_owned())?;
            file.take(MAX_PAYLOAD + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "the input file cannot be read".to_owned())?;
        }
        Source::Stdin => {
            use std::io::IsTerminal;
            let stdin = std::io::stdin();
            if stdin.is_terminal() {
                return Err(
                    "--stdin refuses an interactive terminal; pipe the value from an approved local source"
                        .into(),
                );
            }
            stdin
                .lock()
                .take(MAX_PAYLOAD + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| "stdin cannot be read".to_owned())?;
        }
    }
    request::check_payload(&bytes, format)?;
    Ok(bytes)
}

/// The real gcloud. stderr is captured for classification only and never echoed.
pub struct SystemGcloud;

impl Gcloud for SystemGcloud {
    fn run(&mut self, args: &[String], stdin: Option<&[u8]>) -> Invocation {
        let mut command = Command::new(gcloud::PROGRAM);
        command
            .args(args)
            .envs(gcloud::ENV)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Invocation::NotInstalled
            }
            Err(_) => {
                return Invocation::Exited {
                    code: Some(127),
                    stdout: String::new(),
                    stderr: String::new(),
                }
            }
        };
        if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
            // A failed write still waits for the child, whose exit then decides the outcome.
            let _ = pipe.write_all(bytes);
        }
        match child.wait_with_output() {
            Ok(output) => Invocation::Exited {
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            },
            Err(_) => Invocation::Exited {
                code: None,
                stdout: String::new(),
                stderr: String::new(),
            },
        }
    }
}

/// Appends to an ignored, owner-only local ledger. A ledger failure is reported, never fatal to
/// the record of what Secret Manager did.
pub struct FileLedger {
    pub path: PathBuf,
    pub failed: bool,
}

impl FileLedger {
    pub fn new(checkout: &Path) -> Self {
        Self {
            path: checkout.join(LEDGER_FILE),
            failed: false,
        }
    }
}

pub fn render_entry(entry: &LedgerEntry, unix_time: u64) -> String {
    serde_json::json!({
        "time_unix": unix_time,
        "project": entry.project,
        "secret": entry.secret,
        "account": entry.account,
        "action": entry.action,
        "result": entry.result,
        "version": entry.version,
    })
    .to_string()
}

impl Ledger for FileLedger {
    fn append(&mut self, entry: &LedgerEntry) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let line = render_entry(entry, now);
        let written = (|| -> std::io::Result<()> {
            let dir = self.path.parent().unwrap_or(Path::new("."));
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(dir)?;
            let mut options = std::fs::OpenOptions::new();
            options.create(true).append(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            writeln!(options.open(&self.path)?, "{line}")
        })();
        if written.is_err() {
            self.failed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "cantos-secrets-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    #[test]
    fn a_private_synthetic_file_is_read_and_a_shared_or_linked_one_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("input");
        let checkout = scratch("checkout");
        let path = dir.join("synthetic-envelope-key");
        std::fs::write(&path, [9u8; 32]).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let source = Source::File(path.clone());
        assert_eq!(
            read_payload(&source, Format::Bytes32, &checkout).unwrap(),
            vec![9u8; 32]
        );

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_payload(&source, Format::Bytes32, &checkout)
            .unwrap_err()
            .contains("chmod 600"));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let link = dir.join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(
            read_payload(&Source::File(link), Format::Bytes32, &checkout)
                .unwrap_err()
                .contains("symbolic link")
        );

        // A file inside the checkout is refused even when private.
        let inside = checkout.join("secret");
        std::fs::write(&inside, [9u8; 32]).unwrap();
        std::fs::set_permissions(&inside, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(
            read_payload(&Source::File(inside), Format::Bytes32, &checkout)
                .unwrap_err()
                .contains("inside the repository")
        );

        assert!(read_payload(&Source::File(dir.join("absent")), Format::Text, &checkout).is_err());
        std::fs::remove_dir_all(dir).unwrap();
        std::fs::remove_dir_all(checkout).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn the_ledger_is_owner_only_and_never_holds_a_payload() {
        use std::os::unix::fs::PermissionsExt;
        let checkout = scratch("ledger");
        let mut ledger = FileLedger::new(&checkout);
        ledger.append(&LedgerEntry {
            project: "cantos-a348b".into(),
            secret: "cantos-dev-identity-smtp-password".into(),
            account: "operator@example.test".into(),
            action: "add-version",
            result: "added",
            version: Some(1),
        });
        assert!(!ledger.failed);
        let text = std::fs::read_to_string(&ledger.path).unwrap();
        let line: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        let mut keys: Vec<&str> = line
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "account",
                "action",
                "project",
                "result",
                "secret",
                "time_unix",
                "version"
            ]
        );
        let mode = std::fs::metadata(&ledger.path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        std::fs::remove_dir_all(checkout).unwrap();
    }
}
