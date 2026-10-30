//! Generation-pinned covers: the current public Work pin and independent per-image display consent.
//! The origin byte route reprojects the same selection, including after private object I/O.
use super::{
    domain::public_cover::{admit_cover_fetch, CoverProjection},
    listener_application::{id, ListenerError, ListenerService},
    listener_cover::{cover_image, facts_projection, AdmittedCover},
    turso::{
        cover::{pinned_cover_facts, PinnedCoverFacts, PinnedWork},
        listener,
    },
};
use crate::{
    contexts::catalog::{domain::WorkId, public_work},
    contract::theatre::discovery as dto,
};

impl ListenerService {
    pub(super) fn shown_generation_cover(
        &self,
        work: &crate::contexts::catalog::domain::Work,
        facts: Option<&PinnedCoverFacts>,
        now: i64,
    ) -> Option<dto::CoverImage> {
        let origin = &self.covers.as_ref()?.origin;
        match facts_projection(
            facts,
            |facts| super::domain::public_cover::project_at(work, facts),
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
        let work = id(work).map_err(|_| ListenerError::NotFound)?;
        let store = self
            .metadata_store
            .as_ref()
            .ok_or(ListenerError::NotFound)?;
        let connection = store.connection().await?;
        let tx = connection.read().await?;
        let now = listener::unit_millis(&tx);
        let disclosed = public_work(
            &tx,
            &WorkId::parse(&work).map_err(|_| ListenerError::NotFound)?,
        )
        .await?
        .ok_or(ListenerError::NotFound)?;
        if disclosed.first_visible_at > now {
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
        let now = (store
            .database()
            .now()
            .map_err(|_| ListenerError::Unavailable)?
            .get()
            / 1000)
            .max(now);
        let projection = facts_projection(
            Some(&facts),
            |facts| super::domain::public_cover::project_at(&disclosed.work, facts),
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
