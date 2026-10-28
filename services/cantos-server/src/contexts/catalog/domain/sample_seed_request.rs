//! Validation of an offline operator's request, not authentication or publication authority.
//!
//! This request names the entire pinned sample; it cannot select another owner, source or Work.
//! The operator reference is self-declared and must not be described as a Studio identity.

use super::metadata_disclosure::{APPROVED_SAMPLE_SOURCE_SHA256, FICTIONAL_SAMPLE_SOURCE_SHA256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleSeedAction {
    Publish,
    Withdraw,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SampleSeedRequest {
    pub operation_id: String,
    pub action: SampleSeedAction,
    pub source_sha256: String,
    pub operator_reference: String,
    pub evidence_reference: String,
    pub rights_statement: String,
    pub confirm_source: bool,
    pub confirm_metadata_rights: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SampleSeedRequestError {
    InvalidOperation,
    WrongSource,
    InvalidReference,
    RightsNotConfirmed,
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}

impl SampleSeedRequest {
    /// Only validates the declared scope and attestation. The application must separately
    /// verify the source bytes, fresh principal, durable evidence and atomic transaction.
    pub fn validate(&self) -> Result<(), SampleSeedRequestError> {
        if !uuid::Uuid::parse_str(&self.operation_id)
            .is_ok_and(|id| id.to_string() == self.operation_id)
        {
            return Err(SampleSeedRequestError::InvalidOperation);
        }
        if ![
            APPROVED_SAMPLE_SOURCE_SHA256,
            FICTIONAL_SAMPLE_SOURCE_SHA256,
        ]
        .contains(&self.source_sha256.as_str())
        {
            return Err(SampleSeedRequestError::WrongSource);
        }
        if !valid_text(&self.operator_reference)
            || !valid_text(&self.evidence_reference)
            || !valid_text(&self.rights_statement)
        {
            return Err(SampleSeedRequestError::InvalidReference);
        }
        if self.action == SampleSeedAction::Publish
            && (!self.confirm_source || !self.confirm_metadata_rights)
        {
            return Err(SampleSeedRequestError::RightsNotConfirmed);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SampleSeedRequest {
        SampleSeedRequest {
            operation_id: "00000000-0000-4000-8000-000000000001".into(),
            action: SampleSeedAction::Publish,
            source_sha256: APPROVED_SAMPLE_SOURCE_SHA256.into(),
            operator_reference: "test operator".into(),
            evidence_reference: "test evidence".into(),
            rights_statement: "test statement".into(),
            confirm_source: true,
            confirm_metadata_rights: true,
        }
    }

    #[test]
    fn publish_requires_both_explicit_attestations_and_exact_scope() {
        assert_eq!(request().validate(), Ok(()));
        let mut input = request();
        input.confirm_source = false;
        assert_eq!(
            input.validate(),
            Err(SampleSeedRequestError::RightsNotConfirmed)
        );
        input.confirm_source = true;
        input.confirm_metadata_rights = false;
        assert_eq!(
            input.validate(),
            Err(SampleSeedRequestError::RightsNotConfirmed)
        );
        input.source_sha256 = "unrelated".into();
        assert_eq!(input.validate(), Err(SampleSeedRequestError::WrongSource));
    }

    #[test]
    fn withdrawal_does_not_require_a_new_rights_claim_but_keeps_audit_references() {
        let mut input = request();
        input.action = SampleSeedAction::Withdraw;
        input.confirm_source = false;
        input.confirm_metadata_rights = false;
        assert_eq!(input.validate(), Ok(()));
        input.operator_reference = " \n".into();
        assert_eq!(
            input.validate(),
            Err(SampleSeedRequestError::InvalidReference)
        );
        input.operator_reference = "operator".into();
        input.operation_id = "not-a-uuid".into();
        assert_eq!(
            input.validate(),
            Err(SampleSeedRequestError::InvalidOperation)
        );
    }

    #[test]
    fn input_cannot_choose_an_owner_or_expand_the_scope() {
        let input = r#"{"operation_id":"00000000-0000-4000-8000-000000000001","action":"publish","source_sha256":"d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212","operator_reference":"x","evidence_reference":"x","rights_statement":"x","confirm_source":true,"confirm_metadata_rights":true,"owner_id":"someone_else"}"#;
        assert!(serde_json::from_str::<SampleSeedRequest>(input).is_err());
    }
}
