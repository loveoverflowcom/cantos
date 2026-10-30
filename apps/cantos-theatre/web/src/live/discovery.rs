//! Anonymous JSON discovery reads of the Work list. No identity cookie, credential, write or
//! player effect. The canonical `GET /works` reads only active published generations, including
//! their public `unit_count` and required-nullable `cover`.
use super::public_work::{classify_error, is_json};
use crate::contract::discovery::MetadataWorkPage;
use crate::{
    application::{
        public_discovery::{DiscoveryQuery, DiscoveryReadError, PUBLIC_WORK_PAGE_LIMIT},
        public_work::CatalogReadError,
    },
    core::public_ids::PublicWorkId,
};
use cantos_web_auth::address::THEATRE_BASE;
use std::collections::BTreeSet;

// Twenty legal catalog records can each carry a 16 KiB description, a 512-byte title and
// 63-byte language. This also allows their JSON escaping while keeping decoding bounded.
const MAX_RESPONSE_BYTES: usize = 2_097_152;

/// Query values are form-encoded; the cursor is opaque and only encoded, never parsed. The
/// Server binds each cursor to this query and each Unit walk to its generation.
pub fn works_path(query: &DiscoveryQuery, cursor: Option<&str>) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in query.parameters() {
        serializer.append_pair(name, &value);
    }
    serializer.append_pair("limit", &PUBLIC_WORK_PAGE_LIMIT.to_string());
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
) -> Result<MetadataWorkPage, DiscoveryReadError> {
    if status != 200 {
        return Err(match classify_error(status, content_type, body) {
            CatalogReadError::Diagnostic(kind) => DiscoveryReadError::Diagnostic(kind),
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
    let page: MetadataWorkPage =
        serde_json::from_slice(body).map_err(|_| DiscoveryReadError::Protocol)?;
    let mut ids = BTreeSet::new();
    if page.items.len() > PUBLIC_WORK_PAGE_LIMIT
        || page.items.iter().any(|work| {
            PublicWorkId::parse(&work.work_id)
                .is_none_or(|id| !crate::application::public_cover::work_is_admitted(work, id))
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
) -> Result<MetadataWorkPage, DiscoveryReadError> {
    let path = works_path(ticket.query(), ticket.cursor());
    let (status, content_type, body) = super::public_work::get(&path, MAX_RESPONSE_BYTES)
        .await
        .map_err(|error| match error {
            CatalogReadError::Diagnostic(kind) => DiscoveryReadError::Diagnostic(kind),
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
    use crate::contract::discovery::{CoverImage, CoverMime, MetadataWorkSummary, WorkKind};

    const PAGE: &[u8] = include_bytes!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/covered_works.json"
    );
    const EMPTY: &[u8] = include_bytes!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/empty.json"
    );
    const INVALID_CURSOR: &[u8] = include_bytes!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/invalid_cursor.json"
    );
    const INVALID_REQUEST: &[u8] = include_bytes!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/invalid_request.json"
    );
    const UNAVAILABLE: &[u8] = include_bytes!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/unavailable.json"
    );
    const NOT_FOUND: &[u8] = include_bytes!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/not_found.json"
    );
    const JSON: Option<&str> = Some("application/json; charset=utf-8");

    fn decode(body: &[u8]) -> Result<MetadataWorkPage, DiscoveryReadError> {
        decode_response(200, JSON, body, false)
    }
    fn query(text: &str, kind: Option<WorkKind>, language: &str) -> DiscoveryQuery {
        DiscoveryQuery::new(text, kind, language).unwrap()
    }

    #[test]
    fn every_read_is_the_one_canonical_covered_works_request() {
        assert_eq!(
            works_path(&DiscoveryQuery::default(), None),
            "/api/v1/theatre/works?limit=20"
        );
        assert_eq!(
            works_path(&query("Ánh đèn & ma", Some(WorkKind::Story), "VI-vn"), None),
            "/api/v1/theatre/works?q=%C3%81nh+%C4%91%C3%A8n+%26+ma&kind=story&language=vi-vn&limit=20"
        );
    }

    #[test]
    fn the_cursor_is_only_form_encoded_and_follows_the_same_query() {
        let path = works_path(&query("đèn", None, ""), Some("lc1_a+b/c=&d"));
        assert_eq!(
            path,
            "/api/v1/theatre/works?q=%C4%91%C3%A8n&limit=20&cursor=lc1_a%2Bb%2Fc%3D%26d"
        );
        // Decoding the query string returns exactly the typed values: nothing was lost or merged.
        let pairs: Vec<(String, String)> =
            url::form_urlencoded::parse(path.split_once('?').unwrap().1.as_bytes())
                .into_owned()
                .collect();
        assert_eq!(
            pairs,
            [("q", "đèn"), ("limit", "20"), ("cursor", "lc1_a+b/c=&d")]
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
    fn covered_response_is_decoded_once_and_retains_identity_unit_count_and_cover() {
        let page = decode_response(200, JSON, PAGE, false).unwrap();
        assert_eq!(page.items[0].title, "Ánh đèn cuối sân khấu");
        assert_eq!(
            page.items[0].work_id,
            "10000000-0000-4000-8000-000000000001"
        );
        assert_eq!(page.items[0].unit_count, 3);
        assert!(page.items[0].cover.is_some() && page.items[1].cover.is_none());
        assert!(page.next_cursor.is_some());
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
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/works.json"
        );
        assert_eq!(decode(v1), Err(DiscoveryReadError::Protocol));
        let mut value: serde_json::Value = serde_json::from_slice(PAGE).unwrap();
        value["items"][0]["unit_count"] = serde_json::json!(0);
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_ok());
        for bad in [
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

    const COVER_WORK: &str = "10000000-0000-4000-8000-000000000001";
    const COVER_SHA: &str = "a3c1f0e2b4d6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e6f8a0b2c4d6e8f0a2b4c6d8";

    /// The Server's cover fixture with one mutation applied to the first Work's `cover` object.
    fn with_cover(mutate: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
        let mut value: serde_json::Value = serde_json::from_slice(PAGE).unwrap();
        mutate(&mut value["items"][0]["cover"]);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn a_null_cover_is_a_legitimate_work_but_a_missing_one_is_a_protocol_error() {
        assert!(decode(PAGE).unwrap().items[1].cover.is_none());
        let mut value: serde_json::Value = serde_json::from_slice(PAGE).unwrap();
        value["items"][1].as_object_mut().unwrap().remove("cover");
        assert_eq!(
            decode(&serde_json::to_vec(&value).unwrap()),
            Err(DiscoveryReadError::Protocol)
        );
        // The earlier presentation levels are not a v1.2 page, so a stale cache or proxy answer
        // never decodes as "no cover".
        for earlier in [
            include_bytes!("../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/presented_works.json").as_slice(),
            include_bytes!("../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/works.json").as_slice(),
        ] {
            assert_eq!(decode(earlier), Err(DiscoveryReadError::Protocol));
        }
    }

    #[test]
    fn a_malformed_or_unauthorized_cover_refuses_the_whole_page() {
        type Mutation = Box<dyn FnOnce(&mut serde_json::Value)>;
        let cases: [(&str, Mutation); 12] = [
            ("unknown field", Box::new(|c| c["asset_id"] = "x".into())),
            ("sha256 shape", Box::new(|c| c["sha256"] = "ABC".into())),
            (
                "another work",
                Box::new(|c| {
                    c["url"] = format!("https://api.cantos.example/api/v1/theatre/works/10000000-0000-4000-8000-000000000002/cover/{COVER_SHA}").into();
                }),
            ),
            (
                "another digest",
                Box::new(|c| {
                    c["url"] = format!(
                        "https://api.cantos.example/api/v1/theatre/works/{COVER_WORK}/cover/{}",
                        "b".repeat(64)
                    )
                    .into();
                }),
            ),
            (
                "insecure scheme",
                Box::new(|c| {
                    c["url"] = format!(
                        "http://api.cantos.example/api/v1/theatre/works/{COVER_WORK}/cover/{COVER_SHA}"
                    )
                    .into();
                }),
            ),
            (
                "studio route",
                Box::new(|c| {
                    c["url"] = format!(
                        "https://api.cantos.example/api/v1/studio/works/{COVER_WORK}/cover/{COVER_SHA}"
                    )
                    .into();
                }),
            ),
            (
                "query",
                Box::new(|c| {
                    c["url"] = format!(
                        "https://api.cantos.example/api/v1/theatre/works/{COVER_WORK}/cover/{COVER_SHA}?x=1"
                    )
                    .into();
                }),
            ),
            (
                "content type",
                Box::new(|c| c["content_type"] = "image/gif".into()),
            ),
            ("width", Box::new(|c| c["width_px"] = 4097.into())),
            ("height", Box::new(|c| c["height_px"] = 0.into())),
            (
                "length",
                Box::new(|c| c["byte_length"] = 8_388_609_u64.into()),
            ),
            ("blank alt", Box::new(|c| c["alt_text"] = "  ".into())),
        ];
        for (name, mutate) in cases {
            assert_eq!(
                decode(&with_cover(mutate)),
                Err(DiscoveryReadError::Protocol),
                "{name}"
            );
        }
    }

    #[test]
    fn errors_are_classified_by_the_contract_pairing_of_status_code_and_retryability() {
        let cases: [(u16, &[u8], bool, DiscoveryReadError); 13] = [
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
            (
                401,
                b"{}",
                false,
                DiscoveryReadError::Diagnostic(
                    crate::application::errors::ErrorKind::Unauthenticated,
                ),
            ),
            (
                403,
                b"{}",
                false,
                DiscoveryReadError::Diagnostic(crate::application::errors::ErrorKind::Forbidden),
            ),
            (
                504,
                b"",
                false,
                DiscoveryReadError::Diagnostic(crate::application::errors::ErrorKind::Timeout),
            ),
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
        let page = MetadataWorkPage {
            items: (1..=20)
                .map(|id| {
                    let work_id = uuid::Uuid::from_u128(id).to_string();
                    let sha256 = "a".repeat(64);
                    MetadataWorkSummary {
                        cover: Some(CoverImage {
                            url: format!(
                                "https://{}.example/api/v1/theatre/works/{work_id}/cover/{sha256}",
                                "a".repeat(200)
                            ),
                            sha256,
                            content_type: CoverMime::Jpeg,
                            width_px: 4096,
                            height_px: 4096,
                            byte_length: 8_388_608,
                            alt_text: "\"".repeat(1024),
                        }),
                        work_id,
                        kind: WorkKind::Story,
                        title: "T".repeat(512),
                        description: "\"".repeat(16_384),
                        language: format!("vi-x{}-abcd", "-abcdefgh".repeat(6)),
                        first_visible_at_ms: 1,
                        unit_count: 9_007_199_254_740_991,
                    }
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
