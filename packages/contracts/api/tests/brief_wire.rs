use cantos_contracts::*;
use serde_json::{json, Value};

const NARRATION: &str = include_str!("../../fixtures/briefs/v1/create-narration.json");
const RECORDING: &str = include_str!("../../fixtures/briefs/v1/create-recording.json");
#[test]
fn independent_narration_and_recording_fixtures_round_trip_without_cast_or_script() {
    for fixture in [NARRATION, RECORDING] {
        let raw: CreateProductionBriefRequest = serde_json::from_str(fixture).unwrap();
        assert_eq!(raw.guidance.voice, None);
        assert_eq!(raw.guidance.sound_design, None);
        assert_eq!(
            serde_json::to_value(raw).unwrap(),
            serde_json::from_str::<Value>(fixture).unwrap()
        );
    }
    let fixture = include_str!("../../fixtures/briefs/v1/narration-document.json");
    let value: ProductionBriefDocument = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        serde_json::from_str::<Value>(fixture).unwrap()
    );
}
#[test]
fn required_profile_and_closed_guidance_and_recording_fields_are_transport_barriers() {
    let raw: Value = serde_json::from_str(NARRATION).unwrap();
    for path in ["top", "editorial", "voice", "delivery"] {
        let mut value = raw.clone();
        match path {
            "top" => value["unknown"] = json!(true),
            "editorial" => value["guidance"]["editorial"]["provider"] = json!("fake"),
            "voice" => {
                value["guidance"]["voice"] = json!({"revision":1,"notes":"","pronunciation":[],"casting":[],"model":"secret"})
            }
            "delivery" => {
                value["guidance"]["delivery"]
                    .as_object_mut()
                    .unwrap()
                    .remove("profile_id");
            }
            _ => unreachable!(),
        }
        assert!(
            serde_json::from_value::<CreateProductionBriefRequest>(value).is_err(),
            "{path}"
        );
    }
    assert!(serde_json::from_value::<ProductionInstructions>(
        json!({"kind":"recording","script":{}})
    )
    .is_err());
    assert!(serde_json::from_value::<ProducerCapability>(json!("tts_provider")).is_err());
}
