//! Test-harness-only: the real `cantos-identity` executable over its private Unix socket, a real
//! file-backed Turso database and a run-owned SMTP sink. Nothing here is a production path and no
//! mail leaves the machine.
//!
//! [`IDENTITY_BINARY`] names a prebuilt executable by absolute path; it is then the exact file
//! every process here runs, which is what the native isolation guard binds by digest. Without it
//! the ordinary real-Identity HTTP journeys build the executable once. An isolated run
//! ([`NATIVE_TEST_DIRECTORY`]) never builds anything, and the candidate journey requires the
//! artifact. Nothing here needs a program on `PATH`.
#![allow(dead_code)]

use std::{
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    process::{Child, Command},
    task::JoinHandle,
};

/// Test-only input: the absolute path of an already built `cantos-identity` executable.
pub const IDENTITY_BINARY: &str = "CANTOS_TEST_IDENTITY_BINARY";
/// Set by the native isolation guard to the run directory it owns.
pub const NATIVE_TEST_DIRECTORY: &str = "CANTOS_NATIVE_TEST_DIRECTORY";

/// The supplied artifact, checked to be an absolute regular executable file. A malformed value
/// is a failed test, never a reason to pick another binary.
fn supplied() -> Option<PathBuf> {
    let value = std::env::var_os(IDENTITY_BINARY)?;
    let path = PathBuf::from(value);
    let metadata = std::fs::symlink_metadata(&path).ok();
    assert!(
        path.is_absolute()
            && metadata.as_ref().is_some_and(|meta| {
                meta.file_type().is_file() && meta.permissions().mode() & 0o111 != 0
            }),
        "{IDENTITY_BINARY} must name an absolute regular executable file"
    );
    Some(path)
}

/// The prebuilt executable a candidate journey must run. It is never built here.
pub fn supplied_binary() -> &'static Path {
    static SUPPLIED: OnceLock<PathBuf> = OnceLock::new();
    SUPPLIED.get_or_init(|| {
        supplied().unwrap_or_else(|| {
            panic!("set {IDENTITY_BINARY} to the prebuilt cantos-identity executable; this journey never builds it")
        })
    })
}

/// The supplied executable, else (outside isolation only) one built once per test run. A failure
/// to build is a failed test, never a skipped one.
pub fn binary() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        if let Some(path) = supplied() {
            return path;
        }
        assert!(
            std::env::var_os(NATIVE_TEST_DIRECTORY).is_none(),
            "an isolated run never builds; pass {IDENTITY_BINARY}"
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = std::process::Command::new(option_env!("CARGO").unwrap_or("cargo"))
            .current_dir(&root)
            .args([
                "build",
                "--locked",
                "-p",
                "cantos-identity",
                "--features",
                "service",
                "--bin",
                "cantos-identity",
                "--message-format=json-render-diagnostics",
            ])
            .stderr(Stdio::inherit())
            .output()
            .expect("cargo runs");
        assert!(output.status.success(), "building cantos-identity failed");
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|v| {
                v["reason"] == "compiler-artifact" && v["target"]["name"] == "cantos-identity"
            })
            .filter_map(|v| v["executable"].as_str().map(PathBuf::from))
            .next_back()
            .expect("cargo reported the cantos-identity executable")
    })
}

// ---- run directory ---------------------------------------------------------------------------

/// A private directory with a SHORT path: Unix socket paths are limited to about 100 bytes.
/// Under the isolation guard it lives in the guard's own run directory, otherwise in `/tmp`.
pub struct RunDir(PathBuf);

/// Identity's configured socket path limit; the Server refuses a longer one as configuration.
const MAX_SOCKET_PATH: usize = 100;

impl RunDir {
    pub fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::var_os(NATIVE_TEST_DIRECTORY)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        assert!(
            root.is_absolute(),
            "{NATIVE_TEST_DIRECTORY} must be absolute"
        );
        let path = root.join(format!(
            "cs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        let run = Self(path);
        // The mode is checked, not assumed: a umask can only narrow it, never widen it.
        let mode = std::fs::metadata(&run.0).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "the run directory must be private");
        assert!(
            run.path("identity.sock").as_os_str().len() <= MAX_SOCKET_PATH,
            "the run directory leaves no room for a socket path"
        );
        run
    }
    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    pub fn uid(&self) -> u32 {
        std::fs::metadata(&self.0).unwrap().uid()
    }
}

impl Drop for RunDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---- SMTP sink and mailed link ----------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Captured {
    pub to: Vec<String>,
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedLink {
    pub operation: String,
    pub proof: String,
    pub secret: String,
}

impl Captured {
    fn body(&self) -> String {
        let (headers, body) = self.raw.split_once("\r\n\r\n").unwrap_or((&self.raw, ""));
        let lower = headers.to_ascii_lowercase();
        let bytes = if lower.contains("content-transfer-encoding: base64") {
            decode_base64(body)
        } else if lower.contains("content-transfer-encoding: quoted-printable") {
            decode_quoted_printable(body)
        } else {
            body.as_bytes().to_vec()
        };
        String::from_utf8(bytes).expect("utf-8 body")
    }

    /// The `<base>#operation=…&proof=…&secret=…` link, exactly as the mailbox owner would open it.
    pub fn link(&self) -> (String, ParsedLink) {
        let body = self.body();
        let start = body.find("https://").expect("body carries a link");
        let end = body[start..]
            .find(|c: char| c.is_whitespace())
            .map_or(body.len(), |n| start + n);
        let link = &body[start..end];
        let (base, fragment) = link.split_once('#').expect("fragment");
        let mut parsed = ParsedLink {
            operation: String::new(),
            proof: String::new(),
            secret: String::new(),
        };
        for pair in fragment.split('&') {
            let (key, value) = pair.split_once('=').unwrap();
            match key {
                "operation" => parsed.operation = value.to_owned(),
                "proof" => parsed.proof = value.to_owned(),
                "secret" => parsed.secret = value.to_owned(),
                other => panic!("unexpected link field {other}"),
            }
        }
        (base.to_owned(), parsed)
    }
}

fn decode_base64(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0_u32, 0);
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    out
}

fn decode_quoted_printable(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'=' {
            if bytes[index + 1..].starts_with(b"\r\n") {
                index += 3;
                continue;
            }
            out.push(u8::from_str_radix(&text[index + 1..index + 3], 16).unwrap());
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    out
}

pub struct SmtpSink {
    pub port: u16,
    inbox: Arc<Mutex<Vec<Captured>>>,
    task: JoinHandle<()>,
}

impl SmtpSink {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let inbox: Arc<Mutex<Vec<Captured>>> = Arc::default();
        let shared = Arc::clone(&inbox);
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    return;
                };
                let inbox = Arc::clone(&shared);
                tokio::spawn(async move { session(stream, inbox).await });
            }
        });
        Self { port, inbox, task }
    }

    pub fn messages(&self) -> Vec<Captured> {
        self.inbox.lock().unwrap().clone()
    }

    pub async fn wait_for(&self, count: usize) -> Vec<Captured> {
        for _ in 0..400 {
            let messages = self.messages();
            if messages.len() >= count {
                return messages;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!(
            "expected {count} captured message(s), saw {}",
            self.messages().len()
        );
    }
}

impl Drop for SmtpSink {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn session(stream: tokio::net::TcpStream, inbox: Arc<Mutex<Vec<Captured>>>) {
    let mut stream = BufReader::new(stream);
    if stream
        .get_mut()
        .write_all(b"220 sink ESMTP\r\n")
        .await
        .is_err()
    {
        return;
    }
    let mut to = Vec::new();
    loop {
        let mut line = String::new();
        if stream.read_line(&mut line).await.unwrap_or(0) == 0 {
            return;
        }
        let response: &[u8] = if line.starts_with("EHLO ") {
            b"250-sink\r\n250 8BITMIME\r\n"
        } else if line.starts_with("MAIL FROM:") {
            b"250 ok\r\n"
        } else if let Some(recipient) = line.strip_prefix("RCPT TO:") {
            to.push(recipient.trim().trim_matches(['<', '>']).to_owned());
            b"250 ok\r\n"
        } else if line == "DATA\r\n" {
            if stream.get_mut().write_all(b"354 go\r\n").await.is_err() {
                return;
            }
            let mut raw = String::new();
            loop {
                let mut body_line = String::new();
                if stream.read_line(&mut body_line).await.unwrap_or(0) == 0 {
                    return;
                }
                if body_line == ".\r\n" {
                    break;
                }
                raw.push_str(&body_line);
            }
            inbox.lock().unwrap().push(Captured {
                to: std::mem::take(&mut to),
                raw,
            });
            b"250 queued\r\n"
        } else if line == "QUIT\r\n" {
            let _ = stream.get_mut().write_all(b"221 bye\r\n").await;
            return;
        } else if line == "RSET\r\n" || line == "NOOP\r\n" {
            b"250 ok\r\n"
        } else {
            b"502 unsupported\r\n"
        };
        if stream.get_mut().write_all(response).await.is_err() {
            return;
        }
    }
}

// ---- the process -------------------------------------------------------------------------------

#[derive(Clone)]
pub struct IdentityEnv {
    pub database: PathBuf,
    pub socket: PathBuf,
    pub uid: u32,
    pub smtp_port: u16,
    binary: &'static Path,
}

impl IdentityEnv {
    pub fn new(run: &RunDir, smtp_port: u16) -> Self {
        Self::with_binary(run, smtp_port, binary())
    }

    /// Runs exactly the prebuilt executable named by [`IDENTITY_BINARY`].
    pub fn supplied(run: &RunDir, smtp_port: u16) -> Self {
        Self::with_binary(run, smtp_port, supplied_binary())
    }

    fn with_binary(run: &RunDir, smtp_port: u16, binary: &'static Path) -> Self {
        Self {
            database: run.path("identity.db"),
            socket: run.path("identity.sock"),
            uid: run.uid(),
            smtp_port,
            binary,
        }
    }

    pub fn binary(&self) -> &Path {
        self.binary
    }

    fn command(&self, verb: &str) -> Command {
        let mut command = Command::new(self.binary);
        command
            .arg(verb)
            .env_clear()
            .env("CANTOS_IDENTITY_DATABASE", &self.database)
            .env("CANTOS_IDENTITY_SOCKET", &self.socket)
            .env("CANTOS_IDENTITY_SERVER_UIDS", self.uid.to_string())
            .env(
                "CANTOS_IDENTITY_SIGNUP_LINK_BASE",
                "https://theatre.example/signup",
            )
            .env("CANTOS_IDENTITY_SMTP_HOST", "127.0.0.1")
            .env("CANTOS_IDENTITY_SMTP_PORT", self.smtp_port.to_string())
            .env("CANTOS_IDENTITY_SMTP_TLS", "loopback-plaintext")
            .env("CANTOS_IDENTITY_SMTP_FROM", "cantos@example.test")
            .kill_on_drop(true)
            .stdin(Stdio::null());
        command
    }

    pub async fn migrate(&self) {
        let output = self.command("migrate").output().await.unwrap();
        assert!(
            output.status.success(),
            "migrate failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    pub async fn serve(&self) -> Instance {
        let mut child = self
            .command("serve")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut lines = BufReader::new(stdout).lines();
        let ready = tokio::time::timeout(Duration::from_secs(30), lines.next_line())
            .await
            .expect("identity became ready in time")
            .unwrap()
            .expect("identity printed its readiness line");
        assert!(ready.contains("\"event\":\"ready\""), "{ready}");
        // Drain so the child never blocks on a full pipe.
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(_)) = lines.next_line().await {}
            });
        }
        Instance { child }
    }
}

pub struct Instance {
    child: Child,
}

impl Instance {
    /// SIGTERM and a bounded wait: the graceful path that also removes this run's socket. The
    /// signal goes to this owned, not yet reaped child only, so its PID cannot have been reused.
    /// A child that does not exit in time is killed and reaped before the test fails.
    pub async fn terminate(mut self) {
        let pid = libc::pid_t::try_from(self.child.id().expect("the child is not yet reaped"))
            .expect("a process id fits pid_t");
        // SAFETY: kill(2) takes no pointers; the target is our own unreaped child.
        assert_eq!(
            unsafe { libc::kill(pid, libc::SIGTERM) },
            0,
            "signal the owned child"
        );
        match tokio::time::timeout(Duration::from_secs(30), self.child.wait()).await {
            Ok(status) => {
                status.unwrap();
            }
            Err(_) => {
                let _ = self.child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await;
                panic!("identity did not drain and exit after SIGTERM");
            }
        }
    }
}
