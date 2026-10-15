//! Bounded, captured execution of one external program with a typed failure vocabulary.
//!
//! [`Runner`](crate::command::Runner) inherits stdio and reports only success or failure. An
//! operation whose output must be interpreted (Android inspection, the Firebase CLI) needs the
//! bytes, a deadline, interruption and an explicit distinction between "the program ran and
//! failed" and "the outcome is unknown". The child owns a process group that is signalled before
//! its leader is reaped, so a timeout or Ctrl-C stops descendants too. Stopping a local child
//! never undoes a remote effect; callers decide what an interrupted write means.

use std::{
    fmt,
    io::{Read, Seek, SeekFrom},
    sync::atomic::AtomicBool,
    time::Duration,
};

use crate::command::CommandSpec;

/// Per-stream capture bound. Larger output fails instead of growing unbounded.
pub const OUTPUT_LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Whatever the child had written before it was stopped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Partial {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    /// The program could not be started; no effect of the program can have happened.
    Spawn {
        program: String,
        message: String,
    },
    TimedOut {
        after: Duration,
        partial: Partial,
    },
    /// `started` is false when the stop request arrived before the program was launched.
    Interrupted {
        partial: Partial,
        started: bool,
    },
    OutputLimit {
        partial: Partial,
    },
    /// Local plumbing (temporary files, waiting) failed after the program may have started.
    Plumbing {
        message: String,
        started: bool,
    },
}

impl CaptureError {
    /// Whether the program may have started and so may have had effects.
    pub fn may_have_run(&self) -> bool {
        match self {
            CaptureError::Spawn { .. } => false,
            CaptureError::Plumbing { started, .. } | CaptureError::Interrupted { started, .. } => {
                *started
            }
            _ => true,
        }
    }

    pub fn partial(&self) -> Option<&Partial> {
        match self {
            CaptureError::TimedOut { partial, .. }
            | CaptureError::Interrupted { partial, .. }
            | CaptureError::OutputLimit { partial } => Some(partial),
            _ => None,
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaptureError::Spawn { program, message } => {
                write!(f, "could not start `{program}`: {message}")
            }
            CaptureError::TimedOut { after, .. } => {
                write!(f, "timed out after {}s", after.as_secs())
            }
            CaptureError::Interrupted { .. } => f.write_str("interrupted"),
            CaptureError::OutputLimit { .. } => f.write_str("output exceeded the 4 MiB bound"),
            CaptureError::Plumbing { message, .. } => write!(f, "local capture failed: {message}"),
        }
    }
}

/// Runs `spec` to completion (or its bound), capturing both streams.
pub trait Capture {
    fn capture(&mut self, spec: &CommandSpec, timeout: Duration) -> Result<Captured, CaptureError>;
}

/// The real implementation. Owns the interruption flag that Ctrl-C/SIGTERM/SIGHUP set.
pub struct SystemCapture {
    interrupted: std::sync::Arc<AtomicBool>,
    #[cfg(unix)]
    signals: Vec<signal_hook::SigId>,
}

impl SystemCapture {
    pub fn new() -> anyhow::Result<Self> {
        let interrupted = std::sync::Arc::new(AtomicBool::new(false));
        #[cfg(unix)]
        let mut signals = Vec::new();
        #[cfg(unix)]
        for signal in [
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGHUP,
        ] {
            signals.push(signal_hook::flag::register(
                signal,
                std::sync::Arc::clone(&interrupted),
            )?);
        }
        Ok(Self {
            interrupted,
            #[cfg(unix)]
            signals,
        })
    }
}

#[cfg(unix)]
impl Drop for SystemCapture {
    fn drop(&mut self) {
        for id in self.signals.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}

impl Capture for SystemCapture {
    fn capture(&mut self, spec: &CommandSpec, timeout: Duration) -> Result<Captured, CaptureError> {
        capture_command(spec, timeout, &self.interrupted)
    }
}

#[cfg(not(unix))]
fn capture_command(
    _spec: &CommandSpec,
    _timeout: Duration,
    _interrupted: &AtomicBool,
) -> Result<Captured, CaptureError> {
    Err(CaptureError::Plumbing {
        message: "captured execution requires a POSIX host".to_owned(),
        started: false,
    })
}

#[cfg(unix)]
fn capture_command(
    spec: &CommandSpec,
    timeout: Duration,
    interrupted: &AtomicBool,
) -> Result<Captured, CaptureError> {
    use std::{
        fs::{DirBuilder, File, OpenOptions},
        os::unix::{
            fs::{DirBuilderExt, OpenOptionsExt},
            process::CommandExt,
        },
        process::Stdio,
        sync::atomic::{AtomicU64, Ordering},
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let plumbing = |message: String, started: bool| CaptureError::Plumbing { message, started };

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let directory = std::env::temp_dir().join(format!(
        "cantos-capture-{}-{nanos}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|error| plumbing(error.to_string(), false))?;
    struct Temporary(std::path::PathBuf);
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _temporary = Temporary(directory.clone());
    let open = |name: &str| {
        OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(directory.join(name))
            .map_err(|error| plumbing(error.to_string(), false))
    };
    let stdout = open("stdout")?;
    let stderr = open("stderr")?;
    if interrupted.load(Ordering::Relaxed) {
        return Err(CaptureError::Interrupted {
            partial: Partial::default(),
            started: false,
        });
    }

    let mut command = spec.to_command();
    let child = command
        .stdin(Stdio::null())
        .stdout(
            stdout
                .try_clone()
                .map_err(|error| plumbing(error.to_string(), false))?,
        )
        .stderr(
            stderr
                .try_clone()
                .map_err(|error| plumbing(error.to_string(), false))?,
        )
        .process_group(0)
        .spawn()
        .map_err(|error| CaptureError::Spawn {
            program: spec.program.clone(),
            message: error.kind().to_string(),
        })?;

    struct OwnedChild(std::process::Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            // The leader is deliberately not reaped before the group is signalled, so its ID
            // cannot be reused as another group while descendants are still alive.
            // SAFETY: signals only the process group this function created.
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.wait();
        }
    }
    let child = OwnedChild(child);

    let snapshot = || Partial {
        stdout: read_stream(&stdout),
        stderr: read_stream(&stderr),
    };

    let start = Instant::now();
    let code = loop {
        if interrupted.load(Ordering::Relaxed) {
            let partial = snapshot();
            drop(child);
            return Err(CaptureError::Interrupted {
                partial,
                started: true,
            });
        }
        if start.elapsed() >= timeout {
            let partial = snapshot();
            drop(child);
            return Err(CaptureError::TimedOut {
                after: timeout,
                partial,
            });
        }
        let too_large = |file: &File| file.metadata().is_ok_and(|m| m.len() > OUTPUT_LIMIT);
        if too_large(&stdout) || too_large(&stderr) {
            let partial = snapshot();
            drop(child);
            return Err(CaptureError::OutputLimit { partial });
        }
        // waitid with WNOWAIT observes completion without freeing the group leader's PID.
        // SAFETY: siginfo_t accepts zero initialization; waitid writes only this valid pointer.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.0.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            let message = std::io::Error::last_os_error().to_string();
            drop(child);
            return Err(plumbing(message, true));
        }
        // SAFETY: a successful waitid with WEXITED initialized the child-status fields.
        if unsafe { info.si_pid() } != 0 {
            let status = unsafe { info.si_status() };
            break if info.si_code == libc::CLD_EXITED {
                status
            } else {
                128 + status
            };
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    // Dropping kills any remaining descendants before reaping the leader.
    drop(child);
    let too_large = |file: &File| file.metadata().map_or(true, |m| m.len() > OUTPUT_LIMIT);
    if too_large(&stdout) || too_large(&stderr) {
        return Err(CaptureError::OutputLimit {
            partial: snapshot(),
        });
    }
    let partial = snapshot();
    Ok(Captured {
        code,
        stdout: partial.stdout,
        stderr: partial.stderr,
    })
}

/// Best-effort bounded read of a capture file from its start.
#[cfg(unix)]
fn read_stream(file: &std::fs::File) -> String {
    let mut seekable = file;
    let mut bytes = Vec::new();
    let _ = seekable.seek(SeekFrom::Start(0));
    let _ = file.take(OUTPUT_LIMIT).read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
pub mod testing {
    use super::*;
    use std::collections::VecDeque;

    /// Replays scripted results and records every invocation.
    #[derive(Default)]
    pub struct ScriptedCapture {
        pub calls: Vec<CommandSpec>,
        pub results: VecDeque<Result<Captured, CaptureError>>,
    }

    impl ScriptedCapture {
        pub fn push_ok(&mut self, code: i32, stdout: &str, stderr: &str) -> &mut Self {
            self.results.push_back(Ok(Captured {
                code,
                stdout: stdout.to_owned(),
                stderr: stderr.to_owned(),
            }));
            self
        }
    }

    impl Capture for ScriptedCapture {
        fn capture(
            &mut self,
            spec: &CommandSpec,
            _timeout: Duration,
        ) -> Result<Captured, CaptureError> {
            self.calls.push(spec.clone());
            self.results
                .pop_front()
                .expect("a scripted capture result for every invocation")
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn sh(script: &str) -> CommandSpec {
        CommandSpec::new("sh", std::env::temp_dir()).args(["-c", script])
    }

    #[test]
    fn captures_both_streams_and_the_exit_code() {
        let flag = AtomicBool::new(false);
        let captured = capture_command(
            &sh("printf out; printf err >&2; exit 3"),
            Duration::from_secs(10),
            &flag,
        )
        .unwrap();
        assert_eq!(
            captured,
            Captured {
                code: 3,
                stdout: "out".into(),
                stderr: "err".into()
            }
        );
    }

    #[test]
    fn a_missing_program_is_a_spawn_failure_that_cannot_have_run() {
        let flag = AtomicBool::new(false);
        let error = capture_command(
            &CommandSpec::new("cantos-no-such-program", std::env::temp_dir()),
            Duration::from_secs(1),
            &flag,
        )
        .unwrap_err();
        assert!(matches!(error, CaptureError::Spawn { .. }), "{error:?}");
        assert!(!error.may_have_run());
    }

    #[test]
    fn a_timeout_keeps_the_partial_output_and_stops_the_group() {
        let flag = AtomicBool::new(false);
        let error = capture_command(
            &sh("printf started; sleep 30"),
            Duration::from_millis(400),
            &flag,
        )
        .unwrap_err();
        match &error {
            CaptureError::TimedOut { partial, .. } => assert_eq!(partial.stdout, "started"),
            other => panic!("{other:?}"),
        }
        assert!(error.may_have_run());
    }

    #[test]
    fn interruption_before_start_and_during_a_run_are_reported() {
        let flag = AtomicBool::new(true);
        let error = capture_command(&sh("exit 0"), Duration::from_secs(5), &flag).unwrap_err();
        assert!(
            matches!(error, CaptureError::Interrupted { .. }),
            "{error:?}"
        );
        flag.store(false, Ordering::Relaxed);
        let flag = std::sync::Arc::new(AtomicBool::new(false));
        let setter = std::sync::Arc::clone(&flag);
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            setter.store(true, Ordering::Relaxed);
        });
        let error = capture_command(&sh("printf half; sleep 30"), Duration::from_secs(20), &flag)
            .unwrap_err();
        handle.join().unwrap();
        match error {
            CaptureError::Interrupted {
                partial,
                started: true,
            } => assert_eq!(partial.stdout, "half"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn oversized_output_is_refused_not_buffered() {
        let flag = AtomicBool::new(false);
        let error = capture_command(
            &sh("head -c 5000000 /dev/zero"),
            Duration::from_secs(20),
            &flag,
        )
        .unwrap_err();
        assert!(
            matches!(error, CaptureError::OutputLimit { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn working_directory_and_environment_come_from_the_spec() {
        let flag = AtomicBool::new(false);
        let dir = std::env::temp_dir();
        let spec = CommandSpec::new("sh", &dir)
            .args(["-c", "printf '%s|%s' \"$PWD\" \"$CANTOS_CAPTURE_PROBE\""])
            .env("CANTOS_CAPTURE_PROBE", "ünï code");
        let captured = capture_command(&spec, Duration::from_secs(5), &flag).unwrap();
        let (pwd, probe) = captured.stdout.split_once('|').unwrap();
        assert_eq!(
            std::fs::canonicalize(pwd).unwrap(),
            std::fs::canonicalize(&dir).unwrap()
        );
        assert_eq!(probe, "ünï code");
    }
}
