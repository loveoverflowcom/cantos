//! Operator admission is separate from credential verification and resource permissions.
use std::collections::BTreeSet;

use crate::platform::{identity::domain::ExternalIdentityKey, oidc::core::AuthError};

pub(super) struct StudioAdministrators {
    identities: BTreeSet<ExternalIdentityKey>,
}

impl StudioAdministrators {
    pub(super) fn new(issuer: &str, subjects: Vec<String>) -> Result<Self, AuthError> {
        if !(1..=2).contains(&subjects.len()) {
            return Err(AuthError::Configuration);
        }
        let count = subjects.len();
        let identities = subjects
            .into_iter()
            .map(|subject| {
                ExternalIdentityKey::new(issuer, subject).map_err(|_| AuthError::Configuration)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if identities.len() != count {
            return Err(AuthError::Configuration);
        }
        Ok(Self { identities })
    }

    pub(super) fn authorize(&self, identity: &ExternalIdentityKey) -> Result<(), AuthError> {
        if self.identities.contains(identity) {
            Ok(())
        } else {
            Err(AuthError::Inactive)
        }
    }
}
