//! Script IR diff fixtures are built in code from one explicit base document, so every expected
//! change is written out by hand. The oracles are independent of the implementation: literal
//! expected reports, a naive dynamic-programming LCS for order, canonical export bytes for
//! "identical", and the existing per-line speech bytes for "respoken".
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use cantos_api::script_ir::{
    diff_scripts, read_script, ScopeDifference, ScopeLevel, ScriptContent, DIFF_REPORT_VERSION,
    MAX_DOCUMENT_BYTES,
};
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;

const LINE_01: &str = "Ánh đèn cuối cùng còn sáng trên sân khấu.";
const LINE_02: &str = "Ngày mai, mình có diễn tiếp không?";

fn dialogue(id: &str, speaker: &str, text: &str, emotion: &str, intensity: u32) -> Value {
    json!({
        "id": id,
        "speaker_id": speaker,
        "text": text,
        "delivery": { "emotion": emotion, "intensity_permille": intensity },
    })
}

fn cue(id: &str, kind: &str, description: &str, anchor: &str, edge: &str) -> Value {
    json!({
        "id": id,
        "kind": kind,
        "description": description,
        "anchor": { "dialogue_id": anchor, "edge": edge },
    })
}

fn character(id: &str, name: &str, role: &str) -> Value {
    json!({ "id": id, "name": name, "role": role, "personality": format!("{name} trên sân khấu") })
}

/// Two acts, three scenes, seven lines, three cues (one with an asset), one pronunciation
/// override, three characters and two provenance sources, so every entity kind can move or change.
fn base() -> Value {
    let mut line_04 = dialogue(
        "line-04",
        "an",
        "Vọng Đài vẫn sáng… Hẹn gặp lại!",
        "warm",
        400,
    );
    line_04["pronunciation_overrides"] =
        json!([{ "surface": "Vọng Đài", "replacement": "vọng đài" }]);
    let mut cue_2 = cue("cue-2", "sfx", "Tiếng cửa khép nhẹ", "line-03", "end");
    cue_2["asset"] = json!({ "id": "asset-door", "rights_record_id": "rights-asset-door" });
    json!({
        "schema_version": "0.1.0",
        "work": {
            "id": "work-cantos",
            "title": "Ánh đèn cuối sân khấu",
            "source_ref": "origin",
            "rights_record_id": "rights-work",
        },
        "adaptation": {
            "id": "adaptation-1",
            "language": "vi-VN",
            "rights_record_id": "rights-adaptation",
            "provenance_refs": ["generated-1", "origin"],
        },
        "characters": [
            character("narrator", "Người dẫn chuyện", "narrator"),
            character("an", "An", "character"),
            character("minh", "Minh", "character"),
        ],
        "episode": {
            "id": "episode-1",
            "title": "Một lời hẹn",
            "acts": [
                {
                    "id": "act-1",
                    "title": "Sân khấu trống",
                    "scenes": [
                        {
                            "id": "scene-1",
                            "title": "Sau buổi diễn",
                            "dialogues": [
                                dialogue("line-01", "narrator", LINE_01, "calm", 300),
                                dialogue("line-02", "an", LINE_02, "hopeful", 600),
                                dialogue("line-03", "minh", "Có chứ. Tôi sẽ chờ cậu ở đây.", "warm", 400),
                            ],
                            "sound_cues": [
                                cue("cue-1", "ambience", "Quiet theatre room tone", "line-01", "start"),
                                cue_2,
                            ],
                        },
                        {
                            "id": "scene-2",
                            "title": "Bên cửa sổ",
                            "dialogues": [
                                line_04,
                                dialogue("line-05", "narrator", "Cửa sổ khép lại, phố khuya lặng gió.", "calm", 250),
                            ],
                            "sound_cues": [cue("cue-3", "music", "Dây đàn nhẹ", "line-05", "end")],
                        },
                    ],
                },
                {
                    "id": "act-2",
                    "title": "Lời hẹn",
                    "scenes": [{
                        "id": "scene-3",
                        "title": "Sáng hôm sau",
                        "dialogues": [
                            dialogue("line-06", "minh", "Sáng hôm sau, tôi đến sớm.", "warm", 350),
                            dialogue("line-07", "an", "Anh đến sớm thật đấy!", "hopeful", 550),
                        ],
                    }],
                },
            ],
        },
        "provenance": [
            { "id": "generated-1", "kind": "generated", "source_record_id": "source-gen",
              "rights_record_id": "rights-gen", "generation_record_id": "attempt-1" },
            { "id": "origin", "kind": "original", "source_record_id": "source-origin",
              "rights_record_id": "rights-origin" },
        ],
    })
}

fn bytes(document: &Value) -> Vec<u8> {
    serde_json::to_vec(document).unwrap()
}

fn admit(document: &Value) -> ScriptContent {
    read_script(&bytes(document)).expect("test document is admitted")
}

fn report(before: &Value, after: &Value) -> Value {
    let diff = diff_scripts(&admit(before), &admit(after)).expect("same episode");
    serde_json::to_value(diff).unwrap()
}

fn edited(edit: impl FnOnce(&mut Value)) -> Value {
    let mut document = base();
    edit(&mut document);
    document
}

fn set(document: &mut Value, pointer: &str, value: Value) {
    *document
        .pointer_mut(pointer)
        .unwrap_or_else(|| panic!("no value at {pointer}")) = value;
}

fn array<'a>(document: &'a mut Value, pointer: &str) -> &'a mut Vec<Value> {
    document
        .pointer_mut(pointer)
        .and_then(Value::as_array_mut)
        .unwrap_or_else(|| panic!("no array at {pointer}"))
}

/// Remove the item at `from` and re-insert it so that it ends up at index `to`.
fn relocate(document: &mut Value, pointer: &str, from: usize, to: usize) {
    let items = array(document, pointer);
    let item = items.remove(from);
    items.insert(to, item);
}

fn changes(report: &Value) -> &[Value] {
    report["changes"].as_array().unwrap()
}

const SCENE_1_LINES: &str = "/episode/acts/0/scenes/0/dialogues";
const SCENE_1_CUES: &str = "/episode/acts/0/scenes/0/sound_cues";
const SCENE_2_LINES: &str = "/episode/acts/0/scenes/1/dialogues";
const ACT_1_SCENES: &str = "/episode/acts/0/scenes";
const ACTS: &str = "/episode/acts";

fn speech(script: &ScriptContent) -> BTreeMap<String, Vec<u8>> {
    script
        .spoken_lines()
        .map(|line| (line.dialogue_id().into(), line.canonical_bytes()))
        .collect()
}

fn zero_summary() -> Value {
    json!({
        "total": 0, "added": 0, "removed": 0, "modified": 0, "moved": 0,
        "by_aspect": {
            "text": 0, "speaker": 0, "delivery": 0, "pronunciation": 0, "order": 0, "cue": 0,
            "character": 0, "title": 0, "language": 0, "provenance": 0, "rights": 0,
        },
    })
}

fn nfd(text: &str) -> String {
    text.nfd().collect()
}

#[test]
fn identical_versions_report_no_change_and_the_versioned_envelope() {
    let report = report(&base(), &base());
    let digest = admit(&base()).content_digest().to_string();
    assert_eq!(
        report,
        json!({
            "report_version": "cantos-script-diff-1",
            "schema_version": "0.1.0",
            "scope": { "work_id": "work-cantos", "adaptation_id": "adaptation-1", "episode_id": "episode-1" },
            "before": { "content_digest": digest },
            "after": { "content_digest": digest },
            "content_digest_equal": true,
            "identical": true,
            "summary": zero_summary(),
            "changes": [],
        })
    );
    assert_eq!(DIFF_REPORT_VERSION, "cantos-script-diff-1");
}

#[test]
fn representation_only_differences_are_not_changes() {
    let after = edited(|d| {
        // Order that validation canonicalizes, NFD and spacing that admission normalizes.
        array(d, "/characters").reverse();
        array(d, "/provenance").reverse();
        array(d, "/adaptation/provenance_refs").reverse();
        set(
            d,
            "/episode/acts/0/scenes/0/dialogues/1/text",
            json!(nfd(&format!("  {LINE_02}\t\n"))),
        );
        set(d, "/work/title", json!(nfd("Ánh đèn cuối sân khấu")));
        let spaced = "Vọng  Đài vẫn sáng…\u{a0}Hẹn gặp lại!";
        set(
            d,
            "/episode/acts/0/scenes/1/dialogues/0/text",
            json!(spaced),
        );
        set(
            d,
            "/episode/acts/0/scenes/1/dialogues/0/pronunciation_overrides/0/surface",
            json!(nfd("Vọng Đài")),
        );
    });
    let report = report(&base(), &after);
    assert_eq!(report["identical"], true);
    assert_eq!(report["changes"], json!([]));
    assert_eq!(report["summary"], zero_summary());
    assert_eq!(admit(&base()).export_bytes(), admit(&after).export_bytes());
}

#[test]
fn a_text_edit_is_one_modified_dialogue_with_before_and_after() {
    let edit = "Ngày mai, mình diễn tiếp nhé?";
    let after = edited(|d| set(d, "/episode/acts/0/scenes/0/dialogues/1/text", json!(edit)));
    let report = report(&base(), &after);
    assert_eq!(
        report["changes"],
        json!([{
            "change": "modified", "entity": "dialogue", "id": "line-02",
            "fields": [{ "field": "text", "aspect": "text", "before": LINE_02, "after": edit }],
        }])
    );
    assert_eq!(report["identical"], false);
    assert_eq!(report["content_digest_equal"], false);
    let mut summary = zero_summary();
    summary["total"] = json!(1);
    summary["modified"] = json!(1);
    summary["by_aspect"]["text"] = json!(1);
    assert_eq!(report["summary"], summary);
}

#[test]
fn a_diacritic_only_edit_and_a_non_bmp_edit_are_text_changes_reported_exactly() {
    for (before, after) in [
        (
            "Có chứ. Tôi sẽ chờ cậu ở đây.",
            "Cố chứ. Tôi sẽ chờ cậu ở đây.",
        ),
        (LINE_01, "Ánh đèn cuối cùng còn sáng trên sân khấu. 🎭"),
    ] {
        let edited = edited(|d| set(d, "/episode/acts/0/scenes/0/dialogues/2/text", json!(after)));
        let mut original = base();
        set(
            &mut original,
            "/episode/acts/0/scenes/0/dialogues/2/text",
            json!(before),
        );
        let report = report(&original, &edited);
        assert_eq!(
            report["changes"],
            json!([{
                "change": "modified", "entity": "dialogue", "id": "line-03",
                "fields": [{ "field": "text", "aspect": "text", "before": before, "after": after }],
            }])
        );
    }
}

#[test]
fn moving_a_line_within_a_scene_is_one_move_not_a_delete_and_add() {
    let after = edited(|d| relocate(d, SCENE_1_LINES, 2, 0));
    let report = report(&base(), &after);
    assert_eq!(
        report["changes"],
        json!([{
            "change": "moved", "entity": "dialogue", "id": "line-03",
            "from": { "parent": "scene-1", "index": 2 },
            "to": { "parent": "scene-1", "index": 0 },
        }])
    );
    let mut summary = zero_summary();
    summary["total"] = json!(1);
    summary["moved"] = json!(1);
    summary["by_aspect"]["order"] = json!(1);
    assert_eq!(report["summary"], summary);
    assert_eq!(report["content_digest_equal"], false);
}

#[test]
fn moving_a_line_to_another_scene_names_both_scenes() {
    let after = edited(|d| {
        let line = array(d, SCENE_1_LINES).remove(1);
        array(d, SCENE_2_LINES).insert(1, line);
    });
    let report = report(&base(), &after);
    assert_eq!(
        report["changes"],
        json!([{
            "change": "moved", "entity": "dialogue", "id": "line-02",
            "from": { "parent": "scene-1", "index": 1 },
            "to": { "parent": "scene-2", "index": 1 },
        }])
    );
}

#[test]
fn inserting_or_deleting_a_line_does_not_move_its_neighbours() {
    let inserted = edited(|d| {
        array(d, SCENE_1_LINES).insert(
            0,
            dialogue("line-new", "an", "Chào mọi người.", "warm", 400),
        );
    });
    let report = report(&base(), &inserted);
    assert_eq!(
        report["changes"],
        json!([{
            "change": "added", "entity": "dialogue", "id": "line-new",
            "at": { "parent": "scene-1", "index": 0 },
            "values": [
                { "field": "speaker_id", "aspect": "speaker", "value": "an" },
                { "field": "text", "aspect": "text", "value": "Chào mọi người." },
                { "field": "delivery.emotion", "aspect": "delivery", "value": "warm" },
                { "field": "delivery.intensity_permille", "aspect": "delivery", "value": 400 },
            ],
        }])
    );
    // line-02 has no cue, so deleting it keeps the script valid; line-01 and line-03 stay put.
    let deleted = edited(|d| {
        array(d, SCENE_1_LINES).remove(1);
    });
    let reverse = self::report(&base(), &deleted);
    assert_eq!(
        reverse["changes"],
        json!([{
            "change": "removed", "entity": "dialogue", "id": "line-02",
            "at": { "parent": "scene-1", "index": 1 },
            "values": [
                { "field": "speaker_id", "aspect": "speaker", "value": "an" },
                { "field": "text", "aspect": "text", "value": LINE_02 },
                { "field": "delivery.emotion", "aspect": "delivery", "value": "hopeful" },
                { "field": "delivery.intensity_permille", "aspect": "delivery", "value": 600 },
            ],
        }])
    );
}

#[test]
fn a_moved_and_edited_line_yields_a_modified_and_a_moved_change() {
    let after = edited(|d| {
        relocate(d, SCENE_1_LINES, 0, 2);
        set(
            d,
            "/episode/acts/0/scenes/0/dialogues/2/delivery/emotion",
            json!("sad"),
        );
    });
    let report = report(&base(), &after);
    assert_eq!(
        report["changes"],
        json!([
            {
                "change": "modified", "entity": "dialogue", "id": "line-01",
                "fields": [{ "field": "delivery.emotion", "aspect": "delivery", "before": "calm", "after": "sad" }],
            },
            {
                "change": "moved", "entity": "dialogue", "id": "line-01",
                "from": { "parent": "scene-1", "index": 0 },
                "to": { "parent": "scene-1", "index": 2 },
            },
        ])
    );
}

#[test]
fn scenes_and_acts_move_by_identity_and_children_stay_put() {
    // scene-2 moves from act-1 into act-2; act-2 then moves ahead of act-1.
    let after = edited(|d| {
        let scene = array(d, ACT_1_SCENES).remove(1);
        array(d, "/episode/acts/1/scenes").insert(0, scene);
        relocate(d, ACTS, 1, 0);
    });
    let report = report(&base(), &after);
    assert_eq!(
        report["changes"],
        json!([
            {
                "change": "moved", "entity": "act", "id": "act-2",
                "from": { "parent": "episode-1", "index": 1 },
                "to": { "parent": "episode-1", "index": 0 },
            },
            {
                "change": "moved", "entity": "scene", "id": "scene-2",
                "from": { "parent": "act-1", "index": 1 },
                "to": { "parent": "act-2", "index": 0 },
            },
        ])
    );
}

#[test]
fn reordering_cues_within_a_scene_and_moving_one_with_its_anchor_are_moves() {
    let reordered = edited(|d| relocate(d, SCENE_1_CUES, 1, 0));
    assert_eq!(
        report(&base(), &reordered)["changes"],
        json!([{
            "change": "moved", "entity": "cue", "id": "cue-2",
            "from": { "parent": "scene-1", "index": 1 },
            "to": { "parent": "scene-1", "index": 0 },
        }])
    );
    // cue-1 now anchors to line-04 and lives in scene-2: its anchor changed and it moved.
    let moved = edited(|d| {
        let mut cue = array(d, SCENE_1_CUES).remove(0);
        cue["anchor"]["dialogue_id"] = json!("line-04");
        array(d, "/episode/acts/0/scenes/1/sound_cues").insert(0, cue);
    });
    assert_eq!(
        report(&base(), &moved)["changes"],
        json!([
            {
                "change": "modified", "entity": "cue", "id": "cue-1",
                "fields": [{ "field": "anchor.dialogue_id", "aspect": "cue", "before": "line-01", "after": "line-04" }],
            },
            {
                "change": "moved", "entity": "cue", "id": "cue-1",
                "from": { "parent": "scene-1", "index": 0 },
                "to": { "parent": "scene-2", "index": 0 },
            },
        ])
    );
}

#[test]
fn a_speaker_only_edit_changes_only_the_speaker_field() {
    let after = edited(|d| {
        set(
            d,
            "/episode/acts/0/scenes/0/dialogues/1/speaker_id",
            json!("minh"),
        )
    });
    let report = report(&base(), &after);
    assert_eq!(
        report["changes"],
        json!([{
            "change": "modified", "entity": "dialogue", "id": "line-02",
            "fields": [{ "field": "speaker_id", "aspect": "speaker", "before": "an", "after": "minh" }],
        }])
    );
    let by_aspect = &report["summary"]["by_aspect"];
    assert_eq!(
        (by_aspect["speaker"].clone(), by_aspect["text"].clone()),
        (json!(1), json!(0))
    );
    // The speaker is not speech content: the line's canonical speech bytes are unchanged.
    assert_eq!(speech(&admit(&base())), speech(&admit(&after)));
}

#[test]
fn a_cue_only_edit_changes_only_cue_fields_and_leaves_every_line_alone() {
    let after = edited(|d| {
        set(
            d,
            "/episode/acts/0/scenes/0/sound_cues/0/description",
            json!("Tiếng mưa rơi trên mái"),
        );
        set(
            d,
            "/episode/acts/0/scenes/0/sound_cues/0/kind",
            json!("sfx"),
        );
        array(d, SCENE_1_CUES).push(cue("cue-new", "music", "Nhạc nền êm", "line-02", "start"));
    });
    let report = report(&base(), &after);
    assert_eq!(
        report["changes"],
        json!([
            {
                "change": "modified", "entity": "cue", "id": "cue-1",
                "fields": [
                    { "field": "kind", "aspect": "cue", "before": "ambience", "after": "sfx" },
                    { "field": "description", "aspect": "cue", "before": "Quiet theatre room tone", "after": "Tiếng mưa rơi trên mái" },
                ],
            },
            {
                "change": "added", "entity": "cue", "id": "cue-new",
                "at": { "parent": "scene-1", "index": 2 },
                "values": [
                    { "field": "kind", "aspect": "cue", "value": "music" },
                    { "field": "description", "aspect": "cue", "value": "Nhạc nền êm" },
                    { "field": "anchor.dialogue_id", "aspect": "cue", "value": "line-02" },
                    { "field": "anchor.edge", "aspect": "cue", "value": "start" },
                ],
            },
        ])
    );
    let mut counts = zero_summary()["by_aspect"].clone();
    counts["cue"] = json!(2);
    assert_eq!(report["summary"]["by_aspect"], counts);
    assert_eq!(speech(&admit(&base())), speech(&admit(&after)));
}

#[test]
fn a_rights_only_edit_is_reported_although_the_content_digests_are_equal() {
    let after = edited(|d| {
        set(d, "/work/rights_record_id", json!("rights-work-2"));
        set(
            d,
            "/episode/acts/0/scenes/0/sound_cues/1/asset/rights_record_id",
            json!("rights-door-2"),
        );
        set(
            d,
            "/provenance/1/rights_record_id",
            json!("rights-origin-2"),
        );
    });
    let report = report(&base(), &after);
    assert_eq!(report["content_digest_equal"], true);
    assert_eq!(
        report["before"]["content_digest"],
        report["after"]["content_digest"]
    );
    assert_eq!(report["identical"], false);
    assert_eq!(
        report["changes"],
        json!([
            {
                "change": "modified", "entity": "work", "id": "work-cantos",
                "fields": [{ "field": "rights_record_id", "aspect": "rights", "before": "rights-work", "after": "rights-work-2" }],
            },
            {
                "change": "modified", "entity": "cue", "id": "cue-2",
                "fields": [{ "field": "asset.rights_record_id", "aspect": "rights", "before": "rights-asset-door", "after": "rights-door-2" }],
            },
            {
                "change": "modified", "entity": "provenance", "id": "origin",
                "fields": [{ "field": "rights_record_id", "aspect": "rights", "before": "rights-origin", "after": "rights-origin-2" }],
            },
        ])
    );
    assert_eq!(report["summary"]["by_aspect"]["rights"], 3);
    assert_ne!(admit(&base()).export_bytes(), admit(&after).export_bytes());
}

#[test]
fn removing_a_cue_asset_shows_the_asset_and_its_rights_as_changed_to_null() {
    let after = edited(|d| {
        d.pointer_mut("/episode/acts/0/scenes/0/sound_cues/1")
            .and_then(Value::as_object_mut)
            .unwrap()
            .remove("asset");
    });
    assert_eq!(
        report(&base(), &after)["changes"],
        json!([{
            "change": "modified", "entity": "cue", "id": "cue-2",
            "fields": [
                { "field": "asset.id", "aspect": "cue", "before": "asset-door", "after": null },
                { "field": "asset.rights_record_id", "aspect": "rights", "before": "rights-asset-door", "after": null },
            ],
        }])
    );
}

/// One edit, the single field it must surface, and whether the content digest sees it.
struct FieldCase {
    pointer: String,
    value: Value,
    entity: &'static str,
    id: &'static str,
    field: &'static str,
    aspect: &'static str,
    in_content_digest: bool,
}

fn field_cases() -> Vec<FieldCase> {
    let case = |pointer: &str, value, entity, id, field, aspect, in_content_digest| FieldCase {
        pointer: pointer.to_owned(),
        value,
        entity,
        id,
        field,
        aspect,
        in_content_digest,
    };
    let line = "/episode/acts/0/scenes/0/dialogues/1";
    let cue_2 = "/episode/acts/0/scenes/0/sound_cues/1";
    vec![
        case(
            "/work/title",
            json!("Đèn sân khấu"),
            "work",
            "work-cantos",
            "title",
            "title",
            true,
        ),
        case(
            "/work/source_ref",
            json!("generated-1"),
            "work",
            "work-cantos",
            "source_ref",
            "provenance",
            false,
        ),
        case(
            "/work/rights_record_id",
            json!("rights-w"),
            "work",
            "work-cantos",
            "rights_record_id",
            "rights",
            false,
        ),
        case(
            "/adaptation/language",
            json!("en-US"),
            "adaptation",
            "adaptation-1",
            "language",
            "language",
            true,
        ),
        case(
            "/adaptation/rights_record_id",
            json!("rights-a"),
            "adaptation",
            "adaptation-1",
            "rights_record_id",
            "rights",
            false,
        ),
        case(
            "/adaptation/provenance_refs",
            json!(["origin"]),
            "adaptation",
            "adaptation-1",
            "provenance_refs",
            "provenance",
            false,
        ),
        case(
            "/episode/title",
            json!("Lời hẹn mới"),
            "episode",
            "episode-1",
            "title",
            "title",
            true,
        ),
        case(
            "/episode/acts/0/title",
            json!("Sân khấu sáng"),
            "act",
            "act-1",
            "title",
            "title",
            true,
        ),
        case(
            "/episode/acts/0/scenes/1/title",
            json!("Bên ô cửa"),
            "scene",
            "scene-2",
            "title",
            "title",
            true,
        ),
        case(
            &format!("{line}/speaker_id"),
            json!("minh"),
            "dialogue",
            "line-02",
            "speaker_id",
            "speaker",
            true,
        ),
        case(
            &format!("{line}/text"),
            json!("Mai mình diễn tiếp nhé?"),
            "dialogue",
            "line-02",
            "text",
            "text",
            true,
        ),
        case(
            &format!("{line}/delivery/emotion"),
            json!("sad"),
            "dialogue",
            "line-02",
            "delivery.emotion",
            "delivery",
            true,
        ),
        case(
            &format!("{line}/delivery/intensity_permille"),
            json!(601),
            "dialogue",
            "line-02",
            "delivery.intensity_permille",
            "delivery",
            true,
        ),
        case(
            "/episode/acts/0/scenes/1/dialogues/0/pronunciation_overrides",
            json!([]),
            "dialogue",
            "line-04",
            "pronunciation_overrides",
            "pronunciation",
            true,
        ),
        case(
            &format!("{cue_2}/kind"),
            json!("music"),
            "cue",
            "cue-2",
            "kind",
            "cue",
            true,
        ),
        case(
            &format!("{cue_2}/description"),
            json!("Cánh cửa khép lại"),
            "cue",
            "cue-2",
            "description",
            "cue",
            true,
        ),
        case(
            &format!("{cue_2}/anchor/dialogue_id"),
            json!("line-02"),
            "cue",
            "cue-2",
            "anchor.dialogue_id",
            "cue",
            true,
        ),
        case(
            &format!("{cue_2}/anchor/edge"),
            json!("start"),
            "cue",
            "cue-2",
            "anchor.edge",
            "cue",
            true,
        ),
        case(
            &format!("{cue_2}/asset/id"),
            json!("asset-window"),
            "cue",
            "cue-2",
            "asset.id",
            "cue",
            true,
        ),
        case(
            &format!("{cue_2}/asset/rights_record_id"),
            json!("rights-asset-2"),
            "cue",
            "cue-2",
            "asset.rights_record_id",
            "rights",
            false,
        ),
        case(
            "/characters/1/name",
            json!("An Nhiên"),
            "character",
            "an",
            "name",
            "character",
            true,
        ),
        case(
            "/characters/1/personality",
            json!("Tò mò và hy vọng"),
            "character",
            "an",
            "personality",
            "character",
            true,
        ),
        case(
            "/provenance/1/kind",
            json!("imported"),
            "provenance",
            "origin",
            "kind",
            "provenance",
            false,
        ),
        case(
            "/provenance/0/source_record_id",
            json!("source-gen-2"),
            "provenance",
            "generated-1",
            "source_record_id",
            "provenance",
            false,
        ),
        case(
            "/provenance/0/rights_record_id",
            json!("rights-gen-2"),
            "provenance",
            "generated-1",
            "rights_record_id",
            "rights",
            false,
        ),
        case(
            "/provenance/0/generation_record_id",
            json!("attempt-2"),
            "provenance",
            "generated-1",
            "generation_record_id",
            "provenance",
            false,
        ),
    ]
}

#[test]
fn every_single_field_edit_surfaces_exactly_that_field_with_its_aspect() {
    let original = base();
    let original_script = admit(&original);
    for case in field_cases() {
        let before_value = original.pointer(&case.pointer).unwrap().clone();
        let mut after = original.clone();
        set(&mut after, &case.pointer, case.value.clone());
        let report = report(&original, &after);
        let label = case.pointer.as_str();
        assert_eq!(changes(&report).len(), 1, "{label}");
        let change = &changes(&report)[0];
        assert_eq!(change["change"], "modified", "{label}");
        assert_eq!(
            (&change["entity"], &change["id"]),
            (&json!(case.entity), &json!(case.id)),
            "{label}"
        );
        let fields = change["fields"].as_array().unwrap();
        assert_eq!(fields.len(), 1, "{label}");
        let reported = &fields[0];
        assert_eq!(
            (&reported["field"], &reported["aspect"]),
            (&json!(case.field), &json!(case.aspect)),
            "{label}"
        );
        assert_eq!(
            (&reported["before"], &reported["after"]),
            (&before_value, &case.value),
            "{label}"
        );
        // The independent oracles agree: the exported document differs, the content digest
        // moves exactly for content fields, and the speech bytes move only for speech fields.
        let after_script = admit(&after);
        assert_ne!(
            original_script.export_bytes(),
            after_script.export_bytes(),
            "{label}"
        );
        assert_eq!(
            report["content_digest_equal"], !case.in_content_digest,
            "{label}"
        );
        let speech_fields = [
            "text",
            "delivery.emotion",
            "delivery.intensity_permille",
            "pronunciation_overrides",
        ];
        let respoken: BTreeSet<String> = speech(&original_script)
            .into_iter()
            .filter(|(id, bytes)| speech(&after_script).get(id) != Some(bytes))
            .map(|(id, _)| id)
            .collect();
        let expected: BTreeSet<String> = match case.entity {
            "dialogue" if speech_fields.contains(&case.field) => {
                BTreeSet::from([case.id.to_owned()])
            }
            "adaptation" if case.field == "language" => {
                speech(&original_script).into_keys().collect()
            }
            _ => BTreeSet::new(),
        };
        assert_eq!(respoken, expected, "{label}");
    }
}

#[test]
fn characters_are_added_removed_and_edited_by_id_and_have_no_order() {
    let after = edited(|d| {
        // minh speaks nothing after the edit, so the character can be removed.
        for pointer in [SCENE_1_LINES, "/episode/acts/1/scenes/0/dialogues"] {
            for line in array(d, pointer) {
                if line["speaker_id"] == "minh" {
                    line["speaker_id"] = json!("an");
                }
            }
        }
        array(d, "/characters").remove(2);
        array(d, "/characters").insert(0, character("ba-cu", "Bà cụ", "character"));
        set(d, "/characters/2/name", json!("An Nhiên"));
    });
    let report = report(&base(), &after);
    let characters: Vec<&Value> = changes(&report)
        .iter()
        .filter(|c| c["entity"] == "character")
        .collect();
    assert_eq!(
        characters,
        [
            &json!({
                "change": "modified", "entity": "character", "id": "an",
                "fields": [{ "field": "name", "aspect": "character", "before": "An", "after": "An Nhiên" }],
            }),
            &json!({
                "change": "added", "entity": "character", "id": "ba-cu",
                "values": [
                    { "field": "name", "aspect": "character", "value": "Bà cụ" },
                    { "field": "role", "aspect": "character", "value": "character" },
                    { "field": "personality", "aspect": "character", "value": "Bà cụ trên sân khấu" },
                ],
            }),
            &json!({
                "change": "removed", "entity": "character", "id": "minh",
                "values": [
                    { "field": "name", "aspect": "character", "value": "Minh" },
                    { "field": "role", "aspect": "character", "value": "character" },
                    { "field": "personality", "aspect": "character", "value": "Minh trên sân khấu" },
                ],
            }),
        ]
    );
    // Characters carry no position, and nothing else moved.
    assert!(changes(&report).iter().all(|c| c["change"] != "moved"));
}

#[test]
fn reassigning_the_narrator_shows_both_role_changes_in_id_order() {
    let after = edited(|d| {
        set(d, "/characters/0/role", json!("character"));
        set(d, "/characters/1/role", json!("narrator"));
    });
    let report = report(&base(), &after);
    assert_eq!(
        report["changes"],
        json!([
            {
                "change": "modified", "entity": "character", "id": "an",
                "fields": [{ "field": "role", "aspect": "character", "before": "character", "after": "narrator" }],
            },
            {
                "change": "modified", "entity": "character", "id": "narrator",
                "fields": [{ "field": "role", "aspect": "character", "before": "narrator", "after": "character" }],
            },
        ])
    );
}

#[test]
fn a_removed_scene_lists_the_scene_its_lines_and_its_cues_one_by_one() {
    let after = edited(|d| {
        array(d, ACT_1_SCENES).remove(1);
    });
    let report = report(&base(), &after);
    let removed: Vec<(String, String)> = changes(&report)
        .iter()
        .map(|c| {
            assert_eq!(c["change"], "removed");
            (
                c["entity"].as_str().unwrap().to_owned(),
                c["id"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let expected = [
        ("scene", "scene-2"),
        ("dialogue", "line-04"),
        ("dialogue", "line-05"),
        ("cue", "cue-3"),
    ];
    assert_eq!(removed, expected.map(|(e, i)| (e.to_owned(), i.to_owned())));
    let mut summary = zero_summary();
    summary["total"] = json!(4);
    summary["removed"] = json!(4);
    summary["by_aspect"]["title"] = json!(1);
    summary["by_aspect"]["speaker"] = json!(2);
    summary["by_aspect"]["text"] = json!(2);
    summary["by_aspect"]["delivery"] = json!(2);
    summary["by_aspect"]["pronunciation"] = json!(1);
    summary["by_aspect"]["cue"] = json!(1);
    assert_eq!(report["summary"], summary);
    // The reverse direction adds the same entities, with their positions in the new version.
    let forward = self::report(&after, &base());
    assert_eq!(forward["summary"]["added"], 4);
    assert_eq!(forward["summary"]["removed"], 0);
    assert_eq!(
        changes(&forward)[0]["at"],
        json!({ "parent": "act-1", "index": 1 })
    );
}

#[test]
fn a_copied_line_is_an_addition_and_a_reused_id_of_another_kind_is_remove_plus_add() {
    let copied = edited(|d| {
        let mut copy = array(d, SCENE_1_LINES)[1].clone();
        copy["id"] = json!("line-02-copy");
        array(d, SCENE_1_LINES).insert(2, copy);
    });
    let report_copy = report(&base(), &copied);
    assert_eq!(report_copy["summary"]["added"], 1);
    assert_eq!(report_copy["summary"]["moved"], 0);
    assert_eq!(report_copy["summary"]["modified"], 0);
    // cue-3 is deleted and a dialogue takes over the same ID: different kinds never match.
    let reused = edited(|d| {
        array(d, "/episode/acts/0/scenes/1/sound_cues").clear();
        array(d, SCENE_2_LINES).push(dialogue("cue-3", "an", "Một lời mới.", "calm", 300));
    });
    let report_reuse = report(&base(), &reused);
    let kinds: Vec<(&str, &str, &str)> = changes(&report_reuse)
        .iter()
        .map(|c| {
            (
                c["change"].as_str().unwrap(),
                c["entity"].as_str().unwrap(),
                c["id"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [("added", "dialogue", "cue-3"), ("removed", "cue", "cue-3")]
    );
}

#[test]
fn a_mixed_review_counts_every_change_under_each_aspect_it_involves() {
    let after = edited(|d| {
        set(
            d,
            "/episode/acts/0/scenes/0/dialogues/1/text",
            json!("Mai mình diễn tiếp nhé?"),
        );
        set(
            d,
            "/episode/acts/0/scenes/0/dialogues/1/speaker_id",
            json!("minh"),
        );
        relocate(d, SCENE_1_LINES, 2, 0);
        set(
            d,
            "/episode/acts/0/scenes/0/sound_cues/0/kind",
            json!("music"),
        );
        set(d, "/work/rights_record_id", json!("rights-work-2"));
        set(d, "/adaptation/language", json!("vi"));
    });
    let report = report(&base(), &after);
    assert_eq!(
        report["summary"],
        json!({
            "total": 5, "added": 0, "removed": 0, "modified": 4, "moved": 1,
            "by_aspect": {
                "text": 1, "speaker": 1, "delivery": 0, "pronunciation": 0, "order": 1,
                "cue": 1, "character": 0, "title": 0, "language": 1, "provenance": 0, "rights": 1,
            },
        })
    );
}

#[test]
fn versions_of_different_episodes_cannot_be_compared() {
    let other = edited(|d| {
        set(d, "/work/id", json!("work-other"));
        set(d, "/episode/id", json!("episode-2"));
    });
    let mismatch = diff_scripts(&admit(&base()), &admit(&other)).unwrap_err();
    assert_eq!(
        mismatch.differences(),
        [
            ScopeDifference {
                level: ScopeLevel::Work,
                before: "work-cantos".into(),
                after: "work-other".into()
            },
            ScopeDifference {
                level: ScopeLevel::Episode,
                before: "episode-1".into(),
                after: "episode-2".into()
            },
        ]
    );
    assert_eq!(
        mismatch.to_string(),
        "not versions of one episode: work id `work-cantos` vs `work-other`; episode id `episode-1` vs `episode-2`"
    );
    let adaptation = edited(|d| set(d, "/adaptation/id", json!("adaptation-2")));
    let mismatch = diff_scripts(&admit(&base()), &admit(&adaptation)).unwrap_err();
    assert_eq!(mismatch.differences().len(), 1);
    assert_eq!(mismatch.differences()[0].level, ScopeLevel::Adaptation);
}

/// Longest common subsequence length by the textbook table, independent of the implementation.
fn lcs_len(left: &[String], right: &[String]) -> usize {
    let mut table = vec![vec![0usize; right.len() + 1]; left.len() + 1];
    for (i, a) in left.iter().enumerate() {
        for (j, b) in right.iter().enumerate() {
            table[i + 1][j + 1] = if a == b {
                table[i][j] + 1
            } else {
                table[i][j + 1].max(table[i + 1][j])
            };
        }
    }
    table[left.len()][right.len()]
}

/// Every ordered selection of `size` items from `pool`.
fn arrangements(pool: &[String], size: usize) -> Vec<Vec<String>> {
    if size == 0 {
        return vec![Vec::new()];
    }
    let mut all = Vec::new();
    for (i, head) in pool.iter().enumerate() {
        let mut rest = pool.to_vec();
        rest.remove(i);
        for mut tail in arrangements(&rest, size - 1) {
            tail.insert(0, head.clone());
            all.push(tail);
        }
    }
    all
}

fn order_document(ids: &[String]) -> Value {
    edited(|d| {
        let lines: Vec<Value> = ids
            .iter()
            .map(|id| dialogue(id, "an", &format!("Lời của {id}"), "calm", 300))
            .collect();
        set(
            d,
            ACTS,
            json!([{ "id": "act-1", "title": "Màn một", "scenes": [{ "id": "scene-1", "title": "Cảnh một", "dialogues": lines }] }]),
        );
    })
}

#[test]
fn moves_and_membership_agree_with_an_independent_lcs_for_every_arrangement() {
    let before_ids: Vec<String> = (0..5).map(|i| format!("line-{i}")).collect();
    let mut pool = before_ids.clone();
    pool.push("line-new".into());
    let original = order_document(&before_ids);
    let mut checked = 0;
    for size in 1..=pool.len() {
        for after_ids in arrangements(&pool, size) {
            let report = report(&original, &order_document(&after_ids));
            let in_report = |kind: &str| -> BTreeSet<String> {
                changes(&report)
                    .iter()
                    .filter(|c| c["change"] == kind)
                    .map(|c| c["id"].as_str().unwrap().to_owned())
                    .collect()
            };
            let survivors: Vec<String> = after_ids
                .iter()
                .filter(|id| before_ids.contains(id))
                .cloned()
                .collect();
            let earlier: Vec<String> = before_ids
                .iter()
                .filter(|id| after_ids.contains(id))
                .cloned()
                .collect();
            let moved = in_report("moved");
            assert_eq!(
                in_report("added"),
                after_ids
                    .iter()
                    .filter(|id| !before_ids.contains(id))
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                "{after_ids:?}"
            );
            assert_eq!(
                in_report("removed"),
                before_ids
                    .iter()
                    .filter(|id| !after_ids.contains(id))
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                "{after_ids:?}"
            );
            assert_eq!(in_report("modified"), BTreeSet::new(), "{after_ids:?}");
            // Minimal: as few moves as the independent LCS allows, and no more.
            assert_eq!(
                moved.len(),
                survivors.len() - lcs_len(&earlier, &survivors),
                "{after_ids:?}"
            );
            // Sufficient: dropping the moved lines leaves both versions in the same order.
            let steady = |ids: &[String]| -> Vec<String> {
                ids.iter()
                    .filter(|id| !moved.contains(*id))
                    .cloned()
                    .collect()
            };
            assert_eq!(steady(&earlier), steady(&survivors), "{after_ids:?}");
            // Indices count every sibling of that version.
            for change in changes(&report).iter().filter(|c| c["change"] == "moved") {
                let id = change["id"].as_str().unwrap().to_owned();
                let from = before_ids.iter().position(|b| *b == id).unwrap();
                let to = after_ids.iter().position(|a| *a == id).unwrap();
                assert_eq!(
                    (
                        change["from"]["index"].clone(),
                        change["to"]["index"].clone()
                    ),
                    (json!(from), json!(to))
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 6 + 30 + 120 + 360 + 720 + 720);
}

// ---------------------------------------------------------------------------------------------
// Command line: exit codes, bounded reads, read-only behavior and exact output.

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("script-diff-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_json(dir: &Path, name: &str, document: &Value) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, bytes(document)).unwrap();
    path
}

fn diff_script(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_diff-script"))
        .args(args)
        .output()
        .unwrap()
}

fn run_pair(before: &Path, after: &Path) -> Output {
    diff_script(&[before.as_os_str(), after.as_os_str()])
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn cli_prints_the_library_report_and_exits_zero_whether_or_not_the_versions_differ() {
    let dir = scratch("report");
    let before = write_json(&dir, "before.json", &base());
    let changed = edited(|d| {
        set(
            d,
            "/episode/acts/0/scenes/0/dialogues/1/text",
            json!("Mai mình diễn tiếp nhé?"),
        )
    });
    let after = write_json(&dir, "after.json", &changed);

    let output = run_pair(&before, &after);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        report(&base(), &changed)
    );
    assert!(output.stdout.ends_with(b"}\n"));

    let same = run_pair(&before, &before);
    assert_eq!(same.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<Value>(&same.stdout).unwrap()["identical"],
        true
    );
    // Deterministic: the same inputs give byte-identical output.
    assert_eq!(run_pair(&before, &after).stdout, output.stdout);
}

#[test]
fn cli_reads_only_and_leaves_inputs_and_their_directory_untouched() {
    let dir = scratch("read-only");
    let before = write_json(&dir, "before.json", &base());
    let after = write_json(
        &dir,
        "after.json",
        &edited(|d| set(d, "/work/rights_record_id", json!("rights-x"))),
    );
    let listing = |dir: &Path| -> BTreeMap<String, Vec<u8>> {
        fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .map(|p| {
                (
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    fs::read(&p).unwrap(),
                )
            })
            .collect()
    };
    let snapshot = listing(&dir);
    for _ in 0..2 {
        assert_eq!(run_pair(&before, &after).status.code(), Some(0));
        assert_eq!(run_pair(&after, &before).status.code(), Some(0));
    }
    assert_eq!(listing(&dir), snapshot);
}

#[test]
fn cli_reports_vietnamese_text_exactly_and_ignores_unicode_normalization_form() {
    let dir = scratch("unicode");
    let before = write_json(&dir, "before.json", &base());
    let composed = "Có chứ. Tôi sẽ chờ cậu ở đây, dưới ánh đèn 🎭.";
    let changed = edited(|d| {
        set(
            d,
            "/episode/acts/0/scenes/0/dialogues/2/text",
            json!(composed),
        )
    });
    let output = run_pair(&before, &write_json(&dir, "after.json", &changed));
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 report");
    let parsed: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["changes"][0]["fields"][0]["after"], composed);
    assert!(
        stdout.contains("Tôi sẽ chờ cậu ở đây"),
        "text is written as UTF-8, not escaped"
    );

    // The same words in decomposed form are not an edit.
    let decomposed = edited(|d| {
        set(
            d,
            "/episode/acts/0/scenes/0/dialogues/2/text",
            json!(nfd("Có chứ. Tôi sẽ chờ cậu ở đây.")),
        )
    });
    let output = run_pair(&before, &write_json(&dir, "nfd.json", &decomposed));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["identical"],
        true
    );
}

#[test]
fn cli_refuses_bad_usage_with_no_report() {
    let before = scratch("usage").join("a.json");
    let path = before.as_os_str();
    let cases: [&[&std::ffi::OsStr]; 4] = [
        &[],
        &[path],
        &[path, path, path],
        &[std::ffi::OsStr::new("--max-bytes"), path],
    ];
    for args in cases {
        let output = diff_script(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("usage: diff-script <before.json> <after.json>"));
    }
    let help = diff_script(&[std::ffi::OsStr::new("--help")]);
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8(help.stdout)
        .unwrap()
        .starts_with("usage: diff-script"));
}

#[test]
fn cli_refuses_missing_directory_and_oversized_input_and_accepts_the_exact_limit() {
    let dir = scratch("limits");
    let good = write_json(&dir, "good.json", &base());

    let missing = run_pair(&dir.join("missing.json"), &good);
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
    assert!(stderr(&missing).starts_with(&format!("before {}", dir.join("missing.json").display())));

    let directory = run_pair(&good, &dir);
    assert_eq!(directory.status.code(), Some(2));
    assert!(
        stderr(&directory).starts_with("after ")
            && stderr(&directory).contains("not a regular file")
    );

    // Leading whitespace is valid JSON, so a read that stops one byte early cuts the document.
    let padded = |length: usize| {
        let mut document = vec![b' '; length - bytes(&base()).len()];
        document.extend(bytes(&base()));
        document
    };
    let at_limit = dir.join("at-limit.json");
    fs::write(&at_limit, padded(MAX_DOCUMENT_BYTES)).unwrap();
    let accepted = run_pair(&at_limit, &good);
    assert_eq!(accepted.status.code(), Some(0), "{}", stderr(&accepted));
    assert_eq!(
        serde_json::from_slice::<Value>(&accepted.stdout).unwrap()["identical"],
        true
    );

    let over_limit = dir.join("over-limit.json");
    fs::write(&over_limit, padded(MAX_DOCUMENT_BYTES + 1)).unwrap();
    for (first, second, label) in [
        (&over_limit, &good, "before"),
        (&good, &over_limit, "after"),
    ] {
        let refused = run_pair(first, second);
        assert_eq!(refused.status.code(), Some(2));
        assert!(refused.stdout.is_empty());
        assert!(stderr(&refused).starts_with(label));
        assert!(stderr(&refused).contains(&format!(
            "exceeds the {MAX_DOCUMENT_BYTES}-byte Script IR limit"
        )));
    }
}

#[test]
fn cli_exits_one_without_a_report_when_an_input_is_rejected_and_names_which() {
    let dir = scratch("rejected");
    let good = write_json(&dir, "good.json", &base());
    let cases: Vec<(&str, Vec<u8>, &str)> = vec![
        ("truncated", b"{\"schema_version\": ".to_vec(), "invalid JSON at line 1"),
        ("no-version", bytes(&edited(|d| { d.as_object_mut().unwrap().remove("schema_version"); })), "missing schema_version"),
        ("old-version", bytes(&edited(|d| set(d, "/schema_version", json!("0.2.0")))), "unsupported schema_version 0.2.0; this reader supports 0.1.0"),
        ("unknown-field", bytes(&edited(|d| { d["extra"] = json!(1); })), "unknown field `extra`"),
        ("bad-id", bytes(&edited(|d| set(d, "/work/id", json!("Work Cantos")))), "\"rule\":\"id_format\""),
        (
            "unknown-speaker",
            bytes(&edited(|d| set(d, "/episode/acts/0/scenes/0/dialogues/0/speaker_id", json!("ghost")))),
            "\"path\":\"episode/act:act-1/scene:scene-1/dialogue:line-01/speaker_id\",\"issue\":{\"code\":\"unknown_speaker\",\"speaker_id\":\"ghost\"}",
        ),
    ];
    for (name, document, expected) in cases {
        let rejected = dir.join(format!("{name}.json"));
        fs::write(&rejected, document).unwrap();
        for (first, second, label) in [(&rejected, &good, "before"), (&good, &rejected, "after")] {
            let output = run_pair(first, second);
            assert_eq!(output.status.code(), Some(1), "{name}");
            assert!(output.stdout.is_empty(), "{name}");
            let message = stderr(&output);
            assert!(
                message.starts_with(&format!(
                    "{label} {}: Script IR rejected: ",
                    rejected.display()
                )),
                "{name}: {message}"
            );
            assert!(message.contains(expected), "{name}: {message}");
        }
    }
}

#[test]
fn cli_caps_the_diagnostics_it_prints() {
    let dir = scratch("many-diagnostics");
    let many = edited(|d| {
        for line in array(d, SCENE_1_LINES) {
            line["speaker_id"] = json!("ghost");
        }
        for scene in array(d, ACT_1_SCENES) {
            for line in scene["dialogues"].as_array_mut().unwrap() {
                line["speaker_id"] = json!("ghost");
            }
        }
        array(d, SCENE_1_LINES).extend(
            (0..30).map(|i| dialogue(&format!("extra-{i}"), "ghost", "Lời thừa.", "calm", 300)),
        );
    });
    let output = run_pair(
        &write_json(&dir, "many.json", &many),
        &write_json(&dir, "good.json", &base()),
    );
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.contains("35 semantic issue(s)"), "{message}");
    assert!(message.contains("... and 15 more"), "{message}");
    assert_eq!(
        message
            .lines()
            .filter(|line| line.starts_with("  {"))
            .count(),
        20
    );
}

#[test]
fn cli_prints_every_diagnostic_up_to_the_cap_and_counts_the_rest() {
    let dir = scratch("diagnostic-cap");
    let good = write_json(&dir, "good.json", &base());
    // Each extra line names an unknown speaker: exactly one semantic issue.
    let with_issues = |count: usize| {
        edited(|d| {
            let lines = array(d, "/episode/acts/1/scenes/0/dialogues");
            lines.extend(
                (0..count)
                    .map(|i| dialogue(&format!("extra-{i}"), "ghost", "Lời thừa.", "calm", 300)),
            );
        })
    };
    for (count, hidden) in [(1, None), (20, None), (21, Some(1)), (22, Some(2))] {
        let bad = write_json(&dir, "bad.json", &with_issues(count));
        let output = run_pair(&bad, &good);
        assert_eq!(output.status.code(), Some(1));
        let message = stderr(&output);
        assert!(
            message.contains(&format!("{count} semantic issue(s)")),
            "{count}: {message}"
        );
        assert_eq!(
            message
                .lines()
                .filter(|line| line.starts_with("  {"))
                .count(),
            count.min(20),
            "{count}"
        );
        match hidden {
            None => assert!(!message.contains("more"), "{count}: {message}"),
            Some(hidden) => assert!(
                message.ends_with(&format!("  ... and {hidden} more\n")),
                "{count}: {message}"
            ),
        }
    }
}

#[test]
fn cli_exits_one_for_versions_of_different_work_adaptation_or_episode() {
    let dir = scratch("scope");
    let before = write_json(&dir, "before.json", &base());
    let other = edited(|d| {
        set(d, "/adaptation/id", json!("adaptation-2"));
        set(d, "/episode/id", json!("episode-2"));
    });
    let output = run_pair(&before, &write_json(&dir, "after.json", &other));
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        stderr(&output),
        "scripts cannot be compared: not versions of one episode: adaptation id `adaptation-1` vs `adaptation-2`; episode id `episode-1` vs `episode-2`\n"
    );
}

#[test]
fn committed_fixtures_that_differ_only_in_provenance_report_metadata_changes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/contracts/fixtures/script-ir/0.1.0/accept");
    let output = run_pair(&root.join("two-scenes.json"), &root.join("provenance.json"));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["content_digest_equal"], true);
    assert_eq!(report["identical"], false);
    let touched: Vec<(&str, &str, &str)> = changes(&report)
        .iter()
        .map(|c| {
            (
                c["change"].as_str().unwrap(),
                c["entity"].as_str().unwrap(),
                c["id"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        touched,
        [
            ("modified", "adaptation", "adaptation-demo"),
            ("added", "provenance", "origin-adaptation"),
            ("modified", "provenance", "origin-demo"),
        ]
    );
    assert_eq!(report["summary"]["by_aspect"]["provenance"], 3);
    assert_eq!(report["summary"]["by_aspect"]["rights"], 1);
}
