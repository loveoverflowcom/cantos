//! Proposed #574 policy, explicitly opt-in at trusted composition. Reuses the independently
//! recorded per-image public-display consent and the exact metadata-disclosed Work revision.
//! Production composition deliberately does not enable this pending owner policy acceptance.
use super::{
    domain::public_cover::{admit_cover_fetch, project_metadata_cover, CoverProjection},
    listener_application::{id, ListenerError, ListenerService},
    listener_cover::{cover_image, facts_projection, AdmittedCover},
    turso::{
        cover::{pinned_cover_facts, PinnedCoverFacts, PinnedWork},
        listener,
    },
};
use crate::{contexts::catalog::sample_disclosed_work, contract::theatre::discovery as dto};

impl ListenerService {
    /// Separate host policy capability. It grants no image consent, metadata disclosure or
    /// owner authority. No production caller enables it until #574's policy is accepted.
    pub fn with_metadata_cover_policy(mut self) -> Self {
        self.metadata_covers = true;
        self
    }

    pub(super) fn shown_metadata_cover(
        &self,
        work: &crate::contexts::catalog::domain::Work,
        facts: Option<&PinnedCoverFacts>,
        now: i64,
    ) -> Option<dto::CoverImage> {
        if !self.metadata_covers {
            return None;
        }
        let consent_revision = facts.and_then(|facts| facts.metadata_revision);
        let origin = &self.covers.as_ref()?.origin;
        match facts_projection(
            facts,
            |facts| project_metadata_cover(work, consent_revision, facts),
            now,
        ) {
            CoverProjection::Shown(cover) => Some(cover_image(origin, work.id().as_str(), &cover)),
            CoverProjection::Fallback(_) => None,
        }
    }

    pub(super) async fn admit_metadata_cover(
        &self,
        work: &str,
        sha256: &str,
    ) -> Result<AdmittedCover, ListenerError> {
        if !self.metadata_covers {
            return Err(ListenerError::NotFound);
        }
        let work = id(work).map_err(|_| ListenerError::NotFound)?;
        let store = self
            .metadata_store
            .as_ref()
            .ok_or(ListenerError::NotFound)?;
        let connection = store.connection().await?;
        let tx = connection.read().await?;
        let now = listener::unit_millis(&tx);
        // Release precedence is absolute: an eligible release with no cover cannot expose the
        // metadata-only candidate. Recheck in this same snapshot, including on byte readback.
        if !listener::presented_work(&tx, &work, now)
            .await?
            .0
            .is_empty()
        {
            return Err(ListenerError::NotFound);
        }
        let disclosed = sample_disclosed_work(&tx, &work)
            .await?
            .ok_or(ListenerError::NotFound)?;
        if disclosed.first_at > now {
            return Err(ListenerError::NotFound);
        }
        let mut facts = pinned_cover_facts(
            &tx,
            &[PinnedWork {
                work_id: work.clone(),
                owner_id: disclosed.work.owner_id().as_str().into(),
                revision: disclosed.work.revision(),
            }],
        )
        .await?;
        let facts = facts.remove(&work).ok_or(ListenerError::NotFound)?;
        let consent_revision = facts.metadata_revision;
        let now = (store
            .database()
            .now()
            .map_err(|_| ListenerError::Unavailable)?
            .get()
            / 1000)
            .max(now);
        let projection = facts_projection(
            Some(&facts),
            |facts| project_metadata_cover(&disclosed.work, consent_revision, facts),
            now,
        );
        let cover = admit_cover_fetch(&projection, sha256)
            .ok_or(ListenerError::NotFound)?
            .clone();
        let asset = facts.asset.ok_or(ListenerError::NotFound)?;
        tx.commit().await?;
        Ok(AdmittedCover { work, cover, asset })
    }
}
