//! Projection into the existing versioned revision document, shared by HTTP and frozen
//! Adaptation/Production records. Field order and exact canonical text are compatibility data.
use crate::contract::studio::RevisionResponse;

use super::facts::RevisionFacts;

pub(crate) fn revision(facts: RevisionFacts) -> RevisionResponse {
    RevisionResponse {
        script_id: facts.script_id,
        revision: facts.revision,
        accepted_by: facts.accepted_by,
        accepted_at: facts.accepted_at,
        content_digest: facts.content_digest,
        export_digest: facts.export_digest,
        script_json: facts.canonical_export,
    }
}

/// Shape-only mapping: classification and redaction belong to the pure diagnostic owner.
pub(crate) fn issues(
    diagnostics: Vec<super::domain::diagnostics::SafeDiagnostic>,
) -> Vec<crate::contract::studio::FieldIssue> {
    diagnostics
        .into_iter()
        .map(|diagnostic| crate::contract::studio::FieldIssue {
            path: diagnostic.path,
            rule: diagnostic.rule.into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn revision_document_keeps_fixture_bytes_digests_and_timestamp_precision() {
        let fixture: RevisionResponse = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/editorial/v1/fixtures/revision.json"
        ))
        .unwrap();
        let projected = revision(RevisionFacts {
            script_id: fixture.script_id.clone(),
            revision: fixture.revision,
            accepted_by: fixture.accepted_by.clone(),
            accepted_at: fixture.accepted_at.clone(),
            content_digest: fixture.content_digest.clone(),
            export_digest: fixture.export_digest.clone(),
            canonical_export: fixture.script_json.clone(),
        });
        assert_eq!(projected, fixture);
        // Independent Python stdlib serialization/hash of the pre-existing fixture field order.
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&projected).unwrap())
            ),
            "f2e7e69f1045f0fbbe85a153a72272480f971dc25c055aa9d289bbedbc87c5ad"
        );
    }
}
