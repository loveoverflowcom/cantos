//! Pure editorial decisions and validated commands; no HTTP or database dependencies.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::script_ir::{self, ScriptContent, ValidationReport};

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("invalid request")]
    InvalidRequest,
    #[error("invalid script")]
    InvalidScript(ValidationReport),
    #[error("script identity or provenance changed")]
    IdentityChanged,
    #[error("stale base version")]
    Conflict { head_version: i64 },
    #[error("operation id reused")]
    OperationReused,
    #[error("stored script is corrupt")]
    Corrupt,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SourceKind {
    Original,
    Imported,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInput {
    pub id: String,
    pub kind: SourceKind,
    pub reference: String,
    pub original_text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RightsInput {
    pub id: String,
    pub holder: String,
    pub evidence_reference: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateRequest {
    pub operation_id: Uuid,
    pub content: Value,
    pub source: SourceInput,
    pub rights: RightsInput,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SaveRequest {
    pub operation_id: Uuid,
    pub base_version: i64,
    pub content: Value,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptRequest {
    pub operation_id: Uuid,
    pub version: i64,
}

pub(crate) struct NewScript {
    pub operation_id: Uuid,
    pub content: ScriptContent,
    pub source: SourceInput,
    pub rights: RightsInput,
    pub request: Vec<u8>,
}

pub(crate) struct Save {
    pub operation_id: Uuid,
    pub base_version: i64,
    pub content: ScriptContent,
    pub request: Vec<u8>,
}

fn valid_label(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.contains('\0')
}

pub(crate) fn prepare_create(raw: CreateRequest) -> Result<NewScript, EditError> {
    if raw.operation_id.is_nil()
        || !valid_label(&raw.source.reference, 2048)
        || !valid_label(&raw.source.original_text, 1_048_576)
        || !valid_label(&raw.rights.holder, 512)
        || !valid_label(&raw.rights.evidence_reference, 2048)
    {
        return Err(EditError::InvalidRequest);
    }
    let content = script_ir::parse(raw.content).map_err(EditError::InvalidScript)?;
    if raw.source.id != content.source_ref() || raw.rights.id != content.rights_ref() {
        return Err(EditError::IdentityChanged);
    }
    let request = fingerprint(&serde_json::json!({
        "content": content.as_json(), "source": raw.source, "rights": raw.rights
    }))?;
    Ok(NewScript {
        operation_id: raw.operation_id,
        content,
        source: raw.source,
        rights: raw.rights,
        request,
    })
}

pub(crate) fn prepare_save(raw: SaveRequest) -> Result<Save, EditError> {
    if raw.operation_id.is_nil() || raw.base_version < 1 || raw.base_version == i64::MAX {
        return Err(EditError::InvalidRequest);
    }
    let content = script_ir::parse(raw.content).map_err(EditError::InvalidScript)?;
    let request = fingerprint(&serde_json::json!({
        "base_version": raw.base_version, "content": content.as_json()
    }))?;
    Ok(Save {
        operation_id: raw.operation_id,
        base_version: raw.base_version,
        content,
        request,
    })
}

pub(crate) fn fingerprint(value: &Value) -> Result<Vec<u8>, EditError> {
    let bytes = serde_json::to_vec(value).map_err(|_| EditError::InvalidRequest)?;
    Ok(Sha256::digest(bytes).to_vec())
}

/// Retry lookup precedes the stale-base decision, including after later edits.
pub(crate) fn decide_save(
    head: i64,
    base: i64,
    request: &[u8],
    prior: Option<(i64, &[u8])>,
) -> Result<i64, EditError> {
    if let Some((version, previous_request)) = prior {
        return if previous_request == request {
            Ok(version)
        } else {
            Err(EditError::OperationReused)
        };
    }
    require_head(head, base)?;
    base.checked_add(1).ok_or(EditError::InvalidRequest)
}

pub(crate) fn require_head(head: i64, version: i64) -> Result<(), EditError> {
    if head != version {
        return Err(EditError::Conflict { head_version: head });
    }
    Ok(())
}

pub(crate) fn require_identity(
    content: &ScriptContent,
    previous: &ScriptContent,
) -> Result<(), EditError> {
    if content.work_id() != previous.work_id()
        || content.adaptation_id() != previous.adaptation_id()
        || content.episode_id() != previous.episode_id()
        || content.source_ref() != previous.source_ref()
        || content.rights_ref() != previous.rights_ref()
    {
        return Err(EditError::IdentityChanged);
    }
    Ok(())
}

pub(crate) fn verify_stored(
    value: Value,
    canonical: &[u8],
    digest: &str,
    wire_digest: &[u8],
) -> Result<ScriptContent, EditError> {
    let stored_wire_digest = fingerprint(&value).map_err(|_| EditError::Corrupt)?;
    let content = script_ir::parse(value).map_err(|_| EditError::Corrupt)?;
    if content.canonical_bytes() != canonical
        || content.digest() != digest
        || stored_wire_digest != wire_digest
    {
        return Err(EditError::Corrupt);
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::{EditError, decide_save, require_head};

    #[test]
    fn stored_wire_tampering_cannot_hide_behind_semantic_normalization() {
        let fixture = serde_json::from_str(include_str!(
            "../../../contracts/script-ir/fixtures/accepted/episode.json"
        ))
        .unwrap();
        let content = crate::script_ir::parse(fixture).unwrap();
        let canonical = content.canonical_bytes();
        let digest = content.digest();
        let wire_digest = super::fingerprint(&content.as_json()).unwrap();
        assert!(super::verify_stored(content.as_json(), &canonical, &digest, &wire_digest).is_ok());
        let mut changed = content.as_json();
        let text = changed["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"]
            .as_str()
            .unwrap()
            .to_owned();
        changed["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] =
            serde_json::json!(format!("  {text}  "));
        assert_eq!(
            crate::script_ir::parse(changed.clone()).unwrap().digest(),
            digest
        );
        assert!(matches!(
            super::verify_stored(changed, &canonical, &digest, &wire_digest),
            Err(EditError::Corrupt)
        ));
        let mut changed = content.as_json();
        changed["work"]["rights_ref"] = serde_json::json!("different-rights");
        assert_eq!(
            crate::script_ir::parse(changed.clone()).unwrap().digest(),
            digest
        );
        assert!(matches!(
            super::verify_stored(changed, &canonical, &digest, &wire_digest),
            Err(EditError::Corrupt)
        ));
    }

    #[test]
    fn retry_returns_original_version_after_head_advances() {
        assert_eq!(decide_save(4, 1, b"same", Some((2, b"same"))).unwrap(), 2);
    }

    #[test]
    fn same_operation_with_changed_request_is_rejected_before_base_check() {
        assert!(matches!(
            decide_save(4, 1, b"new", Some((2, b"old"))),
            Err(EditError::OperationReused)
        ));
    }

    #[test]
    fn stale_save_and_accept_report_exact_current_head() {
        assert!(matches!(
            decide_save(2, 1, b"new", None),
            Err(EditError::Conflict { head_version: 2 })
        ));
        assert!(matches!(
            require_head(2, 1),
            Err(EditError::Conflict { head_version: 2 })
        ));
        assert_eq!(decide_save(2, 2, b"new", None).unwrap(), 3);
    }
}
