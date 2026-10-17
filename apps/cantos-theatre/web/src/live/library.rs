//! Actual account-owned read adapter, independent of the isolated mock catalog/player.
use crate::application::account_view::{AccountFailure, LibraryProjection, ProgressSummary};
use cantos_contracts::listener_library::{ContinuePage, HistoryPage, SavedPage};

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

pub fn decode_projection(
    continuing: &[u8],
    saved: &[u8],
    history: &[u8],
) -> Result<LibraryProjection, AccountFailure> {
    let continuing: ContinuePage =
        serde_json::from_slice(continuing).map_err(|_| AccountFailure::Protocol)?;
    let saved: SavedPage = serde_json::from_slice(saved).map_err(|_| AccountFailure::Protocol)?;
    let history: HistoryPage =
        serde_json::from_slice(history).map_err(|_| AccountFailure::Protocol)?;
    if continuing.items.len() > 20 || saved.items.len() > 20 || history.items.len() > 20 {
        return Err(AccountFailure::Protocol);
    }
    let valid_work = |work: &str| uuid::Uuid::parse_str(work).is_ok();
    if continuing.items.iter().any(|item| {
        !valid_work(&item.progress.audio.work_id)
            || item.progress.audio.duration_ms == 0
            || item.progress.position_ms > item.progress.audio.duration_ms
    }) || saved
        .items
        .iter()
        .any(|item| !valid_work(&item.saved.work_id) || !item.saved.saved)
        || history.items.iter().any(|item| {
            !valid_work(&item.audio.work_id)
                || item.audio.duration_ms == 0
                || item.progress.as_ref().is_some_and(|progress| {
                    progress.audio != item.audio
                        || progress.position_ms > progress.audio.duration_ms
                })
        })
    {
        return Err(AccountFailure::Protocol);
    }
    let has_more = continuing.next_cursor.is_some()
        || saved.next_cursor.is_some()
        || history.next_cursor.is_some();
    Ok(LibraryProjection {
        continuing: continuing
            .items
            .into_iter()
            .map(|item| ProgressSummary {
                work: item.progress.audio.work_id,
                position_ms: item.progress.position_ms,
                duration_ms: item.progress.audio.duration_ms,
                resume: item.resume,
            })
            .collect(),
        saved: saved
            .items
            .into_iter()
            .map(|item| item.saved.work_id)
            .collect(),
        history: history
            .items
            .into_iter()
            .map(|item| item.audio.work_id)
            .collect(),
        has_more,
    })
}

#[cfg(target_arch = "wasm32")]
pub async fn load(expected_session: &str) -> Result<LibraryProjection, AccountFailure> {
    use gloo_net::http::Request;
    use web_sys::{AbortController, RequestCredentials};

    let controller = AbortController::new().map_err(|_| AccountFailure::Unavailable)?;
    let deadline = super::browser::deadline(controller).map_err(|_| AccountFailure::Unavailable)?;
    let signal = deadline.controller.signal();
    let mut bodies = Vec::new();
    // One bounded read batch; no writes, receipt replay, history event or playback effect.
    for path in [
        "/api/v1/theatre/library/continue?limit=20",
        "/api/v1/theatre/library/saved?limit=20",
        "/api/v1/theatre/library/history?limit=20",
    ] {
        let response = Request::get(
            &crate::api_endpoint::endpoint(path).map_err(|_| AccountFailure::Unavailable)?,
        )
        .credentials(RequestCredentials::Include)
        .abort_signal(Some(&signal))
        .header("Accept", "application/json")
        .header("X-Cantos-Theatre-Session", expected_session)
        .send()
        .await
        .map_err(|_| AccountFailure::Unavailable)?;
        check_response(
            response.status(),
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
        if body.len() > 65_536 {
            return Err(AccountFailure::Protocol);
        }
        bodies.push(body);
    }
    decode_projection(&bodies[0], &bodies[1], &bodies[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    const CONTINUE: &[u8] = include_bytes!(
        "../../../../../packages/contracts/fixtures/theatre/library/v1/continue.json"
    );
    const SAVED: &[u8] =
        include_bytes!("../../../../../packages/contracts/fixtures/theatre/library/v1/saved.json");
    const HISTORY: &[u8] = include_bytes!(
        "../../../../../packages/contracts/fixtures/theatre/library/v1/history.json"
    );

    #[test]
    fn actual_library_fixtures_project_progress_saved_and_history_without_starting_playback() {
        let projection = decode_projection(CONTINUE, SAVED, HISTORY).unwrap();
        assert_eq!(projection.saved.len(), 1);
        assert_eq!(projection.history, projection.saved);
        assert_eq!(projection.continuing[0].position_ms, 15_000);
        assert_eq!(projection.continuing[0].duration_ms, 60_000);
        assert!(!projection.has_more);
    }

    #[test]
    fn malformed_or_impossible_private_response_is_failure_never_an_empty_library() {
        assert_eq!(
            decode_projection(b"{}", SAVED, HISTORY),
            Err(AccountFailure::Protocol)
        );
        let mut bad: serde_json::Value = serde_json::from_slice(CONTINUE).unwrap();
        bad["items"][0]["progress"]["position_ms"] = serde_json::json!(60_001);
        assert_eq!(
            decode_projection(&serde_json::to_vec(&bad).unwrap(), SAVED, HISTORY),
            Err(AccountFailure::Protocol)
        );
    }

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
