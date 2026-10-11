//! Lifecycle of long-running local processes started by `run` and `app`.
//!
//! Each child starts in its own process group, so stopping it also stops the build tools it
//! spawned (Trunk runs Cargo and wasm-bindgen). xtask only ever signals groups it created in
//! this invocation; it never looks up or stops processes by port or name. Ctrl-C, SIGTERM and
//! SIGHUP are caught while children run and turned into an orderly shutdown: SIGINT to each
//! group (Axum and Trunk shut down gracefully on it), then SIGKILL after a grace period.
//! If xtask itself is killed with SIGKILL its children survive; the next run then finds their
//! ports taken and refuses to start rather than stopping them.

use std::{
    net::SocketAddr,
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
    probe::{self, Probe, Readiness},
};

const POLL: Duration = Duration::from_millis(200);
const GRACE: Duration = Duration::from_secs(10);

struct Managed {
    name: String,
    child: Child,
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
        });
        Ok(())
    }

    /// Polls the most recently spawned process until `readiness` holds at `addr`.
    /// Returns `Ok(false)` if interrupted first; fails if the process exits or times out.
    pub fn await_ready(
        &mut self,
        addr: SocketAddr,
        readiness: &Readiness,
        timeout: Duration,
    ) -> Result<bool> {
        let deadline = Instant::now() + timeout;
        loop {
            if self.interrupted() {
                return Ok(false);
            }
            let managed = self
                .children
                .last_mut()
                .context("no process has been started")?;
            if let Some(status) = managed.child.try_wait()? {
                bail!(
                    "{} exited with {status} before becoming ready at {addr}",
                    managed.name
                );
            }
            // The port was free when the child started, so an unexpected answer is most likely
            // the child itself still building; keep waiting and report it on timeout.
            let last = match probe::probe(addr, readiness) {
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
                    managed.name,
                    timeout.as_secs()
                );
            }
            thread::sleep(POLL);
        }
    }

    /// Blocks until the user interrupts or any supervised process exits.
    pub fn wait(&mut self) -> Result<Wait> {
        loop {
            if self.interrupted() {
                return Ok(Wait::Interrupted);
            }
            for managed in &mut self.children {
                if let Some(status) = managed.child.try_wait()? {
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
            if let Ok(Some(_)) = managed.child.try_wait() {
                continue;
            }
            println!("Stopping {} (pid {})", managed.name, managed.child.id());
            terminate(&mut managed.child, self.grace);
        }
    }
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
        if let Ok(Some(_)) = child.try_wait() {
            #[cfg(unix)]
            signal_group(child, libc::SIGKILL); // stragglers left in the group
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
    use std::{net::TcpListener, path::PathBuf};

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
}
