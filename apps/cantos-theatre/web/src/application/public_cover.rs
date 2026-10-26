//! Admission of the public cover a Work summary carries (Discovery v1.2, ADR 0033).
//!
//! The Server is the only authority for a cover. The client accepts the exact delivery shape the
//! contract promises and refuses anything else as a protocol fault, so a response can never point
//! the image element at another origin scheme, another Work's image, a credentialed URL or a
//! private asset. It never builds a cover URL: the one admitted is used exactly as given. `null`
//! is the legitimate no-cover state and is not examined here.
use crate::contract::discovery::{CoverImage, CoveredWorkSummary};
use crate::core::public_ids::PublicWorkId;
use cantos_web_auth::address::THEATRE_BASE;

/// The private Artwork v1 text bound (UTF-8 bytes); the schema's character bound is looser.
const MAX_ALT_BYTES: usize = 1024;

/// Whether `cover` is a well-formed cover of exactly this Work.
pub fn is_admitted(work: PublicWorkId, cover: &CoverImage) -> bool {
    is_sha256(&cover.sha256)
        && !cover.alt_text.trim().is_empty()
        && cover.alt_text.len() <= MAX_ALT_BYTES
        && is_delivery_url(&cover.url, work, &cover.sha256)
}

/// A Work with no cover is admitted; one with a cover needs an admitted cover.
pub fn work_is_admitted(work: &CoveredWorkSummary, id: PublicWorkId) -> bool {
    work.cover
        .as_ref()
        .is_none_or(|cover| is_admitted(id, cover))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// `https://{origin}/api/v1/theatre/works/{work_id}/cover/{sha256}`: no credentials, query or
/// fragment, and the summary's own Work and digest.
fn is_delivery_url(raw: &str, work: PublicWorkId, sha256: &str) -> bool {
    let Ok(url) = url::Url::parse(raw) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.path() == format!("{THEATRE_BASE}/works/{work}/cover/{sha256}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::discovery::CoveredWorkPage;

    const WORK: &str = "10000000-0000-4000-8000-000000000001";
    const COVERED: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/cover/covered_works.json"
    );

    fn id() -> PublicWorkId {
        PublicWorkId::parse(WORK).unwrap()
    }

    fn cover() -> CoverImage {
        let page: CoveredWorkPage = serde_json::from_str(COVERED).unwrap();
        page.items[0].cover.clone().unwrap()
    }

    #[test]
    fn the_server_fixture_cover_is_admitted_and_its_null_is_too() {
        let page: CoveredWorkPage = serde_json::from_str(COVERED).unwrap();
        assert!(page.items[0].cover.is_some() && page.items[1].cover.is_none());
        assert!(is_admitted(id(), &cover()));
        assert!(work_is_admitted(&page.items[0], id()));
        let second = PublicWorkId::parse(&page.items[1].work_id).unwrap();
        assert!(work_is_admitted(&page.items[1], second));
    }

    #[test]
    fn a_cover_of_another_work_or_digest_is_refused() {
        let other = PublicWorkId::parse("10000000-0000-4000-8000-000000000002").unwrap();
        assert!(!is_admitted(other, &cover()));
        let mut moved = cover();
        moved.sha256 = "0".repeat(64);
        assert!(!is_admitted(id(), &moved));
    }

    #[test]
    fn only_the_contract_delivery_url_shape_is_admitted() {
        let good = cover();
        let with = |url: String| CoverImage {
            url,
            ..good.clone()
        };
        let path = format!("/api/v1/theatre/works/{WORK}/cover/{}", good.sha256);
        for refused in [
            format!("http://api.cantos.example{path}"),
            format!("//api.cantos.example{path}"),
            path.clone(),
            format!("https://user:pw@api.cantos.example{path}"),
            format!("https://api.cantos.example{path}?token=1"),
            format!("https://api.cantos.example{path}#frag"),
            format!("https://api.cantos.example{path}/"),
            format!(
                "https://api.cantos.example/api/v1/studio/works/{WORK}/cover/{}",
                good.sha256
            ),
            format!(
                "https://api.cantos.example/{}",
                path.trim_start_matches('/').to_uppercase()
            ),
            "data:image/png;base64,AAAA".to_owned(),
            "javascript:alert(1)".to_owned(),
            String::new(),
        ] {
            assert!(!is_admitted(id(), &with(refused.clone())), "{refused}");
        }
        assert!(is_admitted(
            id(),
            &with(format!("https://api.cantos.example:8443{path}"))
        ));
    }

    #[test]
    fn digest_and_alt_text_are_bounded() {
        let mut bad = cover();
        bad.sha256 = bad.sha256.to_uppercase();
        assert!(!is_admitted(id(), &bad));
        for alt in ["", "   ", "\n"] {
            let mut blank = cover();
            blank.alt_text = alt.to_owned();
            assert!(!is_admitted(id(), &blank), "{alt:?}");
        }
        let mut long = cover();
        long.alt_text = "đ".repeat(513); // 1026 UTF-8 bytes
        assert!(!is_admitted(id(), &long));
        long.alt_text = "đ".repeat(512); // exactly 1024 bytes
        assert!(is_admitted(id(), &long));
    }
}
