//! Listener use cases and typed persistence boundary. Domain decisions execute under adapter locks.
use super::{
    auth::AuthenticatedListener,
    cursor::{CursorVault, PageAnchor},
    domain::{
        self, DomainError, HistoryClear, HistoryOutcome, HistoryStart, Intent, ListenerId,
        OperationId, ProgressCandidate, ProgressOutcome, ProgressRecord, SavedCandidate,
        SavedOutcome, SavedRecord,
    },
};
use crate::contexts::catalog::domain::WorkId;
use cantos_contracts::listener_library as dto;
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryError {
    InvalidRequest,
    InvalidPosition,
    InvalidCursor,
    Unauthenticated,
    NotFound,
    OperationReused,
    StaleSequence,
    RevisionExhausted,
    Unavailable,
    Corrupt,
}
impl From<DomainError> for LibraryError {
    fn from(e: DomainError) -> Self {
        match e {
            DomainError::InvalidId
            | DomainError::InvalidRevision
            | DomainError::InvalidSequence
            | DomainError::InvalidTime
            | DomainError::BindingMismatch => Self::InvalidRequest,
            DomainError::InvalidPosition => Self::InvalidPosition,
            DomainError::RevisionExhausted => Self::RevisionExhausted,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PageKind {
    Continue,
    Saved,
    History,
}
#[derive(Clone, Debug)]
pub(super) enum Command {
    Progress(ProgressCandidate),
    Saved(SavedCandidate),
    Start(HistoryStart),
    Clear(HistoryClear),
}
impl Command {
    pub fn intent(&self) -> &Intent {
        match self {
            Self::Progress(c) => c.intent(),
            Self::Saved(c) => c.intent(),
            Self::Start(c) => c.intent(),
            Self::Clear(c) => c.intent(),
        }
    }
    pub fn sequence_key(&self) -> String {
        match self {
            Self::Progress(c) => format!("progress:{}", c.audio().publication().as_str()),
            Self::Saved(c) => format!("saved:{}", c.work().as_str()),
            Self::Start(_) | Self::Clear(_) => "history".into(),
        }
    }
}
#[derive(Clone, Debug)]
pub(super) struct Mutation {
    pub command: Command,
    pub fingerprint: Vec<u8>,
}
#[derive(Clone, Debug)]
pub(super) enum ReadRequest {
    Progress(String),
    Saved(WorkId),
    Receipt(OperationId),
    Page {
        kind: PageKind,
        limit: usize,
        anchor: Option<PageAnchor>,
    },
}
pub(super) enum ReadResult {
    Progress(Option<ProgressRecord>),
    Saved(SavedRecord),
    Receipt(dto::Receipt),
    Page(PageSnapshot),
}
pub(super) enum PageData {
    Continue(Vec<dto::ContinueItem>),
    Saved(Vec<dto::SavedItem>),
    History {
        state: dto::HistoryState,
        items: Vec<dto::HistoryItem>,
    },
}
pub(super) struct PageSnapshot {
    pub data: PageData,
    pub next: Option<PageAnchor>,
}
pub(super) type ReadFuture =
    Pin<Box<dyn Future<Output = Result<ReadResult, LibraryError>> + Send + 'static>>;
pub(super) type WriteFuture =
    Pin<Box<dyn Future<Output = Result<dto::Receipt, LibraryError>> + Send + 'static>>;
pub(super) trait LibraryRepository: Send + Sync {
    fn read(&self, listener: ListenerId, request: ReadRequest) -> ReadFuture;
    fn mutate(&self, listener: ListenerId, mutation: Mutation) -> WriteFuture;
}
#[derive(Clone)]
pub struct LibraryService {
    pub(super) repository: Arc<dyn LibraryRepository>,
    pub(super) cursors: Arc<CursorVault>,
}
fn now() -> Result<i64, LibraryError> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| LibraryError::Unavailable)?
            .as_millis(),
    )
    .map_err(|_| LibraryError::Unavailable)
}
impl LibraryService {
    pub async fn progress(
        &self,
        principal: &AuthenticatedListener,
        publication: &str,
    ) -> Result<Option<dto::ProgressRecord>, LibraryError> {
        let id = domain::PublicationId::parse(publication)?;
        match self
            .repository
            .read(
                principal.listener().clone(),
                ReadRequest::Progress(id.as_str().into()),
            )
            .await?
        {
            ReadResult::Progress(r) => Ok(r.map(|v| v.raw().clone())),
            _ => Err(LibraryError::Corrupt),
        }
    }
    pub async fn saved(
        &self,
        principal: &AuthenticatedListener,
        work: &str,
    ) -> Result<dto::SavedRecord, LibraryError> {
        let work = WorkId::parse(work).map_err(|_| LibraryError::InvalidRequest)?;
        match self
            .repository
            .read(principal.listener().clone(), ReadRequest::Saved(work))
            .await?
        {
            ReadResult::Saved(r) => Ok(r.raw().clone()),
            _ => Err(LibraryError::Corrupt),
        }
    }
    pub async fn receipt(
        &self,
        principal: &AuthenticatedListener,
        operation: &str,
    ) -> Result<dto::Receipt, LibraryError> {
        let id = OperationId::parse(operation)?;
        match self
            .repository
            .read(
                principal.listener().clone(),
                ReadRequest::Receipt(id.clone()),
            )
            .await?
        {
            ReadResult::Receipt(r) if r.operation_id == id.as_str() => Ok(r),
            _ => Err(LibraryError::Corrupt),
        }
    }
    async fn mutate<T: serde::Serialize>(
        &self,
        principal: &AuthenticatedListener,
        route: &str,
        raw: &T,
        command: Command,
    ) -> Result<dto::Receipt, LibraryError> {
        let canonical = serde_json::to_vec(&("library-v1", route, raw))
            .map_err(|_| LibraryError::InvalidRequest)?;
        let operation = command.intent().operation().as_str().to_owned();
        let fingerprint = Sha256::digest(canonical).to_vec();
        let receipt = self
            .repository
            .mutate(
                principal.listener().clone(),
                Mutation {
                    command,
                    fingerprint,
                },
            )
            .await?;
        if receipt.operation_id != operation {
            return Err(LibraryError::Corrupt);
        }
        Ok(receipt)
    }
    pub async fn put_progress(
        &self,
        principal: &AuthenticatedListener,
        publication: &str,
        raw: dto::ProgressWrite,
    ) -> Result<dto::Receipt, LibraryError> {
        let candidate = ProgressCandidate::new(&raw)?;
        if candidate.audio().publication().as_str() != publication {
            return Err(LibraryError::InvalidRequest);
        }
        self.mutate(
            principal,
            &format!("progress:{publication}"),
            &raw,
            Command::Progress(candidate),
        )
        .await
    }
    pub async fn set_saved(
        &self,
        principal: &AuthenticatedListener,
        work: &str,
        raw: dto::SavedWrite,
    ) -> Result<dto::Receipt, LibraryError> {
        let candidate = SavedCandidate::new(work, &raw)?;
        self.mutate(
            principal,
            &format!("saved:{work}"),
            &raw,
            Command::Saved(candidate),
        )
        .await
    }
    pub async fn playback_started(
        &self,
        principal: &AuthenticatedListener,
        raw: dto::HistoryStart,
    ) -> Result<dto::Receipt, LibraryError> {
        let candidate = HistoryStart::new(&raw)?;
        self.mutate(principal, "history:start", &raw, Command::Start(candidate))
            .await
    }
    pub async fn clear_history(
        &self,
        principal: &AuthenticatedListener,
        raw: dto::HistoryClear,
    ) -> Result<dto::Receipt, LibraryError> {
        let candidate = HistoryClear::new(&raw)?;
        self.mutate(principal, "history:clear", &raw, Command::Clear(candidate))
            .await
    }
    pub(super) async fn page(
        &self,
        principal: &AuthenticatedListener,
        kind: PageKind,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<(PageData, Option<String>), LibraryError> {
        if !(1..=50).contains(&limit) {
            return Err(LibraryError::InvalidRequest);
        }
        let anchor = cursor
            .map(|v| {
                self.cursors
                    .read(v, principal.listener(), kind, limit, now()?)
            })
            .transpose()?;
        let ReadResult::Page(snapshot) = self
            .repository
            .read(
                principal.listener().clone(),
                ReadRequest::Page {
                    kind,
                    limit: usize::from(limit),
                    anchor,
                },
            )
            .await?
        else {
            return Err(LibraryError::Corrupt);
        };
        let (actual_kind, len) = match &snapshot.data {
            PageData::Continue(v) => (PageKind::Continue, v.len()),
            PageData::Saved(v) => (PageKind::Saved, v.len()),
            PageData::History { items, .. } => (PageKind::History, items.len()),
        };
        if actual_kind != kind || len > usize::from(limit) {
            return Err(LibraryError::Corrupt);
        }
        let next = snapshot
            .next
            .map(|a| {
                self.cursors
                    .issue(principal.listener(), kind, limit, a, now()?)
            })
            .transpose()?;
        Ok((snapshot.data, next))
    }
    pub async fn continue_listening(
        &self,
        principal: &AuthenticatedListener,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::ContinuePage, LibraryError> {
        let (PageData::Continue(items), next_cursor) = self
            .page(principal, PageKind::Continue, limit, cursor)
            .await?
        else {
            return Err(LibraryError::Corrupt);
        };
        Ok(dto::ContinuePage { items, next_cursor })
    }
    pub async fn saved_works(
        &self,
        principal: &AuthenticatedListener,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::SavedPage, LibraryError> {
        let (PageData::Saved(items), next_cursor) =
            self.page(principal, PageKind::Saved, limit, cursor).await?
        else {
            return Err(LibraryError::Corrupt);
        };
        Ok(dto::SavedPage { items, next_cursor })
    }
    pub async fn history(
        &self,
        principal: &AuthenticatedListener,
        limit: u16,
        cursor: Option<&str>,
    ) -> Result<dto::HistoryPage, LibraryError> {
        let (PageData::History { state, items }, next_cursor) = self
            .page(principal, PageKind::History, limit, cursor)
            .await?
        else {
            return Err(LibraryError::Corrupt);
        };
        Ok(dto::HistoryPage {
            state,
            items,
            next_cursor,
        })
    }
}
pub(super) fn progress_receipt(operation: &OperationId, outcome: &ProgressOutcome) -> dto::Receipt {
    let (kind, current, resolution) = match outcome {
        ProgressOutcome::Accepted(v) => (dto::WriteOutcome::Accepted, Some(v.raw().clone()), None),
        ProgressOutcome::AlreadyCurrent(v) => (
            dto::WriteOutcome::AlreadyCurrent,
            Some(v.raw().clone()),
            None,
        ),
        ProgressOutcome::Conflict {
            current,
            resolution,
        } => (
            dto::WriteOutcome::RevisionConflict,
            current.as_ref().map(|r| r.raw().clone()),
            Some(*resolution),
        ),
    };
    dto::Receipt {
        operation_id: operation.as_str().into(),
        outcome: kind,
        resolution,
        state: dto::MutationState::Progress { current },
    }
}
pub(super) fn saved_receipt(operation: &OperationId, outcome: &SavedOutcome) -> dto::Receipt {
    let (kind, current, resolution) = match outcome {
        SavedOutcome::Accepted(v) => (dto::WriteOutcome::Accepted, v.raw().clone(), None),
        SavedOutcome::AlreadyCurrent(v) => {
            (dto::WriteOutcome::AlreadyCurrent, v.raw().clone(), None)
        }
        SavedOutcome::Conflict(v) => (
            dto::WriteOutcome::RevisionConflict,
            v.raw().clone(),
            Some(dto::Resolution::ListenerChooses),
        ),
    };
    dto::Receipt {
        operation_id: operation.as_str().into(),
        outcome: kind,
        resolution,
        state: dto::MutationState::Saved { current },
    }
}
pub(super) fn history_receipt(operation: &OperationId, outcome: HistoryOutcome) -> dto::Receipt {
    let (kind, state, resolution) = match outcome {
        HistoryOutcome::Accepted(v) => (dto::WriteOutcome::Accepted, v, None),
        HistoryOutcome::AlreadyCurrent(v) => (dto::WriteOutcome::AlreadyCurrent, v, None),
        HistoryOutcome::RevisionConflict(v) => (
            dto::WriteOutcome::RevisionConflict,
            v,
            Some(dto::Resolution::ListenerChooses),
        ),
        HistoryOutcome::EpochConflict(v) => (
            dto::WriteOutcome::HistoryEpochConflict,
            v,
            Some(dto::Resolution::KeepServer),
        ),
    };
    dto::Receipt {
        operation_id: operation.as_str().into(),
        outcome: kind,
        resolution,
        state: dto::MutationState::History {
            current: state.raw(),
        },
    }
}
#[cfg(test)]
mod tests;
