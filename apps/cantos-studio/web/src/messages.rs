//! Typed resource catalog; vi-VN and en are exhaustive, with no view literals.
use crate::editor::Status;

mod adaptation;
pub use adaptation::*;
pub mod authoring;
pub mod entry;
#[cfg(debug_assertions)]
pub mod forge;
pub mod production;
pub mod signin;
pub mod workspace;

/// Interface locale owned by the Studio root. Namespaces that still take `english: bool` read it
/// through the [`Locale::is_english`] adapter until they move to typed copy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    Vietnamese,
    English,
}

impl Locale {
    pub const ALL: [Self; 2] = [Self::Vietnamese, Self::English];

    /// Exact supported tag only; an unknown tag is `None`, never a silent default.
    pub fn from_lang(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|locale| locale.lang() == tag)
    }

    /// BCP 47 tag for `lang` attributes, including the document root.
    pub const fn lang(self) -> &'static str {
        match self {
            Self::Vietnamese => "vi-VN",
            Self::English => "en",
        }
    }

    /// The language's own name; a language switch shows it untranslated.
    pub const fn autonym(self) -> &'static str {
        match self {
            Self::Vietnamese => "Tiếng Việt",
            Self::English => "English",
        }
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::Vietnamese => Self::English,
            Self::English => Self::Vietnamese,
        }
    }

    /// Migration adapter for copy functions that still take `english: bool`.
    pub const fn is_english(self) -> bool {
        matches!(self, Self::English)
    }
}

#[derive(Clone, Copy)]
pub struct Copy {
    pub theme: &'static str,
    pub language: &'static str,
    pub system: &'static str,
    pub light: &'static str,
    pub dark: &'static str,
    pub unknown: &'static str,
    pub title: &'static str,
    pub mode: &'static str,
    pub token: &'static str,
    pub login: &'static str,
    pub script: &'static str,
    pub read: &'static str,
    pub draft: &'static str,
    pub save: &'static str,
    pub retry: &'static str,
    pub base: &'static str,
    pub preview: &'static str,
    pub rebase: &'static str,
    pub help: &'static str,
    pub busy: &'static str,
    pub pending: &'static str,
    pub actor: &'static str,
    pub validation: &'static str,
    pub sign_in_first: &'static str,
}

pub fn copy(english: bool) -> Copy {
    if english {
        Copy {
        theme:"Appearance", language:"Language", system:"System", light:"Light", dark:"Dark", unknown:"Unknown",
        title:"Cantos Studio", mode:"Live · local Axum / PostgreSQL · no mock data",
        token:"Development session token", login:"Sign in", script:"Script ID (UUID)",
        read:"Read current revision", draft:"Complete Script IR draft (JSON)",
        save:"Save a new immutable revision", retry:"Retry the submitted snapshot",
        base:"Base revision", preview:"Stored export · read only", rebase:"Keep my text and use this head as base",
        help:"Paste a complete 0.1.0 export. Reading keeps any existing draft. Compare the stored export before choosing a new base. A save does not approve production or publication.",
        busy:"Request in progress; the submitted text is retained.", pending:"Reconcile the submitted snapshot with Retry before reading or making a new save. Sign in as the original submitting actor.",
        actor:"Signed-in actor", validation:"Validation findings", sign_in_first:"Sign in before saving.",
    }
    } else {
        Copy {
        theme:"Giao diện", language:"Ngôn ngữ", system:"Theo hệ thống", light:"Sáng", dark:"Tối", unknown:"Chưa xác định",
        title:"Cantos Studio", mode:"Live · Axum / PostgreSQL local · không dùng dữ liệu mock",
        token:"Token phiên phát triển", login:"Đăng nhập", script:"ID kịch bản (UUID)",
        read:"Đọc bản sửa đổi hiện tại", draft:"Bản nháp Script IR đầy đủ (JSON)",
        save:"Lưu bản sửa đổi bất biến mới", retry:"Thử lại snapshot đã gửi",
        base:"Bản sửa đổi gốc", preview:"Export đã lưu · chỉ đọc", rebase:"Giữ văn bản và dùng head này làm gốc",
        help:"Dán export 0.1.0 đầy đủ. Khi đọc, bản nháp hiện có được giữ nguyên. So sánh export đã lưu trước khi chọn gốc mới. Lưu kịch bản chưa phải duyệt sản xuất hay xuất bản.",
        busy:"Đang xử lý; văn bản đã gửi được giữ nguyên.", pending:"Dùng Thử lại để xác định kết quả snapshot đã gửi trước khi đọc hoặc lưu lần mới. Đăng nhập đúng actor đã gửi snapshot.",
        actor:"Actor đã đăng nhập", validation:"Các lỗi cần sửa", sign_in_first:"Đăng nhập trước khi lưu.",
    }
    }
}

pub fn status(status: Status, english: bool) -> &'static str {
    match (status,english) {
        (Status::Dirty,false) => "Có văn bản chưa lưu.",
        (Status::Dirty,true) => "Your text has unsaved changes.",
        (Status::Idle,false) => "Đăng nhập, rồi đọc hoặc lưu một kịch bản.",
        (Status::Idle,true) => "Sign in, then read or save a script.",
        (Status::Loading,false) => "Đang đọc bản đã lưu…", (Status::Loading,true) => "Reading stored revision…",
        (Status::Saving,false) => "Đang lưu snapshot…", (Status::Saving,true) => "Saving snapshot…",
        (Status::Loaded,false) => "Đã mở bản đã lưu.", (Status::Loaded,true) => "Stored revision opened.",
        (Status::Saved,false) => "Đã lưu bản sửa đổi mới.", (Status::Saved,true) => "New revision saved.",
        (Status::DraftPreserved,false) => "Đã giữ văn bản đang sửa. So sánh với export đã lưu bên dưới.",
        (Status::DraftPreserved,true) => "Your draft is preserved. Compare it with the stored export below.",
        (Status::SignedIn,false) => "Đăng nhập thành công.", (Status::SignedIn,true) => "Signed in.",
        (Status::Unauthenticated,false) => "Phiên chưa hợp lệ hoặc đã hết hạn. Đăng nhập lại; văn bản được giữ nguyên.",
        (Status::Unauthenticated,true) => "Session missing or expired. Sign in again; your text is preserved.",
        (Status::NotFound,false) => "Không tìm thấy kịch bản trong phạm vi quyền của bạn.",
        (Status::NotFound,true) => "Script not found within your access scope.",
        (Status::Forbidden,false) => "Bạn không có quyền thực hiện thao tác này.", (Status::Forbidden,true) => "You do not have permission for this action.",
        (Status::InvalidRequest,false) => "Kiểm tra ID và định dạng yêu cầu; văn bản chưa bị thay đổi.",
        (Status::InvalidRequest,true) => "Check the ID and request format; your text has not changed.",
        (Status::InvalidScript,false) => "Kịch bản chưa hợp lệ. Sửa các lỗi dưới đây rồi lưu lại.",
        (Status::InvalidScript,true) => "Script invalid. Correct the findings below and save again.",
        (Status::EvidenceUnavailable,false) => "Tham chiếu nguồn hoặc quyền chưa có trong tài khoản sở hữu. Cần đăng ký hồ sơ trước khi lưu.",
        (Status::EvidenceUnavailable,true) => "Source or rights references are unavailable in the owner account. Register evidence before saving.",
        (Status::StaleRevision,false) => "Có bản mới hơn. Văn bản của bạn được giữ nguyên; đọc head, so sánh rồi chọn gốc mới.",
        (Status::StaleRevision,true) => "A newer revision exists. Your text is preserved; read the head, compare, then choose a new base.",
        (Status::OperationReused,false) => "Operation ID đã dùng với snapshot khác. Văn bản được giữ; cần bắt đầu lần lưu mới.",
        (Status::OperationReused,true) => "Operation ID was used with another snapshot. Text preserved; start a new save.",
        (Status::ProposalAlreadySubmitted,false) => "Context này đã có đề xuất hợp lệ. Mở lại đề xuất đã lưu để xem và chấp nhận; không tự thay thế kết quả.",
        (Status::ProposalAlreadySubmitted,true) => "This context already has a valid proposal. Open the saved proposal for review and acceptance; it is not silently replaced.",
        (Status::Unavailable,false) => "Chưa xác định được kết quả. Văn bản và snapshot được giữ; thử lại khi kết nối phục hồi.",
        (Status::Unavailable,true) => "Outcome unavailable. Text and snapshot preserved; retry after connection recovers.",
        (Status::CorruptRevision,false) => "Không kiểm chứng được bản đã lưu. Văn bản được giữ; cần kiểm tra backend.",
        (Status::CorruptRevision,true) => "Stored revision integrity check failed. Text preserved; inspect the backend.",
        (Status::StaleProductionInputs, false) => "Đầu vào sản xuất đã đổi. Đọc lại thiết lập và quyền, kiểm tra rồi đóng băng snapshot mới; các giá trị hiện tại được giữ.",
        (Status::StaleProductionInputs, true) => "Production inputs changed. Reload settings and rights, review, then freeze a new snapshot; current values are preserved.",
        (Status::ProductionBlocked, false) => "Phê duyệt bị chặn bởi quyền, ngân sách hoặc rà soát kịch bản. Xem các điều kiện và kiểm tra lại snapshot.",
        (Status::ProductionBlocked, true) => "Approval blocked by permissions, budget or script review. Inspect the findings and check the snapshot again.",
    }
}

pub fn language_toggle_label(english: bool) -> &'static str {
    if english {
        "VI"
    } else {
        "EN"
    }
}

pub fn language_toggle_tooltip(english: bool) -> &'static str {
    if english {
        "Language: English · Switch to Tiếng Việt"
    } else {
        "Ngôn ngữ: Tiếng Việt · Chuyển sang English"
    }
}

#[cfg(test)]
mod tests {
    use super::{copy, status, Locale};
    use crate::editor::Status;

    #[test]
    fn locale_is_typed_at_the_root_and_adapts_to_legacy_copy() {
        assert_eq!(Locale::default(), Locale::Vietnamese);
        for (locale, lang, autonym, english) in [
            (Locale::Vietnamese, "vi-VN", "Tiếng Việt", false),
            (Locale::English, "en", "English", true),
        ] {
            assert_eq!(locale.lang(), lang);
            assert_eq!(Locale::from_lang(lang), Some(locale));
            assert_eq!(locale.autonym(), autonym);
            assert_eq!(locale.is_english(), english);
            assert_ne!(locale.toggled(), locale);
            assert_eq!(locale.toggled().toggled(), locale);
            assert_eq!(copy(locale.is_english()).theme, copy(english).theme);
        }
        for unsupported in ["", "vi", "EN", "en-US", "fr"] {
            assert_eq!(Locale::from_lang(unsupported), None, "{unsupported}");
        }
    }
    #[test]
    fn every_async_and_error_state_has_both_locales() {
        for state in [
            Status::Idle,
            Status::Dirty,
            Status::Loading,
            Status::Saving,
            Status::Loaded,
            Status::Saved,
            Status::DraftPreserved,
            Status::SignedIn,
            Status::Unauthenticated,
            Status::NotFound,
            Status::Forbidden,
            Status::InvalidRequest,
            Status::InvalidScript,
            Status::EvidenceUnavailable,
            Status::StaleRevision,
            Status::OperationReused,
            Status::ProposalAlreadySubmitted,
            Status::Unavailable,
            Status::CorruptRevision,
        ] {
            assert!(!status(state, false).is_empty());
            assert!(!status(state, true).is_empty());
        }
        assert!(copy(false).mode.contains("Live"));
        assert!(copy(true).mode.contains("no mock"));
    }
}

#[derive(Clone, Copy)]
pub struct ImportCopy {
    pub title: &'static str,
    pub help: &'static str,
    pub file: &'static str,
    pub format: &'static str,
    pub txt: &'static str,
    pub markdown: &'static str,
    pub docx: &'static str,
    pub script_ir: &'static str,
    pub reference: &'static str,
    pub rights_holder: &'static str,
    pub permission_evidence: &'static str,
    pub usage_scope: &'static str,
    pub rights_help: &'static str,
    pub save: &'static str,
    pub retry: &'static str,
    pub pending: &'static str,
    pub required: &'static str,
    pub source_id: &'static str,
    pub open: &'static str,
    pub stored: &'static str,
    pub original: &'static str,
    pub download: &'static str,
    pub binary: &'static str,
    pub parsed: &'static str,
    pub parsed_help: &'static str,
    pub warnings: &'static str,
    pub no_warnings: &'static str,
    pub failed: &'static str,
    pub speaker: &'static str,
    pub scene: &'static str,
    pub cue: &'static str,
    pub unknown: &'static str,
    pub script_json: &'static str,
    pub checksum: &'static str,
    pub operation: &'static str,
    pub extractor: &'static str,
    pub recorded_at: &'static str,
    pub imported_by: &'static str,
    pub file_name: &'static str,
    pub byte_len: &'static str,
    pub block: &'static str,
    pub support_code: &'static str,
    pub absent: &'static str,
    pub byte_offset: &'static str,
    pub previous_blocks: &'static str,
    pub next_blocks: &'static str,
}

pub fn import_copy(english: bool) -> ImportCopy {
    if english {
        ImportCopy {
            title: "Import a manuscript",
            help: "Choose an explicit format and a nonempty source reference. Files must be 1 byte to 1 MiB. Importing stores a private immutable source and a separate extraction; it does not create an accepted script revision.",
            file: "Manuscript file", format: "Source format", txt: "Plain text (TXT)", markdown: "Markdown", docx: "Word document (DOCX)", script_ir: "Script IR (JSON)",
            reference: "Source reference (required)", rights_holder: "Rights holder (optional)", permission_evidence: "Permission evidence (optional)", usage_scope: "Allowed usage scope (optional)",
            rights_help: "Publication rights remain unknown. These are recorded claims for review; importing never establishes permission to adapt, generate audio, or publish.",
            save: "Import and save source", retry: "Retry the same import", pending: "Reconcile this submitted snapshot before changing the file or metadata. Sign in as its original submitting actor and retry the same operation.",
            required: "Sign in, choose a file, and enter a source reference before importing.", source_id: "Stored source ID (src_…)", open: "Open stored source", stored: "Stored source · read only", original: "Original source", download: "Download exact original bytes", binary: "This original cannot be shown as UTF-8 text. Download its exact bytes for comparison; the extraction below is a separate representation.",
            parsed: "Normalized extraction", parsed_help: "Prose labels and scene headings are source suggestions requiring review. Unknown speakers and scenes remain unresolved. No AI adaptation has run.", warnings: "Warnings for this page, including source-wide warnings", no_warnings: "There are no warnings for the displayed blocks and no source-wide warnings. Other pages may have warnings. This is not an editorial or rights approval.", failed: "Extraction failed; the original source is preserved. Correct the source or choose another format and submit a new import.",
            speaker: "Speaker label", scene: "Scene", cue: "Cue type", unknown: "Unknown / unresolved", script_json: "Validated Script IR · source preview", checksum: "Original SHA-256", operation: "Submitted operation ID", extractor: "Extractor version", recorded_at: "Recorded at (UTC)", imported_by: "Imported by", file_name: "Original file name", byte_len: "Original bytes", block: "Block index", support_code: "Diagnostic code", absent: "Not recorded", byte_offset: "UTF-8 byte offset", previous_blocks: "Previous blocks", next_blocks: "Next blocks",
        }
    } else {
        ImportCopy {
            title: "Nhập bản thảo",
            help: "Chọn đúng định dạng và nhập tham chiếu nguồn. Tệp cần có dung lượng từ 1 byte đến 1 MiB. Nhập bản thảo lưu nguồn riêng tư bất biến cùng bản trích xuất riêng; chưa tạo bản sửa đổi kịch bản được chấp nhận.",
            file: "Tệp bản thảo", format: "Định dạng nguồn", txt: "Văn bản thuần (TXT)", markdown: "Markdown", docx: "Tài liệu Word (DOCX)", script_ir: "Script IR (JSON)",
            reference: "Tham chiếu nguồn (bắt buộc)", rights_holder: "Chủ sở hữu quyền (không bắt buộc)", permission_evidence: "Bằng chứng cho phép (không bắt buộc)", usage_scope: "Phạm vi sử dụng được phép (không bắt buộc)",
            rights_help: "Quyền xuất bản vẫn chưa xác định. Đây là thông tin được ghi nhận để rà soát; việc nhập không chứng minh quyền chuyển thể, tạo âm thanh hay xuất bản.",
            save: "Nhập và lưu nguồn", retry: "Thử lại cùng lần nhập", pending: "Xác định kết quả snapshot đã gửi trước khi đổi tệp hoặc thông tin. Đăng nhập đúng actor đã gửi và thử lại cùng operation ID.",
            required: "Đăng nhập, chọn tệp và nhập tham chiếu nguồn trước khi lưu.", source_id: "ID nguồn đã lưu (src_…)", open: "Mở nguồn đã lưu", stored: "Nguồn đã lưu · chỉ đọc", original: "Nguồn gốc", download: "Tải xuống đúng byte gốc", binary: "Nguồn gốc này không thể hiển thị dưới dạng văn bản UTF-8. Tải xuống đúng byte gốc để đối chiếu; bản trích xuất bên dưới là biểu diễn riêng.",
            parsed: "Nội dung trích xuất đã chuẩn hóa", parsed_help: "Nhãn người nói và tiêu đề cảnh trong văn xuôi là gợi ý từ nguồn, cần rà soát. Người nói và cảnh chưa rõ vẫn chưa được xác định. Chưa chạy chuyển thể AI.", warnings: "Cảnh báo của trang này, gồm cảnh báo chung cho nguồn", no_warnings: "Các đoạn đang hiển thị không có cảnh báo; cũng không có cảnh báo chung cho nguồn. Các trang khác vẫn có thể có cảnh báo. Điều này chưa phải duyệt biên tập hay duyệt quyền sử dụng.", failed: "Không trích xuất được; nguồn gốc đã được giữ lại. Sửa nguồn hoặc chọn định dạng khác rồi tạo lần nhập mới.",
            speaker: "Nhãn người nói", scene: "Cảnh", cue: "Loại cue", unknown: "Chưa rõ / chưa xác định", script_json: "Script IR đã kiểm tra · xem trước nguồn", checksum: "SHA-256 nguồn gốc", operation: "Operation ID đã gửi", extractor: "Phiên bản bộ trích xuất", recorded_at: "Thời điểm ghi nhận (UTC)", imported_by: "Actor nhập nguồn", file_name: "Tên tệp gốc", byte_len: "Số byte gốc", block: "Chỉ số đoạn", support_code: "Mã chẩn đoán", absent: "Chưa ghi nhận", byte_offset: "Vị trí byte UTF-8", previous_blocks: "Các đoạn trước", next_blocks: "Các đoạn tiếp theo",
        }
    }
}

pub fn import_status(value: &crate::import::ImportStatus, english: bool) -> &'static str {
    use crate::contract::ErrorCode;
    use crate::import::ImportStatus;
    match (value, english) {
        (ImportStatus::Error(ErrorCode::StaleProductionInputs), true) => "Production input scope changed; import selection is preserved.",
        (ImportStatus::Error(ErrorCode::StaleProductionInputs), false) => "Phạm vi đầu vào sản xuất đã đổi; tệp nhập được giữ.",
        (ImportStatus::Error(ErrorCode::ProductionBlocked), true) => "Production inputs are blocked; import selection is preserved.",
        (ImportStatus::Error(ErrorCode::ProductionBlocked), false) => "Đầu vào sản xuất đang bị chặn; tệp nhập được giữ.",
        (ImportStatus::Idle, true) => "Choose a manuscript to import, or open a stored source by its ID.",
        (ImportStatus::Idle, false) => "Chọn bản thảo để nhập hoặc mở nguồn đã lưu bằng ID.",
        (ImportStatus::ReadingFile, true) => "Reading the selected file…",
        (ImportStatus::ReadingFile, false) => "Đang đọc tệp đã chọn…",
        (ImportStatus::Ready, true) => "File prepared. Review the format and source metadata, then import.",
        (ImportStatus::Ready, false) => "Tệp đã sẵn sàng. Kiểm tra định dạng và thông tin nguồn rồi nhập.",
        (ImportStatus::Saving, true) => "Importing and storing the source…",
        (ImportStatus::Saving, false) => "Đang nhập và lưu nguồn…",
        (ImportStatus::Loading, true) => "Opening the stored source…",
        (ImportStatus::Loading, false) => "Đang mở nguồn đã lưu…",
        (ImportStatus::Stored, true) => "Stored source opened. Compare the original with the separate extraction below.",
        (ImportStatus::Stored, false) => "Đã mở nguồn lưu trữ. Đối chiếu nguồn gốc với bản trích xuất riêng bên dưới.",
        (ImportStatus::FileTooLarge, true) => "File must be 1 byte to 1 MiB. Choose a smaller nonempty file; no upload was sent.",
        (ImportStatus::FileTooLarge, false) => "Tệp cần có dung lượng từ 1 byte đến 1 MiB. Chọn tệp nhỏ hơn và có nội dung; chưa gửi tệp lên máy chủ.",
        (ImportStatus::FileReadFailed, true) => "The browser could not read this file. Select it again; no upload was sent.",
        (ImportStatus::FileReadFailed, false) => "Trình duyệt không đọc được tệp này. Chọn lại tệp; chưa gửi tệp lên máy chủ.",
        (ImportStatus::Error(ErrorCode::Unauthenticated), true) => "Session missing or expired. Sign in again; the selected file and submitted snapshot are preserved.",
        (ImportStatus::Error(ErrorCode::Unauthenticated), false) => "Phiên chưa hợp lệ hoặc đã hết hạn. Đăng nhập lại; tệp đã chọn và snapshot đã gửi được giữ nguyên.",
        (ImportStatus::Error(ErrorCode::Forbidden), true) => "This action is unavailable for the signed-in actor. Your file is preserved; retry an unresolved import as its original actor.",
        (ImportStatus::Error(ErrorCode::Forbidden), false) => "Actor đang đăng nhập không có quyền thao tác. Tệp được giữ nguyên; thử lại lần nhập chưa rõ kết quả bằng đúng actor đã gửi.",
        (ImportStatus::Error(ErrorCode::NotFound), true) => "Source not found within your access scope. Check its ID and account; the chosen file is preserved.",
        (ImportStatus::Error(ErrorCode::NotFound), false) => "Không tìm thấy nguồn trong phạm vi quyền của bạn. Kiểm tra ID và tài khoản; tệp đã chọn được giữ nguyên.",
        (ImportStatus::Error(ErrorCode::InvalidRequest), true) => "Check the source ID, format, and metadata limits. Your selected file is preserved; correct the request and submit again.",
        (ImportStatus::Error(ErrorCode::InvalidRequest), false) => "Kiểm tra ID nguồn, định dạng và giới hạn thông tin. Tệp đã chọn được giữ nguyên; sửa yêu cầu rồi gửi lại.",
        (ImportStatus::Error(ErrorCode::OperationReused), true) => "This operation ID belongs to another snapshot. File preserved; review the request and submit a new import.",
        (ImportStatus::Error(ErrorCode::OperationReused), false) => "Operation ID này thuộc snapshot khác. Tệp được giữ nguyên; kiểm tra yêu cầu rồi tạo lần nhập mới.",
        (ImportStatus::Error(ErrorCode::Unavailable), true) => "The import outcome is unavailable. File and submitted snapshot are preserved; retry the same operation after connection recovers.",
        (ImportStatus::Error(ErrorCode::Unavailable), false) => "Chưa xác định được kết quả nhập. Tệp và snapshot được giữ nguyên; thử lại cùng operation khi kết nối phục hồi.",
        (ImportStatus::Error(ErrorCode::CorruptRevision), true) => "Stored source integrity could not be verified. Your file is preserved; retry the unresolved snapshot or ask the operator to inspect storage.",
        (ImportStatus::Error(ErrorCode::CorruptRevision), false) => "Không kiểm chứng được tính toàn vẹn của nguồn đã lưu. Tệp được giữ nguyên; thử lại snapshot chưa rõ kết quả hoặc nhờ người vận hành kiểm tra lưu trữ.",
        (ImportStatus::Error(ErrorCode::InvalidScript), true) => "Structured input was rejected. File preserved; inspect the source and submit a corrected import.",
        (ImportStatus::Error(ErrorCode::InvalidScript), false) => "Nội dung có cấu trúc chưa hợp lệ. Tệp được giữ nguyên; kiểm tra nguồn rồi nhập bản đã sửa.",
        (ImportStatus::Error(ErrorCode::EvidenceUnavailable), true) => "Source evidence is unavailable for this account. File preserved; check the source metadata and account.",
        (ImportStatus::Error(ErrorCode::EvidenceUnavailable), false) => "Tài khoản này chưa có bằng chứng nguồn cần thiết. Tệp được giữ nguyên; kiểm tra thông tin nguồn và tài khoản.",
        (ImportStatus::Error(ErrorCode::StaleRevision), true) => "The referenced revision changed. File preserved; reopen the current source before another request.",
        (ImportStatus::Error(ErrorCode::StaleRevision), false) => "Bản sửa đổi được tham chiếu đã thay đổi. Tệp được giữ nguyên; mở lại nguồn hiện tại trước khi gửi yêu cầu mới.",
        (ImportStatus::Error(ErrorCode::ProposalAlreadySubmitted), english) => status(Status::ProposalAlreadySubmitted, english),
        (ImportStatus::Error(ErrorCode::InvalidCatalog | ErrorCode::AudioRejected), english) => status(Status::InvalidRequest, english),
        (ImportStatus::Error(ErrorCode::CatalogConflict | ErrorCode::StaleCursor), english) => status(Status::StaleRevision, english),
    }
}

pub fn import_format(format: crate::contract::ImportFormat, english: bool) -> &'static str {
    let copy = import_copy(english);
    match format {
        crate::contract::ImportFormat::Txt => copy.txt,
        crate::contract::ImportFormat::Markdown => copy.markdown,
        crate::contract::ImportFormat::Docx => copy.docx,
        crate::contract::ImportFormat::ScriptIr => copy.script_ir,
    }
}

pub fn import_kind(kind: crate::contract::ImportBlockKind, english: bool) -> &'static str {
    use crate::contract::ImportBlockKind;
    match (kind, english) {
        (ImportBlockKind::Paragraph, true) => "Paragraph",
        (ImportBlockKind::Paragraph, false) => "Đoạn văn",
        (ImportBlockKind::Dialogue, true) => "Dialogue",
        (ImportBlockKind::Dialogue, false) => "Lời thoại",
        (ImportBlockKind::Narration, true) => "Narration",
        (ImportBlockKind::Narration, false) => "Lời dẫn",
        (ImportBlockKind::SceneCue, true) => "Scene cue",
        (ImportBlockKind::SceneCue, false) => "Chỉ dẫn cảnh",
        (ImportBlockKind::SoundCue, true) => "Sound cue",
        (ImportBlockKind::SoundCue, false) => "Chỉ dẫn âm thanh",
    }
}

pub fn import_diagnostic(code: &str, english: bool) -> &'static str {
    match (code, english) {
        ("utf8_bom_removed", true) => "The UTF-8 byte-order marker was removed only from the extraction; original bytes are preserved.",
        ("utf8_bom_removed", false) => "Dấu thứ tự byte UTF-8 chỉ được bỏ trong bản trích xuất; byte gốc vẫn được giữ nguyên.",
        ("line_endings_normalized", true) => "Line endings were normalized in the extraction; the original is unchanged.",
        ("line_endings_normalized", false) => "Dấu xuống dòng được chuẩn hóa trong bản trích xuất; nguồn gốc không bị thay đổi.",
        ("unicode_nfc_normalized", true) => "Unicode characters were normalized to NFC in the extraction; compare with the preserved original.",
        ("unicode_nfc_normalized", false) => "Ký tự Unicode được chuẩn hóa sang NFC trong bản trích xuất; đối chiếu với nguồn gốc đã giữ.",
        ("paragraph_edge_whitespace_trimmed", true) => "Whitespace at paragraph edges was trimmed only in the extraction.",
        ("paragraph_edge_whitespace_trimmed", false) => "Khoảng trắng ở đầu và cuối đoạn chỉ được bỏ trong bản trích xuất.",
        ("markdown_list_or_dialogue_ambiguous", true) => "A Markdown list marker could also mean dialogue. The paragraph is preserved without assigning a speaker.",
        ("markdown_list_or_dialogue_ambiguous", false) => "Dấu danh sách Markdown cũng có thể là lời thoại. Đoạn văn được giữ mà không gán người nói.",
        ("docx_images_omitted", true) => "Images are omitted from the text extraction; they remain in the original DOCX.",
        ("docx_images_omitted", false) => "Hình ảnh không được đưa vào bản trích xuất văn bản; chúng vẫn có trong DOCX gốc.",
        ("docx_annotations_omitted", true) => "DOCX annotations are omitted. Review them in the preserved original.",
        ("docx_annotations_omitted", false) => "Chú thích DOCX không được trích xuất. Rà soát chúng trong nguồn gốc đã giữ.",
        ("docx_headers_footers_omitted", true) => "DOCX headers and footers are omitted from the extraction.",
        ("docx_headers_footers_omitted", false) => "Đầu trang và chân trang DOCX không được đưa vào bản trích xuất.",
        ("docx_soft_breaks_segmented", true) => "DOCX soft line breaks were split into text segments. Review the resulting paragraph boundaries.",
        ("docx_soft_breaks_segmented", false) => "Xuống dòng mềm trong DOCX được tách thành các đoạn văn bản. Cần kiểm tra ranh giới đoạn.",
        ("xml_processing_instruction_ignored", true) => "XML processing instructions were ignored and were not executed.",
        ("xml_processing_instruction_ignored", false) => "Chỉ thị xử lý XML được bỏ qua và không được thực thi.",
        ("docx_tables_flattened", true) => "DOCX table text was flattened into reading order. Review the layout in the original.",
        ("docx_tables_flattened", false) => "Văn bản bảng DOCX được chuyển thành thứ tự đọc liên tục. Rà soát bố cục trong nguồn gốc.",
        ("docx_textboxes_flattened", true) => "DOCX text boxes were flattened into text. Review their reading order in the original.",
        ("docx_textboxes_flattened", false) => "Hộp văn bản DOCX được chuyển thành văn bản liên tục. Rà soát thứ tự đọc trong nguồn gốc.",
        ("docx_field_instructions_omitted", true) => "DOCX field instructions were omitted and were not evaluated.",
        ("docx_field_instructions_omitted", false) => "Chỉ thị trường DOCX được bỏ qua và không được thực thi.",
        ("docx_external_content_omitted", true) => "External DOCX content was omitted; no external resource was loaded.",
        ("docx_external_content_omitted", false) => "Nội dung DOCX bên ngoài được bỏ qua; không tải tài nguyên bên ngoài.",
        ("docx_symbols_omitted", true) => "Some DOCX symbol glyphs were omitted. Compare the extraction with the original.",
        ("docx_symbols_omitted", false) => "Một số ký hiệu DOCX không được trích xuất. Đối chiếu bản trích xuất với nguồn gốc.",
        ("script_ir_full_document_preserved", true) => "The complete validated Script IR is preserved below; the block preview is only a reading aid.",
        ("script_ir_full_document_preserved", false) => "Script IR đầy đủ đã kiểm tra được giữ bên dưới; danh sách đoạn chỉ giúp đọc nội dung.",
        ("speaker_label_requires_review", true) => "A source speaker label needs review; it is not a resolved character identity.",
        ("speaker_label_requires_review", false) => "Nhãn người nói từ nguồn cần rà soát; chưa phải danh tính nhân vật được xác nhận.",
        ("speaker_unknown", true) => "The speaker is unknown and remains unresolved.",
        ("speaker_unknown", false) => "Chưa rõ người nói; danh tính vẫn chưa được xác định.",
        ("heading_scene_suggestion", true) => "A source heading suggests a scene; review its meaning and boundaries.",
        ("heading_scene_suggestion", false) => "Tiêu đề nguồn gợi ý một cảnh; cần kiểm tra ý nghĩa và ranh giới cảnh.",
        ("ambiguous_bracket_cue", true) => "Bracketed text has ambiguous cue meaning and has been preserved as source text.",
        ("ambiguous_bracket_cue", false) => "Nội dung trong ngoặc có ý nghĩa cue chưa rõ và đã được giữ như văn bản nguồn.",
        ("markdown_markup_preserved", true) => "Markdown syntax is preserved as source text.",
        ("markdown_markup_preserved", false) => "Cú pháp Markdown được giữ nguyên như văn bản nguồn.",
        ("docx_formatting_omitted", true) => "DOCX formatting is omitted from the extraction; original bytes are preserved.",
        ("docx_formatting_omitted", false) => "Bản trích xuất không giữ định dạng DOCX; byte gốc vẫn được giữ nguyên.",
        ("external_relationships_ignored", true) => "External relationships were ignored; no external resource was loaded.",
        ("external_relationships_ignored", false) => "Các liên kết ngoài được bỏ qua; không tải tài nguyên bên ngoài.",
        ("macros_ignored", true) => "Embedded macros were ignored and were not executed.",
        ("macros_ignored", false) => "Macro nhúng được bỏ qua và không được thực thi.",
        ("embedded_objects_ignored", true) => "Embedded objects were omitted from the extraction.",
        ("embedded_objects_ignored", false) => "Đối tượng nhúng được bỏ qua trong bản trích xuất.",
        ("invalid_utf8", true) => "Text is not valid UTF-8. Convert a copy to UTF-8 and import it again.",
        ("invalid_utf8", false) => "Văn bản không phải UTF-8 hợp lệ. Chuyển một bản sao sang UTF-8 rồi nhập lại.",
        ("empty_source", true) => "No usable source text was found.",
        ("empty_source", false) => "Không tìm thấy văn bản nguồn có thể sử dụng.",
        ("source_too_large" | "decoded_text_limit" | "archive_entry_limit" | "archive_expansion_limit" | "archive_ratio_limit", true) => "The source exceeds a safe extraction limit. Use a smaller chapter or a plain-text copy.",
        ("source_too_large" | "decoded_text_limit" | "archive_entry_limit" | "archive_expansion_limit" | "archive_ratio_limit", false) => "Nguồn vượt giới hạn trích xuất an toàn. Dùng chương nhỏ hơn hoặc bản sao văn bản thuần.",
        ("format_mismatch" | "legacy_or_encrypted_word", true) => "File content does not match the supported format. Choose the correct format or export a plain-text copy.",
        ("format_mismatch" | "legacy_or_encrypted_word", false) => "Nội dung tệp không đúng định dạng được hỗ trợ. Chọn đúng định dạng hoặc xuất bản sao văn bản thuần.",
        ("docx_tracked_changes_require_choice", true) => "DOCX contains tracked changes. Accept or reject them explicitly in a copy before importing.",
        ("docx_tracked_changes_require_choice", false) => "DOCX có thay đổi được theo dõi. Chấp nhận hoặc từ chối rõ ràng trong một bản sao trước khi nhập.",
        ("docx_legacy_encoding_requires_choice" | "unsupported_xml_encoding", true) => "The document uses an unsupported encoding. Export a UTF-8 or current DOCX copy.",
        ("docx_legacy_encoding_requires_choice" | "unsupported_xml_encoding", false) => "Tài liệu dùng mã hóa chưa được hỗ trợ. Xuất bản sao UTF-8 hoặc DOCX hiện hành.",
        ("unsupported_script_ir_version" | "missing_script_ir_version", true) => "This Script IR version is unsupported. Supply a supported complete export.",
        ("unsupported_script_ir_version" | "missing_script_ir_version", false) => "Phiên bản Script IR này chưa được hỗ trợ. Dùng export đầy đủ thuộc phiên bản được hỗ trợ.",
        ("invalid_script_ir" | "script_ir_shape_invalid" | "script_ir_semantics_invalid", true) => "Script IR did not pass validation. Correct the source export before importing again.",
        ("invalid_script_ir" | "script_ir_shape_invalid" | "script_ir_semantics_invalid", false) => "Script IR không qua kiểm tra hợp lệ. Sửa export nguồn trước khi nhập lại.",
        ("malformed_archive" | "malformed_or_encrypted_archive" | "duplicate_archive_entry" | "unsafe_archive_path" | "malformed_xml" | "xml_doctype_forbidden" | "unsupported_control_character", true) => "The source contains malformed or unsafe structures. Export a clean copy; the original is preserved.",
        ("malformed_archive" | "malformed_or_encrypted_archive" | "duplicate_archive_entry" | "unsafe_archive_path" | "malformed_xml" | "xml_doctype_forbidden" | "unsupported_control_character", false) => "Nguồn có cấu trúc hỏng hoặc không an toàn. Xuất bản sao sạch; nguồn gốc đã được giữ lại.",
        ("archive_zip64_unsupported" | "archive_multidisk_unsupported", true) => "This archive layout is unsupported. Export a smaller ordinary DOCX file or a plain-text copy.",
        ("archive_zip64_unsupported" | "archive_multidisk_unsupported", false) => "Kiểu lưu trữ này chưa được hỗ trợ. Xuất tệp DOCX thông thường nhỏ hơn hoặc bản sao văn bản thuần.",
        (_, true) => "The extractor recorded a diagnostic. Review the preserved source and use this code when asking the operator for help.",
        (_, false) => "Bộ trích xuất ghi nhận một chẩn đoán. Kiểm tra nguồn đã giữ và dùng mã này khi nhờ người vận hành hỗ trợ.",
    }
}

pub fn import_cue(kind: crate::contract::ImportCueKind, english: bool) -> &'static str {
    use crate::contract::ImportCueKind;
    match (kind, english) {
        (ImportCueKind::Ambience, true) => "Ambience",
        (ImportCueKind::Ambience, false) => "Âm thanh môi trường",
        (ImportCueKind::Music, true) => "Music",
        (ImportCueKind::Music, false) => "Nhạc",
        (ImportCueKind::Sfx, true) => "Sound effect",
        (ImportCueKind::Sfx, false) => "Hiệu ứng âm thanh",
    }
}

#[cfg(test)]
mod import_tests {
    use super::{import_copy, import_diagnostic, import_kind, import_status};
    use crate::contract::{ErrorCode, ImportBlockKind};
    use crate::import::ImportStatus;

    #[test]
    fn import_states_and_identity_ambiguity_have_both_locales() {
        let mut states = vec![
            ImportStatus::Idle,
            ImportStatus::ReadingFile,
            ImportStatus::Ready,
            ImportStatus::Saving,
            ImportStatus::Loading,
            ImportStatus::Stored,
            ImportStatus::FileTooLarge,
            ImportStatus::FileReadFailed,
        ];
        for code in [
            ErrorCode::Unauthenticated,
            ErrorCode::NotFound,
            ErrorCode::Forbidden,
            ErrorCode::InvalidRequest,
            ErrorCode::InvalidCatalog,
            ErrorCode::AudioRejected,
            ErrorCode::CatalogConflict,
            ErrorCode::StaleCursor,
            ErrorCode::InvalidScript,
            ErrorCode::EvidenceUnavailable,
            ErrorCode::StaleRevision,
            ErrorCode::OperationReused,
            ErrorCode::ProposalAlreadySubmitted,
            ErrorCode::Unavailable,
            ErrorCode::CorruptRevision,
        ] {
            states.push(ImportStatus::Error(code));
        }
        for state in states {
            assert!(!import_status(&state, false).is_empty());
            assert!(!import_status(&state, true).is_empty());
            assert_ne!(import_status(&state, false), import_status(&state, true));
        }
        for kind in [
            ImportBlockKind::Paragraph,
            ImportBlockKind::Dialogue,
            ImportBlockKind::Narration,
            ImportBlockKind::SceneCue,
            ImportBlockKind::SoundCue,
        ] {
            assert!(!import_kind(kind, false).is_empty());
            assert!(!import_kind(kind, true).is_empty());
        }
        assert!(import_copy(true).rights_help.contains("remain unknown"));
        assert!(import_copy(false).rights_help.contains("chưa xác định"));
        assert!(import_diagnostic("speaker_label_requires_review", true).contains("not a resolved"));
        assert!(import_diagnostic("future-diagnostic", true).contains("operator"));
        assert!(import_diagnostic("future-diagnostic", false).contains("người vận hành"));
    }
}

#[cfg(test)]
mod import_warning_tests {
    use super::import_diagnostic;

    #[test]
    fn every_implemented_extraction_warning_has_specific_vietnamese_and_english_copy() {
        for code in [
            "utf8_bom_removed",
            "line_endings_normalized",
            "unicode_nfc_normalized",
            "paragraph_edge_whitespace_trimmed",
            "heading_scene_suggestion",
            "speaker_label_requires_review",
            "speaker_unknown",
            "markdown_list_or_dialogue_ambiguous",
            "ambiguous_bracket_cue",
            "markdown_markup_preserved",
            "macros_ignored",
            "embedded_objects_ignored",
            "docx_images_omitted",
            "docx_annotations_omitted",
            "docx_headers_footers_omitted",
            "docx_formatting_omitted",
            "docx_soft_breaks_segmented",
            "external_relationships_ignored",
            "xml_processing_instruction_ignored",
            "docx_tables_flattened",
            "docx_textboxes_flattened",
            "docx_field_instructions_omitted",
            "docx_external_content_omitted",
            "docx_symbols_omitted",
            "script_ir_full_document_preserved",
        ] {
            for english in [false, true] {
                assert_ne!(
                    import_diagnostic(code, english),
                    import_diagnostic("unknown-future-code", english),
                    "{code}"
                );
            }
        }
    }
}

pub fn import_block_range(page: usize, total: usize, english: bool) -> String {
    let range = crate::import::page_bounds(page, total);
    let start = if total == 0 { 0 } else { range.start + 1 };
    if english {
        format!("Showing blocks {start}–{} of {total}.", range.end)
    } else {
        format!(
            "Đang hiển thị đoạn {start}–{} trên tổng số {total} đoạn.",
            range.end
        )
    }
}

pub fn import_field(path: &str, english: bool) -> &'static str {
    let copy = import_copy(english);
    match path {
        "/metadata/file_name" => copy.file_name,
        "/metadata/reference" => copy.reference,
        "/metadata/rights_holder" => copy.rights_holder,
        "/metadata/permission_evidence" => copy.permission_evidence,
        "/metadata/usage_scope" => copy.usage_scope,
        "/metadata/operation_id" => copy.operation,
        "/original_bytes" => copy.file,
        _ => copy.support_code,
    }
}

pub fn import_field_rule(rule: &str, english: bool) -> &'static str {
    match (rule, english) {
        ("nonblank", true) => "Enter a nonempty value. The selected file is preserved.",
        ("nonblank", false) => "Nhập giá trị có nội dung. Tệp đã chọn được giữ nguyên.",
        ("byte_length", true) => "This file or field is outside its byte length limits. Adjust it and submit again; the selected file is preserved.",
        ("byte_length", false) => "Tệp hoặc trường này nằm ngoài giới hạn số byte. Điều chỉnh rồi gửi lại; tệp đã chọn được giữ nguyên.",
        ("control_character", true) => "Remove unsupported control characters. The selected file is preserved.",
        ("control_character", false) => "Bỏ ký tự điều khiển chưa được hỗ trợ. Tệp đã chọn được giữ nguyên.",
        ("canonical_uuid", true) => "The generated operation ID is invalid. Submit a new request; the selected file is preserved.",
        ("canonical_uuid", false) => "Operation ID được tạo chưa hợp lệ. Gửi yêu cầu mới; tệp đã chọn được giữ nguyên.",
        (_, true) => "Review this field and diagnostic code. The selected file is preserved.",
        (_, false) => "Kiểm tra trường này và mã chẩn đoán. Tệp đã chọn được giữ nguyên.",
    }
}

#[cfg(test)]
mod import_field_tests {
    use super::{import_field, import_field_rule};

    #[test]
    fn safe_metadata_diagnostics_have_field_names_and_recovery_copy_in_both_locales() {
        for path in [
            "/metadata/file_name",
            "/metadata/reference",
            "/metadata/rights_holder",
            "/metadata/permission_evidence",
            "/metadata/usage_scope",
            "/metadata/operation_id",
            "/original_bytes",
        ] {
            for english in [false, true] {
                assert_ne!(
                    import_field(path, english),
                    import_field("future-field", english)
                );
            }
        }
        for rule in [
            "nonblank",
            "byte_length",
            "control_character",
            "canonical_uuid",
        ] {
            for english in [false, true] {
                assert_ne!(
                    import_field_rule(rule, english),
                    import_field_rule("future-rule", english)
                );
            }
        }
    }
}
