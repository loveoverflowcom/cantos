//! Anonymous discovery walk: one request owner for the Home/Explore Work list. Identity lookup
//! never gates public content.
//!
//! A *walk* is one search over the public catalog: its normalized [`DiscoveryQuery`], the rows
//! answered so far and the opaque cursor that continues it. The shell issues exactly the
//! requests returned here and hands each outcome back with its ticket, so an answer that
//! belongs to another query, retry or page never lands. The walk follows Discovery v1: the
//! cursor is opaque and bound to its query, an empty page with a cursor is not the end, a
//! failed page is retried with the same cursor, and `invalid_cursor` needs a fresh walk that is
//! never merged into the old snapshot. The list ends only with `next_cursor: null`.
use crate::contract::discovery::{MetadataWorkPage, MetadataWorkSummary, WorkKind};
use std::collections::BTreeSet;

/// The same bound drives the anonymous request, decoder and public page notice.
pub const PUBLIC_WORK_PAGE_LIMIT: usize = 20;
/// Automatic follow-ups (pages that added nothing) per listener intent. The budget prevents an
/// unbounded request loop when the server keeps answering empty pages with a cursor.
const AUTOMATIC_FOLLOW_UPS: u8 = 4;
/// An opaque `lc1_` cursor is 68 ASCII bytes today; anything far beyond that is not a cursor.
const MAX_CURSOR_BYTES: usize = 512;
/// Server bounds for the normalized search text and the audio-language filter.
pub const MAX_SEARCH_BYTES: usize = 512;
pub const MAX_LANGUAGE_BYTES: usize = 63;

/// Why the listener's search text or language filter cannot be sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryError {
    SearchTooLong,
    SearchControlCharacter,
    /// The server compares the actual released language tag: ASCII letters, digits and `-`.
    LanguageSyntax,
}

/// The listener's search. Normalization is the server's: this value only trims and rejects what
/// the server would refuse, so equal spellings share one identity and an invalid search is never
/// sent. `kind` is novel/story/podcast, never a genre.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiscoveryQuery {
    text: String,
    kind: Option<WorkKind>,
    language: Option<String>,
}

impl DiscoveryQuery {
    pub fn new(text: &str, kind: Option<WorkKind>, language: &str) -> Result<Self, QueryError> {
        let text = text.trim();
        if text.len() > MAX_SEARCH_BYTES {
            return Err(QueryError::SearchTooLong);
        }
        if text.chars().any(char::is_control) {
            return Err(QueryError::SearchControlCharacter);
        }
        let language = language.trim();
        let language = if language.is_empty() {
            None
        } else if language.len() > MAX_LANGUAGE_BYTES
            || !language
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(QueryError::LanguageSyntax);
        } else {
            Some(language.to_ascii_lowercase())
        };
        Ok(Self {
            text: text.to_owned(),
            kind,
            language,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn kind(&self) -> Option<WorkKind> {
        self.kind
    }

    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// Nothing narrows the catalog.
    pub fn is_unfiltered(&self) -> bool {
        self.text.is_empty() && self.kind.is_none() && self.language.is_none()
    }

    /// Only the parameters that narrow the catalog, in the contract's names.
    pub fn parameters(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        if !self.text.is_empty() {
            pairs.push(("q", self.text.clone()));
        }
        if let Some(kind) = self.kind {
            pairs.push(("kind", kind_name(kind).to_owned()));
        }
        if let Some(language) = &self.language {
            pairs.push(("language", language.clone()));
        }
        pairs
    }

    /// Rebuild a query from parameters read out of a link. A refused value yields the unfiltered
    /// catalog instead of a half-applied search.
    pub fn from_parameters<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let (mut text, mut kind, mut language) = ("", None, "");
        for (name, value) in pairs {
            match name {
                "q" => text = value,
                "kind" => match kind_from_name(value) {
                    Some(parsed) => kind = Some(parsed),
                    None => return Self::default(),
                },
                "language" => language = value,
                _ => (),
            }
        }
        Self::new(text, kind, language).unwrap_or_default()
    }
}

pub const fn kind_name(kind: WorkKind) -> &'static str {
    match kind {
        WorkKind::Novel => "novel",
        WorkKind::Story => "story",
        WorkKind::Podcast => "podcast",
    }
}

pub fn kind_from_name(name: &str) -> Option<WorkKind> {
    [WorkKind::Novel, WorkKind::Story, WorkKind::Podcast]
        .into_iter()
        .find(|kind| kind_name(*kind) == name)
}

/// Classified result of one anonymous list read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoveryReadError {
    Diagnostic(crate::application::errors::ErrorKind),
    /// Transport, timeout, 503 or an uncoded server answer; retryable.
    Unavailable,
    /// The response does not satisfy the public contract.
    Protocol,
    /// The cursor expired, was refused or belongs to another walk; the walk must start again.
    InvalidCursor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicDiscoveryFailure {
    Diagnostic(crate::application::errors::ErrorKind),
    Unavailable,
    Protocol,
}

/// Issued only by [`PublicDiscoveryState`]; `cursor` is `None` for page one of a walk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryTicket {
    generation: u64,
    query: DiscoveryQuery,
    cursor: Option<String>,
}

impl DiscoveryTicket {
    pub fn query(&self) -> &DiscoveryQuery {
        &self.query
    }

    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }
}

/// What the shell does after handing back an outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Stale or duplicate outcome; nothing changed.
    Ignored,
    /// Applied; no request follows.
    Settled,
    /// Applied; issue this page request next.
    Continue(DiscoveryTicket),
}

/// What the whole region shows. `Ready` and `Incomplete` carry their continuation in [`Tail`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicDiscoveryPhase {
    Idle,
    Loading,
    /// A complete walk that matched nothing.
    Empty,
    /// No row yet, but the server has more pages: this is not "no results".
    Incomplete,
    Ready,
    /// Nothing could be read and no row is kept.
    Failed(PublicDiscoveryFailure),
    /// The cursor ended before any row was kept; only a fresh walk continues.
    Expired,
}

/// The state of the list's continuation below its rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tail {
    /// `next_cursor: null` — the only end of a walk.
    End,
    More,
    Loading,
    Failed(PublicDiscoveryFailure),
    Expired,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Status {
    #[default]
    Idle,
    Loading,
    Settled,
    Failed(PublicDiscoveryFailure),
    CursorExpired,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublicDiscoveryState {
    generation: u64,
    query: DiscoveryQuery,
    /// Query of the displayed snapshot; pending requests never relabel retained rows.
    applied_query: Option<DiscoveryQuery>,
    rows: Vec<MetadataWorkSummary>,
    seen: BTreeSet<String>,
    cursor: Option<String>,
    /// Page one of the current walk has answered.
    walk_started: bool,
    follow_ups: u8,
    status: Status,
    /// The first row added since the last listener intent, for focus placement.
    first_new: Option<String>,
}

fn cursor_is_well_formed(cursor: &str) -> bool {
    !cursor.is_empty()
        && cursor.len() <= MAX_CURSOR_BYTES
        && cursor.bytes().all(|b| b.is_ascii_graphic())
}

impl PublicDiscoveryState {
    /// Leaving the visible route drops this access snapshot without resetting its ticket fence.
    /// Returning must read current counts again; a previous request cannot repopulate it.
    pub fn discard_snapshot(&mut self) {
        *self = Self {
            generation: self.generation.saturating_add(1),
            ..Self::default()
        };
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn query(&self) -> &DiscoveryQuery {
        &self.query
    }

    pub fn applied_query(&self) -> Option<&DiscoveryQuery> {
        self.applied_query.as_ref()
    }

    pub fn replacing(&self) -> bool {
        !self.walk_started && self.status == Status::Loading
    }

    pub fn appending(&self) -> bool {
        self.walk_started && self.status == Status::Loading
    }

    pub fn replacement_failed(&self) -> bool {
        !self.walk_started && matches!(self.status, Status::Failed(_))
    }

    pub fn rows(&self) -> &[MetadataWorkSummary] {
        &self.rows
    }

    /// The first row added since the last listener intent, so focus can follow the intent.
    pub fn first_new_row(&self) -> Option<&str> {
        self.first_new.as_deref()
    }

    pub fn phase(&self) -> PublicDiscoveryPhase {
        let has_more = self.has_more();
        match self.status {
            Status::Idle => PublicDiscoveryPhase::Idle,
            _ if !self.rows.is_empty() => PublicDiscoveryPhase::Ready,
            Status::Loading => PublicDiscoveryPhase::Loading,
            Status::Failed(failure) => PublicDiscoveryPhase::Failed(failure),
            Status::CursorExpired => PublicDiscoveryPhase::Expired,
            Status::Settled if has_more => PublicDiscoveryPhase::Incomplete,
            Status::Settled => PublicDiscoveryPhase::Empty,
        }
    }

    pub fn tail(&self) -> Tail {
        match self.status {
            Status::Loading => Tail::Loading,
            Status::Failed(failure) => Tail::Failed(failure),
            Status::CursorExpired => Tail::Expired,
            Status::Idle | Status::Settled if self.has_more() => Tail::More,
            Status::Idle | Status::Settled => Tail::End,
        }
    }

    /// The server still holds a continuation of the current walk.
    pub fn has_more(&self) -> bool {
        self.walk_started && self.cursor.is_some()
    }

    /// Route entry. A state that already answers (or is answering) this query is kept, so
    /// returning from a Work keeps its rows and cursor; another query starts a fresh walk.
    pub fn open(&mut self, query: DiscoveryQuery) -> Option<DiscoveryTicket> {
        if self.status != Status::Idle && self.query == query {
            return None;
        }
        self.begin(query)
    }

    /// A listener intent: a fresh walk of `query`. Cursor and outstanding tickets are reset;
    /// the applied snapshot remains readable until a valid replacement page arrives. Repeating the search that is
    /// still answering its first page is inert.
    pub fn begin(&mut self, query: DiscoveryQuery) -> Option<DiscoveryTicket> {
        if self.status == Status::Loading && !self.walk_started && self.query == query {
            return None;
        }
        let generation = self.generation.checked_add(1)?;
        self.generation = generation;
        self.query = query.clone();
        self.cursor = None;
        self.walk_started = false;
        self.follow_ups = AUTOMATIC_FOLLOW_UPS;
        self.first_new = None;
        self.status = Status::Loading;
        Some(DiscoveryTicket {
            generation,
            query,
            cursor: None,
        })
    }

    /// Retry the failed read only: the failed page with its same cursor, or page one when the
    /// walk never started. An expired cursor needs [`Self::begin`] instead.
    pub fn retry(&mut self) -> Option<DiscoveryTicket> {
        match self.status {
            Status::Failed(_) if self.walk_started => self.issue_page(true),
            Status::Failed(_) => self.begin(self.query.clone()),
            _ => None,
        }
    }

    /// Continue the walk. Inert while a request is in flight or when there is nothing to read.
    pub fn load_more(&mut self) -> Option<DiscoveryTicket> {
        match self.status {
            Status::Settled if self.has_more() => self.issue_page(true),
            Status::Failed(_) if self.walk_started => self.issue_page(true),
            _ => None,
        }
    }

    /// Unmount/cancel invalidates the old callback without resetting another surface's state.
    pub fn cancel(&mut self) -> bool {
        let Some(generation) = self.generation.checked_add(1) else {
            return false;
        };
        *self = Self {
            generation,
            ..Self::default()
        };
        true
    }

    /// Private cache invalidation purges facts even when no new ticket can be issued.
    pub fn clear_shared(&mut self) {
        let generation = self.generation.saturating_add(1);
        *self = Self {
            generation,
            ..Self::default()
        };
    }

    pub fn finish(
        &mut self,
        ticket: &DiscoveryTicket,
        result: Result<MetadataWorkPage, DiscoveryReadError>,
    ) -> Step {
        if ticket.generation != self.generation || self.status != Status::Loading {
            return Step::Ignored;
        }
        let page = match result {
            Ok(page) => page,
            Err(DiscoveryReadError::InvalidCursor) if ticket.cursor.is_some() => {
                // A rejected access/generation fence ends permission to retain this snapshot.
                self.rows.clear();
                self.seen.clear();
                self.first_new = None;
                self.applied_query = None;
                self.cursor = None;
                self.status = Status::CursorExpired;
                return Step::Settled;
            }
            // Page one carries no cursor, so it cannot be an invalid cursor.
            Err(DiscoveryReadError::InvalidCursor | DiscoveryReadError::Protocol) => {
                self.status = Status::Failed(PublicDiscoveryFailure::Protocol);
                return Step::Settled;
            }
            Err(DiscoveryReadError::Diagnostic(kind)) => {
                if matches!(
                    kind,
                    crate::application::errors::ErrorKind::Unauthenticated
                        | crate::application::errors::ErrorKind::Forbidden
                        | crate::application::errors::ErrorKind::ContentUnavailable
                ) {
                    self.rows.clear();
                    self.seen.clear();
                    self.applied_query = None;
                    self.cursor = None;
                    self.walk_started = false;
                    self.first_new = None;
                }
                self.status = Status::Failed(PublicDiscoveryFailure::Diagnostic(kind));
                return Step::Settled;
            }
            Err(DiscoveryReadError::Unavailable) => {
                self.status = Status::Failed(PublicDiscoveryFailure::Unavailable);
                return Step::Settled;
            }
        };
        if page.items.len() > PUBLIC_WORK_PAGE_LIMIT
            || page
                .next_cursor
                .as_deref()
                .is_some_and(|c| !cursor_is_well_formed(c))
        {
            self.status = Status::Failed(PublicDiscoveryFailure::Protocol);
            return Step::Settled;
        }
        if !self.walk_started {
            self.rows.clear();
            self.seen.clear();
            self.applied_query = Some(ticket.query.clone());
        }
        let before = self.rows.len();
        for work in page.items {
            // One identity appears once; a repeat of an earlier row is dropped, never doubled.
            if self.seen.insert(work.work_id.clone()) {
                if self.first_new.is_none() {
                    self.first_new = Some(work.work_id.clone());
                }
                self.rows.push(work);
            }
        }
        let added = self.rows.len() > before;
        self.walk_started = true;
        self.cursor = page.next_cursor;
        self.status = Status::Settled;
        if !added && self.cursor.is_some() && self.follow_ups > 0 {
            self.follow_ups -= 1;
            return self.issue_page(false).map_or(Step::Settled, Step::Continue);
        }
        Step::Settled
    }

    /// `intent` refills the automatic follow-up budget: only a listener may do that.
    fn issue_page(&mut self, intent: bool) -> Option<DiscoveryTicket> {
        let generation = self.generation.checked_add(1)?;
        self.generation = generation;
        if intent {
            self.follow_ups = AUTOMATIC_FOLLOW_UPS;
            self.first_new = None;
        }
        self.status = Status::Loading;
        Some(DiscoveryTicket {
            generation,
            query: self.query.clone(),
            cursor: self.cursor.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(n: u128, title: &str) -> MetadataWorkSummary {
        MetadataWorkSummary {
            work_id: uuid::Uuid::from_u128(0x1000_0000_0000_4000_8000_0000_0000_0000 + n)
                .to_string(),
            kind: WorkKind::Story,
            title: title.into(),
            description: "Nội dung đã được công bố.".into(),
            language: "vi-VN".into(),
            first_visible_at_ms: 1,
            unit_count: 3,
            cover: None,
        }
    }
    fn page(items: Vec<MetadataWorkSummary>, next: Option<&str>) -> MetadataWorkPage {
        MetadataWorkPage {
            items,
            next_cursor: next.map(str::to_owned),
        }
    }
    fn query(text: &str) -> DiscoveryQuery {
        DiscoveryQuery::new(text, None, "").unwrap()
    }
    fn titles(state: &PublicDiscoveryState) -> Vec<&str> {
        state.rows().iter().map(|w| w.title.as_str()).collect()
    }
    fn started(q: &str) -> (PublicDiscoveryState, DiscoveryTicket) {
        let mut state = PublicDiscoveryState::default();
        let ticket = state.begin(query(q)).unwrap();
        (state, ticket)
    }

    #[test]
    fn query_trims_lowercases_the_language_and_refuses_what_the_server_would_refuse() {
        let q = DiscoveryQuery::new("  đèn  ", Some(WorkKind::Podcast), " VI-vn ").unwrap();
        assert_eq!(q.text(), "đèn");
        assert_eq!(q.language(), Some("vi-vn"));
        assert_eq!(
            q.parameters(),
            vec![
                ("q", "đèn".to_owned()),
                ("kind", "podcast".to_owned()),
                ("language", "vi-vn".to_owned())
            ]
        );
        assert!(DiscoveryQuery::new("   ", None, "  ")
            .unwrap()
            .is_unfiltered());
        assert!(DiscoveryQuery::default().parameters().is_empty());
        assert_eq!(
            DiscoveryQuery::new(&"a".repeat(513), None, ""),
            Err(QueryError::SearchTooLong)
        );
        assert!(DiscoveryQuery::new(&"a".repeat(512), None, "").is_ok());
        // 512 bytes, not 512 characters: the server measures bytes.
        assert_eq!(
            DiscoveryQuery::new(&"đ".repeat(257), None, ""),
            Err(QueryError::SearchTooLong)
        );
        assert_eq!(
            DiscoveryQuery::new("a\nb", None, ""),
            Err(QueryError::SearchControlCharacter)
        );
        for bad in ["vi_VN", "vi VN", "vi-VN!", "việt", &"a".repeat(64)] {
            assert_eq!(
                DiscoveryQuery::new("", None, bad),
                Err(QueryError::LanguageSyntax),
                "{bad}"
            );
        }
        assert!(DiscoveryQuery::new("", None, &"a".repeat(63)).is_ok());
    }

    #[test]
    fn link_parameters_round_trip_and_a_refused_value_never_half_applies() {
        let q = DiscoveryQuery::new("lời hẹn", Some(WorkKind::Story), "vi-vn").unwrap();
        let parameters = q.parameters();
        let borrowed = parameters.iter().map(|(n, v)| (*n, v.as_str()));
        assert_eq!(DiscoveryQuery::from_parameters(borrowed), q);
        assert_eq!(
            DiscoveryQuery::from_parameters([("q", "đèn"), ("kind", "genre")]),
            DiscoveryQuery::default()
        );
        assert_eq!(
            DiscoveryQuery::from_parameters([("q", "đèn"), ("language", "vi_VN")]),
            DiscoveryQuery::default()
        );
        assert_eq!(
            DiscoveryQuery::from_parameters([("unknown", "x"), ("q", "a")]).text(),
            "a"
        );
        for kind in [WorkKind::Novel, WorkKind::Story, WorkKind::Podcast] {
            assert_eq!(kind_from_name(kind_name(kind)), Some(kind));
        }
        assert_eq!(kind_from_name("Story"), None);
    }

    #[test]
    fn first_page_rows_and_the_cursor_are_kept_and_only_a_null_cursor_ends_the_list() {
        let (mut state, first) = started("");
        assert_eq!(state.phase(), PublicDiscoveryPhase::Loading);
        assert_eq!(first.cursor(), None);
        assert_eq!(
            state.finish(&first, Ok(page(vec![work(1, "A")], Some("lc1_a")))),
            Step::Settled
        );
        assert_eq!(state.phase(), PublicDiscoveryPhase::Ready);
        assert_eq!(state.tail(), Tail::More);
        let more = state.load_more().unwrap();
        assert_eq!(more.cursor(), Some("lc1_a"));
        assert_eq!(state.tail(), Tail::Loading);
        assert_eq!(state.load_more(), None, "a duplicate request is inert");
        state.finish(&more, Ok(page(vec![work(2, "B")], None)));
        assert_eq!(titles(&state), ["A", "B"]);
        assert_eq!(state.tail(), Tail::End);
        assert_eq!(state.load_more(), None, "nothing follows a null cursor");
    }

    #[test]
    fn focus_follows_the_first_row_a_listener_intent_added() {
        let (mut state, first) = started("");
        assert_eq!(state.first_new_row(), None);
        state.finish(
            &first,
            Ok(page(vec![work(1, "A"), work(2, "B")], Some("c1"))),
        );
        assert_eq!(state.first_new_row(), Some(work(1, "A").work_id.as_str()));
        let more = state.load_more().unwrap();
        assert_eq!(state.first_new_row(), None);
        state.finish(&more, Ok(page(vec![work(3, "C")], None)));
        assert_eq!(state.first_new_row(), Some(work(3, "C").work_id.as_str()));
    }

    #[test]
    fn a_repeated_identity_is_kept_once_even_across_pages() {
        let (mut state, first) = started("");
        state.finish(
            &first,
            Ok(page(vec![work(1, "A"), work(2, "B")], Some("c1"))),
        );
        let more = state.load_more().unwrap();
        state.finish(
            &more,
            Ok(page(vec![work(2, "B again"), work(3, "C")], None)),
        );
        assert_eq!(titles(&state), ["A", "B", "C"]);
    }

    #[test]
    fn an_empty_intermediate_page_is_not_the_end_and_follow_ups_are_bounded() {
        let (mut state, first) = started("đèn");
        let Step::Continue(second) = state.finish(&first, Ok(page(vec![], Some("c1")))) else {
            panic!("an empty page with a cursor continues");
        };
        assert_eq!(second.cursor(), Some("c1"));
        assert_eq!(state.phase(), PublicDiscoveryPhase::Loading);
        let Step::Continue(third) = state.finish(&second, Ok(page(vec![], Some("c2")))) else {
            panic!("still not the end");
        };
        assert_eq!(third.cursor(), Some("c2"));
        // Rows arrive: the walk stops following by itself.
        assert_eq!(
            state.finish(&third, Ok(page(vec![work(1, "A")], Some("c3")))),
            Step::Settled
        );
        assert_eq!(state.tail(), Tail::More);

        // A server that never stops answering empty pages exhausts the budget and then waits
        // for the listener; it is neither "no results" nor an endless loop.
        let (mut state, mut ticket) = started("");
        let mut requests = 1;
        let step = loop {
            match state.finish(&ticket, Ok(page(vec![], Some("again")))) {
                Step::Continue(next) => {
                    requests += 1;
                    ticket = next;
                }
                done => break done,
            }
        };
        assert_eq!(step, Step::Settled);
        assert_eq!(requests, 1 + usize::from(AUTOMATIC_FOLLOW_UPS));
        assert_eq!(state.phase(), PublicDiscoveryPhase::Incomplete);
        assert_eq!(state.tail(), Tail::More);
        // The listener can look further, with a refilled budget.
        let more = state.load_more().unwrap();
        assert_eq!(more.cursor(), Some("again"));
        assert!(matches!(
            state.finish(&more, Ok(page(vec![], Some("again")))),
            Step::Continue(_)
        ));
    }

    #[test]
    fn a_complete_walk_without_rows_is_the_only_empty_result() {
        let (mut state, first) = started("không có");
        state.finish(&first, Ok(page(vec![], None)));
        assert_eq!(state.phase(), PublicDiscoveryPhase::Empty);
    }

    #[test]
    fn a_failed_later_page_keeps_the_rows_and_retries_with_the_same_cursor() {
        let (mut state, first) = started("");
        state.finish(&first, Ok(page(vec![work(1, "A")], Some("c1"))));
        let more = state.load_more().unwrap();
        state.finish(&more, Err(DiscoveryReadError::Unavailable));
        assert_eq!(titles(&state), ["A"]);
        assert_eq!(state.phase(), PublicDiscoveryPhase::Ready);
        assert_eq!(
            state.tail(),
            Tail::Failed(PublicDiscoveryFailure::Unavailable)
        );
        let retry = state.retry().unwrap();
        assert_eq!(retry.cursor(), Some("c1"));
        state.finish(&retry, Err(DiscoveryReadError::Protocol));
        assert_eq!(state.tail(), Tail::Failed(PublicDiscoveryFailure::Protocol));
        let retry = state.load_more().unwrap();
        assert_eq!(retry.cursor(), Some("c1"));
        state.finish(&retry, Ok(page(vec![work(2, "B")], None)));
        assert_eq!(titles(&state), ["A", "B"]);
        assert_eq!(state.tail(), Tail::End);
    }

    #[test]
    fn a_failed_first_page_retries_from_page_one_and_never_becomes_empty() {
        let (mut state, first) = started("");
        state.finish(&first, Err(DiscoveryReadError::Unavailable));
        assert_eq!(
            state.phase(),
            PublicDiscoveryPhase::Failed(PublicDiscoveryFailure::Unavailable)
        );
        assert_eq!(state.load_more(), None);
        let retry = state.retry().unwrap();
        assert_eq!(retry.cursor(), None);
        state.finish(&retry, Err(DiscoveryReadError::Protocol));
        assert_eq!(
            state.phase(),
            PublicDiscoveryPhase::Failed(PublicDiscoveryFailure::Protocol)
        );
        // Page one has no cursor to be invalid: that answer is a protocol failure.
        let retry = state.retry().unwrap();
        state.finish(&retry, Err(DiscoveryReadError::InvalidCursor));
        assert_eq!(
            state.phase(),
            PublicDiscoveryPhase::Failed(PublicDiscoveryFailure::Protocol)
        );
    }

    #[test]
    fn leaving_a_route_discards_old_counts_and_fences_a_late_answer_after_same_query_return() {
        let (mut state, first) = started("đèn");
        state.finish(&first, Ok(page(vec![work(1, "A")], Some("c1"))));
        let old = state.load_more().unwrap();
        state.discard_snapshot();
        assert!(state.rows().is_empty());
        let fresh = state.open(old.query().clone()).unwrap();
        state.finish(&old, Ok(page(vec![work(2, "HIDDEN_OLD")], None)));
        assert!(state.rows().is_empty());
        state.finish(&fresh, Ok(page(vec![work(3, "Current")], None)));
        assert_eq!(titles(&state), ["Current"]);
    }

    #[test]
    fn an_invalid_cursor_discards_the_rows_and_only_a_fresh_walk_continues() {
        let (mut state, first) = started("đèn");
        state.finish(&first, Ok(page(vec![work(1, "A")], Some("c1"))));
        let more = state.load_more().unwrap();
        state.finish(&more, Err(DiscoveryReadError::InvalidCursor));
        assert!(titles(&state).is_empty());
        assert_eq!(state.tail(), Tail::Expired);
        assert_eq!(state.load_more(), None, "an expired cursor is never reused");
        assert_eq!(state.retry(), None);
        let fresh = state.begin(state.query().clone()).unwrap();
        assert_eq!(fresh.cursor(), None);
        assert_eq!(
            titles(&state),
            Vec::<String>::new(),
            "the old access scope cannot be shown while reloading"
        );
        state.finish(&fresh, Ok(page(vec![work(2, "B")], None)));
        assert_eq!(titles(&state), ["B"]);
    }

    #[test]
    fn an_invalid_cursor_before_any_row_shows_a_recoverable_expired_state() {
        let (mut state, first) = started("");
        let Step::Continue(second) = state.finish(&first, Ok(page(vec![], Some("c1")))) else {
            panic!()
        };
        state.finish(&second, Err(DiscoveryReadError::InvalidCursor));
        assert_eq!(state.phase(), PublicDiscoveryPhase::Expired);
        assert!(state.begin(DiscoveryQuery::default()).is_some());
    }

    #[test]
    fn changing_the_query_fences_the_late_answer_of_the_previous_walk() {
        let (mut state, a) = started("a");
        let b = state.begin(query("b")).unwrap();
        assert_eq!(state.query().text(), "b");
        assert_eq!(
            state.finish(&a, Ok(page(vec![work(1, "late A")], Some("cA")))),
            Step::Ignored
        );
        assert!(state.rows().is_empty() && state.phase() == PublicDiscoveryPhase::Loading);
        state.finish(&b, Ok(page(vec![work(2, "B")], None)));
        assert_eq!(titles(&state), ["B"]);
        assert_eq!(
            state.finish(&b, Err(DiscoveryReadError::Unavailable)),
            Step::Ignored,
            "a settled ticket cannot answer twice"
        );
        assert_eq!(titles(&state), ["B"]);
    }

    #[test]
    fn a_cursor_of_one_query_is_never_sent_with_another() {
        let (mut state, a) = started("a");
        state.finish(&a, Ok(page(vec![work(1, "A")], Some("cursor-of-a"))));
        let b = state.begin(query("b")).unwrap();
        assert_eq!(b.cursor(), None);
        assert_eq!(b.query().text(), "b");
        state.finish(&b, Ok(page(vec![work(2, "B")], Some("cursor-of-b"))));
        assert_eq!(state.load_more().unwrap().cursor(), Some("cursor-of-b"));
        // The same text with another filter is another query and another walk.
        let kind = DiscoveryQuery::new("b", Some(WorkKind::Podcast), "").unwrap();
        assert_ne!(kind, query("b"));
        assert_eq!(state.begin(kind).unwrap().cursor(), None);
    }

    #[test]
    fn route_entry_keeps_the_answered_walk_and_only_a_different_query_restarts() {
        let mut state = PublicDiscoveryState::default();
        let first = state.open(query("đèn")).unwrap();
        assert_eq!(state.open(query("đèn")), None, "still loading: inert");
        state.finish(&first, Ok(page(vec![work(1, "A")], Some("c1"))));
        assert_eq!(
            state.open(query("đèn")),
            None,
            "back from a Work keeps rows"
        );
        assert_eq!(titles(&state), ["A"]);
        assert_eq!(state.load_more().unwrap().cursor(), Some("c1"));
        let other = state.open(DiscoveryQuery::default()).unwrap();
        assert_eq!(titles(&state), ["A"]);
        state.finish(&other, Ok(page(vec![], None)));
        assert_eq!(state.open(DiscoveryQuery::default()), None);
        assert_eq!(state.phase(), PublicDiscoveryPhase::Empty);
    }

    #[test]
    fn a_failed_first_page_is_not_reissued_by_route_entry_and_cancel_resets_without_reuse() {
        let mut state = PublicDiscoveryState::default();
        let first = state.open(query("")).unwrap();
        state.finish(&first, Err(DiscoveryReadError::Unavailable));
        assert_eq!(
            state.open(query("")),
            None,
            "Retry is the listener's choice"
        );
        let generation = state.generation();
        assert!(state.cancel());
        assert!(state.generation() > generation);
        assert_eq!(state.phase(), PublicDiscoveryPhase::Idle);
        let late = state.begin(query("")).unwrap();
        assert_eq!(
            state.finish(&first, Ok(page(vec![work(1, "old")], None))),
            Step::Ignored
        );
        assert!(state.finish(&late, Ok(page(vec![work(2, "new")], None))) == Step::Settled);
    }

    #[test]
    fn oversized_or_malformed_pages_fail_closed_and_keep_earlier_rows() {
        let (mut state, first) = started("");
        state.finish(&first, Ok(page(vec![work(1, "A")], Some("c1"))));
        for (items, cursor) in [
            ((10..40).map(|n| work(n, "x")).collect::<Vec<_>>(), None),
            (vec![work(9, "x")], Some("")),
            (vec![work(9, "x")], Some("has space")),
            (vec![work(9, "x")], Some("é")),
        ] {
            let more = state.load_more().unwrap();
            state.finish(&more, Ok(page(items, cursor)));
            assert_eq!(state.tail(), Tail::Failed(PublicDiscoveryFailure::Protocol));
            assert_eq!(titles(&state), ["A"]);
        }
        let long = "c".repeat(MAX_CURSOR_BYTES + 1);
        let more = state.load_more().unwrap();
        state.finish(&more, Ok(page(vec![], Some(&long))));
        assert_eq!(state.tail(), Tail::Failed(PublicDiscoveryFailure::Protocol));
    }

    #[test]
    fn replacement_keeps_the_applied_snapshot_on_failure_and_commits_rows_with_query() {
        let (mut state, first) = started("a");
        state.finish(&first, Ok(page(vec![work(1, "A")], Some("cursor-a"))));
        let replacement = state.begin(query("b")).unwrap();
        assert_eq!(titles(&state), ["A"]);
        assert_eq!(state.applied_query(), Some(&query("a")));
        assert!(state.replacing());
        assert_eq!(replacement.cursor(), None);
        assert_eq!(state.load_more(), None);
        state.finish(&replacement, Err(DiscoveryReadError::Unavailable));
        assert_eq!(titles(&state), ["A"]);
        assert_eq!(state.applied_query(), Some(&query("a")));
        assert!(!state.replacing());
        assert!(state.replacement_failed());
        let retry = state.retry().unwrap();
        assert_eq!(retry.query().text(), "b");
        assert_eq!(retry.cursor(), None);
        state.finish(&retry, Ok(page(vec![work(2, "B")], Some("cursor-b"))));
        assert_eq!(titles(&state), ["B"]);
        assert_eq!(state.applied_query(), Some(&query("b")));
        assert_eq!(state.load_more().unwrap().cursor(), Some("cursor-b"));
        assert!(state.appending());
        assert!(!state.replacement_failed());
    }

    #[test]
    fn a_permission_or_withdrawal_refusal_clears_the_retained_snapshot_and_its_context() {
        use crate::application::errors::ErrorKind;
        for kind in [
            ErrorKind::Unauthenticated,
            ErrorKind::Forbidden,
            ErrorKind::ContentUnavailable,
        ] {
            let (mut state, first) = started("a");
            state.finish(&first, Ok(page(vec![work(1, "A")], Some("cursor-a"))));
            let refresh = state.begin(query("b")).unwrap();
            state.finish(&refresh, Err(DiscoveryReadError::Diagnostic(kind)));
            assert!(state.rows().is_empty());
            assert_eq!(state.applied_query(), None);
            assert_eq!(
                state.phase(),
                PublicDiscoveryPhase::Failed(PublicDiscoveryFailure::Diagnostic(kind))
            );
            assert_eq!(state.load_more(), None);
            assert_eq!(
                state.finish(&first, Ok(page(vec![work(1, "late")], None))),
                Step::Ignored
            );
        }
    }

    #[test]
    fn generation_exhaustion_preserves_the_last_state_without_reusing_identity() {
        let mut state = PublicDiscoveryState {
            generation: u64::MAX,
            rows: vec![work(1, "Đang hiển thị")],
            status: Status::Settled,
            walk_started: true,
            cursor: Some("c".into()),
            ..PublicDiscoveryState::default()
        };
        let previous = state.clone();
        assert_eq!(state.begin(query("x")), None);
        assert_eq!(state.load_more(), None);
        assert!(!state.cancel());
        assert_eq!(state, previous);
    }
}
