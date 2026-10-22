//! Test-harness-only helpers. Nothing here is compiled into the library or the executable:
//! the capture inbox and the SMTP sink belong to a test run and are never a production path.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use cantos_identity::{
    domain::ids::OperationId,
    mail::{MailFault, MailFuture, Mailer, ProofMail},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    task::JoinHandle,
};

// ---- link parsing ------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedLink {
    pub operation: [u8; 16],
    pub proof: [u8; 16],
    pub secret: [u8; 32],
}

fn unhex<const N: usize>(text: &str) -> [u8; N] {
    assert_eq!(text.len(), N * 2, "unexpected hex length");
    let mut out = [0; N];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap();
    }
    out
}

pub fn parse_link(link: &str) -> ParsedLink {
    let fragment = link.split_once('#').expect("link has a fragment").1;
    let mut operation = None;
    let mut proof = None;
    let mut secret = None;
    for pair in fragment.split('&') {
        let (key, value) = pair.split_once('=').unwrap();
        match key {
            "operation" => operation = Some(unhex::<16>(value)),
            "proof" => proof = Some(unhex::<16>(value)),
            "secret" => secret = Some(unhex::<32>(value)),
            other => panic!("unexpected link field {other}"),
        }
    }
    ParsedLink {
        operation: operation.unwrap(),
        proof: proof.unwrap(),
        secret: secret.unwrap(),
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---- in-process capture mailer -------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    Deliver,
    Reject,
    Uncertain,
}

/// A mail port that records what it was asked to send. Used where the transport itself is not
/// under test; the process journey uses a real SMTP session into [`SmtpSink`] instead.
pub struct CaptureMailer {
    pub behavior: Mutex<Behavior>,
    sent: Mutex<Vec<(String, String)>>,
}

impl CaptureMailer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            behavior: Mutex::new(Behavior::Deliver),
            sent: Mutex::new(Vec::new()),
        })
    }

    pub fn set(&self, behavior: Behavior) {
        *self.behavior.lock().unwrap() = behavior;
    }

    pub fn count(&self) -> usize {
        self.sent.lock().unwrap().len()
    }

    pub fn last(&self) -> (String, ParsedLink) {
        let sent = self.sent.lock().unwrap();
        let (to, link) = sent.last().expect("a message was sent").clone();
        (to, parse_link(&link))
    }

    pub fn recipients(&self) -> Vec<String> {
        self.sent
            .lock()
            .unwrap()
            .iter()
            .map(|(to, _)| to.clone())
            .collect()
    }
}

impl Mailer for CaptureMailer {
    fn is_configured(&self) -> bool {
        true
    }

    fn send<'a>(&'a self, mail: &'a ProofMail) -> MailFuture<'a> {
        Box::pin(async move {
            match *self.behavior.lock().unwrap() {
                Behavior::Reject => Err(MailFault::Rejected),
                Behavior::Uncertain => {
                    // The relay may have the message: it is recorded, but the answer was lost.
                    self.sent
                        .lock()
                        .unwrap()
                        .push((mail.recipient().as_str().to_owned(), mail.link().to_owned()));
                    Err(MailFault::Uncertain)
                }
                Behavior::Deliver => {
                    self.sent
                        .lock()
                        .unwrap()
                        .push((mail.recipient().as_str().to_owned(), mail.link().to_owned()));
                    Ok(())
                }
            }
        })
    }
}

pub fn operation(value: u128) -> OperationId {
    OperationId::from_bytes(value.to_be_bytes()).unwrap()
}

// ---- run-owned SMTP sink --------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SinkMode {
    Accept,
    RejectRecipient,
    RejectData,
    /// Reads the whole DATA section, then drops the connection without the final answer.
    DropDataAck,
}

#[derive(Clone, Debug)]
pub struct Captured {
    pub to: Vec<String>,
    pub raw: String,
}

impl Captured {
    /// The message body with its transfer encoding removed.
    pub fn body(&self) -> String {
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

    pub fn link(&self) -> ParsedLink {
        let body = self.body();
        let start = body.find("https://").expect("body carries a link");
        let end = body[start..]
            .find(|c: char| c.is_whitespace())
            .map_or(body.len(), |n| start + n);
        parse_link(&body[start..end])
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
    pub async fn start(mode: SinkMode) -> Self {
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
                tokio::spawn(async move { session(stream, mode, inbox).await });
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
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
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

async fn session(stream: tokio::net::TcpStream, mode: SinkMode, inbox: Arc<Mutex<Vec<Captured>>>) {
    let mut stream = BufReader::new(stream);
    let write = |line: &'static [u8]| line;
    if stream
        .get_mut()
        .write_all(write(b"220 sink ESMTP\r\n"))
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
            if mode == SinkMode::RejectRecipient {
                b"550 refused\r\n"
            } else {
                let address = recipient.trim().trim_matches(['<', '>']).to_owned();
                to.push(address);
                b"250 ok\r\n"
            }
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
            match mode {
                SinkMode::DropDataAck => {
                    inbox.lock().unwrap().push(Captured {
                        to: std::mem::take(&mut to),
                        raw,
                    });
                    return;
                }
                SinkMode::RejectData => b"451 not accepted\r\n",
                _ => {
                    inbox.lock().unwrap().push(Captured {
                        to: std::mem::take(&mut to),
                        raw,
                    });
                    b"250 queued\r\n"
                }
            }
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

// ---- run directory and process harness ---------------------------------------------------------

use std::{
    os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
    time::Duration,
};

use tokio::process::{Child, Command};

/// Test-only input: the absolute path of an already built `cantos-identity` executable, which is
/// then the exact file every process here runs (the native isolation guard binds its digest).
pub const IDENTITY_BINARY: &str = "CANTOS_TEST_IDENTITY_BINARY";
/// Set by the native isolation guard to the run directory it owns.
pub const NATIVE_TEST_DIRECTORY: &str = "CANTOS_NATIVE_TEST_DIRECTORY";

/// A private directory with a SHORT path: Unix socket paths are limited to about 100 bytes and
/// macOS temporary directories alone can use half of that. Under the isolation guard it lives in
/// the guard's own run directory, otherwise in `/tmp`.
pub struct RunDir(PathBuf);

/// The longest socket path the executable accepts.
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
            "cid-{}-{}",
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

    pub fn dir(&self) -> &Path {
        &self.0
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

/// The executable every process here runs: exactly the supplied [`IDENTITY_BINARY`], else the
/// one Cargo built for this test target. An isolated run must supply it, so the guard's receipt
/// binds the child actually started; a malformed value is a failed test, never a fallback.
pub fn binary() -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| match std::env::var_os(IDENTITY_BINARY) {
        Some(value) => {
            let path = PathBuf::from(value);
            let metadata = std::fs::symlink_metadata(&path).ok();
            assert!(
                path.is_absolute()
                    && metadata.as_ref().is_some_and(|meta| {
                        meta.file_type().is_file() && meta.permissions().mode() & 0o111 != 0
                    }),
                "{IDENTITY_BINARY} must name an absolute regular executable file"
            );
            path
        }
        None => {
            assert!(
                std::env::var_os(NATIVE_TEST_DIRECTORY).is_none(),
                "an isolated run must pass the Identity executable as {IDENTITY_BINARY}"
            );
            PathBuf::from(env!("CARGO_BIN_EXE_cantos-identity"))
        }
    })
}

#[derive(Clone)]
pub struct Env {
    pub database: PathBuf,
    pub socket: PathBuf,
    pub uids: String,
    pub smtp_port: Option<u16>,
}

impl Env {
    pub fn new(run: &RunDir) -> Self {
        Self {
            database: run.path("identity.db"),
            socket: run.path("identity.sock"),
            uids: run.uid().to_string(),
            smtp_port: None,
        }
    }

    fn command(&self, verb: &str) -> Command {
        let mut command = Command::new(binary());
        command
            .arg(verb)
            .env_clear()
            .env("CANTOS_IDENTITY_DATABASE", &self.database)
            .env("CANTOS_IDENTITY_SOCKET", &self.socket)
            .env("CANTOS_IDENTITY_SERVER_UIDS", &self.uids)
            .env(
                "CANTOS_IDENTITY_SIGNUP_LINK_BASE",
                "https://theatre.example/signup",
            )
            .kill_on_drop(true)
            .stdin(Stdio::null());
        if let Some(port) = self.smtp_port {
            command
                .env("CANTOS_IDENTITY_SMTP_HOST", "127.0.0.1")
                .env("CANTOS_IDENTITY_SMTP_PORT", port.to_string())
                .env("CANTOS_IDENTITY_SMTP_TLS", "loopback-plaintext")
                .env("CANTOS_IDENTITY_SMTP_FROM", "cantos@example.test");
        }
        command
    }

    /// Runs `migrate` to completion and returns (success, stderr).
    pub async fn migrate(&self) -> (bool, String) {
        let output = self.command("migrate").output().await.unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    pub async fn serve(&self) -> Result<Instance, Failed> {
        Instance::start(self).await
    }
}

pub struct Failed {
    pub code: Option<i32>,
    pub stderr: String,
}

pub struct Instance {
    child: Child,
    out: Arc<Mutex<String>>,
    err: Arc<Mutex<String>>,
    pub socket: PathBuf,
}

impl Instance {
    async fn start(env: &Env) -> Result<Self, Failed> {
        let mut child = env
            .command("serve")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let out: Arc<Mutex<String>> = Arc::default();
        let err: Arc<Mutex<String>> = Arc::default();
        let (ready_tx, mut ready_rx) = tokio::sync::mpsc::unbounded_channel();
        let stdout = child.stdout.take().unwrap();
        let sink = Arc::clone(&out);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = ready_tx.send(line.clone());
                let mut guard = sink.lock().unwrap();
                guard.push_str(&line);
                guard.push('\n');
            }
        });
        let stderr = child.stderr.take().unwrap();
        let sink = Arc::clone(&err);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut guard = sink.lock().unwrap();
                guard.push_str(&line);
                guard.push('\n');
            }
        });
        let outcome = tokio::time::timeout(Duration::from_secs(30), async {
            tokio::select! {
                line = ready_rx.recv() => line.map(|_| ()).ok_or(()),
                _ = child.wait() => Err(()),
            }
        })
        .await;
        match outcome {
            Ok(Ok(())) => Ok(Self {
                child,
                out,
                err,
                socket: env.socket.clone(),
            }),
            _ => {
                let status = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                let _ = child.start_kill();
                tokio::time::sleep(Duration::from_millis(100)).await;
                Err(Failed {
                    code: status.ok().and_then(Result::ok).and_then(|s| s.code()),
                    stderr: err.lock().unwrap().clone(),
                })
            }
        }
    }

    pub fn pid(&self) -> u32 {
        self.child.id().unwrap()
    }

    pub fn ready_line(&self) -> String {
        self.out
            .lock()
            .unwrap()
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned()
    }

    /// Everything the process wrote to stdout and stderr so far.
    pub fn logs(&self) -> String {
        format!("{}{}", self.out.lock().unwrap(), self.err.lock().unwrap())
    }

    /// SIGTERM, then a bounded wait: the graceful path. The signal goes to this owned, not yet
    /// reaped child only, so its PID cannot have been reused; nothing on `PATH` is needed. A child
    /// that does not exit in time is killed and reaped before the test fails.
    pub async fn terminate(mut self) -> (std::process::ExitStatus, String) {
        let pid = libc::pid_t::try_from(self.pid()).expect("a process id fits pid_t");
        // SAFETY: kill(2) takes no pointers; the target is our own unreaped child.
        assert_eq!(
            unsafe { libc::kill(pid, libc::SIGTERM) },
            0,
            "signal the owned child"
        );
        let exit = match tokio::time::timeout(Duration::from_secs(30), self.child.wait()).await {
            Ok(exit) => exit.unwrap(),
            Err(_) => {
                let _ = self.child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await;
                panic!("process did not drain and exit after SIGTERM");
            }
        };
        tokio::time::sleep(Duration::from_millis(50)).await;
        (exit, self.logs())
    }

    /// SIGKILL: a crash that leaves its socket file behind.
    pub async fn crash(mut self) {
        self.child.kill().await.unwrap();
    }
}

// ---- RPC client over the Unix socket ------------------------------------------------------------

use cantos_identity_rpc::identity_client::IdentityClient;
use hyper_util::rt::TokioIo;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

pub async fn channel(socket: &Path) -> Result<Channel, tonic::transport::Error> {
    let path = socket.to_owned();
    Endpoint::try_from("http://identity.invalid")
        .unwrap()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .connect_with_connector(service_fn(move |_: Uri| {
            let path = path.clone();
            async move {
                Ok::<_, std::io::Error>(TokioIo::new(tokio::net::UnixStream::connect(path).await?))
            }
        }))
        .await
}

pub async fn client(socket: &Path) -> IdentityClient<Channel> {
    IdentityClient::new(channel(socket).await.expect("connect"))
}
