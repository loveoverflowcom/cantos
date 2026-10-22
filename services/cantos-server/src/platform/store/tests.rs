//! The statement facade and the Studio development-token admission on real files. Barrier tests
//! write through the fixture's raw SQL, never through the code under test.
use cantos_store_turso::testing::FixedClock;

use super::{active_actors, authenticate, token_hash, Error, Param, StoreError};
use crate::test_support::ServerFixture;

const T0: i64 = 1_700_000_000_000_000;
const TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[tokio::test]
async fn rows_decode_checked_types_and_never_default() {
    let fixture = ServerFixture::new().await;
    fixture.seed_actors(&["alice"]).await;
    let conn = fixture.store().connection().await.unwrap();
    let probe = conn.read().await.unwrap();
    let rows = probe
        .query("SELECT id,active,NULL AS absent FROM actors", &[])
        .await
        .map_err(|error| panic!("{error:?}"))
        .unwrap();
    let row = &rows[0];
    assert_eq!(row.try_get::<_, String>("id").unwrap(), "alice");
    assert!(row.try_get::<_, bool>(1).unwrap());
    assert_eq!(row.try_get::<_, Option<String>>(2).unwrap(), None);
    // A column of another type, an unknown name and an out-of-range index are corruption/misuse.
    assert!(matches!(row.try_get::<_, i64>("id"), Err(Error::Column)));
    assert!(matches!(row.try_get::<_, String>(1), Err(Error::Column)));
    assert!(matches!(
        row.try_get::<_, String>("missing"),
        Err(Error::Column)
    ));
    assert!(matches!(row.try_get::<_, String>(9), Err(Error::Column)));
    assert!(matches!(
        StoreError::from(Error::Column),
        StoreError::CorruptRevision
    ));
    // `active` is a boolean only when it is 0 or 1.
    let wide = fixture.rows("SELECT 2 AS flag", &[]).await.unwrap();
    assert!(matches!(wide[0].try_get::<_, bool>(0), Err(Error::Column)));
    // Narrowing is checked, never wrapped.
    let big = fixture.rows("SELECT 4294967296 AS n", &[]).await.unwrap();
    assert!(matches!(big[0].try_get::<_, i32>(0), Err(Error::Column)));
    assert_eq!(big[0].try_get::<_, i64>(0).unwrap(), 4_294_967_296);
}

#[tokio::test]
async fn parameters_bind_by_position_and_sets_travel_as_json_arrays() {
    let fixture = ServerFixture::new().await;
    fixture.seed_actors(&["alice", "bob", "carol"]).await;
    fixture
        .exec(
            "UPDATE actors SET active=?2 WHERE id=?1",
            &[&"carol", &false],
        )
        .await
        .unwrap();
    let ids = vec!["alice".to_owned(), "carol".to_owned(), "nobody".to_owned()];
    let mut conn = fixture.store().connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    let found = active_actors(&tx, &ids).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(found.into_iter().collect::<Vec<_>>(), ["alice"]);
    // A hostile identifier is data inside the JSON array, never SQL.
    let hostile = vec!["x') OR 1=1 --".to_owned()];
    let tx = conn.transaction().await.unwrap();
    assert!(active_actors(&tx, &hostile).await.unwrap().is_empty());
    tx.rollback().await.unwrap();
    let none: Option<&str> = None;
    let row = fixture
        .rows(
            "SELECT ?1 IS NULL, ?2",
            &[&none, &7_i64 as &(dyn Param + Sync)],
        )
        .await
        .unwrap();
    assert!(row[0].get::<_, bool>(0));
    assert_eq!(row[0].get::<_, i64>(1), 7);
}

#[tokio::test]
async fn a_write_unit_commits_atomically_and_an_abandoned_one_changes_nothing() {
    let fixture = ServerFixture::new().await;
    let mut conn = fixture.store().connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    tx.execute("INSERT INTO actors(id) VALUES('kept')", &[])
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let tx = conn.transaction().await.unwrap();
    tx.execute("INSERT INTO actors(id) VALUES('dropped')", &[])
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    // A unit dropped without an answer (an early `?`) is rolled back before the next write.
    let tx = conn.transaction().await.unwrap();
    tx.execute("INSERT INTO actors(id) VALUES('abandoned')", &[])
        .await
        .unwrap();
    drop(tx);
    let tx = conn.transaction().await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(fixture.count("SELECT count(*) FROM actors", &[]).await, 1);
}

#[tokio::test]
async fn a_read_snapshot_cannot_write_and_stays_pinned_while_a_write_commits() {
    let fixture = ServerFixture::new().await;
    fixture.seed_actors(&["first"]).await;
    let conn = fixture.store().connection().await.unwrap();
    let snapshot = conn.read().await.unwrap();
    assert!(matches!(
        snapshot
            .execute("INSERT INTO actors(id) VALUES('x')", &[])
            .await,
        Err(Error::ReadOnly)
    ));
    let before = snapshot
        .query_one("SELECT count(*) FROM actors", &[])
        .await
        .unwrap();
    fixture.seed_actors(&["second"]).await;
    let during = snapshot
        .query_one("SELECT count(*) FROM actors", &[])
        .await
        .unwrap();
    assert_eq!(before.get::<_, i64>(0), 1);
    assert_eq!(during.get::<_, i64>(0), 1, "the snapshot is pinned");
    snapshot.commit().await.unwrap();
    assert_eq!(fixture.count("SELECT count(*) FROM actors", &[]).await, 2);
}

#[tokio::test]
async fn constraint_and_guard_faults_are_typed_and_leave_the_unit_open_for_rollback() {
    let fixture = ServerFixture::new().await;
    fixture.seed_actors(&["alice"]).await;
    let mut conn = fixture.store().connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    let duplicate = tx
        .execute("INSERT INTO actors(id) VALUES('alice')", &[])
        .await
        .unwrap_err();
    // The registry's no-replace guard answers before a unique conflict can resolve.
    assert_eq!(duplicate.guard(), Some("immutable.actor_id"));
    assert!(duplicate.is_integrity_violation());
    let check = tx
        .execute("INSERT INTO actors(id) VALUES('UPPER')", &[])
        .await
        .unwrap_err();
    assert_eq!(
        check.constraint(),
        Some(cantos_store_turso::error::ConstraintKind::Check)
    );
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn query_one_demands_exactly_one_row() {
    let fixture = ServerFixture::new().await;
    fixture.seed_actors(&["a", "b"]).await;
    let conn = fixture.store().connection().await.unwrap();
    let tx = conn.read().await.unwrap();
    assert!(matches!(
        tx.query_one("SELECT id FROM actors", &[]).await,
        Err(Error::Cardinality)
    ));
    assert!(matches!(
        tx.query_one("SELECT id FROM actors WHERE id='none'", &[])
            .await,
        Err(Error::Cardinality)
    ));
    assert!(tx
        .query_opt("SELECT id FROM actors WHERE id='none'", &[])
        .await
        .unwrap()
        .is_none());
    assert!(matches!(
        tx.query_opt("SELECT id FROM actors", &[]).await,
        Err(Error::Cardinality)
    ));
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn development_tokens_admit_only_live_active_unrevoked_sessions_inside_the_unit() {
    let clock = FixedClock::new(T0);
    let fixture = ServerFixture::with_clock(clock.clone()).await;
    fixture.studio_caller("alice", TOKEN).await;
    let store = fixture.store();
    assert_eq!(store.authenticate(TOKEN).await.unwrap(), "alice");
    assert!(store.development_sessions_enabled());

    // Shapes that are not development tokens never reach the table.
    for token in ["", "short", &"g".repeat(64), "ca1_legacy", "cr1_legacy"] {
        assert!(matches!(
            store.authenticate(token).await,
            Err(StoreError::Unauthenticated)
        ));
    }
    // An unknown well-formed token is unauthenticated, not unavailable.
    assert!(matches!(
        store.authenticate(&"b".repeat(64)).await,
        Err(StoreError::Unauthenticated)
    ));
    // The business unit re-admits the same token at its own stamp.
    let mut conn = store.connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    assert_eq!(authenticate(&tx, TOKEN).await.unwrap(), "alice");
    tx.rollback().await.unwrap();

    // Deactivation is observed by the next admission.
    fixture
        .exec("UPDATE actors SET active=0 WHERE id='alice'", &[])
        .await
        .unwrap();
    assert!(matches!(
        store.authenticate(TOKEN).await,
        Err(StoreError::Unauthenticated)
    ));
    fixture
        .exec("UPDATE actors SET active=1 WHERE id='alice'", &[])
        .await
        .unwrap();
    assert_eq!(store.authenticate(TOKEN).await.unwrap(), "alice");

    // Expiry is exclusive and follows the trusted clock, never SQL time.
    clock.set(T0 + 3_600_000_000 - 1);
    assert_eq!(store.authenticate(TOKEN).await.unwrap(), "alice");
    clock.set(T0 + 3_600_000_000);
    assert!(matches!(
        store.authenticate(TOKEN).await,
        Err(StoreError::Unauthenticated)
    ));
    clock.set(T0);

    // Revocation is durable and permanent.
    store.revoke(TOKEN).await.unwrap();
    assert!(matches!(
        store.authenticate(TOKEN).await,
        Err(StoreError::Unauthenticated)
    ));
    assert!(fixture
        .exec(
            "UPDATE development_sessions SET revoked=0 WHERE token_hash=?1",
            &[&token_hash(TOKEN)]
        )
        .await
        .is_err());
}

#[tokio::test]
async fn a_production_host_refuses_every_development_token_and_a_restart_keeps_the_rows() {
    let mut fixture = ServerFixture::new().await;
    fixture.studio_caller("alice", TOKEN).await;
    let production = fixture.store().clone().without_development_sessions();
    assert!(!production.development_sessions_enabled());
    assert!(matches!(
        production.authenticate(TOKEN).await,
        Err(StoreError::Unauthenticated)
    ));
    drop(production);
    fixture.restart().await;
    assert_eq!(fixture.store().authenticate(TOKEN).await.unwrap(), "alice");
    let id = fixture.store().studio_session_id(TOKEN).await.unwrap();
    assert!(id.is_some_and(|id| uuid::Uuid::parse_str(&id).is_ok()));
    assert_eq!(
        fixture
            .store()
            .studio_session_id(&"c".repeat(64))
            .await
            .unwrap(),
        None
    );
}
