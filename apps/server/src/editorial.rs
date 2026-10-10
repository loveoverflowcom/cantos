//! Pure editorial decisions and validated commands; no HTTP or database dependencies.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::script_ir::{self, ReadError, ScriptContent};

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("invalid request")]
    InvalidRequest,
    #[error("invalid script")]
    InvalidScript(ReadError),
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
    let content = read_content(&raw.content)?;
    if raw.source.id != content.work_source_record_id()
        || raw.rights.id != content.work_rights_record_id()
    {
        return Err(EditError::IdentityChanged);
    }
    let request = fingerprint(&serde_json::json!({
        "content": document(&content)?, "source": raw.source, "rights": raw.rights
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
    let content = read_content(&raw.content)?;
    let request = fingerprint(&serde_json::json!({
        "base_version": raw.base_version, "content": document(&content)?
    }))?;
    Ok(Save {
        operation_id: raw.operation_id,
        base_version: raw.base_version,
        content,
        request,
    })
}

/// Admit untrusted request content through the Script IR reader.
fn read_content(value: &Value) -> Result<ScriptContent, EditError> {
    let bytes = serde_json::to_vec(value).map_err(|_| EditError::InvalidRequest)?;
    script_ir::read_script(&bytes).map_err(EditError::InvalidScript)
}

/// The canonical versioned document, as a detached JSON value for responses.
pub(crate) fn document(content: &ScriptContent) -> Result<Value, EditError> {
    serde_json::from_slice(&content.export_bytes()).map_err(|_| EditError::Corrupt)
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
        || content.work_source_record_id() != previous.work_source_record_id()
        || content.work_rights_record_id() != previous.work_rights_record_id()
    {
        return Err(EditError::IdentityChanged);
    }
    Ok(())
}

/// The SHA-256 of the exact stored document, covering references the content digest omits.
pub(crate) fn document_digest(document: &[u8]) -> Vec<u8> {
    Sha256::digest(document).to_vec()
}

/// Re-admit a stored version without normalizing it; any divergence is corruption.
pub(crate) fn verify_stored(
    document: &[u8],
    canonical: &[u8],
    digest: &str,
    stored_document_digest: &[u8],
) -> Result<ScriptContent, EditError> {
    if document_digest(document) != stored_document_digest {
        return Err(EditError::Corrupt);
    }
    let content = script_ir::read_canonical_script(document).map_err(|_| EditError::Corrupt)?;
    if content.canonical_bytes() != canonical || content.content_digest().to_string() != digest {
        return Err(EditError::Corrupt);
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::{EditError, decide_save, require_head};

    fn stored(document: &[u8]) -> (Vec<u8>, String, Vec<u8>) {
        let content = crate::script_ir::read_canonical_script(document).unwrap();
        (
            content.canonical_bytes(),
            content.content_digest().to_string(),
            super::document_digest(document),
        )
    }

    #[test]
    fn stored_document_must_stay_canonical_and_match_every_recorded_digest() {
        let script = crate::script_ir::read_script(include_bytes!(
            "../../../contracts/fixtures/script-ir/0.1.0/accept/provenance.json"
        ))
        .unwrap();
        let document = script.export_bytes();
        let (canonical, digest, document_digest) = stored(&document);
        assert!(super::verify_stored(&document, &canonical, &digest, &document_digest).is_ok());

        // Equivalent but non-canonical bytes are corruption, never silently normalized.
        let mut padded = document.clone();
        padded.push(b'\n');
        assert!(matches!(
            super::verify_stored(
                &padded,
                &canonical,
                &digest,
                &super::document_digest(&padded)
            ),
            Err(EditError::Corrupt)
        ));
    }

    #[test]
    fn changed_rights_reference_is_caught_although_the_content_digest_is_unchanged() {
        let script = crate::script_ir::read_script(include_bytes!(
            "../../../contracts/fixtures/script-ir/0.1.0/accept/provenance.json"
        ))
        .unwrap();
        let document = script.export_bytes();
        let (canonical, digest, document_digest) = stored(&document);
        let rights = script.work_rights_record_id();
        let changed = String::from_utf8(document.clone())
            .unwrap()
            .replacen(
                &format!("\"rights_record_id\":\"{rights}\",\"source_ref\""),
                "\"rights_record_id\":\"different-rights\",\"source_ref\"",
                1,
            )
            .into_bytes();
        assert_ne!(changed, document);
        assert_eq!(stored(&changed).1, digest);
        assert!(matches!(
            super::verify_stored(&changed, &canonical, &digest, &document_digest),
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
