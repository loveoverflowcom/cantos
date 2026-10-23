//! Owner-side Work delegation presentation (ADR 0028, delegation v1). The backend decides
//! ownership, actor registration, action admission, expiry bounds and lifecycle; this module
//! only shapes raw requests, correlates replies and maps typed facts to presentation.
//!
//! The show-once `cantos_dlg_` secret is held only in memory: it has no `Serialize`, `Display`
//! or readable `Debug`, is never placed in a URL or storage, and is dropped on dismissal,
//! revocation, Work change or actor change.
use super::{ClientError, Command, Phase, Reply, RequestState};
use crate::contract::{
    DelegationAction, DelegationGrantResponse, ErrorCode, FieldIssue, IssueDelegationRequest,
    IssueDelegationResponse,
};
use std::collections::BTreeSet;

pub mod copy;
#[cfg(target_arch = "wasm32")]
pub mod panel;
#[cfg(test)]
mod tests;
pub mod view;

/// In-flight effects block authentication controls; retained writes only protect their owner.
#[cfg(any(test, target_arch = "wasm32"))]
pub(super) struct RequestActivity {
    pub busy: bool,
    pub unresolved: bool,
}

#[cfg(any(test, target_arch = "wasm32"))]
pub(super) fn request_activity(state: &RequestState) -> RequestActivity {
    RequestActivity {
        busy: state.phase() == Phase::Pending,
        unresolved: state.pending().is_some(),
    }
}

/// Grants per list request; the contract accepts 1..=50.
pub const PAGE_LIMIT: u8 = 20;
const TOKEN_PREFIX: &str = "cantos_dlg_";
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// The closed action set in canonical (contract) order.
pub const ACTIONS: [DelegationAction; 3] = [
    DelegationAction::WorkRead,
    DelegationAction::ContentRead,
    DelegationAction::ContentWrite,
];

/// A server-minted delegation credential exactly as the contract spells it.
///
/// ```compile_fail
/// use cantos_studio::live::delegation::DelegationToken;
/// let token = DelegationToken(String::new());
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct DelegationToken(String);

impl std::fmt::Debug for DelegationToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DelegationToken([redacted])")
    }
}

impl DelegationToken {
    /// `None` for anything that is not `cantos_dlg_` plus 64 lowercase hexadecimal characters.
    pub fn parse(value: String) -> Option<Self> {
        let hex = value.strip_prefix(TOKEN_PREFIX)?;
        (hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then_some(Self(value))
    }
    /// Only the one-time reveal field and the explicit clipboard effect read the secret.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

/// An issuance acknowledgement. `token` is present only on the response that created the
/// grant (`201`); a retry of the same operation (`200`) can never recover it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuedGrant {
    pub grant: DelegationGrantResponse,
    pub token: Option<DelegationToken>,
}

impl IssuedGrant {
    /// Status and secret must agree: `201` with a well-formed token or `200` with none.
    pub fn admit(status: u16, response: IssueDelegationResponse) -> Option<Self> {
        let token = match (status, response.token) {
            (201, Some(token)) => Some(DelegationToken::parse(token)?),
            (200, None) => None,
            _ => return None,
        };
        Some(Self {
            grant: response.grant,
            token,
        })
    }
    /// Correlate to the exact request; never decides whether the grant is still usable.
    pub fn answers(&self, work: &str, request: &IssueDelegationRequest) -> bool {
        let mut granted = self.grant.actions.clone();
        let mut requested = request.actions.clone();
        granted.sort();
        requested.sort();
        self.grant.id == request.id
            && self.grant.work_id == work
            && self.grant.delegate_actor_id == request.delegate_actor_id
            && self.grant.expires_at == request.expires_at
            && granted == requested
    }
}

/// Offered lifetimes. The server measures `expires_at` from its own issuance time and refuses
/// anything beyond 30 days; the browser never decides that bound.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lifetime {
    OneDay,
    #[default]
    Week,
    TwoWeeks,
    Maximum,
}

impl Lifetime {
    pub const ALL: [Self; 4] = [Self::OneDay, Self::Week, Self::TwoWeeks, Self::Maximum];

    pub const fn days(self) -> i64 {
        match self {
            Self::OneDay => 1,
            Self::Week => 7,
            Self::TwoWeeks => 14,
            Self::Maximum => 30,
        }
    }
    pub const fn value(self) -> &'static str {
        match self {
            Self::OneDay => "1",
            Self::Week => "7",
            Self::TwoWeeks => "14",
            Self::Maximum => "30",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|lifetime| lifetime.value() == value)
    }
    pub fn expires_at(self, now_ms: i64) -> Option<i64> {
        now_ms.checked_add(self.days().checked_mul(DAY_MS)?)
    }
}

/// Raw form values. Admission (syntax, self-delegation, registration, duplicates, expiry) is
/// the server's; an empty selection is sent so the server's `empty` refusal is shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssueDraft {
    pub delegate: String,
    pub actions: Vec<DelegationAction>,
    pub lifetime: Lifetime,
}

impl Default for IssueDraft {
    /// Read-only by default; content writing is an explicit choice.
    fn default() -> Self {
        Self {
            delegate: String::new(),
            actions: vec![DelegationAction::WorkRead, DelegationAction::ContentRead],
            lifetime: Lifetime::default(),
        }
    }
}

impl IssueDraft {
    /// A current draft or still-visible show-once secret needs navigation protection.
    pub fn is_dirty(&self, has_secret: bool) -> bool {
        has_secret || *self != Self::default()
    }
    pub fn acknowledge_issue(&mut self, submitted: &Self) -> bool {
        if self == submitted {
            *self = Self::default();
            true
        } else {
            false
        }
    }
    /// Toggle one action, keeping canonical order and no duplicates.
    pub fn with_action(&self, action: DelegationAction, selected: bool) -> Self {
        let actions = ACTIONS
            .into_iter()
            .filter(|candidate| {
                if *candidate == action {
                    selected
                } else {
                    self.actions.contains(candidate)
                }
            })
            .collect();
        Self {
            actions,
            ..self.clone()
        }
    }
    pub fn request(
        &self,
        operation_id: String,
        id: String,
        now_ms: i64,
    ) -> Result<IssueDelegationRequest, ClientError> {
        Ok(IssueDelegationRequest {
            operation_id,
            id,
            delegate_actor_id: self.delegate.trim().to_owned(),
            actions: ACTIONS
                .into_iter()
                .filter(|action| self.actions.contains(action))
                .collect(),
            expires_at: self
                .lifetime
                .expires_at(now_ms)
                .ok_or(ClientError::InvalidInput)?,
        })
    }
}

/// Lifecycle shown beside a grant. `Revoked` is a server fact (or this session's acknowledged
/// revocation); `Expired` compares the server's `expires_at` with the device clock for display
/// only. The server alone admits a delegated request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantState {
    Active,
    Expired,
    Revoked,
}

pub fn grant_state(grant: &DelegationGrantResponse, revoked_here: bool, now_ms: i64) -> GrantState {
    if grant.revoked_at.is_some() || revoked_here {
        GrantState::Revoked
    } else if now_ms >= grant.expires_at {
        GrantState::Expired
    } else {
        GrantState::Active
    }
}

/// The single reveal of a freshly created secret, bound to the grant that owns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reveal {
    pub grant: DelegationGrantResponse,
    pub token: DelegationToken,
}

/// Acknowledged delegation facts for one selected Work.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grants {
    pub work: String,
    pub items: Vec<DelegationGrantResponse>,
    pub next_after: Option<String>,
    pub loaded: bool,
    /// Revocations acknowledged by `204` in this session; their time arrives with the next read.
    pub revoked_here: BTreeSet<String>,
    pub reveal: Option<Reveal>,
    /// A replayed issuance (`200`) whose secret can never be shown again.
    pub unrecoverable: Option<DelegationGrantResponse>,
    /// The server refused owner access (`403`).
    pub forbidden: bool,
}

impl Grants {
    pub fn for_work(work: &str) -> Self {
        Self {
            work: work.to_owned(),
            ..Self::default()
        }
    }
    fn upsert(items: &mut Vec<DelegationGrantResponse>, grant: DelegationGrantResponse) {
        match items.iter_mut().find(|item| item.id == grant.id) {
            Some(item) => *item = grant,
            None => items.push(grant),
        }
        // Mirror the server's grant-ID order across pages and local inserts.
        items.sort_by(|a, b| a.id.cmp(&b.id));
    }
    /// Project an acknowledged reply; replies for another Work never change these facts.
    pub fn apply(&self, command: &Command, reply: Reply) -> Self {
        let mut next = self.clone();
        match (command, reply) {
            (Command::Delegations { work, after }, Reply::Delegations(page))
                if *work == self.work =>
            {
                if after.is_none() {
                    next.items.clear();
                }
                for grant in page.items {
                    Self::upsert(&mut next.items, grant);
                }
                next.next_after = page.next_after;
                next.loaded = true;
                next.forbidden = false;
                // A read that carries the server's revocation time supersedes the local mark.
                next.revoked_here.retain(|id| {
                    !next
                        .items
                        .iter()
                        .any(|grant| grant.id == *id && grant.revoked_at.is_some())
                });
                // Revocation may have happened in another owner tab. Absence from one page
                // says nothing, but an authoritative revocation ends the reveal here too.
                if next.reveal.as_ref().is_some_and(|reveal| {
                    next.items
                        .iter()
                        .any(|grant| grant.id == reveal.grant.id && grant.revoked_at.is_some())
                }) {
                    next.reveal = None;
                }
            }
            (Command::IssueDelegation { work, .. }, Reply::DelegationIssued(issued))
                if *work == self.work =>
            {
                let IssuedGrant { grant, token } = *issued;
                Self::upsert(&mut next.items, grant.clone());
                match token {
                    Some(token) => {
                        next.reveal = Some(Reveal { grant, token });
                        next.unrecoverable = None;
                    }
                    None => {
                        next.reveal = None;
                        next.unrecoverable = Some(grant);
                    }
                }
            }
            (Command::RevokeDelegation { work, grant }, Reply::Revoked) if *work == self.work => {
                next.revoked_here.insert(grant.clone());
                // A revoked credential is useless; stop displaying it.
                if next.reveal.as_ref().is_some_and(|r| r.grant.id == *grant) {
                    next.reveal = None;
                }
                if next.unrecoverable.as_ref().is_some_and(|g| g.id == *grant) {
                    next.unrecoverable = None;
                }
            }
            _ => {}
        }
        next
    }
    pub fn dismiss_reveal(&self) -> Self {
        Self {
            reveal: None,
            ..self.clone()
        }
    }
    pub fn forbid(&self) -> Self {
        Self {
            forbidden: true,
            reveal: None,
            ..self.clone()
        }
    }
    pub fn revoked_here(&self, grant: &str) -> bool {
        self.revoked_here.contains(grant)
    }
}

/// The issuance form field a refusal belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Delegate,
    Actions,
    Expiry,
}

/// Every refusal the panel can explain. Unknown server issues stay visible verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    InvalidActor,
    SelfDelegation,
    UnavailableActor,
    EmptyActions,
    DuplicateAction,
    InvalidExpiry,
    DuplicateGrantId,
    Archived,
    OperationReused,
    NotOwner,
    NotFound,
    SignIn,
    Malformed,
    Unavailable,
    Interrupted,
    UnexpectedReply,
    InvalidInput,
    Unrecognized { path: String, rule: String },
}

impl Problem {
    pub fn field(&self) -> Option<Field> {
        match self {
            Self::InvalidActor | Self::SelfDelegation | Self::UnavailableActor => {
                Some(Field::Delegate)
            }
            Self::EmptyActions | Self::DuplicateAction => Some(Field::Actions),
            Self::InvalidExpiry => Some(Field::Expiry),
            Self::DuplicateGrantId
            | Self::Archived
            | Self::OperationReused
            | Self::NotOwner
            | Self::NotFound
            | Self::SignIn
            | Self::Malformed
            | Self::Unavailable
            | Self::Interrupted
            | Self::UnexpectedReply
            | Self::InvalidInput
            | Self::Unrecognized { .. } => None,
        }
    }
}

fn issue_problem(issue: &FieldIssue) -> Problem {
    match (issue.path.as_str(), issue.rule.as_str()) {
        ("delegate_actor_id", "invalid_actor") => Problem::InvalidActor,
        ("delegate_actor_id", "self_delegation") => Problem::SelfDelegation,
        ("delegate_actor_id", "unavailable_actor") => Problem::UnavailableActor,
        ("actions", "empty") => Problem::EmptyActions,
        ("actions", "duplicate_action") => Problem::DuplicateAction,
        ("expires_at", "invalid_expiry") => Problem::InvalidExpiry,
        ("id", "duplicate_id") => Problem::DuplicateGrantId,
        ("lifecycle", "archived") => Problem::Archived,
        (path, rule) => Problem::Unrecognized {
            path: path.to_owned(),
            rule: rule.to_owned(),
        },
    }
}

/// Exhaustive projection of a refused or interrupted delegation request.
pub fn problems(error: &ClientError) -> Vec<Problem> {
    let ClientError::Http { status, error } = error else {
        return vec![match error {
            ClientError::InvalidInput => Problem::InvalidInput,
            ClientError::ReplyMismatch | ClientError::Protocol { .. } => Problem::UnexpectedReply,
            ClientError::Cancelled
            | ClientError::SessionChanged
            | ClientError::ReceiptNotEstablished
            | ClientError::RateLimited { .. }
            | ClientError::Transport => Problem::Interrupted,
            ClientError::Http { .. } => Problem::Unavailable,
        }];
    };
    let from_issues = |fallback: Problem| -> Vec<Problem> {
        if error.issues.is_empty() {
            vec![fallback]
        } else {
            error.issues.iter().map(issue_problem).collect()
        }
    };
    match (status, &error.code) {
        (_, ErrorCode::Unauthenticated) => vec![Problem::SignIn],
        (422, _) => from_issues(Problem::Malformed),
        (409, ErrorCode::OperationReused) => vec![Problem::OperationReused],
        (409, _) => from_issues(Problem::Unavailable),
        (403, _) => vec![Problem::NotOwner],
        (404, _) => vec![Problem::NotFound],
        (400, _) => vec![Problem::Malformed],
        _ => vec![Problem::Unavailable],
    }
}

/// The first field-level refusal for `field`, if any.
pub fn field_problem(problems: &[Problem], field: Field) -> Option<Problem> {
    problems
        .iter()
        .find(|problem| problem.field() == Some(field))
        .cloned()
}

/// The one progress line the panel's polite status region shows; refusals are listed apart.
pub fn progress(state: &RequestState) -> Option<copy::Key> {
    use copy::Key;
    match state.phase() {
        Phase::Idle => Some(Key::Idle),
        Phase::Pending => Some(match state.pending() {
            Some(Command::IssueDelegation { .. }) => Key::Issuing,
            Some(Command::RevokeDelegation { .. }) => Key::Revoking,
            _ => Key::Loading,
        }),
        Phase::Ready => Some(match state.result() {
            Some(Reply::DelegationIssued(issued)) if issued.token.is_some() => Key::IssuedOk,
            Some(Reply::DelegationIssued(_)) => Key::Replayed,
            Some(Reply::Revoked) => Key::RevokedOk,
            _ => Key::Listed,
        }),
        Phase::Failed => None,
        Phase::Ambiguous => Some(Key::Ambiguous),
    }
}

/// Where keyboard focus returns after an outcome when the request disabled the control that
/// held it: the first refused field, the exact-retry action or the explaining status.
pub fn recovery_focus(command: &Command, outcome: Phase, problems: &[Problem]) -> &'static str {
    if outcome == Phase::Ambiguous {
        return view::RETRY;
    }
    match command {
        Command::IssueDelegation { .. } => match problems.iter().find_map(Problem::field) {
            Some(Field::Delegate) => view::DELEGATE_FIELD,
            Some(Field::Actions) => view::FIRST_ACTION,
            Some(Field::Expiry) => view::LIFETIME_FIELD,
            None => view::STATUS,
        },
        Command::Delegations { .. } if outcome == Phase::Ready => view::LIST_HEADING,
        _ => view::STATUS,
    }
}

/// Format an instant for display. The browser renders local time with `Intl`; native tests
/// render UTC deterministically.
#[cfg(target_arch = "wasm32")]
pub fn display_time(ms: i64, english: bool) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms as f64));
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"dateStyle".into(), &"medium".into());
    let _ = js_sys::Reflect::set(&options, &"timeStyle".into(), &"short".into());
    date.to_locale_string(if english { "en-GB" } else { "vi-VN" }, &options)
        .into()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn display_time(ms: i64, _english: bool) -> String {
    // Civil-from-days (Howard Hinnant), proleptic Gregorian, UTC.
    let seconds = ms.div_euclid(1000);
    let (days, time) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{day:02}/{month:02}/{year} {:02}:{:02} UTC",
        time / 3600,
        time % 3600 / 60
    )
}
