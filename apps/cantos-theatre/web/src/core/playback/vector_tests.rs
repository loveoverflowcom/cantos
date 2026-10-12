//! Strict, test-only consumer of hand-authored contract vectors. No expected output is generated.
use super::{
    reduce, CallbackIdentity, Effect, Engine, EngineCommand, Event, Intent, Interruption,
    PauseCause, PlaybackSession, PlayerState, Rejection, ResumeSnapshot, Source, Status,
};
use crate::core::ids::{EpisodeId, WorkId};
use serde_json::Value;

const CONTRACT: &str = "theatre-mock-playback@1";
const INDEX: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../packages/contracts/fixtures/theatre/playback/v1/index.json"
));

struct EmbeddedVector {
    name: &'static str,
    json: &'static str,
    sha256: &'static str,
    byte_fingerprint: u64,
}

macro_rules! embedded_vector {
    ($name:literal, $sha256:literal, $fingerprint:literal) => {
        EmbeddedVector {
            name: concat!($name, ".json"),
            json: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../packages/contracts/fixtures/theatre/playback/v1/",
                $name,
                ".json"
            )),
            sha256: $sha256,
            byte_fingerprint: $fingerprint,
        }
    };
}

// Pin the reviewed inventory independently of the index, including exact embedded bytes.
// FNV-1a is a non-cryptographic drift sentinel. Exact SHA-256 byte verification is a separate
// check; the SHA-256 values below are pinned from the reviewed index. No crypto code or app
// runtime dependency is introduced.
const VECTORS: [EmbeddedVector; 7] = [
    embedded_vector!(
        "play-pause",
        "bc9b73d4be95362d1a6d7e178464bf26d100d93edac02edb27cf018c45a54622",
        0x91e2d9aa26005cdc
    ),
    embedded_vector!(
        "buffering",
        "4b5f7df3d009c0855f328a600f2c4c9149e6277d50469b01b864ea2604a1797c",
        0x2695743e2e5ee7e7
    ),
    embedded_vector!(
        "interruption",
        "3c39358f06d38dce35fd73ddebae35fe0a6a36bc04727ceee8dd6feb624fb945",
        0xa8f639846bc12bb3
    ),
    embedded_vector!(
        "stale-response",
        "d58ea2b08b79ab453c3864da3cb5532fe01825adf4db0c93f895c4c81f018ae6",
        0x91aa2f29801c7141
    ),
    embedded_vector!(
        "restore-session",
        "bca2510896ed12250a3979abe42414e66c14357c73d403532a90c54316ddb900",
        0xfc1ad5b2413c178b
    ),
    embedded_vector!(
        "preparing-races",
        "b5d21a9cece278313b89de9b5e885bff424ca1752f5b0d23d8fc39f838fa8082",
        0x5bc5db51bc5986f2
    ),
    embedded_vector!(
        "invalid-controls",
        "bfa0807d0df0186dd5429ac082ef40df87879d487c6823d3c365e6e5ba0304a9",
        0x2391a88f3ef69f18
    ),
];

type Decode<T> = Result<T, &'static str>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    state: PlayerState,
    status: Status,
}

impl Snapshot {
    fn of(state: PlayerState) -> Self {
        let status = state.status();
        Self { state, status }
    }
}

#[derive(Debug)]
struct Step {
    event: Event,
    state: Snapshot,
    effects: Vec<Effect>,
}

#[derive(Debug)]
struct Vector {
    id: String,
    initial_state: Snapshot,
    steps: Vec<Step>,
}

fn fields(value: &Value, expected: &[&str]) -> Decode<()> {
    let object = value.as_object().ok_or("expected object")?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err("unexpected object fields");
    }
    Ok(())
}

fn string(value: &Value) -> Decode<&str> {
    value.as_str().ok_or("expected string")
}

fn array(value: &Value) -> Decode<&[Value]> {
    value.as_array().map(Vec::as_slice).ok_or("expected array")
}

fn u32_value(value: &Value) -> Decode<u32> {
    u32::try_from(value.as_u64().ok_or("expected unsigned integer")?)
        .map_err(|_| "unsigned integer out of bounds")
}

fn u16_value(value: &Value) -> Decode<u16> {
    u16::try_from(u32_value(value)?).map_err(|_| "unsigned integer out of bounds")
}

fn speed(value: &Value) -> Decode<u16> {
    let speed = u16_value(value)?;
    if [750, 1000, 1250, 1500, 2000].contains(&speed) {
        Ok(speed)
    } else {
        Err("unsupported snapshot speed")
    }
}

fn episode(value: &Value) -> Decode<EpisodeId> {
    let work = WorkId::from_slug(string(&value["work_id"])?).ok_or("unknown work key")?;
    let index = u16_value(&value["episode_index"])?;
    Ok(EpisodeId {
        work,
        index: usize::from(index),
    })
}

fn source(value: &Value) -> Decode<Source> {
    fields(value, &["work_id", "episode_index", "duration_ms"])?;
    Source::new(episode(value)?, u32_value(&value["duration_ms"])?).ok_or("invalid source")
}

fn identity(value: &Value) -> Decode<CallbackIdentity> {
    fields(value, &["work_id", "episode_index", "generation"])?;
    Ok(CallbackIdentity {
        episode: episode(value)?,
        generation: u32_value(&value["generation"])?,
    })
}

fn pause_cause(value: &Value) -> Decode<PauseCause> {
    match string(value)? {
        "user" => Ok(PauseCause::User),
        "interruption" => Ok(PauseCause::Interruption),
        "cold_restore" => Ok(PauseCause::ColdRestore),
        _ => Err("unknown pause cause"),
    }
}

fn intent(value: &Value) -> Decode<Intent> {
    match string(&value["type"])? {
        "play" => {
            fields(value, &["type"])?;
            Ok(Intent::Play)
        }
        "pause" => {
            fields(value, &["type", "cause"])?;
            Ok(Intent::Pause(pause_cause(&value["cause"])?))
        }
        _ => Err("unknown intent type"),
    }
}

fn engine(value: &Value) -> Decode<Engine> {
    match string(value)? {
        "preparing" => Ok(Engine::Preparing),
        "ready" => Ok(Engine::Ready),
        "running" => Ok(Engine::Running),
        "paused" => Ok(Engine::Paused),
        "buffering" => Ok(Engine::Buffering),
        "ended" => Ok(Engine::Ended),
        _ => Err("unknown engine phase"),
    }
}

fn interruption(value: &Value) -> Decode<Interruption> {
    match string(value)? {
        "none" => Ok(Interruption::None),
        "active" => Ok(Interruption::Active),
        _ => Err("unknown interruption phase"),
    }
}

fn session(value: &Value) -> Decode<PlaybackSession> {
    fields(
        value,
        &[
            "source",
            "position_ms",
            "speed_permille",
            "intent",
            "engine",
            "interruption",
        ],
    )?;
    let source = source(&value["source"])?;
    let position_ms = u32_value(&value["position_ms"])?;
    if position_ms > source.duration_ms() {
        return Err("snapshot position out of bounds");
    }
    Ok(PlaybackSession {
        source,
        position_ms,
        speed_permille: speed(&value["speed_permille"])?,
        intent: intent(&value["intent"])?,
        engine: engine(&value["engine"])?,
        interruption: interruption(&value["interruption"])?,
    })
}

fn snapshot(value: &Value) -> Decode<Snapshot> {
    fields(value, &["generation", "session", "status", "pause_cause"])?;
    let cause = if value["pause_cause"].is_null() {
        None
    } else {
        Some(pause_cause(&value["pause_cause"])?)
    };
    let status = match string(&value["status"])? {
        "idle" => Status::Idle,
        "loading" => Status::Loading,
        "paused" => Status::Paused(cause.ok_or("paused status requires a cause")?),
        "simulating" => Status::Simulating,
        "buffering" => Status::Buffering,
        "ended" => Status::Ended,
        _ => return Err("unknown status"),
    };
    if !matches!(status, Status::Paused(_)) && cause.is_some() {
        return Err("non-paused status must have no cause");
    }
    let state = PlayerState {
        generation: u32_value(&value["generation"])?,
        session: if value["session"].is_null() {
            None
        } else {
            Some(session(&value["session"])?)
        },
    };
    Ok(Snapshot { state, status })
}

fn resume_snapshot(value: &Value) -> Decode<ResumeSnapshot> {
    fields(value, &["source", "position_ms", "speed_permille"])?;
    Ok(ResumeSnapshot {
        source: source(&value["source"])?,
        position_ms: u32_value(&value["position_ms"])?,
        speed_permille: speed(&value["speed_permille"])?,
    })
}

fn event(value: &Value) -> Decode<Event> {
    let tag = string(&value["type"])?;
    match tag {
        "play_requested" | "pause_requested" | "interruption_began" => {
            fields(value, &["type"])?;
            match tag {
                "play_requested" => Ok(Event::PlayRequested),
                "pause_requested" => Ok(Event::PauseRequested),
                _ => Ok(Event::InterruptionBegan),
            }
        }
        "source_selected" => {
            fields(value, &["type", "source", "position_ms"])?;
            Ok(Event::SourceSelected {
                source: source(&value["source"])?,
                position_ms: u32_value(&value["position_ms"])?,
            })
        }
        "restore_requested" => {
            fields(value, &["type", "snapshot"])?;
            Ok(Event::RestoreRequested {
                snapshot: resume_snapshot(&value["snapshot"])?,
            })
        }
        "seek_requested" => {
            fields(value, &["type", "position_ms"])?;
            Ok(Event::SeekRequested {
                position_ms: u32_value(&value["position_ms"])?,
            })
        }
        "skip_requested" => {
            fields(value, &["type", "delta_ms"])?;
            let delta = value["delta_ms"]
                .as_i64()
                .ok_or("expected signed integer")?;
            Ok(Event::SkipRequested {
                delta_ms: i32::try_from(delta).map_err(|_| "signed integer out of bounds")?,
            })
        }
        "speed_requested" => {
            fields(value, &["type", "speed_permille"])?;
            Ok(Event::SpeedRequested {
                speed_permille: u16_value(&value["speed_permille"])?,
            })
        }
        "interruption_ended" => {
            fields(value, &["type", "resume_allowed"])?;
            Ok(Event::InterruptionEnded {
                resume_allowed: value["resume_allowed"]
                    .as_bool()
                    .ok_or("expected boolean")?,
            })
        }
        "engine_ready" | "engine_started" | "engine_paused" | "engine_buffering"
        | "engine_ended" => {
            fields(value, &["type", "identity"])?;
            let identity = identity(&value["identity"])?;
            match tag {
                "engine_ready" => Ok(Event::EngineReady { identity }),
                "engine_started" => Ok(Event::EngineStarted { identity }),
                "engine_paused" => Ok(Event::EnginePaused { identity }),
                "engine_buffering" => Ok(Event::EngineBuffering { identity }),
                _ => Ok(Event::EngineEnded { identity }),
            }
        }
        "engine_position" => {
            fields(value, &["type", "identity", "position_ms"])?;
            Ok(Event::EnginePosition {
                identity: identity(&value["identity"])?,
                position_ms: u32_value(&value["position_ms"])?,
            })
        }
        _ => Err("unknown event type"),
    }
}

fn rejection(value: &Value) -> Decode<Rejection> {
    match string(value)? {
        "no_session" => Ok(Rejection::NoSession),
        "interrupted" => Ok(Rejection::Interrupted),
        "out_of_bounds" => Ok(Rejection::OutOfBounds),
        "unsupported_speed" => Ok(Rejection::UnsupportedSpeed),
        "generation_exhausted" => Ok(Rejection::GenerationExhausted),
        "invalid_source" => Ok(Rejection::InvalidSource),
        _ => Err("unknown rejection reason"),
    }
}

fn effect(value: &Value) -> Decode<Effect> {
    let tag = string(&value["type"])?;
    match tag {
        "prepare_engine" | "start_engine" | "pause_engine" | "seek_engine" | "set_speed" => {
            fields(
                value,
                &[
                    "type",
                    "identity",
                    "source",
                    "position_ms",
                    "speed_permille",
                ],
            )?;
            let command = EngineCommand {
                identity: identity(&value["identity"])?,
                source: source(&value["source"])?,
                position_ms: u32_value(&value["position_ms"])?,
                speed_permille: speed(&value["speed_permille"])?,
            };
            match tag {
                "prepare_engine" => Ok(Effect::PrepareEngine(command)),
                "start_engine" => Ok(Effect::StartEngine(command)),
                "pause_engine" => Ok(Effect::PauseEngine(command)),
                "seek_engine" => Ok(Effect::SeekEngine(command)),
                _ => Ok(Effect::SetSpeed(command)),
            }
        }
        "offer_resume" => {
            fields(value, &["type", "source", "position_ms"])?;
            Ok(Effect::OfferResume {
                source: source(&value["source"])?,
                position_ms: u32_value(&value["position_ms"])?,
            })
        }
        "rejected" => {
            fields(value, &["type", "reason"])?;
            Ok(Effect::Rejected(rejection(&value["reason"])?))
        }
        _ => Err("unknown effect type"),
    }
}

fn version(value: &Value) -> Decode<()> {
    if value["vector_format"].as_u64() != Some(1) {
        return Err("unknown vector format");
    }
    if string(&value["contract"])? != CONTRACT {
        return Err("unknown playback contract");
    }
    Ok(())
}

fn decode_vector(value: &Value) -> Decode<Vector> {
    fields(
        value,
        &["vector_format", "contract", "id", "initial_state", "steps"],
    )?;
    version(value)?;
    let id = string(&value["id"])?;
    if id.is_empty() {
        return Err("empty vector id");
    }
    if !VECTORS
        .iter()
        .any(|vector| vector.name.strip_suffix(".json") == Some(id))
    {
        return Err("unknown vector id");
    }
    let steps = array(&value["steps"])?;
    if steps.is_empty() {
        return Err("empty vector steps");
    }
    let steps = steps
        .iter()
        .map(|value| {
            fields(value, &["event", "expected"])?;
            let expected = &value["expected"];
            fields(expected, &["state", "effects"])?;
            Ok(Step {
                event: event(&value["event"])?,
                state: snapshot(&expected["state"])?,
                effects: array(&expected["effects"])?
                    .iter()
                    .map(effect)
                    .collect::<Decode<Vec<_>>>()?,
            })
        })
        .collect::<Decode<Vec<_>>>()?;
    Ok(Vector {
        id: id.to_owned(),
        initial_state: snapshot(&value["initial_state"])?,
        steps,
    })
}

fn decode_index(value: &Value) -> Decode<Vec<(String, String)>> {
    fields(value, &["vector_format", "contract", "vectors"])?;
    version(value)?;
    let entries = array(&value["vectors"])?;
    if entries.is_empty() {
        return Err("empty vector inventory");
    }
    let mut decoded: Vec<(String, String)> = Vec::new();
    for entry in entries {
        fields(entry, &["file", "sha256"])?;
        let file = string(&entry["file"])?;
        let sha256 = string(&entry["sha256"])?;
        if !VECTORS.iter().any(|vector| vector.name == file) {
            return Err("unknown vector file");
        }
        if decoded.iter().any(|(existing, _)| existing == file) {
            return Err("duplicate vector file");
        }
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("invalid sha256");
        }
        decoded.push((file.to_owned(), sha256.to_owned()));
    }
    if decoded.len() != VECTORS.len() {
        return Err("incomplete vector inventory");
    }
    Ok(decoded)
}

fn fixture(name: &str) -> Value {
    serde_json::from_str(VECTORS.iter().find(|file| file.name == name).unwrap().json).unwrap()
}

fn replay(name: &str) {
    let vector = decode_vector(&fixture(name)).unwrap();
    assert_eq!(format!("{}.json", vector.id), name);
    assert_eq!(
        Snapshot::of(vector.initial_state.state.clone()),
        vector.initial_state,
        "{name}: initial state projection"
    );
    let mut state = vector.initial_state.state;
    for (index, step) in vector.steps.into_iter().enumerate() {
        let transition = reduce(state, step.event);
        assert_eq!(
            Snapshot::of(transition.state.clone()),
            step.state,
            "{name}#{index}: complete state"
        );
        assert_eq!(
            transition.effects, step.effects,
            "{name}#{index}: ordered effects"
        );
        state = transition.state;
    }
}

macro_rules! vector_test {
    ($test:ident, $file:literal) => {
        #[test]
        fn $test() {
            replay($file);
        }
    };
}

vector_test!(play_pause_matches_the_canonical_vector, "play-pause.json");
vector_test!(buffering_matches_the_canonical_vector, "buffering.json");
vector_test!(
    interruption_matches_the_canonical_vector,
    "interruption.json"
);
vector_test!(
    stale_response_matches_the_canonical_vector,
    "stale-response.json"
);
vector_test!(
    restore_session_matches_the_canonical_vector,
    "restore-session.json"
);
vector_test!(
    preparing_races_matches_the_canonical_vector,
    "preparing-races.json"
);
vector_test!(
    invalid_controls_matches_the_canonical_vector,
    "invalid-controls.json"
);

#[test]
fn reviewed_index_hashes_and_embedded_bytes_have_no_unnoticed_drift() {
    let index: Value = serde_json::from_str(INDEX).unwrap();
    let entries = decode_index(&index).unwrap();
    let expected = VECTORS
        .iter()
        .map(|file| (file.name.to_owned(), file.sha256.to_owned()))
        .collect::<Vec<_>>();
    assert_eq!(entries, expected);
    let mut step_count = 0;
    for file in &VECTORS {
        let fingerprint = file
            .json
            .bytes()
            .fold(0xcbf29ce484222325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
            });
        assert_eq!(
            fingerprint, file.byte_fingerprint,
            "{}: embedded bytes changed",
            file.name
        );
        step_count += decode_vector(&fixture(file.name)).unwrap().steps.len();
    }
    assert_eq!(step_count, 51);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn every_repository_vector_is_listed_and_embedded() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../packages/contracts/fixtures/theatre/playback/v1");
    let mut actual = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.ends_with(".json"))
        .collect::<Vec<_>>();
    let mut expected = VECTORS
        .iter()
        .map(|file| file.name.to_owned())
        .collect::<Vec<_>>();
    expected.push("index.json".to_owned());
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn vector_decoder_rejects_unknown_versions_and_empty_scenarios() {
    for (field, replacement, error) in [
        ("vector_format", Value::from(2), "unknown vector format"),
        (
            "contract",
            Value::from("theatre-mock-playback@2"),
            "unknown playback contract",
        ),
        ("id", Value::from(""), "empty vector id"),
        ("id", Value::from("unlisted"), "unknown vector id"),
        ("steps", Value::Array(vec![]), "empty vector steps"),
    ] {
        let mut value = fixture("play-pause.json");
        value[field] = replacement;
        assert_eq!(decode_vector(&value).unwrap_err(), error);
    }
}

#[test]
fn vector_decoder_rejects_unknown_tags_at_every_semantic_level() {
    for (pointer, error) in [
        ("/steps/0/event/type", "unknown event type"),
        ("/steps/0/expected/effects/0/type", "unknown effect type"),
        ("/initial_state/status", "unknown status"),
        ("/initial_state/pause_cause", "unknown pause cause"),
        ("/initial_state/session/intent/type", "unknown intent type"),
        ("/initial_state/session/intent/cause", "unknown pause cause"),
        ("/initial_state/session/engine", "unknown engine phase"),
        (
            "/initial_state/session/interruption",
            "unknown interruption phase",
        ),
        ("/initial_state/session/source/work_id", "unknown work key"),
    ] {
        let mut value = fixture("play-pause.json");
        *value.pointer_mut(pointer).unwrap() = Value::from("future");
        assert_eq!(decode_vector(&value).unwrap_err(), error, "{pointer}");
    }
    assert_eq!(
        effect(&serde_json::json!({"type": "rejected", "reason": "future"})),
        Err("unknown rejection reason")
    );
}

#[test]
fn vector_decoder_rejects_unknown_fields_in_every_nested_shape() {
    for pointer in [
        "",
        "/initial_state",
        "/initial_state/session",
        "/initial_state/session/source",
        "/initial_state/session/intent",
        "/steps/0",
        "/steps/0/event",
        "/steps/0/expected",
        "/steps/0/expected/state",
        "/steps/0/expected/effects/0",
        "/steps/0/expected/effects/0/identity",
        "/steps/0/expected/effects/0/source",
    ] {
        let mut value = fixture("play-pause.json");
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("future".to_owned(), Value::Null);
        assert_eq!(
            decode_vector(&value).unwrap_err(),
            "unexpected object fields",
            "{pointer}"
        );
    }
    let mut restore = fixture("restore-session.json");
    restore["steps"][0]["event"]["snapshot"]
        .as_object_mut()
        .unwrap()
        .insert("future".to_owned(), Value::Null);
    assert_eq!(
        decode_vector(&restore).unwrap_err(),
        "unexpected object fields"
    );
}

#[test]
fn vector_decoder_rejects_missing_fields_and_invalid_integer_bounds() {
    let mut missing = fixture("play-pause.json");
    missing["initial_state"]
        .as_object_mut()
        .unwrap()
        .remove("session");
    assert_eq!(
        decode_vector(&missing).unwrap_err(),
        "unexpected object fields"
    );
    for (pointer, replacement, error) in [
        (
            "/initial_state/generation",
            Value::from(4_294_967_296_u64),
            "unsigned integer out of bounds",
        ),
        (
            "/initial_state/session/source/episode_index",
            Value::from(65_536),
            "unsigned integer out of bounds",
        ),
        (
            "/initial_state/session/source/duration_ms",
            Value::from(0),
            "invalid source",
        ),
        (
            "/initial_state/session/position_ms",
            Value::from(u32::MAX),
            "snapshot position out of bounds",
        ),
        (
            "/initial_state/session/speed_permille",
            Value::from(999),
            "unsupported snapshot speed",
        ),
        (
            "/initial_state/generation",
            Value::from(-1),
            "expected unsigned integer",
        ),
        (
            "/initial_state/generation",
            Value::from(1.5),
            "expected unsigned integer",
        ),
    ] {
        let mut value = fixture("play-pause.json");
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(decode_vector(&value).unwrap_err(), error, "{pointer}");
    }
    assert_eq!(
        event(&serde_json::json!({"type": "skip_requested", "delta_ms": 2_147_483_648_u64})),
        Err("signed integer out of bounds")
    );
    assert_eq!(
        event(&serde_json::json!({"type": "interruption_ended", "resume_allowed": 1})),
        Err("expected boolean")
    );
}

#[test]
fn inventory_decoder_rejects_unlisted_duplicate_missing_and_unknown_fields() {
    for (pointer, replacement, error) in [
        ("/vector_format", Value::from(2), "unknown vector format"),
        (
            "/contract",
            Value::from("theatre-mock-playback@2"),
            "unknown playback contract",
        ),
        (
            "/vectors/0/file",
            Value::from("unlisted.json"),
            "unknown vector file",
        ),
        (
            "/vectors/0/sha256",
            Value::from("not-a-digest"),
            "invalid sha256",
        ),
    ] {
        let mut value: Value = serde_json::from_str(INDEX).unwrap();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(decode_index(&value), Err(error));
    }
    let mut duplicate: Value = serde_json::from_str(INDEX).unwrap();
    duplicate["vectors"][1] = duplicate["vectors"][0].clone();
    assert_eq!(decode_index(&duplicate), Err("duplicate vector file"));
    let mut missing: Value = serde_json::from_str(INDEX).unwrap();
    missing["vectors"].as_array_mut().unwrap().pop();
    assert_eq!(decode_index(&missing), Err("incomplete vector inventory"));
    for pointer in ["", "/vectors/0"] {
        let mut value: Value = serde_json::from_str(INDEX).unwrap();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("future".to_owned(), Value::Null);
        assert_eq!(decode_index(&value), Err("unexpected object fields"));
    }
}
