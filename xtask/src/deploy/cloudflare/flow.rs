//! The ordered Pages deployment. Each stage either passes or halts the run; nothing after a
//! failed stage executes, and the one remote write is attempted at most once and never retried.
//!
//! The stages are ordered by what they may touch: pure checks first, then local files, then two
//! read-only provider lookups, and only then the single `pages deploy`. A timeout or an
//! unrecognised result after the write is **unknown**, and the run says how to read the target
//! back instead of repeating the write.

use std::path::PathBuf;

use serde_json::{json, Map, Value};

use super::super::{
    bundle::{self, Finding, Manifest, Provider},
    smoke::{self, Observed, Verdict},
};
use super::{
    artifact::{self, Release},
    host::{Host, Run, ARCHIVE_SUFFIX},
    policy::{self, PolicyFiles},
    request::{self, Destination, Environment, Request, Stage},
    wrangler::{self, Expected, Invocation, Publish, Readback, RemoteOutcome},
};
use crate::{
    capture::{Capture, CaptureError},
    report::{Report, Status},
    run_files::sha256_hex,
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

/// Cloudflare Pages (Free plan) limits at the time of writing: 20,000 files and 25 MiB per asset.
// ACCEPTANCE(NOT_RUN): the limits were read from the public documentation; real uploads (3 MB and 6 MB) were
// far below them and never exercised them. A paid plan raises the file count only; the per-asset bound is checked as measured on
// the decoded `.wasm`, never on its compressed size. Owner: the first authorized Pages deploy.
pub const PAGES_MAX_FILES: usize = 20_000;
pub const PAGES_MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;

pub struct Ports<'a> {
    pub tools: &'a dyn ToolProbe,
    pub capture: &'a mut dyn Capture,
    pub host: &'a mut dyn Host,
}

pub fn run<'a>(workspace: &'a Workspace, request: &'a Request, ports: Ports<'a>) -> Report {
    let mut flow = Flow {
        workspace,
        request,
        ports,
        rows: Vec::new(),
        notes: Vec::new(),
        destination: None,
        policy: None,
        release: None,
        archive_sha256: None,
        manifest_sha256: None,
        wrangler_version: None,
        run: None,
        receipt: None,
        bundle: None,
        remote: None,
        domains: Vec::new(),
    };
    let _ = flow.go();
    flow.finish()
}

struct Halt;

struct Flow<'a> {
    workspace: &'a Workspace,
    request: &'a Request,
    ports: Ports<'a>,
    rows: Vec<(Stage, Status, String)>,
    notes: Vec<String>,
    destination: Option<Destination>,
    policy: Option<PolicyFiles>,
    release: Option<Release>,
    archive_sha256: Option<String>,
    manifest_sha256: Option<String>,
    wrangler_version: Option<String>,
    run: Option<Run>,
    receipt: Option<Map<String, Value>>,
    bundle: Option<Manifest>,
    remote: Option<RemoteOutcome>,
    domains: Vec<String>,
}

impl Flow<'_> {
    fn record(&mut self, stage: Stage, status: Status, detail: impl Into<String>) {
        let detail = detail.into();
        println!("{:<9} {:<20} {detail}", status.to_string(), stage.label());
        self.rows.push((stage, status, detail));
        self.stamp_stage();
    }

    fn pass(&mut self, stage: Stage, detail: impl Into<String>) {
        self.record(stage, Status::Pass, detail);
    }

    fn halt(&mut self, stage: Stage, status: Status, detail: impl Into<String>) -> Halt {
        self.record(stage, status, detail);
        Halt
    }

    fn go(&mut self) -> Result<(), Halt> {
        self.prerequisites()?;
        self.policy_and_mapping()?;
        self.wrangler()?;
        self.artifact()?;
        self.staging()?;
        self.bundle()?;
        self.project()?;
        self.intent()?;
        self.publish();
        self.readback();
        self.smoke();
        Ok(())
    }

    fn invocation<'b>(
        &'b self,
        binary: &'b std::path::Path,
        cwd: &'b std::path::Path,
    ) -> Invocation<'b> {
        Invocation {
            binary,
            cwd,
            account: &self.request.account,
        }
    }

    fn prerequisites(&mut self) -> Result<(), Halt> {
        if !self.ports.tools.available(Tool::Node) {
            return Err(self.halt(
                Stage::Prerequisites,
                Status::Unsupported,
                "Node.js (22 or newer, required by the pinned Wrangler) was not found",
            ));
        }
        if !self.ports.host.api_token_present() {
            return Err(self.halt(
                Stage::Prerequisites,
                Status::Unsupported,
                "CLOUDFLARE_API_TOKEN is not set. xtask never logs in; use a token limited to Account / Cloudflare Pages / Edit (see the runbook)",
            ));
        }
        if let Some(ambient) = self.ports.host.ambient_account() {
            if ambient != self.request.account.as_str() {
                return Err(self.halt(
                    Stage::Prerequisites,
                    Status::Fail,
                    "CLOUDFLARE_ACCOUNT_ID in the environment names a different account than --account",
                ));
            }
        }
        self.pass(
            Stage::Prerequisites,
            "Node.js and a token variable are present (the token value is never read or recorded)",
        );
        Ok(())
    }

    fn policy_and_mapping(&mut self) -> Result<(), Halt> {
        let text = match self.ports.host.read_mapping() {
            Ok(text) => text,
            Err(why) => return Err(self.halt(Stage::Policy, Status::Fail, why)),
        };
        let destination = match policy::resolve_mapping(
            text.as_deref(),
            &self.request.account,
            self.request.target(),
        ) {
            Ok(destination) => destination,
            Err(why) => return Err(self.halt(Stage::Policy, Status::Fail, why)),
        };
        if let Err(mismatch) = request::agree(self.request, &destination) {
            return Err(self.halt(
                Stage::Policy,
                Status::Fail,
                format!("{mismatch}; nothing was extracted or uploaded"),
            ));
        }
        let files = match self.ports.host.read_policy(self.request.target()) {
            Ok(files) => files,
            Err(why) => return Err(self.halt(Stage::Policy, Status::Fail, why)),
        };
        if let Err(problems) = files.validate() {
            return Err(self.halt(Stage::Policy, Status::Fail, summarize(&problems)));
        }
        self.pass(
            Stage::Policy,
            format!(
                "account, project and branch agree with the mapping; policy {}",
                short(&files.digest())
            ),
        );
        self.destination = Some(destination);
        self.policy = Some(files);
        Ok(())
    }

    fn wrangler(&mut self) -> Result<(), Halt> {
        if !self.ports.host.wrangler_installed() {
            return Err(self.halt(
                Stage::Wrangler,
                Status::Unsupported,
                format!(
                    "the pinned Wrangler is not installed. Run `npm ci` in {} (no implicit npx download, no global install)",
                    super::host::wrangler_directory()
                ),
            ));
        }
        let binary = self.ports.host.wrangler_binary();
        let root = self.workspace.root().to_path_buf();
        let spec = wrangler::version_command(&self.invocation(&binary, &root));
        let result = self.ports.capture.capture(&spec, wrangler::VERSION_TIMEOUT);
        let output = match result {
            Ok(captured) if captured.code == 0 => captured.stdout,
            Ok(captured) => {
                return Err(self.halt(
                    Stage::Wrangler,
                    Status::Unsupported,
                    format!(
                        "`wrangler --version` exited {}: {}",
                        captured.code,
                        wrangler::redact(captured.stderr.lines().next().unwrap_or(""))
                    ),
                ))
            }
            Err(error) => {
                return Err(self.halt(Stage::Wrangler, Status::Unsupported, error.to_string()))
            }
        };
        match wrangler::parse_version(&output) {
            Some(version) if wrangler::version_supported(&version) => {
                self.pass(
                    Stage::Wrangler,
                    format!("Wrangler {version} (the pinned release)"),
                );
                self.wrangler_version = Some(version);
                Ok(())
            }
            Some(version) => Err(self.halt(
                Stage::Wrangler,
                Status::Unsupported,
                format!(
                    "Wrangler {version} is installed; this contract is tested against {} only",
                    wrangler::PINNED_VERSION
                ),
            )),
            None => Err(self.halt(
                Stage::Wrangler,
                Status::Unsupported,
                "no Wrangler version was reported",
            )),
        }
    }

    fn artifact(&mut self) -> Result<(), Halt> {
        let directory = &self.request.artifact;
        let names = match self.ports.host.list_artifact(directory) {
            Ok(names) => names,
            Err(why) => return Err(self.halt(Stage::Artifact, Status::Fail, why)),
        };
        let archive_name = format!("{}{ARCHIVE_SUFFIX}", self.request.component());
        if let Some(extra) = names
            .iter()
            .find(|name| name.as_str() != artifact::MANIFEST_FILE && **name != archive_name)
        {
            return Err(self.halt(
                Stage::Artifact,
                Status::Fail,
                format!("unexpected entry `{}` in the artifact directory (only manifest.json and {archive_name})", printable(extra)),
            ));
        }
        let manifest = match self.ports.host.read_manifest(directory) {
            Ok(text) => text,
            Err(why) => return Err(self.halt(Stage::Artifact, Status::Fail, why)),
        };
        let release = match artifact::parse_release(self.request, &manifest) {
            Ok(release) => release,
            Err(error) => return Err(self.halt(Stage::Artifact, Status::Fail, error.to_string())),
        };
        let facts = self
            .ports
            .host
            .archive_facts(&directory.join(&archive_name));
        let (sha256, bytes) = match facts {
            Ok(facts) => facts,
            Err(why) => return Err(self.halt(Stage::Artifact, Status::Fail, why)),
        };
        if let Err(error) = artifact::verify_archive(&release.archive, &sha256, bytes) {
            return Err(self.halt(Stage::Artifact, Status::Fail, error.to_string()));
        }
        self.pass(
            Stage::Artifact,
            format!(
                "{} at {} matches its manifest ({} bytes, sha256 {}); the bytes are checked, their origin is a recorded claim",
                archive_name,
                &release.source_sha[..12],
                bytes,
                short(&sha256)
            ),
        );
        self.manifest_sha256 = Some(sha256_hex(manifest.as_bytes()));
        self.archive_sha256 = Some(sha256);
        self.release = Some(release);
        Ok(())
    }

    fn staging(&mut self) -> Result<(), Halt> {
        let run = match self.ports.host.create_run() {
            Ok(run) => run,
            Err(why) => {
                return Err(self.halt(
                    Stage::Staging,
                    Status::Fail,
                    format!("could not create the run directory: {why}"),
                ))
            }
        };
        let now = self.ports.host.now();
        self.receipt = Some(self.new_receipt(&run, &now));
        self.run = Some(run.clone());
        let _ = self.persist();
        let hazards = self.ports.host.wrangler_hazards(&run);
        if !hazards.is_empty() {
            return Err(self.halt(
                Stage::Staging,
                Status::Fail,
                format!(
                    "Wrangler could pick up configuration other than the staged static directory: {}",
                    hazards.iter().take(3).cloned().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        let archive = self
            .request
            .artifact
            .join(format!("{}{ARCHIVE_SUFFIX}", self.request.component()));
        match self.ports.host.extract(&archive, &run) {
            Ok(done) => {
                self.pass(
                    Stage::Staging,
                    format!(
                        "{} files ({} bytes) extracted into a new run-owned directory",
                        done.files, done.bytes
                    ),
                );
                Ok(())
            }
            Err(error) => Err(self.halt(Stage::Staging, Status::Fail, error.to_string())),
        }
    }

    fn bundle(&mut self) -> Result<(), Halt> {
        let (Some(run), Some(policy)) = (self.run.clone(), self.policy.clone()) else {
            return Err(Halt);
        };
        for (name, content) in policy.files() {
            if let Err(why) = self.ports.host.add_file(&run, name, content.as_bytes()) {
                return Err(self.halt(
                    Stage::Bundle,
                    Status::Fail,
                    format!("could not add policy file {name}: {why}"),
                ));
            }
        }
        let manifest = match self.validated_bundle(&run) {
            Ok(manifest) => manifest,
            Err(why) => return Err(self.halt(Stage::Bundle, Status::Fail, why)),
        };
        self.pass(
            Stage::Bundle,
            format!(
                "{} files, {} bytes, bundle {}; policy files are part of the inventory",
                manifest.files.len(),
                manifest.total_bytes,
                short(&manifest.digest)
            ),
        );
        self.set_bundle(&manifest);
        self.bundle = Some(manifest);
        Ok(())
    }

    fn validated_bundle(&mut self, run: &Run) -> Result<Manifest, String> {
        let scan = self.ports.host.scan_bundle(run)?;
        let manifest = bundle::validate_for(self.request.app, &scan, Provider::Pages)
            .map_err(|findings| summarize_findings(&findings))?;
        let mut problems = Vec::new();
        if manifest.files.len() > PAGES_MAX_FILES {
            problems.push(format!(
                "{} files exceed the {PAGES_MAX_FILES}-file Pages limit",
                manifest.files.len()
            ));
        }
        for file in &manifest.files {
            if file.size > PAGES_MAX_FILE_BYTES {
                problems.push(format!("`{}` is {} bytes, over the {PAGES_MAX_FILE_BYTES}-byte per-asset Pages limit (measured on the decoded file)", file.path, file.size));
            }
        }
        if problems.is_empty() {
            Ok(manifest)
        } else {
            Err(problems.join("; "))
        }
    }

    fn project(&mut self) -> Result<(), Halt> {
        let (Some(run), Some(destination)) = (self.run.clone(), self.destination.clone()) else {
            return Err(Halt);
        };
        let binary = self.ports.host.wrangler_binary();
        let spec = wrangler::projects_command(&self.invocation(&binary, &run.path));
        let result = self.ports.capture.capture(&spec, wrangler::READ_TIMEOUT);
        match wrangler::interpret_projects(result, &self.request.project) {
            Ok(domains) => {
                self.pass(
                    Stage::Project,
                    format!(
                        "project `{}` exists as Direct Upload (production branch `{}` is the operator's mapping, confirmed by the deploy response)",
                        self.request.project.as_str(),
                        destination.production_branch.as_str()
                    ),
                );
                self.domains = domains;
                Ok(())
            }
            Err(error) => Err(self.halt(Stage::Project, Status::Fail, error.to_string())),
        }
    }

    fn intent(&mut self) -> Result<(), Halt> {
        let Some(run) = self.run.clone() else {
            return Err(Halt);
        };
        // The bytes must be exactly those validated, immediately before the write.
        let before = self.bundle.as_ref().map(|m| m.digest.clone());
        match self.validated_bundle(&run) {
            Ok(again) if Some(&again.digest) == before.as_ref() => {}
            Ok(_) => {
                return Err(self.halt(
                    Stage::IntentReceipt,
                    Status::Fail,
                    "the staged bytes changed after validation; nothing was uploaded",
                ))
            }
            Err(why) => return Err(self.halt(Stage::IntentReceipt, Status::Fail, why)),
        }
        self.set_remote(json!({"state": "intent", "reason": null, "release": null}));
        match self.persist() {
            Ok(path) => {
                self.pass(
                    Stage::IntentReceipt,
                    format!("intent recorded in {}", self.workspace.relative(&path)),
                );
                Ok(())
            }
            Err(why) => Err(self.halt(
                Stage::IntentReceipt,
                Status::Fail,
                format!("could not record intent, so no remote write was attempted: {why}"),
            )),
        }
    }

    fn publish(&mut self) {
        let (Some(run), Some(release)) = (self.run.clone(), self.release.clone()) else {
            return;
        };
        println!("\n==> {}", Stage::Publish.label());
        let binary = self.ports.host.wrangler_binary();
        let message = format!(
            "cantos {} {} {}",
            self.request.target(),
            &release.source_sha[..12],
            run.id
        );
        let output_file = run.output_file();
        let spec = wrangler::deploy_command(
            &self.invocation(&binary, &run.path),
            &Publish {
                project: &self.request.project,
                branch: &self.request.branch,
                directory: &run.public(),
                commit_hash: &release.source_sha,
                commit_message: &message,
                output_file: &output_file,
            },
        );
        let captured = self.ports.capture.capture(&spec, wrangler::DEPLOY_TIMEOUT);
        let diagnostics = diagnostics(&captured);
        let output = self.ports.host.read_output_file(&run);
        let outcome = wrangler::interpret_deploy(
            captured,
            output.as_deref(),
            wrangler::DEPLOY_TIMEOUT,
            &Expected {
                project: &self.request.project,
                branch: &self.request.branch,
                environment: self.request.environment,
                commit_hash: &release.source_sha,
            },
        );
        let (status, detail) = describe(&outcome);
        self.set_outcome(&outcome, diagnostics);
        self.remote = Some(outcome);
        self.record(Stage::Publish, status, detail);
        let _ = self.persist();
    }

    fn readback(&mut self) {
        let (Some(run), Some(outcome)) = (self.run.clone(), self.remote.clone()) else {
            return;
        };
        let binary = self.ports.host.wrangler_binary();
        let environment = self.request.environment;
        match &outcome {
            RemoteOutcome::NotStarted(_) | RemoteOutcome::Rejected { .. } => {}
            RemoteOutcome::Confirmed { release } | RemoteOutcome::Mismatch { release, .. } => {
                let spec = wrangler::list_command(
                    &self.invocation(&binary, &run.path),
                    &self.request.project,
                    parse_environment(&release.environment, environment),
                );
                let result = self.ports.capture.capture(&spec, wrangler::READ_TIMEOUT);
                match wrangler::interpret_list(result, &release.deployment_id, parse_environment(&release.environment, environment)) {
                    Readback::Listed => self.pass(Stage::Readback, format!("deployment {} is listed in `{}`", release.deployment_id, release.environment)),
                    Readback::Absent => self.record(Stage::Readback, Status::Fail, format!("deployment {} was reported but is not in the listing yet; check the dashboard before any retry", release.deployment_id)),
                    Readback::Unavailable(why) => self.record(Stage::Readback, Status::NotRun, format!("the listing could not be read ({why}); the deployment identity came from Wrangler's own result")),
                }
            }
            RemoteOutcome::Unknown { .. } => {
                let spec = wrangler::list_command(
                    &self.invocation(&binary, &run.path),
                    &self.request.project,
                    environment,
                );
                let result = self.ports.capture.capture(&spec, wrangler::READ_TIMEOUT);
                let source = self
                    .release
                    .as_ref()
                    .map(|r| r.source_sha.clone())
                    .unwrap_or_default();
                let found = wrangler::interpret_candidates(
                    result,
                    self.request.branch.as_str(),
                    &source[..source.len().min(7)],
                    environment,
                );
                match found {
                    Ok(candidates) if candidates.is_empty() => self.record(Stage::Readback, Status::NotRun, "no deployment of this branch and commit is listed; the write may still be in flight, read the project again before any retry"),
                    Ok(candidates) => {
                        let ids = candidates.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
                        self.merge_remote(json!({"candidates": ids}));
                        self.record(Stage::Readback, Status::NotRun, format!("{} deployment(s) of this branch and commit are listed ({}); the one made by this run is not established by the listing alone", candidates.len(), ids.join(", ")));
                    }
                    Err(why) => self.record(Stage::Readback, Status::NotRun, format!("the listing could not be read ({why})")),
                }
            }
        }
        let _ = self.persist();
    }

    fn smoke(&mut self) {
        let Some(RemoteOutcome::Confirmed { release }) = self.remote.clone() else {
            return;
        };
        let (Some(manifest), Some(run)) = (self.bundle.clone(), self.run.clone()) else {
            return;
        };
        println!("\n==> {}", Stage::Smoke.label());
        let probes = smoke::probes(&manifest, &run.id);
        let mut verdict = Verdict::unavailable();
        for attempt in 1..=smoke::ATTEMPTS {
            verdict = self.smoke_attempt(&release.url, &run, &probes, attempt);
            if verdict.tool_missing || verdict.passed() || attempt == smoke::ATTEMPTS {
                break;
            }
            self.ports.host.pause(smoke::PAUSE);
        }
        self.set_smoke(&release.url, &verdict);
        match verdict.state() {
            "pass" => self.pass(
                Stage::Smoke,
                format!(
                    "{} requests to {} answered as built ({} attempt(s)); static delivery only",
                    verdict.probes.len(),
                    release.url,
                    verdict.attempts
                ),
            ),
            "unavailable" => self.record(
                Stage::Smoke,
                Status::NotRun,
                "curl is not installed here, so the served bytes were not observed",
            ),
            _ => {
                let problems = verdict.problems();
                let shown: Vec<_> = problems.iter().take(4).cloned().collect();
                self.record(
                    Stage::Smoke,
                    Status::Fail,
                    format!("the deployment is confirmed but its URL did not serve the bundle as built: {}", shown.join("; ")),
                );
            }
        }
        let _ = self.persist();
    }

    fn smoke_attempt(
        &mut self,
        base: &str,
        run: &Run,
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
                Err(error) => Err(wrangler::redact(&error.to_string())),
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

    // ---- receipt ------------------------------------------------------------------------

    fn new_receipt(&self, run: &Run, now: &str) -> Map<String, Value> {
        let release = self.release.as_ref();
        let mut map = Map::new();
        let mut put = |key: &str, value: Value| {
            map.insert(key.to_owned(), value);
        };
        put("schema_version", json!(1));
        put("run_id", json!(run.id));
        put("created_at", json!(now));
        put("updated_at", json!(now));
        put("app", json!(self.request.app.cli_name()));
        put("target", json!(self.request.target()));
        put("account", json!(self.request.account.as_str()));
        put("project", json!(self.request.project.as_str()));
        put("branch", json!(self.request.branch.as_str()));
        put("environment", json!(self.request.environment.label()));
        put(
            "production_branch",
            json!(self
                .destination
                .as_ref()
                .map(|d| d.production_branch.as_str())),
        );
        put(
            "artifact",
            json!({
                "manifest_sha256": self.manifest_sha256,
                "archive": release.map(|r| json!({"path": r.archive.path, "bytes": r.archive.bytes, "sha256": r.archive.sha256})),
                "source_sha": release.map(|r| r.source_sha.clone()),
                "api_origin": release.map(|r| r.api_origin.clone()),
                "provenance": "recorded claim: the hash proves the bytes are unchanged, not who built them",
            }),
        );
        put(
            "policy",
            json!({"digest": self.policy.as_ref().map(PolicyFiles::digest)}),
        );
        put("tools", json!({"wrangler": self.wrangler_version}));
        put("stages", json!([]));
        put("bundle", Value::Null);
        put(
            "remote",
            json!({"state": "not_started", "reason": null, "release": null}),
        );
        put("smoke", Value::Null);
        map
    }

    fn with_receipt(&mut self, change: impl FnOnce(&mut Map<String, Value>)) {
        if let Some(receipt) = &mut self.receipt {
            change(receipt);
        }
    }

    fn stamp_stage(&mut self) {
        let Some((stage, status, detail)) = self.rows.last().cloned() else {
            return;
        };
        let now = self.ports.host.now();
        self.with_receipt(|receipt| {
            if let Some(Value::Array(stages)) = receipt.get_mut("stages") {
                stages.push(json!({"name": stage.label(), "status": status.to_string(), "detail": wrangler::redact(&detail), "at": now}));
            }
            receipt.insert("updated_at".to_owned(), json!(now));
        });
    }

    fn set_bundle(&mut self, manifest: &Manifest) {
        let files: Vec<Value> = manifest
            .files
            .iter()
            .map(|f| json!({"path": f.path, "size": f.size, "sha256": f.sha256}))
            .collect();
        self.with_receipt(|receipt| {
            receipt.insert("bundle".to_owned(), json!({
                "file_count": manifest.files.len(), "total_bytes": manifest.total_bytes,
                "digest": manifest.digest, "entry_assets": manifest.entry_assets, "files": files,
            }));
        });
    }

    fn set_remote(&mut self, value: Value) {
        self.with_receipt(|receipt| {
            receipt.insert("remote".to_owned(), value);
        });
    }

    fn merge_remote(&mut self, extra: Value) {
        self.with_receipt(|receipt| {
            if let (Some(Value::Object(remote)), Value::Object(extra)) =
                (receipt.get_mut("remote"), extra)
            {
                remote.extend(extra);
            }
        });
    }

    fn set_outcome(&mut self, outcome: &RemoteOutcome, diagnostics: Vec<String>) {
        let release = outcome.release().map(|r| json!({
            "deployment_id": r.deployment_id, "url": r.url, "alias": r.alias,
            "environment": r.environment, "production_branch": r.production_branch, "commit_hash": r.commit_hash,
        }));
        let reason = match outcome {
            RemoteOutcome::NotStarted(why) => Some(why.clone()),
            RemoteOutcome::Mismatch { problems, .. } => Some(problems.join("; ")),
            RemoteOutcome::Rejected { detail, .. } => Some(detail.clone()),
            RemoteOutcome::Unknown { reason } => Some(reason.to_string()),
            RemoteOutcome::Confirmed { .. } => None,
        };
        let reconcile = matches!(outcome, RemoteOutcome::Unknown { .. }).then_some(RECONCILE);
        self.set_remote(json!({
            "state": outcome.state(), "reason": reason, "release": release,
            "reconcile": reconcile, "diagnostics": diagnostics,
        }));
    }

    fn set_smoke(&mut self, url: &str, verdict: &Verdict) {
        let probes: Vec<Value> = verdict.probes.iter().map(|p| json!({
            "path": p.path, "expectation": p.expectation, "status": p.status, "content_type": p.content_type,
            "cache_control": p.cache_control, "bytes_match": p.bytes_match, "verdict": p.verdict, "problems": p.problems,
        })).collect();
        self.with_receipt(|receipt| {
            receipt.insert("smoke".to_owned(), json!({"url": url, "state": verdict.state(), "attempts": verdict.attempts, "probes": probes}));
        });
    }

    fn persist(&mut self) -> Result<PathBuf, String> {
        let (Some(run), Some(receipt)) = (self.run.clone(), self.receipt.clone()) else {
            return Err("no run record".to_owned());
        };
        self.ports.host.persist(&run, &Value::Object(receipt))
    }

    // ---- report -------------------------------------------------------------------------

    fn finish(mut self) -> Report {
        let attempted = self
            .remote
            .as_ref()
            .is_some_and(|r| !matches!(r, RemoteOutcome::NotStarted(_)));
        for stage in Stage::ALL {
            if stage != Stage::FinalReceipt && !self.rows.iter().any(|(s, ..)| *s == stage) {
                let detail = match stage {
                    Stage::Readback | Stage::Smoke if attempted => {
                        "nothing to read back or request for this outcome"
                    }
                    _ => "not started: an earlier stage did not pass",
                };
                self.rows.push((stage, Status::NotRun, detail.to_owned()));
            }
        }
        let persisted = self.receipt.is_some().then(|| self.persist());
        match (&persisted, attempted) {
            (Some(Err(why)), true) => {
                let run = self
                    .run
                    .as_ref()
                    .map(|r| self.workspace.relative(&r.path))
                    .unwrap_or_default();
                self.rows.push((
                    Stage::FinalReceipt,
                    Status::Fail,
                    format!("the receipt could not be written: {why}"),
                ));
                self.notes.push(format!("RECOVERY: the remote outcome is real but receipt.json is stale or missing ({why}). Run directory: {run}. Record it by hand; do not rerun to regenerate it."));
            }
            (Some(Ok(_)), true) => self.rows.push((
                Stage::FinalReceipt,
                Status::Pass,
                "observed outcome recorded".to_owned(),
            )),
            _ => self.rows.push((
                Stage::FinalReceipt,
                Status::NotRun,
                "no remote write was attempted; the failure above is kept in the run record"
                    .to_owned(),
            )),
        }
        let mut report = Report::new("deploy cloudflare");
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
            for line in outcome_notes(outcome, self.request.environment) {
                report.note(line);
            }
        }
        if !self.domains.is_empty() {
            report.note(format!(
                "project domains (read-only observation): {}",
                self.domains.join(", ")
            ));
        }
        for note in self.notes {
            report.note(note);
        }
        report.note("NOT_RUN: browser/API checks, custom-domain binding and cutover are separate acceptance steps (docs/deployment/cloudflare-pages.md)");
        report
    }
}

pub const RECONCILE: &str = "Read the project again (`wrangler pages deployment list --project-name <project> --json`, or the dashboard) and compare branch, commit and time before any retry. Never replay the deployment blindly; this command never rolls back or deletes a deployment.";

fn parse_environment(text: &str, fallback: Environment) -> Environment {
    match text {
        "production" => Environment::Production,
        "preview" => Environment::Preview,
        _ => fallback,
    }
}

fn describe(outcome: &RemoteOutcome) -> (Status, String) {
    match outcome {
        RemoteOutcome::NotStarted(why) => (Status::Fail, format!("Wrangler did not start, so nothing was uploaded: {why}")),
        RemoteOutcome::Confirmed { release } => (
            Status::Pass,
            format!("deployment {} ({}) at {}", release.deployment_id, release.environment, release.url),
        ),
        RemoteOutcome::Mismatch { release, problems } => (
            Status::Fail,
            format!("deployment {} exists at {} but is not what was requested: {}", release.deployment_id, release.url, problems.join("; ")),
        ),
        RemoteOutcome::Rejected { kind, detail } => (
            Status::Fail,
            format!("Cloudflare refused ({kind:?}) and reported no deployment: {detail}"),
        ),
        RemoteOutcome::Unknown { reason } => (
            Status::Fail,
            format!("UNKNOWN: {reason}. The deployment may exist; read the project back before any retry"),
        ),
    }
}

fn outcome_notes(outcome: &RemoteOutcome, environment: Environment) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(release) = outcome.release() {
        notes.push(format!("URL: {}", release.url));
        notes.push(format!(
            "deployment ID: {} (keep it to find this deployment in the dashboard)",
            release.deployment_id
        ));
        if let Some(alias) = &release.alias {
            notes.push(format!("branch alias: {alias}"));
        }
        if release.environment == "preview" {
            notes.push("this preview URL is PUBLIC and does not isolate its backend; it uses the API origin built into the artifact".to_owned());
        }
    }
    match outcome {
        RemoteOutcome::Unknown { .. } => notes.push(format!("RECONCILE: {RECONCILE}")),
        RemoteOutcome::Mismatch { .. } => notes.push("the deployment is not rolled back or deleted automatically; decide in the dashboard".to_owned()),
        RemoteOutcome::Confirmed { .. } if environment == Environment::Production => {
            notes.push("production deployment accepted; a custom domain is served only if it is already bound to the project".to_owned())
        }
        _ => {}
    }
    notes
}

fn diagnostics(result: &Result<crate::capture::Captured, CaptureError>) -> Vec<String> {
    let text = match result {
        Ok(captured) => format!("{}\n{}", captured.stdout, captured.stderr),
        Err(error) => error
            .partial()
            .map(|p| format!("{}\n{}", p.stdout, p.stderr))
            .unwrap_or_default(),
    };
    wrangler::redact(&text)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(8)
        .map(|line| line.chars().take(240).collect())
        .collect::<Vec<String>>()
        .into_iter()
        .rev()
        .collect()
}

fn short(hex: &str) -> &str {
    &hex[..hex.len().min(12)]
}

fn printable(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(60).collect()
}

fn summarize(problems: &[String]) -> String {
    let shown: Vec<_> = problems.iter().take(4).cloned().collect();
    let more = problems.len().saturating_sub(shown.len());
    if more > 0 {
        format!("{}; +{more} more", shown.join("; "))
    } else {
        shown.join("; ")
    }
}

fn summarize_findings(findings: &[Finding]) -> String {
    let all: Vec<String> = findings.iter().map(ToString::to_string).collect();
    summarize(&all)
}

#[cfg(test)]
mod tests;
