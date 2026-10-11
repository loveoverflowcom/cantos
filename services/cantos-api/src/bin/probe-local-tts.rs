//! User-invoked local sample probe. No HTTP, production dispatch, artifact acceptance or retry.
use std::{
    env,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::ExitCode,
    time::Duration,
};

use cantos_api::production::MAX_PRODUCTION_DOCUMENT_BYTES;
use cantos_api::tts::attempt::{
    ContractCapability, DeliveryState, FailureClass, ReconciliationCapabilities,
};
use cantos_api::tts::local::{run_local, LocalPhase, LocalRunError, LocalSynthesisRequest};
use cantos_api::tts::{
    audit_receipt, decode_document, ChargeObservation, ReceiptEvidence, ReceiptOutcome,
    TtsProviderIdentity, TtsReceipt, UsageObservation, RECEIPT_VERSION,
};

const USAGE: &str = "usage: probe-local-tts <frozen-document.json> <dialogue-id> <absolute-espeak-ng-path> <new-private-output-directory>";

fn main() -> ExitCode {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.len() != 4 {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    match probe(
        Path::new(&args[0]),
        args[1].to_str(),
        Path::new(&args[2]),
        Path::new(&args[3]),
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
    runtime: &Path,
    directory: &Path,
) -> Result<(), String> {
    let bytes = read_document(document_path)?;
    let document =
        decode_document(&bytes).map_err(|error| format!("document rejected: {error:?}"))?;
    let line = document
        .resolved
        .iter()
        .find(|line| Some(line.dialogue_id.as_str()) == dialogue_id)
        .ok_or("dialogue is absent from the frozen document")?;
    let request = LocalSynthesisRequest::from_resolved(line).map_err(|error| error.to_string())?;
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
        // No billing receipt is issued by this local tool; resource costs stay unmeasured.
        charge: ChargeObservation::Unknown {},
        // Persist conservative intent first. A crash after send cannot leave a record that
        // falsely proves non-execution; a later observation records known setup failures.
        outcome: ReceiptOutcome::Failure {
            class: FailureClass::Timeout,
            delivery: DeliveryState::MayHaveExecuted,
        },
    };
    // Existing production authority validates identity before any local sample is rendered.
    // This offline check deliberately makes no claim of current owner/rights approval.
    audit_receipt(&document, &receipt, None)
        .map_err(|error| format!("scope rejected: {error:?}"))?;
    create_private_directory(directory)?;
    write_new_json(&directory.join("intent.json"), &receipt)?;
    write_new_json(&directory.join("frozen-document.json"), &document)?;
    let workspace =
        fs::canonicalize(directory).map_err(|_| "private directory could not be resolved")?;
    let output = match run_local(&request, runtime, &workspace, Duration::from_secs(30)) {
        Ok(output) => output,
        Err(error) => {
            let (class, delivery) = failure_facts(&error);
            receipt.outcome = ReceiptOutcome::Failure { class, delivery };
            write_new_json(&directory.join("receipt.json"), &receipt)?;
            return Err(format!(
                "{error}; attempt retained for inspection; no automatic retry"
            ));
        }
    };
    receipt.usage = UsageObservation::Reported {
        unit: "effective-unicode-scalars".into(),
        count: line.text.chars().count() as u64,
    };
    receipt.outcome = ReceiptOutcome::Output {
        audio_sha256: output.audio().file_sha256().into(),
    };
    let audit = audit_receipt(&document, &receipt, Some(output.bytes()))
        .map_err(|error| format!("output rejected: {error:?}"))?;
    write_new(&directory.join("output.wav"), output.bytes())?;
    write_new_json(&directory.join("receipt.json"), &receipt)?;
    write_new_json(&directory.join("audit.json"), &audit)?;
    println!("local sample inspected; private output and immutable attempt records saved");
    Ok(())
}

fn failure_facts(error: &LocalRunError) -> (FailureClass, DeliveryState) {
    match error {
        LocalRunError::TimedOut {
            phase: LocalPhase::Synthesize,
        } => (FailureClass::Timeout, DeliveryState::MayHaveExecuted),
        LocalRunError::ProcessFailed {
            phase: LocalPhase::Synthesize,
            ..
        }
        | LocalRunError::ProcessIoFailed {
            phase: LocalPhase::Synthesize,
        }
        | LocalRunError::ProcessOutputExceeded {
            phase: LocalPhase::Synthesize,
        } => (FailureClass::Permanent, DeliveryState::MayHaveExecuted),
        LocalRunError::Audio(_)
        | LocalRunError::MissingOrUnsafeOutput
        | LocalRunError::OutputReadFailed => {
            (FailureClass::MalformedAudio, DeliveryState::MayHaveExecuted)
        }
        LocalRunError::RuntimeVoiceUnavailable => {
            (FailureClass::UnsupportedVoice, DeliveryState::NotSent)
        }
        // Setup/version checks issue no synthesis. Cleanup failure is conservative: the child
        // may have completed even when local workspace removal failed.
        LocalRunError::WorkspaceCleanupFailed => {
            (FailureClass::Permanent, DeliveryState::MayHaveExecuted)
        }
        LocalRunError::UnsupportedPlatform
        | LocalRunError::InvalidTimeout
        | LocalRunError::InvalidRuntimePath
        | LocalRunError::InvalidWorkspaceRoot
        | LocalRunError::WorkspaceCreationFailed
        | LocalRunError::ProcessStartFailed { .. }
        | LocalRunError::WrongRuntimeVersion
        | LocalRunError::TimedOut {
            phase: LocalPhase::Version | LocalPhase::Voices,
        }
        | LocalRunError::ProcessFailed {
            phase: LocalPhase::Version | LocalPhase::Voices,
            ..
        }
        | LocalRunError::ProcessIoFailed {
            phase: LocalPhase::Version | LocalPhase::Voices,
        }
        | LocalRunError::ProcessOutputExceeded {
            phase: LocalPhase::Version | LocalPhase::Voices,
        } => (FailureClass::Permanent, DeliveryState::NotSent),
    }
}

fn read_document(path: &Path) -> Result<Vec<u8>, String> {
    if !fs::metadata(path)
        .map_err(|_| "document is unreadable")?
        .is_file()
    {
        return Err("document is not a regular file".into());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
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

fn create_private_directory(path: &Path) -> Result<(), String> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|_| "output directory must be new, with an existing parent".into())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "private record could not be created")?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "private record could not be saved".into())
}

fn write_new_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|_| "private record could not be serialized")?;
    write_new(path, &bytes)
}
