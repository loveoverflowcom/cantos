//! The candidate `server.db` schema on the real pinned engine and run-owned files: the slice it
//! creates, the files it refuses without adopting them, and the database-level barrier behind
//! the actor registry's domain constructors. No constraint is weakened to build a fixture.
use std::{collections::BTreeSet, path::Path, sync::Arc};

use cantos_store_turso::{
    clock::Clock,
    error::{ConstraintKind, GuardCode, OpenError, StoreFault},
    params_from_iter,
    registry::{DatabaseKind, Intent, Migration, Refusal, Registry},
    testing::{ledger_registry, FixedClock, TestRun},
    OpenRequest, OwnedDatabase, Value,
};

use super::{ServerDatabase, ServerDatabaseError, MIGRATIONS, SERVER_EPOCH};
use crate::platform::identity::domain::{ActorId, ExternalIdentityKey};

const T0: i64 = 1_700_000_000_000_000;

static FUTURE: [Migration; 2] = [
    MIGRATIONS[0],
    Migration {
        version: 2,
        name: "server_future_slice",
        sql: "CREATE TABLE future_slice(id INTEGER PRIMARY KEY) STRICT;",
    },
];

fn clock() -> Arc<dyn Clock> {
    FixedClock::new(T0)
}

async fn open(path: &Path, intent: Intent) -> Result<ServerDatabase, ServerDatabaseError> {
    ServerDatabase::open(path, intent, clock()).await
}

async fn refused(path: &Path, intent: Intent) -> ServerDatabaseError {
    match open(path, intent).await {
        Ok(_) => panic!("{} must be refused", path.display()),
        Err(error) => error,
    }
}

/// Create a file with any registry through the shared owner, as another binary would.
async fn foreign(path: &Path, registry: Registry) {
    let owner = OwnedDatabase::open(OpenRequest::new(path, registry, Intent::Migrate, clock()))
        .await
        .unwrap();
    drop(owner);
}

/// Main file and write-ahead log bytes; a refused open must change neither.
fn bytes(path: &Path) -> (Vec<u8>, Option<Vec<u8>>) {
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    (std::fs::read(path).unwrap(), std::fs::read(wal).ok())
}

async fn values(database: &ServerDatabase, sql: &str) -> Vec<Vec<Value>> {
    let reader = database.owner().reader().await.unwrap();
    let mut rows = reader.query(sql, ()).await.unwrap();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        out.push(
            (0..rows.column_count())
                .map(|index| row.get_value(index).unwrap())
                .collect(),
        );
    }
    out
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn guard(code: &str) -> StoreFault {
    StoreFault::Guard(GuardCode::new(code).unwrap())
}

fn constraint(kind: ConstraintKind) -> StoreFault {
    StoreFault::Constraint { kind, target: None }
}

/// Run one statement in its own transaction and roll it back, returning the engine's answer.
async fn attempt(
    database: &ServerDatabase,
    sql: &str,
    params: Vec<Value>,
) -> Result<u64, StoreFault> {
    let tx = database.owner().begin_write().await.unwrap();
    let result = tx.execute(sql, params_from_iter(params)).await;
    tx.rollback().await.unwrap();
    result
}

async fn commit(database: &ServerDatabase, statements: &[&str]) {
    let tx = database.owner().begin_write().await.unwrap();
    for sql in statements {
        tx.execute(sql, ()).await.unwrap();
    }
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn a_new_file_holds_only_the_registry_metadata_and_the_actor_registry_slice() {
    let run = TestRun::new("server-schema-slice");
    let path = run.path("server.db");
    drop(open(&path, Intent::Migrate).await.unwrap());
    let database = open(&path, Intent::Serve).await.unwrap();
    assert_eq!(
        values(&database, "PRAGMA application_id").await,
        [[Value::Integer(i64::from(
            DatabaseKind::Server.application_id()
        ))]]
    );
    assert_eq!(
        values(&database, "PRAGMA user_version").await,
        [[Value::Integer(i64::from(SERVER_EPOCH))]]
    );
    let objects = values(
        &database,
        "SELECT type,name FROM sqlite_schema WHERE substr(name,1,7) <> 'sqlite_' ORDER BY type,name",
    )
    .await;
    let expected = [
        ("index", "actor_external_identities_actor"),
        ("table", "actor_external_identities"),
        ("table", "actors"),
        ("table", "cantos_schema_history"),
        ("trigger", "actor_external_identities_no_delete"),
        ("trigger", "actor_external_identities_no_replace"),
        ("trigger", "actor_external_identities_no_update"),
        ("trigger", "actors_id_immutable"),
        ("trigger", "actors_no_delete"),
        ("trigger", "actors_no_replace"),
        ("trigger", "cantos_schema_history_no_delete"),
        ("trigger", "cantos_schema_history_no_update"),
    ]
    .map(|(kind, name)| vec![text(kind), text(name)]);
    assert_eq!(objects, expected);
    assert_eq!(
        values(
            &database,
            "SELECT version,name,checksum,applied_at FROM cantos_schema_history"
        )
        .await,
        [[
            Value::Integer(1),
            text("server_actor_registry"),
            Value::Blob(MIGRATIONS[0].checksum().to_vec()),
            Value::Integer(T0),
        ]]
    );
}

#[tokio::test]
async fn foreign_diverged_newer_other_epoch_and_non_database_files_are_refused_unchanged() {
    let run = TestRun::new("server-schema-refusals");
    let missing = run.path("missing.db");
    assert_eq!(
        refused(&missing, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::Missing))
    );
    assert!(!missing.exists());

    let cases = [
        (
            "identity.db",
            Registry::new(DatabaseKind::Identity, SERVER_EPOCH, &MIGRATIONS).unwrap(),
            Refusal::ForeignApplication {
                found: DatabaseKind::Identity.application_id(),
            },
        ),
        (
            "fixture.db",
            ledger_registry(DatabaseKind::Server),
            Refusal::HistoryDiverged { version: 1 },
        ),
        (
            "epoch.db",
            Registry::new(DatabaseKind::Server, SERVER_EPOCH + 1, &MIGRATIONS).unwrap(),
            Refusal::EpochMismatch {
                found: 2,
                expected: SERVER_EPOCH,
            },
        ),
        (
            "newer.db",
            Registry::new(DatabaseKind::Server, SERVER_EPOCH, &FUTURE).unwrap(),
            Refusal::NewerThanBinary {
                applied: 2,
                known: 1,
            },
        ),
    ];
    for (name, registry, refusal) in cases {
        let path = run.path(name);
        foreign(&path, registry).await;
        let before = bytes(&path);
        for intent in [Intent::Migrate, Intent::Serve] {
            assert_eq!(
                refused(&path, intent).await,
                ServerDatabaseError::Open(OpenError::Refused(refusal)),
                "{name}"
            );
        }
        assert_eq!(bytes(&path), before, "{name} must not be adopted");
    }

    let garbage = run.path("garbage.db");
    std::fs::write(&garbage, [0x5a_u8; 8192]).unwrap();
    let error = refused(&garbage, Intent::Migrate).await;
    assert!(
        matches!(
            error,
            ServerDatabaseError::Open(OpenError::Store(StoreFault::NotDatabase))
        ),
        "{error:?}"
    );
    assert_eq!(std::fs::read(&garbage).unwrap(), vec![0x5a_u8; 8192]);
}

#[tokio::test]
async fn a_second_owner_is_refused_while_the_file_is_open() {
    let run = TestRun::new("server-schema-owner");
    let path = run.path("server.db");
    let first = open(&path, Intent::Migrate).await.unwrap();
    assert_eq!(
        refused(&path, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::AlreadyOwned)
    );
    drop(first);
    drop(open(&path, Intent::Serve).await.unwrap());
}

#[tokio::test]
async fn actor_id_checks_admit_exactly_what_the_domain_constructor_admits() {
    let run = TestRun::new("server-schema-actor-ids");
    let database = open(&run.path("server.db"), Intent::Migrate).await.unwrap();
    let mut inputs: BTreeSet<String> = (0_u8..=0x7f)
        .flat_map(|byte| {
            let c = char::from(byte);
            [c.to_string(), format!("a{c}"), format!("{c}9")]
        })
        .collect();
    inputs.extend(
        [
            "",
            "Đ",
            "é",
            "ß",
            "ａ",
            "a\u{0}",
            &"a".repeat(64),
            &"a".repeat(65),
            "creator-01",
        ]
        .map(str::to_owned),
    );
    let tx = database.owner().begin_write().await.unwrap();
    for input in &inputs {
        let stored = tx
            .execute("INSERT INTO actors(id) VALUES(?1)", [input.as_str()])
            .await;
        match ActorId::new(input.as_str()) {
            Ok(_) => assert_eq!(stored, Ok(1), "{input:?}"),
            Err(_) => assert_eq!(stored, Err(constraint(ConstraintKind::Check)), "{input:?}"),
        }
    }
    tx.rollback().await.unwrap();
}

/// Every C0/C1 control, NUL and Latin-1 scalar, each Unicode space the PostgreSQL classes name
/// with both neighbours, Vietnamese letters and combining marks, and private/BOM/astral scalars.
fn boundary_chars() -> Vec<char> {
    let mut points: Vec<u32> = (0..=0xff).collect();
    points.extend(0x1fff..=0x200b);
    for space in [0x1680, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000] {
        points.extend([space - 1, space, space + 1]);
    }
    points.extend([
        0x0110, 0x0111, 0x01a0, 0x01b0, 0x0300, 0x0323, 0x0e01, 0x1ea1, 0x1ec7, 0x1ef9, 0xe000,
        0xfeff, 0xfffd, 0x1_f600, 0x10_ffff,
    ]);
    points.into_iter().filter_map(char::from_u32).collect()
}

#[tokio::test]
async fn external_key_checks_admit_exactly_what_the_domain_constructor_admits() {
    let run = TestRun::new("server-schema-external-keys");
    let database = open(&run.path("server.db"), Intent::Migrate).await.unwrap();
    commit(&database, &["INSERT INTO actors(id) VALUES('a')"]).await;
    let mut parts: BTreeSet<String> = boundary_chars()
        .into_iter()
        .flat_map(|c| {
            let mut forms = vec![c.to_string(), format!("x{c}y")];
            if c.is_whitespace() {
                forms.push(format!("{c}{c}"));
            }
            forms
        })
        .collect();
    for (unit, count) in [("x", 512), ("Đ", 256), ("x", 1024), ("Đ", 512)] {
        let part = unit.repeat(count);
        parts.insert(format!("{part}x"));
        parts.insert(part);
    }
    let tx = database.owner().begin_write().await.unwrap();
    let insert = "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES(?1,?2,'a',0)";
    for (index, part) in parts.iter().enumerate() {
        let unique = format!("unique-{index}");
        for (issuer, subject) in [
            (part.as_str(), unique.as_str()),
            (unique.as_str(), part.as_str()),
        ] {
            let stored = tx.execute(insert, [issuer, subject]).await;
            match ExternalIdentityKey::new(issuer, subject) {
                Ok(_) => assert_eq!(stored, Ok(1), "{issuer:?} / {subject:?}"),
                Err(_) => assert_eq!(
                    stored,
                    Err(constraint(ConstraintKind::Check)),
                    "{issuer:?} / {subject:?}"
                ),
            }
        }
    }
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn actor_rows_keep_their_id_forever_and_store_only_a_boolean_active_flag() {
    let run = TestRun::new("server-schema-actors");
    let database = open(&run.path("server.db"), Intent::Migrate).await.unwrap();
    commit(&database, &["INSERT INTO actors(id) VALUES('alice')"]).await;
    assert_eq!(
        values(&database, "SELECT id,active FROM actors").await,
        [[text("alice"), Value::Integer(1)]]
    );
    let insert = "INSERT INTO actors(id,active) VALUES(?1,?2)";
    for (active, fault) in [
        (Value::Integer(2), constraint(ConstraintKind::Check)),
        (Value::Integer(-1), constraint(ConstraintKind::Check)),
        (
            Value::Null,
            StoreFault::Constraint {
                kind: ConstraintKind::NotNull,
                target: Some("actors.active".into()),
            },
        ),
    ] {
        assert_eq!(
            attempt(&database, insert, vec![text("bob"), active.clone()]).await,
            Err(fault),
            "{active:?}"
        );
    }
    // STRICT refuses a non-integer; it stores a lossless integer spelling as that integer.
    for active in [text("yes"), Value::Blob(vec![1]), Value::Real(0.5)] {
        assert!(
            attempt(&database, insert, vec![text("bob"), active.clone()])
                .await
                .is_err(),
            "{active:?}"
        );
    }
    commit(&database, &["UPDATE actors SET active=0 WHERE id='alice'"]).await;
    // An insert over an existing id is refused before conflict resolution: REPLACE would
    // otherwise delete the row without firing the delete guard and reset `active`.
    for sql in [
        "INSERT INTO actors(id) VALUES('alice')",
        "INSERT INTO actors(id) VALUES('alice') ON CONFLICT(id) DO NOTHING",
        "INSERT OR REPLACE INTO actors(id,active) VALUES('alice',1)",
        "REPLACE INTO actors(id) VALUES('alice')",
        "UPDATE actors SET id='alice2' WHERE id='alice'",
        "DELETE FROM actors WHERE id='alice'",
        "DELETE FROM actors",
    ] {
        assert_eq!(
            attempt(&database, sql, Vec::new()).await,
            Err(guard("immutable.actor_id")),
            "{sql}"
        );
    }
    // Operator deactivation and reactivation stay possible; they change nothing else.
    assert_eq!(
        values(&database, "SELECT id,active FROM actors").await,
        [[text("alice"), Value::Integer(0)]]
    );
    commit(&database, &["UPDATE actors SET active=1 WHERE id='alice'"]).await;
    assert_eq!(
        values(&database, "SELECT id,active FROM actors").await,
        [[text("alice"), Value::Integer(1)]]
    );
}

#[tokio::test]
async fn bindings_are_exact_byte_keys_reference_an_actor_and_never_change() {
    let run = TestRun::new("server-schema-bindings");
    let database = open(&run.path("server.db"), Intent::Migrate).await.unwrap();
    commit(
        &database,
        &[
            "INSERT INTO actors(id) VALUES('alice')",
            "INSERT INTO actors(id) VALUES('bob')",
        ],
    )
    .await;
    let insert = "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES(?1,?2,?3,?4)";
    let tx = database.owner().begin_write().await.unwrap();
    // Case, Unicode normalization and issuer each make a different key.
    for subject in ["Sub", "sub", "Người", "Ngu\u{31b}\u{300}o\u{31b}i"] {
        tx.execute(
            insert,
            params_from_iter([
                text("issuer"),
                text(subject),
                text("alice"),
                Value::Integer(T0),
            ]),
        )
        .await
        .unwrap();
    }
    tx.execute(
        insert,
        params_from_iter([
            text("issuer-b"),
            text("sub"),
            text("bob"),
            Value::Integer(i64::MAX),
        ]),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let before = values(
        &database,
        "SELECT issuer,subject,actor_id,linked_at FROM actor_external_identities ORDER BY issuer,subject",
    )
    .await;
    assert_eq!(before.len(), 5);

    let row = |subject: &str, actor: &str, at: Value| {
        vec![text("issuer"), text(subject), text(actor), at]
    };
    for (values, fault) in [
        (
            row("new", "missing", Value::Integer(T0)),
            constraint(ConstraintKind::ForeignKey),
        ),
        (
            row("new", "alice", Value::Integer(-1)),
            constraint(ConstraintKind::Check),
        ),
        (
            row("new", "alice", Value::Null),
            StoreFault::Constraint {
                kind: ConstraintKind::NotNull,
                target: Some("actor_external_identities.linked_at".into()),
            },
        ),
    ] {
        assert_eq!(
            attempt(&database, insert, values.clone()).await,
            Err(fault),
            "{values:?}"
        );
    }
    // An insert over an existing key is refused before conflict resolution, so neither a plain
    // duplicate nor REPLACE (which deletes without firing the delete guard) can rebind it.
    assert_eq!(
        attempt(&database, insert, row("sub", "bob", Value::Integer(T0))).await,
        Err(guard("immutable.actor_external_identities"))
    );
    for sql in [
        "INSERT OR REPLACE INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('issuer','sub','bob',0)",
        "REPLACE INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('issuer','sub','bob',0)",
        "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('issuer','sub','bob',0) ON CONFLICT(issuer,subject) DO UPDATE SET actor_id=excluded.actor_id",
        "UPDATE actor_external_identities SET actor_id='bob' WHERE subject='sub'",
        "UPDATE actor_external_identities SET linked_at=0",
        "UPDATE actor_external_identities SET issuer='other' WHERE subject='Sub'",
        "DELETE FROM actor_external_identities WHERE subject='sub'",
        "DELETE FROM actor_external_identities",
    ] {
        assert_eq!(
            attempt(&database, sql, Vec::new()).await,
            Err(guard("immutable.actor_external_identities")),
            "{sql}"
        );
    }
    assert_eq!(
        values(
            &database,
            "SELECT issuer,subject,actor_id,linked_at FROM actor_external_identities ORDER BY issuer,subject",
        )
        .await,
        before
    );
}
