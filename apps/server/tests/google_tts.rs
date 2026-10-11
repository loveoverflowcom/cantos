//! Google adapter core and its one-attempt shell, against a scripted transport. Every provider
//! response here is synthetic; none of these tests reaches Google.
use std::{future::Future, sync::Mutex};

use base64::Engine as _;
use cantos_api::{ResolvedSynthesisInput, SynthesisPerformance};
use cantos_server::synthesis::{
    google::{
        classify, decode_audio, plan_line, plan_resolved, policy, record, GooglePlan, PlanFinding,
        SpeechLine, VoiceFamily, ADAPTER_VERSION, LIMITATIONS, MAX_REQUEST_TEXT_BYTES,
        MAX_RESPONSE_BYTES, OUTPUT_CONTRACT, PROVIDER_ID, VI_VN_VOICES,
    },
    google_http::{synthesize_once, GoogleTransport},
    AttemptOutcome, AudioDefect, ChargeExposure, FailureClass, FailureEvidence, FailurePolicy,
    LostCause, RecoveryAction, ResponseBody, RetryDecision, TransportObservation,
};
use cantos_server::wav_inspection::StructuralError;
use sha2::{Digest, Sha256};

const TEXT: &str = "Ánh đèn cuối cùng còn sáng trên sân khấu.";

fn line(voice_id: &str, model_id: &str) -> SpeechLine {
    SpeechLine {
        text: TEXT.into(),
        language: "vi-VN".into(),
        model_id: model_id.into(),
        voice_id: voice_id.into(),
        rate_permille: 1_000,
        pitch_semitones: 0,
        emotion: None,
        intensity_permille: None,
    }
}

fn neural2() -> SpeechLine {
    line("vi-VN-Neural2-A", "neural2")
}

fn chirp() -> SpeechLine {
    line("vi-VN-Chirp3-HD-Aoede", "chirp3-hd")
}

fn plan() -> GooglePlan {
    plan_line(&neural2()).unwrap()
}

fn findings(line: &SpeechLine) -> Vec<PlanFinding> {
    plan_line(line).unwrap_err()
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

fn failed(outcome: AttemptOutcome) -> (FailureClass, FailureEvidence, FailurePolicy) {
    match outcome {
        AttemptOutcome::Failed(failure) => (failure.class, failure.evidence, failure.policy),
        AttemptOutcome::Succeeded(audio) => panic!("unexpected success: {:?}", audio.facts()),
    }
}

fn defect(body: Vec<u8>) -> AudioDefect {
    decode_audio(ResponseBody::Complete(body)).unwrap_err()
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
    assert_eq!(
        [
            count(VoiceFamily::Chirp3Hd),
            count(VoiceFamily::Neural2),
            count(VoiceFamily::Standard),
            count(VoiceFamily::Wavenet),
        ],
        [30, 2, 4, 4]
    );
}

#[test]
fn neural2_line_maps_to_the_exact_request_body_and_digest() {
    let mut line = neural2();
    line.rate_permille = 1_250;
    line.pitch_semitones = -2;
    let plan = plan_line(&line).unwrap();
    // Expected bytes and digest were computed independently with Python's json and hashlib.
    assert_eq!(
        std::str::from_utf8(plan.body()).unwrap(),
        r#"{"audioConfig":{"audioEncoding":"LINEAR16","pitch":-2.0,"sampleRateHertz":24000,"speakingRate":1.25},"input":{"text":"Ánh đèn cuối cùng còn sáng trên sân khấu."},"voice":{"languageCode":"vi-VN","name":"vi-VN-Neural2-A"}}"#
    );
    assert_eq!(
        plan.request_digest(),
        "google-tts-request-1:sha256:8a83d5b5b76243561f44a37cdea5b2dd49e820d70b6ad63cbb9ee9ace94f29d2"
    );
    assert_eq!(plan.voice().name, "vi-VN-Neural2-A");
}

#[test]
fn chirp3_hd_line_omits_the_rate_and_pitch_it_does_not_support() {
    let plan = plan_line(&chirp()).unwrap();
    assert_eq!(
        std::str::from_utf8(plan.body()).unwrap(),
        r#"{"audioConfig":{"audioEncoding":"LINEAR16","sampleRateHertz":24000},"input":{"text":"Ánh đèn cuối cùng còn sáng trên sân khấu."},"voice":{"languageCode":"vi-VN","name":"vi-VN-Chirp3-HD-Aoede"}}"#
    );
    assert_eq!(
        plan.request_digest(),
        "google-tts-request-1:sha256:d3db384130b8277040cfe0d04e31fcb5325f0b7a5fee5e72e50c140254f276c6"
    );
}

#[test]
fn equal_lines_plan_identically_and_any_effective_change_changes_the_digest() {
    assert_eq!(
        plan_line(&neural2()).unwrap(),
        plan_line(&neural2()).unwrap()
    );
    let base = plan().request_digest().to_owned();
    let mut variants = vec![
        line("vi-VN-Neural2-D", "neural2"),
        line("vi-VN-Wavenet-A", "wavenet"),
    ];
    let mut text = neural2();
    text.text.push('!');
    variants.push(text);
    let mut rate = neural2();
    rate.rate_permille = 1_001;
    variants.push(rate);
    let mut pitch = neural2();
    pitch.pitch_semitones = 1;
    variants.push(pitch);
    for variant in variants {
        assert_ne!(
            plan_line(&variant).unwrap().request_digest(),
            base,
            "{variant:?}"
        );
    }
}

#[test]
fn estimate_is_list_price_rounded_up_per_request_and_never_a_charge() {
    let estimate = plan().estimate().clone();
    assert_eq!(estimate.billable_characters, 41);
    assert_eq!(estimate.amount_minor, 1);
    assert_eq!(estimate.currency, "USD");
    assert_eq!(estimate.cents_per_million_characters, 1_600);
    assert_eq!(
        estimate.basis,
        "list_price_before_free_tier_rounded_up_per_request"
    );
    assert_eq!(estimate.price_observed_on, "2026-10-11");
    let mut long = chirp();
    long.text = "a".repeat(1_667);
    assert_eq!(plan_line(&long).unwrap().estimate().amount_minor, 6);
    long.text = "a".repeat(MAX_REQUEST_TEXT_BYTES);
    assert_eq!(plan_line(&long).unwrap().estimate().amount_minor, 15);
    // 2,500 characters at 400 cents per million is exactly one cent; one more rounds up.
    let mut standard = line("vi-VN-Standard-A", "standard");
    standard.text = "a".repeat(2_500);
    assert_eq!(plan_line(&standard).unwrap().estimate().amount_minor, 1);
    standard.text.push('a');
    assert_eq!(plan_line(&standard).unwrap().estimate().amount_minor, 2);
}

#[test]
fn every_unsupported_control_is_an_exact_blocking_finding() {
    let mut cases: Vec<(SpeechLine, Vec<PlanFinding>)> = Vec::new();
    let mut language = neural2();
    language.language = "vi".into();
    cases.push((
        language,
        vec![PlanFinding::UnsupportedLanguage { found: "vi".into() }],
    ));
    cases.push((
        line("vi-VN-Neural2-Z", "neural2"),
        vec![PlanFinding::UnsupportedVoice {
            voice_id: "vi-VN-Neural2-Z".into(),
        }],
    ));
    cases.push((
        line("vi-VN-Neural2-A", "wavenet"),
        vec![PlanFinding::ModelMismatch {
            voice_id: "vi-VN-Neural2-A".into(),
            expected: "neural2",
            found: "wavenet".into(),
        }],
    ));
    for (rate, pitch) in [(249, 0), (2_001, 0), (1_000, 21), (1_000, -21)] {
        let mut prosody = neural2();
        prosody.rate_permille = rate;
        prosody.pitch_semitones = pitch;
        let mut expected = Vec::new();
        if rate != 1_000 {
            expected.push(PlanFinding::RateUnsupported {
                requested_permille: rate,
                min_permille: 250,
                max_permille: 2_000,
            });
        }
        if pitch != 0 {
            expected.push(PlanFinding::PitchUnsupported {
                requested_semitones: pitch,
                min_semitones: -20,
                max_semitones: 20,
            });
        }
        cases.push((prosody, expected));
    }
    let mut chirp_prosody = chirp();
    chirp_prosody.rate_permille = 1_100;
    chirp_prosody.pitch_semitones = -1;
    cases.push((
        chirp_prosody,
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
    ));
    for emotion in ["hopeful", "neutral"] {
        let mut delivery = neural2();
        delivery.emotion = Some(emotion.into());
        cases.push((
            delivery,
            vec![PlanFinding::EmotionUnsupported {
                requested: emotion.into(),
            }],
        ));
    }
    let mut intensity = neural2();
    intensity.intensity_permille = Some(600);
    cases.push((
        intensity,
        vec![PlanFinding::IntensityUnsupported {
            requested_permille: 600,
        }],
    ));
    for (text, expected) in [
        (" \u{3000} ".to_owned(), PlanFinding::EmptyText),
        ("Chuông\u{7}reo".to_owned(), PlanFinding::TextNotNormalized),
        ("A\u{301}nh đèn".to_owned(), PlanFinding::TextNotNormalized),
        (
            "ấ".repeat(1_667),
            PlanFinding::TextTooLong {
                bytes: 5_001,
                limit_bytes: 5_000,
            },
        ),
    ] {
        let mut bad = neural2();
        bad.text = text;
        cases.push((bad, vec![expected]));
    }
    for (line, expected) in cases {
        assert_eq!(findings(&line), expected, "{line:?}");
    }
    let mut at_limit = neural2();
    at_limit.text = format!("{}ab", "ấ".repeat(1_666));
    assert_eq!(at_limit.text.len(), MAX_REQUEST_TEXT_BYTES);
    assert!(plan_line(&at_limit).is_ok());
}

#[test]
fn all_findings_are_reported_together() {
    let bad = SpeechLine {
        text: String::new(),
        language: "en-US".into(),
        model_id: "neural2".into(),
        voice_id: "vi-VN-Chirp3-HD-Kore".into(),
        rate_permille: 1_000,
        pitch_semitones: 0,
        emotion: Some("sad".into()),
        intensity_permille: Some(900),
    };
    assert_eq!(
        findings(&bad),
        vec![
            PlanFinding::UnsupportedLanguage {
                found: "en-US".into()
            },
            PlanFinding::ModelMismatch {
                voice_id: "vi-VN-Chirp3-HD-Kore".into(),
                expected: "chirp3-hd",
                found: "neural2".into(),
            },
            PlanFinding::EmotionUnsupported {
                requested: "sad".into()
            },
            PlanFinding::IntensityUnsupported {
                requested_permille: 900
            },
            PlanFinding::EmptyText,
        ]
    );
}

fn resolved(provider: &str, adapter: &str, contract: &str) -> ResolvedSynthesisInput {
    ResolvedSynthesisInput {
        dialogue_id: "dialogue-02".into(),
        character_id: "an".into(),
        text: TEXT.into(),
        language: "vi-VN".into(),
        provider_id: provider.into(),
        model_id: "neural2".into(),
        voice_id: "vi-VN-Neural2-A".into(),
        voice_rights_record_id: "voice-rights-an".into(),
        adapter_version: adapter.into(),
        output_contract: contract.into(),
        performance: SynthesisPerformance {
            rate_permille: 1_000,
            pitch_semitones: 0,
            emotion: Some("hopeful".into()),
            intensity_permille: Some(600),
        },
        pronunciation: Vec::new(),
    }
}

#[test]
fn a_line_frozen_for_the_reference_catalog_is_refused_not_reinterpreted() {
    let input = resolved(
        "reference-only",
        "reference-only-no-dispatch-v1",
        "reference-only/no-audio",
    );
    assert_eq!(
        plan_resolved(&input).unwrap_err(),
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
                requested: "hopeful".into()
            },
            PlanFinding::IntensityUnsupported {
                requested_permille: 600
            },
        ]
    );
}

#[test]
fn a_frozen_google_line_blocks_on_script_delivery_until_a_mapping_exists() {
    let mut input = resolved(PROVIDER_ID, ADAPTER_VERSION, OUTPUT_CONTRACT);
    assert_eq!(
        plan_resolved(&input).unwrap_err(),
        vec![
            PlanFinding::EmotionUnsupported {
                requested: "hopeful".into()
            },
            PlanFinding::IntensityUnsupported {
                requested_permille: 600
            },
        ]
    );
    input.performance.emotion = None;
    input.performance.intensity_permille = None;
    assert_eq!(plan_resolved(&input).unwrap(), plan());
}

fn status(code: u16, body: &str) -> TransportObservation {
    TransportObservation::Responded {
        status: code,
        body: ResponseBody::Complete(body.as_bytes().to_vec()),
    }
}

fn error_body(code: u16, name: &str) -> String {
    format!(
        r#"{{"error":{{"code":{code},"message":"{TEXT} could not be processed","status":"{name}","details":[]}}}}"#
    )
}

#[test]
fn every_observation_maps_to_one_class_with_its_recovery_policy() {
    use ChargeExposure as C;
    use FailureClass as F;
    use RecoveryAction as A;
    use RetryDecision as R;
    let with_status = |http_status: u16, name: Option<&str>| FailureEvidence::Status {
        http_status,
        provider_status: name.map(str::to_owned),
    };
    let cases = vec![
        (
            TransportObservation::NotSent,
            F::NotSent,
            FailureEvidence::NotConnected,
            (C::None, R::RetryAllowed, A::CheckConnectivity),
        ),
        (
            TransportObservation::LostAfterSend(LostCause::Timeout),
            F::Ambiguous,
            FailureEvidence::Lost {
                cause: LostCause::Timeout,
            },
            (
                C::Unknown,
                R::ResolveBeforeRetry,
                A::ResolveUncertainAttempt,
            ),
        ),
        (
            TransportObservation::LostAfterSend(LostCause::ConnectionError),
            F::Ambiguous,
            FailureEvidence::Lost {
                cause: LostCause::ConnectionError,
            },
            (
                C::Unknown,
                R::ResolveBeforeRetry,
                A::ResolveUncertainAttempt,
            ),
        ),
        (
            status(400, &error_body(400, "INVALID_ARGUMENT")),
            F::InvalidInput,
            with_status(400, Some("INVALID_ARGUMENT")),
            (C::Unknown, R::DoNotRetry, A::FixLineOrCasting),
        ),
        (
            status(401, &error_body(401, "UNAUTHENTICATED")),
            F::AccountRejected,
            with_status(401, Some("UNAUTHENTICATED")),
            (C::Unknown, R::DoNotRetry, A::FixProviderAccount),
        ),
        (
            status(403, &error_body(403, "PERMISSION_DENIED")),
            F::AccountRejected,
            with_status(403, Some("PERMISSION_DENIED")),
            (C::Unknown, R::DoNotRetry, A::FixProviderAccount),
        ),
        (
            status(429, &error_body(429, "RESOURCE_EXHAUSTED")),
            F::RateLimited,
            with_status(429, Some("RESOURCE_EXHAUSTED")),
            (
                C::Unknown,
                R::ResolveBeforeRetry,
                A::WaitForQuotaThenResolve,
            ),
        ),
        (
            status(500, &error_body(500, "INTERNAL")),
            F::ProviderError,
            with_status(500, Some("INTERNAL")),
            (
                C::Unknown,
                R::ResolveBeforeRetry,
                A::ResolveUncertainAttempt,
            ),
        ),
        (
            status(502, "<html>bad gateway</html>"),
            F::ProviderError,
            with_status(502, None),
            (
                C::Unknown,
                R::ResolveBeforeRetry,
                A::ResolveUncertainAttempt,
            ),
        ),
        (
            status(503, &error_body(503, "UNAVAILABLE")),
            F::ProviderError,
            with_status(503, Some("UNAVAILABLE")),
            (
                C::Unknown,
                R::ResolveBeforeRetry,
                A::ResolveUncertainAttempt,
            ),
        ),
        (
            status(504, &error_body(504, "DEADLINE_EXCEEDED")),
            F::ProviderError,
            with_status(504, Some("DEADLINE_EXCEEDED")),
            (
                C::Unknown,
                R::ResolveBeforeRetry,
                A::ResolveUncertainAttempt,
            ),
        ),
        (
            status(302, ""),
            F::UnexpectedResponse,
            with_status(302, None),
            (C::Unknown, R::ResolveBeforeRetry, A::ReportAdapterMismatch),
        ),
        (
            status(404, &error_body(404, "NOT_FOUND")),
            F::UnexpectedResponse,
            with_status(404, Some("NOT_FOUND")),
            (C::Unknown, R::ResolveBeforeRetry, A::ReportAdapterMismatch),
        ),
        (
            TransportObservation::Responded {
                status: 503,
                body: ResponseBody::Interrupted,
            },
            F::ProviderError,
            with_status(503, None),
            (
                C::Unknown,
                R::ResolveBeforeRetry,
                A::ResolveUncertainAttempt,
            ),
        ),
        (
            ok(b"{}".to_vec()),
            F::MalformedAudio,
            FailureEvidence::Audio {
                http_status: 200,
                defect: AudioDefect::MissingAudioContent,
            },
            (
                C::UsageObserved,
                R::RetryAllowed,
                A::RetryAndReportIfRepeated,
            ),
        ),
    ];
    for (observation, class, evidence, (charge, retry, recovery)) in cases {
        let label = format!("{observation:?}");
        assert_eq!(
            failed(classify(observation)),
            (
                class,
                evidence,
                FailurePolicy {
                    charge,
                    retry,
                    recovery
                }
            ),
            "{label}"
        );
    }
}

#[test]
fn no_class_with_an_unknown_charge_permits_an_automatic_repeat() {
    for class in FailureClass::ALL {
        let policy = policy(class);
        if policy.charge == ChargeExposure::Unknown {
            assert_ne!(policy.retry, RetryDecision::RetryAllowed, "{class:?}");
        }
        if policy.retry == RetryDecision::RetryAllowed {
            assert_ne!(policy.charge, ChargeExposure::Unknown, "{class:?}");
        }
    }
    assert_eq!(policy(FailureClass::NotSent).charge, ChargeExposure::None);
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
        let (_, evidence, _) = failed(classify(status(400, body)));
        assert_eq!(
            evidence,
            FailureEvidence::Status {
                http_status: 400,
                provider_status: expected.map(str::to_owned),
            },
            "{body}"
        );
    }
    let long = format!(r#"{{"error":{{"status":"{}"}}}}"#, "A".repeat(65));
    let (_, evidence, _) = failed(classify(status(400, &long)));
    assert_eq!(
        evidence,
        FailureEvidence::Status {
            http_status: 400,
            provider_status: None,
        }
    );
}

#[test]
fn valid_mono_24khz_pcm_is_accepted_with_exact_facts() {
    let audio = wav(1, 1, 24_000, 16, 2_400);
    let outcome = classify(ok(envelope(&audio)));
    let AttemptOutcome::Succeeded(validated) = outcome else {
        panic!("expected success: {outcome:?}");
    };
    let facts = validated.facts().clone();
    assert_eq!(facts.bytes, 44 + 4_800);
    assert_eq!(facts.sha256, format!("{:x}", Sha256::digest(&audio)));
    assert_eq!(
        (
            facts.sample_rate_hz,
            facts.channels,
            facts.bits_per_sample,
            facts.frames
        ),
        (24_000, 1, 16, 2_400)
    );
    assert_eq!(validated.into_bytes(), audio);
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
        AudioDefect::InvalidWav { errors } if !errors.is_empty()
    ));
    assert!(matches!(
        defect(envelope(b"ID3\x04 not a wave file")),
        AudioDefect::InvalidWav { .. }
    ));
    for (tag, channels, rate, bits) in [(1, 2, 24_000, 16), (1, 1, 48_000, 16), (1, 1, 24_000, 8)] {
        assert_eq!(
            defect(envelope(&wav(tag, channels, rate, bits, 10))),
            AudioDefect::UnexpectedFormat {
                format_tag: tag,
                channels,
                sample_rate_hz: rate,
                bits_per_sample: bits,
            }
        );
    }
    // The inspector admits integer PCM only, so float audio is structurally refused.
    assert_eq!(
        defect(envelope(&wav(3, 1, 24_000, 32, 10))),
        AudioDefect::InvalidWav {
            errors: vec![StructuralError::UnsupportedFormatTag { format_tag: 3 }],
        }
    );
    assert_eq!(
        defect(envelope(&wav(1, 1, 24_000, 16, 0))),
        AudioDefect::NoFrames
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

#[tokio::test]
async fn a_response_lost_after_execution_is_recorded_once_and_never_resent() {
    let transport = ScriptedTransport::new(
        true,
        TransportObservation::LostAfterSend(LostCause::Timeout),
    );
    let plan = plan();
    let outcome = synthesize_once(&transport, &plan).await;
    let (class, _, policy) = failed(outcome.clone());
    assert_eq!(class, FailureClass::Ambiguous);
    assert_eq!(policy.retry, RetryDecision::ResolveBeforeRetry);
    assert_eq!(
        transport.executions(),
        vec![plan.request_digest().to_owned()]
    );
    let record = record(&plan, &outcome);
    assert_eq!(record.charge, ChargeExposure::Unknown);
    assert_eq!(record.provider_request_id, None);
    assert_eq!(record.limitations, LIMITATIONS);
}

#[tokio::test]
async fn a_connection_that_never_opened_has_no_charge() {
    let transport = ScriptedTransport::new(false, TransportObservation::NotSent);
    let plan = plan();
    let outcome = synthesize_once(&transport, &plan).await;
    assert!(transport.executions().is_empty());
    assert_eq!(record(&plan, &outcome).charge, ChargeExposure::None);
}

#[tokio::test]
async fn success_records_usage_without_text_credentials_or_settlement() {
    let audio = wav(1, 1, 24_000, 16, 480);
    let transport = ScriptedTransport::new(true, ok(envelope(&audio)));
    let plan = plan();
    let outcome = synthesize_once(&transport, &plan).await;
    assert_eq!(transport.executions().len(), 1);
    let record = record(&plan, &outcome);
    assert_eq!(record.charge, ChargeExposure::UsageObserved);
    let json = serde_json::to_value(&record).unwrap();
    assert_eq!(json["provider_id"], "google-cloud-tts");
    assert_eq!(json["adapter_version"], ADAPTER_VERSION);
    assert_eq!(json["model_id"], "neural2");
    assert_eq!(json["voice_id"], "vi-VN-Neural2-A");
    assert_eq!(json["outcome"]["status"], "succeeded");
    assert_eq!(json["outcome"]["audio"]["frames"], 480);
    assert_eq!(json["estimate"]["amount_minor"], 1);
    assert_eq!(json["charge"], "usage_observed");
    let rendered = json.to_string();
    assert!(!rendered.contains("Ánh đèn"), "{rendered}");
    assert!(!rendered.contains("audioContent"), "{rendered}");
}

#[tokio::test]
async fn provider_error_messages_never_reach_the_record() {
    let transport = ScriptedTransport::new(true, status(400, &error_body(400, "INVALID_ARGUMENT")));
    let plan = plan();
    let outcome = synthesize_once(&transport, &plan).await;
    let rendered = serde_json::to_string(&record(&plan, &outcome)).unwrap();
    assert!(!rendered.contains("could not be processed"), "{rendered}");
    assert!(!rendered.contains("Ánh đèn"), "{rendered}");
    assert!(rendered.contains(r#""provider_status":"INVALID_ARGUMENT""#));
    assert!(rendered.contains(r#""retry":"do_not_retry""#));
    assert!(rendered.contains(r#""recovery":"fix_line_or_casting""#));
}
