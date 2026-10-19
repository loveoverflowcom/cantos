//! Anonymous JSON discovery reads of the Work list. No identity cookie, credential, write or
//! player effect. The request is always the one canonical `GET /works` with the opt-in
//! presentation capability, so every row carries its public `unit_count`.
use super::public_work::{classify_error, is_json};
use crate::{
    application::{
        public_discovery::{DiscoveryQuery, DiscoveryReadError, PUBLIC_WORK_PAGE_LIMIT},
        public_work::CatalogReadError,
    },
    core::public_ids::PublicWorkId,
};
use cantos_contracts::{routes::THEATRE_BASE, theatre::PresentedWorkPage};
use std::collections::BTreeSet;

// Twenty legal catalog records can each carry a 16 KiB description, a 512-byte title and
// 63-byte language. This also allows their JSON escaping while keeping decoding bounded.
const MAX_RESPONSE_BYTES: usize = 2_097_152;

/// Query values are form-encoded; the cursor is opaque and only encoded, never parsed. The
/// presentation capability is part of every request so a cursor never crosses representations.
pub fn works_path(query: &DiscoveryQuery, cursor: Option<&str>) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in query.parameters() {
        serializer.append_pair(name, &value);
    }
    serializer.append_pair("limit", &PUBLIC_WORK_PAGE_LIMIT.to_string());
    serializer.append_pair("presentation", "1");
    if let Some(cursor) = cursor {
        serializer.append_pair("cursor", cursor);
    }
    format!("{THEATRE_BASE}/works?{}", serializer.finish())
}

/// An empty page that still has a cursor is a valid intermediate page, not a protocol failure.
pub fn decode_response(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
    cursor_sent: bool,
) -> Result<PresentedWorkPage, DiscoveryReadError> {
    if status != 200 {
        return Err(match classify_error(status, content_type, body) {
            CatalogReadError::Unavailable => DiscoveryReadError::Unavailable,
            // Page one carries no cursor, so it cannot be an invalid cursor.
            CatalogReadError::InvalidCursor if cursor_sent => DiscoveryReadError::InvalidCursor,
            // The list has no not-found answer: a coded one is not this contract.
            CatalogReadError::InvalidCursor
            | CatalogReadError::NotFound
            | CatalogReadError::Protocol => DiscoveryReadError::Protocol,
        });
    }
    if body.len() > MAX_RESPONSE_BYTES || !is_json(content_type) {
        return Err(DiscoveryReadError::Protocol);
    }
    let page: PresentedWorkPage =
        serde_json::from_slice(body).map_err(|_| DiscoveryReadError::Protocol)?;
    let mut ids = BTreeSet::new();
    if page.items.len() > PUBLIC_WORK_PAGE_LIMIT
        || page.items.iter().any(|work| {
            PublicWorkId::parse(&work.work_id).is_none()
                || !ids.insert(work.work_id.as_str())
                || work.title.trim().is_empty()
                || work.language.trim().is_empty()
        })
    {
        return Err(DiscoveryReadError::Protocol);
    }
    Ok(page)
}

#[cfg(target_arch = "wasm32")]
pub async fn load(
    ticket: &crate::application::public_discovery::DiscoveryTicket,
) -> Result<PresentedWorkPage, DiscoveryReadError> {
    let path = works_path(ticket.query(), ticket.cursor());
    let (status, content_type, body) = super::public_work::get(&path, MAX_RESPONSE_BYTES)
        .await
        .map_err(|error| match error {
            CatalogReadError::Protocol => DiscoveryReadError::Protocol,
            _ => DiscoveryReadError::Unavailable,
        })?;
    decode_response(
        status,
        content_type.as_deref(),
        &body,
        ticket.cursor().is_some(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_contracts::theatre::{PresentedWorkSummary, WorkKind};

    const PAGE: &[u8] = include_bytes!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/presented_works.json"
    );
    const EMPTY: &[u8] = include_bytes!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/empty.json"
    );
    const INVALID_CURSOR: &[u8] = include_bytes!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/invalid_cursor.json"
    );
    const INVALID_REQUEST: &[u8] = include_bytes!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/invalid_request.json"
    );
    const UNAVAILABLE: &[u8] = include_bytes!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/unavailable.json"
    );
    const NOT_FOUND: &[u8] = include_bytes!(
        "../../../../../packages/contracts/fixtures/theatre/discovery/v1/not_found.json"
    );
    const JSON: Option<&str> = Some("application/json; charset=utf-8");

    fn decode(body: &[u8]) -> Result<PresentedWorkPage, DiscoveryReadError> {
        decode_response(200, JSON, body, false)
    }
    fn query(text: &str, kind: Option<WorkKind>, language: &str) -> DiscoveryQuery {
        DiscoveryQuery::new(text, kind, language).unwrap()
    }

    #[test]
    fn every_read_is_the_one_canonical_presented_works_request() {
        assert_eq!(
            works_path(&DiscoveryQuery::default(), None),
            "/api/v1/theatre/works?limit=20&presentation=1"
        );
        assert_eq!(
            works_path(&query("Ánh đèn & ma", Some(WorkKind::Story), "VI-vn"), None),
            "/api/v1/theatre/works?q=%C3%81nh+%C4%91%C3%A8n+%26+ma&kind=story&language=vi-vn&limit=20&presentation=1"
        );
    }

    #[test]
    fn the_cursor_is_only_form_encoded_and_follows_the_same_query() {
        let path = works_path(&query("đèn", None, ""), Some("lc1_a+b/c=&d"));
        assert_eq!(
            path,
            "/api/v1/theatre/works?q=%C4%91%C3%A8n&limit=20&presentation=1&cursor=lc1_a%2Bb%2Fc%3D%26d"
        );
        // Decoding the query string returns exactly the typed values: nothing was lost or merged.
        let pairs: Vec<(String, String)> =
            url::form_urlencoded::parse(path.split_once('?').unwrap().1.as_bytes())
                .into_owned()
                .collect();
        assert_eq!(
            pairs,
            [
                ("q", "đèn"),
                ("limit", "20"),
                ("presentation", "1"),
                ("cursor", "lc1_a+b/c=&d")
            ]
            .map(|(n, v)| (n.to_owned(), v.to_owned()))
        );
        for kind in [WorkKind::Novel, WorkKind::Story, WorkKind::Podcast] {
            assert!(
                works_path(&query("", Some(kind), ""), None).contains(&format!(
                    "kind={}",
                    crate::application::public_discovery::kind_name(kind)
                ))
            );
        }
    }

    #[test]
    fn presented_response_is_decoded_once_and_retains_identity_and_unit_count() {
        let page = decode_response(200, JSON, PAGE, false).unwrap();
        assert_eq!(page.items[0].title, "Ánh đèn cuối sân khấu");
        assert_eq!(
            page.items[0].work_id,
            "10000000-0000-4000-8000-000000000001"
        );
        assert_eq!(page.items[0].unit_count, 3);
        assert_eq!(page.next_cursor, None);
        assert_eq!(decode(EMPTY).unwrap().items.len(), 0);
    }

    #[test]
    fn an_empty_intermediate_page_with_a_cursor_is_valid_and_not_the_end() {
        let page = decode(
            br#"{"items":[],"next_cursor":"lc1_0000000000000000000000000000000000000000000000000000000000000001"}"#,
        )
        .unwrap();
        assert!(page.items.is_empty() && page.next_cursor.is_some());
    }

    #[test]
    fn the_v1_work_shape_without_unit_count_is_not_a_presented_page() {
        let v1 = include_bytes!(
            "../../../../../packages/contracts/fixtures/theatre/discovery/v1/works.json"
        );
        assert_eq!(decode(v1), Err(DiscoveryReadError::Protocol));
        let mut value: serde_json::Value = serde_json::from_slice(PAGE).unwrap();
        for bad in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!(9_007_199_254_740_992_u64),
        ] {
            value["items"][0]["unit_count"] = bad;
            assert_eq!(
                decode(&serde_json::to_vec(&value).unwrap()),
                Err(DiscoveryReadError::Protocol)
            );
        }
    }

    #[test]
    fn errors_are_classified_by_the_contract_pairing_of_status_code_and_retryability() {
        let cases: [(u16, &[u8], bool, DiscoveryReadError); 11] = [
            (400, INVALID_CURSOR, true, DiscoveryReadError::InvalidCursor),
            (400, INVALID_CURSOR, false, DiscoveryReadError::Protocol),
            (400, INVALID_REQUEST, true, DiscoveryReadError::Protocol),
            (503, UNAVAILABLE, false, DiscoveryReadError::Unavailable),
            (404, NOT_FOUND, false, DiscoveryReadError::Protocol),
            (200, NOT_FOUND, false, DiscoveryReadError::Protocol),
            (
                500,
                br#"{"code":"unavailable","retryable":false}"#,
                false,
                DiscoveryReadError::Protocol,
            ),
            (401, b"{}", false, DiscoveryReadError::Unavailable),
            (
                502,
                b"<html>Bad gateway</html>",
                false,
                DiscoveryReadError::Unavailable,
            ),
            (
                404,
                b"<html>Not Found</html>",
                false,
                DiscoveryReadError::Unavailable,
            ),
            (503, b"", true, DiscoveryReadError::Unavailable),
        ];
        for (status, body, cursor_sent, expected) in cases {
            assert_eq!(
                decode_response(status, JSON, body, cursor_sent),
                Err(expected),
                "{status} {}",
                String::from_utf8_lossy(body)
            );
        }
    }

    #[test]
    fn malformed_wrong_type_or_oversized_answers_never_become_an_empty_catalog() {
        for (kind, body) in [
            (None, PAGE),
            (Some("text/html"), PAGE),
            (Some("application/json"), b"{}".as_slice()),
            (Some("application/json"), b"not json".as_slice()),
        ] {
            assert_eq!(
                decode_response(200, kind, body, false),
                Err(DiscoveryReadError::Protocol)
            );
        }
        let oversized = vec![b' '; MAX_RESPONSE_BYTES + 1];
        assert_eq!(decode(&oversized), Err(DiscoveryReadError::Protocol));
    }

    #[test]
    fn impossible_or_duplicate_work_identity_is_rejected_before_rendering() {
        let valid: serde_json::Value = serde_json::from_slice(PAGE).unwrap();
        for (key, bad_value) in [
            ("work_id", "not-a-work"),
            ("work_id", "{10000000-0000-4000-8000-000000000001}"),
            ("work_id", "10000000000040008000000000000001"),
            ("title", " "),
            ("language", ""),
        ] {
            let mut bad = valid.clone();
            bad["items"][0][key] = serde_json::json!(bad_value);
            assert_eq!(
                decode(&serde_json::to_vec(&bad).unwrap()),
                Err(DiscoveryReadError::Protocol)
            );
        }
        let mut duplicate = valid.clone();
        let item = duplicate["items"][0].clone();
        duplicate["items"].as_array_mut().unwrap().push(item);
        assert_eq!(
            decode(&serde_json::to_vec(&duplicate).unwrap()),
            Err(DiscoveryReadError::Protocol)
        );
        let mut crowded = valid;
        let item = crowded["items"][0].clone();
        let many: Vec<_> = (0..=PUBLIC_WORK_PAGE_LIMIT as u128)
            .map(|n| {
                let mut item = item.clone();
                item["work_id"] = serde_json::json!(uuid::Uuid::from_u128(n + 1).to_string());
                item
            })
            .collect();
        crowded["items"] = serde_json::Value::Array(many);
        assert_eq!(
            decode(&serde_json::to_vec(&crowded).unwrap()),
            Err(DiscoveryReadError::Protocol)
        );
    }

    #[test]
    fn twenty_maximum_length_legal_catalog_records_fit_the_bounded_response() {
        let page = PresentedWorkPage {
            items: (1..=20)
                .map(|id| PresentedWorkSummary {
                    work_id: uuid::Uuid::from_u128(id).to_string(),
                    kind: WorkKind::Story,
                    title: "T".repeat(512),
                    description: "\"".repeat(16_384),
                    language: format!("vi-x{}-abcd", "-abcdefgh".repeat(6)),
                    first_published_at_ms: 1,
                    unit_count: 9_007_199_254_740_991,
                })
                .collect(),
            next_cursor: None,
        };
        let body = serde_json::to_vec(&page).unwrap();
        assert!(body.len() > 131_072);
        assert!(body.len() < MAX_RESPONSE_BYTES);
        assert_eq!(decode(&body), Ok(page));
    }
}
