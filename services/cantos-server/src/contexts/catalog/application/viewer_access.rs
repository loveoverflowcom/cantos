//! S06 private control plane. RPCs finish before the writer; owner/session admission repeats
//! inside settlement. No method here enables restricted Theatre metadata or bytes.
use super::super::{
    domain::{
        viewer_access::{AccessError, GroupId},
        WorkId,
    },
    turso::viewer_access as db,
};
use super::owner_work::{owner_work, recheck_owner_work};
use crate::contract::theatre::viewer_access::*;
use crate::{
    contract::studio::*,
    platform::{
        identity::domain::ActorId,
        identity_auth::{
            credential::{access_channel, Channel, Surface},
            turso::admit_in,
        },
        store::{Error, Store, StoreError},
    },
};

#[derive(Debug)]
pub enum ViewerAccessError {
    Storage(StoreError),
    Access(AccessError),
}
impl From<StoreError> for ViewerAccessError {
    fn from(e: StoreError) -> Self {
        Self::Storage(e)
    }
}
impl From<Error> for ViewerAccessError {
    fn from(e: Error) -> Self {
        Self::Storage(e.into())
    }
}
impl From<AccessError> for ViewerAccessError {
    fn from(e: AccessError) -> Self {
        Self::Access(e)
    }
}
impl From<super::CatalogStoreError> for ViewerAccessError {
    fn from(e: super::CatalogStoreError) -> Self {
        match e {
            super::CatalogStoreError::Storage(e) => Self::Storage(e),
            _ => Self::Access(AccessError::Invalid),
        }
    }
}
pub(crate) type Result<T> = std::result::Result<T, ViewerAccessError>;
pub(crate) fn operation(id: &str) -> Result<[u8; 16]> {
    let uuid = uuid::Uuid::parse_str(id).map_err(|_| AccessError::Invalid)?;
    if uuid.is_nil() || uuid.to_string() != id {
        return Err(AccessError::Invalid.into());
    }
    Ok(*uuid.as_bytes())
}
fn auth_error(e: crate::platform::identity_auth::AuthFailure) -> ViewerAccessError {
    use crate::platform::identity_auth::AuthFailure;
    match e {
        AuthFailure::Unauthenticated => StoreError::Unauthenticated.into(),
        AuthFailure::InvalidRequest => AccessError::Invalid.into(),
        _ => AccessError::Unavailable.into(),
    }
}
impl Store {
    pub async fn viewer_access_state(&self, token: &str, work: &str) -> Result<ViewerAccessState> {
        let work = WorkId::parse(work).map_err(|_| AccessError::Invalid)?;
        self.authenticate(token).await?;
        let conn = self.connection().await?;
        let tx = conn.read().await?;
        let owner = owner_work(&tx, token, &work).await?;
        let state = db::state(&tx, work.as_str()).await?;
        recheck_owner_work(&tx, token, &owner).await?;
        tx.commit().await?;
        Ok(state)
    }
    pub async fn viewer_access_execute(
        &self,
        token: &str,
        work: &str,
        mut request: ViewerAccessRequest,
    ) -> Result<ViewerAccessReceipt> {
        let operation_id = operation(&request.operation_id)?;
        let work = WorkId::parse(work).map_err(|_| AccessError::Invalid)?;
        self.authenticate(token).await?;
        // Fingerprint the canonical admitted request; never store raw owner text in diagnostics.
        if let ViewerAccessChange::ShareMailbox { email, .. } = &mut request.change {
            *email = self
                .identity_auth()
                .ok_or(AccessError::Unavailable)?
                .canonical_mailbox(operation_id, email)
                .await
                .map_err(auth_error)?;
        }
        let mut conn = self.connection().await?;
        let tx = conn.transaction().await?;
        let owner = owner_work(&tx, token, &work).await?;
        let mut rights = true;
        if matches!(
            request.change,
            ViewerAccessChange::SetAudience {
                audience: ViewerAudience::Public,
                ..
            }
        ) {
            if let super::super::domain::metadata_publication::PublicationState::Published(
                published,
            ) = super::super::turso::metadata_publication::state(&tx, work.as_str()).await?
            {
                match super::metadata_publication::recheck_work_pin(
                    &tx,
                    &owner,
                    published.generation().work(),
                )
                .await
                {
                    Ok(()) => {}
                    Err(super::CatalogStoreError::Publication(_)) => rights = false,
                    Err(error) => return Err(error.into()),
                }
            }
        }
        let receipt = db::policy_command(&tx, &owner, &request, rights).await?;
        recheck_owner_work(&tx, token, &owner).await?;
        tx.commit().await?;
        Ok(receipt)
    }
    pub async fn viewer_group_execute(
        &self,
        token: &str,
        group: &str,
        request: ViewerGroupRequest,
    ) -> Result<ViewerGroupReceipt> {
        operation(&request.operation_id)?;
        GroupId::parse(group)?;
        self.authenticate(token).await?;
        let mut conn = self.connection().await?;
        let tx = conn.transaction().await?;
        let actor = super::current_actor(&tx, token).await?;
        let receipt = db::group_command(&tx, &actor, group, &request).await?;
        if super::current_actor(&tx, token).await? != actor {
            return Err(StoreError::Unauthenticated.into());
        }
        tx.commit().await?;
        Ok(receipt)
    }
    pub async fn viewer_groups(&self, token: &str) -> Result<ViewerGroups> {
        self.authenticate(token).await?;
        let conn = self.connection().await?;
        let tx = conn.read().await?;
        let actor = super::current_actor(&tx, token).await?;
        let result = db::groups(&tx, &actor).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn claim_viewer_shares(
        &self,
        token: &str,
        request: ClaimViewerSharesRequest,
    ) -> Result<ClaimViewerSharesReceipt> {
        let operation = operation(&request.operation_id)?;
        let channel = access_channel(token)
            .filter(|c| c.surface() == Surface::Theatre)
            .ok_or(StoreError::Unauthenticated)?;
        // Native Bearer or Theatre cookie only. A development/Studio token cannot supply a subject.
        if !matches!(channel, Channel::Native | Channel::Web(Surface::Theatre)) {
            return Err(StoreError::Unauthenticated.into());
        }
        let (before, fact) = self
            .identity_auth()
            .ok_or(AccessError::Unavailable)?
            .claim_mailbox(self, token, channel, operation)
            .await
            .map_err(auth_error)?;
        let mut conn = self.connection().await?;
        let tx = conn.transaction().await?;
        let current = admit_in(&tx, token, channel)
            .await
            .map_err(|_| StoreError::Unauthenticated)?;
        if current.session_id != before.session_id
            || current.actor_id != before.actor_id
            || !fact.valid_at(&current.observed, tx.now_micros())
        {
            return Err(StoreError::Unauthenticated.into());
        }
        ActorId::new(&current.actor_id).map_err(|_| StoreError::CorruptRevision)?;
        let receipt = db::claim(&tx, &current, request, fact.email()).await?;
        tx.commit().await?;
        Ok(receipt)
    }
}
