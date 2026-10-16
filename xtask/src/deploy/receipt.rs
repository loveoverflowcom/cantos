//! The per-run deploy receipt. Intent is recorded before the remote write and the observed outcome
//! after it; both are atomic replacements of one file in a private per-run directory, so
//! concurrent runs never share a record. It carries source, tool, configuration and bundle
//! identity, the project, site, target and channel, the release identity the CLI actually reported
//! and the stage outcomes. It holds no credential, token, account address or Firebase
//! configuration beyond the digests of the effective files.

use serde::Serialize;

use super::{
    bundle::Manifest,
    firebase::{Released, RemoteOutcome},
    smoke::Verdict,
};
use crate::report::Status;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct Receipt {
    pub schema_version: u32,
    pub run_id: String,
    pub created_at: String,
    pub updated_at: String,
    /// `started`, `intent` (before the remote write) or `complete`.
    pub phase: &'static str,
    pub app: &'static str,
    /// The Hosting deploy target (`theatre`/`studio`), not a site ID.
    pub target: &'static str,
    pub project: String,
    /// The Hosting site the operator mapped to the target, once resolved.
    pub site: Option<String>,
    /// `live` or the preview channel ID.
    pub channel: String,
    pub expires: Option<String>,
    pub source: Option<SourceRecord>,
    pub tools: ToolRecord,
    pub config: Option<ConfigRecord>,
    pub build: Option<BuildRecord>,
    pub bundle: Option<BundleRecord>,
    pub stages: Vec<StageRecord>,
    pub remote: RemoteRecord,
    pub smoke: Option<SmokeRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRecord {
    pub commit: String,
    pub dirty: bool,
    /// Present only for a dirty tree: a digest of the status, tracked diff and untracked files.
    pub content_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ToolRecord {
    pub cargo: Option<String>,
    pub trunk: Option<String>,
    pub firebase_cli: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigRecord {
    /// SHA-256 of the tracked policy file as read.
    pub policy_sha256: String,
    /// SHA-256 of the effective `firebase.json` and `.firebaserc` written for this run.
    pub effective_firebase_json_sha256: String,
    pub effective_firebaserc_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildRecord {
    pub command: String,
    pub dist: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BundleRecord {
    pub file_count: usize,
    pub total_bytes: u64,
    pub manifest_sha256: String,
    pub entry_assets: Vec<String>,
    pub files: Vec<FileRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileRecord {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageRecord {
    pub name: &'static str,
    pub status: &'static str,
    pub detail: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteRecord {
    /// `not_attempted`, `write_pending`, `not_started`, `confirmed`, `rejected` or `unknown`.
    pub state: &'static str,
    pub reason: Option<String>,
    pub release: Option<ReleaseRecord>,
    pub reconcile: Option<&'static str>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseRecord {
    pub channel: String,
    pub site: String,
    pub version: Option<String>,
    pub url: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SmokeRecord {
    pub url: String,
    /// `pass`, `fail` or `unavailable`.
    pub state: &'static str,
    pub attempts: u32,
    pub probes: Vec<ProbeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProbeRecord {
    pub path: String,
    pub expectation: String,
    pub status: Option<u16>,
    pub content_type: Option<String>,
    pub cache_control: Option<String>,
    /// Whether the served bytes equal the bundle's file; `None` when no body was compared.
    pub bytes_match: Option<bool>,
    pub verdict: &'static str,
    pub problems: Vec<String>,
}

pub const RECONCILE: &str = "Open the Firebase console Hosting page for this site and check the channel and release history \
(and the version named in this receipt, if any) before any replay. A rerun uploads the same bytes again (unchanged files are \
deduplicated) and creates a new release or re-times the preview channel; it never restores an earlier release. To return a \
live site to an earlier release use the console's rollback.";

impl Receipt {
    pub fn new(run_id: &str, now: &str, request: &super::request::Request) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            run_id: run_id.to_owned(),
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
            phase: "started",
            app: request.app.cli_name(),
            target: request.target(),
            project: request.project.as_str().to_owned(),
            site: None,
            channel: request.channel.label().to_owned(),
            expires: request.expires.as_ref().map(|e| e.as_arg().to_owned()),
            source: None,
            tools: ToolRecord::default(),
            config: None,
            build: None,
            bundle: None,
            stages: Vec::new(),
            remote: RemoteRecord {
                state: "not_attempted",
                reason: None,
                release: None,
                reconcile: None,
                diagnostics: Vec::new(),
            },
            smoke: None,
        }
    }

    pub fn record_stage(&mut self, name: &'static str, status: Status, detail: &str, at: &str) {
        self.stages.push(StageRecord {
            name,
            status: match status {
                Status::Pass => "PASS",
                Status::Fail => "FAIL",
                Status::NotRun => "NOT_RUN",
                Status::Unsupported => "UNSUPPORTED",
            },
            detail: detail.to_owned(),
            at: at.to_owned(),
        });
    }

    pub fn set_bundle(&mut self, manifest: &Manifest) {
        self.bundle = Some(BundleRecord {
            file_count: manifest.files.len(),
            total_bytes: manifest.total_bytes,
            manifest_sha256: manifest.digest.clone(),
            entry_assets: manifest.entry_assets.clone(),
            files: manifest
                .files
                .iter()
                .map(|file| FileRecord {
                    path: file.path.clone(),
                    size: file.size,
                    sha256: file.sha256.clone(),
                })
                .collect(),
        });
    }

    /// Intent is recorded before the write, so a lost response is reconcilable from the file.
    pub fn mark_intent(&mut self) {
        self.phase = "intent";
        self.remote.state = "write_pending";
        self.remote.reason = Some(
            "recorded before the remote write; if this is the last state, the outcome is unknown"
                .to_owned(),
        );
        self.remote.reconcile = Some(RECONCILE);
    }

    /// Undo [`Receipt::mark_intent`] when the intent could not be persisted: nothing was written
    /// remotely, so a later successful write of this receipt must not claim a pending write.
    pub fn unmark_intent(&mut self) {
        self.phase = "started";
        self.remote.state = "not_attempted";
        self.remote.reason = None;
        self.remote.reconcile = None;
    }

    pub fn set_outcome(&mut self, outcome: &RemoteOutcome, diagnostics: Vec<String>) {
        self.phase = "complete";
        self.remote.state = outcome.state();
        self.remote.reason = None;
        self.remote.reconcile = None;
        self.remote.diagnostics = diagnostics;
        self.remote.release = outcome.release().map(release_record);
        match outcome {
            RemoteOutcome::NotStarted(why) => self.remote.reason = Some(why.clone()),
            RemoteOutcome::Confirmed { .. } => {
                self.remote.reason = Some(
                    "Hosting accepted the release; browser behavior, the API origin and sign-in are not established"
                        .to_owned(),
                );
            }
            RemoteOutcome::Rejected { kind, detail } => {
                self.remote.reason = Some(format!("{kind}: {detail}"));
                self.remote.reconcile = Some(RECONCILE);
            }
            RemoteOutcome::Unknown { reason } => {
                self.remote.reason = Some(reason.to_string());
                self.remote.reconcile = Some(RECONCILE);
            }
        }
    }

    pub fn set_smoke(&mut self, url: &str, verdict: &Verdict) {
        self.smoke = Some(SmokeRecord {
            url: url.to_owned(),
            state: verdict.state(),
            attempts: verdict.attempts,
            probes: verdict.probes.clone(),
        });
    }

    pub fn to_json(&self) -> Vec<u8> {
        let mut bytes =
            serde_json::to_vec_pretty(self).expect("a receipt of plain strings always serializes");
        bytes.push(b'\n');
        bytes
    }
}

fn release_record(release: &Released) -> ReleaseRecord {
    ReleaseRecord {
        channel: release.channel.clone(),
        site: release.site.clone(),
        version: release.version.clone(),
        url: release.url.clone(),
        expires_at: release.expires_at.clone(),
    }
}
