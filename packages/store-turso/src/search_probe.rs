//! Opt-in engine-only localhost qualification. No business schema or public search API.
//! The caller selects this explicitly; ordinary gates never run the corpus campaign.
use crate::testing::TestRun;
use turso::{Builder, Connection, Value};

pub const VECTOR_DOCUMENTS: [(&str, &str, [f32; 3]); 5] = [
    ("a", "red", [0., 0., 0.]),
    ("b", "blue", [0., 0., 0.]),
    ("c", "red", [3., 4., 0.]),
    ("d", "blue", [-3., 0., 0.]),
    ("e", "red", [0., 0., 12.]),
];
pub const FTS_DOCUMENTS: [(&str, &str, &str); 5] = [
    ("a", "River lantern", "a quiet harbour"),
    ("b", "Harbour", "river river river"),
    ("c", "Mountain", "no matching term"),
    ("d", "", ""),
    ("e", "Punctuation", "river-bank AND + - \"quoted\""),
];
// Membership candidates for a future enabled tokenizer qualification, not runtime assertions.
pub const FTS_QUERIES: [(&str, &[&str]); 3] = [
    ("river", &["a", "b", "e"]),
    ("harbour", &["a", "b"]),
    ("absentterm", &[]),
];

#[derive(Debug)]
pub struct Report {
    pub status: &'static str,
    pub detail: String,
    pub checks: Vec<String>,
}
fn vector(v: [f32; 3]) -> String {
    format!("[{},{},{}]", v[0], v[1], v[2])
}
async fn neighbors(c: &Connection, filter: &str, k: i64) -> Vec<(String, f64)> {
    let mut rows=c.query("SELECT id, vector_distance_l2(v, vector32('[0,0,0]')) AS distance FROM vectors WHERE (?1='' OR category=?1) ORDER BY distance,id LIMIT ?2",(filter,k)).await.expect("actual vector query");
    let mut out = vec![];
    while let Some(row) = rows.next().await.expect("vector row") {
        let Value::Text(id) = row.get_value(0).unwrap() else {
            panic!("vector id")
        };
        let Value::Real(distance) = row.get_value(1).unwrap() else {
            panic!("vector distance")
        };
        out.push((id, distance));
    }
    out
}
fn assert_neighbors(actual: &[(String, f64)], expected: &[(&str, f64)]) {
    assert_eq!(actual.len(), expected.len());
    for ((id, d), (want, v)) in actual.iter().zip(expected) {
        assert_eq!(id, want);
        assert!(
            d.is_finite() && (d - v).abs() <= 1e-6,
            "distance {d} vs {v}"
        );
    }
}
async fn query_error(c: &Connection, sql: &str) -> String {
    match c.query(sql, ()).await {
        Err(e) => e.to_string(),
        Ok(mut rows) => match rows.next().await {
            Err(e) => e.to_string(),
            Ok(_) => panic!("negative vector query was accepted: {sql}"),
        },
    }
}
/// Exact full-scan L2, not ANN/index/recall or a product integration claim.
pub async fn vectors() -> Report {
    let run = TestRun::new("dataset-vector-probe");
    let db = Builder::new_local(&run.path("vectors.db").to_string_lossy())
        .build()
        .await
        .unwrap();
    let c = db.connect().unwrap();
    c.execute(
        "CREATE TABLE vectors(id TEXT PRIMARY KEY, category TEXT NOT NULL, v BLOB NOT NULL)",
        (),
    )
    .await
    .unwrap();
    assert!(neighbors(&c, "", 10).await.is_empty());
    for (id, category, v) in VECTOR_DOCUMENTS {
        c.execute(
            "INSERT INTO vectors VALUES(?1,?2,vector32(?3))",
            (id, category, vector(v)),
        )
        .await
        .unwrap();
    }
    let oracle = [("a", 0.), ("b", 0.), ("d", 3.), ("c", 5.), ("e", 12.)];
    for k in [1, 2, 3, 5, 20] {
        assert_neighbors(&neighbors(&c, "", k).await, &oracle[..(k as usize).min(5)]);
    }
    assert_neighbors(
        &neighbors(&c, "red", 20).await,
        &[("a", 0.), ("c", 5.), ("e", 12.)],
    );
    assert!(neighbors(&c, "absent", 20).await.is_empty());
    let error = query_error(
        &c,
        "SELECT vector_distance_l2(vector32('[0,0]'), vector32('[0,0,0]'))",
    )
    .await;
    assert!(error.contains("same dimensions"), "{error}");
    for value in ["NaN", "Infinity", "1e1000"] {
        let error = query_error(&c, &format!("SELECT vector32('[{value},0,0]')")).await;
        assert!(!error.is_empty());
    }
    assert_neighbors(&neighbors(&c, "", 20).await, &oracle);
    c.execute("UPDATE vectors SET v=vector32('[0,0,1]') WHERE id='c'", ())
        .await
        .unwrap();
    c.execute("DELETE FROM vectors WHERE id='a'", ())
        .await
        .unwrap();
    c.execute(
        "INSERT INTO vectors VALUES('z','blue',vector32('[0,0,0]'))",
        (),
    )
    .await
    .unwrap();
    let changed = [("b", 0.), ("z", 0.), ("c", 1.), ("d", 3.), ("e", 12.)];
    assert_neighbors(&neighbors(&c, "", 20).await, &changed);
    drop(c);
    drop(db);
    let db = Builder::new_local(&run.path("vectors.db").to_string_lossy())
        .build()
        .await
        .unwrap();
    let c = db.connect().unwrap();
    assert_neighbors(&neighbors(&c, "", 20).await, &changed);
    c.execute("DELETE FROM vectors", ()).await.unwrap();
    assert!(neighbors(&c, "", 20).await.is_empty());
    drop(c);
    drop(db);
    Report{status:"PASS",detail:"turso 0.8.3, default-features=false; 3D vector32, Euclidean L2 full scan, tolerance 1e-6, explicit id tie-break; ANN/index and product integration NOT_RUN".into(),checks:vec!["empty/top-k/ties/filter/no-hit".into(),"dimension/nonfinite rejection preserves corpus".into(),"insert/update/delete/reopen".into()]}
}
/// Probe the actual module lookup with experimental index methods enabled only in this
/// disposable engine. FTS is intentionally absent from the production dependency features.
pub async fn fts() -> Report {
    let run = TestRun::new("dataset-fts-probe");
    let db = Builder::new_local(&run.path("fts.db").to_string_lossy())
        .experimental_index_method(true)
        .build()
        .await
        .unwrap();
    let c = db.connect().unwrap();
    c.execute(
        "CREATE TABLE documents(id TEXT PRIMARY KEY,title TEXT,body TEXT)",
        (),
    )
    .await
    .unwrap();
    for (id, title, body) in FTS_DOCUMENTS {
        c.execute("INSERT INTO documents VALUES(?1,?2,?3)", (id, title, body))
            .await
            .unwrap();
    }
    let error = c
        .execute(
            "CREATE INDEX corpus_fts ON documents USING fts (title, body)",
            (),
        )
        .await
        .expect_err("feature change requires separate FTS qualification")
        .to_string();
    assert!(
        error.contains("unknown module name 'fts'"),
        "unexpected FTS failure, not a capability absence: {error}"
    );
    drop(c);
    drop(db);
    Report{status:"UNSUPPORTED",detail:"turso 0.8.3 default-features=false excludes fts/Tantivy; experimental_index_method(true) reached unknown module name 'fts'. No dependency/default changed. Enablement review then real membership/phrase/rank/update/delete qualification required.".into(),checks:vec!["real disposable DB and native CREATE INDEX ... USING fts probe".into()]}
}
