//! Production Studio router and real file-backed Turso. Semantic/codec oracles ported from
//! legacy-pg-tests/tests/revisions_postgres.rs; development sessions are synthetic credentials.
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use cantos_server::contract::{
    codec::{decode, decode_pilot, encode, encode_pilot, WireFormat},
    studio::protobuf::ProtobufDto,
    studio::{
        ReviewRequest, RevisionResponse, SaveRevisionRequest, ScriptValidationResponse,
        ValidateScriptRequest,
    },
};
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    platform::store::{Params, Row, StoreError},
    script_ir::read_script,
    test_support::ServerFixture,
};
use http_body_util::BodyExt;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ORIGIN: &str = "http://127.0.0.1:8080";
const FIXTURE: &str =
    include_str!("../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json");

struct Harness {
    admin: ServerFixture,
}

// Fixture conveniences retain each ported test's exact SQL/response assertions. These are
// real statements through the same owner, never a persistence double.
trait FixtureQueries {
    async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64, StoreError>;
    async fn query_one(&self, sql: &str, params: Params<'_>) -> Result<Row, StoreError>;
}
impl FixtureQueries for ServerFixture {
    async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64, StoreError> {
        self.exec(sql, params).await
    }
    async fn query_one(&self, sql: &str, params: Params<'_>) -> Result<Row, StoreError> {
        let rows = self.rows(sql, params).await?;
        assert_eq!(rows.len(), 1);
        Ok(rows.into_iter().next().unwrap())
    }
}
impl Harness {
    async fn new() -> Self {
        let admin = ServerFixture::new().await;
        for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
            admin.studio_caller(actor, token).await;
            for (kind, id) in read_script(FIXTURE.as_bytes()).unwrap().evidence_refs() {
                admin.exec("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES(?1,?2,?3,'synthetic fixture; no publication clearance')", &[&actor, &kind, &id]).await.unwrap();
            }
            admin.exec("INSERT INTO script_evidence VALUES(?1,'rights','rights-new','synthetic changed rights')", &[&actor]).await.unwrap();
        }
        Self { admin }
    }
    fn app(&self) -> Router {
        router(
            AppState {
                store: self.admin.store().clone(),
                web_origin: ORIGIN.into(),
            },
            "apps/cantos-studio/web/dist",
        )
    }
    async fn count(&self, script: &str) -> (i64, i64) {
        let row = self.admin.query_one("SELECT head_revision,(SELECT count(*) FROM script_revisions WHERE script_id=?1) FROM scripts WHERE id=?1", &[&script]).await.unwrap();
        (row.try_get(0).unwrap(), row.try_get(1).unwrap())
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(format!("/api/v1/studio{path}"))
            .header("origin", ORIGIN)
            .header("content-type", "application/json")
            .header("x-actor-id", "alice");
        if let Some(token) = token {
            request = request.header("cookie", format!("cantos_session={token}"));
        }
        let response = self
            .app()
            .oneshot(
                request
                    .body(Body::from(
                        body.map(|value| value.to_string()).unwrap_or_default(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        let definition = if !status.is_success() {
            "ApiError"
        } else if path.contains("/history") {
            "History"
        } else if path.ends_with("/reviews") {
            "Review"
        } else if path.contains("/sources/") {
            "Source"
        } else {
            "Revision"
        };
        validate_response(&value, definition);
        (status, value)
    }
}

const MSGPACK: &str = "application/msgpack";

/// One exchange with explicit representation headers; every API response keeps its headers.
struct Exchange {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Exchange {
    /// A MessagePack success with the negotiated headers, decoded as `T`.
    fn packed<T: DeserializeOwned>(&self) -> T {
        assert_eq!(
            self.status,
            StatusCode::OK,
            "{:?}",
            String::from_utf8_lossy(&self.body)
        );
        assert_eq!(self.headers["content-type"], MSGPACK);
        assert_eq!(self.headers["vary"], "accept");
        decode(WireFormat::MessagePack, &self.body).unwrap()
    }

    /// Pilot success is decoded by actual Content-Type, independently of request encoding.
    fn pilot<T: DeserializeOwned + ProtobufDto>(&self, wanted: WireFormat) -> T {
        assert_eq!(
            self.status,
            StatusCode::OK,
            "{:?}",
            String::from_utf8_lossy(&self.body)
        );
        assert_eq!(self.headers["content-type"], wanted.media_type());
        assert_eq!(self.headers["vary"], "accept");
        let actual =
            WireFormat::from_content_type(self.headers["content-type"].to_str().unwrap()).unwrap();
        decode_pilot(actual, &self.body).unwrap()
    }

    /// Every failure stays a JSON ApiError, whatever the request asked for.
    fn error(&self, status: StatusCode) -> Value {
        assert_eq!(self.status, status);
        assert_eq!(self.headers["content-type"], "application/json");
        let value: Value = serde_json::from_slice(&self.body).unwrap();
        validate_response(&value, "ApiError");
        value
    }
}

fn packed<T: Serialize>(value: &T) -> Vec<u8> {
    encode(WireFormat::MessagePack, value).unwrap()
}

impl Harness {
    async fn exchange(
        &self,
        origin: &str,
        method: &str,
        path: &str,
        token: Option<&str>,
        (content_type, accept): (Option<&str>, Option<&str>),
        body: Vec<u8>,
    ) -> Exchange {
        let mut request = Request::builder()
            .method(method)
            .uri(format!("/api/v1/studio{path}"))
            .header("origin", origin);
        for (name, value) in [("content-type", content_type), ("accept", accept)] {
            if let Some(value) = value {
                request = request.header(name, value);
            }
        }
        if let Some(token) = token {
            request = request.header("cookie", format!("cantos_session={token}"));
        }
        let response = self
            .app()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        Exchange {
            status: response.status(),
            headers: response.headers().clone(),
            body: response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        }
    }

    async fn msgpack(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Vec<u8>,
    ) -> Exchange {
        let content_type = (!body.is_empty()).then_some(MSGPACK);
        self.exchange(
            ORIGIN,
            method,
            path,
            token,
            (content_type, Some(MSGPACK)),
            body,
        )
        .await
    }

    async fn reviews(&self, script: &str) -> i64 {
        self.admin
            .query_one(
                "SELECT count(*) FROM script_reviews WHERE script_id=?1",
                &[&script],
            )
            .await
            .unwrap()
            .try_get(0)
            .unwrap()
    }
}

fn validate_response(value: &Value, definition: &str) {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../services/cantos-server/contracts/studio/editorial/v1/editorial.schema.json"
    ))
    .unwrap();
    let schema = json!({"$ref":format!("#/$defs/{definition}"),"$defs":schema["$defs"]});
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(value)
        .unwrap();
}

fn request(base: u64) -> SaveRevisionRequest {
    SaveRevisionRequest {
        expected_revision: base,
        operation_id: Uuid::new_v4().to_string(),
        script_json: FIXTURE.into(),
    }
}

fn changed_rights(mut request: SaveRevisionRequest) -> SaveRevisionRequest {
    let mut value: Value = serde_json::from_str(&request.script_json).unwrap();
    value["work"]["rights_record_id"] = json!("rights-new");
    request.script_json = value.to_string();
    request
}

fn review_request(revision: u64) -> Value {
    json!({"operation_id": Uuid::new_v4().to_string(), "revision": revision})
}

#[tokio::test]
async fn editorial_reviews_are_pinned_replayable_owner_only_and_history_is_bounded() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    h.admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    let path = format!("/scripts/{script}/reviews");
    let intent = review_request(1);
    let (status, reviewed) = h
        .request("POST", &path, Some(ALICE), Some(intent.clone()))
        .await;
    assert_eq!(status, StatusCode::OK);
    h.admin
        .store()
        .save(ALICE, &script, request(1))
        .await
        .unwrap();
    h.admin
        .store()
        .save(ALICE, &script, request(2))
        .await
        .unwrap();
    let (status, replay) = h
        .request("POST", &path, Some(ALICE), Some(intent.clone()))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reviewed, replay);
    assert_eq!(
        h.request("POST", &path, Some(ALICE), Some(review_request(1)))
            .await
            .1,
        reviewed
    );
    let mut changed = intent;
    changed["revision"] = json!(3);
    let (status, error) = h.request("POST", &path, Some(ALICE), Some(changed)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "operation_reused");
    let (status, stale) = h
        .request("POST", &path, Some(ALICE), Some(review_request(2)))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale["current_revision"], 3);
    let intent = review_request(3);
    let (first, second) = tokio::join!(
        h.request("POST", &path, Some(ALICE), Some(intent.clone())),
        h.request("POST", &path, Some(ALICE), Some(intent)),
    );
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(first, second);
    for token in [None, Some(BOB)] {
        let status = h
            .request("GET", &format!("/scripts/{script}/history"), token, None)
            .await
            .0;
        assert_eq!(
            status,
            if token.is_none() {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::NOT_FOUND
            }
        );
    }
    h.admin
        .execute(
            "INSERT INTO script_members VALUES(?1,'bob','editor')",
            &[&script],
        )
        .await
        .unwrap();
    assert_eq!(
        h.request("POST", &path, Some(BOB), Some(review_request(3)))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, page) = h
        .request(
            "GET",
            &format!("/scripts/{script}/history?limit=1"),
            Some(BOB),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["next_after"], 1);
    assert_eq!(page["reviews"], json!([reviewed]));
    assert!(page["revisions"][0].get("script_json").is_none());
    let (_, page) = h
        .request(
            "GET",
            &format!("/scripts/{script}/history?after_revision=1&limit=2"),
            Some(ALICE),
            None,
        )
        .await;
    assert_eq!(page["revisions"].as_array().unwrap().len(), 2);
    assert_eq!(page["reviews"].as_array().unwrap().len(), 1);
    assert_eq!(page["next_after"], Value::Null);
    for query in [
        "limit=0",
        "limit=51",
        "limit=no",
        "after_revision=9223372036854775808",
        "extra=1",
    ] {
        assert_eq!(
            h.request(
                "GET",
                &format!("/scripts/{script}/history?{query}"),
                Some(ALICE),
                None
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    h.admin
        .execute("DELETE FROM script_members WHERE script_id=?1", &[&script])
        .await
        .unwrap();
    assert_eq!(
        h.request(
            "GET",
            &format!("/scripts/{script}/history"),
            Some(BOB),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let rows = h.admin.query_one("SELECT (SELECT count(*) FROM script_reviews),(SELECT count(*) FROM script_review_operations)", &[]).await.unwrap();
    assert_eq!(rows.try_get::<_, i64>(0).unwrap(), 2);
    assert_eq!(rows.try_get::<_, i64>(1).unwrap(), 3);
}

#[tokio::test]
async fn complete_exports_metadata_retries_and_pinned_history_survive_edits() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let first_request = request(0);
    let first = h
        .admin
        .store()
        .save(ALICE, &script, first_request.clone())
        .await
        .unwrap();
    let fixture: RevisionResponse = serde_json::from_str(include_str!(
        "../../../services/cantos-server/contracts/studio/editorial/v1/fixtures/revision.json"
    ))
    .unwrap();
    assert_eq!(first.script_json, fixture.script_json);
    assert_eq!(first.export_digest, fixture.export_digest);
    assert_eq!(
        first.script_json.as_bytes(),
        read_script(FIXTURE.as_bytes()).unwrap().export_bytes()
    );
    let second = h
        .admin
        .store()
        .save(ALICE, &script, changed_rights(request(1)))
        .await
        .unwrap();
    assert_eq!(first.content_digest, second.content_digest);
    assert_ne!(first.export_digest, second.export_digest);
    assert_ne!(first.script_json, second.script_json);
    assert_eq!(
        h.admin
            .store()
            .save(ALICE, &script, first_request.clone())
            .await
            .unwrap(),
        first
    );
    assert_eq!(
        h.admin.store().load(ALICE, &script, Some(1)).await.unwrap(),
        first
    );
    assert_eq!(
        h.admin.store().load(ALICE, &script, None).await.unwrap(),
        second
    );
    assert!(matches!(
        h.admin
            .store()
            .save(ALICE, &script, changed_rights(first_request.clone()))
            .await,
        Err(StoreError::OperationReused)
    ));
    let mut wrong_base = first_request;
    wrong_base.expected_revision = 1;
    assert!(matches!(
        h.admin.store().save(ALICE, &script, wrong_base).await,
        Err(StoreError::OperationReused)
    ));
    assert!(matches!(
        h.admin.store().save(ALICE, &script, request(1)).await,
        Err(StoreError::StaleRevision(2))
    ));
    assert_eq!(h.count(&script).await, (2, 2));
    let evidence_count: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM revision_evidence WHERE script_id=?1",
            &[&script],
        )
        .await
        .unwrap()
        .try_get(0)
        .unwrap();
    assert_eq!(evidence_count, 11);
}

#[tokio::test]
async fn backend_denies_anonymous_wrong_owner_reader_revoked_and_expired_sessions() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let saved = h
        .admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    let head = format!("/scripts/{script}/head");
    let writes = format!("/scripts/{script}/revisions");
    let (status, body) = h.request("GET", &head, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthenticated");
    assert_eq!(
        h.request("POST", &writes, None, Some(json!({"invalid":"body"})))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    for path in [&head, &format!("/scripts/{script}/revisions/1")] {
        let (status, body) = h.request("GET", path, Some(BOB), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "not_found");
        assert!(!body.to_string().contains("Ánh đèn"));
    }
    assert_eq!(
        h.request(
            "POST",
            &writes,
            Some(BOB),
            Some(serde_json::to_value(request(1)).unwrap())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    h.admin
        .execute(
            "INSERT INTO script_members VALUES(?1,'bob','reader')",
            &[&script],
        )
        .await
        .unwrap();
    let (status, body) = h.request("GET", &head, Some(BOB), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_value::<RevisionResponse>(body).unwrap(),
        saved
    );
    assert_eq!(
        h.request(
            "POST",
            &writes,
            Some(BOB),
            Some(serde_json::to_value(request(1)).unwrap())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    h.admin
        .execute(
            "UPDATE script_members SET role='editor' WHERE script_id=?1",
            &[&script],
        )
        .await
        .unwrap();
    let edited = h
        .admin
        .store()
        .save(BOB, &script, request(1))
        .await
        .unwrap();
    assert_eq!(edited.accepted_by, "bob");
    h.admin
        .execute("DELETE FROM script_members WHERE script_id=?1", &[&script])
        .await
        .unwrap();
    assert_eq!(
        h.request("GET", &head, Some(BOB), None).await.0,
        StatusCode::NOT_FOUND
    );
    const EXPIRED: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    h.admin.seed_development_session("bob", EXPIRED, -1).await;
    assert_eq!(
        h.request("GET", &head, Some(EXPIRED), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    h.admin
        .execute(
            "UPDATE development_sessions SET revoked=1 WHERE actor_id='alice'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        h.request("GET", &head, Some(ALICE), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(h.count(&script).await, (2, 2));
}

#[tokio::test]
async fn invalid_exports_and_foreign_evidence_leave_no_partial_script() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let mut invalid = request(0);
    invalid.script_json = "{}".into();
    assert!(matches!(
        h.admin.store().save(ALICE, &script, invalid).await,
        Err(StoreError::InvalidScript(_))
    ));
    // Every input field fits its scalar bound, but NFC doubles the whole export past 2 MiB.
    // This must be a typed admission failure, rather than a SQL CHECK failure mapped to 503.
    let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
    let lines = value["episode"]["acts"][0]["scenes"][0]["dialogues"]
        .as_array_mut()
        .unwrap();
    for number in 0..110 {
        lines.push(
            json!({"id":format!("expansion-{number}"),"speaker_id":"narrator",
            "text":"\u{0344}".repeat(5000),
            "delivery":{"emotion":"neutral","intensity_permille":300}}),
        );
    }
    let mut expanded = request(0);
    expanded.script_json = value.to_string();
    assert!(expanded.script_json.len() < 2 * 1024 * 1024);
    let (status, error) = h
        .request(
            "POST",
            &format!("/scripts/{script}/revisions"),
            Some(ALICE),
            Some(serde_json::to_value(expanded).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["code"], "invalid_script");
    assert_eq!(
        error["issues"],
        json!([{"path":"$","rule":"DocumentTooLarge"}])
    );
    h.admin.execute("INSERT INTO script_evidence VALUES('bob','rights','bob-private','synthetic foreign evidence')",&[]).await.unwrap();
    let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
    value["work"]["rights_record_id"] = json!("bob-private");
    let mut foreign = request(0);
    foreign.script_json = value.to_string();
    assert!(matches!(
        h.admin.store().save(ALICE, &script, foreign).await,
        Err(StoreError::EvidenceUnavailable)
    ));
    let count: i64 = h
        .admin
        .query_one("SELECT count(*) FROM scripts WHERE id=?1", &[&script])
        .await
        .unwrap()
        .try_get(0)
        .unwrap();
    assert_eq!(count, 0);
    assert!(matches!(
        h.admin.store().save(ALICE, &script, request(1)).await,
        Err(StoreError::StaleRevision(0))
    ));
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM scripts WHERE id=?1", &[&script])
            .await
            .unwrap()
            .try_get::<_, i64>(0)
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn msgpack_and_json_carry_the_same_revision_facts_on_every_negotiated_route() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let first = h
        .admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    let head = format!("/scripts/{script}/head");
    for path in [head.clone(), format!("/scripts/{script}/revisions/1")] {
        let (status, json) = h.request("GET", &path, Some(ALICE), None).await;
        assert_eq!(status, StatusCode::OK);
        let packed: RevisionResponse = h.msgpack("GET", &path, Some(ALICE), vec![]).await.packed();
        assert_eq!(
            packed,
            serde_json::from_value::<RevisionResponse>(json).unwrap()
        );
        assert_eq!(packed, first);
    }
    let writes = format!("/scripts/{script}/revisions");
    let second: RevisionResponse = h
        .msgpack("POST", &writes, Some(ALICE), packed(&request(1)))
        .await
        .packed();
    assert_eq!((second.revision, second.accepted_by.as_str()), (2, "alice"));
    let (_, json) = h.request("GET", &head, Some(ALICE), None).await;
    assert_eq!(
        serde_json::from_value::<RevisionResponse>(json).unwrap(),
        second
    );

    let valid = ValidateScriptRequest {
        script_json: second.script_json.clone(),
    };
    let preview: ScriptValidationResponse = h
        .msgpack("POST", "/validation", Some(ALICE), packed(&valid))
        .await
        .packed();
    assert!(preview.issues.is_empty());
    let invalid = ValidateScriptRequest {
        script_json: include_str!(
            "../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/reject/multiple-faults.json"
        )
        .into(),
    };
    let (status, json_error) = h
        .request(
            "POST",
            "/validation",
            Some(ALICE),
            Some(serde_json::to_value(&invalid).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let packed_error = h
        .msgpack("POST", "/validation", Some(ALICE), packed(&invalid))
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(packed_error, json_error);
    assert_eq!(h.count(&script).await, (2, 2));
}

#[tokio::test]
async fn a_lost_msgpack_response_replays_in_either_representation_without_a_second_revision() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    h.admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    let writes = format!("/scripts/{script}/revisions");
    let intent = request(1);
    let committed: RevisionResponse = h
        .msgpack("POST", &writes, Some(ALICE), packed(&intent))
        .await
        .packed();
    // The client lost that response. The same key, base and snapshot replays the first result
    // whichever representation the retry uses; the codec never changes the idempotency key.
    let (status, replay) = h
        .request(
            "POST",
            &writes,
            Some(ALICE),
            Some(serde_json::to_value(&intent).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_value::<RevisionResponse>(replay).unwrap(),
        committed
    );
    let replay: RevisionResponse = h
        .msgpack("POST", &writes, Some(ALICE), packed(&intent))
        .await
        .packed();
    assert_eq!(replay, committed);
    assert_eq!(h.count(&script).await, (2, 2));

    let reused = h
        .msgpack(
            "POST",
            &writes,
            Some(ALICE),
            packed(&changed_rights(intent)),
        )
        .await
        .error(StatusCode::CONFLICT);
    assert_eq!(reused["code"], "operation_reused");
    let stale = h
        .msgpack("POST", &writes, Some(ALICE), packed(&request(1)))
        .await
        .error(StatusCode::CONFLICT);
    assert_eq!(stale["code"], "stale_revision");
    assert_eq!(stale["current_revision"], 2);
    assert_eq!(h.count(&script).await, (2, 2));
}

#[tokio::test]
async fn refused_representations_write_nothing_and_other_routes_stay_json_only() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    h.admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    let writes = format!("/scripts/{script}/revisions");
    let intent = request(1);
    let body = packed(&intent);
    let not_acceptable = h
        .exchange(
            ORIGIN,
            "POST",
            &writes,
            Some(ALICE),
            (Some(MSGPACK), Some("text/html")),
            body.clone(),
        )
        .await;
    assert_eq!(not_acceptable.headers["vary"], "accept");
    assert_eq!(
        not_acceptable.error(StatusCode::NOT_ACCEPTABLE),
        json!({"code":"invalid_request","current_revision":null,
               "issues":[{"path":"accept","rule":"not_acceptable"}]})
    );
    let unsupported = h
        .exchange(
            ORIGIN,
            "POST",
            &writes,
            Some(ALICE),
            (Some("text/plain"), None),
            serde_json::to_vec(&intent).unwrap(),
        )
        .await;
    assert_eq!(
        unsupported.error(StatusCode::UNSUPPORTED_MEDIA_TYPE)["issues"],
        json!([{"path":"content-type","rule":"unsupported_media_type"}])
    );
    let forged_field = packed(&json!({
        "expected_revision": 1, "operation_id": intent.operation_id,
        "script_json": intent.script_json, "accepted_by": "mallory"}));
    for malformed in [body[..body.len() - 1].to_vec(), forged_field] {
        let refused = h.msgpack("POST", &writes, Some(ALICE), malformed).await;
        assert_eq!(
            refused.error(StatusCode::BAD_REQUEST)["code"],
            "invalid_request"
        );
    }
    assert_eq!(h.count(&script).await, (1, 1));
    // Nothing was half-applied: the same intent still commits once.
    let saved: RevisionResponse = h.msgpack("POST", &writes, Some(ALICE), body).await.packed();
    assert_eq!(saved.revision, 2);

    let review = ReviewRequest {
        operation_id: Uuid::new_v4().to_string(),
        revision: 2,
    };
    let refused = h
        .msgpack(
            "POST",
            &format!("/scripts/{script}/reviews"),
            Some(ALICE),
            packed(&review),
        )
        .await;
    assert_eq!(
        refused.error(StatusCode::BAD_REQUEST)["code"],
        "invalid_request"
    );
    assert_eq!(h.reviews(&script).await, 0);
    let history = h
        .msgpack(
            "GET",
            &format!("/scripts/{script}/history"),
            Some(ALICE),
            vec![],
        )
        .await;
    assert_eq!(history.status, StatusCode::OK);
    assert_eq!(history.headers["content-type"], "application/json");
    assert!(!history.headers.contains_key("vary"));
}

#[tokio::test]
async fn msgpack_requests_get_the_same_origin_session_and_permission_decisions() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let saved = h
        .admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    let head = format!("/scripts/{script}/head");
    let writes = format!("/scripts/{script}/revisions");
    let body = packed(&request(1));
    for (method, path, body) in [("GET", &head, vec![]), ("POST", &writes, body.clone())] {
        let anonymous = h.msgpack(method, path, None, body.clone()).await;
        assert_eq!(
            anonymous.error(StatusCode::UNAUTHORIZED)["code"],
            "unauthenticated"
        );
        let stranger = h.msgpack(method, path, Some(BOB), body).await;
        let hidden = stranger.error(StatusCode::NOT_FOUND);
        assert_eq!(hidden["code"], "not_found");
        assert!(!hidden.to_string().contains("Ánh đèn"));
    }
    let forged = h
        .exchange(
            "https://attacker.invalid",
            "POST",
            &writes,
            Some(ALICE),
            (Some(MSGPACK), Some(MSGPACK)),
            body.clone(),
        )
        .await;
    assert_eq!(forged.error(StatusCode::FORBIDDEN)["code"], "forbidden");
    h.admin
        .execute(
            "INSERT INTO script_members VALUES(?1,'bob','reader')",
            &[&script],
        )
        .await
        .unwrap();
    let read: RevisionResponse = h.msgpack("GET", &head, Some(BOB), vec![]).await.packed();
    assert_eq!(read, saved);
    let denied = h.msgpack("POST", &writes, Some(BOB), body).await;
    assert_eq!(denied.error(StatusCode::FORBIDDEN)["code"], "forbidden");
    h.admin
        .execute(
            "UPDATE development_sessions SET revoked=1 WHERE actor_id='bob'",
            &[],
        )
        .await
        .unwrap();
    let revoked = h.msgpack("GET", &head, Some(BOB), vec![]).await;
    assert_eq!(
        revoked.error(StatusCode::UNAUTHORIZED)["code"],
        "unauthenticated"
    );
    assert_eq!(h.count(&script).await, (1, 1));
}

#[tokio::test]
async fn legacy_studio_paths_remain_compatible_and_sessions_stay_scoped_to_their_mount() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let send = |method: &'static str, uri: String, cookie: Option<String>, body: String| {
        let app = h.app();
        async move {
            let mut request = Request::builder()
                .method(method)
                .uri(uri)
                .header("origin", ORIGIN)
                .header("content-type", "application/json");
            if let Some(cookie) = cookie {
                request = request.header("cookie", cookie);
            }
            app.oneshot(request.body(Body::from(body)).unwrap())
                .await
                .unwrap()
        }
    };
    // Legacy login keeps the pre-split cookie scope and is flagged deprecated.
    let login = send(
        "POST",
        "/api/v1/session".into(),
        None,
        json!({"token":ALICE}).to_string(),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    assert!(login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .ends_with("Path=/api/v1"));
    assert_eq!(login.headers()["deprecation"], "true");
    // A write through the legacy path and a read through the current path see one revision.
    let session = Some(format!("cantos_session={ALICE}"));
    let body = serde_json::to_string(&request(0)).unwrap();
    let saved = send(
        "POST",
        format!("/api/v1/scripts/{script}/revisions"),
        session.clone(),
        body,
    )
    .await;
    assert_eq!(saved.status(), StatusCode::OK);
    let saved: RevisionResponse =
        serde_json::from_slice(&saved.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let (status, current) = h
        .request("GET", &format!("/scripts/{script}/head"), Some(ALICE), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_value::<RevisionResponse>(current).unwrap(),
        saved
    );
    assert_eq!(h.count(&script).await, (1, 1));
    // A valid Studio session grants nothing on the reserved surfaces.
    for base in ["/api/v1/theatre", "/api/v1/admin"] {
        for path in [format!("/scripts/{script}/head"), "/session".into()] {
            let response = send(
                "GET",
                format!("{base}{path}"),
                session.clone(),
                String::new(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{base}{path}");
        }
    }
}

#[tokio::test]
async fn protobuf_json_and_legacy_msgpack_have_identical_revision_and_validation_facts() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let writes = format!("/scripts/{script}/revisions");
    let intent = request(0);
    let mut original = None;
    for sent in [
        WireFormat::Json,
        WireFormat::Protobuf,
        WireFormat::MessagePack,
    ] {
        for wanted in [
            WireFormat::Json,
            WireFormat::Protobuf,
            WireFormat::MessagePack,
        ] {
            let saved: RevisionResponse = h
                .exchange(
                    ORIGIN,
                    "POST",
                    &writes,
                    Some(ALICE),
                    (Some(sent.media_type()), Some(wanted.media_type())),
                    encode_pilot(sent, &intent).unwrap(),
                )
                .await
                .pilot(wanted);
            if let Some(original) = &original {
                assert_eq!(&saved, original);
            } else {
                original = Some(saved.clone());
            }
            assert_eq!(
                saved.script_json.as_bytes(),
                read_script(FIXTURE.as_bytes()).unwrap().export_bytes()
            );
            for path in [
                format!("/scripts/{script}/head"),
                format!("/scripts/{script}/revisions/1"),
            ] {
                let loaded: RevisionResponse = h
                    .exchange(
                        ORIGIN,
                        "GET",
                        &path,
                        Some(ALICE),
                        (None, Some(wanted.media_type())),
                        vec![],
                    )
                    .await
                    .pilot(wanted);
                assert_eq!(loaded, saved);
            }
            let valid = ValidateScriptRequest {
                script_json: saved.script_json.clone(),
            };
            let preview: ScriptValidationResponse = h
                .exchange(
                    ORIGIN,
                    "POST",
                    "/validation",
                    Some(ALICE),
                    (Some(sent.media_type()), Some(wanted.media_type())),
                    encode_pilot(sent, &valid).unwrap(),
                )
                .await
                .pilot(wanted);
            assert_eq!(preview.issues, vec![]);
        }
    }
    let invalid = ValidateScriptRequest {
        script_json: include_str!(
            "../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/reject/multiple-faults.json"
        )
        .into(),
    };
    let (_, json_error) = h
        .request(
            "POST",
            "/validation",
            Some(ALICE),
            Some(serde_json::to_value(&invalid).unwrap()),
        )
        .await;
    let protobuf_error = h
        .exchange(
            ORIGIN,
            "POST",
            "/validation",
            Some(ALICE),
            (Some("application/protobuf"), Some("application/protobuf")),
            encode_pilot(WireFormat::Protobuf, &invalid).unwrap(),
        )
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(protobuf_error, json_error);
    assert_eq!(h.count(&script).await, (1, 1));
}

#[tokio::test]
async fn lost_json_and_protobuf_responses_replay_both_directions_without_another_write() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let writes = format!("/scripts/{script}/revisions");
    for (base, first, replay) in [
        (0, WireFormat::Json, WireFormat::Protobuf),
        (1, WireFormat::Protobuf, WireFormat::Json),
    ] {
        let intent = request(base);
        // Drop the first response without decoding it, simulating an ambiguous client outcome.
        let lost = h
            .exchange(
                ORIGIN,
                "POST",
                &writes,
                Some(ALICE),
                (Some(first.media_type()), Some(first.media_type())),
                encode_pilot(first, &intent).unwrap(),
            )
            .await;
        assert_eq!(lost.status, StatusCode::OK);
        drop(lost);
        let authoritative = h
            .admin
            .store()
            .load(ALICE, &script, Some(base + 1))
            .await
            .unwrap();
        let reconciled: RevisionResponse = h
            .exchange(
                ORIGIN,
                "POST",
                &writes,
                Some(ALICE),
                (Some(replay.media_type()), Some(replay.media_type())),
                encode_pilot(replay, &intent).unwrap(),
            )
            .await
            .pilot(replay);
        assert_eq!(reconciled, authoritative);
        assert_eq!(
            h.count(&script).await,
            ((base + 1) as i64, (base + 1) as i64)
        );
        let reused = h
            .exchange(
                ORIGIN,
                "POST",
                &writes,
                Some(ALICE),
                (Some(replay.media_type()), Some(replay.media_type())),
                encode_pilot(replay, &changed_rights(intent)).unwrap(),
            )
            .await
            .error(StatusCode::CONFLICT);
        assert_eq!(reused["code"], "operation_reused");
        let stale = h
            .exchange(
                ORIGIN,
                "POST",
                &writes,
                Some(ALICE),
                (Some(replay.media_type()), Some(replay.media_type())),
                encode_pilot(replay, &request(base)).unwrap(),
            )
            .await
            .error(StatusCode::CONFLICT);
        assert_eq!(stale["code"], "stale_revision");
        assert_eq!(stale["current_revision"], base + 1);
        assert_eq!(
            h.count(&script).await,
            ((base + 1) as i64, (base + 1) as i64)
        );
    }
}

#[tokio::test]
async fn protobuf_refusals_preserve_database_state_and_do_not_expand_other_routes() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let original = h
        .admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    let writes = format!("/scripts/{script}/revisions");
    let intent = request(1);
    let body = encode_pilot(WireFormat::Protobuf, &intent).unwrap();
    let mut duplicate = body.clone();
    duplicate.extend_from_slice(&[0x08, 0x01]);
    let mut unknown = body.clone();
    unknown.extend_from_slice(&[0x20, 0x00]);
    for malformed in [
        vec![],
        body[..body.len() - 1].to_vec(),
        duplicate,
        unknown,
        vec![0x00],
        vec![0x12, 0x01, 0xff],
        vec![0x1a, 0xff, 0xff, 0xff, 0xff, 0x07],
        vec![0; 8 * 1024 * 1024 + 1],
    ] {
        let refused = h
            .exchange(
                ORIGIN,
                "POST",
                &writes,
                Some(ALICE),
                (Some("application/protobuf"), Some("application/protobuf")),
                malformed,
            )
            .await;
        assert_eq!(
            refused.error(StatusCode::BAD_REQUEST),
            json!({"code":"invalid_request","current_revision":null,"issues":[]})
        );
        assert_eq!(h.count(&script).await, (1, 1));
        assert_eq!(
            h.admin.store().load(ALICE, &script, None).await.unwrap(),
            original
        );
    }
    for content_type in [
        "application/protobuf;version=2",
        "application/protobuf;charset=utf-8",
        "application/protobuf;encoding=json",
        "application/protobuf;version=1;version=1",
        "application/protobuf+json",
    ] {
        let refused = h
            .exchange(
                ORIGIN,
                "POST",
                &writes,
                Some(ALICE),
                (Some(content_type), Some("application/protobuf")),
                body.clone(),
            )
            .await;
        assert_eq!(
            refused.error(StatusCode::UNSUPPORTED_MEDIA_TYPE)["issues"],
            json!([{"path":"content-type","rule":"unsupported_media_type"}])
        );
        assert_eq!(h.count(&script).await, (1, 1));
    }
    for accept in [
        "text/html",
        "application/protobuf;version=2",
        "application/protobuf;encoding=json",
        "application/protobuf;q=0,application/json;q=0,application/msgpack;q=0,*/*",
    ] {
        let refused = h
            .exchange(
                ORIGIN,
                "POST",
                &writes,
                Some(ALICE),
                (Some("application/protobuf"), Some(accept)),
                body.clone(),
            )
            .await;
        assert_eq!(refused.headers["vary"], "accept");
        assert_eq!(
            refused.error(StatusCode::NOT_ACCEPTABLE)["issues"],
            json!([{"path":"accept","rule":"not_acceptable"}])
        );
        assert_eq!(h.count(&script).await, (1, 1));
    }
    let saved: RevisionResponse = h
        .exchange(
            ORIGIN,
            "POST",
            &writes,
            Some(ALICE),
            (
                Some("application/x-protobuf;encoding=binary;version=1"),
                Some("application/x-protobuf"),
            ),
            body.clone(),
        )
        .await
        .pilot(WireFormat::Protobuf);
    assert_eq!(saved.revision, 2);
    let refused = h
        .exchange(
            ORIGIN,
            "POST",
            &format!("/scripts/{script}/reviews"),
            Some(ALICE),
            (Some("application/protobuf"), Some("application/protobuf")),
            body,
        )
        .await;
    assert_eq!(
        refused.error(StatusCode::BAD_REQUEST)["code"],
        "invalid_request"
    );
    assert_eq!(h.reviews(&script).await, 0);
    let history = h
        .exchange(
            ORIGIN,
            "GET",
            &format!("/scripts/{script}/history"),
            Some(ALICE),
            (None, Some("application/protobuf")),
            vec![],
        )
        .await;
    assert_eq!(history.headers["content-type"], "application/json");
    assert!(!history.headers.contains_key("vary"));
}

#[tokio::test]
async fn protobuf_authorization_and_legacy_aliases_keep_existing_json_error_taxonomy() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let saved = h
        .admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    let head = format!("/scripts/{script}/head");
    let writes = format!("/scripts/{script}/revisions");
    let body = encode_pilot(WireFormat::Protobuf, &request(1)).unwrap();
    for (method, path, body) in [("GET", &head, vec![]), ("POST", &writes, body.clone())] {
        for (token, status, code) in [
            (None, StatusCode::UNAUTHORIZED, "unauthenticated"),
            (Some(BOB), StatusCode::NOT_FOUND, "not_found"),
        ] {
            let error = h
                .exchange(
                    ORIGIN,
                    method,
                    path,
                    token,
                    (Some("application/protobuf"), Some("application/protobuf")),
                    body.clone(),
                )
                .await
                .error(status);
            assert_eq!(error["code"], code);
        }
    }
    let forged = h
        .exchange(
            "https://attacker.invalid",
            "POST",
            &writes,
            Some(ALICE),
            (Some("application/protobuf;version=2"), Some("text/html")),
            body.clone(),
        )
        .await
        .error(StatusCode::FORBIDDEN);
    assert_eq!(forged["code"], "forbidden");
    h.admin
        .execute(
            "INSERT INTO script_members VALUES(?1,'bob','reader')",
            &[&script],
        )
        .await
        .unwrap();
    let loaded: RevisionResponse = h
        .exchange(
            ORIGIN,
            "GET",
            &head,
            Some(BOB),
            (None, Some("application/protobuf")),
            vec![],
        )
        .await
        .pilot(WireFormat::Protobuf);
    assert_eq!(loaded, saved);
    assert_eq!(
        h.exchange(
            ORIGIN,
            "POST",
            &writes,
            Some(BOB),
            (Some("application/protobuf"), Some("application/protobuf")),
            body
        )
        .await
        .error(StatusCode::FORBIDDEN)["code"],
        "forbidden"
    );
    h.admin
        .execute(
            "UPDATE development_sessions SET revoked=1 WHERE actor_id='bob'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        h.exchange(
            ORIGIN,
            "GET",
            &head,
            Some(BOB),
            (None, Some("application/protobuf")),
            vec![]
        )
        .await
        .error(StatusCode::UNAUTHORIZED)["code"],
        "unauthenticated"
    );
    for (method, path, body) in [
        ("GET", format!("/scripts/{script}/head"), vec![]),
        ("GET", format!("/scripts/{script}/revisions/1"), vec![]),
        (
            "POST",
            writes,
            encode_pilot(WireFormat::Protobuf, &request(1)).unwrap(),
        ),
        (
            "POST",
            "/validation".to_owned(),
            encode_pilot(
                WireFormat::Protobuf,
                &ValidateScriptRequest {
                    script_json: saved.script_json.clone(),
                },
            )
            .unwrap(),
        ),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(format!("/api/v1{path}"))
            .header("origin", ORIGIN)
            .header("cookie", format!("cantos_session={ALICE}"))
            .header("content-type", "application/protobuf")
            .header("accept", "application/protobuf")
            .body(Body::from(body))
            .unwrap();
        let response = h.app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/protobuf");
        assert_eq!(response.headers()["deprecation"], "true");
        assert_eq!(
            response.headers()["link"],
            "</api/v1/studio>; rel=\"successor-version\""
        );
        assert_eq!(response.headers()["vary"], "accept");
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    }
    assert_eq!(h.count(&script).await, (2, 2));
}

#[tokio::test]
async fn session_exchange_origin_guard_and_malformed_inputs_are_safe() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    h.admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    let login = Request::builder()
        .method("POST")
        .uri("/api/v1/studio/session")
        .header("origin", ORIGIN)
        .header("content-type", "application/json")
        .body(Body::from(json!({"token":ALICE}).to_string()))
        .unwrap();
    let response = h.app().oneshot(login).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()["set-cookie"].to_str().unwrap();
    assert!(
        cookie.contains("HttpOnly")
            && cookie.contains("SameSite=Strict")
            && cookie.contains("Path=/api/v1/studio")
    );
    let identity: cantos_server::contract::studio::SessionResponse =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(identity.actor_id, "alice");
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/studio/scripts/{script}/revisions"))
        .header("origin", "https://attacker.invalid")
        .header("cookie", format!("cantos_session={ALICE}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&self::request(1)).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        h.app().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let (status, error) = h
        .request(
            "GET",
            &format!("/scripts/{script}/revisions/not-a-number"),
            Some(ALICE),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["code"], "invalid_request");
    let (status, error) = h
        .request(
            "POST",
            &format!("/scripts/{script}/revisions"),
            Some(ALICE),
            Some(json!({"unknown":"field"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["code"], "invalid_request");
    let mut invalid = self::request(1);
    invalid.script_json = include_str!(
        "../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/reject/multiple-faults.json"
    )
    .into();
    let (status, error) = h
        .request(
            "POST",
            &format!("/scripts/{script}/revisions"),
            Some(ALICE),
            Some(serde_json::to_value(invalid).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["code"], "invalid_script");
    assert!(error["issues"].as_array().unwrap().len() > 1);
    assert_eq!(h.count(&script).await, (1, 1));
    let logout = Request::builder()
        .method("DELETE")
        .uri("/api/v1/studio/session")
        .header("origin", ORIGIN)
        .header("cookie", format!("cantos_session={ALICE}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        h.app().oneshot(logout).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert!(matches!(
        h.admin.store().authenticate(ALICE).await,
        Err(StoreError::Unauthenticated)
    ));
}

#[tokio::test]
async fn failure_after_a_new_revision_insert_preserves_the_previous_head_and_retries_once() {
    let h = Harness::new().await;
    let script = Uuid::new_v4().to_string();
    let first = h
        .admin
        .store()
        .save(ALICE, &script, request(0))
        .await
        .unwrap();
    h.admin.exec("CREATE TRIGGER refuse_second_revision AFTER INSERT ON script_revisions WHEN NEW.revision=2 BEGIN SELECT RAISE(ABORT,'cantos:test.after_revision'); END", &[]).await.unwrap();
    let intent = request(1);
    assert!(matches!(
        h.admin.store().save(ALICE, &script, intent.clone()).await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.count(&script).await, (1, 1));
    assert_eq!(
        h.admin.store().load(ALICE, &script, None).await.unwrap(),
        first
    );
    h.admin
        .exec("DROP TRIGGER refuse_second_revision", &[])
        .await
        .unwrap();
    let second = h
        .admin
        .store()
        .save(ALICE, &script, intent.clone())
        .await
        .unwrap();
    assert_eq!(
        h.admin.store().save(ALICE, &script, intent).await.unwrap(),
        second
    );
    assert_eq!(h.count(&script).await, (2, 2));
}
