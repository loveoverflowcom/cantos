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
    time::{SystemTime, UNIX_EPOCH},
};

use cantos_api::contexts::production::{prepare_inputs, MAX_PRODUCTION_DOCUMENT_BYTES};
use cantos_api::contexts::synthesis::attempt::{
    ContractCapability, DeliveryState, FailureClass, ReconciliationCapabilities,
};
use cantos_api::contexts::synthesis::providers::google::http::{
    synthesize_once, AccessToken, GoogleCredential, GoogleHttpTransport, QuotaProject,
};
use cantos_api::contexts::synthesis::providers::google::{self, GoogleOutcome, GooglePlan};
use cantos_api::contexts::synthesis::receipt::{
    audit_receipt, decode_document, ChargeObservation, ReceiptEvidence, ReceiptOutcome,
    TtsProviderIdentity, TtsReceipt, UsageObservation, RECEIPT_VERSION,
};
use cantos_contracts::{FrozenProductionDocument, ProductionCurrency, ResolvedSynthesisInput};
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|time| i64::try_from(time.as_secs()).ok())
        .ok_or("the clock cannot establish current rights validity")?;
    if let Some(finding) = live_blocker(&document, &plan, live.max_estimate_cents, now) {
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
        "authorization": "sample-only; current production approval is not checked",
    })
}

/// A paid sample cannot bypass the frozen rights/budget policy. These are pinned caller
/// assertions only: current revocation, editorial review and owner approval need the backend
/// authority in #7. Passing this check neither authorizes production nor settles a charge.
fn live_blocker(
    document: &FrozenProductionDocument,
    plan: &GooglePlan,
    cap: u64,
    now: i64,
) -> Option<Value> {
    let budget = &document.settings.settings.budget;
    if budget.currency != ProductionCurrency::USD {
        return Some(json!({ "code": "budget_currency_mismatch", "required_currency": "USD" }));
    }
    if plan.estimate().amount_minor > cap || plan.estimate().amount_minor > budget.limit_minor {
        return Some(json!({
            "code": "estimate_exceeds_cap",
            "estimate_cents": plan.estimate().amount_minor,
            "cap_cents": cap,
            "frozen_budget_cents": budget.limit_minor,
        }));
    }
    match prepare_inputs(
        &document.owner_id,
        &document.revision,
        &document.settings,
        &document.rights,
        true,
        now,
    ) {
        Ok(prepared) if prepared.document() == *document => {
            let preview = prepared.preview();
            (!preview.inputs_eligible).then(|| {
                json!({
                    "code": "frozen_policy_blocked",
                    "policy_findings": preview.findings,
                })
            })
        }
        _ => Some(json!({ "code": "frozen_policy_invalid" })),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_api::script_ir::read_script;
    use cantos_contracts::{
        ProductionRightsClaim, ProductionRightsClaimResponse, ProductionRightsDeclaration,
        ProductionRightsScope, ProductionRightsStatus, ProductionRightsSubject,
        ProductionSettingsResponse, RevisionResponse, SaveProductionSettingsRequest,
    };
    use sha2::{Digest, Sha256};

    fn document() -> FrozenProductionDocument {
        let script = read_script(include_bytes!(
            "../../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
        ))
        .unwrap();
        let script_json = String::from_utf8(script.export_bytes()).unwrap();
        let mut hash = Sha256::new();
        hash.update(b"cantos/script-export/e1\n");
        hash.update(script_json.as_bytes());
        let revision = RevisionResponse {
            script_id: "00000000-0000-4000-8000-000000000006".into(),
            revision: 1,
            accepted_by: "owner".into(),
            accepted_at: "fixture".into(),
            content_digest: script.content_digest().to_string(),
            export_digest: format!("sir-e1:sha256:{:x}", hash.finalize()),
            script_json,
        };
        let request: SaveProductionSettingsRequest = serde_json::from_slice(include_bytes!(
            "../../../../packages/contracts/fixtures/production/v1/settings-request.json"
        ))
        .unwrap();
        let mut settings = ProductionSettingsResponse {
            script_id: revision.script_id.clone(),
            version: 1,
            script_revision: 1,
            operation_id: request.operation_id,
            recorded_by: "owner".into(),
            recorded_at: "fixture".into(),
            settings: request.settings,
        };
        settings.settings.budget.currency = ProductionCurrency::USD;
        settings.settings.budget.limit_minor = 10;
        for binding in &mut settings.settings.bindings {
            binding.provider_id =
                cantos_api::contexts::production::catalog::google::PROVIDER_ID.into();
            binding.model_id = "google-neural2".into();
            binding.voice_id = "vi-VN-Neural2-A".into();
            binding.voice_rights_record_id = format!("voice-{}", binding.character_id);
            binding.performance.emotion = Some("neutral".into());
            binding.performance.intensity_permille = Some(0);
        }
        let mut subjects: Vec<_> = ["rights-demo", "rights-adaptation-demo", "rights-asset-demo"]
            .into_iter()
            .map(|id| {
                (
                    id.to_owned(),
                    ProductionRightsSubject::Evidence {
                        record_id: id.into(),
                    },
                )
            })
            .collect();
        subjects.extend(settings.settings.bindings.iter().map(|binding| {
            (
                binding.voice_rights_record_id.clone(),
                ProductionRightsSubject::Voice {
                    provider_id: binding.provider_id.clone(),
                    model_id: binding.model_id.clone(),
                    voice_id: binding.voice_id.clone(),
                },
            )
        }));
        let rights: Vec<_> = subjects
            .into_iter()
            .map(|(id, subject)| ProductionRightsClaimResponse {
                claim: ProductionRightsClaim {
                    version: 1,
                    declaration: ProductionRightsDeclaration {
                        record_id: id,
                        subject,
                        scope: ProductionRightsScope::ProductionSynthesis,
                        rights_holder: "Synthetic test author".into(),
                        languages: vec!["vi-VN".into()],
                        territory: settings.settings.budget.territory.clone(),
                        permitted_scope: settings.settings.budget.scope.clone(),
                        attribution: "fixture only".into(),
                        restrictions: "private-planning-only".into(),
                        status: ProductionRightsStatus::Granted,
                        reference: "Synthetic assertion; no real permission".into(),
                        valid_from_unix: 10,
                        valid_until_unix: Some(200),
                    },
                },
                operation_id: "fixture-claim".into(),
                recorded_by: "owner".into(),
                recorded_at: "fixture".into(),
            })
            .collect();
        prepare_inputs("owner", &revision, &settings, &rights, true, 100)
            .unwrap()
            .document()
    }

    fn refreeze(document: &FrozenProductionDocument) -> FrozenProductionDocument {
        prepare_inputs(
            &document.owner_id,
            &document.revision,
            &document.settings,
            &document.rights,
            true,
            100,
        )
        .unwrap()
        .document()
    }

    #[test]
    fn paid_sample_blocks_missing_expired_wrong_voice_rights_and_budget_before_credentials() {
        let original = document();
        let plan = google::plan(&original.resolved[0]).unwrap();
        assert_eq!(live_blocker(&original, &plan, 1, 100), None);
        assert_eq!(
            live_blocker(&original, &plan, 0, 100).unwrap()["code"],
            "estimate_exceeds_cap"
        );
        for now in [9, 200] {
            assert_eq!(
                live_blocker(&original, &plan, 1, now).unwrap()["code"],
                "frozen_policy_blocked"
            );
        }
        type PolicyCase = (fn(&mut FrozenProductionDocument), &'static str);
        let cases: [PolicyCase; 5] = [
            (|doc| doc.rights.clear(), "frozen_policy_blocked"),
            (
                |doc| doc.rights[0].claim.declaration.status = ProductionRightsStatus::Revoked,
                "frozen_policy_blocked",
            ),
            (
                |doc| {
                    let right = doc
                        .rights
                        .iter_mut()
                        .find(|r| {
                            matches!(
                                r.claim.declaration.subject,
                                ProductionRightsSubject::Voice { .. }
                            )
                        })
                        .unwrap();
                    if let ProductionRightsSubject::Voice { voice_id, .. } =
                        &mut right.claim.declaration.subject
                    {
                        *voice_id = "vi-VN-Neural2-D".into();
                    }
                },
                "frozen_policy_blocked",
            ),
            (
                |doc| doc.settings.settings.budget.currency = ProductionCurrency::VND,
                "budget_currency_mismatch",
            ),
            (
                |doc| doc.settings.settings.budget.limit_minor = 0,
                "estimate_exceeds_cap",
            ),
        ];
        let root = env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        for (index, (change, code)) in cases.into_iter().enumerate() {
            let mut changed = original.clone();
            change(&mut changed);
            let changed = refreeze(&changed);
            let input = root.join(format!("input-{index}.json"));
            fs::write(&input, serde_json::to_vec(&changed).unwrap()).unwrap();
            let destination = root.join(format!("output-{index}"));
            let (report, success) = run(Request {
                document: input,
                dialogue_id: "dialogue-01".into(),
                live: Some(Live {
                    directory: destination.clone(),
                    max_estimate_cents: 1,
                }),
            })
            .unwrap();
            assert!(!success);
            assert_eq!(report["findings"][0]["code"], code);
            assert!(
                !destination.exists(),
                "blocked sample must write/send nothing"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn plan_only_is_side_effect_free_and_forged_scope_is_rejected() {
        let root = env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        let input = root.join("input.json");
        let mut doc = document();
        fs::write(&input, serde_json::to_vec(&doc).unwrap()).unwrap();
        let (report, success) = run(Request {
            document: input.clone(),
            dialogue_id: "dialogue-01".into(),
            live: None,
        })
        .unwrap();
        assert!(success);
        assert_eq!(report["status"], "planned");
        assert_eq!(report["sent"], false);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        doc.input_digest.push('0');
        fs::write(&input, serde_json::to_vec(&doc).unwrap()).unwrap();
        let (report, success) = run(Request {
            document: input,
            dialogue_id: "dialogue-01".into(),
            live: None,
        })
        .unwrap();
        assert!(!success);
        assert_eq!(report["status"], "scope_rejected");
        fs::remove_dir_all(root).unwrap();
    }
}
