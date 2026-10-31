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
    pub async fn chapter_matrix(
        &self,
        token: &str,
        work: &str,
        limit: u64,
        cursor: Option<&str>,
    ) -> Result<ChapterMatrixPage> {
        use super::super::domain::metadata_publication::{AudioBasis, Live, PublicationState};
        let work = WorkId::parse(work).map_err(|_| AccessError::Invalid)?;
        let limit = super::page_limit(limit)?;
        let cursor = cursor.map(super::CatalogCursor::decode).transpose()?;
        self.authenticate(token).await?;
        let conn = self.connection().await?;
        let tx = conn.read().await?;
        let owner = owner_work(&tx, token, &work).await?;
        let (policy_version, work_audience) = db::policy(&tx, work.as_str()).await?;
        let publication =
            super::super::turso::metadata_publication::state(&tx, work.as_str()).await?;
        let epoch = super::super::turso::lock_clock(&tx).await?;
        let scope = format!(
            "chapter-matrix:{}:{}:{}:{}",
            owner.actor.as_str(),
            work.as_str(),
            policy_version,
            publication.version()
        );
        let page =
            super::queries::units_page(&tx, &work, &scope, epoch, limit, false, cursor.as_ref())
                .await
                .map_err(|e| match e {
                    super::CatalogStoreError::InvalidCursor
                    | super::CatalogStoreError::StaleCursor => AccessError::Stale {
                        current: policy_version,
                    }
                    .into(),
                    e => ViewerAccessError::from(e),
                })?;
        let rules = db::units::rules(&tx, work.as_str(), &owner.actor).await?;
        let entries = match &publication {
            PublicationState::Published(p) if p.lifecycle() == Live::Active => Some(p.generation()),
            _ => None,
        };
        let items = page
            .items
            .into_iter()
            .map(|u| {
                let entry = entries.and_then(|g| g.entry(u.id()));
                ChapterMatrixRow {
                    unit_id: u.id().as_str().into(),
                    title: u.details().title.clone(),
                    draft_revision: u.revision(),
                    published_revision: entry.map(|e| e.pin().revision().get()),
                    published_position: entries
                        .and_then(|g| g.entries().iter().position(|e| e.unit() == u.id()))
                        .map(|n| n as u32 + 1),
                    audio: entry
                        .and_then(|e| e.audio())
                        .map(|a| MetadataGenerationAudio {
                            release_id: a.release().release().into(),
                            release_digest: a.release().digest().into(),
                            basis: match a.basis() {
                                AudioBasis::Exact => MetadataAudioBasis::Exact,
                                AudioBasis::DisplayOnly => MetadataAudioBasis::DisplayOnly,
                            },
                        }),
                    policy: rules.policy(u.id().as_str()),
                }
            })
            .collect();
        let result = ChapterMatrixPage {
            publication_lifecycle: match &publication {
                PublicationState::NeverPublished => MetadataPublicationLifecycle::NeverPublished,
                PublicationState::Published(p) => {
                    if p.lifecycle() == Live::Active {
                        MetadataPublicationLifecycle::Active
                    } else {
                        MetadataPublicationLifecycle::Withdrawn
                    }
                }
            },
            work_id: work.as_str().into(),
            work_revision: owner.work.revision(),
            publication_version: publication.version(),
            policy_version,
            work_audience,
            items,
            next_cursor: page.next.map(|c| c.encode()).transpose()?,
        };
        recheck_owner_work(&tx, token, &owner).await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn unit_viewer_access_state(
        &self,
        token: &str,
        work: &str,
        unit: &str,
    ) -> Result<UnitViewerAccessState> {
        let work = WorkId::parse(work).map_err(|_| AccessError::Invalid)?;
        self.authenticate(token).await?;
        let conn = self.connection().await?;
        let tx = conn.read().await?;
        let owner = owner_work(&tx, token, &work).await?;
        let state = db::units::state(&tx, work.as_str(), unit).await?;
        recheck_owner_work(&tx, token, &owner).await?;
        tx.commit().await?;
        Ok(state)
    }
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
            } | ViewerAccessChange::SetUnitPolicy {
                policy: UnitViewerPolicy::Inherit,
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
