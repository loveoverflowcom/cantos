//! Deterministic fake engine: no sound, DOM media, network, wall clock or persistence.
use crate::{
    application::ports::MediaPort,
    core::playback::{Effect, Event},
};

#[derive(Clone, Debug, Default)]
pub struct FakeMedia;
impl MediaPort for FakeMedia {
    fn execute(&mut self, effect: &Effect) -> Vec<Event> {
        match effect {
            Effect::PrepareEngine(command) => vec![Event::EngineReady {
                identity: command.identity,
            }],
            Effect::StartEngine(command) => vec![Event::EngineStarted {
                identity: command.identity,
            }],
            Effect::PauseEngine(command) => vec![Event::EnginePaused {
                identity: command.identity,
            }],
            Effect::SeekEngine(command) => vec![Event::EnginePosition {
                identity: command.identity,
                position_ms: command.position_ms,
            }],
            Effect::SetSpeed(_) | Effect::OfferResume { .. } | Effect::Rejected(_) => Vec::new(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        ids::{EpisodeId, WorkId},
        playback::{reduce, PlayerState, Source, Status},
    };
    #[test]
    fn fake_confirms_play_after_intent_and_never_ticks_position() {
        let source = Source::new(
            EpisodeId {
                work: WorkId::River,
                index: 0,
            },
            1232000,
        )
        .unwrap();
        let selected = reduce(
            PlayerState::default(),
            Event::SourceSelected {
                source,
                position_ms: 0,
            },
        );
        let intent = reduce(selected.state, Event::PlayRequested);
        assert_eq!(intent.state.status(), Status::Loading);
        let mut port = FakeMedia;
        let callback = port.execute(&intent.effects[0]).pop().unwrap();
        let confirmed = reduce(intent.state, callback);
        assert_eq!(confirmed.state.status(), Status::Playing);
        assert_eq!(confirmed.state.session().unwrap().position_ms(), 0);
        assert!(confirmed.effects.is_empty());
    }
}
