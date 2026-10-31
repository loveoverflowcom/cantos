//! S12 recovery qualification. These are disposable, quiesced cuts, not a production restore
//! command. The stale-pair row is a security-gap characterization, explicitly NO_GO even when
//! this regression test passes. No restored pair is opted into restricted delivery.
use super::*;
use cantos_identity::{
    domain::{
        admission::AccountState,
        ids::{OperationId, SubjectId},
    },
    persistence::{IdentityStore, StoredSubject},
};
use cantos_store_turso::{
    clock::SystemClock, registry::Intent, BackupReceipt, OpenRequest, OwnedDatabase,
};
use sha2::{Digest, Sha256};

struct Checkpoint {
    server: BackupReceipt,
    identity: BackupReceipt,
    security: StoredSubject,
}

fn receipt(value: &BackupReceipt) -> Value {
    json!({"bytes": value.bytes, "sha256": format!("{:x}", Sha256::digest(std::fs::read(&value.path).unwrap()))})
}

fn closed_snapshot_copy(backup: &BackupReceipt, destination: &Path) {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let bytes = std::fs::read(&backup.path).unwrap();
    assert_eq!(bytes.len() as u64, backup.bytes);
    assert_eq!(<[u8; 32]>::from(Sha256::digest(&bytes)), backup.sha256);
    assert!(!destination.exists());
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(destination)
        .unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
}

async fn identity_security(path: &Path, subject: SubjectId) -> StoredSubject {
    let identity = IdentityStore::open(path, Intent::Serve, Arc::new(SystemClock))
        .await
        .unwrap();
    identity.check_integrity().await.unwrap();
    identity.subject(subject).await.unwrap().unwrap()
}

/// The Identity process is stopped before this independent owner opens identity.db. The
/// Server fixture never reads that file; the harness coordinates two distinct owner cuts.
async fn checkpoint(
    fixture: &ServerFixture,
    env: &IdentityEnv,
    run: &RunDir,
    label: &str,
    subject: SubjectId,
) -> Checkpoint {
    let identity = OwnedDatabase::open(OpenRequest::new(
        &env.database,
        cantos_identity::persistence::registry().unwrap(),
        Intent::Serve,
        Arc::new(SystemClock),
    ))
    .await
    .unwrap();
    let identity_backup = identity
        .backup_to(&run.path(&format!("{label}-identity.db")))
        .await
        .unwrap();
    drop(identity);
    let security = identity_security(&env.database, subject).await;
    let server = fixture
        .backup_to(&run.path(&format!("{label}-server.db")))
        .await;
    Checkpoint {
        server,
        identity: identity_backup,
        security,
    }
}

fn closed_host(
    fixture: &ServerFixture,
    env: &IdentityEnv,
    objects: Arc<dyn PublicationObjects>,
) -> Host {
    let store = fixture
        .store()
        .clone()
        .without_development_sessions()
        .with_identity_auth(identity_auth(env));
    let publication = PublicationService::new(store.clone());
    let origin = OriginPlayback::new(
        PublicMediaOrigin::parse("https://api.example").unwrap(),
        GrantKey::new(&[7; 32]).unwrap(),
        objects,
    );
    let origins = SurfaceOrigins::new("https://studio.example", THEATRE, None).unwrap();
    Host {
        router: server_router(
            AppState {
                store,
                web_origin: THEATRE.into(),
            },
            &origins,
            None,
            Some((publication, origin)),
        ),
    }
}

async fn security_row(fixture: &ServerFixture, session: &str) -> Option<String> {
    fixture
        .rows(
            "SELECT revoked_reason FROM identity_sessions WHERE id=?1",
            &[&session],
        )
        .await
        .unwrap()[0]
        .try_get(0)
        .unwrap()
}

async fn publication_fingerprint(fixture: &ServerFixture) -> Value {
    // Immutable release/rights/approval bytes and exact generation state, plus row counts.
    // These queries are privileged
    // fixture observations; HTTP and delivery requests remain the disclosure oracle below.
    let mut result = serde_json::Map::new();
    for table in [
        "audio_releases",
        "catalog_metadata_generations",
        "catalog_metadata_generation_entries",
        "catalog_metadata_generation_audio",
        "audio_distribution_clearances",
        "audio_publication_approvals",
    ] {
        let rows = fixture
            .rows(&format!("SELECT * FROM {table}"), &[])
            .await
            .unwrap();
        result.insert(table.into(), json!(rows.len()));
    }
    for table in [
        "audio_releases",
        "audio_distribution_clearances",
        "audio_publication_approvals",
    ] {
        let documents = fixture
            .rows(&format!("SELECT snapshot FROM {table} ORDER BY id"), &[])
            .await
            .unwrap();
        result.insert(
            format!("{table}_sha256"),
            json!(documents
                .iter()
                .map(|row| {
                    let bytes: Vec<u8> = row.try_get(0).unwrap();
                    format!("{:x}", Sha256::digest(bytes))
                })
                .collect::<Vec<_>>()),
        );
    }
    let state = fixture
        .store()
        .metadata_publication_state(
            CONTROL,
            &fixture
                .rows(
                    "SELECT work_id FROM catalog_metadata_publication_pointers",
                    &[],
                )
                .await
                .unwrap()[0]
                .try_get::<_, String>(0)
                .unwrap(),
        )
        .await
        .unwrap();
    result.insert(
        "metadata_state".into(),
        serde_json::to_value(state).unwrap(),
    );
    Value::Object(result)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paired_restore_keeps_private_delivery_closed_and_exposes_stale_security_recovery_gap() {
    let _ = supplied_identity();
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let identity = env.serve().await;
    let mut fixture = ServerFixture::new().await;
    // The isolation guard intentionally chooses /tmp. macOS aliases it to /private/tmp;
    // name the actual owned directory rather than weakening storage's canonical-root rule.
    let publication_root = std::fs::canonicalize(fixture.dir())
        .unwrap()
        .join("publication");
    let objects = Arc::new(FilesystemPublicationStorage::new(&publication_root).unwrap());
    let work = fixture
        .seed_published_work(
            Some(&objects),
            &WorkSeed::podcast(0x658, "RECOVERY_PRIVATE_SENTINEL", &["One", "Two"]),
        )
        .await;
    fixture.studio_caller("alice", CONTROL).await;
    let public = closed_host(&fixture, &env, objects.clone());
    let viewer = enroll(&public, &sink, OWNER, 1).await;
    let subject_text: String = fixture
        .rows(
            "SELECT subject FROM identity_sessions WHERE id=?1",
            &[&viewer.session],
        )
        .await
        .unwrap()[0]
        .try_get(0)
        .unwrap();
    let subject =
        SubjectId::from_bytes(*Uuid::parse_str(&subject_text).unwrap().as_bytes()).unwrap();
    change(
        &fixture,
        &work.work_id,
        Change::SetAudience {
            audience: ViewerAudience::Restricted,
            confirm_metadata_rights: false,
        },
    )
    .await;
    let grant = actor_grant(&fixture, &work.work_id, &viewer.actor).await;
    let live = restricted_host(&fixture, &env, objects.clone());
    let release = work.units[0].chain.release_id.as_ref().unwrap();
    let old_media = private_grant(&live, &viewer.access, release).await;
    let admitted = send(
        &live.router,
        "GET",
        &old_media,
        &[bearer(&viewer.access), ("range", "bytes=0-9".into())],
        None,
    )
    .await;
    assert_eq!(admitted.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(admitted.body, work.units[0].chain.bytes[..10]);
    let page_path = theatre(&format!("/shared/works/{}/units?limit=1", work.work_id));
    let page = send(
        &live.router,
        "GET",
        &page_path,
        &[bearer(&viewer.access)],
        None,
    )
    .await;
    assert_eq!(page.status, StatusCode::OK);
    let old_cursor = page.json()["next_cursor"].as_str().unwrap().to_owned();
    let immutable = publication_fingerprint(&fixture).await;
    drop(live);
    drop(public);
    identity.terminate().await;
    let before = checkpoint(&fixture, &env, &run, "before", subject).await;
    assert_eq!(before.security.state, AccountState::Active);

    let identity = env.serve().await;
    change(
        &fixture,
        &work.work_id,
        Change::RevokeGrant { grant_id: grant },
    )
    .await;
    let live = restricted_host(&fixture, &env, objects.clone());
    refused_fetch(&live, &old_media, &viewer.access).await;
    let logout = send(
        &live.router,
        "DELETE",
        &theatre("/native/identity/session"),
        &[bearer(&viewer.access)],
        None,
    )
    .await;
    assert_eq!(logout.status, StatusCode::NO_CONTENT);
    assert_eq!(
        security_row(&fixture, &viewer.session).await.as_deref(),
        Some("logout")
    );
    drop(live);
    identity.terminate().await;
    let owner = IdentityStore::open(&env.database, Intent::Serve, Arc::new(SystemClock))
        .await
        .unwrap();
    owner
        .disable_subject(
            OperationId::from_bytes(*Uuid::new_v4().as_bytes()).unwrap(),
            subject,
            before.security.version,
            before.security.epoch,
        )
        .await
        .unwrap();
    drop(owner);
    let after = checkpoint(&fixture, &env, &run, "after", subject).await;
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM catalog_viewer_grant_revocations", &[])
            .await,
        1
    );
    assert_eq!(after.security.state, AccountState::Disabled);
    assert_eq!(
        after.security.version.get(),
        before.security.version.get() + 1
    );
    assert_eq!(after.security.epoch, before.security.epoch);
    assert_eq!(publication_fingerprint(&fixture).await, immutable);
    fixture.close();

    let mut observations = Vec::new();
    for (name, server, identity, expected_session, expected_revoke) in [
        (
            "current_pair",
            &after.server,
            &after.identity,
            StatusCode::UNAUTHORIZED,
            Some("logout"),
        ),
        (
            "old_server_current_identity",
            &before.server,
            &after.identity,
            StatusCode::UNAUTHORIZED,
            None,
        ),
        // This is deliberately a characterization of the unresolved recovery hazard, not
        // an accepted session outcome. Never opt this restored pair into private serving.
        (
            "stale_pair_UNSAFE",
            &before.server,
            &before.identity,
            StatusCode::OK,
            None,
        ),
    ] {
        let restored = ServerFixture::from_verified_backup(server).await;
        assert_eq!(publication_fingerprint(&restored).await, immutable);
        assert_eq!(
            security_row(&restored, &viewer.session).await.as_deref(),
            expected_revoke
        );
        let grant_revocations = restored
            .count("SELECT count(*) FROM catalog_viewer_grant_revocations", &[])
            .await;
        assert_eq!(grant_revocations, i64::from(name == "current_pair"));
        let restored_run = RunDir::new();
        let mut restored_env = env.clone();
        restored_env.database = restored_run.path("identity.db");
        restored_env.socket = restored_run.path("identity.sock");
        closed_snapshot_copy(identity, &restored_env.database);
        let facts = identity_security(&restored_env.database, subject).await;
        let process = restored_env.serve().await;
        let host = closed_host(&restored, &restored_env, objects.clone());
        let session = send(
            &host.router,
            "GET",
            &theatre("/native/identity/session"),
            &[bearer(&viewer.access)],
            None,
        )
        .await;
        assert_eq!(
            session.status, expected_session,
            "{name}: exact recovery observation"
        );
        let expected_private = if expected_session == StatusCode::OK {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::UNAUTHORIZED
        };
        for path in [
            theatre("/shared/works"),
            theatre(&format!("/shared/works/{}", work.work_id)),
            format!("{page_path}&cursor={old_cursor}"),
        ] {
            let reply = send(&host.router, "GET", &path, &[bearer(&viewer.access)], None).await;
            assert_eq!(
                reply.status, expected_private,
                "{name}: restored shared routes stay closed"
            );
            assert!(!String::from_utf8_lossy(&reply.body).contains("RECOVERY_PRIVATE_SENTINEL"));
        }
        for (method, extra) in [
            ("GET", vec![]),
            ("HEAD", vec![]),
            ("GET", vec![("range", "bytes=0-9".into())]),
            ("GET", vec![("if-none-match", "*".into())]),
        ] {
            let mut headers = vec![bearer(&viewer.access)];
            headers.extend(extra);
            let reply = send(&host.router, method, &old_media, &headers, None).await;
            assert_eq!(
                reply.status, expected_private,
                "{name}: old private media stays closed"
            );
            assert_ne!(reply.header("content-type"), Some("audio/ogg"));
        }
        let discovery = send(&host.router, "GET", &theatre("/works"), &[], None).await;
        assert_eq!(discovery.status, StatusCode::OK);
        assert!(discovery.json()["items"].as_array().unwrap().is_empty());
        let public_detail = send(
            &host.router,
            "GET",
            &theatre(&format!("/works/{}", work.work_id)),
            &[],
            None,
        )
        .await;
        assert_eq!(public_detail.status, StatusCode::NOT_FOUND);
        observations.push(json!({"cut":name,"session_http":session.status.as_u16(),"grant_revocations":grant_revocations,
            "private_http":expected_private.as_u16(), "identity_epoch":format!("{:032x}",u128::from_be_bytes(facts.epoch.to_bytes())),
            "identity_security_version":facts.version.get(), "identity_disabled":facts.state == AccountState::Disabled,
            "restricted_activation":false,"security_recovery":if expected_session == StatusCode::OK {"FAIL: revoked session resurrected; epoch rotation/resync unsupported"} else {"bounded current-revocation denial"}}));
        drop(host);
        process.terminate().await;
    }
    let report = json!({"task":658,"engine":"turso=0.8.3",
        "server_schema":cantos_server::platform::turso::registry().unwrap().migrations().len(),
        "identity_schema":cantos_identity::persistence::registry().unwrap().migrations().len(),
        "method":"quiesced independent verified backup_to; closed snapshots copied only to new private roots",
        "atomic_pair":false,"server_binary_tcp":false,"identity_process_rpc":true,"media_decode":false,
        "before":{"server":receipt(&before.server),"identity":receipt(&before.identity)},
        "after":{"server":receipt(&after.server),"identity":receipt(&after.identity)},
        "immutable_publication":immutable,"observations":observations,"readiness":"NO_GO",
        "blocker":"No restore epoch rotation/Server resync or external revocation floor. Default restricted OFF is not full session recovery.",
        "rpo_rto_retention":"PENDING owner acceptance; not measured by this drill"});
    if let Some(path) = std::env::var_os("CANTOS_TEST_RECOVERY_RECEIPT") {
        let path = std::path::PathBuf::from(path);
        assert!(
            path.is_absolute() && !path.exists(),
            "receipt must be a new absolute task-owned file"
        );
        std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    println!("S12-RECOVERY: private fence PASS; stale-pair session safety FAIL; readiness NO_GO");
}

#[tokio::test]
async fn current_backup_refuses_pre_epic_binary_and_wrong_service_without_rewriting_bytes() {
    use cantos_store_turso::{
        error::OpenError,
        registry::{Refusal, Registry},
    };

    let fixture = ServerFixture::new().await;
    let run = RunDir::new();
    let backup = fixture.backup_to(&run.path("server.db")).await;
    let schema = cantos_server::platform::turso::registry().unwrap();
    let old = Registry::new(
        schema.application_id(),
        schema.epoch(),
        &schema.migrations()[..30],
    )
    .unwrap();
    for (registry, expected) in [
        (
            old,
            Refusal::NewerThanBinary {
                applied: 35,
                known: 30,
            },
        ),
        (
            cantos_identity::persistence::registry().unwrap(),
            Refusal::ForeignApplication {
                found: schema.application_id(),
            },
        ),
    ] {
        for intent in [Intent::Serve, Intent::Migrate] {
            let result = OwnedDatabase::open(OpenRequest::new(
                &backup.path,
                registry,
                intent,
                Arc::new(SystemClock),
            ))
            .await;
            match result {
                Err(error) => assert_eq!(error, OpenError::Refused(expected)),
                Ok(_) => panic!("an incompatible owner must not adopt the snapshot"),
            }
            let bytes = std::fs::read(&backup.path).unwrap();
            assert_eq!(bytes.len() as u64, backup.bytes);
            assert_eq!(<[u8; 32]>::from(Sha256::digest(bytes)), backup.sha256);
        }
    }
}
