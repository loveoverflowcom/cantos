//! Trusted local operator tools. They open `server.db` directly as its one owner, so they run
//! only while the Server is stopped; the HTTP runtime never receives these privileges.
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::platform::{
    identity::{
        domain::{ActorId, ExternalIdentityKey, LinkOutcome, Registration},
        IdentityService, RegistryError, TursoIdentityRegistry,
    },
    store::{token_hash, Store, StoreError},
};

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn valid_text(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.len() <= max && !text.contains('\0')
}

pub async fn create_creator(store: &Store) -> Result<String, StoreError> {
    let actor = ActorId::new(Uuid::new_v4().to_string()).map_err(|_| StoreError::InvalidRequest)?;
    store
        .connection()
        .await?
        .execute("INSERT INTO actors(id) VALUES(?1)", &[&actor.as_str()])
        .await?;
    Ok(actor.into())
}

fn identity_service(store: &Store) -> IdentityService<TursoIdentityRegistry> {
    IdentityService::new(TursoIdentityRegistry::new(store.database().clone()))
}

/// Explicit operator action only; does not verify a provider claim or issue a session.
pub async fn register_identity(
    store: &Store,
    key: &ExternalIdentityKey,
) -> Result<Registration, RegistryError> {
    let candidate = ActorId::new(Uuid::new_v4().to_string())?;
    identity_service(store).register(key, &candidate).await
}

/// Explicit operator intent; never called by an HTTP handler.
pub async fn link_identity(
    store: &Store,
    actor: &ActorId,
    key: &ExternalIdentityKey,
) -> Result<LinkOutcome, RegistryError> {
    identity_service(store).link(key, actor).await
}

/// A development Studio token. A production host refuses every one of them.
pub async fn issue_token(store: &Store, actor: &str, hours: u32) -> Result<String, StoreError> {
    if !valid_id(actor) || !(1..=720).contains(&hours) {
        return Err(StoreError::InvalidRequest);
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| StoreError::Unavailable)?;
    let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let mut client = store.connection().await?;
    let tx = client.transaction().await?;
    let expires = tx
        .now_micros()
        .checked_add(i64::from(hours) * 3_600_000_000)
        .ok_or(StoreError::InvalidRequest)?;
    let changed = tx
        .execute(
            "INSERT INTO development_sessions(token_hash,actor_id,expires_at,session_id) \
             SELECT ?1,id,?2,?3 FROM actors WHERE id=?4 AND active=1",
            &[
                &token_hash(&token),
                &expires,
                &Uuid::new_v4().hyphenated().to_string(),
                &actor,
            ],
        )
        .await?;
    if changed != 1 {
        return Err(StoreError::InvalidRequest);
    }
    tx.commit().await?;
    Ok(token)
}

pub async fn revoke_tokens(store: &Store, actor: &str) -> Result<(), StoreError> {
    if !valid_id(actor) {
        return Err(StoreError::InvalidRequest);
    }
    store
        .connection()
        .await?
        .execute(
            "UPDATE development_sessions SET revoked=1 WHERE actor_id=?1",
            &[&actor],
        )
        .await?;
    Ok(())
}

pub async fn register_evidence(
    store: &Store,
    owner: &str,
    kind: &str,
    id: &str,
    description: &str,
) -> Result<(), StoreError> {
    if !valid_id(owner)
        || !valid_id(id)
        || !matches!(kind, "source" | "rights" | "generation" | "asset")
        || !valid_text(description, 2048)
    {
        return Err(StoreError::InvalidRequest);
    }
    store
        .connection()
        .await?
        .execute(
            "INSERT INTO script_evidence(owner_id,kind,id,description) VALUES(?1,?2,?3,?4)",
            &[&owner, &kind, &id, &description],
        )
        .await?;
    Ok(())
}

/// Preserve UTF-8 bytes exactly, including NFD, CRLF and source whitespace.
pub async fn record_source(
    store: &Store,
    owner: &str,
    id: &str,
    reference: &str,
    text: &str,
) -> Result<(), StoreError> {
    if !valid_id(owner)
        || !valid_id(id)
        || !valid_text(reference, 2048)
        || !valid_text(text, 1_048_576)
    {
        return Err(StoreError::InvalidRequest);
    }
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    let mut client = store.connection().await?;
    let tx = client.transaction().await?;
    tx.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES(?1,'source',?2,?3) ON CONFLICT DO NOTHING", &[&owner,&id,&reference]).await?;
    tx.execute("INSERT INTO source_records(owner_id,id,reference,original_text,sha256) VALUES(?1,?2,?3,?4,?5)", &[&owner,&id,&reference,&text,&hash]).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{valid_id, valid_text};

    #[test]
    fn operator_inputs_use_existing_opaque_ids_and_bounded_nonempty_utf8() {
        assert!(valid_id("source-01"));
        for id in ["", "Upper", "has space", "người"] {
            assert!(!valid_id(id));
        }
        assert!(!valid_id(&"x".repeat(65)));
        assert!(valid_text("Người\r\n  ", 100));
        for text in ["", " \n", "a\0b"] {
            assert!(!valid_text(text, 100));
        }
        assert!(!valid_text("Đ", 1));
    }
}
