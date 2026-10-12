//! Bounded read-only media tool invocation. No shell, encoder, URL, or user argument is used.
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::audio_intake::AudioIntakeError;

const TIME_LIMIT: Duration = Duration::from_secs(8);
const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;

#[cfg(unix)]
pub(super) fn run(command: &mut Command) -> Result<Vec<u8>, AudioIntakeError> {
    run_limited(command, TIME_LIMIT)
}

#[cfg(unix)]
fn run_limited(command: &mut Command, time_limit: Duration) -> Result<Vec<u8>, AudioIntakeError> {
    use std::os::unix::process::ExitStatusExt;
    use std::os::{fd::AsRawFd, unix::process::CommandExt};

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // The child owns a process group so timeout/cancellation cannot affect another process.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            for (resource, limit) in [
                (libc::RLIMIT_CPU, 6),
                (libc::RLIMIT_AS, 512 * 1024 * 1024),
                (libc::RLIMIT_NOFILE, 32),
                (libc::RLIMIT_CORE, 0),
            ] {
                let value = libc::rlimit {
                    rlim_cur: limit,
                    rlim_max: limit,
                };
                if libc::setrlimit(resource, &value) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .map_err(|_| AudioIntakeError::InspectionUnavailable)?;
    let pid = child.id();
    let mut stdout = child
        .stdout
        .take()
        .ok_or(AudioIntakeError::InspectionUnavailable)?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or(AudioIntakeError::InspectionUnavailable)?;
    for fd in [stdout.as_raw_fd(), stderr.as_raw_fd()] {
        if unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) } == -1 {
            terminate(&mut child, pid);
            return Err(AudioIntakeError::InspectionUnavailable);
        }
    }
    let deadline = Instant::now() + time_limit;
    let mut out = Vec::new();
    let mut err = Vec::new();
    loop {
        if let Err(error) = drain(&mut stdout, &mut out).and_then(|()| drain(&mut stderr, &mut err))
        {
            terminate(&mut child, pid);
            return Err(error);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                drain(&mut stdout, &mut out)?;
                drain(&mut stderr, &mut err)?;
                // Decode errors and even nonfatal parser warnings at -loglevel error fail closed.
                return if status.success() && err.is_empty() {
                    Ok(out)
                } else if matches!(status.signal(), Some(libc::SIGXCPU) | Some(libc::SIGKILL)) {
                    Err(AudioIntakeError::InspectionTimeout)
                } else {
                    Err(AudioIntakeError::MalformedAudio)
                };
            }
            Ok(None) => (),
            Err(_) => {
                terminate(&mut child, pid);
                return Err(AudioIntakeError::InspectionUnavailable);
            }
        }
        if Instant::now() >= deadline {
            terminate(&mut child, pid);
            return Err(AudioIntakeError::InspectionTimeout);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(unix)]
fn terminate(child: &mut std::process::Child, pid: u32) {
    if let Ok(pid) = i32::try_from(pid) {
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(unix)]
fn drain(reader: &mut impl Read, output: &mut Vec<u8>) -> Result<(), AudioIntakeError> {
    let mut buffer = [0_u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(size) if output.len() + size <= OUTPUT_LIMIT => {
                output.extend_from_slice(&buffer[..size])
            }
            Ok(_) => return Err(AudioIntakeError::InspectionLimit),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => (),
            Err(_) => return Err(AudioIntakeError::InspectionUnavailable),
        }
    }
}

#[cfg(not(unix))]
pub(super) fn run(_: &mut Command) -> Result<Vec<u8>, AudioIntakeError> {
    Err(AudioIntakeError::InspectionUnavailable)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_timeout_kills_and_reaps_only_the_inspectors_process_group() {
        let mut command = Command::new("python3");
        command.args(["-c", "import time; time.sleep(10)"]);
        assert_eq!(
            run_limited(&mut command, Duration::from_millis(50)),
            Err(AudioIntakeError::InspectionTimeout)
        );
    }

    #[test]
    fn unavailable_tools_and_output_exhaustion_fail_closed() {
        let mut missing = Command::new("cantos-nonexistent-inspector-for-test");
        assert_eq!(
            run(&mut missing),
            Err(AudioIntakeError::InspectionUnavailable)
        );
        let mut oversized = Command::new("python3");
        oversized.args([
            "-c",
            "import sys; sys.stdout.write('x' * (9 * 1024 * 1024))",
        ]);
        assert_eq!(run(&mut oversized), Err(AudioIntakeError::InspectionLimit));
    }
}
