//! Listener use cases: no Studio token, session cookie, editorial access or production work.
use super::{
    domain::listening::{after_work, DiscoveryQuery, ListenerDomainError},
    listener_cursor::{CursorVault, Keyset},
    listener_delivery::{PlaybackDelivery, PlaybackPermit, UnavailablePlayback},
    postgres::listener::{PgRead, ReadItem},
};
use crate::platform::{
    ids::canonical_uuid,
    postgres::{Store, StoreError},
};
use cantos_contracts::theatre as dto;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerError {
    InvalidRequest,
    InvalidCursor,
    NotFound,
    RenditionUnavailable,
    Unavailable,
    Corrupt,
}
impl From<StoreError> for ListenerError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::CorruptRevision => Self::Corrupt,
            _ => Self::Unavailable,
        }
    }
}
impl From<ListenerDomainError> for ListenerError {
    fn from(e: ListenerDomainError) -> Self {
        match e {
            ListenerDomainError::InvalidQuery | ListenerDomainError::InvalidLimit => {
                Self::InvalidRequest
            }
            ListenerDomainError::BindingMismatch | ListenerDomainError::InvalidTime => {
                Self::Corrupt
            }
        }
    }
}
fn clock() -> Result<i64, ListenerError> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ListenerError::Unavailable)?
            .as_millis(),
    )
    .map_err(|_| ListenerError::Unavailable)
}
fn id(raw: &str) -> Result<String, ListenerError> {
    canonical_uuid(raw)
        .filter(|v| v == raw)
        .ok_or(ListenerError::InvalidRequest)
}
pub(super) enum ReadRequest {
    Work {
        work: String,
    },
    Works {
        query: DiscoveryQuery,
        after: Option<Keyset>,
    },
    Units {
        work: String,
        after: Option<Keyset>,
        limit: usize,
    },
    Publication {
        id: String,
    },
}
pub(super) struct ReadSnapshot {
    pub now: i64,
    pub items: Vec<ReadItem>,
    pub next_key: Option<Keyset>,
}
pub(super) type ReadFuture =
    Pin<Box<dyn Future<Output = Result<ReadSnapshot, ListenerError>> + Send + 'static>>;
pub(super) trait ListenerRead: Send + Sync {
    fn read(&self, request: ReadRequest) -> ReadFuture;
}
#[derive(Clone)]
pub struct ListenerService {
    read: Arc<dyn ListenerRead>,
    cursors: Arc<CursorVault>,
    delivery: Arc<dyn PlaybackDelivery>,
}
impl ListenerService {
    pub fn new(store: Store) -> Self {
        Self {
            read: Arc::new(PgRead(store)),
            cursors: Arc::new(CursorVault::default()),
            delivery: Arc::new(UnavailablePlayback),
        }
    }
    pub fn with_playback_delivery(mut self, delivery: Arc<dyn PlaybackDelivery>) -> Self {
        self.delivery = delivery;
        self
    }
    pub async fn works(
        &self,
        search: &str,
        kind: Option<&str>,
        language: Option<&str>,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::WorkPage, ListenerError> {
        let query = DiscoveryQuery::new(search, kind, language, limit)?;
        let after = cursor
            .map(|v| self.cursors.read(v, &query, clock()?))
            .transpose()?;
        if after
            .as_ref()
            .is_some_and(|k| !matches!(k, Keyset::Works { .. }))
        {
            return Err(ListenerError::InvalidCursor);
        }
        let snapshot = self
            .read
            .read(ReadRequest::Works {
                query: query.clone(),
                after: after.clone(),
            })
            .await?;
        let mut items = Vec::new();
        for r in snapshot.items {
            let summary = r.publication.summary(r.first_at);
            if !query.matches(&summary) {
                return Err(ListenerError::Corrupt);
            }
            if let Some(Keyset::Works { time, id: after_id }) = &after {
                let current = uuid::Uuid::parse_str(&summary.work_id)
                    .map_err(|_| ListenerError::Corrupt)?
                    .as_u128();
                let old = uuid::Uuid::parse_str(after_id)
                    .map_err(|_| ListenerError::Corrupt)?
                    .as_u128();
                if !after_work(r.first_at, current, *time, old) {
                    return Err(ListenerError::Corrupt);
                }
            }
            items.push(summary);
        }
        items.truncate(query.limit());
        let next_cursor = snapshot
            .next_key
            .map(|key| self.cursors.issue(&query, key, clock()?))
            .transpose()?;
        Ok(dto::WorkPage { items, next_cursor })
    }
    pub async fn work(&self, work: &str) -> Result<dto::WorkSummary, ListenerError> {
        let work = id(work)?;
        let snapshot = self.read.read(ReadRequest::Work { work }).await?;
        snapshot
            .items
            .first()
            .map(|r| r.publication.summary(r.first_at))
            .ok_or(ListenerError::NotFound)
    }
    pub async fn units(
        &self,
        work: &str,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::UnitPage, ListenerError> {
        let work = id(work)?;
        let query = DiscoveryQuery::new("", None, None, limit)?;
        let after = cursor
            .map(|v| self.cursors.read(v, &query, clock()?))
            .transpose()?;
        if after
            .as_ref()
            .is_some_and(|k| !matches!(k,Keyset::Units{work:w,..} if w==&work))
        {
            return Err(ListenerError::InvalidCursor);
        }
        let snapshot = self
            .read
            .read(ReadRequest::Units {
                work: work.clone(),
                after,
                limit: query.limit(),
            })
            .await?;
        if snapshot.items.is_empty() && cursor.is_none() {
            return Err(ListenerError::NotFound);
        }
        let mut items: Vec<_> = snapshot
            .items
            .into_iter()
            .map(|r| {
                let mut unit = r.publication.unit();
                unit.position = r.position;
                unit
            })
            .collect();
        items.truncate(query.limit());
        let next_cursor = snapshot
            .next_key
            .map(|key| self.cursors.issue(&query, key, clock()?))
            .transpose()?;
        Ok(dto::UnitPage {
            work_id: work,
            items,
            next_cursor,
        })
    }
    async fn resolve(&self, publication: &str) -> Result<(ReadItem, i64), ListenerError> {
        let snapshot = self
            .read
            .read(ReadRequest::Publication {
                id: id(publication)?,
            })
            .await?;
        let p = snapshot
            .items
            .into_iter()
            .next()
            .ok_or(ListenerError::NotFound)?;
        if p.publication.manifest().publication_id != publication {
            return Err(ListenerError::Corrupt);
        }
        Ok((p, snapshot.now))
    }
    pub async fn manifest(
        &self,
        publication: &str,
    ) -> Result<dto::PublicationManifest, ListenerError> {
        Ok(self.resolve(publication).await?.0.publication.manifest())
    }
    pub async fn authorize_playback(
        &self,
        publication: &str,
    ) -> Result<dto::PlaybackAuthorization, ListenerError> {
        let (initial, now) = self.resolve(publication).await?;
        let d = initial.publication.release().document();
        let m = &d.manifest;
        let until = now
            .checked_add(120_000)
            .ok_or(ListenerError::Corrupt)?
            .min(m.distribution_clearance.request.valid_until);
        let permit = PlaybackPermit {
            publication_id: d.id.clone(),
            rendition_id: m.rendition_id.clone(),
            sha256: m.media.sha256.clone(),
            object_id: m.delivery_object_id.clone(),
            byte_length: m.media.byte_length,
            valid_until: until,
        };
        let grant = self.delivery.grant(permit).await?;
        // Re-read from a fresh PG transaction after external signing. A stale snapshot cannot grant.
        let (fresh, fresh_now) = self.resolve(publication).await?;
        if fresh.publication.manifest() != initial.publication.manifest()
            || grant.expires <= fresh_now
            || grant.expires > until
            || !grant.range
        {
            return Err(ListenerError::RenditionUnavailable);
        }
        Ok(dto::PlaybackAuthorization {
            publication_id: d.id.clone(),
            rendition_id: m.rendition_id.clone(),
            sha256: m.media.sha256.clone(),
            url: grant.url,
            expires_at_ms: grant.expires,
            content_type: "audio/ogg".into(),
            byte_range_supported: true,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::super::{
        domain::{
            listening::{ListenerFacts, ListenerPublication},
            publication::{decide, tests::fixture, AudioRelease, ReleaseId},
        },
        listener_delivery::{MediaGrant, PlaybackDeliveryFuture},
    };
    use super::*;
    use crate::contexts::catalog::domain::{NewWork, Visibility, Work};
    use std::sync::Mutex;
    fn item() -> ReadItem {
        let mut f = fixture();
        let mut details = f.work.details().clone();
        details.visibility = Visibility::Public;
        f.work = Work::new(NewWork {
            id: f.work.id().clone(),
            kind: f.work.kind(),
            owner_id: f.work.owner_id().clone(),
            details,
            at: 1000,
        })
        .unwrap();
        let r = AudioRelease::publish(
            ReleaseId::parse("80000000-0000-4000-8000-000000000008").unwrap(),
            &f.actor,
            5000,
            None,
            decide(&f.facts()).unwrap(),
        )
        .unwrap();
        let p = ListenerPublication::resolve(ListenerFacts {
            release: &r,
            pinned_work: &f.work,
            pinned_unit: &f.unit,
            current_work: &f.work,
            current_unit: &f.unit,
            rights: Some(&f.rights),
            approval: Some(&f.approval),
            active: true,
            owner_active: true,
            withdrawn: false,
            submission_revoked: false,
            now: 5000,
        })
        .unwrap()
        .unwrap();
        ReadItem {
            publication: p,
            first_at: 5000,
            position: 1,
        }
    }
    struct SequencedRead(Mutex<Vec<bool>>);
    impl ListenerRead for SequencedRead {
        fn read(&self, _request: ReadRequest) -> ReadFuture {
            let visible = self.0.lock().unwrap().remove(0);
            Box::pin(async move {
                Ok(ReadSnapshot {
                    now: 5000,
                    items: if visible { vec![item()] } else { vec![] },
                    next_key: None,
                })
            })
        }
    }
    struct NamedDelivery {
        expires: i64,
        range: bool,
    }
    impl PlaybackDelivery for NamedDelivery {
        fn grant(&self, permit: PlaybackPermit) -> PlaybackDeliveryFuture {
            assert_eq!(permit.publication_object_id(), "publication-fixture");
            let expires = self.expires;
            let range = self.range;
            Box::pin(async move {
                MediaGrant::verified(
                    "https://synthetic.invalid/approved-media".into(),
                    expires,
                    range,
                )
            })
        }
    }
    fn service(sequence: Vec<bool>, delivery: Arc<dyn PlaybackDelivery>) -> ListenerService {
        ListenerService {
            read: Arc::new(SequencedRead(Mutex::new(sequence))),
            cursors: Arc::new(CursorVault::default()),
            delivery,
        }
    }
    const ID: &str = "80000000-0000-4000-8000-000000000008";
    #[tokio::test]
    async fn grant_rechecks_withdrawal_and_never_uses_stale_read() {
        let s = service(
            vec![true, false],
            Arc::new(NamedDelivery {
                expires: 9000,
                range: true,
            }),
        );
        assert_eq!(
            s.authorize_playback(ID).await.unwrap_err(),
            ListenerError::NotFound
        );
    }
    #[tokio::test]
    async fn expiry_window_and_range_are_enforced_after_io() {
        for (expires, range) in [(5000, true), (10001, true), (9000, false)] {
            let s = service(vec![true, true], Arc::new(NamedDelivery { expires, range }));
            assert_eq!(
                s.authorize_playback(ID).await.unwrap_err(),
                ListenerError::RenditionUnavailable
            );
        }
        let s = service(
            vec![true, true],
            Arc::new(NamedDelivery {
                expires: 9000,
                range: true,
            }),
        );
        let result = s.authorize_playback(ID).await.unwrap();
        assert_eq!(result.sha256, "a".repeat(64));
        assert_eq!(result.expires_at_ms, 9000);
    }
    #[tokio::test]
    async fn default_delivery_is_explicitly_unavailable() {
        let s = service(vec![true], Arc::new(UnavailablePlayback));
        assert_eq!(
            s.authorize_playback(ID).await.unwrap_err(),
            ListenerError::RenditionUnavailable
        );
    }
    #[tokio::test]
    async fn empty_catalog_and_private_or_absent_release_are_distinct() {
        let s = service(vec![false, false], Arc::new(UnavailablePlayback));
        assert_eq!(
            s.works("", None, None, 20, None).await.unwrap(),
            dto::WorkPage {
                items: vec![],
                next_cursor: None
            }
        );
        assert_eq!(s.manifest(ID).await.unwrap_err(), ListenerError::NotFound);
    }
}
