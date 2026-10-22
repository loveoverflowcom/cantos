//! Visible and accessible copy belongs to the app, in both supported locales.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
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
        (Key::Help,true)=>"Private Studio workspace. Stored content, briefs and audio reviews are immutable. Public delivery is unavailable.",(Key::Help,false)=>"Không gian Studio riêng tư. Nội dung, chỉ dẫn và đánh giá đã lưu là bất biến. Phát hành công khai chưa khả dụng.",
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
        (Key::Private,true)=>"Private only · public_delivery_not_implemented",(Key::Private,false)=>"Chỉ riêng tư · public_delivery_not_implemented",
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

pub fn review_kind(kind: cantos_contracts::AudioReviewKind, en: bool) -> &'static str {
    use cantos_contracts::AudioReviewKind;
    text(
        match kind {
            AudioReviewKind::Qc => Key::ReviewQc,
            AudioReviewKind::Rights => Key::ReviewRights,
            AudioReviewKind::Editorial => Key::ReviewEditorial,
        },
        en,
    )
}

pub fn review_decision(decision: cantos_contracts::AudioReviewDecision, en: bool) -> &'static str {
    use cantos_contracts::AudioReviewDecision;
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
    use crate::live::ClientError;
    use cantos_contracts::{ApiError, ErrorCode};

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
    fn review_variants_and_catalog_types_have_exact_bilingual_presentation() {
        use cantos_contracts::{AudioReviewDecision as Decision, AudioReviewKind as Kind};
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
