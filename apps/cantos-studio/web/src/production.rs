//! Pure Studio casting forms and request reconciliation. Authorization belongs to the backend.
use crate::contract::*;
use crate::{authoring::Document, editor::Editor};

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::ProductionInputs;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub actor: String,
    pub script: String,
    pub revision: u64,
    pub export_digest: String,
}

impl Context {
    pub fn from_editor(editor: &Editor) -> Self {
        let saved = editor.remote().filter(|r| r.script_id == editor.script());
        Self {
            actor: editor.actor().to_owned(),
            script: editor.script().to_owned(),
            revision: saved.map_or(0, |r| r.revision),
            export_digest: saved.map(|r| r.export_digest.clone()).unwrap_or_default(),
        }
    }

    pub fn ready(&self) -> bool {
        !self.actor.is_empty() && !self.script.is_empty() && self.revision > 0
    }
}

/// A casting control that the selected provider/model profile may not support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastingControl {
    Model,
    Voice,
    Language,
    Rate,
    Pitch,
    Emotion,
    Intensity,
    Pronunciation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingForm {
    pub character_id: String,
    pub provider_id: String,
    pub model_id: String,
    pub voice_id: String,
    pub language: String,
    pub voice_rights_record_id: String,
    pub rate: String,
    pub pitch: String,
    pub emotion: String,
    pub intensity: String,
    pub pronunciation: Vec<ProductionPronunciation>,
}

impl BindingForm {
    fn from_binding(binding: &CharacterCasting) -> Self {
        Self {
            character_id: binding.character_id.clone(),
            provider_id: binding.provider_id.clone(),
            model_id: binding.model_id.clone(),
            voice_id: binding.voice_id.clone(),
            language: binding.language.clone(),
            voice_rights_record_id: binding.voice_rights_record_id.clone(),
            rate: binding.performance.rate_permille.to_string(),
            pitch: binding.performance.pitch_semitones.to_string(),
            emotion: binding.performance.emotion.clone().unwrap_or_default(),
            intensity: binding
                .performance
                .intensity_permille
                .map(|v| v.to_string())
                .unwrap_or_default(),
            pronunciation: binding.pronunciation.clone(),
        }
    }

    /// The exact provider/model profile, never a fallback to another catalog entry.
    pub fn capability<'a>(
        &self,
        catalog: &'a ProductionCatalogResponse,
    ) -> Option<&'a ProductionCapability> {
        catalog
            .capabilities
            .iter()
            .find(|c| c.provider_id == self.provider_id && c.model_id == self.model_id)
    }

    /// A provider switch changes only the provider and model. Voice, language, performance and
    /// pronunciation keep their text so an unsupported value is shown, never replaced. The model
    /// is kept when the provider offers it, adopted when the provider has exactly one, and left
    /// for an explicit choice otherwise.
    pub fn select_provider(&mut self, catalog: &ProductionCatalogResponse, provider: String) {
        let models: Vec<_> = catalog
            .capabilities
            .iter()
            .filter(|c| c.provider_id == provider)
            .map(|c| c.model_id.clone())
            .collect();
        if !models.contains(&self.model_id) {
            self.model_id = match models.as_slice() {
                [only] => only.clone(),
                _ => String::new(),
            };
        }
        self.provider_id = provider;
    }

    /// Controls whose current value the selected profile cannot honor, for display only: backend
    /// admission and preview still decide. Text that does not parse is reported by settings
    /// parsing, not here. An unknown profile reports only the model, because nothing else can be
    /// judged against it.
    pub fn unsupported(&self, capability: Option<&ProductionCapability>) -> Vec<CastingControl> {
        let Some(c) = capability else {
            return vec![CastingControl::Model];
        };
        let mut controls = Vec::new();
        if !self.voice_id.is_empty() && !c.voice_ids.contains(&self.voice_id) {
            controls.push(CastingControl::Voice);
        }
        if !c.languages.contains(&self.language) {
            controls.push(CastingControl::Language);
        }
        if self
            .rate
            .parse::<u16>()
            .is_ok_and(|v| !(c.rate_min_permille..=c.rate_max_permille).contains(&v))
        {
            controls.push(CastingControl::Rate);
        }
        if self
            .pitch
            .parse::<i16>()
            .is_ok_and(|v| !(c.pitch_min_semitones..=c.pitch_max_semitones).contains(&v))
        {
            controls.push(CastingControl::Pitch);
        }
        if !self.emotion.is_empty() && !c.emotions.contains(&self.emotion) {
            controls.push(CastingControl::Emotion);
        }
        if self
            .intensity
            .parse::<u16>()
            .is_ok_and(|v| v > 1_000 || (!c.intensity_supported && v != 0))
        {
            controls.push(CastingControl::Intensity);
        }
        if !c.pronunciation_supported && !self.pronunciation.is_empty() {
            controls.push(CastingControl::Pronunciation);
        }
        controls
    }

    fn binding(&self) -> Option<CharacterCasting> {
        Some(CharacterCasting {
            character_id: self.character_id.clone(),
            provider_id: self.provider_id.clone(),
            model_id: self.model_id.clone(),
            voice_id: self.voice_id.clone(),
            language: self.language.clone(),
            voice_rights_record_id: self.voice_rights_record_id.clone(),
            performance: SynthesisPerformance {
                rate_permille: self.rate.parse().ok()?,
                pitch_semitones: self.pitch.parse().ok()?,
                emotion: (!self.emotion.is_empty()).then(|| self.emotion.clone()),
                intensity_permille: if self.intensity.is_empty() {
                    None
                } else {
                    Some(self.intensity.parse().ok()?)
                },
            },
            pronunciation: self.pronunciation.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsForm {
    pub bindings: Vec<BindingForm>,
    pub currency: ProductionCurrency,
    pub limit: String,
    pub scope: String,
    pub territory: String,
    pub known_rate: bool,
    pub rate_reference: String,
    pub rate_version: String,
    pub rate_units: String,
    pub rate_amount: String,
}

impl SettingsForm {
    pub fn new(script: &str, catalog: &ProductionCatalogResponse) -> Self {
        let document = Document::parse(script).ok();
        let capability = catalog.capabilities.first();
        Self {
            bindings: document
                .as_ref()
                .map(Document::characters)
                .unwrap_or_default()
                .into_iter()
                .map(|character| BindingForm {
                    character_id: character.id,
                    provider_id: capability
                        .map(|c| c.provider_id.clone())
                        .unwrap_or_default(),
                    model_id: capability.map(|c| c.model_id.clone()).unwrap_or_default(),
                    voice_id: String::new(),
                    language: document
                        .as_ref()
                        .map(|d| d.metadata().language)
                        .unwrap_or_default(),
                    voice_rights_record_id: String::new(),
                    rate: "1000".into(),
                    pitch: "0".into(),
                    emotion: String::new(),
                    intensity: String::new(),
                    pronunciation: vec![],
                })
                .collect(),
            currency: ProductionCurrency::USD,
            limit: String::new(),
            scope: String::new(),
            territory: "private-planning".into(),
            known_rate: false,
            rate_reference: String::new(),
            rate_version: String::new(),
            rate_units: String::new(),
            rate_amount: String::new(),
        }
    }

    pub fn from_settings(settings: &ProductionSettings) -> Self {
        Self {
            bindings: settings
                .bindings
                .iter()
                .map(BindingForm::from_binding)
                .collect(),
            currency: settings.budget.currency,
            limit: settings.budget.limit_minor.to_string(),
            scope: settings.budget.scope.clone(),
            known_rate: settings.budget.rate.is_some(),
            territory: settings.budget.territory.clone(),
            rate_reference: settings
                .budget
                .rate
                .as_ref()
                .map(|r| r.reference.clone())
                .unwrap_or_default(),
            rate_version: settings
                .budget
                .rate
                .as_ref()
                .map(|r| r.version.clone())
                .unwrap_or_default(),
            rate_units: settings
                .budget
                .rate
                .as_ref()
                .map(|r| r.units_per_charge.to_string())
                .unwrap_or_default(),
            rate_amount: settings
                .budget
                .rate
                .as_ref()
                .map(|r| r.amount_minor.to_string())
                .unwrap_or_default(),
        }
    }

    /// Parse transport integers without rounding or silently replacing invalid user text.
    pub fn settings(&self) -> Option<ProductionSettings> {
        Some(ProductionSettings {
            bindings: self
                .bindings
                .iter()
                .map(BindingForm::binding)
                .collect::<Option<_>>()?,
            budget: ProductionBudget {
                currency: self.currency,
                limit_minor: self.limit.parse().ok()?,
                scope: self.scope.clone(),
                territory: self.territory.clone(),
                rate: if self.known_rate {
                    Some(ProductionRate {
                        reference: self.rate_reference.clone(),
                        version: self.rate_version.clone(),
                        units_per_charge: self.rate_units.parse().ok()?,
                        amount_minor: self.rate_amount.parse().ok()?,
                    })
                } else {
                    None
                },
            },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadTicket {
    pub context: Context,
    pub sequence: u64,
    pub kind: ReadKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadKind {
    State,
    Preview,
    Snapshot(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Settings(SaveProductionSettingsRequest),
    Rights(SaveProductionRightsRequest),
    Freeze(FreezeProductionRequest),
    Approve {
        snapshot_id: String,
        request: ApproveProductionRequest,
    },
}

impl Command {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::Settings(r) => &r.operation_id,
            Self::Rights(r) => &r.operation_id,
            Self::Freeze(r) => &r.operation_id,
            Self::Approve { request, .. } => &request.operation_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intent {
    pub context: Context,
    pub command: Command,
}

/// A submitted command whose outcome Production has not observed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingCommand {
    intent: Intent,
    /// An attempt timed out, so a later retry rejection cannot settle this earlier attempt.
    outcome_unknown: bool,
}

/// The one request Production owns; a read and a command can never overlap.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Request {
    #[default]
    Idle,
    Reading(ReadTicket),
    /// The command is in flight.
    Sending(PendingCommand),
    /// The command is not in flight; only an exact retry or matching receipt may settle it.
    Unresolved(PendingCommand),
}

impl Request {
    fn pending(&self) -> Option<&PendingCommand> {
        match self {
            Self::Sending(pending) | Self::Unresolved(pending) => Some(pending),
            Self::Idle | Self::Reading(_) => None,
        }
    }

    /// Nothing stays in flight across a context change: a read is abandoned, a command survives.
    fn settled(&self) -> Self {
        match self.pending() {
            Some(pending) => Self::Unresolved(pending.clone()),
            None => Self::Idle,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Receipt {
    Settings(ProductionSettingsResponse),
    Rights(ProductionRightsClaimResponse),
    Freeze(Box<ProductionSnapshotResponse>),
    Approve(ProductionApprovalResponse),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedForm {
    context: Context,
    catalog: Option<ProductionCatalogResponse>,
    stored: Option<ProductionStateResponse>,
    form: SettingsForm,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Production {
    pub context: Context,
    sequence: u64,
    pub catalog: Option<ProductionCatalogResponse>,
    pub stored: Option<ProductionStateResponse>,
    pub form: Option<SettingsForm>,
    pub preview: Option<ProductionPreviewResponse>,
    pub snapshot: Option<ProductionSnapshotResponse>,
    pub confirmation: Option<(String, String)>,
    request: Request,
    pub error: Option<ApiError>,
    pub rights_draft_dirty: bool,
    pub input_composing: bool,
    /// A session switch hides another actor's unsaved inputs without discarding their bytes.
    pub retained: Option<RetainedForm>,
}

impl Production {
    pub fn observe(&self, editor: &Editor) -> Self {
        let context = Context::from_editor(editor);
        if context == self.context {
            return self.clone();
        }
        let mut next = Self {
            context: context.clone(),
            sequence: self.sequence.saturating_add(1),
            request: self.request.settled(),
            ..Self::default()
        };
        if context.script == self.context.script {
            if context.actor == self.context.actor {
                next.form = self.form.clone();
                next.catalog = self.catalog.clone();
                next.stored = self.stored.clone();
                next.retained = self.retained.clone();
            } else if self.retained.as_ref().is_some_and(|r| {
                r.context.actor == context.actor && r.context.script == context.script
            }) {
                if let Some(retained) = &self.retained {
                    next.form = Some(retained.form.clone());
                    next.catalog = retained.catalog.clone();
                    next.stored = retained.stored.clone();
                }
            } else {
                next.retained = self
                    .form
                    .as_ref()
                    .filter(|_| self.visible_dirty())
                    .map(|form| RetainedForm {
                        context: self.context.clone(),
                        catalog: self.catalog.clone(),
                        stored: self.stored.clone(),
                        form: form.clone(),
                    })
                    .or_else(|| self.retained.clone());
            }
        }
        next
    }

    /// A read or command is in flight.
    pub fn busy(&self) -> bool {
        match self.request {
            Request::Reading(_) | Request::Sending(_) => true,
            Request::Idle | Request::Unresolved(_) => false,
        }
    }
    pub fn has_activity(&self) -> bool {
        self.request != Request::Idle
    }
    /// The unresolved command, whether in flight or awaiting reconciliation.
    pub fn pending(&self) -> Option<&Intent> {
        self.request.pending().map(|pending| &pending.intent)
    }
    fn visible_dirty(&self) -> bool {
        self.form.as_ref().is_some_and(|form| {
            self.stored
                .as_ref()
                .and_then(|s| s.settings.as_ref())
                .is_none_or(|s| {
                    s.script_revision != self.context.revision
                        || SettingsForm::from_settings(&s.settings) != *form
                })
        })
    }
    pub fn is_dirty(&self) -> bool {
        self.visible_dirty() || self.retained.is_some() || self.rights_draft_dirty
    }

    pub fn rights_edited(&self) -> Self {
        let mut next = self.clone();
        next.rights_draft_dirty = true;
        next.preview = None;
        next.confirmation = None;
        next
    }

    pub fn rights_reconciled(&self) -> Self {
        let mut next = self.clone();
        next.rights_draft_dirty = false;
        next
    }

    pub fn discard_drafts(&self) -> Option<Self> {
        if self.has_activity() {
            return None;
        }
        let mut next = self.clone();
        next.form = self
            .stored
            .as_ref()
            .and_then(|s| s.settings.as_ref())
            .map(|s| SettingsForm::from_settings(&s.settings));
        next.retained = None;
        next.rights_draft_dirty = false;
        next.preview = None;
        next.confirmation = None;
        next.error = None;
        Some(next)
    }

    pub fn start_read(&self, kind: ReadKind) -> Option<(Self, ReadTicket)> {
        if self.has_activity() || self.input_composing || !self.context.ready() {
            return None;
        }
        if kind == ReadKind::Preview
            && (self.is_dirty()
                || self
                    .stored
                    .as_ref()
                    .and_then(|s| s.settings.as_ref())
                    .is_none())
        {
            return None;
        }
        let ticket = ReadTicket {
            context: self.context.clone(),
            sequence: self.sequence.checked_add(1)?,
            kind,
        };
        let mut next = self.clone();
        next.sequence = ticket.sequence;
        next.request = Request::Reading(ticket.clone());
        next.error = None;
        Some((next, ticket))
    }

    fn accepts_read(&self, ticket: &ReadTicket) -> bool {
        matches!(&self.request, Request::Reading(reading) if reading == ticket)
            && self.context == ticket.context
    }

    pub fn loaded(
        &self,
        ticket: &ReadTicket,
        catalog: ProductionCatalogResponse,
        stored: ProductionStateResponse,
        script: &str,
    ) -> Self {
        if !self.accepts_read(ticket) || ticket.kind != ReadKind::State {
            return self.clone();
        }
        if stored
            .settings
            .as_ref()
            .is_some_and(|s| s.script_id != self.context.script)
            || stored
                .snapshots
                .iter()
                .any(|s| s.script_id != self.context.script)
        {
            return self.read_failed(ticket, error(ErrorCode::CorruptRevision));
        }
        let mut next = self.clone();
        next.request = Request::Idle;
        if next.form.is_none() || !next.is_dirty() {
            next.form = Some(
                stored
                    .settings
                    .as_ref()
                    .map(|s| SettingsForm::from_settings(&s.settings))
                    .unwrap_or_else(|| SettingsForm::new(script, &catalog)),
            );
        }
        next.catalog = Some(catalog);
        next.snapshot = None;
        next.stored = Some(stored);
        next.preview = None;
        next.confirmation = None;
        next
    }

    pub fn read_failed(&self, ticket: &ReadTicket, error: ApiError) -> Self {
        if !self.accepts_read(ticket) {
            return self.clone();
        }
        let mut next = self.clone();
        next.request = Request::Idle;
        next.error = Some(error);
        next
    }

    pub fn edit(&self, change: impl FnOnce(&mut SettingsForm)) -> Self {
        let mut next = self.clone();
        if let Some(form) = &mut next.form {
            change(form);
        }
        next.preview = None;
        next.confirmation = None;
        next.error = None;
        next
    }

    pub fn preview_loaded(&self, ticket: &ReadTicket, preview: ProductionPreviewResponse) -> Self {
        if !self.accepts_read(ticket) || ticket.kind != ReadKind::Preview {
            return self.clone();
        }
        let mut next = self.clone();
        next.request = Request::Idle;
        if preview.script_revision == self.context.revision
            && self
                .stored
                .as_ref()
                .and_then(|s| s.settings.as_ref())
                .is_some_and(|s| s.version == preview.settings_version)
            && !self.is_dirty()
        {
            next.preview = Some(preview);
            next.confirmation = None;
        } else {
            next.error = Some(error(ErrorCode::StaleProductionInputs));
        }
        next
    }

    pub fn snapshot_loaded(
        &self,
        ticket: &ReadTicket,
        snapshot: ProductionSnapshotResponse,
    ) -> Self {
        if !self.accepts_read(ticket) {
            return self.clone();
        }
        if snapshot.script_id != self.context.script
            || ticket.kind != ReadKind::Snapshot(snapshot.id.clone())
        {
            return self.read_failed(ticket, error(ErrorCode::CorruptRevision));
        }
        let mut next = self.clone();
        next.request = Request::Idle;
        next.snapshot = Some(snapshot);
        next.confirmation = None;
        next
    }

    pub fn confirm(&self, checked: bool) -> Self {
        let mut next = self.clone();
        next.confirmation = if checked && !self.has_activity() && !self.is_dirty() {
            self.snapshot
                .as_ref()
                .map(|s| (s.id.clone(), s.document.input_digest.clone()))
        } else {
            None
        };
        next
    }

    pub fn start(&self, command: Command) -> Option<(Self, Intent)> {
        if self.has_activity() || self.input_composing || !self.context.ready() {
            return None;
        }
        match &command {
            Command::Settings(r) if r.expected_revision != self.context.revision => return None,
            Command::Freeze(r)
                if self.is_dirty()
                    || self.preview.as_ref().is_none_or(|p| {
                        p.input_digest != r.input_digest
                            || p.settings_version != r.settings_version
                            || p.script_revision != r.expected_revision
                    }) =>
            {
                return None
            }
            Command::Approve {
                snapshot_id,
                request,
            } if self.is_dirty()
                || self.confirmation.as_ref()
                    != Some(&(snapshot_id.clone(), request.input_digest.clone())) =>
            {
                return None
            }
            _ => {}
        }
        let intent = Intent {
            context: self.context.clone(),
            command,
        };
        let mut next = self.clone();
        next.request = Request::Sending(PendingCommand {
            intent: intent.clone(),
            outcome_unknown: false,
        });
        next.error = None;
        next.confirmation = None;
        Some((next, intent))
    }

    pub fn can_retry(&self) -> bool {
        matches!(&self.request, Request::Unresolved(pending)
            if pending.intent.context.actor == self.context.actor)
    }
    pub fn retry(&self) -> Option<(Self, Intent)> {
        let Request::Unresolved(pending) = &self.request else {
            return None;
        };
        if !self.can_retry() {
            return None;
        }
        let mut next = self.clone();
        next.request = Request::Sending(pending.clone());
        next.error = None;
        Some((next, pending.intent.clone()))
    }

    pub fn failed(&self, intent: &Intent, failure: ApiError) -> Self {
        let Some(pending) = self.request.pending().filter(|p| p.intent == *intent) else {
            return self.clone();
        };
        let unresolved = |outcome_unknown| {
            Request::Unresolved(PendingCommand {
                intent: intent.clone(),
                outcome_unknown,
            })
        };
        let mut next = self.clone();
        next.request = match failure.code {
            ErrorCode::Unavailable => unresolved(true),
            // Reauthenticate as the submitting actor, then reconcile the same operation.
            ErrorCode::Unauthenticated => unresolved(pending.outcome_unknown),
            ErrorCode::NotFound
            | ErrorCode::Forbidden
            | ErrorCode::InvalidRequest
            | ErrorCode::InvalidScript
            | ErrorCode::EvidenceUnavailable
            | ErrorCode::StaleRevision
            | ErrorCode::OperationReused
            | ErrorCode::ProposalAlreadySubmitted
            | ErrorCode::CorruptRevision
            | ErrorCode::StaleProductionInputs
            | ErrorCode::ProductionBlocked
            | ErrorCode::InvalidCatalog
            | ErrorCode::CatalogConflict
            | ErrorCode::StaleCursor
            | ErrorCode::AudioRejected => {
                if pending.outcome_unknown {
                    unresolved(true)
                } else {
                    Request::Idle
                }
            }
        };
        if self.context == intent.context {
            next.error = Some(failure);
        }
        next
    }

    pub fn recorded(&self, intent: &Intent, receipt: Receipt) -> Self {
        if self.pending() != Some(intent) {
            return self.clone();
        }
        let valid = match (&intent.command, &receipt) {
            (Command::Settings(r), Receipt::Settings(s)) => {
                s.script_id == intent.context.script
                    && s.recorded_by == intent.context.actor
                    && s.operation_id == r.operation_id
                    && s.script_revision == r.expected_revision
                    && Some(s.version) == r.expected_settings_version.checked_add(1)
                    && s.settings == r.settings
            }
            (Command::Rights(r), Receipt::Rights(s)) => {
                s.recorded_by == intent.context.actor
                    && s.operation_id == r.operation_id
                    && Some(s.claim.version) == r.expected_version.checked_add(1)
                    && s.claim.declaration == r.claim
            }
            (Command::Freeze(r), Receipt::Freeze(s)) => {
                s.created_by == intent.context.actor
                    && s.script_id == intent.context.script
                    && s.operation_id == r.operation_id
                    && s.document.input_digest == r.input_digest
                    && s.document.revision.revision == r.expected_revision
                    && s.document.settings.version == r.settings_version
            }
            (
                Command::Approve {
                    snapshot_id,
                    request,
                },
                Receipt::Approve(s),
            ) => {
                s.approved_by == intent.context.actor
                    && s.snapshot_id == *snapshot_id
                    && s.operation_id == request.operation_id
                    && s.input_digest == request.input_digest
            }
            _ => false,
        };
        if !valid {
            return self.failed(intent, error(ErrorCode::Unavailable));
        }
        let mut next = self.clone();
        next.request = Request::Idle;
        if self.context != intent.context {
            return next;
        }
        next.preview = None;
        next.confirmation = None;
        next.error = None;
        match receipt {
            Receipt::Settings(settings) => {
                next.snapshot = None;
                if next.form.as_ref().and_then(SettingsForm::settings).as_ref()
                    == match &intent.command {
                        Command::Settings(r) => Some(&r.settings),
                        _ => None,
                    }
                {
                    next.form = Some(SettingsForm::from_settings(&settings.settings));
                }
                if let Some(stored) = &mut next.stored {
                    stored.settings = Some(settings);
                }
            }
            Receipt::Rights(rights) => {
                if let Some(stored) = &mut next.stored {
                    next.snapshot = None;
                    stored.rights.retain(|r| {
                        r.claim.declaration.record_id != rights.claim.declaration.record_id
                    });
                    stored.rights.push(rights);
                }
            }
            Receipt::Freeze(snapshot) => {
                if let Some(stored) = &mut next.stored {
                    stored.snapshots.insert(
                        0,
                        ProductionSnapshotSummary {
                            id: snapshot.id.clone(),
                            script_id: snapshot.script_id.clone(),
                            created_by: snapshot.created_by.clone(),
                            recorded_at: snapshot.recorded_at.clone(),
                            input_digest: snapshot.document.input_digest.clone(),
                            script_revision: snapshot.document.revision.revision,
                            settings_version: snapshot.document.settings.version,
                        },
                    );
                    stored.snapshots.truncate(20);
                }
                next.snapshot = Some(*snapshot);
            }
            Receipt::Approve(_) => {
                next.snapshot = None;
            }
        }
        next
    }
}

pub fn error(code: ErrorCode) -> ApiError {
    ApiError {
        code,
        current_revision: None,
        issues: vec![],
    }
}

pub fn format_money(
    currency: ProductionCurrency,
    minor: u64,
    locale: crate::localization::Locale,
) -> String {
    let currency = match currency {
        ProductionCurrency::USD => cantos_localization::Currency::Usd,
        ProductionCurrency::VND => cantos_localization::Currency::Vnd,
    };
    crate::localization::present(
        cantos_localization::Formatter::try_new(locale.lang())
            .and_then(|formatter| formatter.currency(minor, currency)),
        locale,
    )
}

/// Derive permission IDs from the complete saved export, including sound assets and provenance.
pub fn evidence_ids(script: &str) -> Vec<String> {
    fn walk(value: &serde_json::Value, ids: &mut std::collections::BTreeSet<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    if key == "rights_record_id" {
                        if let Some(id) = value.as_str() {
                            ids.insert(id.into());
                        }
                    } else {
                        walk(value, ids);
                    }
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    walk(value, ids);
                }
            }
            _ => {}
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    if let Ok(value) = serde_json::from_str(script) {
        walk(&value, &mut ids);
    }
    ids.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SaveProductionSettingsRequest {
        serde_json::from_str(include_str!(
            "../../../../services/cantos-server/contracts/studio/production/v1/fixtures/settings-request.json"
        ))
        .unwrap()
    }

    fn settings_receipt(request: &SaveProductionSettingsRequest) -> ProductionSettingsResponse {
        ProductionSettingsResponse {
            script_id: "script".into(),
            version: request.expected_settings_version + 1,
            script_revision: request.expected_revision,
            operation_id: request.operation_id.clone(),
            recorded_by: "owner".into(),
            recorded_at: "2026-10-10T15:00:00Z".into(),
            settings: request.settings.clone(),
        }
    }

    fn state() -> Production {
        let request = request();
        Production {
            context: Context {
                actor: "owner".into(),
                script: "script".into(),
                revision: 1,
                export_digest: "complete-export".into(),
            },
            form: Some(SettingsForm::from_settings(&request.settings)),
            stored: Some(ProductionStateResponse {
                settings: None,
                rights: vec![],
                snapshots: vec![],
            }),
            ..Production::default()
        }
    }

    fn editor(actor: &str, revision: u64) -> Editor {
        editor_with_export(actor, revision, format!("export-{revision}"))
    }

    /// `actor` viewing saved `revision`, built only from public Editor transitions.
    fn editor_with_export(actor: &str, revision: u64, export_digest: String) -> Editor {
        Editor::default()
            .signed_in(actor.into())
            .open_revision(RevisionResponse {
                script_id: "script".into(),
                revision,
                accepted_by: "owner".into(),
                accepted_at: "2026-10-10T15:00:00Z".into(),
                content_digest: "content".into(),
                export_digest,
                script_json: "{}".into(),
            })
    }

    fn catalog() -> ProductionCatalogResponse {
        serde_json::from_str(include_str!(
            "../../../../services/cantos-server/contracts/studio/production/v1/fixtures/catalog-response.json"
        ))
        .unwrap()
    }

    fn narrator() -> BindingForm {
        SettingsForm::from_settings(&request().settings).bindings[0].clone()
    }

    #[test]
    fn explicit_neutral_at_intensity_zero_round_trips_distinct_from_inheritance() {
        let mut form = narrator();
        assert_eq!((form.emotion.as_str(), form.intensity.as_str()), ("", ""));
        let inherited = form.binding().unwrap().performance;
        assert_eq!(
            (inherited.emotion, inherited.intensity_permille),
            (None, None)
        );
        form.emotion = "neutral".into();
        form.intensity = "0".into();
        let explicit = form.binding().unwrap();
        assert_eq!(explicit.performance.emotion.as_deref(), Some("neutral"));
        assert_eq!(explicit.performance.intensity_permille, Some(0));
        let reopened = BindingForm::from_binding(&explicit);
        assert_eq!(
            (reopened.emotion.as_str(), reopened.intensity.as_str()),
            ("neutral", "0")
        );
    }

    #[test]
    fn provider_switch_changes_only_provider_and_model_and_flags_kept_values() {
        let catalog = catalog();
        let mut form = narrator();
        form.rate = "1200".into();
        form.emotion = "warm".into();
        form.intensity = "600".into();
        form.pronunciation = vec![ProductionPronunciation {
            surface: "Vọng Đài".into(),
            replacement: "Vọng Đài".into(),
        }];
        let before = form.clone();
        form.select_provider(&catalog, "espeak-ng-local".into());
        assert_eq!(
            form.model_id, "espeak-ng-1.52.0",
            "the only model is adopted"
        );
        assert_eq!(
            BindingForm {
                provider_id: before.provider_id.clone(),
                model_id: before.model_id.clone(),
                ..form.clone()
            },
            before,
            "voice, language, performance and pronunciation are never rewritten"
        );
        assert_eq!(
            form.unsupported(form.capability(&catalog)),
            vec![
                CastingControl::Voice,
                CastingControl::Rate,
                CastingControl::Emotion,
                CastingControl::Intensity
            ]
        );
        form.select_provider(&catalog, "google-cloud-tts".into());
        assert_eq!(
            form.model_id, "",
            "several models require an explicit choice"
        );
        assert!(form.capability(&catalog).is_none());
        assert_eq!(form.unsupported(None), vec![CastingControl::Model]);
        form.model_id = "google-neural2".into();
        form.select_provider(&catalog, "google-cloud-tts".into());
        assert_eq!(
            form.model_id, "google-neural2",
            "a model of the same provider is kept"
        );
        assert_eq!(
            form.unsupported(form.capability(&catalog)),
            vec![
                CastingControl::Voice,
                CastingControl::Emotion,
                CastingControl::Intensity
            ],
            "Neural2 admits rate 1200"
        );
    }

    #[test]
    fn espeak_and_google_profiles_accept_only_explicit_neutral_or_inherited_delivery() {
        let catalog = catalog();
        for (provider, model, voice) in [
            ("espeak-ng-local", "espeak-ng-1.52.0", "vi-vn-x-south"),
            (
                "google-cloud-tts",
                "google-chirp3-hd",
                "vi-VN-Chirp3-HD-Aoede",
            ),
            ("google-cloud-tts", "google-wavenet", "vi-VN-Wavenet-D"),
        ] {
            let mut form = narrator();
            form.select_provider(&catalog, provider.into());
            form.model_id = model.into();
            form.voice_id = voice.into();
            let capability = form.capability(&catalog);
            assert!(
                capability.is_some_and(|c| !c.intensity_supported),
                "{model}"
            );
            assert_eq!(form.unsupported(capability), vec![], "{model} inherit");
            form.emotion = "neutral".into();
            form.intensity = "0".into();
            assert_eq!(form.unsupported(capability), vec![], "{model} explicit");
            let performance = form.binding().unwrap().performance;
            assert_eq!(performance.intensity_permille, Some(0), "{model}");
            for (emotion, intensity, expected) in [
                ("calm", "0", CastingControl::Emotion),
                ("neutral", "1", CastingControl::Intensity),
            ] {
                let mut changed = form.clone();
                changed.emotion = emotion.into();
                changed.intensity = intensity.into();
                assert_eq!(
                    changed.unsupported(capability),
                    vec![expected],
                    "{model} {emotion}"
                );
            }
        }
    }

    #[test]
    fn unsupported_controls_follow_the_selected_profile_bounds() {
        let catalog = catalog();
        let mut form = narrator();
        form.select_provider(&catalog, "google-cloud-tts".into());
        form.model_id = "google-neural2".into();
        form.voice_id = "vi-VN-Neural2-A".into();
        for (rate, pitch, expected) in [
            ("250", "-20", vec![]),
            ("2000", "20", vec![]),
            (
                "249",
                "21",
                vec![CastingControl::Rate, CastingControl::Pitch],
            ),
            ("not-a-number", "", vec![]),
        ] {
            form.rate = rate.into();
            form.pitch = pitch.into();
            assert_eq!(
                form.unsupported(form.capability(&catalog)),
                expected,
                "{rate}"
            );
        }
        form.rate = "1000".into();
        form.pitch = "0".into();
        form.language = "en".into();
        assert_eq!(
            form.unsupported(form.capability(&catalog)),
            vec![CastingControl::Language]
        );
        form.language = "vi-VN".into();
        form.pronunciation.push(ProductionPronunciation {
            surface: "a".into(),
            replacement: "b".into(),
        });
        let mut without = form.capability(&catalog).unwrap().clone();
        without.pronunciation_supported = false;
        assert_eq!(
            form.unsupported(Some(&without)),
            vec![CastingControl::Pronunciation]
        );
        assert_eq!(
            form.pronunciation.len(),
            1,
            "flagging never removes overrides"
        );
    }

    #[test]
    fn retry_preserves_exact_actor_operation_version_and_settings() {
        let original = state();
        let (sending, intent) = original.start(Command::Settings(request())).unwrap();
        assert!(sending.start(Command::Settings(request())).is_none());
        let uncertain = sending.failed(&intent, error(ErrorCode::Unavailable));
        assert_eq!(uncertain.pending(), Some(&intent));
        let (_, retry) = uncertain.retry().unwrap();
        assert_eq!(retry, intent);
        let other = uncertain.observe(&editor("other", 1));
        assert_eq!(other.pending(), Some(&intent));
        assert!(!other.can_retry());
    }

    fn assert_denied_retry_retains_unknown_settings_outcome(denial: ErrorCode) {
        let original = state().edit(|form| form.bindings[0].rate = "1200".into());
        let mut original_request = request();
        original_request.operation_id = "original-settings-operation".into();
        original_request.settings = original.form.as_ref().unwrap().settings().unwrap();
        let (sending, intent) = original
            .start(Command::Settings(original_request.clone()))
            .unwrap();
        let uncertain = sending.failed(&intent, error(ErrorCode::Unavailable));
        let (retrying, replay) = uncertain.retry().unwrap();
        assert_eq!(replay, intent);
        let denied = retrying.failed(&replay, error(denial.clone()));
        assert_eq!(
            denied.pending(),
            Some(&intent),
            "a retry denial cannot establish the first attempt's outcome"
        );
        assert_eq!(denied.form, original.form);
        assert_eq!(denied.stored, original.stored);
        assert_eq!(denied.error, Some(error(denial.clone())));
        assert!(!denied.busy());
        assert!(denied.can_retry());
        assert!(denied.has_activity());
        let mut fresh_request = original_request;
        fresh_request.operation_id = "replacement-settings-operation".into();
        assert!(denied.start(Command::Settings(fresh_request)).is_none());
        assert!(denied.discard_drafts().is_none());
        let (retrying, repeated_replay) = denied.retry().unwrap();
        assert_eq!(repeated_replay, intent);
        let denied_again = retrying.failed(&repeated_replay, error(denial));
        assert_eq!(denied_again, denied);
    }

    #[test]
    fn forbidden_retry_preserves_unknown_settings_operation_and_form() {
        assert_denied_retry_retains_unknown_settings_outcome(ErrorCode::Forbidden);
    }

    #[test]
    fn not_found_retry_preserves_unknown_settings_operation_and_form() {
        assert_denied_retry_retains_unknown_settings_outcome(ErrorCode::NotFound);
    }

    #[test]
    fn unknown_settings_outcome_survives_actor_and_revision_context_return() {
        for denial in [ErrorCode::Forbidden, ErrorCode::NotFound] {
            let original = state().edit(|form| form.scope = "private-stage-2".into());
            let (sending, intent) = original.start(Command::Settings(request())).unwrap();
            let uncertain = sending.failed(&intent, error(ErrorCode::Unavailable));
            let other = uncertain.observe(&editor("other", 1));
            assert!(other.form.is_none());
            assert!(other.retry().is_none());
            let rejected_elsewhere = other.failed(&intent, error(denial.clone()));
            assert_eq!(rejected_elsewhere.pending(), Some(&intent));
            assert_eq!(rejected_elsewhere.error, None);
            let returned = rejected_elsewhere.observe(&editor("owner", 2));
            assert_eq!(returned.form, original.form);
            let original_editor =
                editor_with_export("owner", 1, original.context.export_digest.clone());
            let restored = returned.observe(&original_editor);
            assert_eq!(restored.context, intent.context);
            assert_eq!(restored.form, original.form);
            assert_eq!(restored.pending(), Some(&intent));
            let (retrying, replay) = restored.retry().unwrap();
            assert_eq!(replay, intent);
            let denied = retrying.failed(&replay, error(denial.clone()));
            assert_eq!(denied.pending(), Some(&intent));
            assert_eq!(denied.form, original.form);
            assert_eq!(denied.error, Some(error(denial)));
            assert!(denied.start(Command::Settings(request())).is_none());
            assert!(denied.discard_drafts().is_none());
        }
    }

    #[test]
    fn only_matching_settings_receipt_settles_an_unknown_outcome() {
        let original = state();
        let (sending, intent) = original.start(Command::Settings(request())).unwrap();
        let uncertain = sending.failed(&intent, error(ErrorCode::Unavailable));
        let (retrying, replay) = uncertain.retry().unwrap();
        let denied = retrying.failed(&replay, error(ErrorCode::Forbidden));
        let mut different_intent = intent.clone();
        if let Command::Settings(request) = &mut different_intent.command {
            request.operation_id = "another-operation".into();
        }
        assert_eq!(
            denied.failed(&different_intent, error(ErrorCode::Forbidden)),
            denied
        );
        let mut mismatched_receipt = settings_receipt(&request());
        mismatched_receipt.operation_id = "another-operation".into();
        let unresolved = denied.recorded(&intent, Receipt::Settings(mismatched_receipt));
        assert_eq!(unresolved.pending(), Some(&intent));
        assert_eq!(unresolved.form, original.form);
        assert_eq!(unresolved.error, Some(error(ErrorCode::Unavailable)));
        let settled = unresolved.recorded(&intent, Receipt::Settings(settings_receipt(&request())));
        assert!(settled.pending().is_none());
        assert_eq!(settled.request, Request::Idle);
        assert!(!settled.has_activity());
        let mut next_request = request();
        next_request.operation_id = "next-settings-operation".into();
        next_request.expected_settings_version = 1;
        let (sending, next_intent) = settled.start(Command::Settings(next_request)).unwrap();
        let denied = sending.failed(&next_intent, error(ErrorCode::NotFound));
        assert!(denied.pending().is_none());
        assert_eq!(denied.request, Request::Idle);
    }

    #[test]
    fn first_attempt_denial_does_not_invent_an_unknown_outcome() {
        for denial in [ErrorCode::Forbidden, ErrorCode::NotFound] {
            let original = state();
            let (sending, intent) = original.start(Command::Settings(request())).unwrap();
            let denied = sending.failed(&intent, error(denial.clone()));
            assert!(denied.pending().is_none());
            assert_eq!(denied.request, Request::Idle);
            assert_eq!(denied.form, original.form);
            assert_eq!(denied.stored, original.stored);
            assert_eq!(denied.error, Some(error(denial)));
            assert!(!denied.has_activity());
            assert!(denied.retry().is_none());
            assert!(denied.start(Command::Settings(request())).is_some());
        }
    }

    #[test]
    fn matching_receipt_settles_unknown_outcome_while_other_actor_stays_private() {
        let (sending, intent) = state().start(Command::Settings(request())).unwrap();
        let uncertain = sending.failed(&intent, error(ErrorCode::Unavailable));
        let switched = uncertain.observe(&editor("other", 1));
        let settled = switched.recorded(&intent, Receipt::Settings(settings_receipt(&request())));
        assert!(settled.pending().is_none());
        assert_eq!(settled.request, Request::Idle);
        assert!(!settled.has_activity());
        assert!(settled.form.is_none());
        assert!(settled.stored.is_none());
        assert!(settled.error.is_none());
    }

    #[test]
    fn newer_typing_survives_the_acknowledgement_of_older_settings() {
        let (sending, intent) = state().start(Command::Settings(request())).unwrap();
        let changed = sending.edit(|form| form.bindings[0].pitch = "3".into());
        let acknowledged =
            changed.recorded(&intent, Receipt::Settings(settings_receipt(&request())));
        assert_eq!(acknowledged.form.as_ref().unwrap().bindings[0].pitch, "3");
        assert!(acknowledged.is_dirty());
        assert!(acknowledged.pending().is_none());
        assert_eq!(
            acknowledged
                .stored
                .unwrap()
                .settings
                .unwrap()
                .settings
                .bindings[0]
                .performance
                .pitch_semitones,
            0
        );
    }

    #[test]
    fn actor_switch_hides_then_restores_unsaved_settings_and_old_receipt_is_invisible() {
        let original = state();
        let changed = original.edit(|form| form.scope = "private-stage-2".into());
        let switched = changed.observe(&editor("other", 1));
        assert!(switched.form.is_none());
        assert!(switched.retained.is_some());
        let restored = switched.observe(&editor("owner", 1));
        assert_eq!(restored.form.as_ref().unwrap().scope, "private-stage-2");
        let (sending, intent) = original.start(Command::Settings(request())).unwrap();
        let switched = sending.observe(&editor("other", 1));
        let acknowledged =
            switched.recorded(&intent, Receipt::Settings(settings_receipt(&request())));
        assert!(acknowledged.form.is_none());
        assert!(acknowledged.stored.is_none());
        assert!(acknowledged.pending().is_none());
    }

    #[test]
    fn response_actor_operation_version_and_settings_must_match_the_pending_intent() {
        let (sending, intent) = state().start(Command::Settings(request())).unwrap();
        for field in ["actor", "operation", "version", "settings"] {
            let mut receipt = settings_receipt(&request());
            match field {
                "actor" => receipt.recorded_by = "another-owner".into(),
                "operation" => receipt.operation_id = "another-operation".into(),
                "version" => receipt.version = 2,
                "settings" => receipt.settings.budget.limit_minor = 999,
                _ => unreachable!(),
            };
            let result = sending.recorded(&intent, Receipt::Settings(receipt));
            assert_eq!(result.pending(), Some(&intent));
            assert_eq!(result.error.unwrap().code, ErrorCode::Unavailable);
        }
    }

    #[test]
    fn context_change_abandons_a_read_and_holds_a_command_for_reconciliation() {
        let (reading, ticket) = state().start_read(ReadKind::State).unwrap();
        assert!(reading.busy());
        let moved = reading.observe(&editor("other", 1));
        assert_eq!(moved.request, Request::Idle);
        assert!(!moved.has_activity());
        assert_eq!(
            moved.read_failed(&ticket, error(ErrorCode::Unavailable)),
            moved
        );

        let (sending, intent) = state().start(Command::Settings(request())).unwrap();
        assert!(sending.busy() && !sending.can_retry());
        let moved = sending.observe(&editor("other", 1));
        assert!(!moved.busy());
        assert_eq!(moved.pending(), Some(&intent));
        assert!(!moved.can_retry(), "only the submitting actor may retry");
        let returned = moved.observe(&editor("owner", 1));
        assert!(returned.can_retry());
        let (retrying, retried) = returned.retry().unwrap();
        assert_eq!(retried, intent);
        assert!(retrying.busy() && retrying.retry().is_none());
    }

    #[test]
    fn stale_read_cannot_replace_new_session_or_revision_context() {
        let (reading, ticket) = state().start_read(ReadKind::State).unwrap();
        let changed = reading.observe(&editor("owner", 2));
        let response = ProductionStateResponse {
            settings: Some(settings_receipt(&request())),
            rights: vec![],
            snapshots: vec![],
        };
        let result = changed.loaded(
            &ticket,
            ProductionCatalogResponse {
                version: "reference".into(),
                capabilities: vec![],
            },
            response,
            "{}",
        );
        assert_eq!(result, changed);
        assert!(result.preview.is_none());
        assert!(result.is_dirty());
    }

    #[test]
    fn new_script_revision_keeps_casting_text_but_requires_a_new_settings_version() {
        let clean = state().recorded_for_test();
        assert!(!clean.is_dirty());
        let edited = clean.edit(|form| form.bindings[0].rate = "1100".into());
        let advanced = edited.observe(&editor("owner", 2));
        assert_eq!(advanced.form.unwrap().bindings[0].rate, "1100");
        let advanced = clean.observe(&editor("owner", 2));
        assert!(advanced.is_dirty());
        assert!(advanced.start_read(ReadKind::Preview).is_none());
    }

    impl Production {
        fn recorded_for_test(&self) -> Self {
            let (sending, intent) = self.start(Command::Settings(request())).unwrap();
            sending.recorded(&intent, Receipt::Settings(settings_receipt(&request())))
        }
    }

    #[test]
    fn every_local_settings_edit_clears_review_and_exact_scope_acknowledgement() {
        let original = state().recorded_for_test();
        let preview = ProductionPreviewResponse {
            input_digest: "scope".into(),
            script_revision: 1,
            settings_version: 1,
            resolved: vec![],
            estimate: ProductionEstimate::Unavailable {
                reason: "none".into(),
            },
            findings: vec![],
            inputs_eligible: false,
            billable_dispatch_available: false,
        };
        let (reading, ticket) = original.start_read(ReadKind::Preview).unwrap();
        let reviewed = reading.preview_loaded(&ticket, preview);
        let mut confirmed = reviewed.clone();
        confirmed.confirmation = Some(("snapshot".into(), "scope".into()));
        for field in [
            "voice",
            "model",
            "language",
            "performance",
            "pronunciation",
            "rights",
            "budget",
            "scope",
        ] {
            let edited = confirmed.edit(|form| match field {
                "voice" => form.bindings[0].voice_id = "synthetic-character".into(),
                "model" => form.bindings[0].model_id = "reference-v2".into(),
                "language" => form.bindings[0].language = "en".into(),
                "performance" => form.bindings[0].pitch = "2".into(),
                "pronunciation" => form.bindings[0]
                    .pronunciation
                    .push(ProductionPronunciation {
                        surface: "Vọng Đài".into(),
                        replacement: "Vọng Đài".into(),
                    }),
                "rights" => form.bindings[0].voice_rights_record_id = "new-permission".into(),
                "budget" => form.limit = "101".into(),
                "scope" => form.scope = "different-stage".into(),
                _ => unreachable!(),
            });
            assert!(edited.preview.is_none(), "{field}");
            assert!(edited.confirmation.is_none(), "{field}");
            assert!(edited.is_dirty(), "{field}");
        }
    }

    #[test]
    fn unsaved_permission_changes_clear_acknowledgement_and_block_preview_freeze_and_approval() {
        let mut reviewed = state().recorded_for_test();
        reviewed.preview = Some(ProductionPreviewResponse {
            input_digest: "scope".into(),
            script_revision: 1,
            settings_version: 1,
            resolved: vec![],
            estimate: ProductionEstimate::Unavailable {
                reason: "none".into(),
            },
            findings: vec![],
            inputs_eligible: false,
            billable_dispatch_available: false,
        });
        reviewed.confirmation = Some(("snapshot".into(), "scope".into()));
        let edited = reviewed.rights_edited();
        assert!(edited.rights_draft_dirty);
        assert!(edited.preview.is_none());
        assert!(edited.confirmation.is_none());
        assert!(edited.start_read(ReadKind::Preview).is_none());
        assert!(edited
            .start(Command::Freeze(FreezeProductionRequest {
                operation_id: "freeze".into(),
                expected_revision: 1,
                settings_version: 1,
                input_digest: "scope".into()
            }))
            .is_none());
        assert!(edited
            .start(Command::Approve {
                snapshot_id: "snapshot".into(),
                request: ApproveProductionRequest {
                    operation_id: "approve".into(),
                    input_digest: "scope".into()
                }
            })
            .is_none());
        let reconciled = edited.rights_reconciled();
        assert!(!reconciled.is_dirty());
        assert!(reconciled.preview.is_none());
        assert!(reconciled.confirmation.is_none());
    }

    #[test]
    fn integer_forms_are_exact_and_invalid_values_remain_editable() {
        let mut form = SettingsForm::from_settings(&request().settings);
        form.limit = u64::MAX.to_string();
        assert_eq!(form.settings().unwrap().budget.limit_minor, u64::MAX);
        form.bindings[0].pitch = "-12".into();
        assert_eq!(
            form.settings().unwrap().bindings[0]
                .performance
                .pitch_semitones,
            -12
        );
        for invalid in ["1.5", "-1", "18446744073709551616", ""] {
            form.limit = invalid.into();
            assert_eq!(form.settings(), None);
            assert_eq!(form.limit, invalid);
        }
    }

    #[test]
    fn rights_subjects_come_from_all_saved_script_evidence_not_only_content_digest() {
        assert_eq!(
            evidence_ids(include_str!(
                "../../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json"
            )),
            vec!["rights-adaptation-demo", "rights-asset-demo", "rights-demo"]
        );
    }

    #[test]
    fn money_display_uses_currency_exponent_without_float_rounding() {
        assert_eq!(
            format_money(
                ProductionCurrency::USD,
                3,
                crate::localization::Locale::English
            ),
            "USD\u{a0}0.03"
        );
        assert_eq!(
            format_money(
                ProductionCurrency::USD,
                3,
                crate::localization::Locale::Vietnamese
            ),
            "0,03\u{a0}USD"
        );
        assert_eq!(
            format_money(
                ProductionCurrency::VND,
                18_500,
                crate::localization::Locale::Vietnamese
            ),
            "18.500\u{a0}VND"
        );
        assert_eq!(
            format_money(
                ProductionCurrency::USD,
                u64::MAX,
                crate::localization::Locale::English
            ),
            "USD\u{a0}184,467,440,737,095,516.15"
        );
    }

    fn snapshot(id: &str) -> ProductionSnapshotResponse {
        ProductionSnapshotResponse {
            id: id.into(),
            script_id: "script".into(),
            created_by: "owner".into(),
            recorded_at: "2026-10-10T15:00:00Z".into(),
            operation_id: "freeze".into(),
            document: FrozenProductionDocument {
                owner_id: "owner".into(),
                catalog_version: "reference-catalog-v1".into(),
                revision: editor("owner", 1).remote().cloned().unwrap(),
                settings: settings_receipt(&request()),
                rights: vec![],
                resolved: vec![],
                estimate: ProductionEstimate::Unavailable {
                    reason: "not attested".into(),
                },
                input_digest: "scope".into(),
            },
            approval: None,
            eligibility: ProductionEligibility {
                inputs_eligible: true,
                approval_current: false,
                billable_dispatch_available: false,
                findings: vec![ProductionFinding {
                    code: ProductionFindingCode::ApprovalMissing,
                    path: "approval".into(),
                    detail: "none".into(),
                }],
            },
        }
    }

    #[test]
    fn snapshot_reads_require_the_requested_identity_and_clear_old_acknowledgement() {
        let original = state().recorded_for_test();
        let (reading, ticket) = original
            .start_read(ReadKind::Snapshot("chosen".into()))
            .unwrap();
        let rejected = reading.snapshot_loaded(&ticket, snapshot("other"));
        assert_eq!(rejected.error.unwrap().code, ErrorCode::CorruptRevision);
        assert!(rejected.snapshot.is_none());
        let loaded = reading.snapshot_loaded(&ticket, snapshot("chosen"));
        let confirmed = loaded.confirm(true);
        assert_eq!(
            confirmed.confirmation,
            Some(("chosen".into(), "scope".into()))
        );
        let (reading, ticket) = confirmed
            .start_read(ReadKind::Snapshot("newer".into()))
            .unwrap();
        let latest = reading.snapshot_loaded(&ticket, snapshot("newer"));
        assert!(latest.confirmation.is_none());
        assert_eq!(latest.snapshot.unwrap().id, "newer");
    }

    #[test]
    fn approval_acknowledgement_never_invents_current_eligibility_and_requires_authoritative_read()
    {
        let mut original = state().recorded_for_test();
        original.snapshot = Some(snapshot("chosen"));
        let original = original.confirm(true);
        let (sending, intent) = original
            .start(Command::Approve {
                snapshot_id: "chosen".into(),
                request: ApproveProductionRequest {
                    operation_id: "approve".into(),
                    input_digest: "scope".into(),
                },
            })
            .unwrap();
        let recorded = sending.recorded(
            &intent,
            Receipt::Approve(ProductionApprovalResponse {
                id: "approval".into(),
                snapshot_id: "chosen".into(),
                approved_by: "owner".into(),
                approved_at: "2026-10-10T15:00:00Z".into(),
                operation_id: "approve".into(),
                input_digest: "scope".into(),
            }),
        );
        assert!(recorded.snapshot.is_none());
        assert!(recorded.pending().is_none());
        assert!(recorded.confirmation.is_none());
        assert!(recorded
            .start_read(ReadKind::Snapshot("chosen".into()))
            .is_some());
    }

    #[test]
    fn explicit_discard_restores_stored_settings_and_never_abandons_an_uncertain_operation() {
        let clean = state().recorded_for_test();
        let dirty = clean
            .edit(|form| form.bindings[0].rate = "1200".into())
            .rights_edited();
        let discarded = dirty.discard_drafts().unwrap();
        assert!(!discarded.is_dirty());
        assert_eq!(discarded.form.unwrap().bindings[0].rate, "1000");
        let (sending, intent) = state().start(Command::Settings(request())).unwrap();
        let uncertain = sending.failed(&intent, error(ErrorCode::Unavailable));
        assert!(uncertain.discard_drafts().is_none());
    }

    #[test]
    fn unfinished_composition_blocks_settings_and_rights_commands_until_native_text_commits() {
        let rights: SaveProductionRightsRequest = serde_json::from_str(include_str!(
            "../../../../services/cantos-server/contracts/studio/production/v1/fixtures/rights-request.json"
        ))
        .unwrap();
        let mut composing = state();
        composing.input_composing = true;
        for command in [Command::Settings(request()), Command::Rights(rights)] {
            assert!(composing.start(command.clone()).is_none());
            let mut committed = composing.clone();
            committed.input_composing = false;
            assert!(committed.start(command).is_some());
        }
    }

    #[test]
    fn unfinished_composition_blocks_every_read_without_disabling_the_native_field() {
        let mut composing = state().recorded_for_test();
        composing.input_composing = true;
        for kind in [
            ReadKind::State,
            ReadKind::Preview,
            ReadKind::Snapshot("chosen".into()),
        ] {
            assert!(composing.start_read(kind.clone()).is_none());
            assert!(!composing.has_activity());
            let mut committed = composing.clone();
            committed.input_composing = false;
            assert!(committed.start_read(kind).is_some());
        }
    }
}
