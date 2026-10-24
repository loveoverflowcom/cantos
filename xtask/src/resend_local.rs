//! Opt-in synthetic Resend HTTP mail fixture. This adapter deliberately has no Identity
//! Mailer/VerificationActions implementation and accepts no caller-supplied message or secret.

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;

use crate::{probe, process::Supervisor, run_files, workspace::Workspace};

pub const VERSION: &str = "0.0.7";
const NEXT_VERSION: &str = "15.5.28";
const PACKAGE_VARIABLE: &str = "CANTOS_RESEND_LOCAL_PACKAGE";
const PAYLOAD: &str = include_str!("../../infra/local-mail/resend-local/synthetic-email.json");
// Public nonfunctional marker. The emulator only checks the re_ prefix; this is no credential.
const AUTHORIZATION: &str = "Bearer re_cantos_synthetic_fixture";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailFault {
    Rejected,
    /// A request may have been recorded. Never retry the write automatically.
    Uncertain,
}

impl std::fmt::Display for MailFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Rejected => "synthetic mail request rejected",
            Self::Uncertain => "synthetic mail outcome uncertain; no retry attempted",
        })
    }
}
impl std::error::Error for MailFault {}

/// Only proves that the local fixture recorded and returned the fixed synthetic message.
/// It cannot be converted into a mailbox proof, authentication fact or application session.
#[derive(Debug, PartialEq, Eq)]
pub struct SimulatedReceipt {
    id: String,
}

fn mail_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn accepted(status: u16, body: &[u8]) -> std::result::Result<SimulatedReceipt, MailFault> {
    if matches!(status, 200 | 201) {
        let value: serde_json::Value =
            serde_json::from_slice(body).map_err(|_| MailFault::Uncertain)?;
        let id = value["id"]
            .as_str()
            .filter(|value| mail_id(value))
            .ok_or(MailFault::Uncertain)?;
        Ok(SimulatedReceipt { id: id.to_owned() })
    } else if status == 408 || status >= 500 {
        Err(MailFault::Uncertain)
    } else {
        Err(MailFault::Rejected)
    }
}

/// The sole write is this compile-time synthetic message. No recipient, body, URL or key input.
pub fn send_preview(address: SocketAddr) -> std::result::Result<SimulatedReceipt, MailFault> {
    if address.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) || address.port() == 0 {
        return Err(MailFault::Rejected);
    }
    let headers = [("Authorization", AUTHORIZATION)];
    let response = probe::http_request(
        address,
        "POST",
        "/emails",
        "application/json",
        &headers,
        PAYLOAD.trim().as_bytes(),
    )
    .map_err(|_| MailFault::Uncertain)?;
    let receipt = accepted(response.status, &response.body)?;
    let capture = probe::http_request(
        address,
        "GET",
        &format!("/emails/{}", receipt.id),
        "application/json",
        &headers,
        &[],
    )
    .map_err(|_| MailFault::Uncertain)?;
    let value: serde_json::Value =
        serde_json::from_slice(&capture.body).map_err(|_| MailFault::Uncertain)?;
    let expected: serde_json::Value =
        serde_json::from_str(PAYLOAD).map_err(|_| MailFault::Rejected)?;
    if capture.status != 200
        || value["id"] != receipt.id
        || ["from", "to", "subject", "text"]
            .iter()
            .any(|key| value[key] != expected[key])
        || value["last_event"] != "delivered"
    {
        return Err(MailFault::Uncertain);
    }
    Ok(receipt)
}

#[derive(Deserialize)]
struct Release {
    name: String,
    version: String,
    next_version: String,
    files: BTreeMap<String, String>,
}

fn verify_files(root: &Path, files: &BTreeMap<String, String>) -> Result<()> {
    for (relative, expected) in files {
        let path = Path::new(relative);
        ensure!(
            !path.is_absolute()
                && path
                    .components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "invalid Resend release manifest path"
        );
        let path = root.join(path);
        ensure!(
            std::fs::symlink_metadata(&path)?.file_type().is_file(),
            "Resend package file is not regular"
        );
        ensure!(
            path.canonicalize()?.starts_with(root),
            "Resend package path escapes its root"
        );
        let (actual, _) = run_files::hash_path(&path).map_err(anyhow::Error::msg)?;
        ensure!(
            &actual == expected,
            "installed resend-local bytes differ from the reviewed 0.0.7 archive"
        );
    }
    Ok(())
}

pub struct Installed {
    node: PathBuf,
    root: PathBuf,
}

fn node_executable() -> Result<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|path| path.join("node"))
        .find(|path| path.is_file())
        .context("Resend local needs installed Node.js 22+; xtask never installs it")?
        .canonicalize()
        .context("resolve installed Node.js")
}

impl Installed {
    /// Inspection only. Installation is separate and explicit, never npx or an implicit fetch.
    pub fn inspect(workspace: &Workspace) -> Result<Self> {
        let node = node_executable()?;
        let root = std::env::var_os(PACKAGE_VARIABLE)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                workspace.path("infra/local-mail/resend-local/node_modules/resend-local")
            });
        ensure!(
            root.is_absolute(),
            "CANTOS_RESEND_LOCAL_PACKAGE must be absolute"
        );
        let root = root.canonicalize().context("resend-local@0.0.7 is missing; prepare the reviewed infra/local-mail/resend-local lockfile separately with npm ci --ignore-scripts; no fallback occurs")?;
        let release: Release = serde_json::from_slice(&std::fs::read(
            workspace.path("infra/local-mail/resend-local/release-files.json"),
        )?)?;
        ensure!(
            release.name == "resend-local"
                && release.version == VERSION
                && release.next_version == NEXT_VERSION
                && release.files.len() == 133,
            "Resend release identity differs from the reviewed fixture"
        );
        verify_files(&root, &release.files)?;
        let next = root
            .parent()
            .context("invalid Resend install layout")?
            .join("next/package.json");
        let next: serde_json::Value = serde_json::from_slice(&std::fs::read(next)?)?;
        ensure!(next["name"] == "next" && next["version"] == NEXT_VERSION,
            "Resend local requires the locked Next.js 15.5.28 security patch; unpatched 15.5.2 is refused");
        Ok(Self { node, root })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ready {
    host: IpAddr,
    port: u16,
    version: String,
    outbound_guard: bool,
    synthetic_only: bool,
}

struct StartingChild(Option<Child>);
impl Drop for StartingChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct Run {
    pub address: SocketAddr,
    /// Retained private synthetic-only capture, never a native Identity database.
    pub directory: PathBuf,
}

impl Run {
    pub fn start(
        workspace: &Workspace,
        installed: &Installed,
        supervisor: &mut Supervisor,
    ) -> Result<Self> {
        let run = run_files::create_run(workspace.root(), "artifacts/local-mail", &[])
            .map_err(anyhow::Error::msg)?;
        let database = std::fs::read(installed.root.join("dist/app/resend-local.sqlite"))?;
        let database =
            run_files::write_private(&run, "mail.sqlite", &database).map_err(anyhow::Error::msg)?;
        let mut command = Command::new(&installed.node);
        command
            .arg(workspace.path("infra/local-mail/resend-local/launcher.cjs"))
            .arg(&installed.root)
            .arg(&database)
            .env_clear()
            .env("NEXT_TELEMETRY_DISABLED", "1")
            .env("TMPDIR", &run.path)
            .current_dir(&run.path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = StartingChild(Some(
            command
                .spawn()
                .context("start installed Resend local fixture")?,
        ));
        let stdout = child
            .0
            .as_mut()
            .and_then(|child| child.stdout.take())
            .context("Resend local readiness pipe unavailable")?;
        let (send, receive) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = Vec::new();
            let ready = (&mut reader)
                .take(4097)
                .read_until(b'\n', &mut line)
                .ok()
                .filter(|size| *size <= 4096)
                .map(|_| line);
            let _ = send.send(ready);
            let _ = std::io::copy(&mut reader, &mut std::io::sink());
        });
        let ready = receive.recv_timeout(Duration::from_secs(15)).ok().flatten()
            .and_then(|line| serde_json::from_slice::<Ready>(&line).ok())
            .context("Resend local failed its 15s private readiness handshake; provider output suppressed; no fallback or retry")?;
        if ready.host != IpAddr::V4(Ipv4Addr::LOCALHOST)
            || ready.port == 0
            || ready.version != VERSION
            || !ready.outbound_guard
            || !ready.synthetic_only
        {
            bail!("invalid Resend local readiness metadata");
        }
        let owned = child.0.take().context("Resend local child missing")?;
        supervisor.adopt("Resend local synthetic mail", owned);
        Ok(Self {
            address: SocketAddr::new(ready.host, ready.port),
            directory: run.path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn receipt_is_simulated_and_uncertain_writes_are_not_success() {
        let id = "ca831b66-8ed7-4ae5-9366-655a866cbe98";
        assert_eq!(
            accepted(200, format!("{{\"id\":\"{id}\"}}").as_bytes()),
            Ok(SimulatedReceipt { id: id.into() })
        );
        for (status, body, fault) in [
            (302, b"{}".as_slice(), MailFault::Rejected),
            (401, b"{}", MailFault::Rejected),
            (422, b"{}", MailFault::Rejected),
            (408, b"{}", MailFault::Uncertain),
            (500, b"{}", MailFault::Uncertain),
            (200, b"not json", MailFault::Uncertain),
            (200, b"{\"id\":\"../../emails\"}", MailFault::Uncertain),
        ] {
            assert_eq!(accepted(status, body), Err(fault));
        }
    }

    #[test]
    fn only_the_literal_loopback_fixture_can_receive_the_fixed_mail() {
        for address in [
            "192.0.2.1:443",
            "127.0.0.2:8005",
            "[::1]:8005",
            "127.0.0.1:0",
        ] {
            assert_eq!(
                send_preview(address.parse().unwrap()),
                Err(MailFault::Rejected)
            );
        }
        let payload: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
        assert_eq!(
            payload["to"],
            serde_json::json!(["recipient@cantos.invalid"])
        );
        assert_eq!(
            payload["text"],
            "Nonfunctional fixture only. No verification link, password or token."
        );
        assert_eq!(payload.as_object().unwrap().len(), 4);
    }

    fn capture_request(stream: &mut std::net::TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut request = String::new();
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert!(!line.is_empty() && request.len() < 4096);
            request.push_str(&line);
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                length = value.trim().parse().unwrap();
            }
            if line == "\r\n" {
                break;
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        request.push_str(std::str::from_utf8(&body).unwrap());
        request
    }

    #[test]
    fn real_http_adapter_posts_once_and_checks_the_captured_synthetic_message() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let id = "ca831b66-8ed7-4ae5-9366-655a866cbe98";
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            let mut captured: serde_json::Value = serde_json::from_str(PAYLOAD).unwrap();
            captured["id"] = id.into();
            captured["last_event"] = "delivered".into();
            for body in [format!("{{\"id\":\"{id}\"}}"), captured.to_string()] {
                let (mut stream, _) = listener.accept().unwrap();
                requests.push(capture_request(&mut stream));
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
            requests
        });
        assert_eq!(
            send_preview(address),
            Ok(SimulatedReceipt { id: id.into() })
        );
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("POST /emails HTTP/1.1\r\n"));
        assert_eq!(
            requests[0].split_once("\r\n\r\n").unwrap().1,
            PAYLOAD.trim()
        );
        assert!(requests[1].starts_with(&format!("GET /emails/{id} HTTP/1.1\r\n")));
    }

    #[test]
    fn lost_or_rejected_acknowledgement_has_no_automatic_second_write() {
        for reply in [
            "HTTP/1.1 500 Error\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\ntruncated",
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let request = capture_request(&mut stream);
                stream.write_all(reply.as_bytes()).unwrap();
                request
            });
            assert_eq!(send_preview(address), Err(MailFault::Uncertain));
            assert!(server.join().unwrap().starts_with("POST /emails "));
        }
    }

    #[test]
    fn installed_package_drift_and_path_escapes_refuse_before_execution() {
        let root = run_files::create_run(&std::env::temp_dir(), "cantos-resend-unit", &[]).unwrap();
        let path = root.path.join("file.js");
        let canonical = root.path.canonicalize().unwrap();
        std::fs::write(&path, "reviewed").unwrap();
        let files = BTreeMap::from([("file.js".to_owned(), run_files::sha256_hex(b"reviewed"))]);
        verify_files(&canonical, &files).unwrap();
        std::fs::write(&path, "changed").unwrap();
        assert!(verify_files(&canonical, &files)
            .unwrap_err()
            .to_string()
            .contains("bytes differ"));
        let escape = BTreeMap::from([("../file.js".to_owned(), String::new())]);
        assert!(verify_files(&canonical, &escape)
            .unwrap_err()
            .to_string()
            .contains("manifest path"));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&path, root.path.join("linked.js")).unwrap();
            let link = BTreeMap::from([("linked.js".to_owned(), String::new())]);
            assert!(verify_files(&canonical, &link)
                .unwrap_err()
                .to_string()
                .contains("not regular"));
        }
        std::fs::remove_dir_all(root.path).unwrap();
    }

    #[test]
    fn launcher_guard_regressions_need_no_installed_emulator() {
        let workspace = Workspace::discover().unwrap();
        let output = Command::new(node_executable().unwrap())
            .arg("--test")
            .arg(workspace.path("infra/local-mail/resend-local/launcher.test.cjs"))
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .output()
            .expect("Node 22+ is the existing repository tool prerequisite");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
