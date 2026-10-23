//! Independent Admin capability: verify -> current role observation -> local policy -> atomic audit.
//!
//! Verification is a port ([`AdminVerifier`]) with **no composed implementation**: the former
//! Kanidm/OIDC introspection adapter is retired and no native Admin credential exists yet. With
//! no verifier in the [`Store`], every Admin route answers the route group's unavailable error.
//! An Identity v1 password session, a Studio or a Theatre credential is never Admin authority.
mod domain;
mod turso;
use crate::contract::admin::{CapabilityResponse, RevokeAccessResponse};
use crate::platform::store::Store;
pub use domain::{map_groups, Action, AdminError, Role, RoleEvidence};
use std::{future::Future, pin::Pin, sync::Arc};
use uuid::Uuid;

pub const CLIENT_ID: &str = "cantos-admin-api";

pub type VerifyFuture<'a> =
    Pin<Box<dyn Future<Output = Result<RoleEvidence, AdminError>> + Send + 'a>>;

/// Turns one presented Admin credential into current, request-local [`RoleEvidence`], or refuses.
/// Implementations must bind the credential to the dedicated Admin purpose and observe the
/// role at verification time; they never mint evidence from a Studio/Theatre/Identity session.
pub trait AdminVerifier: Send + Sync {
    fn verify<'a>(&'a self, token: &'a str) -> VerifyFuture<'a>;
}

pub struct AdminService {
    verifier: Arc<dyn AdminVerifier>,
}
impl AdminService {
    pub fn new(verifier: Arc<dyn AdminVerifier>) -> Self {
        Self { verifier }
    }
    pub async fn capabilities(
        &self,
        store: &Store,
        token: Option<&str>,
        correlation: Uuid,
    ) -> Result<CapabilityResponse, AdminError> {
        let proof = self
            .authenticate(store, token, Action::Capabilities, None, correlation)
            .await?;
        turso::capabilities(store, proof, correlation).await
    }
    pub async fn revoke_access(
        &self,
        store: &Store,
        token: Option<&str>,
        target: &str,
        operation: &str,
        correlation: Uuid,
    ) -> Result<RevokeAccessResponse, AdminError> {
        use crate::platform::identity::domain::ActorId;
        let target = ActorId::new(target).map_err(|_| AdminError::InvalidRequest);
        let operation = Uuid::parse_str(operation).map_err(|_| AdminError::InvalidRequest);
        let (target, operation) = match (target, operation) {
            (Ok(t), Ok(o)) if !o.is_nil() => (t, o),
            _ => {
                turso::deny(
                    store,
                    None,
                    Action::RevokeAccess,
                    None,
                    "denied",
                    correlation,
                )
                .await?;
                return Err(AdminError::InvalidRequest);
            }
        };
        let proof = self
            .authenticate(
                store,
                token,
                Action::RevokeAccess,
                Some(&target),
                correlation,
            )
            .await?;
        turso::revoke(store, proof, target, operation, correlation).await
    }
    async fn authenticate(
        &self,
        store: &Store,
        token: Option<&str>,
        action: Action,
        target: Option<&crate::platform::identity::domain::ActorId>,
        correlation: Uuid,
    ) -> Result<domain::RoleEvidence, AdminError> {
        let result = match token {
            Some(t) => self.verifier.verify(t).await,
            None => Err(AdminError::Credential),
        };
        match result {
            Ok(proof) => Ok(proof),
            Err(error) => {
                turso::deny(
                    store,
                    None,
                    action,
                    target,
                    error.audit_outcome(),
                    correlation,
                )
                .await?;
                Err(error)
            }
        }
    }
}
impl Store {
    pub fn with_admin(mut self, service: AdminService) -> Self {
        self.admin = Some(std::sync::Arc::new(service));
        self
    }
    pub fn admin(&self) -> Option<&AdminService> {
        self.admin.as_deref()
    }
}
#[cfg(test)]
mod tests;
