//! Lifecycle of long-running local processes started by `run` and `app`.
//!
//! Each child starts in its own process group, so stopping it also stops the build tools it
//! spawned (Trunk runs Cargo and wasm-bindgen). xtask only ever signals groups it created in
//! this invocation; it never looks up or stops processes by port or name. Ctrl-C, SIGTERM and
//! SIGHUP are caught while children run and turned into an orderly shutdown: SIGINT to each
//! group (Axum and Trunk shut down gracefully on it), then SIGKILL after a grace period.
//! If xtask itself is killed with SIGKILL its children survive; the next run then finds their
//! ports taken and refuses to start rather than stopping them.

#[cfg(test)]
use std::net::SocketAddr;

use std::{
    process::{Child, ExitStatus},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};

use crate::{
    command::CommandSpec,
    probe::{self, Endpoint, Probe, Readiness},
};

const POLL: Duration = Duration::from_millis(200);
const GRACE: Duration = Duration::from_secs(10);

struct Managed {
    name: String,
    child: Child,
    group_stopped: bool,
}

/// How waiting ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Wait {
    /// The user asked to stop (Ctrl-C, SIGTERM or SIGHUP).
    Interrupted,
    /// A supervised process exited on its own.
    Exited { name: String, status: ExitStatus },
}

pub struct Supervisor {
    children: Vec<Managed>,
    stop: Arc<AtomicBool>,
    grace: Duration,
}

impl Supervisor {
    /// Adopt a directly spawned child already placed in its own process group. Used after the
    /// Auth Emulator's bounded private readiness handshake, never for an existing process.
    pub fn adopt(&mut self, name: &str, child: Child) {
        self.children.push(Managed {
            name: name.to_owned(),
            child,
            group_stopped: false,
        });
    }

    /// Installs the stop-signal handlers for the rest of this process's life.
    pub fn new() -> Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        #[cfg(unix)]
        for signal in [
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
        ] {
            signal_hook::flag::register(signal, Arc::clone(&stop))
                .context("could not install the stop-signal handler")?;
        }
        Ok(Self::with_stop_flag(stop, GRACE))
    }

    fn with_stop_flag(stop: Arc<AtomicBool>, grace: Duration) -> Self {
        Self {
            children: Vec::new(),
            stop,
            grace,
        }
    }

    pub fn interrupted(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// Starts `spec` in a new process group with inherited stdio.
    pub fn spawn(&mut self, name: &str, spec: &CommandSpec) -> Result<()> {
        let mut command = spec.to_command();
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let child = command
            .spawn()
            .with_context(|| format!("could not start {name} (`{}`)", spec.program))?;
        self.children.push(Managed {
            name: name.to_owned(),
            child,
            group_stopped: false,
        });
        Ok(())
    }

    /// [`Self::await_endpoint`] for a loopback TCP address.
    #[cfg(test)]
    pub fn await_ready(
        &mut self,
        addr: SocketAddr,
        readiness: &Readiness,
        timeout: Duration,
    ) -> Result<bool> {
        self.await_endpoint(&Endpoint::Tcp(addr), readiness, timeout)
    }

    /// Polls the most recently spawned process until `readiness` holds at `endpoint`.
    /// Returns `Ok(false)` if interrupted first; fails if the process exits or times out.
    pub fn await_endpoint(
        &mut self,
        endpoint: &Endpoint,
        readiness: &Readiness,
        timeout: Duration,
    ) -> Result<bool> {
        let addr = endpoint;
        let deadline = Instant::now() + timeout;
        let mut backoff = POLL;
        loop {
            if self.interrupted() {
                return Ok(false);
            }
            // An earlier dependency may die while the latest service is still compiling.
            for managed in &mut self.children {
                if let Some(status) = poll_exit(managed)? {
                    bail!(
                        "{} exited with {status} before becoming ready at {addr}",
                        managed.name
                    );
                }
            }
            let name = &self
                .children
                .last()
                .context("no process has been started")?
                .name;
            // The port was free when the child started, so an unexpected answer is most likely
            // the child itself still building; keep waiting and report it on timeout.
            let last = match probe::probe_endpoint(endpoint, readiness) {
                Probe::Ready => return Ok(true),
                other => other,
            };
            if Instant::now() >= deadline {
                let observed = match last {
                    Probe::Unexpected(answer) => format!("last answer: {answer}"),
                    _ => "nothing answered".to_owned(),
                };
                bail!(
                    "{} did not become ready at {addr} within {}s ({observed})",
                    name,
                    timeout.as_secs()
                );
            }
            thread::sleep(backoff.min(deadline.saturating_duration_since(Instant::now())));
            backoff = (backoff * 2).min(Duration::from_secs(1));
        }
    }

    /// Blocks until the user interrupts or any supervised process exits.
    pub fn wait(&mut self) -> Result<Wait> {
        loop {
            if self.interrupted() {
                return Ok(Wait::Interrupted);
            }
            for managed in &mut self.children {
                if let Some(status) = poll_exit(managed)? {
                    return Ok(Wait::Exited {
                        name: managed.name.clone(),
                        status,
                    });
                }
            }
            thread::sleep(POLL);
        }
    }

    /// Stops every supervised process group, newest first, and reaps it.
    pub fn shutdown(&mut self) {
        while let Some(mut managed) = self.children.pop() {
            if managed.group_stopped {
                continue;
            }
            println!("Stopping {} (pid {})", managed.name, managed.child.id());
            terminate(&mut managed.child, self.grace);
        }
    }
}

/// Observe Unix exit without reaping: the leader reserves its identity until the final owned
/// group signal. Reaping first could leave descendants alive or allow that identity to be reused.
fn exited_without_reaping(child: &mut Child) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        // SAFETY: siginfo_t accepts zero initialization. waitid writes this valid pointer,
        // selects only our child, and WNOWAIT deliberately retains its waitable status.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == -1 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: successful waitid with WEXITED initializes the child-status fields.
        Ok(unsafe { info.si_pid() } != 0)
    }
    #[cfg(not(unix))]
    {
        child.try_wait().map(|status| status.is_some())
    }
}

fn poll_exit(managed: &mut Managed) -> std::io::Result<Option<ExitStatus>> {
    if managed.group_stopped {
        return managed.child.try_wait();
    }
    if !exited_without_reaping(&mut managed.child)? {
        return Ok(None);
    }
    #[cfg(unix)]
    signal_group(&managed.child, libc::SIGKILL);
    managed.group_stopped = true;
    managed.child.wait().map(Some)
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(unix)]
fn signal_group(child: &Child, signal: libc::c_int) {
    let Ok(pid) = libc::pid_t::try_from(child.id()) else {
        return;
    };
    // SAFETY: `kill` has no memory-safety preconditions. The negative pid addresses only the
    // process group this supervisor created with `process_group(0)` for this child. Its id is
    // not reused while the leader is unreaped or any member (a straggler) is still alive.
    unsafe {
        libc::kill(-pid, signal);
    }
}

fn terminate(child: &mut Child, grace: Duration) {
    #[cfg(unix)]
    signal_group(child, libc::SIGINT);
    #[cfg(not(unix))]
    let _ = child.kill();
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if let Ok(true) = exited_without_reaping(child) {
            #[cfg(unix)]
            signal_group(child, libc::SIGKILL); // stragglers left in the group
            let _ = child.wait();
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    #[cfg(unix)]
    signal_group(child, libc::SIGKILL);
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        io::Read,
        net::TcpListener,
        os::{fd::AsRawFd, unix::process::CommandExt},
        path::PathBuf,
        process::Stdio,
    };

    fn shell(script: &str) -> CommandSpec {
        CommandSpec::new("sh", std::env::temp_dir()).args(["-c", script])
    }

    fn supervisor() -> Supervisor {
        Supervisor::with_stop_flag(Arc::new(AtomicBool::new(false)), Duration::from_secs(5))
    }

    /// Running and not a zombie awaiting its (re-parented) reaper.
    fn alive(pid: i32) -> bool {
        let output = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .unwrap();
        let state = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        !state.is_empty() && !state.starts_with('Z')
    }

    #[test]
    fn shutdown_stops_the_whole_process_group_including_grandchildren() {
        let pid_file: PathBuf =
            std::env::temp_dir().join(format!("xtask-grandchild-{}", std::process::id()));
        let mut supervisor = supervisor();
        supervisor
            .spawn(
                "tree",
                &shell(&format!(
                    "sleep 60 & echo $! > {}; wait",
                    pid_file.display()
                )),
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let grandchild: i32 = loop {
            if let Some(pid) = std::fs::read_to_string(&pid_file)
                .ok()
                .and_then(|text| text.trim().parse().ok())
            {
                break pid;
            }
            assert!(Instant::now() < deadline, "grandchild did not start");
            thread::sleep(Duration::from_millis(20));
        };
        assert!(alive(grandchild));
        let started = Instant::now();
        supervisor.shutdown();
        assert!(started.elapsed() < Duration::from_secs(5));
        let deadline = Instant::now() + Duration::from_secs(2);
        while alive(grandchild) {
            assert!(Instant::now() < deadline, "grandchild survived shutdown");
            thread::sleep(Duration::from_millis(20));
        }
        let _ = std::fs::remove_file(pid_file);
    }

    #[test]
    fn wait_reports_a_child_that_exits_on_its_own() {
        let mut supervisor = supervisor();
        supervisor.spawn("short", &shell("exit 7")).unwrap();
        match supervisor.wait().unwrap() {
            Wait::Exited { name, status } => {
                assert_eq!(name, "short");
                assert_eq!(status.code(), Some(7));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn an_exited_launcher_cannot_leave_a_descendant_holding_its_owned_pipe() {
        // The pipe observes only our descendants; no process-name or PID lookup is needed.
        // Stdout's pipe goes only to this child; parallel test children cannot retain it.
        let script = "import os,signal,time\nchild=os.fork()\nif child:\n os.write(1,b'ready')\n os._exit(7)\nsignal.signal(signal.SIGINT,signal.SIG_IGN)\ntime.sleep(60)";
        let mut child = CommandSpec::new("python3", std::env::temp_dir())
            .args(["-c", script])
            .to_command()
            .stdout(Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap();
        let mut reader = child.stdout.take().unwrap();
        let reader_fd = reader.as_raw_fd();
        let mut supervisor = supervisor();
        supervisor.children.push(Managed {
            name: "exited launcher".to_owned(),
            child,
            group_stopped: false,
        });
        // SAFETY: this is the valid reader descriptor, still owned by reader.
        assert_eq!(
            unsafe { libc::fcntl(reader_fd, libc::F_SETFL, libc::O_NONBLOCK) },
            0
        );
        let mut readiness = [0; 5];
        let mut received = 0;
        let deadline = Instant::now() + Duration::from_secs(5);
        while received < readiness.len() {
            match reader.read(&mut readiness[received..]) {
                Ok(0) => panic!("owned launcher exited before pipe readiness"),
                Ok(count) => received += count,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "owned launcher did not become ready"
                    );
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => panic!("owned pipe readiness failed: {error}"),
            }
        }
        assert_eq!(&readiness, b"ready");
        assert!(
            matches!(supervisor.wait().unwrap(), Wait::Exited { status, .. } if status.code() == Some(7))
        );
        supervisor.shutdown();
        // A surviving pipe owner fails within a bounded time.
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut byte = [0];
        loop {
            match reader.read(&mut byte) {
                Ok(0) => break,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "owned descendant survived the exited launcher"
                    );
                    thread::sleep(Duration::from_millis(20));
                }
                other => panic!("unexpected pipe state: {other:?}"),
            }
        }
    }

    #[test]
    fn the_stop_flag_ends_waiting_without_killing_by_itself() {
        let stop = Arc::new(AtomicBool::new(false));
        let mut supervisor = Supervisor::with_stop_flag(Arc::clone(&stop), Duration::from_secs(5));
        supervisor.spawn("long", &shell("sleep 60")).unwrap();
        stop.store(true, Ordering::SeqCst);
        assert_eq!(supervisor.wait().unwrap(), Wait::Interrupted);
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        assert!(!supervisor
            .await_ready(addr, &Readiness::Tcp, Duration::from_secs(1))
            .unwrap());
        supervisor.shutdown();
    }

    #[test]
    fn readiness_fails_fast_when_the_process_exits_and_on_timeout() {
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let mut crash = supervisor();
        crash.spawn("crash", &shell("exit 1")).unwrap();
        let error = crash
            .await_ready(addr, &Readiness::Tcp, Duration::from_secs(10))
            .unwrap_err();
        assert!(
            error.to_string().contains("before becoming ready"),
            "{error}"
        );

        let mut silent = supervisor();
        silent.spawn("silent", &shell("sleep 60")).unwrap();
        let error = silent
            .await_ready(addr, &Readiness::Tcp, Duration::from_millis(300))
            .unwrap_err();
        assert!(
            error.to_string().contains("did not become ready"),
            "{error}"
        );
    }

    #[test]
    fn an_earlier_dependency_exit_aborts_later_service_readiness() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let mut supervisor = supervisor();
        supervisor
            .spawn("api dependency", &shell("exit 7"))
            .unwrap();
        supervisor
            .spawn("frontend building", &shell("sleep 60"))
            .unwrap();
        let started = Instant::now();
        let error = supervisor
            .await_ready(addr, &Readiness::Tcp, Duration::from_secs(10))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("api dependency exited with exit status: 7 before becoming ready"),
            "{error}"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        supervisor.shutdown();
        assert!(supervisor.children.is_empty());
    }

    #[test]
    fn readiness_succeeds_once_the_child_listens() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let addr = crate::config::loopback(port);
        let mut supervisor = supervisor();
        supervisor
            .spawn(
                "listener",
                &CommandSpec::new("python3", std::env::temp_dir()).args([
                    "-c",
                    &format!(
                        "import socket,time\ns=socket.socket()\ns.bind(('127.0.0.1',{port}))\ns.listen()\ntime.sleep(60)"
                    ),
                ]),
            )
            .unwrap();
        assert!(supervisor
            .await_ready(addr, &Readiness::Tcp, Duration::from_secs(20))
            .unwrap());
        supervisor.shutdown();
        assert!(probe::port_is_free(addr));
    }

    #[test]
    fn a_unix_socket_endpoint_becomes_ready_once_the_child_listens_and_never_for_a_crash() {
        let dir = std::env::temp_dir().join(format!("xtask-uds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("i.sock");
        let endpoint = Endpoint::Unix(path.clone());
        let mut supervisor = supervisor();
        supervisor
            .spawn(
                "uds listener",
                &CommandSpec::new("python3", std::env::temp_dir()).args([
                    "-c",
                    &format!(
                        "import socket,time\ns=socket.socket(socket.AF_UNIX)\ns.bind('{}')\ns.listen()\ntime.sleep(60)",
                        path.display()
                    ),
                ]),
            )
            .unwrap();
        assert!(supervisor
            .await_endpoint(&endpoint, &Readiness::Tcp, Duration::from_secs(20))
            .unwrap());
        supervisor.shutdown();
        let mut crash = self::supervisor();
        crash.spawn("crash", &shell("exit 1")).unwrap();
        let error = crash
            .await_endpoint(
                &Endpoint::Unix(dir.join("none.sock")),
                &Readiness::Tcp,
                Duration::from_secs(10),
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("before becoming ready"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
