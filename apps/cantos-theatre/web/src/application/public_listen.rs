//! A listener's intent to hear one public unit, as a pure state machine. Choosing a unit resolves
//! the exact release's manifest; only an admitted manifest becomes a source for the player.
//! Navigation, reload and sign-in never reach this state, so none of them can start audio.
//!
//! Each intent carries a ticket. A newer intent supersedes an older one, and a manifest answer
//! for a superseded ticket is dropped, so a slow response cannot replace what the listener
//! chose since.
use crate::contract::discovery::PublicationManifest;
use crate::{
    application::public_work::CatalogReadError,
    core::{
        playback::{Source, TrackId},
        public_track::{ManifestRejection, PinnedRelease, PublicTrack, ReleaseRequest},
    },
};
use uuid::Uuid;

/// What the listener sees while a release is being resolved or after it could not be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenFailure {
    Diagnostic(crate::application::errors::ErrorKind),
    /// The release is gone: withdrawn, revoked or replaced since the list was read.
    NotFound,
    /// Transport or a retryable server answer; trying again is reasonable.
    Unavailable,
    /// The manifest breaks the contract or names another release.
    Protocol,
}

impl From<ManifestRejection> for ListenFailure {
    fn from(_: ManifestRejection) -> Self {
        // Every rejection means the manifest cannot be trusted for this request.
        Self::Protocol
    }
}

/// Display facts captured with the intent. They are never identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListenLabel {
    pub work_title: String,
    pub unit_title: String,
    pub language: String,
}

/// Display facts for the track in the player. They are never identity: a label that does not
/// describe the session's track is ignored rather than shown under another release's audio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NowPlaying {
    track: PublicTrack,
    label: ListenLabel,
}

impl NowPlaying {
    pub fn new(release: &PinnedRelease, label: ListenLabel) -> Self {
        Self {
            track: release.track(),
            label,
        }
    }
    pub fn track(&self) -> PublicTrack {
        self.track
    }
    pub fn label(&self) -> &ListenLabel {
        &self.label
    }
    /// Whether these facts describe the audio `source` currently plays.
    pub fn describes(&self, source: Source) -> bool {
        source.track() == TrackId::Public(self.track)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListenPhase {
    Idle,
    Resolving(ReleaseRequest),
    Failed(ReleaseRequest, ListenFailure),
}

/// Issued only by [`ListenState::begin`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListenTicket {
    generation: u64,
    request: ReleaseRequest,
    label: ListenLabel,
}

impl ListenTicket {
    pub fn publication(&self) -> Uuid {
        self.request.publication()
    }
}

/// An admitted release and the display facts captured with the intent that chose it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub release: PinnedRelease,
    pub label: ListenLabel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListenOutcome {
    /// A stale or duplicate answer; nothing changed.
    Ignored,
    /// The admitted release to hand to the player.
    Select(Box<Selection>),
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListenState {
    generation: u64,
    phase: ListenPhase,
}

impl Default for ListenState {
    fn default() -> Self {
        Self {
            generation: 0,
            phase: ListenPhase::Idle,
        }
    }
}

impl ListenState {
    pub fn phase(&self) -> &ListenPhase {
        &self.phase
    }

    /// Whether `request`'s release is the one being resolved.
    pub fn is_resolving(&self, publication: Uuid) -> bool {
        matches!(&self.phase, ListenPhase::Resolving(request) if request.publication() == publication)
    }

    pub fn failure_of(&self, publication: Uuid) -> Option<ListenFailure> {
        match &self.phase {
            ListenPhase::Failed(request, failure) if request.publication() == publication => {
                Some(*failure)
            }
            _ => None,
        }
    }

    /// Start resolving `request`. A second press for the release already being resolved is
    /// ignored; any other release supersedes the first.
    pub fn begin(&mut self, request: ReleaseRequest, label: ListenLabel) -> Option<ListenTicket> {
        if matches!(&self.phase, ListenPhase::Resolving(current) if *current == request) {
            return None;
        }
        self.generation = self.generation.checked_add(1)?;
        self.phase = ListenPhase::Resolving(request);
        Some(ListenTicket {
            generation: self.generation,
            request,
            label,
        })
    }

    /// Apply the manifest read for `ticket`.
    pub fn finish(
        &mut self,
        ticket: &ListenTicket,
        result: Result<PublicationManifest, CatalogReadError>,
    ) -> ListenOutcome {
        if ticket.generation != self.generation
            || !matches!(&self.phase, ListenPhase::Resolving(request) if *request == ticket.request)
        {
            return ListenOutcome::Ignored;
        }
        let failure = match result {
            Ok(manifest) => match PinnedRelease::admit(&manifest, ticket.request) {
                Ok(release) => {
                    self.phase = ListenPhase::Idle;
                    return ListenOutcome::Select(Box::new(Selection {
                        release,
                        label: ticket.label.clone(),
                    }));
                }
                Err(rejection) => ListenFailure::from(rejection),
            },
            Err(CatalogReadError::Diagnostic(kind)) => ListenFailure::Diagnostic(kind),
            Err(CatalogReadError::NotFound) => ListenFailure::NotFound,
            Err(CatalogReadError::Unavailable) => ListenFailure::Unavailable,
            Err(CatalogReadError::Protocol | CatalogReadError::InvalidCursor) => {
                ListenFailure::Protocol
            }
        };
        self.phase = ListenPhase::Failed(ticket.request, failure);
        ListenOutcome::Failed
    }

    /// Drop a pending or failed intent without a new one, fencing any answer still in flight.
    pub fn clear(&mut self) {
        if let Some(next) = self.generation.checked_add(1) {
            self.generation = next;
            self.phase = ListenPhase::Idle;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::public_ids::PublicWorkId;

    const MANIFEST: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/manifest.json"
    );
    const WORK: &str = "10000000-0000-4000-8000-000000000001";
    const UNIT: &str = "20000000-0000-4000-8000-000000000002";
    const PUBLICATION: &str = "80000000-0000-4000-8000-000000000008";
    const OTHER_PUBLICATION: &str = "80000000-0000-4000-8000-000000000009";

    fn request(publication: &str) -> ReleaseRequest {
        ReleaseRequest::parse(PublicWorkId::parse(WORK).unwrap(), UNIT, publication).unwrap()
    }
    fn label(title: &str) -> ListenLabel {
        ListenLabel {
            work_title: "Ánh đèn cuối sân khấu".into(),
            unit_title: title.into(),
            language: "vi-VN".into(),
        }
    }
    fn manifest(publication: &str) -> PublicationManifest {
        let mut manifest: PublicationManifest = serde_json::from_str(MANIFEST).unwrap();
        manifest.publication_id = publication.into();
        manifest
    }

    #[test]
    fn an_admitted_manifest_selects_exactly_the_requested_release_with_its_label() {
        let mut state = ListenState::default();
        let ticket = state
            .begin(request(PUBLICATION), label("Chương 1"))
            .unwrap();
        assert!(state.is_resolving(request(PUBLICATION).publication()));
        match state.finish(&ticket, Ok(manifest(PUBLICATION))) {
            ListenOutcome::Select(selection) => {
                assert_eq!(
                    selection.release.track().publication(),
                    request(PUBLICATION).publication()
                );
                assert_eq!(selection.label.unit_title, "Chương 1");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(state.phase(), &ListenPhase::Idle);
    }

    #[test]
    fn now_playing_describes_only_the_audio_it_was_captured_for() {
        let admitted = |publication: &str| {
            PinnedRelease::admit(&manifest(publication), request(publication)).unwrap()
        };
        let (first, other) = (admitted(PUBLICATION), admitted(OTHER_PUBLICATION));
        let playing = NowPlaying::new(&first, label("Chương 1"));
        let source = |release: &PinnedRelease| {
            Source::public(release.track(), release.duration_ms()).unwrap()
        };
        assert!(playing.describes(source(&first)));
        assert!(!playing.describes(source(&other)));
        assert_eq!(playing.label().unit_title, "Chương 1");
    }

    #[test]
    fn a_manifest_for_another_release_fails_and_never_selects() {
        let mut state = ListenState::default();
        let ticket = state
            .begin(request(PUBLICATION), label("Chương 1"))
            .unwrap();
        assert_eq!(
            state.finish(&ticket, Ok(manifest(OTHER_PUBLICATION))),
            ListenOutcome::Failed
        );
        assert_eq!(
            state.failure_of(request(PUBLICATION).publication()),
            Some(ListenFailure::Protocol)
        );
    }

    #[test]
    fn read_failures_keep_their_class_and_are_retried_only_by_a_new_intent() {
        for (error, expected) in [
            (CatalogReadError::NotFound, ListenFailure::NotFound),
            (CatalogReadError::Unavailable, ListenFailure::Unavailable),
            (CatalogReadError::Protocol, ListenFailure::Protocol),
            (CatalogReadError::InvalidCursor, ListenFailure::Protocol),
        ] {
            let mut state = ListenState::default();
            let ticket = state.begin(request(PUBLICATION), label("a")).unwrap();
            assert_eq!(state.finish(&ticket, Err(error)), ListenOutcome::Failed);
            assert_eq!(
                state.failure_of(request(PUBLICATION).publication()),
                Some(expected)
            );
            // The same ticket cannot answer twice; a fresh press can try again.
            assert_eq!(
                state.finish(&ticket, Ok(manifest(PUBLICATION))),
                ListenOutcome::Ignored
            );
            assert!(state.begin(request(PUBLICATION), label("a")).is_some());
        }
    }

    #[test]
    fn a_newer_intent_supersedes_a_slow_manifest_answer() {
        let mut state = ListenState::default();
        let slow = state
            .begin(request(PUBLICATION), label("Chương 1"))
            .unwrap();
        let newer = state
            .begin(request(OTHER_PUBLICATION), label("Chương 2"))
            .unwrap();
        assert_eq!(
            state.finish(&slow, Ok(manifest(PUBLICATION))),
            ListenOutcome::Ignored
        );
        assert!(state.is_resolving(request(OTHER_PUBLICATION).publication()));
        assert!(matches!(
            state.finish(&newer, Ok(manifest(OTHER_PUBLICATION))),
            ListenOutcome::Select(..)
        ));
    }

    #[test]
    fn pressing_twice_for_the_release_already_resolving_issues_one_request() {
        let mut state = ListenState::default();
        assert!(state.begin(request(PUBLICATION), label("a")).is_some());
        assert!(state.begin(request(PUBLICATION), label("a")).is_none());
        assert!(state
            .begin(request(OTHER_PUBLICATION), label("b"))
            .is_some());
    }

    #[test]
    fn an_answer_for_an_older_intent_cannot_settle_a_newer_intent_for_the_same_release() {
        let mut state = ListenState::default();
        let first = state.begin(request(PUBLICATION), label("a")).unwrap();
        state.clear();
        let second = state.begin(request(PUBLICATION), label("a")).unwrap();
        // The same release is resolving again, so only the ticket tells the two answers apart.
        assert_eq!(
            state.finish(&first, Ok(manifest(PUBLICATION))),
            ListenOutcome::Ignored
        );
        assert!(state.is_resolving(request(PUBLICATION).publication()));
        assert!(matches!(
            state.finish(&second, Ok(manifest(PUBLICATION))),
            ListenOutcome::Select(..)
        ));
    }

    #[test]
    fn clearing_fences_an_answer_still_in_flight() {
        let mut state = ListenState::default();
        let ticket = state.begin(request(PUBLICATION), label("a")).unwrap();
        state.clear();
        assert_eq!(state.phase(), &ListenPhase::Idle);
        assert_eq!(
            state.finish(&ticket, Ok(manifest(PUBLICATION))),
            ListenOutcome::Ignored
        );
    }

    #[test]
    fn an_exhausted_generation_refuses_a_new_intent_without_changing_state() {
        let mut state = ListenState {
            generation: u64::MAX,
            phase: ListenPhase::Idle,
        };
        assert_eq!(state.begin(request(PUBLICATION), label("a")), None);
        assert_eq!(state.phase(), &ListenPhase::Idle);
    }
}
