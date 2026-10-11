//! Plan one frozen Google Cloud TTS line, and synthesize it only on an explicit, capped
//! `--live` run. No HTTP route, production dispatch, artifact acceptance or retry.
//!
//! Without `--live` nothing is read from the environment, created or sent. A live run binds
//! the exact frozen snapshot and dialogue, writes create-only private records, sends exactly
//! one request and leaves every outcome for `inspect-tts-receipt`.
use std::{
    env,
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use cantos_api::{FrozenProductionDocument, ResolvedSynthesisInput};
use cantos_server::production::MAX_PRODUCTION_DOCUMENT_BYTES;
use cantos_server::tts::attempt::{
    ContractCapability, DeliveryState, FailureClass, ReconciliationCapabilities,
};
use cantos_server::tts::google::{self, GoogleOutcome, GooglePlan};
use cantos_server::tts::google_http::{
    synthesize_once, AccessToken, GoogleCredential, GoogleHttpTransport, QuotaProject,
};
use cantos_server::tts::{
    audit_receipt, decode_document, ChargeObservation, ReceiptEvidence, ReceiptOutcome,
    TtsProviderIdentity, TtsReceipt, UsageObservation, RECEIPT_VERSION,
};
use serde_json::{json, Value};

const USAGE: &str = "usage: probe-google-tts <frozen-document.json> <dialogue-id> \
[--live <new-private-output-directory> --max-estimate-cents <n>]";
const TOKEN_VARIABLE: &str = "CANTOS_GOOGLE_TTS_ACCESS_TOKEN";
const PROJECT_VARIABLE: &str = "CANTOS_GOOGLE_TTS_QUOTA_PROJECT";

struct Live {
    directory: PathBuf,
    max_estimate_cents: u64,
}

struct Request {
    document: PathBuf,
    dialogue_id: String,
    live: Option<Live>,
}

enum Command {
    Help,
    Run(Request),
}

/// A finished run: the JSON report and whether it is a success.
type Report = (Value, bool);

fn main() -> ExitCode {
    let request = match parse_args(env::args_os().skip(1)) {
        Ok(Command::Run(request)) => request,
        Ok(Command::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(request) {
        Ok((report, succeeded)) => match print_json(&report) {
            Ok(()) if succeeded => ExitCode::SUCCESS,
            Ok(()) => ExitCode::from(1),
            Err(error) => {
                eprintln!("report could not be written: {error}");
                ExitCode::from(2)
            }
        },
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}

fn run(request: Request) -> Result<Report, String> {
    let bytes = read_document(&request.document)?;
    let document =
        decode_document(&bytes).map_err(|error| format!("document rejected: {error:?}"))?;
    let line = document
        .resolved
        .iter()
        .find(|line| line.dialogue_id == request.dialogue_id)
        .ok_or("dialogue is absent from the frozen document")?;
    let plan = match google::plan(line) {
        Ok(plan) => plan,
        Err(findings) => return Ok((json!({ "status": "blocked", "findings": findings }), false)),
    };
    let mut receipt = intent_receipt(&document, line);
    // The existing production authority re-admits the snapshot and the line identity before
    // anything is planned for sending. This makes no claim of current owner/rights approval.
    if let Err(error) = audit_receipt(&document, &receipt, None) {
        return Ok((json!({ "status": "scope_rejected", "error": error }), false));
    }
    let Some(live) = request.live else {
        return Ok((planned(&document, line, &plan), true));
    };
    if plan.estimate().amount_minor > live.max_estimate_cents {
        let finding = json!({
            "code": "estimate_exceeds_cap",
            "estimate_cents": plan.estimate().amount_minor,
            "cap_cents": live.max_estimate_cents,
        });
        return Ok((json!({ "status": "blocked", "findings": [finding] }), false));
    }
    let transport = GoogleHttpTransport::new(credential()?)
        .map_err(|_| "the HTTPS client could not be initialized".to_owned())?;
    let directory = &live.directory;
    create_private_directory(directory)?;
    // Conservative intent first: a crash after sending cannot leave a record that falsely
    // proves non-execution.
    write_new_json(&directory.join("intent.json"), &receipt)?;
    write_new_json(&directory.join("frozen-document.json"), &document)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("runtime could not start: {error}"))?;
    let outcome = runtime.block_on(synthesize_once(&transport, &plan));
    write_new_json(
        &directory.join("observation.json"),
        &google::observation(&plan, &outcome),
    )?;
    match outcome {
        GoogleOutcome::Output(audio) => {
            receipt.usage = UsageObservation::Reported {
                unit: "effective-unicode-scalars".into(),
                count: line.text.chars().count() as u64,
            };
            receipt.outcome = ReceiptOutcome::Output {
                audio_sha256: audio.inspected().file_sha256().into(),
            };
            let audit = audit_receipt(&document, &receipt, Some(audio.bytes()))
                .map_err(|error| format!("output rejected: {error:?}"))?;
            write_new(&directory.join("output.wav"), audio.bytes())?;
            write_new_json(&directory.join("receipt.json"), &receipt)?;
            write_new_json(&directory.join("audit.json"), &audit)?;
            let report = json!({
                "status": "output",
                "request_digest": plan.request_digest(),
                "audio": audio.inspected(),
            });
            Ok((report, true))
        }
        GoogleOutcome::Failure(failure) => {
            receipt.outcome = ReceiptOutcome::Failure {
                class: failure.class,
                delivery: failure.delivery,
            };
            write_new_json(&directory.join("receipt.json"), &receipt)?;
            let recovery = audit_receipt(&document, &receipt, None)
                .map_err(|error| format!("failure receipt rejected: {error:?}"))?
                .recovery()
                .cloned();
            let report = json!({
                "status": "failed",
                "request_digest": plan.request_digest(),
                "failure": failure,
                "recovery": recovery,
                "automatic_retry": false,
            });
            Ok((report, false))
        }
    }
}

/// The receipt as it stands before sending. Google offers no lookup or idempotency key.
fn intent_receipt(
    document: &FrozenProductionDocument,
    line: &ResolvedSynthesisInput,
) -> TtsReceipt {
    TtsReceipt {
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
        // Google's response carries no billing data; the list-price estimate is not a charge.
        charge: ChargeObservation::Unknown {},
        outcome: ReceiptOutcome::Failure {
            class: FailureClass::Timeout,
            delivery: DeliveryState::MayHaveExecuted,
        },
    }
}

fn planned(
    document: &FrozenProductionDocument,
    line: &ResolvedSynthesisInput,
    plan: &GooglePlan,
) -> Value {
    json!({
        "status": "planned",
        "sent": false,
        "input_digest": document.input_digest,
        "dialogue_id": line.dialogue_id,
        "provider_id": line.provider_id,
        "adapter_version": line.adapter_version,
        "model_id": line.model_id,
        "voice_id": line.voice_id,
        "request_digest": plan.request_digest(),
        "estimate": plan.estimate(),
        "limitations": google::LIMITATIONS,
    })
}

fn credential() -> Result<GoogleCredential, String> {
    let token = env::var(TOKEN_VARIABLE)
        .map_err(|_| format!("{TOKEN_VARIABLE} must hold an OAuth access token for --live"))?;
    let access_token =
        AccessToken::new(token.trim()).map_err(|_| format!("{TOKEN_VARIABLE} is malformed"))?;
    let quota_project = match env::var(PROJECT_VARIABLE) {
        Ok(project) => Some(
            QuotaProject::new(project.trim())
                .map_err(|_| format!("{PROJECT_VARIABLE} is not a project ID"))?,
        ),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!("{PROJECT_VARIABLE} is not a project ID"))
        }
    };
    Ok(GoogleCredential {
        access_token,
        quota_project,
    })
}

fn print_json(value: &Value) -> io::Result<()> {
    let mut json = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    json.push(b'\n');
    io::stdout().lock().write_all(&json)
}

fn parse_args(args: impl Iterator<Item = OsString>) -> Result<Command, String> {
    let mut args = args;
    let mut positional = Vec::new();
    let (mut directory, mut cap) = (None, None);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => return Ok(Command::Help),
            Some("--live") => {
                let path = args.next().ok_or("--live needs a new private directory")?;
                directory = Some(PathBuf::from(path));
            }
            Some("--max-estimate-cents") => {
                let value = args.next().and_then(|value| value.into_string().ok());
                let parsed = value.and_then(|value| value.parse::<u64>().ok());
                cap = Some(parsed.ok_or("--max-estimate-cents expects a whole number of cents")?);
            }
            Some(flag) if flag.starts_with('-') => {
                return Err(format!("unexpected option: {flag}"));
            }
            _ => positional.push(arg),
        }
    }
    let [document, dialogue_id] = <[OsString; 2]>::try_from(positional)
        .map_err(|_| "expected a frozen document and a dialogue ID")?;
    let dialogue_id = dialogue_id
        .into_string()
        .map_err(|_| "dialogue ID must be UTF-8")?;
    let live = match (directory, cap) {
        (None, None) => None,
        (Some(directory), Some(max_estimate_cents)) => Some(Live {
            directory,
            max_estimate_cents,
        }),
        _ => return Err("--live and --max-estimate-cents must be given together".into()),
    };
    Ok(Command::Run(Request {
        document: PathBuf::from(document),
        dialogue_id,
        live,
    }))
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
