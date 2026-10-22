//! The Turso Identity sessions on the real pinned engine and run-owned files: the port contract,
//! deadlines, atomic binding with issue, rollback after each important write, controlled
//! interleavings on one owner, lost COMMIT acknowledgements, reopen and corrupt projections.
//! Expected deadlines are literal seconds of the production policies; no second session model
//! computes them, and every outcome is checked against the durable rows as well.
use std::{sync::Arc, time::Duration};

use cantos_store_turso::{
    params_from_iter,
    registry::Intent,
    testing::{FixedClock, TestRun},
    Row, Value,
};

use super::{
    admit_in, record, refresh_record, session_key, stored_session_id, Acknowledgement,
    TursoSessions,
};
use crate::platform::{
    bearer::core::Lifetime,
    identity::{
        domain::{ActorId, ActorRecord, ExternalIdentityKey},
        IdentityRegistry, TursoIdentityRegistry,
    },
    identity_auth::{
        credential::{Channel, Secrets, Surface},
        decision::{Epoch, ObservedSecurity, RevokeReason, Subject},
        session::{digest, SessionError, SessionStore, ISSUER},
    },
    turso::ServerDatabase,
};

/// 1_700_000_000 s. Theatre policy: access 120 s, idle 600 s, absolute 900 s; Studio 120/300/600.
const T0: i64 = 1_700_000_000_000_000;
const S0: u64 = 1_700_000_000;
const SECOND: i64 = 1_000_000;

const THEATRE: Channel = Channel::Web(Surface::Theatre);
const STUDIO: Channel = Channel::Web(Surface::Studio);
const NATIVE: Channel = Channel::Native;

fn subject(byte: u8) -> Subject {
    Subject::from_bytes([byte; 16]).unwrap()
}

fn observed(byte: u8, version: u64) -> ObservedSecurity {
    ObservedSecurity {
        subject: subject(byte),
        epoch: Epoch::from_bytes([0xe0; 16]).unwrap(),
        version,
    }
}

fn secrets(channel: Channel) -> Secrets {
    Secrets::new(channel).unwrap()
}

fn at(seconds: u64) -> i64 {
    T0 + i64::try_from(seconds).unwrap() * SECOND
}

async fn open(run: &TestRun, intent: Intent, clock: Arc<FixedClock>) -> ServerDatabase {
    ServerDatabase::open(run.path("server.db"), intent, clock)
        .await
        .unwrap()
}

async fn execute(database: &ServerDatabase, sql: &str) {
    let tx = database.owner().begin_write().await.unwrap();
    tx.execute(sql, ()).await.unwrap();
    tx.commit().await.unwrap();
}

fn cell(row: &Row, index: usize) -> Value {
    row.get_value(index).unwrap()
}

/// Every stored session and token, read back independently of the adapter's decoders.
#[derive(Clone, Debug, PartialEq)]
struct Stored {
    actors: Vec<(String, i64)>,
    bindings: Vec<(String, String, String)>,
    /// `(id, actor, channel, subject, authenticated_at, version, absolute, idle, revoked, reason)`
    sessions: Vec<Vec<Value>>,
    /// `(access_hash, refresh_hash, session_id, expires_at, used)`
    tokens: Vec<Vec<Value>>,
}

async fn rows(database: &ServerDatabase, sql: &str) -> Vec<Vec<Value>> {
    let reader = database.owner().reader().await.unwrap();
    let mut rows = reader.query(sql, ()).await.unwrap();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        out.push((0..rows.column_count()).map(|i| cell(&row, i)).collect());
    }
    out
}

async fn stored(database: &ServerDatabase) -> Stored {
    let text = |value: &Value| match value {
        Value::Text(text) => text.clone(),
        other => panic!("{other:?}"),
    };
    let integer = |value: &Value| match value {
        Value::Integer(value) => *value,
        other => panic!("{other:?}"),
    };
    Stored {
        actors: rows(database, "SELECT id,active FROM actors ORDER BY id")
            .await
            .iter()
            .map(|row| (text(&row[0]), integer(&row[1])))
            .collect(),
        bindings: rows(
            database,
            "SELECT issuer,subject,actor_id FROM actor_external_identities ORDER BY issuer,subject",
        )
        .await
        .iter()
        .map(|row| (text(&row[0]), text(&row[1]), text(&row[2])))
        .collect(),
        sessions: rows(
            database,
            "SELECT id,actor_id,channel,subject,authenticated_at,observed_security_version,\
             absolute_expires,idle_expires,revoked,revoked_reason FROM identity_sessions ORDER BY id",
        )
        .await,
        tokens: rows(
            database,
            "SELECT access_hash,refresh_hash,session_id,expires_at,used FROM identity_session_tokens \
             ORDER BY session_id,expires_at,access_hash",
        )
        .await,
    }
}

async fn session_row(database: &ServerDatabase, id: &str) -> Vec<Value> {
    stored(database)
        .await
        .sessions
        .into_iter()
        .find(|row| row[0] == Value::Text(id.to_owned()))
        .expect("the session row exists")
}

fn revoked_with(row: &[Value]) -> (i64, Option<String>) {
    let revoked = match row[8] {
        Value::Integer(value) => value,
        ref other => panic!("{other:?}"),
    };
    let reason = match &row[9] {
        Value::Null => None,
        Value::Text(reason) => Some(reason.clone()),
        other => panic!("{other:?}"),
    };
    (revoked, reason)
}

fn lifetime(access: u64, idle: u64, absolute: u64) -> Lifetime {
    Lifetime {
        access: S0 + access,
        idle: S0 + idle,
        absolute: S0 + absolute,
    }
}

#[tokio::test]
async fn issue_access_renewal_rotation_and_logout_follow_the_port_contract_on_a_real_file() {
    let run = TestRun::new("sessions-turso-contract");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock.clone()).await;
    let sessions = TursoSessions::new(database.clone());

    // Web: one cookie secret bound to the exact subject's actor.
    let web = secrets(THEATRE);
    let issued = sessions
        .issue(THEATRE, &observed(1, 3), 1_699_999_990, &web)
        .await
        .unwrap();
    assert_eq!(issued.lifetime, lifetime(120, 600, 900));
    assert_eq!(
        session_key(&issued.session_id),
        Some(issued.session_id.clone())
    );
    let record = sessions.access(&web.access, THEATRE).await.unwrap();
    assert_eq!(record.session_id, issued.session_id);
    assert_eq!(record.actor_id, issued.actor_id);
    assert_eq!(record.channel, THEATRE);
    assert_eq!(record.observed, observed(1, 3));
    assert_eq!(record.authenticated_at, 1_699_999_990);
    assert_eq!(
        (
            record.absolute_expires,
            record.idle_expires,
            record.token_expires
        ),
        (S0 + 900, S0 + 600, S0 + 600)
    );
    // The same secret never opens another channel, and an unknown secret opens nothing.
    for channel in [STUDIO, NATIVE] {
        assert_eq!(
            sessions.access(&web.access, channel).await.unwrap_err(),
            SessionError::Inactive
        );
    }
    assert_eq!(
        sessions
            .access(&secrets(THEATRE).access, THEATRE)
            .await
            .unwrap_err(),
        SessionError::Inactive
    );
    // The binding is exact and idempotent: a second login of the subject reuses the actor.
    let again = secrets(STUDIO);
    let studio = sessions
        .issue(STUDIO, &observed(1, 3), 1_699_999_995, &again)
        .await
        .unwrap();
    assert_eq!(studio.actor_id, issued.actor_id);
    assert_eq!(studio.lifetime, lifetime(120, 300, 600));
    assert_eq!(sessions.bind(subject(1)).await.unwrap(), issued.actor_id);
    let registry = TursoIdentityRegistry::new(database.clone());
    assert_eq!(
        registry
            .lookup(&ExternalIdentityKey::new(ISSUER, subject(1).hex()).unwrap())
            .await,
        Ok(Some(ActorRecord {
            id: ActorId::new(issued.actor_id.clone()).unwrap(),
            active: true,
        }))
    );

    // Web renewal extends the same secret's idle deadline, never past the absolute one.
    clock.set(at(500));
    let (renewed, renewal) = sessions.renew_web(&web.access, THEATRE).await.unwrap();
    assert_eq!(renewal, lifetime(620, 900, 900));
    assert_eq!(
        (
            renewed.idle_expires,
            renewed.token_expires,
            renewed.absolute_expires
        ),
        (S0 + 900, S0 + 900, S0 + 900)
    );
    assert_eq!(renewed.authenticated_at, 1_699_999_990);
    assert_eq!(renewed.observed, observed(1, 3));
    clock.set(at(899));
    assert!(sessions.access(&web.access, THEATRE).await.is_ok());
    clock.set(at(900));
    assert_eq!(
        sessions.access(&web.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        sessions.renew_web(&web.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive
    );

    // Native: an access/refresh pair; rotation spends the refresh secret exactly once.
    clock.set(T0);
    let first = secrets(NATIVE);
    let native = sessions
        .issue(NATIVE, &observed(1, 3), S0, &first)
        .await
        .unwrap();
    let refresh = first.refresh.clone().unwrap();
    assert_eq!(
        sessions
            .access(&first.access, NATIVE)
            .await
            .unwrap()
            .token_expires,
        S0 + 120
    );
    clock.set(at(100));
    let peek = sessions
        .peek_refresh(&refresh, &native.session_id)
        .await
        .unwrap();
    assert_eq!(peek.session_id, native.session_id);
    let next = secrets(NATIVE);
    let rotated = sessions
        .rotate(&refresh, &native.session_id, &next)
        .await
        .unwrap();
    assert_eq!(rotated.actor_id, issued.actor_id);
    assert_eq!(rotated.authenticated_at, S0);
    assert_eq!(rotated.lifetime, lifetime(220, 700, 900));
    assert_eq!(
        sessions.access(&first.access, NATIVE).await.unwrap_err(),
        SessionError::Inactive,
        "the spent pair's access secret dies with its refresh secret"
    );
    let current = sessions.access(&next.access, NATIVE).await.unwrap();
    assert_eq!(
        (
            current.absolute_expires,
            current.idle_expires,
            current.token_expires
        ),
        (S0 + 900, S0 + 700, S0 + 220)
    );
    // Replaying the spent refresh secret revokes the whole family, the new pair included.
    assert_eq!(
        sessions
            .peek_refresh(&refresh, &native.session_id)
            .await
            .unwrap_err(),
        SessionError::Replay
    );
    assert_eq!(
        sessions.access(&next.access, NATIVE).await.unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        revoked_with(&session_row(&database, &native.session_id).await),
        (1, Some("replay".into()))
    );
    assert_eq!(
        sessions
            .rotate(
                next.refresh.as_deref().unwrap(),
                &native.session_id,
                &secrets(NATIVE)
            )
            .await
            .unwrap_err(),
        SessionError::Inactive
    );

    // Logout ends the family by its own secret; the fence id stays readable, nothing else does.
    clock.set(at(10));
    assert!(sessions.access(&again.access, STUDIO).await.is_ok());
    sessions.logout(&again.access, THEATRE).await.unwrap();
    assert!(
        sessions.access(&again.access, STUDIO).await.is_ok(),
        "a wrong-channel logout selects nothing"
    );
    sessions.logout(&again.access, STUDIO).await.unwrap();
    sessions.logout(&again.access, STUDIO).await.unwrap();
    assert_eq!(
        sessions.access(&again.access, STUDIO).await.unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        sessions.session_id_of(&again.access, STUDIO).await,
        Ok(Some(studio.session_id.clone()))
    );
    assert_eq!(
        sessions.session_id_of(&again.access, THEATRE).await,
        Ok(None)
    );
    assert_eq!(
        revoked_with(&session_row(&database, &studio.session_id).await),
        (1, Some("logout".into()))
    );
}

#[tokio::test]
async fn every_deadline_is_exclusive_and_no_renewal_moves_the_absolute_one() {
    let run = TestRun::new("sessions-turso-deadlines");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock.clone()).await;
    let sessions = TursoSessions::new(database.clone());
    let web = secrets(THEATRE);
    let issued = sessions
        .issue(THEATRE, &observed(2, 1), S0, &web)
        .await
        .unwrap();
    clock.set(at(599));
    assert!(sessions.access(&web.access, THEATRE).await.is_ok());
    clock.set(at(600));
    assert_eq!(
        sessions.access(&web.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        sessions.renew_web(&web.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive,
        "an idle-expired cookie cannot renew itself"
    );

    // A clock read that went backwards cannot shorten a stored idle deadline: the guard refuses
    // it and the whole renewal rolls back.
    let studio = secrets(STUDIO);
    clock.set(at(200));
    let renewable = sessions
        .issue(STUDIO, &observed(2, 1), S0, &studio)
        .await
        .unwrap();
    assert_eq!(renewable.lifetime, lifetime(320, 500, 800));
    let before = stored(&database).await;
    clock.set(at(100));
    assert_eq!(
        sessions
            .renew_web(&studio.access, STUDIO)
            .await
            .unwrap_err(),
        SessionError::Unavailable
    );
    assert_eq!(stored(&database).await, before);

    let native = secrets(NATIVE);
    clock.set(T0);
    let family = sessions
        .issue(NATIVE, &observed(2, 1), S0, &native)
        .await
        .unwrap();
    let refresh = native.refresh.clone().unwrap();
    clock.set(at(600));
    assert_eq!(
        sessions
            .rotate(&refresh, &family.session_id, &secrets(NATIVE))
            .await
            .unwrap_err(),
        SessionError::Expired,
        "the refresh secret carries the idle deadline"
    );
    clock.set(at(599));
    let next = secrets(NATIVE);
    let rotated = sessions
        .rotate(&refresh, &family.session_id, &next)
        .await
        .unwrap();
    assert_eq!(rotated.lifetime, lifetime(719, 900, 900));
    let row = session_row(&database, &family.session_id).await;
    assert_eq!(row[6], Value::Integer((S0 + 900) as i64));
    assert_eq!(row[7], Value::Integer((S0 + 900) as i64));
    clock.set(at(900));
    assert_eq!(
        sessions
            .rotate(
                next.refresh.as_deref().unwrap(),
                &family.session_id,
                &secrets(NATIVE)
            )
            .await
            .unwrap_err(),
        SessionError::Expired
    );
    assert_eq!(issued.lifetime.absolute, S0 + 900);
}

#[tokio::test]
async fn binding_and_issue_are_one_unit_and_any_failed_write_rolls_both_back() {
    let run = TestRun::new("sessions-turso-atomic");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock).await;
    let sessions = TursoSessions::new(database.clone());
    let empty = stored(&database).await;

    // Fault injection through the real engine after the actor, binding and session inserts.
    execute(
        &database,
        "CREATE TRIGGER test_fail_token AFTER INSERT ON identity_session_tokens \
         BEGIN SELECT RAISE(ABORT, 'cantos:test.token'); END",
    )
    .await;
    assert_eq!(
        sessions
            .issue(THEATRE, &observed(3, 1), S0, &secrets(THEATRE))
            .await
            .unwrap_err(),
        SessionError::Unavailable
    );
    assert_eq!(
        stored(&database).await,
        empty,
        "no actor, binding or session"
    );
    execute(&database, "DROP TRIGGER test_fail_token").await;

    execute(
        &database,
        "CREATE TRIGGER test_fail_session AFTER INSERT ON identity_sessions \
         BEGIN SELECT RAISE(ABORT, 'cantos:test.session'); END",
    )
    .await;
    assert_eq!(
        sessions
            .issue(NATIVE, &observed(3, 1), S0, &secrets(NATIVE))
            .await
            .unwrap_err(),
        SessionError::Unavailable
    );
    assert_eq!(stored(&database).await, empty);
    execute(&database, "DROP TRIGGER test_fail_session").await;

    // A later rotation failure keeps the refresh secret unspent and the family unchanged.
    let native = secrets(NATIVE);
    let family = sessions
        .issue(NATIVE, &observed(3, 1), S0, &native)
        .await
        .unwrap();
    let before = stored(&database).await;
    execute(
        &database,
        "CREATE TRIGGER test_fail_rotation AFTER UPDATE ON identity_sessions \
         BEGIN SELECT RAISE(ABORT, 'cantos:test.rotation'); END",
    )
    .await;
    assert_eq!(
        sessions
            .rotate(
                native.refresh.as_deref().unwrap(),
                &family.session_id,
                &secrets(NATIVE)
            )
            .await
            .unwrap_err(),
        SessionError::Unavailable
    );
    assert_eq!(stored(&database).await, before);
    execute(&database, "DROP TRIGGER test_fail_rotation").await;
    assert!(sessions
        .rotate(
            native.refresh.as_deref().unwrap(),
            &family.session_id,
            &secrets(NATIVE)
        )
        .await
        .is_ok());

    // A failed revocation write is reported and leaves the session exactly as it was; the shell
    // treats it as best effort and redoes it only while Identity still differs (inventory gap).
    let web = secrets(THEATRE);
    let live = sessions
        .issue(THEATRE, &observed(3, 1), S0, &web)
        .await
        .unwrap();
    execute(
        &database,
        "CREATE TRIGGER test_fail_revoke AFTER UPDATE ON identity_sessions WHEN NEW.revoked = 1 \
         BEGIN SELECT RAISE(ABORT, 'cantos:test.revoke'); END",
    )
    .await;
    for _ in 0..2 {
        assert_eq!(
            sessions
                .revoke(&live.session_id, RevokeReason::Disabled)
                .await
                .unwrap_err(),
            SessionError::Unavailable
        );
        assert!(sessions.access(&web.access, THEATRE).await.is_ok());
    }
    assert_eq!(
        sessions.logout(&web.access, THEATRE).await.unwrap_err(),
        SessionError::Unavailable
    );
    execute(&database, "DROP TRIGGER test_fail_revoke").await;
    sessions
        .revoke(&live.session_id, RevokeReason::Disabled)
        .await
        .unwrap();
    assert_eq!(
        revoked_with(&session_row(&database, &live.session_id).await),
        (1, Some("disabled".into()))
    );
}

#[tokio::test]
async fn wrong_subject_actor_or_channel_and_an_inactive_actor_never_admit() {
    let run = TestRun::new("sessions-turso-refusals");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock.clone()).await;
    let sessions = TursoSessions::new(database.clone());
    let web = secrets(THEATRE);
    let native = secrets(NATIVE);
    let first = sessions
        .issue(THEATRE, &observed(4, 2), S0, &web)
        .await
        .unwrap();
    let family = sessions
        .issue(NATIVE, &observed(4, 2), S0, &native)
        .await
        .unwrap();
    // Another subject gets another actor; nothing links them by email or anything else.
    let unrelated = secrets(THEATRE);
    let other = sessions
        .issue(THEATRE, &observed(5, 2), S0, &unrelated)
        .await
        .unwrap();
    assert_ne!(other.actor_id, first.actor_id);
    // A refresh secret never matches another family, and a session id never another secret.
    assert_eq!(
        sessions
            .peek_refresh(native.refresh.as_deref().unwrap(), &first.session_id)
            .await
            .unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        sessions
            .peek_refresh(&format!("ci1_r_{}", "a".repeat(64)), &family.session_id)
            .await
            .unwrap_err(),
        SessionError::Inactive
    );
    // Any spelling the shell's parser admits names the same family; garbage names none.
    let upper = family.session_id.to_uppercase();
    assert_eq!(
        sessions
            .peek_refresh(native.refresh.as_deref().unwrap(), &upper)
            .await
            .unwrap()
            .session_id,
        family.session_id
    );
    let urn = format!("urn:uuid:{}", family.session_id);
    let braced = format!("{{{}}}", family.session_id);
    assert_eq!(
        sessions
            .peek_refresh(native.refresh.as_deref().unwrap(), &braced)
            .await
            .unwrap()
            .session_id,
        family.session_id
    );
    for id in [
        "",
        "not-a-uuid",
        "00000000-0000-0000-0000-00000000000g",
        urn.as_str(),
        &urn.to_uppercase(),
    ] {
        assert_eq!(
            sessions
                .peek_refresh(native.refresh.as_deref().unwrap(), id)
                .await
                .unwrap_err(),
            SessionError::Inactive
        );
    }

    // Deactivating the actor stops every access, renewal and rotation, and issues nothing new.
    let before = stored(&database).await;
    execute(
        &database,
        &format!("UPDATE actors SET active=0 WHERE id='{}'", first.actor_id),
    )
    .await;
    assert_eq!(
        sessions.access(&web.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        sessions.renew_web(&web.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        sessions
            .rotate(
                native.refresh.as_deref().unwrap(),
                &family.session_id,
                &secrets(NATIVE)
            )
            .await
            .unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        sessions
            .issue(STUDIO, &observed(4, 2), S0, &secrets(STUDIO))
            .await
            .unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        sessions.bind(subject(4)).await.unwrap_err(),
        SessionError::Inactive
    );
    let after = stored(&database).await;
    assert_eq!(after.sessions, before.sessions);
    assert_eq!(after.tokens, before.tokens);
    // The other subject's session is unaffected.
    assert_eq!(
        sessions
            .access(&unrelated.access, THEATRE)
            .await
            .unwrap()
            .actor_id,
        other.actor_id
    );
}

#[tokio::test]
async fn revocation_is_idempotent_keeps_its_first_reason_and_survives_reopen() {
    let run = TestRun::new("sessions-turso-reopen");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock.clone()).await;
    let sessions = TursoSessions::new(database.clone());
    let revoked = secrets(THEATRE);
    let spent = secrets(NATIVE);
    let live = secrets(STUDIO);
    let first = sessions
        .issue(THEATRE, &observed(6, 1), S0, &revoked)
        .await
        .unwrap();
    let family = sessions
        .issue(NATIVE, &observed(6, 1), S0, &spent)
        .await
        .unwrap();
    sessions
        .issue(STUDIO, &observed(6, 1), S0, &live)
        .await
        .unwrap();
    sessions
        .revoke(&first.session_id, RevokeReason::VersionAdvanced)
        .await
        .unwrap();
    sessions
        .revoke(&first.session_id, RevokeReason::Disabled)
        .await
        .unwrap();
    sessions.logout(&revoked.access, THEATRE).await.unwrap();
    assert_eq!(
        revoked_with(&session_row(&database, &first.session_id).await),
        (1, Some("version-advanced".into()))
    );
    let next = secrets(NATIVE);
    sessions
        .rotate(spent.refresh.as_deref().unwrap(), &family.session_id, &next)
        .await
        .unwrap();
    let before = stored(&database).await;
    drop(sessions);
    drop(database);

    let database = open(&run, Intent::Serve, clock.clone()).await;
    let sessions = TursoSessions::new(database.clone());
    assert_eq!(stored(&database).await, before);
    assert_eq!(
        sessions.access(&revoked.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        sessions.access(&spent.access, NATIVE).await.unwrap_err(),
        SessionError::Inactive
    );
    assert!(sessions.access(&next.access, NATIVE).await.is_ok());
    assert!(sessions.access(&live.access, STUDIO).await.is_ok());
    // A spent secret is still a replay after reopen, and the revocation it causes is permanent.
    assert_eq!(
        sessions
            .peek_refresh(spent.refresh.as_deref().unwrap(), &family.session_id)
            .await
            .unwrap_err(),
        SessionError::Replay
    );
    drop(sessions);
    drop(database);
    let database = open(&run, Intent::Serve, clock).await;
    let sessions = TursoSessions::new(database.clone());
    assert_eq!(
        sessions.access(&next.access, NATIVE).await.unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        revoked_with(&session_row(&database, &family.session_id).await),
        (1, Some("replay".into()))
    );
}

#[tokio::test]
async fn stored_rows_hold_only_digests_of_the_secrets() {
    let run = TestRun::new("sessions-turso-digests");
    let database = open(&run, Intent::Migrate, FixedClock::new(T0)).await;
    let sessions = TursoSessions::new(database.clone());
    let native = secrets(NATIVE);
    let web = secrets(STUDIO);
    let family = sessions
        .issue(NATIVE, &observed(7, 1), S0, &native)
        .await
        .unwrap();
    let cookie = sessions
        .issue(STUDIO, &observed(7, 1), S0, &web)
        .await
        .unwrap();
    let rows = stored(&database).await;
    let refresh = native.refresh.as_deref().unwrap();
    assert_eq!(rows.tokens, {
        let mut expected = vec![
            vec![
                Value::Blob(digest(&native.access).to_vec()),
                Value::Blob(digest(refresh).to_vec()),
                Value::Text(family.session_id.clone()),
                Value::Integer((S0 + 120) as i64),
                Value::Integer(0),
            ],
            vec![
                Value::Blob(digest(&web.access).to_vec()),
                Value::Null,
                Value::Text(cookie.session_id.clone()),
                Value::Integer((S0 + 300) as i64),
                Value::Integer(0),
            ],
        ];
        expected.sort_by_key(|row| format!("{:?}", row[2]));
        expected
    });
    let everything = format!("{rows:?}");
    for secret in [native.access.as_str(), refresh, web.access.as_str()] {
        assert!(!everything.contains(secret));
        assert!(!everything.contains(&secret[6..]));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_refreshes_spend_once_and_the_loser_revokes_the_family() {
    let run = TestRun::new("sessions-turso-concurrent-refresh");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock).await;
    let sessions = TursoSessions::new(database.clone());
    let native = secrets(NATIVE);
    let family = sessions
        .issue(NATIVE, &observed(8, 1), S0, &native)
        .await
        .unwrap();
    let refresh = native.refresh.clone().unwrap();
    // Both requests peeked before either rotates, as two devices racing with one secret would.
    for _ in 0..2 {
        sessions
            .peek_refresh(&refresh, &family.session_id)
            .await
            .unwrap();
    }
    let pairs = [secrets(NATIVE), secrets(NATIVE)];
    let accesses: Vec<String> = pairs.iter().map(|pair| pair.access.clone()).collect();
    let racing = pairs.map(|pair| {
        let sessions = sessions.clone();
        let refresh = refresh.clone();
        let id = family.session_id.clone();
        tokio::spawn(async move { sessions.rotate(&refresh, &id, &pair).await })
    });
    let mut outcomes = Vec::new();
    for task in racing {
        outcomes.push(task.await.unwrap().map(|rotated| rotated.lifetime));
    }
    assert_eq!(
        outcomes.iter().filter(|outcome| outcome.is_ok()).count(),
        1,
        "{outcomes:?}"
    );
    assert!(outcomes.contains(&Err(SessionError::Replay)));
    for access in &accesses {
        assert_eq!(
            sessions.access(access, NATIVE).await.unwrap_err(),
            SessionError::Inactive,
            "the losing replay revoked the winner's family too"
        );
    }
    let rows = stored(&database).await;
    assert_eq!(revoked_with(&rows.sessions[0]), (1, Some("replay".into())));
    // Exactly one secret was spent, and only the winner's pair was ever written.
    let used: Vec<_> = rows.tokens.iter().map(|row| row[4].clone()).collect();
    assert_eq!(used.iter().filter(|v| **v == Value::Integer(1)).count(), 1);
    assert_eq!(rows.tokens.len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_business_write_admits_inside_its_unit_and_logout_linearizes_around_it() {
    let run = TestRun::new("sessions-turso-admission");
    let database = open(&run, Intent::Migrate, FixedClock::new(T0)).await;
    let sessions = TursoSessions::new(database.clone());
    let web = secrets(THEATRE);
    let issued = sessions
        .issue(THEATRE, &observed(9, 1), S0, &web)
        .await
        .unwrap();

    // Admitted work holds the writer; a logout waits for it instead of interleaving.
    let tx = database.owner().begin_write().await.unwrap();
    let admitted = admit_in(&tx, &web.access, THEATRE).await.unwrap();
    assert_eq!(admitted.session_id, issued.session_id);
    assert_eq!(
        admit_in(&tx, &web.access, STUDIO).await.unwrap_err(),
        SessionError::Inactive
    );
    let logout = {
        let sessions = sessions.clone();
        let token = web.access.clone();
        tokio::spawn(async move { sessions.logout(&token, THEATRE).await })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !logout.is_finished(),
        "logout must wait for the admitted unit"
    );
    // Readers keep their snapshot: the session is live until the logout commits.
    assert!(sessions.access(&web.access, THEATRE).await.is_ok());
    tx.commit().await.unwrap();
    logout.await.unwrap().unwrap();

    // A logout committed first is the fact the next unit admits against.
    let tx = database.owner().begin_write().await.unwrap();
    assert_eq!(
        admit_in(&tx, &web.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn a_lost_acknowledgement_is_unknown_and_reconciles_without_a_retry() {
    let run = TestRun::new("sessions-turso-unknown");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock.clone()).await;
    let engine = TursoSessions::new(database.clone());
    let committed =
        TursoSessions::with_acknowledgement(database.clone(), Acknowledgement::LostAfterCommit);
    let rolled_back =
        TursoSessions::with_acknowledgement(database.clone(), Acknowledgement::LostAfterRollback);

    // Lost after a real commit: the session exists, and its own secret is the reconciliation.
    let web = secrets(THEATRE);
    assert_eq!(
        committed
            .issue(THEATRE, &observed(10, 1), S0, &web)
            .await
            .unwrap_err(),
        SessionError::Unknown
    );
    let durable = engine.access(&web.access, THEATRE).await.unwrap();
    // Lost after a real rollback: nothing exists and a new login simply starts over.
    let lost = secrets(THEATRE);
    let before = stored(&database).await;
    assert_eq!(
        rolled_back
            .issue(THEATRE, &observed(11, 1), S0, &lost)
            .await
            .unwrap_err(),
        SessionError::Unknown
    );
    assert_eq!(
        stored(&database).await,
        before,
        "no actor, binding or session"
    );
    assert_eq!(
        engine.access(&lost.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive
    );

    // A refresh whose acknowledgement was lost after commit spent its secret. Nothing retries
    // it: the client's own retry is a replay that ends the family, exactly as designed.
    let native = secrets(NATIVE);
    let family = engine
        .issue(NATIVE, &observed(10, 1), S0, &native)
        .await
        .unwrap();
    let refresh = native.refresh.clone().unwrap();
    let next = secrets(NATIVE);
    assert_eq!(
        committed
            .rotate(&refresh, &family.session_id, &next)
            .await
            .unwrap_err(),
        SessionError::Unknown
    );
    assert!(engine.access(&next.access, NATIVE).await.is_ok());
    assert_eq!(
        engine
            .peek_refresh(&refresh, &family.session_id)
            .await
            .unwrap_err(),
        SessionError::Replay
    );
    assert_eq!(
        engine.access(&next.access, NATIVE).await.unwrap_err(),
        SessionError::Inactive
    );

    // A lost logout acknowledgement is reconciled by the durable state the next request reads.
    assert_eq!(
        committed.logout(&web.access, THEATRE).await.unwrap_err(),
        SessionError::Unknown
    );
    assert_eq!(
        engine.access(&web.access, THEATRE).await.unwrap_err(),
        SessionError::Inactive
    );
    assert_eq!(
        revoked_with(&session_row(&database, &durable.session_id).await),
        (1, Some("logout".into()))
    );
    // A decision that wrote nothing needs no acknowledgement.
    assert_eq!(committed.logout(&web.access, THEATRE).await, Ok(()));
    assert!(committed.access(&native.access, NATIVE).await.is_err());
}

/// Production hydration receives malformed `SELECT` projections from the real engine; no table
/// constraint, trigger or foreign-key setting is weakened to construct a row.
async fn projection(database: &ServerDatabase, values: Vec<Value>) -> Row {
    let columns = (1..=values.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let reader = database.owner().reader().await.unwrap();
    let mut rows = reader
        .query(&format!("SELECT {columns}"), params_from_iter(values))
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert!(rows.next().await.unwrap().is_none());
    row
}

#[tokio::test]
async fn corrupt_projections_are_refused_by_the_production_decoders() {
    let run = TestRun::new("sessions-turso-decoders");
    let database = open(&run, Intent::Migrate, FixedClock::new(T0)).await;
    let text = |value: &str| Value::Text(value.into());
    let id = "0f0e0d0c-0b0a-4908-8706-050403020100";
    let valid = vec![
        text(id),
        text("actor-1"),
        text("theatre-native"),
        text(&"ab".repeat(16)),
        Value::Blob(vec![7; 16]),
        Value::Integer(4),
        Value::Integer(10),
        Value::Integer(900),
        Value::Integer(600),
        Value::Integer(120),
        Value::Integer(0),
        Value::Integer(1),
    ];
    let decoded = record(&projection(&database, valid[..10].to_vec()).await).unwrap();
    assert_eq!(decoded.session_id, id);
    assert_eq!(decoded.channel, NATIVE);
    assert_eq!(decoded.observed.version, 4);
    assert_eq!(
        (
            decoded.absolute_expires,
            decoded.idle_expires,
            decoded.token_expires
        ),
        (900, 600, 120)
    );
    let (_, revoked, used) = refresh_record(&projection(&database, valid.clone()).await).unwrap();
    assert_eq!((revoked, used), (false, true));
    let replace = |index: usize, value: Value| {
        let mut values = valid.clone();
        values[index] = value;
        values
    };
    for values in [
        replace(0, text(&id.to_uppercase())),
        replace(0, text("0f0e0d0c0b0a490887060504030201")),
        replace(0, Value::Null),
        replace(1, text("Actor")),
        replace(1, text("")),
        replace(2, text("studio-native")),
        replace(3, text(&"AB".repeat(16))),
        replace(3, text(&"00".repeat(16))),
        replace(3, text(&"ab".repeat(15))),
        replace(4, Value::Blob(vec![0; 16])),
        replace(4, Value::Blob(vec![7; 15])),
        replace(4, text(&"07".repeat(16))),
        replace(5, Value::Integer(0)),
        replace(5, Value::Integer(-1)),
        replace(5, text("4")),
        replace(6, Value::Integer(0)),
        replace(7, Value::Integer(-1)),
        replace(8, Value::Integer(901)),
        replace(9, Value::Real(120.0)),
        replace(10, Value::Integer(2)),
        replace(11, Value::Null),
        valid[..11].to_vec(),
    ] {
        assert_eq!(
            refresh_record(&projection(&database, values.clone()).await).unwrap_err(),
            SessionError::Unavailable,
            "{values:?}"
        );
    }
    // A Web row can never be a refresh family.
    assert_eq!(
        refresh_record(&projection(&database, replace(2, text("theatre-web"))).await).unwrap_err(),
        SessionError::Unavailable
    );
    assert_eq!(
        stored_session_id(&projection(&database, vec![text(id)]).await, 0),
        Ok(id.to_owned())
    );
    let reader = database.owner().reader().await.unwrap();
    let rows = reader
        .query("SELECT 'a' UNION ALL SELECT 'b'", ())
        .await
        .unwrap();
    assert_eq!(
        super::at_most_one(rows, |row| super::text(row, 0)).await,
        Err(SessionError::Unavailable)
    );
}

// ---- the production orchestration over a scripted Identity --------------------------------------
//
// `IdentityAuth` with these adapters against the scripted `cantos.identity.v1` server of the
// PostgreSQL facts suite: the answers the real executable never gives (a security change at
// admission or between fact and issue, an unknown subject, a slow reply). Mock-integration
// evidence about what the Server refuses on Turso, never a login proof; the real executable runs
// in the `identity_auth_candidate` journey.

#[path = "../../../../tests/support/fake_identity.rs"]
mod fake;

use crate::platform::{
    identity_auth::{config::IdentityAuthConfig, AuthFailure, IdentityAuth, Login as IssuedLogin},
    identity_auth::PeerPolicy,
};
use cantos_store_turso::clock::SystemClock;
use fake::{Script, Serving};

const EMAIL: &str = "owner@example.com";
const PASSWORD: &str = "a correct horse battery";

/// A short private directory for the scripted socket (Unix socket paths stay under ~100 bytes),
/// inside the isolation guard's run directory when there is one.
struct Scripted {
    dir: std::path::PathBuf,
    fake: Serving,
    auth: IdentityAuth,
    database: ServerDatabase,
    sessions: TursoSessions,
    _run: TestRun,
}

impl Scripted {
    async fn start() -> Self {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::var_os("CANTOS_NATIVE_TEST_DIRECTORY")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        let dir = root.join(format!(
            "ct-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let uid = std::fs::metadata(&dir).unwrap().uid();
        let socket = dir.join("identity.sock");
        let fake = Serving::start(&socket, Script::healthy());
        let auth = IdentityAuth::new(
            IdentityAuthConfig {
                socket,
                peer_uid: uid,
                peer_policy: PeerPolicy::DirectPeer,
                studio_subjects: Vec::new(),
            },
            true,
        );
        let run = TestRun::new("sessions-scripted");
        let database = ServerDatabase::open(
            run.path("server.db"),
            Intent::Migrate,
            Arc::new(SystemClock),
        )
        .await
        .unwrap();
        let sessions = TursoSessions::new(database.clone());
        Self {
            dir,
            fake,
            auth,
            database,
            sessions,
            _run: run,
        }
    }

    fn client() -> std::net::IpAddr {
        "127.0.0.1".parse().unwrap()
    }

    async fn login(&self, channel: Channel) -> Result<IssuedLogin, AuthFailure> {
        self.auth
            .login(&self.sessions, channel, EMAIL, PASSWORD, Self::client())
            .await
    }

    async fn admitted(&self, token: &str, channel: Channel) -> Result<(), AuthFailure> {
        self.auth
            .authenticate(&self.sessions, token, channel)
            .await
            .map(|_| ())
    }

    async fn reasons(&self) -> Vec<Value> {
        rows(
            &self.database,
            "SELECT revoked_reason FROM identity_sessions ORDER BY authenticated_at,id",
        )
        .await
        .into_iter()
        .map(|row| row[0].clone())
        .collect()
    }
}

impl Drop for Scripted {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn reason(text: &str) -> Value {
    Value::Text(text.to_owned())
}

#[tokio::test]
async fn every_security_change_revokes_the_turso_family_durably_and_never_resurrects() {
    type Mutation = fn(&mut Script);
    let changes: [(&str, Mutation, &str); 5] = [
        ("version advanced", |s| s.version = 4, "version-advanced"),
        ("version regressed", |s| s.version = 2, "version-regressed"),
        ("disabled", |s| s.active = false, "disabled"),
        (
            "restored instance",
            |s| s.epoch = [0x33; 16],
            "epoch-changed",
        ),
        (
            "unknown subject",
            |s| s.state_not_found = true,
            "subject-missing",
        ),
    ];
    for (label, mutate, expected) in changes {
        let h = Scripted::start().await;
        let login = h.login(THEATRE).await.expect(label);
        assert_eq!(
            h.admitted(&login.secrets.access, THEATRE).await,
            Ok(()),
            "{label}"
        );
        let original = h.fake.script.lock().unwrap().clone();
        h.fake.set(mutate);
        assert_eq!(
            h.admitted(&login.secrets.access, THEATRE).await,
            Err(AuthFailure::Unauthenticated),
            "{label}"
        );
        assert_eq!(h.reasons().await, [reason(expected)], "{label}");
        // Identity looks exactly as it did at issue again: the family stays dead, because the
        // revocation is a durable fact, not a comparison.
        h.fake.set(|s| *s = original);
        assert_eq!(
            h.admitted(&login.secrets.access, THEATRE).await,
            Err(AuthFailure::Unauthenticated),
            "{label}: resurrected"
        );
        assert_eq!(h.reasons().await, [reason(expected)], "{label}");
    }
}

#[tokio::test]
async fn a_security_change_between_the_fact_and_the_issue_binds_and_cuts_nothing() {
    type Mutation = fn(&mut Script);
    let races: [(&str, Mutation, AuthFailure); 6] = [
        (
            "version advanced",
            |s| s.version = 4,
            AuthFailure::Unauthenticated,
        ),
        (
            "version regressed",
            |s| s.version = 2,
            AuthFailure::Unauthenticated,
        ),
        (
            "disabled",
            |s| s.active = false,
            AuthFailure::Unauthenticated,
        ),
        (
            "restored epoch",
            |s| s.epoch = [0x33; 16],
            AuthFailure::Unauthenticated,
        ),
        // Not even a well-formed answer: fail closed as unavailable, not as a denial.
        (
            "another subject",
            |s| s.state_subject = Some(vec![0x44; 16]),
            AuthFailure::Unavailable,
        ),
        (
            "unknown state",
            |s| s.state_value = Some(0),
            AuthFailure::Unavailable,
        ),
    ];
    for (label, mutate, expected) in races {
        let h = Scripted::start().await;
        h.fake.set(mutate);
        for channel in [THEATRE, NATIVE] {
            assert_eq!(h.login(channel).await.err(), Some(expected), "{label}");
        }
        let rows = stored(&h.database).await;
        assert!(
            rows.actors.is_empty() && rows.bindings.is_empty() && rows.sessions.is_empty(),
            "{label}: {rows:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_first_logins_of_one_subject_bind_exactly_one_actor() {
    let h = Scripted::start().await;
    let (a, b, c) = tokio::join!(h.login(THEATRE), h.login(NATIVE), h.login(THEATRE));
    let (a, b, c) = (a.unwrap(), b.unwrap(), c.unwrap());
    assert_eq!(a.actor_id, b.actor_id);
    assert_eq!(b.actor_id, c.actor_id);
    let rows = stored(&h.database).await;
    assert_eq!(
        rows.actors.len(),
        1,
        "a losing candidate actor is never left behind"
    );
    assert_eq!(rows.bindings.len(), 1);
    assert_eq!(rows.sessions.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn renew_racing_logout_always_ends_revoked_and_never_fails_unavailable() {
    for _ in 0..4 {
        let h = Scripted::start().await;
        let login = h.login(THEATRE).await.unwrap();
        let token = login.secrets.access.clone();
        let (renewed, out) = tokio::join!(
            h.auth.renew_web(&h.sessions, &token, THEATRE),
            h.auth.logout(&h.sessions, &token, THEATRE)
        );
        assert!(
            matches!(renewed, Ok(_) | Err(AuthFailure::Unauthenticated)),
            "{:?}",
            renewed.err()
        );
        assert_eq!(out, Ok(()));
        assert_eq!(
            h.admitted(&token, THEATRE).await,
            Err(AuthFailure::Unauthenticated)
        );
        assert_eq!(h.reasons().await, [reason("logout")]);
    }
}

#[tokio::test]
async fn a_slow_identity_fails_closed_without_revoking_and_the_session_resumes() {
    let h = Scripted::start().await;
    let login = h.login(THEATRE).await.unwrap();
    h.fake.set(|s| s.state_delay = Duration::from_secs(4));
    assert_eq!(
        h.admitted(&login.secrets.access, THEATRE).await,
        Err(AuthFailure::Unavailable)
    );
    assert_eq!(
        h.reasons().await,
        [Value::Null],
        "an outage is not a security change"
    );
    h.fake.set(|s| s.state_delay = Duration::ZERO);
    assert_eq!(h.admitted(&login.secrets.access, THEATRE).await, Ok(()));
}
