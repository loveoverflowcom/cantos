//! Disposable PostgreSQL only, enabled by cargo xtask verify postgres.
use std::{env, time::Duration};

use cantos_api::{
    platform::{
        identity::{
            domain::{ActorId, ExternalIdentityKey, IdentityError, LinkOutcome, Registration},
            IdentityService, PostgresIdentityRegistry, RegistryError,
        },
        postgres::{local_config, migrate, token_hash, Store},
    },
    script_ir::read_script,
};
use cantos_contracts::SaveRevisionRequest;
use tokio_postgres::{Client, Config, NoTls};
use uuid::Uuid;

const SCRIPT: &str =
    include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
const TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn actor(value: &str) -> ActorId {
    ActorId::new(value).unwrap()
}
fn key(issuer: &str, subject: &str) -> ExternalIdentityKey {
    ExternalIdentityKey::new(issuer, subject).unwrap()
}

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

struct Harness {
    admin: Client,
    config: Config,
    service: IdentityService<PostgresIdentityRegistry>,
}

impl Harness {
    async fn new() -> Self {
        let url = env::var("CANTOS_TEST_CLUSTER_URL").expect("use cargo xtask verify postgres");
        let mut config = local_config(&url).unwrap();
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let db = format!("cantos_test_{}", Uuid::new_v4().simple());
        connect(&config)
            .await
            .batch_execute(&format!("CREATE DATABASE {db}"))
            .await
            .unwrap();
        config.dbname(&db);
        let admin = connect(&config).await;
        migrate(&config).await.unwrap();
        let service = IdentityService::new(PostgresIdentityRegistry::new(
            Store::new(config.clone()).unwrap(),
        ));
        Self {
            admin,
            config,
            service,
        }
    }

    async fn counts(&self) -> (i64, i64) {
        let row = self.admin.query_one("SELECT (SELECT count(*) FROM actors),(SELECT count(*) FROM actor_external_identities)", &[]).await.unwrap();
        (row.get(0), row.get(1))
    }
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL via cargo xtask verify postgres"]
async fn baseline_replay_preserves_ownership_sessions_and_immutable_history() {
    let h = Harness::new().await;
    h.admin
        .batch_execute("INSERT INTO actors(id) VALUES('legacy');")
        .await
        .unwrap();
    h.admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,'legacy',clock_timestamp()+interval '1 hour')", &[&token_hash(TOKEN)]).await.unwrap();
    for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
        h.admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('legacy',$1,$2,'Synthetic migration fixture')", &[&kind,&id]).await.unwrap();
    }
    let store = Store::new(h.config.clone()).unwrap();
    let script_id = Uuid::new_v4().to_string();
    let saved = store
        .save(
            TOKEN,
            &script_id,
            SaveRevisionRequest {
                expected_revision: 0,
                operation_id: Uuid::new_v4().to_string(),
                script_json: SCRIPT.into(),
            },
        )
        .await
        .unwrap();
    let before = h.admin.query_one("SELECT canonical_export,accepted_at,accepted_by,export_digest FROM script_revisions WHERE script_id=$1", &[&script_id]).await.unwrap();
    // Preserve timestamps as server-rendered text to avoid adding a temporal dependency.
    let timestamp: String = h
        .admin
        .query_one(
            "SELECT accepted_at::text FROM script_revisions WHERE script_id=$1",
            &[&script_id],
        )
        .await
        .unwrap()
        .get(0);
    migrate(&h.config).await.unwrap();
    migrate(&h.config).await.unwrap();
    assert_eq!(
        h.service
            .link(&key("issuer", "legacy-sub"), &actor("legacy"))
            .await,
        Ok(LinkOutcome::Linked)
    );
    let after = h.admin.query_one("SELECT canonical_export,accepted_at::text,accepted_by,export_digest FROM script_revisions WHERE script_id=$1", &[&script_id]).await.unwrap();
    assert_eq!(before.get::<_, Vec<u8>>(0), after.get::<_, Vec<u8>>(0));
    assert_eq!(timestamp, after.get::<_, String>(1));
    assert_eq!(after.get::<_, String>(2), "legacy");
    assert_eq!(before.get::<_, String>(3), after.get::<_, String>(3));
    assert_eq!(store.authenticate(TOKEN).await.unwrap(), "legacy");
    assert_eq!(
        store
            .load(TOKEN, &script_id, None)
            .await
            .unwrap()
            .export_digest,
        saved.export_digest
    );
    let row = h
        .admin
        .query_one(
            "SELECT count(*),min(version),min(octet_length(checksum)) FROM cantos_migrations",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i64>(0), 3);
    assert_eq!(row.get::<_, i32>(1), 1);
    assert_eq!(row.get::<_, i32>(2), 32);
    assert_eq!(h.counts().await, (1, 1));
    for value in ["\u{0085}", "\u{2003}", "a\u{009f}b", "\u{3000}"] {
        for sql in [
            "INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES($1,'valid','legacy')",
            "INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES('valid',$1,'legacy')",
        ] {
            assert_eq!(
                h.admin
                    .execute(sql, &[&value])
                    .await
                    .unwrap_err()
                    .code()
                    .unwrap()
                    .code(),
                "23514"
            );
        }
    }
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL via cargo xtask verify postgres"]
async fn identity_concurrent_registration_and_acknowledgement_loss_have_no_orphan_actor() {
    let h = Harness::new().await;
    let k = key("issuer-a", "sub");
    let candidates = [actor("alice"), actor("bob")];
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut tasks = Vec::new();
    for id in candidates {
        let registry = IdentityService::new(PostgresIdentityRegistry::new(
            Store::new(h.config.clone()).unwrap(),
        ));
        let key = k.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            registry.register(&key, &id).await.unwrap()
        }));
    }
    let a = tasks.remove(0).await.unwrap();
    let b = tasks.remove(0).await.unwrap();
    assert_eq!(a.actor_id(), b.actor_id());
    assert!(matches!(
        (&a, &b),
        (Registration::Created(_), Registration::Existing(_))
            | (Registration::Existing(_), Registration::Created(_))
    ));
    // Discard the acknowledged first result; a fresh pool resolves the durable winner.
    let fresh = IdentityService::new(PostgresIdentityRegistry::new(
        Store::new(h.config.clone()).unwrap(),
    ));
    assert_eq!(
        fresh.register(&k, &actor("discarded-retry")).await.unwrap(),
        Registration::Existing(a.actor_id().clone())
    );
    assert_eq!(h.counts().await, (1, 1));
    assert_eq!(
        fresh
            .register(&key("issuer-b", "sub"), &actor("other"))
            .await,
        Ok(Registration::Created(actor("other")))
    );
    assert_eq!(
        fresh
            .register(&key("issuer-a", "Sub"), &actor("case"))
            .await,
        Ok(Registration::Created(actor("case")))
    );
    assert_eq!(h.counts().await, (3, 3));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL via cargo xtask verify postgres"]
async fn identity_linking_collision_and_disable_are_fail_closed() {
    let h = Harness::new().await;
    h.admin
        .batch_execute(
            "INSERT INTO actors(id,active) VALUES('legacy',true),('inactive',false),('other',true)",
        )
        .await
        .unwrap();
    let k = key("issuer", "s");
    assert_eq!(
        h.service.register(&k, &actor("legacy")).await,
        Err(IdentityError::ActorIdCollision.into())
    );
    assert_eq!(
        h.service.link(&k, &actor("absent")).await,
        Err(IdentityError::ActorMissing.into())
    );
    assert_eq!(
        h.service.link(&k, &actor("inactive")).await,
        Err(IdentityError::ActorInactive.into())
    );
    assert_eq!(h.counts().await, (3, 0));
    assert_eq!(
        h.service.link(&k, &actor("legacy")).await,
        Ok(LinkOutcome::Linked)
    );
    assert_eq!(
        h.service.link(&k, &actor("legacy")).await,
        Ok(LinkOutcome::Existing)
    );
    assert_eq!(
        h.service
            .link(&key("issuer-b", "s"), &actor("legacy"))
            .await,
        Ok(LinkOutcome::Linked)
    );
    assert_eq!(
        h.service.link(&k, &actor("other")).await,
        Err(IdentityError::IdentityAlreadyBound.into())
    );
    h.admin
        .batch_execute("UPDATE actors SET active=false WHERE id='legacy'")
        .await
        .unwrap();
    assert_eq!(
        h.service.register(&k, &actor("new")).await,
        Err(IdentityError::ActorInactive.into())
    );
    assert_eq!(h.counts().await, (3, 2));
    assert!(!h.service.lookup(&k).await.unwrap().unwrap().active);
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL via cargo xtask verify postgres"]
async fn identity_mutations_recheck_disable_after_actor_lock_wait() {
    for replay_registration in [true, false] {
        let mut h = Harness::new().await;
        let bound_key = key("issuer", "bound");
        let new_key = key("issuer", "new");
        h.service
            .register(&bound_key, &actor("existing"))
            .await
            .unwrap();
        let observer = connect(&h.config).await;
        let blocker: i32 = h
            .admin
            .query_one("SELECT pg_backend_pid()", &[])
            .await
            .unwrap()
            .get(0);
        let disable = h.admin.transaction().await.unwrap();
        disable
            .execute("UPDATE actors SET active=false WHERE id='existing'", &[])
            .await
            .unwrap();

        let application_name = format!("identity-wait-{}", Uuid::new_v4().simple());
        let mut waiting_config = h.config.clone();
        waiting_config.application_name(&application_name);
        let service = IdentityService::new(PostgresIdentityRegistry::new(
            Store::new(waiting_config).unwrap(),
        ));
        let waiting_key = if replay_registration {
            bound_key.clone()
        } else {
            new_key.clone()
        };
        let waiting = tokio::spawn(async move {
            if replay_registration {
                service
                    .register(&waiting_key, &actor("discarded-candidate"))
                    .await
                    .map(|_| ())
            } else {
                service
                    .link(&waiting_key, &actor("existing"))
                    .await
                    .map(|_| ())
            }
        });

        // Observe the actual actor-lock interleaving before releasing the disable transaction.
        // Separate autocommit reads avoid retaining a stale pg_stat_activity snapshot.
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let blocked: bool = observer
                    .query_one(
                        "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock' AND $2=ANY(pg_blocking_pids(pid)))",
                        &[&application_name, &blocker],
                    )
                    .await
                    .unwrap()
                    .get(0);
                if blocked {
                    break;
                }
                assert!(
                    !waiting.is_finished(),
                    "registry mutation completed before waiting for the actor lock"
                );
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("registry mutation must wait for the actor lock before its 3s lock deadline");

        disable.commit().await.unwrap();
        assert_eq!(
            waiting.await.unwrap(),
            Err(RegistryError::Rejected(IdentityError::ActorInactive))
        );
        assert_eq!(h.counts().await, (1, 1));
        let binding = h.service.lookup(&bound_key).await.unwrap().unwrap();
        assert_eq!(binding.id, actor("existing"));
        assert!(!binding.active);
        assert_eq!(h.service.lookup(&new_key).await.unwrap(), None);
    }
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL via cargo xtask verify postgres"]
async fn identity_constraints_immutability_rollback_and_http_role_isolation_hold() {
    let mut h = Harness::new().await;
    h.service
        .register(&key("issuer", "s"), &actor("a"))
        .await
        .unwrap();
    // Include the new referencing flow table so PostgreSQL reaches the immutable trigger;
    // omitting it refuses earlier with0A000 and does not exercise the55000 guard.
    for (sql,code) in [
        ("INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES('issuer','s','a')", "23505"),
        ("INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES('new','s','absent')", "23503"),
        ("INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES('','s','a')", "23514"),
        ("UPDATE actor_external_identities SET actor_id='a'", "55000"),
        ("DELETE FROM actor_external_identities", "55000"),
        ("TRUNCATE actor_external_identities,studio_oidc_sessions,theatre_identity_sessions,theatre_identity_flows,theatre_native_sessions,bearer_families,bearer_tokens", "55000"),
    ] {
        assert_eq!(h.admin.batch_execute(sql).await.unwrap_err().code().unwrap().code(),code);
    }
    let tx = h.admin.transaction().await.unwrap();
    tx.batch_execute("INSERT INTO actors(id) VALUES('rolled-back'); INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES('rollback','s','rolled-back')").await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(h.counts().await, (1, 1));
    let mut app_config = h.config.clone();
    app_config.user("cantos_app");
    let app = connect(&app_config).await;
    for sql in [
        "SELECT * FROM actor_external_identities",
        "INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES('x','y','a')",
    ] {
        assert_eq!(
            app.batch_execute(sql)
                .await
                .unwrap_err()
                .code()
                .unwrap()
                .code(),
            "42501"
        );
    }
    let registry = IdentityService::new(PostgresIdentityRegistry::new(
        Store::new(app_config).unwrap(),
    ));
    assert_eq!(
        registry
            .register(&key("not-authorized", "s"), &actor("b"))
            .await,
        Err(RegistryError::Unavailable)
    );
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL via cargo xtask verify postgres"]
async fn identity_adapter_rolls_back_actor_when_binding_insert_fails() {
    let h = Harness::new().await;
    h.admin.batch_execute("CREATE TRIGGER identity_test_refuse_insert BEFORE INSERT ON actor_external_identities FOR EACH ROW EXECUTE FUNCTION reject_settled_change()").await.unwrap();
    let k = key("issuer", "rollback");
    assert_eq!(
        h.service.register(&k, &actor("candidate")).await,
        Err(RegistryError::Unavailable)
    );
    assert_eq!(h.counts().await, (0, 0));
    h.admin
        .batch_execute("DROP TRIGGER identity_test_refuse_insert ON actor_external_identities")
        .await
        .unwrap();
    assert_eq!(
        h.service.register(&k, &actor("candidate")).await,
        Ok(Registration::Created(actor("candidate")))
    );
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL via cargo xtask verify postgres"]
async fn identity_candidate_collision_across_distinct_keys_has_one_winner() {
    let h = Harness::new().await;
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut tasks = Vec::new();
    for subject in ["one", "two"] {
        let service = IdentityService::new(PostgresIdentityRegistry::new(
            Store::new(h.config.clone()).unwrap(),
        ));
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            service
                .register(&key("issuer", subject), &actor("shared-candidate"))
                .await
        }));
    }
    let a = tasks.remove(0).await.unwrap();
    let b = tasks.remove(0).await.unwrap();
    assert!(matches!(
        (a, b),
        (
            Ok(Registration::Created(_)),
            Err(RegistryError::Rejected(IdentityError::ActorIdCollision))
        ) | (
            Err(RegistryError::Rejected(IdentityError::ActorIdCollision)),
            Ok(Registration::Created(_))
        )
    ));
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL via cargo xtask verify postgres"]
async fn identity_operator_commands_consume_the_real_registry_without_issuing_a_session() {
    let h = Harness::new().await;
    let host = match &h.config.get_hosts()[0] {
        tokio_postgres::config::Host::Tcp(host) => host,
        _ => panic!("disposable harness uses TCP"),
    };
    let url = format!(
        "postgresql://cantos_test_admin@{host}:{}/{}",
        h.config.get_ports()[0],
        h.config.get_dbname().unwrap()
    );
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_cantos-operator"))
            .args(args)
            .env("DATABASE_URL", &url)
            .output()
            .unwrap()
    };
    let first = run(&["register-identity", "issuer", "cli-subject"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let id = String::from_utf8(first.stdout).unwrap();
    assert_eq!(
        run(&["register-identity", "issuer", "cli-subject"]).stdout,
        id.as_bytes()
    );
    assert_eq!(
        run(&["link-identity", id.trim(), "other-issuer", "new-subject"]).stdout,
        b"Linked\n"
    );
    assert_eq!(
        run(&["link-identity", id.trim(), "other-issuer", "new-subject"]).stdout,
        b"Existing\n"
    );
    assert_eq!(h.counts().await, (1, 2));
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM sessions", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL via cargo xtask verify postgres"]
async fn identity_exact_lookup_uses_primary_key_without_an_incremental_view() {
    let h = Harness::new().await;
    h.admin.batch_execute("INSERT INTO actors(id) VALUES('indexed'); INSERT INTO actor_external_identities(issuer,subject,actor_id) SELECT 'synthetic-issuer-'||n,'subject','indexed' FROM generate_series(1,2000) n; ANALYZE actor_external_identities; ANALYZE actors;").await.unwrap();
    let issuer = "synthetic-issuer-1750";
    let subject = "subject";
    let rows = h.admin.query("EXPLAIN (ANALYZE,BUFFERS) SELECT a.id,a.active FROM actor_external_identities e JOIN actors a ON a.id=e.actor_id WHERE e.issuer=$1 AND e.subject=$2", &[&issuer,&subject]).await.unwrap();
    let plan = rows
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(plan.contains("actor_external_identities_pkey"), "{plan}");
    let k = key(issuer, subject);
    assert_eq!(
        h.service.lookup(&k).await.unwrap().unwrap().id,
        actor("indexed")
    );
    let mut samples = Vec::new();
    for _ in 0..20 {
        let start = std::time::Instant::now();
        let record = h.service.lookup(&k).await.unwrap().unwrap();
        assert_eq!(record.id, actor("indexed"));
        assert!(record.active);
        samples.push(start.elapsed().as_micros());
    }
    println!("Identity synthetic warm lookup: 2000 bindings / 1 actor / concurrency1 /20 samples(us): {samples:?}\n{plan}");
    assert_eq!(
        h.service
            .lookup(&key("synthetic-issuer-1750", "Subject"))
            .await
            .unwrap(),
        None
    );
}
