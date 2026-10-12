//! Bounded, explicitly invoked research samples from one pinned local neural bundle.
//! Production is refused at the same boundary that starts the child process.
use std::{error::Error, fmt, path::Path, time::Duration};

use cantos_contracts::{FrozenProductionDocument, ResolvedSynthesisInput};

pub use crate::contexts::production::catalog::neural::{
    NeuralRequestError, ADAPTER_VERSION, MAX_TEXT_BYTES, MODEL_ID, OUTPUT_CONTRACT, PROVIDER_ID,
};
use crate::contexts::synthesis::media::audio::{AudioError, InspectedTtsAudio};

pub const MAX_RUNTIME_SECONDS: u64 = 120;
pub const MAX_STDERR_BYTES: usize = 128 * 1024;
pub const MAX_RESIDENT_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_MODEL_BYTES: u64 = 281_756_486;
pub const RUNTIME_HELPER: &str = include_str!("../../../../../../scripts/vieneu_nano_runtime.py");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeuralPurpose {
    ResearchSample,
    Production,
}

/// Research scope can only be constructed from an admitted, exact frozen document.
/// It is not an owner approval or a production authorization witness.
#[derive(Clone, PartialEq, Eq)]
pub struct NeuralSynthesisRequest {
    input: ResolvedSynthesisInput,
    input_digest: String,
}

impl fmt::Debug for NeuralSynthesisRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NeuralSynthesisRequest")
            .field("input_digest", &self.input_digest)
            .field("dialogue_id", &self.input.dialogue_id)
            .field("voice_id", &self.input.voice_id)
            .field("text_bytes", &self.input.text.len())
            .finish()
    }
}

impl NeuralSynthesisRequest {
    pub fn from_document(
        document: &FrozenProductionDocument,
        dialogue_id: &str,
    ) -> Result<Self, NeuralRequestError> {
        if serde_json::to_vec(document).map_or(true, |bytes| {
            bytes.len() > crate::contexts::production::MAX_PRODUCTION_DOCUMENT_BYTES
        }) {
            return Err(NeuralRequestError::InvalidDocument);
        }
        let prepared = crate::contexts::production::prepare_inputs(
            &document.owner_id,
            &document.revision,
            &document.settings,
            &document.rights,
            true,
            0,
        )
        .map_err(|_| NeuralRequestError::InvalidDocument)?;
        if prepared.document() != *document {
            return Err(NeuralRequestError::InvalidDocument);
        }
        let input = document
            .resolved
            .iter()
            .find(|line| line.dialogue_id == dialogue_id)
            .ok_or(NeuralRequestError::DialogueMissing)?;
        crate::contexts::production::catalog::neural::validate_resolved(input)?;
        Ok(Self {
            input: input.clone(),
            input_digest: document.input_digest.clone(),
        })
    }

    pub fn input(&self) -> &ResolvedSynthesisInput {
        &self.input
    }
    pub fn input_digest(&self) -> &str {
        &self.input_digest
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
}

impl fmt::Display for NeuralRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported neural sample input: {self:?}")
    }
}
impl Error for NeuralRequestError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NeuralRunError {
    ProductionForbidden,
    UnsupportedPlatform,
    InvalidTimeout,
    InvalidRuntimePath,
    InvalidModelDirectory,
    ModelArtifactRejected { name: &'static str },
    ConcurrencyLimit,
    ConcurrencyGuardFailed,
    ProcessStartFailed,
    ProcessIoFailed,
    ProcessFailed { code: Option<i32> },
    OutputExceeded,
    StderrExceeded,
    TimedOut,
    MemoryExceeded,
    ResourceInspectionFailed,
    Audio(AudioError),
}

impl fmt::Display for NeuralRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Source text, local paths and child stderr never enter operational diagnostics.
        write!(formatter, "neural sample failed: {self:?}")
    }
}
impl Error for NeuralRunError {}

impl NeuralRunError {
    pub fn failure_facts(
        &self,
    ) -> (
        crate::contexts::synthesis::attempt::FailureClass,
        crate::contexts::synthesis::attempt::DeliveryState,
    ) {
        use crate::contexts::synthesis::attempt::{DeliveryState, FailureClass};
        match self {
            Self::ProductionForbidden
            | Self::UnsupportedPlatform
            | Self::InvalidTimeout
            | Self::InvalidRuntimePath
            | Self::InvalidModelDirectory
            | Self::ModelArtifactRejected { .. }
            | Self::ConcurrencyLimit
            | Self::ConcurrencyGuardFailed
            | Self::ProcessStartFailed => (FailureClass::Permanent, DeliveryState::NotSent),
            // Model hashing may time out before spawn, but a timeout from the same
            // public boundary cannot assert that inference never started.
            Self::TimedOut => (FailureClass::Timeout, DeliveryState::MayHaveExecuted),
            Self::Audio(_) => (FailureClass::MalformedAudio, DeliveryState::MayHaveExecuted),
            Self::ProcessIoFailed
            | Self::ProcessFailed { .. }
            | Self::OutputExceeded
            | Self::StderrExceeded
            | Self::MemoryExceeded
            | Self::ResourceInspectionFailed => {
                (FailureClass::Permanent, DeliveryState::MayHaveExecuted)
            }
        }
    }
}

pub struct NeuralSynthesisOutput {
    bytes: Vec<u8>,
    audio: InspectedTtsAudio,
    sampled_peak_resident_bytes: u64,
}

impl fmt::Debug for NeuralSynthesisOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NeuralSynthesisOutput")
            .field("audio_bytes", &self.bytes.len())
            .field("audio", &self.audio)
            .field(
                "sampled_peak_resident_bytes",
                &self.sampled_peak_resident_bytes,
            )
            .finish()
    }
}

impl NeuralSynthesisOutput {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn audio(&self) -> &InspectedTtsAudio {
        &self.audio
    }
    pub fn permitted_purpose(&self) -> NeuralPurpose {
        NeuralPurpose::ResearchSample
    }
    /// Polling observation, not an exact peak measurement or a hard OS memory quota.
    pub fn sampled_peak_resident_bytes(&self) -> u64 {
        self.sampled_peak_resident_bytes
    }
}

pub fn run_neural(
    request: &NeuralSynthesisRequest,
    purpose: NeuralPurpose,
    python_path: &Path,
    model_directory: &Path,
    timeout: Duration,
) -> Result<NeuralSynthesisOutput, NeuralRunError> {
    // This refusal precedes runtime/model inspection and child creation, even if an
    // externally supplied snapshot has granted claims and a current owner approval.
    if purpose == NeuralPurpose::Production {
        return Err(NeuralRunError::ProductionForbidden);
    }
    if timeout.is_zero() || timeout > Duration::from_secs(MAX_RUNTIME_SECONDS) {
        return Err(NeuralRunError::InvalidTimeout);
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        unix::run(request, python_path, model_directory, timeout)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (request, python_path, model_directory);
        Err(NeuralRunError::UnsupportedPlatform)
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix {
    use super::*;
    use crate::contexts::synthesis::media::audio::{
        inspect_output_for_profile, OutputProfile, MAX_OUTPUT_BYTES,
    };
    use serde::Serialize;
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File, OpenOptions},
        io::{self, Read, Write},
        os::{
            fd::AsRawFd,
            unix::{
                fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
                process::CommandExt,
            },
        },
        path::PathBuf,
        process::{Child, Command, ExitStatus, Stdio},
        thread,
        time::Instant,
    };

    pub(super) const MODEL_FILES: [(&str, u64, &str); 6] = [
        (
            "codec_decoder.onnx",
            99_319_941,
            "b0ab15e7828a39d53679e25b1ba4ba415a61311307202a6323130b9e1cc3029d",
        ),
        (
            "duration_predictor.onnx",
            727_809,
            "20fd7fa60006d0a48ee82e0451b3f920d2052588c083c756cce669f3947c0a68",
        ),
        (
            "text_encoder.onnx",
            26_519_943,
            "204f02cccae1f16ccb2d3840f05721a37fe250b82cbd456337a0ccb49615e4bf",
        ),
        (
            "vector_estimator.onnx",
            155_132_418,
            "c6c1d4398ca35d3ad1bd3f0459413d1b975d09e7f2f3a2b4493a7b92bbf6ce93",
        ),
        (
            "constants.npz",
            53_448,
            "7c011938effe41687a9af85107a31a040dfcf0fb3d0f048ec10f4fb65c1a8829",
        ),
        (
            "config.json",
            2_927,
            "3762f1716fd8d7a1451ddf2e051c03b0a341e8adda4f1dbd0db0ae9777514e56",
        ),
    ];

    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// Advisory lock on one owner-private regular file; OS release survives crashes.
    /// The pathname stays in place so waiting processes always lock the same inode.
    fn concurrency_guard() -> Result<File, NeuralRunError> {
        // SAFETY: geteuid reads the calling process identity and has no pointer arguments.
        let uid = unsafe { libc::geteuid() };
        // A caller-controlled TMPDIR must not create another concurrency lane.
        // /tmp is the fixed system namespace on both supported Unix platforms;
        // the per-user filename and descriptor checks retain private ownership.
        let path = Path::new("/tmp").join(format!("cantos-vieneu-nano-{uid}.lock"));
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| NeuralRunError::ConcurrencyGuardFailed)?;
        let metadata = file
            .metadata()
            .map_err(|_| NeuralRunError::ConcurrencyGuardFailed)?;
        if !metadata.is_file()
            || metadata.uid() != uid
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
        {
            return Err(NeuralRunError::ConcurrencyGuardFailed);
        }
        // SAFETY: the live File owns this valid descriptor throughout lock acquisition/use.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            return Err(if error.kind() == io::ErrorKind::WouldBlock {
                NeuralRunError::ConcurrencyLimit
            } else {
                NeuralRunError::ConcurrencyGuardFailed
            });
        }
        Ok(file)
    }

    pub(super) fn run(
        request: &NeuralSynthesisRequest,
        python: &Path,
        models: &Path,
        timeout: Duration,
    ) -> Result<NeuralSynthesisOutput, NeuralRunError> {
        let deadline = Instant::now() + timeout;
        let _guard = concurrency_guard()?;
        if !python.is_absolute()
            || !fs::metadata(python).is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        {
            return Err(NeuralRunError::InvalidRuntimePath);
        }
        // Preserve the launcher path: resolving a venv/bin/python symlink changes
        // Python's environment prefix and would bypass its pinned dependencies.
        let python = python.to_path_buf();
        let models = validate_models(models, deadline)?;
        execute(request, &python, &models, deadline)
    }

    fn validate_models(path: &Path, deadline: Instant) -> Result<PathBuf, NeuralRunError> {
        if !path.is_absolute()
            || !fs::symlink_metadata(path)
                .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        {
            return Err(NeuralRunError::InvalidModelDirectory);
        }
        let root = fs::canonicalize(path).map_err(|_| NeuralRunError::InvalidModelDirectory)?;
        for (name, size, expected) in MODEL_FILES {
            validate_artifact(&root.join(name), name, size, expected, deadline)?;
        }
        Ok(root)
    }

    fn validate_artifact(
        path: &Path,
        name: &'static str,
        size: u64,
        expected: &str,
        deadline: Instant,
    ) -> Result<(), NeuralRunError> {
        let failure = || NeuralRunError::ModelArtifactRejected { name };
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| failure())?;
        let before = file.metadata().map_err(|_| failure())?;
        if !before.is_file() || before.len() != size {
            return Err(failure());
        }
        let mut hash = Sha256::new();
        let mut file = file.take(size + 1);
        let mut count = 0_u64;
        let mut buffer = [0_u8; 16_384];
        loop {
            if Instant::now() >= deadline {
                return Err(NeuralRunError::TimedOut);
            }
            let read = file.read(&mut buffer).map_err(|_| failure())?;
            if read == 0 {
                break;
            }
            count += read as u64;
            hash.update(&buffer[..read]);
        }
        let after = file.get_ref().metadata().map_err(|_| failure())?;
        if count != size
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || format!("{:x}", hash.finalize()) != expected
        {
            return Err(failure());
        }
        Ok(())
    }

    #[derive(Serialize)]
    struct HelperInput<'a> {
        provider_id: &'a str,
        model_id: &'a str,
        adapter_version: &'a str,
        voice_id: &'a str,
        language: &'a str,
        text: &'a str,
    }

    const RESOURCE_INSPECTION_GRACE: Duration = Duration::from_millis(50);

    #[derive(Default)]
    struct InspectionGrace {
        first_failure: Option<Instant>,
    }

    impl InspectionGrace {
        /// A valid observation resets the gap. Unknown RSS is tolerated only for
        /// this bounded consecutive interval, never treated as a zero measurement.
        fn observe(
            &mut self,
            observation: Result<u64, NeuralRunError>,
            now: Instant,
        ) -> Result<Option<u64>, NeuralRunError> {
            match observation {
                Ok(resident) => {
                    self.first_failure = None;
                    Ok(Some(resident))
                }
                Err(NeuralRunError::ResourceInspectionFailed) => {
                    let first = *self.first_failure.get_or_insert(now);
                    if now.duration_since(first) >= RESOURCE_INSPECTION_GRACE {
                        return Err(NeuralRunError::ResourceInspectionFailed);
                    }
                    Ok(None)
                }
                Err(error) => Err(error),
            }
        }
    }

    fn execute(
        request: &NeuralSynthesisRequest,
        python: &Path,
        models: &Path,
        deadline: Instant,
    ) -> Result<NeuralSynthesisOutput, NeuralRunError> {
        execute_with_observer(request, python, models, deadline, resident_bytes)
    }

    /// Private seam for deterministic resource-inspection faults on the real shell.
    /// Public dispatch always supplies the OS observer above.
    fn execute_with_observer(
        request: &NeuralSynthesisRequest,
        python: &Path,
        models: &Path,
        deadline: Instant,
        mut observe_resident: impl FnMut(u32) -> Result<u64, NeuralRunError>,
    ) -> Result<NeuralSynthesisOutput, NeuralRunError> {
        let input = serde_json::to_vec(&HelperInput {
            provider_id: PROVIDER_ID,
            model_id: MODEL_ID,
            adapter_version: ADAPTER_VERSION,
            voice_id: request.voice_id(),
            language: &request.input.language,
            text: request.text(),
        })
        .map_err(|_| NeuralRunError::ProcessIoFailed)?;
        let mut command = Command::new(python);
        command
            .args(["-I", "-c", RUNTIME_HELPER])
            .arg(models)
            .current_dir(models)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("OMP_NUM_THREADS", "2")
            .env("OPENBLAS_NUM_THREADS", "2")
            .env("MKL_NUM_THREADS", "2")
            .env("VECLIB_MAXIMUM_THREADS", "2")
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // SAFETY: only async-signal-safe setrlimit calls and errno retrieval occur post-fork.
        unsafe {
            command.pre_exec(|| {
                let cpu = libc::rlimit {
                    rlim_cur: MAX_RUNTIME_SECONDS as libc::rlim_t,
                    rlim_max: MAX_RUNTIME_SECONDS as libc::rlim_t,
                };
                let disk = libc::rlimit {
                    rlim_cur: MAX_OUTPUT_BYTES as libc::rlim_t,
                    rlim_max: MAX_OUTPUT_BYTES as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_CPU, &cpu) != 0
                    || libc::setrlimit(libc::RLIMIT_FSIZE, &disk) != 0
                {
                    return Err(io::Error::last_os_error());
                }
                // Linux enforces address-space admission. Darwin needs resident polling;
                // its RLIMIT_AS cannot establish an enforced physical-memory quota.
                #[cfg(target_os = "linux")]
                {
                    let memory = libc::rlimit {
                        rlim_cur: MAX_RESIDENT_BYTES as libc::rlim_t,
                        rlim_max: MAX_RESIDENT_BYTES as libc::rlim_t,
                    };
                    if libc::setrlimit(libc::RLIMIT_AS, &memory) != 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        // Preflight hashing, encoding or scheduling can exhaust the same wall
        // deadline. Refuse inference before crossing the process boundary.
        if Instant::now() >= deadline {
            return Err(NeuralRunError::TimedOut);
        }
        let mut child = OwnedChild(
            command
                .spawn()
                .map_err(|_| NeuralRunError::ProcessStartFailed)?,
        );
        let stdin = child
            .0
            .stdin
            .take()
            .ok_or(NeuralRunError::ProcessIoFailed)?;
        let mut stdout = child
            .0
            .stdout
            .take()
            .ok_or(NeuralRunError::ProcessIoFailed)?;
        let mut stderr = child
            .0
            .stderr
            .take()
            .ok_or(NeuralRunError::ProcessIoFailed)?;
        for fd in [stdin.as_raw_fd(), stdout.as_raw_fd(), stderr.as_raw_fd()] {
            nonblocking(fd)?;
        }
        let mut stdin = Some(stdin);
        let mut written = 0;
        let mut bytes = Vec::new();
        let mut diagnostics = Vec::new();
        let mut peak = 0;
        let mut inspection_grace = InspectionGrace::default();
        loop {
            if Instant::now() >= deadline {
                return Err(NeuralRunError::TimedOut);
            }
            drain(
                &mut stdout,
                &mut bytes,
                MAX_OUTPUT_BYTES,
                NeuralRunError::OutputExceeded,
                deadline,
            )?;
            drain(
                &mut stderr,
                &mut diagnostics,
                MAX_STDERR_BYTES,
                NeuralRunError::StderrExceeded,
                deadline,
            )?;
            if let Some(pipe) = &mut stdin {
                match pipe.write(&input[written..]) {
                    Ok(count) => written += count,
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) => return Err(NeuralRunError::ProcessIoFailed),
                }
                if written == input.len() {
                    stdin = None;
                }
            }
            if let Some(status) = child
                .0
                .try_wait()
                .map_err(|_| NeuralRunError::ProcessIoFailed)?
            {
                drain(
                    &mut stdout,
                    &mut bytes,
                    MAX_OUTPUT_BYTES,
                    NeuralRunError::OutputExceeded,
                    deadline,
                )?;
                drain(
                    &mut stderr,
                    &mut diagnostics,
                    MAX_STDERR_BYTES,
                    NeuralRunError::StderrExceeded,
                    deadline,
                )?;
                return finish(status, written == input.len(), bytes, peak);
            }
            // During an exit transition proc_pidinfo can fail before waitpid sees
            // the exit. Keep servicing pipes/deadline and polling for at most the
            // bounded inspection gap; a persistent failure still kills/reaps.
            if let Some(resident) =
                inspection_grace.observe(observe_resident(child.0.id()), Instant::now())?
            {
                peak = admit_resident_bytes(resident, peak)?;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn finish(
        status: ExitStatus,
        input_complete: bool,
        bytes: Vec<u8>,
        peak: u64,
    ) -> Result<NeuralSynthesisOutput, NeuralRunError> {
        if !status.success() {
            return Err(NeuralRunError::ProcessFailed {
                code: status.code(),
            });
        }
        if !input_complete {
            return Err(NeuralRunError::ProcessIoFailed);
        }
        let checksum = format!("{:x}", Sha256::digest(&bytes));
        let audio = inspect_output_for_profile(&bytes, &checksum, OutputProfile::Neural24Khz)
            .map_err(NeuralRunError::Audio)?;
        Ok(NeuralSynthesisOutput {
            bytes,
            audio,
            sampled_peak_resident_bytes: peak,
        })
    }

    fn admit_resident_bytes(resident: u64, prior_peak: u64) -> Result<u64, NeuralRunError> {
        if resident > MAX_RESIDENT_BYTES {
            return Err(NeuralRunError::MemoryExceeded);
        }
        Ok(prior_peak.max(resident))
    }

    fn nonblocking(fd: std::os::fd::RawFd) -> Result<(), NeuralRunError> {
        // SAFETY: descriptors belong to live child pipes while both operations run.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(NeuralRunError::ProcessIoFailed);
        }
        Ok(())
    }

    fn drain(
        pipe: &mut impl Read,
        bytes: &mut Vec<u8>,
        cap: usize,
        exceeded: NeuralRunError,
        deadline: Instant,
    ) -> Result<(), NeuralRunError> {
        let mut buffer = [0_u8; 8_192];
        loop {
            if Instant::now() >= deadline {
                return Err(NeuralRunError::TimedOut);
            }
            match pipe.read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(count) => {
                    if count > cap.saturating_sub(bytes.len()) {
                        return Err(exceeded);
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => return Err(NeuralRunError::ProcessIoFailed),
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn resident_bytes(pid: u32) -> Result<u64, NeuralRunError> {
        let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::uninit();
        let size = std::mem::size_of::<libc::proc_taskinfo>();
        // SAFETY: proc_pidinfo writes at most the supplied live buffer size; facts are
        // read only after its result confirms the full proc_taskinfo was initialized.
        let count = unsafe {
            libc::proc_pidinfo(
                pid as libc::c_int,
                libc::PROC_PIDTASKINFO,
                0,
                info.as_mut_ptr().cast(),
                size as libc::c_int,
            )
        };
        if count != size as libc::c_int {
            return Err(NeuralRunError::ResourceInspectionFailed);
        }
        // SAFETY: the complete structure was initialized by the successful OS call.
        Ok(unsafe { info.assume_init() }.pti_resident_size)
    }

    #[cfg(target_os = "linux")]
    fn resident_bytes(pid: u32) -> Result<u64, NeuralRunError> {
        let stat = fs::read_to_string(format!("/proc/{pid}/statm"))
            .map_err(|_| NeuralRunError::ResourceInspectionFailed)?;
        let pages = stat
            .split_whitespace()
            .nth(1)
            .and_then(|field| field.parse::<u64>().ok())
            .ok_or(NeuralRunError::ResourceInspectionFailed)?;
        // SAFETY: sysconf takes a supported integer selector and no pointer arguments.
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if page_size <= 0 {
            return Err(NeuralRunError::ResourceInspectionFailed);
        }
        pages
            .checked_mul(page_size as u64)
            .ok_or(NeuralRunError::ResourceInspectionFailed)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use cantos_contracts::SynthesisPerformance;
        use std::{os::unix::fs::symlink, sync::Mutex};

        // Process fixtures share the real per-user concurrency lock. They use one
        // test lane; the explicit two-run case below owns concurrency assertions.
        static PROCESS_TESTS: Mutex<()> = Mutex::new(());

        struct Fixture {
            root: PathBuf,
            runtime: PathBuf,
        }
        impl Fixture {
            fn new(body: &str) -> Self {
                let root = std::env::temp_dir()
                    .join(format!("cantos-neural-test-{}", uuid::Uuid::new_v4()));
                fs::create_dir(&root).unwrap();
                let root = fs::canonicalize(root).unwrap();
                let runtime = root.join("fake-python");
                let script = format!("#!/bin/sh\nprintf '%s\\n' \"$$\" > '{}/pid'\nprintf '%s\\n' \"$1\" \"$2\" \"$4\" > '{}/args'\n/bin/cat > '{}/input'\n{body}\n", root.display(), root.display(), root.display());
                fs::write(&runtime, script).unwrap();
                fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
                fs::write(root.join("fixture.wav"), wav()).unwrap();
                Self { root, runtime }
            }
            fn execute(&self, timeout: Duration) -> Result<NeuralSynthesisOutput, NeuralRunError> {
                execute(
                    &request(),
                    &self.runtime,
                    &self.root,
                    Instant::now() + timeout,
                )
            }
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.root);
            }
        }

        fn request() -> NeuralSynthesisRequest {
            NeuralSynthesisRequest {
                input_digest: "synthetic-unit-scope".into(),
                input: ResolvedSynthesisInput {
                    dialogue_id: "dialogue-1".into(),
                    character_id: "narrator".into(),
                    text: "Mỹ Duyên hỏi: Đức Trí đã về chưa?".into(),
                    language: "vi-VN".into(),
                    provider_id: PROVIDER_ID.into(),
                    model_id: MODEL_ID.into(),
                    voice_id: "duc-tri".into(),
                    voice_rights_record_id: "fixture_voice".into(),
                    adapter_version: ADAPTER_VERSION.into(),
                    output_contract: OUTPUT_CONTRACT.into(),
                    pronunciation: vec![],
                    performance: SynthesisPerformance {
                        rate_permille: 1_000,
                        pitch_semitones: 0,
                        emotion: Some("neutral".into()),
                        intensity_permille: Some(0),
                    },
                },
            }
        }

        fn wav() -> Vec<u8> {
            let mut bytes = b"RIFF".to_vec();
            bytes.extend(40_u32.to_le_bytes());
            bytes.extend(b"WAVEfmt ");
            bytes.extend(16_u32.to_le_bytes());
            bytes.extend(1_u16.to_le_bytes());
            bytes.extend(1_u16.to_le_bytes());
            bytes.extend(24_000_u32.to_le_bytes());
            bytes.extend(48_000_u32.to_le_bytes());
            bytes.extend(2_u16.to_le_bytes());
            bytes.extend(16_u16.to_le_bytes());
            bytes.extend(b"data");
            bytes.extend(4_u32.to_le_bytes());
            bytes.extend([1, 0, 255, 255]);
            bytes
        }

        #[test]
        fn expired_preflight_deadline_refuses_even_an_attempt_to_spawn() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            let fixture = Fixture::new("exec /bin/cat fixture.wav");
            // An absent launcher distinguishes refusing dispatch from spawning
            // first and only checking the deadline in the monitoring loop.
            let error = execute(
                &request(),
                &fixture.root.join("absent-runtime"),
                &fixture.root,
                Instant::now() - Duration::from_secs(1),
            )
            .unwrap_err();
            assert_eq!(error, NeuralRunError::TimedOut);
            assert!(!fixture.root.join("pid").exists());
            assert!(!fixture.root.join("input").exists());
        }

        #[test]
        fn fixed_helper_arguments_and_typed_stdin_produce_only_inspected_wav() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            let fixture = Fixture::new("exec /bin/cat fixture.wav");
            let output = fixture.execute(Duration::from_secs(10)).unwrap();
            assert_eq!(output.bytes(), wav());
            assert_eq!(output.audio().sample_rate_hz(), 24_000);
            assert_eq!(output.audio().frames(), 2);
            assert_eq!(output.permitted_purpose(), NeuralPurpose::ResearchSample);
            assert_eq!(
                fs::read_to_string(fixture.root.join("args")).unwrap(),
                format!("-I\n-c\n{}\n", fixture.root.display())
            );
            let input: serde_json::Value =
                serde_json::from_slice(&fs::read(fixture.root.join("input")).unwrap()).unwrap();
            assert_eq!(
                input,
                serde_json::json!({"provider_id":PROVIDER_ID,"model_id":MODEL_ID,
                "adapter_version":ADAPTER_VERSION,"voice_id":"duc-tri","language":"vi-VN","text":request().text()})
            );
            assert!(!format!("{output:?}").contains("[82, 73, 70, 70"));
        }

        #[test]
        fn crash_discards_valid_partial_output_and_redacts_private_stderr() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            let fixture =
                Fixture::new("/bin/cat fixture.wav\nprintf 'PRIVATE_SENTINEL' >&2\nexit 7");
            let error = fixture.execute(Duration::from_secs(10)).unwrap_err();
            assert_eq!(error, NeuralRunError::ProcessFailed { code: Some(7) });
            assert!(!format!("{error:?} {error}").contains("PRIVATE_SENTINEL"));
            assert_eq!(
                error.failure_facts(),
                (
                    crate::contexts::synthesis::attempt::FailureClass::Permanent,
                    crate::contexts::synthesis::attempt::DeliveryState::MayHaveExecuted
                )
            );
        }

        #[test]
        fn empty_truncated_malformed_and_wrong_rate_outputs_are_refused() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            for bytes in [vec![], b"not audio".to_vec(), wav()[..20].to_vec()] {
                let fixture = Fixture::new("exec /bin/cat fixture.wav");
                fs::write(fixture.root.join("fixture.wav"), bytes).unwrap();
                assert!(matches!(
                    fixture.execute(Duration::from_secs(10)).unwrap_err(),
                    NeuralRunError::Audio(AudioError::InvalidWav { .. })
                ));
            }
            let fixture = Fixture::new("exec /bin/cat fixture.wav");
            let mut bytes = wav();
            bytes[24..28].copy_from_slice(&22_050_u32.to_le_bytes());
            bytes[28..32].copy_from_slice(&44_100_u32.to_le_bytes());
            fs::write(fixture.root.join("fixture.wav"), bytes).unwrap();
            assert_eq!(
                fixture.execute(Duration::from_secs(10)).unwrap_err(),
                NeuralRunError::Audio(AudioError::UnsupportedProfile {
                    format_tag: 1,
                    sample_rate_hz: 22_050,
                    channels: 1,
                    bits_per_sample: 16
                })
            );
        }

        #[test]
        fn captured_stdout_and_stderr_have_independent_byte_caps() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            for (body, expected) in [
                (
                    "/bin/dd if=/dev/zero bs=1048576 count=17",
                    NeuralRunError::OutputExceeded,
                ),
                (
                    "/bin/dd if=/dev/zero bs=131073 count=1 >&2",
                    NeuralRunError::StderrExceeded,
                ),
            ] {
                let fixture = Fixture::new(body);
                assert_eq!(
                    fixture.execute(Duration::from_secs(10)).unwrap_err(),
                    expected
                );
            }
        }

        #[test]
        fn timeout_kills_and_reaps_the_exact_owned_child() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            let fixture = Fixture::new("while :; do :; done");
            let start = Instant::now();
            assert_eq!(
                fixture.execute(Duration::from_secs(1)).unwrap_err(),
                NeuralRunError::TimedOut
            );
            assert!(start.elapsed() < Duration::from_secs(4));
            let pid = fs::read_to_string(fixture.root.join("pid"))
                .unwrap()
                .trim()
                .parse::<libc::pid_t>()
                .unwrap();
            // SAFETY: this PID was recorded by this exact task-owned fixture child.
            let result = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
            assert_eq!(result, -1);
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::ECHILD)
            );
        }

        #[test]
        fn inspection_grace_requires_recovery_within_each_consecutive_gap() {
            let start = Instant::now();
            let mut grace = InspectionGrace::default();
            assert_eq!(
                grace.observe(Err(NeuralRunError::ResourceInspectionFailed), start),
                Ok(None)
            );
            assert_eq!(
                grace.observe(
                    Err(NeuralRunError::ResourceInspectionFailed),
                    start + Duration::from_millis(49)
                ),
                Ok(None)
            );
            assert_eq!(
                grace.observe(
                    Err(NeuralRunError::ResourceInspectionFailed),
                    start + RESOURCE_INSPECTION_GRACE
                ),
                Err(NeuralRunError::ResourceInspectionFailed)
            );

            let mut grace = InspectionGrace::default();
            assert_eq!(
                grace.observe(Err(NeuralRunError::ResourceInspectionFailed), start),
                Ok(None)
            );
            assert_eq!(
                grace.observe(Ok(17), start + Duration::from_millis(49)),
                Ok(Some(17))
            );
            assert_eq!(
                grace.observe(
                    Err(NeuralRunError::ResourceInspectionFailed),
                    start + Duration::from_millis(50)
                ),
                Ok(None)
            );
            assert_eq!(
                grace.observe(
                    Err(NeuralRunError::ResourceInspectionFailed),
                    start + Duration::from_millis(99)
                ),
                Ok(None)
            );
            assert_eq!(
                grace.observe(
                    Err(NeuralRunError::ResourceInspectionFailed),
                    start + Duration::from_millis(100)
                ),
                Err(NeuralRunError::ResourceInspectionFailed)
            );
            assert_eq!(
                grace.observe(Err(NeuralRunError::MemoryExceeded), start),
                Err(NeuralRunError::MemoryExceeded)
            );
        }

        #[test]
        fn transient_inspection_faults_recover_while_the_real_child_finishes() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            let fixture = Fixture::new("/bin/sleep 0.1\nexec /bin/cat fixture.wav");
            let mut failed_samples = 0;
            let mut recovered_samples = 0;
            let output = execute_with_observer(
                &request(),
                &fixture.runtime,
                &fixture.root,
                Instant::now() + Duration::from_secs(10),
                |pid| {
                    if failed_samples < 2 {
                        failed_samples += 1;
                        Err(NeuralRunError::ResourceInspectionFailed)
                    } else {
                        let observation = resident_bytes(pid);
                        if observation.is_ok() {
                            recovered_samples += 1;
                        }
                        observation
                    }
                },
            )
            .unwrap();
            assert_eq!(failed_samples, 2);
            assert!(recovered_samples > 0);
            assert!(output.sampled_peak_resident_bytes() > 0);
            assert_eq!(output.bytes(), wav());
        }

        #[test]
        fn persistent_inspection_faults_stop_and_reap_the_real_child() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            let fixture = Fixture::new("while :; do :; done");
            let marker = fixture.root.join("pid");
            let start = Instant::now();
            let mut failed_samples = 0;
            let error = execute_with_observer(
                &request(),
                &fixture.runtime,
                &fixture.root,
                start + Duration::from_secs(10),
                |pid| {
                    // Begin deterministic failures after this exact fixture has
                    // recorded its PID, so scheduling cannot hide reap evidence.
                    if marker.exists() {
                        failed_samples += 1;
                        Err(NeuralRunError::ResourceInspectionFailed)
                    } else {
                        resident_bytes(pid)
                    }
                },
            )
            .unwrap_err();
            assert_eq!(error, NeuralRunError::ResourceInspectionFailed);
            assert!(failed_samples >= 2);
            assert!(start.elapsed() >= RESOURCE_INSPECTION_GRACE);
            assert!(start.elapsed() < Duration::from_secs(4));
            let pid = fs::read_to_string(marker)
                .unwrap()
                .trim()
                .parse::<libc::pid_t>()
                .unwrap();
            // SAFETY: this PID was recorded by this exact task-owned fixture child.
            let result = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
            assert_eq!(result, -1);
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::ECHILD)
            );
        }

        #[test]
        fn concurrency_guard_refuses_a_second_holder_and_releases_on_drop() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            let first = concurrency_guard().unwrap();
            assert_eq!(
                concurrency_guard().unwrap_err(),
                NeuralRunError::ConcurrencyLimit
            );
            drop(first);
            drop(concurrency_guard().unwrap());
        }

        /// Run only by the two-process regression below. The ordinary test run has
        /// no fixture environment and performs no subprocess or lock operation.
        #[test]
        fn concurrency_subprocess_fixture() {
            let Some(root) = std::env::var_os("CANTOS_NEURAL_TEST_ROOT") else {
                return;
            };
            let root = PathBuf::from(root);
            let role = std::env::var("CANTOS_NEURAL_TEST_ROLE").unwrap();
            let _lane = PROCESS_TESTS.lock().unwrap();
            let guarded_output = match concurrency_guard() {
                Ok(_guard) => execute(
                    &request(),
                    &root.join("fake-python"),
                    &root,
                    Instant::now() + Duration::from_secs(30),
                ),
                Err(error) => Err(error),
            };
            match role.as_str() {
                "hold" => assert_eq!(guarded_output.unwrap().bytes(), wav()),
                "reject" => assert_eq!(
                    guarded_output.unwrap_err(),
                    NeuralRunError::ConcurrencyLimit
                ),
                _ => panic!("unknown task-owned concurrency fixture role"),
            }
        }

        fn concurrency_fixture_process(root: &Path, temporary: &Path, role: &str) -> OwnedChild {
            OwnedChild(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "contexts::synthesis::providers::neural::unix::tests::concurrency_subprocess_fixture",
                        "--test-threads=1",
                    ])
                    .env("TMPDIR", temporary)
                    .env("CANTOS_NEURAL_TEST_ROOT", root)
                    .env("CANTOS_NEURAL_TEST_ROLE", role)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit())
                    .spawn()
                    .unwrap(),
            )
        }

        fn wait_for_fixture_child(child: &mut OwnedChild) -> ExitStatus {
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                if let Some(status) = child.0.try_wait().unwrap() {
                    return status;
                }
                assert!(
                    Instant::now() < deadline,
                    "task-owned test child exceeded its deadline"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }

        struct HeldFixture {
            child: OwnedChild,
            release: PathBuf,
        }

        impl Drop for HeldFixture {
            fn drop(&mut self) {
                // Release the fixture before killing its supervising test process:
                // otherwise a panic could strand the task-owned fake grandchild.
                let _ = fs::write(&self.release, b"finish task-owned fixture");
                let deadline = Instant::now() + Duration::from_secs(2);
                while matches!(self.child.0.try_wait(), Ok(None)) && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }

        #[test]
        fn separate_processes_with_distinct_tmpdir_cannot_start_a_second_runtime() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            let first = Fixture::new(
                "while [ ! -f release ]; do /bin/sleep 0.02; done\nexec /bin/cat fixture.wav",
            );
            let second = Fixture::new("exec /bin/cat fixture.wav");
            let first_tmp = first.root.join("temporary");
            let second_tmp = second.root.join("temporary");
            fs::create_dir(&first_tmp).unwrap();
            fs::create_dir(&second_tmp).unwrap();
            assert_ne!(first_tmp, second_tmp);
            let mut holder = HeldFixture {
                child: concurrency_fixture_process(&first.root, &first_tmp, "hold"),
                release: first.root.join("release"),
            };
            let deadline = Instant::now() + Duration::from_secs(10);
            while !first.root.join("input").exists() {
                assert!(
                    holder.child.0.try_wait().unwrap().is_none(),
                    "holder exited before runtime dispatch"
                );
                assert!(
                    Instant::now() < deadline,
                    "holder did not start the fake runtime"
                );
                thread::sleep(Duration::from_millis(10));
            }
            let mut contender = concurrency_fixture_process(&second.root, &second_tmp, "reject");
            assert!(wait_for_fixture_child(&mut contender).success());
            assert!(
                !second.root.join("pid").exists(),
                "rejected request started a runtime"
            );
            assert!(!second.root.join("input").exists());
            fs::write(first.root.join("release"), b"finish task-owned fixture").unwrap();
            assert!(wait_for_fixture_child(&mut holder.child).success());
            drop(concurrency_guard().unwrap());
        }

        #[test]
        fn sampled_resident_bound_admits_the_limit_and_rejects_one_byte_over() {
            assert_eq!(admit_resident_bytes(0, 17), Ok(17));
            assert_eq!(
                admit_resident_bytes(MAX_RESIDENT_BYTES, 17),
                Ok(MAX_RESIDENT_BYTES)
            );
            assert_eq!(
                admit_resident_bytes(MAX_RESIDENT_BYTES + 1, 0),
                Err(NeuralRunError::MemoryExceeded)
            );
        }

        #[test]
        fn pinned_artifact_hash_size_and_regular_file_checks_precede_model_loading() {
            let _lane = PROCESS_TESTS.lock().unwrap();
            let fixture = Fixture::new("exit 0");
            let path = fixture.root.join("artifact");
            let digest = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
            let mismatched_digest = "0".repeat(64);
            fs::write(&path, b"abc").unwrap();
            let check = |size, expected| {
                validate_artifact(
                    &path,
                    "fixture",
                    size,
                    expected,
                    Instant::now() + Duration::from_secs(2),
                )
            };
            assert_eq!(check(3, digest), Ok(()));
            assert_eq!(
                check(2, digest),
                Err(NeuralRunError::ModelArtifactRejected { name: "fixture" })
            );
            assert_eq!(
                check(3, &mismatched_digest),
                Err(NeuralRunError::ModelArtifactRejected { name: "fixture" })
            );
            fs::remove_file(&path).unwrap();
            symlink("fixture.wav", &path).unwrap();
            assert_eq!(
                check(3, digest),
                Err(NeuralRunError::ModelArtifactRejected { name: "fixture" })
            );
            fs::remove_file(&path).unwrap();
            assert!(Command::new("/usr/bin/mkfifo")
                .arg(&path)
                .status()
                .unwrap()
                .success());
            assert_eq!(
                check(3, digest),
                Err(NeuralRunError::ModelArtifactRejected { name: "fixture" })
            );
            assert_eq!(
                MODEL_FILES.iter().map(|(_, size, _)| size).sum::<u64>(),
                MAX_MODEL_BYTES
            );
        }
    }
}
