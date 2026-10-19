//! Flow tests: the real stage order, receipt and report against a fake host (scripted facts over a
//! real temporary directory) and scripted Wrangler/curl results.

use std::{collections::VecDeque, path::Path, time::Duration};

use super::*;
use crate::{
    capture::{Capture, Captured},
    command::CommandSpec,
    deploy::{
        bundle::{
            testing::{file, good_scan},
            Scan,
        },
        cloudflare::{
            request::{self, Raw},
            wrangler::fixtures::{deploy_entry, ID, SHA},
        },
        request::HostedApp,
        smoke::Expect,
    },
    tools::testing::FakeTools,
};

const ACCOUNT: &str = "8339afe44b4e8cd2b633a38a77f1db41";
const ARCHIVE_SHA: &str = "a3f1c3a2b0c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e";
const MAPPING: &str = r#"{"schema_version":1,"accounts":{"8339afe44b4e8cd2b633a38a77f1db41":{
  "theatre":{"project":"cantos-theatre","production_branch":"main"},
  "studio":{"project":"cantos-studio","production_branch":"main"}}}}"#;
const LISTING: &str = r#"[{"Project Name":"cantos-theatre","Project Domains":"cantos-theatre.pages.dev","Git Provider":"No","Last Modified":"x"},{"Project Name":"cantos-studio","Project Domains":"cantos-studio.pages.dev","Git Provider":"No","Last Modified":"x"}]"#;

type Log = std::rc::Rc<std::cell::RefCell<Vec<String>>>;

struct Capturer {
    log: Log,
    results: VecDeque<Result<Captured, CaptureError>>,
    calls: Vec<CommandSpec>,
}

impl Capturer {
    fn ok(&mut self, code: i32, stdout: &str, stderr: &str) -> &mut Self {
        self.results.push_back(Ok(Captured {
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }));
        self
    }
}

impl Capture for Capturer {
    fn capture(&mut self, spec: &CommandSpec, _: Duration) -> Result<Captured, CaptureError> {
        self.log.borrow_mut().push(format!(
            "capture {} {}",
            Path::new(&spec.program)
                .file_name()
                .unwrap()
                .to_string_lossy(),
            spec.args.first().cloned().unwrap_or_default()
        ));
        self.calls.push(spec.clone());
        self.results
            .pop_front()
            .expect("a scripted result for every capture")
    }
}

struct FakeHost {
    log: Log,
    root: std::path::PathBuf,
    app: crate::config::App,
    mapping: Result<Option<String>, String>,
    policy: Result<PolicyFiles, String>,
    token: bool,
    ambient: Option<String>,
    installed: bool,
    names: Vec<String>,
    manifest: Result<String, String>,
    archive: Result<(String, u64), String>,
    hazards: Vec<String>,
    extract: Result<artifact::Extracted, artifact::ArtifactError>,
    added: Vec<(String, Vec<u8>)>,
    scans: VecDeque<Result<Scan, String>>,
    hashes: VecDeque<Result<(String, u64), String>>,
    output: Option<String>,
    persisted: Vec<Value>,
    persist_fails_after: Option<usize>,
    fail_once_smoked: bool,
    pauses: usize,
    runs: usize,
}

fn policy_files() -> PolicyFiles {
    let read = |name: &str| {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../infra/cloudflare/theatre")
                .join(name),
        )
        .unwrap()
    };
    PolicyFiles {
        headers: read("_headers"),
        not_found: read("404.html"),
        redirects: None,
    }
}

fn release_manifest(component: &str, bytes: u64) -> String {
    json!({
        "schema_version": 1, "target": component, "source_sha": SHA,
        "api_origin": "https://api.cantos.cc", "deployed": false,
        "artifacts": [{"path": format!("{component}.tar.gz"), "bytes": bytes, "sha256": ARCHIVE_SHA}]
    })
    .to_string()
}

impl FakeHost {
    fn new(log: Log, app: crate::config::App) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("cantos-cf-flow-{}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let component = format!(
            "{}-web",
            if app == crate::config::App::Theatre {
                "theatre"
            } else {
                "studio"
            }
        );
        Self {
            log,
            root,
            app,
            mapping: Ok(Some(MAPPING.to_owned())),
            policy: Ok(policy_files()),
            token: true,
            ambient: None,
            installed: true,
            names: vec!["manifest.json".into(), format!("{component}.tar.gz")],
            manifest: Ok(release_manifest(&component, 10)),
            archive: Ok((ARCHIVE_SHA.into(), 10)),
            hazards: Vec::new(),
            extract: Ok(artifact::Extracted {
                files: 5,
                bytes: 100,
            }),
            added: Vec::new(),
            scans: VecDeque::new(),
            hashes: VecDeque::new(),
            output: None,
            persisted: Vec::new(),
            persist_fails_after: None,
            fail_once_smoked: false,
            pauses: 0,
            runs: 0,
        }
    }

    fn scan(&self) -> Scan {
        let mut scan = good_scan(self.app);
        for (name, bytes) in &self.added {
            scan.files.push(file(name, bytes));
        }
        scan
    }
}

impl Host for FakeHost {
    fn now(&mut self) -> String {
        "2026-10-19T10:00:00Z".to_owned()
    }
    fn read_mapping(&mut self) -> Result<Option<String>, String> {
        self.mapping.clone()
    }
    fn read_policy(&mut self, _: &str) -> Result<PolicyFiles, String> {
        self.policy.clone()
    }
    fn api_token_present(&self) -> bool {
        self.token
    }
    fn ambient_account(&self) -> Option<String> {
        self.ambient.clone()
    }
    fn wrangler_binary(&self) -> std::path::PathBuf {
        "/repo/infra/cloudflare/wrangler/node_modules/.bin/wrangler".into()
    }
    fn wrangler_installed(&self) -> bool {
        self.installed
    }
    fn list_artifact(&mut self, _: &Path) -> Result<Vec<String>, String> {
        Ok(self.names.clone())
    }
    fn read_manifest(&mut self, _: &Path) -> Result<String, String> {
        self.manifest.clone()
    }
    fn archive_facts(&mut self, _: &Path) -> Result<(String, u64), String> {
        self.archive.clone()
    }
    fn create_run(&mut self) -> Result<Run, String> {
        self.log.borrow_mut().push("create_run".to_owned());
        self.runs += 1;
        let path = self.root.join("run");
        std::fs::create_dir_all(path.join("public")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(path.join("out")).map_err(|e| e.to_string())?;
        Ok(Run {
            id: "RUN".to_owned(),
            path,
        })
    }
    fn wrangler_hazards(&mut self, _: &Run) -> Vec<String> {
        self.hazards.clone()
    }
    fn extract(
        &mut self,
        _: &Path,
        _: &Run,
        claim: &artifact::ArchiveClaim,
    ) -> Result<artifact::Extracted, artifact::ArtifactError> {
        assert_eq!(claim.sha256, ARCHIVE_SHA);
        assert_eq!(claim.bytes, 10);
        self.log.borrow_mut().push("extract".to_owned());
        self.extract.clone()
    }
    fn add_file(&mut self, _: &Run, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.added.push((name.to_owned(), bytes.to_vec()));
        Ok(())
    }
    fn scan_bundle(&mut self, _: &Run) -> Result<Scan, String> {
        self.log.borrow_mut().push("scan".to_owned());
        self.scans.pop_front().unwrap_or_else(|| Ok(self.scan()))
    }
    fn hash_file(&mut self, _: &Path) -> Result<(String, u64), String> {
        self.hashes
            .pop_front()
            .unwrap_or_else(|| Err("no body".into()))
    }
    fn read_output_file(&mut self, _: &Run) -> Option<String> {
        self.output.clone()
    }
    fn persist(&mut self, _: &Run, receipt: &Value) -> Result<std::path::PathBuf, String> {
        if self.fail_once_smoked && !receipt["smoke"].is_null() {
            return Err("disk full".to_owned());
        }
        if self
            .persist_fails_after
            .is_some_and(|limit| self.persisted.len() >= limit)
        {
            return Err("disk full".to_owned());
        }
        self.persisted.push(receipt.clone());
        Ok(self.root.join("run/receipt.json"))
    }
    fn pause(&mut self, _: Duration) {
        self.pauses += 1;
    }
}

type HostMutation = Box<dyn FnOnce(&mut FakeHost)>;

struct Rig {
    log: Log,
    host: FakeHost,
    capture: Capturer,
    tools: FakeTools,
    app: crate::config::App,
}

fn raw(app: HostedApp, environment: &str, branch: &str) -> Raw {
    let project = if app == HostedApp::CantosTheatre {
        "cantos-theatre"
    } else {
        "cantos-studio"
    };
    Raw {
        app,
        surface: Some(crate::build::Surface::Web),
        account: ACCOUNT.into(),
        project: project.into(),
        branch: branch.into(),
        environment: environment.into(),
        artifact: "artifacts/release-build/x/SHA".into(),
        dry_run: false,
    }
}

fn request_of(app: HostedApp, environment: &str, branch: &str) -> Request {
    request::resolve(&raw(app, environment, branch)).unwrap()
}

fn media_type(family: smoke::Family) -> &'static str {
    match family {
        smoke::Family::Script => "text/javascript",
        smoke::Family::Style => "text/css",
        smoke::Family::Wasm => "application/wasm",
    }
}

impl Rig {
    fn new(app: HostedApp) -> Self {
        let log: Log = Default::default();
        Self {
            host: FakeHost::new(log.clone(), app.app()),
            capture: Capturer {
                log: log.clone(),
                results: VecDeque::new(),
                calls: Vec::new(),
            },
            tools: FakeTools::all(),
            app: app.app(),
            log,
        }
    }

    /// Scripted for a complete run: version, project listing, deploy, readback, healthy smoke.
    fn complete(app: HostedApp, environment: &str, branch: &str) -> Self {
        let mut rig = Self::new(app);
        let project = if app == HostedApp::CantosTheatre {
            "cantos-theatre"
        } else {
            "cantos-studio"
        };
        let production_branch = "main";
        rig.host.output = Some(deploy_entry(project, environment, production_branch, SHA));
        rig.capture
            .ok(0, "4.151.2\n", "")
            .ok(0, LISTING, "")
            .ok(0, "Deployment complete", "");
        let env_word = if environment == "production" {
            "Production"
        } else {
            "Preview"
        };
        let listing = format!("[{{\"Id\":\"{ID}\",\"Environment\":\"{env_word}\",\"Branch\":\"{branch}\",\"Source\":\"0123456\"}}]");
        rig.capture.ok(0, &listing, "");
        rig.good_smoke_round();
        rig
    }

    fn probes(&self) -> Vec<smoke::Probe> {
        let manifest = bundle::validate_for(self.app, &self.host.scan(), Provider::Pages);
        // The scan has no policy files until they are added; use the same ones the run will add.
        let mut scan = good_scan(self.app);
        for (name, content) in policy_files().files() {
            scan.files.push(file(name, content.as_bytes()));
        }
        drop(manifest);
        smoke::probes(
            &bundle::validate_for(self.app, &scan, Provider::Pages).unwrap(),
            "RUN",
        )
    }

    fn good_smoke_round(&mut self) {
        for probe in self.probes() {
            let (headers, sha) = match &probe.expect {
                Expect::Page { sha256 } => ("HTTP/2 200\r\ncontent-type: text/html; charset=utf-8\r\ncache-control: no-cache, no-store\r\n\r\n".to_owned(), sha256.clone()),
                Expect::Asset { family, sha256 } => (format!("HTTP/2 200\r\ncontent-type: {}\r\ncache-control: public, max-age=0, must-revalidate\r\n\r\n", media_type(*family)), sha256.clone()),
                Expect::Absent => ("HTTP/2 404\r\ncontent-type: text/html\r\n\r\n".to_owned(), "not-compared".to_owned()),
            };
            self.capture.ok(0, &headers, "");
            self.host.hashes.push_back(Ok((sha, 1)));
        }
    }

    fn run(&mut self, request: &Request) -> Report {
        let workspace = Workspace::discover().unwrap();
        super::super::flow::run(
            &workspace,
            request,
            Ports {
                tools: &self.tools,
                capture: &mut self.capture,
                host: &mut self.host,
            },
        )
    }

    fn events(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    fn deploys(&self) -> usize {
        self.capture
            .calls
            .iter()
            .filter(|c| c.args.iter().any(|a| a == "deploy"))
            .count()
    }

    fn receipt(&self) -> &Value {
        self.host.persisted.last().expect("a persisted receipt")
    }
}

fn status_of(report: &Report, stage: Stage) -> Status {
    report
        .outcomes
        .iter()
        .find(|o| o.name == stage.label())
        .unwrap_or_else(|| panic!("{}", stage.label()))
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
fn a_complete_preview_run_validates_everything_then_writes_remotely_exactly_once() {
    let mut rig = Rig::complete(HostedApp::CantosStudio, "preview", "review-pages");
    rig.host.output = Some(deploy_entry("cantos-studio", "preview", "main", SHA));
    let report = rig.run(&request_of(
        HostedApp::CantosStudio,
        "preview",
        "review-pages",
    ));
    for stage in Stage::ALL {
        assert_eq!(
            status_of(&report, stage),
            Status::Pass,
            "{}: {}",
            stage.label(),
            detail_of(&report, stage)
        );
    }
    assert_eq!(rig.deploys(), 1);
    let events = rig.events();
    let at = |needle: &str| {
        events
            .iter()
            .position(|e| e.contains(needle))
            .unwrap_or_else(|| panic!("{needle} in {events:?}"))
    };
    assert!(at("--version") < at("create_run"));
    assert!(at("create_run") < at("extract"));
    assert!(
        at("extract") < at("capture wrangler pages"),
        "extraction precedes any provider call"
    );
    assert!(report.succeeded());
}

#[test]
fn the_receipt_binds_source_artifact_policy_destination_and_remote_deployment_without_secrets() {
    let mut rig = Rig::complete(HostedApp::CantosTheatre, "production", "main");
    let _ = rig.run(&request_of(HostedApp::CantosTheatre, "production", "main"));
    let receipt = rig.receipt();
    assert_eq!(receipt["account"], ACCOUNT);
    assert_eq!(receipt["project"], "cantos-theatre");
    assert_eq!(receipt["branch"], "main");
    assert_eq!(receipt["environment"], "production");
    assert_eq!(receipt["artifact"]["source_sha"], SHA);
    assert_eq!(receipt["artifact"]["archive"]["sha256"], ARCHIVE_SHA);
    assert_eq!(receipt["artifact"]["api_origin"], "https://api.cantos.cc");
    assert!(receipt["policy"]["digest"].as_str().unwrap().len() == 64);
    assert_eq!(receipt["tools"]["wrangler"], "4.151.2");
    assert_eq!(receipt["remote"]["state"], "confirmed");
    assert_eq!(receipt["remote"]["release"]["deployment_id"], ID);
    assert_eq!(receipt["smoke"]["state"], "pass");
    let names: Vec<&str> = receipt["bundle"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"_headers") && names.contains(&"404.html"),
        "policy files are in the final inventory: {names:?}"
    );
    let text = receipt.to_string();
    assert!(!text.contains("CLOUDFLARE_API_TOKEN") && !text.to_lowercase().contains("bearer"));
}

#[test]
fn wrong_destination_agreements_stop_before_any_run_file_or_provider_call() {
    for (environment, branch, needle) in [
        ("production", "feature/x", "would publish a preview"),
        ("preview", "main", "replace production"),
    ] {
        let mut rig = Rig::new(HostedApp::CantosTheatre);
        rig.capture.ok(0, "4.151.2", "");
        let report = rig.run(&request_of(HostedApp::CantosTheatre, environment, branch));
        assert_eq!(status_of(&report, Stage::Policy), Status::Fail);
        assert!(
            detail_of(&report, Stage::Policy).contains(needle)
                || detail_of(&report, Stage::Policy).contains("Pages would"),
            "{}",
            detail_of(&report, Stage::Policy)
        );
        assert!(rig.capture.calls.is_empty(), "no tool may start");
        assert_eq!(rig.host.runs, 0, "no run directory");
        assert_eq!(status_of(&report, Stage::Publish), Status::NotRun);
    }
}

#[test]
fn another_account_project_or_app_is_never_reached() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    let mut other = raw(HostedApp::CantosTheatre, "preview", "x");
    other.project = "cantos-studio".into();
    let report = rig.run(&request::resolve(&other).unwrap());
    assert!(detail_of(&report, Stage::Policy).contains("not the project mapped"));
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    let mut foreign = raw(HostedApp::CantosTheatre, "preview", "x");
    foreign.account = "1".repeat(32);
    let report = rig.run(&request::resolve(&foreign).unwrap());
    assert!(detail_of(&report, Stage::Policy).contains("is not in"));
    assert!(rig.capture.calls.is_empty());
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.host.ambient = Some("1".repeat(32));
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Prerequisites), Status::Fail);
}

#[test]
fn missing_tools_token_or_pinned_wrangler_are_unsupported_before_any_effect() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.tools = FakeTools::without(&[Tool::Node]);
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(
        status_of(&report, Stage::Prerequisites),
        Status::Unsupported
    );
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.host.token = false;
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert!(detail_of(&report, Stage::Prerequisites).contains("never logs in"));
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.host.installed = false;
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Wrangler), Status::Unsupported);
    assert!(detail_of(&report, Stage::Wrangler).contains("npm ci"));
    assert!(rig.capture.calls.is_empty());
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.capture.ok(0, "4.99.0\n", "");
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Wrangler), Status::Unsupported);
    assert_eq!(rig.host.runs, 0);
}

#[test]
fn an_invalid_policy_or_mapping_stops_before_the_artifact_is_read() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.host.policy = Ok(PolicyFiles {
        not_found: String::new(),
        ..policy_files()
    });
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Policy), Status::Fail);
    assert_eq!(status_of(&report, Stage::Artifact), Status::NotRun);
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.host.mapping = Ok(None);
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert!(detail_of(&report, Stage::Policy).contains("projects.example.json"));
}

#[test]
fn an_artifact_that_does_not_match_its_manifest_never_reaches_staging() {
    let cases: Vec<(&str, HostMutation)> = vec![
        ("hash", Box::new(|h| h.archive = Ok(("b".repeat(64), 10)))),
        (
            "size",
            Box::new(|h| h.archive = Ok((ARCHIVE_SHA.into(), 11))),
        ),
        ("extra file", Box::new(|h| h.names.push("notes.txt".into()))),
        (
            "other component",
            Box::new(|h| h.manifest = Ok(release_manifest("studio-web", 10))),
        ),
        (
            "missing manifest",
            Box::new(|h| h.manifest = Err("manifest.json is missing".into())),
        ),
    ];
    for (name, mutate) in cases {
        let mut rig = Rig::new(HostedApp::CantosTheatre);
        rig.capture.ok(0, "4.151.2", "");
        mutate(&mut rig.host);
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
        assert_eq!(status_of(&report, Stage::Artifact), Status::Fail, "{name}");
        assert_eq!(
            rig.host.runs, 0,
            "{name}: no staging before the artifact is verified"
        );
        assert_eq!(rig.deploys(), 0, "{name}");
    }
}

#[test]
fn inherited_wrangler_configuration_or_functions_halt_before_extraction_and_upload() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.capture.ok(0, "4.151.2", "");
    rig.host.hazards = vec!["/repo/wrangler.toml".into(), "/run/functions".into()];
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Staging), Status::Fail);
    assert!(detail_of(&report, Stage::Staging).contains("wrangler.toml"));
    assert!(!rig.events().contains(&"extract".to_owned()));
    assert_eq!(rig.deploys(), 0);
}

#[test]
fn an_unsafe_archive_halts_staging() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.capture.ok(0, "4.151.2", "");
    rig.host.extract = Err(artifact::ArtifactError::Unsafe("a `..` component".into()));
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Staging), Status::Fail);
    assert_eq!(status_of(&report, Stage::Bundle), Status::NotRun);
    assert_eq!(rig.deploys(), 0);
}

#[test]
fn an_archive_mismatch_during_extraction_stops_before_any_provider_request() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.capture.ok(0, "4.151.2", "");
    rig.host.extract = Err(artifact::ArtifactError::ArchiveMismatch(
        "SHA-256 differs from the manifest".into(),
    ));
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Staging), Status::Fail);
    assert_eq!(
        detail_of(&report, Stage::Staging),
        "archive does not match its manifest: SHA-256 differs from the manifest"
    );
    assert_eq!(status_of(&report, Stage::Bundle), Status::NotRun);
    assert_eq!(status_of(&report, Stage::Project), Status::NotRun);
    assert_eq!(
        rig.capture.calls.len(),
        1,
        "only the local version probe ran"
    );
    assert_eq!(rig.deploys(), 0);
}

#[test]
fn server_code_secrets_foreign_apps_and_oversized_assets_are_never_uploaded() {
    let build = |extra: Vec<crate::deploy::bundle::FileFacts>| {
        move |host: &mut FakeHost| {
            let mut scan = good_scan(host.app);
            scan.files.extend(extra.clone());
            host.scans = [Ok(scan.clone()), Ok(scan)].into();
        }
    };
    let oversized = {
        let mut big = file("big.wasm", crate::deploy::bundle::testing::WASM);
        big.size = PAGES_MAX_FILE_BYTES + 1;
        big
    };
    let cases: Vec<(&str, HostMutation)> = vec![
        (
            "_worker.js",
            Box::new(build(vec![file("_worker.js", b"export default {}")])),
        ),
        ("env file", Box::new(build(vec![file(".env", b"KEY=1")]))),
        (
            "package.json",
            Box::new(build(vec![file("package.json", b"{}")])),
        ),
        (
            "functions dir",
            Box::new(build(vec![file("functions/api.js", b"x")])),
        ),
        (
            "_routes.json",
            Box::new(build(vec![file("_routes.json", b"{}")])),
        ),
        (
            "source map",
            Box::new(build(vec![file("app.js.map", b"{}")])),
        ),
        (
            "wrangler config",
            Box::new(build(vec![file("wrangler.toml", b"x")])),
        ),
        ("oversized asset", Box::new(build(vec![oversized]))),
    ];
    for (name, mutate) in cases {
        let mut rig = Rig::new(HostedApp::CantosTheatre);
        rig.capture.ok(0, "4.151.2", "");
        mutate(&mut rig.host);
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
        assert_eq!(
            status_of(&report, Stage::Bundle),
            Status::Fail,
            "{name}: {}",
            detail_of(&report, Stage::Bundle)
        );
        assert_eq!(rig.deploys(), 0, "{name}");
    }
}

#[test]
fn an_oversized_inventory_is_refused_before_any_provider_call() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.capture.ok(0, "4.151.2", "");
    let mut scan = good_scan(rig.app);
    // The shared sanity bound refuses first; the Pages bound is the stricter documented one.
    scan.truncated = true;
    rig.host.scans = [Ok(scan)].into();
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Bundle), Status::Fail);
}

#[test]
fn bytes_that_change_between_validation_and_the_write_are_never_uploaded() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.capture.ok(0, "4.151.2", "").ok(0, LISTING, "");
    // The policy files are added before the first scan; mimic that for both scans.
    let mut base = good_scan(rig.app);
    for (name, content) in policy_files().files() {
        base.files.push(file(name, content.as_bytes()));
    }
    let mut late = base.clone();
    late.files.push(file("late.txt", b"x"));
    rig.host.scans = [Ok(base), Ok(late)].into();
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::IntentReceipt), Status::Fail);
    assert!(detail_of(&report, Stage::IntentReceipt).contains("changed after validation"));
    assert_eq!(rig.deploys(), 0);
}

#[test]
fn a_missing_or_git_integrated_project_stops_before_the_write() {
    for (listing, needle) in [
        ("[]", "not in this account"),
        (
            r#"[{"Project Name":"cantos-theatre","Project Domains":"","Git Provider":"Yes"}]"#,
            "Git-integrated",
        ),
    ] {
        let mut rig = Rig::new(HostedApp::CantosTheatre);
        rig.capture.ok(0, "4.151.2", "").ok(0, listing, "");
        let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
        assert_eq!(status_of(&report, Stage::Project), Status::Fail);
        assert!(detail_of(&report, Stage::Project).contains(needle));
        assert_eq!(rig.deploys(), 0);
        assert_eq!(status_of(&report, Stage::IntentReceipt), Status::NotRun);
    }
}

#[test]
fn intent_is_recorded_before_the_write_and_a_failed_record_prevents_the_write() {
    let mut rig = Rig::complete(HostedApp::CantosTheatre, "preview", "x");
    rig.host.output = Some(deploy_entry("cantos-theatre", "preview", "main", SHA));
    let _ = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    let states: Vec<&str> = rig
        .host
        .persisted
        .iter()
        .filter_map(|r| r["remote"]["state"].as_str())
        .collect();
    let intent = states
        .iter()
        .position(|s| *s == "intent")
        .expect("intent persisted");
    let confirmed = states
        .iter()
        .position(|s| *s == "confirmed")
        .expect("confirmed persisted");
    assert!(intent < confirmed);

    let mut rig = Rig::complete(HostedApp::CantosTheatre, "preview", "x");
    // Every receipt write fails, including the intent record that must precede the write.
    rig.host.persist_fails_after = Some(0);
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::IntentReceipt), Status::Fail);
    assert_eq!(rig.deploys(), 0, "no write without a recorded intent");
}

#[test]
fn a_timeout_after_the_write_is_unknown_is_never_retried_and_names_how_to_read_it_back() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.capture.ok(0, "4.151.2", "").ok(0, LISTING, "");
    rig.capture.results.push_back(Err(CaptureError::TimedOut {
        after: Duration::from_secs(1),
        partial: Default::default(),
    }));
    let candidate = format!(
        "[{{\"Id\":\"{ID}\",\"Environment\":\"Preview\",\"Branch\":\"x\",\"Source\":\"0123456\"}}]"
    );
    rig.capture.ok(0, &candidate, "");
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Publish), Status::Fail);
    assert!(detail_of(&report, Stage::Publish).contains("UNKNOWN"));
    assert_eq!(rig.deploys(), 1, "the write is never replayed");
    assert_eq!(status_of(&report, Stage::Smoke), Status::NotRun);
    let receipt = rig.receipt();
    assert_eq!(receipt["remote"]["state"], "unknown");
    assert!(receipt["remote"]["reconcile"]
        .as_str()
        .unwrap()
        .contains("before any retry"));
    assert_eq!(receipt["remote"]["candidates"][0], ID);
    assert!(report.notes.iter().any(|n| n.starts_with("RECONCILE")));
}

#[test]
fn exit_zero_without_a_deployment_identity_is_unknown_not_success() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.capture
        .ok(0, "4.151.2", "")
        .ok(0, LISTING, "")
        .ok(
            0,
            "Deployment complete! https://0f6d9a52.cantos-theatre.pages.dev",
            "",
        )
        .ok(0, "[]", "");
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Publish), Status::Fail);
    assert_eq!(rig.receipt()["remote"]["state"], "unknown");
    assert_eq!(status_of(&report, Stage::Smoke), Status::NotRun);
    assert!(!report.succeeded());
}

#[test]
fn a_rejected_write_reports_the_refusal_without_a_deployment() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.capture.ok(0, "4.151.2", "").ok(0, LISTING, "").ok(
        1,
        "",
        "Authentication error [code: 10000]",
    );
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert!(detail_of(&report, Stage::Publish).contains("refused"));
    assert_eq!(rig.receipt()["remote"]["state"], "rejected");
    assert_eq!(rig.receipt()["remote"]["release"], Value::Null);
}

#[test]
fn a_preview_when_production_was_intended_is_a_failed_mismatch_that_keeps_its_identity() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.host.output = Some(deploy_entry("cantos-theatre", "preview", "develop", SHA));
    rig.capture
        .ok(0, "4.151.2", "")
        .ok(0, LISTING, "")
        .ok(0, "done", "");
    let listing = format!("[{{\"Id\":\"{ID}\",\"Environment\":\"Preview\",\"Branch\":\"main\",\"Source\":\"0123456\"}}]");
    rig.capture.ok(0, &listing, "");
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "production", "main"));
    assert_eq!(status_of(&report, Stage::Publish), Status::Fail);
    assert_eq!(rig.receipt()["remote"]["state"], "mismatch");
    assert_eq!(rig.receipt()["remote"]["release"]["deployment_id"], ID);
    assert_eq!(
        status_of(&report, Stage::Smoke),
        Status::NotRun,
        "a mismatched deployment is not smoked as accepted"
    );
    assert_eq!(rig.deploys(), 1);
}

#[test]
fn a_failed_smoke_is_a_separate_result_and_the_deployment_stays_known() {
    let mut rig = Rig::new(HostedApp::CantosTheatre);
    rig.host.output = Some(deploy_entry("cantos-theatre", "preview", "main", SHA));
    rig.capture
        .ok(0, "4.151.2", "")
        .ok(0, LISTING, "")
        .ok(0, "done", "");
    let listing = format!(
        "[{{\"Id\":\"{ID}\",\"Environment\":\"Preview\",\"Branch\":\"x\",\"Source\":\"0123456\"}}]"
    );
    rig.capture.ok(0, &listing, "");
    // Every smoke attempt answers 200 text/html for everything, including the absence probes.
    for _ in 0..(rig.probes().len() * smoke::ATTEMPTS as usize) {
        rig.capture.ok(
            0,
            "HTTP/2 200\r\ncontent-type: text/html\r\ncache-control: no-cache\r\n\r\n",
            "",
        );
        rig.host.hashes.push_back(Ok(("x".into(), 1)));
    }
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Publish), Status::Pass);
    assert_eq!(status_of(&report, Stage::Smoke), Status::Fail);
    assert!(detail_of(&report, Stage::Smoke).contains("did not serve the bundle as built"));
    assert_eq!(rig.receipt()["remote"]["state"], "confirmed");
    assert_eq!(rig.receipt()["smoke"]["state"], "fail");
    assert_eq!(rig.host.pauses, smoke::ATTEMPTS as usize - 1);
    assert!(!report.succeeded());
}

#[test]
fn an_unwritable_final_receipt_after_a_real_write_points_at_recovery() {
    let mut rig = Rig::complete(HostedApp::CantosTheatre, "preview", "x");
    rig.host.output = Some(deploy_entry("cantos-theatre", "preview", "main", SHA));
    // Every write up to and including the publish succeeds; the record that follows the smoke does not.
    rig.host.fail_once_smoked = true;
    let report = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    assert_eq!(status_of(&report, Stage::Publish), Status::Pass);
    assert_eq!(status_of(&report, Stage::FinalReceipt), Status::Fail);
    assert!(report.notes.iter().any(|n| n.starts_with("RECOVERY")));
}

#[test]
fn every_wrangler_invocation_is_isolated_to_the_run_and_the_requested_account() {
    let mut rig = Rig::complete(HostedApp::CantosTheatre, "preview", "x");
    rig.host.output = Some(deploy_entry("cantos-theatre", "preview", "main", SHA));
    let _ = rig.run(&request_of(HostedApp::CantosTheatre, "preview", "x"));
    let wrangler_calls: Vec<&CommandSpec> = rig
        .capture
        .calls
        .iter()
        .filter(|c| c.program.ends_with("/wrangler"))
        .collect();
    assert!(wrangler_calls.len() >= 3);
    for call in &wrangler_calls[1..] {
        assert!(
            call.cwd.ends_with("run"),
            "cwd is the run directory, never the repository root: {:?}",
            call.cwd
        );
        assert!(call
            .env
            .iter()
            .any(|(k, v)| k == "CLOUDFLARE_ACCOUNT_ID" && v == ACCOUNT));
        assert!(call
            .env_remove
            .iter()
            .any(|k| k == "CLOUDFLARE_API_BASE_URL"));
    }
    assert!(!rig
        .capture
        .calls
        .iter()
        .any(|c| c.program == "npx" || c.program == "npm"));
}

#[test]
fn studio_deploys_independently_and_never_names_theatre() {
    let mut rig = Rig::complete(HostedApp::CantosStudio, "preview", "x");
    rig.host.output = Some(deploy_entry("cantos-studio", "preview", "main", SHA));
    let report = rig.run(&request_of(HostedApp::CantosStudio, "preview", "x"));
    assert_eq!(
        status_of(&report, Stage::Publish),
        Status::Pass,
        "{}",
        detail_of(&report, Stage::Publish)
    );
    let all = rig
        .capture
        .calls
        .iter()
        .map(|c| c.args.join(" "))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(all.contains("cantos-studio") && !all.contains("cantos-theatre"));
}
