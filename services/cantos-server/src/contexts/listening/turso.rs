//! Turso (`server.db`) is the sole durable Library authority. No demo state or provider calls.
//!
//! Every write is one `BEGIN IMMEDIATE` unit on the serialized writer. In that unit the Server
//! re-admits the Identity session the principal was earned from, decides from facts read in the
//! unit, and writes the effect, its device sequence, the account order and the operation receipt
//! together: PostgreSQL's row locks, `listener_lock_actor` and the catalog fence are replaced by
//! that ordering. A lost COMMIT acknowledgement is not a rollback; the client's retry of the same
//! operation id reads the receipt if the unit became durable. Times are the domain's Unix
//! milliseconds from the transaction's trusted stamp.
use super::{
    application::{
        bookmark_create_receipt, bookmark_delete_receipt, history_receipt, progress_receipt,
        saved_receipt, Access, Command, LibraryError, LibraryRepository, LibraryService, Mutation,
        PageData, PageKind, PageSnapshot, ReadFuture, ReadRequest, ReadResult, WriteFuture,
    },
    cursor::{CursorVault, PageAnchor},
    domain::{
        reconcile_bookmark_create, reconcile_bookmark_delete, reconcile_progress, reconcile_saved,
        resume_status, AudioIdentity, BookmarkCreateOutcome, BookmarkDeleteOutcome, BookmarkRecord,
        HistoryOutcome, HistoryState, ListenerId, ProgressOutcome, ProgressRecord, Revision,
        SavedOutcome, SavedRecord,
    },
};
use crate::contract::theatre::library as dto;
use crate::platform::store::Row;
use crate::{
    contexts::{
        catalog::domain::WorkId,
        media::{
            library_presentations, library_public_work, library_publications,
            library_work_presentations, ListenerError,
        },
    },
    platform::{
        identity_auth::session::SessionError,
        store::{active_actors, Store, StoreError, Tx},
    },
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
struct TursoLibrary(Store);
impl LibraryService {
    pub fn new(store: Store) -> Self {
        Self {
            repository: Arc::new(TursoLibrary(store)),
            cursors: Arc::new(CursorVault::default()),
            presented_cursors: Arc::new(CursorVault::default()),
        }
    }
}
fn storage(e: StoreError) -> LibraryError {
    match e {
        StoreError::CorruptRevision => LibraryError::Corrupt,
        StoreError::Unauthenticated => LibraryError::Unauthenticated,
        _ => LibraryError::Unavailable,
    }
}
fn database(e: crate::platform::store::Error) -> LibraryError {
    if e.is_integrity_violation() {
        LibraryError::Corrupt
    } else {
        LibraryError::Unavailable
    }
}
fn media(e: ListenerError) -> LibraryError {
    match e {
        ListenerError::InvalidRequest | ListenerError::InvalidCursor | ListenerError::Corrupt => {
            LibraryError::Corrupt
        }
        ListenerError::NotFound => LibraryError::NotFound,
        ListenerError::Unavailable | ListenerError::RenditionUnavailable => {
            LibraryError::Unavailable
        }
    }
}
fn number(n: u64) -> Result<i64, LibraryError> {
    i64::try_from(n).map_err(|_| LibraryError::Corrupt)
}
fn unsigned(n: i64) -> Result<u64, LibraryError> {
    u64::try_from(n).map_err(|_| LibraryError::Corrupt)
}
fn col<T: crate::platform::store::FromColumn>(r: &Row, key: &str) -> Result<T, LibraryError> {
    r.try_get(key).map_err(|_| LibraryError::Corrupt)
}
fn encode<T: Serialize>(v: &T) -> Result<(Vec<u8>, Vec<u8>), LibraryError> {
    let bytes = serde_json::to_vec(v).map_err(|_| LibraryError::Corrupt)?;
    let digest = Sha256::digest(&bytes).to_vec();
    Ok((bytes, digest))
}
fn decode<T: DeserializeOwned + Serialize>(
    r: &Row,
    bytes: &str,
    digest: &str,
    max: usize,
) -> Result<T, LibraryError> {
    let b: Vec<u8> = col(r, bytes)?;
    let d: Vec<u8> = col(r, digest)?;
    if b.is_empty() || b.len() > max || Sha256::digest(&b).as_slice() != d {
        return Err(LibraryError::Corrupt);
    }
    let v: T = serde_json::from_slice(&b).map_err(|_| LibraryError::Corrupt)?;
    if encode(&v)?.0 != b {
        return Err(LibraryError::Corrupt);
    }
    Ok(v)
}
fn match_text(r: &Row, key: &str, v: &str) -> Result<(), LibraryError> {
    if col::<String>(r, key)? != v {
        Err(LibraryError::Corrupt)
    } else {
        Ok(())
    }
}
fn match_number(r: &Row, key: &str, v: u64) -> Result<(), LibraryError> {
    if col::<i64>(r, key)? != number(v)? {
        Err(LibraryError::Corrupt)
    } else {
        Ok(())
    }
}
fn progress(r: &Row, listener: &ListenerId) -> Result<ProgressRecord, LibraryError> {
    match_text(r, "listener_id", listener.as_str())?;
    let v: dto::ProgressRecord = decode(r, "snapshot", "snapshot_digest", 8192)?;
    for (key, value) in [
        ("publication_id", &v.audio.publication_id),
        ("work_id", &v.audio.work_id),
        ("unit_id", &v.audio.unit_id),
        ("rendition_id", &v.audio.rendition_id),
        ("sha256", &v.audio.sha256),
    ] {
        match_text(r, key, value)?;
    }
    for (key, value) in [
        ("revision", v.revision),
        ("listener_order", v.listener_order),
        ("duration_ms", v.audio.duration_ms),
        ("position_ms", v.position_ms),
    ] {
        match_number(r, key, value)?;
    }
    if col::<i64>(r, "accepted_at")? != v.accepted_at_ms {
        return Err(LibraryError::Corrupt);
    }
    match_text(r, "completion", completion_text(v.completion))?;
    ProgressRecord::restore(v).map_err(|_| LibraryError::Corrupt)
}
/// The stored spelling, pinned by `CHECK (completion IN (...))` in `listener_library.sql` and the
/// `completion=` literal in the continue-listening query: the write and verify paths share it.
fn completion_text(completion: dto::Completion) -> &'static str {
    match completion {
        dto::Completion::InProgress => "in_progress",
        dto::Completion::Completed => "completed",
    }
}
fn saved(r: &Row, listener: &ListenerId) -> Result<SavedRecord, LibraryError> {
    match_text(r, "listener_id", listener.as_str())?;
    let v: dto::SavedRecord = decode(r, "snapshot", "snapshot_digest", 4096)?;
    match_text(r, "work_id", &v.work_id)?;
    match_number(r, "revision", v.revision)?;
    match_number(r, "listener_order", v.listener_order)?;
    if col::<bool>(r, "saved")? != v.saved || col::<i64>(r, "accepted_at")? != v.accepted_at_ms {
        return Err(LibraryError::Corrupt);
    }
    SavedRecord::restore(v).map_err(|_| LibraryError::Corrupt)
}
fn bookmark(r: &Row, listener: &ListenerId) -> Result<BookmarkRecord, LibraryError> {
    match_text(r, "listener_id", listener.as_str())?;
    let v: dto::BookmarkRecord = decode(r, "snapshot", "snapshot_digest", 4096)?;
    for (key, value) in [
        ("bookmark_id", &v.bookmark_id),
        ("publication_id", &v.audio.publication_id),
        ("work_id", &v.audio.work_id),
        ("unit_id", &v.audio.unit_id),
        ("rendition_id", &v.audio.rendition_id),
        ("sha256", &v.audio.sha256),
    ] {
        match_text(r, key, value)?;
    }
    for (key, value) in [
        ("listener_order", v.listener_order),
        ("duration_ms", v.audio.duration_ms),
        ("position_ms", v.position_ms),
    ] {
        match_number(r, key, value)?;
    }
    if col::<bool>(r, "deleted")? != v.deleted || col::<i64>(r, "accepted_at")? != v.accepted_at_ms
    {
        return Err(LibraryError::Corrupt);
    }
    BookmarkRecord::restore(v).map_err(|_| LibraryError::Corrupt)
}
async fn current_bookmark(
    tx: &Tx<'_>,
    listener: &ListenerId,
    id: &str,
) -> Result<Option<BookmarkRecord>, LibraryError> {
    tx.query_opt(
        "SELECT * FROM listener_bookmarks WHERE listener_id=?1 AND bookmark_id=?2",
        &[&listener.as_str(), &id],
    )
    .await
    .map_err(database)?
    .as_ref()
    .map(|r| bookmark(r, listener))
    .transpose()
}
async fn current_progress(
    tx: &Tx<'_>,
    listener: &ListenerId,
    id: &str,
) -> Result<Option<ProgressRecord>, LibraryError> {
    tx.query_opt(
        "SELECT * FROM listener_progress WHERE listener_id=?1 AND publication_id=?2",
        &[&listener.as_str(), &id],
    )
    .await
    .map_err(database)?
    .as_ref()
    .map(|r| progress(r, listener))
    .transpose()
}
async fn current_saved(
    tx: &Tx<'_>,
    listener: &ListenerId,
    work: &WorkId,
) -> Result<SavedRecord, LibraryError> {
    Ok(tx
        .query_opt(
            "SELECT * FROM listener_saved WHERE listener_id=?1 AND work_id=?2",
            &[&listener.as_str(), &work.as_str()],
        )
        .await
        .map_err(database)?
        .as_ref()
        .map(|r| saved(r, listener))
        .transpose()?
        .unwrap_or(SavedRecord::empty(work)))
}
/// Admission inside the unit that reads or writes. A principal earned from an Identity session
/// re-admits exactly that session at the unit's own stamp (live session, active actor, unexpired),
/// so a logout, revocation or deactivation either committed before this unit (and it is refused
/// with no effect) or waits behind it on the serialized writer. The re-admitted actor and session
/// must be the ones the principal was issued for; a principal vouched for by a trusted host
/// override has no session and is checked against the active actor only.
async fn admit(tx: &Tx<'_>, access: &Access) -> Result<(), LibraryError> {
    let listener = &access.listener;
    let Some(admission) = &access.admission else {
        let active = active_actors(tx, &[listener.as_str().to_owned()])
            .await
            .map_err(storage)?;
        return if active.contains(listener.as_str()) {
            Ok(())
        } else {
            Err(LibraryError::Unauthenticated)
        };
    };
    let record =
        crate::platform::identity_auth::turso::admit_in(tx, admission.token(), admission.channel())
            .await
            .map_err(|e| match e {
                SessionError::Unavailable | SessionError::Unknown => LibraryError::Unavailable,
                SessionError::Inactive | SessionError::Expired | SessionError::Replay => {
                    LibraryError::Unauthenticated
                }
            })?;
    if record.actor_id != listener.as_str() || record.session_id != admission.session_id() {
        return Err(LibraryError::Unauthenticated);
    }
    Ok(())
}
/// The unit's trusted instant in the domain's Unix milliseconds.
fn clock(tx: &Tx<'_>) -> Result<i64, LibraryError> {
    let now = crate::contexts::media::unit_millis(tx);
    if now < 0 {
        Err(LibraryError::Corrupt)
    } else {
        Ok(now)
    }
}
async fn account(
    tx: &Tx<'_>,
    listener: &ListenerId,
) -> Result<(Revision, HistoryState), LibraryError> {
    let Some(r) = tx
        .query_opt(
            "SELECT * FROM listener_state WHERE listener_id=?1",
            &[&listener.as_str()],
        )
        .await
        .map_err(database)?
    else {
        return Ok((
            Revision::new(0).map_err(|_| LibraryError::Corrupt)?,
            HistoryState::restore(0, 0).map_err(|_| LibraryError::Corrupt)?,
        ));
    };
    Ok((
        Revision::new(unsigned(col(&r, "revision")?)?).map_err(|_| LibraryError::Corrupt)?,
        HistoryState::restore(
            unsigned(col(&r, "history_revision")?)?,
            unsigned(col(&r, "history_epoch")?)?,
        )
        .map_err(|_| LibraryError::Corrupt)?,
    ))
}
fn audio(
    manifest: &crate::contract::theatre::discovery::PublicationManifest,
) -> Result<AudioIdentity, LibraryError> {
    AudioIdentity::new(&dto::AudioIdentity {
        work_id: manifest.work_id.clone(),
        unit_id: manifest.unit_id.clone(),
        publication_id: manifest.publication_id.clone(),
        rendition_id: manifest.rendition_id.clone(),
        sha256: manifest.sha256.clone(),
        duration_ms: manifest.duration_ms,
    })
    .map_err(|_| LibraryError::Corrupt)
}
async fn require_audio(
    tx: &Tx<'_>,
    identity: &AudioIdentity,
    now: i64,
) -> Result<(), LibraryError> {
    let current = library_publications(tx, &[identity.unit().as_str().into()], now)
        .await
        .map_err(media)?;
    if !current
        .iter()
        .map(audio)
        .collect::<Result<Vec<_>, _>>()?
        .contains(identity)
    {
        return Err(LibraryError::NotFound);
    }
    Ok(())
}
async fn prior(
    tx: &Tx<'_>,
    listener: &ListenerId,
    operation: &str,
) -> Result<Option<(Vec<u8>, dto::Receipt)>, LibraryError> {
    tx.query_opt(
        "SELECT * FROM listener_operations WHERE listener_id=?1 AND operation_id=?2",
        &[&listener.as_str(), &operation],
    )
    .await
    .map_err(database)?
    .as_ref()
    .map(|r| {
        match_text(r, "listener_id", listener.as_str())?;
        let receipt: dto::Receipt = decode(r, "response_bytes", "response_digest", 16384)?;
        if receipt.operation_id != operation {
            return Err(LibraryError::Corrupt);
        }
        validate_receipt(&receipt)?;
        Ok((col(r, "request_digest")?, receipt))
    })
    .transpose()
}
fn validate_receipt(r: &dto::Receipt) -> Result<(), LibraryError> {
    super::domain::OperationId::parse(&r.operation_id).map_err(|_| LibraryError::Corrupt)?;
    let conflict = matches!(
        r.outcome,
        dto::WriteOutcome::RevisionConflict
            | dto::WriteOutcome::HistoryEpochConflict
            | dto::WriteOutcome::BookmarkDeleted
    );
    if conflict != r.resolution.is_some() {
        return Err(LibraryError::Corrupt);
    }
    match &r.state {
        dto::MutationState::Progress { current } => {
            if !conflict && current.is_none() {
                return Err(LibraryError::Corrupt);
            }
            if let Some(v) = current {
                ProgressRecord::restore(v.clone()).map_err(|_| LibraryError::Corrupt)?;
            }
        }
        dto::MutationState::Saved { current } => {
            SavedRecord::restore(current.clone()).map_err(|_| LibraryError::Corrupt)?;
        }
        dto::MutationState::History { current } => {
            HistoryState::restore(current.revision, current.epoch)
                .map_err(|_| LibraryError::Corrupt)?;
        }
        dto::MutationState::Bookmark { current } => {
            BookmarkRecord::restore(current.clone()).map_err(|_| LibraryError::Corrupt)?;
            // A refused create names the tombstone that refused it.
            if r.outcome == dto::WriteOutcome::BookmarkDeleted && !current.deleted {
                return Err(LibraryError::Corrupt);
            }
        }
    }
    Ok(())
}
async fn apply_progress(
    tx: &Tx<'_>,
    listener: &ListenerId,
    v: &ProgressRecord,
    base: u64,
) -> Result<(), LibraryError> {
    let v = v.raw();
    let (b, d) = encode(v)?;
    let completion = completion_text(v.completion);
    let fields: [&(dyn crate::platform::store::Param + Sync); 14] = [
        &listener.as_str(),
        &v.audio.publication_id,
        &v.audio.work_id,
        &v.audio.unit_id,
        &v.audio.rendition_id,
        &v.audio.sha256,
        &number(v.audio.duration_ms)?,
        &number(v.position_ms)?,
        &completion,
        &number(v.revision)?,
        &number(v.listener_order)?,
        &v.accepted_at_ms,
        &b,
        &d,
    ];
    let count = if base == 0 {
        tx.execute(
            "INSERT INTO listener_progress(listener_id,publication_id,work_id,unit_id,rendition_id,sha256,duration_ms,position_ms,completion,revision,listener_order,accepted_at,snapshot,snapshot_digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            &fields,
        )
        .await
        .map_err(database)?
    } else {
        let expected_base = number(base)?;
        let params = fields
            .iter()
            .copied()
            .chain(std::iter::once(
                &expected_base as &(dyn crate::platform::store::Param + Sync),
            ))
            .collect::<Vec<_>>();
        tx.execute("UPDATE listener_progress SET position_ms=?8,completion=?9,revision=?10,listener_order=?11,accepted_at=?12,snapshot=?13,snapshot_digest=?14 WHERE listener_id=?1 AND publication_id=?2 AND work_id=?3 AND unit_id=?4 AND rendition_id=?5 AND sha256=?6 AND duration_ms=?7 AND revision=?15",&params).await.map_err(database)?
    };
    if count != 1 {
        return Err(LibraryError::Corrupt);
    }
    Ok(())
}
async fn apply_bookmark(
    tx: &Tx<'_>,
    listener: &ListenerId,
    v: &BookmarkRecord,
    existing: bool,
) -> Result<(), LibraryError> {
    let v = v.raw();
    let (b, d) = encode(v)?;
    let count = if existing {
        // Only the final tombstone is ever written over an accepted value (see the table guard).
        tx.execute(
            "UPDATE listener_bookmarks SET deleted=?3,listener_order=?4,accepted_at=?5,snapshot=?6,snapshot_digest=?7 WHERE listener_id=?1 AND bookmark_id=?2 AND deleted=0",
            &[
                &listener.as_str(),
                &v.bookmark_id,
                &v.deleted,
                &number(v.listener_order)?,
                &v.accepted_at_ms,
                &b,
                &d,
            ],
        )
        .await
        .map_err(database)?
    } else {
        tx.execute(
            "INSERT INTO listener_bookmarks(listener_id,bookmark_id,publication_id,work_id,unit_id,rendition_id,sha256,duration_ms,position_ms,deleted,listener_order,accepted_at,snapshot,snapshot_digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            &[
                &listener.as_str(),
                &v.bookmark_id,
                &v.audio.publication_id,
                &v.audio.work_id,
                &v.audio.unit_id,
                &v.audio.rendition_id,
                &v.audio.sha256,
                &number(v.audio.duration_ms)?,
                &number(v.position_ms)?,
                &v.deleted,
                &number(v.listener_order)?,
                &v.accepted_at_ms,
                &b,
                &d,
            ],
        )
        .await
        .map_err(database)?
    };
    if count != 1 {
        return Err(LibraryError::Corrupt);
    }
    Ok(())
}
async fn apply_saved(
    tx: &Tx<'_>,
    listener: &ListenerId,
    v: &SavedRecord,
    base: u64,
) -> Result<(), LibraryError> {
    let v = v.raw();
    let (b, d) = encode(v)?;
    // An explicit insert or conditional update, never an upsert: the table refuses an insert over
    // an existing key, and the update proves the revision it replaces.
    let count = if base == 0 {
        tx.execute(
            "INSERT INTO listener_saved(listener_id,work_id,saved,revision,listener_order,accepted_at,snapshot,snapshot_digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            &[
                &listener.as_str(),
                &v.work_id,
                &v.saved,
                &number(v.revision)?,
                &number(v.listener_order)?,
                &v.accepted_at_ms,
                &b,
                &d,
            ],
        )
        .await
        .map_err(database)?
    } else {
        tx.execute(
            "UPDATE listener_saved SET saved=?3,revision=?4,listener_order=?5,accepted_at=?6,snapshot=?7,snapshot_digest=?8 WHERE listener_id=?1 AND work_id=?2 AND revision=?9",
            &[
                &listener.as_str(),
                &v.work_id,
                &v.saved,
                &number(v.revision)?,
                &number(v.listener_order)?,
                &v.accepted_at_ms,
                &b,
                &d,
                &number(base)?,
            ],
        )
        .await
        .map_err(database)?
    };
    if count != 1 {
        return Err(LibraryError::Corrupt);
    }
    Ok(())
}
impl LibraryRepository for TursoLibrary {
    fn mutate(&self, access: Access, request: Mutation) -> WriteFuture {
        let store = self.0.clone();
        Box::pin(async move {
            let listener = access.listener.clone();
            let mut connection = store.connection().await.map_err(storage)?;
            let tx = connection.transaction().await.map_err(database)?;
            // Re-admit the session inside this unit before anything is decided or written.
            admit(&tx, &access).await?;
            // The account row is the order authority of the account, not a parallel progress
            // store. Created once, by a pre-checked insert (the table refuses an insert over an
            // existing key, so there is no conflict clause to lean on).
            tx.execute(
                "INSERT INTO listener_state(listener_id) SELECT ?1 WHERE NOT EXISTS (SELECT 1 FROM listener_state WHERE listener_id=?1)",
                &[&listener.as_str()],
            )
            .await
            .map_err(database)?;
            let (order, h) = account(&tx, &listener).await?;
            let intent = request.command.intent();
            if let Some((digest, receipt)) =
                prior(&tx, &listener, intent.operation().as_str()).await?
            {
                if digest != request.fingerprint {
                    return Err(LibraryError::OperationReused);
                }
                tx.commit().await.map_err(database)?;
                return Ok(receipt);
            }
            let key = request.command.sequence_key();
            let previous=tx.query_opt("SELECT sequence FROM listener_device_sequences WHERE listener_id=?1 AND device_id=?2 AND resource_key=?3",&[&listener.as_str(),&intent.device().as_str(),&key]).await.map_err(database)?;
            if previous
                .as_ref()
                .map(|r| col::<i64>(r, "sequence"))
                .transpose()?
                .is_some_and(|seq| seq >= intent.sequence() as i64)
            {
                return Err(LibraryError::StaleSequence);
            }
            let next = order.next()?;
            let now = clock(&tx)?;
            let mut admitted_audio = None;
            let mut admitted_work = None;
            let (receipt, changed, history) = match &request.command {
                Command::Progress(c) => {
                    let current =
                        current_progress(&tx, &listener, c.audio().publication().as_str()).await?;
                    if current.is_none() {
                        require_audio(&tx, c.audio(), now).await?;
                        admitted_audio = Some(c.audio().clone());
                    }
                    let out = reconcile_progress(current.as_ref(), c, now, next)?;
                    let changed = if let ProgressOutcome::Accepted(v) = &out {
                        apply_progress(
                            &tx,
                            &listener,
                            v,
                            current.as_ref().map_or(0, |r| r.raw().revision),
                        )
                        .await?;
                        true
                    } else {
                        false
                    };
                    (progress_receipt(intent.operation(), &out), changed, h)
                }
                Command::Saved(c) => {
                    let current = current_saved(&tx, &listener, c.work()).await?;
                    if c.desired() && current.raw().revision == 0 {
                        if !library_public_work(&tx, c.work().as_str(), now)
                            .await
                            .map_err(media)?
                        {
                            return Err(LibraryError::NotFound);
                        }
                        admitted_work = Some(c.work().clone());
                    }
                    let out = reconcile_saved(&current, c, now, next)?;
                    let changed = if let SavedOutcome::Accepted(v) = &out {
                        apply_saved(&tx, &listener, v, current.raw().revision).await?;
                        true
                    } else {
                        false
                    };
                    (saved_receipt(intent.operation(), &out), changed, h)
                }
                Command::Bookmark(c) => {
                    let current = current_bookmark(&tx, &listener, c.id().as_str()).await?;
                    // Only a new bookmark needs the exact audio to be public now; an existing one
                    // (live or tombstoned) is decided from its stored value alone.
                    if current.is_none() {
                        require_audio(&tx, c.audio(), now).await?;
                        admitted_audio = Some(c.audio().clone());
                    }
                    let out = reconcile_bookmark_create(current.as_ref(), c, now, next)?;
                    let changed = if let BookmarkCreateOutcome::Accepted(v) = &out {
                        apply_bookmark(&tx, &listener, v, false).await?;
                        true
                    } else {
                        false
                    };
                    (
                        bookmark_create_receipt(intent.operation(), &out),
                        changed,
                        h,
                    )
                }
                Command::BookmarkDelete(c) => {
                    let current = current_bookmark(&tx, &listener, c.id().as_str()).await?;
                    let out = reconcile_bookmark_delete(current.as_ref(), c, now, next)?;
                    let changed = if let BookmarkDeleteOutcome::Accepted(v) = &out {
                        apply_bookmark(&tx, &listener, v, true).await?;
                        true
                    } else {
                        false
                    };
                    let Some(receipt) = bookmark_delete_receipt(intent.operation(), &out) else {
                        // Nothing to tombstone: no receipt, no effect, the transaction rolls back.
                        return Err(LibraryError::NotFound);
                    };
                    (receipt, changed, h)
                }
                Command::Start(c) => {
                    let seen=tx.query_opt("SELECT audio_snapshot FROM listener_playback_sessions WHERE listener_id=?1 AND device_id=?2 AND engine_session_id=?3",&[&listener.as_str(),&intent.device().as_str(),&c.session().as_str()]).await.map_err(database)?;
                    let (b, d) = encode(&c.audio().raw())?;
                    if seen
                        .as_ref()
                        .map(|r| col::<Vec<u8>>(r, "audio_snapshot"))
                        .transpose()?
                        .is_some_and(|v| v != b)
                    {
                        return Err(LibraryError::OperationReused);
                    }
                    let out = h.start(c, seen.is_some())?;
                    let history = if let HistoryOutcome::Accepted(next_h) = out {
                        require_audio(&tx, c.audio(), now).await?;
                        admitted_audio = Some(c.audio().clone());
                        tx.execute(
                            "INSERT INTO listener_playback_sessions(listener_id,device_id,engine_session_id,audio_snapshot) VALUES(?1,?2,?3,?4)",
                            &[
                                &listener.as_str(),
                                &intent.device().as_str(),
                                &c.session().as_str(),
                                &b,
                            ],
                        )
                        .await
                        .map_err(database)?;
                        let raw = c.audio().raw();
                        tx.execute("INSERT INTO listener_history(listener_id,work_id,publication_id,unit_id,listener_order,started_at,snapshot,snapshot_digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(listener_id,work_id) DO UPDATE SET publication_id=EXCLUDED.publication_id,unit_id=EXCLUDED.unit_id,listener_order=EXCLUDED.listener_order,started_at=EXCLUDED.started_at,snapshot=EXCLUDED.snapshot,snapshot_digest=EXCLUDED.snapshot_digest",&[&listener.as_str(),&raw.work_id,&raw.publication_id,&raw.unit_id,&number(next.value())?,&now,&b,&d]).await.map_err(database)?;
                        next_h
                    } else {
                        h
                    };
                    (
                        history_receipt(intent.operation(), out),
                        matches!(out, HistoryOutcome::Accepted(_)),
                        history,
                    )
                }
                Command::Clear(c) => {
                    let out = h.clear(c)?;
                    let history = if let HistoryOutcome::Accepted(next_h) = out {
                        tx.execute(
                            "DELETE FROM listener_history WHERE listener_id=?1",
                            &[&listener.as_str()],
                        )
                        .await
                        .map_err(database)?;
                        next_h
                    } else {
                        h
                    };
                    (
                        history_receipt(intent.operation(), out),
                        matches!(out, HistoryOutcome::Accepted(_)),
                        history,
                    )
                }
            };
            if changed {
                let hs = history.raw();
                let count=tx.execute("UPDATE listener_state SET revision=?2,history_revision=?3,history_epoch=?4 WHERE listener_id=?1 AND revision=?5",&[&listener.as_str(),&number(next.value())?,&number(hs.revision)?,&number(hs.epoch)?,&number(order.value())?]).await.map_err(database)?;
                if count != 1 {
                    return Err(LibraryError::Corrupt);
                }
            }
            tx.execute("INSERT INTO listener_device_sequences(listener_id,device_id,resource_key,sequence) VALUES(?1,?2,?3,?4) ON CONFLICT(listener_id,device_id,resource_key) DO UPDATE SET sequence=EXCLUDED.sequence WHERE listener_device_sequences.sequence<EXCLUDED.sequence",&[&listener.as_str(),&intent.device().as_str(),&key,&number(intent.sequence())?]).await.map_err(database)?;
            validate_receipt(&receipt)?;
            let (b, d) = encode(&receipt)?;
            tx.execute(
                "INSERT INTO listener_operations(listener_id,operation_id,request_digest,response_bytes,response_digest) VALUES(?1,?2,?3,?4,?5)",
                &[
                    &listener.as_str(),
                    &intent.operation().as_str(),
                    &request.fingerprint,
                    &b,
                    &d,
                ],
            )
            .await
            .map_err(database)?;
            // The unit holds the writer from its stamp to its commit and reads and writes nothing
            // that can change under it, so the identities admitted above are still current here.
            // Re-checking them against the finished effects keeps the original order of proof:
            // admission is decided after every effect and immediately before the atomic commit.
            if let Some(a) = admitted_audio {
                require_audio(&tx, &a, now).await?;
            }
            if let Some(w) = admitted_work {
                if !library_public_work(&tx, w.as_str(), now)
                    .await
                    .map_err(media)?
                {
                    return Err(LibraryError::NotFound);
                }
            }
            tx.commit().await.map_err(database)?;
            Ok(receipt)
        })
    }
    fn read(&self, access: Access, request: ReadRequest) -> ReadFuture {
        let store = self.0.clone();
        Box::pin(async move {
            let listener = access.listener.clone();
            let connection = store.connection().await.map_err(storage)?;
            let tx = connection.read().await.map_err(database)?;
            // A read is admitted at its own pinned snapshot only; a write re-admits in its unit.
            admit(&tx, &access).await?;
            let result = match request {
                ReadRequest::Progress(id) => {
                    ReadResult::Progress(current_progress(&tx, &listener, &id).await?)
                }
                ReadRequest::Saved(work) => {
                    ReadResult::Saved(current_saved(&tx, &listener, &work).await?)
                }
                ReadRequest::Receipt(op) => ReadResult::Receipt(
                    prior(&tx, &listener, op.as_str())
                        .await?
                        .ok_or(LibraryError::NotFound)?
                        .1,
                ),
                ReadRequest::Page {
                    kind,
                    limit,
                    anchor,
                    presentation,
                } => {
                    ReadResult::Page(page(&tx, &listener, kind, limit, anchor, presentation).await?)
                }
            };
            tx.commit().await.map_err(database)?;
            Ok(result)
        })
    }
}
async fn page(
    tx: &Tx<'_>,
    listener: &ListenerId,
    kind: PageKind,
    limit: usize,
    anchor: Option<PageAnchor>,
    presentation: bool,
) -> Result<PageSnapshot, LibraryError> {
    let (order, history) = account(tx, listener).await?;
    let visibility_scope = crate::contexts::catalog::shared_scope(tx)
        .await
        .map_err(storage)?;
    if anchor.as_ref().is_some_and(|a| {
        a.visibility_scope != visibility_scope
            || (a.history_epoch != history.raw().epoch && kind == PageKind::History)
    }) {
        return Err(LibraryError::InvalidCursor);
    }
    let upper = anchor.as_ref().map_or(order.value(), |a| a.upper_order);
    let after = anchor.as_ref().map(|a| number(a.after_order)).transpose()?;
    let after_work = anchor.as_ref().map(|a| a.after_work.as_str());
    let count = (limit + 1) as i64;
    // Continue chooses the latest account record first. HIDE then precedes keyset/LIMIT;
    // filtering must never fall back to an older chapter of that Work. Private Library
    // restoration remains unsupported even for a currently authorized shared viewer.
    let visible = crate::contexts::catalog::library_visibility(kind != PageKind::Saved);
    let query=match kind{
        PageKind::Continue=>format!("SELECT * FROM (SELECT *,row_number() OVER (PARTITION BY work_id ORDER BY listener_order DESC,publication_id) AS pick FROM listener_progress WHERE listener_id=?1) x WHERE pick=1 AND completion='in_progress' AND {visible} AND listener_order<=?2 AND (?3 IS NULL OR listener_order<?3 OR (listener_order=?3 AND work_id>?4)) ORDER BY listener_order DESC,work_id LIMIT ?5"),
        PageKind::Saved=>format!("SELECT * FROM listener_saved x WHERE listener_id=?1 AND saved=1 AND {visible} AND listener_order<=?2 AND (?3 IS NULL OR listener_order<?3 OR (listener_order=?3 AND work_id>?4)) ORDER BY listener_order DESC,work_id LIMIT ?5"),
        PageKind::Bookmarks=>format!("SELECT * FROM listener_bookmarks x WHERE listener_id=?1 AND deleted=0 AND {visible} AND listener_order<=?2 AND (?3 IS NULL OR listener_order<?3 OR (listener_order=?3 AND bookmark_id>?4)) ORDER BY listener_order DESC,bookmark_id LIMIT ?5"),
        PageKind::History=>format!("SELECT * FROM listener_history x WHERE listener_id=?1 AND {visible} AND listener_order<=?2 AND (?3 IS NULL OR listener_order<?3 OR (listener_order=?3 AND work_id>?4)) ORDER BY listener_order DESC,work_id LIMIT ?5"),
    };
    let mut rows = tx
        .query(
            &query,
            &[
                &listener.as_str(),
                &number(upper)?,
                &after,
                &after_work,
                &count,
            ],
        )
        .await
        .map_err(database)?;
    let more = rows.len() > limit;
    rows.truncate(limit);
    let next = if more {
        let last = rows.last().ok_or(LibraryError::Corrupt)?;
        Some(PageAnchor {
            upper_order: upper,
            after_order: unsigned(col(last, "listener_order")?)?,
            after_work: col(
                last,
                if kind == PageKind::Bookmarks {
                    "bookmark_id"
                } else {
                    "work_id"
                },
            )?,
            history_epoch: history.raw().epoch,
            visibility_scope,
        })
    } else {
        None
    };
    let data = match kind {
        PageKind::Saved => {
            let records = rows
                .iter()
                .map(|r| saved(r, listener))
                .collect::<Result<Vec<_>, _>>()?;
            let mut metadata: BTreeMap<_, _> = if presentation {
                let works = records
                    .iter()
                    .map(|r| r.raw().work_id.clone())
                    .collect::<Vec<_>>();
                library_work_presentations(tx, &works, clock(tx)?)
                    .await
                    .map_err(media)?
                    .into_iter()
                    .map(|m| (m.work_id.clone(), m))
                    .collect()
            } else {
                BTreeMap::new()
            };
            PageData::Saved(
                records
                    .into_iter()
                    .map(|r| dto::PresentedSavedItem {
                        presentation: metadata.remove(&r.raw().work_id),
                        saved: r.raw().clone(),
                    })
                    .collect(),
            )
        }
        PageKind::Continue => {
            let records = rows
                .iter()
                .map(|r| progress(r, listener))
                .collect::<Result<Vec<_>, _>>()?;
            let count = records.len();
            let records = super::domain::projection::continue_records(records);
            if records.len() != count {
                return Err(LibraryError::Corrupt);
            }
            let identities = records
                .iter()
                .map(|r| AudioIdentity::new(&r.raw().audio).map_err(|_| LibraryError::Corrupt))
                .collect::<Result<Vec<_>, _>>()?;
            let (current, metadata) = current_audio(tx, &identities, presentation).await?;
            PageData::Continue(
                records
                    .into_iter()
                    .zip(identities)
                    .map(|(r, a)| {
                        Ok(dto::PresentedContinueItem {
                            presentation: exact_presentation(&a, &current, &metadata)?,
                            resume: resume_status(
                                &a,
                                current
                                    .get(a.unit().as_str())
                                    .filter(|v| v.publication() == a.publication()),
                                current.get(a.unit().as_str()),
                            ),
                            progress: r.raw().clone(),
                        })
                    })
                    .collect::<Result<_, LibraryError>>()?,
            )
        }
        PageKind::Bookmarks => {
            let records = rows
                .iter()
                .map(|r| bookmark(r, listener))
                .collect::<Result<Vec<_>, _>>()?;
            let identities = records
                .iter()
                .map(|r| r.audio().map_err(|_| LibraryError::Corrupt))
                .collect::<Result<Vec<_>, _>>()?;
            // Metadata is always requested: a new resource has no strict legacy reader to keep.
            let (current, metadata) = current_audio(tx, &identities, true).await?;
            PageData::Bookmarks(
                records
                    .into_iter()
                    .zip(identities)
                    .map(|(r, a)| {
                        Ok(dto::BookmarkItem {
                            presentation: exact_presentation(&a, &current, &metadata)?,
                            resume: resume_status(
                                &a,
                                current
                                    .get(a.unit().as_str())
                                    .filter(|v| v.publication() == a.publication()),
                                current.get(a.unit().as_str()),
                            ),
                            bookmark: r.raw().clone(),
                        })
                    })
                    .collect::<Result<_, LibraryError>>()?,
            )
        }
        PageKind::History => {
            let mut items = Vec::new();
            let mut identities = Vec::new();
            for r in rows {
                match_text(&r, "listener_id", listener.as_str())?;
                let raw: dto::AudioIdentity = decode(&r, "snapshot", "snapshot_digest", 4096)?;
                let a = AudioIdentity::new(&raw).map_err(|_| LibraryError::Corrupt)?;
                for (key, value) in [
                    ("work_id", &raw.work_id),
                    ("unit_id", &raw.unit_id),
                    ("publication_id", &raw.publication_id),
                ] {
                    match_text(&r, key, value)?;
                }
                let started_at = col::<i64>(&r, "started_at")?;
                if started_at < 0 {
                    return Err(LibraryError::Corrupt);
                }
                let order = unsigned(col(&r, "listener_order")?)?;
                if order == 0 {
                    return Err(LibraryError::Corrupt);
                }
                items.push(dto::PresentedHistoryItem {
                    audio: raw,
                    started_at_ms: started_at,
                    listener_order: order,
                    progress: None,
                    resume: dto::ResumeStatus::AccessUnavailable,
                    presentation: None,
                });
                identities.push(a);
            }
            let pubs = identities
                .iter()
                .map(|a| a.publication().as_str().to_owned())
                .collect::<Vec<_>>();
            let progress_rows=tx.query("SELECT * FROM listener_progress WHERE listener_id=?1 AND publication_id IN (SELECT value FROM json_each(?2))",&[&listener.as_str(),&pubs]).await.map_err(database)?;
            let progress = progress_rows
                .iter()
                .map(|r| {
                    progress(r, listener)
                        .map(|v| (v.raw().audio.publication_id.clone(), v.raw().clone()))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let (current, metadata) = current_audio(tx, &identities, presentation).await?;
            for (item, a) in items.iter_mut().zip(identities) {
                item.progress = progress.get(a.publication().as_str()).cloned();
                if item
                    .progress
                    .as_ref()
                    .is_some_and(|p| p.audio != item.audio)
                {
                    return Err(LibraryError::Corrupt);
                }
                item.resume = resume_status(
                    &a,
                    current
                        .get(a.unit().as_str())
                        .filter(|v| v.publication() == a.publication()),
                    current.get(a.unit().as_str()),
                );
                item.presentation = exact_presentation(&a, &current, &metadata)?;
            }
            PageData::History {
                state: history.raw(),
                items,
            }
        }
    };
    Ok(PageSnapshot { data, next })
}
async fn current_audio(
    tx: &Tx<'_>,
    identities: &[AudioIdentity],
    presentation: bool,
) -> Result<
    (
        BTreeMap<String, AudioIdentity>,
        BTreeMap<String, dto::AudioPresentation>,
    ),
    LibraryError,
> {
    let units = identities
        .iter()
        .map(|a| a.unit().as_str().to_owned())
        .collect::<Vec<_>>();
    let (manifests, metadata) = if presentation {
        library_presentations(tx, &units, clock(tx)?)
            .await
            .map_err(media)?
            .into_iter()
            .unzip::<_, _, Vec<_>, Vec<_>>()
    } else {
        (
            library_publications(tx, &units, clock(tx)?)
                .await
                .map_err(media)?,
            Vec::new(),
        )
    };
    let current = manifests
        .iter()
        .map(|m| audio(m).map(|a| (a.unit().as_str().to_owned(), a)))
        .collect::<Result<_, _>>()?;
    let metadata = metadata
        .into_iter()
        .map(|m| (m.unit_id.clone(), m))
        .collect();
    Ok((current, metadata))
}

/// An account record keeps its exact audio. Replacement metadata cannot label that audio.
fn exact_presentation(
    identity: &AudioIdentity,
    current: &BTreeMap<String, AudioIdentity>,
    metadata: &BTreeMap<String, dto::AudioPresentation>,
) -> Result<Option<dto::AudioPresentation>, LibraryError> {
    if current.get(identity.unit().as_str()) != Some(identity) {
        return Ok(None);
    }
    let Some(metadata) = metadata.get(identity.unit().as_str()) else {
        return Ok(None);
    };
    if metadata.work.work_id != identity.work().as_str()
        || metadata.work.publication_id != identity.publication().as_str()
        || metadata.unit_id != identity.unit().as_str()
    {
        return Err(LibraryError::Corrupt);
    }
    Ok(Some(metadata.clone()))
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    fn identity() -> AudioIdentity {
        AudioIdentity::new(&dto::AudioIdentity {
            work_id: "00000000-0000-4000-8000-000000000001".into(),
            unit_id: "00000000-0000-4000-8000-000000000002".into(),
            publication_id: "00000000-0000-4000-8000-000000000003".into(),
            rendition_id: "00000000-0000-4000-8000-000000000004".into(),
            sha256: "a".repeat(64),
            duration_ms: 60000,
        })
        .unwrap()
    }
    #[test]
    fn presentation_requires_the_entire_exact_audio_identity_and_matching_release() {
        let a = identity();
        let metadata = dto::AudioPresentation {
            work: dto::WorkPresentation {
                work_id: a.work().as_str().into(),
                publication_id: a.publication().as_str().into(),
                title: "Ánh đèn cuối sân khấu".into(),
                kind: crate::contract::theatre::discovery::WorkKind::Story,
                language: "vi-VN".into(),
            },
            unit_id: a.unit().as_str().into(),
            unit_title: "Một lời hẹn".into(),
        };
        let current = BTreeMap::from([(a.unit().as_str().into(), a.clone())]);
        let mut presentations = BTreeMap::from([(a.unit().as_str().into(), metadata.clone())]);
        assert_eq!(
            exact_presentation(&a, &current, &presentations),
            Ok(Some(metadata.clone()))
        );
        assert_eq!(
            exact_presentation(&a, &BTreeMap::new(), &presentations),
            Ok(None)
        );
        let mut changed = a.raw();
        changed.rendition_id = "00000000-0000-4000-8000-000000000005".into();
        let replacement = BTreeMap::from([(
            a.unit().as_str().into(),
            AudioIdentity::new(&changed).unwrap(),
        )]);
        assert_eq!(
            exact_presentation(&a, &replacement, &presentations),
            Ok(None)
        );
        for field in ["work", "unit", "publication"] {
            let mut invalid = metadata.clone();
            let foreign = "00000000-0000-4000-8000-000000000006".into();
            match field {
                "work" => invalid.work.work_id = foreign,
                "unit" => invalid.unit_id = foreign,
                "publication" => invalid.work.publication_id = foreign,
                _ => unreachable!(),
            }
            presentations.insert(a.unit().as_str().into(), invalid);
            assert_eq!(
                exact_presentation(&a, &current, &presentations),
                Err(LibraryError::Corrupt)
            );
        }
    }
}

#[cfg(test)]
mod tests;
