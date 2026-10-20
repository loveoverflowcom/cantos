//! Capability probes: what the pinned Turso engine actually does with the constructs the
//! PostgreSQL schema relies on.
//!
//! Each probe runs against the real engine in a run-owned file and reports `Pass` (the needed
//! behavior holds), `Unsupported` (the construct is absent, with the chosen substitute) or
//! `Fail` (the engine silently weakens an invariant). The expected outcome of every probe is
//! recorded next to it, so an engine upgrade that changes behavior fails a test instead of
//! a production invariant. A report is evidence for one engine pin, one OS and one filesystem.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, Instant};

use turso::{Builder, Connection, Value};

use crate::error::{classify_constraint, ConstraintKind, GuardCode, StoreFault};

/// Kept equal to the exact dependency pin by a unit test over `Cargo.toml`.
pub const ENGINE_PIN: &str = "turso =0.8.3 (default-features = false)";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Unsupported,
    Fail,
}

impl Outcome {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Unsupported => "UNSUPPORTED",
            Self::Fail => "FAIL",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Probe {
    pub id: &'static str,
    pub group: &'static str,
    /// The PostgreSQL construct or invariant this answers.
    pub requirement: &'static str,
    pub expected: Outcome,
    /// What the migration uses where the engine lacks the construct.
    pub substitute: Option<&'static str>,
    pub outcome: Outcome,
    pub detail: String,
}

impl Probe {
    pub fn as_expected(&self) -> bool {
        self.outcome == self.expected
    }
}

#[derive(Clone, Debug)]
pub struct Report {
    pub engine: &'static str,
    pub sqlite_compat_version: String,
    pub os: &'static str,
    pub arch: &'static str,
    pub filesystem: String,
    pub probes: Vec<Probe>,
}

impl Report {
    pub fn unexpected(&self) -> Vec<&Probe> {
        self.probes
            .iter()
            .filter(|probe| !probe.as_expected())
            .collect()
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "engine: {}\nsqlite compatibility version: {}\nos/arch: {}/{}\nfilesystem: {}\n\n",
            self.engine, self.sqlite_compat_version, self.os, self.arch, self.filesystem
        );
        let mut group = "";
        for probe in &self.probes {
            if probe.group != group {
                group = probe.group;
                out.push_str(&format!("[{group}]\n"));
            }
            let flag = if probe.as_expected() {
                ""
            } else {
                "  <-- UNEXPECTED"
            };
            out.push_str(&format!(
                "  {:<11} {}{flag}\n      needs: {}\n      observed: {}\n",
                probe.outcome.label(),
                probe.id,
                probe.requirement,
                probe.detail
            ));
            if let Some(substitute) = probe.substitute {
                out.push_str(&format!("      substitute: {substitute}\n"));
            }
        }
        let count = |outcome| self.probes.iter().filter(|p| p.outcome == outcome).count();
        out.push_str(&format!(
            "\ntotal {}: {} PASS, {} UNSUPPORTED, {} FAIL; {} differ from the recorded expectation\n",
            self.probes.len(),
            count(Outcome::Pass),
            count(Outcome::Unsupported),
            count(Outcome::Fail),
            self.unexpected().len()
        ));
        out
    }
}

enum Verdict {
    Pass(String),
    Unsupported(String),
    Fail(String),
}

use Verdict::{Fail, Pass, Unsupported};

type ProbeResult = Result<Verdict, turso::Error>;
type Runner = for<'a> fn(&'a Env) -> Pin<Box<dyn Future<Output = ProbeResult> + 'a>>;

struct Case {
    id: &'static str,
    group: &'static str,
    requirement: &'static str,
    expected: Outcome,
    substitute: Option<&'static str>,
    run: Runner,
}

macro_rules! case {
    ($group:literal, $id:literal, $requirement:literal, $expected:ident, $substitute:expr, $probe:ident) => {
        Case {
            id: $id,
            group: $group,
            requirement: $requirement,
            expected: Outcome::$expected,
            substitute: $substitute,
            run: |env| Box::pin($probe(env)),
        }
    };
}

/// Where probes create their files, and the executable used for process-level probes.
pub struct Env {
    dir: PathBuf,
}

struct Db {
    database: turso::Database,
    conn: Connection,
}

impl Db {
    fn second(&self) -> Result<Connection, turso::Error> {
        self.database.connect()
    }
}

impl Env {
    async fn db(&self, name: &str) -> Result<Db, turso::Error> {
        let path = self.dir.join(format!("{name}.db"));
        let database = Builder::new_local(&path.to_string_lossy()).build().await?;
        let conn = database.connect()?;
        conn.busy_timeout(Duration::from_millis(250))?;
        conn.execute("PRAGMA foreign_keys = ON", ()).await?;
        Ok(Db { database, conn })
    }
}

async fn run(conn: &Connection, sql: &str) -> Result<u64, turso::Error> {
    conn.execute(sql, ()).await
}

/// The error a statement fails with, or `None` when it succeeds.
async fn failure(conn: &Connection, sql: &str) -> Option<turso::Error> {
    conn.execute(sql, ()).await.err()
}

async fn number(conn: &Connection, sql: &str) -> Result<i64, turso::Error> {
    let mut rows = conn.query(sql, ()).await?;
    let value = match rows.next().await? {
        Some(row) => match row.get_value(0)? {
            Value::Integer(value) => value,
            other => return Err(turso::Error::ConversionFailure(format!("{other:?}"))),
        },
        None => return Err(turso::Error::QueryReturnedNoRows),
    };
    while rows.next().await?.is_some() {}
    Ok(value)
}

async fn words(conn: &Connection, sql: &str) -> Result<Vec<String>, turso::Error> {
    let mut rows = conn.query(sql, ()).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(match row.get_value(0)? {
            Value::Text(text) => text,
            Value::Integer(value) => value.to_string(),
            Value::Null => "NULL".into(),
            other => format!("{other:?}"),
        });
    }
    Ok(out)
}

fn constraint(error: &Option<turso::Error>) -> Option<ConstraintKind> {
    match error {
        Some(turso::Error::Constraint(message)) => match classify_constraint(message) {
            StoreFault::Constraint { kind, .. } => Some(kind),
            _ => None,
        },
        _ => None,
    }
}

// ---- constraints ---------------------------------------------------------------------------

async fn check_octet_length(env: &Env) -> ProbeResult {
    let db = env.db("check_octet_length").await?;
    run(
        &db.conn,
        "CREATE TABLE t(a TEXT NOT NULL CHECK (octet_length(a) <= 4)) STRICT",
    )
    .await?;
    run(&db.conn, "INSERT INTO t VALUES ('abcd')").await?;
    let multibyte = failure(&db.conn, "INSERT INTO t VALUES ('éé€')").await;
    Ok(if constraint(&multibyte) == Some(ConstraintKind::Check) {
        Pass("counts bytes: 4 ASCII bytes accepted, 3 characters / 7 bytes rejected".into())
    } else {
        Fail(format!(
            "multibyte overflow not rejected as CHECK: {multibyte:?}"
        ))
    })
}

async fn check_regexp(env: &Env) -> ProbeResult {
    let db = env.db("check_regexp").await?;
    run(
        &db.conn,
        "CREATE TABLE t(a TEXT NOT NULL CHECK (coalesce(a REGEXP '^[a-z0-9]{3,8}$', 0))) STRICT",
    )
    .await?;
    run(&db.conn, "INSERT INTO t VALUES ('abc123')").await?;
    let rejected = [
        failure(&db.conn, "INSERT INTO t VALUES ('AB')").await,
        failure(&db.conn, "INSERT INTO t VALUES ('abcdefghi')").await,
        failure(&db.conn, "INSERT INTO t VALUES ('abc def')").await,
    ];
    Ok(
        if rejected
            .iter()
            .all(|r| constraint(r) == Some(ConstraintKind::Check))
        {
            Pass("anchors, classes and {m,n} are enforced by REGEXP in CHECK".into())
        } else {
            Fail(format!("REGEXP CHECK let a bad value in: {rejected:?}"))
        },
    )
}

async fn check_regexp_invalid_pattern_is_open(env: &Env) -> ProbeResult {
    let db = env.db("check_regexp_invalid").await?;
    run(
        &db.conn,
        "CREATE TABLE plain(a TEXT NOT NULL CHECK (a REGEXP '(')) STRICT",
    )
    .await?;
    let accepted = failure(&db.conn, "INSERT INTO plain VALUES ('anything')")
        .await
        .is_none();
    // FAIL is the engine silently weakening an invariant. PASS here would mean the hazard is gone.
    Ok(if accepted {
        Fail(
            "a CHECK whose pattern does not compile evaluates to NULL and accepts every row".into(),
        )
    } else {
        Pass("the invalid pattern was refused at DDL or insert; the NULL-passes hazard no longer exists".into())
    })
}

async fn check_regexp_coalesce_fails_closed(env: &Env) -> ProbeResult {
    let db = env.db("check_regexp_closed").await?;
    run(
        &db.conn,
        "CREATE TABLE closed(a TEXT NOT NULL CHECK (coalesce(a REGEXP '(', 0))) STRICT",
    )
    .await?;
    let rejected = constraint(&failure(&db.conn, "INSERT INTO closed VALUES ('anything')").await);
    Ok(if rejected == Some(ConstraintKind::Check) {
        Pass("coalesce(a REGEXP 'p', 0) rejects every row when the pattern is broken, so a typo cannot disable the constraint unnoticed".into())
    } else {
        Fail(format!("{rejected:?}"))
    })
}

async fn check_glob(env: &Env) -> ProbeResult {
    let db = env.db("check_glob").await?;
    run(&db.conn, "CREATE TABLE t(a TEXT NOT NULL CHECK (a GLOB '[a-z]*' AND a NOT GLOB '*[^a-z0-9_]*')) STRICT").await?;
    run(&db.conn, "INSERT INTO t VALUES ('abc_1')").await?;
    let bad = [
        failure(&db.conn, "INSERT INTO t VALUES ('1abc')").await,
        failure(&db.conn, "INSERT INTO t VALUES ('abC')").await,
    ];
    Ok(
        if bad
            .iter()
            .all(|r| constraint(r) == Some(ConstraintKind::Check))
        {
            Pass("GLOB with negated classes enforces ASCII identifier shapes".into())
        } else {
            Fail(format!("GLOB CHECK accepted a bad value: {bad:?}"))
        },
    )
}

async fn check_nonnegative_integer(env: &Env) -> ProbeResult {
    let db = env.db("check_nonneg").await?;
    run(
        &db.conn,
        "CREATE TABLE t(n INTEGER NOT NULL CHECK (typeof(n) = 'integer' AND n >= 0)) STRICT",
    )
    .await?;
    run(&db.conn, "INSERT INTO t VALUES (0)").await?;
    let negative = failure(&db.conn, "INSERT INTO t VALUES (-1)").await;
    Ok(if constraint(&negative) == Some(ConstraintKind::Check) {
        Pass("non-negative integer domain enforced".into())
    } else {
        Fail(format!("negative accepted: {negative:?}"))
    })
}

async fn foreign_keys_per_connection(env: &Env) -> ProbeResult {
    let path = env.dir.join("fk_default.db");
    let database = Builder::new_local(&path.to_string_lossy()).build().await?;
    let conn = database.connect()?;
    run(&conn, "CREATE TABLE p(id INTEGER PRIMARY KEY) STRICT").await?;
    run(
        &conn,
        "CREATE TABLE c(id INTEGER PRIMARY KEY, p INTEGER NOT NULL REFERENCES p(id)) STRICT",
    )
    .await?;
    let default_off = number(&conn, "PRAGMA foreign_keys").await? == 0;
    let silently_accepted = failure(&conn, "INSERT INTO c VALUES (1, 99)")
        .await
        .is_none();
    run(&conn, "PRAGMA foreign_keys = ON").await?;
    let on_conn = database.connect()?;
    let other_default_off = number(&on_conn, "PRAGMA foreign_keys").await? == 0;
    let enforced = constraint(&failure(&conn, "INSERT INTO c VALUES (2, 98)").await)
        == Some(ConstraintKind::ForeignKey);
    Ok(
        if default_off && silently_accepted && other_default_off && enforced {
            Pass("OFF by default and per connection: a dangling row is accepted until each connection enables it; OwnedDatabase enables and verifies it".into())
        } else {
            Fail(format!("default_off={default_off} accepted={silently_accepted} per_connection={other_default_off} enforced={enforced}"))
        },
    )
}

async fn foreign_key_actions(env: &Env) -> ProbeResult {
    let db = env.db("fk_actions").await?;
    let c = &db.conn;
    run(c, "CREATE TABLE p(id INTEGER PRIMARY KEY) STRICT").await?;
    run(c, "CREATE TABLE restrict_c(id INTEGER PRIMARY KEY, p INTEGER NOT NULL REFERENCES p(id) ON DELETE RESTRICT) STRICT").await?;
    run(c, "CREATE TABLE cascade_c(id INTEGER PRIMARY KEY, p INTEGER NOT NULL REFERENCES p(id) ON DELETE CASCADE) STRICT").await?;
    run(c, "CREATE TABLE null_c(id INTEGER PRIMARY KEY, p INTEGER REFERENCES p(id) ON DELETE SET NULL) STRICT").await?;
    run(c, "INSERT INTO p VALUES (1), (2), (3)").await?;
    run(c, "INSERT INTO restrict_c VALUES (1, 1)").await?;
    run(c, "INSERT INTO cascade_c VALUES (1, 2)").await?;
    run(c, "INSERT INTO null_c VALUES (1, 3)").await?;
    let blocked = constraint(&failure(c, "DELETE FROM p WHERE id = 1").await)
        == Some(ConstraintKind::ForeignKey);
    run(c, "DELETE FROM p WHERE id = 2").await?;
    run(c, "DELETE FROM p WHERE id = 3").await?;
    let cascaded = number(c, "SELECT count(*) FROM cascade_c").await? == 0;
    let nulled = number(c, "SELECT count(*) FROM null_c WHERE p IS NULL").await? == 1;
    Ok(if blocked && cascaded && nulled {
        Pass("RESTRICT blocks, CASCADE deletes children, SET NULL clears the reference".into())
    } else {
        Fail(format!(
            "restrict={blocked} cascade={cascaded} set_null={nulled}"
        ))
    })
}

async fn foreign_key_deferred(env: &Env) -> ProbeResult {
    let db = env.db("fk_deferred").await?;
    let c = &db.conn;
    run(c, "CREATE TABLE head(id INTEGER PRIMARY KEY, rev INTEGER REFERENCES rev(id) DEFERRABLE INITIALLY DEFERRED) STRICT").await?;
    run(c, "CREATE TABLE rev(id INTEGER PRIMARY KEY, head INTEGER NOT NULL REFERENCES head(id) DEFERRABLE INITIALLY DEFERRED) STRICT").await?;
    run(c, "BEGIN IMMEDIATE").await?;
    run(c, "INSERT INTO head VALUES (1, 1)").await?;
    run(c, "INSERT INTO rev VALUES (1, 1)").await?;
    run(c, "COMMIT").await?;
    run(c, "BEGIN IMMEDIATE").await?;
    run(c, "INSERT INTO head VALUES (2, 77)").await?;
    let commit = failure(c, "COMMIT").await;
    let still_open = !c.is_autocommit()?;
    run(c, "ROLLBACK").await?;
    let left_over = number(c, "SELECT count(*) FROM head").await?;
    Ok(
        if constraint(&commit) == Some(ConstraintKind::ForeignKey) && still_open && left_over == 1 {
            Pass("the head<->revision cycle commits; a dangling reference fails at COMMIT as a constraint, the transaction stays open and must be rolled back (Rejected, not Unknown)".into())
        } else {
            Fail(format!(
                "commit={commit:?} open={still_open} rows={left_over}"
            ))
        },
    )
}

async fn unique_deferrable(env: &Env) -> ProbeResult {
    let db = env.db("unique_deferrable").await?;
    let result = failure(&db.conn, "CREATE TABLE u(a INTEGER, pos INTEGER, UNIQUE (a, pos) DEFERRABLE INITIALLY DEFERRED) STRICT").await;
    Ok(match result {
        Some(error) => Unsupported(format!("refused at DDL: {}", brief(&error))),
        None => Pass("accepted".into()),
    })
}

async fn unique_reorder_two_phase(env: &Env) -> ProbeResult {
    let db = env.db("unique_reorder").await?;
    let c = &db.conn;
    run(c, "CREATE TABLE u(work INTEGER NOT NULL, pos INTEGER NOT NULL, label TEXT NOT NULL, UNIQUE (work, pos)) STRICT").await?;
    run(
        c,
        "INSERT INTO u VALUES (1, 1, 'a'), (1, 2, 'b'), (1, 3, 'c')",
    )
    .await?;
    run(c, "BEGIN IMMEDIATE").await?;
    let single = failure(
        c,
        "UPDATE u SET pos = CASE pos WHEN 1 THEN 3 WHEN 3 THEN 1 ELSE pos END WHERE work = 1",
    )
    .await;
    run(c, "ROLLBACK").await?;
    run(c, "BEGIN IMMEDIATE").await?;
    run(c, "UPDATE u SET pos = pos + 1000000 WHERE work = 1").await?;
    run(c, "UPDATE u SET pos = CASE pos - 1000000 WHEN 1 THEN 3 WHEN 3 THEN 1 ELSE pos - 1000000 END WHERE work = 1").await?;
    run(c, "COMMIT").await?;
    let order = words(c, "SELECT label FROM u ORDER BY pos").await?;
    Ok(
        if constraint(&single) == Some(ConstraintKind::Unique) && order == ["c", "b", "a"] {
            Pass("a single-statement swap collides row by row; shifting out of range then renumbering inside one transaction reorders atomically".into())
        } else {
            Fail(format!("single={single:?} order={order:?}"))
        },
    )
}

async fn partial_unique_index(env: &Env) -> ProbeResult {
    let db = env.db("partial_index").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(id INTEGER PRIMARY KEY, k TEXT NOT NULL, active INTEGER NOT NULL) STRICT",
    )
    .await?;
    run(c, "CREATE UNIQUE INDEX t_active ON t(k) WHERE active = 1").await?;
    run(
        c,
        "INSERT INTO t VALUES (1, 'a', 1), (2, 'a', 0), (3, 'a', 0)",
    )
    .await?;
    let duplicate = failure(c, "INSERT INTO t VALUES (4, 'a', 1)").await;
    Ok(if constraint(&duplicate) == Some(ConstraintKind::Unique) {
        Pass("a partial UNIQUE index constrains only the matching rows".into())
    } else {
        Fail(format!("{duplicate:?}"))
    })
}

async fn collation_unknown_is_loud(env: &Env) -> ProbeResult {
    let db = env.db("collation_unknown").await?;
    let c = &db.conn;
    let query = c
        .query("SELECT 'a' = 'A' COLLATE NO_SUCH_COLLATION", ())
        .await;
    let query_refused = match query {
        Err(_) => true,
        Ok(mut rows) => rows.next().await.is_err(),
    };
    let ddl = failure(c, "CREATE TABLE t(a TEXT COLLATE NO_SUCH_COLLATION) STRICT").await;
    let index = {
        run(c, "CREATE TABLE ok(a TEXT) STRICT").await?;
        failure(c, "CREATE INDEX ok_a ON ok(a COLLATE NO_SUCH_COLLATION)").await
    };
    Ok(if query_refused && ddl.is_some() && index.is_some() {
        Pass("an unknown collation is refused in queries, column definitions and indexes (no silent fallback to BINARY)".into())
    } else {
        Fail(format!(
            "query_refused={query_refused} ddl={ddl:?} index={index:?}"
        ))
    })
}

async fn collation_binary_is_byte_order(env: &Env) -> ProbeResult {
    let db = env.db("collation_binary").await?;
    let c = &db.conn;
    run(c, "CREATE TABLE t(a TEXT NOT NULL COLLATE BINARY) STRICT").await?;
    run(c, "INSERT INTO t VALUES ('Z'), ('a'), ('Đ'), ('é'), ('B')").await?;
    let order = words(c, "SELECT a FROM t ORDER BY a").await?;
    Ok(if order == ["B", "Z", "a", "é", "Đ"] {
        Pass("BINARY orders by UTF-8 byte value, as PostgreSQL COLLATE \"C\" does".into())
    } else {
        Fail(format!("{order:?}"))
    })
}

// ---- triggers ------------------------------------------------------------------------------

async fn trigger_guard_update_delete(env: &Env) -> ProbeResult {
    let db = env.db("trigger_guard").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE settled(id INTEGER PRIMARY KEY, v TEXT NOT NULL) STRICT",
    )
    .await?;
    run(c, "CREATE TRIGGER settled_u BEFORE UPDATE ON settled BEGIN SELECT RAISE(ABORT, 'cantos:immutable.settled'); END").await?;
    run(c, "CREATE TRIGGER settled_d BEFORE DELETE ON settled BEGIN SELECT RAISE(ABORT, 'cantos:immutable.settled'); END").await?;
    run(c, "INSERT INTO settled VALUES (1, 'a')").await?;
    let update = failure(c, "UPDATE settled SET v = 'b'")
        .await
        .map(StoreFault::from);
    let delete = failure(c, "DELETE FROM settled")
        .await
        .map(StoreFault::from);
    let guard = Some(StoreFault::Guard(
        GuardCode::new("immutable.settled").expect("valid code"),
    ));
    let intact = words(c, "SELECT v FROM settled").await? == ["a"];
    Ok(if update == guard && delete == guard && intact {
        Pass("row-level BEFORE UPDATE/DELETE with RAISE(ABORT) replaces reject_settled_change; the message arrives as a typed Guard".into())
    } else {
        Fail(format!(
            "update={update:?} delete={delete:?} intact={intact}"
        ))
    })
}

async fn trigger_when_and_case(env: &Env) -> ProbeResult {
    let db = env.db("trigger_when_case").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT NOT NULL, note TEXT) STRICT",
    )
    .await?;
    run(c, "CREATE TRIGGER t_v BEFORE UPDATE ON t BEGIN SELECT CASE WHEN OLD.v <> NEW.v THEN RAISE(ABORT, 'cantos:immutable.t_v') END; END").await?;
    run(c, "CREATE TRIGGER t_d BEFORE DELETE ON t WHEN OLD.note IS NULL BEGIN SELECT RAISE(ABORT, 'cantos:keep.t'); END").await?;
    run(
        c,
        "INSERT INTO t VALUES (1, 'a', NULL), (2, 'b', 'x'), (3, 'c', NULL)",
    )
    .await?;
    let same = failure(c, "UPDATE t SET note = 'y' WHERE id = 1").await;
    let changed = failure(c, "UPDATE t SET v = 'z' WHERE id = 1").await;
    let blocked = failure(c, "DELETE FROM t WHERE id = 3").await;
    let allowed = failure(c, "DELETE FROM t WHERE id = 2").await;
    Ok(
        if same.is_none() && changed.is_some() && blocked.is_some() && allowed.is_none() {
            Pass("WHEN clauses and CASE-wrapped RAISE express column-level immutability and conditional guards".into())
        } else {
            Fail(format!(
                "same={same:?} changed={changed:?} blocked={blocked:?} allowed={allowed:?}"
            ))
        },
    )
}

async fn trigger_abort_is_statement_atomic(env: &Env) -> ProbeResult {
    let db = env.db("trigger_atomic").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(id INTEGER PRIMARY KEY, n INTEGER NOT NULL) STRICT",
    )
    .await?;
    run(c, "CREATE TRIGGER t_u BEFORE UPDATE ON t WHEN NEW.n = 3 BEGIN SELECT RAISE(ABORT, 'cantos:stop'); END").await?;
    run(c, "INSERT INTO t VALUES (1, 0), (2, 0), (3, 0)").await?;
    let error = failure(c, "UPDATE t SET n = id").await;
    let untouched = number(c, "SELECT count(*) FROM t WHERE n = 0").await?;
    Ok(if error.is_some() && untouched == 3 {
        Pass("a guard aborting the third row leaves none of the three rows changed".into())
    } else {
        Fail(format!("error={error:?} unchanged_rows={untouched}"))
    })
}

async fn trigger_for_each_statement(env: &Env) -> ProbeResult {
    let db = env.db("trigger_statement").await?;
    run(&db.conn, "CREATE TABLE t(a INTEGER) STRICT").await?;
    let result = failure(
        &db.conn,
        "CREATE TRIGGER s AFTER INSERT ON t FOR EACH STATEMENT BEGIN SELECT 1; END",
    )
    .await;
    Ok(match result {
        Some(error) => Unsupported(format!("refused at DDL: {}", brief(&error))),
        None => Pass("accepted".into()),
    })
}

async fn trigger_recursive(env: &Env) -> ProbeResult {
    let db = env.db("trigger_recursive").await?;
    let value = words(&db.conn, "PRAGMA recursive_triggers").await?;
    Ok(if value.is_empty() {
        Unsupported(
            "PRAGMA recursive_triggers returns nothing; a trigger must not rely on re-entry".into(),
        )
    } else {
        Pass(format!("recursive_triggers = {value:?}"))
    })
}

// ---- SQL surface ---------------------------------------------------------------------------

async fn sql_upsert_and_ignore(env: &Env) -> ProbeResult {
    let db = env.db("sql_upsert").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(k TEXT PRIMARY KEY, n INTEGER NOT NULL) STRICT",
    )
    .await?;
    run(
        c,
        "INSERT INTO t VALUES ('a', 1) ON CONFLICT (k) DO UPDATE SET n = n + 1",
    )
    .await?;
    run(
        c,
        "INSERT INTO t VALUES ('a', 1) ON CONFLICT (k) DO UPDATE SET n = n + 1",
    )
    .await?;
    let ignored = run(
        c,
        "INSERT INTO t VALUES ('a', 9) ON CONFLICT (k) DO NOTHING",
    )
    .await?;
    let fresh = run(
        c,
        "INSERT INTO t VALUES ('b', 9) ON CONFLICT (k) DO NOTHING",
    )
    .await?;
    let n = number(c, "SELECT n FROM t WHERE k = 'a'").await?;
    Ok(if n == 2 && ignored == 0 && fresh == 1 {
        Pass("ON CONFLICT DO UPDATE / DO NOTHING; DO NOTHING reports 0 affected rows, so a replayed receipt is detectable".into())
    } else {
        Fail(format!("n={n} ignored={ignored} fresh={fresh}"))
    })
}

async fn sql_returning_and_busy_cursor(env: &Env) -> ProbeResult {
    let db = env.db("sql_returning").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT NOT NULL) STRICT",
    )
    .await?;
    let mut rows = c
        .query("INSERT INTO t VALUES (1, 'a') RETURNING id", ())
        .await?;
    let first = rows.next().await?.is_some();
    let second_write = failure(c, "INSERT INTO t VALUES (2, 'b')")
        .await
        .map(StoreFault::from);
    while rows.next().await?.is_some() {}
    drop(rows);
    let after = failure(c, "INSERT INTO t VALUES (2, 'b')").await;
    Ok(
        if first && second_write == Some(StoreFault::Busy) && after.is_none() {
            Pass("RETURNING works; a second write on the same connection while the cursor is open is Busy until it is drained".into())
        } else {
            Fail(format!(
                "first={first} during={second_write:?} after={after:?}"
            ))
        },
    )
}

async fn sql_json(env: &Env) -> ProbeResult {
    let db = env.db("sql_json").await?;
    let c = &db.conn;
    let extracted = number(c, "SELECT json_extract('{\"a\":[1,2,3]}', '$.a[1]')").await?;
    let each = words(c, "SELECT value FROM json_each('[10,20,30]')").await?;
    let object = words(c, "SELECT json_object('k', 'v')").await?;
    Ok(
        if extracted == 2 && each == ["10", "20", "30"] && object == ["{\"k\":\"v\"}"] {
            Pass("json_extract, json_each and json_object cover jsonb_* / unnest uses".into())
        } else {
            Fail(format!("{extracted} {each:?} {object:?}"))
        },
    )
}

async fn sql_window_functions(env: &Env) -> ProbeResult {
    let db = env.db("sql_window").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(g TEXT NOT NULL, v INTEGER NOT NULL) STRICT",
    )
    .await?;
    run(c, "INSERT INTO t VALUES ('x', 3), ('x', 1), ('y', 2)").await?;
    let totals = words(
        c,
        "SELECT count(*) OVER (PARTITION BY g) FROM t ORDER BY g, v",
    )
    .await?;
    let ranks = words(
        c,
        "SELECT row_number() OVER (PARTITION BY g ORDER BY v) FROM t ORDER BY g, v",
    )
    .await?;
    Ok(if totals == ["2", "2", "1"] && ranks == ["1", "2", "1"] {
        Pass(
            "count(*) OVER (PARTITION BY) and row_number() for listener discovery pagination"
                .into(),
        )
    } else {
        Fail(format!("{totals:?} {ranks:?}"))
    })
}

async fn sql_recursive_cte(env: &Env) -> ProbeResult {
    let db = env.db("sql_cte").await?;
    let rows = words(&db.conn, "WITH RECURSIVE r(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM r WHERE n < 4) SELECT n FROM r").await;
    Ok(match rows {
        Ok(rows) if rows == ["1", "2", "3", "4"] => {
            Pass("recursive CTEs run on this pin (the planning notes assumed they did not)".into())
        }
        Ok(rows) => Fail(format!("{rows:?}")),
        Err(error) => Unsupported(brief(&error)),
    })
}

// ---- types ---------------------------------------------------------------------------------

async fn type_strict(env: &Env) -> ProbeResult {
    let db = env.db("type_strict").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(i INTEGER, t TEXT, b BLOB, r REAL) STRICT",
    )
    .await?;
    let text_in_integer = failure(c, "INSERT INTO t(i) VALUES ('abc')").await;
    let blob_in_text = failure(c, "INSERT INTO t(t) VALUES (x'00ff')").await;
    let text_in_blob = failure(c, "INSERT INTO t(b) VALUES ('text')").await;
    Ok(
        if text_in_integer.is_some() && blob_in_text.is_some() && text_in_blob.is_some() {
            Pass("STRICT refuses TEXT in INTEGER, BLOB in TEXT and TEXT in BLOB".into())
        } else {
            Fail(format!(
                "{text_in_integer:?} {blob_in_text:?} {text_in_blob:?}"
            ))
        },
    )
}

async fn type_u64_boundary(env: &Env) -> ProbeResult {
    let db = env.db("type_u64").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(n INTEGER NOT NULL CHECK (n >= 0)) STRICT",
    )
    .await?;
    c.execute("INSERT INTO t VALUES (?1)", [i64::MAX]).await?;
    let stored = number(c, "SELECT n FROM t").await?;
    let literal = failure(c, "INSERT INTO t VALUES (9223372036854775808)").await;
    let wrapped = c.execute("INSERT INTO t VALUES (?1)", [-1_i64]).await.err();
    Ok(
        if stored == i64::MAX && literal.is_some() && wrapped.is_some() {
            Pass("i64::MAX round-trips; a literal above it becomes REAL and is refused; negatives fail the CHECK: u64 above i64::MAX must be rejected by the typed constructor, never wrapped".into())
        } else {
            Fail(format!(
                "stored={stored} literal={literal:?} negative={wrapped:?}"
            ))
        },
    )
}

async fn type_identifier_columns(env: &Env) -> ProbeResult {
    let db = env.db("type_ids").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE b(id BLOB PRIMARY KEY CHECK (length(id) = 16)) STRICT",
    )
    .await?;
    run(c, "CREATE TABLE t(id TEXT PRIMARY KEY CHECK (length(id) = 36 AND id GLOB '[0-9a-f]*' AND id NOT GLOB '*[^0-9a-f-]*')) STRICT").await?;
    c.execute("INSERT INTO b VALUES (?1)", [Value::Blob(vec![7; 16])])
        .await?;
    let short_blob = c
        .execute("INSERT INTO b VALUES (?1)", [Value::Blob(vec![7; 15])])
        .await
        .err();
    run(
        c,
        "INSERT INTO t VALUES ('0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b')",
    )
    .await?;
    let upper = failure(
        c,
        "INSERT INTO t VALUES ('0190A1B2-C3D4-7E5F-8A9B-0C1D2E3F4A5B')",
    )
    .await;
    Ok(if short_blob.is_some() && upper.is_some() {
        Pass("both BLOB(16) and canonical lowercase TEXT UUID columns can be length- and shape-checked; the representation is chosen in the server schema".into())
    } else {
        Fail(format!("short_blob={short_blob:?} upper={upper:?}"))
    })
}

async fn type_timestamp_micros(env: &Env) -> ProbeResult {
    let db = env.db("type_time").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(at INTEGER NOT NULL CHECK (at >= 0)) STRICT",
    )
    .await?;
    for micros in [1_700_000_000_123_456_i64, 1_700_000_000_123_457, i64::MAX] {
        c.execute("INSERT INTO t VALUES (?1)", [micros]).await?;
    }
    let ordered = words(c, "SELECT at FROM t ORDER BY at").await?;
    let exact = ordered
        == [
            "1700000000123456",
            "1700000000123457",
            i64::MAX.to_string().as_str(),
        ];
    Ok(if exact {
        Pass("microsecond INTEGER timestamps keep exact value and order up to i64::MAX".into())
    } else {
        Fail(format!("{ordered:?}"))
    })
}

async fn type_null_in_unique(env: &Env) -> ProbeResult {
    let db = env.db("type_null").await?;
    let c = &db.conn;
    run(c, "CREATE TABLE t(a TEXT, UNIQUE (a)) STRICT").await?;
    run(c, "INSERT INTO t VALUES (NULL), (NULL), ('x')").await?;
    let duplicate = failure(c, "INSERT INTO t VALUES ('x')").await;
    let nulls = number(c, "SELECT count(*) FROM t WHERE a IS NULL").await?;
    Ok(
        if nulls == 2 && constraint(&duplicate) == Some(ConstraintKind::Unique) {
            Pass("NULLs are distinct in UNIQUE (as in PostgreSQL); `IS` compares NULLs".into())
        } else {
            Fail(format!("nulls={nulls} duplicate={duplicate:?}"))
        },
    )
}

async fn type_text_is_not_normalized(env: &Env) -> ProbeResult {
    let db = env.db("type_nfc").await?;
    let c = &db.conn;
    run(c, "CREATE TABLE t(a TEXT NOT NULL UNIQUE) STRICT").await?;
    let nfc = "Việt Nam";
    let nfd: String = "Vie\u{323}\u{302}t Nam".into();
    c.execute("INSERT INTO t VALUES (?1)", [nfc]).await?;
    c.execute("INSERT INTO t VALUES (?1)", [nfd.as_str()])
        .await?;
    let distinct = number(c, "SELECT count(*) FROM t").await?;
    let bytes = number(c, "SELECT sum(length(CAST(a AS BLOB))) FROM t").await?;
    Ok(
        if distinct == 2 && bytes == (nfc.len() + nfd.len()) as i64 {
            Pass("NFC and NFD spellings are stored byte for byte as different values: normalization stays in the domain constructor".into())
        } else {
            Fail(format!("distinct={distinct} bytes={bytes}"))
        },
    )
}

// ---- transactions --------------------------------------------------------------------------

async fn tx_second_writer_is_busy(env: &Env) -> ProbeResult {
    let db = env.db("tx_busy").await?;
    run(&db.conn, "CREATE TABLE t(a INTEGER) STRICT").await?;
    let other = db.second()?;
    other.busy_timeout(Duration::from_millis(300))?;
    run(&db.conn, "BEGIN IMMEDIATE").await?;
    let started = Instant::now();
    let second = failure(&other, "BEGIN IMMEDIATE")
        .await
        .map(StoreFault::from);
    let waited = started.elapsed();
    run(&db.conn, "ROLLBACK").await?;
    let free = failure(&other, "BEGIN IMMEDIATE").await;
    run(&other, "ROLLBACK").await.ok();
    Ok(
        if second == Some(StoreFault::Busy)
            && waited >= Duration::from_millis(250)
            && free.is_none()
        {
            Pass(format!("a second BEGIN IMMEDIATE waited {waited:?} (busy_timeout 300ms) then reported Busy"))
        } else {
            Fail(format!("second={second:?} waited={waited:?} free={free:?}"))
        },
    )
}

async fn tx_deferred_upgrade_is_stale(env: &Env) -> ProbeResult {
    let db = env.db("tx_stale").await?;
    run(&db.conn, "CREATE TABLE t(a INTEGER) STRICT").await?;
    let other = db.second()?;
    run(&db.conn, "BEGIN").await?;
    number(&db.conn, "SELECT count(*) FROM t").await?;
    run(&other, "INSERT INTO t VALUES (1)").await?;
    let upgrade = failure(&db.conn, "INSERT INTO t VALUES (2)")
        .await
        .map(StoreFault::from);
    run(&db.conn, "ROLLBACK").await.ok();
    Ok(if upgrade == Some(StoreFault::StaleSnapshot) {
        Pass("a read transaction upgraded to a write after another commit fails as StaleSnapshot: every mutation starts BEGIN IMMEDIATE".into())
    } else {
        Fail(format!("{upgrade:?}"))
    })
}

async fn tx_reader_snapshot(env: &Env) -> ProbeResult {
    let db = env.db("tx_reader").await?;
    run(&db.conn, "CREATE TABLE t(a INTEGER) STRICT").await?;
    let reader = db.second()?;
    run(&db.conn, "BEGIN IMMEDIATE").await?;
    run(&db.conn, "INSERT INTO t VALUES (1)").await?;
    let during = number(&reader, "SELECT count(*) FROM t").await?;
    run(&db.conn, "COMMIT").await?;
    let after = number(&reader, "SELECT count(*) FROM t").await?;
    Ok(if during == 0 && after == 1 {
        Pass("a reader never sees an uncommitted write and sees it once committed".into())
    } else {
        Fail(format!("during={during} after={after}"))
    })
}

async fn tx_ddl_is_transactional(env: &Env) -> ProbeResult {
    let db = env.db("tx_ddl").await?;
    let c = &db.conn;
    run(c, "BEGIN IMMEDIATE").await?;
    run(c, "PRAGMA application_id = 1129270355").await?;
    run(c, "PRAGMA user_version = 3").await?;
    c.execute_batch("CREATE TABLE a(x INTEGER) STRICT; CREATE TABLE b(x INTEGER) STRICT;")
        .await?;
    run(c, "ROLLBACK").await?;
    let objects = number(c, "SELECT count(*) FROM sqlite_schema").await?;
    let app = number(c, "PRAGMA application_id").await?;
    let version = number(c, "PRAGMA user_version").await?;
    Ok(if objects == 0 && app == 0 && version == 0 {
        Pass("DDL, application_id and user_version roll back together: file creation can be one atomic transaction".into())
    } else {
        Fail(format!(
            "objects={objects} application_id={app} user_version={version}"
        ))
    })
}

async fn tx_savepoints(env: &Env) -> ProbeResult {
    let db = env.db("tx_savepoint").await?;
    let c = &db.conn;
    run(c, "CREATE TABLE t(a INTEGER) STRICT").await?;
    run(c, "BEGIN IMMEDIATE").await?;
    run(c, "INSERT INTO t VALUES (1)").await?;
    run(c, "SAVEPOINT s").await?;
    run(c, "INSERT INTO t VALUES (2)").await?;
    run(c, "ROLLBACK TO s").await?;
    run(c, "RELEASE s").await?;
    run(c, "COMMIT").await?;
    let n = number(c, "SELECT count(*) FROM t").await?;
    Ok(if n == 1 {
        Pass("SAVEPOINT / ROLLBACK TO / RELEASE".into())
    } else {
        Fail(format!("{n}"))
    })
}

async fn tx_failed_statement_keeps_transaction(env: &Env) -> ProbeResult {
    let db = env.db("tx_keep").await?;
    let c = &db.conn;
    run(c, "CREATE TABLE t(a INTEGER PRIMARY KEY) STRICT").await?;
    run(c, "BEGIN IMMEDIATE").await?;
    run(c, "INSERT INTO t VALUES (1)").await?;
    let duplicate = failure(c, "INSERT INTO t VALUES (1)").await;
    let open = !c.is_autocommit()?;
    run(c, "COMMIT").await?;
    let n = number(c, "SELECT count(*) FROM t").await?;
    Ok(if duplicate.is_some() && open && n == 1 {
        Pass("a rejected statement does not end the transaction: the earlier change commits unless the adapter rolls back, so adapters must roll back on any rejected step".into())
    } else {
        Fail(format!("duplicate={duplicate:?} open={open} rows={n}"))
    })
}

async fn tx_mvcc_not_enabled(env: &Env) -> ProbeResult {
    let db = env.db("tx_mvcc").await?;
    let mode = words(&db.conn, "PRAGMA journal_mode").await?;
    run(&db.conn, "CREATE TABLE t(a INTEGER) STRICT").await?;
    let concurrent = failure(&db.conn, "BEGIN CONCURRENT").await;
    run(&db.conn, "ROLLBACK").await.ok();
    Ok(if mode == ["wal"] && concurrent.is_some() {
        Pass(
            "the default journal is wal and BEGIN CONCURRENT is refused: MVCC stays off in v1"
                .into(),
        )
    } else {
        Fail(format!("mode={mode:?} concurrent={concurrent:?}"))
    })
}

// ---- durability and backup -----------------------------------------------------------------

async fn durability_defaults(env: &Env) -> ProbeResult {
    let db = env.db("durability_defaults").await?;
    let synchronous = number(&db.conn, "PRAGMA synchronous").await?;
    let mode = words(&db.conn, "PRAGMA journal_mode").await?;
    Ok(if synchronous == 2 && mode == ["wal"] {
        Pass("synchronous = FULL and journal_mode = wal by default; OwnedDatabase refuses to open otherwise".into())
    } else {
        Fail(format!("synchronous={synchronous} journal={mode:?}"))
    })
}

async fn durability_reopen_and_integrity(env: &Env) -> ProbeResult {
    let path = env.dir.join("durability_reopen.db");
    let text = path.to_string_lossy().into_owned();
    {
        let database = Builder::new_local(&text).build().await?;
        let c = database.connect()?;
        run(
            &c,
            "CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT NOT NULL) STRICT",
        )
        .await?;
        for n in 0..200 {
            c.execute("INSERT INTO t VALUES (?1, ?2)", (n, "row"))
                .await?;
        }
    }
    let database = Builder::new_local(&text).build().await?;
    let c = database.connect()?;
    let rows = number(&c, "SELECT count(*) FROM t").await?;
    let check = words(&c, "PRAGMA integrity_check").await?;
    Ok(if rows == 200 && check == ["ok"] {
        Pass("200 committed rows survive close and reopen; integrity_check is ok".into())
    } else {
        Fail(format!("rows={rows} integrity={check:?}"))
    })
}

async fn durability_checkpoint_truncate(env: &Env) -> ProbeResult {
    let db = env.db("durability_checkpoint").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT NOT NULL) STRICT",
    )
    .await?;
    for n in 0..100 {
        c.execute("INSERT INTO t VALUES (?1, ?2)", (n, "x".repeat(200)))
            .await?;
    }
    let wal = env.dir.join("durability_checkpoint.db-wal");
    let before = std::fs::metadata(&wal).map_or(0, |m| m.len());
    let checkpoint = words(c, "PRAGMA wal_checkpoint(TRUNCATE)").await;
    let after = std::fs::metadata(&wal).map_or(0, |m| m.len());
    Ok(match checkpoint {
        Ok(_) if after < before || before == 0 => Pass(format!(
            "wal_checkpoint(TRUNCATE) shrank the log from {before} to {after} bytes"
        )),
        Ok(_) => Fail(format!("log did not shrink: {before} -> {after}")),
        Err(error) => Unsupported(brief(&error)),
    })
}

async fn backup_needs_no_experimental_flag(env: &Env) -> ProbeResult {
    let db = env.db("backup_flag").await?;
    let target = env.dir.join("backup_flag_copy.db");
    run(&db.conn, "CREATE TABLE t(a INTEGER) STRICT").await?;
    run(&db.conn, "INSERT INTO t VALUES (1)").await?;
    let result = failure(&db.conn, &format!("VACUUM INTO '{}'", target.display())).await;
    let copy = Builder::new_local(&target.to_string_lossy())
        .read_only(true)
        .build()
        .await;
    let rows = match copy {
        Ok(copy) => number(&copy.connect()?, "SELECT count(*) FROM t")
            .await
            .unwrap_or(-1),
        Err(_) => -1,
    };
    Ok(match (&result, rows) {
        (None, 1) => Pass("VACUUM INTO produces a readable copy with no experimental feature enabled; in-place VACUUM stays off".into()),
        (Some(error), _) => Fail(format!("refused: {}", brief(error))),
        (None, rows) => Fail(format!("reported success but the copy holds {rows} rows")),
    })
}

/// Rows in `t` that the copy lacks, after `rounds` rounds where `a` commits and `b` copies.
async fn copy_lag(env: &Env, name: &str, refresh: bool) -> Result<Vec<i64>, turso::Error> {
    let db = env.db(name).await?;
    let (a, b) = (&db.conn, db.second()?);
    run(a, "CREATE TABLE t(x INTEGER PRIMARY KEY) STRICT").await?;
    // `b` learns the schema first, so only the missing newest commits are measured here.
    number(&b, "SELECT count(*) FROM t").await?;
    let mut lags = Vec::new();
    let mut next = 0_i64;
    for round in 0..5 {
        for _ in 0..3 {
            a.execute("INSERT INTO t VALUES (?1)", [next]).await?;
            next += 1;
        }
        if refresh {
            number(&b, "SELECT count(*) FROM t").await?;
        }
        let target = env.dir.join(format!("{name}_{round}.db"));
        b.execute(&format!("VACUUM INTO '{}'", target.display()), ())
            .await?;
        let copy = Builder::new_local(&target.to_string_lossy())
            .read_only(true)
            .build()
            .await?;
        lags.push(next - number(&copy.connect()?, "SELECT count(*) FROM t").await?);
    }
    Ok(lags)
}

async fn backup_is_stale_for_another_connections_commits(env: &Env) -> ProbeResult {
    let lags = copy_lag(env, "backup_stale", false).await?;
    Ok(if lags.iter().any(|lag| *lag > 0) {
        Fail(format!("VACUUM INTO on a connection that did not make the newest commits omitted them (rows missing per round: {lags:?})"))
    } else {
        Pass("a copy made by another connection already contained its peer's newest commits".into())
    })
}

async fn backup_with_stale_schema_leaves_a_partial_file(env: &Env) -> ProbeResult {
    let db = env.db("backup_schema").await?;
    let b = db.second()?;
    run(&db.conn, "CREATE TABLE t(x INTEGER PRIMARY KEY) STRICT").await?;
    run(&db.conn, "INSERT INTO t VALUES (1)").await?;
    let target = env.dir.join("backup_schema_copy.db");
    let result = failure(&b, &format!("VACUUM INTO '{}'", target.display())).await;
    let left_over = target.exists();
    Ok(match (&result, left_over) {
        (Some(error), true) => Fail(format!("a connection that predates the schema change failed with `{}` and left the output file behind", brief(error))),
        (Some(error), false) => Fail(format!("failed with `{}` (no file left)", brief(error))),
        (None, _) => Pass("a connection that predates a schema change copied without error".into()),
    })
}

async fn backup_after_refresh_is_current(env: &Env) -> ProbeResult {
    let lags = copy_lag(env, "backup_refresh", true).await?;
    Ok(if lags.iter().all(|lag| *lag == 0) {
        Pass("a read on the copying connection just before VACUUM INTO makes the copy include every committed row".into())
    } else {
        Fail(format!(
            "copy still lagged after a refreshing read: {lags:?}"
        ))
    })
}

async fn backup_copy_is_consistent_under_writes(env: &Env) -> ProbeResult {
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
    use std::sync::Arc;

    let db = env.db("backup_load").await?;
    let c = &db.conn;
    run(
        c,
        "CREATE TABLE t(a INTEGER PRIMARY KEY, b INTEGER NOT NULL) STRICT",
    )
    .await?;
    run(
        c,
        "CREATE TABLE twin(a INTEGER PRIMARY KEY, b INTEGER NOT NULL) STRICT",
    )
    .await?;
    // Another connection commits (t, twin) pairs continuously; a copy must never split a pair.
    let writer = db.second()?;
    writer.busy_timeout(Duration::from_secs(5))?;
    let stop = Arc::new(AtomicBool::new(false));
    let committed = Arc::new(AtomicI64::new(0));
    let (flag, count) = (stop.clone(), committed.clone());
    let load = tokio::spawn(async move {
        let mut n = 1_i64;
        while !flag.load(Ordering::Relaxed) {
            if writer.execute("BEGIN IMMEDIATE", ()).await.is_err() {
                tokio::time::sleep(Duration::from_millis(1)).await;
                continue;
            }
            let wrote = writer
                .execute("INSERT INTO t VALUES (?1, ?1)", [n])
                .await
                .is_ok()
                && writer
                    .execute("INSERT INTO twin VALUES (?1, ?1)", [n])
                    .await
                    .is_ok();
            if wrote && writer.execute("COMMIT", ()).await.is_ok() {
                count.store(n, Ordering::SeqCst);
                n += 1;
            } else {
                let _ = writer.execute("ROLLBACK", ()).await;
            }
            tokio::task::yield_now().await;
        }
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while committed.load(Ordering::SeqCst) < 25 && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // Commits acknowledged before the refreshing read must all be in the copy (see backup_stale).
    let before = committed.load(Ordering::SeqCst);
    number(c, "SELECT count(*) FROM t").await?;
    let target = env.dir.join("backup_load_copy.db");
    let copy = failure(c, &format!("VACUUM INTO '{}'", target.display())).await;
    stop.store(true, Ordering::Relaxed);
    let _ = load.await;
    let total = committed.load(Ordering::SeqCst);
    if let Some(error) = copy {
        return Ok(Fail(format!("VACUUM INTO under write load: {error:?}")));
    }
    let restored = Builder::new_local(&target.to_string_lossy())
        .read_only(true)
        .build()
        .await?;
    let r = restored.connect()?;
    let integrity = words(&r, "PRAGMA integrity_check").await?;
    let t = number(&r, "SELECT count(*) FROM t").await?;
    let twin = number(&r, "SELECT count(*) FROM twin").await?;
    Ok(if before < 25 {
        Fail(format!("the load never reached 25 commits before the copy ({before}); the probe would be vacuous"))
    } else if integrity == ["ok"] && t == twin && t >= before {
        Pass(format!("{before} pairs were committed before the copy and {total} by the end of the load; the copy holds {t} t rows and {twin} twin rows, integrity ok"))
    } else {
        Fail(format!(
            "integrity={integrity:?} t={t} twin={twin} committed_before={before}"
        ))
    })
}

fn brief(error: &turso::Error) -> String {
    let text = error.to_string();
    text.chars().take(120).collect()
}

fn cases() -> Vec<Case> {
    vec![
        case!("constraints", "check.octet_length", "CHECK on octet_length(text) (~130 uses)", Pass, None, check_octet_length),
        case!("constraints", "check.regexp", "CHECK with regular expressions (~100 `~`/`!~` uses)", Pass, None, check_regexp),
        case!("constraints", "check.regexp_invalid_pattern", "a broken pattern must not disable a CHECK", Fail, Some("write every regexp CHECK as coalesce(col REGEXP 'p', 0)"), check_regexp_invalid_pattern_is_open),
        case!("constraints", "check.regexp_coalesce_closed", "fail-closed regexp CHECK form", Pass, None, check_regexp_coalesce_fails_closed),
        case!("constraints", "check.glob", "CHECK with identifier-shape patterns", Pass, None, check_glob),
        case!("constraints", "check.nonnegative_integer", "CHECK on u64-range integers", Pass, None, check_nonnegative_integer),
        case!("constraints", "fk.enabled_per_connection", "foreign keys enforced for every connection", Pass, None, foreign_keys_per_connection),
        case!("constraints", "fk.actions", "ON DELETE RESTRICT / CASCADE / SET NULL", Pass, None, foreign_key_actions),
        case!("constraints", "fk.deferred", "FK DEFERRABLE INITIALLY DEFERRED (head <-> revision)", Pass, None, foreign_key_deferred),
        case!("constraints", "unique.deferrable", "UNIQUE catalog_units(work_id, position) DEFERRABLE", Unsupported, Some("two-phase reorder inside one transaction (probe unique.reorder_two_phase)"), unique_deferrable),
        case!("constraints", "unique.reorder_two_phase", "atomic reorder of a UNIQUE position", Pass, None, unique_reorder_two_phase),
        case!("constraints", "index.partial_unique", "5 partial indexes", Pass, None, partial_unique_index),
        case!("constraints", "collation.unknown_is_loud", "no silent fallback for an unknown collation", Pass, None, collation_unknown_is_loud),
        case!("constraints", "collation.binary_is_byte_order", "COLLATE \"C\" ordering", Pass, None, collation_binary_is_byte_order),
        case!("triggers", "trigger.guard_update_delete", "106 reject_settled_change triggers", Pass, None, trigger_guard_update_delete),
        case!("triggers", "trigger.when_and_case", "guards with WHEN / column conditions", Pass, None, trigger_when_and_case),
        case!("triggers", "trigger.statement_atomic", "a rejected multi-row statement changes nothing", Pass, None, trigger_abort_is_statement_atomic),
        case!("triggers", "trigger.for_each_statement", "57 FOR EACH STATEMENT triggers", Unsupported, Some("row-level triggers, or a guard checked inside the writing transaction"), trigger_for_each_statement),
        case!("triggers", "trigger.recursive", "trigger re-entry", Unsupported, Some("no trigger may rely on recursion"), trigger_recursive),
        case!("sql", "sql.upsert_and_ignore", "ON CONFLICT upsert and idempotent receipts", Pass, None, sql_upsert_and_ignore),
        case!("sql", "sql.returning_and_open_cursor", "INSERT ... RETURNING", Pass, None, sql_returning_and_busy_cursor),
        case!("sql", "sql.json", "jsonb_* / unnest replacements", Pass, None, sql_json),
        case!("sql", "sql.window_functions", "listener discovery windows", Pass, None, sql_window_functions),
        case!("sql", "sql.recursive_cte", "WITH RECURSIVE", Pass, None, sql_recursive_cte),
        case!("types", "type.strict", "typed columns", Pass, None, type_strict),
        case!("types", "type.u64_boundary", "bigint / u64 above i64::MAX", Pass, None, type_u64_boundary),
        case!("types", "type.identifier_columns", "uuid columns", Pass, None, type_identifier_columns),
        case!("types", "type.timestamp_micros", "timestamptz", Pass, None, type_timestamp_micros),
        case!("types", "type.null_in_unique", "NULL semantics in UNIQUE", Pass, None, type_null_in_unique),
        case!("types", "type.text_not_normalized", "Vietnamese NFC identity", Pass, None, type_text_is_not_normalized),
        case!("transactions", "tx.second_writer_busy", "single serialized writer", Pass, None, tx_second_writer_is_busy),
        case!("transactions", "tx.deferred_upgrade_stale", "why every mutation is BEGIN IMMEDIATE", Pass, None, tx_deferred_upgrade_is_stale),
        case!("transactions", "tx.reader_snapshot", "readers do not see uncommitted writes", Pass, None, tx_reader_snapshot),
        case!("transactions", "tx.ddl_transactional", "atomic file creation and migration", Pass, None, tx_ddl_is_transactional),
        case!("transactions", "tx.savepoints", "partial rollback", Pass, None, tx_savepoints),
        case!("transactions", "tx.failed_statement_keeps_transaction", "adapter must roll back on a rejected step", Pass, None, tx_failed_statement_keeps_transaction),
        case!("transactions", "tx.mvcc_not_enabled", "no MVCC / BEGIN CONCURRENT in v1", Pass, None, tx_mvcc_not_enabled),
        case!("durability", "durability.defaults", "FULL synchronous write-ahead journal", Pass, None, durability_defaults),
        case!("durability", "durability.reopen_and_integrity", "committed data survives reopen", Pass, None, durability_reopen_and_integrity),
        case!("durability", "durability.checkpoint_truncate", "log checkpoint", Pass, None, durability_checkpoint_truncate),
        case!("durability", "backup.no_experimental_flag", "online backup path without experimental features", Pass, None, backup_needs_no_experimental_flag),
        case!("durability", "backup.stale_for_another_connection", "VACUUM INTO must include every committed row", Fail, Some("copy on the writer connection after a refreshing read, then compare a row-count manifest"), backup_is_stale_for_another_connections_commits),
        case!("durability", "backup.stale_schema_partial_file", "a failed VACUUM INTO must not leave a half-written copy", Fail, Some("refresh with a read first; remove a destination this process created when the copy fails"), backup_with_stale_schema_leaves_a_partial_file),
        case!("durability", "backup.after_refresh_read", "refreshing read before VACUUM INTO", Pass, None, backup_after_refresh_is_current),
        case!("durability", "backup.consistent_under_writes", "VACUUM INTO while another connection writes", Pass, None, backup_copy_is_consistent_under_writes),
    ]
}

/// Run every probe in `dir`, which must already exist and be owned by the caller.
pub async fn run_all(dir: &Path) -> Report {
    let env = Env {
        dir: dir.to_path_buf(),
    };
    let mut probes = Vec::new();
    for case in cases() {
        let (outcome, detail) = match (case.run)(&env).await {
            Ok(Pass(detail)) => (Outcome::Pass, detail),
            Ok(Unsupported(detail)) => (Outcome::Unsupported, detail),
            Ok(Fail(detail)) => (Outcome::Fail, detail),
            Err(error) => (
                Outcome::Fail,
                format!("unexpected engine error: {}", brief(&error)),
            ),
        };
        probes.push(Probe {
            id: case.id,
            group: case.group,
            requirement: case.requirement,
            expected: case.expected,
            substitute: case.substitute,
            outcome,
            detail,
        });
    }
    let sqlite_compat_version = match env.db("report_version").await {
        Ok(db) => words(&db.conn, "SELECT sqlite_version()")
            .await
            .ok()
            .and_then(|mut v| v.pop())
            .unwrap_or_else(|| "unknown".into()),
        Err(_) => "unknown".into(),
    };
    Report {
        engine: ENGINE_PIN,
        sqlite_compat_version,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        filesystem: filesystem_of(dir),
        probes,
    }
}

/// The filesystem type under `dir` from `/proc/self/mountinfo` (Linux); other hosts say so.
fn filesystem_of(dir: &Path) -> String {
    let Ok(real) = std::fs::canonicalize(dir) else {
        return "not detected".into();
    };
    let Ok(info) = std::fs::read_to_string("/proc/self/mountinfo") else {
        return "not detected on this OS".into();
    };
    let mut best: Option<(usize, String)> = None;
    for line in info.lines() {
        let mut fields = line.split(' ');
        let mount_point = fields.nth(4).unwrap_or("");
        let rest: Vec<&str> = line.split(" - ").collect();
        let fstype = rest.get(1).and_then(|r| r.split(' ').next()).unwrap_or("?");
        if real.starts_with(mount_point)
            && best
                .as_ref()
                .is_none_or(|(len, _)| mount_point.len() >= *len)
        {
            best = Some((mount_point.len(), fstype.to_owned()));
        }
    }
    best.map_or_else(|| "not detected".into(), |(_, fstype)| fstype)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_report_pin_matches_the_manifest() {
        let manifest = include_str!("../Cargo.toml");
        assert!(
            manifest.contains(r#"turso = { version = "=0.8.3", default-features = false }"#),
            "ENGINE_PIN names a version that Cargo.toml does not pin"
        );
        assert!(super::ENGINE_PIN.contains("=0.8.3"));
    }
}
