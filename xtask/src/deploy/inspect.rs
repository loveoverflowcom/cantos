//! Read-only release acceptance, independent of any publisher. The release contract owns
//! provenance/extraction; the existing static-byte judge owns MIME/cache/absence decisions.
//! No response body, raw header, stderr, cookie or credential enters the exported receipt.

use std::{collections::BTreeMap, process::ExitCode};

use anyhow::Result;
use clap::Args;
use serde_json::{json, Value};

use super::{bundle, host, request::HostedApp, smoke};
use crate::{
    capture::{Capture, CaptureError, SystemCapture},
    release::ArtifactArgs,
    report::{Report, Status},
    run_files,
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

#[derive(Debug, Args)]
pub struct HostingSmokeArgs {
    #[arg(value_enum)]
    pub target: HostedApp,
    /// Exact HTTPS static origin, copied from provider readback or the approved final domain.
    #[arg(long)]
    pub origin: String,
    /// Expected public API origin in the release manifest; no API request is made.
    #[arg(long)]
    pub api_origin: String,
    /// Nonsecret deployment ID from provider readback (recorded as an operator declaration).
    #[arg(long)]
    pub deployment: String,
    #[command(flatten)]
    pub artifact: ArtifactArgs,
    /// Pure plan: no record/artifact read, tool probe, subprocess, network or output file.
    #[arg(long)]
    pub dry_run: bool,
}

fn validate_request(args: &HostingSmokeArgs) -> Result<(), &'static str> {
    let Some(origin) = smoke::url_for(&args.origin, "/") else {
        return Err("an exact lowercase HTTPS static origin without port/path/query is required");
    };
    if url::Url::parse(&origin).map(|url| url.origin().ascii_serialization())
        != Ok(args.origin.clone())
    {
        return Err("a canonical HTTPS static origin is required");
    }
    if host::checked_api_origin(Some(&args.api_origin)).is_err() || args.origin == args.api_origin {
        return Err("a separate exact HTTPS API origin is required");
    }
    if args.deployment.is_empty()
        || args.deployment.len() > 128
        || !args
            .deployment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err("a nonsecret deployment ID of 1–128 safe characters is required");
    }
    let artifact = &args.artifact;
    if artifact.release_manifest.is_none()
        || !hex_identity(artifact.release_digest.as_deref(), 64)
        || !hex_identity(artifact.release_sha.as_deref(), 40)
        || artifact.local_artifact == artifact.release_request.is_some()
    {
        return Err("manifest, full digest/SHA and exactly one provenance mode are required");
    }
    Ok(())
}

fn hex_identity(value: Option<&str>, length: usize) -> bool {
    value.is_some_and(|v| {
        v.len() == length
            && v.bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    })
}

fn probes(manifest: &bundle::Manifest, id: &str) -> Result<Vec<smoke::Probe>, &'static str> {
    let mut probes = smoke::probes(manifest, id);
    for suffix in ["js", "css", "png"] {
        probes.push(smoke::Probe {
            path: format!("/__cantos-smoke-{id}.{suffix}"),
            expect: smoke::Expect::Absent,
        });
    }
    if !probes
        .iter()
        .any(|probe| matches!(probe.expect, smoke::Expect::Page { .. }))
        || probes.len() > 128
        || probes.iter().any(|probe| {
            probe
                .path
                .split('/')
                .any(|segment| matches!(segment, "." | ".."))
                || smoke::url_for("https://static.example", &probe.path).is_none()
        })
    {
        return Err("the bundle needs an entry page and at most 128 safe smoke paths");
    }
    Ok(probes)
}

/// Only the final HTTP response contributes facts (proxy CONNECT/1xx blocks are discarded).
/// Duplicate header fields are combined, so a permissive duplicate cannot be hidden.
fn entry_privacy(dump: &str) -> bool {
    let mut headers = BTreeMap::<String, String>::new();
    for line in dump.lines() {
        if line.starts_with("HTTP/") {
            headers.clear();
        } else if let Some((name, value)) = line.split_once(':') {
            headers
                .entry(name.trim().to_ascii_lowercase())
                .and_modify(|previous| {
                    previous.push(',');
                    previous.push_str(&value.trim().to_ascii_lowercase());
                })
                .or_insert_with(|| value.trim().to_ascii_lowercase());
        }
    }
    headers
        .get("cache-control")
        .is_some_and(|value| value.split(',').any(|v| v.trim() == "no-store"))
        && headers
            .get("referrer-policy")
            .is_some_and(|v| v == "no-referrer")
        && headers
            .get("x-content-type-options")
            .is_some_and(|v| v == "nosniff")
        && headers.get("x-frame-options").is_some_and(|v| v == "deny")
}

pub fn run(
    workspace: &Workspace,
    args: &HostingSmokeArgs,
    tools: &dyn ToolProbe,
) -> Result<ExitCode> {
    if let Err(why) = validate_request(args) {
        eprintln!("hosting-smoke: {why}");
        return Ok(ExitCode::from(2));
    }
    if args.dry_run {
        println!("hosting-smoke: verify manifest v2/provenance and extract locally; compare entry JS/CSS/WASM and absence paths at {}; record sanitized observations only; no publish/API/auth/browser/recovery acceptance", args.origin);
        return Ok(ExitCode::SUCCESS);
    }
    if !tools.available(Tool::Python3) {
        let mut report = Report::new("hosting-smoke");
        report.record(
            "artifact",
            Status::Unsupported,
            "python3 is required for the release contract",
        );
        report.record(
            "static-http",
            Status::NotRun,
            "artifact validation could not start",
        );
        report.print();
        return Ok(report.exit_code());
    }
    let mut capture = SystemCapture::new()?;
    let report = observe(workspace, args, &mut capture)?;
    report.print();
    Ok(report.exit_code())
}

fn observe(
    workspace: &Workspace,
    args: &HostingSmokeArgs,
    capture: &mut dyn Capture,
) -> Result<Report> {
    let run = run_files::create_run(workspace.root(), "artifacts/hosting-smoke", &["responses"])
        .map_err(anyhow::Error::msg)?;
    let mut report = Report::new("hosting-smoke");
    report.note("HTTP observations only; deployment identity is operator-declared. API/browser/auth, recovery and retirement remain NOT_RUN. No publishing command is invoked.");
    let mut receipt = json!({
        "schema_version": 1, "kind": "read-only-http-observation", "observed_at": run_files::now(),
        "origin": args.origin, "api_origin_claim": args.api_origin,
        "deployment_declaration": args.deployment,
        "source_sha": args.artifact.release_sha, "manifest_sha256": args.artifact.release_digest,
        "state": "NOT_RUN", "probes": [],
        "not_covered": ["API", "browser", "auth", "fonts/images", "CSP/Permissions-Policy", "ETag/revalidation round trip", "rollback", "DNS", "retirement"]
    });
    let result = inspect(workspace, args, capture, &run, &mut receipt);
    match (receipt.get("bundle_sha256").is_some(), result) {
        (true, result) => {
            report.record(
                "artifact",
                Status::Pass,
                "release v2 contract and static bundle validation passed",
            );
            match result {
                Ok(()) => report.record(
                    "static-http",
                    Status::Pass,
                    "every selected static byte/MIME/cache/404 and entry privacy check passed",
                ),
                Err((status, why)) => report.record("static-http", status, why),
            }
        }
        (false, Err((status, why))) => {
            report.record("artifact", status, why);
            report.record(
                "static-http",
                Status::NotRun,
                "artifact validation did not pass; no HTTP request made",
            );
        }
        (false, Ok(())) => report.record(
            "artifact",
            Status::Fail,
            "missing validated bundle identity",
        ),
    }
    receipt["state"] = json!(if report.succeeded() {
        "PASS"
    } else {
        "INCOMPLETE"
    });
    receipt["stages"] = json!(report
        .outcomes
        .iter()
        .map(|outcome| json!({"name": outcome.name, "status": outcome.status.to_string()}))
        .collect::<Vec<_>>());
    let bytes = serde_json::to_vec_pretty(&receipt)?;
    let path =
        run_files::persist_atomic(&run, "receipt.json", &bytes).map_err(anyhow::Error::msg)?;
    report.note(format!("private receipt: {}", workspace.relative(&path)));
    Ok(report)
}

type Halt = (Status, &'static str);

fn inspect(
    workspace: &Workspace,
    args: &HostingSmokeArgs,
    capture: &mut dyn Capture,
    run: &run_files::RunDir,
    receipt: &mut Value,
) -> Result<(), Halt> {
    let component = format!(
        "{}-web",
        match args.target {
            HostedApp::CantosTheatre => "theatre",
            HostedApp::CantosStudio => "studio",
        }
    );
    let public = run.path.join("public");
    let extract = args
        .artifact
        .extract_command(workspace, &component, &args.api_origin, &public);
    match capture.capture(&extract, std::time::Duration::from_secs(60)) {
        Ok(output) if output.code == 0 => {}
        Err(CaptureError::Spawn { .. }) => {
            return Err((
                Status::Unsupported,
                "release validator could not start; no HTTP request made",
            ))
        }
        _ => {
            return Err((
                Status::Fail,
                "release contract/extraction failed; no HTTP request made (raw output withheld)",
            ))
        }
    }
    let scan = host::scan_directory(&public)
        .map_err(|_| (Status::Fail, "could not scan extracted artifact"))?;
    let manifest = bundle::validate_for(args.target.app(), &scan, bundle::Provider::Pages)
        .map_err(|_| (Status::Fail, "static bundle refused; no HTTP request made"))?;
    receipt["bundle_sha256"] = json!(manifest.digest);
    let probes = probes(&manifest, &run.id).map_err(|why| (Status::Fail, why))?;
    run_files::persist_atomic(
        run,
        "receipt.json",
        &serde_json::to_vec_pretty(receipt)
            .map_err(|_| (Status::Fail, "could not encode observation intent"))?,
    )
    .map_err(|_| {
        (
            Status::Fail,
            "could not persist intent; no HTTP request made",
        )
    })?;
    let mut records = Vec::new();
    let mut failed = false;
    for (index, probe) in probes.iter().enumerate() {
        let url =
            smoke::url_for(&args.origin, &probe.path).ok_or((Status::Fail, "unsafe smoke path"))?;
        let body = run.path.join("responses").join(format!("{index}.body"));
        run_files::create_private(&body)
            .map_err(|_| (Status::Fail, "could not prepare private response file"))?;
        let command = smoke::curl_command(&url, &body, &run.path);
        let output = capture.capture(&command, smoke::TIMEOUT);
        let hash = run_files::hash_path(&body).ok();
        // Even an unexpected private response is removed; only hashes and fixed checks survive.
        std::fs::remove_file(&body)
            .map_err(|_| (Status::Fail, "could not remove response body"))?;
        let output = match output {
            Err(CaptureError::Spawn { .. }) => {
                return Err((
                    Status::Unsupported,
                    "curl could not start; HTTP acceptance incomplete",
                ))
            }
            Err(CaptureError::Interrupted { .. }) => {
                return Err((
                    Status::Fail,
                    "HTTP observation interrupted; remaining paths were not requested",
                ))
            }
            output => output,
        };
        let privacy = !matches!(probe.expect, smoke::Expect::Page { .. })
            || output
                .as_ref()
                .is_ok_and(|output| entry_privacy(&output.stdout));
        let observation = output
            .ok()
            .filter(|o| o.code == 0)
            .and_then(|o| smoke::parse_headers(&o.stdout))
            .map(|response| smoke::Observed {
                response,
                body_sha256: hash.as_ref().map(|(hash, _)| hash.clone()),
            })
            .ok_or_else(|| "incomplete HTTP response".to_owned());
        let judged = smoke::judge(probe, observation);
        let passed = judged.verdict == "ok" && privacy;
        failed |= !passed;
        records.push(json!({"path": probe.path, "status": judged.status, "static_ok": judged.verdict == "ok",
            "entry_privacy_ok": matches!(probe.expect, smoke::Expect::Page { .. }).then_some(privacy),
            "bytes_match": judged.bytes_match, "decoded_sha256": hash.as_ref().map(|(hash, _)| hash),
            "decoded_bytes": hash.as_ref().map(|(_, bytes)| bytes), "passed": passed}));
        receipt["probes"] = json!(records);
        run_files::persist_atomic(
            run,
            "receipt.json",
            &serde_json::to_vec_pretty(receipt)
                .map_err(|_| (Status::Fail, "could not encode observations"))?,
        )
        .map_err(|_| (Status::Fail, "could not persist partial observations"))?;
    }
    if failed {
        Err((
            Status::Fail,
            "one or more static/entry privacy checks failed; see sanitized receipt",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
