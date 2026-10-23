//! Explicit review consent is tied to every immutable subject and submitted evidence field.
use crate::contract::{AudioReviewDecision, AudioReviewKind, CreateAudioReviewRequest};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewConsent {
    pub unit: String,
    pub submission: String,
    pub rendition_digest: String,
    pub sha256: String,
    pub kind: AudioReviewKind,
    pub decision: AudioReviewDecision,
    pub notes: String,
    pub evidence_refs: Vec<String>,
}
impl ReviewConsent {
    pub fn request(&self, operation_id: String) -> CreateAudioReviewRequest {
        CreateAudioReviewRequest {
            operation_id,
            rendition_digest: self.rendition_digest.clone(),
            sha256: self.sha256.clone(),
            kind: self.kind,
            decision: self.decision,
            notes: self.notes.clone(),
            evidence_refs: self.evidence_refs.clone(),
        }
    }
    pub fn authorizes(&self, current: &Self) -> bool {
        self == current
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consent_for_a_never_authorizes_b_or_changed_review_evidence() {
        let original = ReviewConsent {
            unit: "unit".into(),
            submission: "a".into(),
            rendition_digest: "digest-a".into(),
            sha256: "hash-a".into(),
            kind: AudioReviewKind::Qc,
            decision: AudioReviewDecision::Approved,
            notes: "Named listening".into(),
            evidence_refs: vec!["record".into()],
        };
        let mut changes = vec![original.clone(); 8];
        changes[0].unit = "other".into();
        changes[1].submission = "b".into();
        changes[2].rendition_digest = "digest-b".into();
        changes[3].sha256 = "hash-b".into();
        changes[4].kind = AudioReviewKind::Rights;
        changes[5].decision = AudioReviewDecision::Withdrawn;
        changes[6].notes = "changed".into();
        changes[7].evidence_refs = vec![];
        assert!(original.authorizes(&original));
        for changed in changes {
            assert!(!original.authorizes(&changed));
        }
    }
}
