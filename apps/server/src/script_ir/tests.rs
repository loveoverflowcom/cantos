use std::path::PathBuf;

use serde_json::{Value, json};
use unicode_normalization::UnicodeNormalization;

use super::{ValidationCode, ValidationIssue, parse};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/script-ir/fixtures")
}

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../contracts/script-ir/fixtures/accepted/episode.json"
    ))
    .unwrap()
}

fn first_dialogue(value: &mut Value) -> &mut Value {
    &mut value["episode"]["acts"][0]["scenes"][0]["dialogues"][0]
}

#[test]
fn committed_rejection_corpus_has_exact_ordered_issues() {
    let root = fixture_root().join("rejected");
    let cases: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("expected.json")).unwrap())
            .unwrap();
    for case in cases.as_array().unwrap() {
        let filename = case["file"].as_str().unwrap();
        let input =
            serde_json::from_str(&std::fs::read_to_string(root.join(filename)).unwrap()).unwrap();
        let report = parse(input).unwrap_err();
        assert_eq!(
            serde_json::to_value(report).unwrap()["issues"],
            case["issues"],
            "fixture {filename}"
        );
    }
}

#[test]
fn committed_golden_matches_independent_python_canonical_bytes_and_digest() {
    let content = parse(fixture()).unwrap();
    assert_eq!(
        content.canonical_bytes(),
        include_bytes!("../../../../contracts/script-ir/fixtures/accepted/episode.canonical.json")
            .strip_suffix(b"\n")
            .unwrap()
    );
    assert_eq!(
        content.digest(),
        include_str!("../../../../contracts/script-ir/fixtures/accepted/episode.digest.txt").trim()
    );
    assert_eq!(content, parse(content.as_json()).unwrap());
    assert_eq!(content.work_id(), "work-demo");
    assert_eq!(content.episode_id(), "episode-demo-01");
    assert_eq!(content.source_ref(), "source-demo");
    assert_eq!(content.rights_ref(), "rights-demo");
}

#[test]
fn nfd_whitespace_and_windows_1258_derived_unicode_have_one_value() {
    let mut reference = fixture();
    first_dialogue(&mut reference)["text"] = json!("Người dẫn chuyện…");
    let expected = parse(reference.clone()).unwrap();
    let inputs = [
        "Người dẫn chuyện…".nfd().collect::<String>(),
        "  Người\t dẫn\u{00a0}chuyện…\n".into(),
        "Ngươ\u{0300}i dẫn chuyện…".into(),
        "Người\u{200b} dẫn\u{feff} chuyện\u{00ad}…".into(),
    ];
    for input in inputs {
        let mut value = reference.clone();
        first_dialogue(&mut value)["text"] = json!(input);
        let actual = parse(value).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.canonical_bytes(), expected.canonical_bytes());
        assert_eq!(actual.digest(), expected.digest());
    }
}

#[test]
fn vietnamese_tone_placement_and_punctuation_remain_meaningful() {
    for (a, b) in [("hòa", "hoà"), ("thúy", "thuý"), ("Đ", "Ð"), ("…", "...")] {
        let mut left = fixture();
        first_dialogue(&mut left)["text"] = json!(a);
        let mut right = fixture();
        first_dialogue(&mut right)["text"] = json!(b);
        assert_ne!(
            parse(left).unwrap().digest(),
            parse(right).unwrap().digest()
        );
    }
}

#[test]
fn optional_absence_empty_lists_and_null_options_canonicalize_identically() {
    let original = parse(fixture()).unwrap();
    let mut value = fixture();
    first_dialogue(&mut value)["pronunciation"] = json!([]);
    value["characters"][1]["characterization"] = Value::Null;
    value["episode"]["acts"][0]["scenes"][1]["sound_cues"] = json!([]);
    value["episode"]["acts"][0]["scenes"][0]["sound_cues"][0]["asset_ref"] = Value::Null;
    value["episode"]["acts"][0]["scenes"][0]["sound_cues"][0]["rights_ref"] = Value::Null;
    let actual = parse(value).unwrap();
    assert_eq!(original, actual);
    assert_eq!(original.canonical_bytes(), actual.canonical_bytes());
}

#[test]
fn characters_are_a_set_but_dialogue_order_is_performance_content() {
    let original = parse(fixture()).unwrap();
    let mut value = fixture();
    value["characters"].as_array_mut().unwrap().reverse();
    assert_eq!(parse(value.clone()).unwrap().digest(), original.digest());
    value["episode"]["acts"][0]["scenes"][0]["dialogues"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_ne!(parse(value).unwrap().digest(), original.digest());
}

#[test]
fn provenance_and_rights_references_are_preserved_outside_performance_digest() {
    let original = parse(fixture()).unwrap();
    let mut value = fixture();
    value["work"]["source_ref"] = json!("source-other");
    value["work"]["rights_ref"] = json!("rights-other");
    let actual = parse(value).unwrap();
    assert_eq!(actual.digest(), original.digest());
    assert_eq!(actual.source_ref(), "source-other");
    assert_eq!(actual.rights_ref(), "rights-other");
    assert_ne!(actual.as_json(), original.as_json());
}

#[test]
fn cue_asset_identity_changes_performance_digest_but_its_rights_reference_does_not() {
    let original = parse(fixture()).unwrap();
    let mut value = fixture();
    let cue = &mut value["episode"]["acts"][0]["scenes"][0]["sound_cues"][0];
    cue["asset_ref"] = json!("asset-room");
    cue["rights_ref"] = json!("rights-room");
    let selected = parse(value.clone()).unwrap();
    assert_ne!(original.digest(), selected.digest());
    value["episode"]["acts"][0]["scenes"][0]["sound_cues"][0]["rights_ref"] =
        json!("rights-room-v2");
    assert_eq!(selected.digest(), parse(value).unwrap().digest());
}

#[test]
fn detached_json_cannot_mutate_validated_content() {
    let content = parse(fixture()).unwrap();
    let original = content.digest();
    let mut detached = content.as_json();
    first_dialogue(&mut detached)["text"] = json!("");
    assert_eq!(content.digest(), original);
    assert_eq!(
        parse(detached).unwrap_err().issues,
        vec![ValidationIssue {
            code: ValidationCode::EmptyText,
            path: "/episode/acts/0/scenes/0/dialogues/0/text".into(),
        }]
    );
}

#[test]
fn structural_errors_do_not_echo_private_source_or_unknown_fields() {
    let mut value = fixture();
    first_dialogue(&mut value)["private-manuscript-excerpt"] = json!("private text");
    assert_eq!(
        serde_json::to_value(parse(value).unwrap_err()).unwrap(),
        json!({"issues":[{"code":"invalid_shape","path":"/"}]})
    );
}

#[test]
fn string_and_integer_boundaries_are_enforced_exactly() {
    let mut value = fixture();
    value["work"]["id"] = json!("a".repeat(64));
    first_dialogue(&mut value)["text"] = json!("x".repeat(16384));
    first_dialogue(&mut value)["delivery"]["intensity_permille"] = json!(1000);
    assert!(parse(value.clone()).is_ok());
    value["work"]["id"] = json!("a".repeat(65));
    first_dialogue(&mut value)["text"] = json!("x".repeat(16385));
    first_dialogue(&mut value)["delivery"]["intensity_permille"] = json!(1001);
    assert_eq!(
        serde_json::to_value(parse(value).unwrap_err()).unwrap(),
        json!({"issues":[
            {"code":"invalid_id","path":"/work/id"},
            {"code":"text_too_long","path":"/episode/acts/0/scenes/0/dialogues/0/text"},
            {"code":"invalid_intensity","path":"/episode/acts/0/scenes/0/dialogues/0/delivery/intensity_permille"}
        ]})
    );
}

#[test]
fn representative_performance_fields_change_the_content_digest() {
    let original = parse(fixture()).unwrap().digest();
    let mutations = [
        ("/work/id", json!("work-other")),
        ("/work/title", json!("A title")),
        ("/adaptation/id", json!("adaptation-other")),
        ("/adaptation/language", json!("en")),
        ("/characters/0/name", json!("Narrator")),
        ("/characters/0/characterization", json!("Observant")),
        ("/episode/id", json!("episode-other")),
        ("/episode/title", json!("An episode")),
        ("/episode/acts/0/id", json!("act-other")),
        ("/episode/acts/0/title", json!("An act")),
        ("/episode/acts/0/scenes/0/id", json!("scene-other")),
        ("/episode/acts/0/scenes/0/title", json!("A scene")),
        (
            "/episode/acts/0/scenes/0/dialogues/0/speaker_id",
            json!("an"),
        ),
        ("/episode/acts/0/scenes/0/dialogues/0/text", json!("A line")),
        (
            "/episode/acts/0/scenes/0/dialogues/0/delivery/emotion",
            json!("sad"),
        ),
        (
            "/episode/acts/0/scenes/0/dialogues/0/delivery/intensity_permille",
            json!(301),
        ),
        (
            "/episode/acts/0/scenes/0/sound_cues/0/id",
            json!("cue-other"),
        ),
        ("/episode/acts/0/scenes/0/sound_cues/0/kind", json!("music")),
        (
            "/episode/acts/0/scenes/0/sound_cues/0/description",
            json!("Rain"),
        ),
        (
            "/episode/acts/0/scenes/0/sound_cues/0/anchor/dialogue_id",
            json!("dialogue-02"),
        ),
        (
            "/episode/acts/0/scenes/0/sound_cues/0/anchor/edge",
            json!("end"),
        ),
        (
            "/episode/acts/0/scenes/1/dialogues/0/pronunciation/0/respelling",
            json!("Vọng đài"),
        ),
    ];
    for (path, replacement) in mutations {
        let mut value = fixture();
        *value.pointer_mut(path).unwrap() = replacement;
        assert_ne!(original, parse(value).unwrap().digest(), "field {path}");
    }
}

#[test]
fn mathematical_integer_spellings_share_one_canonical_value() {
    let original = parse(fixture()).unwrap();
    for number in ["300", "300.0", "3e2"] {
        let mut value = fixture();
        first_dialogue(&mut value)["delivery"]["intensity_permille"] =
            serde_json::from_str(number).unwrap();
        assert_eq!(parse(value).unwrap(), original);
    }
}
