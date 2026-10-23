//! Visible and accessible copy belongs to the app, in both supported locales.
/// Locale-neutral notation for an immutable Script export, owned with visible Studio copy.
pub fn script_binding(script: &crate::contract::CatalogScriptRef) -> String {
    format!(
        "{} · r{} · {}",
        script.id, script.revision, script.export_digest
    )
}

/// Locale-neutral identity notation keeps complete IDs and digests unchanged.
pub fn brief_binding(brief: &crate::contract::ProductionBriefDocument) -> String {
    format!(
        "v{} · {}\n{}\nr{} · {}\n{}",
        brief.version,
        brief.id,
        brief.digest,
        brief.content.revision,
        brief.content.id,
        brief.content.revision_digest
    )
}

pub fn content_binding(content: &crate::contract::ContentRevisionResponse) -> String {
    format!(
        "r{} · {} · {}",
        content.revision, content.id, content.revision_digest
    )
}

pub fn submission_binding(submission: &crate::contract::AudioSubmissionDocument) -> String {
    format!(
        "{} · {} · {}",
        submission.id, submission.binding.brief_digest, submission.expected_sha256
    )
}

pub fn rendition_summary(rendition: &crate::contract::AudioRenditionDocument) -> String {
    format!(
        "{} · {} · {} B · {} ms",
        rendition.id,
        rendition.verified_media.profile_id,
        rendition.verified_media.byte_length,
        rendition.verified_media.duration_ms
    )
}

pub fn candidate_binding(candidate: &crate::contract::ReleaseCandidateDocument) -> String {
    format!(
        "{} · {} · {}",
        candidate.id, candidate.digest, candidate.rendition_digest
    )
}

pub fn active_release(
    release: &crate::contract::AudioReleaseDocument,
    pointer_version: u64,
) -> String {
    format!(
        "{} · {} · {}",
        release.id, release.manifest.digest, pointer_version
    )
}

pub fn work_visibility(value: &str, en: bool) -> &'static str {
    text(
        if value == "public" {
            Key::PublicVisibility
        } else {
            Key::PrivateVisibility
        },
        en,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    DistributionRevoked,
    AudioPreview,
    LoadAudioPreview,
    ClearAudioPreview,
    AudioPreviewHelp,
    AudioPreviewFailed,

    WorkVisibility,
    PrivateVisibility,
    PublicVisibility,
    SaveVisibility,
    VisibilityHelp,
    VisibilityConfirm,

    BriefHeadHelp,
    InspectDocument,
    CopyBrief,
    CopyBriefHelp,
    CopiedBrief,
    CopyBriefFailed,
    ProducerProvenance,
    CredentialMissing,
    Publication,
    PublicationHelp,
    ReadReadiness,
    ReadinessNotRead,
    ReadyToPublish,
    PublicationBlocked,
    DistributionRights,
    DistributionDecision,
    Cleared,
    ValidUntil,
    ValidUntilHelp,
    DistributionNotes,
    RecordRights,
    PublicationApproval,
    ApprovalNotes,
    RecordApproval,
    PublishRelease,
    PublishConfirm,
    ReadActiveRelease,
    ActiveRelease,
    NoActiveRelease,
    ActiveNotRead,
    PublicationReceipt,
    ReceiptNotCurrent,
    WithdrawRelease,
    WithdrawalReason,
    WithdrawConfirm,
    WithdrawalReceipt,
    ResetPublicationForm,
    ResetPublicationConfirm,
    PublicationFormDirty,

    Pane,
    Help,
    Refresh,
    Next,
    Title,
    Kind,
    Rights,
    Synopsis,
    CreateWork,
    Work,
    CreateUnit,
    Unit,
    SaveMetadata,
    EditWork,
    ReadWork,
    ReloadWorkMetadata,
    ReloadWorkConfirm,
    ReloadMetadata,
    UnsavedDrafts,
    SelectedUnitMetadata,
    Source,
    Script,
    OpenEditor,
    EditorScript,
    ScriptBindingHelp,
    ArchiveWork,
    ArchiveUnit,
    ArchiveWorkConfirm,
    ArchiveUnitConfirm,
    ArchiveHelp,
    RefreshUnits,
    MoveEarlier,
    MoveLater,
    ReorderHelp,
    Content,
    Body,
    Text,
    Recording,
    Provenance,
    SaveContent,
    ReadHead,
    History,
    Brief,
    Version,
    Notes,
    Profile,
    Use,
    Attribution,
    Naming,
    Qa,
    CreateBrief,
    ReadBrief,
    Audio,
    File,
    Producer,
    Method,
    Quality,
    Prepare,
    Submission,
    ReadSubmission,
    Upload,
    Review,
    ReviewKind,
    Decision,
    Evidence,
    Candidate,
    CreateCandidate,
    ReadCandidate,
    Revoke,
    Cancel,
    Retry,
    Ready,
    Pending,
    Ambiguous,
    Idle,
    Failed,
    Private,
    Empty,
    SelectProfile,
    Confirm,
    ConfirmHelp,
    Stale,
    NoContent,
    FileReady,
    FileReadFailed,
    Reconcile,
    ReviewQc,
    ReviewRights,
    ReviewEditorial,
    Rejected,
    Approved,
    Withdrawn,
    Novel,
    Story,
    Podcast,
    Chapter,
    Episode,
    UnknownKind,
    ErrorInvalidInput,
    ErrorInterrupted,
    ErrorRateLimited,
    ErrorUnexpectedReply,
    ErrorSignIn,
    ErrorForbidden,
    ErrorNotFound,
    ErrorConflict,
    ErrorTooLarge,
    ErrorUnsupported,
    ErrorAdmission,
    ErrorUnavailable,
}
pub fn text(key: Key, en: bool) -> &'static str {
    match (key,en) {
        (Key::DistributionRevoked, false) => "Thu hồi quyền phân phối", (Key::DistributionRevoked, true) => "Revoke distribution rights",
        (Key::AudioPreview, false) => "Nghe thử bản âm thanh riêng tư", (Key::AudioPreview, true) => "Private rendition preview",
        (Key::LoadAudioPreview, false) => "Đọc và kiểm tra đúng bytes để nghe thử", (Key::LoadAudioPreview, true) => "Load exact verified bytes for preview",
        (Key::ClearAudioPreview, false) => "Đóng bản nghe thử riêng tư", (Key::ClearAudioPreview, true) => "Close private preview",
        (Key::AudioPreviewHelp, false) => "Chỉ đọc đúng bản đã tiếp nhận qua phiên Studio. Bản nghe thử không xác nhận QC hoặc quyền phát hành; hãy ghi bằng chứng thực tế.", (Key::AudioPreviewHelp, true) => "Reads the exact admitted rendition through the Studio session. Preview does not establish QC or publication rights; record actual evidence.",
        (Key::AudioPreviewFailed, false) => "Không thể xác minh bản nghe thử riêng tư. Đọc lại biên nhận hoặc thử lại qua phiên hiện tại.", (Key::AudioPreviewFailed, true) => "Could not verify the private preview. Refresh the exact receipt or retry through the current session.",

        (Key::WorkVisibility, false) => "Hiển thị tác phẩm trong danh mục", (Key::WorkVisibility, true) => "Work catalog visibility",
        (Key::PrivateVisibility, false) => "Riêng tư", (Key::PrivateVisibility, true) => "Private",
        (Key::PublicVisibility, false) => "Công khai trong danh mục sau phát hành", (Key::PublicVisibility, true) => "Public catalog after publication",
        (Key::SaveVisibility, false) => "Lưu chế độ hiển thị tác phẩm", (Key::SaveVisibility, true) => "Save work visibility",
        (Key::VisibilityHelp, false) => "Chọn chế độ hiển thị trước khi tạo ứng viên. Đổi thông tin làm ứng viên cũ cần kiểm tra lại. Công khai danh mục không tự phát hành âm thanh.", (Key::VisibilityHelp, true) => "Choose visibility before creating a candidate. Metadata changes require a new candidate check. Public catalog visibility never publishes audio automatically.",
        (Key::VisibilityConfirm, false) => "Đổi chế độ hiển thị của đúng tác phẩm này?", (Key::VisibilityConfirm, true) => "Change catalog visibility for this exact work?",

        (Key::BriefHeadHelp, false) => "Chỉ lưu chỉ dẫn từ nội dung hiện tại đã lưu; hãy lưu bản nháp trước. Đọc lịch sử không đổi liên kết này.", (Key::BriefHeadHelp, true) => "Freeze only saved current content; save local edits first. Reading history does not change this binding.",
        (Key::InspectDocument, false) => "Xem tài liệu đã lưu chính xác", (Key::InspectDocument, true) => "Inspect exact stored document",
        (Key::CopyBrief, false) => "Sao chép chỉ dẫn sản xuất bất biến", (Key::CopyBrief, true) => "Copy immutable production brief",
        (Key::CopyBriefHelp, false) => "Bản sao chứa nội dung riêng tư và khai báo quyền; chỉ gửi qua kênh riêng tư được phép. Không chứa quyền upload.", (Key::CopyBriefHelp, true) => "The copy contains private content and declared permissions; deliver only through an authorized private channel. No upload credential is included.",
        (Key::CopiedBrief, false) => "Đã sao chép đúng bản chỉ dẫn.", (Key::CopiedBrief, true) => "Exact brief copied.",
        (Key::CopyBriefFailed, false) => "Không truy cập được clipboard; chọn và sao chép đúng tài liệu bên dưới.", (Key::CopyBriefFailed, true) => "Clipboard unavailable; select and copy the exact document below.",
        (Key::ProducerProvenance, false) => "Nguồn gốc sản xuất âm thanh", (Key::ProducerProvenance, true) => "Audio production provenance",
        (Key::CredentialMissing, false) => "Quyền upload không còn trong trình duyệt. Đối chiếu thao tác chuẩn bị chưa rõ kết quả trước; nếu đã xác định kết quả, thu hồi và chuẩn bị bản tiếp nhận mới.", (Key::CredentialMissing, true) => "Upload permission is not retained in this browser. Reconcile an unknown prepare first; otherwise revoke and prepare a new submission.",
        (Key::Publication, false) => "Phát hành", (Key::Publication, true) => "Publication",
        (Key::PublicationHelp, false) => "Kiểm tra đúng ứng viên, ghi quyền phân phối và duyệt riêng trước khi chủ động phát hành. Server kiểm tra lại mọi điều kiện khi ghi.", (Key::PublicationHelp, true) => "Inspect the exact candidate, record distribution rights and separate approval, then explicitly publish. The server checks every condition again at commit.",
        (Key::ReadReadiness, false) => "Đọc điều kiện phát hành", (Key::ReadReadiness, true) => "Read publication readiness",
        (Key::ReadinessNotRead, false) => "Chưa đọc điều kiện của đúng ứng viên.", (Key::ReadinessNotRead, true) => "Readiness for this exact candidate has not been read.",
        (Key::ReadyToPublish, false) => "Lần kiểm tra này không có điều kiện chặn; phát hành vẫn cần xác nhận riêng.", (Key::ReadyToPublish, true) => "This observation has no blockers; publication still requires separate confirmation.",
        (Key::PublicationBlocked, false) => "Điều kiện đang chặn phát hành", (Key::PublicationBlocked, true) => "Publication blockers",
        (Key::DistributionRights, false) => "Quyền phân phối của ứng viên", (Key::DistributionRights, true) => "Candidate distribution rights",
        (Key::DistributionDecision, false) => "Quyết định về quyền phân phối", (Key::DistributionDecision, true) => "Distribution decision",
        (Key::Cleared, false) => "Xác nhận quyền phân phối", (Key::Cleared, true) => "Cleared",
        (Key::ValidUntil, false) => "Hết hạn quyền (mili giây Unix UTC)", (Key::ValidUntil, true) => "Rights expiry (UTC Unix milliseconds)",
        (Key::ValidUntilHelp, false) => "Thời điểm hết hạn là độc quyền. Chỉ dùng tham chiếu quyền và bằng chứng đã được đăng ký cho chủ sở hữu. Khai báo không chứng minh tính pháp lý.", (Key::ValidUntilHelp, true) => "The expiry instant is exclusive. Use rights and evidence references already registered for this owner. A declaration does not establish legal verification.",
        (Key::DistributionNotes, false) => "Ghi chú quyền phân phối", (Key::DistributionNotes, true) => "Distribution notes",
        (Key::RecordRights, false) => "Ghi quyết định quyền phân phối", (Key::RecordRights, true) => "Record distribution decision",
        (Key::PublicationApproval, false) => "Duyệt phát hành riêng", (Key::PublicationApproval, true) => "Separate publication approval",
        (Key::ApprovalNotes, false) => "Ghi chú duyệt phát hành", (Key::ApprovalNotes, true) => "Publication approval notes",
        (Key::RecordApproval, false) => "Ghi quyết định duyệt phát hành", (Key::RecordApproval, true) => "Record publication approval",
        (Key::PublishRelease, false) => "Phát hành đúng ứng viên", (Key::PublishRelease, true) => "Publish exact candidate",
        (Key::PublishConfirm, false) => "Phát hành công khai đúng bản đã duyệt này?", (Key::PublishConfirm, true) => "Publish this exact approved version publicly?",
        (Key::ReadActiveRelease, false) => "Đọc bản phát hành đang hoạt động", (Key::ReadActiveRelease, true) => "Read authoritative active release",
        (Key::ActiveRelease, false) => "Bản phát hành đang hoạt động đã đọc", (Key::ActiveRelease, true) => "Observed active release",
        (Key::NoActiveRelease, false) => "Server xác nhận hiện không có bản phát hành hoạt động.", (Key::NoActiveRelease, true) => "The server confirmed no active release at this observation.",
        (Key::ActiveNotRead, false) => "Chưa đọc trạng thái phát hành hiện tại.", (Key::ActiveNotRead, true) => "Current publication state has not been read.",
        (Key::PublicationReceipt, false) => "Biên nhận phát hành bất biến", (Key::PublicationReceipt, true) => "Immutable publication receipt",
        (Key::ReceiptNotCurrent, false) => "Biên nhận xác nhận thao tác gốc; hãy đọc bản phát hành hoạt động để xác nhận trạng thái hiện tại.", (Key::ReceiptNotCurrent, true) => "This receipt acknowledges the original operation. Read the active release to confirm current state.",
        (Key::WithdrawRelease, false) => "Rút bản phát hành đang hoạt động", (Key::WithdrawRelease, true) => "Withdraw observed active release",
        (Key::WithdrawalReason, false) => "Lý do rút bản phát hành", (Key::WithdrawalReason, true) => "Withdrawal reason",
        (Key::WithdrawConfirm, false) => "Rút đúng bản phát hành này? Không tự khôi phục bản cũ.", (Key::WithdrawConfirm, true) => "Withdraw this exact release? No earlier release is restored automatically.",
        (Key::WithdrawalReceipt, false) => "Biên nhận rút bản phát hành", (Key::WithdrawalReceipt, true) => "Withdrawal receipt",
        (Key::ResetPublicationForm, false) => "Bỏ phần nhập phát hành cục bộ", (Key::ResetPublicationForm, true) => "Reset local publication form",
        (Key::ResetPublicationConfirm, false) => "Bỏ phần nhập phát hành chưa lưu? Bản ghi trên server được giữ nguyên.", (Key::ResetPublicationConfirm, true) => "Discard unsaved publication form input? Server records are retained.",
        (Key::PublicationFormDirty, false) => "Có phần nhập phát hành chưa được xác nhận.", (Key::PublicationFormDirty, true) => "Publication form input has not been acknowledged.",

        (Key::UnsavedDrafts, false) => "Có thay đổi chưa lưu trong các chương / tập.", (Key::UnsavedDrafts, true) => "There are unsaved chapter / episode changes.",
        (Key::SelectedUnitMetadata, false) => "Thông tin chương / tập đã chọn", (Key::SelectedUnitMetadata, true) => "Selected chapter / episode metadata",
        (Key::Source, false) => "Nguồn bản thảo", (Key::Source, true) => "Preserved source",
        (Key::Script, false) => "Script IR", (Key::Script, true) => "Script IR",
        (Key::OpenEditor, false) => "Mở trình biên tập", (Key::OpenEditor, true) => "Open editor",
        (Key::EditorScript, false) => "Ghim bản Script IR đã lưu trong trình biên tập", (Key::EditorScript, true) => "Pin the saved Script IR from the editor",
        (Key::ScriptBindingHelp, false) => "Liên kết chính xác bản đã lưu; không tự chuyển sang bản mới nhất.", (Key::ScriptBindingHelp, true) => "Pins this exact saved revision; it never follows the latest revision.",
        (Key::ReloadMetadata, false) => "Nạp lại thông tin chương / tập đã chọn", (Key::ReloadMetadata, true) => "Reload selected chapter / episode metadata",
        (Key::ReloadWorkMetadata, false) => "Dùng thông tin tác phẩm vừa đọc", (Key::ReloadWorkMetadata, true) => "Use last-read work metadata",
        (Key::ReloadWorkConfirm, false) => "Bỏ thay đổi tiêu đề và mô tả để dùng đúng thông tin tác phẩm này? Các bản nháp khác vẫn được giữ.", (Key::ReloadWorkConfirm, true) => "Discard title and synopsis changes and use this exact work metadata? Other drafts are retained.",
        (Key::ReadWork, false) => "Đọc lại tác phẩm đã chọn", (Key::ReadWork, true) => "Refresh selected work",
        (Key::EditWork, false) => "Sửa tác phẩm", (Key::EditWork, true) => "Edit work",
        (Key::ArchiveWork, false) => "Lưu trữ tác phẩm", (Key::ArchiveWork, true) => "Archive work",
        (Key::ArchiveUnit, false) => "Lưu trữ chương / tập", (Key::ArchiveUnit, true) => "Archive chapter / episode",
        (Key::ArchiveWorkConfirm, false) => "Lưu trữ tác phẩm này? Nội dung và lịch sử được giữ; hiện chưa thể khôi phục.", (Key::ArchiveWorkConfirm, true) => "Archive this work? Content and history are retained; restore is currently unavailable.",
        (Key::ArchiveUnitConfirm, false) => "Lưu trữ chương / tập này? Nội dung và lịch sử được giữ; hiện chưa thể khôi phục.", (Key::ArchiveUnitConfirm, true) => "Archive this chapter / episode? Content and history are retained; restore is currently unavailable.",
        (Key::ArchiveHelp, false) => "Lưu hoặc chủ động bỏ mọi thay đổi trước khi lưu trữ. Lưu trữ giữ nguyên lịch sử; hiện chưa thể khôi phục.", (Key::ArchiveHelp, true) => "Save or explicitly discard all changes before archiving. History is retained; restore is currently unavailable.",
        (Key::RefreshUnits, false) => "Tải lại danh sách chương / tập", (Key::RefreshUnits, true) => "Refresh chapters / episodes",
        (Key::MoveEarlier, false) => "Chuyển lên một vị trí", (Key::MoveEarlier, true) => "Move one position earlier",
        (Key::MoveLater, false) => "Chuyển xuống một vị trí", (Key::MoveLater, true) => "Move one position later",
        (Key::ReorderHelp, false) => "Chỉ sắp xếp khi toàn bộ chương / tập đang hoạt động có trong trang đầu. Sau khi sắp xếp, tải lại danh sách để đọc thứ tự đã lưu.", (Key::ReorderHelp, true) => "Reordering requires every active chapter / episode in the first page. After reordering, refresh the list to read the saved order.",
        (Key::FileReadFailed, false) => "Không đọc được tệp; kiểm tra kích thước và thử lại.", (Key::FileReadFailed, true) => "Could not read the file; check size and try again.",
        (Key::Reconcile, false) => "Đối chiếu đúng biên nhận riêng tư", (Key::Reconcile, true) => "Reconcile exact private receipt",
        (Key::ReviewQc, false) => "QC", (Key::ReviewQc, true) => "QC",
        (Key::ReviewRights, false) => "Quyền sử dụng", (Key::ReviewRights, true) => "Rights",
        (Key::ReviewEditorial, false) => "Biên tập", (Key::ReviewEditorial, true) => "Editorial",
        (Key::Rejected, false) => "Từ chối", (Key::Rejected, true) => "Rejected",
        (Key::Approved, false) => "Chấp thuận", (Key::Approved, true) => "Approved",
        (Key::Withdrawn, false) => "Rút lại", (Key::Withdrawn, true) => "Withdrawn",
        (Key::Novel, false) => "Tiểu thuyết", (Key::Novel, true) => "Novel",
        (Key::Story, false) => "Truyện", (Key::Story, true) => "Story",
        (Key::Podcast, false) => "Podcast", (Key::Podcast, true) => "Podcast",
        (Key::Chapter, false) => "Chương", (Key::Chapter, true) => "Chapter",
        (Key::Episode, false) => "Tập", (Key::Episode, true) => "Episode",
        (Key::UnknownKind, false) => "Loại chưa xác định", (Key::UnknownKind, true) => "Unknown type",
        (Key::ErrorInvalidInput, false) => "Kiểm tra các trường bắt buộc, ID cụ thể và giới hạn tệp.", (Key::ErrorInvalidInput, true) => "Check required fields, exact IDs and file bounds.",
        (Key::ErrorInterrupted, false) => "Kết nối bị gián đoạn hoặc hết thời gian.", (Key::ErrorInterrupted, true) => "Connection interrupted or timed out.",
        (Key::ErrorRateLimited, false) => "Quá nhiều yêu cầu. Hãy đợi một lúc rồi thử lại; thao tác trước chưa được coi là thất bại và không được gửi lại tự động.", (Key::ErrorRateLimited, true) => "Too many requests. Wait a while, then try again; the previous action is not treated as failed and is never resent automatically.",
        (Key::ErrorUnexpectedReply, false) => "Phản hồi không hợp lệ; chưa có xác nhận.", (Key::ErrorUnexpectedReply, true) => "Unexpected response; no acknowledgement.",
        (Key::ErrorSignIn, false) => "Đăng nhập lại.", (Key::ErrorSignIn, true) => "Sign in again.",
        (Key::ErrorForbidden, false) => "Quyền hoặc nguồn yêu cầu bị từ chối.", (Key::ErrorForbidden, true) => "Permission or request origin refused.",
        (Key::ErrorNotFound, false) => "Tài nguyên chưa có hoặc không được phép đọc.", (Key::ErrorNotFound, true) => "Resource absent or inaccessible.",
        (Key::ErrorConflict, false) => "Xung đột: đọc lại đúng dữ liệu; giữ bản nháp hiện tại.", (Key::ErrorConflict, true) => "Conflict: refresh exact inputs; keep the current draft.",
        (Key::ErrorTooLarge, false) => "Dữ liệu vượt giới hạn server.", (Key::ErrorTooLarge, true) => "Payload exceeds the server limit.",
        (Key::ErrorUnsupported, false) => "Định dạng dữ liệu chưa được hỗ trợ.", (Key::ErrorUnsupported, true) => "Unsupported media or representation.",
        (Key::ErrorAdmission, false) => "Dữ liệu hoặc âm thanh bị từ chối. Kiểm tra khai báo và bằng chứng.", (Key::ErrorAdmission, true) => "Input or media admission refused. Review declarations and evidence.",
        (Key::ErrorUnavailable, false) => "Dịch vụ chưa khả dụng hoặc yêu cầu bị từ chối.", (Key::ErrorUnavailable, true) => "Service unavailable or request refused.",

        (Key::Pane,true)=>"Content & audio",(Key::Pane,false)=>"Nội dung & âm thanh",
        (Key::Help,true)=>"Private Studio workspace. Stored content, briefs and audio reviews are immutable. Publication requires separate server confirmation.",(Key::Help,false)=>"Không gian Studio riêng tư. Nội dung, chỉ dẫn và đánh giá đã lưu là bất biến. Phát hành cần xác nhận riêng từ server.",
        (Key::Refresh,true)=>"Refresh works",(Key::Refresh,false)=>"Tải tác phẩm",
        (Key::Next,true)=>"Next page",(Key::Next,false)=>"Trang tiếp",
        (Key::Title,true)=>"Title",(Key::Title,false)=>"Tiêu đề",
        (Key::Kind,true)=>"Work type",(Key::Kind,false)=>"Loại tác phẩm",
        (Key::Rights,true)=>"Declared rights references (comma separated)",(Key::Rights,false)=>"Tham chiếu quyền đã khai báo (phân cách bằng dấu phẩy)",
        (Key::Synopsis,true)=>"Synopsis / description",(Key::Synopsis,false)=>"Giới thiệu / mô tả",
        (Key::CreateWork,true)=>"Create work",(Key::CreateWork,false)=>"Tạo tác phẩm",
        (Key::Work,true)=>"Work",(Key::Work,false)=>"Tác phẩm",
        (Key::CreateUnit,true)=>"Create chapter / episode",(Key::CreateUnit,false)=>"Tạo chương / tập",
        (Key::Unit,true)=>"Chapter / episode",(Key::Unit,false)=>"Chương / tập",
        (Key::SaveMetadata,true)=>"Save title and description",(Key::SaveMetadata,false)=>"Lưu tiêu đề và mô tả",
        (Key::Content,true)=>"Immutable content",(Key::Content,false)=>"Nội dung bất biến",
        (Key::Body,true)=>"Content type",(Key::Body,false)=>"Loại nội dung",
        (Key::Text,true)=>"Text",(Key::Text,false)=>"Văn bản",
        (Key::Recording,true)=>"Recording",(Key::Recording,false)=>"Bản thu",
        (Key::Provenance,true)=>"Authorship / source provenance",(Key::Provenance,false)=>"Nguồn gốc / tác giả",
        (Key::SaveContent,true)=>"Append content revision",(Key::SaveContent,false)=>"Lưu bản nội dung mới",
        (Key::ReadHead,true)=>"Read current content",(Key::ReadHead,false)=>"Đọc nội dung hiện tại",
        (Key::History,true)=>"Read content history",(Key::History,false)=>"Đọc lịch sử nội dung",
        (Key::Brief,true)=>"Production instructions",(Key::Brief,false)=>"Chỉ dẫn sản xuất",
        (Key::Version,true)=>"Brief version (0 before first creation)",(Key::Version,false)=>"Phiên bản chỉ dẫn (0 trước lần tạo đầu)",
        (Key::Notes,true)=>"Editorial direction",(Key::Notes,false)=>"Chỉ dẫn biên tập",
        (Key::Profile,true)=>"Explicit delivery profile",(Key::Profile,false)=>"Profile bàn giao cụ thể",
        (Key::Use,true)=>"Permitted use",(Key::Use,false)=>"Phạm vi sử dụng",
        (Key::Attribution,true)=>"Attribution",(Key::Attribution,false)=>"Ghi công",
        (Key::Naming,true)=>"Output naming instruction",(Key::Naming,false)=>"Quy tắc tên bản bàn giao",
        (Key::Qa,true)=>"Quality review instructions",(Key::Qa,false)=>"Chỉ dẫn kiểm tra chất lượng",
        (Key::CreateBrief,true)=>"Freeze new brief",(Key::CreateBrief,false)=>"Lưu chỉ dẫn bất biến mới",
        (Key::ReadBrief,true)=>"Read exact brief",(Key::ReadBrief,false)=>"Đọc đúng bản chỉ dẫn",
        (Key::Audio,true)=>"Private audio admission",(Key::Audio,false)=>"Tiếp nhận âm thanh riêng tư",
        (Key::File,true)=>"Ogg Opus file (1 byte–32 MiB; encoded externally)",(Key::File,false)=>"Tệp Ogg Opus (1 byte–32 MiB; mã hóa bên ngoài)",
        (Key::Producer,true)=>"Producer identity",(Key::Producer,false)=>"Người sản xuất",
        (Key::Method,true)=>"Production method",(Key::Method,false)=>"Phương pháp sản xuất",
        (Key::Quality,true)=>"Producer quality report (measured / listened / NOT_RUN)",(Key::Quality,false)=>"Báo cáo chất lượng của người sản xuất (đo / nghe / NOT_RUN)",
        (Key::Prepare,true)=>"Prepare exact private submission",(Key::Prepare,false)=>"Chuẩn bị tiếp nhận riêng tư",
        (Key::Submission,true)=>"Submission ID",(Key::Submission,false)=>"ID bản tiếp nhận",
        (Key::ReadSubmission,true)=>"Refresh submission status",(Key::ReadSubmission,false)=>"Tải trạng thái tiếp nhận",
        (Key::Upload,true)=>"Upload prepared bytes",(Key::Upload,false)=>"Gửi bytes đã chuẩn bị",
        (Key::Review,true)=>"Named owner review",(Key::Review,false)=>"Đánh giá của chủ sở hữu",
        (Key::ReviewKind,true)=>"Review subject",(Key::ReviewKind,false)=>"Loại đánh giá",
        (Key::Decision,true)=>"Decision",(Key::Decision,false)=>"Quyết định",
        (Key::Evidence,true)=>"Evidence references (comma separated)",(Key::Evidence,false)=>"Tham chiếu bằng chứng (phân cách bằng dấu phẩy)",
        (Key::Candidate,true)=>"Private candidate ID",(Key::Candidate,false)=>"ID ứng viên riêng tư",
        (Key::CreateCandidate,true)=>"Record private candidate",(Key::CreateCandidate,false)=>"Ghi ứng viên riêng tư",
        (Key::ReadCandidate,true)=>"Refresh candidate blockers",(Key::ReadCandidate,false)=>"Tải điều kiện chặn ứng viên",
        (Key::Revoke,true)=>"Revoke upload permission",(Key::Revoke,false)=>"Thu hồi quyền upload",
        (Key::Cancel,true)=>"Cancel pending request",(Key::Cancel,false)=>"Hủy yêu cầu đang chờ",
        (Key::Retry,true)=>"Retry exact pending operation",(Key::Retry,false)=>"Gửi lại đúng thao tác đang chờ",
        (Key::Ready,true)=>"Server acknowledgement received.",(Key::Ready,false)=>"Đã nhận xác nhận từ server.",
        (Key::Pending,true)=>"Request pending…",(Key::Pending,false)=>"Đang chờ phản hồi…",
        (Key::Ambiguous,true)=>"Outcome unknown. Retry the same operation or refresh its exact resource; no successful save is claimed.",(Key::Ambiguous,false)=>"Chưa biết kết quả. Gửi lại đúng thao tác hoặc đọc tài nguyên cụ thể; chưa xác nhận lưu thành công.",
        (Key::Idle,true)=>"No request sent.",(Key::Idle,false)=>"Chưa gửi yêu cầu.",
        (Key::Failed,true)=>"Request failed; browser input is retained.",(Key::Failed,false)=>"Yêu cầu thất bại; phần nhập trong trình duyệt vẫn được giữ.",
        (Key::Private,true)=>"A private candidate is not a public release. Read publication readiness separately.",(Key::Private,false)=>"Ứng viên riêng tư chưa phải bản phát hành. Đọc riêng điều kiện phát hành.",
        (Key::Empty,true)=>"This page is empty.",(Key::Empty,false)=>"Trang này chưa có nội dung.",
        (Key::SelectProfile,true)=>"Choose 20 / 24 / 32 / 40 kbps",(Key::SelectProfile,false)=>"Chọn 20 / 24 / 32 / 40 kbps",
        (Key::Confirm,true)=>"I reviewed this exact rendition and the evidence for the selected decision.",(Key::Confirm,false)=>"Tôi đã xem đúng bản âm thanh và bằng chứng cho quyết định đã chọn.",
        (Key::ConfirmHelp,true)=>"A checkbox does not establish listening or rights. Record actual evidence. Candidate creation never publishes.",(Key::ConfirmHelp,false)=>"Ô xác nhận không chứng minh đã nghe hoặc đủ quyền. Ghi bằng chứng thực tế. Tạo ứng viên không phát hành.",
        (Key::Stale,true)=>"Stored snapshots stay unchanged; refresh exact inputs after a conflict.",(Key::Stale,false)=>"Các bản đã lưu không đổi; đọc lại đúng dữ liệu sau xung đột.",
        (Key::NoContent,true)=>"Read the unit first. A 404 content head is absent, not an empty saved revision.",(Key::NoContent,false)=>"Đọc chương/tập trước. Head nội dung trả404 là chưa có bản, không phải bản rỗng đã lưu.",
        (Key::FileReady,true)=>"File bytes and SHA-256 read; server inspection still required.",(Key::FileReady,false)=>"Đã đọc bytes và SHA-256; vẫn cần server kiểm tra.",
    }
}

/// UI copy is independent from the protocol values kept in the create request.
pub fn work_kind(kind: &str, en: bool) -> &'static str {
    text(
        match kind {
            "novel" => Key::Novel,
            "story" => Key::Story,
            "podcast" => Key::Podcast,
            _ => Key::UnknownKind,
        },
        en,
    )
}

pub fn unit_kind(kind: &str, en: bool) -> &'static str {
    text(
        match kind {
            "chapter" => Key::Chapter,
            "episode" => Key::Episode,
            _ => Key::UnknownKind,
        },
        en,
    )
}

pub fn review_kind(kind: crate::contract::AudioReviewKind, en: bool) -> &'static str {
    use crate::contract::AudioReviewKind;
    text(
        match kind {
            AudioReviewKind::Qc => Key::ReviewQc,
            AudioReviewKind::Rights => Key::ReviewRights,
            AudioReviewKind::Editorial => Key::ReviewEditorial,
        },
        en,
    )
}

pub fn review_decision(decision: crate::contract::AudioReviewDecision, en: bool) -> &'static str {
    use crate::contract::AudioReviewDecision;
    text(
        match decision {
            AudioReviewDecision::Rejected => Key::Rejected,
            AudioReviewDecision::Approved => Key::Approved,
            AudioReviewDecision::Withdrawn => Key::Withdrawn,
        },
        en,
    )
}

/// Closed client errors map to resources; transport/debug details never become user prose.
pub fn error_key(error: &super::ClientError) -> Key {
    use super::ClientError;
    match error {
        ClientError::InvalidInput => Key::ErrorInvalidInput,
        ClientError::Cancelled
        | ClientError::SessionChanged
        | ClientError::ReceiptNotEstablished
        | ClientError::Transport => Key::ErrorInterrupted,
        ClientError::RateLimited { .. } => Key::ErrorRateLimited,
        ClientError::ReplyMismatch | ClientError::Protocol { .. } => Key::ErrorUnexpectedReply,
        ClientError::Http { status, .. } => match status {
            401 => Key::ErrorSignIn,
            403 => Key::ErrorForbidden,
            404 => Key::ErrorNotFound,
            409 => Key::ErrorConflict,
            413 => Key::ErrorTooLarge,
            415 => Key::ErrorUnsupported,
            422 => Key::ErrorAdmission,
            _ => Key::ErrorUnavailable,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{ApiError, ErrorCode};
    use crate::live::ClientError;

    #[test]
    fn every_workspace_resource_has_both_locales() {
        for key in [
            Key::Pane,
            Key::Help,
            Key::Refresh,
            Key::Next,
            Key::Title,
            Key::Kind,
            Key::Rights,
            Key::Synopsis,
            Key::CreateWork,
            Key::Work,
            Key::CreateUnit,
            Key::Unit,
            Key::SaveMetadata,
            Key::Content,
            Key::Body,
            Key::Text,
            Key::Recording,
            Key::Provenance,
            Key::SaveContent,
            Key::ReadHead,
            Key::History,
            Key::Brief,
            Key::Version,
            Key::Notes,
            Key::Profile,
            Key::Use,
            Key::Attribution,
            Key::Naming,
            Key::Qa,
            Key::CreateBrief,
            Key::ReadBrief,
            Key::Audio,
            Key::File,
            Key::Producer,
            Key::Method,
            Key::Quality,
            Key::Prepare,
            Key::Submission,
            Key::ReadSubmission,
            Key::Upload,
            Key::Review,
            Key::ReviewKind,
            Key::Decision,
            Key::Evidence,
            Key::Candidate,
            Key::CreateCandidate,
            Key::ReadCandidate,
            Key::Revoke,
            Key::Cancel,
            Key::Retry,
            Key::Ready,
            Key::Pending,
            Key::Ambiguous,
            Key::Idle,
            Key::Failed,
            Key::Private,
            Key::Empty,
            Key::SelectProfile,
            Key::Confirm,
            Key::ConfirmHelp,
            Key::Stale,
            Key::NoContent,
            Key::FileReady,
            Key::FileReadFailed,
            Key::Reconcile,
            Key::ReviewQc,
            Key::ReviewRights,
            Key::ReviewEditorial,
            Key::Rejected,
            Key::Approved,
            Key::Withdrawn,
            Key::Novel,
            Key::Story,
            Key::Podcast,
            Key::Chapter,
            Key::Episode,
            Key::UnknownKind,
            Key::ErrorInvalidInput,
            Key::ErrorInterrupted,
            Key::ErrorRateLimited,
            Key::ErrorUnexpectedReply,
            Key::ErrorSignIn,
            Key::ErrorForbidden,
            Key::ErrorNotFound,
            Key::ErrorConflict,
            Key::ErrorTooLarge,
            Key::ErrorUnsupported,
            Key::ErrorAdmission,
            Key::ErrorUnavailable,
        ] {
            for en in [false, true] {
                assert!(!text(key, en).trim().is_empty(), "{key:?} / {en}");
            }
        }
    }

    #[test]
    fn every_client_failure_maps_to_copy_without_transport_or_server_prose() {
        let cases = [
            (ClientError::InvalidInput, Key::ErrorInvalidInput),
            (ClientError::Cancelled, Key::ErrorInterrupted),
            (ClientError::SessionChanged, Key::ErrorInterrupted),
            (ClientError::ReceiptNotEstablished, Key::ErrorInterrupted),
            (ClientError::Transport, Key::ErrorInterrupted),
            (
                ClientError::RateLimited { seconds: 30 },
                Key::ErrorRateLimited,
            ),
            (ClientError::ReplyMismatch, Key::ErrorUnexpectedReply),
            (
                ClientError::Protocol { status: 502 },
                Key::ErrorUnexpectedReply,
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error_key(&error), expected);
            for en in [false, true] {
                assert!(!text(error_key(&error), en).is_empty());
            }
        }
        for (status, expected) in [
            (401, Key::ErrorSignIn),
            (403, Key::ErrorForbidden),
            (404, Key::ErrorNotFound),
            (409, Key::ErrorConflict),
            (413, Key::ErrorTooLarge),
            (415, Key::ErrorUnsupported),
            (422, Key::ErrorAdmission),
            (500, Key::ErrorUnavailable),
            (599, Key::ErrorUnavailable),
        ] {
            let error = ClientError::Http {
                status,
                error: ApiError {
                    code: ErrorCode::InvalidCatalog,
                    current_revision: Some(987654321),
                    issues: vec![],
                },
            };
            assert_eq!(error_key(&error), expected);
            for en in [false, true] {
                let message = text(error_key(&error), en);
                assert!(!message.contains("HTTP"));
                assert!(!message.contains("InvalidCatalog"));
                assert!(!message.contains("987654321"));
            }
        }
    }

    #[test]
    fn script_binding_preserves_exact_identifiers_without_expanding_inserted_braces() {
        let script = crate::contract::CatalogScriptRef {
            id: "script-{revision}".into(),
            revision: 7,
            export_digest: "sir-e1:sha256:abc".into(),
        };
        assert_eq!(
            script_binding(&script),
            "script-{revision} · r7 · sir-e1:sha256:abc"
        );
    }

    #[test]
    fn review_variants_and_catalog_types_have_exact_bilingual_presentation() {
        use crate::contract::{AudioReviewDecision as Decision, AudioReviewKind as Kind};
        for (kind, vi, en) in [
            (Kind::Qc, "QC", "QC"),
            (Kind::Rights, "Quyền sử dụng", "Rights"),
            (Kind::Editorial, "Biên tập", "Editorial"),
        ] {
            assert_eq!(review_kind(kind, false), vi);
            assert_eq!(review_kind(kind, true), en);
        }
        for (decision, vi, en) in [
            (Decision::Rejected, "Từ chối", "Rejected"),
            (Decision::Approved, "Chấp thuận", "Approved"),
            (Decision::Withdrawn, "Rút lại", "Withdrawn"),
        ] {
            assert_eq!(review_decision(decision, false), vi);
            assert_eq!(review_decision(decision, true), en);
        }
        assert_eq!(work_kind("story", false), "Truyện");
        assert_eq!(unit_kind("episode", true), "Episode");
        assert_eq!(unit_kind("chapter", false), "Chương");
        assert_eq!(work_kind("new-private-kind", false), "Loại chưa xác định");
        assert_eq!(unit_kind("new-private-kind", true), "Unknown type");
    }
}

pub fn brief_freshness(value: crate::contract::BriefFreshness, en: bool) -> &'static str {
    use crate::contract::BriefFreshness;
    match (value, en) {
        (BriefFreshness::Current, false) => "Chỉ dẫn còn hiện hành",
        (BriefFreshness::Current, true) => "Current brief",
        (BriefFreshness::ContentChanged, false) => "Nội dung đã thay đổi",
        (BriefFreshness::ContentChanged, true) => "Content changed",
        (BriefFreshness::RightsChanged, false) => "Quyền đã thay đổi",
        (BriefFreshness::RightsChanged, true) => "Rights changed",
        (BriefFreshness::GuidanceChanged, false) => "Đã có chỉ dẫn mới hơn",
        (BriefFreshness::GuidanceChanged, true) => "Newer guidance exists",
        (BriefFreshness::Archived, false) => "Đã lưu trữ",
        (BriefFreshness::Archived, true) => "Archived",
    }
}
pub fn submission_state(value: crate::contract::AudioSubmissionState, en: bool) -> &'static str {
    use crate::contract::AudioSubmissionState;
    match (value, en) {
        (AudioSubmissionState::AwaitingBytes, false) => "Đang chờ bytes âm thanh",
        (AudioSubmissionState::AwaitingBytes, true) => "Awaiting audio bytes",
        (AudioSubmissionState::Admitted, false) => "Đã kiểm tra và tiếp nhận riêng tư",
        (AudioSubmissionState::Admitted, true) => "Verified private admission",
        (AudioSubmissionState::Revoked, false) => "Quyền upload đã bị thu hồi",
        (AudioSubmissionState::Revoked, true) => "Upload permission revoked",
        (AudioSubmissionState::Expired, false) => "Quyền upload đã hết hạn",
        (AudioSubmissionState::Expired, true) => "Upload permission expired",
        (AudioSubmissionState::Stale, false) => "Đầu vào đã cũ",
        (AudioSubmissionState::Stale, true) => "Production inputs stale",
    }
}
pub fn audio_blocker(value: crate::contract::AudioReleaseBlocker, en: bool) -> &'static str {
    use crate::contract::AudioReleaseBlocker;
    match (value, en) {
        (AudioReleaseBlocker::StaleBrief, false) => "Chỉ dẫn đã cũ",
        (AudioReleaseBlocker::StaleBrief, true) => "Brief is stale",
        (AudioReleaseBlocker::UploadPermissionRevoked, false) => "Quyền upload bị thu hồi",
        (AudioReleaseBlocker::UploadPermissionRevoked, true) => "Upload permission revoked",
        (AudioReleaseBlocker::UploadPermissionExpired, false) => "Quyền upload hết hạn",
        (AudioReleaseBlocker::UploadPermissionExpired, true) => "Upload permission expired",
        (AudioReleaseBlocker::RightsUnrecorded, false) => "Chưa ghi quyền",
        (AudioReleaseBlocker::RightsUnrecorded, true) => "Rights unrecorded",
        (AudioReleaseBlocker::QcNotApproved, false) => "QC chưa được chấp thuận",
        (AudioReleaseBlocker::QcNotApproved, true) => "QC not approved",
        (AudioReleaseBlocker::RightsNotApproved, false) => "Đánh giá quyền chưa được chấp thuận",
        (AudioReleaseBlocker::RightsNotApproved, true) => "Rights review not approved",
        (AudioReleaseBlocker::EditorialNotApproved, false) => "Biên tập chưa được chấp thuận",
        (AudioReleaseBlocker::EditorialNotApproved, true) => "Editorial review not approved",
        (AudioReleaseBlocker::CandidateReviewsChanged, false) => {
            "Đánh giá của ứng viên đã thay đổi"
        }
        (AudioReleaseBlocker::CandidateReviewsChanged, true) => "Candidate reviews changed",
        (AudioReleaseBlocker::MetadataChanged, false) => {
            "Thông tin tác phẩm hoặc chương/tập đã thay đổi"
        }
        (AudioReleaseBlocker::MetadataChanged, true) => "Work or unit metadata changed",
        (AudioReleaseBlocker::PublicDeliveryNotImplemented, false) => {
            "Ứng viên riêng tư không tự phát hành; đọc điều kiện phát hành riêng"
        }
        (AudioReleaseBlocker::PublicDeliveryNotImplemented, true) => {
            "Private candidate creation does not publish; read publication readiness separately"
        }
    }
}
pub fn publication_blocker(value: crate::contract::PublicationBlocker, en: bool) -> &'static str {
    use crate::contract::PublicationBlocker;
    match (value, en) {
        (PublicationBlocker::CandidateMismatch, false) => "Ứng viên không khớp",
        (PublicationBlocker::CandidateMismatch, true) => "Candidate mismatch",
        (PublicationBlocker::OwnerScopeLost, false) => "Không còn quyền chủ sở hữu",
        (PublicationBlocker::OwnerScopeLost, true) => "Owner scope lost",
        (PublicationBlocker::Archived, false) => "Tác phẩm hoặc chương/tập đã lưu trữ",
        (PublicationBlocker::Archived, true) => "Work or unit archived",
        (PublicationBlocker::StaleBrief, false) => "Chỉ dẫn đã cũ",
        (PublicationBlocker::StaleBrief, true) => "Brief is stale",
        (PublicationBlocker::MetadataChanged, false) => {
            "Thông tin tác phẩm hoặc chương/tập đã thay đổi"
        }
        (PublicationBlocker::MetadataChanged, true) => "Work or unit metadata changed",
        (PublicationBlocker::UploadPermissionRevoked, false) => "Quyền upload bị thu hồi",
        (PublicationBlocker::UploadPermissionRevoked, true) => "Upload permission revoked",
        (PublicationBlocker::UploadPermissionExpired, false) => "Quyền upload hết hạn",
        (PublicationBlocker::UploadPermissionExpired, true) => "Upload permission expired",
        (PublicationBlocker::ProfileUnsupported, false) => "Profile âm thanh chưa được hỗ trợ",
        (PublicationBlocker::ProfileUnsupported, true) => "Unsupported audio profile",
        (PublicationBlocker::ObjectMissing, false) => "Thiếu bytes âm thanh riêng tư",
        (PublicationBlocker::ObjectMissing, true) => "Private media missing",
        (PublicationBlocker::ObjectUnverified, false) => "Bytes âm thanh chưa được xác minh",
        (PublicationBlocker::ObjectUnverified, true) => "Media bytes unverified",
        (PublicationBlocker::ObjectIntegrityMismatch, false) => {
            "Bytes âm thanh không khớp checksum"
        }
        (PublicationBlocker::ObjectIntegrityMismatch, true) => "Media integrity mismatch",
        (PublicationBlocker::QcNotApproved, false) => "QC chưa được chấp thuận",
        (PublicationBlocker::QcNotApproved, true) => "QC not approved",
        (PublicationBlocker::RightsReviewNotApproved, false) => {
            "Đánh giá quyền chưa được chấp thuận"
        }
        (PublicationBlocker::RightsReviewNotApproved, true) => "Rights review not approved",
        (PublicationBlocker::EditorialNotApproved, false) => "Biên tập chưa được chấp thuận",
        (PublicationBlocker::EditorialNotApproved, true) => "Editorial review not approved",
        (PublicationBlocker::CandidateReviewsChanged, false) => "Đánh giá của ứng viên đã thay đổi",
        (PublicationBlocker::CandidateReviewsChanged, true) => "Candidate reviews changed",
        (PublicationBlocker::RightsUnrecorded, false) => "Chưa ghi quyền phân phối",
        (PublicationBlocker::RightsUnrecorded, true) => "Distribution rights unrecorded",
        (PublicationBlocker::RightsRevoked, false) => "Quyền phân phối bị thu hồi",
        (PublicationBlocker::RightsRevoked, true) => "Distribution rights revoked",
        (PublicationBlocker::RightsExpired, false) => "Quyền phân phối hết hạn",
        (PublicationBlocker::RightsExpired, true) => "Distribution rights expired",
        (PublicationBlocker::RightsScopeMismatch, false) => "Quyền phân phối không đủ phạm vi",
        (PublicationBlocker::RightsScopeMismatch, true) => "Distribution rights scope mismatch",
        (PublicationBlocker::ApprovalMissing, false) => "Chưa duyệt phát hành",
        (PublicationBlocker::ApprovalMissing, true) => "Publication approval missing",
        (PublicationBlocker::ApprovalWithdrawn, false) => "Duyệt phát hành đã bị rút",
        (PublicationBlocker::ApprovalWithdrawn, true) => "Publication approval withdrawn",
        (PublicationBlocker::ApprovalStale, false) => "Duyệt phát hành đã cũ",
        (PublicationBlocker::ApprovalStale, true) => "Publication approval stale",
        (PublicationBlocker::DeliveryUnavailable, false) => {
            "Đường phân phối công khai chưa khả dụng"
        }
        (PublicationBlocker::DeliveryUnavailable, true) => "Public delivery unavailable",
        (PublicationBlocker::DeliveryMissing, false) => "Thiếu đối tượng phân phối công khai",
        (PublicationBlocker::DeliveryMissing, true) => "Publication object missing",
        (PublicationBlocker::DeliveryIntegrityMismatch, false) => {
            "Bytes phân phối công khai không khớp"
        }
        (PublicationBlocker::DeliveryIntegrityMismatch, true) => {
            "Public delivery integrity mismatch"
        }
        (PublicationBlocker::DeliveryEvidenceExpired, false) => {
            "Bằng chứng đường phân phối đã hết hạn"
        }
        (PublicationBlocker::DeliveryEvidenceExpired, true) => "Delivery evidence expired",
    }
}

pub fn publication_confirmation(action: Key, title: &str, identity: &str, en: bool) -> String {
    format!("{}\n{}\n{}", text(action, en), title, identity)
}

#[cfg(test)]
mod production_copy_tests {
    use super::*;
    #[test]
    fn production_resources_and_closed_states_have_both_locales() {
        for key in [
            Key::BriefHeadHelp,
            Key::InspectDocument,
            Key::CopyBrief,
            Key::CopyBriefHelp,
            Key::CopiedBrief,
            Key::CopyBriefFailed,
            Key::ProducerProvenance,
            Key::CredentialMissing,
            Key::Publication,
            Key::PublicationHelp,
            Key::ReadReadiness,
            Key::ReadinessNotRead,
            Key::ReadyToPublish,
            Key::PublicationBlocked,
            Key::DistributionRights,
            Key::DistributionDecision,
            Key::Cleared,
            Key::ValidUntil,
            Key::ValidUntilHelp,
            Key::DistributionNotes,
            Key::RecordRights,
            Key::PublicationApproval,
            Key::ApprovalNotes,
            Key::RecordApproval,
            Key::PublishRelease,
            Key::PublishConfirm,
            Key::ReadActiveRelease,
            Key::ActiveRelease,
            Key::NoActiveRelease,
            Key::ActiveNotRead,
            Key::PublicationReceipt,
            Key::ReceiptNotCurrent,
            Key::WithdrawRelease,
            Key::WithdrawalReason,
            Key::WithdrawConfirm,
            Key::WithdrawalReceipt,
            Key::ResetPublicationForm,
            Key::ResetPublicationConfirm,
            Key::PublicationFormDirty,
        ] {
            for en in [false, true] {
                assert!(!text(key, en).trim().is_empty());
            }
        }
        for value in [
            crate::contract::BriefFreshness::Current,
            crate::contract::BriefFreshness::ContentChanged,
            crate::contract::BriefFreshness::RightsChanged,
            crate::contract::BriefFreshness::GuidanceChanged,
            crate::contract::BriefFreshness::Archived,
        ] {
            for en in [false, true] {
                assert!(!brief_freshness(value.clone(), en).trim().is_empty());
            }
        }
        for value in [
            crate::contract::AudioSubmissionState::AwaitingBytes,
            crate::contract::AudioSubmissionState::Admitted,
            crate::contract::AudioSubmissionState::Revoked,
            crate::contract::AudioSubmissionState::Expired,
            crate::contract::AudioSubmissionState::Stale,
        ] {
            for en in [false, true] {
                assert!(!submission_state(value, en).trim().is_empty());
            }
        }
        for value in [
            crate::contract::AudioReleaseBlocker::StaleBrief,
            crate::contract::AudioReleaseBlocker::UploadPermissionRevoked,
            crate::contract::AudioReleaseBlocker::UploadPermissionExpired,
            crate::contract::AudioReleaseBlocker::RightsUnrecorded,
            crate::contract::AudioReleaseBlocker::QcNotApproved,
            crate::contract::AudioReleaseBlocker::RightsNotApproved,
            crate::contract::AudioReleaseBlocker::EditorialNotApproved,
            crate::contract::AudioReleaseBlocker::CandidateReviewsChanged,
            crate::contract::AudioReleaseBlocker::MetadataChanged,
            crate::contract::AudioReleaseBlocker::PublicDeliveryNotImplemented,
        ] {
            for en in [false, true] {
                assert!(!audio_blocker(value, en).trim().is_empty());
            }
        }
        for value in [
            crate::contract::PublicationBlocker::CandidateMismatch,
            crate::contract::PublicationBlocker::OwnerScopeLost,
            crate::contract::PublicationBlocker::Archived,
            crate::contract::PublicationBlocker::StaleBrief,
            crate::contract::PublicationBlocker::MetadataChanged,
            crate::contract::PublicationBlocker::UploadPermissionRevoked,
            crate::contract::PublicationBlocker::UploadPermissionExpired,
            crate::contract::PublicationBlocker::ProfileUnsupported,
            crate::contract::PublicationBlocker::ObjectMissing,
            crate::contract::PublicationBlocker::ObjectUnverified,
            crate::contract::PublicationBlocker::ObjectIntegrityMismatch,
            crate::contract::PublicationBlocker::QcNotApproved,
            crate::contract::PublicationBlocker::RightsReviewNotApproved,
            crate::contract::PublicationBlocker::EditorialNotApproved,
            crate::contract::PublicationBlocker::CandidateReviewsChanged,
            crate::contract::PublicationBlocker::RightsUnrecorded,
            crate::contract::PublicationBlocker::RightsRevoked,
            crate::contract::PublicationBlocker::RightsExpired,
            crate::contract::PublicationBlocker::RightsScopeMismatch,
            crate::contract::PublicationBlocker::ApprovalMissing,
            crate::contract::PublicationBlocker::ApprovalWithdrawn,
            crate::contract::PublicationBlocker::ApprovalStale,
            crate::contract::PublicationBlocker::DeliveryUnavailable,
            crate::contract::PublicationBlocker::DeliveryMissing,
            crate::contract::PublicationBlocker::DeliveryIntegrityMismatch,
            crate::contract::PublicationBlocker::DeliveryEvidenceExpired,
        ] {
            for en in [false, true] {
                assert!(!publication_blocker(value, en).trim().is_empty());
            }
        }
    }
    #[test]
    fn confirmation_keeps_private_identity_literal_and_exact() {
        for en in [false, true] {
            let result = publication_confirmation(
                Key::PublishConfirm,
                "Lời hẹn {identity}",
                "candidate-{title} · cc-r1:abc",
                en,
            );
            assert!(result.ends_with("Lời hẹn {identity}\ncandidate-{title} · cc-r1:abc"));
        }
    }
}
