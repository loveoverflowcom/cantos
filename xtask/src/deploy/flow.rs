//! The deploy coordinator. A thin, ordered shell around pure decisions: it checks every local
//! prerequisite and the Hosting configuration, resolves the destination read-only *before*
//! building, builds the one selected app once, validates and fingerprints the exact public bytes,
//! records intent, performs the single remote write, observes the result with read-only requests
//! and records what was seen. No stage after a failure runs; unreached stages are `NOT_RUN`. There
//! is no retry of the write: an uncertain remote outcome is reported, never replayed.
//!
//! Only the selected app is built, validated and uploaded. The coordinator never builds the other
//! web app, Android, the API or any service, never runs migrations or the broad preflight, and
//! never touches `distribute firebase`.

use std::path::PathBuf;

use super::{
    bundle::{self, Finding, Manifest},
    firebase::{
        self, interpret_release, interpret_sites, release_command, sites_command, Expected,
        Invocation, ReleaseKind, RemoteOutcome,
    },
    host::{checked_api_origin, Host},
    hosting_config::{effective_firebaserc, resolve_mapping, Policy, SiteId, PUBLIC_DIR},
    receipt::{BuildRecord, ConfigRecord, Receipt, SourceRecord, ToolRecord},
    request::{Channel, Request, Stage},
    smoke::{self, Observed, Verdict},
};
use crate::{
    build,
    capture::{Capture, CaptureError, Captured},
    command::Runner,
    firebase_cli::{
        parse_version, version_command, version_supported, MINIMUM_VERSION, SUPPORTED_MAJOR,
        TESTED_VERSION, VERSION_TIMEOUT,
    },
    report::{Report, Status},
    run_files::{sha256_hex, RunDir, SourceIdentity},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

/// The effects a run may use. All local, except the Firebase CLI: its `release` invocation is the
/// one remote write, issued exactly once and only after every earlier stage passed; the lookup and
/// the smoke requests are read-only.
pub struct Ports<'a> {
    pub tools: &'a dyn ToolProbe,
    pub runner: &'a mut dyn Runner,
    pub capture: &'a mut dyn Capture,
    pub host: &'a mut dyn Host,
}

#[cfg(test)]
pub fn run<'a>(workspace: &'a Workspace, request: &'a Request, ports: Ports<'a>) -> Report {
    run_with_artifact(workspace, request, ports, None)
}

pub fn run_with_artifact<'a>(
    workspace: &'a Workspace,
    request: &'a Request,
    ports: Ports<'a>,
    artifact: Option<&'a crate::release::ArtifactArgs>,
) -> Report {
    let mut flow = Flow {
        artifact,
        workspace,
        request,
        ports,
        rows: Vec::new(),
        notes: Vec::new(),
        receipt: None,
        run: None,
        prepared: None,
        firebase_version: None,
        source: None,
        default_url: None,
        manifest: None,
        remote: None,
    };
    let _halted = flow.go();
    flow.finish()
}

/// A recorded failure; the stage row already carries the reason.
struct Halt;

/// What the Hosting configuration resolved to, held until the run directory exists.
struct Prepared {
    api_origin: String,
    site: SiteId,
    policy_sha256: String,
    firebase_json: String,
    firebaserc: String,
}

struct Flow<'a> {
    artifact: Option<&'a crate::release::ArtifactArgs>,
    workspace: &'a Workspace,
    request: &'a Request,
    ports: Ports<'a>,
    rows: Vec<(Stage, Status, String)>,
    notes: Vec<String>,
    receipt: Option<Receipt>,
    run: Option<RunDir>,
    prepared: Option<Prepared>,
    firebase_version: Option<String>,
    source: Option<SourceIdentity>,
    default_url: Option<String>,
    manifest: Option<Manifest>,
    remote: Option<RemoteOutcome>,
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
        let config = self.hosting_config();
        let cli = self.firebase_cli();
        local?;
        config?;
        cli?;
        self.open_run()?;
        self.destination()?;
        self.build()?;
        self.bundle()?;
        self.intent()?;
        self.release();
        self.smoke();
        Ok(())
    }

    fn prerequisites(&mut self) -> Result<(), Halt> {
        let mut problems = Vec::new();
        if !cfg!(unix) {
            problems.push("captured execution and private run files need a POSIX host".to_owned());
        }
        let needed: &[Tool] = if self.artifact.is_some() {
            &[Tool::Python3, Tool::Git]
        } else {
            &[Tool::Cargo, Tool::Wasm32Target, Tool::Trunk, Tool::Git]
        };
        let missing = self.ports.tools.missing(needed);
        if !missing.is_empty() {
            let details: Vec<_> = missing
                .iter()
                .map(|tool| format!("{} ({})", tool.label(), tool.hint()))
                .collect();
            problems.push(format!("missing {}", details.join(", ")));
        }
        let overrides = self.ports.host.trunk_overrides();
        if self.artifact.is_none() && !overrides.is_empty() {
            problems.push(format!(
                "inherited Trunk environment variables could redirect the build ({}); unset them for this command",
                overrides.join(", ")
            ));
        }
        if problems.is_empty() {
            self.pass(
                Stage::Prerequisites,
                if self.artifact.is_some() { "Python and Git are installed for artifact validation; no product build" } else { "Rust, the wasm32 target, Trunk and Git are installed; no TRUNK_* override is set" },
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

    fn hosting_config(&mut self) -> Result<(), Halt> {
        let configured_origin = self
            .ports
            .host
            .api_origin()
            .map_err(|why| self.halt(Stage::HostingConfig, Status::Fail, why))?;
        let api_origin = checked_api_origin(configured_origin.as_deref())
            .map_err(|why| self.halt(Stage::HostingConfig, Status::Fail, why))?;
        let files = match self.ports.host.read_config() {
            Ok(files) => files,
            Err(why) => return Err(self.halt(Stage::HostingConfig, Status::Fail, why)),
        };
        let policy = match Policy::parse(&files.policy) {
            Ok(policy) => policy,
            Err(error) => {
                return Err(self.halt(Stage::HostingConfig, Status::Fail, error.to_string()))
            }
        };
        let target = self.request.target();
        let site = match resolve_mapping(files.firebaserc.as_deref(), &self.request.project, target)
        {
            Ok(site) => site,
            Err(error) => {
                return Err(self.halt(Stage::HostingConfig, Status::Fail, error.to_string()))
            }
        };
        let firebase_json = policy
            .effective_firebase_json(target)
            .expect("a validated policy holds both known targets");
        let firebaserc = effective_firebaserc(&self.request.project, target, &site);
        let detail = format!(
            "target {target} -> site {} in project {} (from .firebaserc); policy accepted",
            site.as_str(),
            self.request.project.as_str()
        );
        self.prepared = Some(Prepared {
            api_origin,
            site,
            policy_sha256: sha256_hex(files.policy.as_bytes()),
            firebase_json,
            firebaserc,
        });
        self.pass(Stage::HostingConfig, detail);
        Ok(())
    }

    fn firebase_cli(&mut self) -> Result<(), Halt> {
        let command = version_command(self.workspace.root());
        let reason = match self.ports.capture.capture(&command, VERSION_TIMEOUT) {
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
        let (cargo, trunk) = self.ports.host.tool_versions();
        receipt.tools = ToolRecord {
            cargo,
            trunk,
            firebase_cli: self.firebase_version.clone(),
        };
        let prepared = self
            .prepared
            .as_ref()
            .expect("the configuration resolved before the run record");
        receipt.site = Some(prepared.site.as_str().to_owned());
        receipt.config = Some(ConfigRecord {
            policy_sha256: prepared.policy_sha256.clone(),
            effective_firebase_json_sha256: sha256_hex(prepared.firebase_json.as_bytes()),
            effective_firebaserc_sha256: sha256_hex(prepared.firebaserc.as_bytes()),
        });
        // Earlier stage rows belong in the record too.
        for (stage, status, detail) in &self.rows {
            receipt.record_stage(stage.label(), *status, detail, &now);
        }
        let identity = self.ports.host.source_identity();
        self.run = Some(run.clone());
        match identity {
            Ok(identity) => {
                receipt.source = Some(SourceRecord {
                    commit: identity.commit.clone(),
                    dirty: identity.dirty,
                    content_fingerprint: identity.content_fingerprint.clone(),
                });
                self.source = Some(identity);
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
        let (firebase_json, firebaserc) = {
            let prepared = self.prepared.as_ref().expect("resolved above");
            (prepared.firebase_json.clone(), prepared.firebaserc.clone())
        };
        for (name, text) in [
            ("firebase.json", &firebase_json),
            (".firebaserc", &firebaserc),
        ] {
            if let Err(why) = self.ports.host.write_private(&run, name, text.as_bytes()) {
                return Err(self.halt(
                    Stage::RunRecord,
                    Status::Fail,
                    format!("cannot write the effective Hosting configuration: {why}"),
                ));
            }
        }
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
            format!(
                "{relative} (receipt.json plus an effective firebase.json and .firebaserc for the {} target only; private)",
                self.request.target()
            ),
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

    fn dist(&self) -> PathBuf {
        self.run_dir().join(PUBLIC_DIR)
    }

    // ACCEPTANCE(NOT_RUN): the site lookup has never run against a real Firebase project, so the
    // `hosting:sites:list` response shape is a fixture reconstructed from the CLI sources and a
    // successful lookup is not proof of deploy permission (`firebasehosting.sites.update`).
    // Owner: the operator of the first live preview deploy.
    fn destination(&mut self) -> Result<(), Halt> {
        println!("\n==> {}", Stage::Destination.label());
        let run_dir = self.run_dir();
        let config = run_dir.join("firebase.json");
        let inv = Invocation {
            project: &self.request.project,
            config: &config,
            cwd: &run_dir,
        };
        let command = sites_command(&inv);
        println!("    $ {}", command.display(self.workspace.root()));
        let result = self
            .ports
            .capture
            .capture(&command, firebase::LOOKUP_TIMEOUT);
        let site = self
            .prepared
            .as_ref()
            .expect("resolved before the destination")
            .site
            .clone();
        match interpret_sites(&site, result) {
            Ok(destination) => {
                self.default_url = destination.default_url;
                self.pass(
                    Stage::Destination,
                    format!(
                        "site {} exists in project {} (a successful lookup is not proof of deploy permission)",
                        site.as_str(),
                        self.request.project.as_str()
                    ),
                );
                Ok(())
            }
            Err(error) => Err(self.halt(Stage::Destination, Status::Fail, error.to_string())),
        }
    }

    fn build(&mut self) -> Result<(), Halt> {
        println!("\n==> {}", Stage::Build.label());
        let dist = self.dist();
        let api_origin = self
            .prepared
            .as_ref()
            .map(|prepared| prepared.api_origin.clone())
            .ok_or_else(|| {
                self.halt(
                    Stage::Build,
                    Status::Fail,
                    "Hosting configuration was not prepared",
                )
            })?;
        let command = match self.artifact {
            Some(artifact) => artifact.extract_command(
                self.workspace,
                &format!("{}-web", self.request.target()),
                &api_origin,
                &dist,
            ),
            None => build::release_bundle(self.workspace, self.request.app, &dist)
                .env("CANTOS_API_ORIGIN", &api_origin),
        };
        if let Some(receipt) = &mut self.receipt {
            receipt.build = Some(BuildRecord {
                api_origin: Some(api_origin.clone()),
                command: command.display(self.workspace.root()),
                dist: self.workspace.relative(&dist),
            });
        }
        if let Err(error) = self.ports.runner.run(&command) {
            return Err(self.halt(Stage::Build, Status::Fail, format!("{error:#}")));
        }
        // The bytes just built are only meaningful for the source that was recorded.
        match self.ports.host.source_identity() {
            Ok(after) if Some(&after) == self.source.as_ref() => {}
            Ok(_) => {
                return Err(self.halt(
                    Stage::Build,
                    Status::Fail,
                    "the source tree changed while the build ran (commit, dirty state or content); nothing was uploaded, rebuild from a stable tree",
                ))
            }
            Err(why) => {
                return Err(self.halt(
                    Stage::Build,
                    Status::Fail,
                    format!("could not re-read the source identity after the build: {why}"),
                ))
            }
        }
        self.pass(
            Stage::Build,
            format!(
                "{} release bytes prepared; source unchanged during preparation",
                self.request.app.cli_name()
            ),
        );
        Ok(())
    }

    fn bundle(&mut self) -> Result<(), Halt> {
        let dist = self.dist();
        let scan = match self.ports.host.scan_bundle(&dist) {
            Ok(scan) => scan,
            Err(why) => return Err(self.halt(Stage::Bundle, Status::Fail, why)),
        };
        let manifest = match bundle::validate(self.request.app, &scan) {
            Ok(manifest) => manifest,
            Err(findings) => {
                return Err(self.halt(Stage::Bundle, Status::Fail, summarize(&findings)))
            }
        };
        if let Some(receipt) = &mut self.receipt {
            receipt.set_bundle(&manifest);
        }
        let detail = format!(
            "{} files, {} bytes, manifest {}; loads {}",
            manifest.files.len(),
            manifest.total_bytes,
            short(&manifest.digest),
            manifest.entry_assets.join(", ")
        );
        self.manifest = Some(manifest);
        self.pass(Stage::Bundle, detail);
        Ok(())
    }

    /// Everything the remote write needs is validated; the bytes must still be the validated
    /// bytes, and intent is recorded before the write.
    fn intent(&mut self) -> Result<(), Halt> {
        let fail =
            |flow: &mut Self, why: String| flow.halt(Stage::IntentReceipt, Status::Fail, why);
        let dist = self.dist();
        let expected = self
            .manifest
            .as_ref()
            .expect("the bundle was validated")
            .digest
            .clone();
        let again = self
            .ports
            .host
            .scan_bundle(&dist)
            .map_err(|why| why.to_string())
            .and_then(|scan| {
                bundle::validate(self.request.app, &scan).map_err(|findings| summarize(&findings))
            });
        match again {
            Ok(manifest) if manifest.digest == expected => {}
            Ok(_) => {
                return Err(fail(
                    self,
                    "the bundle changed after validation; nothing was uploaded".to_owned(),
                ))
            }
            Err(why) => {
                return Err(fail(
                    self,
                    format!("the bundle no longer validates, so nothing was uploaded: {why}"),
                ))
            }
        }
        if let Some(receipt) = &mut self.receipt {
            receipt.mark_intent();
        }
        self.record(
            Stage::IntentReceipt,
            Status::Pass,
            "bundle unchanged since validation; intent recorded before the remote write",
        );
        if let Err(why) = self.persist() {
            // The row just pushed would claim success; replace it with the failure.
            self.rows.pop();
            if let Some(receipt) = &mut self.receipt {
                receipt.stages.pop();
                receipt.unmark_intent();
            }
            return Err(fail(
                self,
                format!("cannot record intent, so nothing was uploaded: {why}"),
            ));
        }
        Ok(())
    }

    // ACCEPTANCE(NOT_RUN): this stage has never executed against a real Firebase project. Both
    // invocations (`hosting:channel:deploy` and `deploy`), the CLI's real result shapes, that the
    // unselected site is left unchanged, and the "unknown" paths (timeout, interruption) are
    // verified only with fixtures and fakes. Prerequisites: an operator-owned project with two
    // Hosting sites and an explicit go-ahead naming project, channel and app. Next command:
    // `cargo xtask deploy firebase <app> --web --project <id> --channel <preview>` once per app, then
    // `--channel live` for one. Acceptance: `PASS` on this row, a `confirmed` receipt, captured
    // output replacing the reconstructed fixtures, and the other site's URL unchanged.
    // Owner: the operator who owns the Firebase project.
    fn release(&mut self) {
        let run_dir = self.run_dir();
        let config = run_dir.join("firebase.json");
        let site = self
            .prepared
            .as_ref()
            .expect("resolved before the release")
            .site
            .clone();
        let target = self.request.target();
        let channel = self.request.channel.clone();
        let message = self.live_message();
        let inv = Invocation {
            project: &self.request.project,
            config: &config,
            cwd: &run_dir,
        };
        let kind = match &channel {
            Channel::Preview(preview) => ReleaseKind::Preview {
                channel: preview.as_str(),
                target,
                expires: self.request.expires.as_ref().map(|expiry| expiry.as_arg()),
            },
            Channel::Live => ReleaseKind::Live {
                target,
                message: &message,
            },
        };
        let command = release_command(&inv, kind);
        println!("\n==> {} (REMOTE WRITE)", Stage::Release.label());
        println!("    $ {}", command.display(self.workspace.root()));
        let result = self
            .ports
            .capture
            .capture(&command, firebase::RELEASE_TIMEOUT);
        let diagnostics = diagnostics(&result);
        let expected = Expected {
            target,
            site: &site,
            channel: channel.label(),
            default_url: self.default_url.as_deref(),
        };
        let outcome = interpret_release(result, firebase::RELEASE_TIMEOUT, &expected);
        if let Some(receipt) = &mut self.receipt {
            receipt.set_outcome(&outcome, diagnostics);
        }
        let (status, detail) = describe(&outcome);
        self.record(Stage::Release, status, detail);
        self.remote = Some(outcome);
        // Make the observed identity durable before the (long) smoke; a failure here is reported
        // by `finish`, which persists again and prints the recovery pointer.
        let _ = self.persist();
    }

    /// The release message attached to a live deploy, so the console's release history names the
    /// app, the source commit and the run whose receipt describes it. No secret or address.
    fn live_message(&self) -> String {
        let commit: String = self
            .source
            .as_ref()
            .map(|source| source.commit.chars().take(12).collect())
            .unwrap_or_default();
        let dirty = if self.source.as_ref().is_some_and(|source| source.dirty) {
            "+dirty"
        } else {
            ""
        };
        let run = self.run.as_ref().map(|run| run.id.as_str()).unwrap_or("");
        format!(
            "cantos {} {commit}{dirty} {run}",
            self.request.app.cli_name()
        )
    }

    /// Read-only HTTPS requests to the URL the CLI reported. A separate result from the release:
    /// a failure here never changes what is known about the release.
    fn smoke(&mut self) {
        let Some(release) = self
            .remote
            .as_ref()
            .and_then(RemoteOutcome::release)
            .cloned()
        else {
            return;
        };
        let Some(url) = release.url.clone() else {
            self.record(
                Stage::Smoke,
                Status::NotRun,
                "the release did not name a URL to request",
            );
            return;
        };
        println!("\n==> {}", Stage::Smoke.label());
        let (Some(manifest), Some(run)) = (self.manifest.clone(), self.run.clone()) else {
            return;
        };
        let probes = smoke::probes(&manifest, &run.id);
        let mut verdict = Verdict::unavailable();
        for attempt in 1..=smoke::ATTEMPTS {
            verdict = self.smoke_attempt(&url, &run, &probes, attempt);
            if verdict.tool_missing || verdict.passed() || attempt == smoke::ATTEMPTS {
                break;
            }
            self.ports.host.pause(smoke::PAUSE);
        }
        if let Some(receipt) = &mut self.receipt {
            receipt.set_smoke(&url, &verdict);
        }
        match verdict.state() {
            "pass" => self.pass(
                Stage::Smoke,
                format!(
                    "{} requests to {url} answered as built ({} attempt(s)); static delivery only",
                    verdict.probes.len(),
                    verdict.attempts
                ),
            ),
            "unavailable" => self.record(
                Stage::Smoke,
                Status::NotRun,
                "curl is not installed here, so the served bytes were not observed; request the URL yourself",
            ),
            _ => {
                let problems = verdict.problems();
                let shown: Vec<_> = problems.iter().take(4).cloned().collect();
                let more = problems.len().saturating_sub(shown.len());
                let tail = if more > 0 {
                    format!("; +{more} more in receipt.json")
                } else {
                    String::new()
                };
                self.record(
                    Stage::Smoke,
                    Status::Fail,
                    format!(
                        "the release is confirmed but its URL did not serve the bundle as built: {}{tail}",
                        shown.join("; ")
                    ),
                );
            }
        }
    }

    fn smoke_attempt(
        &mut self,
        base: &str,
        run: &RunDir,
        probes: &[smoke::Probe],
        attempt: u32,
    ) -> Verdict {
        let mut records = Vec::new();
        let base = base.trim_end_matches('/');
        for (index, probe) in probes.iter().enumerate() {
            let Some(url) = smoke::url_for(base, &probe.path) else {
                records.push(smoke::judge(
                    probe,
                    Err("the observed URL is not a plain https origin".to_owned()),
                ));
                continue;
            };
            let body = run.path.join("smoke").join(format!("{index}.body"));
            let command = smoke::curl_command(&url, &body, &run.path);
            let observed = match self.ports.capture.capture(&command, smoke::TIMEOUT) {
                Err(CaptureError::Spawn { .. }) => return Verdict::unavailable(),
                Err(error) => Err(firebase::redact(&error.to_string())),
                Ok(captured) if captured.code != 0 => {
                    Err(format!("curl exited with {}", captured.code))
                }
                Ok(captured) => match smoke::parse_headers(&captured.stdout) {
                    None => Err("no HTTP response headers".to_owned()),
                    Some(response) => Ok(Observed {
                        response,
                        body_sha256: self.ports.host.hash_file(&body).ok().map(|(hash, _)| hash),
                    }),
                },
            };
            records.push(smoke::judge(probe, observed));
        }
        Verdict {
            attempts: attempt,
            probes: records,
            tool_missing: false,
        }
    }

    fn finish(mut self) -> Report {
        let attempted = self.remote.is_some();
        // Record what was never reached before the receipt is finalized.
        for stage in Stage::ALL {
            if stage != Stage::FinalReceipt && !self.rows.iter().any(|(s, ..)| *s == stage) {
                self.record(
                    stage,
                    Status::NotRun,
                    if stage == Stage::Smoke && attempted {
                        "no release was confirmed, so there is nothing to request"
                    } else {
                        "not started: an earlier stage did not pass"
                    },
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

        let mut report = Report::new("deploy firebase");
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
            for line in self.outcome_notes(outcome) {
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

    fn outcome_notes(&self, outcome: &RemoteOutcome) -> Vec<String> {
        let mut notes = Vec::new();
        if let Some(release) = outcome.release() {
            if let Some(url) = &release.url {
                notes.push(format!("URL: {url}"));
            }
            if let Some(version) = &release.version {
                notes.push(format!(
                    "Hosting version: {version} (site {}); keep it to find this release in the console",
                    release.site
                ));
            }
            if let Some(expires) = &release.expires_at {
                notes.push(format!("preview channel expires {expires}"));
            }
        }
        match outcome {
            RemoteOutcome::Confirmed { release } => {
                if release.channel != "live" {
                    notes.push(
                        "a preview URL is PUBLIC (not tester-gated like App Distribution) and is not isolated from any backend; never put private Studio content on it"
                            .to_owned(),
                    );
                }
                // ACCEPTANCE(NOT_RUN): see the API-origin gate in docs/deployment/firebase-hosting.md.
                notes.push(
                    "static delivery only: clients use the API origin recorded in the build receipt; Hosting serves no API, and sign-in, catalog, library and playback require separate runtime acceptance"
                        .to_owned(),
                );
            }
            RemoteOutcome::Unknown { .. } => {
                notes.push(format!("RECONCILE FIRST: {}", super::receipt::RECONCILE));
            }
            RemoteOutcome::Rejected { .. } => {
                notes.push(
                    "no release was reported; a preview channel or an unreleased version may remain, which fixing the cause and running again reuses"
                        .to_owned(),
                );
            }
            RemoteOutcome::NotStarted(_) => {}
        }
        notes
    }
}

fn short(hex: &str) -> String {
    format!("{}…", hex.chars().take(12).collect::<String>())
}

/// The first findings, bounded, so one bad bundle does not print thousands of lines.
fn summarize(findings: &[Finding]) -> String {
    const SHOWN: usize = 6;
    let shown: Vec<String> = findings
        .iter()
        .take(SHOWN)
        .map(ToString::to_string)
        .collect();
    let more = findings.len().saturating_sub(SHOWN);
    let tail = if more > 0 {
        format!("; +{more} more")
    } else {
        String::new()
    };
    format!("{}{tail}", shown.join("; "))
}

/// Redacted tail of what the CLI printed, for the receipt.
fn diagnostics(result: &Result<Captured, CaptureError>) -> Vec<String> {
    let text = match result {
        Ok(captured) => format!("{}\n{}", captured.stdout, captured.stderr),
        Err(error) => error
            .partial()
            .map(|p| format!("{}\n{}", p.stdout, p.stderr))
            .unwrap_or_default(),
    };
    let lines: Vec<String> = firebase::redact(&text)
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
            match (&release.version, release.channel.as_str()) {
                (Some(version), "live") => format!(
                    "LIVE release of site {} accepted (version {version})",
                    release.site
                ),
                (None, "live") => format!("LIVE release of site {} accepted", release.site),
                (Some(version), channel) => format!(
                    "preview channel `{channel}` of site {} accepted (version {version})",
                    release.site
                ),
                (None, channel) => {
                    format!(
                        "preview channel `{channel}` of site {} accepted",
                        release.site
                    )
                }
            },
        ),
        RemoteOutcome::Rejected { kind, detail } => (
            Status::Fail,
            format!("REFUSED, no release was reported: {kind} ({detail})"),
        ),
        RemoteOutcome::Unknown { reason } => {
            (Status::Fail, format!("UNKNOWN remote outcome: {reason}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        build::Surface,
        capture::Partial,
        command::CommandSpec,
        deploy::{
            bundle::testing::{file, good_scan},
            bundle::Scan,
            host::ConfigFiles,
            request::{HostedApp, Raw},
            smoke::{Expect, Family},
        },
        tools::testing::FakeTools,
    };
    use serde_json::Value;
    use std::{cell::RefCell, collections::VecDeque, path::Path, rc::Rc, time::Duration};

    const POLICY: &str = include_str!("../../../infra/firebase/hosting/firebase.json");
    const RC: &str = r#"{"targets":{"cantos-example":{"hosting":{"theatre":["example-theatre"],"studio":["example-studio"]}}}}"#;
    const SITES: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-sites-list.json");
    const PREVIEW: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-preview-success.json");
    const LIVE: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-live-success.json");
    const E_AUTH: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-error-auth.json");
    const E_PERMISSION: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-error-permission.json");
    const E_UPLOAD: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-error-upload-failed.json");
    const CHANGED: &str =
        include_str!("../../fixtures/firebase-tools-15.34.0/hosting-preview-changed-shape.json");
    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    type Log = Rc<RefCell<Vec<String>>>;

    struct LogCapture {
        log: Log,
        results: VecDeque<Result<Captured, CaptureError>>,
        calls: Vec<CommandSpec>,
    }

    impl LogCapture {
        fn ok(&mut self, code: i32, stdout: &str, stderr: &str) -> &mut Self {
            self.results.push_back(Ok(Captured {
                code,
                stdout: stdout.to_owned(),
                stderr: stderr.to_owned(),
            }));
            self
        }
    }

    impl Capture for LogCapture {
        fn capture(
            &mut self,
            spec: &CommandSpec,
            _timeout: Duration,
        ) -> Result<Captured, CaptureError> {
            self.log.borrow_mut().push(format!(
                "capture:{} {}",
                spec.program,
                spec.args.first().cloned().unwrap_or_default()
            ));
            self.calls.push(spec.clone());
            self.results
                .pop_front()
                .expect("a scripted capture result for every invocation")
        }
    }

    struct LogRunner {
        log: Log,
        commands: Vec<CommandSpec>,
        fail: bool,
    }

    impl Runner for LogRunner {
        fn run(&mut self, spec: &CommandSpec) -> anyhow::Result<()> {
            self.log.borrow_mut().push("build".to_owned());
            self.commands.push(spec.clone());
            if self.fail {
                anyhow::bail!("simulated trunk failure");
            }
            Ok(())
        }
    }

    struct FakeHost {
        log: Log,
        scanned: Vec<PathBuf>,
        config: Result<ConfigFiles, String>,
        overrides: Vec<String>,
        api_origin: Option<String>,
        identities: VecDeque<Result<SourceIdentity, String>>,
        scans: VecDeque<Result<Scan, String>>,
        hashes: VecDeque<Result<(String, u64), String>>,
        fail_create_run: bool,
        fail_write_of: Option<&'static str>,
        fail_persist_from: Option<usize>,
        fail_persist_only: Option<usize>,
        persisted: Vec<Value>,
        persist_calls: usize,
        files: Vec<(String, String)>,
        runs_created: usize,
        pauses: Vec<Duration>,
    }

    fn identity(dirty: bool) -> SourceIdentity {
        SourceIdentity {
            commit: COMMIT.into(),
            dirty,
            content_fingerprint: dirty.then(|| "f".repeat(64)),
        }
    }

    impl FakeHost {
        fn new(log: Log, app: crate::config::App) -> Self {
            Self {
                log,
                scanned: Vec::new(),
                config: Ok(ConfigFiles {
                    policy: POLICY.to_owned(),
                    firebaserc: Some(RC.to_owned()),
                }),
                overrides: Vec::new(),
                api_origin: Some("https://api.example.org".into()),
                identities: [Ok(identity(false)), Ok(identity(false))].into(),
                scans: [Ok(good_scan(app)), Ok(good_scan(app))].into(),
                hashes: VecDeque::new(),
                fail_create_run: false,
                fail_write_of: None,
                fail_persist_from: None,
                fail_persist_only: None,
                persisted: Vec::new(),
                persist_calls: 0,
                files: Vec::new(),
                runs_created: 0,
                pauses: Vec::new(),
            }
        }
    }

    impl Host for FakeHost {
        fn now(&mut self) -> String {
            "2026-10-16T00:00:00Z".into()
        }
        fn source_identity(&mut self) -> Result<SourceIdentity, String> {
            self.identities
                .pop_front()
                .expect("a scripted source identity")
        }
        fn trunk_overrides(&self) -> Vec<String> {
            self.overrides.clone()
        }
        fn api_origin(&self) -> Result<Option<String>, String> {
            Ok(self.api_origin.clone())
        }
        fn read_config(&mut self) -> Result<ConfigFiles, String> {
            self.config.clone()
        }
        fn tool_versions(&mut self) -> (Option<String>, Option<String>) {
            (Some("cargo 1.98.1".into()), Some("trunk 0.21.14".into()))
        }
        fn create_run(&mut self) -> Result<RunDir, String> {
            if self.fail_create_run {
                return Err("read-only file system".into());
            }
            self.runs_created += 1;
            self.log.borrow_mut().push("create_run".to_owned());
            Ok(RunDir {
                id: "RUN".into(),
                path: PathBuf::from("/repo/artifacts/hosting/RUN"),
            })
        }
        fn write_private(
            &mut self,
            run: &RunDir,
            name: &str,
            bytes: &[u8],
        ) -> Result<PathBuf, String> {
            if self.fail_write_of == Some(name) {
                return Err("disk full".into());
            }
            self.files
                .push((name.to_owned(), String::from_utf8(bytes.to_vec()).unwrap()));
            Ok(run.path.join(name))
        }
        fn scan_bundle(&mut self, directory: &Path) -> Result<Scan, String> {
            self.scanned.push(directory.to_path_buf());
            self.log.borrow_mut().push("scan".to_owned());
            self.scans.pop_front().expect("a scripted bundle scan")
        }
        fn hash_file(&mut self, _: &Path) -> Result<(String, u64), String> {
            self.hashes
                .pop_front()
                .unwrap_or_else(|| Err("no body".into()))
        }
        fn persist(&mut self, run: &RunDir, receipt: &Receipt) -> Result<PathBuf, String> {
            self.persist_calls += 1;
            if self
                .fail_persist_from
                .is_some_and(|from| self.persist_calls >= from)
                || self.fail_persist_only == Some(self.persist_calls)
            {
                return Err("disk full".into());
            }
            self.log
                .borrow_mut()
                .push(format!("persist:{}", receipt.phase));
            self.persisted
                .push(serde_json::from_slice(&receipt.to_json()).unwrap());
            Ok(run.path.join("receipt.json"))
        }
        fn pause(&mut self, duration: Duration) {
            self.pauses.push(duration);
        }
    }

    struct Rig {
        log: Log,
        host: FakeHost,
        runner: LogRunner,
        capture: LogCapture,
        tools: FakeTools,
        app: crate::config::App,
    }

    fn raw(app: HostedApp, channel: &str) -> Raw {
        Raw {
            app,
            surface: Some(Surface::Web),
            project: "cantos-example".into(),
            channel: channel.into(),
            expires: None,
            dry_run: false,
        }
    }

    fn request_of(app: HostedApp, channel: &str) -> Request {
        super::super::request::resolve(&raw(app, channel)).unwrap()
    }

    fn preview_for(app: HostedApp) -> String {
        match app {
            HostedApp::CantosTheatre => PREVIEW.to_owned(),
            HostedApp::CantosStudio => PREVIEW.replace("theatre", "studio"),
        }
    }

    fn media_type(family: Family) -> &'static str {
        match family {
            Family::Script => "text/javascript",
            Family::Style => "text/css",
            Family::Wasm => "application/wasm",
        }
    }

    impl Rig {
        /// A rig scripted for a complete preview run of `app` with a healthy smoke.
        fn preview(app: HostedApp) -> Self {
            let mut rig = Self::bare(app);
            rig.capture
                .ok(0, "15.34.0\n", "")
                .ok(0, SITES, "")
                .ok(0, &preview_for(app), "");
            rig.good_smoke_round();
            rig
        }

        fn bare(app: HostedApp) -> Self {
            let log: Log = Rc::default();
            Self {
                host: FakeHost::new(Rc::clone(&log), app.app()),
                runner: LogRunner {
                    log: Rc::clone(&log),
                    commands: Vec::new(),
                    fail: false,
                },
                capture: LogCapture {
                    log: Rc::clone(&log),
                    results: VecDeque::new(),
                    calls: Vec::new(),
                },
                tools: FakeTools::all(),
                app: app.app(),
                log,
            }
        }

        fn probes(&self) -> Vec<smoke::Probe> {
            let manifest = bundle::validate(self.app, &good_scan(self.app)).unwrap();
            smoke::probes(&manifest, "RUN")
        }

        fn good_smoke_round(&mut self) {
            for probe in self.probes() {
                let (headers, sha) = match &probe.expect {
                    Expect::Page { sha256 } => (
                        "HTTP/2 200\r\ncontent-type: text/html; charset=utf-8\r\ncache-control: no-cache\r\n\r\n"
                            .to_owned(),
                        sha256.clone(),
                    ),
                    Expect::Asset { family, sha256 } => (
                        format!(
                            "HTTP/2 200\r\ncontent-type: {}\r\ncache-control: no-cache\r\n\r\n",
                            media_type(*family)
                        ),
                        sha256.clone(),
                    ),
                    Expect::Absent => (
                        "HTTP/2 404\r\ncontent-type: text/html\r\n\r\n".to_owned(),
                        "not-compared".to_owned(),
                    ),
                };
                self.capture.ok(0, &headers, "");
                self.host.hashes.push_back(Ok((sha, 1)));
            }
        }

        /// A round in which Hosting has not propagated the release yet: everything is a 404.
        fn stale_smoke_round(&mut self) {
            for _ in self.probes() {
                self.capture
                    .ok(0, "HTTP/2 404\r\ncontent-type: text/html\r\n\r\n", "");
                self.host.hashes.push_back(Ok(("x".into(), 1)));
            }
        }

        fn run(&mut self, request: &Request) -> Report {
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

        fn calls_of(&self, subcommand: &str) -> Vec<&CommandSpec> {
            self.capture
                .calls
                .iter()
                .filter(|c| c.args.first().is_some_and(|a| a == subcommand))
                .collect()
        }

        fn release_calls(&self) -> usize {
            self.calls_of("hosting:channel:deploy").len() + self.calls_of("deploy").len()
        }

        fn events(&self) -> Vec<String> {
            self.log.borrow().clone()
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

    fn assert_rows(report: &Report, expected: &[(Stage, Status)]) {
        for (stage, status) in expected {
            assert_eq!(
                status_of(report, *stage),
                *status,
                "{}: {}",
                stage.label(),
                report.render()
            );
        }
    }

    fn after(report: &Report, stage: Stage) -> Vec<Stage> {
        let position = Stage::ALL.iter().position(|s| *s == stage).unwrap();
        Stage::ALL[position + 1..]
            .to_vec()
            .into_iter()
            .filter(|s| status_of(report, *s) != Status::NotRun)
            .collect()
    }

    fn last_receipt(rig: &Rig) -> &Value {
        rig.host.persisted.last().expect("a persisted receipt")
    }

    #[test]
    fn a_complete_preview_run_builds_once_validates_then_writes_remotely_exactly_once() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert!(report.succeeded(), "{}", report.render());
        assert!(report.outcomes.iter().all(|o| o.status == Status::Pass));
        assert_eq!(report.outcomes.len(), Stage::ALL.len());
        // Version, lookup, one release, then the smoke requests: probes' count.
        let mut expected = vec![
            "firebase --version".to_owned(),
            "firebase hosting:sites:list".to_owned(),
            "firebase hosting:channel:deploy".to_owned(),
        ];
        expected.extend(rig.probes().iter().map(|_| "curl --disable".to_owned()));
        assert_eq!(rig.programs(), expected);
        assert_eq!(rig.runner.commands.len(), 1, "one build");
        assert_eq!(rig.release_calls(), 1, "one remote write");
        // The order that makes the write safe: record, look up, build, validate, record intent, write.
        let events = rig.events();
        let position = |needle: &str| events.iter().position(|e| e == needle).unwrap();
        assert!(position("persist:started") < position("capture:firebase hosting:sites:list"));
        assert!(position("capture:firebase hosting:sites:list") < position("build"));
        assert!(position("build") < position("persist:intent"));
        assert!(position("persist:intent") < position("capture:firebase hosting:channel:deploy"));
        // Receipts: started, intent (before the write), complete.
        let phases: Vec<_> = rig
            .host
            .persisted
            .iter()
            .map(|r| r["phase"].as_str().unwrap().to_owned())
            .collect();
        // The outcome is persisted right after the write, before the smoke, and again at the end.
        assert_eq!(phases, ["started", "intent", "complete", "complete"]);
    }

    #[test]
    fn artifact_mode_reuses_the_release_flow_without_trunk_wasm_or_product_compilation() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        rig.tools.0.remove(&Tool::Trunk);
        rig.tools.0.remove(&Tool::Wasm32Target);
        let request = request_of(HostedApp::CantosTheatre, "review-hosting");
        let artifact = crate::release::ArtifactArgs {
            release_manifest: Some("/verified/manifest.json".into()),
            release_digest: Some("d".repeat(64)),
            release_sha: Some(COMMIT.into()),
            local_artifact: true,
            ..Default::default()
        };
        let report = run_with_artifact(
            &Workspace::discover().unwrap(),
            &request,
            Ports {
                tools: &rig.tools,
                runner: &mut rig.runner,
                capture: &mut rig.capture,
                host: &mut rig.host,
            },
            Some(&artifact),
        );
        assert_eq!(report.exit_code(), std::process::ExitCode::SUCCESS);
        assert_eq!(rig.runner.commands.len(), 1);
        let command = &rig.runner.commands[0];
        assert_eq!(command.program, "python3");
        assert!(command.args.iter().any(|arg| arg == "extract"));
        assert!(command.args.iter().any(|arg| arg == "--release-digest"));
        assert!(!command
            .args
            .iter()
            .any(|arg| arg == "build" || arg == "trunk"));
        assert_eq!(rig.release_calls(), 1);
    }

    #[test]
    fn the_receipt_records_identity_destination_bundle_release_and_smoke_without_secrets() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert!(report.succeeded(), "{}", report.render());
        let receipt = last_receipt(&rig);
        assert_eq!(receipt["schema_version"], 1);
        assert_eq!(receipt["app"], "cantos-theatre");
        assert_eq!(receipt["target"], "theatre");
        assert_eq!(receipt["project"], "cantos-example");
        assert_eq!(receipt["site"], "example-theatre");
        assert_eq!(receipt["channel"], "review-hosting");
        assert_eq!(receipt["source"]["commit"], COMMIT);
        assert_eq!(receipt["source"]["dirty"], false);
        assert_eq!(receipt["tools"]["firebase_cli"], "15.34.0");
        assert_eq!(receipt["tools"]["trunk"], "trunk 0.21.14");
        assert_eq!(receipt["remote"]["state"], "confirmed");
        assert_eq!(receipt["remote"]["release"]["version"], "0123456789abcdef");
        assert_eq!(
            receipt["remote"]["release"]["url"],
            "https://example-theatre--review-hosting-a1b2c3d4.web.app"
        );
        assert_eq!(receipt["smoke"]["state"], "pass");
        assert_eq!(receipt["smoke"]["attempts"], 1);
        let bundle = &receipt["bundle"];
        assert_eq!(bundle["file_count"], 5);
        assert_eq!(bundle["manifest_sha256"].as_str().unwrap().len(), 64);
        assert!(bundle["files"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["sha256"].as_str().unwrap().len() == 64));
        assert!(receipt["config"]["policy_sha256"].as_str().unwrap().len() == 64);
        assert_eq!(
            receipt["stages"].as_array().unwrap().len(),
            Stage::ALL.len()
        );
        let text = receipt.to_string();
        for secret in ["@", "ya29.", "AIza", "Bearer", "password", "secret"] {
            assert!(!text.contains(secret), "{secret} in {text}");
        }
    }

    #[test]
    fn the_effective_config_holds_only_the_selected_target_site_and_project() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        let files: std::collections::BTreeMap<_, _> = rig.host.files.iter().cloned().collect();
        assert_eq!(files.len(), 2);
        let json: Value = serde_json::from_str(&files["firebase.json"]).unwrap();
        assert_eq!(json["hosting"].as_array().unwrap().len(), 1);
        assert_eq!(json["hosting"][0]["target"], "theatre");
        assert_eq!(json["hosting"][0]["public"], "dist");
        assert!(json.get("emulators").is_none() && !files["firebase.json"].contains("studio"));
        let rc: Value = serde_json::from_str(&files[".firebaserc"]).unwrap();
        assert_eq!(
            rc["targets"],
            serde_json::json!({"cantos-example": {"hosting": {"theatre": ["example-theatre"]}}})
        );
        assert!(
            !files[".firebaserc"].contains("studio")
                && !files[".firebaserc"].contains("example-studio")
        );
        // Every Firebase subprocess is told the project and this run's config explicitly.
        for call in &rig.capture.calls {
            if call.args.first().is_some_and(|a| a.starts_with("hosting:")) {
                let pair = |flag: &str| {
                    call.args
                        .windows(2)
                        .find(|w| w[0] == flag)
                        .map(|w| w[1].clone())
                };
                assert_eq!(pair("--project").as_deref(), Some("cantos-example"));
                assert_eq!(
                    pair("--config").as_deref(),
                    Some("/repo/artifacts/hosting/RUN/firebase.json")
                );
                assert_eq!(call.cwd, PathBuf::from("/repo/artifacts/hosting/RUN"));
            }
        }
    }

    #[test]
    fn the_build_and_every_scan_use_the_runs_own_fresh_dist_and_nothing_private_lives_in_it() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert!(report.succeeded(), "{}", report.render());
        let dist = PathBuf::from("/repo/artifacts/hosting/RUN/dist");
        // Trunk writes into the run-owned directory; the bundle that is validated, re-validated
        // and uploaded is that directory, never an older `dist` found by name or modification time.
        let build = &rig.runner.commands[0];
        let position = build.args.iter().position(|a| a == "--dist").unwrap();
        assert_eq!(PathBuf::from(&build.args[position + 1]), dist);
        assert_eq!(rig.host.scanned, [dist.clone(), dist.clone()]);
        // The effective configuration, its mapping and the receipt sit beside it, outside `public`.
        let files: Vec<&str> = rig
            .host
            .files
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(files, ["firebase.json", ".firebaserc"]);
        assert!(rig.host.files[0].1.contains("\"public\": \"dist\""));
    }

    #[test]
    fn studio_deploys_independently_and_never_builds_or_names_theatre() {
        let mut rig = Rig::preview(HostedApp::CantosStudio);
        let report = rig.run(&request_of(HostedApp::CantosStudio, "review-hosting"));
        assert!(report.succeeded(), "{}", report.render());
        assert_eq!(rig.runner.commands.len(), 1);
        let build = &rig.runner.commands[0];
        assert!(
            build.cwd.ends_with("apps/cantos-studio/web"),
            "{:?}",
            build.cwd
        );
        assert!(build.args.contains(&"--release".to_owned()));
        assert!(build
            .args
            .contains(&"/repo/artifacts/hosting/RUN/dist".to_owned()));
        let release = rig.calls_of("hosting:channel:deploy")[0];
        assert!(release.args.windows(2).any(|w| w == ["--only", "studio"]));
        let everything = format!("{:?} {:?}", rig.runner.commands, rig.capture.calls);
        assert!(!everything.contains("theatre"), "{everything}");
        assert!(rig
            .host
            .files
            .iter()
            .all(|(_, text)| !text.contains("theatre")));
        assert_eq!(last_receipt(&rig)["site"], "example-studio");
    }

    #[test]
    fn a_live_run_uses_the_deploy_command_with_a_message_and_takes_its_url_from_the_lookup() {
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture
            .ok(0, "15.34.0\n", "")
            .ok(0, SITES, "")
            .ok(0, LIVE, "");
        rig.good_smoke_round();
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "live"));
        assert!(report.succeeded(), "{}", report.render());
        assert!(rig.calls_of("hosting:channel:deploy").is_empty());
        let deploys = rig.calls_of("deploy");
        assert_eq!(deploys.len(), 1);
        let args = &deploys[0].args;
        assert!(args.windows(2).any(|w| w == ["--only", "hosting:theatre"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--message", "cantos cantos-theatre 0123456789ab RUN"]));
        assert!(!args.contains(&"--expires".to_owned()));
        assert!(
            detail_of(&report, Stage::Release).starts_with("LIVE release of site example-theatre")
        );
        let receipt = last_receipt(&rig);
        assert_eq!(receipt["channel"], "live");
        assert_eq!(
            receipt["remote"]["release"]["url"],
            "https://example-theatre.web.app"
        );
        assert_eq!(receipt["smoke"]["url"], "https://example-theatre.web.app");
        assert!(report.notes.iter().all(|n| !n.contains("PUBLIC")));
    }

    #[test]
    fn a_preview_deploy_names_expiry_and_warns_that_the_url_is_public_and_static_only() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        let request = Request {
            expires: Some(super::super::request::Expiry::parse("12h").unwrap()),
            ..request_of(HostedApp::CantosTheatre, "review-hosting")
        };
        let report = rig.run(&request);
        assert!(report.succeeded(), "{}", report.render());
        let release = rig.calls_of("hosting:channel:deploy")[0];
        assert!(release.args.windows(2).any(|w| w == ["--expires", "12h"]));
        let notes = report.notes.join("\n");
        assert!(notes.contains("PUBLIC"), "{notes}");
        assert!(notes.contains("static delivery only"), "{notes}");
        assert!(notes.contains("preview channel expires"), "{notes}");
        assert!(
            notes.contains("Hosting version: 0123456789abcdef"),
            "{notes}"
        );
    }

    #[test]
    fn missing_tools_or_a_redirecting_environment_stop_everything_before_any_run_file_or_build() {
        for (label, tools, overrides) in [
            ("no trunk", FakeTools::without(&[Tool::Trunk]), vec![]),
            (
                "no wasm target",
                FakeTools::without(&[Tool::Wasm32Target]),
                vec![],
            ),
            ("no git", FakeTools::without(&[Tool::Git]), vec![]),
            (
                "trunk override",
                FakeTools::all(),
                vec!["TRUNK_BUILD_DIST".to_owned()],
            ),
        ] {
            let mut rig = Rig::bare(HostedApp::CantosTheatre);
            rig.tools = tools;
            rig.host.overrides = overrides;
            rig.capture.ok(0, "15.34.0\n", "");
            let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
            assert!(!report.succeeded(), "{label}");
            assert_eq!(
                status_of(&report, Stage::Prerequisites),
                Status::Unsupported,
                "{label}"
            );
            assert_eq!(rig.host.runs_created, 0, "{label}");
            assert!(rig.runner.commands.is_empty(), "{label}");
            assert_eq!(
                rig.programs(),
                ["firebase --version"],
                "{label}: only the version probe may run"
            );
            assert!(
                after(&report, Stage::Prerequisites)
                    .iter()
                    .all(|s| *s == Stage::FirebaseCli || *s == Stage::HostingConfig),
                "{label}"
            );
        }
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.host.overrides = vec!["TRUNK_BUILD_DIST".into(), "TRUNK_BUILD_PUBLIC_URL".into()];
        rig.capture.ok(0, "15.34.0\n", "");
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        let detail = detail_of(&report, Stage::Prerequisites);
        assert!(
            detail.contains("TRUNK_BUILD_DIST") && detail.contains("TRUNK_BUILD_PUBLIC_URL"),
            "{detail}"
        );
    }

    #[test]
    fn configuration_problems_fail_before_the_run_record_the_lookup_or_the_build() {
        let broken_policy = POLICY.replace("\"hosting\"", "\"functions\"");
        let two_sites = RC.replace("[\"example-studio\"]", "[\"example-theatre\"]");
        let other_project = RC.replace("cantos-example", "some-other-project");
        let ambiguous = RC.replace("[\"example-theatre\"]", "[\"a\",\"b\"]");
        let ambiguous_other = RC.replace(
            "[\"example-studio\"]",
            "[\"example-theatre\",\"example-studio\"]",
        );
        let dotted_site = RC.replace("example-studio", "studio.cantos.web.app");
        let cases: Vec<(&str, Result<ConfigFiles, String>, &str)> = vec![
            (
                "no mapping",
                Ok(ConfigFiles {
                    policy: POLICY.into(),
                    firebaserc: None,
                }),
                "firebaserc",
            ),
            (
                "unreadable",
                Err("infra/firebase/hosting/.firebaserc: PermissionDenied".into()),
                "PermissionDenied",
            ),
            (
                "another project",
                Ok(ConfigFiles {
                    policy: POLICY.into(),
                    firebaserc: Some(other_project),
                }),
                "no `targets` entry for this project",
            ),
            (
                "both targets one site",
                Ok(ConfigFiles {
                    policy: POLICY.into(),
                    firebaserc: Some(two_sites),
                }),
                "two distinct sites",
            ),
            (
                "ambiguous",
                Ok(ConfigFiles {
                    policy: POLICY.into(),
                    firebaserc: Some(ambiguous),
                }),
                "exactly one",
            ),
            (
                "forbidden policy",
                Ok(ConfigFiles {
                    policy: broken_policy,
                    firebaserc: Some(RC.into()),
                }),
                "top-level key",
            ),
            (
                "other target conceals a shared site",
                Ok(ConfigFiles {
                    policy: POLICY.into(),
                    firebaserc: Some(ambiguous_other),
                }),
                "exactly one",
            ),
            (
                "custom domain in other site mapping",
                Ok(ConfigFiles {
                    policy: POLICY.into(),
                    firebaserc: Some(dotted_site),
                }),
                "invalid site",
            ),
        ];
        for (label, config, needle) in cases {
            let mut rig = Rig::bare(HostedApp::CantosTheatre);
            rig.host.config = config;
            rig.capture.ok(0, "15.34.0\n", "");
            let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
            assert!(!report.succeeded(), "{label}");
            assert_eq!(
                status_of(&report, Stage::HostingConfig),
                Status::Fail,
                "{label}"
            );
            assert!(
                detail_of(&report, Stage::HostingConfig).contains(needle),
                "{label}: {}",
                detail_of(&report, Stage::HostingConfig)
            );
            assert_eq!(rig.host.runs_created, 0, "{label}");
            assert!(rig.runner.commands.is_empty(), "{label}");
            assert_eq!(rig.programs(), ["firebase --version"], "{label}");
        }
    }

    #[test]
    fn an_absent_or_incompatible_firebase_cli_is_unsupported_before_any_run_file() {
        for (label, result) in [
            (
                "not installed",
                Err(CaptureError::Spawn {
                    program: "firebase".into(),
                    message: "No such file".into(),
                }),
            ),
            (
                "old",
                Ok(Captured {
                    code: 0,
                    stdout: "15.33.0\n".into(),
                    stderr: String::new(),
                }),
            ),
            (
                "new major",
                Ok(Captured {
                    code: 0,
                    stdout: "16.0.0\n".into(),
                    stderr: String::new(),
                }),
            ),
            (
                "garbage",
                Ok(Captured {
                    code: 0,
                    stdout: "firebase".into(),
                    stderr: String::new(),
                }),
            ),
            (
                "nonzero",
                Ok(Captured {
                    code: 1,
                    stdout: String::new(),
                    stderr: "boom".into(),
                }),
            ),
        ] {
            let mut rig = Rig::bare(HostedApp::CantosTheatre);
            rig.capture.results.push_back(result);
            let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
            assert_eq!(
                status_of(&report, Stage::FirebaseCli),
                Status::Unsupported,
                "{label}"
            );
            assert_eq!(rig.host.runs_created, 0, "{label}");
            assert!(rig.runner.commands.is_empty(), "{label}");
            assert_eq!(rig.release_calls(), 0, "{label}");
            assert_eq!(rig.capture.calls.len(), 1, "{label}");
        }
    }

    #[test]
    fn destination_failures_stop_before_the_build_and_the_upload() {
        let wrong_site = SITES.replace("example-theatre", "someone-elses-site");
        let cases: Vec<(&str, Result<Captured, CaptureError>, &str)> = vec![
            (
                "not logged in",
                Ok(Captured {
                    code: 1,
                    stdout: E_AUTH.into(),
                    stderr: String::new(),
                }),
                "authentication",
            ),
            (
                "no permission",
                Ok(Captured {
                    code: 1,
                    stdout: E_PERMISSION.into(),
                    stderr: String::new(),
                }),
                "permission",
            ),
            (
                "site not in project",
                Ok(Captured {
                    code: 0,
                    stdout: wrong_site,
                    stderr: String::new(),
                }),
                "not in this project",
            ),
            (
                "empty project",
                Ok(Captured {
                    code: 0,
                    stdout: "{\"status\":\"success\",\"result\":{\"sites\":[]}}".into(),
                    stderr: String::new(),
                }),
                "not in this project",
            ),
            (
                "timeout",
                Err(CaptureError::TimedOut {
                    after: Duration::from_secs(90),
                    partial: Partial::default(),
                }),
                "did not complete",
            ),
            (
                "changed output",
                Ok(Captured {
                    code: 0,
                    stdout: "ok".into(),
                    stderr: String::new(),
                }),
                "unrecognized",
            ),
        ];
        for (label, result, needle) in cases {
            let mut rig = Rig::bare(HostedApp::CantosTheatre);
            rig.capture.ok(0, "15.34.0\n", "");
            rig.capture.results.push_back(result);
            let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
            assert_rows(
                &report,
                &[
                    (Stage::RunRecord, Status::Pass),
                    (Stage::Destination, Status::Fail),
                    (Stage::Build, Status::NotRun),
                    (Stage::Bundle, Status::NotRun),
                    (Stage::Release, Status::NotRun),
                    (Stage::Smoke, Status::NotRun),
                ],
            );
            assert!(
                detail_of(&report, Stage::Destination).contains(needle),
                "{label}: {}",
                detail_of(&report, Stage::Destination)
            );
            assert!(rig.runner.commands.is_empty(), "{label}");
            assert_eq!(rig.release_calls(), 0, "{label}");
        }
    }

    #[test]
    fn a_failed_build_never_validates_an_old_bundle_or_writes() {
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture.ok(0, "15.34.0\n", "").ok(0, SITES, "");
        rig.runner.fail = true;
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert_rows(
            &report,
            &[
                (Stage::Build, Status::Fail),
                (Stage::Bundle, Status::NotRun),
                (Stage::IntentReceipt, Status::NotRun),
                (Stage::Release, Status::NotRun),
            ],
        );
        assert!(
            !rig.events().contains(&"scan".to_owned()),
            "no bundle is scanned after a failed build"
        );
        assert_eq!(rig.release_calls(), 0);
    }

    #[test]
    fn a_source_change_during_the_build_refuses_the_bundle() {
        for after_build in [
            Ok(SourceIdentity {
                commit: "f".repeat(40),
                ..identity(false)
            }),
            Ok(identity(true)),
            Err("git failed".to_owned()),
        ] {
            let mut rig = Rig::bare(HostedApp::CantosTheatre);
            rig.capture.ok(0, "15.34.0\n", "").ok(0, SITES, "");
            rig.host.identities = [Ok(identity(false)), after_build].into();
            let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
            assert_eq!(status_of(&report, Stage::Build), Status::Fail);
            assert_eq!(status_of(&report, Stage::Release), Status::NotRun);
            assert_eq!(rig.release_calls(), 0);
        }
    }

    #[test]
    fn an_invalid_bundle_is_never_uploaded_and_the_findings_are_bounded() {
        let mut scan = good_scan(crate::config::App::Theatre);
        scan.files.push(file("receipt.json", b"{}"));
        scan.files.push(file(".env", b"SECRET=hunter2"));
        scan.files.push(file("take.wav", b"RIFF"));
        scan.files.push(file("html-as.wasm", b"<!doctype html>"));
        for index in 0..10 {
            scan.files.push(file(&format!("x{index}.map"), b"x"));
        }
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture.ok(0, "15.34.0\n", "").ok(0, SITES, "");
        rig.host.scans = [Ok(scan)].into();
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert_rows(
            &report,
            &[
                (Stage::Build, Status::Pass),
                (Stage::Bundle, Status::Fail),
                (Stage::IntentReceipt, Status::NotRun),
                (Stage::Release, Status::NotRun),
            ],
        );
        let detail = detail_of(&report, Stage::Bundle);
        assert!(detail.contains("+") && detail.contains("more"), "{detail}");
        assert!(!detail.contains("hunter2"));
        assert!(detail.len() < 1500, "{}", detail.len());
        assert_eq!(rig.release_calls(), 0);
        // The failure is kept in the run record, with no remote write attempted.
        assert_eq!(last_receipt(&rig)["remote"]["state"], "not_attempted");
    }

    #[test]
    fn a_bundle_that_changes_between_validation_and_upload_is_not_uploaded() {
        let mut changed = good_scan(crate::config::App::Theatre);
        let index = changed
            .files
            .iter()
            .position(|f| f.path == "assets/icons/home.svg")
            .unwrap();
        changed.files[index] = file("assets/icons/home.svg", b"<svg>tampered</svg>");
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture.ok(0, "15.34.0\n", "").ok(0, SITES, "");
        rig.host.scans = [Ok(good_scan(crate::config::App::Theatre)), Ok(changed)].into();
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert_rows(
            &report,
            &[
                (Stage::Bundle, Status::Pass),
                (Stage::IntentReceipt, Status::Fail),
                (Stage::Release, Status::NotRun),
            ],
        );
        assert!(detail_of(&report, Stage::IntentReceipt).contains("changed after validation"));
        assert_eq!(rig.release_calls(), 0);
        // A second scan that stopped validating is refused too.
        let mut broken = good_scan(crate::config::App::Theatre);
        broken.files.push(file(".env", b"x"));
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture.ok(0, "15.34.0\n", "").ok(0, SITES, "");
        rig.host.scans = [Ok(good_scan(crate::config::App::Theatre)), Ok(broken)].into();
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert_eq!(status_of(&report, Stage::IntentReceipt), Status::Fail);
        assert_eq!(rig.release_calls(), 0);
    }

    #[test]
    fn if_intent_cannot_be_recorded_nothing_is_written_remotely() {
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture.ok(0, "15.34.0\n", "").ok(0, SITES, "");
        rig.host.fail_persist_from = Some(2);
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert_rows(
            &report,
            &[
                (Stage::IntentReceipt, Status::Fail),
                (Stage::Release, Status::NotRun),
            ],
        );
        assert!(detail_of(&report, Stage::IntentReceipt).contains("nothing was uploaded"));
        assert_eq!(rig.release_calls(), 0);
    }

    #[test]
    fn a_transient_intent_persist_failure_never_leaves_a_pending_write_in_the_receipt() {
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture.ok(0, "15.34.0\n", "").ok(0, SITES, "");
        // Only the intent write fails; the final receipt can still be written afterwards.
        rig.host.fail_persist_only = Some(2);
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert_eq!(status_of(&report, Stage::IntentReceipt), Status::Fail);
        assert_eq!(rig.release_calls(), 0);
        let receipt = last_receipt(&rig);
        assert_eq!(receipt["remote"]["state"], "not_attempted");
        assert_eq!(receipt["phase"], "started");
        assert!(receipt["remote"]["reconcile"].is_null());
    }

    #[test]
    fn the_confirmed_release_identity_is_durable_before_the_smoke_starts() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        let events = rig.events();
        let write = events
            .iter()
            .position(|e| e == "capture:firebase hosting:channel:deploy")
            .unwrap();
        let first_smoke = events
            .iter()
            .position(|e| e.starts_with("capture:curl"))
            .unwrap();
        let persisted_between = events[write..first_smoke]
            .iter()
            .any(|e| e == "persist:complete");
        assert!(persisted_between, "{events:?}");
    }

    #[test]
    fn a_trailing_slash_on_the_reported_url_does_not_break_the_smoke() {
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        let slashed = PREVIEW.replace(".web.app\"", ".web.app/\"");
        rig.capture
            .ok(0, "15.34.0\n", "")
            .ok(0, SITES, "")
            .ok(0, &slashed, "");
        rig.good_smoke_round();
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert!(report.succeeded(), "{}", report.render());
        let curl = rig
            .capture
            .calls
            .iter()
            .find(|c| c.program == "curl")
            .unwrap();
        assert!(
            !curl.args.last().unwrap().contains(".app//"),
            "{:?}",
            curl.args.last()
        );
    }

    #[test]
    fn the_run_record_and_its_effective_files_must_exist_before_any_effect() {
        let mut no_dir = Rig::bare(HostedApp::CantosTheatre);
        no_dir.host.fail_create_run = true;
        no_dir.capture.ok(0, "15.34.0\n", "");
        let report = no_dir.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert_eq!(status_of(&report, Stage::RunRecord), Status::Fail);
        assert_eq!(no_dir.capture.calls.len(), 1, "only the version probe ran");

        for file_name in ["firebase.json", ".firebaserc"] {
            let mut rig = Rig::bare(HostedApp::CantosTheatre);
            rig.host.fail_write_of = Some(file_name);
            rig.capture.ok(0, "15.34.0\n", "");
            let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
            assert_eq!(
                status_of(&report, Stage::RunRecord),
                Status::Fail,
                "{file_name}"
            );
            assert_eq!(rig.capture.calls.len(), 1, "{file_name}");
        }

        let mut no_receipt = Rig::bare(HostedApp::CantosTheatre);
        no_receipt.host.fail_persist_from = Some(1);
        no_receipt.capture.ok(0, "15.34.0\n", "");
        let report = no_receipt.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert_eq!(status_of(&report, Stage::RunRecord), Status::Fail);
        assert!(detail_of(&report, Stage::RunRecord).contains("no effect was started"));
        assert_eq!(no_receipt.capture.calls.len(), 1);
    }

    #[test]
    fn refusals_timeouts_and_changed_output_are_reported_without_replay_or_a_smoke() {
        let cases: Vec<(&str, Result<Captured, CaptureError>, &str)> = vec![
            (
                "refused: auth",
                Ok(Captured {
                    code: 1,
                    stdout: E_AUTH.into(),
                    stderr: String::new(),
                }),
                "REFUSED",
            ),
            (
                "refused: permission",
                Ok(Captured {
                    code: 1,
                    stdout: E_PERMISSION.into(),
                    stderr: String::new(),
                }),
                "REFUSED",
            ),
            (
                "unclassified failure",
                Ok(Captured {
                    code: 1,
                    stdout: E_UPLOAD.into(),
                    stderr: String::new(),
                }),
                "UNKNOWN",
            ),
            (
                "changed shape",
                Ok(Captured {
                    code: 0,
                    stdout: CHANGED.into(),
                    stderr: String::new(),
                }),
                "UNKNOWN",
            ),
            (
                "empty success",
                Ok(Captured {
                    code: 0,
                    stdout: String::new(),
                    stderr: String::new(),
                }),
                "UNKNOWN",
            ),
            (
                "timeout",
                Err(CaptureError::TimedOut {
                    after: Duration::from_secs(1200),
                    partial: Partial::default(),
                }),
                "UNKNOWN",
            ),
            (
                "interrupt",
                Err(CaptureError::Interrupted {
                    partial: Partial::default(),
                    started: true,
                }),
                "UNKNOWN",
            ),
            (
                "output limit",
                Err(CaptureError::OutputLimit {
                    partial: Partial::default(),
                }),
                "UNKNOWN",
            ),
            (
                "never started",
                Err(CaptureError::Spawn {
                    program: "firebase".into(),
                    message: "gone".into(),
                }),
                "did not start",
            ),
        ];
        for (label, result, needle) in cases {
            let mut rig = Rig::bare(HostedApp::CantosTheatre);
            rig.capture.ok(0, "15.34.0\n", "").ok(0, SITES, "");
            rig.capture.results.push_back(result);
            let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
            assert!(!report.succeeded(), "{label}");
            assert_eq!(status_of(&report, Stage::Release), Status::Fail, "{label}");
            assert!(
                detail_of(&report, Stage::Release).contains(needle),
                "{label}: {}",
                detail_of(&report, Stage::Release)
            );
            assert_eq!(status_of(&report, Stage::Smoke), Status::NotRun, "{label}");
            assert_eq!(rig.release_calls(), 1, "{label}: never replayed");
            assert_eq!(
                rig.capture.calls.len(),
                3,
                "{label}: version, lookup, one release, no curl"
            );
            let state = last_receipt(&rig)["remote"]["state"].as_str().unwrap();
            assert!(
                ["rejected", "unknown", "not_started"].contains(&state),
                "{label}: {state}"
            );
            if needle == "UNKNOWN" {
                assert!(
                    report.notes.join("\n").contains("RECONCILE FIRST"),
                    "{label}"
                );
                assert!(
                    last_receipt(&rig)["remote"]["reconcile"].is_string(),
                    "{label}"
                );
            }
            assert_eq!(
                status_of(&report, Stage::FinalReceipt),
                Status::Pass,
                "{label}"
            );
        }
    }

    #[test]
    fn no_address_or_token_from_cli_output_reaches_the_report_or_the_receipts() {
        let leaky = r#"{"status":"error","error":"HTTP Error: 403, caller qa.person@example.com lacks permission; token=abc ya29.secret"}"#;
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture.ok(0, "15.34.0\n", "").ok(0, SITES, "");
        rig.capture
            .ok(1, leaky, "stderr mentions other@example.org and AIzaSyFAKE");
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        let everything = format!(
            "{} {}",
            report.render(),
            rig.host
                .persisted
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        );
        for secret in ["qa.person", "other@", "ya29.", "AIza", "token=abc"] {
            assert!(
                !everything.contains(secret),
                "{secret} leaked: {everything}"
            );
        }
    }

    #[test]
    fn a_failed_final_receipt_after_a_confirmed_release_keeps_a_recovery_pointer() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        rig.host.fail_persist_from = Some(3);
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert!(!report.succeeded());
        assert_eq!(status_of(&report, Stage::Release), Status::Pass);
        assert_eq!(status_of(&report, Stage::FinalReceipt), Status::Fail);
        let notes = report.notes.join("\n");
        assert!(notes.contains("RECOVERY"), "{notes}");
        assert!(notes.contains("do not rerun"), "{notes}");
        assert!(
            notes.contains("URL: https://example-theatre--review-hosting-a1b2c3d4.web.app"),
            "{notes}"
        );
        assert_eq!(rig.release_calls(), 1);
    }

    #[test]
    fn a_smoke_failure_is_a_separate_local_result_and_the_release_stays_known() {
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture
            .ok(0, "15.34.0\n", "")
            .ok(0, SITES, "")
            .ok(0, PREVIEW, "");
        for _ in 0..smoke::ATTEMPTS {
            // The wasm is served as an octet stream every time; the rest is fine.
            let probes = rig.probes();
            for probe in probes {
                let wasm = matches!(
                    &probe.expect,
                    Expect::Asset {
                        family: Family::Wasm,
                        ..
                    }
                );
                let headers = match &probe.expect {
                    Expect::Absent => "HTTP/2 404\r\n\r\n".to_owned(),
                    _ if wasm => "HTTP/2 200\r\ncontent-type: application/octet-stream\r\ncache-control: no-cache\r\n\r\n".to_owned(),
                    Expect::Page { .. } => "HTTP/2 200\r\ncontent-type: text/html\r\ncache-control: no-cache\r\n\r\n".to_owned(),
                    Expect::Asset { family, .. } => format!("HTTP/2 200\r\ncontent-type: {}\r\ncache-control: no-cache\r\n\r\n", media_type(*family)),
                };
                let sha = match probe.expect {
                    Expect::Page { sha256 } | Expect::Asset { sha256, .. } => sha256,
                    Expect::Absent => "x".into(),
                };
                rig.capture.ok(0, &headers, "");
                rig.host.hashes.push_back(Ok((sha, 1)));
            }
        }
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert!(!report.succeeded());
        assert_rows(
            &report,
            &[
                (Stage::Release, Status::Pass),
                (Stage::Smoke, Status::Fail),
                (Stage::FinalReceipt, Status::Pass),
            ],
        );
        let detail = detail_of(&report, Stage::Smoke);
        assert!(
            detail.contains("release is confirmed") && detail.contains("application/octet-stream"),
            "{detail}"
        );
        assert_eq!(
            rig.release_calls(),
            1,
            "a failed smoke never retries the write"
        );
        assert_eq!(rig.host.pauses, [smoke::PAUSE, smoke::PAUSE]);
        let receipt = last_receipt(&rig);
        assert_eq!(receipt["remote"]["state"], "confirmed");
        assert_eq!(receipt["smoke"]["state"], "fail");
        assert_eq!(receipt["smoke"]["attempts"], smoke::ATTEMPTS);
    }

    #[test]
    fn read_only_smoke_requests_wait_out_propagation_but_the_write_is_never_repeated() {
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture
            .ok(0, "15.34.0\n", "")
            .ok(0, SITES, "")
            .ok(0, PREVIEW, "");
        rig.stale_smoke_round();
        rig.good_smoke_round();
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert!(report.succeeded(), "{}", report.render());
        assert_eq!(rig.release_calls(), 1);
        assert_eq!(rig.host.pauses, [smoke::PAUSE]);
        assert_eq!(last_receipt(&rig)["smoke"]["attempts"], 2);
    }

    #[test]
    fn a_missing_curl_leaves_the_smoke_not_run_without_failing_a_confirmed_release() {
        let mut rig = Rig::bare(HostedApp::CantosTheatre);
        rig.capture
            .ok(0, "15.34.0\n", "")
            .ok(0, SITES, "")
            .ok(0, PREVIEW, "");
        rig.capture.results.push_back(Err(CaptureError::Spawn {
            program: "curl".into(),
            message: "not found".into(),
        }));
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert!(report.succeeded(), "{}", report.render());
        assert_eq!(status_of(&report, Stage::Release), Status::Pass);
        assert_eq!(status_of(&report, Stage::Smoke), Status::NotRun);
        assert!(detail_of(&report, Stage::Smoke).contains("curl is not installed"));
        assert_eq!(last_receipt(&rig)["smoke"]["state"], "unavailable");
        assert_eq!(
            rig.capture.calls.len(),
            4,
            "no further request after curl is missing"
        );
    }

    #[test]
    fn requests_to_the_released_url_are_get_only_https_and_carry_no_credential() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        let curls: Vec<&CommandSpec> = rig
            .capture
            .calls
            .iter()
            .filter(|c| c.program == "curl")
            .collect();
        assert_eq!(curls.len(), rig.probes().len());
        for call in curls {
            let url = call.args.last().unwrap();
            assert!(
                url.starts_with("https://example-theatre--review-hosting-a1b2c3d4.web.app/"),
                "{url}"
            );
            let line = call.args.join(" ");
            for forbidden in [
                "--request",
                "-X",
                "--data",
                "--header",
                "-H",
                "--user",
                "--cookie",
                "--location",
            ] {
                assert!(!line.contains(forbidden), "{line}");
            }
            assert!(call.env.is_empty());
        }
    }

    #[test]
    fn the_coordinator_never_builds_another_app_android_the_api_or_distribution() {
        for (app, other) in [
            (HostedApp::CantosTheatre, "cantos-studio"),
            (HostedApp::CantosStudio, "cantos-theatre"),
        ] {
            let mut rig = Rig::preview(app);
            let report = rig.run(&request_of(app, "review-hosting"));
            assert!(report.succeeded(), "{}", report.render());
            let line = rig
                .runner
                .commands
                .iter()
                .chain(&rig.capture.calls)
                .map(|command| {
                    format!("{} {:?}", command.program, command.args).replace(
                        Workspace::discover().unwrap().root().to_str().unwrap(),
                        "<workspace>",
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            for forbidden in [
                other,
                "gradle",
                "cargo",
                "appdistribution",
                "podman",
                "migrat",
                "preflight",
                "xtask",
            ] {
                assert!(!line.contains(forbidden), "{forbidden} in {line}");
            }
            assert_eq!(rig.runner.commands.len(), 1);
            assert_eq!(rig.runner.commands[0].program, "trunk");
        }
    }
    #[test]
    fn missing_api_origin_refuses_hosting_before_a_run_lookup_or_build() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        rig.host.api_origin = None;
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "review-hosting"));
        assert_eq!(status_of(&report, Stage::HostingConfig), Status::Fail);
        assert_eq!(
            detail_of(&report, Stage::HostingConfig),
            "CANTOS_API_ORIGIN is required for Firebase Hosting; set an exact HTTPS API origin"
        );
        assert_eq!(rig.host.runs_created, 0);
        assert!(rig.host.persisted.is_empty());
        assert!(rig.runner.commands.is_empty());
        assert!(rig.calls_of("hosting:sites:list").is_empty());
        assert_eq!(rig.release_calls(), 0);
    }

    #[test]
    fn invalid_api_origins_refuse_both_apps_and_channels_without_exposing_values() {
        for app in [HostedApp::CantosTheatre, HostedApp::CantosStudio] {
            for channel in ["live", "review-hosting"] {
                for origin in [
                    "",
                    "http://127.0.0.1:8080",
                    "https://user:secret@api.example.org",
                    "https://api.example.org/api",
                    "https://api.example.org?token=secret",
                ] {
                    let mut rig = Rig::preview(app);
                    rig.host.api_origin = Some(origin.into());
                    let report = rig.run(&request_of(app, channel));
                    assert_eq!(status_of(&report, Stage::HostingConfig), Status::Fail);
                    assert_eq!(
                        detail_of(&report, Stage::HostingConfig),
                        "invalid CANTOS_API_ORIGIN"
                    );
                    assert!(!report.render().contains("secret"));
                    assert_eq!(rig.host.runs_created, 0);
                    assert!(rig.host.persisted.is_empty());
                    assert!(rig.runner.commands.is_empty());
                    assert!(rig.calls_of("hosting:sites:list").is_empty());
                    assert_eq!(rig.release_calls(), 0);
                }
            }
        }
    }

    #[test]
    fn explicit_api_origin_is_pinned_in_the_build_command_and_receipt() {
        let mut rig = Rig::preview(HostedApp::CantosTheatre);
        rig.host.api_origin = Some("https://api.example.org".into());
        let request = request_of(HostedApp::CantosTheatre, "review");
        let report = rig.run(&request);
        assert_eq!(status_of(&report, Stage::Build), Status::Pass);
        assert!(rig.runner.commands.iter().any(|command| command
            .env
            .contains(&("CANTOS_API_ORIGIN".into(), "https://api.example.org".into()))));
        assert_eq!(
            rig.host.persisted.last().unwrap()["build"]["api_origin"],
            "https://api.example.org"
        );
    }
}
