//! Strict cookie syntax plumbing shared by independently owned surface policies.
use crate::platform::oidc::core::AuthError;
use axum::http::{header, HeaderMap};
pub(super) fn unique_cookie(headers: &HeaderMap, name: &str) -> Result<Option<String>, AuthError> {
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        let value = value.to_str().map_err(|_| AuthError::State)?;
        for part in value.split(';') {
            if let Some(value) = part.trim().strip_prefix(&format!("{name}=")) {
                if found.is_some() {
                    return Err(AuthError::State);
                }
                found = Some(value.into());
            }
        }
    }
    Ok(found)
}
