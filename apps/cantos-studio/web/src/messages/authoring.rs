//! Localized copy for the structured Studio editor; script content is never translated.

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

pub fn authoring_copy(english: bool) -> AuthoringCopy {
    if english {
        AuthoringCopy {
            title: "Script workshop", navigation: "Script navigation", scenes: "Acts and scenes", characters: "Characters", metadata: "Work and episode", scene: "Scene", act: "Act", scene_title: "Scene title", act_title: "Act title", dialogue: "Dialogue", narration: "Narration", speaker: "Speaker", unresolved: "Choose a speaker · current reference is unresolved", spoken_text: "Spoken text", emotion: "Emotion", intensity: "Intensity · 0–1000", performance: "Performance and pronunciation", pronunciation: "Pronunciation overrides", surface: "Written phrase", replacement: "Spoken replacement", pronunciation_help: "Use a phrase present in this line. Overrides are structured instructions; keep spoken text free of cue markup.", cues: "Sound cues", cue_kind: "Cue type", description: "Sound description", anchor: "Anchor dialogue", edge: "Anchor edge", asset_help: "Existing asset and rights metadata is preserved. Asset selection belongs to the production workflow.", character_name: "Character name", role: "Role", personality: "Characterization", work_title: "Work title", episode_title: "Episode title", language: "Content language", metadata_help: "Stable IDs, provenance and rights records are preserved. Inspect the complete document in Advanced JSON; acceptance revalidates every field.", add_act: "Add act", add_scene: "Add scene", add_dialogue: "Add dialogue", add_narration: "Add narration", add_character: "Add character", add_cue: "Add sound cue", add_pronunciation: "Add pronunciation override", move_up: "Move up", move_down: "Move down", remove: "Remove", confirm_remove: "Confirm removal", remove_help: "Remove this item from the browser draft? Remove dependent references explicitly before deleting a referenced item. Undo restores the exact item and its ID.", cancel: "Cancel", undo: "Undo edit", redo: "Redo edit", history_help: "Undo and redo affect browser edits. Native text undo remains available inside each field.", empty: "Open a script or submitted proposal to begin editing.", invalid: "This draft cannot be shown as a structured script. Its complete text is preserved below; correct the JSON or reopen a valid revision.", advanced: "Advanced JSON · complete document", advanced_help: "For unsupported or additional fields, edit the complete JSON here. Structured edits preserve every unedited field. Invalid JSON remains available and is never silently replaced.", json: "Complete Script IR JSON", readonly: "Read only · editing is unavailable for this record or actor", inspector: "Review inspector", validation: "Validation findings", no_issues: "No findings are currently shown.", validation_help: "The backend validates the complete document when saving or accepting. No findings shown is not a production or rights approval.", comparison: "Changes from opened version", unchanged: "No committed browser changes from the opened version.", before: "Opened text", after: "Current text", added: "Added", removed: "Removed", changed: "Changed", reordered: "Order changed", comparison_help: "Compare edits by stable identity. The preserved source and revision history remain separate evidence to review.", operation_failed: "The edit could not be applied safely. The existing draft is preserved; inspect its JSON and references.", edited: "Browser draft updated.", buffered: "A field is still being edited. Leave it to commit, correct an invalid value, or press Escape to restore its last committed value before navigating or saving.", new_act: "New act", new_scene: "New scene", new_character: "New character", new_personality: "Describe the character", new_dialogue: "Write the spoken line here.", new_cue: "Describe the sound here.",
        }
    } else {
        AuthoringCopy {
            title: "Xưởng kịch bản", navigation: "Điều hướng kịch bản", scenes: "Hồi và cảnh", characters: "Nhân vật", metadata: "Tác phẩm và tập", scene: "Cảnh", act: "Hồi", scene_title: "Tên cảnh", act_title: "Tên hồi", dialogue: "Lời thoại", narration: "Lời dẫn", speaker: "Người nói", unresolved: "Chọn người nói · tham chiếu hiện tại chưa hợp lệ", spoken_text: "Văn bản được đọc", emotion: "Cảm xúc", intensity: "Cường độ · 0–1000", performance: "Diễn xuất và phát âm", pronunciation: "Chỉ dẫn phát âm", surface: "Cụm từ trong văn bản", replacement: "Cách đọc thay thế", pronunciation_help: "Chọn cụm từ có trong lời này. Phát âm là chỉ dẫn có cấu trúc; không chèn cue vào văn bản được đọc.", cues: "Chỉ dẫn âm thanh", cue_kind: "Loại âm thanh", description: "Mô tả âm thanh", anchor: "Neo vào lời thoại", edge: "Vị trí neo", asset_help: "Metadata về tài nguyên và quyền hiện có được giữ nguyên. Chọn tài nguyên thuộc bước sản xuất.", character_name: "Tên nhân vật", role: "Vai trò", personality: "Tính cách", work_title: "Tên tác phẩm", episode_title: "Tên tập", language: "Ngôn ngữ nội dung", metadata_help: "ID ổn định, nguồn gốc và hồ sơ quyền được giữ nguyên. Xem toàn bộ tài liệu trong JSON nâng cao; khi chấp nhận, mọi trường được kiểm tra lại.", add_act: "Thêm hồi", add_scene: "Thêm cảnh", add_dialogue: "Thêm lời thoại", add_narration: "Thêm lời dẫn", add_character: "Thêm nhân vật", add_cue: "Thêm chỉ dẫn âm thanh", add_pronunciation: "Thêm chỉ dẫn phát âm", move_up: "Chuyển lên", move_down: "Chuyển xuống", remove: "Xóa", confirm_remove: "Xác nhận xóa", remove_help: "Xóa mục này khỏi bản nháp trong trình duyệt? Hãy sửa các tham chiếu liên quan trước khi xóa mục đang được dùng. Hoàn tác khôi phục đúng nội dung và ID cũ.", cancel: "Hủy", undo: "Hoàn tác chỉnh sửa", redo: "Làm lại chỉnh sửa", history_help: "Hoàn tác và làm lại áp dụng cho phần sửa trong trình duyệt. Bạn vẫn có thể hoàn tác văn bản trực tiếp trong từng trường.", empty: "Mở kịch bản hoặc đề xuất đã gửi để bắt đầu biên tập.", invalid: "Bản nháp này chưa hiển thị được dưới dạng kịch bản có cấu trúc. Toàn bộ văn bản được giữ bên dưới; hãy sửa JSON hoặc mở lại bản hợp lệ.", advanced: "JSON nâng cao · toàn bộ tài liệu", advanced_help: "Sửa toàn bộ JSON tại đây khi cần trường bổ sung hoặc chưa hỗ trợ. Biên tập có cấu trúc giữ mọi trường chưa sửa. JSON chưa hợp lệ vẫn được giữ, không tự thay thế.", json: "Toàn bộ Script IR JSON", readonly: "Chỉ đọc · bản ghi hoặc quyền hiện tại chưa cho phép sửa", inspector: "Bảng kiểm duyệt", validation: "Các điểm cần sửa", no_issues: "Hiện chưa có lỗi được hiển thị.", validation_help: "Backend kiểm tra toàn bộ tài liệu khi lưu hoặc chấp nhận. Không có lỗi hiển thị chưa có nghĩa đã duyệt sản xuất hay quyền nội dung.", comparison: "Thay đổi so với bản đã mở", unchanged: "Chưa có thay đổi đã ghi vào bản nháp so với bản đã mở.", before: "Văn bản đã mở", after: "Văn bản hiện tại", added: "Đã thêm", removed: "Đã xóa", changed: "Đã sửa", reordered: "Đã đổi thứ tự", comparison_help: "Đối chiếu phần sửa theo ID ổn định. Nguồn được giữ và lịch sử revision là những bằng chứng riêng cần xem lại.", operation_failed: "Chưa áp dụng được chỉnh sửa an toàn. Bản nháp hiện tại được giữ; hãy kiểm tra JSON và tham chiếu.", edited: "Đã cập nhật bản nháp trong trình duyệt.", buffered: "Có trường đang sửa. Rời trường để ghi vào bản nháp, sửa giá trị chưa hợp lệ hoặc nhấn Escape để khôi phục giá trị đã ghi trước khi chuyển trang hay lưu.", new_act: "Hồi mới", new_scene: "Cảnh mới", new_character: "Nhân vật mới", new_personality: "Mô tả tính cách nhân vật", new_dialogue: "Viết lời được đọc tại đây.", new_cue: "Mô tả âm thanh tại đây.",
        }
    }
}

pub fn choice(value: &str, english: bool) -> &'static str {
    match (value, english) {
        ("narrator", true) => "Narrator",
        ("narrator", false) => "Người dẫn chuyện",
        ("character", true) => "Character",
        ("character", false) => "Nhân vật",
        ("neutral", true) => "Neutral",
        ("neutral", false) => "Trung tính",
        ("calm", true) => "Calm",
        ("calm", false) => "Điềm tĩnh",
        ("hopeful", true) => "Hopeful",
        ("hopeful", false) => "Hy vọng",
        ("warm", true) => "Warm",
        ("warm", false) => "Ấm áp",
        ("sad", true) => "Sad",
        ("sad", false) => "Buồn",
        ("angry", true) => "Angry",
        ("angry", false) => "Giận dữ",
        ("ambience", true) => "Ambience",
        ("ambience", false) => "Âm nền",
        ("music", true) => "Music",
        ("music", false) => "Nhạc",
        ("sfx", true) => "Sound effect",
        ("sfx", false) => "Hiệu ứng",
        ("start", true) => "Before the line",
        ("start", false) => "Trước lời thoại",
        ("end", true) => "After the line",
        ("end", false) => "Sau lời thoại",
        ("vi", _) => "Tiếng Việt · vi",
        ("vi-VN", _) => "Tiếng Việt · vi-VN",
        ("en", _) => "English · en",
        ("en-US", _) => "English · en-US",
        (_, true) => "Unrecognized value · preserved",
        (_, false) => "Giá trị chưa hỗ trợ · được giữ",
    }
}

#[derive(Clone, Copy)]
pub enum ItemAction {
    MoveUp,
    MoveDown,
    Remove,
    ConfirmRemove,
}

pub fn item_action(action: ItemAction, id: &str, english: bool) -> String {
    match (action, english) {
        (ItemAction::MoveUp, true) => format!("Move item {id} up"),
        (ItemAction::MoveUp, false) => format!("Chuyển mục {id} lên"),
        (ItemAction::MoveDown, true) => format!("Move item {id} down"),
        (ItemAction::MoveDown, false) => format!("Chuyển mục {id} xuống"),
        (ItemAction::Remove, true) => format!("Remove item {id}"),
        (ItemAction::Remove, false) => format!("Xóa mục {id}"),
        (ItemAction::ConfirmRemove, true) => format!("Confirm removal of item {id}"),
        (ItemAction::ConfirmRemove, false) => format!("Xác nhận xóa mục {id}"),
    }
}

pub fn pronunciation_remove(id: &str, index: usize, english: bool) -> String {
    let position = index + 1;
    if english {
        format!("Remove pronunciation override {position} from line {id}")
    } else {
        format!("Xóa chỉ dẫn phát âm {position} của lời {id}")
    }
}

pub fn item_moved(id: &str, position: usize, group: &str, english: bool) -> String {
    if english {
        format!("Moved item {id} to position {position} in {group}.")
    } else {
        format!("Đã chuyển mục {id} tới vị trí {position} trong {group}.")
    }
}

/// Project one selected option from the model, including an unresolved value without dropping it.
/// The view applies this boolean to `option.selected` after constructing its dynamic choices.
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

pub fn validation_rule(rule: &str, english: bool) -> &'static str {
    match (rule, english) {
        ("duplicate_id", true) => "Each item needs a unique stable ID.",
        ("duplicate_id", false) => "Mỗi mục cần một ID ổn định riêng.",
        ("narrator_count", true) => "Keep exactly one narrator character.",
        ("narrator_count", false) => "Giữ đúng một nhân vật dẫn chuyện.",
        ("unknown_speaker", true) => "Choose a speaker from the character list.",
        ("unknown_speaker", false) => "Chọn người nói trong danh sách nhân vật.",
        ("cue_anchor_unresolved", true) => "Anchor this cue to a dialogue in the same scene.",
        ("cue_anchor_unresolved", false) => "Neo chỉ dẫn vào lời thoại trong cùng cảnh.",
        ("unknown_provenance", true) => "The provenance reference must resolve.",
        ("unknown_provenance", false) => "Tham chiếu nguồn gốc cần trỏ tới hồ sơ hiện có.",
        ("duplicate_provenance_ref", true) => "Remove repeated provenance references.",
        ("duplicate_provenance_ref", false) => "Bỏ tham chiếu nguồn gốc bị lặp.",
        ("empty_text", true) => "Enter nonempty text.",
        ("empty_text", false) => "Nhập văn bản có nội dung.",
        ("text_length" | "normalized_text_too_long", true) => {
            "Keep this text within the supported length."
        }
        ("text_length" | "normalized_text_too_long", false) => {
            "Giữ văn bản trong giới hạn độ dài hỗ trợ."
        }
        ("forbidden_character", true) => "Remove unsupported control characters.",
        ("forbidden_character", false) => "Bỏ ký tự điều khiển chưa hỗ trợ.",
        ("pronunciation_target_missing", true) => {
            "The written phrase must occur in this spoken line."
        }
        ("pronunciation_target_missing", false) => "Cụm từ cần có trong lời được đọc này.",
        ("pronunciation_overlap", true) => "Pronunciation overrides must not overlap.",
        ("pronunciation_overlap", false) => "Các chỉ dẫn phát âm không được chồng lên nhau.",
        ("id_format", true) => "Use a supported stable ID format.",
        ("id_format", false) => "Dùng định dạng ID ổn định được hỗ trợ.",
        ("array_length", true) => "Check the required sequence and its supported size.",
        ("array_length", false) => "Kiểm tra danh sách bắt buộc và giới hạn số mục.",
        ("intensity_range", true) => "Use an integer intensity from 0 to 1000.",
        ("intensity_range", false) => "Nhập cường độ nguyên từ 0 đến 1000.",
        ("UnsupportedSchemaVersion" | "MissingSchemaVersion", true) => {
            "This editor supports Script IR 0.1.0."
        }
        ("UnsupportedSchemaVersion" | "MissingSchemaVersion", false) => {
            "Editor này hỗ trợ Script IR 0.1.0."
        }
        ("DocumentTooLarge", true) => "The document exceeds the supported size.",
        ("DocumentTooLarge", false) => "Tài liệu vượt quá kích thước hỗ trợ.",
        ("InvalidDocument" | "NonCanonicalDocument", true) => {
            "Check the complete JSON document and its fields."
        }
        ("InvalidDocument" | "NonCanonicalDocument", false) => {
            "Kiểm tra toàn bộ tài liệu JSON và các trường."
        }
        (_, true) => "This field needs review before acceptance.",
        (_, false) => "Cần kiểm tra trường này trước khi chấp nhận.",
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
            assert!(!choice(value, true).contains("Unrecognized"));
            assert!(!choice(value, false).contains("chưa hỗ trợ"));
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
                validation_rule(rule, true),
                validation_rule("unknown", true)
            );
            assert_ne!(
                validation_rule(rule, false),
                validation_rule("unknown", false)
            );
        }
    }

    #[test]
    fn dynamic_choices_exclusively_select_current_identity_across_changes_and_locales() {
        for english in [false, true] {
            for current in ["narrator", "mai", "narrator"] {
                let choices = vec![
                    (
                        "narrator".to_string(),
                        choice("narrator", english).to_string(),
                    ),
                    ("mai".to_string(), "Mai".to_string()),
                ];
                let options =
                    selected_choices(current, choices, authoring_copy(english).unresolved);
                let selected = options
                    .iter()
                    .filter(|(_, _, selected)| *selected)
                    .map(|(id, _, _)| id.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(selected, vec![current]);
            }
            for current in ["ambience", "music", "sfx", "music"] {
                let choices = ["ambience", "music", "sfx"]
                    .map(|value| (value.to_string(), choice(value, english).to_string()))
                    .to_vec();
                let options = selected_choices(current, choices, choice("unknown", english));
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
            authoring_copy(false).unresolved,
        );
        assert_eq!(
            options[0],
            (
                "missing-speaker".into(),
                authoring_copy(false).unresolved.into(),
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
            authoring_copy(false).unresolved,
        );
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[1], ("missing-speaker".into(), "Mai".into(), true));
        assert!(!resolved[0].2);
    }
}
