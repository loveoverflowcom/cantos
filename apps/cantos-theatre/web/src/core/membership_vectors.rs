//! Hand-authored preview expectations shared with the CMP test consumer.

use std::collections::BTreeMap;

use serde_json::Value;

use super::membership::{
    validate, FinishOutcome, MembershipDraft, MembershipScreen, MembershipState, PlansOrigin,
    PlansStatus, SubmissionAttempt,
};

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().expect("fixture string")
}

fn corpus() -> Value {
    serde_json::from_str(include_str!("../../fixtures/membership-interest-v1.json"))
        .expect("valid shared preview corpus")
}

#[test]
fn shared_membership_validation_vectors() {
    let corpus = corpus();
    assert_eq!(corpus["schemaVersion"], 1);
    let vectors = corpus["validation"].as_array().expect("validation vectors");
    assert_eq!(
        vectors.len(),
        12,
        "corpus selection must not silently shrink"
    );
    for vector in vectors {
        let actual = validate(&MembershipDraft {
            name: text(vector, "name").into(),
            email: text(vector, "email").into(),
        })
        .into_iter()
        .map(|error| Value::String(format!("{error:?}")))
        .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vector["errors"]
                .as_array()
                .expect("expected errors")
                .clone(),
            "{}",
            text(vector, "id")
        );
    }
}

#[test]
fn shared_membership_transition_vectors() {
    let corpus = corpus();
    let scenarios = corpus["scenarios"].as_array().expect("scenario vectors");
    assert_eq!(
        scenarios.len(),
        7,
        "corpus selection must not silently shrink"
    );
    for scenario in scenarios {
        let mut state = match text(scenario, "mode") {
            "fixture" => MembershipState::fixture(),
            "unavailable" => MembershipState::default(),
            other => panic!("unknown mode {other}"),
        };
        let mut attempts: BTreeMap<String, SubmissionAttempt> = BTreeMap::new();
        for (index, action) in scenario["actions"]
            .as_array()
            .expect("actions")
            .iter()
            .enumerate()
        {
            let label = format!(
                "{} action {index}: {}",
                text(scenario, "id"),
                text(action, "type")
            );
            let accepted = match text(action, "type") {
                "open-form" => Some(state.open_form()),
                "open-intro" => {
                    state.open_intro();
                    None
                }
                "open-account" => {
                    state.open_account();
                    None
                }
                "show-received" => Some(state.show_received()),
                "account-registered" => Some(state.show_account_registered_fixture()),
                "open-plans" => Some(state.open_plans()),
                "deep-plans" => {
                    state.open_plans_deep_link();
                    None
                }
                "back" => {
                    state.back();
                    None
                }
                "clear-session" => {
                    state.clear_session();
                    None
                }
                "plans-status" => {
                    state.set_plans_status(match text(action, "value") {
                        "unpublished" => PlansStatus::Unpublished,
                        "loading" => PlansStatus::Loading,
                        "empty" => PlansStatus::Empty,
                        "unavailable" => PlansStatus::Unavailable,
                        "failed" => PlansStatus::Failed,
                        other => panic!("unknown plan status {other}"),
                    });
                    None
                }
                "edit-name" | "edit-email" => {
                    let result = if text(action, "type") == "edit-name" {
                        state.set_name(text(action, "value").into())
                    } else {
                        state.set_email(text(action, "value").into())
                    };
                    assert_eq!(
                        result.err().map(|error| format!("{error:?}")),
                        action["expectedRefusal"].as_str().map(str::to_owned),
                        "{label}"
                    );
                    None
                }
                "submit" => {
                    match state.begin_submission() {
                        Err(error) => assert_eq!(
                            format!("{error:?}"),
                            text(action, "expectedRefusal"),
                            "{label}"
                        ),
                        Ok(attempt) => {
                            assert!(
                                action["expectedRefusal"].is_null(),
                                "unexpected dispatch: {label}"
                            );
                            for (key, actual) in [
                                ("operationId", attempt.operation_id()),
                                ("requestId", attempt.request_id()),
                            ] {
                                if let Some(expected) = action[key].as_u64() {
                                    assert_eq!(actual, expected, "{label}: {key}");
                                }
                            }
                            for (key, actual) in [
                                ("payloadName", attempt.payload().name()),
                                ("payloadEmail", attempt.payload().email()),
                            ] {
                                if let Some(expected) = action[key].as_str() {
                                    assert_eq!(actual, expected, "{label}: {key}");
                                }
                            }
                            attempts.insert(text(action, "saveAs").into(), attempt);
                        }
                    }
                    None
                }
                "finish" => Some(
                    state.finish(
                        attempts
                            .get(text(action, "attempt"))
                            .expect("saved attempt"),
                        match text(action, "outcome") {
                            "received" => FinishOutcome::Received,
                            "failed" => FinishOutcome::Failed,
                            "unknown" => FinishOutcome::Unknown,
                            other => panic!("unknown outcome {other}"),
                        },
                    ),
                ),
                other => panic!("unknown action {other}"),
            };
            if let Some(expected) = action["accepted"].as_bool() {
                assert_eq!(accepted, Some(expected), "{label}");
            }
            if let Some(expected) = action["expect"].as_object() {
                for (key, expected) in expected {
                    let actual = match key.as_str() {
                        "screen" => Value::String(screen(state.screen()).into()),
                        "submission" => {
                            Value::String(format!("{:?}", state.submission()).to_lowercase())
                        }
                        "receipt" => Value::Bool(state.receipt().is_some()),
                        "name" => Value::String(state.draft().name.clone()),
                        "email" => Value::String(state.draft().email.clone()),
                        "plansStatus" => {
                            Value::String(format!("{:?}", state.plans_status()).to_lowercase())
                        }
                        other => panic!("unknown projection {other}"),
                    };
                    assert_eq!(&actual, expected, "{label}: {key}");
                }
            }
        }
    }
}

fn screen(screen: MembershipScreen) -> &'static str {
    match screen {
        MembershipScreen::Intro => "intro",
        MembershipScreen::Form => "form",
        MembershipScreen::Received => "received",
        MembershipScreen::AccountRegistered => "account-registered",
        MembershipScreen::Account => "account",
        MembershipScreen::Plans(PlansOrigin::Intro) => "plans:intro",
        MembershipScreen::Plans(PlansOrigin::Account) => "plans:account",
        MembershipScreen::Plans(PlansOrigin::AccountRegistered) => "plans:account-registered",
        MembershipScreen::Plans(PlansOrigin::Received) => "plans:received",
    }
}
