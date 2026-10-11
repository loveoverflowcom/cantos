//! Google Cloud Text-to-Speech `v1/text:synthesize` adapter core for Vietnamese voices.
//!
//! Pure: capability checks, the exact request body, failure classification and returned-audio
//! validation. No network, clock, credential or file access; `google_http` is the shell. The
//! declaration below was checked against Google's published documentation on
//! [`CAPABILITIES_VERIFIED_ON`]; it is not evidence that any voice is enabled for an account
//! or that its audio is acceptable. See ADR 0009.
use base64::Engine as _;
use cantos_api::ResolvedSynthesisInput;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::{
    AttemptFailure, AttemptOutcome, AudioDefect, AudioFacts, ChargeExposure, FailureClass,
    FailureEvidence, FailurePolicy, RecoveryAction, ResponseBody, RetryDecision,
    TransportObservation, ValidatedAudio,
};
use crate::wav_inspection::{inspect_wav, Structure};

pub const PROVIDER_ID: &str = "google-cloud-tts";
/// Bump for any change to the request mapping, controls, output contract or validation.
pub const ADAPTER_VERSION: &str = "google-cloud-tts-synthesize-1";
pub const OUTPUT_CONTRACT: &str = "audio/wav;codec=pcm_s16le;rate=24000;channels=1";
pub const ENDPOINT: &str = "https://texttospeech.googleapis.com/v1/text:synthesize";
pub const LANGUAGE: &str = "vi-VN";
pub const SAMPLE_RATE_HZ: u32 = 24_000;
/// Google's "total bytes per request" content limit, which cannot be raised.
pub const MAX_REQUEST_TEXT_BYTES: usize = 5_000;
/// Comfortably above 5,000 bytes of Vietnamese text as base64 24 kHz PCM.
pub const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
pub const CAPABILITIES_VERIFIED_ON: &str = "2026-10-11";
pub const PRICE_SOURCE: &str = "https://cloud.google.com/text-to-speech/pricing";
/// What this provider path cannot offer; recorded on every attempt.
pub const LIMITATIONS: [&str; 4] = [
    "no_provider_request_id",
    "no_idempotency_key",
    "no_request_lookup",
    "model_revision_unpinned",
];

const DEFAULT_RATE_PERMILLE: u16 = 1_000;
const RATE_PERMILLE: (u16, u16) = (250, 2_000);
const PITCH_SEMITONES: (i16, i16) = (-20, 20);
const WAVE_FORMAT_PCM: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;
const MONO: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum VoiceFamily {
    Standard,
    Wavenet,
    Neural2,
    Chirp3Hd,
}

impl VoiceFamily {
    /// Google selects the model through the voice name; this is Cantos' name for that family.
    pub const fn model_id(self) -> &'static str {
        match self {
            VoiceFamily::Standard => "standard",
            VoiceFamily::Wavenet => "wavenet",
            VoiceFamily::Neural2 => "neural2",
            VoiceFamily::Chirp3Hd => "chirp3-hd",
        }
    }

    /// Chirp 3: HD voices do not support the speaking-rate and pitch parameters.
    const fn supports_rate_and_pitch(self) -> bool {
        match self {
            VoiceFamily::Standard | VoiceFamily::Wavenet | VoiceFamily::Neural2 => true,
            VoiceFamily::Chirp3Hd => false,
        }
    }

    /// Published list price after the free monthly allowance, in US cents per million characters.
    pub const fn cents_per_million_characters(self) -> u64 {
        match self {
            VoiceFamily::Standard | VoiceFamily::Wavenet => 400,
            VoiceFamily::Neural2 => 1_600,
            VoiceFamily::Chirp3Hd => 3_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceGender {
    Female,
    Male,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct GoogleVoice {
    pub name: &'static str,
    pub family: VoiceFamily,
    pub gender: VoiceGender,
}

const fn voice(name: &'static str, family: VoiceFamily, gender: VoiceGender) -> GoogleVoice {
    GoogleVoice {
        name,
        family,
        gender,
    }
}

use VoiceFamily::{Chirp3Hd, Neural2, Standard, Wavenet};
use VoiceGender::{Female, Male};

/// Every vi-VN voice Google listed on [`CAPABILITIES_VERIFIED_ON`].
pub static VI_VN_VOICES: [GoogleVoice; 40] = [
    voice("vi-VN-Chirp3-HD-Achernar", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Achird", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Algenib", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Algieba", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Alnilam", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Aoede", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Autonoe", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Callirrhoe", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Charon", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Despina", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Enceladus", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Erinome", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Fenrir", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Gacrux", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Iapetus", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Kore", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Laomedeia", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Leda", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Orus", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Puck", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Pulcherrima", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Rasalgethi", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Sadachbia", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Sadaltager", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Schedar", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Sulafat", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Umbriel", Chirp3Hd, Male),
    voice("vi-VN-Chirp3-HD-Vindemiatrix", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Zephyr", Chirp3Hd, Female),
    voice("vi-VN-Chirp3-HD-Zubenelgenubi", Chirp3Hd, Male),
    voice("vi-VN-Neural2-A", Neural2, Female),
    voice("vi-VN-Neural2-D", Neural2, Male),
    voice("vi-VN-Standard-A", Standard, Female),
    voice("vi-VN-Standard-B", Standard, Male),
    voice("vi-VN-Standard-C", Standard, Female),
    voice("vi-VN-Standard-D", Standard, Male),
    voice("vi-VN-Wavenet-A", Wavenet, Female),
    voice("vi-VN-Wavenet-B", Wavenet, Male),
    voice("vi-VN-Wavenet-C", Wavenet, Female),
    voice("vi-VN-Wavenet-D", Wavenet, Male),
];

pub fn find_voice(name: &str) -> Option<&'static GoogleVoice> {
    VI_VN_VOICES.iter().find(|voice| voice.name == name)
}

/// One line to speak. Delivery controls the voice cannot honor are refused, never dropped.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechLine {
    pub text: String,
    pub language: String,
    pub model_id: String,
    pub voice_id: String,
    pub rate_permille: u16,
    pub pitch_semitones: i16,
    #[serde(default)]
    pub emotion: Option<String>,
    #[serde(default)]
    pub intensity_permille: Option<u16>,
}

/// A blocking finding found before anything is sent; the fix is always a casting or line change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum PlanFinding {
    ProviderMismatch {
        found: String,
    },
    AdapterVersionMismatch {
        found: String,
    },
    OutputContractMismatch {
        found: String,
    },
    UnsupportedLanguage {
        found: String,
    },
    UnsupportedVoice {
        voice_id: String,
    },
    ModelMismatch {
        voice_id: String,
        expected: &'static str,
        found: String,
    },
    RateUnsupported {
        requested_permille: u16,
        min_permille: u16,
        max_permille: u16,
    },
    PitchUnsupported {
        requested_semitones: i16,
        min_semitones: i16,
        max_semitones: i16,
    },
    EmotionUnsupported {
        requested: String,
    },
    IntensityUnsupported {
        requested_permille: u16,
    },
    EmptyText,
    TextNotNormalized,
    TextTooLong {
        bytes: usize,
        limit_bytes: usize,
    },
}

/// A planning estimate at list price; never a quote, reservation or settled charge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListPriceEstimate {
    pub currency: &'static str,
    pub amount_minor: u64,
    pub billable_characters: u64,
    pub cents_per_million_characters: u64,
    pub basis: &'static str,
    pub price_source: &'static str,
    pub price_observed_on: &'static str,
}

/// The exact request for one line. Only [`plan_line`] builds one, so a request that failed a
/// capability check cannot reach a transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GooglePlan {
    body: Vec<u8>,
    request_digest: String,
    voice: &'static GoogleVoice,
    estimate: ListPriceEstimate,
}

impl GooglePlan {
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn request_digest(&self) -> &str {
        &self.request_digest
    }

    pub fn voice(&self) -> &'static GoogleVoice {
        self.voice
    }

    pub fn estimate(&self) -> &ListPriceEstimate {
        &self.estimate
    }
}

/// Check one line against the declared capabilities and build its request body.
pub fn plan_line(line: &SpeechLine) -> Result<GooglePlan, Vec<PlanFinding>> {
    let mut findings = Vec::new();
    if line.language != LANGUAGE {
        findings.push(PlanFinding::UnsupportedLanguage {
            found: line.language.clone(),
        });
    }
    let voice = find_voice(&line.voice_id);
    match voice {
        None => findings.push(PlanFinding::UnsupportedVoice {
            voice_id: line.voice_id.clone(),
        }),
        Some(voice) if voice.family.model_id() != line.model_id => {
            findings.push(PlanFinding::ModelMismatch {
                voice_id: line.voice_id.clone(),
                expected: voice.family.model_id(),
                found: line.model_id.clone(),
            });
        }
        Some(voice) => check_prosody(voice.family, line, &mut findings),
    }
    if let Some(emotion) = &line.emotion {
        findings.push(PlanFinding::EmotionUnsupported {
            requested: emotion.clone(),
        });
    }
    if let Some(intensity) = line.intensity_permille {
        findings.push(PlanFinding::IntensityUnsupported {
            requested_permille: intensity,
        });
    }
    check_text(&line.text, &mut findings);
    match voice {
        Some(voice) if findings.is_empty() => Ok(build_plan(voice, line)),
        _ => Err(findings),
    }
}

/// Plan a frozen production line. Lines frozen for another adapter or output contract are
/// refused rather than reinterpreted.
pub fn plan_resolved(input: &ResolvedSynthesisInput) -> Result<GooglePlan, Vec<PlanFinding>> {
    let mut findings = Vec::new();
    if input.provider_id != PROVIDER_ID {
        findings.push(PlanFinding::ProviderMismatch {
            found: input.provider_id.clone(),
        });
    }
    if input.adapter_version != ADAPTER_VERSION {
        findings.push(PlanFinding::AdapterVersionMismatch {
            found: input.adapter_version.clone(),
        });
    }
    if input.output_contract != OUTPUT_CONTRACT {
        findings.push(PlanFinding::OutputContractMismatch {
            found: input.output_contract.clone(),
        });
    }
    let line = SpeechLine {
        text: input.text.clone(),
        language: input.language.clone(),
        model_id: input.model_id.clone(),
        voice_id: input.voice_id.clone(),
        rate_permille: input.performance.rate_permille,
        pitch_semitones: input.performance.pitch_semitones,
        emotion: input.performance.emotion.clone(),
        intensity_permille: input.performance.intensity_permille,
    };
    match plan_line(&line) {
        Ok(plan) if findings.is_empty() => Ok(plan),
        Ok(_) => Err(findings),
        Err(line_findings) => {
            findings.extend(line_findings);
            Err(findings)
        }
    }
}

fn check_prosody(family: VoiceFamily, line: &SpeechLine, findings: &mut Vec<PlanFinding>) {
    let ((rate_min, rate_max), (pitch_min, pitch_max)) = if family.supports_rate_and_pitch() {
        (RATE_PERMILLE, PITCH_SEMITONES)
    } else {
        ((DEFAULT_RATE_PERMILLE, DEFAULT_RATE_PERMILLE), (0, 0))
    };
    if !(rate_min..=rate_max).contains(&line.rate_permille) {
        findings.push(PlanFinding::RateUnsupported {
            requested_permille: line.rate_permille,
            min_permille: rate_min,
            max_permille: rate_max,
        });
    }
    if !(pitch_min..=pitch_max).contains(&line.pitch_semitones) {
        findings.push(PlanFinding::PitchUnsupported {
            requested_semitones: line.pitch_semitones,
            min_semitones: pitch_min,
            max_semitones: pitch_max,
        });
    }
}

fn check_text(text: &str, findings: &mut Vec<PlanFinding>) {
    if text.trim().is_empty() {
        findings.push(PlanFinding::EmptyText);
    } else if text.chars().any(char::is_control) || !text.nfc().eq(text.chars()) {
        findings.push(PlanFinding::TextNotNormalized);
    }
    if text.len() > MAX_REQUEST_TEXT_BYTES {
        findings.push(PlanFinding::TextTooLong {
            bytes: text.len(),
            limit_bytes: MAX_REQUEST_TEXT_BYTES,
        });
    }
}

fn build_plan(voice: &'static GoogleVoice, line: &SpeechLine) -> GooglePlan {
    let mut audio_config = json!({
        "audioEncoding": "LINEAR16",
        "sampleRateHertz": SAMPLE_RATE_HZ,
    });
    if voice.family.supports_rate_and_pitch() {
        audio_config["speakingRate"] = json!(f64::from(line.rate_permille) / 1_000.0);
        audio_config["pitch"] = json!(f64::from(line.pitch_semitones));
    }
    // serde_json orders object keys, so equal lines always produce byte-identical bodies.
    let body = json!({
        "input": { "text": line.text },
        "voice": { "languageCode": LANGUAGE, "name": voice.name },
        "audioConfig": audio_config,
    })
    .to_string()
    .into_bytes();
    let mut hash = Sha256::new();
    hash.update(b"cantos/google-tts-request\n");
    hash.update(ADAPTER_VERSION.as_bytes());
    hash.update(b"\n");
    hash.update(&body);
    GooglePlan {
        request_digest: format!("google-tts-request-1:sha256:{:x}", hash.finalize()),
        estimate: list_price_estimate(voice.family, &line.text),
        voice,
        body,
    }
}

/// Google counts every character of the input, including spaces and newlines. Cantos counts
/// Unicode scalar values of the NFC text and rounds each request up to a whole cent.
fn list_price_estimate(family: VoiceFamily, text: &str) -> ListPriceEstimate {
    let characters = text.chars().count() as u64;
    let rate = family.cents_per_million_characters();
    // At most 5,000 characters per request, so the product cannot overflow.
    let amount_minor = (characters * rate).div_ceil(1_000_000);
    ListPriceEstimate {
        currency: "USD",
        amount_minor,
        billable_characters: characters,
        cents_per_million_characters: rate,
        basis: "list_price_before_free_tier_rounded_up_per_request",
        price_source: PRICE_SOURCE,
        price_observed_on: CAPABILITIES_VERIFIED_ON,
    }
}

/// Google's REST API documents no idempotency key, request lookup or non-execution guarantee
/// for errors, so every outcome that may have reached it is treated as possibly billed.
pub const fn policy(class: FailureClass) -> FailurePolicy {
    use ChargeExposure as Charge;
    use RecoveryAction as Recover;
    use RetryDecision as Retry;
    let (charge, retry, recovery) = match class {
        FailureClass::NotSent => (
            Charge::None,
            Retry::RetryAllowed,
            Recover::CheckConnectivity,
        ),
        FailureClass::InvalidInput => (
            Charge::Unknown,
            Retry::DoNotRetry,
            Recover::FixLineOrCasting,
        ),
        FailureClass::AccountRejected => (
            Charge::Unknown,
            Retry::DoNotRetry,
            Recover::FixProviderAccount,
        ),
        FailureClass::RateLimited => (
            Charge::Unknown,
            Retry::ResolveBeforeRetry,
            Recover::WaitForQuotaThenResolve,
        ),
        FailureClass::ProviderError | FailureClass::Ambiguous => (
            Charge::Unknown,
            Retry::ResolveBeforeRetry,
            Recover::ResolveUncertainAttempt,
        ),
        FailureClass::MalformedAudio => (
            Charge::UsageObserved,
            Retry::RetryAllowed,
            Recover::RetryAndReportIfRepeated,
        ),
        FailureClass::UnexpectedResponse => (
            Charge::Unknown,
            Retry::ResolveBeforeRetry,
            Recover::ReportAdapterMismatch,
        ),
    };
    FailurePolicy {
        charge,
        retry,
        recovery,
    }
}

/// Status codes follow Google's HTTP mapping of `google.rpc.Code`; anything else is a mismatch
/// between this adapter and the provider.
const fn class_for_status(status: u16) -> FailureClass {
    match status {
        400 => FailureClass::InvalidInput,
        401 | 403 => FailureClass::AccountRejected,
        429 => FailureClass::RateLimited,
        500 | 502 | 503 | 504 => FailureClass::ProviderError,
        _ => FailureClass::UnexpectedResponse,
    }
}

fn failure(class: FailureClass, evidence: FailureEvidence) -> AttemptOutcome {
    AttemptOutcome::Failed(AttemptFailure {
        class,
        evidence,
        policy: policy(class),
    })
}

/// Classify one transport observation. Total over every observation; never panics.
pub fn classify(observation: TransportObservation) -> AttemptOutcome {
    match observation {
        TransportObservation::NotSent => {
            failure(FailureClass::NotSent, FailureEvidence::NotConnected)
        }
        TransportObservation::LostAfterSend(cause) => {
            failure(FailureClass::Ambiguous, FailureEvidence::Lost { cause })
        }
        TransportObservation::Responded { status, body } if (200..300).contains(&status) => {
            match decode_audio(body) {
                Ok(audio) => AttemptOutcome::Succeeded(audio),
                Err(defect) => failure(
                    FailureClass::MalformedAudio,
                    FailureEvidence::Audio {
                        http_status: status,
                        defect,
                    },
                ),
            }
        }
        TransportObservation::Responded { status, body } => failure(
            class_for_status(status),
            FailureEvidence::Status {
                http_status: status,
                provider_status: provider_status(&body),
            },
        ),
    }
}

#[derive(Deserialize)]
struct SynthesizeResponse {
    #[serde(rename = "audioContent")]
    audio_content: Option<String>,
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorStatus,
}

#[derive(Deserialize)]
struct ErrorStatus {
    status: Option<String>,
}

/// Keeps only the canonical status name; the message can echo the submitted text.
fn provider_status(body: &ResponseBody) -> Option<String> {
    let ResponseBody::Complete(bytes) = body else {
        return None;
    };
    let status = serde_json::from_slice::<ErrorEnvelope>(bytes)
        .ok()?
        .error
        .status?;
    let canonical = !status.is_empty()
        && status.len() <= 64
        && status.bytes().all(|b| b.is_ascii_uppercase() || b == b'_');
    canonical.then_some(status)
}

/// Decode `audioContent` and accept only the declared WAV contract with at least one frame.
pub fn decode_audio(body: ResponseBody) -> Result<ValidatedAudio, AudioDefect> {
    let bytes = match body {
        ResponseBody::Complete(bytes) => bytes,
        ResponseBody::TooLarge => {
            return Err(AudioDefect::ResponseTooLarge {
                limit_bytes: MAX_RESPONSE_BYTES,
            })
        }
        ResponseBody::Interrupted => return Err(AudioDefect::ResponseInterrupted),
    };
    let response: SynthesizeResponse =
        serde_json::from_slice(&bytes).map_err(|_| AudioDefect::NotJson)?;
    let encoded = response
        .audio_content
        .ok_or(AudioDefect::MissingAudioContent)?;
    let audio = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| AudioDefect::InvalidBase64)?;
    if audio.is_empty() {
        return Err(AudioDefect::EmptyAudio);
    }
    let report = inspect_wav(&audio);
    if report.structure() == Structure::Invalid {
        return Err(AudioDefect::InvalidWav {
            errors: report.errors().to_vec(),
        });
    }
    let (Some(fmt), Some(measurements)) = (report.fmt(), report.measurements()) else {
        return Err(AudioDefect::InvalidWav { errors: Vec::new() });
    };
    if (
        fmt.format_tag,
        fmt.channels,
        fmt.sample_rate_hz,
        fmt.bits_per_sample,
    ) != (WAVE_FORMAT_PCM, MONO, SAMPLE_RATE_HZ, BITS_PER_SAMPLE)
    {
        return Err(AudioDefect::UnexpectedFormat {
            format_tag: fmt.format_tag,
            channels: fmt.channels,
            sample_rate_hz: fmt.sample_rate_hz,
            bits_per_sample: fmt.bits_per_sample,
        });
    }
    if measurements.frames == 0 {
        return Err(AudioDefect::NoFrames);
    }
    let facts = AudioFacts {
        bytes: report.file().bytes,
        sha256: report.file().sha256.clone(),
        sample_rate_hz: fmt.sample_rate_hz,
        channels: fmt.channels,
        bits_per_sample: fmt.bits_per_sample,
        frames: measurements.frames,
    };
    Ok(ValidatedAudio::new(audio, facts))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RecordedOutcome {
    Succeeded { audio: AudioFacts },
    Failed { failure: AttemptFailure },
}

/// The accountable description of one attempt. Credentials, dialogue text and provider error
/// messages are never part of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttemptRecord {
    pub provider_id: &'static str,
    pub adapter_version: &'static str,
    pub model_id: &'static str,
    pub voice_id: &'static str,
    pub request_digest: String,
    pub provider_request_id: Option<String>,
    pub limitations: [&'static str; 4],
    pub estimate: ListPriceEstimate,
    pub charge: ChargeExposure,
    pub outcome: RecordedOutcome,
}

pub fn record(plan: &GooglePlan, outcome: &AttemptOutcome) -> AttemptRecord {
    AttemptRecord {
        provider_id: PROVIDER_ID,
        adapter_version: ADAPTER_VERSION,
        model_id: plan.voice.family.model_id(),
        voice_id: plan.voice.name,
        request_digest: plan.request_digest.clone(),
        provider_request_id: None,
        limitations: LIMITATIONS,
        estimate: plan.estimate.clone(),
        charge: outcome.charge(),
        outcome: match outcome {
            AttemptOutcome::Succeeded(audio) => RecordedOutcome::Succeeded {
                audio: audio.facts().clone(),
            },
            AttemptOutcome::Failed(failure) => RecordedOutcome::Failed {
                failure: failure.clone(),
            },
        },
    }
}
