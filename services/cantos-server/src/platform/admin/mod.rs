//! Independent Admin capability: verify → current role observation → local policy → atomic audit.
mod credential;
mod domain;
mod postgres;
mod provider;
use crate::platform::store::Store;
use cantos_contracts::admin::{CapabilityResponse, RevokeAccessResponse};
pub use domain::{Action, AdminError, Role};
pub use provider::AdminConfig;
use provider::AdminProvider;
use uuid::Uuid;

pub const CLIENT_ID: &str = "cantos-admin-api";
pub struct AdminService {
    provider: AdminProvider,
}
impl AdminService {
    pub fn new(config: AdminConfig) -> Result<Self, AdminError> {
        Ok(Self {
            provider: AdminProvider::new(config)?,
        })
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
        postgres::capabilities(store, proof, correlation).await
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
                postgres::deny(
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
        postgres::revoke(store, proof, target, operation, correlation).await
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
            Some(t) => self.provider.authenticate(t).await,
            None => Err(AdminError::Credential),
        };
        match result {
            Ok(proof) => Ok(proof),
            Err(error) => {
                postgres::deny(
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
pub(super) fn now() -> Result<u64, AdminError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|n| n.as_secs())
        .map_err(|_| AdminError::Unavailable)
}
#[cfg(test)]
mod credential_tests;
#[cfg(test)]
mod tests;
