//! #333 CMP business API over **loopback HTTP between OS processes**: the `cantos-server` binary
//! (its real `main`, environment parsing, `Intent::Serve` open, host settings and route policies),
//! the real `cantos-identity` executable and this test as the client. The database is a real
//! file-backed `server.db` migrated and seeded by this process, closed, and then owned by the
//! server process; a second server process proves the persisted outcomes survive a restart.
//!
//! Not established here: a creator -> publication journey (the catalog is seeded with valid
//! domain values), Ogg/Opus decoding, TLS or a proxy, a CDN, a device or the CMP client itself.
//! The CMP client is covered by its own desktop tests against recorded wire fixtures.
#[path = "support/identity_process.rs"]
mod process;

use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use cantos_server::test_support::{
    library::{audio, bookmark, history_start, progress, saved},
    ServerFixture, WorkSeed,
};
use process::{IdentityEnv, RunDir, SmtpSink};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

const PASSWORD: &str = "a correct horse battery";

struct Server {
    child: Child,
    base: String,
}

impl Server {
    async fn start(
        database: &Path,
        env: &IdentityEnv,
        publication: &Path,
        key: &Path,
    ) -> Result<Self, String> {
        // Reserve a loopback port; the server binds it itself right after.
        let port = {
            let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            probe.local_addr().unwrap().port()
        };
        let mut child = Command::new(env!("CARGO_BIN_EXE_cantos-server"))
            .env_clear()
            .env("CANTOS_ENV", "development")
            .env("CANTOS_BIND_ADDRESS", format!("127.0.0.1:{port}"))
            .env("CANTOS_SERVER_DATABASE", database)
            .env("CANTOS_IDENTITY_SOCKET", &env.socket)
            .env("CANTOS_IDENTITY_PEER_UID", env.uid.to_string())
            .env("CANTOS_IDENTITY_PEER_POLICY", "direct_peer")
            .env("CANTOS_PUBLICATION_ROOT", publication)
            .env("CANTOS_PUBLIC_MEDIA_ORIGIN", "https://api.example")
            .env("CANTOS_PLAYBACK_KEY_FILE", key)
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
        let ready = tokio::time::timeout(Duration::from_secs(30), async {
            while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
                if line.contains("development host:") {
                    return Ok(line);
                }
                // Anything before the readiness line is the refusal the operator would read.
                if line.contains("rror") || line.contains("required") || line.contains("refus") {
                    return Err(line);
                }
            }
            Err("the server exited before it was ready".to_owned())
        })
        .await
        .map_err(|_| "the server was not ready in time".to_owned())??;
        assert!(ready.contains(&port.to_string()), "{ready}");
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
        Ok(Self {
            child,
            base: format!("http://127.0.0.1:{port}"),
        })
    }

    /// SIGTERM and a bounded wait: the graceful path a service manager uses.
    async fn terminate(mut self) {
        let pid = libc::pid_t::try_from(self.child.id().expect("not yet reaped")).unwrap();
        // SAFETY: kill(2) takes no pointers; the target is our own unreaped child.
        assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
        let status = tokio::time::timeout(Duration::from_secs(30), self.child.wait())
            .await
            .expect("the server drains and exits after SIGTERM")
            .unwrap();
        assert!(status.success(), "{status:?}");
    }
}

struct Http {
    client: reqwest::Client,
    base: String,
}

impl Http {
    fn new(base: &str) -> Self {
        Self {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            base: base.to_owned(),
        }
    }
    async fn call(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        extra: &[(&str, &str)],
        body: Option<Value>,
    ) -> (u16, reqwest::header::HeaderMap, Vec<u8>) {
        let mut request = self.client.request(
            method.parse().unwrap(),
            format!("{}/api/v1/theatre{path}", self.base),
        );
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        for (name, value) in extra {
            request = request.header(*name, *value);
        }
        if let Some(body) = body {
            request = request
                .header("content-type", "application/json")
                .body(serde_json::to_vec(&body).unwrap());
        }
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        (status, headers, response.bytes().await.unwrap().to_vec())
    }
    async fn json(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (u16, Value) {
        let (status, _, bytes) = self.call(method, path, token, &[], body).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

fn to_json(value: &impl serde::Serialize) -> Option<Value> {
    Some(serde_json::to_value(value).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_server_binary_serves_the_cmp_slice_over_loopback_and_a_second_process_reads_the_same_outcomes(
) {
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let _identity = env.serve().await;

    // The explicit operator steps first: migrate and seed as this process, then hand the file over.
    let mut fixture = ServerFixture::new().await;
    let root = std::fs::canonicalize(fixture.dir()).unwrap();
    let publication: PathBuf = root.join("publication");
    let storage =
        cantos_server::contexts::media::FilesystemPublicationStorage::new(&publication).unwrap();
    let work = fixture
        .seed_work(
            Some(&storage),
            &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Một lời hẹn"]),
        )
        .await;
    drop(storage);
    let unit = &work.units[0];
    let release = unit.chain.release_id.clone().unwrap();
    let a = audio(&work, unit);
    let key = root.join("playback.key");
    std::fs::write(&key, [7_u8; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let database = fixture.database_path();
    fixture.close();

    // A file that was never migrated is refused by the serving process, which creates nothing.
    let missing = root.join("never-migrated.db");
    let refusal = Server::start(&missing, &env, &publication, &key).await;
    assert!(
        refusal.is_err(),
        "serving must not create or adopt an unmigrated file"
    );
    assert!(!missing.exists());

    let server = Server::start(&database, &env, &publication, &key)
        .await
        .unwrap();
    let http = Http::new(&server.base);

    // Anonymous catalog, manifest, authorization and the production media route.
    let (status, works) = http.json("GET", "/works", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(works["items"][0]["work_id"], work.work_id);
    let (_, units) = http
        .json("GET", &format!("/works/{}/units", work.work_id), None, None)
        .await;
    assert_eq!(units["items"][0]["publication_id"], release);
    let (status, manifest) = http
        .json("GET", &format!("/publications/{release}"), None, None)
        .await;
    assert_eq!(
        (status, manifest["sha256"].as_str()),
        (200, Some(unit.chain.sha256.as_str()))
    );
    let (status, grant) = http
        .json(
            "POST",
            &format!("/publications/{release}/playback-authorizations"),
            None,
            Some(json!({})),
        )
        .await;
    assert_eq!(status, 200, "{grant}");
    let media = grant["url"]
        .as_str()
        .unwrap()
        .strip_prefix("https://api.example/api/v1/theatre")
        .unwrap()
        .to_owned();
    let (status, headers, bytes) = http
        .call("GET", &media, None, &[("range", "bytes=100-199")], None)
        .await;
    assert_eq!(status, 206);
    assert_eq!(headers["content-range"], "bytes 100-199/4096");
    assert_eq!(bytes, unit.chain.bytes[100..200]);
    let (status, _, whole) = http.call("GET", &media, None, &[], None).await;
    assert_eq!((status, whole), (200, unit.chain.bytes.clone()));

    // Earn a session with the real Identity, then use the Library.
    let (status, _) = http
        .json(
            "POST",
            "/native/identity/signup",
            None,
            Some(json!({"email":"owner@example.com"})),
        )
        .await;
    assert_eq!(status, 202);
    let (_, link) = sink.wait_for(1).await.remove(0).link();
    let (status, _) = http
        .json(
            "POST",
            "/native/identity/signup/complete",
            None,
            Some(json!({"operation_id":link.operation,"proof_id":link.proof,"proof_secret":link.secret,"password":PASSWORD,"explicit_confirmation":true})),
        )
        .await;
    assert_eq!(status, 200);
    let (status, pair) = http
        .json(
            "POST",
            "/native/identity/login",
            None,
            Some(json!({"email":"owner@example.com","password":PASSWORD})),
        )
        .await;
    assert_eq!(status, 200, "{pair}");
    let token = pair["access_token"].as_str().unwrap().to_owned();
    let session = pair["session_id"].as_str().unwrap().to_owned();

    // No credential, no write.
    let (status, error) = http
        .json(
            "PUT",
            &format!("/library/saved/{}", work.work_id),
            None,
            to_json(&saved(1, 90, 0, true)),
        )
        .await;
    assert_eq!(
        (status, error["code"].as_str()),
        (401, Some("unauthenticated"))
    );

    let (status, saved_receipt) = http
        .json(
            "PUT",
            &format!("/library/saved/{}", work.work_id),
            Some(&token),
            to_json(&saved(1, 90, 0, true)),
        )
        .await;
    assert_eq!(
        (status, saved_receipt["outcome"].as_str()),
        (200, Some("accepted"))
    );
    let write = progress(&a, 1, 90, 0, 100, dto_kind());
    let (status, progress_receipt) = http
        .json(
            "PUT",
            &format!("/progress/{release}"),
            Some(&token),
            to_json(&write),
        )
        .await;
    assert_eq!(
        (status, progress_receipt["outcome"].as_str()),
        (200, Some("accepted"))
    );
    let (status, _) = http
        .json(
            "POST",
            "/library/history/playback-starts",
            Some(&token),
            to_json(&history_start(&a, 1, 90, 99, 0)),
        )
        .await;
    assert_eq!(status, 200);
    let mark = bookmark(1, &a, 2, 90, 6000);
    let (status, _) = http
        .json(
            "PUT",
            &format!("/library/bookmarks/{}", mark.bookmark_id),
            Some(&token),
            to_json(&mark),
        )
        .await;
    assert_eq!(status, 200);

    async fn read_back(http: &Http, token: &str, release: &str, op: &str) -> Vec<Value> {
        let mut out = Vec::new();
        for path in [
            "/library/continue?limit=20&presentation=1".to_owned(),
            "/library/saved?limit=20&presentation=1".to_owned(),
            "/library/history?limit=20&presentation=1".to_owned(),
            "/library/bookmarks?limit=20".to_owned(),
            format!("/progress/{release}"),
            format!("/operations/{op}"),
        ] {
            let (status, value) = http.json("GET", &path, Some(token), None).await;
            assert_eq!(status, 200, "{path}: {value}");
            out.push(value);
        }
        out
    }
    let before = read_back(&http, &token, &release, &write.operation.operation_id).await;
    assert_eq!(before[0]["items"][0]["progress"]["position_ms"], 100);
    assert_eq!(before[1]["items"][0]["saved"]["work_id"], work.work_id);
    assert_eq!(before[3]["items"][0]["bookmark"]["position_ms"], 6000);
    assert_eq!(before[5], progress_receipt);

    // A real process restart: the server stops gracefully, a new process owns the same file.
    server.terminate().await;
    let server = Server::start(&database, &env, &publication, &key)
        .await
        .unwrap();
    let http = Http::new(&server.base);
    let after = read_back(&http, &token, &release, &write.operation.operation_id).await;
    assert_eq!(
        after, before,
        "the new process returns exactly the persisted outcomes"
    );
    let (status, replay) = http
        .json(
            "PUT",
            &format!("/progress/{release}"),
            Some(&token),
            to_json(&write),
        )
        .await;
    assert_eq!((status, replay), (200, progress_receipt));
    // The session survived with its row; logout ends it for the next request.
    let (status, _, _) = http
        .call(
            "DELETE",
            "/native/identity/session",
            Some(&token),
            &[("x-cantos-theatre-session", &session)],
            None,
        )
        .await;
    assert_eq!(status, 204);
    let (status, error) = http
        .json("GET", "/library/continue", Some(&token), None)
        .await;
    assert_eq!(
        (status, error["code"].as_str()),
        (401, Some("unauthenticated"))
    );
    server.terminate().await;

    // The file the processes shared is intact and holds exactly the account's rows.
    fixture.restart().await;
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM listener_operations", &[])
            .await,
        4
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM listener_progress", &[])
            .await,
        1
    );
}

fn dto_kind() -> cantos_contracts::listener_library::ProgressKind {
    cantos_contracts::listener_library::ProgressKind::Checkpoint
}
