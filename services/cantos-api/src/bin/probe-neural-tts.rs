//! Explicit research-only neural sample. No production dispatch, acceptance or retry.
use std::{
    env,
    ffi::OsStr,
    fs::OpenOptions,
    io::Read,
    path::{Component, Path},
    process::ExitCode,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::{
    ffi::{CStr, CString},
    fs::File,
    io::Write,
    os::{fd::AsRawFd, fd::FromRawFd, unix::ffi::OsStrExt, unix::fs::MetadataExt},
};

use cantos_api::contexts::production::MAX_PRODUCTION_DOCUMENT_BYTES;
use cantos_api::contexts::synthesis::attempt::{
    ContractCapability, DeliveryState, FailureClass, ReconciliationCapabilities,
};
use cantos_api::contexts::synthesis::providers::neural::{
    run_neural, NeuralPurpose, NeuralRunError, NeuralSynthesisRequest, MAX_RUNTIME_SECONDS,
};
use cantos_api::contexts::synthesis::receipt::{
    audit_receipt, decode_document, ChargeObservation, ReceiptEvidence, ReceiptOutcome,
    TtsProviderIdentity, TtsReceipt, UsageObservation, RECEIPT_VERSION,
};

const USAGE: &str = "usage: probe-neural-tts --research-only <frozen-document.json> <dialogue-id> <absolute-python-path> <absolute-pinned-model-directory> <new-absolute-private-output-directory>";

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.len() != 6 || args[0] != "--research-only" {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match probe(
        Path::new(&args[1]),
        args[2].to_str(),
        Path::new(&args[3]),
        Path::new(&args[4]),
        Path::new(&args[5]),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

fn probe(
    document_path: &Path,
    dialogue_id: Option<&str>,
    python: &Path,
    model: &Path,
    directory: &Path,
) -> Result<(), String> {
    let document = decode_document(&read_document(document_path)?)
        .map_err(|error| format!("document rejected: {error:?}"))?;
    let request = NeuralSynthesisRequest::from_document(
        &document,
        dialogue_id.ok_or("invalid dialogue identifier")?,
    )
    .map_err(|error| error.to_string())?;
    let line = request.input();
    let mut receipt = TtsReceipt {
        version: RECEIPT_VERSION.into(),
        evidence: ReceiptEvidence::CallerReported,
        attempt_id: uuid::Uuid::new_v4().to_string(),
        operation_id: uuid::Uuid::new_v4().to_string(),
        provider_request_id: None,
        idempotency_key: None,
        input_digest: document.input_digest.clone(),
        dialogue_id: line.dialogue_id.clone(),
        provider: TtsProviderIdentity {
            provider_id: line.provider_id.clone(),
            model_id: line.model_id.clone(),
            voice_id: line.voice_id.clone(),
            adapter_version: line.adapter_version.clone(),
        },
        reconciliation: ReconciliationCapabilities::new(
            ContractCapability::Unavailable,
            ContractCapability::Unavailable,
            ContractCapability::Unavailable,
        ),
        usage: UsageObservation::Unknown {},
        charge: ChargeObservation::Unknown {},
        outcome: ReceiptOutcome::Failure {
            class: FailureClass::Timeout,
            delivery: DeliveryState::MayHaveExecuted,
        },
    };
    audit_receipt(&document, &receipt, None)
        .map_err(|error| format!("scope rejected: {error:?}"))?;
    let destination = PrivateOutputDirectory::create(directory)?;
    // Preserve conservative intent before runtime/model preflight or synthesis.
    destination.write_new_json("intent.json", &receipt)?;
    destination.write_new_json("frozen-document.json", &document)?;
    destination.write_new_json(
        "purpose.json",
        &serde_json::json!({
            "purpose": "research-only-no-production-acceptance",
            "input_digest": request.input_digest(),
            "dialogue_id": request.dialogue_id(),
            "model_id": line.model_id,
            "rights_assessment": "training-provenance-unresolved-production-disabled"
        }),
    )?;
    let start = Instant::now();
    let output = match run_neural(
        &request,
        NeuralPurpose::ResearchSample,
        python,
        model,
        Duration::from_secs(MAX_RUNTIME_SECONDS),
    ) {
        Ok(output) => output,
        Err(error) => {
            let (class, delivery) = failure_facts(&error);
            receipt.outcome = ReceiptOutcome::Failure { class, delivery };
            destination.write_new_json("receipt.json", &receipt)?;
            return Err(format!("{error}; attempt retained; no automatic retry"));
        }
    };
    let run_elapsed = start.elapsed();
    receipt.usage = UsageObservation::Reported {
        unit: "effective-unicode-scalars".into(),
        count: line.text.chars().count() as u64,
    };
    receipt.outcome = ReceiptOutcome::Output {
        audio_sha256: output.audio().file_sha256().into(),
    };
    let audit = audit_receipt(&document, &receipt, Some(output.bytes()))
        .map_err(|error| format!("output rejected: {error:?}"))?;
    destination.write_new("output.wav", output.bytes())?;
    destination.write_new_json("receipt.json", &receipt)?;
    destination.write_new_json("audit.json", &audit)?;
    destination.write_new_json(
        "run-observation.json",
        &serde_json::json!({
            "input_digest": request.input_digest(),
            "dialogue_id": request.dialogue_id(),
            "model_id": line.model_id,
            "wav_sha256": output.audio().file_sha256(),
            "wall_seconds": run_elapsed.as_secs_f64(),
            "sampled_peak_resident_bytes": output.sampled_peak_resident_bytes(),
            "timing_scope": "model preflight and cold child including bounded pipe/RSS monitoring",
            "memory_scope": "sampled child RSS; not an exact peak or hard macOS quota",
            "listening": "NOT_RUN"
        }),
    )?;
    println!("research sample inspected; private immutable evidence saved; production disabled");
    Ok(())
}

fn failure_facts(error: &NeuralRunError) -> (FailureClass, DeliveryState) {
    // The concrete adapter owns the closed mapping; no subprocess text is logged.
    error.failure_facts()
}

fn read_document(path: &Path) -> Result<Vec<u8>, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|_| "document is unreadable")?;
    let metadata = file.metadata().map_err(|_| "document is unreadable")?;
    if !metadata.is_file() || metadata.len() > MAX_PRODUCTION_DOCUMENT_BYTES as u64 {
        return Err("document type/size rejected".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_PRODUCTION_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "document is unreadable")?;
    if bytes.len() > MAX_PRODUCTION_DOCUMENT_BYTES {
        return Err("document is too large".into());
    }
    Ok(bytes)
}

const INVALID_OUTPUT_PATH: &str =
    "output directory must be a new absolute path without parent traversal";
#[cfg(unix)]
const OUTPUT_CREATION_FAILED: &str =
    "output directory must be new, with an existing parent and no symlink ancestors";
#[cfg(unix)]
const INVALID_RECORD_NAME: &str = "private record name must be a single filename";
#[cfg(unix)]
const RECORD_CREATION_FAILED: &str = "private record could not be created";
#[cfg(unix)]
const RECORD_SAVE_FAILED: &str = "private record could not be saved";

fn output_components(path: &Path) -> Result<Vec<&OsStr>, String> {
    if !path.is_absolute() {
        return Err(INVALID_OUTPUT_PATH.into());
    }
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => names.push(name),
            _ => return Err(INVALID_OUTPUT_PATH.into()),
        }
    }
    if names.is_empty() {
        return Err(INVALID_OUTPUT_PATH.into());
    }
    Ok(names)
}

#[derive(Debug)]
struct PrivateOutputDirectory {
    #[cfg(unix)]
    directory: File,
}

impl PrivateOutputDirectory {
    #[cfg(unix)]
    fn create(path: &Path) -> Result<Self, String> {
        let names: Vec<_> = output_components(path)?
            .into_iter()
            .map(|name| CString::new(name.as_bytes()).map_err(|_| INVALID_OUTPUT_PATH.to_string()))
            .collect::<Result<_, _>>()?;
        let (leaf, parents) = names.split_last().ok_or(INVALID_OUTPUT_PATH)?;
        // Every ancestor is opened relative to the last held directory, never by
        // resolving the original path again. A symlink at any depth is refused.
        let mut parent = open_root_directory()?;
        for name in parents {
            parent = open_directory_at(&parent, name)?;
        }
        // SAFETY: parent remains open and leaf is one NUL-terminated component.
        let created = unsafe { libc::mkdirat(parent.as_raw_fd(), leaf.as_ptr(), 0o700) };
        if created != 0 {
            return Err(OUTPUT_CREATION_FAILED.into());
        }
        let directory = open_directory_at(&parent, leaf)?;
        let metadata = directory.metadata().map_err(|_| OUTPUT_CREATION_FAILED)?;
        // A replaced leaf must not cause records to land in a public directory
        // or one owned by another user. Same-owner interference remains outside
        // this private local probe's trust boundary.
        // SAFETY: geteuid has no pointer arguments or ownership side effects.
        let effective_uid = unsafe { libc::geteuid() };
        if metadata.uid() != effective_uid || metadata.mode() & 0o777 != 0o700 {
            return Err(OUTPUT_CREATION_FAILED.into());
        }
        parent
            .sync_all()
            .and_then(|()| directory.sync_all())
            .map_err(|_| "private output directory could not be saved")?;
        Ok(Self { directory })
    }

    #[cfg(not(unix))]
    fn create(path: &Path) -> Result<Self, String> {
        output_components(path)?;
        Err("private output directory requires Unix directory-relative I/O".into())
    }

    #[cfg(unix)]
    fn write_new(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let mut components = Path::new(name).components();
        let is_filename = matches!(
            components.next(),
            Some(Component::Normal(component)) if component == OsStr::new(name)
        );
        if !is_filename || components.next().is_some() {
            return Err(INVALID_RECORD_NAME.into());
        }
        let name = CString::new(name).map_err(|_| INVALID_RECORD_NAME)?;
        // SAFETY: the held directory stays open; the name is a single component.
        // O_EXCL preserves an earlier record, including any existing symlink.
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(RECORD_CREATION_FAILED.into());
        }
        // SAFETY: openat returned a newly owned descriptor, closed once by File.
        let mut file = unsafe { File::from_raw_fd(fd) };
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .and_then(|()| self.directory.sync_all())
            .map_err(|_| RECORD_SAVE_FAILED.into())
    }

    #[cfg(not(unix))]
    fn write_new(&self, _name: &str, _bytes: &[u8]) -> Result<(), String> {
        Err("private records require Unix directory-relative I/O".into())
    }

    fn write_new_json(&self, name: &str, value: &impl serde::Serialize) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|_| "private record could not be serialized")?;
        self.write_new(name, &bytes)
    }
}

#[cfg(unix)]
fn open_root_directory() -> Result<File, String> {
    // SAFETY: this literal is NUL-terminated and open returns an owned descriptor.
    let fd = unsafe {
        libc::open(
            c"/".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(OUTPUT_CREATION_FAILED.into());
    }
    // SAFETY: fd is freshly owned and is closed once by File.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(unix)]
fn open_directory_at(parent: &File, name: &CStr) -> Result<File, String> {
    // SAFETY: parent is live and name is a NUL-terminated path component.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(OUTPUT_CREATION_FAILED.into());
    }
    // SAFETY: fd is freshly owned and is closed once by File.
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{symlink, DirBuilderExt, PermissionsExt},
        path::PathBuf,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            // macOS /tmp is itself a symlink: tests use its resolved equivalent.
            let parent = env::temp_dir().canonicalize().unwrap();
            let path = parent.join(format!("cantos-neural-output-{}", uuid::Uuid::new_v4()));
            fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn output_requires_absolute_path_without_parent_traversal() {
        let base = TestDirectory::new();
        for path in [
            PathBuf::from("relative-output"),
            base.0.join("../escaped-output"),
            base.0.join("nested/../../escaped-output"),
            PathBuf::from("/"),
        ] {
            assert_eq!(
                PrivateOutputDirectory::create(&path).unwrap_err(),
                INVALID_OUTPUT_PATH
            );
        }
        assert_eq!(fs::read_dir(&base.0).unwrap().count(), 0);
    }

    #[test]
    fn symlink_ancestor_cannot_redirect_output_creation() {
        let base = TestDirectory::new();
        let real_parent = base.0.join("real");
        fs::create_dir(&real_parent).unwrap();
        symlink(&real_parent, base.0.join("link")).unwrap();
        assert_eq!(
            PrivateOutputDirectory::create(&base.0.join("link/output")).unwrap_err(),
            OUTPUT_CREATION_FAILED
        );
        assert_eq!(fs::read_dir(real_parent).unwrap().count(), 0);
    }

    #[test]
    fn existing_directory_is_refused_without_modifying_records() {
        let base = TestDirectory::new();
        let path = base.0.join("existing");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("intent.json"), b"original intent").unwrap();
        assert_eq!(
            PrivateOutputDirectory::create(&path).unwrap_err(),
            OUTPUT_CREATION_FAILED
        );
        assert_eq!(
            fs::read(path.join("intent.json")).unwrap(),
            b"original intent"
        );
    }

    #[test]
    fn records_are_private_create_only_and_do_not_follow_symlinks() {
        let base = TestDirectory::new();
        let path = base.0.join("output");
        let destination = PrivateOutputDirectory::create(&path).unwrap();
        destination.write_new("intent.json", b"first").unwrap();
        assert_eq!(
            destination
                .write_new("intent.json", b"replacement")
                .unwrap_err(),
            RECORD_CREATION_FAILED
        );
        assert_eq!(fs::read(path.join("intent.json")).unwrap(), b"first");
        let unrelated = base.0.join("unrelated");
        fs::write(&unrelated, b"preserved").unwrap();
        symlink(&unrelated, path.join("receipt.json")).unwrap();
        assert_eq!(
            destination
                .write_new("receipt.json", b"receipt")
                .unwrap_err(),
            RECORD_CREATION_FAILED
        );
        assert_eq!(fs::read(unrelated).unwrap(), b"preserved");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(path.join("intent.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn directory_rename_cannot_redirect_later_record_writes() {
        let base = TestDirectory::new();
        let path = base.0.join("output");
        let destination = PrivateOutputDirectory::create(&path).unwrap();
        destination.write_new("intent.json", b"intent").unwrap();
        let renamed = base.0.join("retained-output");
        fs::rename(&path, &renamed).unwrap();
        fs::create_dir(&path).unwrap();
        destination
            .write_new_json("receipt.json", &serde_json::json!({"attempt": "original"}))
            .unwrap();
        assert_eq!(fs::read(renamed.join("intent.json")).unwrap(), b"intent");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(renamed.join("receipt.json")).unwrap()
            )
            .unwrap(),
            serde_json::json!({"attempt": "original"})
        );
        assert_eq!(fs::read_dir(path).unwrap().count(), 0);
    }

    #[test]
    fn record_names_cannot_escape_the_held_directory() {
        let base = TestDirectory::new();
        let path = base.0.join("output");
        let destination = PrivateOutputDirectory::create(&path).unwrap();
        for name in [
            "",
            ".",
            "..",
            "../escape",
            "/absolute",
            "nested/record",
            "record/",
            "record/.",
            "./record",
            "nul\0",
        ] {
            assert_eq!(
                destination.write_new(name, b"rejected").unwrap_err(),
                INVALID_RECORD_NAME
            );
        }
        assert_eq!(fs::read_dir(path).unwrap().count(), 0);
    }
}
