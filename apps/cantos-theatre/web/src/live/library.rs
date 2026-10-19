//! Bounded presented Library reads; no metadata fan-out or mutation effects.
use crate::application::{
    account_view::{AccountFailure, LibraryRequest},
    library::{LibraryPage, Section},
};
use cantos_contracts::listener_library::{
    self as dto, AudioIdentity, AudioPresentation, ProgressRecord, ResumeStatus, WorkPresentation,
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
    if !content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    }) {
        return Err(AccountFailure::Protocol);
    }
    Ok(())
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
        && value.duration_ms <= i64::MAX as u64
        && value.sha256.len() == 64
        && value
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn progress(value: &ProgressRecord) -> bool {
    audio(&value.audio)
        && value.position_ms <= value.audio.duration_ms
        && value.revision > 0
        && value.revision <= i64::MAX as u64
        && value.listener_order > 0
        && uuid(&value.written_by)
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
                !uuid(&i.saved.work_id)
                    || !i.saved.saved
                    || i.saved.revision == 0
                    || i.saved.revision > i64::MAX as u64
                    || i.saved.listener_order == 0
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
            ) || page.state.revision > i64::MAX as u64
                || page.state.epoch > i64::MAX as u64
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
