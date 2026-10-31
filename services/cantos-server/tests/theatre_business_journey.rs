//! #333 CMP business API journey over the **production** composition: `server_router` (route
//! policies, Identity session middleware, production Theatre/Library handlers, origin media), a
//! real file-backed `server.db` this process owns, and the **real** `cantos-identity` executable
//! over its authenticated Unix socket with a run-owned SMTP sink. Nothing seeds a session: every
//! credential is earned by signing up through the mailed link and logging in.
//!
//! What this is: a router-level integration (the Axum `Router` is driven in-process; Identity is a
//! second OS process). What it is not: the `cantos-server` binary over TCP, a creator -> publication
//! journey (the catalog is seeded with valid domain values by `test_support`), an inspected Ogg/Opus
//! decode, a CDN, or a device. The media bytes are exact content-addressed transport bytes.
//!
//! The Identity executable is built from this workspace when `CANTOS_TEST_IDENTITY_BINARY` does
//! not name a prebuilt one:
//!
//! ```sh
//! CANTOS_TEST_IDENTITY_BINARY=$PWD/target/debug/cantos-identity \
//!   cargo test -p cantos-server --test theatre_business_journey
//! ```
#[path = "support/identity_process.rs"]
mod process;

use std::{net::SocketAddr, path::Path, sync::Arc};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header::HeaderMap, Request, StatusCode},
    Router,
};
use cantos_server::contract::theatre::library as dto;
use cantos_server::{
    adapters::inbound::http::{server_router, AppState},
    contexts::media::{
        domain::playback_grant::GrantKey, FilesystemPublicationStorage, OriginPlayback,
        PublicMediaOrigin, PublicationService,
    },
    platform::{
        host::SurfaceOrigins,
        identity_auth::{config, IdentityAuth},
    },
    test_support::{
        library::{
            audio, bookmark, bookmark_delete, history_clear, history_start, progress, saved,
        },
        SeededWork, ServerFixture, WorkSeed,
    },
};
use http_body_util::BodyExt;
use process::{IdentityEnv, Instance, RunDir, SmtpSink};
use serde_json::{json, Value};
use tower::ServiceExt;

const THEATRE: &str = "https://theatre.example";
const PASSWORD: &str = "a correct horse battery";
const OWNER: &str = "owner@example.com";
const SECOND: &str = "second@example.com";
const PEER: &str = "127.0.0.1:50000";

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|_| panic!("not JSON: {}", String::from_utf8_lossy(&self.body)))
    }
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
}

async fn send(
    router: &Router,
    method: &str,
    path: &str,
    headers: &[(&str, String)],
    body: Option<Vec<u8>>,
) -> Reply {
    let mut builder = Request::builder().method(method).uri(path);
    for (name, value) in headers {
        builder = builder.header(*name, value);
    }
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let mut request = builder
        .body(body.map_or(Body::empty(), Body::from))
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(PEER.parse::<SocketAddr>().unwrap()));
    let response = router.clone().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    Reply {
        status: parts.status,
        headers: parts.headers,
        body: body.collect().await.unwrap().to_bytes().to_vec(),
    }
}

fn theatre(path: &str) -> String {
    format!("/api/v1/theatre{path}")
}

fn bearer(token: &str) -> (&'static str, String) {
    ("authorization", format!("Bearer {token}"))
}

fn json_body(value: &impl serde::Serialize) -> Option<Vec<u8>> {
    Some(serde_json::to_vec(value).unwrap())
}

/// A response checked against its published schema definition.
fn conforms(schema: &str, definition: &str, value: &Value) {
    let root: Value = serde_json::from_str(schema).unwrap();
    let validator = jsonschema::validator_for(
        &json!({"$schema":root["$schema"],"$defs":root["$defs"],"$ref":format!("#/$defs/{definition}")}),
    )
    .unwrap();
    assert!(
        validator.is_valid(value),
        "contract mismatch: {definition}; response omitted to redact credentials"
    );
}

const DISCOVERY: &str = include_str!(
    "../../../services/cantos-server/contracts/theatre/discovery/v1/discovery.schema.json"
);
const METADATA: &str = include_str!(
    "../../../services/cantos-server/contracts/theatre/discovery/v1/metadata.schema.json"
);
const LIBRARY: &str = include_str!(
    "../../../services/cantos-server/contracts/theatre/library/v1/library.schema.json"
);

/// The composed host: the same assembly `cantos-server` runs, over this process's store.
struct Host {
    router: Router,
}

fn identity_auth(env: &IdentityEnv) -> IdentityAuth {
    let config = config::from_lookup(|name| match name {
        config::SOCKET => Some(env.socket.to_str().unwrap().to_owned()),
        config::PEER_UID => Some(env.uid.to_string()),
        config::PEER_POLICY => Some("direct_peer".to_owned()),
        _ => None,
    })
    .unwrap()
    .expect("the Identity inputs are complete");
    IdentityAuth::new(config, true)
}

fn compose(fixture: &ServerFixture, env: &IdentityEnv) -> Host {
    let storage = Arc::new(
        FilesystemPublicationStorage::new(&fixture.dir().join("publication"))
            .expect("the publication root opens"),
    );
    let origin = OriginPlayback::new(
        PublicMediaOrigin::parse("https://api.example").unwrap(),
        GrantKey::new(&[7; 32]).unwrap(),
        storage.clone(),
    );
    let store = fixture
        .store()
        .clone()
        .with_identity_auth(identity_auth(env));
    let publication = PublicationService::new(store.clone()).with_origin_publication(storage);
    let origins = SurfaceOrigins::new("https://studio.example", THEATRE, None).unwrap();
    Host {
        router: server_router(
            AppState {
                store,
                web_origin: THEATRE.into(),
            },
            &origins,
            None,
            Some((publication, origin)),
        ),
    }
}

struct Account {
    access: String,
    refresh: String,
    session: String,
    actor: String,
}

/// Sign up through the mailed link, then log in natively. Every credential is earned.
async fn enroll(host: &Host, sink: &SmtpSink, email: &str, mail_number: usize) -> Account {
    let start = send(
        &host.router,
        "POST",
        &theatre("/native/identity/signup"),
        &[],
        json_body(&json!({"email": email})),
    )
    .await;
    assert_eq!(
        start.status,
        StatusCode::ACCEPTED,
        "{}",
        String::from_utf8_lossy(&start.body)
    );
    let mail = sink.wait_for(mail_number).await.remove(mail_number - 1);
    assert_eq!(mail.to, [email]);
    let (_, link) = mail.link();
    let complete = send(
        &host.router,
        "POST",
        &theatre("/native/identity/signup/complete"),
        &[],
        json_body(&json!({
            "operation_id": link.operation,
            "proof_id": link.proof,
            "proof_secret": link.secret,
            "password": PASSWORD,
            "explicit_confirmation": true,
        })),
    )
    .await;
    assert_eq!(
        complete.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&complete.body)
    );
    let login = send(
        &host.router,
        "POST",
        &theatre("/native/identity/login"),
        &[],
        json_body(&json!({"email": email, "password": PASSWORD})),
    )
    .await;
    assert_eq!(
        login.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&login.body)
    );
    let pair = login.json();
    Account {
        access: pair["access_token"].as_str().unwrap().to_owned(),
        refresh: pair["refresh_token"].as_str().unwrap().to_owned(),
        session: pair["session_id"].as_str().unwrap().to_owned(),
        actor: pair["actor_id"].as_str().unwrap().to_owned(),
    }
}

async fn world(fixture: &ServerFixture) -> SeededWork {
    let storage = FilesystemPublicationStorage::new(&fixture.dir().join("publication")).unwrap();
    fixture
        .seed_published_work(
            Some(&storage),
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một lời hẹn", "Tập hai"]),
        )
        .await
}

async fn rows(fixture: &ServerFixture, who: &str) -> Vec<i64> {
    let mut out = Vec::new();
    for table in [
        "listener_state",
        "listener_progress",
        "listener_saved",
        "listener_history",
        "listener_bookmarks",
        "listener_operations",
        "listener_device_sequences",
        "listener_playback_sessions",
    ] {
        out.push(
            fixture
                .count(
                    &format!("SELECT count(*) FROM {table} WHERE listener_id=?1"),
                    &[&who],
                )
                .await,
        );
    }
    out
}

fn supplied_identity() -> &'static Path {
    process::binary()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_to_playback_to_library_over_the_production_composition_with_a_real_identity() {
    let _ = supplied_identity();
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let identity: Instance = env.serve().await;
    let mut fixture = ServerFixture::new().await;
    let work = world(&fixture).await;
    let unit = &work.units[0];
    let release = unit.chain.release_id.clone().unwrap();
    let a = audio(&work, unit);
    let mut host = compose(&fixture, &env);

    // ---- anonymous catalog, manifest, authorization and media ------------------------------
    let works = send(&host.router, "GET", &theatre("/works"), &[], None).await;
    assert_eq!(works.status, StatusCode::OK);
    conforms(METADATA, "works", &works.json());
    let page = works.json();
    assert_eq!(page["items"].as_array().unwrap().len(), 1, "{page}");
    assert_eq!(page["items"][0]["work_id"], work.work_id);
    assert_eq!(page["items"][0]["title"], "Ánh đèn cuối sân khấu");
    assert_eq!(page["items"][0]["unit_count"], 2);
    assert_eq!(works.header("cache-control"), Some("no-store"));
    let retired = send(
        &host.router,
        "GET",
        &theatre("/works?presentation=1"),
        &[],
        None,
    )
    .await;
    assert_eq!(retired.status, StatusCode::BAD_REQUEST);
    let found = send(
        &host.router,
        "GET",
        &theatre("/search?q=%C3%81NH%20%C4%90%C3%88N"),
        &[],
        None,
    )
    .await;
    assert_eq!(found.json()["items"][0]["work_id"], work.work_id);
    let detail = send(
        &host.router,
        "GET",
        &theatre(&format!("/works/{}", work.work_id)),
        &[],
        None,
    )
    .await;
    assert_eq!(detail.json()["work_id"], work.work_id);
    let units = send(
        &host.router,
        "GET",
        &theatre(&format!("/works/{}/units", work.work_id)),
        &[],
        None,
    )
    .await;
    assert_eq!(units.status, StatusCode::OK);
    conforms(METADATA, "units", &units.json());
    let listed = units.json();
    assert_eq!(listed["items"].as_array().unwrap().len(), 2);
    assert_eq!(listed["items"][0]["unit_id"], unit.unit_id);
    assert_eq!(listed["items"][0]["playback"]["publication_id"], release);
    let manifest = send(
        &host.router,
        "GET",
        &theatre(&format!("/publications/{release}")),
        &[],
        None,
    )
    .await;
    assert_eq!(manifest.status, StatusCode::OK);
    conforms(DISCOVERY, "PublicationManifest", &manifest.json());
    assert_eq!(manifest.json()["sha256"], unit.chain.sha256);
    assert_eq!(manifest.json()["byte_length"], 4096);
    let grant = send(
        &host.router,
        "POST",
        &theatre(&format!("/publications/{release}/playback-authorizations")),
        &[],
        Some(b"{}".to_vec()),
    )
    .await;
    assert_eq!(
        grant.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&grant.body)
    );
    conforms(DISCOVERY, "PlaybackAuthorization", &grant.json());
    let grant = grant.json();
    assert_eq!(grant["sha256"], unit.chain.sha256);
    let url = grant["url"].as_str().unwrap();
    let path = url
        .strip_prefix("https://api.example")
        .expect("the configured origin");
    // Production media route: HEAD, a Range, a validator, an unsatisfiable range and the whole.
    let head = send(&host.router, "HEAD", path, &[], None).await;
    assert_eq!(head.status, StatusCode::OK);
    assert!(head.body.is_empty());
    assert_eq!(head.header("content-length"), Some("4096"));
    assert_eq!(head.header("accept-ranges"), Some("bytes"));
    assert_eq!(head.header("content-type"), Some("audio/ogg"));
    let etag = head.header("etag").expect("a validator").to_owned();
    assert_eq!(etag, format!("\"{}\"", unit.chain.sha256));
    let ranged = send(
        &host.router,
        "GET",
        path,
        &[("range", "bytes=100-199".into())],
        None,
    )
    .await;
    assert_eq!(ranged.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(ranged.header("content-range"), Some("bytes 100-199/4096"));
    assert_eq!(ranged.body, unit.chain.bytes[100..200]);
    let cached = send(
        &host.router,
        "GET",
        path,
        &[("if-none-match", etag.clone())],
        None,
    )
    .await;
    assert_eq!(cached.status, StatusCode::NOT_MODIFIED);
    assert!(cached.body.is_empty());
    let beyond = send(
        &host.router,
        "GET",
        path,
        &[("range", "bytes=5000-6000".into())],
        None,
    )
    .await;
    assert_eq!(beyond.status, StatusCode::RANGE_NOT_SATISFIABLE);
    let whole = send(&host.router, "GET", path, &[], None).await;
    assert_eq!(whole.status, StatusCode::OK);
    assert_eq!(whole.body, unit.chain.bytes);
    assert!(whole.header("cache-control").unwrap().contains("private"));
    let forged = send(&host.router, "GET", &theatre("/media/forged"), &[], None).await;
    assert_eq!(forged.status, StatusCode::NOT_FOUND);

    // ---- earn a session ----------------------------------------------------------------------
    let alice = enroll(&host, &sink, OWNER, 1).await;
    let bob = enroll(&host, &sink, SECOND, 2).await;
    assert_ne!(alice.actor, bob.actor);
    // Account routes refuse every anonymous or unusable credential before any effect.
    for (method, path, body) in [
        ("GET", "/library/continue".to_owned(), None),
        ("GET", "/library/saved".to_owned(), None),
        ("GET", "/library/history".to_owned(), None),
        ("GET", "/library/bookmarks".to_owned(), None),
        ("GET", format!("/progress/{release}"), None),
        (
            "GET",
            "/operations/00000000-0000-4000-8000-000000000001".to_owned(),
            None,
        ),
        (
            "PUT",
            format!("/library/saved/{}", work.work_id),
            json_body(&saved(1, 90, 0, true)),
        ),
        (
            "PUT",
            format!("/progress/{release}"),
            json_body(&progress(&a, 1, 90, 0, 5, dto::ProgressKind::Checkpoint)),
        ),
    ] {
        for credentials in [
            vec![],
            vec![bearer("not-a-token")],
            vec![bearer(&format!("ci1_n_{}", "0".repeat(64)))],
            vec![bearer(&alice.refresh)],
            vec![("authorization", "Basic YWxpY2U6cGFzc3dvcmQ=".into())],
            vec![("cookie", "cantos_session=dev".into())],
            // Studio and development credentials never authorize a listener.
            vec![bearer(&"a".repeat(64))],
        ] {
            let reply = send(
                &host.router,
                method,
                &theatre(&path),
                &credentials,
                body.clone(),
            )
            .await;
            assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{method} {path}");
            assert_eq!(
                reply.json(),
                json!({"code":"unauthenticated","retryable":false})
            );
        }
    }
    assert_eq!(
        rows(&fixture, &alice.actor).await,
        vec![0; 8],
        "nothing was written"
    );

    // ---- the Library, with the session fence -------------------------------------------------
    let bearer_a = bearer(&alice.access);
    let save = saved(1, 90, 0, true);
    let saved_reply = send(
        &host.router,
        "PUT",
        &theatre(&format!("/library/saved/{}", work.work_id)),
        &[
            bearer_a.clone(),
            ("x-cantos-theatre-session", alice.session.clone()),
        ],
        json_body(&save),
    )
    .await;
    assert_eq!(
        saved_reply.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&saved_reply.body)
    );
    assert_eq!(
        saved_reply.header("x-cantos-theatre-session"),
        Some(alice.session.as_str())
    );
    assert_eq!(saved_reply.header("cache-control"), Some("no-store"));
    conforms(LIBRARY, "Receipt", &saved_reply.json());
    assert_eq!(saved_reply.json()["outcome"], "accepted");
    // The fence names the session the client believes it holds: another family is refused before
    // any effect, and the answer names the session that was actually admitted.
    let wrong = send(
        &host.router,
        "PUT",
        &theatre(&format!("/progress/{release}")),
        &[
            bearer_a.clone(),
            ("x-cantos-theatre-session", bob.session.clone()),
        ],
        json_body(&progress(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint)),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::CONFLICT);
    assert_eq!(
        wrong.header("x-cantos-theatre-session"),
        Some(alice.session.as_str())
    );
    assert_eq!(
        rows(&fixture, &alice.actor).await[1],
        0,
        "the fenced write left no progress"
    );
    let write = progress(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint);
    let progress_reply = send(
        &host.router,
        "PUT",
        &theatre(&format!("/progress/{release}")),
        std::slice::from_ref(&bearer_a),
        json_body(&write),
    )
    .await;
    assert_eq!(progress_reply.status, StatusCode::OK);
    assert_eq!(progress_reply.json()["outcome"], "accepted");
    assert_eq!(
        progress_reply.json()["state"]["current"]["position_ms"],
        100
    );
    let again = send(
        &host.router,
        "PUT",
        &theatre(&format!("/progress/{release}")),
        std::slice::from_ref(&bearer_a),
        json_body(&write),
    )
    .await;
    assert_eq!(
        again.json(),
        progress_reply.json(),
        "a duplicate operation returns its receipt"
    );
    let start = history_start(&a, 1, 90, 99, 0);
    let started = send(
        &host.router,
        "POST",
        &theatre("/library/history/playback-starts"),
        std::slice::from_ref(&bearer_a),
        json_body(&start),
    )
    .await;
    assert_eq!(started.json()["outcome"], "accepted");
    let mark = bookmark(1, &a, 2, 90, 6000);
    let marked = send(
        &host.router,
        "PUT",
        &theatre(&format!("/library/bookmarks/{}", mark.bookmark_id)),
        std::slice::from_ref(&bearer_a),
        json_body(&mark),
    )
    .await;
    assert_eq!(marked.json()["outcome"], "accepted");
    // Bob writes his own progress and sees nothing of Alice's.
    let bobs = send(
        &host.router,
        "PUT",
        &theatre(&format!("/progress/{release}")),
        &[bearer(&bob.access)],
        json_body(&progress(&a, 1, 91, 0, 9, dto::ProgressKind::Checkpoint)),
    )
    .await;
    assert_eq!(bobs.json()["outcome"], "accepted");
    let foreign = send(
        &host.router,
        "GET",
        &theatre(&format!("/operations/{}", write.operation.operation_id)),
        &[bearer(&bob.access)],
        None,
    )
    .await;
    assert_eq!(foreign.status, StatusCode::NOT_FOUND);

    // ---- read the persisted outcomes back; close and reopen; read them again -------------------
    async fn read_back(router: &Router, token: &str, release: &str, op: &str) -> Vec<Value> {
        let auth = [bearer(token)];
        let mut out = Vec::new();
        for path in [
            "/library/continue?limit=20&presentation=1".to_owned(),
            "/library/saved?limit=20&presentation=1".to_owned(),
            "/library/history?limit=20&presentation=1".to_owned(),
            "/library/bookmarks?limit=20".to_owned(),
            format!("/progress/{release}"),
            format!("/operations/{op}"),
        ] {
            let reply = send(router, "GET", &theatre(&path), &auth, None).await;
            assert_eq!(
                reply.status,
                StatusCode::OK,
                "{path}: {}",
                String::from_utf8_lossy(&reply.body)
            );
            assert_eq!(reply.header("cache-control"), Some("no-store"));
            out.push(reply.json());
        }
        out
    }
    let before = read_back(
        &host.router,
        &alice.access,
        &release,
        &write.operation.operation_id,
    )
    .await;
    assert_eq!(before[0]["items"][0]["progress"]["position_ms"], 100);
    assert_eq!(before[0]["items"][0]["resume"], "exact_available");
    assert_eq!(
        before[0]["items"][0]["presentation"]["unit_title"],
        "Một lời hẹn"
    );
    assert_eq!(before[1]["items"][0]["saved"]["work_id"], work.work_id);
    assert_eq!(
        before[1]["items"][0]["presentation"]["title"],
        "Ánh đèn cuối sân khấu"
    );
    assert_eq!(before[2]["items"][0]["audio"]["publication_id"], release);
    assert_eq!(before[2]["items"][0]["progress"]["position_ms"], 100);
    assert_eq!(
        before[3]["items"][0]["bookmark"]["bookmark_id"],
        mark.bookmark_id
    );
    assert_eq!(before[3]["items"][0]["bookmark"]["position_ms"], 6000);
    assert_eq!(before[4]["current"]["revision"], 1);
    assert_eq!(before[5], progress_reply.json());
    conforms(LIBRARY, "PresentedContinuePage", &before[0]);
    conforms(LIBRARY, "PresentedSavedPage", &before[1]);
    conforms(LIBRARY, "PresentedHistoryPage", &before[2]);
    conforms(LIBRARY, "BookmarkPage", &before[3]);
    let alice_rows = rows(&fixture, &alice.actor).await;
    assert_eq!(alice_rows, vec![1, 1, 1, 1, 1, 4, 4, 1]);

    drop(host);
    fixture.restart().await;
    host = compose(&fixture, &env);
    assert_eq!(rows(&fixture, &alice.actor).await, alice_rows);
    let after = read_back(
        &host.router,
        &alice.access,
        &release,
        &write.operation.operation_id,
    )
    .await;
    assert_eq!(
        after, before,
        "a restart returns exactly the persisted outcomes"
    );
    // The same session, not a new login, still works, and the earlier receipt is still the answer.
    let replay = send(
        &host.router,
        "PUT",
        &theatre(&format!("/progress/{release}")),
        std::slice::from_ref(&bearer_a),
        json_body(&write),
    )
    .await;
    assert_eq!(replay.json(), progress_reply.json());

    // ---- history clear keeps Saved and progress; the old epoch cannot resurrect --------------
    let clear = history_clear(2, 90, 1, 0);
    let cleared = send(
        &host.router,
        "POST",
        &theatre("/library/history/clear"),
        std::slice::from_ref(&bearer_a),
        json_body(&clear),
    )
    .await;
    assert_eq!(cleared.json()["outcome"], "accepted");
    let history = send(
        &host.router,
        "GET",
        &theatre("/library/history"),
        std::slice::from_ref(&bearer_a),
        None,
    )
    .await;
    assert_eq!(history.json()["items"], json!([]));
    let late = send(
        &host.router,
        "POST",
        &theatre("/library/history/playback-starts"),
        std::slice::from_ref(&bearer_a),
        json_body(&history_start(&a, 3, 90, 100, 0)),
    )
    .await;
    assert_eq!(late.status, StatusCode::CONFLICT);
    assert_eq!(late.json()["outcome"], "history_epoch_conflict");
    let still = send(
        &host.router,
        "GET",
        &theatre("/library/saved"),
        std::slice::from_ref(&bearer_a),
        None,
    )
    .await;
    assert_eq!(still.json()["items"].as_array().unwrap().len(), 1);

    // ---- bookmark delete then replay never resurrects it --------------------------------------
    let removed = send(
        &host.router,
        "POST",
        &theatre(&format!("/library/bookmarks/{}/delete", mark.bookmark_id)),
        std::slice::from_ref(&bearer_a),
        json_body(&bookmark_delete(3, 90)),
    )
    .await;
    assert_eq!(removed.json()["outcome"], "accepted");
    let replayed = send(
        &host.router,
        "PUT",
        &theatre(&format!("/library/bookmarks/{}", mark.bookmark_id)),
        std::slice::from_ref(&bearer_a),
        json_body(&mark),
    )
    .await;
    assert_eq!(
        replayed.json(),
        marked.json(),
        "the original receipt, whatever happened since"
    );
    let recreate = send(
        &host.router,
        "PUT",
        &theatre(&format!("/library/bookmarks/{}", mark.bookmark_id)),
        std::slice::from_ref(&bearer_a),
        json_body(&bookmark(1, &a, 4, 90, 6000)),
    )
    .await;
    assert_eq!(recreate.status, StatusCode::CONFLICT);
    assert_eq!(recreate.json()["outcome"], "bookmark_deleted");
    let listed = send(
        &host.router,
        "GET",
        &theatre("/library/bookmarks"),
        std::slice::from_ref(&bearer_a),
        None,
    )
    .await;
    assert_eq!(listed.json()["items"], json!([]));

    // ---- authority never leaks across kinds or accounts --------------------------------------
    // A Web cookie session is a different family: its secret is no Bearer, and a Bearer is no cookie.
    let web = send(
        &host.router,
        "POST",
        &theatre("/identity/login"),
        &[("origin", THEATRE.into())],
        json_body(&json!({"email": OWNER, "password": PASSWORD})),
    )
    .await;
    assert_eq!(
        web.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&web.body)
    );
    let cookie = web
        .headers
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let cookie_secret = cookie.split_once('=').unwrap().1.to_owned();

    // The display projection reuses production cookie/Bearer admission without changing Session.
    let display_cookie = send(
        &host.router,
        "GET",
        &theatre("/account/presentation"),
        &[("cookie", cookie.clone())],
        None,
    )
    .await;
    let display_native = send(
        &host.router,
        "GET",
        &theatre("/account/presentation"),
        &[bearer(&alice.access)],
        None,
    )
    .await;
    assert_eq!(display_cookie.status, StatusCode::OK);
    assert_eq!(display_native.status, StatusCode::OK);
    assert_eq!(display_cookie.json(), display_native.json());
    assert_eq!(display_cookie.header("cache-control"), Some("no-store"));
    assert_eq!(
        display_native.header("x-cantos-theatre-session"),
        Some(alice.session.as_str())
    );
    assert_eq!(display_native.json()["version"], "generated-alias-v1");
    assert!(!String::from_utf8_lossy(&display_native.body).contains(&alice.actor));
    let schema: Value = serde_json::from_str(include_str!("../../../services/cantos-server/contracts/theatre/account-presentation/v1/account-presentation.schema.json")).unwrap();
    assert!(jsonschema::validator_for(&schema)
        .unwrap()
        .is_valid(&display_native.json()));
    for headers in [
        vec![],
        vec![("cookie", cookie.clone()), bearer(&alice.access)],
        vec![
            ("cookie", cookie.clone()),
            ("x-cantos-theatre-session", bob.session.clone()),
        ],
    ] {
        let refused = send(
            &host.router,
            "GET",
            &theatre("/account/presentation"),
            &headers,
            None,
        )
        .await;
        assert!(matches!(
            refused.status,
            StatusCode::UNAUTHORIZED | StatusCode::CONFLICT
        ));
        assert!(!String::from_utf8_lossy(&refused.body).contains("generated-alias-v1"));
    }

    let by_cookie = send(
        &host.router,
        "GET",
        &theatre("/library/saved"),
        &[("cookie", cookie.clone())],
        None,
    )
    .await;
    assert_eq!(
        by_cookie.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&by_cookie.body)
    );
    let cookie_as_bearer = send(
        &host.router,
        "GET",
        &theatre("/library/saved"),
        &[bearer(&cookie_secret)],
        None,
    )
    .await;
    assert_eq!(cookie_as_bearer.status, StatusCode::UNAUTHORIZED);
    let bearer_as_cookie = send(
        &host.router,
        "GET",
        &theatre("/library/saved"),
        &[(
            "cookie",
            format!("__Secure-cantos_id_theatre={}", alice.access),
        )],
        None,
    )
    .await;
    assert_eq!(bearer_as_cookie.status, StatusCode::UNAUTHORIZED);
    let both = send(
        &host.router,
        "GET",
        &theatre("/library/saved"),
        &[("cookie", cookie.clone()), bearer(&alice.access)],
        None,
    )
    .await;
    assert_eq!(
        both.status,
        StatusCode::UNAUTHORIZED,
        "a mixed request is refused, not retried"
    );
    let cookie_write_without_origin = send(
        &host.router,
        "PUT",
        &theatre(&format!("/library/saved/{}", work.work_id)),
        &[("cookie", cookie.clone())],
        json_body(&saved(9, 90, 1, false)),
    )
    .await;
    assert_eq!(cookie_write_without_origin.status, StatusCode::UNAUTHORIZED);

    // ---- Identity outage is an honest 503 and grants nothing ---------------------------------
    let before_outage = rows(&fixture, &alice.actor).await;
    identity.terminate().await;
    for (method, path, body) in [
        ("GET", "/library/continue".to_owned(), None),
        (
            "PUT",
            format!("/library/saved/{}", work.work_id),
            json_body(&saved(2, 90, 1, false)),
        ),
    ] {
        let reply = send(
            &host.router,
            method,
            &theatre(&path),
            std::slice::from_ref(&bearer_a),
            body,
        )
        .await;
        assert_eq!(
            reply.status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}"
        );
        assert_eq!(reply.json(), json!({"code":"unavailable","retryable":true}));
        assert!(
            reply.header("x-cantos-theatre-session").is_none(),
            "an unverified session is never echoed"
        );
    }
    assert_eq!(rows(&fixture, &alice.actor).await, before_outage);
    // Anonymous discovery and media need no Identity and keep serving.
    assert_eq!(
        send(&host.router, "GET", &theatre("/works"), &[], None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        send(&host.router, "GET", path, &[], None).await.body,
        unit.chain.bytes
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn logout_ends_the_session_and_a_later_write_changes_nothing() {
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let _identity = env.serve().await;
    let fixture = ServerFixture::new().await;
    let work = world(&fixture).await;
    let a = audio(&work, &work.units[0]);
    let release = a.publication_id.clone();
    let host = compose(&fixture, &env);
    let alice = enroll(&host, &sink, OWNER, 1).await;
    let auth = bearer(&alice.access);
    let first = send(
        &host.router,
        "PUT",
        &theatre(&format!("/progress/{release}")),
        std::slice::from_ref(&auth),
        json_body(&progress(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint)),
    )
    .await;
    assert_eq!(first.json()["outcome"], "accepted");
    let after_write = rows(&fixture, &alice.actor).await;
    let out = send(
        &host.router,
        "DELETE",
        &theatre("/native/identity/session"),
        std::slice::from_ref(&auth),
        None,
    )
    .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    for (method, path, body) in [
        ("GET", "/library/continue".to_owned(), None),
        (
            "PUT",
            format!("/progress/{release}"),
            json_body(&progress(
                &a,
                2,
                90,
                1,
                700,
                dto::ProgressKind::ExplicitSeek,
            )),
        ),
        (
            "PUT",
            format!("/library/saved/{}", work.work_id),
            json_body(&saved(1, 90, 0, true)),
        ),
    ] {
        let reply = send(
            &host.router,
            method,
            &theatre(&path),
            std::slice::from_ref(&auth),
            body,
        )
        .await;
        assert_eq!(reply.status, StatusCode::UNAUTHORIZED, "{method} {path}");
    }
    assert_eq!(
        rows(&fixture, &alice.actor).await,
        after_write,
        "the durable receipt stays, nothing is added"
    );
    // A new login of the same account reads back the receipt the earlier session wrote.
    let again = enroll_login(&host, OWNER).await;
    let receipt = send(
        &host.router,
        "GET",
        &theatre(&format!(
            "/operations/{}",
            first.json()["operation_id"].as_str().unwrap()
        )),
        &[bearer(&again)],
        None,
    )
    .await;
    assert_eq!(receipt.status, StatusCode::OK);
    assert_eq!(receipt.json(), first.json());
}

async fn enroll_login(host: &Host, email: &str) -> String {
    let login = send(
        &host.router,
        "POST",
        &theatre("/native/identity/login"),
        &[],
        json_body(&json!({"email": email, "password": PASSWORD})),
    )
    .await;
    assert_eq!(login.status, StatusCode::OK);
    login.json()["access_token"].as_str().unwrap().to_owned()
}

/// The real CMP authorization owner and HTTP boundary, not the trusted-host fixture lane.
/// Explicit because it needs the installed Gradle/JDK toolchain; secure storage/audio remain doubles.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires installed Gradle/JDK and the real cantos-identity executable"]
async fn cmp_authorized_library_against_real_identity_and_http() {
    use std::{process::Stdio, time::Duration};

    async fn signal(path: &Path) {
        tokio::time::timeout(Duration::from_secs(120), async {
            while !path.is_file() {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("CMP control signal arrived in time");
    }

    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let identity = env.serve().await;
    let fixture = ServerFixture::new().await;
    let work = world(&fixture).await;
    let a = audio(&work, &work.units[0]);
    let host = compose(&fixture, &env);
    let alice = enroll(&host, &sink, OWNER, 1).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            host.router
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });
    let control = run.path("cmp-control");
    std::fs::create_dir(&control).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let gradle = std::env::var("CANTOS_TEST_GRADLE").unwrap_or_else(|_| "gradle".into());
    let mut command = tokio::process::Command::new(gradle);
    command
        .current_dir(&root)
        .args([
            "--project-dir",
            "apps/cantos-theatre/mobile",
            "--no-daemon",
            "-Dorg.gradle.java.installations.auto-download=false",
            "--max-workers",
            "2",
            "-Pkotlin.compiler.execution.strategy=in-process",
            "--no-configuration-cache",
            "-i",
            ":shared:desktopTest",
            "--rerun",
            "--tests",
            "com.cantos.theatre.adapters.library.LiveAuthorizedLibraryApiTest",
        ])
        .env(
            "CANTOS_AUTHORIZED_API_ORIGIN",
            format!("http://127.0.0.1:{port}"),
        )
        .env("CANTOS_AUTHORIZED_CONTROL", &control)
        .env("CANTOS_LIVE_AUDIO", serde_json::to_string(&a).unwrap())
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = command.output();
    tokio::pin!(output);
    let stop_requested = control.join("stop-requested");
    tokio::select! {
        early = &mut output => panic!("CMP exited before outage control: {early:?}"),
        () = signal(&stop_requested) => {},
    }
    identity.terminate().await;
    std::fs::write(control.join("stopped"), b"stopped").unwrap();
    let resume_requested = control.join("resume-requested");
    tokio::select! {
        early = &mut output => panic!("CMP exited before recovery control: {early:?}"),
        () = signal(&resume_requested) => {},
    }
    let resumed_identity = env.serve().await;
    std::fs::write(control.join("restored"), b"restored").unwrap();
    let output = tokio::time::timeout(Duration::from_secs(90), output)
        .await
        .expect("CMP completed after recovery")
        .expect("Gradle started");
    server.abort();
    resumed_identity.terminate().await;
    let log = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{log}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let marker = log
        .lines()
        .find(|line| line.contains("LIVE-AUTHORIZED-LIBRARY-OK"));
    assert!(marker.is_some(), "the explicit CMP test was skipped: {log}");
    println!("{}", marker.unwrap().trim());
    assert_eq!(fixture.bookmark_rows(&alice.actor, false).await, 1);
    assert_eq!(
        fixture
            .bookmark_anomalies(&alice.actor, &a.publication_id)
            .await,
        (0, 0)
    );
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM listener_saved WHERE listener_id=?1 AND saved=1",
                &[&alice.actor]
            )
            .await,
        1
    );
}

/// The opt-in live Desktop Preview composition (`LiveEnrollmentOwners`: shell, real Identity login, authorized
/// Library, real public catalog) over the real Server and Identity process. Explicit: it needs the installed
/// Gradle/JDK and the real `cantos-identity` executable. The preview's audio stays unavailable by design, secure
/// storage is in memory and the Library store is an in-memory SQLDelight database; no GUI window is driven.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires installed Gradle/JDK and the real cantos-identity executable"]
async fn cmp_live_desktop_preview_against_real_identity_and_http() {
    use std::{process::Stdio, time::Duration};

    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let identity = env.serve().await;
    let fixture = ServerFixture::new().await;
    let work = world(&fixture).await;
    let a = audio(&work, &work.units[0]);
    let host = compose(&fixture, &env);
    let alice = enroll(&host, &sink, OWNER, 1).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            host.router
                .into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let gradle = std::env::var("CANTOS_TEST_GRADLE").unwrap_or_else(|_| "gradle".into());
    let output = tokio::process::Command::new(gradle)
        .current_dir(&root)
        .args([
            "--project-dir",
            "apps/cantos-theatre/mobile",
            "--no-daemon",
            "-Dorg.gradle.java.installations.auto-download=false",
            "--max-workers",
            "2",
            "-Pkotlin.compiler.execution.strategy=in-process",
            "--no-configuration-cache",
            "-i",
            ":preview-sandbox:desktopTest",
            "--rerun",
            "--tests",
            "com.cantos.theatre.app.LiveDesktopPreviewRealServerTest",
        ])
        .env(
            "CANTOS_AUTHORIZED_API_ORIGIN",
            format!("http://127.0.0.1:{port}"),
        )
        .env("CANTOS_LIVE_AUDIO", serde_json::to_string(&a).unwrap())
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(Duration::from_secs(300), output)
        .await
        .expect("the live Desktop Preview test completed")
        .expect("Gradle started");
    server.abort();
    identity.terminate().await;
    let log = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "{log}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        log.lines()
            .any(|line| line.contains("LIVE-DESKTOP-PREVIEW-OK")),
        "the explicit live Desktop Preview test was skipped: {log}"
    );
    println!("LIVE-DESKTOP-PREVIEW-OK");
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM listener_saved WHERE listener_id=?1 AND saved=1",
                &[&alice.actor]
            )
            .await,
        1
    );
    assert_eq!(fixture.bookmark_rows(&alice.actor, false).await, 1);
    assert_eq!(
        fixture
            .bookmark_anomalies(&alice.actor, &a.publication_id)
            .await,
        (0, 0)
    );
}

#[path = "support/restricted_journey.rs"]
mod restricted;
