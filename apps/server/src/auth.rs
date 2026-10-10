use std::error::Error;
use std::fmt::{self, Display, Formatter};

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

/// The creator identified by a currently valid database-backed credential.
///
/// The private field and absence of deserialization keep request DTOs from
/// constructing an authenticated actor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Actor {
    id: Uuid,
}

impl Actor {
    pub fn id(&self) -> Uuid {
        self.id
    }
}

#[derive(Debug)]
pub enum AuthError {
    Unauthenticated,
    Storage(sqlx::Error),
}

impl Display for AuthError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthenticated => formatter.write_str("authentication required"),
            Self::Storage(_) => formatter.write_str("authentication storage unavailable"),
        }
    }
}

impl Error for AuthError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unauthenticated => None,
            Self::Storage(error) => Some(error),
        }
    }
}

/// Authenticate one Authorization header. The HTTP adapter rejects duplicate
/// header values before calling this function.
pub async fn authenticate(pool: &PgPool, header: Option<&str>) -> Result<Actor, AuthError> {
    let token = bearer_token(header)?;
    let creator_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT token.creator_id
         FROM creator_tokens AS token
         JOIN creators AS creator ON creator.id = token.creator_id
         WHERE token.token_hash = $1
           AND token.expires_at > statement_timestamp()
           AND token.revoked_at IS NULL
           AND NOT creator.disabled",
    )
    .bind(token_hash(token))
    .fetch_optional(pool)
    .await
    .map_err(AuthError::Storage)?;

    match creator_id {
        Some(id) => Ok(Actor { id }),
        None => Err(AuthError::Unauthenticated),
    }
}

/// Hash the canonical token text; never persist the bearer credential itself.
pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn authorize_owner(actor: &Actor, owner: Uuid) -> bool {
    actor.id == owner
}

fn bearer_token(header: Option<&str>) -> Result<&str, AuthError> {
    let (scheme, token) = header
        .and_then(|value| value.split_once(' '))
        .ok_or(AuthError::Unauthenticated)?;

    let canonical_token = token.len() == 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));

    if scheme.eq_ignore_ascii_case("Bearer") && canonical_token {
        Ok(token)
    } else {
        Err(AuthError::Unauthenticated)
    }
}

#[cfg(test)]
mod tests {
    use super::{Actor, AuthError, authorize_owner, bearer_token};
    use uuid::Uuid;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn bearer_scheme_is_case_insensitive_and_preserves_the_exact_token() {
        for scheme in ["Bearer", "bearer", "BEARER", "bEaReR"] {
            let header = format!("{scheme} {TOKEN}");
            assert_eq!(bearer_token(Some(&header)).ok(), Some(TOKEN));
        }
    }

    #[test]
    fn malformed_credentials_are_unauthenticated() {
        let invalid_headers = [
            String::new(),
            "Bearer".to_owned(),
            "Bearer ".to_owned(),
            format!("Basic {TOKEN}"),
            format!(" Bearer {TOKEN}"),
            format!("Bearer  {TOKEN}"),
            format!("Bearer\t{TOKEN}"),
            format!("Bearer {TOKEN} "),
            format!("Bearer {TOKEN}\n"),
            format!("Bearer {}", &TOKEN[..63]),
            format!("Bearer {TOKEN}0"),
            format!("Bearer {}", TOKEN.to_ascii_uppercase()),
            format!("Bearer {}g", &TOKEN[..63]),
            format!("Bearer {}é", &TOKEN[..62]),
            format!("Bearer {TOKEN}, Bearer {TOKEN}"),
        ];

        assert!(matches!(
            bearer_token(None),
            Err(AuthError::Unauthenticated)
        ));
        for header in invalid_headers {
            assert!(matches!(
                bearer_token(Some(&header)),
                Err(AuthError::Unauthenticated)
            ));
        }
    }

    #[test]
    fn an_authenticated_creator_can_only_authorize_their_own_resources() {
        let owner = Uuid::from_u128(1);
        let other_creator = Uuid::from_u128(2);
        let actor = Actor { id: owner };

        assert_eq!(actor.id(), owner);
        assert!(authorize_owner(&actor, owner));
        assert!(!authorize_owner(&actor, other_creator));
    }
}
