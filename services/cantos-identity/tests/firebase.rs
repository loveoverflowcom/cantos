//! Real Firebase emulator + production Identity executable/private RPC/Turso/Argon2id.
//! The outer xtask fixture owns the emulator. No native session/browser claim follows.
#![cfg(feature = "local-development")]
mod support;
use cantos_identity::{domain::mailbox_proof::ProofState, persistence::IdentityStore};
use cantos_store_turso::{clock::SystemClock, registry::Intent};
use serde_json::Value;
use std::{os::unix::fs::PermissionsExt, time::Duration};
use support::rpc_client::{
    AuthenticatePasswordRequest, ClientContext, CompleteTheatreSignupRequest, Purpose,
    SetupOutcome, StartTheatreSignupRequest,
};
use support::{client, operation, Env, RunDir};
use tonic::Code;

fn context() -> Option<ClientContext> {
    Some(ClientContext {
        client_ip: "127.0.0.1".into(),
    })
}
fn id(value: &str) -> Vec<u8> {
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).unwrap())
        .collect()
}

#[tokio::test]
#[ignore = "outer xtask fixture supplies owned loopback demo emulator and private key file"]
async fn real_emulator_pending_proof_confirmation_password_login_and_restart() {
    let project =
        std::env::var("CANTOS_TEST_FIREBASE_PROJECT").expect("owned demo project required");
    let endpoint =
        std::env::var("CANTOS_TEST_FIREBASE_ENDPOINT").expect("owned loopback endpoint required");
    let address: std::net::SocketAddr = endpoint.parse().unwrap();
    assert!(address.ip().is_loopback() && address.port() != 0 && project.starts_with("demo-"));
    let key = std::env::var_os("CANTOS_TEST_FIREBASE_KEY_FILE")
        .map(std::path::PathBuf::from)
        .expect("private envelope-key file required");
    assert_eq!(
        std::fs::metadata(&key).unwrap().permissions().mode() & 0o077,
        0
    );
    let run = RunDir::new();
    let mut env = Env::new(&run);
    env.firebase = Some((project.clone(), endpoint.clone(), key));
    assert!(env.migrate().await.0);
    let instance = env
        .serve()
        .await
        .ok()
        .expect("real Identity starts with the owned emulator");
    assert!(instance.ready_line().contains("configured"));
    let mut rpc = client(&instance.socket).await;
    let email = "native-recipient@example.test";
    let started = rpc
        .start_theatre_signup(StartTheatreSignupRequest {
            operation_id: vec![1; 16],
            email: email.into(),
            client: context(),
        })
        .await
        .unwrap()
        .into_inner();
    assert!(started.accepted);
    assert_eq!(started.proof_ttl_seconds, 900);
    let http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let response = http
        .get(format!(
            "http://{endpoint}/emulator/v1/projects/{project}/oobCodes"
        ))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let inventory: Value = serde_json::from_str(&response.text().await.unwrap()).unwrap();
    let issued = inventory["oobCodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["email"] == email && action["requestType"] == "VERIFY_EMAIL");
    if issued.is_none() {
        let _ = instance.terminate().await;
        let store = IdentityStore::open(
            env.database.clone(),
            Intent::Serve,
            std::sync::Arc::new(SystemClock),
        )
        .await
        .unwrap();
        let record = store
            .firebase_enrollment(&project, operation(u128::from_be_bytes([1; 16])))
            .await
            .unwrap()
            .unwrap();
        panic!(
            "native signup produced no action; durable provider state: {:?}",
            record.state
        );
    }
    let issued = issued.unwrap();
    let code = issued["oobCode"].as_str().unwrap().to_owned();
    let link = url::Url::parse(issued["oobLink"].as_str().unwrap()).unwrap();
    let continuation = link
        .query_pairs()
        .find(|(key, _)| key == "continueUrl")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    assert!(!continuation.contains("secret"));
    let continuation = url::Url::parse(&continuation).unwrap();
    let proof = continuation
        .query_pairs()
        .find(|(key, _)| key == "proof")
        .map(|(_, value)| id(&value))
        .unwrap();
    let (exit, logs) = instance.terminate().await;
    assert!(exit.success());
    assert!(!logs.contains(&code));
    // The process is stopped before another owner opens identity.db.
    let store = IdentityStore::open(
        env.database.clone(),
        Intent::Serve,
        std::sync::Arc::new(SystemClock),
    )
    .await
    .unwrap();
    let pending = store
        .signup(operation(u128::from_be_bytes([1; 16])))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.proof.state(), ProofState::Pending);
    let encrypted = store
        .firebase_enrollment(&project, pending.operation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(encrypted.envelope.len(), 60);
    drop(store);
    let instance = env
        .serve()
        .await
        .ok()
        .expect("Identity reopens pending proof without adopting a Firebase account");
    let mut rpc = client(&instance.socket).await;
    let mut entropy = [0u8; 24];
    getrandom::fill(&mut entropy).unwrap();
    let password: String = entropy
        .iter()
        .map(|byte| char::from(b'a' + byte % 26))
        .collect();
    let request = CompleteTheatreSignupRequest {
        command_id: vec![2; 16],
        operation_id: vec![1; 16],
        proof_id: proof,
        proof_secret: vec![],
        firebase_oob_code: code.clone(),
        password: password.clone(),
        explicit_confirmation: true,
        client: context(),
    };
    let mut mixed = request.clone();
    mixed.proof_secret = vec![7; 32];
    assert_eq!(
        rpc.complete_theatre_signup(mixed).await.unwrap_err().code(),
        Code::InvalidArgument
    );
    let mut absent = request.clone();
    absent.firebase_oob_code.clear();
    assert_eq!(
        rpc.complete_theatre_signup(absent)
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    let mut scanner = request.clone();
    scanner.explicit_confirmation = false;
    assert_eq!(
        rpc.complete_theatre_signup(scanner)
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    let mut mismatched = request.clone();
    mismatched.operation_id = vec![9; 16];
    assert_eq!(
        rpc.complete_theatre_signup(mismatched)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let mut wrong_proof = request.clone();
    wrong_proof.proof_id = vec![9; 16];
    assert_eq!(
        rpc.complete_theatre_signup(wrong_proof)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let completed = rpc
        .complete_theatre_signup(request.clone())
        .await
        .unwrap()
        .into_inner();
    assert_eq!(completed.outcome, SetupOutcome::Completed as i32);
    assert_eq!(
        rpc.complete_theatre_signup(request.clone())
            .await
            .unwrap()
            .into_inner()
            .outcome,
        SetupOutcome::Replayed as i32
    );
    let mut changed_password = request.clone();
    changed_password.password.push('x');
    assert_eq!(
        rpc.complete_theatre_signup(changed_password)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let mut changed_code = request.clone();
    changed_code.firebase_oob_code.push('x');
    assert_eq!(
        rpc.complete_theatre_signup(changed_code)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let mut replay = request;
    replay.command_id = vec![3; 16];
    assert_eq!(
        rpc.complete_theatre_signup(replay)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let login = AuthenticatePasswordRequest {
        operation_id: vec![4; 16],
        purpose: Purpose::Theatre as i32,
        email: email.into(),
        password: password.clone(),
        client: context(),
    };
    let fact = rpc
        .authenticate_password(login.clone())
        .await
        .unwrap()
        .into_inner()
        .fact
        .unwrap();
    assert_eq!(fact.subject_id, completed.subject_id);
    let known = rpc
        .start_theatre_signup(StartTheatreSignupRequest {
            operation_id: vec![5; 16],
            email: email.into(),
            client: context(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(known, started);
    let (exit, logs) = instance.terminate().await;
    assert!(exit.success());
    assert!(!logs.contains(&code) && !logs.contains(&password));
    let instance = env
        .serve()
        .await
        .ok()
        .expect("restart after native credential commit");
    let mut rpc = client(&instance.socket).await;
    let restored = rpc
        .authenticate_password(login)
        .await
        .unwrap()
        .into_inner()
        .fact
        .unwrap();
    assert_eq!(restored.subject_id, completed.subject_id);
    let (exit, logs) = instance.terminate().await;
    assert!(exit.success());
    assert!(!logs.contains(&code) && !logs.contains(&password));
}
