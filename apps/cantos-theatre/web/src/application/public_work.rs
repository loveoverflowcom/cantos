//! Live public Work detail and its unit walk, as a pure framework-free state machine.
//!
//! The browser shell issues exactly the requests returned here and hands each outcome back with
//! its ticket. A ticket from an older route, retry or page never lands. The walk follows the
//! Discovery v1 rules accepted in ADR 0035: an empty page with a cursor is not the end, a failed
//! page is retried with the same cursor, `invalid_cursor` restarts from page one de-duplicating by
//! `unit_id`, and `position` is display order only. The Work's public `unit_count` and each row's
//! `ordinal` come from the presentation DTOs (`presentation=1`); the loaded rows never supply a
//! total, and the rank is neither an identity nor an author-assigned number.
use crate::contract::discovery::{
    CoveredWorkSummary, PresentedUnitPage, PresentedUnitSummary, UnitKind, WorkKind,
};
use crate::core::public_ids::{canonical_uuid, PublicWorkId};
use std::collections::BTreeSet;

/// Requested page size; the contract default and the decoder's per-page bound.
pub const UNIT_PAGE_LIMIT: usize = 20;
/// Automatic follow-ups (empty intermediate pages, an invalid-cursor restart) per listener intent.
/// The budget prevents an unbounded request loop when the server keeps answering empty pages.
const AUTOMATIC_FOLLOW_UPS: u8 = 4;
/// An opaque `lc1_` cursor is 68 ASCII bytes today; anything far beyond that is not a cursor.
const MAX_CURSOR_BYTES: usize = 512;

/// Classified result of one anonymous catalog read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogReadError {
    Diagnostic(crate::application::errors::ErrorKind),
    /// The contract's merged anonymous answer: absent, private, withdrawn or draft-only.
    NotFound,
    InvalidCursor,
    /// Transport, timeout, 503 or an uncoded server answer; retryable.
    Unavailable,
    /// The response does not satisfy the public contract.
    Protocol,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadFailure {
    Diagnostic(crate::application::errors::ErrorKind),
    Unavailable,
    Protocol,
}

impl From<CatalogReadError> for ReadFailure {
    fn from(error: CatalogReadError) -> Self {
        match error {
            CatalogReadError::Diagnostic(kind) => Self::Diagnostic(kind),
            CatalogReadError::Unavailable => Self::Unavailable,
            CatalogReadError::NotFound
            | CatalogReadError::InvalidCursor
            | CatalogReadError::Protocol => Self::Protocol,
        }
    }
}

/// The current catalog matrix: prose works have chapters, podcasts have episodes.
pub const fn unit_kind_for(kind: WorkKind) -> UnitKind {
    match kind {
        WorkKind::Novel | WorkKind::Story => UnitKind::Chapter,
        WorkKind::Podcast => UnitKind::Episode,
    }
}

/// The detail must be the Work named by the route, with renderable public text.
pub fn validate_work(work: &CoveredWorkSummary, expected: PublicWorkId) -> bool {
    PublicWorkId::parse(&work.work_id) == Some(expected)
        && !work.title.trim().is_empty()
        && !work.language.trim().is_empty()
        && crate::application::public_cover::work_is_admitted(work, expected)
}

/// Page-local checks shared by the HTTP decoder and the state machine.
pub fn validate_page(page: &PresentedUnitPage, work: PublicWorkId, kind: UnitKind) -> bool {
    let mut ids = BTreeSet::new();
    PublicWorkId::parse(&page.work_id) == Some(work)
        && page.items.len() <= UNIT_PAGE_LIMIT
        && page.next_cursor.as_deref().is_none_or(|cursor| {
            !cursor.is_empty()
                && cursor.len() <= MAX_CURSOR_BYTES
                && cursor.bytes().all(|byte| byte.is_ascii_graphic())
        })
        && page.items.iter().all(|unit| {
            canonical_uuid(&unit.unit_id).is_some()
                && canonical_uuid(&unit.publication_id).is_some()
                && ids.insert(unit.unit_id.as_str())
                && unit.kind == kind
                && !unit.title.trim().is_empty()
        })
}

/// Issued only by [`WorkDetailState`]; it names the route generation it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkTicket {
    generation: u64,
    work: PublicWorkId,
}

impl WorkTicket {
    pub fn work(&self) -> PublicWorkId {
        self.work
    }
}

/// Issued only by [`WorkDetailState`]; `cursor` is `None` for page one of a walk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageTicket {
    generation: u64,
    request: u64,
    work: PublicWorkId,
    kind: UnitKind,
    cursor: Option<String>,
}

impl PageTicket {
    pub fn work(&self) -> PublicWorkId {
        self.work
    }
    pub fn kind(&self) -> UnitKind {
        self.kind
    }
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Work(WorkTicket),
    Page(PageTicket),
}

/// What the shell does after handing back an outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Stale or duplicate outcome; nothing changed.
    Ignored,
    /// Applied; no request follows.
    Settled,
    /// Applied; issue this page request next.
    Continue(PageTicket),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PageStatus {
    #[default]
    Idle,
    Loading,
    Failed(ReadFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListNotice {
    /// The cursor expired or was refused; the walk restarted from page one, keeping rows.
    Restarted,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitList {
    rows: Vec<PresentedUnitSummary>,
    /// Units answered by the current walk; a repeat within one walk is a protocol failure.
    walk_seen: BTreeSet<String>,
    walk_started: bool,
    cursor: Option<String>,
    /// Insertion point that keeps server order when a restarted walk merges into kept rows.
    merge_at: usize,
    status: PageStatus,
    notice: Option<ListNotice>,
    /// The current walk restarted after `invalid_cursor`. Unlike the listener-facing `notice`,
    /// it survives later listener intents until the walk's terminal page derives `unavailable`.
    restarted_walk: bool,
    first_new: Option<String>,
    /// Rows a completed restarted walk no longer listed (ADR 0035 §2a). No reason is known.
    unavailable: BTreeSet<String>,
}

impl UnitList {
    pub fn rows(&self) -> &[PresentedUnitSummary] {
        &self.rows
    }
    pub fn status(&self) -> &PageStatus {
        &self.status
    }
    pub fn notice(&self) -> Option<ListNotice> {
        self.notice
    }
    /// Page one of the current walk has answered.
    pub fn walk_started(&self) -> bool {
        self.walk_started
    }
    pub fn has_more(&self) -> bool {
        self.walk_started && self.cursor.is_some()
    }
    /// Only a terminal page completes the list; before that no count may be derived.
    pub fn is_complete(&self) -> bool {
        self.walk_started && self.cursor.is_none() && self.status == PageStatus::Idle
    }
    pub fn is_empty_terminal(&self) -> bool {
        self.is_complete() && self.rows.is_empty()
    }
    /// The first row added since the last listener intent, for focus placement.
    pub fn first_new_row(&self) -> Option<&str> {
        self.first_new.as_deref()
    }
    pub fn is_unavailable(&self, unit_id: &str) -> bool {
        self.unavailable.contains(unit_id)
    }

    fn apply(&mut self, page: PresentedUnitPage) {
        for unit in page.items {
            self.walk_seen.insert(unit.unit_id.clone());
            self.unavailable.remove(&unit.unit_id);
            match self.rows.iter().position(|row| row.unit_id == unit.unit_id) {
                // A changed `publication_id` is a new release of the same unit, never a new row.
                Some(index) => {
                    self.rows[index] = unit;
                    self.merge_at = self.merge_at.max(index + 1);
                }
                None => {
                    if self.first_new.is_none() {
                        self.first_new = Some(unit.unit_id.clone());
                    }
                    let at = self.merge_at.min(self.rows.len());
                    self.rows.insert(at, unit);
                    self.merge_at = at + 1;
                }
            }
        }
        self.walk_started = true;
        self.cursor = page.next_cursor;
        self.status = PageStatus::Idle;
        if self.cursor.is_none() && self.restarted_walk {
            let seen = &self.walk_seen;
            self.unavailable = self
                .rows
                .iter()
                .filter(|row| !seen.contains(&row.unit_id))
                .map(|row| row.unit_id.clone())
                .collect();
            self.restarted_walk = false;
        }
    }

    fn restart(&mut self) {
        self.walk_seen.clear();
        self.walk_started = false;
        self.cursor = None;
        self.merge_at = 0;
        self.notice = Some(ListNotice::Restarted);
        self.restarted_walk = true;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum WorkDetailPhase {
    #[default]
    Idle,
    Loading,
    /// One merged public answer; the client never guesses why.
    NotFound,
    Failed(ReadFailure),
    Ready {
        work: Box<CoveredWorkSummary>,
        units: Box<UnitList>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkDetailState {
    generation: u64,
    request: u64,
    work: Option<PublicWorkId>,
    pending_page: Option<u64>,
    follow_ups: u8,
    phase: WorkDetailPhase,
}

impl WorkDetailState {
    pub fn phase(&self) -> &WorkDetailPhase {
        &self.phase
    }
    pub fn work_id(&self) -> Option<PublicWorkId> {
        self.work
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Route entry. Re-entering the route already shown is inert; another Work fences the old one.
    pub fn open(&mut self, work: PublicWorkId) -> Option<WorkTicket> {
        if self.work == Some(work) && self.phase != WorkDetailPhase::Idle {
            return None;
        }
        self.begin_work(work)
    }

    /// Route exit. Every outstanding ticket becomes stale.
    pub fn leave(&mut self) -> bool {
        if self.work.is_none() && self.phase == WorkDetailPhase::Idle {
            return false;
        }
        let Some(generation) = self.generation.checked_add(1) else {
            return false;
        };
        self.generation = generation;
        self.work = None;
        self.pending_page = None;
        self.phase = WorkDetailPhase::Idle;
        true
    }

    /// Retry the failed read only: the whole detail, or the failed page with its same cursor.
    pub fn retry(&mut self) -> Option<Request> {
        match &self.phase {
            WorkDetailPhase::Failed(_) => self.begin_work(self.work?).map(Request::Work),
            WorkDetailPhase::Ready { units, .. }
                if matches!(units.status, PageStatus::Failed(_)) =>
            {
                self.request_page().map(Request::Page)
            }
            _ => None,
        }
    }

    /// Continue the walk, or retry a failed page with the same cursor. Inert while loading.
    pub fn load_more(&mut self) -> Option<PageTicket> {
        match &self.phase {
            WorkDetailPhase::Ready { units, .. }
                if matches!(units.status, PageStatus::Failed(_))
                    || (units.status == PageStatus::Idle && units.has_more()) =>
            {
                self.request_page()
            }
            _ => None,
        }
    }

    pub fn finish_work(
        &mut self,
        ticket: &WorkTicket,
        result: Result<CoveredWorkSummary, CatalogReadError>,
    ) -> Step {
        if ticket.generation != self.generation
            || self.work != Some(ticket.work)
            || self.phase != WorkDetailPhase::Loading
        {
            return Step::Ignored;
        }
        self.phase = match result {
            Ok(work) if validate_work(&work, ticket.work) => {
                self.phase = WorkDetailPhase::Ready {
                    work: Box::new(work),
                    units: Box::default(),
                };
                return self.request_page().map_or(Step::Settled, Step::Continue);
            }
            Ok(_) => WorkDetailPhase::Failed(ReadFailure::Protocol),
            Err(CatalogReadError::NotFound) => WorkDetailPhase::NotFound,
            Err(error) => WorkDetailPhase::Failed(error.into()),
        };
        Step::Settled
    }

    pub fn finish_page(
        &mut self,
        ticket: &PageTicket,
        result: Result<PresentedUnitPage, CatalogReadError>,
    ) -> Step {
        if ticket.generation != self.generation
            || self.work != Some(ticket.work)
            || self.pending_page != Some(ticket.request)
        {
            return Step::Ignored;
        }
        let WorkDetailPhase::Ready { units, .. } = &mut self.phase else {
            return Step::Ignored;
        };
        if units.status != PageStatus::Loading {
            return Step::Ignored;
        }
        self.pending_page = None;
        let follow = match result {
            Ok(page)
                if validate_page(&page, ticket.work, ticket.kind)
                    && page
                        .items
                        .iter()
                        .all(|unit| !units.walk_seen.contains(&unit.unit_id)) =>
            {
                let empty = page.items.is_empty();
                units.apply(page);
                empty && units.cursor.is_some()
            }
            Ok(_) => {
                units.status = PageStatus::Failed(ReadFailure::Protocol);
                false
            }
            Err(CatalogReadError::InvalidCursor) if ticket.cursor.is_some() => {
                units.restart();
                // Without budget, the listener restarts explicitly from page one.
                units.status = PageStatus::Failed(ReadFailure::Unavailable);
                true
            }
            Err(CatalogReadError::NotFound) => {
                // The Work is no longer public; keeping its rows would imply otherwise.
                self.phase = WorkDetailPhase::NotFound;
                return Step::Settled;
            }
            Err(error) => {
                units.status = PageStatus::Failed(error.into());
                false
            }
        };
        if follow && self.follow_ups > 0 {
            self.follow_ups -= 1;
            return self.issue_page().map_or(Step::Settled, Step::Continue);
        }
        Step::Settled
    }

    fn begin_work(&mut self, work: PublicWorkId) -> Option<WorkTicket> {
        let generation = self.generation.checked_add(1)?;
        self.generation = generation;
        self.work = Some(work);
        self.pending_page = None;
        self.phase = WorkDetailPhase::Loading;
        Some(WorkTicket { generation, work })
    }

    /// A listener intent: refills the automatic follow-up budget and resets focus placement.
    fn request_page(&mut self) -> Option<PageTicket> {
        self.follow_ups = AUTOMATIC_FOLLOW_UPS;
        if let WorkDetailPhase::Ready { units, .. } = &mut self.phase {
            units.first_new = None;
            units.notice = None;
        }
        self.issue_page()
    }

    fn issue_page(&mut self) -> Option<PageTicket> {
        let work = self.work?;
        let request = self.request.checked_add(1)?;
        let WorkDetailPhase::Ready {
            work: summary,
            units,
        } = &mut self.phase
        else {
            return None;
        };
        self.request = request;
        self.pending_page = Some(request);
        units.status = PageStatus::Loading;
        Some(PageTicket {
            generation: self.generation,
            request,
            work,
            kind: unit_kind_for(summary.kind),
            cursor: if units.walk_started {
                units.cursor.clone()
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: &str = "10000000-0000-4000-8000-000000000001";
    const OTHER: &str = "10000000-0000-4000-8000-000000000002";

    /// The shared v1-shaped continuation fixtures carry no rows with an `ordinal`; rank them in
    /// list order, exactly as the presentation capability does.
    fn fixture(json: &str) -> PresentedUnitPage {
        let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
        for (index, item) in value["items"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            item["ordinal"] = serde_json::json!(index + 1);
        }
        serde_json::from_value(value).unwrap()
    }
    fn units_fixture() -> PresentedUnitPage {
        fixture(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/presented_units.json"
        ))
    }
    fn work_id() -> PublicWorkId {
        PublicWorkId::parse(WORK).unwrap()
    }
    fn work_summary() -> CoveredWorkSummary {
        let page: crate::contract::discovery::CoveredWorkPage =
            serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/cover/covered_works.json"
        ))
            .unwrap();
        page.items.into_iter().next().unwrap()
    }
    fn unit(n: u128, position: u32) -> PresentedUnitSummary {
        PresentedUnitSummary {
            unit_id: uuid::Uuid::from_u128(0x2000_0000_0000_4000_8000_0000_0000_0000 + n)
                .to_string(),
            kind: UnitKind::Chapter,
            position,
            title: format!("Chương thử {n}"),
            description: String::new(),
            publication_id: uuid::Uuid::from_u128(0x8000_0000_0000_4000_8000_0000_0000_0000 + n)
                .to_string(),
            published_at_ms: 5_000,
            duration_ms: 61_000,
            ordinal: u64::from(position) + 1,
        }
    }
    fn page(items: Vec<PresentedUnitSummary>, next: Option<&str>) -> PresentedUnitPage {
        PresentedUnitPage {
            work_id: WORK.into(),
            items,
            next_cursor: next.map(str::to_owned),
        }
    }
    fn ids(state: &WorkDetailState) -> Vec<String> {
        units(state)
            .rows()
            .iter()
            .map(|row| row.unit_id.clone())
            .collect()
    }
    fn units(state: &WorkDetailState) -> &UnitList {
        match state.phase() {
            WorkDetailPhase::Ready { units, .. } => units,
            other => panic!("not ready: {other:?}"),
        }
    }
    /// Opens the fixture Work and returns the page-one ticket.
    fn ready() -> (WorkDetailState, PageTicket) {
        let mut state = WorkDetailState::default();
        let ticket = state.open(work_id()).unwrap();
        let Step::Continue(first) = state.finish_work(&ticket, Ok(work_summary())) else {
            panic!("a ready work must request its first page")
        };
        assert_eq!(first.cursor(), None);
        assert_eq!(first.kind(), UnitKind::Chapter);
        (state, first)
    }

    #[test]
    fn shared_fixture_walk_is_ready_complete_and_never_counts_a_partial_list() {
        let (mut state, first) = ready();
        assert!(!units(&state).is_complete());
        assert_eq!(
            state.finish_page(&first, Ok(units_fixture())),
            Step::Settled
        );
        let list = units(&state);
        assert!(list.is_complete() && !list.has_more());
        assert_eq!(list.rows()[0].title, "Một tiếng chuông trong sương");
        assert_eq!(state.load_more(), None);
    }

    #[test]
    fn not_found_unavailable_and_protocol_are_distinct_and_only_failures_retry() {
        for (result, expected) in [
            (Err(CatalogReadError::NotFound), WorkDetailPhase::NotFound),
            (
                Err(CatalogReadError::Unavailable),
                WorkDetailPhase::Failed(ReadFailure::Unavailable),
            ),
            (
                Err(CatalogReadError::Protocol),
                WorkDetailPhase::Failed(ReadFailure::Protocol),
            ),
            (
                Err(CatalogReadError::InvalidCursor),
                WorkDetailPhase::Failed(ReadFailure::Protocol),
            ),
        ] {
            let mut state = WorkDetailState::default();
            let ticket = state.open(work_id()).unwrap();
            assert_eq!(state.finish_work(&ticket, result), Step::Settled);
            assert_eq!(state.phase(), &expected);
            let retried = state.retry();
            assert_eq!(
                retried.is_some(),
                matches!(expected, WorkDetailPhase::Failed(_))
            );
        }
    }

    #[test]
    fn a_different_work_title_less_or_wrong_identity_detail_is_a_protocol_failure() {
        let mut wrong = work_summary();
        wrong.work_id = OTHER.into();
        let mut blank = work_summary();
        blank.title = "  ".into();
        for bad in [wrong, blank] {
            let mut state = WorkDetailState::default();
            let ticket = state.open(work_id()).unwrap();
            assert_eq!(state.finish_work(&ticket, Ok(bad)), Step::Settled);
            assert_eq!(
                state.phase(),
                &WorkDetailPhase::Failed(ReadFailure::Protocol)
            );
        }
    }

    #[test]
    fn stale_route_and_stale_page_responses_never_land() {
        let mut state = WorkDetailState::default();
        let old = state.open(work_id()).unwrap();
        assert_eq!(
            state.open(work_id()),
            None,
            "re-entering the same route is inert"
        );
        let other = PublicWorkId::parse(OTHER).unwrap();
        let current = state.open(other).unwrap();
        assert_eq!(state.finish_work(&old, Ok(work_summary())), Step::Ignored);
        assert_eq!(state.phase(), &WorkDetailPhase::Loading);
        assert!(state.leave());
        assert_eq!(
            state.finish_work(&current, Err(CatalogReadError::Unavailable)),
            Step::Ignored
        );
        assert_eq!(state.phase(), &WorkDetailPhase::Idle);
        assert!(!state.leave(), "leaving twice is inert");

        let (mut state, first) = ready();
        assert!(state.leave());
        assert_eq!(
            state.finish_page(&first, Ok(units_fixture())),
            Step::Ignored
        );
        let (mut state, first) = ready();
        assert_eq!(
            state.finish_page(&first, Ok(units_fixture())),
            Step::Settled
        );
        assert_eq!(
            state.finish_page(&first, Ok(page(vec![unit(9, 9)], None))),
            Step::Ignored,
            "a duplicate completion is ignored"
        );
        assert_eq!(units(&state).rows().len(), 1);
    }

    #[test]
    fn terminal_empty_list_is_distinct_from_failure_and_from_an_intermediate_empty_page() {
        let (mut state, first) = ready();
        let terminal: PresentedUnitPage = fixture(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/units_end_empty.json"
        ));
        assert_eq!(state.finish_page(&first, Ok(terminal)), Step::Settled);
        assert!(units(&state).is_empty_terminal());

        let (mut state, first) = ready();
        let continuation: PresentedUnitPage = fixture(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/units_empty_continuation.json"
        ));
        let cursor = continuation.next_cursor.clone();
        let Step::Continue(next) = state.finish_page(&first, Ok(continuation)) else {
            panic!("an empty page with a cursor is not the end")
        };
        assert_eq!(next.cursor(), cursor.as_deref());
        assert!(!units(&state).is_empty_terminal());
        assert_eq!(units(&state).status(), &PageStatus::Loading);
        assert_eq!(state.finish_page(&next, Ok(units_fixture())), Step::Settled);
        assert_eq!(units(&state).rows().len(), 1);
        assert!(units(&state).is_complete());
    }

    #[test]
    fn repeated_empty_pages_stop_at_the_budget_and_offer_load_more() {
        let (mut state, mut ticket) = ready();
        let mut automatic = 0;
        while let Step::Continue(next) =
            state.finish_page(&ticket, Ok(page(vec![], Some("lc1_more"))))
        {
            automatic += 1;
            ticket = next;
        }
        assert_eq!(automatic, AUTOMATIC_FOLLOW_UPS);
        let list = units(&state);
        assert!(list.rows().is_empty() && list.has_more() && !list.is_empty_terminal());
        let resumed = state.load_more().unwrap();
        assert_eq!(resumed.cursor(), Some("lc1_more"));
    }

    #[test]
    fn next_page_failure_keeps_rows_and_retries_the_same_cursor() {
        let (mut state, first) = ready();
        state.finish_page(
            &first,
            Ok(page(vec![unit(1, 0), unit(2, 1)], Some("lc1_p2"))),
        );
        let second = state.load_more().unwrap();
        assert_eq!(
            state.load_more(),
            None,
            "a loading page is not requested twice"
        );
        assert_eq!(
            state.finish_page(&second, Err(CatalogReadError::Unavailable)),
            Step::Settled
        );
        let list = units(&state);
        assert_eq!(list.rows().len(), 2);
        assert_eq!(list.status(), &PageStatus::Failed(ReadFailure::Unavailable));
        let Some(Request::Page(retry)) = state.retry() else {
            panic!("a failed page retries as a page")
        };
        assert_eq!(retry.cursor(), Some("lc1_p2"));
        assert_ne!(
            retry, second,
            "the retry is a new request with the same cursor"
        );
        assert_eq!(
            state.finish_page(&second, Ok(page(vec![unit(3, 2)], None))),
            Step::Ignored
        );
        assert_eq!(
            state.finish_page(&retry, Ok(page(vec![unit(3, 2)], None))),
            Step::Settled
        );
        assert_eq!(ids(&state).len(), 3);
        assert_eq!(
            units(&state).first_new_row(),
            Some(unit(3, 2).unit_id.as_str())
        );
    }

    #[test]
    fn invalid_cursor_restarts_from_page_one_and_deduplicates_by_unit_id() {
        let (mut state, first) = ready();
        state.finish_page(
            &first,
            Ok(page(vec![unit(1, 0), unit(2, 1)], Some("lc1_p2"))),
        );
        let second = state.load_more().unwrap();
        let Step::Continue(restart) =
            state.finish_page(&second, Err(CatalogReadError::InvalidCursor))
        else {
            panic!("invalid_cursor restarts from page one")
        };
        assert_eq!(restart.cursor(), None);
        assert_eq!(units(&state).notice(), Some(ListNotice::Restarted));
        assert_eq!(
            units(&state).rows().len(),
            2,
            "rows stay visible during restart"
        );

        let mut republished = unit(2, 1);
        republished.publication_id = uuid::Uuid::from_u128(99).to_string();
        let Step::Settled = state.finish_page(
            &restart,
            Ok(page(
                vec![unit(1, 0), republished.clone()],
                Some("lc1_fresh"),
            )),
        ) else {
            panic!("a full page settles")
        };
        assert_eq!(ids(&state), vec![unit(1, 0).unit_id, unit(2, 1).unit_id]);
        assert_eq!(
            units(&state).rows()[1].publication_id,
            republished.publication_id
        );
        let third = state.load_more().unwrap();
        assert_eq!(third.cursor(), Some("lc1_fresh"));
        state.finish_page(&third, Ok(page(vec![unit(3, 2)], None)));
        assert_eq!(ids(&state).len(), 3);
        assert!(units(&state).is_complete());
    }

    #[test]
    fn restarted_walk_merges_in_server_order_and_marks_unlisted_rows_unavailable() {
        let (mut state, first) = ready();
        state.finish_page(
            &first,
            Ok(page(vec![unit(1, 0), unit(3, 2)], Some("lc1_p2"))),
        );
        let second = state.load_more().unwrap();
        let Step::Continue(restart) =
            state.finish_page(&second, Err(CatalogReadError::InvalidCursor))
        else {
            panic!("restart")
        };
        // Unit 3 is withdrawn; unit 2 is newly visible between 1 and 4.
        state.finish_page(
            &restart,
            Ok(page(vec![unit(1, 0), unit(2, 1), unit(4, 3)], None)),
        );
        assert_eq!(
            ids(&state),
            vec![
                unit(1, 0).unit_id,
                unit(2, 1).unit_id,
                unit(4, 3).unit_id,
                unit(3, 2).unit_id
            ]
        );
        let list = units(&state);
        assert!(list.is_unavailable(&unit(3, 2).unit_id));
        assert!(!list.is_unavailable(&unit(1, 0).unit_id));
    }

    #[test]
    fn a_multi_page_restarted_walk_marks_unlisted_rows_after_load_more_or_retry() {
        for retry_first_page in [false, true] {
            let (mut state, first) = ready();
            state.finish_page(
                &first,
                Ok(page(vec![unit(1, 0), unit(2, 1)], Some("lc1_p2"))),
            );
            let second = state.load_more().unwrap();
            let Step::Continue(mut restart) =
                state.finish_page(&second, Err(CatalogReadError::InvalidCursor))
            else {
                panic!("restart")
            };
            if retry_first_page {
                state.finish_page(&restart, Err(CatalogReadError::Unavailable));
                let Some(Request::Page(again)) = state.retry() else {
                    panic!("a failed restart page retries")
                };
                assert_eq!(again.cursor(), None);
                restart = again;
            }
            // Unit 2 was withdrawn; the restarted walk needs a second listener-requested page.
            state.finish_page(&restart, Ok(page(vec![unit(1, 0)], Some("lc1_fresh"))));
            let more = state.load_more().unwrap();
            assert_eq!(more.cursor(), Some("lc1_fresh"));
            state.finish_page(&more, Ok(page(vec![unit(3, 2)], None)));
            let list = units(&state);
            assert!(list.is_complete(), "retry={retry_first_page}");
            assert!(
                list.is_unavailable(&unit(2, 1).unit_id),
                "retry={retry_first_page}"
            );
            assert!(!list.is_unavailable(&unit(1, 0).unit_id));
            assert!(!list.is_unavailable(&unit(3, 2).unit_id));
        }
    }

    #[test]
    fn republished_fixture_after_restart_replaces_the_version_without_a_second_row() {
        let (mut state, first) = ready();
        let original = units_fixture();
        state.finish_page(&first, Ok(page(original.items.clone(), Some("lc1_p2"))));
        let second = state.load_more().unwrap();
        let Step::Continue(restart) =
            state.finish_page(&second, Err(CatalogReadError::InvalidCursor))
        else {
            panic!("restart")
        };
        let republished: PresentedUnitPage = fixture(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/units_republished.json"
        ));
        assert_eq!(
            state.finish_page(&restart, Ok(republished.clone())),
            Step::Settled
        );
        let list = units(&state);
        assert_eq!(list.rows().len(), 1);
        assert_eq!(list.rows()[0].unit_id, original.items[0].unit_id);
        assert_eq!(list.rows()[0].position, original.items[0].position);
        assert_ne!(
            list.rows()[0].publication_id,
            original.items[0].publication_id
        );
        assert_eq!(list.rows()[0], republished.items[0]);
        assert!(list.is_complete() && !list.is_unavailable(&original.items[0].unit_id));
    }

    #[test]
    fn duplicate_within_a_walk_wrong_kind_or_identity_is_a_protocol_failure_keeping_rows() {
        let (mut state, first) = ready();
        state.finish_page(&first, Ok(page(vec![unit(1, 0)], Some("lc1_p2"))));
        let mut episode = unit(2, 1);
        episode.kind = UnitKind::Episode;
        let mut untitled = unit(3, 1);
        untitled.title = " ".into();
        let mut bad_id = unit(0xab, 1);
        bad_id.unit_id = bad_id.unit_id.to_uppercase();
        let mut other_work = page(vec![unit(5, 1)], None);
        other_work.work_id = OTHER.into();
        for bad in [
            page(vec![unit(1, 0)], None),
            page(vec![episode], None),
            page(vec![untitled], None),
            page(vec![bad_id], None),
            page(vec![unit(6, 1), unit(6, 1)], None),
            page((10..31).map(|n| unit(n, n as u32)).collect(), None),
            page(vec![unit(7, 1)], Some("")),
            other_work,
        ] {
            let ticket = state.load_more().unwrap();
            assert_eq!(state.finish_page(&ticket, Ok(bad)), Step::Settled);
            assert_eq!(
                units(&state).status(),
                &PageStatus::Failed(ReadFailure::Protocol)
            );
            assert_eq!(ids(&state), vec![unit(1, 0).unit_id]);
        }
    }

    #[test]
    fn page_not_found_withdraws_the_whole_detail_and_first_page_invalid_cursor_is_protocol() {
        let (mut state, first) = ready();
        assert_eq!(
            state.finish_page(&first, Err(CatalogReadError::InvalidCursor)),
            Step::Settled
        );
        assert_eq!(
            units(&state).status(),
            &PageStatus::Failed(ReadFailure::Protocol)
        );
        let Some(Request::Page(again)) = state.retry() else {
            panic!("first page retries as a page")
        };
        assert_eq!(again.cursor(), None);
        assert_eq!(
            state.finish_page(&again, Err(CatalogReadError::NotFound)),
            Step::Settled
        );
        assert_eq!(state.phase(), &WorkDetailPhase::NotFound);
    }

    #[test]
    fn podcast_requests_episodes_and_generation_exhaustion_preserves_state() {
        assert_eq!(unit_kind_for(WorkKind::Podcast), UnitKind::Episode);
        assert_eq!(unit_kind_for(WorkKind::Novel), UnitKind::Chapter);
        let mut state = WorkDetailState {
            generation: u64::MAX,
            ..WorkDetailState::default()
        };
        let previous = state.clone();
        assert_eq!(state.open(work_id()), None);
        assert_eq!(state, previous);
    }
}
