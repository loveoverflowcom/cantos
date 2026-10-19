//! The per-run execution receipt. Intent is recorded before the remote write and the observed
//! outcome after it; both are atomic replacements of one file in a private per-run directory, so
//! concurrent runs never share a record. It never holds tester addresses, signed links,
//! credentials or an SDK configuration.

use serde::Serialize;

use super::{
    apk::ApkFacts,
    firebase::{Release, RemoteOutcome},
    request::{LiveRequest, Policy},
};
use crate::report::Status;

/// 3: explicit real-only policy, module and audit/optimization status.
pub const SCHEMA_VERSION: u32 = 3;

#[derive(Debug, Clone, Serialize)]
pub struct Receipt {
    pub schema_version: u32,
    pub run_id: String,
    pub created_at: String,
    pub updated_at: String,
    /// `started`, `intent` (before the remote write) or `complete`.
    pub phase: &'static str,
    pub channel: &'static str,
    pub artifact_kind: &'static str,
    pub gradle: GradleRecord,
    pub source: Option<SourceRecord>,
    pub tools: ToolRecord,
    pub firebase: FirebaseRecord,
    pub artifact: Option<ArtifactRecord>,
    pub release_notes: Option<NotesRecord>,
    pub stages: Vec<StageRecord>,
    pub remote: RemoteRecord,
}

#[derive(Debug, Clone, Serialize)]
pub struct GradleRecord {
    pub module: &'static str,
    pub audit: bool,
    pub optimized: bool,
    pub variant: &'static str,
    pub task: &'static str,
    pub api_origin: Option<String>,
    pub native_issuer: Option<String>,
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
    pub gradle: Option<String>,
    pub java: Option<String>,
    pub android_build_tools: Option<String>,
    pub firebase_cli: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FirebaseRecord {
    pub app_id: String,
    pub app_id_source: &'static str,
    pub registered_package: Option<String>,
    pub groups: Vec<String>,
    pub tester_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactRecord {
    pub package: String,
    pub version_name: String,
    pub version_code: String,
    pub debuggable: bool,
    pub sha256: String,
    pub size_bytes: u64,
    pub signing_cert_sha256: String,
    pub expected_cert_sha256: Option<String>,
    pub signature_schemes: Vec<&'static str>,
    pub staged_file: String,
    /// The R8 mapping of a shrunk build, staged beside the APK; `None` for unshrunk variants.
    pub shrinker_mapping: Option<MappingRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MappingRecord {
    pub sha256: String,
    pub size_bytes: u64,
    pub staged_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotesRecord {
    pub source: String,
    pub sha256: String,
    pub bytes: usize,
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
    /// `not_attempted`, `write_pending`, `not_started`, `confirmed`, `rejected`, `partial`, `unknown`.
    pub state: &'static str,
    pub reason: Option<String>,
    pub failed_stage: Option<String>,
    pub release: Option<ReleaseRecord>,
    pub reconcile: Option<&'static str>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseRecord {
    pub display_version: String,
    pub build_version: String,
    pub upload_result: &'static str,
    pub firebase_console_uri: Option<String>,
    pub testing_uri: Option<String>,
}

pub const RECONCILE: &str = "Open the Firebase console release list for this app and check whether the release exists, \
its notes and its groups, before any replay: a repeated run can create or update a release and re-notify testers.";

impl Receipt {
    pub fn new(run_id: &str, now: &str, request: &LiveRequest) -> Self {
        let policy: Policy = request.policy;
        Self {
            schema_version: SCHEMA_VERSION,
            run_id: run_id.to_owned(),
            created_at: now.to_owned(),
            updated_at: now.to_owned(),
            phase: "started",
            channel: policy.channel(),
            artifact_kind: policy.artifact(),
            gradle: GradleRecord {
                module: ":androidApp",
                audit: false,
                optimized: policy.shrinks(),
                variant: policy.variant(),
                task: policy.gradle_task(),
                api_origin: request.api_origin.clone(),
                native_issuer: request.native_issuer.clone(),
            },
            source: None,
            tools: ToolRecord::default(),
            firebase: FirebaseRecord {
                app_id: request.app_id.as_str().to_owned(),
                app_id_source: request.app_id_source.label(),
                registered_package: None,
                groups: request.recipients.groups().to_vec(),
                tester_count: request.recipients.testers().len(),
            },
            artifact: None,
            release_notes: None,
            stages: Vec::new(),
            remote: RemoteRecord {
                state: "not_attempted",
                reason: None,
                failed_stage: None,
                release: None,
                reconcile: None,
                diagnostics: Vec::new(),
            },
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

    pub fn set_artifact(
        &mut self,
        facts: &ApkFacts,
        expected_cert: Option<&str>,
        staged_file: &str,
    ) {
        let signing = &facts.signing;
        self.artifact = Some(ArtifactRecord {
            package: facts.badging.package.clone(),
            version_name: facts.badging.version_name.clone(),
            version_code: facts.badging.version_code.clone(),
            debuggable: facts.badging.debuggable,
            sha256: facts.sha256.clone(),
            size_bytes: facts.size_bytes,
            signing_cert_sha256: signing.cert_sha256.clone(),
            expected_cert_sha256: expected_cert.map(str::to_owned),
            signature_schemes: [(signing.v1, "v1"), (signing.v2, "v2"), (signing.v3, "v3")]
                .into_iter()
                .filter_map(|(present, name)| present.then_some(name))
                .collect(),
            staged_file: staged_file.to_owned(),
            shrinker_mapping: None,
        });
    }

    /// Attach the staged R8 mapping to the recorded artifact (a no-op before an artifact exists).
    pub fn set_shrinker_mapping(&mut self, sha256: &str, size_bytes: u64, staged_file: &str) {
        if let Some(artifact) = &mut self.artifact {
            artifact.shrinker_mapping = Some(MappingRecord {
                sha256: sha256.to_owned(),
                size_bytes,
                staged_file: staged_file.to_owned(),
            });
        }
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

    pub fn set_outcome(&mut self, outcome: &RemoteOutcome, diagnostics: Vec<String>) {
        self.phase = "complete";
        self.remote.state = outcome.state();
        self.remote.reason = None;
        self.remote.failed_stage = None;
        self.remote.reconcile = None;
        self.remote.diagnostics = diagnostics;
        self.remote.release = outcome.release().map(release_record);
        match outcome {
            RemoteOutcome::NotStarted(why) => self.remote.reason = Some(why.clone()),
            RemoteOutcome::Confirmed { .. } => {
                self.remote.reason = Some(
                    "service accepted upload, notes and distribution; tester installation is not established"
                        .to_owned(),
                );
            }
            RemoteOutcome::Rejected { kind, detail } => {
                self.remote.reason = Some(format!("{kind}: {detail}"));
            }
            RemoteOutcome::Partial { failed, detail, .. } => {
                self.remote.failed_stage = Some(failed.to_string());
                self.remote.reason = Some(detail.clone());
                self.remote.reconcile = Some(RECONCILE);
            }
            RemoteOutcome::Unknown { reason, .. } => {
                self.remote.reason = Some(reason.to_string());
                self.remote.reconcile = Some(RECONCILE);
            }
        }
    }

    pub fn to_json(&self) -> Vec<u8> {
        let mut bytes =
            serde_json::to_vec_pretty(self).expect("a receipt of plain strings always serializes");
        bytes.push(b'\n');
        bytes
    }
}

fn release_record(release: &Release) -> ReleaseRecord {
    ReleaseRecord {
        display_version: release.display_version.clone(),
        build_version: release.build_version.clone(),
        upload_result: release.upload.label(),
        firebase_console_uri: release.console_uri.clone(),
        testing_uri: release.testing_uri.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distribute::{
        apk::{Badging, Signing},
        firebase::{FailedStage, UnknownReason, UploadResult},
        request::{AppId, AppIdSource, NotesChoice, Recipients},
    };

    fn request() -> LiveRequest {
        LiveRequest {
            api_origin: None,
            native_issuer: None,
            policy: Policy::DevelopmentApk,
            app_id: AppId::parse("1:000000000000:android:0123456789abcdef").unwrap(),
            app_id_source: AppIdSource::Flag,
            recipients: Recipients::new(
                &["cantos-internal".to_owned()],
                &["hidden.person@example.com".to_owned()],
            )
            .unwrap(),
            notes: NotesChoice::Generated,
            expected_cert: None,
        }
    }

    fn release() -> Release {
        Release {
            display_version: "0.1.0-dev".into(),
            build_version: "1".into(),
            upload: UploadResult::Created,
            console_uri: Some(
                "https://console.firebase.google.com/project/p/appdistribution/app/a/releases/1"
                    .into(),
            ),
            testing_uri: None,
        }
    }

    #[test]
    fn a_new_receipt_names_the_policy_variant_and_hides_tester_addresses() {
        let receipt = Receipt::new("run-1", "2026-10-15T00:00:00Z", &request());
        let json = String::from_utf8(receipt.to_json()).unwrap();
        assert!(json.contains("\"channel\": \"development\""));
        assert!(json.contains("\"variant\": \"debug\""));
        assert!(json.contains("\"task\": \":androidApp:assembleDebug\""));
        assert!(json.contains("\"tester_count\": 1"));
        assert!(json.contains("cantos-internal"));
        assert!(!json.contains("hidden.person"), "{json}");
        assert_eq!(receipt.remote.state, "not_attempted");
    }

    #[test]
    fn intent_precedes_the_outcome_and_flags_uncertainty_if_it_is_the_last_state() {
        let mut receipt = Receipt::new("run-1", "t", &request());
        receipt.mark_intent();
        assert_eq!(receipt.phase, "intent");
        assert_eq!(receipt.remote.state, "write_pending");
        assert!(receipt.remote.reconcile.is_some());
        receipt.set_outcome(&RemoteOutcome::Confirmed { release: release() }, vec![]);
        assert_eq!(receipt.phase, "complete");
        assert_eq!(receipt.remote.state, "confirmed");
        assert!(receipt.remote.reconcile.is_none());
        let record = receipt.remote.release.as_ref().unwrap();
        assert_eq!(record.upload_result, "created");
        assert!(record.firebase_console_uri.is_some());
    }

    #[test]
    fn partial_and_unknown_outcomes_keep_the_release_and_demand_reconciliation() {
        let mut partial = Receipt::new("r", "t", &request());
        partial.set_outcome(
            &RemoteOutcome::Partial {
                release: release(),
                failed: FailedStage::Distribution,
                detail: "failed to distribute".into(),
            },
            vec![],
        );
        assert_eq!(partial.remote.state, "partial");
        assert_eq!(
            partial.remote.failed_stage.as_deref(),
            Some("distribution to testers/groups")
        );
        assert!(partial.remote.release.is_some());
        assert_eq!(partial.remote.reconcile, Some(RECONCILE));

        let mut unknown = Receipt::new("r", "t", &request());
        unknown.set_outcome(
            &RemoteOutcome::Unknown {
                release: None,
                reason: UnknownReason::Interrupted,
            },
            vec!["line".into()],
        );
        assert_eq!(unknown.remote.state, "unknown");
        assert!(unknown.remote.release.is_none());
        assert_eq!(unknown.remote.diagnostics, ["line"]);
        assert_eq!(unknown.remote.reconcile, Some(RECONCILE));
    }

    #[test]
    fn artifact_identity_records_hash_version_and_signing_without_the_key() {
        let mut receipt = Receipt::new("r", "t", &request());
        let facts = ApkFacts {
            sha256: "ab".repeat(32),
            size_bytes: 25,
            badging: Badging {
                package: "com.cantos.theatre.dev".into(),
                version_code: "1".into(),
                version_name: "0.1.0-dev".into(),
                debuggable: true,
            },
            signing: Signing {
                signer_count: 1,
                cert_sha256: "cd".repeat(32),
                v1: true,
                v2: true,
                v3: false,
            },
        };
        receipt.set_artifact(
            &facts,
            Some(&"cd".repeat(32)),
            "artifact/androidApp-debug.apk",
        );
        let artifact = receipt.artifact.unwrap();
        assert_eq!(artifact.signature_schemes, ["v1", "v2"]);
        assert_eq!(
            artifact.expected_cert_sha256.as_deref(),
            Some("cd".repeat(32).as_str())
        );
        assert_eq!(artifact.size_bytes, 25);
    }

    #[test]
    fn the_qa_receipt_names_its_variant_non_debuggable_identity_and_staged_r8_mapping() {
        let mut qa_request = request();
        qa_request.policy = Policy::ReleaseApk;
        let mut receipt = Receipt::new("r", "t", &qa_request);
        let json = String::from_utf8(receipt.to_json()).unwrap();
        assert!(json.contains("\"channel\": \"release\""), "{json}");
        assert!(json.contains("\"variant\": \"release\""), "{json}");
        assert!(
            json.contains("\"task\": \":androidApp:assembleRelease\""),
            "{json}"
        );
        // A mapping cannot be attached before an artifact exists.
        receipt.set_shrinker_mapping(&"11".repeat(32), 7, "mapping.txt");
        assert!(receipt.artifact.is_none());
        let facts = ApkFacts {
            sha256: "ab".repeat(32),
            size_bytes: 25,
            badging: Badging {
                package: "com.cantos.theatre.dev".into(),
                version_code: "1".into(),
                version_name: "0.1.0".into(),
                debuggable: false,
            },
            signing: Signing {
                signer_count: 1,
                cert_sha256: "cd".repeat(32),
                v1: true,
                v2: true,
                v3: false,
            },
        };
        receipt.set_artifact(&facts, Some(&"cd".repeat(32)), "artifact/androidApp-qa.apk");
        assert!(receipt
            .artifact
            .as_ref()
            .unwrap()
            .shrinker_mapping
            .is_none());
        receipt.set_shrinker_mapping(&"11".repeat(32), 7, "mapping.txt");
        let json = String::from_utf8(receipt.to_json()).unwrap();
        assert!(json.contains("\"debuggable\": false"), "{json}");
        assert!(json.contains("\"shrinker_mapping\""), "{json}");
        assert!(json.contains(&"11".repeat(32)), "{json}");
        assert!(json.contains("\"schema_version\": 3"), "{json}");
        // The signing key and its passwords are never part of a receipt.
        assert!(!json.contains("PASSWORD"), "{json}");
    }

    #[test]
    fn stages_keep_the_xtask_vocabulary() {
        let mut receipt = Receipt::new("r", "t", &request());
        for (status, word) in [
            (Status::Pass, "PASS"),
            (Status::Fail, "FAIL"),
            (Status::NotRun, "NOT_RUN"),
            (Status::Unsupported, "UNSUPPORTED"),
        ] {
            receipt.record_stage("x", status, "", "t");
            assert_eq!(receipt.stages.last().unwrap().status, word);
        }
    }
}
