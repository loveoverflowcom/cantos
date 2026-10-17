//! Internal, session-only Forge mock contracts. These are not producer or HTTP DTOs.
pub const MAX_TEXT_BYTES: usize = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryState {
    Ready,
    Empty,
    Loading,
    Error,
    Locked,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revision {
    pub number: u32,
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub id: u64,
    pub work: String,
    pub title: String,
    pub chapter: String,
    pub revisions: Vec<Revision>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionState {
    Generated,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageState {
    Retained,
    Failed,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Take {
    pub id: u64,
    pub draft: u64,
    pub revision: u32,
    pub execution: ExecutionState,
    pub storage: StorageState,
    pub stale: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunState {
    Idle,
    Running { attempt: u64, revision: u32 },
    Retained { attempt: u64, take: u64 },
    Failed { attempt: u64, revision: u32 },
    Uncertain { attempt: u64, revision: u32 },
    StorageFailed { attempt: u64, take: u64 },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPlan {
    pub draft: u64,
    pub revision: u32,
    pub take: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncState {
    Idle,
    Preview(SyncPlan),
    Sending { operation: u64, plan: SyncPlan },
    Receipt { operation: u64, plan: SyncPlan },
    Partial { operation: u64, plan: SyncPlan },
    Unknown { operation: u64, plan: SyncPlan },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub epoch: u64,
    pub draft: u64,
    pub sequence: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestKind {
    Generate { attempt: u64, revision: u32 },
    ReconcileRun { attempt: u64, revision: u32 },
    Sync { operation: u64, plan: SyncPlan },
    ReconcileSync { operation: u64, plan: SyncPlan },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub ticket: Ticket,
    pub kind: RequestKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerationOutcome {
    Retained,
    Failed,
    Uncertain,
    StorageFailed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncOutcome {
    Receipt,
    Partial,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Generated(GenerationOutcome),
    Synced(SyncOutcome),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub request: Request,
    pub outcome: Outcome,
}
/// The debug shell injects a deterministic adapter and advances it only on explicit input.
pub trait ForgePort {
    fn complete(&mut self, request: &Request) -> Completion;
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    Open(u64),
    NewDraft,
    Edit(String),
    Save,
    Generate,
    SelectTake(u64),
    PreviewSync,
    ConfirmSync,
    CancelSync,
    Reconcile,
    SetConnection(ConnectionState),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    Disposed,
    LibraryUnavailable,
    Busy,
    NoDraft,
    UnknownDraft,
    EmptyText,
    TextTooLong,
    Unchanged,
    ConnectionUnavailable,
    Unsaved,
    Unresolved,
    UnknownTake,
    NotRetained,
    Stale,
    NoTake,
    MissingPlan,
    NoUncertainOperation,
    StaleCompletion,
    WrongCompletion,
    LimitReached,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub library: LibraryState,
    pub drafts: Vec<Draft>,
    pub current: Option<Draft>,
    pub buffer: String,
    pub dirty: bool,
    pub takes: Vec<Take>,
    pub selected_take: Option<u64>,
    pub run: RunState,
    pub sync: SyncState,
    pub connection: ConnectionState,
    pub pending: Option<Request>,
    pub can_generate: bool,
    pub can_sync: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct DraftState {
    draft: Draft,
    buffer: String,
    selected_take: Option<u64>,
    run: RunState,
    sync: SyncState,
}
impl DraftState {
    fn revision(&self) -> &Revision {
        &self.draft.revisions[self.draft.revisions.len() - 1]
    }
    fn dirty(&self) -> bool {
        self.buffer != self.revision().text
    }
    fn unresolved(&self) -> bool {
        matches!(self.run, RunState::Uncertain { .. })
            || matches!(
                self.sync,
                SyncState::Unknown { .. } | SyncState::Partial { .. }
            )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    library: LibraryState,
    drafts: Vec<DraftState>,
    selected: Option<u64>,
    takes: Vec<Take>,
    connection: ConnectionState,
    pending: Option<Request>,
    epoch: u64,
    sequence: u64,
    disposed: bool,
}
impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}
impl Model {
    pub fn new() -> Self {
        Self {
            library: LibraryState::Ready,
            drafts: fixtures(),
            selected: Some(1),
            takes: vec![],
            connection: ConnectionState::Connected,
            pending: None,
            epoch: 0,
            sequence: 0,
            disposed: false,
        }
    }
    fn current(&self) -> Option<&DraftState> {
        self.drafts
            .iter()
            .find(|d| Some(d.draft.id) == self.selected)
    }
    fn current_mut(&mut self) -> Option<&mut DraftState> {
        self.drafts
            .iter_mut()
            .find(|d| Some(d.draft.id) == self.selected)
    }
    fn plan(&self) -> Result<SyncPlan, Rejection> {
        let d = self.current().ok_or(Rejection::NoDraft)?;
        if d.dirty() {
            return Err(Rejection::Unsaved);
        }
        if d.unresolved() {
            return Err(Rejection::Unresolved);
        }
        let take = self
            .takes
            .iter()
            .find(|t| Some(t.id) == d.selected_take)
            .ok_or(Rejection::NoTake)?;
        if take.storage != StorageState::Retained {
            return Err(Rejection::NotRetained);
        }
        if take.draft != d.draft.id || take.revision != d.revision().number {
            return Err(Rejection::Stale);
        }
        Ok(SyncPlan {
            draft: d.draft.id,
            revision: d.revision().number,
            take: take.id,
        })
    }
    pub fn snapshot(&self) -> Snapshot {
        let d = self.current();
        let ready = !self.disposed && self.library == LibraryState::Ready && self.pending.is_none();
        Snapshot {
            library: self.library,
            drafts: self.drafts.iter().map(|d| d.draft.clone()).collect(),
            current: d.map(|d| d.draft.clone()),
            buffer: d.map(|d| d.buffer.clone()).unwrap_or_default(),
            dirty: d.is_some_and(DraftState::dirty),
            takes: self
                .takes
                .iter()
                .filter(|t| Some(t.draft) == self.selected)
                .map(|t| {
                    let mut t = t.clone();
                    t.stale = d.is_none_or(|d| d.dirty() || d.revision().number != t.revision);
                    t
                })
                .collect(),
            selected_take: d.and_then(|d| d.selected_take),
            run: d.map(|d| d.run.clone()).unwrap_or(RunState::Idle),
            sync: d.map(|d| d.sync.clone()).unwrap_or(SyncState::Idle),
            connection: self.connection,
            pending: self.pending.clone(),
            can_generate: ready
                && self.connection == ConnectionState::Connected
                && self.takes.len() < 64
                && d.is_some_and(|d| !d.dirty() && !d.unresolved() && !d.buffer.trim().is_empty()),
            can_sync: ready && self.connection == ConnectionState::Connected && self.plan().is_ok(),
        }
    }
    /// All rejected intents leave the complete state unchanged.
    pub fn reduce(&mut self, intent: Intent) -> Result<Option<Request>, Rejection> {
        let mut next = self.clone();
        let result = next.apply(intent)?;
        *self = next;
        Ok(result)
    }
    fn apply(&mut self, intent: Intent) -> Result<Option<Request>, Rejection> {
        if self.disposed {
            return Err(Rejection::Disposed);
        }
        if self.library != LibraryState::Ready
            && !(self.library == LibraryState::Empty && intent == Intent::NewDraft)
        {
            return Err(Rejection::LibraryUnavailable);
        }
        if let Intent::Open(id) = intent {
            if !self.drafts.iter().any(|d| d.draft.id == id) {
                return Err(Rejection::UnknownDraft);
            }
            if self.selected != Some(id) {
                self.retire_pending();
                self.epoch = self.epoch.checked_add(1).ok_or(Rejection::LimitReached)?;
                self.selected = Some(id);
            }
            return Ok(None);
        }
        if self.pending.is_some() {
            return Err(Rejection::Busy);
        }
        if let Intent::SetConnection(connection) = intent {
            self.connection = connection;
            return Ok(None);
        }
        if intent == Intent::NewDraft {
            let id = self
                .drafts
                .iter()
                .map(|d| d.draft.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(Rejection::LimitReached)?;
            if self.drafts.len() >= 32 {
                return Err(Rejection::LimitReached);
            }
            self.drafts.push(draft(
                id,
                "Những lời hẹn dưới ánh đèn",
                "Bản thảo mock mới",
                "Chương mới",
                "",
            ));
            self.selected = Some(id);
            self.library = LibraryState::Ready;
            self.epoch = self.epoch.checked_add(1).ok_or(Rejection::LimitReached)?;
            return Ok(None);
        }
        let d = self.current().ok_or(Rejection::NoDraft)?;
        let revision = d.revision().number;
        match intent {
            Intent::Edit(text) => {
                if text.len() > MAX_TEXT_BYTES {
                    return Err(Rejection::TextTooLong);
                }
                self.current_mut().ok_or(Rejection::NoDraft)?.buffer = text;
            }
            Intent::Save => {
                if !d.dirty() {
                    return Err(Rejection::Unchanged);
                }
                if d.buffer.trim().is_empty() {
                    return Err(Rejection::EmptyText);
                }
                if d.draft.revisions.len() >= 64 {
                    return Err(Rejection::LimitReached);
                }
                let number = revision.checked_add(1).ok_or(Rejection::LimitReached)?;
                let d = self.current_mut().ok_or(Rejection::NoDraft)?;
                d.draft.revisions.push(Revision {
                    number,
                    text: d.buffer.clone(),
                });
                if matches!(d.sync, SyncState::Preview(_)) {
                    d.sync = SyncState::Idle;
                }
            }
            Intent::SelectTake(id) => {
                let take = self
                    .takes
                    .iter()
                    .find(|t| t.id == id && Some(t.draft) == self.selected)
                    .ok_or(Rejection::UnknownTake)?;
                if take.storage != StorageState::Retained {
                    return Err(Rejection::NotRetained);
                }
                if take.revision != revision {
                    return Err(Rejection::Stale);
                }
                let d = self.current_mut().ok_or(Rejection::NoDraft)?;
                d.selected_take = Some(id);
                if matches!(d.sync, SyncState::Preview(_)) {
                    d.sync = SyncState::Idle;
                }
            }
            Intent::PreviewSync => {
                self.connected()?;
                let plan = self.plan()?;
                self.current_mut().ok_or(Rejection::NoDraft)?.sync = SyncState::Preview(plan);
            }
            Intent::CancelSync => {
                if !matches!(d.sync, SyncState::Preview(_)) {
                    return Err(Rejection::MissingPlan);
                }
                self.current_mut().ok_or(Rejection::NoDraft)?.sync = SyncState::Idle;
            }
            Intent::Generate => {
                self.connected()?;
                if d.dirty() {
                    return Err(Rejection::Unsaved);
                }
                if d.unresolved() {
                    return Err(Rejection::Unresolved);
                }
                if d.buffer.trim().is_empty() {
                    return Err(Rejection::EmptyText);
                }
                if self.takes.len() >= 64 {
                    return Err(Rejection::LimitReached);
                }
                let attempt = self
                    .sequence
                    .checked_add(1)
                    .ok_or(Rejection::LimitReached)?;
                return self.request(RequestKind::Generate { attempt, revision });
            }
            Intent::ConfirmSync => {
                self.connected()?;
                let SyncState::Preview(plan) = &d.sync else {
                    return Err(Rejection::MissingPlan);
                };
                if *plan != self.plan()? {
                    return Err(Rejection::Stale);
                }
                let operation = self
                    .sequence
                    .checked_add(1)
                    .ok_or(Rejection::LimitReached)?;
                return self.request(RequestKind::Sync {
                    operation,
                    plan: plan.clone(),
                });
            }
            Intent::Reconcile => {
                self.connected()?;
                let kind = match (&d.run, &d.sync) {
                    (RunState::Uncertain { attempt, revision }, _) => RequestKind::ReconcileRun {
                        attempt: *attempt,
                        revision: *revision,
                    },
                    (
                        _,
                        SyncState::Unknown { operation, plan }
                        | SyncState::Partial { operation, plan },
                    ) => RequestKind::ReconcileSync {
                        operation: *operation,
                        plan: plan.clone(),
                    },
                    _ => return Err(Rejection::NoUncertainOperation),
                };
                return self.request(kind);
            }
            // Handled before the current-draft branch; keep this match total without panicking.
            Intent::Open(_) | Intent::NewDraft | Intent::SetConnection(_) => {
                return Err(Rejection::NoDraft)
            }
        }
        Ok(None)
    }
    fn connected(&self) -> Result<(), Rejection> {
        if self.connection == ConnectionState::Connected {
            Ok(())
        } else {
            Err(Rejection::ConnectionUnavailable)
        }
    }
    fn request(&mut self, kind: RequestKind) -> Result<Option<Request>, Rejection> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(Rejection::LimitReached)?;
        let request = Request {
            ticket: Ticket {
                epoch: self.epoch,
                draft: self.selected.ok_or(Rejection::NoDraft)?,
                sequence: self.sequence,
            },
            kind,
        };
        let d = self.current_mut().ok_or(Rejection::NoDraft)?;
        match &request.kind {
            RequestKind::Generate { attempt, revision }
            | RequestKind::ReconcileRun { attempt, revision } => {
                d.run = RunState::Running {
                    attempt: *attempt,
                    revision: *revision,
                }
            }
            RequestKind::Sync { operation, plan }
            | RequestKind::ReconcileSync { operation, plan } => {
                d.sync = SyncState::Sending {
                    operation: *operation,
                    plan: plan.clone(),
                }
            }
        }
        self.pending = Some(request.clone());
        Ok(Some(request))
    }
    pub fn complete(&mut self, completion: Completion) -> Result<(), Rejection> {
        if self.disposed {
            return Err(Rejection::Disposed);
        }
        if self.pending.as_ref() != Some(&completion.request) {
            return Err(Rejection::StaleCompletion);
        }
        let mut next = self.clone();
        next.settle(completion)?;
        *self = next;
        Ok(())
    }
    fn settle(&mut self, completion: Completion) -> Result<(), Rejection> {
        match (completion.request.kind, completion.outcome) {
            (
                RequestKind::Generate { attempt, revision }
                | RequestKind::ReconcileRun { attempt, revision },
                Outcome::Generated(outcome),
            ) => {
                let run = match outcome {
                    GenerationOutcome::Failed => RunState::Failed { attempt, revision },
                    GenerationOutcome::Uncertain => RunState::Uncertain { attempt, revision },
                    GenerationOutcome::Retained | GenerationOutcome::StorageFailed => {
                        let id = attempt;
                        self.takes.push(Take {
                            id,
                            draft: completion.request.ticket.draft,
                            revision,
                            execution: ExecutionState::Generated,
                            storage: if outcome == GenerationOutcome::Retained {
                                StorageState::Retained
                            } else {
                                StorageState::Failed
                            },
                            stale: false,
                        });
                        if outcome == GenerationOutcome::Retained {
                            RunState::Retained { attempt, take: id }
                        } else {
                            RunState::StorageFailed { attempt, take: id }
                        }
                    }
                };
                self.current_mut().ok_or(Rejection::NoDraft)?.run = run;
            }
            (
                RequestKind::Sync { operation, plan }
                | RequestKind::ReconcileSync { operation, plan },
                Outcome::Synced(outcome),
            ) => {
                self.current_mut().ok_or(Rejection::NoDraft)?.sync = match outcome {
                    SyncOutcome::Receipt => SyncState::Receipt { operation, plan },
                    SyncOutcome::Partial => SyncState::Partial { operation, plan },
                    SyncOutcome::Unknown => SyncState::Unknown { operation, plan },
                };
            }
            _ => return Err(Rejection::WrongCompletion),
        }
        self.pending = None;
        Ok(())
    }
    fn retire_pending(&mut self) {
        if let Some(request) = self.pending.take() {
            if let Some(d) = self.current_mut() {
                match request.kind {
                    RequestKind::Generate { attempt, revision }
                    | RequestKind::ReconcileRun { attempt, revision } => {
                        d.run = RunState::Uncertain { attempt, revision }
                    }
                    RequestKind::Sync { operation, plan }
                    | RequestKind::ReconcileSync { operation, plan } => {
                        d.sync = SyncState::Unknown { operation, plan }
                    }
                }
            }
        }
    }
    pub fn reset(&mut self, library: LibraryState) {
        let epoch = self.epoch.saturating_add(1);
        let sequence = self.sequence;
        *self = Self::new();
        self.epoch = epoch;
        self.sequence = sequence;
        self.library = library;
        if library != LibraryState::Ready {
            self.drafts.clear();
            self.selected = None;
        }
    }
    pub fn dispose(&mut self) {
        self.retire_pending();
        self.disposed = true;
    }
}
fn draft(id: u64, work: &str, title: &str, chapter: &str, text: &str) -> DraftState {
    DraftState {
        draft: Draft {
            id,
            work: work.into(),
            title: title.into(),
            chapter: chapter.into(),
            revisions: vec![Revision {
                number: 1,
                text: text.into(),
            }],
        },
        buffer: text.into(),
        selected_take: None,
        run: RunState::Idle,
        sync: SyncState::Idle,
    }
}
fn fixtures() -> Vec<DraftState> {
    vec![
        draft(1, "Những người giữ lời hẹn trong một đêm mưa", "Ánh đèn cuối sân khấu — Những người giữ lời hẹn trong một đêm mưa", "Chương 1 · Người dẫn chuyện", "Người dẫn chuyện: Cơn mưa đã ngừng, nhưng ánh đèn cuối sân khấu vẫn sáng.\nMai khẽ hỏi: Ngày mai, mình có diễn tiếp không?"),
        draft(2, "Những lời nhắn chưa gửi", "Chuyến tàu qua miền ký ức và lời nhắn chưa gửi", "Chương 2 · Một lời hẹn", "Tiếng tàu vọng qua triền núi. Người dẫn chuyện dừng lại, dành một khoảng lặng cho lá thư chưa kịp gửi."),
    ]
}
