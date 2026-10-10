use std::{collections::BTreeMap, fs, path::PathBuf};

use cantos_server::script_ir::{ReadError, ScriptContent, read_canonical_script, read_script};
use serde_json::{Value, json};
use unicode_normalization::UnicodeNormalization;

fn corpus() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts/fixtures/script-ir/0.1.0")
}

fn load(name: &str) -> Value {
    serde_json::from_slice(&fs::read(corpus().join(name)).unwrap()).unwrap()
}

fn admit(value: &Value) -> ScriptContent {
    read_script(&serde_json::to_vec(value).unwrap()).unwrap()
}

fn speech(script: &ScriptContent) -> BTreeMap<String, Vec<u8>> {
    script
        .spoken_lines()
        .map(|line| (line.dialogue_id().into(), line.canonical_bytes()))
        .collect()
}

fn schema() -> jsonschema::Validator {
    let source = include_str!("../../../contracts/schema/script-ir/0.1.0.schema.json");
    let value: Value = serde_json::from_str(source).unwrap();
    assert!(jsonschema::draft202012::meta::is_valid(&value));
    jsonschema::draft202012::new(&value).unwrap()
}

fn shape_accepts(value: &Value) -> bool {
    matches!(
        read_script(&serde_json::to_vec(value).unwrap()),
        Ok(_) | Err(ReadError::Semantic(_))
    )
}

#[test]
fn accepted_corpus_matches_independent_python_bytes_and_sha256_goldens() {
    let schema = schema();
    for name in ["two-scenes", "provenance"] {
        let raw = load(&format!("accept/{name}.json"));
        assert!(schema.is_valid(&raw));
        let script = admit(&raw);
        let expected = load(&format!("accept/{name}.expected.json"));
        assert_eq!(
            script.content_digest().to_string(),
            expected["content_digest"].as_str().unwrap()
        );
        assert_eq!(
            script.canonical_bytes(),
            expected["canonical_content"].as_str().unwrap().as_bytes()
        );
        let expected_speech: BTreeMap<_, _> = expected["spoken_content"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(id, text)| (id.clone(), text.as_str().unwrap().as_bytes().to_vec()))
            .collect();
        assert_eq!(speech(&script), expected_speech);
        let export = script.export_bytes();
        assert!(schema.is_valid(&serde_json::from_slice(&export).unwrap()));
        assert_eq!(
            read_canonical_script(&export).unwrap().export_bytes(),
            export
        );
    }
}

#[test]
fn rejected_corpus_has_exact_ordered_diagnostics_and_schema_shape_parity() {
    let schema = schema();
    for case in load("cases.json").as_array().unwrap() {
        let raw = load(case["file"].as_str().unwrap());
        let result = read_script(&serde_json::to_vec(&raw).unwrap());
        let stage = case["stage"].as_str().unwrap();
        assert_eq!(
            schema.is_valid(&raw),
            stage == "semantic",
            "{}",
            case["file"]
        );
        match result {
            Err(ReadError::Semantic(report)) => {
                assert_eq!(stage, "semantic", "{}", case["file"]);
                assert_eq!(
                    serde_json::to_value(report).unwrap(),
                    case["diagnostics"],
                    "{}",
                    case["file"]
                );
            }
            Err(ReadError::InvalidDocument { .. }) => assert_eq!(stage, "invalid_document"),
            Err(ReadError::Shape(report)) => {
                assert_eq!(stage, "shape");
                assert_eq!(
                    serde_json::to_value(report).unwrap(),
                    case["shape_diagnostics"]
                );
            }
            Err(ReadError::MissingSchemaVersion) => assert_eq!(stage, "missing_version"),
            Err(ReadError::UnsupportedSchemaVersion { found, supported }) => {
                assert_eq!(stage, "unsupported_version");
                assert_eq!(found, "0.2.0");
                assert_eq!(supported, "0.1.0");
            }
            other => panic!("unexpected admission result for {case}: {other:?}"),
        }
    }
}

fn object_paths(value: &Value, path: String, paths: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            paths.push(path.clone());
            for (name, child) in fields {
                object_paths(child, format!("{path}/{name}"), paths);
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                object_paths(child, format!("{path}/{i}"), paths);
            }
        }
        _ => {}
    }
}

#[test]
fn schema_and_rust_shape_agree_on_missing_unknown_and_null_fields_at_every_object() {
    let schema = schema();
    let base = load("accept/provenance.json");
    let mut paths = Vec::new();
    object_paths(&base, String::new(), &mut paths);
    for path in paths {
        let mut unknown = base.clone();
        unknown
            .pointer_mut(&path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("future_field".into(), json!(true));
        assert!(
            !schema.is_valid(&unknown),
            "schema accepted unknown field at {path}"
        );
        assert!(
            !shape_accepts(&unknown),
            "Rust accepted unknown field at {path}"
        );
        for key in base.pointer(&path).unwrap().as_object().unwrap().keys() {
            let mut removed = base.clone();
            removed
                .pointer_mut(&path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert_eq!(
                schema.is_valid(&removed),
                shape_accepts(&removed),
                "missing {path}/{key}"
            );
            let mut nulled = base.clone();
            nulled
                .pointer_mut(&path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), Value::Null);
            assert_eq!(
                schema.is_valid(&nulled),
                shape_accepts(&nulled),
                "null {path}/{key}"
            );
        }
    }
}

#[test]
fn numeric_spellings_and_id_text_array_limits_agree_with_schema() {
    let schema = schema();
    let base = load("accept/two-scenes.json");
    for (path, values) in [
        (
            "/episode/acts/0/scenes/0/dialogues/0/delivery/intensity_permille",
            vec![
                json!(0),
                json!(1000),
                json!(1001),
                json!(-1),
                json!(300.0),
                json!(0.5),
                json!(65536),
                json!("300"),
            ],
        ),
        (
            "/work/id",
            vec![
                json!(""),
                json!("a".repeat(64)),
                json!("a".repeat(65)),
                json!("Work"),
                json!("a\n"),
                json!("đ"),
                json!("a_1-z"),
            ],
        ),
        (
            "/work/title",
            vec![
                json!(""),
                json!("ậ".repeat(10000)),
                json!("ậ".repeat(10001)),
                json!(null),
            ],
        ),
        (
            "/characters",
            vec![
                json!([]),
                Value::Array(vec![base["characters"][0].clone(); 10000]),
                Value::Array(vec![base["characters"][0].clone(); 10001]),
            ],
        ),
    ] {
        for value in values {
            let mut raw = base.clone();
            *raw.pointer_mut(path).unwrap() = value;
            assert_eq!(
                schema.is_valid(&raw),
                shape_accepts(&raw),
                "shape parity at {path}"
            );
        }
    }
}

#[test]
fn malformed_duplicate_keys_unsupported_history_and_oversized_input_fail_explicitly() {
    assert!(matches!(
        read_script(b"{"),
        Err(ReadError::InvalidDocument { .. })
    ));
    let bytes = serde_json::to_string(&load("accept/two-scenes.json")).unwrap();
    let duplicate = bytes.replacen("\"title\":", "\"title\":\"extra\",\"title\":", 1);
    assert!(matches!(
        read_script(duplicate.as_bytes()),
        Err(ReadError::InvalidDocument { .. })
    ));
    let history = include_bytes!("../../../contracts/examples/episode-draft.json");
    assert_eq!(
        read_script(history).unwrap_err(),
        ReadError::UnsupportedSchemaVersion {
            found: "0.1.0-draft".into(),
            supported: "0.1.0"
        }
    );
    assert_eq!(
        read_script(&vec![b' '; 2 * 1024 * 1024 + 1]).unwrap_err(),
        ReadError::DocumentTooLarge {
            max_bytes: 2 * 1024 * 1024
        }
    );
}

fn transform_text(value: &mut Value, transform: &impl Fn(&str) -> String) {
    match value {
        Value::Object(fields) => {
            for (key, child) in fields {
                if matches!(
                    key.as_str(),
                    "text"
                        | "name"
                        | "title"
                        | "personality"
                        | "description"
                        | "surface"
                        | "replacement"
                ) {
                    *child = json!(transform(child.as_str().unwrap()));
                } else {
                    transform_text(child, transform);
                }
            }
        }
        Value::Array(items) => {
            for child in items {
                transform_text(child, transform);
            }
        }
        _ => {}
    }
}

#[test]
fn nfd_whitespace_key_order_and_empty_optional_arrays_have_one_canonical_value() {
    let base = load("accept/two-scenes.json");
    let expected = admit(&base);
    let mut variant = base.clone();
    transform_text(&mut variant, &|text| {
        format!(
            "\t{}\u{a0}",
            text.nfd().collect::<String>().replace(' ', "\n\t")
        )
    });
    // U+200B removal joins a base and combining mark, so NFC must happen after filtering.
    variant["characters"][1]["name"] = json!("A\u{200b}n");
    for dialogue in variant["episode"]["acts"][0]["scenes"][0]["dialogues"]
        .as_array_mut()
        .unwrap()
    {
        dialogue
            .as_object_mut()
            .unwrap()
            .remove("pronunciation_overrides");
    }
    let actual = read_script(&serde_json::to_vec_pretty(&variant).unwrap()).unwrap();
    assert_eq!(actual.export_bytes(), expected.export_bytes());
    assert_eq!(actual.content_digest(), expected.content_digest());
    assert_eq!(speech(&actual), speech(&expected));
    let canonical = actual.export_bytes();
    assert_eq!(
        admit(&serde_json::from_slice(&canonical).unwrap()).export_bytes(),
        canonical
    );
    assert_eq!(
        read_canonical_script(&serde_json::to_vec_pretty(&base).unwrap()).unwrap_err(),
        ReadError::NonCanonicalDocument
    );
}

#[test]
fn vietnamese_composition_is_idempotent_and_orthographic_variants_stay_distinct() {
    let base = load("accept/two-scenes.json");
    for text in [
        "Người",
        "Người dẫn chuyện",
        "Ánh đèn cuối sân khấu",
        "Vọng Đài",
        "a\u{302}\u{323}",
        "a\u{323}\u{302}",
        "A\u{200b}\u{301}",
        "Ngươ\u{300}i",
        "…",
    ] {
        let mut nfc = base.clone();
        let mut nfd = base.clone();
        nfc["work"]["title"] = json!(text);
        nfd["work"]["title"] = json!(text.nfd().collect::<String>());
        let a = admit(&nfc);
        let b = admit(&nfd);
        assert_eq!(a.export_bytes(), b.export_bytes());
        assert_eq!(
            read_script(&a.export_bytes()).unwrap().export_bytes(),
            a.export_bytes()
        );
    }
    for (left, right) in [("hòa", "hoà"), ("thúy", "thuý"), ("Đ", "Ð"), ("…", "...")] {
        let mut a = base.clone();
        let mut b = base.clone();
        a["work"]["title"] = json!(left);
        b["work"]["title"] = json!(right);
        assert_ne!(admit(&a).content_digest(), admit(&b).content_digest());
    }
}

#[test]
fn reorder_keeps_id_and_speech_but_changes_content_while_cast_set_order_does_not() {
    let base = load("accept/two-scenes.json");
    let original = admit(&base);
    for path in [
        "/episode/acts/0/scenes",
        "/episode/acts/0/scenes/0/dialogues",
    ] {
        let mut variant = base.clone();
        variant
            .pointer_mut(path)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .reverse();
        let moved = admit(&variant);
        assert_ne!(moved.content_digest(), original.content_digest());
        assert_eq!(speech(&moved), speech(&original));
    }
    let mut variant = base;
    variant["characters"].as_array_mut().unwrap().reverse();
    assert_eq!(admit(&variant).export_bytes(), original.export_bytes());
}

#[test]
fn every_performance_field_changes_digest_and_only_speech_inputs_change_spoken_content() {
    let base = load("accept/two-scenes.json");
    let original = admit(&base);
    let dp = "/episode/acts/0/scenes/0/dialogues/1";
    for (path, replacement, speech_changes) in [
        ("/work/id".to_string(), json!("work-other"), false),
        ("/adaptation/id".into(), json!("adaptation-other"), false),
        ("/episode/id".into(), json!("episode-other"), false),
        ("/episode/acts/0/id".into(), json!("act-other"), false),
        (
            "/episode/acts/0/scenes/0/id".into(),
            json!("scene-other"),
            false,
        ),
        ("/work/title".to_string(), json!("Một ngày mới"), false),
        ("/characters/1/name".into(), json!("An Nhiên"), false),
        (
            "/characters/1/personality".into(),
            json!("Trầm tĩnh"),
            false,
        ),
        ("/adaptation/language".into(), json!("vi"), true),
        ("/episode/title".into(), json!("Lời hẹn mới"), false),
        ("/episode/acts/0/title".into(), json!("Hồi mới"), false),
        (
            "/episode/acts/0/scenes/0/title".into(),
            json!("Cảnh mới"),
            false,
        ),
        (format!("{dp}/text"), json!("Ngày mai, hẹn gặp nhé."), true),
        (format!("{dp}/speaker_id"), json!("minh"), false),
        (format!("{dp}/delivery/emotion"), json!("sad"), true),
        (
            format!("{dp}/delivery/intensity_permille"),
            json!(601),
            true,
        ),
        (
            format!("{dp}/pronunciation_overrides"),
            json!([{"surface":"Ngày mai","replacement":"Ngày mai nhé"}]),
            true,
        ),
        (
            "/episode/acts/0/scenes/0/sound_cues/0/description".into(),
            json!("Tiếng mưa nhẹ"),
            false,
        ),
        (
            "/episode/acts/0/scenes/0/sound_cues/0/kind".into(),
            json!("music"),
            false,
        ),
        (
            "/episode/acts/0/scenes/0/sound_cues/0/anchor/edge".into(),
            json!("end"),
            false,
        ),
        (
            "/episode/acts/0/scenes/0/sound_cues/0/id".into(),
            json!("cue-other"),
            false,
        ),
        (
            "/episode/acts/0/scenes/0/sound_cues/0/anchor/dialogue_id".into(),
            json!("dialogue-02"),
            false,
        ),
        (
            "/episode/acts/0/scenes/1/dialogues/0/pronunciation_overrides/0/surface".into(),
            json!("Vọng"),
            true,
        ),
        (
            "/episode/acts/0/scenes/1/dialogues/0/pronunciation_overrides/0/replacement".into(),
            json!("Đài phát thanh"),
            true,
        ),
        (
            "/episode/acts/0/scenes/1/sound_cues/0/asset/id".into(),
            json!("asset-other"),
            false,
        ),
    ] {
        let mut variant = base.clone();
        *variant.pointer_mut(&path).unwrap() = replacement;
        let changed = admit(&variant);
        assert_ne!(
            changed.content_digest(),
            original.content_digest(),
            "{path}"
        );
        assert_eq!(
            speech(&changed) != speech(&original),
            speech_changes,
            "{path}"
        );
        if path.starts_with(dp) && speech_changes {
            let a = speech(&original);
            let b = speech(&changed);
            assert_eq!(
                a.iter()
                    .filter(|(id, bytes)| b.get(*id) != Some(*bytes))
                    .count(),
                1
            );
        }
    }
    let mut character_identity = base.clone();
    character_identity["characters"][1]["id"] = json!("an-other");
    character_identity["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["speaker_id"] =
        json!("an-other");
    character_identity["episode"]["acts"][0]["scenes"][1]["dialogues"][0]["speaker_id"] =
        json!("an-other");
    let changed = admit(&character_identity);
    assert_ne!(changed.content_digest(), original.content_digest());
    assert_eq!(speech(&changed), speech(&original));
    let mut role = base.clone();
    role["characters"][0]["role"] = json!("character");
    role["characters"][1]["role"] = json!("narrator");
    assert_ne!(admit(&role).content_digest(), original.content_digest());
    assert_eq!(speech(&admit(&role)), speech(&original));
    let mut line_identity = base.clone();
    line_identity["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["id"] =
        json!("dialogue-other");
    let changed = admit(&line_identity);
    assert_ne!(changed.content_digest(), original.content_digest());
    assert_eq!(
        changed
            .spoken_lines()
            .map(|line| line.canonical_bytes())
            .collect::<Vec<_>>(),
        original
            .spoken_lines()
            .map(|line| line.canonical_bytes())
            .collect::<Vec<_>>()
    );
}

#[test]
fn provenance_and_rights_are_preserved_in_export_but_do_not_forge_eligibility_or_change_content() {
    let base = load("accept/two-scenes.json");
    let original = admit(&base);
    for path in [
        "/work/rights_record_id",
        "/adaptation/rights_record_id",
        "/provenance/0/rights_record_id",
        "/provenance/0/source_record_id",
        "/episode/acts/0/scenes/1/sound_cues/0/asset/rights_record_id",
    ] {
        let mut variant = base.clone();
        *variant.pointer_mut(path).unwrap() = json!("record-other");
        let changed = admit(&variant);
        assert_eq!(changed.content_digest(), original.content_digest());
        assert_eq!(speech(&changed), speech(&original));
        assert_ne!(changed.export_bytes(), original.export_bytes());
    }
    let imported_generated = admit(&load("accept/provenance.json"));
    assert_eq!(
        imported_generated.content_digest(),
        original.content_digest()
    );
    let export: Value = serde_json::from_slice(&imported_generated.export_bytes()).unwrap();
    assert_eq!(export["provenance"][0]["kind"], "generated");
    assert_eq!(
        export["provenance"][0]["generation_record_id"],
        "attempt-demo"
    );
}

#[test]
fn cli_runs_real_admission_and_returns_nonzero_for_rejected_input() {
    let bin = env!("CARGO_BIN_EXE_validate-script");
    let valid = std::process::Command::new(bin)
        .arg(corpus().join("accept/two-scenes.json"))
        .output()
        .unwrap();
    assert!(valid.status.success());
    assert_eq!(
        String::from_utf8(valid.stdout).unwrap().trim(),
        load("accept/two-scenes.expected.json")["content_digest"]
            .as_str()
            .unwrap()
    );
    let invalid = std::process::Command::new(bin)
        .arg(corpus().join("reject/unknown-speaker.json"))
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(1));
    assert!(
        String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("UnknownSpeaker")
    );
}

#[test]
fn normalization_cannot_create_a_value_that_exceeds_the_export_schema() {
    let mut raw = load("accept/two-scenes.json");
    // U+0344 canonically decomposes to two combining marks even under NFC.
    raw["work"]["title"] = json!("\u{0344}".repeat(5001));
    let result = read_script(&serde_json::to_vec(&raw).unwrap());
    let Err(ReadError::Semantic(report)) = result else {
        panic!("normalization expansion must be rejected before constructing domain content");
    };
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        json!([
            {"path":"work/title","issue":{"code":"normalized_text_too_long","max_chars":10000}}
        ])
    );
}
