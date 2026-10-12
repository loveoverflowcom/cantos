//! Bounded read-only media tool invocation. No shell, encoder, URL, or user argument is used.
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::audio_intake::AudioIntakeError;

const TIME_LIMIT: Duration = Duration::from_secs(8);
const MEMORY_LIMIT: u64 = 512 * 1024 * 1024;
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
            // Darwin rejects RLIMIT_AS/DATA reductions. There the parent enforces a sampled
            // resident-memory ceiling with proc_pid_rusage; other Unix hosts also have hard AS.
            #[cfg(not(target_os = "macos"))]
            {
                let value = libc::rlimit {
                    rlim_cur: MEMORY_LIMIT,
                    rlim_max: MEMORY_LIMIT,
                };
                if libc::setrlimit(libc::RLIMIT_AS, &value) == -1 {
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
        match exited(pid) {
            Ok(true) => {
                // Keep the exited leader unreaped while signalling its group. Its PID/PGID
                // cannot be reused to target an unrelated process; inherited pipe descendants
                // are stopped even after a successful tool exit.
                let status = finish(&mut child, pid)?;
                drain(&mut stdout, &mut out)?;
                drain(&mut stderr, &mut err)?;
                return if status.success() && err.is_empty() {
                    Ok(out)
                } else if matches!(status.signal(), Some(libc::SIGXCPU) | Some(libc::SIGKILL)) {
                    Err(AudioIntakeError::InspectionTimeout)
                } else {
                    Err(AudioIntakeError::MalformedAudio)
                };
            }
            Ok(false) => (),
            Err(error) => {
                terminate(&mut child, pid);
                return Err(error);
            }
        }
        if let Err(error) = check_memory(pid, MEMORY_LIMIT) {
            terminate(&mut child, pid);
            return Err(error);
        }
        if Instant::now() >= deadline {
            terminate(&mut child, pid);
            return Err(AudioIntakeError::InspectionTimeout);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(unix)]
fn exited(pid: u32) -> Result<bool, AudioIntakeError> {
    // SAFETY: zeroed siginfo storage and the live unreaped owned child ID are valid.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    if unsafe {
        libc::waitid(
            libc::P_PID,
            pid,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    } == -1
    {
        return Err(AudioIntakeError::InspectionUnavailable);
    }
    Ok(unsafe { info.si_pid() } != 0)
}
#[cfg(target_os = "macos")]
fn check_memory(pid: u32, limit: u64) -> Result<(), AudioIntakeError> {
    // SAFETY: V0 writes exactly the initialized rusage_info_v0 buffer; child is not reaped.
    let mut usage: libc::rusage_info_v0 = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::proc_pid_rusage(
            pid as i32,
            libc::RUSAGE_INFO_V0,
            (&mut usage as *mut libc::rusage_info_v0).cast(),
        )
    };
    if result == -1 {
        // A process can exit between waitid and this probe; only an observed exit excuses it.
        return if exited(pid)? {
            Ok(())
        } else {
            Err(AudioIntakeError::InspectionUnavailable)
        };
    }
    if usage.ri_resident_size > limit {
        Err(AudioIntakeError::InspectionLimit)
    } else {
        Ok(())
    }
}
#[cfg(all(unix, not(target_os = "macos")))]
fn check_memory(_: u32, _: u64) -> Result<(), AudioIntakeError> {
    Ok(())
}
#[cfg(unix)]
fn finish(
    child: &mut std::process::Child,
    pid: u32,
) -> Result<std::process::ExitStatus, AudioIntakeError> {
    if let Ok(pid) = i32::try_from(pid) {
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    child
        .wait()
        .map_err(|_| AudioIntakeError::InspectionUnavailable)
}
#[cfg(unix)]
fn terminate(child: &mut std::process::Child, pid: u32) {
    let _ = finish(child, pid);
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

#[cfg(all(test, target_os = "macos"))]
mod mac_tests {
    use super::*;
    #[test]
    fn sampled_resident_limit_rejects_a_live_owned_child_at_a_small_threshold() {
        use std::os::unix::process::CommandExt;
        let mut command = Command::new("python3");
        command.args(["-c", "import time; time.sleep(5)"]);
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let mut child = command.spawn().unwrap();
        let pid = child.id();
        let result = check_memory(pid, 1);
        terminate(&mut child, pid);
        assert_eq!(result, Err(AudioIntakeError::InspectionLimit));
    }
    #[test]
    fn successful_tool_exit_stops_inherited_pipe_descendants_and_preserves_an_unrelated_child() {
        let mut unrelated = Command::new("python3")
            .args(["-c", "import time; time.sleep(5)"])
            .spawn()
            .unwrap();
        let mut owned = Command::new("python3");
        owned.args([
            "-c",
            "import os,time; p=os.fork(); print(p,flush=True) if p else time.sleep(5)",
        ]);
        let output = run(&mut owned);
        let unrelated_alive = unrelated.try_wait().unwrap().is_none();
        let _ = unrelated.kill();
        let _ = unrelated.wait();
        let pid = String::from_utf8(output.unwrap())
            .unwrap()
            .trim()
            .to_owned();
        assert!(pid.parse::<u32>().unwrap() > 0);
        assert!(unrelated_alive);
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let observed = Command::new("/bin/ps")
                .args(["-p", &pid, "-o", "stat="])
                .output()
                .unwrap();
            let state = String::from_utf8(observed.stdout).unwrap();
            if state.trim().is_empty() || state.trim().starts_with('Z') {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "owned descendant still live: {state}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}
