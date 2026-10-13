//! A token exchange is not authenticated until the browser's owner cookie is accepted.
use cantos_contracts::{ApiError, ErrorCode, SessionResponse};

pub fn confirm_exchange(
    expected: SessionResponse,
    observed: SessionResponse,
) -> Result<SessionResponse, ApiError> {
    if !expected.actor_id.is_empty() && expected.actor_id == observed.actor_id {
        Ok(observed)
    } else {
        Err(ApiError {
            code: ErrorCode::Unauthenticated,
            current_revision: None,
            issues: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_acknowledgement_requires_the_same_nonempty_cookie_actor() {
        for (expected, observed, accepted) in [
            ("alice", "alice", true),
            ("alice", "bob", false),
            ("alice", "", false),
            ("", "", false),
        ] {
            let result = confirm_exchange(
                SessionResponse {
                    actor_id: expected.into(),
                },
                SessionResponse {
                    actor_id: observed.into(),
                },
            );
            if accepted {
                assert_eq!(result.unwrap().actor_id, expected);
            } else {
                assert_eq!(result.err().unwrap().code, ErrorCode::Unauthenticated);
            }
        }
    }
}
