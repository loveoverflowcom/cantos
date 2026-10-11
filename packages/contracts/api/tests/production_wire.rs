use cantos_contracts::{
    ProductionEstimate, ProductionRightsScope, ProductionRightsStatus, SaveProductionRightsRequest,
    SaveProductionSettingsRequest,
};
use serde_json::{json, Value};

const SETTINGS: &str = include_str!("../../fixtures/production/v1/settings-request.json");
const RIGHTS: &str = include_str!("../../fixtures/production/v1/rights-request.json");

#[test]
fn independently_authored_requests_preserve_scope_and_pending_claim_without_spend_fiction() {
    let settings: SaveProductionSettingsRequest = serde_json::from_str(SETTINGS).unwrap();
    assert_eq!(
        serde_json::to_value(settings).unwrap(),
        serde_json::from_str::<Value>(SETTINGS).unwrap()
    );
    let rights: SaveProductionRightsRequest = serde_json::from_str(RIGHTS).unwrap();
    assert_eq!(rights.claim.status, ProductionRightsStatus::Pending);
    assert_eq!(
        rights.claim.scope,
        ProductionRightsScope::ProductionSynthesis
    );
    assert_eq!(rights.claim.languages, vec!["vi-VN"]);
    assert_eq!(rights.claim.territory, "private-planning");
    assert_eq!(
        serde_json::to_value(rights).unwrap(),
        serde_json::from_str::<Value>(RIGHTS).unwrap()
    );
}

#[test]
fn raw_request_transport_refuses_forged_authority_and_noninteger_money() {
    let settings: Value = serde_json::from_str(SETTINGS).unwrap();
    for path in [
        "approved",
        "actor_id",
        "actual_charge_minor",
        "reservation_id",
    ] {
        let mut value = settings.clone();
        value[path] = json!(true);
        assert!(
            serde_json::from_value::<SaveProductionSettingsRequest>(value).is_err(),
            "{path}"
        );
    }
    for value in [json!(-1), json!(1.5), json!("100"), json!(null)] {
        let mut settings = settings.clone();
        settings["settings"]["budget"]["limit_minor"] = value;
        assert!(serde_json::from_value::<SaveProductionSettingsRequest>(settings).is_err());
    }
    let rights: Value = serde_json::from_str(RIGHTS).unwrap();
    for field in [
        "legally_verified",
        "approved_by",
        "provider_consent_attested",
    ] {
        let mut rights = rights.clone();
        rights["claim"][field] = json!(true);
        assert!(serde_json::from_value::<SaveProductionRightsRequest>(rights).is_err());
    }
}

#[test]
fn rights_states_and_estimate_tags_are_closed_with_unavailable_distinct_from_zero() {
    for (spelling, expected) in [
        ("pending", ProductionRightsStatus::Pending),
        ("granted", ProductionRightsStatus::Granted),
        ("revoked", ProductionRightsStatus::Revoked),
    ] {
        assert_eq!(
            serde_json::from_value::<ProductionRightsStatus>(json!(spelling)).unwrap(),
            expected
        );
        assert_eq!(serde_json::to_value(expected).unwrap(), json!(spelling));
    }
    assert!(serde_json::from_value::<ProductionRightsStatus>(json!("verified")).is_err());
    assert!(serde_json::from_value::<ProductionRightsScope>(json!("public_distribution")).is_err());
    let unavailable = json!({"status":"unavailable","reason":"planning_rate_unrecorded"});
    assert_eq!(
        serde_json::from_value::<ProductionEstimate>(unavailable.clone()).unwrap(),
        ProductionEstimate::Unavailable {
            reason: "planning_rate_unrecorded".into()
        }
    );
    assert_eq!(
        serde_json::to_value(ProductionEstimate::Unavailable {
            reason: "planning_rate_unrecorded".into()
        })
        .unwrap(),
        unavailable
    );
    let mut fabricated = unavailable;
    fabricated["amount_minor"] = json!(0);
    assert!(serde_json::from_value::<ProductionEstimate>(fabricated).is_err());
    assert!(serde_json::from_value::<ProductionEstimate>(
        json!({"status":"actual_charge","amount_minor":0})
    )
    .is_err());
}
