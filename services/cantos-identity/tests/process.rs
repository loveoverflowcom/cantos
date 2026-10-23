//! The production `cantos-identity` executable over its real private Unix socket, with a real
//! file-backed Turso database, real Argon2id and a real SMTP session into a run-owned sink.
//!
//! Authentication facts here end at Identity: no Server session, browser or device is involved.
#![cfg(feature = "service")]

mod support;

use std::{os::unix::fs::PermissionsExt, path::Path, time::Duration};

use cantos_identity::persistence::IDENTITY_APPLICATION_ID;
use support::rpc_client::{
    identity_client::IdentityClient, AccountState, AuthenticatePasswordRequest, ClientContext,
    CompleteTheatreSignupRequest, GetSecurityStateRequest, Purpose, SetupOutcome,
    StartTheatreSignupRequest,
};
use support::{bound_fact, channel, client, hex, Env, RunDir, SinkMode, SmtpSink};
use tonic::{transport::Channel, Code, Request};

const PASSWORD: &str = "chosen by the mailbox owner";
const OWNER: &str = "owner@example.com";

fn ctx(ip: &str) -> Option<ClientContext> {
    Some(ClientContext {
        client_ip: ip.to_owned(),
    })
}

fn start(op: u8, email: &str) -> StartTheatreSignupRequest {
    StartTheatreSignupRequest {
        operation_id: vec![op; 16],
        email: email.to_owned(),
        client: ctx("198.51.100.1"),
    }
}

fn complete(cmd: u8, link: &support::ParsedLink, password: &str) -> CompleteTheatreSignupRequest {
    CompleteTheatreSignupRequest {
        command_id: vec![cmd; 16],
        operation_id: link.operation.to_vec(),
        proof_id: link.proof.to_vec(),
        proof_secret: link.secret.to_vec(),
        firebase_oob_code: String::new(),
        password: password.to_owned(),
        explicit_confirmation: true,
        client: ctx("203.0.113.5"),
    }
}

fn login(op: u8, email: &str, password: &str, ip: &str) -> AuthenticatePasswordRequest {
    AuthenticatePasswordRequest {
        operation_id: vec![op; 16],
        purpose: Purpose::Theatre as i32,
        email: email.to_owned(),
        password: password.to_owned(),
        client: ctx(ip),
    }
}

struct World {
    run: RunDir,
    env: Env,
    sink: SmtpSink,
}

async fn world() -> World {
    let run = RunDir::new();
    let sink = SmtpSink::start(SinkMode::Accept).await;
    let mut env = Env::new(&run);
    env.smtp_port = Some(sink.port);
    let (ok, stderr) = env.migrate().await;
    assert!(ok, "migrate failed: {stderr}");
    World { run, env, sink }
}

async fn assert_not_ready(env: &Env) -> support::Failed {
    match env.serve().await {
        Ok(_) => panic!("the process must refuse to serve"),
        Err(failure) => failure,
    }
}

fn sha(path: &Path) -> Vec<u8> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    bytes
}

// ---- A: the journey -----------------------------------------------------------------------------

#[tokio::test]
async fn mailbox_owner_signs_up_then_authenticates_again_after_a_restart() {
    let w = world().await;
    let instance = w.env.serve().await.ok().expect("serve");
    assert_eq!(
        instance.ready_line(),
        r#"{"event":"ready","mail":"configured"}"#
    );
    let mut rpc = client(&instance.socket).await;

    // Start: the answer is generic and the proof goes only to the mailbox, over real SMTP.
    let started = rpc
        .start_theatre_signup(start(1, OWNER))
        .await
        .unwrap()
        .into_inner();
    assert!(started.accepted && started.proof_ttl_seconds == 900);
    let mail = w.sink.wait_for(1).await.remove(0);
    assert_eq!(mail.to, [OWNER]);
    let link = mail.link();
    assert_eq!(link.operation, [1; 16]);

    // The mailbox owner confirms explicitly and chooses a password.
    let done = rpc
        .complete_theatre_signup(complete(10, &link, PASSWORD))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(done.outcome, SetupOutcome::Completed as i32);

    // Authenticate: a fact bound to this operation, ending at Identity.
    let fact = rpc
        .authenticate_password(login(5, OWNER, PASSWORD, "203.0.113.5"))
        .await
        .unwrap()
        .into_inner()
        .fact
        .unwrap();
    let verified = bound_fact(&fact, [5; 16], Purpose::Theatre);
    assert_eq!(verified.subject, done.subject_id);
    assert_eq!(verified.instance_epoch, done.instance_epoch);
    assert_eq!(verified.security_version, 1);
    // #335 v1: the same account also authenticates for Studio with its password. Identity keeps
    // no Studio roster; that admission is the Server's.
    let mut studio = login(8, OWNER, PASSWORD, "203.0.113.5");
    studio.purpose = Purpose::Studio as i32;
    let studio_fact = rpc
        .authenticate_password(studio)
        .await
        .unwrap()
        .into_inner()
        .fact
        .unwrap();
    let studio_verified = bound_fact(&studio_fact, [8; 16], Purpose::Studio);
    assert_eq!(studio_verified.subject, verified.subject);
    assert_eq!(studio_verified.purpose, Purpose::Studio);
    let state = rpc
        .get_security_state(GetSecurityStateRequest {
            subject_id: done.subject_id.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(state.state, AccountState::Active as i32);
    assert_eq!(state.instance_epoch, done.instance_epoch);

    // Graceful stop: exit 0 and only this run's socket is removed.
    drop(rpc);
    let socket = instance.socket.clone();
    let (exit, first_logs) = instance.terminate().await;
    assert!(exit.success(), "{first_logs}");
    assert!(
        !socket.exists(),
        "the run's own socket is removed on a clean stop"
    );

    // Restart on the same database: same subject, same epoch, same credential.
    let again = w.env.serve().await.ok().expect("restart");
    let mut rpc = client(&again.socket).await;
    let fact = rpc
        .authenticate_password(login(6, OWNER, PASSWORD, "203.0.113.5"))
        .await
        .unwrap()
        .into_inner()
        .fact
        .unwrap();
    let after = bound_fact(&fact, [6; 16], Purpose::Theatre);
    assert_eq!(after.subject, verified.subject);
    assert_eq!(after.instance_epoch, verified.instance_epoch);
    assert!(after.authenticated_at_unix_micros >= verified.authenticated_at_unix_micros);
    let wrong = rpc
        .authenticate_password(login(7, OWNER, "not the password", "203.0.113.5"))
        .await
        .unwrap_err();
    assert_eq!(wrong.code(), Code::Unauthenticated);
    drop(rpc);
    let (exit, second_logs) = again.terminate().await;
    assert!(exit.success());

    // No secret reached a log, and no plaintext password reached any database file.
    for logs in [&first_logs, &second_logs] {
        assert!(!logs.contains(PASSWORD));
        assert!(!logs.contains(OWNER));
        assert!(!logs.contains(&hex(&link.secret)));
    }
    for entry in std::fs::read_dir(w.run.dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            let bytes = std::fs::read(&path).unwrap();
            assert!(!bytes
                .windows(PASSWORD.len())
                .any(|w| w == PASSWORD.as_bytes()));
            assert!(!bytes.windows(32).any(|w| w == link.secret));
        }
    }
}

#[tokio::test]
async fn the_initiator_never_receives_the_proof_and_the_owner_alone_sets_the_password() {
    let w = world().await;
    let instance = w.env.serve().await.ok().unwrap();
    let mut rpc = client(&instance.socket).await;
    // Whoever calls start for "victim" gets only the generic answer.
    let answer = rpc
        .start_theatre_signup(start(1, "victim@example.com"))
        .await
        .unwrap()
        .into_inner();
    assert_eq!((answer.accepted, answer.proof_ttl_seconds), (true, 900));
    let link = w.sink.wait_for(1).await.remove(0).link();
    let mut guessed = link.clone();
    guessed.secret = [7; 32];
    let denied = rpc
        .complete_theatre_signup(complete(10, &guessed, "attacker chose this"))
        .await
        .unwrap_err();
    assert_eq!(denied.code(), Code::PermissionDenied);
    rpc.complete_theatre_signup(complete(11, &link, "the owner chose this"))
        .await
        .unwrap();
    assert_eq!(
        rpc.authenticate_password(login(
            2,
            "victim@example.com",
            "attacker chose this",
            "198.51.100.9"
        ))
        .await
        .unwrap_err()
        .code(),
        Code::Unauthenticated
    );
    rpc.authenticate_password(login(
        3,
        "victim@example.com",
        "the owner chose this",
        "198.51.100.9",
    ))
    .await
    .unwrap();
    // Replaying the confirmed command is a historical answer; a changed password is refused.
    let replay = rpc
        .complete_theatre_signup(complete(11, &link, "the owner chose this"))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(replay.outcome, SetupOutcome::Replayed as i32);
    let changed = rpc
        .complete_theatre_signup(complete(11, &link, "swapped after the fact"))
        .await
        .unwrap_err();
    assert_eq!(changed.code(), Code::PermissionDenied);
    instance.crash().await;
}

// ---- C: boundary refusals -----------------------------------------------------------------------

#[tokio::test]
async fn unknown_methods_and_malformed_requests_are_refused_without_effect() {
    let w = world().await;
    let instance = w.env.serve().await.ok().unwrap();
    let mut rpc = client(&instance.socket).await;
    let mut unspecified = login(1, OWNER, PASSWORD, "203.0.113.5");
    unspecified.purpose = Purpose::Unspecified as i32;
    assert_eq!(
        rpc.authenticate_password(unspecified)
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    // A method that does not exist (an operator/Studio call) is rejected by the server.
    let mut raw = tonic::client::Grpc::new(channel(&instance.socket).await.unwrap());
    raw.ready().await.unwrap();
    let missing = raw
        .unary::<GetSecurityStateRequest, GetSecurityStateRequest, _>(
            Request::new(GetSecurityStateRequest::default()),
            "/cantos.identity.v1.Identity/ProvisionStudioAccount"
                .parse()
                .unwrap(),
            tonic_prost::ProstCodec::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(missing.code(), Code::Unimplemented);
    for bad in [
        StartTheatreSignupRequest {
            operation_id: vec![0; 16],
            ..start(1, OWNER)
        },
        StartTheatreSignupRequest {
            operation_id: vec![1; 15],
            ..start(1, OWNER)
        },
        StartTheatreSignupRequest {
            client: None,
            ..start(1, OWNER)
        },
        StartTheatreSignupRequest {
            client: ctx("not an address"),
            ..start(1, OWNER)
        },
        start(1, "not an email"),
    ] {
        assert_eq!(
            rpc.start_theatre_signup(bad).await.unwrap_err().code(),
            Code::InvalidArgument
        );
    }
    let mut unconfirmed = complete(
        1,
        &support::ParsedLink {
            operation: [1; 16],
            proof: [2; 16],
            secret: [3; 32],
        },
        PASSWORD,
    );
    unconfirmed.explicit_confirmation = false;
    assert_eq!(
        rpc.complete_theatre_signup(unconfirmed)
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    assert!(w.sink.messages().is_empty(), "no refused request sent mail");
    instance.crash().await;
}

#[tokio::test]
async fn an_oversized_request_is_refused_by_the_decode_limit_and_the_server_stays_healthy() {
    let w = world().await;
    let instance = w.env.serve().await.ok().unwrap();
    let mut rpc = client(&instance.socket).await;
    let huge = login(1, OWNER, &"p".repeat(200_000), "203.0.113.5");
    let refused = rpc.authenticate_password(huge).await.unwrap_err();
    assert!(
        matches!(refused.code(), Code::OutOfRange | Code::ResourceExhausted),
        "{refused:?}"
    );
    rpc.start_theatre_signup(start(1, OWNER)).await.unwrap();
    instance.crash().await;
}

#[tokio::test]
async fn a_caller_that_is_not_an_allowed_peer_gets_nothing_and_causes_no_effect() {
    let run = RunDir::new();
    let sink = SmtpSink::start(SinkMode::Accept).await;
    let mut env = Env::new(&run);
    env.smtp_port = Some(sink.port);
    // Allow only a uid that is not the test's own: the test process is the wrong peer.
    env.uids = (run.uid().wrapping_add(7)).to_string();
    assert!(env.migrate().await.0);
    let instance = env.serve().await.ok().unwrap();
    // The connection may be accepted at the transport level, but no RPC is ever served to it.
    let outcome = match channel(&instance.socket).await {
        Err(_) => None,
        Ok(channel) => Some(
            IdentityClient::new(channel)
                .start_theatre_signup(start(1, OWNER))
                .await,
        ),
    };
    if let Some(result) = outcome {
        assert!(result.is_err(), "a wrong peer must never be served");
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(sink.messages().is_empty());
    let (exit, logs) = instance.terminate().await;
    assert!(exit.success(), "{logs}");
    assert!(
        logs.contains("not an allowed"),
        "the refusal is logged: {logs}"
    );
}

// ---- mail ---------------------------------------------------------------------------------------

#[tokio::test]
async fn without_mail_configuration_signup_fails_closed_and_readiness_says_so() {
    let run = RunDir::new();
    let env = Env::new(&run);
    assert!(env.migrate().await.0);
    let instance = env.serve().await.ok().unwrap();
    assert_eq!(
        instance.ready_line(),
        r#"{"event":"ready","mail":"missing"}"#
    );
    let mut rpc = client(&instance.socket).await;
    let unavailable = rpc.start_theatre_signup(start(1, OWNER)).await.unwrap_err();
    assert_eq!(unavailable.code(), Code::Unavailable);
    // Authentication of a nonexistent account still answers generically.
    assert_eq!(
        rpc.authenticate_password(login(2, OWNER, PASSWORD, "203.0.113.5"))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    instance.crash().await;
}

#[tokio::test]
async fn concurrent_starts_over_rpc_are_all_accepted_and_the_mailbox_gets_one_message() {
    let w = world().await;
    let instance = w.env.serve().await.ok().unwrap();
    // The Server mints a fresh operation per start today, so an identical pair is a replay or a
    // racing retry, not a browser double-submit: it must still never surface as ABORTED.
    let tasks: Vec<_> = (0..3)
        .map(|_| {
            let socket = instance.socket.clone();
            tokio::spawn(async move {
                client(&socket)
                    .await
                    .start_theatre_signup(start(1, OWNER))
                    .await
                    .map(|response| response.into_inner().accepted)
            })
        })
        .collect();
    for task in tasks {
        assert_eq!(
            task.await.unwrap().map_err(|status| status.code()),
            Ok(true)
        );
    }
    let messages = w.sink.wait_for(1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(messages.len(), 1);
    assert_eq!(w.sink.messages().len(), 1);
    // The same operation bound to another address is the same generic answer, with no mail.
    let mut rpc = client(&instance.socket).await;
    let other = rpc
        .start_theatre_signup(start(1, "other@example.com"))
        .await
        .unwrap()
        .into_inner();
    assert!(other.accepted);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(w.sink.messages().len(), 1);
    instance.crash().await;
}

// ---- D: ownership, schema and sockets -------------------------------------------------------------

#[tokio::test]
async fn a_second_process_on_the_same_database_is_refused_and_the_first_keeps_serving() {
    let w = world().await;
    let first = w.env.serve().await.ok().unwrap();
    let mut other = w.env.clone();
    other.socket = w.run.path("second.sock");
    let failure = assert_not_ready(&other).await;
    assert_eq!(failure.code, Some(1), "{}", failure.stderr);
    assert!(!w.run.path("second.sock").exists());
    let mut rpc = client(&first.socket).await;
    rpc.start_theatre_signup(start(1, OWNER)).await.unwrap();
    first.crash().await;
}

#[tokio::test]
async fn a_live_socket_is_never_taken_over_and_a_stale_one_is_replaced() {
    let w = world().await;
    let first = w.env.serve().await.ok().unwrap();
    // Another database, same socket path: refused, and the first process is untouched.
    let run = RunDir::new();
    let mut rival = Env::new(&run);
    rival.socket = w.env.socket.clone();
    rival.smtp_port = w.env.smtp_port;
    rival.uids = w.env.uids.clone();
    assert!(rival.migrate().await.0);
    let failure = assert_not_ready(&rival).await;
    assert_eq!(failure.code, Some(1), "{}", failure.stderr);
    assert!(
        failure.stderr.contains("another identity process"),
        "{}",
        failure.stderr
    );
    let mut rpc = client(&first.socket).await;
    rpc.start_theatre_signup(start(1, OWNER)).await.unwrap();
    drop(rpc);
    // A crash leaves the socket file; the next owner may replace it because nobody answers.
    first.crash().await;
    assert!(w.env.socket.exists());
    let next = w
        .env
        .serve()
        .await
        .ok()
        .expect("a stale socket is replaced");
    client(&next.socket)
        .await
        .start_theatre_signup(start(2, "second@example.com"))
        .await
        .unwrap();
    next.crash().await;
}

#[tokio::test]
async fn an_unexpected_file_or_a_permissive_directory_is_refused_and_left_untouched() {
    let w = world().await;
    std::fs::write(&w.env.socket, b"not a socket").unwrap();
    let failure = assert_not_ready(&w.env).await;
    assert_eq!(failure.code, Some(1));
    assert_eq!(std::fs::read(&w.env.socket).unwrap(), b"not a socket");
    std::fs::remove_file(&w.env.socket).unwrap();
    std::fs::set_permissions(w.run.dir(), std::fs::Permissions::from_mode(0o755)).unwrap();
    let failure = assert_not_ready(&w.env).await;
    assert_eq!(failure.code, Some(1), "{}", failure.stderr);
    std::fs::set_permissions(w.run.dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!w.env.socket.exists());
}

#[tokio::test]
async fn serve_never_initializes_a_database_and_corruption_is_never_reset() {
    let run = RunDir::new();
    let env = Env::new(&run);
    // No database yet: serve refuses and must not create one.
    let failure = assert_not_ready(&env).await;
    assert_eq!(failure.code, Some(1), "{}", failure.stderr);
    assert!(!env.database.exists() || std::fs::metadata(&env.database).unwrap().len() == 0);
    // A non-database file at the path: refused, bytes unchanged.
    let garbage = b"definitely not a turso database, do not touch".repeat(40);
    std::fs::write(&env.database, &garbage).unwrap();
    let failure = assert_not_ready(&env).await;
    assert_eq!(failure.code, Some(1), "{}", failure.stderr);
    assert_eq!(sha(&env.database), garbage);
    let (ok, _) = env.migrate().await;
    assert!(!ok, "migrate does not adopt a foreign file either");
    assert_eq!(sha(&env.database), garbage);
}

#[tokio::test]
async fn serve_refuses_a_file_with_pending_migrations_and_explicit_migrate_upgrades_it() {
    use cantos_store_turso::{
        registry::{Intent, Migration, Registry},
        testing::FixedClock,
        OpenRequest, OwnedDatabase, Value,
    };
    const V1: [Migration; 1] = [Migration {
        version: 1,
        name: "identity_security_state",
        sql: include_str!("../src/persistence/schema.sql"),
    }];
    let run = RunDir::new();
    let env = Env::new(&run);
    // An older-schema file exactly as an earlier build would have left it.
    let database = OwnedDatabase::open(OpenRequest::new(
        env.database.clone(),
        Registry::new(IDENTITY_APPLICATION_ID, 1, &V1).unwrap(),
        Intent::Migrate,
        FixedClock::new(1_700_000_000_000_000),
    ))
    .await
    .unwrap();
    let tx = database.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO identity_instance(singleton,instance_epoch) VALUES(1,?1)",
        [Value::Blob(vec![9; 16])],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(database);
    let before = sha(&env.database);
    let failure = assert_not_ready(&env).await;
    assert_eq!(failure.code, Some(1), "{}", failure.stderr);
    assert_eq!(sha(&env.database), before, "serve never migrates");
    let (ok, stderr) = env.migrate().await;
    assert!(ok, "{stderr}");
    let upgraded = env
        .serve()
        .await
        .ok()
        .expect("serves after an explicit migrate");
    upgraded.crash().await;
}

// ---- concurrency ----------------------------------------------------------------------------------

#[tokio::test]
async fn a_burst_beyond_kdf_capacity_is_refused_promptly_and_capacity_returns() {
    let w = world().await;
    let instance = w.env.serve().await.ok().unwrap();
    let mut rpc = client(&instance.socket).await;
    rpc.start_theatre_signup(start(1, OWNER)).await.unwrap();
    let link = w.sink.wait_for(1).await.remove(0).link();
    rpc.complete_theatre_signup(complete(10, &link, PASSWORD))
        .await
        .unwrap();
    // The server holds 16 connections at once and `rpc` above is one of them. One connection
    // admits only a few concurrent streams, so the burst uses many connections:
    // only then can more requests than KDF workers plus waiting slots be in flight at once.
    let mut channels: Vec<Channel> = Vec::new();
    for _ in 0..15 {
        channels.push(channel(&instance.socket).await.unwrap());
    }
    let tasks: Vec<_> = (0..48_u8)
        .map(|n| {
            let mut rpc = IdentityClient::new(channels[usize::from(n) % channels.len()].clone());
            tokio::spawn(async move {
                // A distinct account per request keeps the account-wide failure budget out of the
                // way, so any refusal other than "wrong password" comes from KDF capacity.
                rpc.authenticate_password(login(
                    n + 1,
                    &format!("target{n}@example.com"),
                    "wrong password here",
                    &format!("203.0.113.{}", n + 1),
                ))
                .await
                .map(|_| ())
                .map_err(|status| status.code())
            })
        })
        .collect();
    let mut codes = Vec::new();
    for task in tasks {
        codes.push(task.await.unwrap().unwrap_err());
    }
    assert!(
        codes
            .iter()
            .all(|c| matches!(c, Code::Unauthenticated | Code::Unavailable)),
        "{codes:?}"
    );
    assert!(
        codes.contains(&Code::Unauthenticated),
        "some requests were served"
    );
    assert!(
        codes.contains(&Code::Unavailable),
        "the burst exceeded KDF capacity and was shed: {codes:?}"
    );
    // The right owner, from a fresh address, is served once the burst is over.
    tokio::time::sleep(Duration::from_millis(500)).await;
    rpc.authenticate_password(login(100, OWNER, PASSWORD, "198.51.100.200"))
        .await
        .unwrap();
    instance.crash().await;
}

#[tokio::test]
async fn a_cancelled_login_does_not_leak_capacity() {
    let w = world().await;
    let instance = w.env.serve().await.ok().unwrap();
    let mut rpc = client(&instance.socket).await;
    rpc.start_theatre_signup(start(1, OWNER)).await.unwrap();
    let link = w.sink.wait_for(1).await.remove(0).link();
    rpc.complete_theatre_signup(complete(10, &link, PASSWORD))
        .await
        .unwrap();
    for n in 0..12_u8 {
        let mut doomed = IdentityClient::new(channel(&instance.socket).await.unwrap());
        let request = login(n + 1, OWNER, PASSWORD, &format!("203.0.113.{}", n + 1));
        // The caller gives up almost immediately, while the hash is still running.
        let _ = tokio::time::timeout(
            Duration::from_millis(5),
            doomed.authenticate_password(request),
        )
        .await;
    }
    tokio::time::sleep(Duration::from_secs(2)).await;
    rpc.authenticate_password(login(99, OWNER, PASSWORD, "198.51.100.201"))
        .await
        .unwrap();
    instance.crash().await;
}

// ---- production isolation -------------------------------------------------------------------------

#[test]
fn production_sources_and_the_executable_name_no_capture_inbox_or_test_bypass() {
    // A source-level guard plus a string smoke check of the built binary. The type names live
    // only in the test crate, so the binary scan cannot by itself prove absence of behavior:
    // the real evidence is that production composition (`server::compose`) builds only
    // `UnconfiguredMailer` or `SmtpMailer`, neither of which can accept a message locally.
    let binary = std::fs::read(support::binary()).unwrap();
    for needle in [
        "CaptureMailer",
        "SmtpSink",
        "SinkMode",
        "test_bypass",
        "FixedClock",
    ] {
        assert!(
            !binary.windows(needle.len()).any(|w| w == needle.as_bytes()),
            "{needle} is present in the executable"
        );
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![source];
    while let Some(directory) = stack.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                let production = text.split("#[cfg(test)]").next().unwrap();
                for needle in [
                    "CaptureMailer",
                    "SmtpSink",
                    "test_bypass",
                    "always_succeeds",
                    "AllowAll",
                ] {
                    assert!(
                        !production.contains(needle),
                        "{needle} in {}",
                        path.display()
                    );
                }
            }
        }
    }
}
