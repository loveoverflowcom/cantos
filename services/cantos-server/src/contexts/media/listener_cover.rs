//! Release-pinned public Work cover (accepted ADR 0033): the Discovery v1.2 projection and the
//! origin byte route's admission. Every decision is the pure kernel's (`project_public_cover`,
//! `admit_cover_fetch`); this shell supplies the pinned facts, the read's trusted clock and the
//! composed private image reader. Nothing here widens Studio preview authority.
use std::sync::Arc;

use super::{
    domain::{
        image::{sha256_valid, ImageMime},
        public_cover::{
            admit_cover_fetch, project_public_cover, CoverDisclosure, CoverFacts, CoverFallback,
            CoverProjection, PublicCover,
        },
    },
    image_application::{
        objects::SharedImageObjects, ImageByteStream, ImageObjectError, ImageObjects,
    },
    listener_application::{id, ListenerError, ListenerService, PresentationLevel, ReadRequest},
    listener_delivery::PublicMediaOrigin,
    turso::{image::StoredImageAsset, listener::ReadItem},
};
use crate::contract::theatre::discovery as dto;
use crate::platform::store::Store;

/// Trusted composition of cover delivery: the explicit HTTPS API origin listeners reach and the
/// reviewed private image reader. Absent, every summary answers `cover: null` and the route
/// answers `not_found`; no request can select it.
#[derive(Clone)]
pub(super) struct CoverDelivery {
    pub(super) origin: PublicMediaOrigin,
    objects: Arc<dyn ImageObjects>,
}

/// The kernel's projection for one covered row at the read's instant. A row read without cover
/// facts, or whose stored facts failed admission, is the Corrupt fallback.
pub(super) fn cover_projection(item: &ReadItem, now: i64) -> CoverProjection {
    facts_projection(
        item.cover.as_ref(),
        |facts| project_public_cover(&item.publication, facts),
        now,
    )
}

pub(super) fn facts_projection(
    stored: Option<&super::turso::cover::PinnedCoverFacts>,
    project: impl FnOnce(CoverFacts<'_>) -> CoverProjection,
    now: i64,
) -> CoverProjection {
    let Some(facts) = stored.filter(|facts| !facts.corrupt) else {
        return CoverProjection::Fallback(CoverFallback::Corrupt);
    };
    project(CoverFacts {
        changes: facts.change.as_slice(),
        asset: facts.asset.as_ref().map(|stored| &stored.asset),
        disclosure: facts
            .disclosure
            .as_ref()
            .map_or(CoverDisclosure::Undecided, |d| d.as_cover_disclosure()),
        now_ms: now,
    })
}

pub(super) fn cover_image(
    origin: &PublicMediaOrigin,
    work_id: &str,
    cover: &PublicCover,
) -> dto::CoverImage {
    dto::CoverImage {
        url: origin.cover_url(work_id, cover.sha256()),
        sha256: cover.sha256().into(),
        content_type: match cover.content_type() {
            ImageMime::Png => dto::CoverMime::Png,
            ImageMime::Jpeg => dto::CoverMime::Jpeg,
            ImageMime::Avif => dto::CoverMime::Avif,
        },
        width_px: u64::from(cover.width_px()),
        height_px: u64::from(cover.height_px()),
        byte_length: cover.byte_length(),
        alt_text: cover.alt_text().into(),
    }
}

fn covered(
    summary: dto::PresentedWorkSummary,
    cover: Option<dto::CoverImage>,
) -> dto::CoveredWorkSummary {
    dto::CoveredWorkSummary {
        work_id: summary.work_id,
        kind: summary.kind,
        title: summary.title,
        description: summary.description,
        language: summary.language,
        first_published_at_ms: summary.first_published_at_ms,
        unit_count: summary.unit_count,
        cover,
    }
}

/// Bytes may be served for exactly this projection. Holds the private location internally only.
pub struct AdmittedCover {
    pub(super) work: String,
    pub(super) cover: PublicCover,
    pub(super) asset: StoredImageAsset,
}
impl AdmittedCover {
    pub fn sha256(&self) -> &str {
        self.cover.sha256()
    }
    pub fn content_type(&self) -> &'static str {
        self.cover.content_type().as_str()
    }
    pub fn byte_length(&self) -> u64 {
        self.cover.byte_length()
    }
}

fn object_error(error: ImageObjectError) -> ListenerError {
    match error {
        // The admitted object is gone or its bytes contradict the admitted SHA-256/length.
        ImageObjectError::NotFound | ImageObjectError::Corrupt => ListenerError::Corrupt,
        ImageObjectError::Unavailable | ImageObjectError::Storage(_) => ListenerError::Unavailable,
    }
}

impl ListenerService {
    /// Compose cover delivery on this host's explicit HTTPS origin. Without configured private
    /// media storage nothing is composed and covers stay `null`.
    pub fn with_cover_delivery(mut self, origin: PublicMediaOrigin, store: &Store) -> Self {
        self.covers = store.media_storage.clone().map(|registry| CoverDelivery {
            origin,
            objects: Arc::new(SharedImageObjects::new(Some(registry))),
        });
        self
    }

    pub async fn covered_works(
        &self,
        search: &str,
        kind: Option<&str>,
        language: Option<&str>,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::CoveredWorkPage, ListenerError> {
        let page = self
            .presented_page(
                PresentationLevel::Covered,
                search,
                kind,
                language,
                limit,
                cursor,
            )
            .await?;
        Ok(dto::CoveredWorkPage {
            items: page
                .rows
                .into_iter()
                .map(|(row, summary)| {
                    let cover = self.shown(&row, page.now);
                    covered(summary, cover)
                })
                .collect(),
            next_cursor: page.next_cursor,
        })
    }

    pub async fn covered_work(&self, work: &str) -> Result<dto::CoveredWorkSummary, ListenerError> {
        let (row, summary, now) = self.presented_row(PresentationLevel::Covered, work).await?;
        let cover = self.shown(&row, now);
        Ok(covered(summary, cover))
    }

    /// Units at `presentation=2` answer exactly as at `presentation=1`; only cursors differ.
    pub async fn covered_units(
        &self,
        work: &str,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::PresentedUnitPage, ListenerError> {
        self.presented_units_at(PresentationLevel::Covered, work, limit, cursor)
            .await
    }

    pub(super) fn shown(&self, row: &ReadItem, now: i64) -> Option<dto::CoverImage> {
        let delivery = self.covers.as_ref()?;
        match cover_projection(row, now) {
            CoverProjection::Shown(cover) => Some(cover_image(
                &delivery.origin,
                row.publication.work().id().as_str(),
                &cover,
            )),
            CoverProjection::Fallback(_) => None,
        }
    }

    /// Re-resolve the representative eligible release and re-project with the current
    /// disclosure and clock. Every denial, malformed identity and uncomposed host is the same
    /// `NotFound`; only a stored-fact contradiction is `Corrupt`.
    pub async fn admit_cover(
        &self,
        work: &str,
        sha256: &str,
    ) -> Result<AdmittedCover, ListenerError> {
        if self.covers.is_none() || !sha256_valid(sha256) {
            return Err(ListenerError::NotFound);
        }
        let work = id(work).map_err(|_| ListenerError::NotFound)?;
        let snapshot = self
            .read
            .read(ReadRequest::Cover { work: work.clone() })
            .await?;
        let Some(row) = snapshot.items.into_iter().next() else {
            return self.admit_metadata_cover(&work, sha256).await;
        };
        if row.publication.work().id().as_str() != work {
            return Err(ListenerError::Corrupt);
        }
        let projection = cover_projection(&row, snapshot.now);
        let cover = admit_cover_fetch(&projection, sha256)
            .ok_or(ListenerError::NotFound)?
            .clone();
        let asset = row
            .cover
            .and_then(|facts| facts.asset)
            .ok_or(ListenerError::Corrupt)?;
        if asset.asset.id().as_str() != cover.image_asset_id()
            || asset.asset.metadata().sha256() != cover.sha256()
        {
            return Err(ListenerError::Corrupt);
        }
        Ok(AdmittedCover { work, cover, asset })
    }

    /// Open the verified private bytes (the reader hashes the actual object before streaming),
    /// then re-project: a revocation, expiry or withdrawal committed meanwhile denies.
    pub async fn open_cover(
        &self,
        admitted: &AdmittedCover,
    ) -> Result<ImageByteStream, ListenerError> {
        let delivery = self.covers.as_ref().ok_or(ListenerError::NotFound)?;
        let opened = delivery
            .objects
            .open(admitted.asset.clone())
            .await
            .map_err(object_error)?;
        if opened.byte_length != admitted.cover.byte_length() {
            return Err(ListenerError::Corrupt);
        }
        let fresh = self.admit_cover(&admitted.work, admitted.sha256()).await?;
        if fresh.cover != admitted.cover {
            return Err(ListenerError::NotFound);
        }
        Ok(opened.stream)
    }
}
