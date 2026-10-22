//! Strict cookie syntax plumbing shared by independently owned surface policies.
use axum::http::{header, HeaderMap};

/// A cookie header that cannot name exactly one value: non-text bytes or a duplicate name.
/// Callers refuse the request; they never pick one of the candidates.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct AmbiguousCookie;

pub(super) fn unique_cookie(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<String>, AmbiguousCookie> {
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        let value = value.to_str().map_err(|_| AmbiguousCookie)?;
        for part in value.split(';') {
            if let Some(value) = part.trim().strip_prefix(&format!("{name}=")) {
                if found.is_some() {
                    return Err(AmbiguousCookie);
                }
                found = Some(value.into());
            }
        }
    }
    Ok(found)
}
