//! One anonymous JSON discovery read. No identity cookie, credential, write or player effect.
use crate::application::public_discovery::PublicDiscoveryFailure;
use cantos_contracts::theatre::WorkPage;
use std::collections::BTreeSet;

// Twenty legal catalog records can each carry a 16 KiB description, a 512-byte title and
// 63-byte language. This also allows their JSON escaping while keeping decoding bounded.
const MAX_RESPONSE_BYTES: usize = 2_097_152;

pub fn decode_response(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<WorkPage, PublicDiscoveryFailure> {
    if status != 200 {
        return Err(PublicDiscoveryFailure::Unavailable);
    }
    if body.len() > MAX_RESPONSE_BYTES
        || !content_type.is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return Err(PublicDiscoveryFailure::Protocol);
    }
    let page: WorkPage =
        serde_json::from_slice(body).map_err(|_| PublicDiscoveryFailure::Protocol)?;
    let mut ids = BTreeSet::new();
    if page.items.len() > 20
        || (page.items.is_empty() && page.next_cursor.is_some())
        || page.items.iter().any(|work| {
            crate::core::public_ids::PublicWorkId::parse(&work.work_id).is_none()
                || !ids.insert(work.work_id.as_str())
                || work.title.trim().is_empty()
                || work.language.trim().is_empty()
        })
    {
        return Err(PublicDiscoveryFailure::Protocol);
    }
    Ok(page)
}

#[cfg(target_arch = "wasm32")]
pub async fn load() -> Result<WorkPage, PublicDiscoveryFailure> {
    use gloo_net::http::Request;
    use web_sys::{AbortController, RequestCredentials};

    let controller = AbortController::new().map_err(|_| PublicDiscoveryFailure::Unavailable)?;
    let deadline =
        super::browser::deadline(controller).map_err(|_| PublicDiscoveryFailure::Unavailable)?;
    let signal = deadline.controller.signal();
    let response = Request::get("/api/v1/theatre/works?limit=20")
        .credentials(RequestCredentials::Omit)
        .header("Accept", "application/json")
        .abort_signal(Some(&signal))
        .send()
        .await
        .map_err(|_| PublicDiscoveryFailure::Unavailable)?;
    if response.status() != 200 {
        return Err(PublicDiscoveryFailure::Unavailable);
    }
    let body = response
        .binary()
        .await
        .map_err(|_| PublicDiscoveryFailure::Unavailable)?;
    decode_response(
        response.status(),
        response.headers().get("content-type").as_deref(),
        &body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &[u8] = br#"{"items":[{"work_id":"00000000-0000-0000-0000-000000000001","kind":"story","title":"Public story","description":"Published description","language":"vi-VN","first_published_at_ms":1}],"next_cursor":null}"#;

    #[test]
    fn published_response_is_decoded_once_and_retains_its_public_identity() {
        let page = decode_response(200, Some("application/json; charset=utf-8"), PAGE).unwrap();
        assert_eq!(page.items[0].title, "Public story");
        assert_eq!(
            page.items[0].work_id,
            "00000000-0000-0000-0000-000000000001"
        );
        assert_eq!(page.items[0].description, "Published description");
    }

    #[test]
    fn outage_401_and_malformed_data_never_become_an_empty_catalog() {
        for status in [401, 403, 404, 500, 503] {
            assert_eq!(
                decode_response(status, Some("application/json"), b"{}"),
                Err(PublicDiscoveryFailure::Unavailable)
            );
        }
        for (kind, body) in [
            (None, PAGE),
            (Some("text/html"), PAGE),
            (Some("application/json"), b"{}".as_slice()),
        ] {
            assert_eq!(
                decode_response(200, kind, body),
                Err(PublicDiscoveryFailure::Protocol)
            );
        }
        let oversized = vec![b' '; MAX_RESPONSE_BYTES + 1];
        assert_eq!(
            decode_response(200, Some("application/json"), &oversized),
            Err(PublicDiscoveryFailure::Protocol)
        );
    }

    #[test]
    fn impossible_or_duplicate_work_identity_is_rejected_before_rendering() {
        let valid: serde_json::Value = serde_json::from_slice(PAGE).unwrap();
        for (key, bad_value) in [
            ("work_id", "not-a-work"),
            ("work_id", "{00000000-0000-0000-0000-000000000001}"),
            ("work_id", "0000000000000000000000000000000A"),
            ("title", " "),
            ("language", ""),
        ] {
            let mut bad = valid.clone();
            bad["items"][0][key] = serde_json::json!(bad_value);
            assert_eq!(
                decode_response(
                    200,
                    Some("application/json"),
                    &serde_json::to_vec(&bad).unwrap()
                ),
                Err(PublicDiscoveryFailure::Protocol)
            );
        }
        let mut duplicate = valid;
        let item = duplicate["items"][0].clone();
        duplicate["items"].as_array_mut().unwrap().push(item);
        assert_eq!(
            decode_response(
                200,
                Some("application/json"),
                &serde_json::to_vec(&duplicate).unwrap()
            ),
            Err(PublicDiscoveryFailure::Protocol)
        );
    }

    #[test]
    fn twenty_maximum_length_legal_catalog_records_fit_the_bounded_response() {
        use cantos_contracts::theatre::{WorkKind, WorkSummary};
        let page = WorkPage {
            items: (1..=20)
                .map(|id| WorkSummary {
                    work_id: uuid::Uuid::from_u128(id).to_string(),
                    kind: WorkKind::Story,
                    title: "T".repeat(512),
                    description: "\"".repeat(16_384),
                    language: format!("vi-x{}-abcd", "-abcdefgh".repeat(6)),
                    first_published_at_ms: 1,
                })
                .collect(),
            next_cursor: None,
        };
        let body = serde_json::to_vec(&page).unwrap();
        assert!(body.len() > 131_072);
        assert!(body.len() < MAX_RESPONSE_BYTES);
        assert_eq!(
            decode_response(200, Some("application/json"), &body),
            Ok(page)
        );
    }
}
