//! Explicit, bounded invocation of one installed eSpeak NG runtime.
//! Validation is pure; execution owns only its child and private temporary directory.
//! A request is not production authorization, and this module installs nothing.

use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::Duration;

use cantos_contracts::ResolvedSynthesisInput;

use super::audio::{AudioError, InspectedTtsAudio};
pub use super::local_contract::{
    ADAPTER_VERSION, MAX_TEXT_BYTES, MODEL_ID, OUTPUT_CONTRACT, PROVIDER_ID,
};

pub const RUNTIME_VERSION: &str = "1.52.0";
pub const MAX_RUNTIME_SECONDS: u64 = 30;
pub const MAX_PROCESS_OUTPUT_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalRequestError {
    WrongProvider,
    WrongModel,
    WrongAdapter,
    WrongOutputContract,
    UnsupportedVoice,
    UnsupportedLanguage,
    UnsupportedRate,
    UnsupportedPitch,
    UnsupportedEmotion,
    UnsupportedIntensity,
    InvalidText,
}

impl fmt::Display for LocalRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported local synthesis input: {self:?}")
    }
}

impl Error for LocalRequestError {}

#[derive(Clone, PartialEq, Eq)]
pub struct LocalSynthesisRequest {
    input: ResolvedSynthesisInput,
}

impl fmt::Debug for LocalSynthesisRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalSynthesisRequest")
            .field("dialogue_id", &self.input.dialogue_id)
            .field("voice_id", &self.input.voice_id)
            .field("text_bytes", &self.input.text.len())
            .finish()
    }
}

impl LocalSynthesisRequest {
    pub fn from_resolved(input: &ResolvedSynthesisInput) -> Result<Self, LocalRequestError> {
        for (matches, error) in [
            (
                input.provider_id == PROVIDER_ID,
                LocalRequestError::WrongProvider,
            ),
            (input.model_id == MODEL_ID, LocalRequestError::WrongModel),
            (
                input.adapter_version == ADAPTER_VERSION,
                LocalRequestError::WrongAdapter,
            ),
            (
                input.output_contract == OUTPUT_CONTRACT,
                LocalRequestError::WrongOutputContract,
            ),
        ] {
            if !matches {
                return Err(error);
            }
        }
        if !["vi", "vi-vn-x-central", "vi-vn-x-south"].contains(&input.voice_id.as_str()) {
            return Err(LocalRequestError::UnsupportedVoice);
        }
        if !["vi", "vi-VN"].contains(&input.language.as_str()) {
            return Err(LocalRequestError::UnsupportedLanguage);
        }
        if input.performance.rate_permille != 1_000 {
            return Err(LocalRequestError::UnsupportedRate);
        }
        if input.performance.pitch_semitones != 0 {
            return Err(LocalRequestError::UnsupportedPitch);
        }
        if input.performance.emotion.as_deref() != Some("neutral") {
            return Err(LocalRequestError::UnsupportedEmotion);
        }
        if input.performance.intensity_permille != Some(0) {
            return Err(LocalRequestError::UnsupportedIntensity);
        }
        if !super::local_contract::valid_text(&input.text) {
            return Err(LocalRequestError::InvalidText);
        }
        Ok(Self {
            input: input.clone(),
        })
    }

    pub fn dialogue_id(&self) -> &str {
        &self.input.dialogue_id
    }

    pub fn voice_id(&self) -> &str {
        &self.input.voice_id
    }

    pub fn text(&self) -> &str {
        &self.input.text
    }

    pub fn resolved_input(&self) -> &ResolvedSynthesisInput {
        &self.input
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalPhase {
    Version,
    Voices,
    Synthesize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalRunError {
    UnsupportedPlatform,
    InvalidTimeout,
    InvalidRuntimePath,
    InvalidWorkspaceRoot,
    WorkspaceCreationFailed,
    WorkspaceCleanupFailed,
    ProcessStartFailed {
        phase: LocalPhase,
    },
    ProcessIoFailed {
        phase: LocalPhase,
    },
    ProcessFailed {
        phase: LocalPhase,
        code: Option<i32>,
    },
    ProcessOutputExceeded {
        phase: LocalPhase,
    },
    TimedOut {
        phase: LocalPhase,
    },
    WrongRuntimeVersion,
    RuntimeVoiceUnavailable,
    MissingOrUnsafeOutput,
    OutputReadFailed,
    Audio(AudioError),
}

impl fmt::Display for LocalRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Child stdout/stderr and source text never enter an error or operational log.
        write!(formatter, "local synthesis failed: {self:?}")
    }
}

impl Error for LocalRunError {}

pub struct LocalSynthesisOutput {
    bytes: Vec<u8>,
    audio: InspectedTtsAudio,
}

impl fmt::Debug for LocalSynthesisOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalSynthesisOutput")
            .field("audio_bytes", &self.bytes.len())
            .field("audio", &self.audio)
            .finish()
    }
}

impl LocalSynthesisOutput {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn audio(&self) -> &InspectedTtsAudio {
        &self.audio
    }

    pub fn runtime_version(&self) -> &'static str {
        RUNTIME_VERSION
    }

    pub fn provider_id(&self) -> &'static str {
        PROVIDER_ID
    }

    pub fn model_id(&self) -> &'static str {
        MODEL_ID
    }

    pub fn adapter_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    pub fn output_contract(&self) -> &'static str {
        OUTPUT_CONTRACT
    }
}

/// The total runtime budget covers version inspection, voice inspection and synthesis.
/// Returns actual inspected bytes; it neither saves an accepted artifact nor changes a snapshot.
pub fn run_local(
    request: &LocalSynthesisRequest,
    runtime_path: &Path,
    workspace_root: &Path,
    timeout: Duration,
) -> Result<LocalSynthesisOutput, LocalRunError> {
    if timeout.is_zero() || timeout > Duration::from_secs(MAX_RUNTIME_SECONDS) {
        return Err(LocalRunError::InvalidTimeout);
    }
    #[cfg(unix)]
    {
        unix::run(request, runtime_path, workspace_root, timeout)
    }
    #[cfg(not(unix))]
    {
        let _ = (request, runtime_path, workspace_root);
        Err(LocalRunError::UnsupportedPlatform)
    }
}

#[cfg(unix)]
mod unix {
    use std::fs::{DirBuilder, OpenOptions};
    use std::io::{self, Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::os::unix::process::CommandExt;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::thread;
    use std::time::Instant;

    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    use super::*;
    use crate::tts::audio::{inspect_output_for_profile, OutputProfile, MAX_OUTPUT_BYTES};

    struct OwnedChild(Child);

    impl Drop for OwnedChild {
        fn drop(&mut self) {
            // Only the exact task child is signalled; no process groups or user apps.
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    pub fn run(
        request: &LocalSynthesisRequest,
        runtime_path: &Path,
        workspace_root: &Path,
        timeout: Duration,
    ) -> Result<LocalSynthesisOutput, LocalRunError> {
        let runtime = validate_runtime(runtime_path)?;
        if !workspace_root.is_absolute()
            || !fs::metadata(workspace_root).is_ok_and(|metadata| metadata.is_dir())
        {
            return Err(LocalRunError::InvalidWorkspaceRoot);
        }
        let root =
            fs::canonicalize(workspace_root).map_err(|_| LocalRunError::InvalidWorkspaceRoot)?;
        let private = root.join(format!("cantos-local-tts-{}", Uuid::new_v4()));
        DirBuilder::new()
            .mode(0o700)
            .create(&private)
            .map_err(|_| LocalRunError::WorkspaceCreationFailed)?;
        let deadline = Instant::now() + timeout;
        let result = synthesize(request, &runtime, &private, deadline);
        fs::remove_dir_all(&private).map_err(|_| LocalRunError::WorkspaceCleanupFailed)?;
        result
    }

    fn validate_runtime(path: &Path) -> Result<PathBuf, LocalRunError> {
        if !path.is_absolute() {
            return Err(LocalRunError::InvalidRuntimePath);
        }
        let metadata = fs::metadata(path).map_err(|_| LocalRunError::InvalidRuntimePath)?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return Err(LocalRunError::InvalidRuntimePath);
        }
        fs::canonicalize(path).map_err(|_| LocalRunError::InvalidRuntimePath)
    }

    fn synthesize(
        request: &LocalSynthesisRequest,
        runtime: &Path,
        private: &Path,
        deadline: Instant,
    ) -> Result<LocalSynthesisOutput, LocalRunError> {
        let version = run_process(
            runtime,
            &["--version"],
            None,
            private,
            LocalPhase::Version,
            deadline,
        )?;
        let version =
            std::str::from_utf8(&version).map_err(|_| LocalRunError::WrongRuntimeVersion)?;
        if !version.lines().any(|line| {
            line.strip_prefix("eSpeak NG text-to-speech: ")
                .and_then(|rest| rest.split_whitespace().next())
                == Some(RUNTIME_VERSION)
        }) {
            return Err(LocalRunError::WrongRuntimeVersion);
        }
        let voices = run_process(
            runtime,
            &["--voices=vi"],
            None,
            private,
            LocalPhase::Voices,
            deadline,
        )?;
        let voices =
            std::str::from_utf8(&voices).map_err(|_| LocalRunError::RuntimeVoiceUnavailable)?;
        // A language match alone permits a replacement voice file. The supported
        // built-in voice must have exactly one row and its expected data-file ID.
        let expected_voice_file = match request.voice_id() {
            "vi" => "aav/vi",
            "vi-vn-x-central" => "aav/vi-VN-x-central",
            "vi-vn-x-south" => "aav/vi-VN-x-south",
            _ => return Err(LocalRunError::RuntimeVoiceUnavailable),
        };
        let mut rows = voices
            .lines()
            .filter(|line| line.split_whitespace().nth(1) == Some(request.voice_id()));
        if rows.next().and_then(|line| line.split_whitespace().nth(4)) != Some(expected_voice_file)
            || rows.next().is_some()
        {
            return Err(LocalRunError::RuntimeVoiceUnavailable);
        }
        let output = private.join("output.wav");
        if output
            .try_exists()
            .map_err(|_| LocalRunError::OutputReadFailed)?
        {
            return Err(LocalRunError::MissingOrUnsafeOutput);
        }
        // eSpeak's bulk --stdin path replaces its last byte with NUL. The newline
        // is framing, not a text transformation, and preserves the final UTF-8 byte.
        let mut framed_text = request.text().as_bytes().to_vec();
        framed_text.push(b'\n');
        run_process(
            runtime,
            &[
                "--stdin",
                "-v",
                request.voice_id(),
                "-s",
                "175",
                "-p",
                "50",
                "-a",
                "100",
                "-w",
                // The child owns this private cwd. eSpeak's native filename buffer can
                // truncate deep absolute paths; a fixed basename preserves that boundary.
                "output.wav",
            ],
            Some(&framed_text),
            private,
            LocalPhase::Synthesize,
            deadline,
        )?;
        let bytes = read_output(&output)?;
        let checksum = format!("{:x}", Sha256::digest(&bytes));
        let audio = inspect_output_for_profile(&bytes, &checksum, OutputProfile::Espeak22Khz)
            .map_err(LocalRunError::Audio)?;
        Ok(LocalSynthesisOutput { bytes, audio })
    }

    fn read_output(path: &Path) -> Result<Vec<u8>, LocalRunError> {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| LocalRunError::MissingOrUnsafeOutput)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.nlink() != 1 {
            return Err(LocalRunError::MissingOrUnsafeOutput);
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| LocalRunError::OutputReadFailed)?;
        let opened = file
            .metadata()
            .map_err(|_| LocalRunError::OutputReadFailed)?;
        if !opened.is_file()
            || opened.nlink() != 1
            || opened.dev() != metadata.dev()
            || opened.ino() != metadata.ino()
        {
            return Err(LocalRunError::MissingOrUnsafeOutput);
        }
        let mut bytes = Vec::new();
        file.take(MAX_OUTPUT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| LocalRunError::OutputReadFailed)?;
        if bytes.len() > MAX_OUTPUT_BYTES {
            return Err(LocalRunError::Audio(AudioError::OutputTooLarge {
                actual_bytes: bytes.len(),
                maximum_bytes: MAX_OUTPUT_BYTES,
            }));
        }
        Ok(bytes)
    }

    fn nonblocking(pipe: &impl AsRawFd, phase: LocalPhase) -> Result<(), LocalRunError> {
        // SAFETY: the borrowed descriptor remains open for these fcntl operations.
        let flags = unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_GETFL) };
        if flags < 0 {
            return Err(LocalRunError::ProcessIoFailed { phase });
        }
        // SAFETY: only the valid borrowed descriptor's status flags are modified.
        if unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(LocalRunError::ProcessIoFailed { phase });
        }
        Ok(())
    }

    fn drain(
        pipe: &mut impl Read,
        captured: &mut Vec<u8>,
        phase: LocalPhase,
        deadline: Instant,
    ) -> Result<(), LocalRunError> {
        let mut buffer = [0_u8; 4_096];
        loop {
            if Instant::now() >= deadline {
                return Err(LocalRunError::TimedOut { phase });
            }
            match pipe.read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(count) => {
                    if captured.len() + count > MAX_PROCESS_OUTPUT_BYTES {
                        return Err(LocalRunError::ProcessOutputExceeded { phase });
                    }
                    captured.extend_from_slice(&buffer[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(LocalRunError::ProcessIoFailed { phase }),
            }
        }
    }

    fn run_process(
        runtime: &Path,
        args: &[&str],
        input: Option<&[u8]>,
        private: &Path,
        phase: LocalPhase,
        deadline: Instant,
    ) -> Result<Vec<u8>, LocalRunError> {
        if Instant::now() >= deadline {
            return Err(LocalRunError::TimedOut { phase });
        }
        let mut command = Command::new(runtime);
        command
            .args(args)
            .current_dir(private)
            // Runtime configuration, voice data overrides and loader variables
            // are not frozen synthesis inputs. Use only the trusted installation's
            // defaults; child scratch/configuration stays in this owned directory.
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("HOME", private)
            .env("TMPDIR", private)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: this post-fork hook uses only setrlimit and errno retrieval;
        // no allocations, locks or other non-async-signal-safe work occurs.
        unsafe {
            command.pre_exec(|| {
                let limit = libc::rlimit {
                    rlim_cur: MAX_OUTPUT_BYTES as libc::rlim_t,
                    rlim_max: MAX_OUTPUT_BYTES as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) == 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error())
                }
            });
        }
        let child = command
            .spawn()
            .map_err(|_| LocalRunError::ProcessStartFailed { phase })?;
        let mut child = OwnedChild(child);
        let mut stdin = child.0.stdin.take();
        if let Some(pipe) = &stdin {
            nonblocking(pipe, phase)?;
        }
        let mut stdout = child
            .0
            .stdout
            .take()
            .ok_or(LocalRunError::ProcessIoFailed { phase })?;
        let mut stderr = child
            .0
            .stderr
            .take()
            .ok_or(LocalRunError::ProcessIoFailed { phase })?;
        nonblocking(&stdout, phase)?;
        nonblocking(&stderr, phase)?;
        let input = input.unwrap_or_default();
        let mut written = 0;
        let mut captured_stdout = Vec::new();
        let mut captured_stderr = Vec::new();
        loop {
            drain(&mut stdout, &mut captured_stdout, phase, deadline)?;
            drain(&mut stderr, &mut captured_stderr, phase, deadline)?;
            if let Some(pipe) = &mut stdin {
                match pipe.write(&input[written..]) {
                    Ok(count) => written += count,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(_) => return Err(LocalRunError::ProcessIoFailed { phase }),
                }
                if written == input.len() {
                    stdin = None;
                }
            }
            if let Some(status) = child
                .0
                .try_wait()
                .map_err(|_| LocalRunError::ProcessIoFailed { phase })?
            {
                drain(&mut stdout, &mut captured_stdout, phase, deadline)?;
                drain(&mut stderr, &mut captured_stderr, phase, deadline)?;
                if !status.success() {
                    return Err(LocalRunError::ProcessFailed {
                        phase,
                        code: status.code(),
                    });
                }
                if written != input.len() {
                    return Err(LocalRunError::ProcessIoFailed { phase });
                }
                return Ok(captured_stdout);
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
