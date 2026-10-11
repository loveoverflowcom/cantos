//! Trusted local operator tools. The HTTP runtime role never receives these privileges.
use sha2::{Digest, Sha256};
use tokio_postgres::{Client, Config, NoTls};
use uuid::Uuid;

use crate::postgres::{token_hash, StoreError};

fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn valid_text(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.len() <= max && !text.contains('\0')
}

async fn connect(config: &Config) -> Result<Client, StoreError> {
    let (client, connection) = config.connect(NoTls).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Ok(client)
}

pub async fn create_creator(config: &Config) -> Result<String, StoreError> {
    let actor = Uuid::new_v4().to_string();
    connect(config)
        .await?
        .execute("INSERT INTO actors(id) VALUES($1)", &[&actor])
        .await?;
    Ok(actor)
}

pub async fn issue_token(config: &Config, actor: &str, hours: u32) -> Result<String, StoreError> {
    if !valid_id(actor) || !(1..=720).contains(&hours) {
        return Err(StoreError::InvalidRequest);
    }
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| StoreError::Unavailable)?;
    let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let client = connect(config).await?;
    let changed = client.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) SELECT $1,id,CURRENT_TIMESTAMP+($2::bigint * interval '1 hour') FROM actors WHERE id=$3 AND active", &[&token_hash(&token),&i64::from(hours),&actor]).await?;
    if changed != 1 {
        return Err(StoreError::InvalidRequest);
    }
    Ok(token)
}

pub async fn revoke_tokens(config: &Config, actor: &str) -> Result<(), StoreError> {
    if !valid_id(actor) {
        return Err(StoreError::InvalidRequest);
    }
    connect(config)
        .await?
        .execute(
            "UPDATE sessions SET revoked=true WHERE actor_id=$1",
            &[&actor],
        )
        .await?;
    Ok(())
}

pub async fn register_evidence(
    config: &Config,
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
    connect(config)
        .await?
        .execute(
            "INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,$2,$3,$4)",
            &[&owner, &kind, &id, &description],
        )
        .await?;
    Ok(())
}

/// Preserve UTF-8 bytes exactly, including NFD, CRLF and source whitespace.
pub async fn record_source(
    config: &Config,
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
    let mut client = connect(config).await?;
    let tx = client.transaction().await?;
    tx.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,'source',$2,$3) ON CONFLICT DO NOTHING", &[&owner,&id,&reference]).await?;
    tx.execute("INSERT INTO source_records(owner_id,id,reference,original_text,sha256) VALUES($1,$2,$3,$4,$5)", &[&owner,&id,&reference,&text,&hash]).await?;
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
