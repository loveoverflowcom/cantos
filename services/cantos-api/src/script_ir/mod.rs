//! Versioned Script IR: a pure validator and immutable validated content.

mod canonical;
mod diff;
mod model;
mod validation;
mod wire;

pub use diff::{
    diff_scripts, ScopeDifference, ScopeLevel, ScopeMismatch, ScriptDiff, DIFF_REPORT_VERSION,
};
pub use model::{ContentDigest, ScriptContent, SpokenLine};
pub use validation::{Diagnostic, ValidationIssue};
pub use wire::{ReadError, ShapeDiagnostic, ShapeRule, MAX_DOCUMENT_BYTES, WRITE_VERSION};

/// Admit an untrusted structured draft, normalizing derived text once.
/// This does not grant rights, save a revision, approve or start production.
pub fn read_script(bytes: &[u8]) -> Result<ScriptContent, ReadError> {
    let raw = wire::decode(bytes)?;
    let script = validation::validate(raw).map_err(ReadError::Semantic)?;
    // NFC may expand a document even when each normalized field remains within its bound.
    if script.export_bytes().len() > wire::MAX_DOCUMENT_BYTES {
        return Err(ReadError::DocumentTooLarge {
            max_bytes: wire::MAX_DOCUMENT_BYTES,
        });
    }
    Ok(script)
}

/// Check an immutable stored/exported value without silently normalizing it.
/// The persistence adapter must also verify the stored digest and revision ID.
pub fn read_canonical_script(bytes: &[u8]) -> Result<ScriptContent, ReadError> {
    let raw = wire::decode(bytes)?;
    let script = validation::validate(raw).map_err(ReadError::Semantic)?;
    // Equality to the already bounded input proves both canonicality and the export byte bound.
    if script.export_bytes() != bytes {
        return Err(ReadError::NonCanonicalDocument);
    }
    Ok(script)
}
