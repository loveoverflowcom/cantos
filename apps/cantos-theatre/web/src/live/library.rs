//! Bounded presented Library reads, point reads, and authenticated listener mutations.
#[cfg(target_arch = "wasm32")]
use crate::application::library_sync::MutationPayload;
use crate::application::{
    account_view::{AccountFailure, LibraryRequest},
    library::{LibraryPage, Section},
    library_sync::CommandError,
};
use cantos_contracts::listener_library::{
    self as dto, AudioIdentity, AudioPresentation, HistoryState, MutationState, ProgressKind,
    ProgressRecord, Receipt, ResumeStatus, SavedRecord, WorkPresentation, WriteOutcome,
};

pub fn check_response(
    status: u16,
    content_type: Option<&str>,
    response_session: Option<&str>,
    expected_session: &str,
) -> Result<(), AccountFailure> {
    match status {
        401 => return Err(AccountFailure::SignedOut),
        200 | 409 => (),
        _ => return Err(AccountFailure::Unavailable),
    }
    let actual = response_session.ok_or(AccountFailure::Protocol)?;
    if actual != expected_session {
        return Err(AccountFailure::SessionChanged);
    }
    if status != 200 {
        return Err(AccountFailure::Unavailable);
    }
    if !is_json(content_type) {
        return Err(AccountFailure::Protocol);
    }
    Ok(())
}

fn is_json(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    })
}

fn uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok_and(|id| id.to_string() == value)
}
fn audio(value: &AudioIdentity) -> bool {
    [
        &value.work_id,
        &value.unit_id,
        &value.publication_id,
        &value.rendition_id,
    ]
    .into_iter()
    .all(|id| uuid(id))
        && value.duration_ms > 0
        && value.duration_ms <= crate::core::public_track::MAX_DURATION_MS
        && value.sha256.len() == 64
        && value
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn progress(value: &ProgressRecord) -> bool {
    audio(&value.audio)
        && value.position_ms <= value.audio.duration_ms
        && value.accepted_at_ms >= 0
        && value.revision > 0
        && value.revision <= i64::MAX as u64
        && value.listener_order > 0
        && value.listener_order <= i64::MAX as u64
        && uuid(&value.written_by)
        && (value.kind != ProgressKind::Restart
            || (value.completion != dto::Completion::Completed && value.position_ms == 0))
        && (value.kind != ProgressKind::MarkCompleted
            || (value.position_ms == value.audio.duration_ms
                && value.completion == dto::Completion::Completed))
}
fn saved_record_valid(value: &SavedRecord, expected_work: Option<&str>) -> bool {
    uuid(&value.work_id)
        && expected_work.is_none_or(|w| value.work_id == w)
        && value.revision <= i64::MAX as u64
        && value.listener_order <= i64::MAX as u64
        && value.accepted_at_ms >= 0
        && (value.revision > 0
            || (!value.saved && value.listener_order == 0 && value.accepted_at_ms == 0))
        && (value.revision == 0 || value.listener_order > 0)
}
fn history_state_valid(state: &HistoryState) -> bool {
    state.revision <= i64::MAX as u64
        && state.epoch <= i64::MAX as u64
        && state.epoch <= state.revision
}
fn work(value: &WorkPresentation, expected: &str) -> bool {
    value.work_id == expected
        && uuid(&value.publication_id)
        && !value.title.trim().is_empty()
        && !value.language.trim().is_empty()
}
fn presentation(
    value: &Option<AudioPresentation>,
    identity: &AudioIdentity,
    resume: ResumeStatus,
) -> bool {
    value.as_ref().is_none_or(|value| {
        resume == ResumeStatus::ExactAvailable
            && work(&value.work, &identity.work_id)
            && value.work.publication_id == identity.publication_id
            && value.unit_id == identity.unit_id
            && !value.unit_title.trim().is_empty()
    })
}
fn page_valid<'a>(works: impl Iterator<Item = &'a str>, cursor: Option<&str>) -> bool {
    let mut unique = std::collections::BTreeSet::new();
    let valid = works.into_iter().all(|id| unique.insert(id));
    valid
        && unique.len() <= 20
        && cursor.is_none_or(|c| {
            c.len() == 64
                && c.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
}
pub fn decode_page(section: Section, body: &[u8]) -> Result<LibraryPage, AccountFailure> {
    if body.len() > 65_536 {
        return Err(AccountFailure::Protocol);
    }
    let page = match section {
        Section::Continue => {
            let page: dto::PresentedContinuePage =
                serde_json::from_slice(body).map_err(|_| AccountFailure::Protocol)?;
            if !page_valid(
                page.items.iter().map(|i| i.progress.audio.work_id.as_str()),
                page.next_cursor.as_deref(),
            ) || page.items.iter().any(|i| {
                !progress(&i.progress)
                    || i.progress.completion != dto::Completion::InProgress
                    || !presentation(&i.presentation, &i.progress.audio, i.resume)
            }) {
                return Err(AccountFailure::Protocol);
            }
            LibraryPage::Continue(page)
        }
        Section::Saved => {
            let page: dto::PresentedSavedPage =
                serde_json::from_slice(body).map_err(|_| AccountFailure::Protocol)?;
            if !page_valid(
                page.items.iter().map(|i| i.saved.work_id.as_str()),
                page.next_cursor.as_deref(),
            ) || page.items.iter().any(|i| {
                !saved_record_valid(&i.saved, None)
                    || !i.saved.saved
                    || i.saved.revision == 0
                    || i.presentation
                        .as_ref()
                        .is_some_and(|p| !work(p, &i.saved.work_id))
            }) {
                return Err(AccountFailure::Protocol);
            }
            LibraryPage::Saved(page)
        }
        Section::History => {
            let page: dto::PresentedHistoryPage =
                serde_json::from_slice(body).map_err(|_| AccountFailure::Protocol)?;
            if !page_valid(
                page.items.iter().map(|i| i.audio.work_id.as_str()),
                page.next_cursor.as_deref(),
            ) || !history_state_valid(&page.state)
                || page.items.iter().any(|i| {
                    !audio(&i.audio)
                        || i.listener_order == 0
                        || i.progress
                            .as_ref()
                            .is_some_and(|p| !progress(p) || p.audio != i.audio)
                        || !presentation(&i.presentation, &i.audio, i.resume)
                })
            {
                return Err(AccountFailure::Protocol);
            }
            LibraryPage::History(page)
        }
    };
    Ok(page)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteHttpOutcome {
    Receipt(Box<Receipt>),
    UnknownOutcome,
    Error(CommandError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationLookupOutcome {
    Found(Box<Receipt>),
    NotFound,
    UnknownOutcome,
    Error(CommandError),
}

fn check_session_header(
    status: u16,
    response_session: Option<&str>,
    expected_session: &str,
) -> Result<(), CommandError> {
    if status == 401 || status == 403 {
        return Err(CommandError::SignedOut);
    }
    if let Some(actual) = response_session {
        if actual != expected_session {
            return Err(CommandError::SessionChanged);
        }
        Ok(())
    } else if status >= 500 {
        Err(CommandError::Unavailable)
    } else {
        Err(CommandError::Protocol)
    }
}

fn classify_error_code(code: dto::ErrorCode) -> CommandError {
    match code {
        dto::ErrorCode::Unauthenticated => CommandError::SignedOut,
        dto::ErrorCode::NotFound => CommandError::NotFound,
        dto::ErrorCode::StaleDeviceSequence => CommandError::StaleDeviceSequence,
        dto::ErrorCode::IdempotencyKeyReused => CommandError::IdempotencyKeyReused,
        dto::ErrorCode::RevisionExhausted => CommandError::RevisionExhausted,
        dto::ErrorCode::InvalidPosition => CommandError::InvalidPosition,
        dto::ErrorCode::Unavailable => CommandError::Unavailable,
        dto::ErrorCode::InvalidRequest
        | dto::ErrorCode::InvalidCursor
        | dto::ErrorCode::CorruptState => CommandError::Protocol,
    }
}

fn receipt_valid(receipt: &Receipt, expected_operation: &str, expected_status: u16) -> bool {
    if receipt.operation_id != expected_operation || !uuid(&receipt.operation_id) {
        return false;
    }
    let is_conflict = matches!(
        receipt.outcome,
        WriteOutcome::RevisionConflict | WriteOutcome::HistoryEpochConflict
    );
    if is_conflict != receipt.resolution.is_some() {
        return false;
    }
    if expected_status != 0 {
        let expected_http = if is_conflict { 409 } else { 200 };
        if expected_status != expected_http {
            return false;
        }
    }
    match &receipt.state {
        MutationState::Saved { current } => saved_record_valid(current, None),
        MutationState::Progress { current } => match current {
            Some(record) => progress(record),
            None => is_conflict,
        },
        MutationState::History { current } => history_state_valid(current),
    }
}

pub fn decode_saved_point(
    status: u16,
    content_type: Option<&str>,
    response_session: Option<&str>,
    expected_session: &str,
    expected_work: &str,
    body: &[u8],
) -> Result<SavedRecord, CommandError> {
    check_session_header(status, response_session, expected_session)?;
    if status >= 500 {
        return Err(CommandError::Unavailable);
    }
    if body.len() > 16_384 || !is_json(content_type) {
        return Err(CommandError::Protocol);
    }
    if status != 200 {
        let err: dto::LibraryError =
            serde_json::from_slice(body).map_err(|_| CommandError::Protocol)?;
        return Err(classify_error_code(err.code));
    }
    let record: SavedRecord = serde_json::from_slice(body).map_err(|_| CommandError::Protocol)?;
    if !saved_record_valid(&record, Some(expected_work)) {
        return Err(CommandError::Protocol);
    }
    Ok(record)
}

pub fn decode_progress_point(
    status: u16,
    content_type: Option<&str>,
    response_session: Option<&str>,
    expected_session: &str,
    expected_publication: &str,
    body: &[u8],
) -> Result<Option<ProgressRecord>, CommandError> {
    check_session_header(status, response_session, expected_session)?;
    if status >= 500 {
        return Err(CommandError::Unavailable);
    }
    if body.len() > 16_384 || !is_json(content_type) {
        return Err(CommandError::Protocol);
    }
    if status != 200 {
        let err: dto::LibraryError =
            serde_json::from_slice(body).map_err(|_| CommandError::Protocol)?;
        return Err(classify_error_code(err.code));
    }
    let state: MutationState = serde_json::from_slice(body).map_err(|_| CommandError::Protocol)?;
    let MutationState::Progress { current } = state else {
        return Err(CommandError::Protocol);
    };
    if let Some(record) = &current {
        if !progress(record) || record.audio.publication_id != expected_publication {
            return Err(CommandError::Protocol);
        }
    }
    Ok(current)
}

pub fn decode_write_response(
    status: u16,
    content_type: Option<&str>,
    response_session: Option<&str>,
    expected_session: &str,
    expected_operation: &str,
    body: &[u8],
) -> WriteHttpOutcome {
    if let Err(err) = check_session_header(status, response_session, expected_session) {
        return match err {
            CommandError::Unavailable => WriteHttpOutcome::UnknownOutcome,
            other => WriteHttpOutcome::Error(other),
        };
    }
    if status >= 500 {
        return WriteHttpOutcome::UnknownOutcome;
    }
    if body.len() > 32_768 || !is_json(content_type) {
        return WriteHttpOutcome::Error(CommandError::Protocol);
    }
    match status {
        200 => match serde_json::from_slice::<Receipt>(body) {
            Ok(receipt) if receipt_valid(&receipt, expected_operation, 200) => {
                WriteHttpOutcome::Receipt(Box::new(receipt))
            }
            _ => WriteHttpOutcome::Error(CommandError::Protocol),
        },
        409 => {
            if let Ok(receipt) = serde_json::from_slice::<Receipt>(body) {
                if receipt_valid(&receipt, expected_operation, 409) {
                    return WriteHttpOutcome::Receipt(Box::new(receipt));
                }
                return WriteHttpOutcome::Error(CommandError::Protocol);
            }
            match serde_json::from_slice::<dto::LibraryError>(body) {
                Ok(err) => WriteHttpOutcome::Error(classify_error_code(err.code)),
                Err(_) => WriteHttpOutcome::Error(CommandError::Protocol),
            }
        }
        400 | 404 | 422 => match serde_json::from_slice::<dto::LibraryError>(body) {
            Ok(err) => WriteHttpOutcome::Error(classify_error_code(err.code)),
            Err(_) => WriteHttpOutcome::Error(CommandError::Protocol),
        },
        _ => WriteHttpOutcome::Error(CommandError::Protocol),
    }
}

pub fn decode_operation_lookup(
    status: u16,
    content_type: Option<&str>,
    response_session: Option<&str>,
    expected_session: &str,
    expected_operation: &str,
    body: &[u8],
) -> OperationLookupOutcome {
    if let Err(err) = check_session_header(status, response_session, expected_session) {
        return match err {
            CommandError::Unavailable => OperationLookupOutcome::UnknownOutcome,
            other => OperationLookupOutcome::Error(other),
        };
    }
    if status >= 500 {
        return OperationLookupOutcome::UnknownOutcome;
    }
    if body.len() > 32_768 || !is_json(content_type) {
        return OperationLookupOutcome::Error(CommandError::Protocol);
    }
    match status {
        200 => match serde_json::from_slice::<Receipt>(body) {
            Ok(receipt) if receipt_valid(&receipt, expected_operation, 0) => {
                OperationLookupOutcome::Found(Box::new(receipt))
            }
            _ => OperationLookupOutcome::Error(CommandError::Protocol),
        },
        404 => match serde_json::from_slice::<dto::LibraryError>(body) {
            Ok(dto::LibraryError {
                code: dto::ErrorCode::NotFound,
                ..
            }) => OperationLookupOutcome::NotFound,
            _ => OperationLookupOutcome::Error(CommandError::Protocol),
        },
        400 | 409 | 422 => match serde_json::from_slice::<dto::LibraryError>(body) {
            Ok(err) => OperationLookupOutcome::Error(classify_error_code(err.code)),
            Err(_) => OperationLookupOutcome::Error(CommandError::Protocol),
        },
        _ => OperationLookupOutcome::Error(CommandError::Protocol),
    }
}

pub fn request_path(request: &LibraryRequest) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    query
        .append_pair("presentation", "1")
        .append_pair("limit", "20");
    if let Some(cursor) = request.page.cursor() {
        query.append_pair("cursor", cursor);
    }
    format!(
        "/api/v1/theatre/library/{}?{}",
        request.page.section.name(),
        query.finish()
    )
}

pub fn saved_path(work_id: &str) -> String {
    format!("/api/v1/theatre/library/saved/{work_id}")
}

pub fn progress_path(publication_id: &str) -> String {
    format!("/api/v1/theatre/progress/{publication_id}")
}

pub fn operation_path(operation_id: &str) -> String {
    format!("/api/v1/theatre/operations/{operation_id}")
}

pub const HISTORY_START_PATH: &str = "/api/v1/theatre/library/history/playback-starts";
pub const HISTORY_CLEAR_PATH: &str = "/api/v1/theatre/library/history/clear";

#[cfg(target_arch = "wasm32")]
fn map_auth_to_command_error(error: cantos_web_auth::Failure) -> CommandError {
    match super::account::auth_error(error) {
        AccountFailure::SignedOut => CommandError::SignedOut,
        AccountFailure::SessionChanged => CommandError::SessionChanged,
        AccountFailure::Protocol => CommandError::Protocol,
        _ => CommandError::Unavailable,
    }
}

#[cfg(target_arch = "wasm32")]
async fn send_authenticated(
    method: &str,
    path: &str,
    expected_session: &str,
    json_body: Option<String>,
) -> Result<(u16, Option<String>, Option<String>, Vec<u8>), CommandError> {
    use gloo_net::http::RequestBuilder;
    use web_sys::{AbortController, RequestCredentials};

    let credential = crate::api_endpoint::auth()
        .map_err(map_auth_to_command_error)?
        .access(Some(expected_session))
        .await
        .map_err(map_auth_to_command_error)?;
    let controller = AbortController::new().map_err(|_| CommandError::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| CommandError::Unavailable)?;
    let url = crate::api_endpoint::endpoint(path).map_err(|_| CommandError::Unavailable)?;
    let http_method = match method {
        "PUT" => gloo_net::http::Method::PUT,
        "POST" => gloo_net::http::Method::POST,
        _ => gloo_net::http::Method::GET,
    };
    let mut builder = RequestBuilder::new(&url)
        .method(http_method)
        .credentials(RequestCredentials::Omit)
        .redirect(web_sys::RequestRedirect::Error)
        .header("Authorization", &credential.header())
        .header("Accept", "application/json")
        .header("X-Cantos-Theatre-Session", expected_session)
        .abort_signal(Some(&deadline.controller.signal()));
    let response = if let Some(body) = json_body {
        builder = builder.header("Content-Type", "application/json");
        builder
            .body(body)
            .map_err(|_| CommandError::Protocol)?
            .send()
            .await
            .map_err(|_| CommandError::Unavailable)?
    } else {
        builder
            .send()
            .await
            .map_err(|_| CommandError::Unavailable)?
    };
    credential
        .response(response.status())
        .map_err(map_auth_to_command_error)?;
    let status = response.status();
    let content_type = response.headers().get("content-type");
    let response_session = response.headers().get("X-Cantos-Theatre-Session");
    let bytes = response
        .binary()
        .await
        .map_err(|_| CommandError::Unavailable)?;
    credential.check().map_err(map_auth_to_command_error)?;
    Ok((status, content_type, response_session, bytes))
}

#[cfg(target_arch = "wasm32")]
pub async fn load_saved_point(
    expected_session: &str,
    work_id: &str,
) -> Result<SavedRecord, CommandError> {
    let (status, content_type, response_session, body) =
        send_authenticated("GET", &saved_path(work_id), expected_session, None).await?;
    decode_saved_point(
        status,
        content_type.as_deref(),
        response_session.as_deref(),
        expected_session,
        work_id,
        &body,
    )
}

#[cfg(target_arch = "wasm32")]
pub async fn load_progress_point(
    expected_session: &str,
    publication_id: &str,
) -> Result<Option<ProgressRecord>, CommandError> {
    let (status, content_type, response_session, body) = send_authenticated(
        "GET",
        &progress_path(publication_id),
        expected_session,
        None,
    )
    .await?;
    decode_progress_point(
        status,
        content_type.as_deref(),
        response_session.as_deref(),
        expected_session,
        publication_id,
        &body,
    )
}

#[cfg(target_arch = "wasm32")]
pub async fn load_operation_receipt(
    expected_session: &str,
    operation_id: &str,
) -> OperationLookupOutcome {
    match send_authenticated("GET", &operation_path(operation_id), expected_session, None).await {
        Ok((status, content_type, response_session, body)) => decode_operation_lookup(
            status,
            content_type.as_deref(),
            response_session.as_deref(),
            expected_session,
            operation_id,
            &body,
        ),
        Err(CommandError::Unavailable) => OperationLookupOutcome::UnknownOutcome,
        Err(err) => OperationLookupOutcome::Error(err),
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn send_mutation(expected_session: &str, payload: &MutationPayload) -> WriteHttpOutcome {
    let (method, path, body_res) = match payload {
        MutationPayload::Saved { work_id, write } => {
            ("PUT", saved_path(work_id), serde_json::to_string(write))
        }
        MutationPayload::Progress {
            publication_id,
            write,
        } => (
            "PUT",
            progress_path(publication_id),
            serde_json::to_string(write),
        ),
        MutationPayload::HistoryStart { write } => (
            "POST",
            HISTORY_START_PATH.to_owned(),
            serde_json::to_string(write),
        ),
        MutationPayload::HistoryClear { write } => (
            "POST",
            HISTORY_CLEAR_PATH.to_owned(),
            serde_json::to_string(write),
        ),
    };
    let Ok(json_body) = body_res else {
        return WriteHttpOutcome::Error(CommandError::Protocol);
    };
    let expected_op = payload.operation().operation_id.as_str();
    match send_authenticated(method, &path, expected_session, Some(json_body)).await {
        Ok((status, content_type, response_session, body)) => decode_write_response(
            status,
            content_type.as_deref(),
            response_session.as_deref(),
            expected_session,
            expected_op,
            &body,
        ),
        Err(CommandError::Unavailable) => WriteHttpOutcome::UnknownOutcome,
        Err(err) => WriteHttpOutcome::Error(err),
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn load(request: &LibraryRequest) -> Result<LibraryPage, AccountFailure> {
    use gloo_net::http::Request;
    use web_sys::{AbortController, RequestCredentials};
    let expected_session = request.session();
    let credential = crate::api_endpoint::auth()
        .map_err(super::account::auth_error)?
        .access(Some(expected_session))
        .await
        .map_err(super::account::auth_error)?;
    let controller = AbortController::new().map_err(|_| AccountFailure::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| AccountFailure::Unavailable)?;
    let response = Request::get(
        &crate::api_endpoint::endpoint(&request_path(request))
            .map_err(|_| AccountFailure::Unavailable)?,
    )
    .credentials(RequestCredentials::Omit)
    .redirect(web_sys::RequestRedirect::Error)
    .header("Authorization", &credential.header())
    .abort_signal(Some(&deadline.controller.signal()))
    .header("Accept", "application/json")
    .header("X-Cantos-Theatre-Session", expected_session)
    .send()
    .await
    .map_err(|_| AccountFailure::Unavailable)?;
    credential
        .response(response.status())
        .map_err(super::account::auth_error)?;
    let body_status = response.status();
    // Even error pages must echo this exact session before their body can affect this account.
    if body_status == 400 {
        check_response(
            200,
            response.headers().get("content-type").as_deref(),
            response
                .headers()
                .get("X-Cantos-Theatre-Session")
                .as_deref(),
            expected_session,
        )?;
        if response
            .headers()
            .get("X-Cantos-Theatre-Session")
            .as_deref()
            != Some(expected_session)
        {
            return Err(AccountFailure::SessionChanged);
        }
        let body = response
            .binary()
            .await
            .map_err(|_| AccountFailure::Unavailable)?;
        credential.check().map_err(super::account::auth_error)?;
        return match serde_json::from_slice::<dto::LibraryError>(&body) {
            Ok(dto::LibraryError {
                code: dto::ErrorCode::InvalidCursor,
                ..
            }) if body.len() <= 8192 => Err(AccountFailure::CursorExpired),
            _ => Err(AccountFailure::Protocol),
        };
    }
    check_response(
        body_status,
        response.headers().get("content-type").as_deref(),
        response
            .headers()
            .get("X-Cantos-Theatre-Session")
            .as_deref(),
        expected_session,
    )?;
    let body = response
        .binary()
        .await
        .map_err(|_| AccountFailure::Unavailable)?;
    credential.check().map_err(super::account::auth_error)?;
    decode_page(request.page.section, &body)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_cookie_response_cannot_be_displayed_under_the_previously_verified_session() {
        assert_eq!(
            check_response(
                200,
                Some("application/json"),
                Some("session-b"),
                "session-a"
            ),
            Err(AccountFailure::SessionChanged)
        );
        assert_eq!(
            check_response(
                409,
                Some("application/json"),
                Some("session-b"),
                "session-a"
            ),
            Err(AccountFailure::SessionChanged)
        );
        assert_eq!(
            check_response(200, Some("application/json"), None, "session-a"),
            Err(AccountFailure::Protocol)
        );
        assert_eq!(
            check_response(401, Some("application/json"), None, "session-a"),
            Err(AccountFailure::SignedOut)
        );
        assert_eq!(
            check_response(
                200,
                Some("application/json; charset=utf-8"),
                Some("session-a"),
                "session-a"
            ),
            Ok(())
        );
    }

    #[test]
    fn write_and_operation_decoders_distinguish_receipts_conflict_errors_and_unknown_outcomes() {
        let op = uuid::Uuid::from_u128(100).to_string();
        let work = uuid::Uuid::from_u128(10).to_string();
        let accepted = Receipt {
            operation_id: op.clone(),
            outcome: WriteOutcome::Accepted,
            resolution: None,
            state: MutationState::Saved {
                current: SavedRecord {
                    work_id: work.clone(),
                    saved: true,
                    revision: 1,
                    listener_order: 1,
                    accepted_at_ms: 1_000,
                },
            },
        };
        let conflict = Receipt {
            operation_id: op.clone(),
            outcome: WriteOutcome::RevisionConflict,
            resolution: Some(dto::Resolution::ListenerChooses),
            state: MutationState::Saved {
                current: SavedRecord {
                    work_id: work,
                    saved: false,
                    revision: 2,
                    listener_order: 2,
                    accepted_at_ms: 2_000,
                },
            },
        };

        assert_eq!(
            decode_write_response(
                200,
                Some("application/json"),
                Some("session-a"),
                "session-a",
                &op,
                &serde_json::to_vec(&accepted).unwrap(),
            ),
            WriteHttpOutcome::Receipt(Box::new(accepted.clone()))
        );
        // 409 with a conflict Receipt is decoded as Receipt, not CommandError.
        assert_eq!(
            decode_write_response(
                409,
                Some("application/json"),
                Some("session-a"),
                "session-a",
                &op,
                &serde_json::to_vec(&conflict).unwrap(),
            ),
            WriteHttpOutcome::Receipt(Box::new(conflict.clone()))
        );
        // GET /operations/{op} returns 200 even when the stored receipt was a 409 conflict.
        assert_eq!(
            decode_operation_lookup(
                200,
                Some("application/json"),
                Some("session-a"),
                "session-a",
                &op,
                &serde_json::to_vec(&conflict).unwrap(),
            ),
            OperationLookupOutcome::Found(Box::new(conflict))
        );
        // 409 stale_device_sequence is decoded as CommandError::StaleDeviceSequence.
        assert_eq!(
            decode_write_response(
                409,
                Some("application/json"),
                Some("session-a"),
                "session-a",
                &op,
                br#"{"code":"stale_device_sequence","retryable":false}"#,
            ),
            WriteHttpOutcome::Error(CommandError::StaleDeviceSequence)
        );
        // 422 idempotency_key_reused is decoded as CommandError::IdempotencyKeyReused.
        assert_eq!(
            decode_write_response(
                422,
                Some("application/json"),
                Some("session-a"),
                "session-a",
                &op,
                br#"{"code":"idempotency_key_reused","retryable":false}"#,
            ),
            WriteHttpOutcome::Error(CommandError::IdempotencyKeyReused)
        );
        // 503 is treated as UnknownOutcome so the operation is preserved for receipt recovery.
        assert_eq!(
            decode_write_response(
                503,
                Some("application/json"),
                Some("session-a"),
                "session-a",
                &op,
                br#"{"code":"unavailable","retryable":true}"#,
            ),
            WriteHttpOutcome::UnknownOutcome
        );
        // 404 on operation lookup means the server never committed that operation.
        assert_eq!(
            decode_operation_lookup(
                404,
                Some("application/json"),
                Some("session-a"),
                "session-a",
                &op,
                br#"{"code":"not_found","retryable":false}"#,
            ),
            OperationLookupOutcome::NotFound
        );
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    fn fixture(section: Section, unavailable: bool) -> &'static [u8] {
        match (section, unavailable) {
            (Section::Continue, false) => include_bytes!("../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_continue.json"),
            (Section::Continue, true) => include_bytes!("../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_continue_unavailable.json"),
            (Section::Saved, false) => include_bytes!("../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_saved.json"),
            (Section::Saved, true) => include_bytes!("../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_saved_unavailable.json"),
            (Section::History, false) => include_bytes!("../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_history.json"),
            (Section::History, true) => include_bytes!("../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_history_unavailable.json"),
        }
    }
    #[test]
    fn shared_presented_goldens_retain_every_audio_revision_epoch_and_nullable_member() {
        for section in Section::ALL {
            for unavailable in [false, true] {
                let body = fixture(section, unavailable);
                let page = decode_page(section, body).unwrap();
                let retained = match page {
                    LibraryPage::Continue(page) => serde_json::to_value(page).unwrap(),
                    LibraryPage::Saved(page) => serde_json::to_value(page).unwrap(),
                    LibraryPage::History(page) => serde_json::to_value(page).unwrap(),
                };
                assert_eq!(
                    retained,
                    serde_json::from_slice::<serde_json::Value>(body).unwrap()
                );
                let mut missing = retained;
                missing["items"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("presentation");
                assert_eq!(
                    decode_page(section, &serde_json::to_vec(&missing).unwrap()),
                    Err(AccountFailure::Protocol)
                );
            }
        }
    }
    #[test]
    fn wrong_work_unit_publication_and_impossible_progress_fail_instead_of_substituting_metadata() {
        let original: serde_json::Value =
            serde_json::from_slice(fixture(Section::Continue, false)).unwrap();
        for path in [
            "/items/0/presentation/work/work_id",
            "/items/0/presentation/work/publication_id",
            "/items/0/presentation/unit_id",
        ] {
            let mut body = original.clone();
            *body.pointer_mut(path).unwrap() =
                serde_json::json!(uuid::Uuid::from_u128(99).to_string());
            assert_eq!(
                decode_page(Section::Continue, &serde_json::to_vec(&body).unwrap()),
                Err(AccountFailure::Protocol)
            );
        }
        for (path, value) in [
            (
                "/items/0/progress/audio/sha256",
                serde_json::json!("not-a-sha"),
            ),
            (
                "/items/0/progress/audio/rendition_id",
                serde_json::json!("not-an-id"),
            ),
            ("/items/0/progress/position_ms", serde_json::json!(60001)),
            ("/items/0/progress/revision", serde_json::json!(0)),
            ("/items/0/resume", serde_json::json!("release_replaced")),
        ] {
            let mut body = original.clone();
            *body.pointer_mut(path).unwrap() = value;
            assert_eq!(
                decode_page(Section::Continue, &serde_json::to_vec(&body).unwrap()),
                Err(AccountFailure::Protocol)
            );
        }
        let mut body: serde_json::Value =
            serde_json::from_slice(fixture(Section::History, false)).unwrap();
        body["items"][0]["progress"]["audio"]["publication_id"] =
            serde_json::json!(uuid::Uuid::from_u128(99).to_string());
        assert_eq!(
            decode_page(Section::History, &serde_json::to_vec(&body).unwrap()),
            Err(AccountFailure::Protocol)
        );
    }
    #[test]
    fn null_metadata_replaced_audio_and_empty_pages_are_valid_but_oversized_and_duplicate_pages_are_not(
    ) {
        let mut value: serde_json::Value =
            serde_json::from_slice(fixture(Section::Continue, true)).unwrap();
        value["items"][0]["resume"] = serde_json::json!("release_replaced");
        assert!(matches!(
            decode_page(Section::Continue, &serde_json::to_vec(&value).unwrap()),
            Ok(LibraryPage::Continue(_))
        ));
        value["items"] = serde_json::json!([]);
        assert_eq!(
            decode_page(Section::Continue, &serde_json::to_vec(&value).unwrap()),
            Ok(LibraryPage::Continue(dto::PresentedContinuePage {
                items: vec![],
                next_cursor: None
            }))
        );
        value["items"] = serde_json::Value::Array(vec![
            serde_json::from_slice::<serde_json::Value>(
                fixture(Section::Continue, false)
            )
            .unwrap()["items"][0]
                .clone();
            2
        ]);
        assert_eq!(
            decode_page(Section::Continue, &serde_json::to_vec(&value).unwrap()),
            Err(AccountFailure::Protocol)
        );
        assert_eq!(
            decode_page(Section::Saved, &[b' '; 65537]),
            Err(AccountFailure::Protocol)
        );
    }
}
