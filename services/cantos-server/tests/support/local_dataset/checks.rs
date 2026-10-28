//! Independent observed-state and negative-request oracles. Expected IDs/revisions are fixture
//! intent; no query under test manufactures its own expected result.
use super::plan::{Plan, Work, ACTORS};
use crate::http::HttpHost;
use axum::http::StatusCode;
use serde_json::{json, Value};

pub fn sql(p: &Plan, observed: &Value) {
    let mut works:Vec<_>=p.works.iter().map(|w|json!({"id":w.id,"owner":ACTORS[w.actor],"kind":w.kind,"lifecycle":if w.archived{"archived"}else{"draft"},"revision":w.revision})).collect();
    works.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    assert_eq!(observed["works"], json!(works));
    let mut units:Vec<_>=p.works.iter().flat_map(|w|w.units.iter().map(move |u|json!({"id":u.id,"work_id":w.id,"position":u.position,"lifecycle":if u.archived{"archived"}else{"draft"},"revision":u.revision}))).collect();
    units.sort_by(|a, b| {
        (
            a["work_id"].as_str(),
            a["position"].as_u64(),
            a["id"].as_str(),
        )
            .cmp(&(
                b["work_id"].as_str(),
                b["position"].as_u64(),
                b["id"].as_str(),
            ))
    });
    assert_eq!(observed["units"], json!(units));
    assert_eq!(
        observed["receipts"].as_array().unwrap().len(),
        p.steps.len()
    );
    let expected_ids: std::collections::BTreeSet<_> =
        p.steps.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        observed["receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        expected_ids
    );
    assert_eq!(
        observed["work_history"].as_array().unwrap().len(),
        p.works.iter().map(|w| w.revision as usize).sum::<usize>()
    );
    assert_eq!(
        observed["unit_history"].as_array().unwrap().len(),
        p.works
            .iter()
            .flat_map(|w| &w.units)
            .map(|u| u.revision as usize)
            .sum::<usize>()
    );
    assert_eq!(
        observed["counts"][0],
        json!({"clock":p.steps.len(),"releases":0,"images":0,"sources":0})
    );
}
async fn get(host: &HttpHost, path: &str, token: Option<&str>) -> Value {
    host.json::<Value>("GET", path, token, None, None)
        .await
        .success(StatusCode::OK)
}
async fn walk(host: &HttpHost, path: &str, token: &str, max: usize) -> Vec<Value> {
    let mut all = vec![];
    let mut cursor = None;
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..=max {
        let url = cursor
            .as_ref()
            .map_or_else(|| path.to_owned(), |c| format!("{path}&cursor={c}"));
        let page = get(host, &url, Some(token)).await;
        assert!(page.get("total").is_none());
        all.extend(page["items"].as_array().unwrap().iter().cloned());
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if let Some(c) = &cursor {
            assert!(seen.insert(c.clone()), "non-advancing cursor");
        } else {
            return all;
        }
    }
    panic!("bounded pagination did not finish");
}
fn work(actual: &Value, w: &Work) {
    assert_eq!(actual["id"], w.id);
    assert_eq!(actual["owner_id"], ACTORS[w.actor]);
    assert_eq!(actual["kind"], w.kind);
    assert_eq!(actual["revision"], w.revision);
    assert_eq!(
        actual["lifecycle"],
        if w.archived { "archived" } else { "draft" }
    );
    assert_eq!(
        actual["details"],
        json!({"title":w.title,"synopsis":w.synopsis,"language":w.language,"visibility":w.visibility,"rights_refs":[]})
    );
}
pub async fn http(host: &HttpHost, p: &Plan, tokens: &[String]) {
    let limits: &[usize] = if p.works.len() > 101 {
        &[50]
    } else {
        &[1, 20, 50]
    };
    assert_eq!(tokens.len(), ACTORS.len());
    for (actor, token) in tokens.iter().enumerate() {
        for archived in [false, true] {
            let mut expected: Vec<_> = p
                .works
                .iter()
                .filter(|w| {
                    (w.actor == actor
                        || (p.profile == "permissions-conflicts"
                            && w.actor == 0
                            && matches!(actor, 2 | 3)))
                        && (archived || !w.archived)
                })
                .collect();
            expected.sort_by(|a, b| a.id.cmp(&b.id));
            for limit in limits {
                let actual = walk(
                    host,
                    &format!("/api/v1/studio/works?limit={limit}&include_archived={archived}"),
                    token,
                    expected.len() + 1,
                )
                .await;
                assert_eq!(actual.len(), expected.len());
                for (a, w) in actual.iter().zip(&expected) {
                    work(a, w);
                }
            }
        }
    }
    for (i, w) in p.works.iter().enumerate() {
        // Scale profiles still compare every row via bounded lists and native SQL; only detail
        // probes are sampled. They are workload inputs, not exhaustive performance campaigns.
        if p.profile == "scale-local" && i >= 10 {
            continue;
        }
        let token = &tokens[w.actor];
        work(
            &get(host, &format!("/api/v1/studio/works/{}", w.id), Some(token)).await,
            w,
        );
        for archived in [false, true] {
            let mut expected: Vec<_> = w.units.iter().filter(|u| archived || !u.archived).collect();
            expected.sort_by_key(|u| u.position);
            let limits: &[usize] = if w.units.len() > 101 {
                &[50]
            } else {
                &[1, 20, 50]
            };
            for limit in limits {
                let actual=walk(host,&format!("/api/v1/studio/works/{}/content-units?limit={limit}&include_archived={archived}",w.id),token,expected.len()+1).await;
                assert_eq!(actual.len(), expected.len());
                for (a, u) in actual.iter().zip(&expected) {
                    assert_eq!(a["id"], u.id);
                    assert_eq!(a["position"], u.position);
                    assert_eq!(a["revision"], u.revision);
                    assert_eq!(a["details"]["title"], u.title);
                    assert_eq!(a["details"]["description"], u.description);
                    assert!(
                        a["details"]["source"].is_null()
                            && a["details"]["script"].is_null()
                            && a["details"]["content_digest"].is_null()
                    );
                    assert_eq!(
                        a["lifecycle"],
                        if u.archived { "archived" } else { "draft" }
                    );
                }
            }
        }
        let history = walk(
            host,
            &format!("/api/v1/studio/works/{}/history?limit=50", w.id),
            token,
            w.revision as usize + 1,
        )
        .await;
        assert_eq!(
            history
                .iter()
                .map(|v| v["revision"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            (1..=w.revision).collect::<Vec<_>>()
        );
        work(history.last().unwrap(), w);
        host.json::<Value>(
            "GET",
            &format!("/api/v1/studio/works/{}", w.id),
            Some(&tokens[4]),
            None,
            None,
        )
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
        host.json::<Value>(
            "GET",
            &format!("/api/v1/theatre/works/{}", w.id),
            None,
            None,
            None,
        )
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    }
    for path in [
        "/api/v1/theatre/works?limit=1",
        "/api/v1/theatre/search?q=%C3%A1nh&limit=20",
        "/api/v1/theatre/works?presentation=1&limit=50",
    ] {
        let page = get(host, path, None).await;
        assert_eq!(page["items"], json!([]));
        assert!(page["next_cursor"].is_null());
    }
    for path in [
        "/api/v1/studio/works?limit=0",
        "/api/v1/studio/works?limit=51",
        "/api/v1/studio/works?cursor=garbage",
    ] {
        host.json::<Value>("GET", path, Some(&tokens[0]), None, None)
            .await
            .error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    host.json::<Value>("GET", "/api/v1/studio/works", None, None, None)
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    let page = get(host, "/api/v1/studio/works?limit=1", Some(&tokens[0])).await;
    if let Some(cursor) = page["next_cursor"].as_str() {
        for (token, path) in [
            (
                &tokens[1],
                format!("/api/v1/studio/works?limit=1&cursor={cursor}"),
            ),
            (
                &tokens[0],
                format!("/api/v1/studio/works?limit=1&include_archived=true&cursor={cursor}"),
            ),
        ] {
            host.json::<Value>("GET", &path, Some(token), None, None)
                .await
                .error(StatusCode::BAD_REQUEST, "invalid_request");
        }
    }
    negatives(host, p, tokens).await;
}
pub fn negative_requests(p: &Plan) -> Vec<Value> {
    let mut cases = Vec::new();
    let Some(w) = p.works.iter().find(|w| !w.archived) else {
        return cases;
    };
    let path = format!("/api/v1/studio/works/{}", w.id);
    let mut add = |name: &str,
                   actor: usize,
                   method: &str,
                   path: &str,
                   body: Value,
                   status: u16,
                   code: &str| {
        cases.push(json!({"id":name,"actor":actor,"method":method,"path":path,"body":body,"expected_status":status,"expected_code":code}));
    };
    for (name, details) in [
        ("title-bytes", json!({"title":"é".repeat(257)})),
        ("description-bytes", json!({"synopsis":"é".repeat(8193)})),
        ("null-required", json!({"title":null})),
        ("blank-title", json!({"title":""})),
    ] {
        add(
            name,
            w.actor,
            "PATCH",
            &path,
            json!({"operation_id":p.id(name),"expected_revision":w.revision,"details":details}),
            422,
            "invalid_catalog",
        );
    }
    add(
        "stale-patch",
        w.actor,
        "PATCH",
        &path,
        json!({"operation_id":p.id("stale-patch"),"expected_revision":w.revision+1,"details":{"synopsis":"stale"}}),
        409,
        "stale_revision",
    );
    add(
        "wrong-kind",
        w.actor,
        "POST",
        &format!("{path}/content-units"),
        json!({"operation_id":p.id("wrong-kind"),"id":p.id("wrong-kind-unit"),"kind":if w.kind=="podcast"{"chapter"}else{"episode"},"expected_work_revision":w.revision,"details":{"title":"Loại sai","description":""}}),
        422,
        "invalid_catalog",
    );
    if let Some(step) = p
        .steps
        .iter()
        .find(|s| s.method == "POST" && s.path == "/works")
    {
        let mut body = step.body.clone();
        body["details"]["title"] = json!("Changed operation payload");
        add(
            "operation-reused",
            step.actor,
            "POST",
            "/api/v1/studio/works",
            body,
            409,
            "operation_reused",
        );
    }
    if p.profile == "relationships-capacity" && p.size == 1000 {
        let w = &p.works[2];
        add(
            "capacity-plus-one",
            w.actor,
            "POST",
            &format!("/api/v1/studio/works/{}/content-units", w.id),
            json!({"operation_id":p.id("capacity-plus-one"),"id":p.id("capacity-overflow-unit"),"kind":if w.kind=="podcast"{"episode"}else{"chapter"},"expected_work_revision":w.revision,"details":{"title":"Vượt giới hạn","description":""}}),
            422,
            "invalid_catalog",
        );
    }
    if p.profile == "permissions-conflicts" {
        for actor in [2, 3, 4] {
            let name = format!("archive-denied-{actor}");
            add(
                &name,
                actor,
                "DELETE",
                &path,
                json!({"operation_id":p.id(&name),"expected_revision":w.revision}),
                if actor == 4 { 404 } else { 403 },
                if actor == 4 { "not_found" } else { "forbidden" },
            );
        }
        add(
            "reader-edit-denied",
            3,
            "PATCH",
            &path,
            json!({"operation_id":p.id("reader-edit-denied"),"expected_revision":w.revision,"details":{"synopsis":"No permission"}}),
            403,
            "forbidden",
        );
        add(
            "editor-visibility-denied",
            2,
            "PATCH",
            &path,
            json!({"operation_id":p.id("editor-visibility-denied"),"expected_revision":w.revision,"details":{"visibility":"public"}}),
            403,
            "forbidden",
        );
    }
    cases
}
async fn negatives(host: &HttpHost, p: &Plan, tokens: &[String]) {
    for c in negative_requests(p) {
        host.json(
            c["method"].as_str().unwrap(),
            c["path"].as_str().unwrap(),
            Some(&tokens[c["actor"].as_u64().unwrap() as usize]),
            None,
            Some(&c["body"]),
        )
        .await
        .error(
            StatusCode::from_u16(c["expected_status"].as_u64().unwrap() as u16).unwrap(),
            c["expected_code"].as_str().unwrap(),
        );
    }
}

pub async fn substring(db: &cantos_store_turso::OwnedDatabase, p: &Plan) {
    use super::plan;
    use cantos_server::contexts::media::domain::listening::{normalize_search, DiscoveryQuery};
    let reader = db.reader().await.unwrap();
    let corpus: Value = serde_json::from_str(plan::CORPUS).unwrap();
    for q in corpus["queries"].as_array().unwrap() {
        let query = DiscoveryQuery::new(
            q["q"].as_str().unwrap(),
            q["kind"].as_str(),
            q["language"].as_str(),
            20,
        )
        .unwrap();
        let mut actual = vec![];
        for (i, w) in p.works.iter().enumerate() {
            let text = normalize_search(&format!("{} {}", w.title, w.synopsis));
            let mut rows = reader
                .query(
                    "SELECT instr(?1,?2)>0 AND (?3='' OR ?3=?4) AND (?5='' OR lower(?5)=lower(?6))",
                    (
                        text,
                        query.search(),
                        query.kind().unwrap_or(""),
                        w.kind.as_str(),
                        query.language().unwrap_or(""),
                        w.language.as_str(),
                    ),
                )
                .await
                .unwrap();
            if rows.next().await.unwrap().unwrap().get_value(0).unwrap()
                == cantos_store_turso::Value::Integer(1)
            {
                actual.push(i);
            }
        }
        assert_eq!(json!(actual), q["expected"], "{}", q["id"]);
    }
}
