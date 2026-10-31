//! Current authenticated Theatre generation reads and credential-bound origin delivery.
//! Identity RPC is outside the read transaction. Each snapshot re-admits the same session;
//! byte opens then repeat the entire current read, including fresh Identity security state.
use super::{
    domain::{
        listening::{DiscoverableWork, DiscoveryQuery, ListenerPublication},
        object_storage::NormalizedByteRange,
        playback_grant::{
            decide_media_fetch, grant_expiry, CurrentRelease, GrantClaims, MediaFetchFacts,
        },
    },
    generation_audio::playable_releases,
    listener_application::{id, permit_for, ListenerError, ListenerService},
    listener_cursor::Keyset,
    listener_delivery::PublicationObjectRef,
    listener_media::AuthorizedMedia,
    listener_projection::{publication_manifest, unit_kind, work_kind},
    storage::port::StorageRead,
    turso::listener::{self, ReadItem},
};
use crate::{
    contexts::catalog::{
        self,
        domain::{ContentUnitId, WorkId},
    },
    contract::theatre::discovery as dto,
    platform::{
        identity::domain::ActorId,
        identity_auth::{
            credential::{access_channel, Surface},
            session::{SessionError, SessionRecord},
            turso::admit_in,
            AuthFailure,
        },
        store::{Store, Tx},
    },
};
use sha2::{Digest, Sha256};

fn auth_error(error: AuthFailure) -> ListenerError {
    match error {
        AuthFailure::Unavailable | AuthFailure::Throttled { .. } => ListenerError::Unavailable,
        AuthFailure::Unauthenticated | AuthFailure::Forbidden | AuthFailure::InvalidRequest => {
            ListenerError::NotFound
        }
    }
}
fn session_error(error: SessionError) -> ListenerError {
    match error {
        SessionError::Unavailable | SessionError::Unknown => ListenerError::Unavailable,
        SessionError::Inactive | SessionError::Expired | SessionError::Replay => {
            ListenerError::NotFound
        }
    }
}
fn session_until(record: &SessionRecord) -> Result<i64, ListenerError> {
    let seconds = record
        .absolute_expires
        .min(record.idle_expires)
        .min(record.token_expires);
    i64::try_from(seconds)
        .ok()
        .and_then(|v| v.checked_mul(1000))
        .ok_or(ListenerError::Corrupt)
}
fn work_summary(row: &catalog::PublicWork) -> dto::MetadataWorkSummary {
    let work = &row.work;
    dto::MetadataWorkSummary {
        work_id: work.id().as_str().into(),
        kind: work_kind(work.kind()),
        title: work.details().title.clone(),
        description: work.details().synopsis.clone(),
        language: work.details().language.clone(),
        first_visible_at_ms: row.first_visible_at,
        unit_count: u64::from(row.unit_count),
        cover: None,
    }
}

impl ListenerService {
    /// Trusted host opt-in after policies have been confirmed, for this process only. Nothing
    /// persisted or restored re-enables it; a new activation epoch invalidates earlier grants.
    pub fn with_restricted_serving(mut self) -> Result<Self, ListenerError> {
        if self.media.is_none()
            || self
                .metadata_store
                .as_ref()
                .and_then(Store::identity_auth)
                .is_none()
        {
            return Err(ListenerError::Unavailable);
        }
        self.restricted_epoch = Some(uuid::Uuid::new_v4().to_string());
        Ok(self)
    }
    fn shared_store(&self) -> Result<&Store, ListenerError> {
        if self.restricted_epoch.is_none() || self.media.is_none() {
            return Err(ListenerError::NotFound);
        }
        self.metadata_store.as_ref().ok_or(ListenerError::NotFound)
    }
    fn shared_now(&self, until: i64) -> Result<i64, ListenerError> {
        let now = self
            .shared_store()?
            .database()
            .now()
            .map_err(|_| ListenerError::Unavailable)?
            .get()
            / 1000;
        if now < 0 || now >= until {
            return Err(ListenerError::NotFound);
        }
        Ok(now)
    }
    async fn shared_session(&self, token: &str) -> Result<SessionRecord, ListenerError> {
        let store = self.shared_store()?;
        let channel = access_channel(token)
            .filter(|c| c.surface() == Surface::Theatre)
            .ok_or(ListenerError::NotFound)?;
        store
            .identity_auth()
            .ok_or(ListenerError::NotFound)?
            .authenticate(store, token, channel)
            .await
            .map_err(auth_error)
    }
    async fn shared_scope(
        &self,
        tx: &Tx<'_>,
        token: &str,
        before: &SessionRecord,
    ) -> Result<(String, ActorId), ListenerError> {
        let current = admit_in(tx, token, before.channel)
            .await
            .map_err(session_error)?;
        if current.session_id != before.session_id
            || current.actor_id != before.actor_id
            || current.observed != before.observed
        {
            return Err(ListenerError::NotFound);
        }
        let epoch = self
            .restricted_epoch
            .as_ref()
            .ok_or(ListenerError::NotFound)?;
        let access = catalog::shared_scope(tx).await?;
        let bytes = serde_json::to_vec(&(
            epoch,
            &current.actor_id,
            &current.session_id,
            current.channel.stored(),
            current.observed.subject.bytes(),
            current.observed.epoch.bytes(),
            current.observed.version,
            access,
        ))
        .map_err(|_| ListenerError::Corrupt)?;
        Ok((
            format!("{:x}", Sha256::digest(bytes)),
            ActorId::new(current.actor_id).map_err(|_| ListenerError::Corrupt)?,
        ))
    }
    pub async fn shared_works(
        &self,
        token: &str,
        search: &str,
        kind: Option<&str>,
        language: Option<&str>,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::MetadataWorkPage, ListenerError> {
        let query = DiscoveryQuery::new(search, kind, language, limit)?;
        let session = self.shared_session(token).await?;
        let connection = self.shared_store()?.connection().await?;
        let tx = connection.read().await?;
        let now = listener::unit_millis(&tx);
        let (scope, actor) = self.shared_scope(&tx, token, &session).await?;
        let after = cursor
            .map(|v| self.shared_cursors.read_scoped(v, &query, &scope, now))
            .transpose()?;
        let after = match after {
            None => None,
            Some(Keyset::Works { time, id }) => Some((time, id)),
            _ => return Err(ListenerError::InvalidCursor),
        };
        // SQL narrows candidates only. Admission and matching precede the page boundary and
        // cursor: a denied candidate never becomes an observable continuation. If the bounded
        // lookahead cannot establish a complete page, refuse instead of reporting a false end.
        let candidates = catalog::shared_candidates(
            &tx,
            &actor,
            after.as_ref().map(|(at, id)| (*at, id.as_str())),
            limit as u8 + 1,
            query.kind(),
            query.language(),
            query.search(),
        )
        .await?;
        let bounded = candidates.len() > usize::from(limit);
        let mut admitted = Vec::new();
        let mut until = session_until(&session)?;
        for (raw, _) in candidates {
            let work = WorkId::parse(&raw).map_err(|_| ListenerError::Corrupt)?;
            let Some(work) = catalog::shared_work(&tx, &work, &actor).await? else {
                continue;
            };
            let w = &work.row.work;
            if !query.matches(&DiscoverableWork {
                kind: w.kind(),
                language: &w.details().language,
                title: &w.details().title,
                description: &w.details().synopsis,
            }) {
                continue;
            }
            until = work.deadline_us.map_or(until, |at| until.min(at / 1000));
            admitted.push(work_summary(&work.row));
        }
        let has_more = admitted.len() > usize::from(limit);
        if bounded && !has_more {
            return Err(ListenerError::Unavailable);
        }
        if has_more {
            admitted.pop();
        }
        let next_cursor = if has_more {
            admitted
                .last()
                .map(|last| {
                    self.shared_cursors.issue_scoped(
                        &query,
                        Keyset::Works {
                            time: last.first_visible_at_ms,
                            id: last.work_id.clone(),
                        },
                        &scope,
                        now,
                        until,
                    )
                })
                .transpose()?
        } else {
            None
        };
        let items = admitted;
        self.shared_now(until)?;
        tx.commit().await?;
        Ok(dto::MetadataWorkPage { items, next_cursor })
    }
    pub async fn shared_work(
        &self,
        token: &str,
        raw: &str,
    ) -> Result<dto::MetadataWorkSummary, ListenerError> {
        let work = WorkId::parse(&id(raw)?).map_err(|_| ListenerError::InvalidRequest)?;
        let session = self.shared_session(token).await?;
        let connection = self.shared_store()?.connection().await?;
        let tx = connection.read().await?;
        let (_, actor) = self.shared_scope(&tx, token, &session).await?;
        let row = catalog::shared_work(&tx, &work, &actor)
            .await?
            .ok_or(ListenerError::NotFound)?;
        let result = work_summary(&row.row);
        self.shared_now(
            session_until(&session)?.min(row.deadline_us.map_or(i64::MAX, |at| at / 1000)),
        )?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn shared_units(
        &self,
        token: &str,
        raw: &str,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::MetadataUnitPage, ListenerError> {
        let work = WorkId::parse(&id(raw)?).map_err(|_| ListenerError::InvalidRequest)?;
        let query = DiscoveryQuery::new("", None, None, limit)?;
        let session = self.shared_session(token).await?;
        let connection = self.shared_store()?.connection().await?;
        let tx = connection.read().await?;
        let now = listener::unit_millis(&tx);
        let (scope, actor) = self.shared_scope(&tx, token, &session).await?;
        let walk = cursor
            .map(|v| self.shared_cursors.read_scoped(v, &query, &scope, now))
            .transpose()?;
        let row = catalog::shared_work(&tx, &work, &actor)
            .await?
            .ok_or(ListenerError::NotFound)?;
        let after = match walk {
            None => 0,
            Some(Keyset::Units {
                work: bound,
                position,
                id,
            }) if bound == work.as_str() && id == row.row.generation.get().to_string() => position,
            _ => return Err(ListenerError::InvalidCursor),
        };
        let (rows, next) = catalog::shared_units(&tx, &row, after, limit as u8).await?;
        let asked: Vec<_> = rows
            .iter()
            .map(|r| (r.unit.id(), r.audio.as_ref()))
            .collect();
        let live = playable_releases(&tx, &asked).await?;
        let items = rows
            .into_iter()
            .zip(live)
            .map(|(r, release)| dto::MetadataUnitSummary {
                unit_id: r.unit.id().as_str().into(),
                kind: unit_kind(r.unit.kind()),
                position: r.position,
                title: r.unit.details().title.clone(),
                description: r.unit.details().description.clone(),
                ordinal: u64::from(r.ordinal),
                playback: release.map(|r| {
                    let d = r.release().document();
                    dto::UnitPlayback {
                        publication_id: d.id.clone(),
                        published_at_ms: d.published_at,
                        duration_ms: d.manifest.media.duration_ms,
                    }
                }),
            })
            .collect();
        let until = session_until(&session)?.min(row.deadline_us.map_or(i64::MAX, |at| at / 1000));
        let next_cursor = next
            .map(|position| {
                self.shared_cursors.issue_scoped(
                    &query,
                    Keyset::Units {
                        work: work.as_str().into(),
                        position,
                        id: row.row.generation.get().to_string(),
                    },
                    &scope,
                    now,
                    until,
                )
            })
            .transpose()?;
        self.shared_now(until)?;
        tx.commit().await?;
        Ok(dto::MetadataUnitPage {
            work_id: work.as_str().into(),
            items,
            next_cursor,
        })
    }
    async fn shared_current(
        &self,
        token: &str,
        raw: &str,
    ) -> Result<(ReadItem, i64, String, i64), ListenerError> {
        let publication = id(raw)?;
        let session = self.shared_session(token).await?;
        let connection = self.shared_store()?.connection().await?;
        let tx = connection.read().await?;
        let now = listener::unit_millis(&tx);
        let (scope, actor) = self.shared_scope(&tx, token, &session).await?;
        let verified = listener::verified_generation_release(&tx, &publication, now)
            .await?
            .ok_or(ListenerError::NotFound)?;
        let d = verified.publication.release().document();
        let work_id = WorkId::parse(&d.manifest.work_id).map_err(|_| ListenerError::Corrupt)?;
        let unit_id =
            ContentUnitId::parse(&d.manifest.unit_id).map_err(|_| ListenerError::Corrupt)?;
        let work = catalog::shared_work(&tx, &work_id, &actor)
            .await?
            .ok_or(ListenerError::NotFound)?;
        let unit = catalog::shared_unit(&tx, &work, &unit_id)
            .await?
            .ok_or(ListenerError::NotFound)?;
        if unit
            .audio
            .as_ref()
            .is_none_or(|a| a.release().release() != publication)
        {
            return Err(ListenerError::NotFound);
        }
        let mut releases = playable_releases(&tx, &[(&unit_id, unit.audio.as_ref())]).await?;
        let release = releases.pop().flatten().ok_or(ListenerError::NotFound)?;
        let until = session_until(&session)?.min(work.deadline_us.map_or(i64::MAX, |at| at / 1000));
        let item = ReadItem {
            publication: ListenerPublication::associated(
                release.release().clone(),
                work.row.work,
                unit.unit,
            )?,
            first_at: work.row.first_visible_at,
            position: unit.position,
            presentation: None,
            cover: None,
        };
        let now = self.shared_now(until)?.max(now);
        if !item.publication.valid_at(now) {
            return Err(ListenerError::NotFound);
        }
        tx.commit().await?;
        Ok((item, now, scope, until))
    }
    pub async fn shared_manifest(
        &self,
        token: &str,
        publication: &str,
    ) -> Result<dto::PublicationManifest, ListenerError> {
        let (item, _, _, _) = self.shared_current(token, publication).await?;
        Ok(publication_manifest(&item.publication))
    }
    pub async fn shared_authorize_playback(
        &self,
        token: &str,
        publication: &str,
    ) -> Result<dto::PlaybackAuthorization, ListenerError> {
        let (initial, now, scope, deadline) = self.shared_current(token, publication).await?;
        let d = initial.publication.release().document();
        let m = &d.manifest;
        let until = grant_expiry(
            now,
            m.distribution_clearance.request.valid_until.min(deadline),
        )
        .ok_or(ListenerError::NotFound)?;
        let mut permit = permit_for(&initial.publication, until);
        permit.access_scope = Some(format!("media:{scope}"));
        let grant = self.delivery.grant(permit.clone()).await?;
        let (fresh, at, fresh_scope, fresh_deadline) =
            self.shared_current(token, publication).await?;
        if scope != fresh_scope
            || publication_manifest(&fresh.publication)
                != publication_manifest(&initial.publication)
            || grant.permit != permit
            || grant.expires <= at
            || grant.expires > until.min(fresh_deadline)
            || !grant.range
        {
            return Err(ListenerError::NotFound);
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
    pub async fn authorize_shared_media(
        &self,
        token: &str,
        grant: &str,
    ) -> Result<AuthorizedMedia, ListenerError> {
        let origin = self.media.as_ref().ok_or(ListenerError::NotFound)?;
        let claims = GrantClaims::parse(grant).map_err(|_| ListenerError::NotFound)?;
        let (item, now, scope, deadline) =
            self.shared_current(token, claims.publication_id()).await?;
        let mut permit = permit_for(&item.publication, claims.expires_at_ms());
        permit.access_scope = Some(format!("media:{scope}"));
        let d = item.publication.release().document();
        decide_media_fetch(MediaFetchFacts {
            now_ms: now,
            expires_at_ms: claims.expires_at_ms(),
            current: Some(CurrentRelease {
                published_at_ms: d.published_at,
                rights_valid_until_ms: d
                    .manifest
                    .distribution_clearance
                    .request
                    .valid_until
                    .min(deadline),
            }),
            authenticated: claims.authenticates(origin.key(), &permit.subject()),
        })
        .map_err(|_| ListenerError::NotFound)?;
        Ok(AuthorizedMedia {
            token: grant.into(),
            permit,
        })
    }
    pub async fn open_shared_media(
        &self,
        token: &str,
        authorized: &AuthorizedMedia,
        range: Option<NormalizedByteRange>,
    ) -> Result<StorageRead, ListenerError> {
        if authorized.permit.access_scope.is_none() {
            return Err(ListenerError::NotFound);
        }
        let origin = self.media.as_ref().ok_or(ListenerError::NotFound)?;
        let opened = origin
            .objects()
            .open(PublicationObjectRef::for_permit(&authorized.permit), range)
            .await
            .map_err(super::listener_media::storage_error)?;
        let fresh = self
            .authorize_shared_media(token, &authorized.token)
            .await?;
        if fresh.permit != authorized.permit {
            return Err(ListenerError::NotFound);
        }
        Ok(opened)
    }
}
