//! File-backed `server.db` fixtures for tests. Every database lives in a run-owned private
//! directory removed on drop; nothing here is reachable from a runtime build (the
//! `test-support` feature is enabled only through this package's own dev-dependencies).
//!
//! The fixture holds the one [`Store`] that owns the file, so tests seed and assert through it
//! (a second handle would be refused by the owner lock, as in production). Fixture SQL is
//! parameterized and runs in an ordinary write unit; it is a test convenience, never a model of
//! a privileged operator role.
use std::sync::Arc;

use cantos_store_turso::{
    clock::{Clock, SystemClock},
    registry::Intent,
    testing::TestRun,
};

use crate::{
    application::Store,
    platform::{
        store::{Param, Params, Row, StoreError},
        turso::ServerDatabase,
    },
};

pub mod editorial;
pub mod library;
mod world;
pub use world::{Chain, Corruption, SeededUnit, SeededWork, Stage, UnitSeed, WorkSeed};

pub struct ServerFixture {
    run: TestRun,
    clock: Arc<dyn Clock>,
    store: Option<Store>,
}

impl ServerFixture {
    /// A fresh, migrated `server.db` with the system clock.
    pub async fn new() -> Self {
        Self::with_clock(Arc::new(SystemClock)).await
    }

    pub async fn with_clock(clock: Arc<dyn Clock>) -> Self {
        let run = TestRun::new("server");
        let database = ServerDatabase::open(run.path("server.db"), Intent::Migrate, clock.clone())
            .await
            .expect("a fresh server.db is created");
        Self {
            run,
            clock,
            store: Some(Store::from_database(database)),
        }
    }

    /// The fixture clock's reading in microseconds since the Unix epoch.
    pub fn clock_reading(&self) -> Option<i64> {
        self.clock.now().ok().map(|at| at.get())
    }

    pub fn store(&self) -> &Store {
        self.store.as_ref().expect("the fixture is open")
    }

    /// The directory that holds `server.db` and any private media root a test needs.
    pub fn dir(&self) -> &std::path::Path {
        self.run.dir()
    }

    /// The `server.db` path of this fixture, for a test that serves it from another process.
    pub fn database_path(&self) -> std::path::PathBuf {
        self.run.path("server.db")
    }

    /// Release the file so another process (the `cantos-server` binary) may own it. A test then
    /// reads through [`Self::restart`], which serves the file again.
    pub fn close(&mut self) {
        self.store = None;
    }

    /// Close the file and serve it again, as a restart does. Every clone of the previous store
    /// (a router's state, a spawned task) must have been dropped first, or ownership refuses.
    pub async fn restart(&mut self) {
        self.store = None;
        let database = ServerDatabase::open(
            self.run.path("server.db"),
            Intent::Serve,
            self.clock.clone(),
        )
        .await
        .expect("the file is served again");
        self.store = Some(Store::from_database(database));
    }

    /// Run fixture SQL in one committed write unit; returns the affected row count.
    pub async fn exec(&self, sql: &str, params: Params<'_>) -> Result<u64, StoreError> {
        let mut conn = self.store().connection().await?;
        let tx = conn.transaction().await?;
        match tx.execute(sql, params).await {
            Ok(changed) => {
                tx.commit().await?;
                Ok(changed)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error.into())
            }
        }
    }

    /// Like [`Self::exec`] but keeps the statement's own failure, so a barrier test can assert
    /// the exact guard code or constraint kind that refused it. A refused unit is rolled back.
    pub async fn try_exec(
        &self,
        sql: &str,
        params: Params<'_>,
    ) -> Result<u64, crate::platform::store::Error> {
        let mut conn = self
            .store()
            .connection()
            .await
            .map_err(|_| crate::platform::store::Error::Cardinality)?;
        let tx = conn.transaction().await?;
        match tx.execute(sql, params).await {
            Ok(changed) => {
                tx.commit().await?;
                Ok(changed)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }

    pub async fn rows(&self, sql: &str, params: Params<'_>) -> Result<Vec<Row>, StoreError> {
        let conn = self.store().connection().await?;
        let tx = conn.read().await?;
        let rows = tx.query(sql, params).await;
        tx.rollback().await?;
        Ok(rows?)
    }

    /// One integer, for row-count style assertions.
    pub async fn count(&self, sql: &str, params: Params<'_>) -> i64 {
        self.rows(sql, params).await.expect("the query runs")[0].get(0)
    }

    pub async fn seed_actors(&self, ids: &[&str]) {
        for id in ids {
            // A pre-checked insert: `actors` refuses an insert over an existing id even under
            // `OR IGNORE`, because that conflict clause runs after the guard.
            self.exec(
                "INSERT INTO actors(id) SELECT ?1 WHERE NOT EXISTS (SELECT 1 FROM actors WHERE id=?1)",
                &[id],
            )
            .await
            .expect("the actor is seeded");
        }
    }

    /// A development Studio session for `actor`, valid for `hours` of the fixture clock.
    pub async fn seed_development_session(&self, actor: &str, token: &str, hours: i64) {
        let expires = self.clock.now().expect("the clock reads").get() + hours * 3_600_000_000;
        let session_id = uuid::Uuid::new_v4().hyphenated().to_string();
        self.exec(
            "INSERT INTO development_sessions(token_hash,actor_id,expires_at,session_id) VALUES(?1,?2,?3,?4)",
            &[
                &crate::platform::store::token_hash(token) as &(dyn Param + Sync),
                &actor,
                &expires,
                &session_id,
            ],
        )
        .await
        .expect("the development session is seeded");
    }

    /// An actor and its development token together: the usual Studio test caller.
    pub async fn studio_caller(&self, actor: &str, token: &str) {
        self.seed_actors(&[actor]).await;
        self.seed_development_session(actor, token, 1).await;
    }
}

static DETACHED: std::sync::Mutex<Vec<TestRun>> = std::sync::Mutex::new(Vec::new());

/// A real, migrated `server.db` for synchronous tests that only need a `Store` value in router
/// state and never reach persistence. The directory lives for the rest of the process (a test
/// binary), so every call opens its own private file.
pub fn detached_store() -> Store {
    std::thread::spawn(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime opens the fixture file");
        runtime.block_on(async {
            let run = TestRun::new("detached");
            let database = ServerDatabase::open(
                run.path("server.db"),
                Intent::Migrate,
                Arc::new(SystemClock),
            )
            .await
            .expect("a fresh server.db is created");
            DETACHED.lock().expect("fixture list").push(run);
            Store::from_database(database)
        })
    })
    .join()
    .expect("the fixture thread finishes")
}
