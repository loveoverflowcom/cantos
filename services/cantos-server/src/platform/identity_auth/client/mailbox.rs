//! Private mailbox facts are request/subject/security-bound and short lived, never sessions.
use super::{timed, IdentityClient, RpcFault, STATE_DEADLINE};
use crate::platform::identity_auth::{
    decision::ObservedSecurity,
    rpc::{CanonicalizeMailboxRequest, GetVerifiedMailboxRequest, VerifiedMailboxFact},
};

pub(crate) struct ValidatedMailbox {
    email: String,
    observed_at_us: i64,
    security: ObservedSecurity,
}
impl std::fmt::Debug for ValidatedMailbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ValidatedMailbox([REDACTED])")
    }
}
impl ValidatedMailbox {
    pub(crate) fn email(&self) -> &str {
        &self.email
    }
    pub(crate) fn valid_at(&self, security: &ObservedSecurity, now: i64) -> bool {
        &self.security == security
            && now >= 0
            && self.observed_at_us >= 0
            && now
                .checked_sub(self.observed_at_us)
                .is_some_and(|age| (-5_000_000..=15_000_000).contains(&age))
    }
}
// Shape sanity only. Identity alone owns mailbox syntax and canonicalization.
fn mailbox_shape(email: &str) -> bool {
    (3..=254).contains(&email.len())
        && email.is_ascii()
        && !email.bytes().any(|b| b.is_ascii_whitespace())
        && email.contains('@')
}
fn validate(
    response: VerifiedMailboxFact,
    operation: [u8; 16],
    security: &ObservedSecurity,
) -> Result<ValidatedMailbox, RpcFault> {
    if operation == [0; 16]
        || response.operation_id != operation
        || response.subject_id != security.subject.bytes()
        || response.instance_epoch != security.epoch.bytes()
        || response.security_version != security.version
        || !mailbox_shape(&response.canonical_email)
        || response.observed_at_unix_micros < 0
    {
        return Err(RpcFault::Protocol);
    }
    Ok(ValidatedMailbox {
        email: response.canonical_email,
        observed_at_us: response.observed_at_unix_micros,
        security: *security,
    })
}
impl IdentityClient {
    pub(crate) async fn canonical_mailbox(
        &self,
        operation: [u8; 16],
        email: &str,
    ) -> Result<String, RpcFault> {
        if operation == [0; 16] || email.len() > 254 {
            return Err(RpcFault::InvalidArgument);
        }
        let request = CanonicalizeMailboxRequest {
            operation_id: operation.to_vec(),
            email: email.to_owned(),
        };
        let response = self
            .call(STATE_DEADLINE, |mut rpc| async move {
                rpc.canonicalize_mailbox(timed(request, STATE_DEADLINE))
                    .await
            })
            .await?;
        if response.operation_id != operation || !mailbox_shape(&response.canonical_email) {
            return Err(RpcFault::Protocol);
        }
        Ok(response.canonical_email)
    }
    pub(crate) async fn verified_mailbox(
        &self,
        operation: [u8; 16],
        security: &ObservedSecurity,
    ) -> Result<ValidatedMailbox, RpcFault> {
        let request = GetVerifiedMailboxRequest {
            operation_id: operation.to_vec(),
            subject_id: security.subject.bytes().to_vec(),
            security_version: security.version,
            instance_epoch: security.epoch.bytes().to_vec(),
        };
        let response = self
            .call(STATE_DEADLINE, |mut rpc| async move {
                rpc.get_verified_mailbox(timed(request, STATE_DEADLINE))
                    .await
            })
            .await?;
        validate(response, operation, security)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::identity_auth::decision::{Epoch, Subject};
    #[test]
    fn mailbox_fact_binds_every_request_dimension_and_freshness() {
        let security = ObservedSecurity {
            subject: Subject::from_bytes([1; 16]).unwrap(),
            epoch: Epoch::from_bytes([2; 16]).unwrap(),
            version: 3,
        };
        let fact = VerifiedMailboxFact {
            operation_id: vec![4; 16],
            subject_id: vec![1; 16],
            instance_epoch: vec![2; 16],
            security_version: 3,
            canonical_email: "A+tag@example.test".to_owned(),
            observed_at_unix_micros: 10_000_000,
        };
        for dimension in 0..6 {
            let mut wrong = fact.clone();
            match dimension {
                0 => wrong.operation_id = vec![5; 16],
                1 => wrong.subject_id = vec![5; 16],
                2 => wrong.instance_epoch = vec![5; 16],
                3 => wrong.security_version = 4,
                4 => wrong.canonical_email = "forged".to_owned(),
                _ => wrong.observed_at_unix_micros = -1,
            };
            assert!(matches!(
                validate(wrong, [4; 16], &security),
                Err(RpcFault::Protocol)
            ));
        }
        let admitted = validate(fact, [4; 16], &security).unwrap();
        assert!(admitted.valid_at(&security, 25_000_000));
        assert!(!admitted.valid_at(&security, 25_000_001));
        assert!(!admitted.valid_at(&security, 4_999_999));
        assert!(!admitted.valid_at(
            &ObservedSecurity {
                version: 4,
                ..security
            },
            10_000_000
        ));
        assert!(!format!("{admitted:?}").contains("example.test"));
    }
}
