//! PostgreSQL is the sole durable Library authority. No demo state or provider calls.
use super::{
    application::{
        history_receipt, progress_receipt, saved_receipt, Command, LibraryError, LibraryRepository,
        LibraryService, Mutation, PageData, PageKind, PageSnapshot, ReadFuture, ReadRequest,
        ReadResult, WriteFuture,
    },
    cursor::{CursorVault, PageAnchor},
    domain::{
        reconcile_progress, reconcile_saved, resume_status, AudioIdentity, HistoryOutcome,
        HistoryState, ListenerId, ProgressOutcome, ProgressRecord, Revision, SavedOutcome,
        SavedRecord,
    },
};
use crate::{
    contexts::{
        catalog::{domain::WorkId, lock_listener_catalog},
        media::{library_public_work, library_publications, ListenerError},
    },
    platform::postgres::{Store, StoreError, Tx},
};
use cantos_contracts::listener_library as dto;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use tokio_postgres::{IsolationLevel, Row};
struct PgLibrary(Store);
impl LibraryService {
    pub fn new(store: Store) -> Self {
        Self {
            repository: Arc::new(PgLibrary(store)),
            cursors: Arc::new(CursorVault::default()),
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
fn database(e: tokio_postgres::Error) -> LibraryError {
    if e.code().is_some_and(|c| {
        c.code().starts_with("23") || c.code().starts_with("42") || c.code() == "55000"
    }) {
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
fn col<T: tokio_postgres::types::FromSqlOwned>(r: &Row, key: &str) -> Result<T, LibraryError> {
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
    match_text(
        r,
        "completion",
        match v.completion {
            dto::Completion::InProgress => "in_progress",
            dto::Completion::Completed => "completed",
        },
    )?;
    ProgressRecord::restore(v).map_err(|_| LibraryError::Corrupt)
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
async fn current_progress(
    tx: &Tx<'_>,
    listener: &ListenerId,
    id: &str,
) -> Result<Option<ProgressRecord>, LibraryError> {
    tx.query_opt(
        "SELECT * FROM listener_progress WHERE listener_id=$1 AND publication_id=$2",
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
            "SELECT * FROM listener_saved WHERE listener_id=$1 AND work_id=$2",
            &[&listener.as_str(), &work.as_str()],
        )
        .await
        .map_err(database)?
        .as_ref()
        .map(|r| saved(r, listener))
        .transpose()?
        .unwrap_or(SavedRecord::empty(work)))
}
async fn active(tx: &Tx<'_>, listener: &ListenerId, lock: bool) -> Result<(), LibraryError> {
    let query = if lock {
        "SELECT listener_lock_actor($1) AS active"
    } else {
        "SELECT listener_actor_active($1) AS active"
    };
    let active: Option<bool> = col(
        &tx.query_one(query, &[&listener.as_str()])
            .await
            .map_err(database)?,
        "active",
    )?;
    if active != Some(true) {
        return Err(LibraryError::Unauthenticated);
    }
    Ok(())
}
async fn clock(tx: &Tx<'_>) -> Result<i64, LibraryError> {
    let r = tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint AS now",
            &[],
        )
        .await
        .map_err(database)?;
    let now = col::<i64>(&r, "now")?;
    if now < 0 {
        Err(LibraryError::Corrupt)
    } else {
        Ok(now)
    }
}
async fn account(
    tx: &Tx<'_>,
    listener: &ListenerId,
    lock: bool,
) -> Result<(Revision, HistoryState), LibraryError> {
    let query = if lock {
        "SELECT * FROM listener_state WHERE listener_id=$1 FOR UPDATE"
    } else {
        "SELECT * FROM listener_state WHERE listener_id=$1"
    };
    let Some(r) = tx
        .query_opt(query, &[&listener.as_str()])
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
    manifest: &cantos_contracts::theatre::PublicationManifest,
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
    _now: i64,
) -> Result<(), LibraryError> {
    lock_listener_catalog(tx).await.map_err(storage)?;
    let now = clock(tx).await?;
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
        "SELECT * FROM listener_operations WHERE listener_id=$1 AND operation_id=$2",
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
        dto::WriteOutcome::RevisionConflict | dto::WriteOutcome::HistoryEpochConflict
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
    let completion = match v.completion {
        dto::Completion::InProgress => "in_progress",
        dto::Completion::Completed => "completed",
    };
    let fields: [&(dyn tokio_postgres::types::ToSql + Sync); 14] = [
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
            "INSERT INTO listener_progress VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
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
                &expected_base as &(dyn tokio_postgres::types::ToSql + Sync),
            ))
            .collect::<Vec<_>>();
        tx.execute("UPDATE listener_progress SET position_ms=$8,completion=$9,revision=$10,listener_order=$11,accepted_at=$12,snapshot=$13,snapshot_digest=$14 WHERE listener_id=$1 AND publication_id=$2 AND work_id=$3 AND unit_id=$4 AND rendition_id=$5 AND sha256=$6 AND duration_ms=$7 AND revision=$15",&params).await.map_err(database)?
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
    let count=tx.execute("INSERT INTO listener_saved VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(listener_id,work_id) DO UPDATE SET saved=EXCLUDED.saved,revision=EXCLUDED.revision,listener_order=EXCLUDED.listener_order,accepted_at=EXCLUDED.accepted_at,snapshot=EXCLUDED.snapshot,snapshot_digest=EXCLUDED.snapshot_digest WHERE listener_saved.revision=$9",&[&listener.as_str(),&v.work_id,&v.saved,&number(v.revision)?,&number(v.listener_order)?,&v.accepted_at_ms,&b,&d,&number(base)?]).await.map_err(database)?;
    if count != 1 {
        return Err(LibraryError::Corrupt);
    }
    Ok(())
}
impl LibraryRepository for PgLibrary {
    fn mutate(&self, listener: ListenerId, request: Mutation) -> WriteFuture {
        let store = self.0.clone();
        Box::pin(async move {
            let mut connection = store.connection().await.map_err(storage)?;
            let tx = connection.transaction().await.map_err(database)?;
            active(&tx, &listener, true).await?;
            // The account row is an existing authority lock, not a parallel progress store.
            // It serializes first writes and duplicate operation claims before any decision.
            tx.execute(
                "INSERT INTO listener_state(listener_id) VALUES($1) ON CONFLICT DO NOTHING",
                &[&listener.as_str()],
            )
            .await
            .map_err(database)?;
            let (order, h) = account(&tx, &listener, true).await?;
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
            let previous=tx.query_opt("SELECT sequence FROM listener_device_sequences WHERE listener_id=$1 AND device_id=$2 AND resource_key=$3",&[&listener.as_str(),&intent.device().as_str(),&key]).await.map_err(database)?;
            if previous
                .as_ref()
                .map(|r| col::<i64>(r, "sequence"))
                .transpose()?
                .is_some_and(|seq| seq >= intent.sequence() as i64)
            {
                return Err(LibraryError::StaleSequence);
            }
            let next = order.next()?;
            let now = clock(&tx).await?;
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
                        lock_listener_catalog(&tx).await.map_err(storage)?;
                        if !library_public_work(&tx, c.work().as_str(), clock(&tx).await?)
                            .await
                            .map_err(media)?
                        {
                            return Err(LibraryError::NotFound);
                        }
                    }
                    if c.desired() && current.raw().revision == 0 {
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
                Command::Start(c) => {
                    let seen=tx.query_opt("SELECT audio_snapshot FROM listener_playback_sessions WHERE listener_id=$1 AND device_id=$2 AND engine_session_id=$3",&[&listener.as_str(),&intent.device().as_str(),&c.session().as_str()]).await.map_err(database)?;
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
                            "INSERT INTO listener_playback_sessions VALUES($1,$2,$3,$4)",
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
                        tx.execute("INSERT INTO listener_history VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(listener_id,work_id) DO UPDATE SET publication_id=EXCLUDED.publication_id,unit_id=EXCLUDED.unit_id,listener_order=EXCLUDED.listener_order,started_at=EXCLUDED.started_at,snapshot=EXCLUDED.snapshot,snapshot_digest=EXCLUDED.snapshot_digest",&[&listener.as_str(),&raw.work_id,&raw.publication_id,&raw.unit_id,&number(next.value())?,&now,&b,&d]).await.map_err(database)?;
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
                            "DELETE FROM listener_history WHERE listener_id=$1",
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
                let count=tx.execute("UPDATE listener_state SET revision=$2,history_revision=$3,history_epoch=$4 WHERE listener_id=$1 AND revision=$5",&[&listener.as_str(),&number(next.value())?,&number(hs.revision)?,&number(hs.epoch)?,&number(order.value())?]).await.map_err(database)?;
                if count != 1 {
                    return Err(LibraryError::Corrupt);
                }
            }
            tx.execute("INSERT INTO listener_device_sequences VALUES($1,$2,$3,$4) ON CONFLICT(listener_id,device_id,resource_key) DO UPDATE SET sequence=EXCLUDED.sequence WHERE listener_device_sequences.sequence<EXCLUDED.sequence",&[&listener.as_str(),&intent.device().as_str(),&key,&number(intent.sequence())?]).await.map_err(database)?;
            validate_receipt(&receipt)?;
            let (b, d) = encode(&receipt)?;
            tx.execute(
                "INSERT INTO listener_operations VALUES($1,$2,$3,$4,$5)",
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
            // Time passes even while Catalog/Media mutations are fenced. Re-admit new identities
            // with fresh database time after all effects and immediately before the atomic commit.
            if let Some(a) = admitted_audio {
                require_audio(&tx, &a, clock(&tx).await?).await?;
            }
            if let Some(w) = admitted_work {
                if !library_public_work(&tx, w.as_str(), clock(&tx).await?)
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
    fn read(&self, listener: ListenerId, request: ReadRequest) -> ReadFuture {
        let store = self.0.clone();
        Box::pin(async move {
            let mut connection = store.connection().await.map_err(storage)?;
            let tx = connection
                .build_transaction()
                .isolation_level(IsolationLevel::RepeatableRead)
                .read_only(true)
                .start()
                .await
                .map_err(database)?;
            active(&tx, &listener, false).await?;
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
                } => ReadResult::Page(page(&tx, &listener, kind, limit, anchor).await?),
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
) -> Result<PageSnapshot, LibraryError> {
    let (order, history) = account(tx, listener, false).await?;
    if anchor
        .as_ref()
        .is_some_and(|a| a.history_epoch != history.raw().epoch && kind == PageKind::History)
    {
        return Err(LibraryError::InvalidCursor);
    }
    let upper = anchor.as_ref().map_or(order.value(), |a| a.upper_order);
    let after = anchor.as_ref().map(|a| number(a.after_order)).transpose()?;
    let after_work = anchor.as_ref().map(|a| a.after_work.as_str());
    let count = (limit + 1) as i64;
    let query=match kind{
        PageKind::Continue=>"SELECT * FROM (SELECT DISTINCT ON (work_id COLLATE \"C\") * FROM listener_progress WHERE listener_id=$1 ORDER BY work_id COLLATE \"C\",listener_order DESC,publication_id COLLATE \"C\") latest WHERE completion='in_progress' AND listener_order<=$2 AND ($3::bigint IS NULL OR listener_order<$3 OR (listener_order=$3 AND work_id COLLATE \"C\">$4::text COLLATE \"C\")) ORDER BY listener_order DESC,work_id COLLATE \"C\" LIMIT $5",
        PageKind::Saved=>"SELECT * FROM listener_saved WHERE listener_id=$1 AND saved AND listener_order<=$2 AND ($3::bigint IS NULL OR listener_order<$3 OR (listener_order=$3 AND work_id COLLATE \"C\">$4::text COLLATE \"C\")) ORDER BY listener_order DESC,work_id COLLATE \"C\" LIMIT $5",
        PageKind::History=>"SELECT * FROM listener_history WHERE listener_id=$1 AND listener_order<=$2 AND ($3::bigint IS NULL OR listener_order<$3 OR (listener_order=$3 AND work_id COLLATE \"C\">$4::text COLLATE \"C\")) ORDER BY listener_order DESC,work_id COLLATE \"C\" LIMIT $5",
    };
    let mut rows = tx
        .query(
            query,
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
            after_work: col(last, "work_id")?,
            history_epoch: history.raw().epoch,
        })
    } else {
        None
    };
    let data = match kind {
        PageKind::Saved => PageData::Saved(
            rows.iter()
                .map(|r| {
                    saved(r, listener).map(|v| dto::SavedItem {
                        saved: v.raw().clone(),
                    })
                })
                .collect::<Result<_, _>>()?,
        ),
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
            let current = current_audio(tx, &identities).await?;
            PageData::Continue(
                records
                    .into_iter()
                    .zip(identities)
                    .map(|(r, a)| dto::ContinueItem {
                        resume: resume_status(
                            &a,
                            current
                                .get(a.unit().as_str())
                                .filter(|v| v.publication() == a.publication()),
                            current.get(a.unit().as_str()),
                        ),
                        progress: r.raw().clone(),
                    })
                    .collect(),
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
                items.push(dto::HistoryItem {
                    audio: raw,
                    started_at_ms: started_at,
                    listener_order: order,
                    progress: None,
                    resume: dto::ResumeStatus::AccessUnavailable,
                });
                identities.push(a);
            }
            let pubs = identities
                .iter()
                .map(|a| a.publication().as_str().to_owned())
                .collect::<Vec<_>>();
            let progress_rows=tx.query("SELECT * FROM listener_progress WHERE listener_id=$1 AND publication_id=ANY($2)",&[&listener.as_str(),&pubs]).await.map_err(database)?;
            let progress = progress_rows
                .iter()
                .map(|r| {
                    progress(r, listener)
                        .map(|v| (v.raw().audio.publication_id.clone(), v.raw().clone()))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let current = current_audio(tx, &identities).await?;
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
) -> Result<BTreeMap<String, AudioIdentity>, LibraryError> {
    let units = identities
        .iter()
        .map(|a| a.unit().as_str().to_owned())
        .collect::<Vec<_>>();
    let manifests = library_publications(tx, &units, clock(tx).await?)
        .await
        .map_err(media)?;
    manifests
        .iter()
        .map(|m| audio(m).map(|a| (a.unit().as_str().to_owned(), a)))
        .collect()
}

#[cfg(test)]
mod tests;
