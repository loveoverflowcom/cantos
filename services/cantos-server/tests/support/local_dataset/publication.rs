//! Publication/ACL extension of the existing explicit dataset lane. All mutations below use
//! production owner commands; the synthetic rights declaration is not real rights acceptance.
use super::{
    plan::{self, Plan, ACTORS},
    runtime,
};
use cantos_server::{
    contexts::media::MetadataPublicationService,
    contract::studio::{MetadataPublicationRequest, ViewerAccessRequest, ViewerGroupRequest},
    platform::store::Store,
    test_support::publication_performance as probe,
};
use serde_json::{json, Value};
use std::{path::Path, time::Instant};

fn op(p: &Plan, purpose: &str) -> String {
    p.id(&format!("publication/{purpose}"))
}

pub async fn seed(s: &Store, p: &Plan, tokens: &[String]) -> Value {
    probe::synthetic_rights(s, ACTORS[0]).await;
    let mut times = Vec::new();
    for g in 0..4 {
        let group = op(p, &format!("group/{g}"));
        for (version, change) in [
            (
                0,
                json!({"action":"create","name":format!("Synthetic group {g}")}),
            ),
            (
                1,
                json!({"action":"add_member","actor_id":ACTORS[3],"expires_at_us":plan::CLOCK+86_400_000_000_i64}),
            ),
        ] {
            let request: ViewerGroupRequest = serde_json::from_value(json!({"operation_id":op(p,&format!("group/{g}/{version}")),"expected_version":version,"change":change})).unwrap();
            s.viewer_group_execute(&tokens[0], &group, request)
                .await
                .unwrap();
        }
    }
    for (i, w) in p.works.iter().enumerate() {
        if i % 8 == 7 {
            continue;
        } // Unpublished draft control.
        let start = Instant::now();
        let request: MetadataPublicationRequest = serde_json::from_value(json!({
            "action":"publish","operation_id":op(p,&format!("publish/{i}")),"expected_version":0,
            "work_revision":w.revision,"units":w.units.iter().map(|u|json!({"unit_id":u.id,"revision":u.revision})).collect::<Vec<_>>(),
            "confirm_metadata_rights":true
        })).unwrap();
        MetadataPublicationService::new(s.clone())
            .settle(&tokens[0], &w.id, request)
            .await
            .unwrap();
        times.push(start.elapsed().as_nanos());
        let mut changes = Vec::new();
        if (1..=4).contains(&(i % 8)) {
            changes.push(json!({"action":"set_audience","audience":"restricted","confirm_metadata_rights":false}));
        }
        if matches!(i % 8, 2 | 4) {
            changes.push(json!({"action":"grant_actor","grant_id":op(p,&format!("actor/{i}")),"actor_id":ACTORS[3],"expires_at_us":null}));
        }
        if matches!(i % 8, 3 | 4) {
            changes.push(json!({"action":"grant_group","grant_id":op(p,&format!("grant-group/{i}")),"group_id":op(p,&format!("group/{}",i%4)),"expires_at_us":null}));
        }
        if let Some(u) = w.units.first().filter(|_| i == 0 || matches!(i % 8, 5 | 6)) {
            let principal = if i % 8 == 6 {
                json!({"kind":"group","group_id":op(p,&format!("group/{}",i%4))})
            } else {
                json!({"kind":"actor","actor_id":if i==0 { ACTORS[2] } else { ACTORS[3] }})
            };
            changes.push(json!({"action":"set_unit_policy","unit_id":u.id,"policy":{"mode":"restricted","grants":[{"principal":principal,"expires_at_us":null}]},"confirm_metadata_rights":false}));
        }
        for (version, change) in changes.into_iter().enumerate() {
            let request: ViewerAccessRequest = serde_json::from_value(json!({"operation_id":op(p,&format!("access/{i}/{version}")),"expected_version":version,"change":change})).unwrap();
            s.viewer_access_execute(&tokens[0], &w.id, request)
                .await
                .unwrap();
        }
    }
    json!({"publish_ns":times,"limit":"first create only; exact replay is measured separately by the owner fixture, not conflated with new writes"})
}

fn cases(p: &Plan) -> Vec<Value> {
    let work = &p.works[0].id;
    let mut cases = Vec::new();
    for lane in [
        "public",
        "candidates",
        "shared",
        "units",
        "scope",
        "library-empty-audio-control",
    ] {
        cases.push(json!({"id":lane,"lane":lane,"work":work,"actor":ACTORS[3]}));
    }
    for actor in [ACTORS[2], ACTORS[4]] {
        cases.push(json!({"id":format!("candidates-{actor}"),"lane":"candidates","work":work,"actor":actor}));
    }
    for search in ["bến", "Ánh", "%_", "HIDDEN_SENTINEL", "no-match"] {
        for lane in ["public", "shared"] {
            cases.push(json!({"id":format!("{lane}-search-{search}"),"lane":lane,"work":work,"actor":ACTORS[3],"search":search}));
        }
    }
    let mut ids: Vec<_> = p.works.iter().map(|w| w.id.as_str()).collect();
    ids.sort();
    for lane in ["public", "shared"] {
        cases.push(json!({"id":format!("{lane}-deep"),"lane":lane,"work":work,"actor":ACTORS[3],"after":ids[ids.len()*3/4],"anchor":plan::CLOCK/1000}));
    }
    cases
}
fn percentile(values: &[u128], percent: usize) -> u128 {
    assert!(!values.is_empty() && (1..=100).contains(&percent));
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[(sorted.len() * percent).div_ceil(100) - 1]
}
fn stats(values: &[u128]) -> Value {
    json!({"n":values.len(),"p50":percentile(values,50),"p95":percentile(values,95),"p99":percentile(values,99),"raw":values})
}
fn oracle(p: &Plan, case: &Value, actual: &Value) {
    let lane = case["lane"].as_str().unwrap();
    if !matches!(lane, "public" | "candidates" | "shared") {
        return;
    }
    let search = case["search"].as_str().unwrap_or("");
    let actor = case["actor"].as_str().unwrap();
    let mut expected: Vec<_> = p
        .works
        .iter()
        .enumerate()
        .filter(|(i, w)| {
            let access = if lane == "public" {
                matches!(i % 8, 0 | 5 | 6)
            } else if actor == ACTORS[3] {
                matches!(i % 8, 2..=6)
            } else {
                actor == ACTORS[2] && *i == 0
            };
            let text = match search {
                "" => true,
                "bến" => i % 4 != 0,
                "Ánh" | "%_" => i % 4 == 0,
                "HIDDEN_SENTINEL" | "no-match" => false,
                _ => panic!("closed handwritten corpus"),
            };
            access && text && case["after"].as_str().is_none_or(|id| w.id.as_str() > id)
        })
        .map(|(_, w)| w.id.as_str())
        .collect();
    expected.sort();
    expected.truncate(if lane == "public" { 20 } else { 21 });
    let observed: Vec<_> = actual
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            if lane == "candidates" {
                row[0].as_str().unwrap()
            } else {
                row["id"].as_str().unwrap()
            }
        })
        .collect();
    assert_eq!(
        observed, expected,
        "independent fixture membership/order: {}",
        case["id"]
    );
    if lane != "candidates" {
        for row in actual.as_array().unwrap() {
            let (i, w) = p
                .works
                .iter()
                .enumerate()
                .find(|(_, w)| w.id == row["id"].as_str().unwrap())
                .unwrap();
            let hidden = usize::from(
                lane == "public" && (i == 0 || matches!(i % 8, 5 | 6)) && !w.units.is_empty(),
            );
            assert_eq!(
                row["count"],
                w.units.len() - hidden,
                "independent HIDE count"
            );
        }
    }
}
async fn campaign(s: &Store, p: &Plan) -> Vec<Value> {
    let mut out = Vec::new();
    for case in cases(p) {
        let first = probe::observe(s, &case, true).await;
        oracle(p, &case, &first["result"]);
        for _ in 0..5 {
            probe::observe(s, &case, false).await;
        }
        let mut wall = Vec::new();
        let mut sql = Vec::new();
        for _ in 0..100 {
            let sample = probe::observe(s, &case, false).await;
            assert_eq!(
                sample["result"], first["result"],
                "unstable immutable workload"
            );
            assert_eq!(sample["statements"], first["statements"]);
            wall.push(sample["elapsed_ns"].as_u64().unwrap() as u128);
            sql.push(sample["sql_ns"].as_u64().unwrap() as u128);
        }
        out.push(json!({"case":case,"first":first,"warm_wall_ns":stats(&wall),"warm_sql_ns":stats(&sql)}));
    }
    out
}
fn memory() -> String {
    let out = std::process::Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}
fn storage(root: &Path) -> Value {
    json!({"db_bytes":std::fs::metadata(root.join("server.db")).unwrap().len(),"wal_bytes":std::fs::metadata(root.join("server.db-wal")).map_or(0,|m|m.len()),"per_index_bytes":"NOT_RUN: no dbstat attribution; file/WAL growth is not per-index size"})
}

pub async fn measure(root: &Path, p: &Plan) -> Value {
    let s = runtime::store(root).await;
    let baseline_rss = memory();
    let baseline = campaign(&s, p).await;
    let baseline_storage = storage(root);
    // Experiments are confined to this explicit private profile, not a registered migration.
    let start = Instant::now();
    probe::experimental_indexes(&s, true).await;
    let index_build_ns = start.elapsed().as_nanos();
    let candidate = campaign(&s, p).await;
    let candidate_storage = storage(root);
    for (a, b) in baseline.iter().zip(&candidate) {
        assert_eq!(
            a["first"]["result"], b["first"]["result"],
            "index experiment changed results"
        );
    }
    let candidate_rss = memory();
    probe::experimental_indexes(&s, false).await;
    drop(s);
    let mut reopened = Vec::new();
    for case in cases(p) {
        let mut values = Vec::new();
        for _ in 0..5 {
            let s = runtime::store(root).await;
            let sample = probe::observe(&s, &case, false).await;
            oracle(p, &case, &sample["result"]);
            let expected = baseline.iter().find(|r| r["case"] == case).unwrap();
            assert_eq!(
                sample["result"], expected["first"]["result"],
                "reopen changed projection"
            );
            values.push(sample["elapsed_ns"].as_u64().unwrap() as u128);
            drop(s);
        }
        reopened.push(json!({"case":case,"reopened_wall_ns":stats(&values)}));
    }
    json!({"baseline":baseline,"experimental_indexes":probe::EXPERIMENTAL_INDEXES.iter().map(|(_,sql)|*sql).collect::<Vec<_>>(),"candidate":candidate,"reopened":reopened,"index_build_ns":index_build_ns,"storage":{"baseline":baseline_storage,"candidate":candidate_storage},"rss_kib":{"baseline":baseline_rss,"candidate":candidate_rss},"budget":"PENDING owner acceptance (ADR0054 D08a); no runtime optimization adopted","cache":"NONE","conditions":{"samples":100,"warmup":5,"reopened_samples":5,"reopened_is_os_cold":false,"library":"plan/zero-audio negative control only; populated release-backed cost NOT_RUN","decoded_value_bytes":"sum of returned Text/Blob bytes, 8 per Integer/Real, zero Null; excludes allocation/column names/wire","query_count":"successful Tx.query statements only; excludes snapshot/registry/EXPLAIN, RPC and HTTP","sql_ns":"statement facade execute+drain+decode/measurement overhead, not engine-only CPU","rss":"whole process including held reports/caches; delta is not isolated index memory","cpu_scanned_rows_per_index_bytes":"NOT_RUN; no engine scan-counter or dbstat claim"}})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percentile_nearest_rank_includes_tail_and_single_sample() {
        assert_eq!(percentile(&[100, 1, 50, 10], 50), 10);
        assert_eq!(percentile(&[100, 1, 50, 10], 99), 100);
        assert_eq!(percentile(&[7], 95), 7);
    }
}
