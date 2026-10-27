//! The candidate `server.db` schema on the real pinned engine and run-owned files: the slices it
//! creates, the upgrade from the actor registry slice, the files it refuses without adopting them,
//! and the database-level barriers behind the actor registry's domain constructors and the Identity
//! sessions. No constraint is weakened to build a fixture.
use std::{collections::BTreeSet, path::Path, sync::Arc};

use cantos_store_turso::{
    clock::Clock,
    error::{ConstraintKind, GuardCode, OpenError, StoreFault},
    params_from_iter,
    registry::{Intent, Migration, Refusal, Registry},
    testing::{ledger_registry, FixedClock, TestRun},
    OpenRequest, OwnedDatabase, Value,
};

use super::{ServerDatabase, ServerDatabaseError, MIGRATIONS, SERVER_APPLICATION_ID, SERVER_EPOCH};
use crate::platform::identity::domain::{ActorId, ExternalIdentityKey};

#[test]
fn the_service_registry_keeps_the_persisted_server_marker() {
    assert_eq!(super::registry().unwrap().application_id(), 0x434E_5453);
}

const T0: i64 = 1_700_000_000_000_000;

/// The shipped registry plus one slice this binary does not know: a file written by a newer binary.
fn future() -> &'static [Migration] {
    let next = Migration {
        version: u32::try_from(MIGRATIONS.len()).unwrap() + 1,
        name: "server_future_slice",
        sql: "CREATE TABLE future_slice(id INTEGER PRIMARY KEY) STRICT;",
    };
    Box::leak(
        MIGRATIONS
            .iter()
            .copied()
            .chain([next])
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

/// Every schema object the shipped migrations create, observed by replaying their DDL on a plain
/// engine database that has no owner and no registry. The expectation therefore follows the
/// registry instead of a hand-kept list, and a stray object still fails the comparison.
async fn replayed_objects(run: &TestRun) -> Vec<Vec<Value>> {
    let joined: &'static str = Box::leak(
        MIGRATIONS
            .iter()
            .map(|migration| migration.sql)
            .collect::<Vec<_>>()
            .join("\n")
            .into_boxed_str(),
    );
    let one: &'static [Migration] = Box::leak(Box::new([Migration {
        version: 1,
        name: "replayed_server_schema",
        sql: joined,
    }]));
    let path = run.path("replay.db");
    foreign(
        &path,
        Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, one).unwrap(),
    )
    .await;
    let owner = OwnedDatabase::open(OpenRequest::new(
        &path,
        Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, one).unwrap(),
        Intent::Serve,
        clock(),
    ))
    .await
    .unwrap();
    let reader = owner.reader().await.unwrap();
    let mut rows = reader
        .query(
            "SELECT type,name FROM sqlite_schema WHERE substr(name,1,7) <> 'sqlite_' \
             AND name NOT LIKE 'cantos_schema_history%' ORDER BY type,name",
            (),
        )
        .await
        .unwrap();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        out.push(vec![row.get_value(0).unwrap(), row.get_value(1).unwrap()]);
    }
    out
}

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
async fn a_new_file_holds_the_registry_metadata_and_exactly_the_migrated_schema() {
    let run = TestRun::new("server-schema-slice");
    let path = run.path("server.db");
    drop(open(&path, Intent::Migrate).await.unwrap());
    let database = open(&path, Intent::Serve).await.unwrap();
    assert_eq!(
        values(&database, "PRAGMA application_id").await,
        [[Value::Integer(i64::from(SERVER_APPLICATION_ID))]]
    );
    assert_eq!(
        values(&database, "PRAGMA user_version").await,
        [[Value::Integer(i64::from(SERVER_EPOCH))]]
    );
    let objects = values(
        &database,
        "SELECT type,name FROM sqlite_schema WHERE substr(name,1,7) <> 'sqlite_' \
         AND name NOT LIKE 'cantos_schema_history%' ORDER BY type,name",
    )
    .await;
    assert_eq!(objects, replayed_objects(&run).await);
    let history = values(
        &database,
        "SELECT version,name,checksum,applied_at FROM cantos_schema_history ORDER BY version",
    )
    .await;
    let expected: Vec<Vec<Value>> = MIGRATIONS
        .iter()
        .map(|migration| {
            vec![
                Value::Integer(i64::from(migration.version)),
                text(migration.name),
                Value::Blob(migration.checksum().to_vec()),
                Value::Integer(T0),
            ]
        })
        .collect();
    assert_eq!(history, expected);
    assert_eq!(
        MIGRATIONS
            .iter()
            .enumerate()
            .filter(|(index, migration)| u32::try_from(*index).unwrap() + 1 != migration.version)
            .count(),
        0,
        "versions are dense and ascending"
    );
}

#[tokio::test]
async fn an_actor_registry_file_upgrades_through_every_later_slice_and_keeps_its_rows() {
    let run = TestRun::new("server-schema-upgrade");
    let path = run.path("server.db");
    let v1 = Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..1]).unwrap();
    let old = OwnedDatabase::open(OpenRequest::new(&path, v1, Intent::Migrate, clock()))
        .await
        .unwrap();
    let tx = old.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO actors(id,active) VALUES('alice',1),('bob',0)",
        (),
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) \
         VALUES('cantos-identity','0123456789abcdef0123456789abcdef','alice',7)",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(old);
    let before = bytes(&path);
    // Serving never upgrades: the older file is refused unchanged until an explicit migration.
    assert_eq!(
        refused(&path, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::PendingMigrations {
            applied: 1,
            latest: u32::try_from(MIGRATIONS.len()).unwrap(),
        }))
    );
    assert_eq!(bytes(&path), before);
    let later = FixedClock::new(T0 + 5);
    drop(
        ServerDatabase::open(&path, Intent::Migrate, later.clone())
            .await
            .unwrap(),
    );
    let database = ServerDatabase::open(&path, Intent::Serve, later)
        .await
        .unwrap();
    assert_eq!(
        values(&database, "SELECT id,active FROM actors ORDER BY id").await,
        [
            [text("alice"), Value::Integer(1)],
            [text("bob"), Value::Integer(0)]
        ]
    );
    assert_eq!(
        values(
            &database,
            "SELECT issuer,subject,actor_id,linked_at FROM actor_external_identities"
        )
        .await,
        [[
            text("cantos-identity"),
            text("0123456789abcdef0123456789abcdef"),
            text("alice"),
            Value::Integer(7),
        ]]
    );
    let applied = values(
        &database,
        "SELECT version,applied_at FROM cantos_schema_history ORDER BY version",
    )
    .await;
    let expected: Vec<Vec<Value>> = (1..=MIGRATIONS.len())
        .map(|version| {
            vec![
                Value::Integer(i64::try_from(version).unwrap()),
                Value::Integer(if version == 1 { T0 } else { T0 + 5 }),
            ]
        })
        .collect();
    assert_eq!(applied, expected);
    assert_eq!(
        values(&database, "SELECT count(*) FROM identity_sessions").await,
        [[Value::Integer(0)]]
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
            Registry::new(0x434E_5449, SERVER_EPOCH, &MIGRATIONS).unwrap(),
            Refusal::ForeignApplication { found: 0x434E_5449 },
        ),
        (
            "fixture.db",
            ledger_registry(SERVER_APPLICATION_ID),
            Refusal::HistoryDiverged { version: 1 },
        ),
        (
            "epoch.db",
            Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH + 1, &MIGRATIONS).unwrap(),
            Refusal::EpochMismatch {
                found: 2,
                expected: SERVER_EPOCH,
            },
        ),
        (
            "newer.db",
            Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, future()).unwrap(),
            Refusal::NewerThanBinary {
                applied: u32::try_from(MIGRATIONS.len()).unwrap() + 1,
                known: u32::try_from(MIGRATIONS.len()).unwrap(),
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

const SUBJECT: &str = "0123456789abcdef0123456789abcdef";
const S1: &str = "0a000000-0000-4000-8000-00000000000b";
const S2: &str = "00000000-0000-4000-8000-000000000002";
const S3: &str = "00000000-0000-4000-8000-000000000003";
const SESSION_INSERT: &str = "INSERT INTO identity_sessions(id,actor_id,channel,issuer,subject,\
assurance,authenticated_at,observed_security_version,observed_instance_epoch,absolute_expires,\
idle_expires,revoked,revoked_reason) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)";
const TOKEN_INSERT: &str = "INSERT INTO identity_session_tokens(access_hash,refresh_hash,\
session_id,expires_at,used) VALUES(?1,?2,?3,?4,?5)";

fn epoch() -> Value {
    Value::Blob(vec![0xe0; 16])
}

/// A valid session row for `SESSION_INSERT`, bound to `alice` through `SUBJECT`.
fn session(id: &str, channel: &str) -> Vec<Value> {
    vec![
        text(id),
        text("alice"),
        text(channel),
        text("cantos-identity"),
        text(SUBJECT),
        text("password"),
        Value::Integer(10),
        Value::Integer(1),
        epoch(),
        Value::Integer(900),
        Value::Integer(600),
        Value::Integer(0),
        Value::Null,
    ]
}

fn with(mut row: Vec<Value>, index: usize, value: Value) -> Vec<Value> {
    row[index] = value;
    row
}

/// Actors and exact bindings every session fixture needs, including bindings whose subjects the
/// session CHECK refuses, so each CHECK is reached past the bound-actor guard.
async fn session_fixture(database: &ServerDatabase) {
    let mut statements = vec![
        "INSERT INTO actors(id) VALUES('alice')".to_owned(),
        "INSERT INTO actors(id) VALUES('bob')".to_owned(),
        format!(
            "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('other','{SUBJECT}','alice',0)"
        ),
    ];
    for subject in [
        SUBJECT.to_owned(),
        SUBJECT.to_uppercase(),
        "0".repeat(32),
        SUBJECT[..31].to_owned(),
    ] {
        statements.push(format!(
            "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('cantos-identity','{subject}','alice',0)"
        ));
    }
    let statements: Vec<&str> = statements.iter().map(String::as_str).collect();
    commit(database, &statements).await;
}

async fn sessions_and_tokens(database: &ServerDatabase) -> (Vec<Vec<Value>>, Vec<Vec<Value>>) {
    (
        values(database, "SELECT * FROM identity_sessions ORDER BY id").await,
        values(
            database,
            "SELECT * FROM identity_session_tokens ORDER BY access_hash",
        )
        .await,
    )
}

#[tokio::test]
async fn identity_session_rows_admit_only_bound_well_formed_values() {
    let run = TestRun::new("server-schema-session-values");
    let database = open(&run.path("server.db"), Intent::Migrate).await.unwrap();
    session_fixture(&database).await;
    let check = constraint(ConstraintKind::Check);
    let cases = [
        (
            with(session(S1, "theatre-web"), 0, text(&S1.to_uppercase())),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 0, text("not-a-uuid")),
            check.clone(),
        ),
        (session(S1, "studio-native"), check.clone()),
        (
            with(session(S1, "theatre-web"), 3, text("other")),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 4, text(&SUBJECT.to_uppercase())),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 4, text(&"0".repeat(32))),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 4, text(&SUBJECT[..31])),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 5, text("passkey")),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 6, Value::Integer(0)),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 7, Value::Integer(0)),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 8, Value::Blob(vec![0xe0; 15])),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 8, Value::Blob(vec![0; 16])),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 9, Value::Integer(-1)),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 10, Value::Integer(901)),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 11, Value::Integer(1)),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 12, text("logout")),
            check.clone(),
        ),
        (
            with(
                with(session(S1, "theatre-web"), 11, Value::Integer(1)),
                12,
                text("expired"),
            ),
            check.clone(),
        ),
        (
            with(session(S1, "theatre-web"), 11, Value::Integer(2)),
            check.clone(),
        ),
        // The session's actor must be the one its exact issuer/subject is bound to.
        (
            with(session(S1, "theatre-web"), 1, text("bob")),
            guard("identity_sessions.binding"),
        ),
        (
            with(session(S1, "theatre-web"), 4, text(&"f".repeat(32))),
            guard("identity_sessions.binding"),
        ),
        (
            with(session(S1, "theatre-web"), 1, text("missing")),
            guard("identity_sessions.binding"),
        ),
    ];
    for (row, fault) in cases {
        assert_eq!(
            attempt(&database, SESSION_INSERT, row.clone()).await,
            Err(fault),
            "{row:?}"
        );
    }
    // STRICT refuses another storage class for the epoch digest and a lossy deadline.
    for row in [
        with(session(S1, "theatre-web"), 8, text(&"e0".repeat(16))),
        with(session(S1, "theatre-web"), 9, Value::Real(900.5)),
        with(session(S1, "theatre-web"), 9, text("nine hundred")),
    ] {
        assert!(
            attempt(&database, SESSION_INSERT, row.clone())
                .await
                .is_err(),
            "{row:?}"
        );
    }
    let tx = database.owner().begin_write().await.unwrap();
    for (id, channel) in [
        (S1, "theatre-web"),
        (S2, "theatre-native"),
        (S3, "studio-web"),
    ] {
        tx.execute(SESSION_INSERT, params_from_iter(session(id, channel)))
            .await
            .unwrap();
    }
    tx.execute(
        "UPDATE identity_sessions SET revoked=1,revoked_reason='logout' WHERE id=?1",
        [S3],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let hash = |byte: u8| Value::Blob(vec![byte; 32]);
    let token = |access: u8, refresh: Option<u8>, session: &str| {
        vec![
            hash(access),
            refresh.map_or(Value::Null, hash),
            text(session),
            Value::Integer(600),
            Value::Integer(0),
        ]
    };
    let issue = guard("identity_session_tokens.issue");
    for (row, fault) in [
        (
            with(token(1, None, S1), 0, Value::Blob(vec![1; 31])),
            check.clone(),
        ),
        (
            with(token(2, Some(3), S2), 1, Value::Blob(vec![3; 33])),
            check.clone(),
        ),
        (
            with(token(1, None, S1), 3, Value::Integer(-1)),
            check.clone(),
        ),
        // A new secret starts unspent, matches its session's channel shape and needs a live one.
        (
            with(token(1, None, S1), 4, Value::Integer(1)),
            issue.clone(),
        ),
        (token(1, Some(3), S1), issue.clone()),
        (token(2, None, S2), issue.clone()),
        (token(4, None, S3), issue.clone()),
        (
            token(5, None, "00000000-0000-4000-8000-000000000009"),
            issue.clone(),
        ),
    ] {
        assert_eq!(
            attempt(&database, TOKEN_INSERT, row.clone()).await,
            Err(fault),
            "{row:?}"
        );
    }
    assert!(attempt(
        &database,
        TOKEN_INSERT,
        with(token(1, None, S1), 0, text("digest"))
    )
    .await
    .is_err());
    let tx = database.owner().begin_write().await.unwrap();
    for row in [token(1, None, S1), token(2, Some(3), S2)] {
        tx.execute(TOKEN_INSERT, params_from_iter(row))
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    // A Web cookie cannot be marked spent: only a refresh secret is ever consumed.
    assert_eq!(
        attempt(
            &database,
            "UPDATE identity_session_tokens SET used=1 WHERE session_id=?1",
            vec![text(S1)]
        )
        .await,
        Err(check)
    );
    let (sessions, tokens) = sessions_and_tokens(&database).await;
    assert_eq!((sessions.len(), tokens.len()), (3, 2));
}

#[tokio::test]
async fn identity_session_rows_never_move_resurrect_shrink_or_disappear() {
    let run = TestRun::new("server-schema-session-guards");
    let database = open(&run.path("server.db"), Intent::Migrate).await.unwrap();
    session_fixture(&database).await;
    let tx = database.owner().begin_write().await.unwrap();
    for (id, channel) in [
        (S1, "theatre-web"),
        (S2, "theatre-native"),
        (S3, "studio-web"),
    ] {
        tx.execute(SESSION_INSERT, params_from_iter(session(id, channel)))
            .await
            .unwrap();
    }
    tx.execute(
        TOKEN_INSERT,
        params_from_iter([
            Value::Blob(vec![1; 32]),
            Value::Null,
            text(S1),
            Value::Integer(600),
            Value::Integer(0),
        ]),
    )
    .await
    .unwrap();
    tx.execute(
        TOKEN_INSERT,
        params_from_iter([
            Value::Blob(vec![2; 32]),
            Value::Blob(vec![3; 32]),
            text(S2),
            Value::Integer(120),
            Value::Integer(0),
        ]),
    )
    .await
    .unwrap();
    tx.execute(
        "UPDATE identity_sessions SET revoked=1,revoked_reason='replay' WHERE id=?1",
        [S3],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let before = sessions_and_tokens(&database).await;

    let sessions = guard("immutable.identity_sessions");
    let revocation = guard("immutable.identity_sessions.revocation");
    let tokens = guard("immutable.identity_session_tokens");
    let duplicate = |id: &str| {
        let row = session(id, "theatre-web");
        format!(
            "('{id}','alice','theatre-web','cantos-identity','{SUBJECT}','password',10,1,x'{}',900,600,0,NULL)",
            "e0".repeat(16)
        )
        .replace("{row:?}", &format!("{row:?}"))
    };
    let columns = "id,actor_id,channel,issuer,subject,assurance,authenticated_at,\
observed_security_version,observed_instance_epoch,absolute_expires,idle_expires,revoked,revoked_reason";
    let replays = [
        format!("INSERT INTO identity_sessions({columns}) VALUES {}", duplicate(S1)),
        format!("INSERT OR REPLACE INTO identity_sessions({columns}) VALUES {}", duplicate(S3)),
        format!("REPLACE INTO identity_sessions({columns}) VALUES {}", duplicate(S3)),
        format!(
            "INSERT INTO identity_sessions({columns}) VALUES {} ON CONFLICT(id) DO UPDATE SET revoked=0,revoked_reason=NULL",
            duplicate(S3)
        ),
        format!("INSERT INTO identity_sessions({columns}) VALUES {} ON CONFLICT(id) DO NOTHING", duplicate(S1)),
    ];
    for sql in &replays {
        assert_eq!(
            attempt(&database, sql, Vec::new()).await,
            Err(sessions.clone()),
            "{sql}"
        );
    }
    for (sql, fault) in [
        (format!("UPDATE identity_sessions SET actor_id='bob' WHERE id='{S1}'"), sessions.clone()),
        (format!("UPDATE identity_sessions SET channel='studio-web' WHERE id='{S1}'"), sessions.clone()),
        (format!("UPDATE identity_sessions SET authenticated_at=11 WHERE id='{S1}'"), sessions.clone()),
        (format!("UPDATE identity_sessions SET observed_security_version=2 WHERE id='{S1}'"), sessions.clone()),
        (
            format!("UPDATE identity_sessions SET observed_instance_epoch=x'{}' WHERE id='{S1}'", "e1".repeat(16)),
            sessions.clone(),
        ),
        (format!("UPDATE identity_sessions SET absolute_expires=901 WHERE id='{S1}'"), sessions.clone()),
        (format!("UPDATE identity_sessions SET idle_expires=599 WHERE id='{S1}'"), sessions.clone()),
        (format!("UPDATE identity_sessions SET id='{}' WHERE id='{S1}'", "00000000-0000-4000-8000-000000000007"), sessions.clone()),
        (format!("UPDATE identity_sessions SET revoked=0,revoked_reason=NULL WHERE id='{S3}'"), revocation.clone()),
        (format!("UPDATE identity_sessions SET revoked_reason='logout' WHERE id='{S3}'"), revocation.clone()),
        (format!("UPDATE identity_sessions SET idle_expires=700 WHERE id='{S3}'"), revocation.clone()),
        (format!("DELETE FROM identity_sessions WHERE id='{S3}'"), sessions.clone()),
        ("DELETE FROM identity_sessions".to_owned(), sessions.clone()),
        (
            format!("INSERT INTO identity_session_tokens(access_hash,refresh_hash,session_id,expires_at) VALUES(x'{}',NULL,'{S1}',600)", "01".repeat(32)),
            tokens.clone(),
        ),
        (
            format!("INSERT INTO identity_session_tokens(access_hash,refresh_hash,session_id,expires_at) VALUES(x'{}',x'{}','{S2}',120)", "09".repeat(32), "03".repeat(32)),
            tokens.clone(),
        ),
        (
            format!("INSERT OR REPLACE INTO identity_session_tokens(access_hash,refresh_hash,session_id,expires_at) VALUES(x'{}',x'{}','{S2}',120)", "09".repeat(32), "03".repeat(32)),
            tokens.clone(),
        ),
        (
            format!("REPLACE INTO identity_session_tokens(access_hash,refresh_hash,session_id,expires_at) VALUES(x'{}',NULL,'{S1}',900)", "01".repeat(32)),
            tokens.clone(),
        ),
        (format!("UPDATE identity_session_tokens SET session_id='{S3}' WHERE session_id='{S1}'"), tokens.clone()),
        (format!("UPDATE identity_session_tokens SET access_hash=x'{}' WHERE session_id='{S1}'", "08".repeat(32)), tokens.clone()),
        (format!("UPDATE identity_session_tokens SET refresh_hash=x'{}' WHERE session_id='{S2}'", "08".repeat(32)), tokens.clone()),
        (format!("UPDATE identity_session_tokens SET expires_at=599 WHERE session_id='{S1}'"), tokens.clone()),
        ("DELETE FROM identity_session_tokens".to_owned(), tokens.clone()),
    ] {
        assert_eq!(attempt(&database, &sql, Vec::new()).await, Err(fault), "{sql}");
    }
    assert_eq!(sessions_and_tokens(&database).await, before);

    // The allowed moves: extend an idle deadline or a cookie expiry, spend a refresh secret,
    // revoke once. Spending and revocation are then permanent.
    commit(
        &database,
        &[
            &format!("UPDATE identity_sessions SET idle_expires=700 WHERE id='{S1}'"),
            &format!("UPDATE identity_session_tokens SET expires_at=700 WHERE session_id='{S1}'"),
            &format!("UPDATE identity_session_tokens SET used=1 WHERE session_id='{S2}'"),
            &format!(
                "UPDATE identity_sessions SET revoked=1,revoked_reason='disabled' WHERE id='{S1}'"
            ),
        ],
    )
    .await;
    for (sql, fault) in [
        (
            format!("UPDATE identity_session_tokens SET used=0 WHERE session_id='{S2}'"),
            tokens,
        ),
        (
            format!("UPDATE identity_sessions SET revoked=0,revoked_reason=NULL WHERE id='{S1}'"),
            revocation,
        ),
    ] {
        assert_eq!(
            attempt(&database, &sql, Vec::new()).await,
            Err(fault),
            "{sql}"
        );
    }
    assert_eq!(
        values(
            &database,
            &format!(
                "SELECT revoked,revoked_reason,idle_expires FROM identity_sessions WHERE id='{S1}'"
            )
        )
        .await,
        [[Value::Integer(1), text("disabled"), Value::Integer(700)]]
    );
}

#[tokio::test]
async fn development_session_rows_are_well_formed_bound_and_never_resurrect_or_disappear() {
    let run = TestRun::new("server-schema-development-sessions");
    let database = open(&run.path("server.db"), Intent::Migrate).await.unwrap();
    commit(
        &database,
        &["INSERT INTO actors(id) VALUES('alice'),('bob')"],
    )
    .await;
    let row = |hash: u8, actor: &str, id: &str| {
        vec![
            Value::Blob(vec![hash; 32]),
            text(actor),
            Value::Integer(10),
            text(id),
        ]
    };
    let insert = "INSERT INTO development_sessions(token_hash,actor_id,expires_at,session_id) VALUES(?1,?2,?3,?4)";
    let good = "00000000-0000-4000-8000-000000000001";
    let tx = database.owner().begin_write().await.unwrap();
    tx.execute(insert, params_from_iter(row(1, "alice", good)))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Shape: digest length, actor reference, canonical id, non-negative expiry.
    for (bad, expected) in [
        (
            vec![
                Value::Blob(vec![2; 31]),
                text("alice"),
                Value::Integer(10),
                text("00000000-0000-4000-8000-000000000002"),
            ],
            constraint(ConstraintKind::Check),
        ),
        (
            row(3, "ghost", "00000000-0000-4000-8000-000000000003"),
            constraint(ConstraintKind::ForeignKey),
        ),
        (
            row(4, "alice", "00000000-0000-4000-8000-00000000000G"),
            constraint(ConstraintKind::Check),
        ),
        (
            row(5, "alice", "00000000-0000-4000-8000-0000000000AB"),
            constraint(ConstraintKind::Check),
        ),
        (
            vec![
                Value::Blob(vec![6; 32]),
                text("alice"),
                Value::Integer(-1),
                text("00000000-0000-4000-8000-000000000006"),
            ],
            constraint(ConstraintKind::Check),
        ),
    ] {
        assert_eq!(attempt(&database, insert, bad).await, Err(expected));
    }
    // A token or session id is never rebound: plain, REPLACE and upsert inserts all refuse.
    for sql in [
        insert.to_owned(),
        insert.replace("INSERT INTO", "INSERT OR REPLACE INTO"),
        insert.replace("INSERT INTO", "REPLACE INTO"),
        format!("{insert} ON CONFLICT(token_hash) DO UPDATE SET actor_id='bob'"),
    ] {
        assert_eq!(
            attempt(
                &database,
                &sql,
                row(1, "bob", "00000000-0000-4000-8000-0000000000aa")
            )
            .await,
            Err(guard("immutable.development_sessions")),
            "{sql}"
        );
    }
    // Only revocation moves, and only forward; nothing is deleted.
    let hash = Value::Blob(vec![1; 32]);
    for sql in [
        "UPDATE development_sessions SET actor_id='bob' WHERE token_hash=?1",
        "UPDATE development_sessions SET expires_at=99 WHERE token_hash=?1",
        "UPDATE development_sessions SET session_id='00000000-0000-4000-8000-0000000000bb' WHERE token_hash=?1",
        "DELETE FROM development_sessions WHERE token_hash=?1",
    ] {
        assert_eq!(
            attempt(&database, sql, vec![hash.clone()]).await,
            Err(guard("immutable.development_sessions")),
            "{sql}"
        );
    }
    let revoke = "UPDATE development_sessions SET revoked=1 WHERE token_hash=?1";
    assert_eq!(attempt(&database, revoke, vec![hash.clone()]).await, Ok(1));
    commit(&database, &["UPDATE development_sessions SET revoked=1"]).await;
    assert_eq!(
        attempt(
            &database,
            "UPDATE development_sessions SET revoked=0 WHERE token_hash=?1",
            vec![hash]
        )
        .await,
        Err(guard("immutable.development_sessions"))
    );
}

#[path = "business_tests.rs"]
mod business;

#[path = "../../test_support/historical_migration.rs"]
mod historical;

#[path = "sample_metadata_tests.rs"]
mod sample_metadata;
