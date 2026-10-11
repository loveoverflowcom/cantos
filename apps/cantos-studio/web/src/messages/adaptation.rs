use crate::adaptation::ReviewStatus;
use cantos_contracts::{
    AdaptationCost, AdaptationCurrency, AdaptationFindingCode, AdaptationStatus,
    AdaptationSubmissionStatus, AdaptationUsage,
};

#[derive(Clone, Copy)]
pub struct AdaptationCopy {
    pub title: &'static str,
    pub help: &'static str,
    pub provider: &'static str,
    pub model: &'static str,
    pub endpoint: &'static str,
    pub prompt: &'static str,
    pub config: &'static str,
    pub source_id: &'static str,
    pub script_id: &'static str,
    pub base: &'static str,
    pub pending: &'static str,
    pub retry: &'static str,
    pub operation: &'static str,
    pub run_id: &'static str,
    pub open: &'static str,
    pub refresh: &'static str,
    pub new_run: &'static str,
    pub new_help: &'static str,
    pub provenance: &'static str,
    pub checksum: &'static str,
    pub extraction: &'static str,
    pub input_digest: &'static str,
    pub records: &'static str,
    pub cost: &'static str,
    pub usage: &'static str,
    pub unknown: &'static str,
    pub original: &'static str,
    pub source_missing: &'static str,
    pub original_binary: &'static str,
    pub download: &'static str,
    pub proposal: &'static str,
    pub proposal_help: &'static str,
    pub edit: &'static str,
    pub findings: &'static str,
    pub coverage: &'static str,
    pub coverage_help: &'static str,
    pub represented: &'static str,
    pub omitted: &'static str,
    pub block: &'static str,
    pub review: &'static str,
    pub accept: &'static str,
    pub accept_help: &'static str,
    pub accept_required: &'static str,
    pub accepted: &'static str,
    pub reopen: &'static str,
    pub problem: &'static str,
    pub no_findings: &'static str,
}

pub fn adaptation_copy(english: bool) -> AdaptationCopy {
    if english {
        AdaptationCopy {
            title: "Host-generated radio-drama review", help: "Cantos stores context and validates proposals; it does not generate content or call AI. Prepare a pinned context with the tools, generate in your chosen host under your control, submit the result through the tools, then open its ID here to compare, edit and explicitly accept.",
            provider: "Declared provider / method · unverified", model: "Declared model · unverified", endpoint: "Historical declared destination", prompt: "Prompt / contract version", config: "Recorded settings · unverified",
            source_id: "Adaptation source ID", script_id: "Target script ID (UUID)", base: "Pinned base revision", pending: "The submitted outcome is unresolved. Retry the exact operation as its original actor; keep the operation ID to recover it.", retry: "Retry exact submitted operation", operation: "Operation ID",
            run_id: "Context / proposal ID (UUID)", open: "Open context or submitted proposal", refresh: "Refresh context / proposal status", new_run: "Choose another proposal / discard browser edits", new_help: "Refreshing preserves browser edits. To open another proposal, explicitly discard them. Retry reconciles only the original acceptance snapshot; it does not generate content.", provenance: "Pinned context / proposal provenance", checksum: "Source checksum", extraction: "Extraction version", input_digest: "Input content / export digest", records: "Generation / rights claim records", cost: "Reported cost · unverified", usage: "Reported input / output tokens · unverified", unknown: "Unknown · not reported", original: "Preserved source and extraction", source_missing: "Open a context or proposal to inspect its pinned source. Compare every extraction page and its warnings before acceptance.", original_binary: "The original is a binary document. Download it and compare its extraction below.", download: "Download preserved original", proposal: "Submitted host proposal", proposal_help: "This JSON is editable. The backend validates the complete Script IR and its pinned provenance on acceptance. Source coverage is a submitted claim to review, not proof that every fact is preserved.", edit: "Proposal Script IR (editable JSON)", findings: "Review findings", coverage: "Submitted source coverage claim", coverage_help: "Block numbers follow the extraction above. Review represented text as well as omissions; all coverage records are shown.", represented: "Represented", omitted: "Omitted", block: "Source block", review: "I compared the source and pinned input revision with the current proposal and reviewed unresolved facts, speakers, omissions and performance suggestions.", accept: "Accept current proposal as a new revision", accept_help: "This explicit action saves a new immutable revision. It does not overwrite the source or earlier revisions, approve production, or grant publication rights.", accept_required: "A valid submitted proposal, matching pinned source/input and explicit review of the current text are required. A changed base requires a new context; it is never silently rebased.", accepted: "Accepted immutable revision · read only", reopen: "Reopen this exact accepted revision", problem: "Submitted result failed validation or needs review", no_findings: "No additional result findings. You must still compare the complete source and proposal.",
        }
    } else {
        AdaptationCopy {
            title: "Biên tập đề xuất kịch phát thanh từ host", help: "Cantos lưu context và kiểm tra đề xuất; không tự sinh nội dung hay gọi AI. Dùng tool chuẩn bị context đã ghim, tự tạo nội dung trong host bạn chọn, gửi kết quả qua tool, rồi mở ID tại đây để đối chiếu, sửa và chấp nhận rõ ràng.",
            provider: "Provider / phương thức khai báo · chưa xác minh", model: "Model khai báo · chưa xác minh", endpoint: "Đích xử lý khai báo trước đây", prompt: "Phiên bản prompt / hợp đồng", config: "Thiết lập được khai báo · chưa xác minh",
            source_id: "ID nguồn chuyển thể", script_id: "ID kịch bản đích (UUID)", base: "Bản sửa đổi gốc đã ghim", pending: "Chưa xác định được kết quả đã gửi. Đăng nhập đúng actor ban đầu rồi thử lại chính operation đó; giữ operation ID để khôi phục.", retry: "Thử lại đúng operation đã gửi", operation: "Operation ID",
            run_id: "ID context / đề xuất (UUID)", open: "Mở context hoặc đề xuất đã gửi", refresh: "Cập nhật trạng thái", new_run: "Chọn đề xuất khác / bỏ sửa trong trình duyệt", new_help: "Cập nhật trạng thái giữ phần đang sửa. Để mở đề xuất khác, hãy chủ động bỏ phần sửa đó. Thử lại chỉ xác định kết quả snapshot chấp nhận ban đầu; không sinh nội dung.", provenance: "Nguồn gốc đã ghim của context / đề xuất", checksum: "Checksum nguồn", extraction: "Phiên bản phân tích", input_digest: "Digest nội dung / export đầu vào", records: "Hồ sơ sinh nội dung / xác nhận quyền", cost: "Chi phí được khai báo · chưa xác minh", usage: "Token đầu vào / đầu ra khai báo · chưa xác minh", unknown: "Chưa biết · chưa được báo", original: "Nguồn gốc được giữ và phần phân tích", source_missing: "Mở context hoặc đề xuất để xem nguồn đã ghim. Đối chiếu mọi trang phân tích và cảnh báo trước khi chấp nhận.", original_binary: "Bản gốc là tài liệu nhị phân. Tải xuống và đối chiếu phần phân tích bên dưới.", download: "Tải bản gốc được giữ", proposal: "Đề xuất do host gửi", proposal_help: "Sửa cảnh, nhân vật và lời thoại bằng editor có cấu trúc. JSON nâng cao vẫn có thể mở. Khi chấp nhận, backend kiểm tra Script IR đầy đủ và nguồn gốc đã ghim. Độ bao phủ nguồn được gửi vào cần được xem lại; đó chưa phải bằng chứng giữ đủ mọi sự kiện.", edit: "Script IR đề xuất (JSON có thể sửa)", findings: "Các điểm cần xem lại", coverage: "Độ bao phủ nguồn được khai báo", coverage_help: "Số block khớp phần phân tích bên trên. Kiểm tra cả đoạn được chuyển thể và đoạn bỏ qua; toàn bộ hồ sơ bao phủ được hiển thị.", represented: "Đã thể hiện", omitted: "Bỏ qua", block: "Block nguồn", review: "Tôi đã đối chiếu nguồn và revision đầu vào đã ghim với đề xuất hiện tại và xem lại sự kiện chưa rõ, người nói, phần bỏ qua và gợi ý diễn xuất.", accept: "Chấp nhận đề xuất hiện tại thành bản sửa đổi mới", accept_help: "Thao tác rõ ràng này lưu bản sửa đổi bất biến mới. Nguồn và các bản trước được giữ nguyên; thao tác chưa duyệt sản xuất hay cấp quyền xuất bản.", accept_required: "Cần đề xuất gửi vào hợp lệ, nguồn/đầu vào đã ghim khớp và xác nhận đã xem văn bản hiện tại. Nếu bản gốc đã đổi, cần context mới; không tự đổi gốc.", accepted: "Bản sửa đổi bất biến đã chấp nhận · chỉ đọc", reopen: "Mở lại chính bản sửa đổi đã chấp nhận", problem: "Kết quả đã gửi chưa hợp lệ hoặc cần xem lại", no_findings: "Kết quả chưa ghi thêm điểm cần xem. Bạn vẫn phải đối chiếu toàn bộ nguồn và đề xuất.",
        }
    }
}

pub fn adaptation_status(status: &ReviewStatus, english: bool) -> &'static str {
    match (status, english) {
        (ReviewStatus::Idle, true) => {
            "Source and accepted revisions are preserved. Open a prepared context or submitted proposal."
        }
        (ReviewStatus::Idle, false) => {
            "Nguồn và bản đã chấp nhận được giữ nguyên. Mở context đã chuẩn bị hoặc đề xuất đã gửi."
        }
        (ReviewStatus::LoadingRun, true) => "Reading the saved run…",
        (ReviewStatus::LoadingRun, false) => "Đang đọc run đã lưu…",
        (ReviewStatus::RunOpened, true) => {
            "Saved run opened. Review its status, source and proposal."
        }
        (ReviewStatus::RunOpened, false) => "Đã mở run đã lưu. Xem trạng thái, nguồn và đề xuất.",
        (ReviewStatus::DraftChanged, true) => {
            "Your browser proposal text is preserved; review it before acceptance."
        }
        (ReviewStatus::DraftChanged, false) => {
            "Văn bản đề xuất trong trình duyệt được giữ; xem lại trước khi chấp nhận."
        }
        (ReviewStatus::Accepting, true) => {
            "Validating and accepting the submitted proposal snapshot…"
        }
        (ReviewStatus::Accepting, false) => "Đang kiểm tra và chấp nhận snapshot đề xuất đã gửi…",
        (ReviewStatus::Accepted, true) => {
            "A new immutable revision was accepted. Later browser edits are kept separately."
        }
        (ReviewStatus::Accepted, false) => {
            "Đã chấp nhận bản sửa đổi bất biến mới. Phần sửa trong trình duyệt sau đó được giữ riêng."
        }
        (ReviewStatus::LoadingAccepted, true) => "Reopening the exact accepted revision…",
        (ReviewStatus::LoadingAccepted, false) => "Đang mở lại đúng bản sửa đổi đã chấp nhận…",
        (ReviewStatus::AcceptedOpened, true) => {
            "The exact immutable accepted export reopened successfully."
        }
        (ReviewStatus::AcceptedOpened, false) => "Đã mở lại đúng export bất biến được chấp nhận.",
        (ReviewStatus::Error(code), _) => super::status(crate::editor::error_status(code), english),
    }
}

pub fn adaptation_run_status(status: AdaptationStatus, english: bool) -> &'static str {
    match (status, english) {
        (AdaptationStatus::AwaitingProposal, true) => {
            "Context prepared · awaiting a host-submitted proposal"
        }
        (AdaptationStatus::AwaitingProposal, false) => {
            "Đã chuẩn bị context · chờ đề xuất từ host gửi vào"
        }
        (AdaptationStatus::Queued, true) => "Queued · historical recorded status",
        (AdaptationStatus::Queued, false) => "Đã xếp hàng · trạng thái đã ghi trước đây",
        (AdaptationStatus::Running, true) => "Running · historical recorded status",
        (AdaptationStatus::Running, false) => "Đang xử lý · trạng thái đã ghi trước đây",
        (AdaptationStatus::Succeeded, true) => "Proposal ready for human review",
        (AdaptationStatus::Succeeded, false) => "Đề xuất sẵn sàng để người biên tập xem",
        (AdaptationStatus::InvalidOutput, true) => {
            "Invalid submitted output · no revision accepted"
        }
        (AdaptationStatus::InvalidOutput, false) => {
            "Đầu ra đã gửi không hợp lệ · chưa chấp nhận revision"
        }
        (AdaptationStatus::Failed, true) => "Failed · no revision accepted",
        (AdaptationStatus::Failed, false) => "Thất bại · chưa chấp nhận revision",
        (AdaptationStatus::Ambiguous, true) => {
            "Provider outcome unknown · do not silently generate again"
        }
        (AdaptationStatus::Ambiguous, false) => "Chưa rõ kết quả provider · không tự sinh lại",
        (AdaptationStatus::Cancelled, true) => "Cancelled · source and saved revisions preserved",
        (AdaptationStatus::Cancelled, false) => "Đã hủy · nguồn và revision đã lưu được giữ",
        (AdaptationStatus::Accepted, true) => "Accepted as an immutable revision",
        (AdaptationStatus::Accepted, false) => "Đã chấp nhận thành revision bất biến",
    }
}

pub fn adaptation_finding(code: AdaptationFindingCode, english: bool) -> &'static str {
    match (code, english) {
        (AdaptationFindingCode::SourceWarning, true) => "Source extraction warning",
        (AdaptationFindingCode::SourceWarning, false) => "Cảnh báo từ phần phân tích nguồn",
        (AdaptationFindingCode::UncoveredSource, true) => "Source block has no coverage record",
        (AdaptationFindingCode::UncoveredSource, false) => "Block nguồn chưa có hồ sơ bao phủ",
        (AdaptationFindingCode::OmittedSource, true) => "Source material omitted",
        (AdaptationFindingCode::OmittedSource, false) => "Nội dung nguồn bị bỏ qua",
        (AdaptationFindingCode::TextChanged, true) => "Text changed during adaptation",
        (AdaptationFindingCode::TextChanged, false) => "Văn bản đổi khi chuyển thể",
        (AdaptationFindingCode::UnresolvedSpeaker, true) => "Speaker unresolved",
        (AdaptationFindingCode::UnresolvedSpeaker, false) => "Chưa xác định người nói",
        (AdaptationFindingCode::NewSpeaker, true) => "New speaker proposed",
        (AdaptationFindingCode::NewSpeaker, false) => "Đề xuất người nói mới",
        (AdaptationFindingCode::PossibleNarrationConfusion, true) => {
            "Possible confusion between narration and dialogue"
        }
        (AdaptationFindingCode::PossibleNarrationConfusion, false) => {
            "Có thể nhầm lời dẫn với lời thoại"
        }
        (AdaptationFindingCode::CueLikeMarkup, true) => "Possible sound cue inside spoken text",
        (AdaptationFindingCode::CueLikeMarkup, false) => "Có thể có chỉ dẫn âm thanh trong lời nói",
        (AdaptationFindingCode::UnsupportedPerformanceControl, true) => {
            "Performance control requires explicit fallback or correction"
        }
        (AdaptationFindingCode::UnsupportedPerformanceControl, false) => {
            "Điều khiển diễn xuất cần chọn cách thay thế hoặc sửa rõ ràng"
        }
        (AdaptationFindingCode::ProviderReviewNote, true) => "Submitted review suggestion",
        (AdaptationFindingCode::ProviderReviewNote, false) => "Gợi ý xem lại được gửi vào",
        (AdaptationFindingCode::SourceCoverageRequiresReview, true) => {
            "Source coverage requires human comparison"
        }
        (AdaptationFindingCode::SourceCoverageRequiresReview, false) => {
            "Độ bao phủ nguồn cần được người biên tập đối chiếu"
        }
    }
}

pub fn adaptation_problem(code: &str, english: bool) -> &'static str {
    match (code, english) {
        ("provider_outcome_unknown", true) => "The provider outcome is unknown. The saved attempt will not be automatically generated again. Review the run and its operator recovery record.",
        ("provider_outcome_unknown", false) => "Chưa biết kết quả provider. Lần xử lý đã lưu sẽ không tự sinh lại. Xem run và hồ sơ khôi phục của operator.",
        ("provider_unavailable" | "provider_rejected", true) => "The provider could not fulfill this request. This is a preserved historical failure; Cantos does not start generation.",
        ("provider_unavailable" | "provider_rejected", false) => "Provider chưa xử lý được yêu cầu này. Đây là lỗi trước đây được giữ lại; Cantos không khởi chạy sinh nội dung.",
        ("provider_configuration_changed", true) => "The run's pinned provider configuration changed. An operator must review it; a different configuration cannot silently replace this attempt.",
        ("provider_configuration_changed", false) => "Cấu hình provider đã ghim của run đã đổi. Operator cần xem lại; cấu hình khác không tự thay thế lần xử lý này.",
        ("output_too_large" | "provider_output_too_large" | "output_findings_too_large", true) => "Submitted output exceeded the bounded size. No revision was accepted; the source and prior revisions remain available.",
        ("output_too_large" | "provider_output_too_large" | "output_findings_too_large", false) => "Đầu ra đã gửi vượt giới hạn dung lượng. Chưa chấp nhận revision; nguồn và các bản trước vẫn còn.",
        ("invalid_output" | "invalid_script_output" | "provider_malformed_response", true) => "Submitted output failed the structured or semantic contract. No revision was accepted. Inspect the findings and saved run.",
        ("invalid_output" | "invalid_script_output" | "provider_malformed_response", false) => "Đầu ra đã gửi chưa đạt hợp đồng cấu trúc hoặc ngữ nghĩa. Chưa chấp nhận revision. Xem các lỗi và run đã lưu.",
        (_, true) => "This run needs operator review. Its saved source and accepted revisions remain intact; use the displayed findings for recovery.",
        (_, false) => "Run này cần operator xem lại. Nguồn và revision đã chấp nhận vẫn được giữ; dùng các lỗi hiển thị để khôi phục.",
    }
}

pub fn adaptation_finding_explanation(code: AdaptationFindingCode, english: bool) -> &'static str {
    match (code, english) {
        (AdaptationFindingCode::SourceWarning, true) => "Review the original extraction warning before accepting this proposal.",
        (AdaptationFindingCode::SourceWarning, false) => "Xem cảnh báo phân tích nguồn ban đầu trước khi chấp nhận đề xuất này.",
        (AdaptationFindingCode::UncoveredSource, true) => "No proposed line or cue cites this source block. Check whether its content was lost.",
        (AdaptationFindingCode::UncoveredSource, false) => "Chưa có lời thoại hoặc âm thanh đề xuất dẫn tới block nguồn này. Kiểm tra xem nội dung có bị mất không.",
        (AdaptationFindingCode::OmittedSource, true) => "The submitted proposal omitted this source block. Review the stated reason against the source.",
        (AdaptationFindingCode::OmittedSource, false) => "Đề xuất đã gửi bỏ block nguồn này. Đối chiếu lý do đã nêu với bản gốc.",
        (AdaptationFindingCode::TextChanged, true) => "Adapted spoken text differs from its source block. Compare meaning, facts and omissions.",
        (AdaptationFindingCode::TextChanged, false) => "Lời nói được chuyển thể khác block nguồn. Đối chiếu ý nghĩa, sự kiện và phần bỏ qua.",
        (AdaptationFindingCode::UnresolvedSpeaker, true) => "The speaker is unknown. A placeholder does not resolve attribution; confirm or correct it.",
        (AdaptationFindingCode::UnresolvedSpeaker, false) => "Người nói chưa rõ. Tên tạm chưa xác định được người nói; cần xác nhận hoặc sửa.",
        (AdaptationFindingCode::NewSpeaker, true) => "This speaker name was not recorded in the source extraction. Confirm its attribution.",
        (AdaptationFindingCode::NewSpeaker, false) => "Tên người nói này chưa được ghi trong phần phân tích nguồn. Cần xác nhận cách gán lời.",
        (AdaptationFindingCode::PossibleNarrationConfusion, true) => "The source text type and proposed speaker role differ. Check narration and dialogue separately.",
        (AdaptationFindingCode::PossibleNarrationConfusion, false) => "Loại văn bản nguồn và vai người nói đề xuất khác nhau. Kiểm tra riêng lời dẫn và lời thoại.",
        (AdaptationFindingCode::CueLikeMarkup, true) => "Spoken text contains markup-like characters. Decide whether they should be spoken or moved to a separate cue.",
        (AdaptationFindingCode::CueLikeMarkup, false) => "Lời nói chứa ký tự giống chỉ dẫn. Xác định chúng cần được đọc hay chuyển thành âm thanh riêng.",
        (AdaptationFindingCode::UnsupportedPerformanceControl, true) => "Prosody or pacing is a review note, not an implemented playback control. Choose an explicit fallback or correction.",
        (AdaptationFindingCode::UnsupportedPerformanceControl, false) => "Ngữ điệu hoặc nhịp diễn là ghi chú cần xem, chưa phải điều khiển phát đã triển khai. Chọn cách thay thế hoặc sửa rõ ràng.",
        (AdaptationFindingCode::ProviderReviewNote, true) => "Treat this submitted note as an untrusted suggestion and compare it with the source.",
        (AdaptationFindingCode::ProviderReviewNote, false) => "Xem ghi chú được gửi như gợi ý chưa được xác nhận và đối chiếu với nguồn.",
        (AdaptationFindingCode::SourceCoverageRequiresReview, true) => "Citations show the submitted coverage claim. A person must compare facts, chronology and attribution against the complete source.",
        (AdaptationFindingCode::SourceCoverageRequiresReview, false) => "Trích dẫn thể hiện độ bao phủ được người gửi khai báo. Người biên tập cần đối chiếu sự kiện, thứ tự và cách gán lời với toàn bộ nguồn.",
    }
}

pub fn adaptation_has_provider_detail(code: AdaptationFindingCode) -> bool {
    match code {
        AdaptationFindingCode::NewSpeaker
        | AdaptationFindingCode::OmittedSource
        | AdaptationFindingCode::UnsupportedPerformanceControl
        | AdaptationFindingCode::ProviderReviewNote => true,
        AdaptationFindingCode::SourceWarning
        | AdaptationFindingCode::UncoveredSource
        | AdaptationFindingCode::TextChanged
        | AdaptationFindingCode::UnresolvedSpeaker
        | AdaptationFindingCode::PossibleNarrationConfusion
        | AdaptationFindingCode::CueLikeMarkup
        | AdaptationFindingCode::SourceCoverageRequiresReview => false,
    }
}

pub fn adaptation_provider_detail(english: bool) -> &'static str {
    if english {
        "Host-submitted content · unverified, requires review"
    } else {
        "Nội dung host đã gửi · chưa xác minh, cần xem lại"
    }
}

pub fn adaptation_model_evidence_label(english: bool) -> &'static str {
    if english {
        "Historical model evidence fingerprint"
    } else {
        "Dấu vân tay bằng chứng model đã ghi trước đây"
    }
}

pub fn adaptation_model_evidence(recorded: bool, english: bool) -> &'static str {
    match (recorded, english) {
        (true, true) => "Recorded model evidence from a historical run. This configuration evidence does not prove generation succeeded.",
        (true, false) => "Bằng chứng model được ghi từ run trước đây. Bằng chứng cấu hình này chưa chứng minh sinh nội dung thành công.",
        (false, true) => "No historical model evidence fingerprint recorded. A fixture is not evidence of a real model run.",
        (false, false) => "Chưa ghi dấu vân tay bằng chứng model trước đây. Fixture chưa phải bằng chứng một lần chạy model thật.",
    }
}

pub fn adaptation_cost(cost: Option<&AdaptationCost>, english: bool) -> String {
    match cost {
        None => adaptation_copy(english).unknown.into(),
        Some(cost) => match cost.currency {
            AdaptationCurrency::USD => format!(
                "USD {}.{:02}",
                cost.amount_minor / 100,
                cost.amount_minor % 100
            ),
            AdaptationCurrency::VND => format!("VND {}", cost.amount_minor),
        },
    }
}

#[derive(Clone, Copy)]
pub struct HostCopy {
    pub context_digest: &'static str,
    pub context_version: &'static str,
    pub rights_claim: &'static str,
    pub authorized: &'static str,
    pub absent: &'static str,
    pub submission: &'static str,
    pub caller_warning: &'static str,
    pub host_tool: &'static str,
    pub recorded_prompt: &'static str,
    pub submitted_by: &'static str,
    pub submitted_at: &'static str,
    pub output_checksum: &'static str,
    pub legacy: &'static str,
}

pub fn host_copy(english: bool) -> HostCopy {
    if english {
        HostCopy {
            context_digest: "Pinned context digest",
            context_version: "Context version",
            rights_claim: "Recorded source export permission claim",
            authorized: "Claim recorded · legal eligibility remains unverified",
            absent: "No authorization claim recorded",
            submission: "Latest submitted result",
            caller_warning: "Tool, provider, model, settings, usage and cost below are caller declarations. Cantos did not observe generation or verify billing. An absent value means unknown.",
            host_tool: "Declared host / tool · unverified",
            recorded_prompt: "Declared prompt version · unverified",
            submitted_by: "Result submitted by",
            submitted_at: "Result received at",
            output_checksum: "Submitted output checksum",
            legacy: "Preserved historical proposal. These recorded generation details do not configure or prove a current integration. Cantos does not start generation.",
        }
    } else {
        HostCopy {
            context_digest: "Digest context đã ghim",
            context_version: "Phiên bản context",
            rights_claim: "Xác nhận quyền xuất nguồn đã ghi",
            authorized: "Đã ghi xác nhận · chưa xác minh đủ quyền pháp lý",
            absent: "Chưa ghi xác nhận cho phép",
            submission: "Kết quả gửi vào gần nhất",
            caller_warning: "Host/tool, provider, model, thiết lập, token và chi phí bên dưới do người gửi khai báo. Cantos chưa quan sát lần sinh hay xác minh hóa đơn. Giá trị vắng mặt nghĩa là chưa biết.",
            host_tool: "Host / tool khai báo · chưa xác minh",
            recorded_prompt: "Phiên bản prompt khai báo · chưa xác minh",
            submitted_by: "Người gửi kết quả",
            submitted_at: "Thời điểm nhận kết quả",
            output_checksum: "Checksum đầu ra đã gửi",
            legacy: "Đề xuất trước đây được giữ lại. Thông tin sinh nội dung đã ghi chưa cấu hình hay chứng minh tích hợp hiện tại. Cantos không khởi chạy sinh nội dung.",
        }
    }
}

pub fn adaptation_input_revision(english: bool) -> &'static str {
    if english {
        "Pinned input revision · read only"
    } else {
        "Bản sửa đổi đầu vào đã ghim · chỉ đọc"
    }
}

pub fn adaptation_submission_status(
    status: AdaptationSubmissionStatus,
    english: bool,
) -> &'static str {
    match (status, english) {
        (AdaptationSubmissionStatus::Valid, true) => {
            "Submission admitted · explicit human review still required"
        }
        (AdaptationSubmissionStatus::Valid, false) => {
            "Kết quả gửi vào hợp lệ · vẫn cần người biên tập xem và xác nhận"
        }
        (AdaptationSubmissionStatus::Invalid, true) => {
            "Submission rejected · source and accepted revisions preserved"
        }
        (AdaptationSubmissionStatus::Invalid, false) => {
            "Kết quả gửi vào bị từ chối · nguồn và bản đã chấp nhận được giữ"
        }
    }
}

pub fn adaptation_usage(usage: Option<&AdaptationUsage>, english: bool) -> String {
    let unknown = adaptation_copy(english).unknown;
    format!(
        "{} / {}",
        usage
            .and_then(|usage| usage.input_tokens)
            .map(|value| value.to_string())
            .unwrap_or_else(|| unknown.into()),
        usage
            .and_then(|usage| usage.output_tokens)
            .map(|value| value.to_string())
            .unwrap_or_else(|| unknown.into())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_run_and_review_finding_has_both_locales_and_unknown_cost_is_not_zero() {
        for english in [false, true] {
            for status in [
                AdaptationStatus::AwaitingProposal,
                AdaptationStatus::Queued,
                AdaptationStatus::Running,
                AdaptationStatus::Succeeded,
                AdaptationStatus::InvalidOutput,
                AdaptationStatus::Failed,
                AdaptationStatus::Ambiguous,
                AdaptationStatus::Cancelled,
                AdaptationStatus::Accepted,
            ] {
                assert!(!adaptation_run_status(status, english).is_empty());
            }
            for finding in [
                AdaptationFindingCode::SourceWarning,
                AdaptationFindingCode::UncoveredSource,
                AdaptationFindingCode::OmittedSource,
                AdaptationFindingCode::TextChanged,
                AdaptationFindingCode::UnresolvedSpeaker,
                AdaptationFindingCode::NewSpeaker,
                AdaptationFindingCode::PossibleNarrationConfusion,
                AdaptationFindingCode::CueLikeMarkup,
                AdaptationFindingCode::UnsupportedPerformanceControl,
                AdaptationFindingCode::ProviderReviewNote,
                AdaptationFindingCode::SourceCoverageRequiresReview,
            ] {
                assert!(!adaptation_finding(finding, english).is_empty());
                assert!(!adaptation_finding_explanation(finding, english).is_empty());
            }
            assert!(!adaptation_copy(english).unknown.contains('0'));
            assert!(!host_copy(english).caller_warning.is_empty());
            for status in [
                AdaptationSubmissionStatus::Valid,
                AdaptationSubmissionStatus::Invalid,
            ] {
                assert!(!adaptation_submission_status(status, english).is_empty());
            }
        }
    }

    #[test]
    fn deterministic_findings_use_localized_explanations_and_only_content_notes_are_quoted() {
        for code in [
            AdaptationFindingCode::UncoveredSource,
            AdaptationFindingCode::TextChanged,
            AdaptationFindingCode::UnresolvedSpeaker,
            AdaptationFindingCode::PossibleNarrationConfusion,
            AdaptationFindingCode::CueLikeMarkup,
            AdaptationFindingCode::SourceCoverageRequiresReview,
        ] {
            assert!(!adaptation_has_provider_detail(code));
            assert_ne!(
                adaptation_finding_explanation(code, false),
                adaptation_finding_explanation(code, true)
            );
        }
        for code in [
            AdaptationFindingCode::NewSpeaker,
            AdaptationFindingCode::OmittedSource,
            AdaptationFindingCode::UnsupportedPerformanceControl,
            AdaptationFindingCode::ProviderReviewNote,
        ] {
            assert!(adaptation_has_provider_detail(code));
        }
        assert!(!adaptation_has_provider_detail(
            AdaptationFindingCode::SourceWarning
        ));
    }

    #[test]
    fn cost_uses_recorded_currency_minor_units_and_missing_cost_stays_unknown() {
        use cantos_contracts::AdaptationCostBasis;
        let usd = AdaptationCost {
            currency: AdaptationCurrency::USD,
            amount_minor: 123,
            basis: AdaptationCostBasis::CallerDeclared,
        };
        let vnd = AdaptationCost {
            currency: AdaptationCurrency::VND,
            amount_minor: 123,
            basis: AdaptationCostBasis::CallerDeclared,
        };
        assert_eq!(adaptation_cost(Some(&usd), true), "USD 1.23");
        assert_eq!(adaptation_cost(Some(&vnd), false), "VND 123");
        for english in [false, true] {
            assert_eq!(
                adaptation_cost(None, english),
                adaptation_copy(english).unknown
            );
        }
    }

    #[test]
    fn historical_model_evidence_copy_distinguishes_configuration_from_generation_and_fixture() {
        assert!(
            adaptation_model_evidence(true, true).contains("does not prove generation succeeded")
        );
        assert!(adaptation_model_evidence(true, false)
            .contains("chưa chứng minh sinh nội dung thành công"));
        assert!(adaptation_model_evidence(false, true)
            .contains("No historical model evidence fingerprint recorded"));
        assert!(adaptation_model_evidence(false, false).contains("Fixture chưa phải bằng chứng"));
        assert_ne!(
            adaptation_model_evidence_label(true),
            adaptation_model_evidence_label(false)
        );
    }

    #[test]
    fn host_workflow_copy_discloses_unverified_metadata_and_preserves_unknown_usage() {
        assert!(adaptation_copy(true)
            .help
            .contains("does not generate content or call AI"));
        assert!(adaptation_copy(false)
            .help
            .contains("không tự sinh nội dung hay gọi AI"));
        assert!(host_copy(true)
            .caller_warning
            .contains("caller declarations"));
        assert!(host_copy(false)
            .caller_warning
            .contains("người gửi khai báo"));
        for english in [false, true] {
            let unknown = adaptation_copy(english).unknown;
            assert_eq!(
                adaptation_usage(None, english),
                format!("{unknown} / {unknown}")
            );
            let partial = AdaptationUsage {
                input_tokens: Some(0),
                output_tokens: None,
            };
            assert_eq!(
                adaptation_usage(Some(&partial), english),
                format!("0 / {unknown}")
            );
        }
    }
}
