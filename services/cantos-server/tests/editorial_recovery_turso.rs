//! Native replacement for the Revision/Import runner's process-restart oracles. Real loopback
//! HTTP and the actual Server executable own one file at a time. Development Studio sessions
//! are explicit synthetic credentials; this does not establish Identity/browser acceptance.
use cantos_server::contract::{
    codec::{decode_pilot, encode_pilot, WireFormat},
    studio::{ImportFormat, ImportResponse, RevisionResponse},
};
use cantos_server::{
    platform::turso::registry,
    script_ir::read_script,
    test_support::{
        editorial::{import, request, ALICE, FIXTURE, SCRIPT},
        ServerFixture,
    },
};
use cantos_store_turso::{
    clock::SystemClock,
    error::OpenError,
    registry::{Intent, Refusal},
    OpenRequest, OwnedDatabase, Value,
};
use std::{path::Path, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};

struct Host {
    child: Child,
    base: String,
}
impl Host {
    async fn start(path: &Path) -> Self {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let base = format!("http://127.0.0.1:{port}");
        let mut child = Command::new(env!("CARGO_BIN_EXE_cantos-server"))
            .env_clear()
            .env("CANTOS_ENV", "development")
            .env("CANTOS_BIND_ADDRESS", format!("127.0.0.1:{port}"))
            .env("CANTOS_WEB_ORIGIN", &base)
            .env("CANTOS_SERVER_DATABASE", path)
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let line = lines.next_line().await.unwrap().expect("Server readiness");
                if line.contains("development host:") {
                    break;
                }
                assert!(!line.contains("Error"), "{line}");
            }
        })
        .await
        .expect("Server starts within the bound");
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
        Self { child, base }
    }

    async fn kill(mut self) {
        self.child.kill().await.unwrap();
        self.child.wait().await.unwrap();
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

async fn call(host: &Host, method: &str, path: &str, format: WireFormat, body: Vec<u8>) -> Vec<u8> {
    let mime = match format {
        WireFormat::Json => "application/json",
        WireFormat::MessagePack => "application/msgpack",
        WireFormat::Protobuf => "application/protobuf",
    };
    let response = client()
        .request(
            method.parse().unwrap(),
            format!("{}/api/v1/studio{path}", host.base),
        )
        .header("origin", &host.base)
        .header("cookie", format!("cantos_session={ALICE}"))
        .header("content-type", mime)
        .header("accept", mime)
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    response.bytes().await.unwrap().to_vec()
}

#[tokio::test]
async fn killed_server_reopens_exact_revision_and_import_receipts_with_cross_codec_replay() {
    let mut fixture = ServerFixture::new().await;
    fixture.studio_caller("alice", ALICE).await;
    for (kind, id) in read_script(FIXTURE.as_bytes()).unwrap().evidence_refs() {
        fixture
            .exec(
                "INSERT INTO script_evidence VALUES('alice',?1,?2,'synthetic restart fixture')",
                &[&kind, &id],
            )
            .await
            .unwrap();
    }
    let path = fixture.database_path();
    fixture.close();
    let host = Host::start(&path).await;
    let intent = request(0);
    // Discard an acknowledged Protobuf save without reading its body, then reconcile on the key.
    let response = client()
        .post(format!(
            "{}/api/v1/studio/scripts/{SCRIPT}/revisions",
            host.base
        ))
        .header("origin", &host.base)
        .header("cookie", format!("cantos_session={ALICE}"))
        .header("content-type", "application/protobuf")
        .header("accept", "application/protobuf")
        .body(encode_pilot(WireFormat::Protobuf, &intent).unwrap())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    drop(response);
    let head = format!("/scripts/{SCRIPT}/head");
    let accepted: RevisionResponse = decode_pilot(
        WireFormat::Json,
        &call(&host, "GET", &head, WireFormat::Json, vec![]).await,
    )
    .unwrap();
    assert_eq!(accepted.revision, 1);
    assert_eq!(accepted.accepted_by, "alice");
    let mut imports = Vec::new();
    for (format, bytes) in [
        (ImportFormat::Txt, "\u{feff}Người bạn\r\n".as_bytes()),
        (ImportFormat::Docx, b"PK\x01".as_slice()),
        (ImportFormat::Txt, b"\xff\xfe".as_slice()),
    ] {
        let mut input = import();
        input.metadata.format = format;
        input.original_bytes = bytes.to_vec();
        let receipt: ImportResponse = serde_json::from_slice(
            &call(
                &host,
                "POST",
                "/imports",
                WireFormat::Json,
                serde_json::to_vec(&input).unwrap(),
            )
            .await,
        )
        .unwrap();
        imports.push((input, receipt));
    }
    host.kill().await;
    let host = Host::start(&path).await;
    for format in [
        WireFormat::Json,
        WireFormat::MessagePack,
        WireFormat::Protobuf,
    ] {
        let replay: RevisionResponse = decode_pilot(
            format,
            &call(
                &host,
                "POST",
                &format!("/scripts/{SCRIPT}/revisions"),
                format,
                encode_pilot(format, &intent).unwrap(),
            )
            .await,
        )
        .unwrap();
        let loaded: RevisionResponse =
            decode_pilot(format, &call(&host, "GET", &head, format, vec![]).await).unwrap();
        assert_eq!(replay, accepted);
        assert_eq!(loaded, accepted);
    }
    for (input, receipt) in &imports {
        let replay: ImportResponse = serde_json::from_slice(
            &call(
                &host,
                "POST",
                "/imports",
                WireFormat::Json,
                serde_json::to_vec(input).unwrap(),
            )
            .await,
        )
        .unwrap();
        assert_eq!(&replay, receipt);
        let loaded: ImportResponse = serde_json::from_slice(
            &call(
                &host,
                "GET",
                &format!("/imports/{}", receipt.id),
                WireFormat::Json,
                vec![],
            )
            .await,
        )
        .unwrap();
        assert_eq!(&loaded, receipt);
        let response = client()
            .get(format!(
                "{}/api/v1/studio/imports/{}/original",
                host.base, receipt.id
            ))
            .header("cookie", format!("cantos_session={ALICE}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers()["content-type"],
            "application/octet-stream"
        );
        assert_eq!(
            response.headers()["content-disposition"],
            "attachment; filename=\"manuscript.bin\""
        );
        assert_eq!(
            response.bytes().await.unwrap().as_ref(),
            input.original_bytes
        );
    }
    host.kill().await;
    fixture.restart().await;
    assert_eq!(
        fixture.store().load(ALICE, SCRIPT, None).await.unwrap(),
        accepted
    );
    for (table, count) in [
        ("scripts", 1),
        ("script_revisions", 1),
        ("source_records", 3),
        (
            "cantos_schema_history",
            i64::try_from(registry().unwrap().migrations().len()).unwrap(),
        ),
    ] {
        assert_eq!(
            fixture
                .count(&format!("SELECT count(*) FROM {table}"), &[])
                .await,
            count
        );
    }
}

async fn snapshot(owner: &OwnedDatabase) -> Vec<Vec<Vec<Value>>> {
    let mut snapshot = Vec::new();
    for table in [
        "actors",
        "actor_external_identities",
        "development_sessions",
        "scripts",
        "script_revisions",
        "script_evidence",
        "revision_evidence",
        "source_records",
        "cantos_schema_history",
    ] {
        let mut rows = owner
            .reader()
            .await
            .unwrap()
            .query(&format!("SELECT * FROM {table} ORDER BY 1,2"), ())
            .await
            .unwrap();
        let columns = rows.column_names().len();
        let mut values = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            values.push((0..columns).map(|i| row.get_value(i).unwrap()).collect());
        }
        snapshot.push(values);
    }
    snapshot
}

#[tokio::test]
async fn repeated_native_migrations_preserve_business_bytes_and_corrupt_history_refuses_unchanged()
{
    use cantos_server::platform::{
        identity::domain::{ActorId, ExternalIdentityKey},
        operator,
    };
    let mut fixture = ServerFixture::new().await;
    fixture.studio_caller("alice", ALICE).await;
    for (kind, id) in read_script(FIXTURE.as_bytes()).unwrap().evidence_refs() {
        fixture
            .exec(
                "INSERT INTO script_evidence VALUES('alice',?1,?2,'synthetic migration fixture')",
                &[&kind, &id],
            )
            .await
            .unwrap();
    }
    operator::record_source(
        fixture.store(),
        "alice",
        "source-demo",
        "legacy text only",
        "Người\r\n",
    )
    .await
    .unwrap();
    let saved = fixture
        .store()
        .save(ALICE, SCRIPT, request(0))
        .await
        .unwrap();
    let imported = fixture
        .store()
        .import_manuscript(ALICE, import())
        .await
        .unwrap();
    operator::link_identity(
        fixture.store(),
        &ActorId::new("alice").unwrap(),
        &ExternalIdentityKey::new("issuer", "subject").unwrap(),
    )
    .await
    .unwrap();
    let path = fixture.database_path();
    fixture.close();
    let open = || {
        OpenRequest::new(
            &path,
            registry().unwrap(),
            Intent::Migrate,
            Arc::new(SystemClock),
        )
    };
    let owner = OwnedDatabase::open(open()).await.unwrap();
    let before = snapshot(&owner).await;
    drop(owner);
    for _ in 0..2 {
        let owner = OwnedDatabase::open(open()).await.unwrap();
        assert_eq!(snapshot(&owner).await, before);
    }
    fixture.restart().await;
    assert_eq!(
        fixture.store().load(ALICE, SCRIPT, None).await.unwrap(),
        saved
    );
    assert_eq!(
        fixture
            .store()
            .load_import(ALICE, &imported.id)
            .await
            .unwrap(),
        imported
    );
    assert_eq!(fixture.store().authenticate(ALICE).await.unwrap(), "alice");
    assert_eq!(fixture.count("SELECT count(*) FROM source_records WHERE id='source-demo' AND original_bytes IS NULL AND imported_by IS NULL AND import_operation_id IS NULL AND import_digest IS NULL AND import_metadata IS NULL AND import_outcome IS NULL", &[]).await, 1);
    assert_eq!(fixture.count("SELECT count(*) FROM sqlite_schema WHERE name IN ('durable_runs','speech_clips','adaptation_attempts')", &[]).await, 0);
    fixture
        .exec("DROP TRIGGER cantos_schema_history_no_update", &[])
        .await
        .unwrap();
    fixture
        .exec(
            "UPDATE cantos_schema_history SET checksum=zeroblob(32) WHERE version=16",
            &[],
        )
        .await
        .unwrap();
    fixture.close();
    let damaged = std::fs::read(&path).unwrap();
    let divergent = OpenError::Refused(Refusal::HistoryDiverged { version: 16 });
    assert_eq!(OwnedDatabase::open(open()).await.err(), Some(divergent));
    // The file owner cannot repair a checksum by trying startup repeatedly.
    assert!(matches!(
        OwnedDatabase::open(open()).await,
        Err(OpenError::Refused(Refusal::HistoryDiverged { version: 16 }))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), damaged);
}
