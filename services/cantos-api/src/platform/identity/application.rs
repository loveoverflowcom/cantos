use super::{
    domain::{ActorId, ActorRecord, ExternalIdentityKey, LinkOutcome, Registration},
    IdentityRegistry, RegistryError,
};

/// Explicit registry management; deliberately produces no trusted authentication context.
/// PR2 must verify credentials before using a mapping, and authorize any account linking.
pub struct IdentityService<R> {
    registry: R,
}

impl<R: IdentityRegistry> IdentityService<R> {
    pub fn new(registry: R) -> Self {
        Self { registry }
    }

    pub async fn lookup(
        &self,
        key: &ExternalIdentityKey,
    ) -> Result<Option<ActorRecord>, RegistryError> {
        self.registry.lookup(key).await
    }

    pub async fn register(
        &self,
        key: &ExternalIdentityKey,
        candidate: &ActorId,
    ) -> Result<Registration, RegistryError> {
        self.registry.register(key, candidate).await
    }

    pub async fn link(
        &self,
        key: &ExternalIdentityKey,
        target: &ActorId,
    ) -> Result<LinkOutcome, RegistryError> {
        self.registry.link(key, target).await
    }
}
