//! Google adapter core against frozen production documents, the shared receipt audit and a
//! scripted transport. Every provider response here is synthetic; none of these tests reaches
//! Google.
use std::{future::Future, sync::Mutex};

use base64::Engine as _;
use cantos_api::{
    CharacterCasting, FrozenProductionDocument, ProductionBudget, ProductionCurrency,
    ProductionSettings, ProductionSettingsResponse, ResolvedSynthesisInput, RevisionResponse,
    SynthesisPerformance,
};
use cantos_server::production::prepare_inputs;
use cantos_server::script_ir::read_script;
use cantos_server::tts::attempt::{
    ContractCapability, DeliveryState, FailureClass, ReconciliationCapabilities, RecoveryPlan,
};
use cantos_server::tts::audio::AudioError;
use cantos_server::tts::google::{
    classify, decode_audio, observation, plan, AudioDefect, FailureEvidence, GoogleFailure,
    GoogleOutcome, GooglePlan, LostCause, PlanFinding, ResponseBody, TransportObservation,
    LIMITATIONS, MAX_RESPONSE_BYTES,
};
use cantos_server::tts::google_contract::{VoiceFamily, MAX_TEXT_BYTES, VI_VN_VOICES};
use cantos_server::tts::google_http::{synthesize_once, GoogleTransport};
use cantos_server::tts::{
    audit_receipt, ChargeObservation, ReceiptError, ReceiptEvidence, ReceiptOutcome,
    TtsProviderIdentity, TtsReceipt, UsageObservation, RECEIPT_VERSION,
};
use cantos_server::wav_inspection::StructuralError;
use sha2::{Digest, Sha256};

const SCRIPT: &[u8] =
    include_bytes!("../../../contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
const NOW: i64 = 100;
const FIRST_LINE: &str = "Ánh đèn cuối cùng còn sáng trên sân khấu.";

fn revision() -> RevisionResponse {
    let script = read_script(SCRIPT).unwrap();
    let bytes = script.export_bytes();
    let mut hash = Sha256::new();
    hash.update(b"cantos/script-export/e1\n");
    hash.update(&bytes);
    RevisionResponse {
        script_id: "00000000-0000-4000-8000-000000000006".into(),
        revision: 1,
        accepted_by: "editor".into(),
        accepted_at: "2026-10-11T00:00:00Z".into(),
        content_digest: script.content_digest().to_string(),
        export_digest: format!("sir-e1:sha256:{:x}", hash.finalize()),
        script_json: String::from_utf8(bytes).unwrap(),
    }
}

/// Narrator on Neural2 with explicit rate and pitch, An on Chirp 3: HD, Minh on WaveNet; every
/// binding chooses the neutral delivery explicitly.
fn settings() -> ProductionSettingsResponse {
    let casting = [
        ("narrator", "google-neural2", "vi-VN-Neural2-A", 1_250, -2),
        ("an", "google-chirp3-hd", "vi-VN-Chirp3-HD-Aoede", 1_000, 0),
        ("minh", "google-wavenet", "vi-VN-Wavenet-D", 1_000, 0),
    ];
    ProductionSettingsResponse {
        script_id: revision().script_id,
        version: 1,
        script_revision: 1,
        operation_id: "00000000-0000-4000-8000-000000000011".into(),
        recorded_by: "editor".into(),
        recorded_at: "2026-10-11T00:01:00Z".into(),
        settings: ProductionSettings {
            bindings: casting
                .into_iter()
                .map(|(character, model, voice, rate, pitch)| CharacterCasting {
                    character_id: character.into(),
                    provider_id: "google-cloud-tts".into(),
                    model_id: model.into(),
                    voice_id: voice.into(),
                    language: "vi-VN".into(),
                    voice_rights_record_id: format!("rights-voice-{character}"),
                    performance: SynthesisPerformance {
                        rate_permille: rate,
                        pitch_semitones: pitch,
                        emotion: Some("neutral".into()),
                        intensity_permille: Some(0),
                    },
                    pronunciation: vec![],
                })
                .collect(),
            budget: ProductionBudget {
                currency: ProductionCurrency::USD,
                limit_minor: 10,
                scope: "fixture-private-scene".into(),
                territory: "private-planning".into(),
                rate: None,
            },
        },
    }
}

fn document() -> FrozenProductionDocument {
    prepare_inputs("owner", &revision(), &settings(), &[], true, NOW)
        .unwrap()
        .document()
}

fn line(index: usize) -> ResolvedSynthesisInput {
    document().resolved[index].clone()
}

fn narrator_plan() -> GooglePlan {
    plan(&line(0)).unwrap()
}

/// Canonical 44-byte RIFF/WAVE PCM header followed by a deterministic ramp.
fn wav(format_tag: u16, channels: u16, rate: u32, bits: u16, frames: u32) -> Vec<u8> {
    let block_align = channels * bits / 8;
    let data_bytes = frames * u32::from(block_align);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&format_tag.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(block_align)).to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_bytes.to_le_bytes());
    bytes.extend((0..data_bytes).map(|i| (i % 251) as u8));
    bytes
}

fn envelope(audio: &[u8]) -> Vec<u8> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(audio);
    format!(r#"{{"audioContent":"{encoded}"}}"#).into_bytes()
}

fn ok(body: Vec<u8>) -> TransportObservation {
    TransportObservation::Responded {
        status: 200,
        body: ResponseBody::Complete(body),
    }
}

fn status(code: u16, body: &str) -> TransportObservation {
    TransportObservation::Responded {
        status: code,
        body: ResponseBody::Complete(body.as_bytes().to_vec()),
    }
}

fn error_body(code: u16, name: &str) -> String {
    format!(
        r#"{{"error":{{"code":{code},"message":"{FIRST_LINE} could not be processed","status":"{name}","details":[]}}}}"#
    )
}

fn failed(outcome: GoogleOutcome) -> GoogleFailure {
    match outcome {
        GoogleOutcome::Failure(failure) => failure,
        GoogleOutcome::Output(audio) => panic!("unexpected output: {:?}", audio.inspected()),
    }
}

fn defect(body: Vec<u8>) -> AudioDefect {
    decode_audio(ResponseBody::Complete(body)).unwrap_err()
}

fn findings(input: &ResolvedSynthesisInput) -> Vec<PlanFinding> {
    plan(input).unwrap_err()
}

#[test]
fn declared_vi_vn_voices_are_unique_and_named_for_their_family() {
    assert_eq!(VI_VN_VOICES.len(), 40);
    let mut names: Vec<_> = VI_VN_VOICES.iter().map(|voice| voice.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 40);
    for voice in &VI_VN_VOICES {
        let segment = voice.name.strip_prefix("vi-VN-").unwrap();
        let expected = match segment.rsplit_once('-').unwrap().0 {
            "Chirp3-HD" => VoiceFamily::Chirp3Hd,
            "Neural2" => VoiceFamily::Neural2,
            "Standard" => VoiceFamily::Standard,
            "Wavenet" => VoiceFamily::Wavenet,
            other => panic!("unknown family {other}"),
        };
        assert_eq!(voice.family, expected, "{}", voice.name);
    }
    let count = |family| VI_VN_VOICES.iter().filter(|v| v.family == family).count();
    assert_eq!(VoiceFamily::ALL.map(count), [30, 2, 4, 4]);
}

#[test]
fn frozen_lines_map_to_exact_request_bodies_and_digests() {
    // Expected bytes and digests were computed independently with Python's json and hashlib.
    let narrator = narrator_plan();
    assert_eq!(
        std::str::from_utf8(narrator.body()).unwrap(),
        r#"{"audioConfig":{"audioEncoding":"LINEAR16","pitch":-2.0,"sampleRateHertz":24000,"speakingRate":1.25},"input":{"text":"Ánh đèn cuối cùng còn sáng trên sân khấu."},"voice":{"languageCode":"vi-VN","name":"vi-VN-Neural2-A"}}"#
    );
    assert_eq!(
        narrator.request_digest(),
        "google-tts-request-1:sha256:d842d03f8cd49f583ceb54b0630a0ee653157320e925f5d1ff9e94ed4da47b4a"
    );
    // Chirp 3: HD has no rate or pitch parameter, so neither is sent.
    let an = plan(&line(1)).unwrap();
    assert_eq!(
        std::str::from_utf8(an.body()).unwrap(),
        r#"{"audioConfig":{"audioEncoding":"LINEAR16","sampleRateHertz":24000},"input":{"text":"Ngày mai, mình có diễn tiếp không?"},"voice":{"languageCode":"vi-VN","name":"vi-VN-Chirp3-HD-Aoede"}}"#
    );
    assert_eq!(
        an.request_digest(),
        "google-tts-request-1:sha256:f565fd193009f1a80ad77370954d5ce91afbbb165967034cb91298f62d090702"
    );
    for resolved in &document().resolved {
        assert!(plan(resolved).is_ok(), "{}", resolved.dialogue_id);
    }
}

#[test]
fn a_vi_script_is_sent_with_googles_vi_vn_locale() {
    let mut input = line(0);
    input.language = "vi".into();
    assert_eq!(plan(&input).unwrap().body(), narrator_plan().body());
}

#[test]
fn equal_lines_plan_identically_and_any_effective_change_changes_the_digest() {
    assert_eq!(plan(&line(0)).unwrap(), narrator_plan());
    let base = narrator_plan().request_digest().to_owned();
    let changes: [fn(&mut ResolvedSynthesisInput); 5] = [
        |input| input.text.push('!'),
        |input| input.voice_id = "vi-VN-Neural2-D".into(),
        |input| {
            input.voice_id = "vi-VN-Wavenet-A".into();
            input.model_id = "google-wavenet".into();
        },
        |input| input.performance.rate_permille = 1_251,
        |input| input.performance.pitch_semitones = -1,
    ];
    for change in changes {
        let mut input = line(0);
        change(&mut input);
        assert_ne!(plan(&input).unwrap().request_digest(), base, "{input:?}");
    }
}

#[test]
fn estimate_is_list_price_rounded_up_per_request_and_never_a_charge() {
    let estimate = narrator_plan().estimate().clone();
    assert_eq!(estimate.billable_characters, 41);
    assert_eq!(estimate.amount_minor, 1);
    assert_eq!(estimate.currency, "USD");
    assert_eq!(estimate.cents_per_million_characters, 1_600);
    assert_eq!(
        estimate.basis,
        "list_price_before_free_tier_rounded_up_per_request"
    );
    assert_eq!(estimate.price_observed_on, "2026-10-11");
    let mut chirp = line(1);
    chirp.text = "a".repeat(1_667);
    assert_eq!(plan(&chirp).unwrap().estimate().amount_minor, 6);
    chirp.text = "a".repeat(MAX_TEXT_BYTES);
    assert_eq!(plan(&chirp).unwrap().estimate().amount_minor, 15);
    // 2,500 characters at 400 cents per million is exactly one cent; one more rounds up.
    let mut wavenet = line(2);
    wavenet.text = "a".repeat(2_500);
    assert_eq!(plan(&wavenet).unwrap().estimate().amount_minor, 1);
    wavenet.text.push('a');
    assert_eq!(plan(&wavenet).unwrap().estimate().amount_minor, 2);
}

#[test]
fn every_unsupported_input_is_an_exact_blocking_finding() {
    type Change = fn(&mut ResolvedSynthesisInput);
    let cases: Vec<(usize, Change, Vec<PlanFinding>)> = vec![
        (
            0,
            |i| i.language = "en-US".into(),
            vec![PlanFinding::UnsupportedLanguage {
                found: "en-US".into(),
            }],
        ),
        (
            0,
            |i| i.voice_id = "vi-VN-Neural2-Z".into(),
            vec![PlanFinding::UnsupportedVoice {
                voice_id: "vi-VN-Neural2-Z".into(),
            }],
        ),
        (
            0,
            |i| i.model_id = "google-wavenet".into(),
            vec![PlanFinding::ModelMismatch {
                voice_id: "vi-VN-Neural2-A".into(),
                expected: "google-neural2",
                found: "google-wavenet".into(),
            }],
        ),
        (
            0,
            |i| i.performance.rate_permille = 249,
            vec![PlanFinding::RateUnsupported {
                requested_permille: 249,
                min_permille: 250,
                max_permille: 2_000,
            }],
        ),
        (
            0,
            |i| i.performance.pitch_semitones = 21,
            vec![PlanFinding::PitchUnsupported {
                requested_semitones: 21,
                min_semitones: -20,
                max_semitones: 20,
            }],
        ),
        (
            1,
            |i| {
                i.performance.rate_permille = 1_100;
                i.performance.pitch_semitones = -1;
            },
            vec![
                PlanFinding::RateUnsupported {
                    requested_permille: 1_100,
                    min_permille: 1_000,
                    max_permille: 1_000,
                },
                PlanFinding::PitchUnsupported {
                    requested_semitones: -1,
                    min_semitones: 0,
                    max_semitones: 0,
                },
            ],
        ),
        (
            0,
            |i| i.performance.emotion = Some("hopeful".into()),
            vec![PlanFinding::EmotionUnsupported {
                requested: "hopeful".into(),
            }],
        ),
        (
            0,
            |i| i.performance.intensity_permille = Some(600),
            vec![PlanFinding::IntensityUnsupported {
                requested_permille: 600,
            }],
        ),
        (
            0,
            |i| i.text = " \u{3000} ".into(),
            vec![PlanFinding::InvalidText],
        ),
        (
            0,
            |i| i.text = "Chuông\u{7}reo".into(),
            vec![PlanFinding::InvalidText],
        ),
        (
            0,
            |i| i.text = "Chuông\u{2028}reo".into(),
            vec![PlanFinding::InvalidText],
        ),
        (
            0,
            |i| i.text = "A\u{301}nh đèn".into(),
            vec![PlanFinding::InvalidText],
        ),
        (
            0,
            |i| i.text = "ấ".repeat(1_667),
            vec![PlanFinding::TextTooLong {
                bytes: 5_001,
                limit_bytes: 5_000,
            }],
        ),
    ];
    for (index, change, expected) in cases {
        let mut input = line(index);
        change(&mut input);
        assert_eq!(findings(&input), expected, "{input:?}");
    }
    let mut at_limit = line(0);
    at_limit.text = format!("{}ab", "ấ".repeat(1_666));
    assert_eq!(at_limit.text.len(), MAX_TEXT_BYTES);
    assert!(plan(&at_limit).is_ok());
}

#[test]
fn a_line_frozen_for_another_adapter_is_refused_not_reinterpreted() {
    let mut input = line(0);
    input.provider_id = "reference-only".into();
    input.adapter_version = "reference-only-no-dispatch-v1".into();
    input.output_contract = "reference-only/no-audio".into();
    input.performance.emotion = Some("calm".into());
    input.performance.intensity_permille = Some(300);
    assert_eq!(
        findings(&input),
        vec![
            PlanFinding::ProviderMismatch {
                found: "reference-only".into()
            },
            PlanFinding::AdapterVersionMismatch {
                found: "reference-only-no-dispatch-v1".into()
            },
            PlanFinding::OutputContractMismatch {
                found: "reference-only/no-audio".into()
            },
            PlanFinding::EmotionUnsupported {
                requested: "calm".into()
            },
            PlanFinding::IntensityUnsupported {
                requested_permille: 300
            },
        ]
    );
    let mut previous = line(0);
    previous.adapter_version = "google-cloud-tts-synthesize-1".into();
    assert_eq!(
        findings(&previous),
        vec![PlanFinding::AdapterVersionMismatch {
            found: "google-cloud-tts-synthesize-1".into()
        }]
    );
}

#[test]
fn every_observation_maps_to_the_shared_receipt_vocabulary() {
    use DeliveryState::{MayHaveExecuted, NotSent};
    use FailureClass as F;
    let with_status = |http_status: u16, name: Option<&str>| FailureEvidence::Status {
        http_status,
        provider_status: name.map(str::to_owned),
    };
    let cases = vec![
        (
            TransportObservation::NotSent,
            F::Timeout,
            NotSent,
            FailureEvidence::NotConnected,
        ),
        (
            TransportObservation::LostAfterSend(LostCause::Timeout),
            F::Timeout,
            MayHaveExecuted,
            FailureEvidence::Lost {
                cause: LostCause::Timeout,
            },
        ),
        (
            TransportObservation::LostAfterSend(LostCause::ConnectionError),
            F::Permanent,
            MayHaveExecuted,
            FailureEvidence::Lost {
                cause: LostCause::ConnectionError,
            },
        ),
        (
            status(400, &error_body(400, "INVALID_ARGUMENT")),
            F::Permanent,
            MayHaveExecuted,
            with_status(400, Some("INVALID_ARGUMENT")),
        ),
        (
            status(401, &error_body(401, "UNAUTHENTICATED")),
            F::Permanent,
            MayHaveExecuted,
            with_status(401, Some("UNAUTHENTICATED")),
        ),
        (
            status(403, &error_body(403, "PERMISSION_DENIED")),
            F::Permanent,
            MayHaveExecuted,
            with_status(403, Some("PERMISSION_DENIED")),
        ),
        (
            status(429, &error_body(429, "RESOURCE_EXHAUSTED")),
            F::RateLimit,
            MayHaveExecuted,
            with_status(429, Some("RESOURCE_EXHAUSTED")),
        ),
        (
            status(500, &error_body(500, "INTERNAL")),
            F::Permanent,
            MayHaveExecuted,
            with_status(500, Some("INTERNAL")),
        ),
        (
            status(503, &error_body(503, "UNAVAILABLE")),
            F::Permanent,
            MayHaveExecuted,
            with_status(503, Some("UNAVAILABLE")),
        ),
        (
            status(504, &error_body(504, "DEADLINE_EXCEEDED")),
            F::Timeout,
            MayHaveExecuted,
            with_status(504, Some("DEADLINE_EXCEEDED")),
        ),
        (
            status(302, ""),
            F::Permanent,
            MayHaveExecuted,
            with_status(302, None),
        ),
        (
            TransportObservation::Responded {
                status: 502,
                body: ResponseBody::Interrupted,
            },
            F::Permanent,
            MayHaveExecuted,
            with_status(502, None),
        ),
        (
            ok(b"{}".to_vec()),
            F::MalformedAudio,
            MayHaveExecuted,
            FailureEvidence::Audio {
                http_status: 200,
                defect: AudioDefect::MissingAudioContent,
            },
        ),
    ];
    for (observation, class, delivery, evidence) in cases {
        let label = format!("{observation:?}");
        assert_eq!(
            failed(classify(observation)),
            GoogleFailure {
                class,
                delivery,
                evidence
            },
            "{label}"
        );
    }
}

#[test]
fn provider_status_keeps_only_canonical_names() {
    for (body, expected) in [
        (
            r#"{"error":{"status":"INVALID_ARGUMENT"}}"#,
            Some("INVALID_ARGUMENT"),
        ),
        (r#"{"error":{"status":"invalid argument"}}"#, None),
        (r#"{"error":{"status":""}}"#, None),
        (r#"{"error":{"message":"no status"}}"#, None),
        ("not json", None),
    ] {
        assert_eq!(
            failed(classify(status(400, body))).evidence,
            FailureEvidence::Status {
                http_status: 400,
                provider_status: expected.map(str::to_owned),
            },
            "{body}"
        );
    }
    let long = format!(r#"{{"error":{{"status":"{}"}}}}"#, "A".repeat(65));
    assert_eq!(
        failed(classify(status(400, &long))).evidence,
        FailureEvidence::Status {
            http_status: 400,
            provider_status: None,
        }
    );
}

#[test]
fn valid_mono_24khz_pcm_is_admitted_with_exact_facts() {
    let audio = wav(1, 1, 24_000, 16, 2_400);
    let GoogleOutcome::Output(admitted) = classify(ok(envelope(&audio))) else {
        panic!("expected output");
    };
    let inspected = admitted.inspected();
    assert_eq!(
        inspected.file_sha256(),
        format!("{:x}", Sha256::digest(&audio))
    );
    assert_eq!(
        (
            inspected.sample_rate_hz(),
            inspected.channels(),
            inspected.bits_per_sample(),
            inspected.frames()
        ),
        (24_000, 1, 16, 2_400)
    );
    assert_eq!(
        inspected.output_profile_version(),
        "cantos-google-tts-pcm-1"
    );
    assert_eq!(admitted.bytes(), audio.as_slice());
}

#[test]
fn malformed_or_unexpected_audio_is_rejected_with_its_exact_defect() {
    assert_eq!(defect(b"<html>".to_vec()), AudioDefect::NotJson);
    assert_eq!(defect(b"{}".to_vec()), AudioDefect::MissingAudioContent);
    assert_eq!(
        defect(br#"{"audioContent":null}"#.to_vec()),
        AudioDefect::MissingAudioContent
    );
    assert_eq!(
        defect(br#"{"audioContent":""}"#.to_vec()),
        AudioDefect::EmptyAudio
    );
    assert_eq!(
        defect(br#"{"audioContent":"%%%not-base64"}"#.to_vec()),
        AudioDefect::InvalidBase64
    );
    let mut truncated = wav(1, 1, 24_000, 16, 100);
    truncated.truncate(100);
    assert!(matches!(
        defect(envelope(&truncated)),
        AudioDefect::Rejected { error: AudioError::InvalidWav { errors } } if !errors.is_empty()
    ));
    for (tag, channels, rate, bits) in [
        (1, 2, 24_000, 16),
        (1, 1, 48_000, 16),
        (1, 1, 22_050, 16),
        (1, 1, 24_000, 8),
    ] {
        assert_eq!(
            defect(envelope(&wav(tag, channels, rate, bits, 10))),
            AudioDefect::Rejected {
                error: AudioError::UnsupportedProfile {
                    format_tag: tag,
                    sample_rate_hz: rate,
                    channels,
                    bits_per_sample: bits,
                }
            }
        );
    }
    // The inspector admits integer PCM only, so float audio is structurally refused.
    assert_eq!(
        defect(envelope(&wav(3, 1, 24_000, 32, 10))),
        AudioDefect::Rejected {
            error: AudioError::InvalidWav {
                errors: vec![StructuralError::UnsupportedFormatTag { format_tag: 3 }],
            }
        }
    );
    assert_eq!(
        defect(envelope(&wav(1, 1, 24_000, 16, 0))),
        AudioDefect::Rejected {
            error: AudioError::EmptyFrames
        }
    );
    assert_eq!(
        defect(envelope(&wav(1, 1, 24_000, 16, 2_880_001))),
        AudioDefect::Rejected {
            error: AudioError::DurationExceeded {
                frames: 2_880_001,
                maximum_frames: 2_880_000,
            }
        }
    );
    assert_eq!(
        decode_audio(ResponseBody::TooLarge).unwrap_err(),
        AudioDefect::ResponseTooLarge {
            limit_bytes: MAX_RESPONSE_BYTES
        }
    );
    assert_eq!(
        decode_audio(ResponseBody::Interrupted).unwrap_err(),
        AudioDefect::ResponseInterrupted
    );
}

/// A labeled fake with its own ledger of provider-side executions. It can execute a request
/// and then lose the response, which a real provider can do and a test cannot otherwise force.
struct ScriptedTransport {
    executes: bool,
    reply: TransportObservation,
    executed: Mutex<Vec<String>>,
}

impl ScriptedTransport {
    fn new(executes: bool, reply: TransportObservation) -> ScriptedTransport {
        ScriptedTransport {
            executes,
            reply,
            executed: Mutex::new(Vec::new()),
        }
    }

    fn executions(&self) -> Vec<String> {
        self.executed.lock().unwrap().clone()
    }
}

impl GoogleTransport for ScriptedTransport {
    fn send(&self, plan: &GooglePlan) -> impl Future<Output = TransportObservation> + Send {
        if self.executes {
            self.executed
                .lock()
                .unwrap()
                .push(plan.request_digest().to_owned());
        }
        let reply = self.reply.clone();
        async move { reply }
    }
}

fn receipt(outcome: ReceiptOutcome) -> TtsReceipt {
    let document = document();
    let line = &document.resolved[0];
    TtsReceipt {
        version: RECEIPT_VERSION.into(),
        evidence: ReceiptEvidence::CallerReported,
        attempt_id: "attempt-1".into(),
        operation_id: "operation-1".into(),
        provider_request_id: None,
        idempotency_key: None,
        input_digest: document.input_digest.clone(),
        dialogue_id: line.dialogue_id.clone(),
        provider: TtsProviderIdentity {
            provider_id: line.provider_id.clone(),
            model_id: line.model_id.clone(),
            voice_id: line.voice_id.clone(),
            adapter_version: line.adapter_version.clone(),
        },
        reconciliation: ReconciliationCapabilities::new(
            ContractCapability::Unavailable,
            ContractCapability::Unavailable,
            ContractCapability::Unavailable,
        ),
        usage: UsageObservation::Unknown {},
        charge: ChargeObservation::Unknown {},
        outcome,
    }
}

fn failure_receipt(failure: &GoogleFailure) -> TtsReceipt {
    receipt(ReceiptOutcome::Failure {
        class: failure.class,
        delivery: failure.delivery,
    })
}

#[tokio::test]
async fn a_response_lost_after_execution_is_recorded_once_and_needs_reconciliation() {
    let transport = ScriptedTransport::new(
        true,
        TransportObservation::LostAfterSend(LostCause::Timeout),
    );
    let plan = narrator_plan();
    let failure = failed(synthesize_once(&transport, &plan).await);
    assert_eq!(
        transport.executions(),
        vec![plan.request_digest().to_owned()]
    );
    let audit = audit_receipt(&document(), &failure_receipt(&failure), None).unwrap();
    assert_eq!(audit.recovery(), Some(&RecoveryPlan::ManualReconciliation));
}

#[tokio::test]
async fn every_failure_after_a_connection_opened_needs_reconciliation_not_a_resend() {
    for observation in [
        status(429, &error_body(429, "RESOURCE_EXHAUSTED")),
        status(503, &error_body(503, "UNAVAILABLE")),
        status(400, &error_body(400, "INVALID_ARGUMENT")),
        ok(b"{}".to_vec()),
    ] {
        let transport = ScriptedTransport::new(true, observation);
        let failure = failed(synthesize_once(&transport, &narrator_plan()).await);
        assert_eq!(transport.executions().len(), 1);
        let audit = audit_receipt(&document(), &failure_receipt(&failure), None).unwrap();
        assert_eq!(audit.recovery(), Some(&RecoveryPlan::ManualReconciliation));
    }
}

#[tokio::test]
async fn a_connection_that_never_opened_allows_only_a_freshly_authorized_retry() {
    let transport = ScriptedTransport::new(false, TransportObservation::NotSent);
    let failure = failed(synthesize_once(&transport, &narrator_plan()).await);
    assert!(transport.executions().is_empty());
    let audit = audit_receipt(&document(), &failure_receipt(&failure), None).unwrap();
    assert_eq!(
        audit.recovery(),
        Some(&RecoveryPlan::BoundedRetryNeedsFreshAuthorization)
    );
}

#[tokio::test]
async fn admitted_google_audio_passes_the_shared_receipt_audit() {
    let audio = wav(1, 1, 24_000, 16, 480);
    let transport = ScriptedTransport::new(true, ok(envelope(&audio)));
    let GoogleOutcome::Output(admitted) = synthesize_once(&transport, &narrator_plan()).await
    else {
        panic!("expected output");
    };
    let mut output = receipt(ReceiptOutcome::Output {
        audio_sha256: admitted.inspected().file_sha256().into(),
    });
    output.usage = UsageObservation::Reported {
        unit: "effective-unicode-scalars".into(),
        count: 41,
    };
    let audit = audit_receipt(&document(), &output, Some(admitted.bytes())).unwrap();
    assert_eq!(
        audit.audio().unwrap().output_profile_version(),
        "cantos-google-tts-pcm-1"
    );
    // The eSpeak rate is not this line's contract, even with a matching checksum.
    let espeak_rate = wav(1, 1, 22_050, 16, 480);
    let mut mismatched = output.clone();
    mismatched.outcome = ReceiptOutcome::Output {
        audio_sha256: format!("{:x}", Sha256::digest(&espeak_rate)),
    };
    assert!(matches!(
        audit_receipt(&document(), &mismatched, Some(&espeak_rate)).unwrap_err(),
        ReceiptError::InvalidAudio {
            error: AudioError::UnsupportedProfile {
                sample_rate_hz: 22_050,
                ..
            }
        }
    ));
    let mut other_voice = output;
    other_voice.provider.voice_id = "vi-VN-Neural2-D".into();
    assert_eq!(
        audit_receipt(&document(), &other_voice, Some(admitted.bytes())).unwrap_err(),
        ReceiptError::ProviderIdentityMismatch
    );
}

#[tokio::test]
async fn observations_never_contain_dialogue_text_or_provider_messages() {
    for reply in [
        status(400, &error_body(400, "INVALID_ARGUMENT")),
        ok(envelope(&wav(1, 1, 24_000, 16, 48))),
    ] {
        let transport = ScriptedTransport::new(true, reply);
        let plan = narrator_plan();
        let outcome = synthesize_once(&transport, &plan).await;
        let record = observation(&plan, &outcome);
        assert_eq!(record.limitations, LIMITATIONS);
        assert_eq!(record.adapter_version, "google-cloud-tts-synthesize-2");
        let rendered = serde_json::to_string(&record).unwrap();
        assert!(!rendered.contains("Ánh đèn"), "{rendered}");
        assert!(!rendered.contains("could not be processed"), "{rendered}");
        assert!(!rendered.contains("audioContent"), "{rendered}");
    }
}
