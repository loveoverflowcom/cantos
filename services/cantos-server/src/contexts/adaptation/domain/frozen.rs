//! Immutable frozen inputs of an adaptation and the typed digests that identify them.
//!
//! Caller-owned contexts bind the exact source, base revision and server-minted evidence IDs.
//! These snapshots are not authorization witnesses; transaction-time source/base/access checks
//! remain required.
use crate::contract::studio::{AdaptationContextRequest, ImportResponse, RevisionResponse};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::TrustedBinding;

pub const INPUT_C1_DIGEST: &[u8] = b"cantos/adaptation-input/c1\n";
pub const PROPOSAL_DIGEST: &[u8] = b"cantos/adaptation-proposal/p1\n";
pub const SUBMISSION_DIGEST: &[u8] = b"cantos/adaptation-submission/s1\n";

/// Hex SHA-256 of the typed serialization under a domain label, never JSONB's key order.
pub fn typed_digest<T: Serialize>(label: &[u8], value: &T) -> Result<String, serde_json::Error> {
    let mut digest = Sha256::new();
    digest.update(label);
    digest.update(serde_json::to_vec(value)?);
    Ok(format!("{:x}", digest.finalize()))
}

/// The evidence record identities minted for one run: derived from the run ID, never chosen.
pub fn generation_record_id(run: &str) -> String {
    format!("gen_{}", run.replace('-', ""))
}

pub fn rights_record_id(run: &str) -> String {
    format!("rights_{}", run.replace('-', ""))
}

fn binding(
    source: &ImportResponse,
    base: Option<&RevisionResponse>,
    generation_record_id: &str,
    rights_record_id: &str,
    run: &str,
) -> TrustedBinding {
    TrustedBinding {
        source_id: source.id.clone(),
        source_sha256: source.sha256.clone(),
        generation_record_id: generation_record_id.into(),
        rights_record_id: rights_record_id.into(),
        id_namespace: run.replace('-', ""),
        base_script_json: base.map(|base| base.script_json.clone()),
    }
}

/// Caller-owned context: no provider execution configuration or attestation.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenContext {
    pub request: AdaptationContextRequest,
    pub source: ImportResponse,
    pub base: Option<RevisionResponse>,
    pub context_version: String,
    pub prompt_version: String,
    pub contract_version: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub proposal_schema_json: String,
    pub generation_record_id: String,
    pub rights_record_id: String,
}

impl FrozenContext {
    pub fn binding(&self, run: &str) -> TrustedBinding {
        binding(
            &self.source,
            self.base.as_ref(),
            &self.generation_record_id,
            &self.rights_record_id,
            run,
        )
    }
}
