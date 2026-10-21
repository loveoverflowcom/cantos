//! Wire goldens are encoded by an independent byte-level encoder (a Python script written from
//! the protobuf encoding spec, not from prost output) and pasted as literals.

use cantos_identity_rpc::*;
use prost::Message;

fn decode_hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

const START_HEX: &str =
    "0a100101010101010101010101010101010112066140622e766e1a0a0a0831302e302e302e31";
const FACT_HEX: &str = "0a10020202020202020202020202020202021001180120c0c480c1c1c4820328033210030303030303030303030303030303033a1004040404040404040404040404040404";

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
    assert_eq!(message.encode_to_vec(), decode_hex(START_HEX));
    assert_eq!(
        StartTheatreSignupRequest::decode(decode_hex(START_HEX).as_slice()).unwrap(),
        message
    );
}

#[test]
fn fact_matches_the_independent_golden_including_wide_varints() {
    assert_eq!(fact().encode_to_vec(), decode_hex(FACT_HEX));
    assert_eq!(
        AuthenticationFact::decode(decode_hex(FACT_HEX).as_slice()).unwrap(),
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
    let validated = validate_fact(&fact(), [4; 16], Purpose::Theatre).unwrap();
    let text = format!("{validated:?}");
    assert!(!text.contains("subject") && !text.contains("epoch"));
}

#[test]
fn a_fact_is_accepted_only_for_its_operation_purpose_and_assurance() {
    let accepted = validate_fact(&fact(), [4; 16], Purpose::Theatre).unwrap();
    assert_eq!(accepted.subject, [2; 16]);
    assert_eq!(accepted.security_version, 3);
    assert_eq!(accepted.authenticated_at_unix_micros, 1_700_000_000_123_456);
    assert_eq!(
        validate_fact(&fact(), [9; 16], Purpose::Theatre),
        Err(FactRejected::WrongOperation)
    );
    assert_eq!(
        validate_fact(&fact(), [4; 16], Purpose::Studio),
        Err(FactRejected::WrongPurpose)
    );
    let mut studio = fact();
    studio.purpose = Purpose::Studio as i32;
    assert_eq!(
        validate_fact(&studio, [4; 16], Purpose::Studio),
        Err(FactRejected::WrongPurpose)
    );
    for assurance in [0, 2, 99] {
        let mut weak = fact();
        weak.assurance = assurance;
        assert_eq!(
            validate_fact(&weak, [4; 16], Purpose::Theatre),
            Err(FactRejected::UnsupportedAssurance),
            "assurance {assurance}"
        );
    }
}

#[test]
fn malformed_facts_are_rejected_before_any_binding_decision() {
    type Mutation = fn(&mut AuthenticationFact);
    let mutations: [(&str, Mutation); 9] = [
        ("zero subject", |f| f.subject_id = vec![0; 16]),
        ("short subject", |f| f.subject_id = vec![2; 15]),
        ("long epoch", |f| f.instance_epoch = vec![3; 17]),
        ("zero epoch", |f| f.instance_epoch = vec![0; 16]),
        ("zero operation", |f| f.operation_id = vec![0; 16]),
        ("zero version", |f| f.security_version = 0),
        ("version beyond storage", |f| f.security_version = u64::MAX),
        ("negative time", |f| f.authenticated_at_unix_micros = -1),
        ("unknown purpose", |f| f.purpose = 77),
    ];
    for (name, mutate) in mutations {
        let mut candidate = fact();
        mutate(&mut candidate);
        assert_eq!(
            validate_fact(&candidate, [4; 16], Purpose::Theatre),
            Err(FactRejected::Malformed),
            "{name}"
        );
    }
}

#[test]
fn unknown_enum_numbers_do_not_become_known_values() {
    assert!(Purpose::try_from(99).is_err());
    assert!(Assurance::try_from(99).is_err());
    assert!(AccountState::try_from(99).is_err());
    assert!(SetupOutcome::try_from(99).is_err());
}
