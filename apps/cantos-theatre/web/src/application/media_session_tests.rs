//! The real `PlayerSession`, reducer and `MediaDriver` run together against a simulated element.
//! Only the browser is replaced: these show the listener-visible status follows what the engine
//! reports, across grants, failures, system pauses and source changes. They are
//! mock-integration evidence, not browser or audio evidence.
use super::{
    media_driver::{
        AttachId, AuthorizationError, Command, ElementEvent, Input, MediaDriver, Ticket,
    },
    player_session::PlayerSession,
    ports::MediaPort,
};
use crate::contract::discovery::PlaybackAuthorization;
use crate::core::{
    playback::{Effect, Event, Failure, Intent, PauseCause, Source, Status},
    public_track::testing,
};
use leptos::prelude::*;
use std::{cell::RefCell, rc::Rc};

const NOW: i64 = 1_000_000;

#[derive(Clone)]
struct Element {
    driver: Rc<RefCell<MediaDriver>>,
    log: Rc<RefCell<Vec<Command>>>,
}

impl Element {
    fn step(&self, input: Input) -> Vec<Event> {
        let commands = self.driver.borrow_mut().step(NOW, input);
        let mut events = Vec::new();
        for command in commands {
            match command {
                Command::Emit(event) => events.push(event),
                other => self.log.borrow_mut().push(other),
            }
        }
        events
    }
    fn last_ticket(&self) -> Ticket {
        self.log
            .borrow()
            .iter()
            .rev()
            .find_map(|command| match command {
                Command::Authorize { ticket, .. } => Some(*ticket),
                _ => None,
            })
            .expect("an authorization request")
    }
    fn last_attach(&self) -> AttachId {
        self.log
            .borrow()
            .iter()
            .rev()
            .find_map(|command| match command {
                Command::Attach { attach, .. } => Some(*attach),
                _ => None,
            })
            .expect("an attachment")
    }
    fn count(&self, matches: impl Fn(&Command) -> bool) -> usize {
        self.log.borrow().iter().filter(|c| matches(c)).count()
    }
}

struct Port(Element);
impl MediaPort for Port {
    fn execute(&mut self, effect: &Effect) -> Vec<Event> {
        self.0.step(Input::Engine(Box::new(effect.clone())))
    }
}

struct World {
    session: PlayerSession,
    element: Element,
}

impl World {
    fn new() -> Self {
        let element = Element {
            driver: Rc::new(RefCell::new(MediaDriver::new())),
            log: Rc::new(RefCell::new(Vec::new())),
        };
        let port = Port(element.clone());
        Self {
            session: PlayerSession::new(port),
            element,
        }
    }
    /// What the browser does: feed an outcome back and deliver the events it produced.
    fn feed(&self, input: Input) {
        for event in self.element.step(input) {
            self.session.dispatch(event);
        }
    }
    fn choose(&self, n: u32) -> Source {
        let source = testing::source(n);
        self.session.dispatch(Event::SourceSelected {
            source,
            position_ms: 0,
        });
        source
    }
    fn grant(&self, n: u32) -> Input {
        let track = testing::release(n).track();
        Input::Authorized {
            ticket: self.element.last_ticket(),
            result: Ok(PlaybackAuthorization {
                publication_id: track.publication().to_string(),
                rendition_id: track.rendition().to_string(),
                sha256: track.sha256().to_string(),
                url: format!("https://localhost:8443/api/v1/theatre/media/pg1_{n}"),
                expires_at_ms: NOW + 120_000,
                content_type: "audio/ogg".into(),
                byte_range_supported: true,
            }),
        }
    }
    fn element_fact(&self, event: ElementEvent) {
        let attach = self.element.last_attach();
        self.feed(Input::Element { attach, event });
    }
    fn status(&self) -> Status {
        self.session.status.get_untracked()
    }
    fn intent(&self) -> Intent {
        self.session
            .state
            .get_untracked()
            .session()
            .unwrap()
            .intent()
    }
}

#[test]
fn the_indicator_follows_the_element_from_press_to_playing_to_a_system_pause() {
    Owner::new().with(|| {
        let world = World::new();
        world.choose(1);
        world.session.dispatch(Event::PlayRequested);
        // A press alone is not playing: the grant and the element still have to answer.
        assert_eq!(world.status(), Status::Loading);
        world.feed(world.grant(1));
        assert_eq!(world.status(), Status::Loading);
        world.element_fact(ElementEvent::Loaded);
        assert_eq!(world.status(), Status::Loading);
        world.element_fact(ElementEvent::Playing);
        assert_eq!(world.status(), Status::Playing);
        world.element_fact(ElementEvent::Position(4_000));
        assert_eq!(world.session.view.get_untracked().unwrap().position, 4);
        // A media key pauses the element behind the page's back: it must show paused, with
        // the Play action, not "preparing" and not still "playing".
        world.element_fact(ElementEvent::Paused);
        assert_eq!(world.status(), Status::Paused(PauseCause::User));
        assert_eq!(world.intent(), Intent::Pause(PauseCause::User));
        // The listener plays again: the same attached element is told to play, nothing is fetched.
        let authorizations = world
            .element
            .count(|c| matches!(c, Command::Authorize { .. }));
        world.session.dispatch(Event::PlayRequested);
        assert_eq!(
            world
                .element
                .count(|c| matches!(c, Command::Authorize { .. })),
            authorizations
        );
        world.element_fact(ElementEvent::Playing);
        assert_eq!(world.status(), Status::Playing);
    });
}

#[test]
fn a_withdrawn_release_shows_a_named_failure_and_only_an_explicit_press_asks_again() {
    Owner::new().with(|| {
        let world = World::new();
        world.choose(1);
        world.session.dispatch(Event::PlayRequested);
        let ticket = world.element.last_ticket();
        world.feed(Input::Authorized {
            ticket,
            result: Err(AuthorizationError::NotFound),
        });
        assert_eq!(world.status(), Status::Failed(Failure::Unavailable));
        assert_eq!(world.intent(), Intent::Pause(PauseCause::User));
        assert_eq!(
            world
                .element
                .count(|c| matches!(c, Command::Authorize { .. })),
            1,
            "no automatic second request"
        );
        // The press is the retry, and it fails the same way without inventing a loop.
        world.session.dispatch(Event::PlayRequested);
        assert_eq!(world.status(), Status::Loading);
        assert_eq!(
            world
                .element
                .count(|c| matches!(c, Command::Authorize { .. })),
            2
        );
    });
}

#[test]
fn a_seek_is_pending_until_the_element_reports_the_position() {
    Owner::new().with(|| {
        let world = World::new();
        world.choose(1);
        world.session.dispatch(Event::PlayRequested);
        world.feed(world.grant(1));
        world.element_fact(ElementEvent::Playing);
        world.session.dispatch(Event::SeekRequested {
            position_ms: 90_000,
        });
        assert!(world.session.pending_seek.get_untracked().is_some());
        assert!(world
            .element
            .log
            .borrow()
            .contains(&Command::Seek { to_ms: 90_000 }));
        // A real element lands a few milliseconds off the target; the seek is still confirmed.
        world.element_fact(ElementEvent::Seeked(90_020));
        assert!(world.session.pending_seek.get_untracked().is_none());
        assert_eq!(world.session.view.get_untracked().unwrap().position, 90);
        assert_eq!(
            world
                .session
                .state
                .get_untracked()
                .session()
                .unwrap()
                .position_ms(),
            90_020
        );
    });
}

#[test]
fn choosing_another_release_releases_the_first_and_its_late_facts_change_nothing() {
    Owner::new().with(|| {
        let world = World::new();
        world.choose(1);
        world.session.dispatch(Event::PlayRequested);
        world.feed(world.grant(1));
        let first_attach = world.element.last_attach();
        world.element_fact(ElementEvent::Playing);
        world.choose(2);
        world.session.dispatch(Event::PlayRequested);
        assert!(world.element.count(|c| matches!(c, Command::Release)) >= 1);
        let before = world.session.state.get_untracked();
        for event in [
            ElementEvent::Playing,
            ElementEvent::Position(50_000),
            ElementEvent::Ended,
            ElementEvent::Loaded,
        ] {
            world.feed(Input::Element {
                attach: first_attach,
                event,
            });
        }
        assert_eq!(world.session.state.get_untracked(), before);
        // The new release is authorized for its own publication only, then plays normally.
        world.feed(world.grant(2));
        world.element_fact(ElementEvent::Playing);
        assert_eq!(world.status(), Status::Playing);
        assert_eq!(
            world.session.source.get_untracked(),
            Some(testing::source(2))
        );
    });
}

#[test]
fn ending_replays_from_the_start_on_the_same_element_without_a_new_grant() {
    Owner::new().with(|| {
        let world = World::new();
        world.choose(1);
        world.session.dispatch(Event::PlayRequested);
        world.feed(world.grant(1));
        world.element_fact(ElementEvent::Playing);
        // Chrome raises `pause` just before `ended`.
        world.element_fact(ElementEvent::Paused);
        world.element_fact(ElementEvent::Ended);
        assert_eq!(world.status(), Status::Ended);
        let authorizations = world
            .element
            .count(|c| matches!(c, Command::Authorize { .. }));
        world.session.dispatch(Event::PlayRequested);
        assert!(world
            .element
            .log
            .borrow()
            .contains(&Command::Play { from_ms: 0 }));
        assert_eq!(
            world
                .element
                .count(|c| matches!(c, Command::Authorize { .. })),
            authorizations
        );
    });
}
