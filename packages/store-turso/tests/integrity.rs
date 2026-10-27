//! Explicit recovery integrity checks on the real pinned engine, including a negative control
//! that startup registry validation alone cannot detect. No service/live files are opened.
#![cfg(feature = "testing")]

use cantos_store_turso::{
    error::StoreFault,
    registry::{Intent, Migration, Registry},
    testing::{FixedClock, TestRun, TEST_APPLICATION_ID},
    OpenRequest, OwnedDatabase,
};

const MIGRATIONS: [Migration; 1] = [Migration {
    version: 1,
    name: "integrity_fixture",
    sql: "CREATE TABLE parent(id INTEGER PRIMARY KEY) STRICT; \
          CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id)) STRICT; \
          CREATE TABLE parent_pair(a INTEGER,b TEXT,PRIMARY KEY(a,b)) STRICT; \
          CREATE TABLE child_pair(id INTEGER PRIMARY KEY,a INTEGER,b TEXT,FOREIGN KEY(a,b) REFERENCES parent_pair(a,b)) STRICT;",
}];

#[tokio::test]
async fn integrity_check_detects_a_dangling_foreign_key_even_when_registry_and_pages_are_valid() {
    let run = TestRun::new("integrity-negative");
    let path = run.path("integrity.db");
    let registry = Registry::new(TEST_APPLICATION_ID, 1, &MIGRATIONS).unwrap();
    let request = |intent| OpenRequest::new(&path, registry, intent, FixedClock::new(1));
    let owner = OwnedDatabase::open(request(Intent::Migrate)).await.unwrap();
    owner.check_integrity().await.unwrap();
    drop(owner);

    // The pinned engine defaults to foreign_keys=OFF outside OwnedDatabase. Inject damage
    // using a stopped fixture file, then prove the recovery check fails independently of
    // opening the correct registry. This raw connection is never a production adapter.
    let raw = turso::Builder::new_local(path.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let connection = raw.connect().unwrap();
    connection
        .execute("INSERT INTO child VALUES(1,42)", ())
        .await
        .unwrap();
    drop(connection);
    drop(raw);
    let owner = OwnedDatabase::open(request(Intent::Serve)).await.unwrap();
    let reader = owner.reader().await.unwrap();
    let mut rows = reader
        .query("SELECT count(*) FROM child WHERE parent_id=42", ())
        .await
        .unwrap();
    assert_eq!(
        rows.next().await.unwrap().unwrap().get::<i64>(0).unwrap(),
        1
    );
    assert_eq!(owner.check_integrity().await, Err(StoreFault::Corrupt));
}

#[tokio::test]
async fn composite_keys_are_checked_together_and_partial_null_keys_are_exempt() {
    let run = TestRun::new("integrity-composite");
    let path = run.path("integrity.db");
    let registry = Registry::new(TEST_APPLICATION_ID, 1, &MIGRATIONS).unwrap();
    let request = |intent| OpenRequest::new(&path, registry, intent, FixedClock::new(1));
    let owner = OwnedDatabase::open(request(Intent::Migrate)).await.unwrap();
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO parent_pair VALUES(1,'x'),(2,'y')", ())
        .await
        .unwrap();
    tx.execute(
        "INSERT INTO child_pair VALUES(1,1,'x'),(2,NULL,'absent'),(3,99,NULL)",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    owner.check_integrity().await.unwrap();
    drop(owner);
    let raw = turso::Builder::new_local(path.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let connection = raw.connect().unwrap();
    // Both individual values exist, but their combination does not.
    connection
        .execute("INSERT INTO child_pair VALUES(4,1,'y')", ())
        .await
        .unwrap();
    drop(connection);
    drop(raw);
    let owner = OwnedDatabase::open(request(Intent::Serve)).await.unwrap();
    assert_eq!(owner.check_integrity().await, Err(StoreFault::Corrupt));
}

#[tokio::test]
async fn unsupported_implicit_parent_keys_refuse_instead_of_being_counted_as_checked() {
    static IMPLICIT: [Migration; 1] = [Migration {
        version: 1,
        name: "implicit_parent_fixture",
        sql: "CREATE TABLE parent(id INTEGER PRIMARY KEY) STRICT; \
              CREATE TABLE child(parent_id INTEGER REFERENCES parent) STRICT;",
    }];
    let run = TestRun::new("integrity-implicit");
    let registry = Registry::new(TEST_APPLICATION_ID, 1, &IMPLICIT).unwrap();
    let owner = OwnedDatabase::open(OpenRequest::new(
        run.path("integrity.db"),
        registry,
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .unwrap();
    assert_eq!(owner.check_integrity().await, Err(StoreFault::Misuse));
}
