//! The only container I/O boundary. Bounded output/time, argv execution and owned process
//! groups keep a stuck remote connection from defeating the readiness timeout.
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{bail, ensure, Context, Result};

#[derive(Debug)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait Podman {
    fn invoke(&mut self, args: &[String], timeout: Duration) -> Result<Output>;
}

pub struct SystemPodman {
    connection: Option<String>,
    selected: bool,
    interrupted: std::sync::Arc<std::sync::atomic::AtomicBool>,
    #[cfg(unix)]
    signals: Vec<signal_hook::SigId>,
}

impl SystemPodman {
    pub fn new() -> Result<Self> {
        let mut runner = Self {
            connection: None,
            selected: false,
            interrupted: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(unix)]
            signals: Vec::new(),
        };
        #[cfg(unix)]
        for signal in [
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
        ] {
            runner.signals.push(signal_hook::flag::register(
                signal,
                std::sync::Arc::clone(&runner.interrupted),
            )?);
        }
        Ok(runner)
    }
}

#[cfg(unix)]
impl Drop for SystemPodman {
    fn drop(&mut self) {
        for id in self.signals.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}

impl Podman for SystemPodman {
    fn invoke(&mut self, args: &[String], timeout: Duration) -> Result<Output> {
        if !self.selected {
            if std::env::consts::OS == "macos" {
                let listing = bounded_command(
                    "podman",
                    &[
                        "system".into(),
                        "connection".into(),
                        "list".into(),
                        "--format=json".into(),
                    ],
                    timeout,
                    &self.interrupted,
                )?;
                self.connection = Some(local_machine_connection(&success(
                    listing,
                    "connection list",
                )?)?);
            }
            self.selected = true;
        }
        let mut local = vec![match &self.connection {
            Some(name) => format!("--connection={name}"),
            None => "--remote=false".into(),
        }];
        local.extend_from_slice(args);
        bounded_command("podman", &local, timeout, &self.interrupted)
    }
}

pub(super) fn local_machine_connection(text: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Connection {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "URI")]
        uri: String,
        #[serde(rename = "Default")]
        default: bool,
    }
    let connections: Vec<Connection> =
        serde_json::from_str(text).context("invalid Podman connection listing")?;
    let defaults: Vec<_> = connections
        .into_iter()
        .filter(|connection| connection.default)
        .collect();
    ensure!(defaults.len() == 1, "UNSUPPORTED: macOS needs one existing default Podman Machine connection; no attachment was changed");
    let connection = &defaults[0];
    let local = connection
        .uri
        .strip_prefix("ssh://")
        .and_then(|ssh| ssh.split_once('@'))
        .and_then(|(user, target)| {
            if !user
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            {
                return None;
            }
            let (endpoint, path) = target.split_once('/')?;
            let port = endpoint
                .strip_prefix("127.0.0.1:")
                .or_else(|| endpoint.strip_prefix("localhost:"))
                .or_else(|| endpoint.strip_prefix("[::1]:"))?
                .parse::<u16>()
                .ok()?;
            (port > 0 && path.starts_with("run/user/") && path.ends_with("/podman/podman.sock"))
                .then_some(())
        })
        .is_some();
    ensure!(local && !connection.name.is_empty() && connection.name.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)), "UNSUPPORTED: default connection must be existing loopback SSH to rootless Podman Machine; remote/cloud targets are refused");
    Ok(connection.name.clone())
}

const OUTPUT_LIMIT: u64 = 4 * 1024 * 1024;

#[cfg(unix)]
pub fn bounded_command(
    program: &str,
    args: &[String],
    timeout: Duration,
    interrupted: &std::sync::atomic::AtomicBool,
) -> Result<Output> {
    use std::{
        fs::{DirBuilder, File, OpenOptions},
        os::unix::{
            fs::{DirBuilderExt, OpenOptionsExt},
            process::CommandExt,
        },
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "cantos-service-command-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    DirBuilder::new().mode(0o700).create(&directory)?;
    struct Temporary(std::path::PathBuf);
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _temporary = Temporary(directory.clone());
    let file = |name: &str| {
        OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(directory.join(name))
    };
    let stdout = file("stdout")?;
    let stderr = file("stderr")?;
    ensure!(
        !interrupted.load(Ordering::Relaxed),
        "service command interrupted; inspect retained container state before retry"
    );
    let mut command = Command::new(program);
    if program == "podman" {
        // Freeze the observed local target instead of inheriting a cloud/remote override.
        command
            .env_remove("CONTAINER_HOST")
            .env_remove("CONTAINER_CONNECTION");
    }
    let child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .process_group(0)
        .spawn()
        .with_context(|| format!("could not start {program}"))?;
    struct OwnedChild(std::process::Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            // This leader is deliberately not reaped before group signalling, so its ID cannot
            // be reused as another process group's ID even when it exits before its descendants.
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.wait();
        }
    }
    let child = OwnedChild(child);
    let start = Instant::now();
    let code = loop {
        ensure!(
            !interrupted.load(Ordering::Relaxed),
            "service command interrupted; inspect retained container state before retry"
        );
        ensure!(
            start.elapsed() < timeout,
            "service command timed out after {}s; inspect retained state before retry",
            timeout.as_secs()
        );
        ensure!(
            stdout.metadata()?.len() <= OUTPUT_LIMIT && stderr.metadata()?.len() <= OUTPUT_LIMIT,
            "service command exceeded 4 MiB output limit"
        );
        // waitid with WNOWAIT observes completion without freeing the group leader's PID.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.0.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        ensure!(
            result == 0,
            "could not observe owned service command: {}",
            std::io::Error::last_os_error()
        );
        if unsafe { info.si_pid() } != 0 {
            // Drop kills any remaining descendants before reaping the leader.
            let status = unsafe { info.si_status() };
            break if info.si_code == libc::CLD_EXITED {
                status
            } else {
                128 + status
            };
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    drop(child);
    let read = |mut file: File| -> Result<String> {
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        file.take(OUTPUT_LIMIT + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= OUTPUT_LIMIT,
            "service command exceeded 4 MiB output limit"
        );
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    };
    Ok(Output {
        code,
        stdout: read(stdout)?,
        stderr: read(stderr)?,
    })
}

#[cfg(not(unix))]
pub fn bounded_command(
    _program: &str,
    _args: &[String],
    _timeout: Duration,
    _interrupted: &std::sync::atomic::AtomicBool,
) -> Result<Output> {
    bail!("UNSUPPORTED: services require Linux rootless Podman or macOS Podman Machine")
}

pub fn success(output: Output, operation: &str) -> Result<String> {
    if output.code != 0 {
        bail!(
            "{operation} exited with {}: {}",
            output.code,
            output.stderr.trim()
        );
    }
    Ok(output.stdout)
}
