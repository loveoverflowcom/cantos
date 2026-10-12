use super::{
    state::supported_speed, CallbackIdentity, Effect, Engine, EngineCommand, Event, Intent,
    Interruption, PauseCause, PlaybackSession, PlayerState, Rejection, ResumeSnapshot, Source,
    Transition,
};

fn unchanged(state: PlayerState) -> Transition { Transition { state, effects: Vec::new() } }
fn rejected(state: PlayerState, reason: Rejection) -> Transition { Transition { state, effects: vec![Effect::Rejected(reason)] } }
fn command(state: &PlayerState) -> Option<EngineCommand> {
    let session = state.session.as_ref()?;
    Some(EngineCommand {
        identity: state.identity()?, source: session.source,
        position_ms: session.position_ms, speed_permille: session.speed_permille,
    })
}
fn advance(mut state: PlayerState) -> Result<PlayerState, PlayerState> {
    let Some(generation) = state.generation.checked_add(1) else { return Err(state); };
    state.generation = generation;
    Ok(state)
}
fn loaded(state: PlayerState, source: Source, position_ms: u32, speed: u16, cause: PauseCause, restore: bool) -> Transition {
    if position_ms > source.duration_ms() { return rejected(state, Rejection::OutOfBounds); }
    if !supported_speed(speed) { return rejected(state, Rejection::UnsupportedSpeed); }
    let interruption = if restore { Interruption::None } else { state.session.as_ref().map(|session| session.interruption).unwrap_or(Interruption::None) };
    let mut next = match advance(state) { Ok(next) => next, Err(state) => return rejected(state, Rejection::GenerationExhausted) };
    next.session = Some(PlaybackSession { source, position_ms, speed_permille: speed, intent: Intent::Pause(cause), engine: Engine::Preparing, interruption });
    let mut effects = command(&next).map(Effect::PrepareEngine).into_iter().collect::<Vec<_>>();
    if restore { effects.push(Effect::OfferResume { source, position_ms }); }
    Transition { state: next, effects }
}
fn seek(state: PlayerState, position_ms: u32) -> Transition {
    let Some(session) = state.session.as_ref() else { return rejected(state, Rejection::NoSession); };
    if position_ms > session.source.duration_ms() { return rejected(state, Rejection::OutOfBounds); }
    if position_ms == session.position_ms { return unchanged(state); }
    let mut next = match advance(state) { Ok(next) => next, Err(state) => return rejected(state, Rejection::GenerationExhausted) };
    if let Some(session) = next.session.as_mut() { session.position_ms = position_ms; }
    let Some(command) = command(&next) else { return unchanged(next); };
    let effect = match next.session.as_ref().map(|session| (session.engine, session.intent)) {
        Some((Engine::Preparing, Intent::Play)) => Effect::StartEngine(command),
        Some((Engine::Preparing, Intent::Pause(_))) => Effect::PrepareEngine(command),
        _ => Effect::SeekEngine(command),
    };
    Transition { state: next, effects: vec![effect] }
}
fn callback(mut state: PlayerState, identity: CallbackIdentity, change: impl FnOnce(&mut PlaybackSession)) -> Transition {
    if !state.accepts(identity) { return unchanged(state); }
    if let Some(session) = state.session.as_mut() { change(session); }
    unchanged(state)
}

/// Immutable state in, transition/effects out. No Leptos, ports, clock, storage or engine call.
#[must_use]
pub fn reduce(state: PlayerState, event: Event) -> Transition {
    match event {
        Event::SourceSelected { source, position_ms } => {
            if state.session.as_ref().is_some_and(|session| session.source == source) { return unchanged(state); }
            loaded(state, source, position_ms, 1000, PauseCause::User, false)
        }
        Event::RestoreRequested { snapshot: ResumeSnapshot { source, position_ms, speed_permille } } => loaded(state, source, position_ms, speed_permille, PauseCause::ColdRestore, true),
        Event::PlayRequested => {
            let Some(session) = state.session.as_ref() else { return rejected(state, Rejection::NoSession); };
            if session.interruption == Interruption::Active { return rejected(state, Rejection::Interrupted); }
            if session.intent == Intent::Play && session.engine != Engine::Ended { return unchanged(state); }
            let mut next = match advance(state) { Ok(next) => next, Err(state) => return rejected(state, Rejection::GenerationExhausted) };
            if let Some(session) = next.session.as_mut() {
                if session.position_ms == session.source.duration_ms() { session.position_ms = 0; }
                session.intent = Intent::Play; session.engine = Engine::Preparing;
            }
            let effects = command(&next).map(Effect::StartEngine).into_iter().collect();
            Transition { state: next, effects }
        }
        Event::PauseRequested => {
            let Some(session) = state.session.as_ref() else { return rejected(state, Rejection::NoSession); };
            if session.intent == Intent::Pause(PauseCause::User) && session.engine == Engine::Paused { return unchanged(state); }
            let mut next = match advance(state) { Ok(next) => next, Err(state) => return rejected(state, Rejection::GenerationExhausted) };
            if let Some(session) = next.session.as_mut() { session.intent = Intent::Pause(PauseCause::User); session.engine = Engine::Paused; }
            let effects = command(&next).map(Effect::PauseEngine).into_iter().collect();
            Transition { state: next, effects }
        }
        Event::SeekRequested { position_ms } => seek(state, position_ms),
        Event::SkipRequested { delta_ms } => {
            let Some(session) = state.session.as_ref() else { return rejected(state, Rejection::NoSession); };
            let position_ms = (i64::from(session.position_ms) + i64::from(delta_ms)).clamp(0, i64::from(session.source.duration_ms())) as u32;
            seek(state, position_ms)
        }
        Event::SpeedRequested { speed_permille } => {
            if !supported_speed(speed_permille) { return rejected(state, Rejection::UnsupportedSpeed); }
            let Some(session) = state.session.as_ref() else { return rejected(state, Rejection::NoSession); };
            if session.speed_permille == speed_permille { return unchanged(state); }
            let mut next = state;
            if let Some(session) = next.session.as_mut() { session.speed_permille = speed_permille; }
            let effects = command(&next).map(Effect::SetSpeed).into_iter().collect();
            Transition { state: next, effects }
        }
        Event::InterruptionBegan => {
            let Some(session) = state.session.as_ref() else { return unchanged(state); };
            if session.interruption == Interruption::Active { return unchanged(state); }
            let mut next = match advance(state) { Ok(next) => next, Err(state) => return rejected(state, Rejection::GenerationExhausted) };
            if let Some(session) = next.session.as_mut() { session.interruption = Interruption::Active; session.engine = Engine::Paused; }
            let effects = command(&next).map(Effect::PauseEngine).into_iter().collect();
            Transition { state: next, effects }
        }
        Event::InterruptionEnded { resume_allowed } => {
            let Some(session) = state.session.as_ref() else { return unchanged(state); };
            if session.interruption != Interruption::Active { return unchanged(state); }
            let resume = resume_allowed && session.intent == Intent::Play;
            let mut next = match advance(state) { Ok(next) => next, Err(state) => return rejected(state, Rejection::GenerationExhausted) };
            if let Some(session) = next.session.as_mut() {
                session.interruption = Interruption::None;
                if resume { session.engine = Engine::Preparing; }
                else if session.intent == Intent::Play { session.intent = Intent::Pause(PauseCause::Interruption); }
            }
            let effects = if resume { command(&next).map(Effect::StartEngine).into_iter().collect() } else { Vec::new() };
            Transition { state: next, effects }
        }
        Event::EngineReady { identity } => callback(state, identity, |session| {
            if session.engine == Engine::Preparing { session.engine = Engine::Ready; }
        }),
        Event::EngineStarted { identity } => callback(state, identity, |session| {
            if session.intent == Intent::Play && session.interruption == Interruption::None { session.engine = Engine::Running; }
        }),
        Event::EnginePaused { identity } => callback(state, identity, |session| { session.engine = Engine::Paused; }),
        Event::EngineBuffering { identity } => callback(state, identity, |session| {
            if session.intent == Intent::Play && session.interruption == Interruption::None { session.engine = Engine::Buffering; }
        }),
        Event::EnginePosition { identity, position_ms } => {
            if !state.accepts(identity) { return unchanged(state); }
            if state.session.as_ref().is_some_and(|session| position_ms > session.source.duration_ms()) { return rejected(state, Rejection::OutOfBounds); }
            callback(state, identity, |session| { session.position_ms = position_ms; })
        }
        Event::EngineEnded { identity } => callback(state, identity, |session| {
            session.engine = Engine::Ended; session.intent = Intent::Pause(PauseCause::User);
            session.position_ms = session.source.duration_ms();
        }),
    }
}
