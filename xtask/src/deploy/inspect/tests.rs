use super::*;
use crate::{
    cli::{Cli, Task},
    tools::testing::FakeTools,
};
use clap::Parser;

fn args() -> HostingSmokeArgs {
    HostingSmokeArgs {
        target: HostedApp::CantosTheatre,
        origin: "https://release.example".into(),
        api_origin: "https://api.example".into(),
        deployment: "production-123".into(),
        artifact: ArtifactArgs {
            release_manifest: Some("/missing/manifest.json".into()),
            release_digest: Some("a".repeat(64)),
            release_sha: Some("b".repeat(40)),
            local_artifact: true,
            ..ArtifactArgs::default()
        },
        dry_run: false,
    }
}

const PRIVATE_HEADERS: &str = "HTTP/2 200\r\nContent-Type: text/html\r\nCache-Control: no-cache, no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\n\r\n";

#[test]
fn request_requires_exact_origins_release_identity_and_one_provenance_mode() {
    assert_eq!(validate_request(&args()), Ok(()));
    for origin in [
        "http://release.example",
        "https://release.example/",
        "https://release.example/?code=private",
        "https://user:secret@release.example",
        "https://release.example#secret",
        "https://release.example:443",
        "https://",
        "https://Release.example",
    ] {
        let mut args = args();
        args.origin = origin.into();
        assert_eq!(
            validate_request(&args),
            Err("an exact lowercase HTTPS static origin without port/path/query is required"),
            "{origin}"
        );
    }
    for (origin, refusal) in [
        (
            "https://127.1",
            "a canonical HTTPS static origin is required",
        ),
        (
            "https://api.example",
            "a separate exact HTTPS API origin is required",
        ),
    ] {
        let mut args = args();
        args.origin = origin.into();
        assert_eq!(validate_request(&args), Err(refusal), "{origin}");
    }
    for bad_id in ["", "https://example/?code=secret", "id\nheader", "a/b"] {
        let mut args = args();
        args.deployment = bad_id.into();
        assert_eq!(
            validate_request(&args),
            Err("a nonsecret deployment ID of 1–128 safe characters is required")
        );
    }
    let mut missing = args();
    missing.artifact.release_manifest = None;
    assert_eq!(
        validate_request(&missing),
        Err("manifest, full digest/SHA and exactly one provenance mode are required")
    );
    let mut conflict = args();
    conflict.artifact.release_request = Some("request.json".into());
    assert_eq!(
        validate_request(&conflict),
        Err("manifest, full digest/SHA and exactly one provenance mode are required")
    );
    conflict.artifact.local_artifact = false;
    assert_eq!(validate_request(&conflict), Ok(()));
}

#[test]
fn only_the_final_response_can_supply_entry_privacy_and_duplicates_fail_closed() {
    assert!(entry_privacy(PRIVATE_HEADERS));
    for (old, new) in [
        ("no-store", "x-no-store"),
        ("no-referrer", "unsafe-url"),
        ("nosniff", "sniff"),
        ("DENY", "SAMEORIGIN"),
    ] {
        assert!(!entry_privacy(&PRIVATE_HEADERS.replace(old, new)), "{old}");
    }
    assert!(!entry_privacy(&format!(
        "{PRIVATE_HEADERS}HTTP/2 200\r\nContent-Type: text/html\r\n\r\n"
    )));
    assert!(!entry_privacy(&format!(
        "{}Referrer-Policy: unsafe-url\r\n\r\n",
        PRIVATE_HEADERS.strip_suffix("\r\n").unwrap()
    )));
    assert!(entry_privacy(&format!(
        "HTTP/1.1 200 Connection established\r\n\r\n{PRIVATE_HEADERS}"
    )));
}

#[test]
fn entry_privacy_ignores_trailers_and_uses_only_final_initial_headers() {
    let trailers = "Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\n\r\n";
    let unprotected = "HTTP/2 200\r\nContent-Type: text/html\r\nCache-Control: no-cache\r\n\r\n";
    assert!(
        !entry_privacy(&format!("{unprotected}{trailers}")),
        "trailers cannot establish first-response privacy"
    );
    assert!(entry_privacy(&format!(
        "{PRIVATE_HEADERS}Referrer-Policy: unsafe-url\r\n\r\n"
    )));
    assert!(entry_privacy(&format!(
        "HTTP/1.1 100 Continue\r\n\r\n{PRIVATE_HEADERS}"
    )));
    assert!(!entry_privacy(&format!(
        "HTTP/1.1 103 Early Hints\r\n{trailers}{unprotected}"
    )));
}

#[test]
fn probe_plan_covers_all_entry_assets_and_distinct_missing_suffixes_without_api_effects() {
    let manifest = bundle::Manifest {
        files: ["index.html", "app.js", "app_bg.wasm", "style.css"]
            .iter()
            .map(|path| bundle::ManifestFile {
                path: (*path).into(),
                size: 1,
                sha256: "a".repeat(64),
            })
            .collect(),
        entry_assets: vec!["app.js".into(), "app_bg.wasm".into(), "style.css".into()],
        total_bytes: 4,
        digest: "digest".into(),
    };
    let planned = probes(&manifest, "UNIQUE").unwrap();
    assert_eq!(
        planned.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(),
        [
            "/",
            "/app.js",
            "/app_bg.wasm",
            "/style.css",
            "/__cantos-smoke-UNIQUE.wasm",
            "/api/v1/__cantos-smoke-UNIQUE",
            "/__cantos-smoke-UNIQUE.js",
            "/__cantos-smoke-UNIQUE.css",
            "/__cantos-smoke-UNIQUE.png"
        ]
    );
    let empty = bundle::Manifest {
        files: vec![],
        ..manifest.clone()
    };
    assert_eq!(
        probes(&empty, "RUN"),
        Err("the bundle needs an entry page and at most 128 safe smoke paths")
    );
    let mut largest = manifest.clone();
    largest.entry_assets = (0..122).map(|index| format!("script-{index}.js")).collect();
    largest.files.extend(
        largest
            .entry_assets
            .iter()
            .map(|path| bundle::ManifestFile {
                path: path.clone(),
                size: 1,
                sha256: "a".repeat(64),
            }),
    );
    assert_eq!(probes(&largest, "RUN").unwrap().len(), 128);
    largest.entry_assets.push("one-too-many.js".into());
    largest.files.push(bundle::ManifestFile {
        path: "one-too-many.js".into(),
        size: 1,
        sha256: "a".repeat(64),
    });
    assert_eq!(
        probes(&largest, "RUN"),
        Err("the bundle needs an entry page and at most 128 safe smoke paths")
    );
    let unsafe_manifest = bundle::Manifest {
        entry_assets: vec!["../private.js".into()],
        files: vec![
            bundle::ManifestFile {
                path: "../private.js".into(),
                size: 1,
                sha256: "a".repeat(64),
            },
            manifest.files[0].clone(),
        ],
        ..manifest
    };
    assert_eq!(
        probes(&unsafe_manifest, "RUN"),
        Err("the bundle needs an entry page and at most 128 safe smoke paths")
    );
}

#[test]
fn dry_run_does_not_probe_tools_or_read_even_a_missing_artifact() {
    struct NoProbe;
    impl ToolProbe for NoProbe {
        fn available(&self, _: Tool) -> bool {
            panic!("dry-run probed a tool")
        }
    }
    let fixture = Fixture::new();
    let mut args = args();
    args.dry_run = true;
    assert_eq!(
        run(&fixture.workspace, &args, &NoProbe).unwrap(),
        ExitCode::SUCCESS
    );
    assert!(
        !fixture.root.join("artifacts").exists(),
        "dry-run wrote a run directory"
    );
    args.dry_run = false;
    assert_eq!(
        run(
            &fixture.workspace,
            &args,
            &FakeTools::without(&[Tool::Python3])
        )
        .unwrap(),
        ExitCode::FAILURE
    );
}

#[test]
fn cli_exposes_no_publish_account_or_credential_flags() {
    let argv = [
        "xtask",
        "hosting-smoke",
        "cantos-studio",
        "--origin",
        "https://static.example",
        "--api-origin",
        "https://api.example",
        "--deployment",
        "D",
        "--release-manifest",
        "manifest.json",
        "--release-digest",
        &"a".repeat(64),
        "--release-sha",
        &"b".repeat(40),
        "--local-artifact",
        "--dry-run",
    ];
    assert!(matches!(
        Cli::try_parse_from(argv).unwrap().command,
        Task::HostingSmoke(_)
    ));
    for flag in [
        "--publish",
        "--token",
        "--account",
        "--rollback",
        "--project",
    ] {
        assert_eq!(
            Cli::try_parse_from(argv.into_iter().chain([flag]))
                .unwrap_err()
                .kind(),
            clap::error::ErrorKind::UnknownArgument
        );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reply {
    Healthy,
    CorruptWasm,
    HtmlFallback,
    NoPrivacy,
    MissingCurl,
    Interrupt,
}

struct Fixture {
    root: std::path::PathBuf,
    workspace: Workspace,
}

impl Fixture {
    fn new() -> Self {
        let root = run_files::create_run(&std::env::temp_dir(), "cantos-smoke-tests", &[])
            .unwrap()
            .path;
        std::fs::create_dir(root.join("xtask")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        let workspace = Workspace::fixture(&root).unwrap();
        Self { root, workspace }
    }

    fn receipt(&self) -> Value {
        let path = std::fs::read_dir(self.root.join("artifacts/hosting-smoke"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            std::fs::read_dir(path.join("responses")).unwrap().count(),
            0,
            "no response body retained"
        );
        let bytes = std::fs::read(path.join("receipt.json")).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(
            !text.contains("PRIVATE_SENTINEL"),
            "raw response data escaped"
        );
        serde_json::from_str(&text).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

struct FixtureCapture {
    calls: Vec<crate::command::CommandSpec>,
    app: crate::config::App,
    reply: Reply,
    reject_artifact: bool,
}

impl FixtureCapture {
    fn new(reply: Reply) -> Self {
        Self {
            calls: vec![],
            app: crate::config::App::Theatre,
            reply,
            reject_artifact: false,
        }
    }
}

impl Capture for FixtureCapture {
    fn capture(
        &mut self,
        spec: &crate::command::CommandSpec,
        _: std::time::Duration,
    ) -> Result<crate::capture::Captured, CaptureError> {
        self.calls.push(spec.clone());
        let output = |code, stdout: String| {
            Ok(crate::capture::Captured {
                code,
                stdout,
                stderr: "PRIVATE_SENTINEL".into(),
            })
        };
        if spec.program == "python3" {
            assert_eq!(&spec.args[..2], ["scripts/release_pipeline.py", "extract"]);
            if self.reject_artifact {
                return output(1, "PRIVATE_SENTINEL".into());
            }
            let public = std::path::PathBuf::from(
                &spec.args[spec.args.iter().position(|s| s == "--output").unwrap() + 1],
            );
            std::fs::create_dir(&public).unwrap();
            let scan = bundle::testing::good_scan(self.app);
            for file in scan.files {
                let path = public.join(&file.path);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                let bytes = if file.path == "index.html"
                    || file.path == bundle::THEATRE_VERIFY_EMAIL_ENTRY
                {
                    scan.index_html.clone().unwrap()
                } else {
                    file.head
                };
                std::fs::write(path, bytes).unwrap();
            }
            return output(0, String::new());
        }
        assert_eq!(spec.program, "curl");
        assert!(
            spec.cwd.join("receipt.json").exists(),
            "intent must precede HTTP"
        );
        assert_eq!(spec.args[0], "--disable");
        assert!(!spec.args.iter().any(|a| matches!(
            a.as_str(),
            "--location" | "-L" | "--cookie" | "--user" | "--header"
        )));
        if self.reply == Reply::MissingCurl {
            return Err(CaptureError::Spawn {
                program: "curl".into(),
                message: "PRIVATE_SENTINEL".into(),
            });
        }
        if self.reply == Reply::Interrupt {
            return Err(CaptureError::Interrupted {
                partial: crate::capture::Partial::default(),
                started: true,
            });
        }
        let url = spec.args.last().unwrap();
        assert!(url.starts_with("https://release.example/"));
        let path = url.strip_prefix("https://release.example/").unwrap();
        // Pages serves `name.html` at `/name`.
        let page = path.is_empty() || path == "verify-email";
        let local = spec.cwd.join("public").join(match path {
            "" => "index.html".to_owned(),
            "verify-email" => bundle::THEATRE_VERIFY_EMAIL_ENTRY.to_owned(),
            other => other.to_owned(),
        });
        let body = &spec.args[spec.args.iter().position(|s| s == "--output").unwrap() + 1];
        let (bytes, headers) = if local.exists() {
            let mut bytes = std::fs::read(local).unwrap();
            if self.reply == Reply::CorruptWasm && path.ends_with(".wasm") {
                bytes.push(1);
            }
            let headers = if page {
                if self.reply == Reply::NoPrivacy {
                    PRIVATE_HEADERS.replace("no-referrer", "unsafe-url")
                } else {
                    PRIVATE_HEADERS.into()
                }
            } else {
                let mime = if path.ends_with(".wasm") {
                    "application/wasm"
                } else if path.ends_with(".css") {
                    "text/css"
                } else {
                    "text/javascript"
                };
                format!("HTTP/2 200\r\nContent-Type: {mime}\r\nCache-Control: max-age=0, must-revalidate\r\n\r\n")
            };
            (bytes, headers)
        } else if self.reply == Reply::HtmlFallback {
            (
                b"<html>PRIVATE_SENTINEL</html>".to_vec(),
                PRIVATE_HEADERS.into(),
            )
        } else {
            (
                b"PRIVATE_SENTINEL".to_vec(),
                "HTTP/2 404\r\nContent-Type: text/html\r\n\r\n".into(),
            )
        };
        std::fs::write(body, bytes).unwrap();
        output(0, format!("{headers}Set-Cookie: PRIVATE_SENTINEL\r\nLocation: https://example/?code=PRIVATE_SENTINEL\r\n"))
    }
}

#[test]
fn real_orchestration_compares_both_apps_and_exports_only_sanitized_observations() {
    for target in [HostedApp::CantosTheatre, HostedApp::CantosStudio] {
        let fixture = Fixture::new();
        let mut args = args();
        args.target = target;
        let mut capture = FixtureCapture::new(Reply::Healthy);
        capture.app = target.app();
        let report = observe(&fixture.workspace, &args, &mut capture).unwrap();
        assert_eq!(
            report.outcomes.iter().map(|o| o.status).collect::<Vec<_>>(),
            [Status::Pass, Status::Pass]
        );
        let receipt = fixture.receipt();
        assert_eq!(receipt["state"], "PASS");
        assert_eq!(receipt["source_sha"], "b".repeat(40));
        // Theatre adds its signup email action page (ADR 0048).
        let probes = if target == HostedApp::CantosTheatre {
            10
        } else {
            9
        };
        assert_eq!(receipt["probes"].as_array().unwrap().len(), probes);
        assert_eq!(capture.calls.len(), probes + 1);
    }
}

#[test]
fn corrupt_bytes_spa_fallback_and_missing_privacy_cannot_pass_http_acceptance() {
    for reply in [Reply::CorruptWasm, Reply::HtmlFallback, Reply::NoPrivacy] {
        let fixture = Fixture::new();
        let mut capture = FixtureCapture::new(reply);
        let report = observe(&fixture.workspace, &args(), &mut capture).unwrap();
        assert_eq!(
            report.outcomes.iter().map(|o| o.status).collect::<Vec<_>>(),
            [Status::Pass, Status::Fail]
        );
        assert_eq!(fixture.receipt()["state"], "INCOMPLETE");
        assert_eq!(capture.calls.len(), 11, "one observation round, no replay");
    }
}

#[test]
fn refused_artifact_missing_curl_and_interruption_stop_at_their_boundary() {
    for reply in [Reply::Healthy, Reply::MissingCurl, Reply::Interrupt] {
        let fixture = Fixture::new();
        let mut capture = FixtureCapture::new(reply);
        capture.reject_artifact = reply == Reply::Healthy;
        let report = observe(&fixture.workspace, &args(), &mut capture).unwrap();
        let expected = match reply {
            Reply::Healthy => vec![Status::Fail, Status::NotRun],
            Reply::MissingCurl => vec![Status::Pass, Status::Unsupported],
            _ => vec![Status::Pass, Status::Fail],
        };
        assert_eq!(
            report.outcomes.iter().map(|o| o.status).collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            capture.calls.len(),
            if capture.reject_artifact { 1 } else { 2 }
        );
        assert_eq!(fixture.receipt()["state"], "INCOMPLETE");
    }
}
