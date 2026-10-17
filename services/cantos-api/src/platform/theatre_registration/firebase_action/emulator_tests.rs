//! Optional tests against the already installed, in-memory Firebase Auth emulator.
//! This is a real local adapter boundary, not live Firebase, mail, IAM or enrollment.
//! The emulator ignores API-key/project binding, has no OOB expiry and retains old resend
//! codes. No emulator token is admitted by production code or converted into a session.

use std::{
    io::{BufRead, BufReader, Read},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use reqwest::{header, Client, Method, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::{
    ActionCode, ActionError, BindingField, ConsumptionError, DedicatedUid, ExpectedActionBinding,
    FirebaseActionClient, FirebaseProjectId,
};
use crate::platform::theatre_registration::domain::{Email, PendingRegistration};

const PROJECT: &str = "demo-cantos-issue206";
const EMAIL: &str = "listener@example.test";
const UID: &str = "cantos-emulator-dedicated-uid";

// This small launcher is an implementation detail of the ignored Rust test, not another
// repository command. It invokes neither the Firebase CLI nor a package installer.
// Fresh XDG configuration and an empty child environment isolate existing CLI credentials.
// An outbound-socket refusal is installed before loading the emulator and stays in place;
// the only socket the child opens is its owned literal-loopback HTTP listening socket.
const LAUNCHER: &str = r#"
const net = require('node:net');
const tls = require('node:tls');
net.Socket.prototype.connect = function () {
  throw new Error('Auth emulator fixture forbids outbound connections');
};
tls.connect = function () {
  throw new Error('Auth emulator fixture forbids outbound TLS');
};
const path = require('node:path');
const modulePath = process.argv[1];
const project = process.argv[2];
if (!/^demo-[a-z0-9-]{1,24}$/.test(project)) process.exit(1);
const pkg = require(path.resolve(path.dirname(modulePath), '../../../package.json'));
if (pkg.name !== 'firebase-tools' || pkg.version !== '14.2.2') {
  throw new Error('Review the installed Auth emulator version before running this fixture');
}
(async () => {
  const { createApp } = require(modulePath);
  const app = await createApp(project, 2);
  const server = app.listen(0, '127.0.0.1');
  server.once('error', () => process.exit(1));
  server.once('listening', () => {
    process.stdout.write(JSON.stringify({
      host: '127.0.0.1', port: server.address().port,
      project, version: pkg.version
    }) + '\n');
  });
})().catch(() => process.exit(1));
"#;

struct OwnedChild {
    child: Child,
    output: Option<JoinHandle<()>>,
    directory: PathBuf,
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Only this directly owned Node process is stopped. No process group, unrelated
        // service, emulator registry or existing Firebase project is managed by the test.
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_secs(3);
        let reaped = loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break true,
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10));
                }
                _ => break false,
            }
        };
        if reaped {
            let _ = self.child.wait();
            if let Some(output) = self.output.take() {
                let _ = output.join();
            }
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
}

pub(crate) struct Emulator {
    _owned: OwnedChild,
    pub(crate) address: SocketAddr,
    pub(crate) project: String,
    http: Client,
}

#[derive(Deserialize)]
struct Ready {
    host: IpAddr,
    port: u16,
    project: String,
    version: String,
}

impl Emulator {
    pub(crate) fn directory(&self) -> &std::path::Path {
        &self._owned.directory
    }

    fn start() -> Self {
        Self::start_project(PROJECT)
    }

    pub(crate) fn start_project(project: &str) -> Self {
        FirebaseProjectId::parse(project).expect("syntactically valid fixture project");
        assert!(project.starts_with("demo-"), "demo project required");
        let module = std::env::var_os("CANTOS_TEST_FIREBASE_AUTH_MODULE")
            .map(PathBuf::from)
            .expect("set CANTOS_TEST_FIREBASE_AUTH_MODULE to the installed Auth server.js");
        let module = module.canonicalize().expect("installed Auth module exists");
        assert_eq!(
            module.file_name().and_then(|s| s.to_str()),
            Some("server.js")
        );
        let node = std::env::var_os("PATH")
            .into_iter()
            .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
            .map(|path| path.join(if cfg!(windows) { "node.exe" } else { "node" }))
            .find(|path| path.is_file())
            .expect("an installed Node executable must be on PATH; this test installs nothing");
        let directory =
            std::env::temp_dir().join(format!("cantos-auth-emulator-{}", Uuid::new_v4()));
        std::fs::create_dir(&directory).expect("create this test's isolated working directory");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
                .expect("private emulator working directory");
        }
        let spawned = Command::new(node)
            .arg("-e")
            .arg(LAUNCHER)
            .arg(module)
            .arg(project)
            .env_clear()
            .env("XDG_CONFIG_HOME", directory.join("config"))
            .env(
                "FIREBASE_EMULATORS_PATH",
                directory.join("unused-download-cache"),
            )
            .current_dir(&directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let child = match spawned {
            Ok(child) => child,
            Err(_) => {
                let _ = std::fs::remove_dir_all(&directory);
                panic!("could not start the explicitly selected installed Auth module");
            }
        };
        let mut owned = OwnedChild {
            child,
            output: None,
            directory,
        };
        let stdout = owned.child.stdout.take().expect("owned stdout pipe");
        let (send, receive) = mpsc::sync_channel(1);
        owned.output = Some(thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = Vec::new();
            let result = (&mut reader).take(4097).read_until(b'\n', &mut line);
            let ready = result.ok().filter(|size| *size <= 4096).map(|_| line);
            let _ = send.send(ready);
            // Never emit emulator OOB links, fixture tokens or upstream response bodies.
            let _ = std::io::copy(&mut reader, &mut std::io::sink());
        }));
        let line = receive
            .recv_timeout(Duration::from_secs(10))
            .expect("installed Auth emulator readiness deadline")
            .expect("bounded Auth emulator readiness metadata");
        let ready: Ready = serde_json::from_slice(&line).expect("Auth emulator readiness metadata");
        assert_eq!(ready.host, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_ne!(ready.port, 0);
        assert_eq!(ready.project, project);
        assert_eq!(ready.version, "14.2.2");
        Self {
            _owned: owned,
            address: SocketAddr::new(ready.host, ready.port),
            project: ready.project,
            http: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(3))
                .build()
                .expect("bounded loopback fixture client"),
        }
    }

    pub(crate) async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        owner: bool,
    ) -> Value {
        let url = format!("http://{}{}", self.address, path);
        let mut request = self.http.request(method, url);
        if owner {
            // This is the emulator's fixed local fixture credential, never a real bearer.
            request = request.header(header::AUTHORIZATION, "Bearer owner");
        }
        if let Some(body) = body {
            request = request
                .header(header::CONTENT_TYPE, "application/json")
                .body(serde_json::to_vec(&body).expect("fixture body serialization"));
        }
        let response = request
            .send()
            .await
            .expect("loopback emulator HTTP request");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "fixture request accepted"
        );
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.expect("fixture response body") {
            assert!(
                bytes.len() + chunk.len() <= 262_144,
                "bounded fixture response"
            );
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).expect("fixture response JSON")
    }

    pub(crate) async fn action(&self, name: &str, body: Value) -> Value {
        self.request(
            Method::POST,
            &format!(
                "/identitytoolkit.googleapis.com/v1/accounts:{name}?key=synthetic-fixture-key"
            ),
            Some(body),
            false,
        )
        .await
    }

    pub(crate) async fn pending_codes(&self) -> Vec<Value> {
        self.request(
            Method::GET,
            &format!("/emulator/v1/projects/{}/oobCodes", self.project),
            None,
            false,
        )
        .await["oobCodes"]
            .as_array()
            .expect("fixture OOB inventory")
            .clone()
    }

    async fn send_and_take_code(&self, body: Value) -> String {
        let previous = self.pending_codes().await;
        let result = self.action("sendOobCode", body).await;
        assert_eq!(result["email"], EMAIL);
        assert!(
            result.get("oobCode").is_none(),
            "normal send response has no code"
        );
        self.pending_codes()
            .await
            .into_iter()
            .find(|code| !previous.iter().any(|old| old["oobCode"] == code["oobCode"]))
            .and_then(|code| code["oobCode"].as_str().map(str::to_owned))
            .expect("new code exists only in the emulator fixture inventory")
    }
}

fn binding(email: &str, project: &str) -> ExpectedActionBinding {
    ExpectedActionBinding::new(
        PendingRegistration::new(Uuid::new_v4(), Email::parse(email).expect("fixture email"))
            .expect("fixture registration"),
        FirebaseProjectId::parse(project).expect("fixture project"),
        DedicatedUid::parse(UID).expect("fixture UID"),
    )
}

fn code(value: &str) -> ActionCode {
    ActionCode::parse(value).expect("emulator-issued action code")
}

#[tokio::test]
#[ignore = "requires explicit installed Auth emulator module; no live Firebase"]
async fn installed_auth_emulator_exercises_exact_action_consumption_and_resend_limits() {
    let emulator = Emulator::start();
    let created = emulator
        .request(
            Method::POST,
            &format!("/identitytoolkit.googleapis.com/v1/projects/{PROJECT}/accounts"),
            Some(json!({ "localId": UID, "email": EMAIL, "emailVerified": false })),
            true,
        )
        .await;
    assert_eq!(created["localId"], UID);
    assert_eq!(created["email"], EMAIL);
    assert!(created.get("idToken").is_none());
    // Fake JSON custom tokens are accepted only by this emulator. The production action
    // adapter accepts an OOB code, never this token or historical emailVerified state.
    let signed_in = emulator
        .action(
            "signInWithCustomToken",
            json!({ "token": json!({ "uid": UID }).to_string(), "returnSecureToken": true }),
        )
        .await;
    let id_token = signed_in["idToken"].as_str().expect("fixture ID token");
    let old = emulator
        .send_and_take_code(json!({ "requestType": "VERIFY_EMAIL", "idToken": id_token }))
        .await;
    let newer = emulator
        .send_and_take_code(json!({ "requestType": "VERIFY_EMAIL", "idToken": id_token }))
        .await;
    assert!(old != newer, "resend creates another synthetic code");
    let expected = binding(EMAIL, PROJECT);
    let client =
        FirebaseActionClient::auth_emulator_fixture(emulator.address, Duration::from_secs(3))
            .expect("test-only fixed-prefix loopback seam");
    let other =
        FirebaseActionClient::auth_emulator_fixture(emulator.address, Duration::from_secs(3))
            .expect("separate test client");
    assert_eq!(
        client
            .inspect(binding(EMAIL, "other-project"), code(&old))
            .await
            .err(),
        Some(ActionError::InvalidBinding(BindingField::Project))
    );
    assert_eq!(
        client
            .inspect(binding("other@example.test", PROJECT), code(&old))
            .await
            .err(),
        Some(ActionError::InvalidBinding(BindingField::Email))
    );
    let inspected = client
        .inspect(expected.clone(), code(&old))
        .await
        .expect("inspect old code");
    assert_eq!(
        other.consume(inspected).await.err(),
        Some(ConsumptionError::BeforeConsume(ActionError::WrongClient))
    );
    let reset = emulator
        .send_and_take_code(json!({ "requestType": "PASSWORD_RESET", "email": EMAIL }))
        .await;
    assert_eq!(
        client.inspect(expected.clone(), code(&reset)).await.err(),
        Some(ActionError::WrongPurpose)
    );
    let reset_still_pending = emulator
        .action("resetPassword", json!({ "oobCode": reset }))
        .await;
    assert_eq!(reset_still_pending["requestType"], "PASSWORD_RESET");
    let inspected = client
        .inspect(expected.clone(), code(&old))
        .await
        .expect("same old code remains valid");
    let consumed = client
        .consume(inspected)
        .await
        .expect("consume inspected code");
    assert_eq!(consumed.binding(), &expected);
    assert_eq!(
        client.inspect(expected.clone(), code(&old)).await.err(),
        Some(ActionError::InvalidCode)
    );
    // Demonstrates a limitation of the emulator: issuing/consuming another code does not
    // invalidate this sibling. This is not acceptance of production resend semantics.
    let inspected = client
        .inspect(expected.clone(), code(&newer))
        .await
        .expect("resend sibling remains valid in emulator");
    assert_eq!(
        client
            .consume(inspected)
            .await
            .expect("consume sibling")
            .binding(),
        &expected
    );
    let account = emulator
        .request(
            Method::POST,
            &format!("/identitytoolkit.googleapis.com/v1/projects/{PROJECT}/accounts:lookup"),
            Some(json!({ "localId": [UID] })),
            true,
        )
        .await;
    assert_eq!(account["users"][0]["localId"], UID);
    assert_eq!(account["users"][0]["email"], EMAIL);
    assert_eq!(account["users"][0]["emailVerified"], true);
    assert!(account["users"][0].get("passwordHash").is_none());
}
