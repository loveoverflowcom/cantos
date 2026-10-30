//! Identity's server-side binding against the Identity-owned wire goldens. The bytes in
//! `contracts/identity/v1/fixtures/wire-goldens.json` come from an independent byte-level encoder
//! written from the Protobuf encoding spec, not from prost output. The Server checks its own
//! client binding against the same file; fact acceptance policy is tested in the Server.
#![cfg(feature = "service")]

use cantos_identity::rpc::generated::*;
use prost::Message;

fn decode_hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn golden(name: &str) -> Vec<u8> {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../contracts/identity/v1/fixtures/wire-goldens.json"
    ))
    .unwrap();
    let case = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap();
    decode_hex(case["protobuf_hex"].as_str().unwrap())
}

fn fact() -> AuthenticationFact {
    AuthenticationFact {
        subject_id: vec![2; 16],
        purpose: Purpose::Theatre as i32,
        assurance: Assurance::Password as i32,
        authenticated_at_unix_micros: 1_700_000_000_123_456,
        security_version: 3,
        instance_epoch: vec![3; 16],
        operation_id: vec![4; 16],
    }
}

#[test]
fn start_request_matches_the_independent_golden_in_both_directions() {
    let message = StartTheatreSignupRequest {
        operation_id: vec![1; 16],
        email: "a@b.vn".to_owned(),
        client: Some(ClientContext {
            client_ip: "10.0.0.1".to_owned(),
        }),
    };
    assert_eq!(
        message.encode_to_vec(),
        golden("start_theatre_signup_request")
    );
    assert_eq!(
        StartTheatreSignupRequest::decode(golden("start_theatre_signup_request").as_slice())
            .unwrap(),
        message
    );
}

#[test]
fn fact_matches_the_independent_golden_including_wide_varints() {
    assert_eq!(fact().encode_to_vec(), golden("authentication_fact"));
    assert_eq!(
        AuthenticationFact::decode(golden("authentication_fact").as_slice()).unwrap(),
        fact()
    );
}

#[test]
fn every_message_round_trips_and_truncation_is_refused() {
    let complete = CompleteTheatreSignupRequest {
        command_id: vec![1; 16],
        operation_id: vec![2; 16],
        proof_id: vec![3; 16],
        proof_secret: vec![4; 32],
        firebase_oob_code: String::new(),
        password: "một mật khẩu dài".to_owned(),
        explicit_confirmation: true,
        client: Some(ClientContext {
            client_ip: "2001:db8::1".to_owned(),
        }),
    };
    let bytes = complete.encode_to_vec();
    assert_eq!(
        CompleteTheatreSignupRequest::decode(bytes.as_slice()).unwrap(),
        complete
    );
    assert!(CompleteTheatreSignupRequest::decode(&bytes[..bytes.len() - 3]).is_err());
    let response = CompleteTheatreSignupResponse {
        subject_id: vec![5; 16],
        instance_epoch: vec![6; 16],
        outcome: SetupOutcome::Replayed as i32,
    };
    assert_eq!(
        CompleteTheatreSignupResponse::decode(response.encode_to_vec().as_slice()).unwrap(),
        response
    );
    let state = GetSecurityStateResponse {
        subject_id: vec![5; 16],
        state: AccountState::Disabled as i32,
        security_version: u64::MAX,
        instance_epoch: vec![6; 16],
    };
    assert_eq!(
        GetSecurityStateResponse::decode(state.encode_to_vec().as_slice()).unwrap(),
        state
    );
}

#[test]
fn debug_of_secret_bearing_requests_never_prints_content() {
    let complete = CompleteTheatreSignupRequest {
        proof_secret: vec![0xAB; 32],
        firebase_oob_code: String::new(),
        password: "correct horse battery".to_owned(),
        ..Default::default()
    };
    let login = AuthenticatePasswordRequest {
        email: "someone@example.com".to_owned(),
        password: "correct horse battery".to_owned(),
        ..Default::default()
    };
    let start = StartTheatreSignupRequest {
        email: "someone@example.com".to_owned(),
        ..Default::default()
    };
    for text in [
        format!("{complete:?}"),
        format!("{login:?}"),
        format!("{start:?}"),
    ] {
        assert!(text.contains("[REDACTED]"));
        assert!(!text.contains("correct"));
        assert!(!text.contains("someone"));
        assert!(!text.contains("171"));
    }
}

#[test]
fn unknown_enum_numbers_do_not_become_known_values() {
    assert!(Purpose::try_from(99).is_err());
    assert!(Assurance::try_from(99).is_err());
    assert!(AccountState::try_from(99).is_err());
    assert!(SetupOutcome::try_from(99).is_err());
}

#[test]
fn mailbox_messages_match_independent_wire_goldens() {
    let request = CanonicalizeMailboxRequest {
        operation_id: vec![4; 16],
        email: "A+tag@EXAMPLE.COM".into(),
    };
    assert_eq!(
        request.encode_to_vec(),
        golden("canonicalize_mailbox_request")
    );
    let fact = VerifiedMailboxFact {
        operation_id: vec![4; 16],
        subject_id: vec![2; 16],
        security_version: 3,
        instance_epoch: vec![3; 16],
        canonical_email: "A+tag@example.com".into(),
        observed_at_unix_micros: 1_700_000_000_123_456,
    };
    assert_eq!(fact.encode_to_vec(), golden("verified_mailbox_fact"));
    assert_eq!(
        VerifiedMailboxFact::decode(golden("verified_mailbox_fact").as_slice()).unwrap(),
        fact
    );
}
