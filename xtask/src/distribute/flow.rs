//! The distribution coordinator. A thin, ordered shell around pure decisions: it checks every
//! prerequisite and the Firebase destination before building, builds once through the existing
//! Android plan, inspects and stages exact bytes, records intent, performs the single remote
//! write and records what was observed. No stage after a failure runs; unreached stages are
//! `NOT_RUN`. There is no retry: an uncertain remote outcome is reported, never replayed.

use std::path::PathBuf;

use super::{
    apk::{
        badging_command, parse_badging, parse_output_metadata, parse_signing, signer_command,
        validate, ApkFacts, InspectError, OutputFile,
    },
    firebase::{
        self, distribute_command, interpret_distribution, interpret_lookup_package, lookup_command,
        parse_version, redact, version_command, version_supported, DistributeInputs, RemoteOutcome,
        MINIMUM_VERSION, SUPPORTED_MAJOR, TESTED_VERSION,
    },
    host::{sha256_hex, Host, RunDir, Staged},
    receipt::{NotesRecord, Receipt},
    request::{generated_notes, generated_release_notes, LiveRequest, NotesChoice, Stage},
};
use crate::{
    capture::{Capture, CaptureError, Captured},
    command::Runner,
    config::THEATRE_MOBILE_DIR,
    mobile_toolchain,
    report::{Report, Status},
    tools::ToolProbe,
    workspace::Workspace,
};

const ANDROID_TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
const BUILD_TOOLS_VERSION: &str = "35.0.0";

/// The effects a run may use. All local; the only remote effect is the one `capture` call to
/// the Firebase CLI, issued exactly once and only after every earlier stage passed.
pub struct Ports<'a> {
    pub tools: &'a dyn ToolProbe,
    pub runner: &'a mut dyn Runner,
    pub capture: &'a mut dyn Capture,
    pub host: &'a mut dyn Host,
}

pub fn run<'a>(workspace: &'a Workspace, request: &'a LiveRequest, ports: Ports<'a>) -> Report {
    let mut flow = Flow {
        workspace,
        request,
        ports,
        rows: Vec::new(),
        notes: Vec::new(),
        receipt: None,
        run: None,
        android_tools: None,
        firebase_version: None,
        remote: None,
        testers_file: None,
        mapping: None,
    };
    let _halted = flow.go();
    flow.finish()
}

/// A recorded failure; the stage row already carries the reason.
struct Halt;

struct Flow<'a> {
    workspace: &'a Workspace,
    request: &'a LiveRequest,
    ports: Ports<'a>,
    rows: Vec<(Stage, Status, String)>,
    notes: Vec<String>,
    receipt: Option<Receipt>,
    run: Option<RunDir>,
    /// `(aapt2, apksigner)` from the verified SDK build-tools.
    android_tools: Option<(PathBuf, PathBuf)>,
    firebase_version: Option<String>,
    remote: Option<RemoteOutcome>,
    testers_file: Option<PathBuf>,
    mapping: Option<Staged>,
}

impl Flow<'_> {
    fn record(&mut self, stage: Stage, status: Status, detail: impl Into<String>) {
        let detail = detail.into();
        let at = self.ports.host.now();
        if let Some(receipt) = &mut self.receipt {
            receipt.record_stage(stage.label(), status, &detail, &at);
        }
        self.rows.push((stage, status, detail));
    }

    fn pass(&mut self, stage: Stage, detail: impl Into<String>) {
        self.record(stage, Status::Pass, detail);
    }

    fn halt(&mut self, stage: Stage, status: Status, detail: impl Into<String>) -> Halt {
        self.record(stage, status, detail);
        Halt
    }

    fn go(&mut self) -> Result<(), Halt> {
        let local = self.prerequisites();
        let cli = self.firebase_cli();
        local?;
        cli?;
        self.open_run()?;
        self.destination()?;
        self.build()?;
        let (staged, facts, declared) = self.inspect()?;
        let notes_file = self.prepare(&staged, &facts, &declared)?;
        self.distribute(&staged.path, notes_file);
        Ok(())
    }

    fn prerequisites(&mut self) -> Result<(), Halt> {
        let mut problems = Vec::new();
        if !cfg!(unix) {
            problems.push("captured execution and private run files need a POSIX host".to_owned());
        }
        let required_tools = crate::theatre_host::tools();
        let missing = self.ports.tools.missing(&required_tools);
        if !missing.is_empty() {
            let details: Vec<_> = missing
                .iter()
                .map(|tool| format!("{} ({})", tool.label(), tool.hint()))
                .collect();
            problems.push(format!("missing {}", details.join(", ")));
        }
        let sdk = self.ports.host.android_sdk();
        let project = self.workspace.path(THEATRE_MOBILE_DIR);
        let prerequisite = crate::theatre_host::prerequisite(&project, sdk.as_deref());
        if self.request.policy == super::request::Policy::ProductionApk {
            let missing = self.ports.host.missing_release_signing_inputs();
            if !missing.is_empty() {
                problems.push(format!(
                    "the release signing key is operator-provided and was not supplied: set {} \
                     (xtask never creates a key and never falls back to the debug or QA key)",
                    missing.join(", ")
                ));
            }
        }
        match prerequisite {
            Err(reason) => problems.push(reason.to_owned()),
            Ok(()) => {
                let tools = sdk
                    .as_deref()
                    .map(|sdk| sdk.join(mobile_toolchain::ANDROID_BUILD_TOOLS));
                if let Some(dir) = tools {
                    let (aapt2, apksigner) = (dir.join("aapt2"), dir.join("apksigner"));
                    if mobile_toolchain::local_executable(&apksigner) {
                        self.android_tools = Some((aapt2, apksigner));
                    } else {
                        problems.push(
                            "Android build-tools 35.0.0 has no executable apksigner; xtask never installs it"
                                .to_owned(),
                        );
                    }
                }
            }
        }
        if problems.is_empty() {
            self.pass(
                Stage::Prerequisites,
                "Gradle, JDK, Android SDK platform 35 and build-tools 35.0.0 are installed",
            );
            Ok(())
        } else {
            Err(self.halt(
                Stage::Prerequisites,
                Status::Unsupported,
                problems.join("; "),
            ))
        }
    }

    fn firebase_cli(&mut self) -> Result<(), Halt> {
        let command = version_command(self.workspace.root());
        let reason = match self.ports.capture.capture(&command, firebase::VERSION_TIMEOUT) {
            Err(CaptureError::Spawn { .. }) => {
                "firebase CLI not found on PATH; install firebase-tools yourself (xtask never installs it)"
                    .to_owned()
            }
            Err(error) => format!("`firebase --version` did not report: {error}"),
            Ok(Captured { code, .. }) if code != 0 => {
                format!("`firebase --version` exited with {code}")
            }
            Ok(captured) => match parse_version(&captured.stdout) {
                Some(version) if version_supported(version) => {
                    let text = format!("{}.{}.{}", version.0, version.1, version.2);
                    self.pass(
                        Stage::FirebaseCli,
                        format!("firebase-tools {text} (adapter tested against {TESTED_VERSION})"),
                    );
                    self.firebase_version = Some(text);
                    return Ok(());
                }
                Some(version) => format!(
                    "firebase-tools {}.{}.{} is outside the documented line {SUPPORTED_MAJOR}.x >= {}.{}.{}; the adapter's output contract was not checked against it",
                    version.0, version.1, version.2, MINIMUM_VERSION.0, MINIMUM_VERSION.1, MINIMUM_VERSION.2
                ),
                None => "`firebase --version` printed an unrecognised version".to_owned(),
            },
        };
        Err(self.halt(Stage::FirebaseCli, Status::Unsupported, reason))
    }

    fn open_run(&mut self) -> Result<(), Halt> {
        let run = match self.ports.host.create_run() {
            Ok(run) => run,
            Err(why) => {
                return Err(self.halt(
                    Stage::RunRecord,
                    Status::Fail,
                    format!("could not create the private run directory: {why}"),
                ))
            }
        };
        let now = self.ports.host.now();
        let mut receipt = Receipt::new(&run.id, &now, self.request);
        let (gradle, java) = self.ports.host.tool_versions();
        receipt.tools.gradle = gradle;
        receipt.tools.java = java;
        receipt.tools.android_build_tools = Some(BUILD_TOOLS_VERSION.to_owned());
        receipt.tools.firebase_cli = self.firebase_version.clone();
        // Earlier stage rows belong in the record too.
        for (stage, status, detail) in &self.rows {
            receipt.record_stage(stage.label(), *status, detail, &now);
        }
        let identity = self.ports.host.source_identity();
        self.run = Some(run.clone());
        match identity {
            Ok(identity) => {
                receipt.source = Some(super::receipt::SourceRecord {
                    commit: identity.commit,
                    dirty: identity.dirty,
                    content_fingerprint: identity.content_fingerprint,
                });
            }
            Err(why) => {
                self.receipt = Some(receipt);
                return Err(self.halt(
                    Stage::RunRecord,
                    Status::Fail,
                    format!("source identity unavailable: {why}"),
                ));
            }
        }
        self.receipt = Some(receipt);
        let relative = self.workspace.relative(&run.path);
        if let Err(why) = self.persist() {
            return Err(self.halt(
                Stage::RunRecord,
                Status::Fail,
                format!("cannot write the receipt, so no effect was started: {why}"),
            ));
        }
        self.pass(
            Stage::RunRecord,
            format!("{relative} (receipt.json, private)"),
        );
        Ok(())
    }

    fn persist(&mut self) -> Result<PathBuf, String> {
        let now = self.ports.host.now();
        let (Some(run), Some(receipt)) = (&self.run, &mut self.receipt) else {
            return Err("no run record exists".to_owned());
        };
        receipt.updated_at = now;
        self.ports.host.persist(run, receipt)
    }

    fn run_dir(&self) -> PathBuf {
        self.run
            .as_ref()
            .map(|run| run.path.clone())
            .unwrap_or_else(|| self.workspace.root().to_path_buf())
    }

    fn destination(&mut self) -> Result<(), Halt> {
        println!("\n==> {}", Stage::Destination.label());
        let command = lookup_command(&self.request.app_id, &self.run_dir());
        println!("    $ {}", command.display(self.workspace.root()));
        let result = self
            .ports
            .capture
            .capture(&command, firebase::LOOKUP_TIMEOUT);
        match interpret_lookup_package(&self.request.app_id, self.request.policy.package(), result)
        {
            Ok(registered) => {
                if let Some(receipt) = &mut self.receipt {
                    receipt.firebase.registered_package = Some(registered.package.clone());
                }
                self.pass(
                    Stage::Destination,
                    format!(
                        "app {} is registered for {}",
                        self.request.app_id.as_str(),
                        registered.package
                    ),
                );
                Ok(())
            }
            Err(error) => Err(self.halt(Stage::Destination, Status::Fail, error.to_string())),
        }
    }

    fn build(&mut self) -> Result<(), Halt> {
        println!("\n==> {}", Stage::Build.label());
        let command = super::build_command(
            self.workspace,
            self.request.policy,
            self.request.api_origin.as_deref(),
            self.request.native_issuer.as_deref(),
        );
        match self.ports.runner.run(&command) {
            Ok(()) => {
                self.pass(Stage::Build, self.request.policy.gradle_task());
                Ok(())
            }
            Err(error) => Err(self.halt(Stage::Build, Status::Fail, format!("{error:#}"))),
        }
    }

    fn inspect(&mut self) -> Result<(Staged, ApkFacts, OutputFile), Halt> {
        let policy = self.request.policy;
        let fail = |flow: &mut Self, why: String| flow.halt(Stage::Inspect, Status::Fail, why);
        let metadata = self
            .ports
            .host
            .read_output_metadata(policy)
            .map_err(|error| error.to_string());
        let declared = match metadata.and_then(|bytes| {
            parse_output_metadata(&bytes, policy).map_err(|error| error.to_string())
        }) {
            Ok(declared) => declared,
            Err(why) => return Err(fail(self, why)),
        };
        let run = self
            .run
            .clone()
            .expect("the run record exists before inspection");
        let staged = match self
            .ports
            .host
            .stage_output(policy, &run, &declared.file_name)
        {
            Ok(staged) => staged,
            Err(error) => return Err(fail(self, error.to_string())),
        };
        let (aapt2, apksigner) = self
            .android_tools
            .clone()
            .expect("prerequisites located the Android tools");
        {
            let sdk = self.ports.host.android_sdk().unwrap_or_default();
            let inspect_command = if policy.shrinks() {
                crate::theatre_host::staged_release_inspection_command
            } else {
                crate::theatre_host::debug_inspection_command
            };
            let mut command = inspect_command(
                self.workspace,
                &sdk.to_string_lossy(),
                &staged.path,
                self.request.api_origin.as_deref().unwrap_or_default(),
                self.request.native_issuer.as_deref(),
            );
            // Keep inspection evidence with these staged bytes; concurrent distributions
            // must not overwrite another run's report.
            if let Some(index) = command.args.iter().position(|arg| arg == "--report") {
                command.args[index + 1] = run
                    .path
                    .join("production-inspection.txt")
                    .to_string_lossy()
                    .into_owned();
            }
            if let Err(error) = self.ports.runner.run(&command) {
                return Err(fail(
                    self,
                    format!("production composition inspection failed: {error:#}"),
                ));
            }
        }

        let badging = self
            .android_report(&badging_command(&aapt2, &staged.path, &run.path))
            .and_then(|c| parse_badging(c.code, &c.stdout, &c.stderr));
        let signing = self
            .android_report(&signer_command(&apksigner, &staged.path, &run.path))
            .and_then(|c| parse_signing(c.code, &c.stdout, &c.stderr));
        let (badging, signing) = match (badging, signing) {
            (Ok(badging), Ok(signing)) => (badging, signing),
            (badging, signing) => {
                let reasons: Vec<String> = [badging.err(), signing.err()]
                    .into_iter()
                    .flatten()
                    .map(|error| error.to_string())
                    .collect();
                return Err(fail(self, reasons.join("; ")));
            }
        };
        let facts = ApkFacts {
            sha256: staged.sha256.clone(),
            size_bytes: staged.size_bytes,
            badging,
            signing,
        };
        let findings = validate(
            policy,
            &declared,
            &facts,
            self.request.expected_cert.as_ref(),
        );
        if !findings.is_empty() {
            let reasons: Vec<String> = findings.iter().map(ToString::to_string).collect();
            return Err(fail(self, reasons.join("; ")));
        }
        let relative = self.workspace.relative(&staged.path);
        let expected = self
            .request
            .expected_cert
            .as_ref()
            .map(|cert| cert.as_str().to_owned());
        if let Some(receipt) = &mut self.receipt {
            receipt.set_artifact(&facts, expected.as_deref(), &relative);
        }
        if policy.shrinks() {
            // A shrunk build without its mapping cannot be diagnosed, so it is not distributed.
            match self
                .ports
                .host
                .stage_shrinker_mapping(policy, &run, &staged)
            {
                Ok(mapping) => {
                    self.mapping = Some(mapping.clone());
                    let relative = self.workspace.relative(&mapping.path);
                    if let Some(receipt) = &mut self.receipt {
                        receipt.set_shrinker_mapping(
                            &mapping.sha256,
                            mapping.size_bytes,
                            &relative,
                        );
                    }
                }
                Err(error) => {
                    return Err(fail(
                        self,
                        format!("the R8 mapping of the shrunk build could not be staged: {error}"),
                    ))
                }
            }
        }
        self.pass(
            Stage::Inspect,
            format!(
                "{} {} ({}), {} bytes, sha256 {}, signer {}",
                facts.badging.package,
                facts.badging.version_name,
                facts.badging.version_code,
                facts.size_bytes,
                short(&facts.sha256),
                short(&facts.signing.cert_sha256)
            ),
        );
        Ok((staged, facts, declared))
    }

    fn android_report(
        &mut self,
        command: &crate::command::CommandSpec,
    ) -> Result<Captured, InspectError> {
        self.ports
            .capture
            .capture(command, ANDROID_TOOL_TIMEOUT)
            .map_err(|error| InspectError::ToolUnavailable(error.to_string()))
    }

    /// Everything the remote write needs, validated and written, with intent recorded.
    fn prepare(
        &mut self,
        staged: &Staged,
        facts: &ApkFacts,
        declared: &OutputFile,
    ) -> Result<PathBuf, Halt> {
        let fail =
            |flow: &mut Self, why: String| flow.halt(Stage::IntentReceipt, Status::Fail, why);
        let run = self.run.clone().expect("the run record exists");
        let (source_label, text) = match &self.request.notes {
            NotesChoice::Generated => {
                let (commit, dirty) = self
                    .receipt
                    .as_ref()
                    .and_then(|r| r.source.as_ref())
                    .map(|s| (s.commit.clone(), s.dirty))
                    .unwrap_or_default();
                (
                    "generated".to_owned(),
                    if self.request.policy.shrinks() {
                        generated_release_notes(
                            &declared.version_name,
                            &declared.version_code,
                            &commit,
                            dirty,
                        )
                    } else {
                        generated_notes(
                            &declared.version_name,
                            &declared.version_code,
                            &commit,
                            dirty,
                        )
                    },
                )
            }
            NotesChoice::Inline(text) => ("inline".to_owned(), text.clone()),
            NotesChoice::File { text, .. } => ("file".to_owned(), text.clone()),
        };
        let notes_file =
            match self
                .ports
                .host
                .write_private(&run, "release-notes.txt", text.as_bytes())
            {
                Ok(path) => path,
                Err(why) => {
                    return Err(fail(self, format!("cannot write the release notes: {why}")))
                }
            };
        if !self.request.recipients.testers().is_empty() {
            let mut listing = self.request.recipients.testers().join("\n");
            listing.push('\n');
            match self
                .ports
                .host
                .write_private(&run, "testers.txt", listing.as_bytes())
            {
                Ok(path) => self.testers_file = Some(path),
                Err(why) => return Err(fail(self, format!("cannot write the tester list: {why}"))),
            }
        }
        if let Some(receipt) = &mut self.receipt {
            receipt.release_notes = Some(NotesRecord {
                source: source_label,
                sha256: sha256_hex(text.as_bytes()),
                bytes: text.len(),
            });
        }
        // The staged bytes must still be the inspected bytes.
        match self.ports.host.hash_file(&staged.path) {
            Ok((hash, size)) if hash == facts.sha256 && size == facts.size_bytes => {}
            Ok(_) => {
                return Err(fail(
                    self,
                    "the staged APK changed after inspection; nothing was uploaded".to_owned(),
                ))
            }
            Err(why) => return Err(fail(self, format!("cannot re-read the staged APK: {why}"))),
        }
        if let Some(mapping) = &self.mapping {
            match self.ports.host.hash_file(&mapping.path) {
                Ok((hash, size)) if hash == mapping.sha256 && size == mapping.size_bytes => {}
                _ => {
                    return Err(fail(
                        self,
                        "the staged R8 mapping changed after inspection; nothing was uploaded"
                            .to_owned(),
                    ))
                }
            }
        }
        if let Some(receipt) = &mut self.receipt {
            receipt.mark_intent();
        }
        self.record(
            Stage::IntentReceipt,
            Status::Pass,
            "intent recorded before the remote write",
        );
        if let Err(why) = self.persist() {
            // The row just pushed would claim success; replace it with the failure.
            self.rows.pop();
            if let Some(receipt) = &mut self.receipt {
                receipt.stages.pop();
            }
            return Err(fail(
                self,
                format!("cannot record intent, so nothing was uploaded: {why}"),
            ));
        }
        Ok(notes_file)
    }

    fn distribute(&mut self, staged: &std::path::Path, notes_file: PathBuf) {
        let run_dir = self.run_dir();
        let command = distribute_command(&DistributeInputs {
            run_dir: &run_dir,
            staged_apk: staged,
            app_id: &self.request.app_id,
            groups: self.request.recipients.groups(),
            notes_file: &notes_file,
            testers_file: self.testers_file.as_deref(),
        });
        println!("\n==> {} (REMOTE WRITE)", Stage::Distribution.label());
        println!("    $ {}", command.display(self.workspace.root()));
        let result = self
            .ports
            .capture
            .capture(&command, firebase::DISTRIBUTE_TIMEOUT);
        let testers: Vec<&str> = self
            .request
            .recipients
            .testers()
            .iter()
            .map(String::as_str)
            .collect();
        let diagnostics = diagnostics(&result, &testers);
        let outcome = interpret_distribution(result, firebase::DISTRIBUTE_TIMEOUT, &testers);
        if let Some(receipt) = &mut self.receipt {
            receipt.set_outcome(&outcome, diagnostics);
        }
        let (status, detail) = describe(&outcome);
        self.record(Stage::Distribution, status, detail);
        self.remote = Some(outcome);
    }

    fn finish(mut self) -> Report {
        let attempted = self.remote.is_some();
        // Record what was never reached before the receipt is finalized.
        for stage in Stage::ALL {
            if stage != Stage::FinalReceipt && !self.rows.iter().any(|(s, ..)| *s == stage) {
                self.record(
                    stage,
                    Status::NotRun,
                    "not started: an earlier stage did not pass",
                );
            }
        }
        if attempted {
            self.record(
                Stage::FinalReceipt,
                Status::Pass,
                "observed outcome recorded",
            );
        } else {
            self.record(
                Stage::FinalReceipt,
                Status::NotRun,
                "no remote write was attempted; the failure above is kept in the run record",
            );
        }
        let persisted = if self.receipt.is_some() {
            Some(self.persist())
        } else {
            None
        };
        match persisted {
            Some(Err(why)) if attempted => {
                // The remote effect cannot be undone: replace the optimistic row and point at
                // everything needed to reconcile without the missing file.
                if let Some(row) = self
                    .rows
                    .iter_mut()
                    .find(|(s, ..)| *s == Stage::FinalReceipt)
                {
                    row.1 = Status::Fail;
                    row.2 = format!("the receipt could not be written: {why}");
                }
                let run = self.run.as_ref().map(|r| self.workspace.relative(&r.path));
                self.notes.push(format!(
                    "RECOVERY: the remote outcome above is real but receipt.json is stale or missing ({why}). \
                     Run directory: {}. Record the outcome by hand; do not rerun to regenerate it.",
                    run.unwrap_or_else(|| "unknown".to_owned())
                ));
            }
            Some(Err(why)) => {
                self.notes.push(format!(
                    "the run record could not be updated after the failure: {why}"
                ));
            }
            _ => {}
        }

        let mut report = Report::new("distribute firebase");
        for stage in Stage::ALL {
            if let Some((_, status, detail)) = self.rows.iter().find(|(s, ..)| *s == stage) {
                report.record(stage.label(), *status, detail.clone());
            }
        }
        if let Some(run) = &self.run {
            report.note(format!(
                "run record: {} (private; ignored by Git)",
                self.workspace.relative(&run.path)
            ));
        }
        if let Some(outcome) = &self.remote {
            for line in outcome_notes(outcome) {
                report.note(line);
            }
            report.note(
                "the Firebase CLI may leave firebase-debug.log in the run directory; keep it private and out of shared CI artifacts",
            );
        }
        for note in self.notes {
            report.note(note);
        }
        report
    }
}

fn short(hex: &str) -> String {
    format!("{}…", hex.chars().take(12).collect::<String>())
}

/// Redacted tail of what the CLI printed, for the receipt.
fn diagnostics(result: &Result<Captured, CaptureError>, testers: &[&str]) -> Vec<String> {
    let text = match result {
        Ok(captured) => format!("{}\n{}", captured.stdout, captured.stderr),
        Err(error) => error
            .partial()
            .map(|p| format!("{}\n{}", p.stdout, p.stderr))
            .unwrap_or_default(),
    };
    let lines: Vec<String> = redact(&text, testers)
        .lines()
        .map(|line| line.trim().chars().take(300).collect::<String>())
        .filter(|line| !line.is_empty())
        .collect();
    let skip = lines.len().saturating_sub(40);
    lines.into_iter().skip(skip).collect()
}

fn describe(outcome: &RemoteOutcome) -> (Status, String) {
    match outcome {
        RemoteOutcome::NotStarted(why) => (
            Status::Fail,
            format!("the Firebase CLI did not start, nothing was written: {why}"),
        ),
        RemoteOutcome::Confirmed { release } => (
            Status::Pass,
            format!(
                "release {} ({}) {} and distributed",
                release.display_version,
                release.build_version,
                release.upload.label()
            ),
        ),
        RemoteOutcome::Rejected { kind, detail } => (
            Status::Fail,
            format!("REJECTED, confirmed before any upload was accepted: {kind} ({detail})"),
        ),
        RemoteOutcome::Partial {
            release,
            failed,
            detail,
        } => (
            Status::Fail,
            format!(
                "PARTIAL: release {} ({}) exists but the {failed} failed ({detail})",
                release.display_version, release.build_version
            ),
        ),
        RemoteOutcome::Unknown { release, reason } => (
            Status::Fail,
            match release {
                Some(release) => format!(
                    "UNKNOWN remote outcome: {reason}; release {} ({}) is known to exist",
                    release.display_version, release.build_version
                ),
                None => format!("UNKNOWN remote outcome: {reason}"),
            },
        ),
    }
}

fn outcome_notes(outcome: &RemoteOutcome) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(release) = outcome.release() {
        if let Some(uri) = &release.console_uri {
            notes.push(format!("Firebase console: {uri}"));
        }
        if let Some(uri) = &release.testing_uri {
            notes.push(format!("tester link: {uri}"));
        }
    }
    match outcome {
        RemoteOutcome::Confirmed { .. } => notes.push(
            "service-side acceptance only: tester installation, app startup and native listening are not established; check a named device separately"
                .to_owned(),
        ),
        RemoteOutcome::Partial { .. } | RemoteOutcome::Unknown { .. } => {
            notes.push(format!("RECONCILE FIRST: {}", super::receipt::RECONCILE));
        }
        RemoteOutcome::Rejected { .. } => {
            notes.push("nothing was distributed; fix the cause and run again".to_owned());
        }
        RemoteOutcome::NotStarted(_) => {}
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        capture::{testing::ScriptedCapture, Partial},
        command::{testing::RecordingRunner, CommandSpec},
        distribute::{
            apk::OutputError,
            host::SourceIdentity,
            request::{AppId, AppIdSource, CertFingerprint, Policy, Recipients},
        },
        tools::{testing::FakeTools, Tool},
    };
    use std::path::Path;

    const SDK_REGISTERED: &str = include_str!(
        "../../fixtures/firebase-tools-15.34.0/sdkconfig-registered-development-policy.json"
    );
    const SDK_EMPTY: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/sdkconfig-unregistered.json");
    const CREATED: &str = include_str!(
        "../../fixtures/firebase-tools-15.34.0/distribute-created-development-policy.txt"
    );
    const BADGING: &str = include_str!("../../fixtures/android/badging-development-policy.txt");
    const SIGNER: &str = include_str!("../../fixtures/android/apksigner-demo.txt");
    const METADATA: &str = r#"{"version":3,"artifactType":{"type":"APK","kind":"Directory"},
      "applicationId":"com.cantos.theatre.dev","variantName":"debug",
      "elements":[{"type":"SINGLE","filters":[],"attributes":[],"versionCode":1,
      "versionName":"0.1.0-dev","outputFile":"androidApp-debug.apk"}]}"#;
    const TESTER: &str = "tester.one@example.com";

    struct FakeHost {
        sdk: Option<PathBuf>,
        identity: Result<SourceIdentity, String>,
        metadata: Result<Vec<u8>, OutputError>,
        stage: Result<Staged, OutputError>,
        missing_qa_signing: Vec<&'static str>,
        mapping: Result<Staged, OutputError>,
        rehash: Result<(String, u64), String>,
        mapping_rehash: Option<Result<(String, u64), String>>,
        fail_persist_from: Option<usize>,
        fail_create_run: bool,
        persisted: Vec<String>,
        private_files: Vec<(String, Vec<u8>)>,
        runs_created: usize,
        order: Vec<&'static str>,
    }

    impl FakeHost {
        fn new(sdk: &Path) -> Self {
            Self {
                sdk: Some(sdk.to_path_buf()),
                identity: Ok(SourceIdentity {
                    commit: "0123456789abcdef0123456789abcdef01234567".into(),
                    dirty: false,
                    content_fingerprint: None,
                }),
                metadata: Ok(METADATA.as_bytes().to_vec()),
                stage: Ok(staged()),
                missing_qa_signing: Vec::new(),
                mapping: Ok(Staged {
                    path: PathBuf::from("/repo/artifacts/distributions/RUN/mapping.txt"),
                    sha256: "cd".repeat(32),
                    size_bytes: 31_000_000,
                }),
                rehash: Ok(("ab".repeat(32), 4096)),
                mapping_rehash: None,
                fail_persist_from: None,
                fail_create_run: false,
                persisted: Vec::new(),
                private_files: Vec::new(),
                runs_created: 0,
                order: Vec::new(),
            }
        }
    }

    fn staged() -> Staged {
        Staged {
            path: PathBuf::from("/repo/artifacts/distributions/RUN/artifact/androidApp-debug.apk"),
            sha256: "ab".repeat(32),
            size_bytes: 4096,
        }
    }

    impl Host for FakeHost {
        fn android_sdk(&self) -> Option<PathBuf> {
            self.sdk.clone()
        }
        fn now(&mut self) -> String {
            "2026-10-15T00:00:00Z".into()
        }
        fn source_identity(&mut self) -> Result<SourceIdentity, String> {
            self.identity.clone()
        }
        fn tool_versions(&mut self) -> (Option<String>, Option<String>) {
            (
                Some("Gradle 8.14.3".into()),
                Some("openjdk version \"17\"".into()),
            )
        }
        fn create_run(&mut self) -> Result<RunDir, String> {
            if self.fail_create_run {
                return Err("read-only file system".into());
            }
            self.runs_created += 1;
            self.order.push("create_run");
            Ok(RunDir {
                id: "RUN".into(),
                path: PathBuf::from("/repo/artifacts/distributions/RUN"),
            })
        }
        fn read_output_metadata(&mut self, _: Policy) -> Result<Vec<u8>, OutputError> {
            self.metadata.clone()
        }
        fn stage_output(&mut self, _: Policy, _: &RunDir, _: &str) -> Result<Staged, OutputError> {
            self.order.push("stage");
            self.stage.clone()
        }
        fn missing_release_signing_inputs(&self) -> Vec<&'static str> {
            self.missing_qa_signing.clone()
        }
        fn stage_shrinker_mapping(
            &mut self,
            _: Policy,
            _: &RunDir,
            _: &Staged,
        ) -> Result<Staged, OutputError> {
            self.order.push("stage_mapping");
            self.mapping.clone()
        }
        fn write_private(
            &mut self,
            run: &RunDir,
            name: &str,
            bytes: &[u8],
        ) -> Result<PathBuf, String> {
            self.private_files.push((name.to_owned(), bytes.to_vec()));
            Ok(run.path.join(name))
        }
        fn hash_file(&mut self, path: &Path) -> Result<(String, u64), String> {
            if path.ends_with("mapping.txt") {
                if let Some(result) = &self.mapping_rehash {
                    return result.clone();
                }
                return self
                    .mapping
                    .as_ref()
                    .map(|m| (m.sha256.clone(), m.size_bytes))
                    .map_err(ToString::to_string);
            }
            self.rehash.clone()
        }
        fn persist(&mut self, run: &RunDir, receipt: &Receipt) -> Result<PathBuf, String> {
            let call = self.persisted.len() + 1;
            if self.fail_persist_from.is_some_and(|from| call >= from) {
                self.persisted.push("FAILED".into());
                return Err("disk full".into());
            }
            self.order.push("persist");
            self.persisted
                .push(String::from_utf8(receipt.to_json()).unwrap());
            Ok(run.path.join("receipt.json"))
        }
    }

    fn fake_sdk(name: &str) -> PathBuf {
        let sdk =
            std::env::temp_dir().join(format!("xtask-flow-sdk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&sdk);
        let tools = sdk.join(mobile_toolchain::ANDROID_BUILD_TOOLS);
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::create_dir_all(sdk.join("platforms/android-35")).unwrap();
        std::fs::write(sdk.join("platforms/android-35/android.jar"), "").unwrap();
        for tool in ["aapt2", "d8", "apksigner"] {
            let path = tools.join(tool);
            std::fs::write(&path, "fixture; never executed").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        sdk
    }

    fn live(testers: &[&str]) -> LiveRequest {
        LiveRequest {
            api_origin: Some("https://api.cantos.example".into()),
            native_issuer: None,
            policy: Policy::DevelopmentApk,
            app_id: AppId::parse("1:000000000000:android:0123456789abcdef").unwrap(),
            app_id_source: AppIdSource::Flag,
            recipients: Recipients::new(
                &["cantos-internal".to_owned()],
                &testers.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>(),
            )
            .unwrap(),
            notes: NotesChoice::Generated,
            expected_cert: None,
        }
    }

    struct Rig {
        sdk: PathBuf,
        host: FakeHost,
        runner: RecordingRunner,
        capture: ScriptedCapture,
        tools: FakeTools,
    }

    impl Rig {
        fn new(name: &str) -> Self {
            let sdk = fake_sdk(name);
            let mut capture = ScriptedCapture::default();
            capture
                .push_ok(0, "15.34.0\n", "")
                .push_ok(0, SDK_REGISTERED, "- Downloading")
                .push_ok(0, BADGING, "")
                .push_ok(0, SIGNER, "WARNING: x")
                .push_ok(0, CREATED, "");
            Self {
                host: FakeHost::new(&sdk),
                sdk,
                runner: RecordingRunner::default(),
                capture,
                tools: FakeTools::all(),
            }
        }

        fn run(&mut self, request: &LiveRequest) -> Report {
            let workspace = Workspace::discover().unwrap();
            run(
                &workspace,
                request,
                Ports {
                    tools: &self.tools,
                    runner: &mut self.runner,
                    capture: &mut self.capture,
                    host: &mut self.host,
                },
            )
        }

        fn programs(&self) -> Vec<String> {
            self.capture
                .calls
                .iter()
                .map(|c| {
                    format!(
                        "{} {}",
                        c.program,
                        c.args.first().cloned().unwrap_or_default()
                    )
                })
                .collect()
        }

        fn distribute_calls(&self) -> usize {
            self.capture
                .calls
                .iter()
                .filter(|c| {
                    c.args
                        .first()
                        .is_some_and(|a| a == "appdistribution:distribute")
                })
                .count()
        }
    }

    impl Drop for Rig {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.sdk);
        }
    }

    fn status_of(report: &Report, stage: Stage) -> Status {
        report
            .outcomes
            .iter()
            .find(|o| o.name == stage.label())
            .unwrap_or_else(|| panic!("no row for {}", stage.label()))
            .status
    }

    fn detail_of(report: &Report, stage: Stage) -> String {
        report
            .outcomes
            .iter()
            .find(|o| o.name == stage.label())
            .unwrap()
            .detail
            .clone()
    }

    #[test]
    fn a_complete_run_builds_once_inspects_then_writes_remotely_exactly_once() {
        let mut rig = Rig::new("happy");
        let report = rig.run(&live(&[TESTER]));
        assert!(report.succeeded(), "{}", report.render());
        assert!(report.outcomes.iter().all(|o| o.status == Status::Pass));
        assert_eq!(report.outcomes.len(), Stage::ALL.len());
        // Version, destination, two Android inspections, one distribution; one Gradle build.
        assert_eq!(
            rig.programs(),
            [
                "firebase --version",
                "firebase apps:sdkconfig",
                format!(
                    "{}/aapt2 dump",
                    rig.sdk.join("build-tools/35.0.0").display()
                )
                .as_str(),
                format!(
                    "{}/apksigner verify",
                    rig.sdk.join("build-tools/35.0.0").display()
                )
                .as_str(),
                "firebase appdistribution:distribute",
            ]
        );
        assert_eq!(rig.runner.commands.len(), 2);
        assert!(rig.runner.commands[0]
            .args
            .contains(&":androidApp:assembleDebug".to_owned()));
        assert_eq!(rig.distribute_calls(), 1);
        // Receipts: started, intent (before the write), complete.
        assert_eq!(rig.host.persisted.len(), 3);
        assert!(rig.host.persisted[0].contains("\"phase\": \"started\""));
        assert!(rig.host.persisted[1].contains("\"phase\": \"intent\""));
        assert!(rig.host.persisted[1].contains("\"state\": \"write_pending\""));
        assert!(rig.host.persisted[2].contains("\"state\": \"confirmed\""));
        assert!(rig.host.persisted[2].contains("releases/0a1b2c3d4e5f6"));
    }

    const METADATA_QA: &str = r#"{"version":3,"artifactType":{"type":"APK","kind":"Directory"},
      "applicationId":"com.cantos.theatre","variantName":"release",
      "elements":[{"type":"SINGLE","filters":[],"attributes":[],"versionCode":1,
      "versionName":"0.1.0","outputFile":"androidApp-release.apk"}]}"#;
    const BADGING_QA: &str = include_str!("../../fixtures/android/badging-release-policy.txt");

    /// A rig for the optimized QA lane: QA metadata/badging, the fixture's signer pinned.
    fn qa_rig(name: &str) -> (Rig, LiveRequest) {
        let mut rig = Rig::new(name);
        rig.host.metadata = Ok(METADATA_QA.as_bytes().to_vec());
        rig.capture.results.clear();
        rig.capture
            .push_ok(0, "15.34.0\n", "")
            .push_ok(
                0,
                &SDK_REGISTERED.replace("com.cantos.theatre.dev", "com.cantos.theatre"),
                "",
            )
            .push_ok(0, BADGING_QA, "")
            .push_ok(0, SIGNER, "WARNING: x")
            .push_ok(0, &CREATED.replace("0.1.0-dev", "0.1.0"), "");
        let mut request = live(&[TESTER]);
        request.policy = Policy::ProductionApk;
        request.expected_cert = Some(CertFingerprint::parse(&"a".repeat(64)).unwrap());
        (rig, request)
    }

    #[test]
    fn the_optimized_qa_lane_builds_its_own_task_stages_the_r8_mapping_and_distributes_once() {
        let (mut rig, request) = qa_rig("qa-happy");
        let report = rig.run(&request);
        assert!(report.succeeded(), "{}", report.render());
        assert_eq!(rig.runner.commands.len(), 2);
        let build = &rig.runner.commands[0];
        assert!(build.args.contains(&"android-release".to_owned()));
        assert!(build.args.contains(&"--operator-signing".to_owned()));
        assert!(!build.args.contains(&":androidApp:assembleDebug".to_owned()));
        // The mapping is staged after inspection and before the intent receipt / remote write.
        let stage = rig
            .host
            .order
            .iter()
            .position(|step| *step == "stage")
            .unwrap();
        let mapping = rig
            .host
            .order
            .iter()
            .position(|step| *step == "stage_mapping")
            .unwrap();
        assert!(stage < mapping);
        assert_eq!(rig.distribute_calls(), 1);
        let intent = &rig.host.persisted[1];
        assert!(intent.contains("\"channel\": \"production\""), "{intent}");
        assert!(intent.contains("\"variant\": \"release\""), "{intent}");
        assert!(intent.contains("\"debuggable\": false"), "{intent}");
        assert!(intent.contains("\"shrinker_mapping\""), "{intent}");
        assert!(intent.contains(&"cd".repeat(32)), "{intent}");
        let (_, notes) = rig
            .host
            .private_files
            .iter()
            .find(|(name, _)| name == "release-notes.txt")
            .unwrap();
        let notes = String::from_utf8_lossy(notes);
        assert!(
            notes.contains("optimized") && notes.contains("0.1.0 (1)"),
            "{notes}"
        );
        assert!(notes.contains("preserve app data"), "{notes}");
    }

    #[test]
    fn changed_staged_mapping_stops_before_intent_or_upload() {
        let (mut rig, request) = qa_rig("release-mapping-tamper");
        rig.host.mapping_rehash = Some(Ok(("wrong".to_owned(), 31_000_000)));
        let report = rig.run(&request);
        assert_eq!(status_of(&report, Stage::IntentReceipt), Status::Fail);
        assert!(detail_of(&report, Stage::IntentReceipt).contains("staged R8 mapping changed"));
        assert_eq!(status_of(&report, Stage::Distribution), Status::NotRun);
        assert_eq!(rig.distribute_calls(), 0);
    }

    #[test]
    fn the_qa_lane_needs_the_operators_key_before_any_run_record_or_build() {
        let (mut rig, request) = qa_rig("qa-no-key");
        rig.host.missing_qa_signing =
            vec!["CANTOS_RELEASE_KEYSTORE", "CANTOS_RELEASE_KEY_PASSWORD"];
        let report = rig.run(&request);
        assert_eq!(
            status_of(&report, Stage::Prerequisites),
            Status::Unsupported
        );
        let detail = detail_of(&report, Stage::Prerequisites);
        assert!(
            detail.contains("CANTOS_RELEASE_KEYSTORE, CANTOS_RELEASE_KEY_PASSWORD"),
            "{detail}"
        );
        assert!(detail.contains("never creates a key"), "{detail}");
        assert!(rig.runner.commands.is_empty());
        assert_eq!(rig.distribute_calls(), 0);
        assert_eq!(rig.host.runs_created, 0);
        assert_eq!(status_of(&report, Stage::Build), Status::NotRun);
    }

    #[test]
    fn a_debuggable_wrongly_signed_or_mappingless_qa_artifact_is_never_distributed() {
        // Debuggable bytes (the Debug demo APK) under the QA policy.
        let (mut rig, request) = qa_rig("qa-debuggable");
        rig.capture.results.clear();
        rig.capture
            .push_ok(0, "15.34.0\n", "")
            .push_ok(
                0,
                &SDK_REGISTERED.replace("com.cantos.theatre.dev", "com.cantos.theatre"),
                "",
            )
            .push_ok(
                0,
                &BADGING_QA
                    .to_owned()
                    .replace("main\n", "application-debuggable\nmain\n"),
                "",
            )
            .push_ok(0, SIGNER, "");
        let report = rig.run(&request);
        assert_eq!(status_of(&report, Stage::Inspect), Status::Fail);
        assert!(detail_of(&report, Stage::Inspect).contains("debuggable"));
        assert_eq!(rig.distribute_calls(), 0);

        // A certificate other than the pinned one.
        let (mut rig, mut request) = qa_rig("qa-wrong-signer");
        request.expected_cert = Some(CertFingerprint::parse(&"b".repeat(64)).unwrap());
        let report = rig.run(&request);
        assert_eq!(status_of(&report, Stage::Inspect), Status::Fail);
        assert!(
            detail_of(&report, Stage::Inspect).contains("differs from the configured expectation")
        );
        assert_eq!(rig.distribute_calls(), 0);
        assert!(
            !rig.host.order.contains(&"stage_mapping"),
            "no mapping before validation"
        );

        // A shrunk build whose R8 mapping is missing cannot be diagnosed.
        let (mut rig, request) = qa_rig("qa-no-mapping");
        rig.host.mapping = Err(OutputError::FileMissing("mapping.txt".into()));
        let report = rig.run(&request);
        assert_eq!(status_of(&report, Stage::Inspect), Status::Fail);
        assert!(detail_of(&report, Stage::Inspect).contains("R8 mapping"));
        assert_eq!(rig.distribute_calls(), 0);
        assert_eq!(status_of(&report, Stage::Distribution), Status::NotRun);
    }

    #[test]
    fn the_debug_demo_lane_stages_no_mapping_and_needs_no_operator_key() {
        let mut rig = Rig::new("demo-no-mapping");
        rig.host.missing_qa_signing = vec!["CANTOS_RELEASE_KEYSTORE"];
        let report = rig.run(&live(&[TESTER]));
        assert!(report.succeeded(), "{}", report.render());
        assert!(!rig.host.order.contains(&"stage_mapping"));
        assert!(rig.host.persisted[1].contains("\"shrinker_mapping\": null"));
    }

    #[test]
    fn production_missing_python_stops_before_destination_build_or_upload() {
        let mut rig = Rig::new("production-python");
        let mut request = live(&[]);
        request.policy = Policy::DevelopmentApk;
        request.api_origin = Some("https://api.cantos.example".into());
        rig.tools = FakeTools::without(&[Tool::Python3]);
        let report = rig.run(&request);
        assert_eq!(
            status_of(&report, Stage::Prerequisites),
            Status::Unsupported
        );
        assert!(rig.runner.commands.is_empty());
        // Local prerequisite diagnostics run independently, including the CLI version probe.
        // A missing inspector must prevent destination lookup and every later effect.
        assert_eq!(rig.programs(), ["firebase --version"]);
        assert_eq!(rig.distribute_calls(), 0);
        assert_eq!(rig.host.runs_created, 0);
        assert!(rig.host.persisted.is_empty());
        for stage in [
            Stage::RunRecord,
            Stage::Destination,
            Stage::Build,
            Stage::Inspect,
            Stage::IntentReceipt,
            Stage::Distribution,
            Stage::FinalReceipt,
        ] {
            assert_eq!(status_of(&report, stage), Status::NotRun, "{stage:?}");
        }
    }

    #[test]
    fn production_composition_is_inspected_before_upload_and_failed_exclusion_stops_distribution() {
        for fail_inspection in [false, true] {
            let mut rig = Rig::new("production-inspection");
            let mut request = live(&[]);
            request.policy = Policy::DevelopmentApk;
            request.api_origin = Some("https://api.cantos.example".into());
            if fail_inspection {
                rig.runner.fail_on = Some("inspect_android_release.py".into());
            }
            let report = rig.run(&request);
            assert_eq!(rig.runner.commands.len(), 2);
            assert!(rig.runner.commands[0]
                .args
                .contains(&":androidApp:assembleDebug".to_owned()));
            let inspection = &rig.runner.commands[1];
            assert_eq!(inspection.program, "python3");
            assert!(inspection.args.contains(&"--debug-apk".to_owned()));
            assert!(inspection.args.contains(
                &"/repo/artifacts/distributions/RUN/production-inspection.txt".to_owned()
            ));
            if fail_inspection {
                assert_eq!(status_of(&report, Stage::Inspect), Status::Fail);
                assert_eq!(status_of(&report, Stage::Distribution), Status::NotRun);
                assert_eq!(rig.distribute_calls(), 0);
            } else {
                assert!(report.succeeded(), "{}", report.render());
                assert_eq!(rig.distribute_calls(), 1);
                assert!(rig
                    .host
                    .persisted
                    .last()
                    .unwrap()
                    .contains("https://api.cantos.example"));
            }
        }
    }

    #[test]
    fn the_run_never_shows_tester_addresses_or_signed_links() {
        let mut rig = Rig::new("secrets");
        let report = rig.run(&live(&[TESTER]));
        let everything = format!(
            "{}\n{}\n{:?}",
            report.render(),
            rig.host.persisted.join("\n"),
            rig.capture.calls
        );
        assert!(!everything.contains(TESTER), "{everything}");
        assert!(!everything.contains("googleapis.com"), "{everything}");
        assert!(!everything.contains("SYNTHETIC"), "{everything}");
        // The address reached only the private tester file.
        let (name, bytes) = rig
            .host
            .private_files
            .iter()
            .find(|(name, _)| name == "testers.txt")
            .unwrap();
        assert_eq!(name, "testers.txt");
        assert_eq!(String::from_utf8_lossy(bytes), format!("{TESTER}\n"));
        let distribute = rig.capture.calls.last().unwrap();
        assert!(distribute
            .args
            .iter()
            .any(|a| a.starts_with("--testers-file=")));
        assert!(distribute
            .args
            .iter()
            .any(|a| a == "--groups=cantos-internal"));
    }

    #[test]
    fn generated_notes_are_demo_labeled_and_hashed_not_printed() {
        let mut rig = Rig::new("notes");
        rig.run(&live(&[]));
        let (_, notes) = rig
            .host
            .private_files
            .iter()
            .find(|(name, _)| name == "release-notes.txt")
            .unwrap();
        let text = String::from_utf8_lossy(notes);
        assert!(
            text.contains("real-only")
                && text.contains("0.1.0-dev (1)")
                && text.contains("0123456789ab")
        );
        assert!(rig
            .host
            .persisted
            .last()
            .unwrap()
            .contains("\"source\": \"generated\""));
        assert!(!rig
            .host
            .private_files
            .iter()
            .any(|(n, _)| n == "testers.txt"));
    }

    #[test]
    fn provided_multiline_unicode_notes_are_preserved_byte_for_byte() {
        let notes = "Bản thử nghiệm — dòng 1\nDòng 2: \"trích dẫn\" và \\n literal\n";
        let mut request = live(&[]);
        request.notes = NotesChoice::Inline(notes.to_owned());
        let mut rig = Rig::new("unicode");
        let report = rig.run(&request);
        assert!(report.succeeded());
        let (_, written) = rig
            .host
            .private_files
            .iter()
            .find(|(name, _)| name == "release-notes.txt")
            .unwrap();
        assert_eq!(written, notes.as_bytes());
        let argv_has_notes = rig
            .capture
            .calls
            .iter()
            .any(|c| c.args.iter().any(|a| a.contains("Bản thử")));
        assert!(!argv_has_notes, "notes must travel in the file, not argv");
    }

    #[test]
    fn missing_local_prerequisites_stop_everything_before_any_effect() {
        for missing in [Tool::Gradle, Tool::Java, Tool::TheatrePreviewGradleProject] {
            let mut rig = Rig::new("tools");
            rig.tools = FakeTools::without(&[missing]);
            let report = rig.run(&live(&[]));
            assert_eq!(
                status_of(&report, Stage::Prerequisites),
                Status::Unsupported,
                "{missing:?}"
            );
            assert!(detail_of(&report, Stage::Prerequisites).contains(missing.label()));
            assert!(rig.runner.commands.is_empty());
            assert_eq!(rig.distribute_calls(), 0);
            assert_eq!(
                rig.host.runs_created, 0,
                "no run directory for a failed prerequisite"
            );
            assert!(rig.host.persisted.is_empty());
            assert!(!report.succeeded());
        }
    }

    #[test]
    fn a_missing_sdk_or_apksigner_is_unsupported_without_installing_anything() {
        let mut rig = Rig::new("sdk");
        rig.host.sdk = None;
        let report = rig.run(&live(&[]));
        assert_eq!(
            status_of(&report, Stage::Prerequisites),
            Status::Unsupported
        );
        assert!(detail_of(&report, Stage::Prerequisites).contains("ANDROID_HOME"));
        assert!(rig.runner.commands.is_empty());
        assert_eq!(rig.distribute_calls(), 0);

        let mut rig = Rig::new("apksigner");
        std::fs::remove_file(rig.sdk.join("build-tools/35.0.0/apksigner")).unwrap();
        let report = rig.run(&live(&[]));
        assert!(detail_of(&report, Stage::Prerequisites).contains("apksigner"));
        assert!(rig.runner.commands.is_empty());
    }

    #[test]
    fn an_absent_or_incompatible_firebase_cli_is_unsupported_before_build_or_run_files() {
        for (result, expect) in [
            (
                Err(CaptureError::Spawn {
                    program: "firebase".into(),
                    message: "not found".into(),
                }),
                "not found on PATH",
            ),
            (
                Ok(Captured {
                    code: 0,
                    stdout: "14.9.0\n".into(),
                    stderr: String::new(),
                }),
                "outside the documented line",
            ),
            (
                Ok(Captured {
                    code: 0,
                    stdout: "16.0.0\n".into(),
                    stderr: String::new(),
                }),
                "outside the documented line",
            ),
            (
                Ok(Captured {
                    code: 0,
                    stdout: "garbage\n".into(),
                    stderr: String::new(),
                }),
                "unrecognised",
            ),
            (
                Ok(Captured {
                    code: 2,
                    stdout: String::new(),
                    stderr: String::new(),
                }),
                "exited with 2",
            ),
        ] {
            let mut rig = Rig::new("cli");
            rig.capture.results.clear();
            rig.capture.results.push_back(result);
            let report = rig.run(&live(&[]));
            assert_eq!(status_of(&report, Stage::FirebaseCli), Status::Unsupported);
            assert!(
                detail_of(&report, Stage::FirebaseCli).contains(expect),
                "{expect}"
            );
            assert_eq!(rig.capture.calls.len(), 1, "only the version probe ran");
            assert!(rig.runner.commands.is_empty());
            assert_eq!(rig.host.runs_created, 0);
        }
    }

    #[test]
    fn destination_failures_prevent_the_build_and_the_upload() {
        let cases: Vec<(Result<Captured, CaptureError>, &str)> = vec![
            (
                Ok(Captured {
                    code: 0,
                    stdout: SDK_EMPTY.into(),
                    stderr: String::new(),
                }),
                "no Android app",
            ),
            (
                Ok(Captured {
                    code: 0,
                    stdout: SDK_REGISTERED.replace("com.cantos.theatre.dev", "com.cantos.theatre"),
                    stderr: String::new(),
                }),
                "registered for package `com.cantos.theatre`",
            ),
            (
                Ok(Captured {
                    code: 1,
                    stdout: String::new(),
                    stderr: "Error: Failed to authenticate, have you run firebase login?".into(),
                }),
                "authentication was rejected",
            ),
            (
                Err(CaptureError::TimedOut {
                    after: firebase::LOOKUP_TIMEOUT,
                    partial: Partial::default(),
                }),
                "did not complete",
            ),
        ];
        for (lookup, expect) in cases {
            let mut rig = Rig::new("destination");
            rig.capture.results.truncate(1);
            rig.capture.results.push_back(lookup);
            let report = rig.run(&live(&[]));
            assert_eq!(status_of(&report, Stage::Destination), Status::Fail);
            assert!(
                detail_of(&report, Stage::Destination).contains(expect),
                "{expect}: {}",
                detail_of(&report, Stage::Destination)
            );
            assert_eq!(status_of(&report, Stage::Build), Status::NotRun);
            assert_eq!(status_of(&report, Stage::Distribution), Status::NotRun);
            assert!(rig.runner.commands.is_empty());
            assert_eq!(rig.distribute_calls(), 0);
            // The failure itself is recorded in the run's receipt.
            assert!(rig
                .host
                .persisted
                .last()
                .unwrap()
                .contains("\"status\": \"FAIL\""));
        }
    }

    #[test]
    fn a_failed_build_stops_before_inspection_and_upload() {
        let mut rig = Rig::new("build");
        rig.runner.fail_on = Some("assembleDebug".into());
        let report = rig.run(&live(&[]));
        assert_eq!(status_of(&report, Stage::Build), Status::Fail);
        assert_eq!(status_of(&report, Stage::Inspect), Status::NotRun);
        assert_eq!(rig.host.order.iter().filter(|s| **s == "stage").count(), 0);
        assert_eq!(rig.distribute_calls(), 0);
    }

    #[test]
    fn artifact_problems_each_prevent_the_upload_with_an_exact_reason() {
        type Tweak = fn(&mut Rig);
        let cases: Vec<(&str, Tweak, &str)> = vec![
            (
                "missing metadata",
                |r| r.host.metadata = Err(OutputError::MetadataMissing),
                "no output-metadata.json",
            ),
            (
                "ambiguous output",
                |r| {
                    r.host.metadata = Ok(METADATA
                        .replace(r#""elements":["#, r#""elements":[{"type":"SINGLE","filters":[],"outputFile":"b.apk","versionCode":1,"versionName":"x"},"#)
                        .into_bytes())
                },
                "2 outputs",
            ),
            (
                "variant mismatch",
                |r| r.host.metadata = Ok(METADATA.replace("\"debug\"", "\"release\"").into_bytes()),
                "variant `release`",
            ),
            (
                "declared file missing",
                |r| r.host.stage = Err(OutputError::FileMissing("androidApp-debug.apk".into())),
                "does not exist",
            ),
        ];
        for (name, tweak, expect) in cases {
            let mut rig = Rig::new("artifact");
            tweak(&mut rig);
            let report = rig.run(&live(&[]));
            assert_eq!(status_of(&report, Stage::Inspect), Status::Fail, "{name}");
            assert!(
                detail_of(&report, Stage::Inspect).contains(expect),
                "{name}: {}",
                detail_of(&report, Stage::Inspect)
            );
            assert_eq!(rig.distribute_calls(), 0, "{name}");
            assert_eq!(
                status_of(&report, Stage::Distribution),
                Status::NotRun,
                "{name}"
            );
        }
    }

    #[test]
    fn tool_reports_decide_package_signature_and_certificate() {
        /// What `aapt2` and `apksigner` print (exit code, stdout), the configured certificate and
        /// the reason the run must refuse the APK.
        struct Case {
            name: &'static str,
            badging: (i32, String),
            signer: (i32, String),
            certificate: Option<CertFingerprint>,
            reason: &'static str,
        }
        let wrong_package =
            BADGING.replace("name='com.cantos.theatre.dev'", "name='com.cantos.theatre'");
        let unsigned = "DOES NOT VERIFY\nERROR: Missing META-INF/MANIFEST.MF\n";
        let case = |name, badging: (i32, &str), signer: (i32, &str), certificate, reason| Case {
            name,
            badging: (badging.0, badging.1.to_owned()),
            signer: (signer.0, signer.1.to_owned()),
            certificate,
            reason,
        };
        let cases = [
            case(
                "package mismatch",
                (0, wrong_package.as_str()),
                (0, SIGNER),
                None,
                "requires `com.cantos.theatre.dev`",
            ),
            case(
                "unsigned",
                (0, BADGING),
                (1, unsigned),
                None,
                "signature does not verify",
            ),
            case(
                "unreadable",
                (1, ""),
                (0, SIGNER),
                None,
                "not readable by aapt2",
            ),
            case(
                "certificate mismatch",
                (0, BADGING),
                (0, SIGNER),
                Some(CertFingerprint::parse(&"ee".repeat(32)).unwrap()),
                "configured expectation",
            ),
        ];
        for case in cases {
            let mut rig = Rig::new("tools-report");
            rig.capture.results.truncate(2);
            rig.capture.push_ok(case.badging.0, &case.badging.1, "");
            rig.capture.push_ok(case.signer.0, &case.signer.1, "");
            let mut request = live(&[]);
            request.expected_cert = case.certificate;
            let report = rig.run(&request);
            assert_eq!(
                status_of(&report, Stage::Inspect),
                Status::Fail,
                "{}",
                case.name
            );
            assert!(
                detail_of(&report, Stage::Inspect).contains(case.reason),
                "{}: {}",
                case.name,
                detail_of(&report, Stage::Inspect)
            );
            assert_eq!(rig.distribute_calls(), 0, "{}", case.name);
        }
    }

    #[test]
    fn a_matching_configured_certificate_passes() {
        let mut rig = Rig::new("cert-ok");
        let mut request = live(&[]);
        request.expected_cert = Some(CertFingerprint::parse(&"a".repeat(64)).unwrap());
        let report = rig.run(&request);
        assert!(report.succeeded(), "{}", report.render());
        assert!(rig.host.persisted.last().unwrap().contains(&"a".repeat(64)));
    }

    #[test]
    fn staged_bytes_that_changed_after_inspection_are_never_uploaded() {
        let mut rig = Rig::new("altered");
        rig.host.rehash = Ok(("cd".repeat(32), 4096));
        let report = rig.run(&live(&[]));
        assert_eq!(status_of(&report, Stage::IntentReceipt), Status::Fail);
        assert!(detail_of(&report, Stage::IntentReceipt).contains("changed after inspection"));
        assert_eq!(rig.distribute_calls(), 0);
        // Intent was never recorded, so the receipt cannot claim a pending write.
        assert!(!rig
            .host
            .persisted
            .iter()
            .any(|p| p.contains("write_pending")));
    }

    #[test]
    fn if_intent_cannot_be_recorded_nothing_is_uploaded() {
        let mut rig = Rig::new("intent");
        rig.host.fail_persist_from = Some(2);
        let report = rig.run(&live(&[]));
        assert_eq!(status_of(&report, Stage::IntentReceipt), Status::Fail);
        assert!(detail_of(&report, Stage::IntentReceipt).contains("nothing was uploaded"));
        assert_eq!(rig.distribute_calls(), 0);
        assert_eq!(status_of(&report, Stage::Distribution), Status::NotRun);
    }

    #[test]
    fn if_the_first_receipt_cannot_be_written_no_effect_starts() {
        let mut rig = Rig::new("first");
        rig.host.fail_persist_from = Some(1);
        let report = rig.run(&live(&[]));
        assert_eq!(status_of(&report, Stage::RunRecord), Status::Fail);
        assert!(rig.capture.calls.len() == 1, "only the version probe ran");
        assert!(rig.runner.commands.is_empty());

        let mut rig = Rig::new("no-run");
        rig.host.fail_create_run = true;
        let report = rig.run(&live(&[]));
        assert_eq!(status_of(&report, Stage::RunRecord), Status::Fail);
        assert!(rig.runner.commands.is_empty());
        assert_eq!(rig.distribute_calls(), 0);

        let mut rig = Rig::new("no-git");
        rig.host.identity = Err("git unavailable".into());
        let report = rig.run(&live(&[]));
        assert!(detail_of(&report, Stage::RunRecord).contains("source identity"));
        assert!(rig.runner.commands.is_empty());
    }

    fn scripted_distribution(rig: &mut Rig, result: Result<Captured, CaptureError>) {
        rig.capture.results.truncate(4);
        rig.capture.results.push_back(result);
    }

    #[test]
    fn upload_success_then_distribution_failure_is_partial_and_keeps_the_release() {
        let mut rig = Rig::new("partial");
        scripted_distribution(
            &mut rig,
            Ok(Captured {
                code: 1,
                stdout: CREATED.into(),
                stderr: "Error: failed to distribute to testers/groups: invalid groups".into(),
            }),
        );
        let report = rig.run(&live(&[]));
        assert_eq!(status_of(&report, Stage::Distribution), Status::Fail);
        assert!(detail_of(&report, Stage::Distribution).contains("PARTIAL"));
        assert!(!report.succeeded());
        let last = rig.host.persisted.last().unwrap();
        assert!(last.contains("\"state\": \"partial\""));
        assert!(last.contains("\"build_version\": \"1\""));
        assert!(last.contains("distribution to testers/groups"));
        let rendered = report.render();
        assert!(rendered.contains("RECONCILE FIRST"), "{rendered}");
        assert!(rendered.contains("https://console.firebase.google.com/"));
    }

    #[test]
    fn rejections_timeouts_and_changed_output_are_reported_without_replay() {
        let cases: Vec<(Result<Captured, CaptureError>, &str, &str)> = vec![
            (
                Ok(Captured {
                    code: 1,
                    stdout: String::new(),
                    stderr: "Error: Failed to authenticate, have you run firebase login?".into(),
                }),
                "rejected",
                "REJECTED",
            ),
            (
                Err(CaptureError::TimedOut {
                    after: firebase::DISTRIBUTE_TIMEOUT,
                    partial: Partial {
                        stdout: "i  uploading binary...\n".into(),
                        stderr: String::new(),
                    },
                }),
                "unknown",
                "UNKNOWN remote outcome",
            ),
            (
                Err(CaptureError::Interrupted {
                    partial: Partial::default(),
                    started: true,
                }),
                "unknown",
                "interrupted",
            ),
            (
                Ok(Captured {
                    code: 0,
                    stdout: "all good, trust me\n".into(),
                    stderr: String::new(),
                }),
                "unknown",
                "may have changed",
            ),
        ];
        for (result, state, expect) in cases {
            let mut rig = Rig::new("remote");
            scripted_distribution(&mut rig, result);
            let report = rig.run(&live(&[]));
            assert_eq!(
                status_of(&report, Stage::Distribution),
                Status::Fail,
                "{state}"
            );
            assert!(
                detail_of(&report, Stage::Distribution).contains(expect),
                "{expect}: {}",
                detail_of(&report, Stage::Distribution)
            );
            assert!(rig
                .host
                .persisted
                .last()
                .unwrap()
                .contains(&format!("\"state\": \"{state}\"")));
            assert_eq!(rig.distribute_calls(), 1, "no automatic retry");
            assert!(!report.succeeded());
        }
    }

    #[test]
    fn a_failed_final_receipt_after_remote_success_keeps_a_recovery_pointer() {
        let mut rig = Rig::new("final");
        rig.host.fail_persist_from = Some(3);
        let report = rig.run(&live(&[]));
        assert_eq!(status_of(&report, Stage::Distribution), Status::Pass);
        assert_eq!(status_of(&report, Stage::FinalReceipt), Status::Fail);
        assert!(
            !report.succeeded(),
            "an unrecorded distribution is not a clean success"
        );
        let rendered = report.render();
        assert!(rendered.contains("RECOVERY"), "{rendered}");
        assert!(
            rendered.contains("artifacts/distributions/RUN"),
            "{rendered}"
        );
        assert!(
            rendered.contains("https://console.firebase.google.com/"),
            "{rendered}"
        );
        assert!(rendered.contains("do not rerun"));
        assert_eq!(rig.distribute_calls(), 1);
    }

    #[test]
    fn a_process_that_never_started_is_a_clean_failure_not_an_unknown_write() {
        let mut rig = Rig::new("spawn");
        scripted_distribution(
            &mut rig,
            Err(CaptureError::Spawn {
                program: "firebase".into(),
                message: "permission denied".into(),
            }),
        );
        let report = rig.run(&live(&[]));
        assert!(detail_of(&report, Stage::Distribution).contains("nothing was written"));
        assert!(rig
            .host
            .persisted
            .last()
            .unwrap()
            .contains("\"state\": \"not_started\""));
    }

    #[test]
    fn nothing_after_a_failure_runs_and_every_stage_has_a_row() {
        let mut rig = Rig::new("rows");
        rig.runner.fail_on = Some("assembleDebug".into());
        let report = rig.run(&live(&[]));
        let names: Vec<_> = report.outcomes.iter().map(|o| o.name.clone()).collect();
        let expected: Vec<_> = Stage::ALL.iter().map(|s| s.label().to_owned()).collect();
        assert_eq!(names, expected);
        let statuses: Vec<_> = report.outcomes.iter().map(|o| o.status).collect();
        assert_eq!(
            statuses,
            [
                Status::Pass,
                Status::Pass,
                Status::Pass,
                Status::Pass,
                Status::Fail,
                Status::NotRun,
                Status::NotRun,
                Status::NotRun,
                Status::NotRun,
            ]
        );
    }

    #[test]
    fn the_distribute_argv_uses_the_run_owned_files() {
        let mut rig = Rig::new("argv");
        rig.run(&live(&[TESTER]));
        let command: &CommandSpec = rig.capture.calls.last().unwrap();
        let apk = &command.args[1];
        assert!(
            apk.ends_with("artifacts/distributions/RUN/artifact/androidApp-debug.apk"),
            "{apk}"
        );
        assert!(command
            .args
            .contains(&"--app=1:000000000000:android:0123456789abcdef".to_owned()));
        assert!(command.args.iter().any(
            |a| a.starts_with("--release-notes-file=") && a.ends_with("/RUN/release-notes.txt")
        ));
        assert_eq!(
            command.cwd,
            PathBuf::from("/repo/artifacts/distributions/RUN")
        );
    }
}
