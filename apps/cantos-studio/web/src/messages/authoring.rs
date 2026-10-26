//! Localized copy for the structured Studio editor; script content is never translated.
use crate::localization::generated::authoring as resources;

#[derive(Clone, Copy)]
pub struct AuthoringCopy {
    pub title: &'static str,
    pub navigation: &'static str,
    pub scenes: &'static str,
    pub characters: &'static str,
    pub metadata: &'static str,
    pub scene: &'static str,
    pub act: &'static str,
    pub scene_title: &'static str,
    pub act_title: &'static str,
    pub dialogue: &'static str,
    pub narration: &'static str,
    pub speaker: &'static str,
    pub unresolved: &'static str,
    pub spoken_text: &'static str,
    pub emotion: &'static str,
    pub intensity: &'static str,
    pub performance: &'static str,
    pub pronunciation: &'static str,
    pub surface: &'static str,
    pub replacement: &'static str,
    pub pronunciation_help: &'static str,
    pub cues: &'static str,
    pub cue_kind: &'static str,
    pub description: &'static str,
    pub anchor: &'static str,
    pub edge: &'static str,
    pub asset_help: &'static str,
    pub character_name: &'static str,
    pub role: &'static str,
    pub personality: &'static str,
    pub work_title: &'static str,
    pub episode_title: &'static str,
    pub language: &'static str,
    pub metadata_help: &'static str,
    pub add_act: &'static str,
    pub add_scene: &'static str,
    pub add_dialogue: &'static str,
    pub add_narration: &'static str,
    pub add_character: &'static str,
    pub add_cue: &'static str,
    pub add_pronunciation: &'static str,
    pub move_up: &'static str,
    pub move_down: &'static str,
    pub remove: &'static str,
    pub confirm_remove: &'static str,
    pub remove_help: &'static str,
    pub cancel: &'static str,
    pub undo: &'static str,
    pub redo: &'static str,
    pub history_help: &'static str,
    pub empty: &'static str,
    pub invalid: &'static str,
    pub advanced: &'static str,
    pub advanced_help: &'static str,
    pub json: &'static str,
    pub readonly: &'static str,
    pub readonly_choice: &'static str,
    pub inspector: &'static str,
    pub validation: &'static str,
    pub no_issues: &'static str,
    pub validation_help: &'static str,
    pub comparison: &'static str,
    pub unchanged: &'static str,
    pub before: &'static str,
    pub after: &'static str,
    pub added: &'static str,
    pub removed: &'static str,
    pub changed: &'static str,
    pub reordered: &'static str,
    pub comparison_help: &'static str,
    pub operation_failed: &'static str,
    pub edited: &'static str,
    pub buffered: &'static str,
    pub new_act: &'static str,
    pub new_scene: &'static str,
    pub new_character: &'static str,
    pub new_personality: &'static str,
    pub new_dialogue: &'static str,
    pub new_cue: &'static str,
}

pub fn authoring_copy(locale: crate::localization::Locale) -> AuthoringCopy {
    AuthoringCopy {
        title: resources::text(resources::Key::AuthoringCopyTitle, locale),
        navigation: resources::text(resources::Key::AuthoringCopyNavigation, locale),
        scenes: resources::text(resources::Key::AuthoringCopyScenes, locale),
        characters: resources::text(resources::Key::AuthoringCopyCharacters, locale),
        metadata: resources::text(resources::Key::AuthoringCopyMetadata, locale),
        scene: resources::text(resources::Key::AuthoringCopyScene, locale),
        act: resources::text(resources::Key::AuthoringCopyAct, locale),
        scene_title: resources::text(resources::Key::AuthoringCopySceneTitle, locale),
        act_title: resources::text(resources::Key::AuthoringCopyActTitle, locale),
        dialogue: resources::text(resources::Key::AuthoringCopyDialogue, locale),
        narration: resources::text(resources::Key::AuthoringCopyNarration, locale),
        speaker: resources::text(resources::Key::AuthoringCopySpeaker, locale),
        unresolved: resources::text(resources::Key::AuthoringCopyUnresolved, locale),
        spoken_text: resources::text(resources::Key::AuthoringCopySpokenText, locale),
        emotion: resources::text(resources::Key::AuthoringCopyEmotion, locale),
        intensity: resources::text(resources::Key::AuthoringCopyIntensity, locale),
        performance: resources::text(resources::Key::AuthoringCopyPerformance, locale),
        pronunciation: resources::text(resources::Key::AuthoringCopyPronunciation, locale),
        surface: resources::text(resources::Key::AuthoringCopySurface, locale),
        replacement: resources::text(resources::Key::AuthoringCopyReplacement, locale),
        pronunciation_help: resources::text(resources::Key::AuthoringCopyPronunciationHelp, locale),
        cues: resources::text(resources::Key::AuthoringCopyCues, locale),
        cue_kind: resources::text(resources::Key::AuthoringCopyCueKind, locale),
        description: resources::text(resources::Key::AuthoringCopyDescription, locale),
        anchor: resources::text(resources::Key::AuthoringCopyAnchor, locale),
        edge: resources::text(resources::Key::AuthoringCopyEdge, locale),
        asset_help: resources::text(resources::Key::AuthoringCopyAssetHelp, locale),
        character_name: resources::text(resources::Key::AuthoringCopyCharacterName, locale),
        role: resources::text(resources::Key::AuthoringCopyRole, locale),
        personality: resources::text(resources::Key::AuthoringCopyPersonality, locale),
        work_title: resources::text(resources::Key::AuthoringCopyWorkTitle, locale),
        episode_title: resources::text(resources::Key::AuthoringCopyEpisodeTitle, locale),
        language: resources::text(resources::Key::AuthoringCopyLanguage, locale),
        metadata_help: resources::text(resources::Key::AuthoringCopyMetadataHelp, locale),
        add_act: resources::text(resources::Key::AuthoringCopyAddAct, locale),
        add_scene: resources::text(resources::Key::AuthoringCopyAddScene, locale),
        add_dialogue: resources::text(resources::Key::AuthoringCopyAddDialogue, locale),
        add_narration: resources::text(resources::Key::AuthoringCopyAddNarration, locale),
        add_character: resources::text(resources::Key::AuthoringCopyAddCharacter, locale),
        add_cue: resources::text(resources::Key::AuthoringCopyAddCue, locale),
        add_pronunciation: resources::text(resources::Key::AuthoringCopyAddPronunciation, locale),
        move_up: resources::text(resources::Key::AuthoringCopyMoveUp, locale),
        move_down: resources::text(resources::Key::AuthoringCopyMoveDown, locale),
        remove: resources::text(resources::Key::AuthoringCopyRemove, locale),
        confirm_remove: resources::text(resources::Key::AuthoringCopyConfirmRemove, locale),
        remove_help: resources::text(resources::Key::AuthoringCopyRemoveHelp, locale),
        cancel: resources::text(resources::Key::AuthoringCopyCancel, locale),
        undo: resources::text(resources::Key::AuthoringCopyUndo, locale),
        redo: resources::text(resources::Key::AuthoringCopyRedo, locale),
        history_help: resources::text(resources::Key::AuthoringCopyHistoryHelp, locale),
        empty: resources::text(resources::Key::AuthoringCopyEmpty, locale),
        invalid: resources::text(resources::Key::AuthoringCopyInvalid, locale),
        advanced: resources::text(resources::Key::AuthoringCopyAdvanced, locale),
        advanced_help: resources::text(resources::Key::AuthoringCopyAdvancedHelp, locale),
        json: resources::text(resources::Key::AuthoringCopyJson, locale),
        readonly: resources::text(resources::Key::AuthoringCopyReadonly, locale),
        readonly_choice: resources::text(resources::Key::AuthoringCopyReadonlyChoice, locale),
        inspector: resources::text(resources::Key::AuthoringCopyInspector, locale),
        validation: resources::text(resources::Key::AuthoringCopyValidation, locale),
        no_issues: resources::text(resources::Key::AuthoringCopyNoIssues, locale),
        validation_help: resources::text(resources::Key::AuthoringCopyValidationHelp, locale),
        comparison: resources::text(resources::Key::AuthoringCopyComparison, locale),
        unchanged: resources::text(resources::Key::AuthoringCopyUnchanged, locale),
        before: resources::text(resources::Key::AuthoringCopyBefore, locale),
        after: resources::text(resources::Key::AuthoringCopyAfter, locale),
        added: resources::text(resources::Key::AuthoringCopyAdded, locale),
        removed: resources::text(resources::Key::AuthoringCopyRemoved, locale),
        changed: resources::text(resources::Key::AuthoringCopyChanged, locale),
        reordered: resources::text(resources::Key::AuthoringCopyReordered, locale),
        comparison_help: resources::text(resources::Key::AuthoringCopyComparisonHelp, locale),
        operation_failed: resources::text(resources::Key::AuthoringCopyOperationFailed, locale),
        edited: resources::text(resources::Key::AuthoringCopyEdited, locale),
        buffered: resources::text(resources::Key::AuthoringCopyBuffered, locale),
        new_act: resources::text(resources::Key::AuthoringCopyNewAct, locale),
        new_scene: resources::text(resources::Key::AuthoringCopyNewScene, locale),
        new_character: resources::text(resources::Key::AuthoringCopyNewCharacter, locale),
        new_personality: resources::text(resources::Key::AuthoringCopyNewPersonality, locale),
        new_dialogue: resources::text(resources::Key::AuthoringCopyNewDialogue, locale),
        new_cue: resources::text(resources::Key::AuthoringCopyNewCue, locale),
    }
}

pub fn choice(value: &str, locale: crate::localization::Locale) -> &'static str {
    match value {
        "narrator" => resources::text(resources::Key::ChoiceNarrator, locale),
        "character" => resources::text(resources::Key::ChoiceCharacter, locale),
        "neutral" => resources::text(resources::Key::ChoiceNeutral, locale),
        "calm" => resources::text(resources::Key::ChoiceCalm, locale),
        "hopeful" => resources::text(resources::Key::ChoiceHopeful, locale),
        "warm" => resources::text(resources::Key::ChoiceWarm, locale),
        "sad" => resources::text(resources::Key::ChoiceSad, locale),
        "angry" => resources::text(resources::Key::ChoiceAngry, locale),
        "ambience" => resources::text(resources::Key::ChoiceAmbience, locale),
        "music" => resources::text(resources::Key::ChoiceMusic, locale),
        "sfx" => resources::text(resources::Key::ChoiceSfx, locale),
        "start" => resources::text(resources::Key::ChoiceStart, locale),
        "end" => resources::text(resources::Key::ChoiceEnd, locale),
        "vi" => resources::text(resources::Key::ChoiceVi, locale),
        "vi-VN" => resources::text(resources::Key::ChoiceViVn, locale),
        "en" => resources::text(resources::Key::ChoiceEn, locale),
        "en-US" => resources::text(resources::Key::ChoiceEnUs, locale),
        _ => resources::text(resources::Key::Choice, locale),
    }
}

#[derive(Clone, Copy)]
pub enum ItemAction {
    MoveUp,
    MoveDown,
    Remove,
    ConfirmRemove,
}

pub fn item_action(action: ItemAction, id: &str, locale: crate::localization::Locale) -> String {
    match action {
        ItemAction::MoveUp => crate::localization::present(
            crate::localization::generated::Message::StudioAuthoringItemActionMoveUp { arg_id: id }
                .render(locale),
            locale,
        ),
        ItemAction::MoveDown => crate::localization::present(
            crate::localization::generated::Message::StudioAuthoringItemActionMoveDown {
                arg_id: id,
            }
            .render(locale),
            locale,
        ),
        ItemAction::Remove => crate::localization::present(
            crate::localization::generated::Message::StudioAuthoringItemActionRemove { arg_id: id }
                .render(locale),
            locale,
        ),
        ItemAction::ConfirmRemove => crate::localization::present(
            crate::localization::generated::Message::StudioAuthoringItemActionConfirmRemove {
                arg_id: id,
            }
            .render(locale),
            locale,
        ),
    }
}

#[derive(Clone, Copy)]
pub enum NewContentItem {
    Act,
    Scene,
    Character,
}

/// Only newly authored default names use the content language. An unsupported content tag
/// keeps the opaque item ID as its initial name; it never selects Vietnamese or changes the tag.
pub fn new_content_label<'a>(language: &str, id: &'a str, item: NewContentItem) -> &'a str {
    match crate::localization::Locale::from_lang(language) {
        Some(locale) => {
            let copy = authoring_copy(locale);
            match item {
                NewContentItem::Act => copy.new_act,
                NewContentItem::Scene => copy.new_scene,
                NewContentItem::Character => copy.new_character,
            }
        }
        None => id,
    }
}

pub fn pronunciation_remove(id: &str, index: usize, locale: crate::localization::Locale) -> String {
    let position = index.saturating_add(1) as u64;
    crate::localization::present(
        crate::localization::generated::Message::StudioAuthoringPronunciationRemove {
            arg_id: id,
            arg_position: position,
        }
        .render(locale),
        locale,
    )
}

pub fn item_moved(
    id: &str,
    position: usize,
    group: &str,
    locale: crate::localization::Locale,
) -> String {
    crate::localization::present(
        crate::localization::generated::Message::StudioAuthoringItemMoved {
            arg_id: id,
            arg_position: position as u64,
            arg_group: group,
        }
        .render(locale),
        locale,
    )
}

/// Labels of the debug-only `studio-authoring` composition; its fixture script stays untranslated.
#[derive(Clone, Copy)]
pub struct PreviewCopy {
    pub title: &'static str,
    pub help: &'static str,
    pub scenario: &'static str,
    pub theme: &'static str,
    pub light: &'static str,
    pub dark: &'static str,
    pub locale: &'static str,
    pub commits: &'static str,
}

pub fn preview_copy(locale: crate::localization::Locale) -> PreviewCopy {
    PreviewCopy {
        title: resources::text(resources::Key::PreviewCopyTitle, locale),
        help: resources::text(resources::Key::PreviewCopyHelp, locale),
        scenario: resources::text(resources::Key::PreviewCopyScenario, locale),
        theme: resources::text(resources::Key::PreviewCopyTheme, locale),
        light: resources::text(resources::Key::PreviewCopyLight, locale),
        dark: resources::text(resources::Key::PreviewCopyDark, locale),
        locale: resources::text(resources::Key::PreviewCopyLocale, locale),
        commits: resources::text(resources::Key::PreviewCopyCommits, locale),
    }
}

/// Project one selected option from the model, including an unresolved value without dropping it.
/// The flag marks the model value; an unknown value is listed first under the unresolved label.
pub fn selected_choices(
    current: &str,
    choices: Vec<(String, String)>,
    unresolved_label: &str,
) -> Vec<(String, String, bool)> {
    let known = choices.iter().any(|(value, _)| value == current);
    let mut options = Vec::with_capacity(choices.len() + usize::from(!known));
    if !known {
        options.push((current.to_string(), unresolved_label.to_string(), true));
    }
    options.extend(choices.into_iter().map(|(value, label)| {
        let selected = value == current;
        (value, label, selected)
    }));
    options
}

pub fn validation_rule(rule: &str, locale: crate::localization::Locale) -> &'static str {
    match rule {
        "duplicate_id" => resources::text(resources::Key::ValidationRuleDuplicateId, locale),
        "narrator_count" => resources::text(resources::Key::ValidationRuleNarratorCount, locale),
        "unknown_speaker" => resources::text(resources::Key::ValidationRuleUnknownSpeaker, locale),
        "cue_anchor_unresolved" => {
            resources::text(resources::Key::ValidationRuleCueAnchorUnresolved, locale)
        }
        "unknown_provenance" => {
            resources::text(resources::Key::ValidationRuleUnknownProvenance, locale)
        }
        "duplicate_provenance_ref" => {
            resources::text(resources::Key::ValidationRuleDuplicateProvenanceRef, locale)
        }
        "empty_text" => resources::text(resources::Key::ValidationRuleEmptyText, locale),
        "text_length" | "normalized_text_too_long" => resources::text(
            resources::Key::ValidationRuleTextLengthNormalizedTextTooLong,
            locale,
        ),
        "forbidden_character" => {
            resources::text(resources::Key::ValidationRuleForbiddenCharacter, locale)
        }
        "pronunciation_target_missing" => resources::text(
            resources::Key::ValidationRulePronunciationTargetMissing,
            locale,
        ),
        "pronunciation_overlap" => {
            resources::text(resources::Key::ValidationRulePronunciationOverlap, locale)
        }
        "id_format" => resources::text(resources::Key::ValidationRuleIdFormat, locale),
        "array_length" => resources::text(resources::Key::ValidationRuleArrayLength, locale),
        "intensity_range" => resources::text(resources::Key::ValidationRuleIntensityRange, locale),
        "UnsupportedSchemaVersion" | "MissingSchemaVersion" => resources::text(
            resources::Key::ValidationRuleUnsupportedSchemaVersionMissingSchemaVersion,
            locale,
        ),
        "DocumentTooLarge" => {
            resources::text(resources::Key::ValidationRuleDocumentTooLarge, locale)
        }
        "InvalidDocument" | "NonCanonicalDocument" => resources::text(
            resources::Key::ValidationRuleInvalidDocumentNonCanonicalDocument,
            locale,
        ),
        _ => resources::text(resources::Key::ValidationRule, locale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_script_choices_have_both_localizations() {
        for value in [
            "narrator",
            "character",
            "neutral",
            "calm",
            "hopeful",
            "warm",
            "sad",
            "angry",
            "ambience",
            "music",
            "sfx",
            "start",
            "end",
            "vi",
            "vi-VN",
            "en",
            "en-US",
        ] {
            assert!(!choice(value, crate::localization::Locale::English).contains("Unrecognized"));
            assert!(!choice(value, crate::localization::Locale::Vietnamese).contains("chưa hỗ trợ"));
        }
    }

    #[test]
    fn declared_validation_codes_have_actionable_localized_copy() {
        for rule in [
            "duplicate_id",
            "narrator_count",
            "unknown_speaker",
            "cue_anchor_unresolved",
            "unknown_provenance",
            "duplicate_provenance_ref",
            "empty_text",
            "text_length",
            "normalized_text_too_long",
            "forbidden_character",
            "pronunciation_target_missing",
            "pronunciation_overlap",
            "id_format",
            "array_length",
            "intensity_range",
            "UnsupportedSchemaVersion",
            "MissingSchemaVersion",
            "DocumentTooLarge",
            "InvalidDocument",
            "NonCanonicalDocument",
        ] {
            assert_ne!(
                validation_rule(rule, crate::localization::Locale::English),
                validation_rule("unknown", crate::localization::Locale::English)
            );
            assert_ne!(
                validation_rule(rule, crate::localization::Locale::Vietnamese),
                validation_rule("unknown", crate::localization::Locale::Vietnamese)
            );
        }
    }

    #[test]
    fn dynamic_choices_exclusively_select_current_identity_across_changes_and_locales() {
        for locale in crate::localization::Locale::supported() {
            for current in ["narrator", "mai", "narrator"] {
                let choices = vec![
                    (
                        "narrator".to_string(),
                        choice("narrator", locale).to_string(),
                    ),
                    ("mai".to_string(), "Mai".to_string()),
                ];
                let options = selected_choices(current, choices, authoring_copy(locale).unresolved);
                let selected = options
                    .iter()
                    .filter(|(_, _, selected)| *selected)
                    .map(|(id, _, _)| id.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(selected, vec![current]);
            }
            for current in ["ambience", "music", "sfx", "music"] {
                let choices = ["ambience", "music", "sfx"]
                    .map(|value| (value.to_string(), choice(value, locale).to_string()))
                    .to_vec();
                let options = selected_choices(current, choices, choice("unknown", locale));
                assert_eq!(
                    options
                        .into_iter()
                        .filter(|(_, _, selected)| *selected)
                        .map(|(id, _, _)| id)
                        .collect::<Vec<_>>(),
                    vec![current.to_string()]
                );
            }
        }
    }

    #[test]
    fn unresolved_choice_is_preserved_then_resolves_to_the_existing_option() {
        let options = selected_choices(
            "missing-speaker",
            vec![("narrator".into(), "Người dẫn chuyện".into())],
            authoring_copy(crate::localization::Locale::Vietnamese).unresolved,
        );
        assert_eq!(
            options[0],
            (
                "missing-speaker".into(),
                authoring_copy(crate::localization::Locale::Vietnamese)
                    .unresolved
                    .into(),
                true
            )
        );
        assert_eq!(
            options.iter().filter(|(_, _, selected)| *selected).count(),
            1
        );
        let resolved = selected_choices(
            "missing-speaker",
            vec![
                ("narrator".into(), "Người dẫn chuyện".into()),
                ("missing-speaker".into(), "Mai".into()),
            ],
            authoring_copy(crate::localization::Locale::Vietnamese).unresolved,
        );
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[1], ("missing-speaker".into(), "Mai".into(), true));
        assert!(!resolved[0].2);
    }
}
