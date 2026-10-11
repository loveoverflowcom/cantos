//! Google Cloud Text-to-Speech `v1/text:synthesize` core for frozen vi-VN lines.
//!
//! Pure: capability checks, the exact request body, classification of one transport
//! observation into the shared receipt vocabulary and returned-audio admission. No network,
//! clock, credential or file access; `google_http` is the shell. See ADR 0011.
use base64::Engine as _;
use cantos_contracts::ResolvedSynthesisInput;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::attempt::{DeliveryState, FailureClass};
use super::audio::{inspect_output_for_profile, AudioError, InspectedTtsAudio, OutputProfile};
use super::google_contract::{
    self, find_voice, GoogleVoice, VoiceFamily, ADAPTER_VERSION, CAPABILITIES_VERIFIED_ON,
    LANGUAGE_CODE, MAX_TEXT_BYTES, OUTPUT_CONTRACT, PROVIDER_ID,
};

pub const ENDPOINT: &str = "https://texttospeech.googleapis.com/v1/text:synthesize";
/// Base64 of the 16 MiB audio admission limit plus the JSON envelope.
pub const MAX_RESPONSE_BYTES: usize = 24 * 1024 * 1024;
pub const PRICE_SOURCE: &str = "https://cloud.google.com/text-to-speech/pricing";
/// What this provider path cannot offer; recorded with every observation.
pub const LIMITATIONS: [&str; 4] = [
    "no_provider_request_id",
    "no_idempotency_key",
    "no_request_lookup",
    "model_revision_unpinned",
];

/// A blocking finding found before anything is sent.
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
    InvalidText,
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

/// The exact request for one frozen line. Only [`plan`] builds one, so a line that failed a
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

/// Check one frozen line against the exact adapter identity and declared capabilities, and
/// build its request. Every finding is returned; lines frozen for another adapter are refused
/// rather than reinterpreted. `neutral` at intensity 0 is the voice's default delivery and
/// sends no control; any other delivery is refused, never dropped.
pub fn plan(input: &ResolvedSynthesisInput) -> Result<GooglePlan, Vec<PlanFinding>> {
    let mut findings = Vec::new();
    for (matches, finding) in [
        (
            input.provider_id == PROVIDER_ID,
            PlanFinding::ProviderMismatch {
                found: input.provider_id.clone(),
            },
        ),
        (
            input.adapter_version == ADAPTER_VERSION,
            PlanFinding::AdapterVersionMismatch {
                found: input.adapter_version.clone(),
            },
        ),
        (
            input.output_contract == OUTPUT_CONTRACT,
            PlanFinding::OutputContractMismatch {
                found: input.output_contract.clone(),
            },
        ),
        (
            google_contract::LANGUAGES.contains(&input.language.as_str()),
            PlanFinding::UnsupportedLanguage {
                found: input.language.clone(),
            },
        ),
    ] {
        if !matches {
            findings.push(finding);
        }
    }
    let voice = find_voice(&input.voice_id);
    match voice {
        None => findings.push(PlanFinding::UnsupportedVoice {
            voice_id: input.voice_id.clone(),
        }),
        Some(voice) if voice.family.model_id() != input.model_id => {
            findings.push(PlanFinding::ModelMismatch {
                voice_id: input.voice_id.clone(),
                expected: voice.family.model_id(),
                found: input.model_id.clone(),
            });
        }
        Some(voice) => check_prosody(voice.family, input, &mut findings),
    }
    let performance = &input.performance;
    if let Some(emotion) = performance.emotion.as_ref().filter(|e| *e != "neutral") {
        findings.push(PlanFinding::EmotionUnsupported {
            requested: emotion.clone(),
        });
    }
    if let Some(intensity) = performance.intensity_permille.filter(|v| *v != 0) {
        findings.push(PlanFinding::IntensityUnsupported {
            requested_permille: intensity,
        });
    }
    if input.text.len() > MAX_TEXT_BYTES {
        findings.push(PlanFinding::TextTooLong {
            bytes: input.text.len(),
            limit_bytes: MAX_TEXT_BYTES,
        });
    } else if !google_contract::valid_text(&input.text) {
        findings.push(PlanFinding::InvalidText);
    }
    match voice {
        Some(voice) if findings.is_empty() => Ok(build_plan(voice, input)),
        _ => Err(findings),
    }
}

fn check_prosody(
    family: VoiceFamily,
    input: &ResolvedSynthesisInput,
    findings: &mut Vec<PlanFinding>,
) {
    let (rate_min, rate_max) = family.rate_permille();
    let (pitch_min, pitch_max) = family.pitch_semitones();
    let rate = input.performance.rate_permille;
    let pitch = input.performance.pitch_semitones;
    if !(rate_min..=rate_max).contains(&rate) {
        findings.push(PlanFinding::RateUnsupported {
            requested_permille: rate,
            min_permille: rate_min,
            max_permille: rate_max,
        });
    }
    if !(pitch_min..=pitch_max).contains(&pitch) {
        findings.push(PlanFinding::PitchUnsupported {
            requested_semitones: pitch,
            min_semitones: pitch_min,
            max_semitones: pitch_max,
        });
    }
}

fn build_plan(voice: &'static GoogleVoice, input: &ResolvedSynthesisInput) -> GooglePlan {
    let mut audio_config = json!({
        "audioEncoding": "LINEAR16",
        "sampleRateHertz": OutputProfile::Google24Khz.sample_rate_hz(),
    });
    if voice.family.supports_rate_and_pitch() {
        audio_config["speakingRate"] = json!(f64::from(input.performance.rate_permille) / 1_000.0);
        audio_config["pitch"] = json!(f64::from(input.performance.pitch_semitones));
    }
    // serde_json orders object keys, so equal lines always produce byte-identical bodies.
    let body = json!({
        "input": { "text": input.text },
        "voice": { "languageCode": LANGUAGE_CODE, "name": voice.name },
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
        estimate: list_price_estimate(voice.family, &input.text),
        voice,
        body,
    }
}

/// Google bills every character of the input, including spaces. Cantos counts Unicode scalar
/// values of the NFC text and rounds each request up to a whole cent.
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

/// What the shell saw after handing one request to a transport. Built by transports only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportObservation {
    /// The connection (TCP and TLS) was never established, so no request byte left this host.
    NotSent,
    /// The request may have reached the provider; its outcome was then lost.
    LostAfterSend(LostCause),
    /// The provider answered with an HTTP status.
    Responded { status: u16, body: ResponseBody },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LostCause {
    Timeout,
    ConnectionError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseBody {
    Complete(Vec<u8>),
    /// Reading stopped at the adapter's response limit; the remainder was never buffered.
    TooLarge,
    /// The connection failed or timed out after the status line arrived.
    Interrupted,
}

/// A 2xx body that cannot become audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum AudioDefect {
    ResponseTooLarge { limit_bytes: usize },
    ResponseInterrupted,
    NotJson,
    MissingAudioContent,
    InvalidBase64,
    EmptyAudio,
    Rejected { error: AudioError },
}

/// The exact observation behind a failure. Provider error messages are never kept: they can
/// echo private dialogue text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FailureEvidence {
    NotConnected,
    Lost {
        cause: LostCause,
    },
    Status {
        http_status: u16,
        provider_status: Option<String>,
    },
    Audio {
        http_status: u16,
        defect: AudioDefect,
    },
}

/// A failure in the shared receipt vocabulary plus the adapter's exact evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoogleFailure {
    pub class: FailureClass,
    pub delivery: DeliveryState,
    pub evidence: FailureEvidence,
}

/// Admitted audio bytes, constructible only by [`classify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleAudio {
    bytes: Vec<u8>,
    inspected: InspectedTtsAudio,
}

impl GoogleAudio {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn inspected(&self) -> &InspectedTtsAudio {
        &self.inspected
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoogleOutcome {
    Output(GoogleAudio),
    Failure(GoogleFailure),
}

/// Classify one observation. Google documents no non-execution guarantee for any error, so
/// everything after a connection opened `may_have_executed`; the shared recovery planner then
/// never advises a resend. Total over every observation; never panics.
pub fn classify(observation: TransportObservation) -> GoogleOutcome {
    use DeliveryState::{MayHaveExecuted, NotSent};
    let failure = |class, delivery, evidence| {
        GoogleOutcome::Failure(GoogleFailure {
            class,
            delivery,
            evidence,
        })
    };
    match observation {
        // Receipt v1 has no network class; `timeout` is the class whose not-sent advice
        // (a bounded retry under fresh authorization) fits a connection that never opened.
        TransportObservation::NotSent => failure(
            FailureClass::Timeout,
            NotSent,
            FailureEvidence::NotConnected,
        ),
        TransportObservation::LostAfterSend(cause) => {
            let class = match cause {
                LostCause::Timeout => FailureClass::Timeout,
                LostCause::ConnectionError => FailureClass::Permanent,
            };
            failure(class, MayHaveExecuted, FailureEvidence::Lost { cause })
        }
        TransportObservation::Responded { status, body } if (200..300).contains(&status) => {
            match decode_audio(body) {
                Ok(audio) => GoogleOutcome::Output(audio),
                Err(defect) => failure(
                    FailureClass::MalformedAudio,
                    MayHaveExecuted,
                    FailureEvidence::Audio {
                        http_status: status,
                        defect,
                    },
                ),
            }
        }
        TransportObservation::Responded { status, body } => failure(
            class_for_status(status),
            MayHaveExecuted,
            FailureEvidence::Status {
                http_status: status,
                provider_status: provider_status(&body),
            },
        ),
    }
}

/// `RESOURCE_EXHAUSTED` and `DEADLINE_EXCEEDED` have receipt classes; every other non-2xx
/// status, including `UNAVAILABLE`, is `permanent` in receipt v1 and keeps its exact status
/// in the evidence.
const fn class_for_status(status: u16) -> FailureClass {
    match status {
        429 => FailureClass::RateLimit,
        504 => FailureClass::Timeout,
        _ => FailureClass::Permanent,
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

/// Decode `audioContent` and admit it through the shared Google output profile.
pub fn decode_audio(body: ResponseBody) -> Result<GoogleAudio, AudioDefect> {
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
    // The checksum is computed here, so admission checks format, frames and duration.
    let sha256 = format!("{:x}", Sha256::digest(&audio));
    let inspected = inspect_output_for_profile(&audio, &sha256, OutputProfile::Google24Khz)
        .map_err(|error| AudioDefect::Rejected { error })?;
    Ok(GoogleAudio {
        bytes: audio,
        inspected,
    })
}

/// The adapter's private record of one attempt, beside the shared receipt. Credentials,
/// dialogue text and provider error messages are never part of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GoogleObservation {
    pub adapter_version: &'static str,
    pub request_digest: String,
    pub limitations: [&'static str; 4],
    pub estimate: ListPriceEstimate,
    pub failure: Option<GoogleFailure>,
    pub audio: Option<InspectedTtsAudio>,
}

pub fn observation(plan: &GooglePlan, outcome: &GoogleOutcome) -> GoogleObservation {
    let (failure, audio) = match outcome {
        GoogleOutcome::Output(audio) => (None, Some(audio.inspected.clone())),
        GoogleOutcome::Failure(failure) => (Some(failure.clone()), None),
    };
    GoogleObservation {
        adapter_version: ADAPTER_VERSION,
        request_digest: plan.request_digest.clone(),
        limitations: LIMITATIONS,
        estimate: plan.estimate.clone(),
        failure,
        audio,
    }
}
