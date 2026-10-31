//! Explicit #568/#657 fixture observations, never a runtime cache, query or authority.
use crate::{
    contexts::{
        catalog::{self, domain::WorkId},
        media,
    },
    platform::{identity::domain::ActorId, store::Store},
};
use cantos_store_turso::{params_from_iter, Value as SqlValue};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Only these fixed indexes may be changed in the caller's synthetic, marker-bound dataset.
pub const EXPERIMENTAL_INDEXES: &[(&str, &str)] = &[
    ("s657_unit_policy_work", "CREATE INDEX s657_unit_policy_work ON catalog_unit_viewer_policies(work_id,mode,unit_id,owner_id)"),
    ("s657_grant_expiry", "CREATE INDEX s657_grant_expiry ON catalog_viewer_grants(expires_at_us)"),
    ("s657_member_expiry", "CREATE INDEX s657_member_expiry ON catalog_viewer_members(expires_at_us)"),
    ("s657_unit_grant_expiry", "CREATE INDEX s657_unit_grant_expiry ON catalog_unit_viewer_grants(expires_at_us)"),
];

pub async fn experimental_indexes(store: &Store, create: bool) {
    let mut conn = store.connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    for (name, sql) in EXPERIMENTAL_INDEXES {
        let drop_sql = format!("DROP INDEX {name}");
        tx.execute(if create { sql } else { &drop_sql }, &[])
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
}

/// Measures the actual Catalog/Media read seams in one real snapshot. Excludes HTTP,
/// Identity RPC, cover/media I/O, cursor encryption and application JSON serialization.
pub async fn observe(store: &Store, case: &Value, plans: bool) -> Value {
    let actor = ActorId::new(case["actor"].as_str().unwrap()).unwrap();
    let conn = store.connection().await.unwrap();
    let started = std::time::Instant::now();
    let tx = conn.read().await.unwrap();
    tx.measure_queries();
    let normalized = catalog::domain::metadata_publication::public_search_text(
        case["search"].as_str().unwrap_or(""),
    );
    let search = normalized.as_str();
    let work = case["work"].as_str().unwrap();
    let after = case["after"]
        .as_str()
        .map(|id| (case["anchor"].as_i64().unwrap(), id));
    let result = match case["lane"].as_str().unwrap() {
        "scope" => json!(catalog::shared_scope(&tx).await.unwrap()),
        "public" => {
            let after_id = after.map(|(_, id)| WorkId::parse(id).unwrap());
            let _ = catalog::shared_scope(&tx).await.unwrap();
            let rows = catalog::public_works(
                &tx,
                catalog::PublicWorksQuery {
                    search,
                    kind: None,
                    language: None,
                    limit: 20,
                    after: after_id
                        .as_ref()
                        .map(|id| (case["anchor"].as_i64().unwrap(), id)),
                },
            )
            .await
            .unwrap();
            json!(rows.iter().map(|w| json!({"id":w.work.id().as_str(),"count":w.unit_count,"generation":w.generation.get()})).collect::<Vec<_>>())
        }
        "candidates" | "shared" => {
            let _ = catalog::shared_scope(&tx).await.unwrap();
            let rows = catalog::shared_candidates(&tx, &actor, after, 21, None, None, search)
                .await
                .unwrap();
            if case["lane"] == "candidates" {
                json!(rows)
            } else {
                let mut visible = Vec::new();
                for (id, _) in rows {
                    if let Some(w) = catalog::shared_work(&tx, &WorkId::parse(&id).unwrap(), &actor)
                        .await
                        .unwrap()
                    {
                        visible.push(json!({"id":id,"count":w.row.unit_count,"generation":w.row.generation.get()}));
                    }
                }
                json!(visible)
            }
        }
        "units" => {
            let w = catalog::shared_work(&tx, &WorkId::parse(work).unwrap(), &actor)
                .await
                .unwrap()
                .unwrap();
            let (rows, next) = catalog::shared_units(&tx, &w, 0, 20).await.unwrap();
            json!({"count":w.row.unit_count,"units":rows.iter().map(|r|json!({"id":r.unit.id().as_str(),"ordinal":r.ordinal,"position":r.position})).collect::<Vec<_>>(),"next":next})
        }
        "library-empty-audio-control" => {
            let works = vec![work.to_owned()];
            json!(
                media::library_work_presentations(&tx, &works, tx.now_micros() / 1000)
                    .await
                    .unwrap()
            )
        }
        _ => panic!("closed dataset lane"),
    };
    let samples = tx.take_measurements();
    let elapsed_ns = started.elapsed().as_nanos();
    let mut statements = Vec::new();
    for m in &samples {
        let mut plan = json!({"status":"NOT_RUN"});
        if plans {
            match tx
                .raw_query(
                    &format!("EXPLAIN QUERY PLAN {}", m.sql),
                    params_from_iter(m.params.clone()),
                )
                .await
            {
                Ok(mut rows) => {
                    let mut details = Vec::new();
                    while let Some(row) = rows.next().await.unwrap() {
                        details.push(
                            (0..4)
                                .map(|i| match row.get_value(i).unwrap() {
                                    SqlValue::Text(v) => v,
                                    v => format!("{v:?}"),
                                })
                                .collect::<Vec<_>>(),
                        );
                    }
                    plan = json!({"status":"PASS","rows":details});
                }
                Err(e) => {
                    plan = json!({"status":"UNSUPPORTED","error":format!("{e:?}")});
                }
            }
        }
        statements.push(json!({"sql":m.sql,"sql_sha256":format!("{:x}",Sha256::digest(m.sql.as_bytes())),"elapsed_ns":m.elapsed_ns,"rows":m.rows,"decoded_value_bytes":m.decoded_bytes,"plan":plan}));
    }
    tx.commit().await.unwrap();
    json!({"result":result,"elapsed_ns":elapsed_ns,"sql_ns":samples.iter().map(|m|m.elapsed_ns).sum::<u128>(),"statements":samples.len(),"rows":samples.iter().map(|m|m.rows).sum::<usize>(),"decoded_value_bytes":samples.iter().map(|m|m.decoded_bytes).sum::<usize>(),"queries":statements})
}

pub async fn synthetic_rights(store: &Store, owner: &str) {
    let mut conn = store.connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    tx.execute("INSERT INTO script_evidence(owner_id,kind,id,description) SELECT ?1,'rights','dataset-synthetic-rights','Synthetic #657 declaration; no real rights acceptance' WHERE NOT EXISTS(SELECT 1 FROM script_evidence WHERE owner_id=?1 AND kind='rights' AND id='dataset-synthetic-rights')", &[&owner]).await.unwrap();
    tx.commit().await.unwrap();
}
