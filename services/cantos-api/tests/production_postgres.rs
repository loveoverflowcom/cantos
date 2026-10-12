//! Production input boundary evidence, exclusively in a freshly created disposable cluster.
use std::{env, time::Duration};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use cantos_api::{
    contexts::production::production_catalog,
    interfaces::http::{router, AppState},
    platform::postgres::{local_config, migrate, token_hash, Store, StoreError},
    script_ir::read_script,
};
use cantos_contracts::{
    ApproveProductionRequest, CharacterCasting, FreezeProductionRequest, ProductionBudget,
    ProductionCurrency, ProductionFindingCode, ProductionRate, ProductionRightsDeclaration,
    ProductionRightsScope, ProductionRightsStatus, ProductionRightsSubject, ProductionSettings,
    ReviewRequest, SaveProductionRightsRequest, SaveProductionSettingsRequest, SaveRevisionRequest,
    SynthesisPerformance,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio_postgres::{Client, Config, NoTls};
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ORIGIN: &str = "http://127.0.0.1:8080";
const SCRIPT: &str =
    include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");

struct Harness {
    admin: Client,
    config: Config,
    store: Store,
    app: Router,
    script: String,
}

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

impl Harness {
    async fn new(review: bool) -> Self {
        let url = env::var("CANTOS_TEST_CLUSTER_URL").expect("use scripts/test_postgres.py");
        let mut config = local_config(&url).unwrap();
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let cluster = connect(&config).await;
        let database = format!("cantos_test_{}", Uuid::new_v4().simple());
        cluster
            .batch_execute(&format!("CREATE DATABASE {database}"))
            .await
            .unwrap();
        config.dbname(&database);
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        admin.batch_execute("GRANT USAGE ON SCHEMA public TO cantos_app; GRANT SELECT ON ALL TABLES IN SCHEMA public TO cantos_app; GRANT INSERT ON scripts,script_revisions,revision_evidence,script_evidence,script_reviews,script_review_operations,production_settings,production_rights_claims,production_snapshots,production_approvals TO cantos_app; GRANT UPDATE(head_revision) ON scripts TO cantos_app; GRANT UPDATE(revoked) ON sessions TO cantos_app; GRANT EXECUTE ON FUNCTION production_lock_actor(text),production_lock_member(text,text) TO cantos_app; INSERT INTO actors(id) VALUES('alice'),('bob');").await.unwrap();
        for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
            admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,$2,CURRENT_TIMESTAMP+interval '1 hour')", &[&token_hash(token),&actor]).await.unwrap();
        }
        for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
            admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice',$1,$2,'Original synthetic test evidence; no legal clearance')", &[&kind,&id]).await.unwrap();
        }
        let mut app_config = config.clone();
        app_config
            .user("cantos_app")
            .application_name("cantos-production-test");
        let store = Store::new(app_config).unwrap();
        let script = Uuid::new_v4().to_string();
        store
            .save(
                ALICE,
                &script,
                SaveRevisionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: 0,
                    script_json: SCRIPT.into(),
                },
            )
            .await
            .unwrap();
        if review {
            store
                .review(
                    ALICE,
                    &script,
                    ReviewRequest {
                        operation_id: Uuid::new_v4().to_string(),
                        revision: 1,
                    },
                )
                .await
                .unwrap();
        }
        let app = router(
            AppState {
                store: store.clone(),
                web_origin: ORIGIN.into(),
            },
            "apps/cantos-studio/web/dist",
        );
        Self {
            admin,
            config,
            store,
            app,
            script,
        }
    }

    async fn settings(&self) -> SaveProductionSettingsRequest {
        let version = self
            .store
            .production_state(ALICE, &self.script)
            .await
            .unwrap()
            .settings
            .map_or(0, |settings| settings.version);
        SaveProductionSettingsRequest {
            operation_id: Uuid::new_v4().to_string(),
            expected_revision: 1,
            expected_settings_version: version,
            settings: fixture_settings(),
        }
    }

    async fn claim(&self, declaration: ProductionRightsDeclaration) {
        let rights = self
            .store
            .production_state(ALICE, &self.script)
            .await
            .unwrap()
            .rights;
        let expected_version = rights
            .iter()
            .find(|claim| claim.claim.declaration.record_id == declaration.record_id)
            .map_or(0, |claim| claim.claim.version);
        self.store
            .save_production_rights(
                ALICE,
                &self.script,
                SaveProductionRightsRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_version,
                    claim: declaration,
                },
            )
            .await
            .unwrap();
    }

    async fn ready(&self) {
        self.store
            .save_production_settings(ALICE, &self.script, self.settings().await)
            .await
            .unwrap();
        for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
            if kind == "rights" {
                self.claim(fixture_claim(
                    id,
                    ProductionRightsSubject::Evidence {
                        record_id: id.to_owned(),
                    },
                ))
                .await;
            }
        }
        self.claim(fixture_claim(
            "voice_fixture",
            ProductionRightsSubject::Voice {
                provider_id: "reference-only".into(),
                model_id: "reference-v1".into(),
                voice_id: "synthetic-character".into(),
            },
        ))
        .await;
    }

    async fn freeze(&self) -> cantos_contracts::ProductionSnapshotResponse {
        let preview = self
            .store
            .production_preview(ALICE, &self.script)
            .await
            .unwrap();
        self.store
            .freeze_production(
                ALICE,
                &self.script,
                FreezeProductionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: preview.script_revision,
                    settings_version: preview.settings_version,
                    input_digest: preview.input_digest,
                },
            )
            .await
            .unwrap()
    }

    async fn json(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(format!("/api/v1/studio{path}"))
            .header("origin", ORIGIN)
            .header("content-type", "application/json");
        if let Some(token) = token {
            request = request.header("cookie", format!("cantos_session={token}"));
        }
        let response = self
            .app
            .clone()
            .oneshot(
                request
                    .body(Body::from(
                        body.map_or_else(String::new, |body| body.to_string()),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    async fn counts(&self) -> (i64, i64, i64, i64) {
        let row = self.admin.query_one("SELECT (SELECT count(*) FROM production_settings),(SELECT count(*) FROM production_rights_claims),(SELECT count(*) FROM production_snapshots),(SELECT count(*) FROM production_approvals)", &[]).await.unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3))
    }
}

fn fixture_settings() -> ProductionSettings {
    ProductionSettings {
        bindings: ["narrator", "an", "minh"]
            .into_iter()
            .map(|id| CharacterCasting {
                character_id: id.into(),
                provider_id: "reference-only".into(),
                model_id: "reference-v1".into(),
                voice_id: "synthetic-character".into(),
                language: "vi-VN".into(),
                voice_rights_record_id: "voice_fixture".into(),
                performance: SynthesisPerformance {
                    rate_permille: 1_000,
                    pitch_semitones: 0,
                    emotion: None,
                    intensity_permille: None,
                },
                pronunciation: vec![],
            })
            .collect(),
        budget: ProductionBudget {
            currency: ProductionCurrency::VND,
            limit_minor: 10_000,
            rate: Some(ProductionRate {
                reference: "Synthetic planning rate, not a provider quote".into(),
                version: "fixture-v1".into(),
                units_per_charge: 100,
                amount_minor: 1,
            }),
            scope: "private integration planning only".into(),
            territory: "private-planning".into(),
        },
    }
}

fn fixture_claim(id: &str, subject: ProductionRightsSubject) -> ProductionRightsDeclaration {
    ProductionRightsDeclaration {
        record_id: id.into(),
        subject,
        scope: ProductionRightsScope::ProductionSynthesis,
        rights_holder: "Repository synthetic fixture author".into(),
        languages: vec!["vi-VN".into()],
        territory: "private-planning".into(),
        attribution: "Original synthetic fixtures".into(),
        restrictions: "private-planning-only".into(),
        permitted_scope: "private integration planning only".into(),
        status: ProductionRightsStatus::Granted,
        reference: "Recorded fixture assertion; not legal verification".into(),
        valid_from_unix: 0,
        valid_until_unix: None,
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn local_casting_rights_and_snapshot_approval_survive_reopen() {
    let h = Harness::new(true).await;
    h.ready().await;
    let mut request = h.settings().await;
    for (binding, voice) in
        request
            .settings
            .bindings
            .iter_mut()
            .zip(["vi", "vi-vn-x-central", "vi-vn-x-south"])
    {
        binding.provider_id = "espeak-ng-local".into();
        binding.model_id = "espeak-ng-1.52.0".into();
        binding.voice_id = voice.into();
        binding.voice_rights_record_id = format!("local_voice_{}", binding.character_id);
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
        h.claim(fixture_claim(
            &binding.voice_rights_record_id,
            ProductionRightsSubject::Voice {
                provider_id: binding.provider_id.clone(),
                model_id: binding.model_id.clone(),
                voice_id: binding.voice_id.clone(),
            },
        ))
        .await;
    }
    h.store
        .save_production_settings(ALICE, &h.script, request)
        .await
        .unwrap();
    let snapshot = h.freeze().await;
    assert!(snapshot.document.resolved.iter().all(|line| {
        line.provider_id == "espeak-ng-local"
            && line.adapter_version == "espeak-ng-local-v1"
            && line.output_contract == "wav-pcm-s16le-mono-22050-v1"
    }));
    h.store
        .approve_production(
            ALICE,
            &h.script,
            &snapshot.id,
            ApproveProductionRequest {
                operation_id: Uuid::new_v4().to_string(),
                input_digest: snapshot.document.input_digest.clone(),
            },
        )
        .await
        .unwrap();
    let mut config = h.config.clone();
    config.user("cantos_app");
    let reopened = Store::new(config).unwrap();
    let eligible = reopened
        .production_eligibility(ALICE, &h.script, &snapshot.id)
        .await
        .unwrap();
    assert!(eligible.inputs_eligible && eligible.approval_current);
    assert!(!eligible.billable_dispatch_available);
    assert!(matches!(
        reopened
            .production_eligibility(BOB, &h.script, &snapshot.id)
            .await,
        Err(StoreError::NotFound)
    ));
    let mut changed = h.settings().await;
    changed.settings.bindings = snapshot.document.settings.settings.bindings.clone();
    changed.settings.bindings[0].voice_id = "vi-vn-x-central".into();
    changed.settings.bindings[0].voice_rights_record_id = "local_voice_an".into();
    h.store
        .save_production_settings(ALICE, &h.script, changed)
        .await
        .unwrap();
    assert!(
        !reopened
            .production_eligibility(ALICE, &h.script, &snapshot.id)
            .await
            .unwrap()
            .approval_current
    );
    assert_eq!(h.counts().await.2, 1);
}

/// The Studio journey over HTTP: casting mixes eSpeak and two Google families, inherited
/// non-neutral script delivery is blocked at review, and an explicit neutral delivery at
/// intensity 0 saves, freezes and receives owner approval without enabling dispatch.
#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn mixed_espeak_and_google_casting_freezes_and_approves_only_explicit_neutral_delivery() {
    let h = Harness::new(true).await;
    h.ready().await;
    let prefix = format!("/scripts/{}/production", h.script);
    let (status, catalog) = h
        .json("GET", "/production/catalog", Some(ALICE), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    let choices = [
        (
            "espeak-ng-local",
            "espeak-ng-1.52.0",
            "vi-vn-x-south",
            1_000,
            0,
        ),
        (
            "google-cloud-tts",
            "google-neural2",
            "vi-VN-Neural2-A",
            1_200,
            -2,
        ),
        (
            "google-cloud-tts",
            "google-chirp3-hd",
            "vi-VN-Chirp3-HD-Aoede",
            1_000,
            0,
        ),
    ];
    for (provider, model, voice, _, _) in choices {
        assert!(catalog["capabilities"].as_array().unwrap().iter().any(|c| {
            c["provider_id"] == provider
                && c["model_id"] == model
                && c["intensity_supported"] == false
                && c["voice_ids"].as_array().unwrap().contains(&json!(voice))
        }));
    }
    let mut settings = fixture_settings();
    for (binding, (provider, model, voice, rate, pitch)) in
        settings.bindings.iter_mut().zip(choices)
    {
        binding.provider_id = provider.into();
        binding.model_id = model.into();
        binding.voice_id = voice.into();
        binding.voice_rights_record_id = format!("voice_{}", binding.character_id);
        binding.performance.rate_permille = rate;
        binding.performance.pitch_semitones = pitch;
        h.claim(fixture_claim(
            &binding.voice_rights_record_id,
            ProductionRightsSubject::Voice {
                provider_id: provider.into(),
                model_id: model.into(),
                voice_id: voice.into(),
            },
        ))
        .await;
    }
    let save = |version: u64, settings: &ProductionSettings| {
        json!(SaveProductionSettingsRequest {
            operation_id: Uuid::new_v4().to_string(),
            expected_revision: 1,
            expected_settings_version: version,
            settings: settings.clone(),
        })
    };
    let (status, saved) = h
        .json(
            "POST",
            &format!("{prefix}/settings"),
            Some(ALICE),
            Some(save(1, &settings)),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, review) = h
        .json("GET", &format!("{prefix}/review"), Some(ALICE), None)
        .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "inherited calm/hopeful/warm delivery: {review}"
    );
    assert_eq!(review["code"], "invalid_request");
    assert!(review["issues"].as_array().unwrap().iter().any(|issue| {
        issue["rule"] == "unsupported_control"
            && issue["path"].as_str().unwrap().ends_with(".performance")
    }));
    for binding in &mut settings.bindings {
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
    }
    let (status, saved) = h
        .json(
            "POST",
            &format!("{prefix}/settings"),
            Some(ALICE),
            Some(save(2, &settings)),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, review) = h
        .json("GET", &format!("{prefix}/review"), Some(ALICE), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{review}");
    assert_eq!(review["inputs_eligible"], true, "{review}");
    let (status, snapshot) = h
        .json(
            "POST",
            &format!("{prefix}/snapshots"),
            Some(ALICE),
            Some(json!(FreezeProductionRequest {
                operation_id: Uuid::new_v4().to_string(),
                expected_revision: 1,
                settings_version: review["settings_version"].as_u64().unwrap(),
                input_digest: review["input_digest"].as_str().unwrap().into(),
            })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{snapshot}");
    let lines = snapshot["document"]["resolved"].as_array().unwrap();
    assert!(!lines.is_empty());
    for line in lines {
        let (adapter, output) = if line["provider_id"] == "espeak-ng-local" {
            ("espeak-ng-local-v1", "wav-pcm-s16le-mono-22050-v1")
        } else {
            assert_eq!(line["provider_id"], "google-cloud-tts");
            (
                "google-cloud-tts-synthesize-3",
                "wav-pcm-s16le-mono-24000-v1",
            )
        };
        assert_eq!(line["adapter_version"], adapter);
        assert_eq!(line["output_contract"], output);
        assert_eq!(line["performance"]["emotion"], "neutral");
        assert_eq!(line["performance"]["intensity_permille"], 0);
    }
    let id = snapshot["id"].as_str().unwrap();
    let digest = snapshot["document"]["input_digest"].as_str().unwrap();
    let (status, approval) = h
        .json(
            "POST",
            &format!("{prefix}/snapshots/{id}/approvals"),
            Some(ALICE),
            Some(json!(ApproveProductionRequest {
                operation_id: Uuid::new_v4().to_string(),
                input_digest: digest.into(),
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{approval}");
    let (status, eligibility) = h
        .json(
            "GET",
            &format!("{prefix}/snapshots/{id}/eligibility"),
            Some(ALICE),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(eligibility["inputs_eligible"], true);
    assert_eq!(eligibility["approval_current"], true);
    assert_eq!(eligibility["billable_dispatch_available"], false);
    assert_eq!(h.counts().await.2, 1);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn historical_google_snapshot_reads_and_replays_without_becoming_current_authorization() {
    const HISTORICAL_ADAPTER: &str = "google-cloud-tts-synthesize-2";
    let h = Harness::new(true).await;
    h.ready().await;
    let mut settings = h.settings().await;
    for binding in &mut settings.settings.bindings {
        binding.provider_id = "google-cloud-tts".into();
        binding.model_id = "google-neural2".into();
        binding.voice_id = "vi-VN-Neural2-A".into();
        binding.voice_rights_record_id = "google_voice_fixture".into();
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
    }
    h.claim(fixture_claim(
        "google_voice_fixture",
        ProductionRightsSubject::Voice {
            provider_id: "google-cloud-tts".into(),
            model_id: "google-neural2".into(),
            voice_id: "vi-VN-Neural2-A".into(),
        },
    ))
    .await;
    h.store
        .save_production_settings(ALICE, &h.script, settings)
        .await
        .unwrap();
    let current = h.freeze().await;
    assert!(current.eligibility.inputs_eligible);
    let mut historical = current.document.clone();
    assert!(!historical.resolved.is_empty());
    for line in &mut historical.resolved {
        assert_eq!(line.adapter_version, "google-cloud-tts-synthesize-3");
        line.adapter_version = HISTORICAL_ADAPTER.into();
    }
    historical.input_digest =
        cantos_api::contexts::production::production_input_digest(&historical).unwrap();
    assert_ne!(historical.input_digest, current.document.input_digest);

    // Seed the old immutable shape against real pinned revision/settings/rights
    // receipts. Setup never rewrites a settled row or disables its protection trigger.
    let id = Uuid::new_v4().to_string();
    let freeze = FreezeProductionRequest {
        operation_id: Uuid::new_v4().to_string(),
        expected_revision: historical.revision.revision,
        settings_version: historical.settings.version,
        input_digest: historical.input_digest.clone(),
    };
    let freeze_bytes = serde_json::to_vec(&freeze).unwrap();
    let document_bytes = serde_json::to_vec(&historical).unwrap();
    let revision = i64::try_from(historical.revision.revision).unwrap();
    let settings_version = i64::try_from(historical.settings.version).unwrap();
    h.admin.execute("INSERT INTO production_snapshots(id,script_id,owner_id,script_revision,settings_version,operation_id,created_by,input_digest,request_bytes,document_bytes) VALUES($1,$2,'alice',$3,$4,$5,'alice',$6,$7,$8)", &[&id,&h.script,&revision,&settings_version,&freeze.operation_id,&historical.input_digest,&freeze_bytes,&document_bytes]).await.unwrap();
    let approval_id = Uuid::new_v4().to_string();
    let approval = ApproveProductionRequest {
        operation_id: Uuid::new_v4().to_string(),
        input_digest: historical.input_digest.clone(),
    };
    let approval_bytes = serde_json::to_vec(&approval).unwrap();
    h.admin.execute("INSERT INTO production_approvals(id,snapshot_id,script_id,operation_id,approved_by,input_digest,request_bytes) VALUES($1,$2,$3,$4,'alice',$5,$6)", &[&approval_id,&id,&h.script,&approval.operation_id,&historical.input_digest,&approval_bytes]).await.unwrap();
    let before = h.counts().await;

    let mut config = h.config.clone();
    config.user("cantos_app");
    let reopened = Store::new(config).unwrap();
    let saved = reopened
        .production_snapshot(ALICE, &h.script, &id)
        .await
        .unwrap();
    assert_eq!(saved.document, historical);
    assert_eq!(saved.approval.as_ref().unwrap().id, approval_id);
    assert!(!saved.eligibility.inputs_eligible);
    assert!(!saved.eligibility.approval_current);
    assert!(!saved.eligibility.billable_dispatch_available);
    assert!(saved.eligibility.findings.iter().any(|finding| {
        finding.code == ProductionFindingCode::ApprovalScopeMismatch
            && finding.path == "resolved.adapter_version"
    }));

    let prefix = format!("/scripts/{}/production", h.script);
    let (status, response) = h
        .json(
            "GET",
            &format!("{prefix}/snapshots/{id}"),
            Some(ALICE),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["document"], json!(historical));
    assert_eq!(response["approval"]["id"], approval_id);
    assert_eq!(response["eligibility"]["inputs_eligible"], false);
    assert_eq!(response["eligibility"]["approval_current"], false);
    let replay = reopened
        .freeze_production(ALICE, &h.script, freeze.clone())
        .await
        .unwrap();
    assert_eq!(replay, saved);
    let (status, response) = h
        .json(
            "POST",
            &format!("{prefix}/snapshots"),
            Some(ALICE),
            Some(json!(freeze)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{response}");
    assert_eq!(response["id"], id);
    assert_eq!(response["document"], json!(historical));

    let (status, response) = h
        .json(
            "POST",
            &format!("{prefix}/snapshots/{id}/approvals"),
            Some(ALICE),
            Some(json!(ApproveProductionRequest {
                operation_id: Uuid::new_v4().to_string(),
                input_digest: historical.input_digest.clone(),
            })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{response}");
    assert_eq!(response["code"], "production_blocked");
    assert!(response["issues"].as_array().unwrap().iter().any(|issue| {
        issue["rule"] == "approval_scope_mismatch" && issue["path"] == "resolved.adapter_version"
    }));
    let error = h
        .admin
        .execute(
            "UPDATE production_snapshots SET document_bytes=$2 WHERE id=$1",
            &[&id, &document_bytes],
        )
        .await
        .unwrap_err();
    assert_eq!(error.code().unwrap().code(), "55000");
    let row = h.admin.query_one("SELECT s.document_bytes,s.request_bytes,a.request_bytes AS approval_bytes FROM production_snapshots s JOIN production_approvals a ON a.snapshot_id=s.id WHERE s.id=$1", &[&id]).await.unwrap();
    assert_eq!(row.get::<_, Vec<u8>>("document_bytes"), document_bytes);
    assert_eq!(row.get::<_, Vec<u8>>("request_bytes"), freeze_bytes);
    assert_eq!(row.get::<_, Vec<u8>>("approval_bytes"), approval_bytes);
    assert_eq!(h.counts().await, before);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn immutable_candidates_and_exact_concurrent_replays_survive_reopen() {
    let h = Harness::new(true).await;
    h.ready().await;
    let preview = h.store.production_preview(ALICE, &h.script).await.unwrap();
    assert!(preview.inputs_eligible);
    assert!(!preview.billable_dispatch_available);
    let request = FreezeProductionRequest {
        operation_id: Uuid::new_v4().to_string(),
        expected_revision: 1,
        settings_version: 1,
        input_digest: preview.input_digest,
    };
    let (a, b) = tokio::join!(
        h.store.freeze_production(ALICE, &h.script, request.clone()),
        h.store.freeze_production(ALICE, &h.script, request.clone())
    );
    let snapshot = a.unwrap();
    assert_eq!(snapshot, b.unwrap());
    assert_eq!(h.counts().await.2, 1);
    let mut altered = request.clone();
    altered.input_digest = "production-p1:sha256:".to_owned() + &"0".repeat(64);
    assert!(matches!(
        h.store.freeze_production(ALICE, &h.script, altered).await,
        Err(StoreError::OperationReused)
    ));
    let approval_request = ApproveProductionRequest {
        operation_id: Uuid::new_v4().to_string(),
        input_digest: snapshot.document.input_digest.clone(),
    };
    let (a, b) = tokio::join!(
        h.store
            .approve_production(ALICE, &h.script, &snapshot.id, approval_request.clone()),
        h.store
            .approve_production(ALICE, &h.script, &snapshot.id, approval_request.clone())
    );
    assert_eq!(a.unwrap(), b.unwrap());
    let before = h.counts().await;
    assert!(matches!(
        h.store
            .approve_production(
                ALICE,
                &h.script,
                &snapshot.id,
                ApproveProductionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    input_digest: snapshot.document.input_digest.clone()
                }
            )
            .await,
        Err(StoreError::StaleProductionInputs)
    ));
    assert_eq!(h.counts().await, before);
    let mut config = h.config.clone();
    config.user("cantos_app");
    let reopened = Store::new(config).unwrap();
    assert!(
        reopened
            .production_eligibility(ALICE, &h.script, &snapshot.id)
            .await
            .unwrap()
            .approval_current
    );
    assert_eq!(
        reopened
            .freeze_production(ALICE, &h.script, request)
            .await
            .unwrap()
            .document,
        snapshot.document
    );
    for table in [
        "production_settings",
        "production_rights_claims",
        "production_snapshots",
        "production_approvals",
    ] {
        let error = h
            .admin
            .batch_execute(&format!("DELETE FROM {table}"))
            .await
            .unwrap_err();
        assert_eq!(error.code().unwrap().code(), "55000");
        let error = h
            .admin
            .batch_execute(&format!("TRUNCATE {table} CASCADE"))
            .await
            .unwrap_err();
        assert_eq!(error.code().unwrap().code(), "55000");
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn settings_rights_and_full_export_edits_invalidate_authorization_without_mutating_snapshot()
{
    let h = Harness::new(true).await;
    h.ready().await;
    let snapshot = h.freeze().await;
    let approval_request = ApproveProductionRequest {
        operation_id: Uuid::new_v4().to_string(),
        input_digest: snapshot.document.input_digest.clone(),
    };
    let receipt = h
        .store
        .approve_production(ALICE, &h.script, &snapshot.id, approval_request.clone())
        .await
        .unwrap();
    let mut claim = fixture_claim(
        "voice_fixture",
        ProductionRightsSubject::Voice {
            provider_id: "reference-only".into(),
            model_id: "reference-v1".into(),
            voice_id: "synthetic-character".into(),
        },
    );
    claim.status = ProductionRightsStatus::Revoked;
    h.claim(claim).await;
    let result = h
        .store
        .production_snapshot(ALICE, &h.script, &snapshot.id)
        .await
        .unwrap();
    assert_eq!(result.document, snapshot.document);
    assert!(!result.eligibility.approval_current);
    assert!(result
        .eligibility
        .findings
        .iter()
        .any(|finding| finding.code == ProductionFindingCode::RightsRevoked));
    assert!(result
        .eligibility
        .findings
        .iter()
        .any(|finding| finding.code == ProductionFindingCode::RightsChanged));
    assert_eq!(
        h.store
            .approve_production(ALICE, &h.script, &snapshot.id, approval_request)
            .await
            .unwrap(),
        receipt
    );
    for changed in ["voice", "performance", "pronunciation", "budget", "rate"] {
        let mut request = h.settings().await;
        match changed {
            "voice" => request.settings.bindings[0].voice_id = "synthetic-narrator".into(),
            "performance" => request.settings.bindings[0].performance.rate_permille = 1_100,
            "pronunciation" => request.settings.bindings[0].pronunciation.push(
                cantos_contracts::ProductionPronunciation {
                    surface: "Vọng Đài".into(),
                    replacement: "Vọng Đài".into(),
                },
            ),
            "budget" => request.settings.budget.limit_minor = 20_000,
            "rate" => request.settings.budget.rate.as_mut().unwrap().version = "fixture-v2".into(),
            _ => unreachable!(),
        }
        h.store
            .save_production_settings(ALICE, &h.script, request)
            .await
            .unwrap();
        let eligibility = h
            .store
            .production_eligibility(ALICE, &h.script, &snapshot.id)
            .await
            .unwrap();
        assert!(!eligibility.approval_current, "{changed}");
        assert!(eligibility
            .findings
            .iter()
            .any(|finding| finding.code == ProductionFindingCode::SettingsChanged));
    }
    let mut script: Value = serde_json::from_str(SCRIPT).unwrap();
    script["work"]["title"] =
        json!("Ánh đèn cuối sân khấu — quyền và provenance vẫn thuộc toàn bộ export");
    h.store
        .save(
            ALICE,
            &h.script,
            SaveRevisionRequest {
                operation_id: Uuid::new_v4().to_string(),
                expected_revision: 1,
                script_json: script.to_string(),
            },
        )
        .await
        .unwrap();
    let eligibility = h
        .store
        .production_eligibility(ALICE, &h.script, &snapshot.id)
        .await
        .unwrap();
    assert!(!eligibility.approval_current);
    assert!(eligibility
        .findings
        .iter()
        .any(|finding| finding.code == ProductionFindingCode::RevisionChanged));
    assert_eq!(
        h.store
            .production_snapshot(ALICE, &h.script, &snapshot.id)
            .await
            .unwrap()
            .document,
        snapshot.document
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn private_http_and_owner_actions_fail_closed_and_blocked_candidate_is_inspectable() {
    let h = Harness::new(false).await;
    h.ready().await;
    let snapshot = h.freeze().await;
    assert!(!snapshot.eligibility.inputs_eligible);
    let prefix = format!("/scripts/{}/production", h.script);
    for token in [None, Some(BOB)] {
        let (status, body) = h.json("GET", &prefix, token, None).await;
        assert_eq!(
            status,
            if token.is_none() {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::NOT_FOUND
            }
        );
        assert_eq!(body["issues"], json!([]));
    }
    h.admin
        .execute(
            "INSERT INTO script_members(script_id,actor_id,role) VALUES($1,'bob','editor')",
            &[&h.script],
        )
        .await
        .unwrap();
    let mut app_config = h.config.clone();
    app_config.user("cantos_app");
    let app_actor = connect(&app_config).await;
    let error = app_actor
        .execute("UPDATE actors SET active=false WHERE id='alice'", &[])
        .await
        .unwrap_err();
    assert_eq!(error.code().unwrap().code(), "42501");
    let error = app_actor
        .execute(
            "UPDATE script_members SET role='reader' WHERE script_id=$1 AND actor_id='bob'",
            &[&h.script],
        )
        .await
        .unwrap_err();
    assert_eq!(error.code().unwrap().code(), "42501");
    assert_eq!(
        h.json("GET", &prefix, Some(BOB), None).await.0,
        StatusCode::OK
    );
    let request = ApproveProductionRequest {
        operation_id: Uuid::new_v4().to_string(),
        input_digest: snapshot.document.input_digest.clone(),
    };
    let path = format!("{prefix}/snapshots/{}/approvals", snapshot.id);
    let before = h.counts().await;
    let (status, body) = h
        .json(
            "POST",
            &path,
            Some(BOB),
            Some(serde_json::to_value(&request).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "forbidden");
    let (status, body) = h
        .json(
            "POST",
            &path,
            Some(ALICE),
            Some(serde_json::to_value(&request).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "production_blocked");
    assert!(body["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["rule"] == "script_unreviewed"));
    assert_eq!(h.counts().await, before);
    h.admin
        .execute(
            "DELETE FROM script_members WHERE script_id=$1 AND actor_id='bob'",
            &[&h.script],
        )
        .await
        .unwrap();
    assert_eq!(
        h.json("GET", &prefix, Some(BOB), None).await.0,
        StatusCode::NOT_FOUND
    );
    h.admin
        .execute("UPDATE actors SET active=false WHERE id='alice'", &[])
        .await
        .unwrap();
    assert!(matches!(
        h.store
            .production_snapshot(ALICE, &h.script, &snapshot.id)
            .await,
        Err(StoreError::Unauthenticated)
    ));
    assert!(production_catalog()
        .capabilities
        .iter()
        .all(|capability| !capability.billable_dispatch_available));
    assert_eq!(
        production_catalog()
            .capabilities
            .iter()
            .filter(|c| c.reference_only)
            .count(),
        2
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn expired_rights_unsupported_inputs_and_transaction_failure_leave_no_partial_authorization()
{
    let h = Harness::new(true).await;
    h.ready().await;
    let before = h.counts().await;
    let mut invalid = h.settings().await;
    invalid.settings.bindings[0].provider_id = "unavailable-provider".into();
    assert!(matches!(
        h.store
            .save_production_settings(ALICE, &h.script, invalid)
            .await,
        Err(StoreError::InvalidRequestFields(_))
    ));
    assert_eq!(h.counts().await, before);
    let mut claim = fixture_claim(
        "voice_fixture",
        ProductionRightsSubject::Voice {
            provider_id: "reference-only".into(),
            model_id: "reference-v1".into(),
            voice_id: "synthetic-character".into(),
        },
    );
    claim.valid_until_unix = Some(1);
    h.claim(claim).await;
    let expired = h.freeze().await;
    assert!(expired
        .eligibility
        .findings
        .iter()
        .any(|finding| finding.code == ProductionFindingCode::RightsExpired));
    assert!(matches!(
        h.store
            .approve_production(
                ALICE,
                &h.script,
                &expired.id,
                ApproveProductionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    input_digest: expired.document.input_digest
                }
            )
            .await,
        Err(StoreError::ProductionBlocked(_))
    ));
    h.claim(fixture_claim(
        "voice_fixture",
        ProductionRightsSubject::Voice {
            provider_id: "reference-only".into(),
            model_id: "reference-v1".into(),
            voice_id: "synthetic-character".into(),
        },
    ))
    .await;
    let snapshot = h.freeze().await;
    h.admin.batch_execute("CREATE FUNCTION inject_approval_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic rollback'; END $$; CREATE TRIGGER inject_approval_failure BEFORE INSERT ON production_approvals FOR EACH ROW EXECUTE FUNCTION inject_approval_failure();").await.unwrap();
    let request = ApproveProductionRequest {
        operation_id: Uuid::new_v4().to_string(),
        input_digest: snapshot.document.input_digest.clone(),
    };
    let before = h.counts().await;
    assert!(matches!(
        h.store
            .approve_production(ALICE, &h.script, &snapshot.id, request.clone())
            .await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, before);
    h.admin.batch_execute("DROP TRIGGER inject_approval_failure ON production_approvals; DROP FUNCTION inject_approval_failure();").await.unwrap();
    h.store
        .approve_production(ALICE, &h.script, &snapshot.id, request)
        .await
        .unwrap();
    assert_eq!(h.counts().await.3, before.3 + 1);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn permission_revocation_waits_for_actual_production_transaction_and_next_write_denies() {
    let h = Harness::new(true).await;
    h.ready().await;
    h.admin
        .execute(
            "INSERT INTO script_members(script_id,actor_id,role) VALUES($1,'bob','editor')",
            &[&h.script],
        )
        .await
        .unwrap();
    h.admin.batch_execute("CREATE FUNCTION hold_production_save() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(1128353365); RETURN NEW; END $$; CREATE TRIGGER hold_production_save BEFORE INSERT ON production_settings FOR EACH ROW EXECUTE FUNCTION hold_production_save();").await.unwrap();
    let mut guard = connect(&h.config).await;
    let tx = guard.transaction().await.unwrap();
    tx.query_one("SELECT pg_advisory_xact_lock(1128353365)", &[])
        .await
        .unwrap();
    let store = h.store.clone();
    let script = h.script.clone();
    let request = h.settings().await;
    let writer =
        tokio::spawn(async move { store.save_production_settings(BOB, &script, request).await });
    wait_for_lock(&h.admin, "INSERT INTO production_settings%").await;
    let revoke = connect(&h.config).await;
    let script = h.script.clone();
    let revocation = tokio::spawn(async move {
        revoke
            .execute(
                "DELETE FROM script_members WHERE script_id=$1 AND actor_id='bob'",
                &[&script],
            )
            .await
            .unwrap();
    });
    wait_for_lock(&h.admin, "DELETE FROM script_members%").await;
    assert!(!writer.is_finished());
    assert!(!revocation.is_finished());
    tx.commit().await.unwrap();
    assert_eq!(writer.await.unwrap().unwrap().recorded_by, "bob");
    revocation.await.unwrap();
    assert!(matches!(
        h.store
            .save_production_settings(BOB, &h.script, h.settings().await)
            .await,
        Err(StoreError::NotFound)
    ));
}

async fn wait_for_lock(admin: &Client, query: &str) {
    tokio::time::timeout(Duration::from_secs(2),async {
        loop {
            let waiting:bool = admin.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE $1)", &[&query]).await.unwrap().get(0);
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn a_self_consistent_forged_snapshot_does_not_replace_actual_pinned_receipts() {
    let h = Harness::new(true).await;
    h.ready().await;
    let snapshot = h.freeze().await;
    let mut document = snapshot.document.clone();
    document.settings.settings.budget.limit_minor += 1;
    document.input_digest =
        cantos_api::contexts::production::production_input_digest(&document).unwrap();
    let id = Uuid::new_v4().to_string();
    let operation = Uuid::new_v4().to_string();
    let request = FreezeProductionRequest {
        operation_id: operation.clone(),
        expected_revision: 1,
        settings_version: 1,
        input_digest: document.input_digest.clone(),
    };
    let mut app_config = h.config.clone();
    app_config.user("cantos_app");
    let app = connect(&app_config).await;
    app.execute("INSERT INTO production_snapshots(id,script_id,owner_id,script_revision,settings_version,operation_id,created_by,input_digest,request_bytes,document_bytes) VALUES($1,$2,'alice',1,1,$3,'alice',$4,$5,$6)", &[&id,&h.script,&operation,&document.input_digest,&serde_json::to_vec(&request).unwrap(),&serde_json::to_vec(&document).unwrap()]).await.unwrap();
    assert!(matches!(
        h.store.production_snapshot(ALICE, &h.script, &id).await,
        Err(StoreError::CorruptRevision)
    ));
    assert!(
        h.store
            .production_snapshot(ALICE, &h.script, &snapshot.id)
            .await
            .unwrap()
            .eligibility
            .inputs_eligible
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn concurrent_settings_updates_and_failed_freeze_cannot_accept_a_stale_preview() {
    let h = Harness::new(true).await;
    h.ready().await;
    let preview = h.store.production_preview(ALICE, &h.script).await.unwrap();
    let request = FreezeProductionRequest {
        operation_id: Uuid::new_v4().to_string(),
        expected_revision: 1,
        settings_version: 1,
        input_digest: preview.input_digest,
    };
    h.admin.batch_execute("CREATE FUNCTION fail_after_freeze() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic after-insert rollback'; END $$; CREATE TRIGGER fail_after_freeze AFTER INSERT ON production_snapshots FOR EACH ROW EXECUTE FUNCTION fail_after_freeze();").await.unwrap();
    let before = h.counts().await;
    assert!(matches!(
        h.store
            .freeze_production(ALICE, &h.script, request.clone())
            .await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, before);
    h.admin.batch_execute("DROP TRIGGER fail_after_freeze ON production_snapshots; DROP FUNCTION fail_after_freeze();").await.unwrap();
    let snapshot = h
        .store
        .freeze_production(ALICE, &h.script, request.clone())
        .await
        .unwrap();
    let mut a = h.settings().await;
    let mut b = h.settings().await;
    a.settings.budget.limit_minor = 11_000;
    b.settings.budget.limit_minor = 12_000;
    let (a, b) = tokio::join!(
        h.store.save_production_settings(ALICE, &h.script, a),
        h.store.save_production_settings(ALICE, &h.script, b)
    );
    assert!(matches!(
        (&a, &b),
        (Ok(_), Err(StoreError::StaleProductionInputs))
            | (Err(StoreError::StaleProductionInputs), Ok(_))
    ));
    assert_eq!(h.counts().await.0, 2);
    let mut stale = request.clone();
    stale.operation_id = Uuid::new_v4().to_string();
    let before = h.counts().await;
    assert!(matches!(
        h.store.freeze_production(ALICE, &h.script, stale).await,
        Err(StoreError::StaleProductionInputs)
    ));
    assert_eq!(h.counts().await, before);
    assert_eq!(
        h.store
            .freeze_production(ALICE, &h.script, request)
            .await
            .unwrap()
            .id,
        snapshot.id
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn missing_lock_capability_during_role_rollout_fails_without_authority_changes() {
    let h = Harness::new(true).await;
    h.ready().await;
    let before = h.counts().await;
    h.admin.batch_execute("REVOKE EXECUTE ON FUNCTION production_lock_actor(text),production_lock_member(text,text) FROM cantos_app;").await.unwrap();
    assert!(matches!(
        h.store.production_state(ALICE, &h.script).await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, before);
    h.admin.batch_execute("GRANT EXECUTE ON FUNCTION production_lock_actor(text),production_lock_member(text,text) TO cantos_app;").await.unwrap();
    assert_eq!(
        h.store
            .production_state(ALICE, &h.script)
            .await
            .unwrap()
            .settings
            .unwrap()
            .version,
        1
    );
}
