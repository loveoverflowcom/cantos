//! HTTPS shell for the Google adapter: one POST per call with separate connect and total
//! deadlines, a bounded body and a redacted credential. It decides nothing about the outcome;
//! [`google::classify`](super::google::classify) does.
use std::{fmt, future::Future, time::Duration};

use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    redirect::Policy,
    Client,
};

use super::google::{
    self, GoogleOutcome, GooglePlan, LostCause, ResponseBody, TransportObservation, ENDPOINT,
    MAX_RESPONSE_BYTES,
};

const MAX_ACCESS_TOKEN_BYTES: usize = 4_096;
const QUOTA_PROJECT_HEADER: HeaderName = HeaderName::from_static("x-goog-user-project");

/// Sends a plan's body to the provider. Implementations send exactly once and never retry.
pub trait GoogleTransport {
    fn send(&self, plan: &GooglePlan) -> impl Future<Output = TransportObservation> + Send;
}

/// One attempt: send once, then classify. Repeating is a caller decision taken from the
/// outcome's retry policy, never a loop here.
pub async fn synthesize_once<T: GoogleTransport>(
    transport: &T,
    plan: &GooglePlan,
) -> GoogleOutcome {
    google::classify(transport.send(plan).await)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    InvalidAccessToken,
    InvalidQuotaProject,
}

/// An OAuth 2.0 access token (for example from `gcloud auth print-access-token`). It has no
/// `Display` or `Serialize`, its `Debug` is redacted and its header is marked sensitive.
pub struct AccessToken(HeaderValue);

impl AccessToken {
    pub fn new(token: &str) -> Result<AccessToken, CredentialError> {
        let visible = !token.is_empty()
            && token.len() <= MAX_ACCESS_TOKEN_BYTES
            && token.bytes().all(|b| b.is_ascii_graphic());
        if !visible {
            return Err(CredentialError::InvalidAccessToken);
        }
        let mut value = HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| CredentialError::InvalidAccessToken)?;
        value.set_sensitive(true);
        Ok(AccessToken(value))
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken(<redacted>)")
    }
}

/// The Google Cloud project billed for the request, sent as `x-goog-user-project`.
#[derive(Debug, Clone)]
pub struct QuotaProject(HeaderValue);

impl QuotaProject {
    /// Project IDs are 6–30 lowercase letters, digits or hyphens, starting with a letter and
    /// not ending with a hyphen.
    pub fn new(project: &str) -> Result<QuotaProject, CredentialError> {
        let bytes = project.as_bytes();
        let valid = (6..=30).contains(&bytes.len())
            && bytes[0].is_ascii_lowercase()
            && bytes[bytes.len() - 1] != b'-'
            && bytes
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
        if !valid {
            return Err(CredentialError::InvalidQuotaProject);
        }
        HeaderValue::from_str(project)
            .map(QuotaProject)
            .map_err(|_| CredentialError::InvalidQuotaProject)
    }
}

#[derive(Debug)]
pub struct GoogleCredential {
    pub access_token: AccessToken,
    pub quota_project: Option<QuotaProject>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportLimits {
    /// Bounds TCP and TLS setup; failing inside it proves nothing was sent.
    pub connect_timeout: Duration,
    /// Bounds the whole exchange; failing after connect leaves the outcome unknown.
    pub total_timeout: Duration,
    pub max_response_bytes: usize,
}

pub const DEFAULT_LIMITS: TransportLimits = TransportLimits {
    connect_timeout: Duration::from_secs(10),
    total_timeout: Duration::from_secs(60),
    max_response_bytes: MAX_RESPONSE_BYTES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportSetupError;

#[derive(Debug)]
pub struct GoogleHttpTransport {
    client: Client,
    endpoint: String,
    credential: GoogleCredential,
    max_response_bytes: usize,
}

impl GoogleHttpTransport {
    /// HTTPS only, to the documented endpoint, without following redirects.
    pub fn new(credential: GoogleCredential) -> Result<GoogleHttpTransport, TransportSetupError> {
        GoogleHttpTransport::build(ENDPOINT.into(), credential, DEFAULT_LIMITS, true)
    }

    fn build(
        endpoint: String,
        credential: GoogleCredential,
        limits: TransportLimits,
        https_only: bool,
    ) -> Result<GoogleHttpTransport, TransportSetupError> {
        let client = Client::builder()
            .connect_timeout(limits.connect_timeout)
            .timeout(limits.total_timeout)
            // Pinned reqwest 0.12.15 can internally replay HTTP/2 protocol failures. Keep this
            // paid POST on HTTP/1.1 even if another dependency enables the HTTP/2 feature.
            .http1_only()
            // Avoid idle pooled-connection cancellation and its library reconnection path.
            .pool_max_idle_per_host(0)
            // Keep the resource cap about actual response bytes if features are unified later.
            .no_gzip()
            .no_brotli()
            .no_zstd()
            .no_deflate()
            .redirect(Policy::none())
            .https_only(https_only)
            .build()
            .map_err(|_| TransportSetupError)?;
        Ok(GoogleHttpTransport {
            client,
            endpoint,
            credential,
            max_response_bytes: limits.max_response_bytes,
        })
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        );
        headers.insert(AUTHORIZATION, self.credential.access_token.0.clone());
        if let Some(project) = &self.credential.quota_project {
            headers.insert(QUOTA_PROJECT_HEADER, project.0.clone());
        }
        headers
    }

    async fn read_body(&self, mut response: reqwest::Response) -> ResponseBody {
        let limit = self.max_response_bytes;
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return ResponseBody::TooLarge;
        }
        let mut body = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) if chunk.len() > limit.saturating_sub(body.len()) => {
                    return ResponseBody::TooLarge
                }
                Ok(Some(chunk)) => body.extend_from_slice(&chunk),
                Ok(None) => return ResponseBody::Complete(body),
                Err(_) => return ResponseBody::Interrupted,
            }
        }
    }
}

impl GoogleTransport for GoogleHttpTransport {
    async fn send(&self, plan: &GooglePlan) -> TransportObservation {
        let sent = self
            .client
            .post(&self.endpoint)
            .headers(self.headers())
            .body(plan.body().to_vec())
            .send()
            .await;
        match sent {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = self.read_body(response).await;
                TransportObservation::Responded { status, body }
            }
            Err(error) => unanswered(&error),
        }
    }
}

/// A connect error happens before any request byte is written; everything later may have
/// reached the provider.
fn unanswered(error: &reqwest::Error) -> TransportObservation {
    if error.is_connect() || error.is_builder() {
        TransportObservation::NotSent
    } else if error.is_timeout() {
        TransportObservation::LostAfterSend(LostCause::Timeout)
    } else {
        TransportObservation::LostAfterSend(LostCause::ConnectionError)
    }
}

#[cfg(test)]
mod tests {
    //! A scripted loopback server drives the real client; it is not the provider.
    use super::*;
    use crate::tts::attempt::{DeliveryState, FailureClass};
    use crate::tts::google::plan;
    use crate::tts::google_contract::{ADAPTER_VERSION, OUTPUT_CONTRACT, PROVIDER_ID};
    use base64::Engine as _;
    use cantos_contracts::{ResolvedSynthesisInput, SynthesisPerformance};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        task::JoinHandle,
    };

    const TOKEN: &str = "ya29.sentinel-token-never-logged";

    enum Reply {
        Bytes(Vec<u8>),
        Stall(Duration),
        PartialThenStall(Vec<u8>, Duration),
        Close,
    }

    fn sample_plan() -> GooglePlan {
        plan(&ResolvedSynthesisInput {
            dialogue_id: "dialogue-01".into(),
            character_id: "narrator".into(),
            text: "Ánh đèn cuối cùng còn sáng trên sân khấu.".into(),
            language: "vi-VN".into(),
            provider_id: PROVIDER_ID.into(),
            model_id: "google-neural2".into(),
            voice_id: "vi-VN-Neural2-A".into(),
            voice_rights_record_id: "voice-narrator".into(),
            adapter_version: ADAPTER_VERSION.into(),
            output_contract: OUTPUT_CONTRACT.into(),
            performance: SynthesisPerformance {
                rate_permille: 1_000,
                pitch_semitones: 0,
                emotion: Some("neutral".into()),
                intensity_permille: Some(0),
            },
            pronunciation: Vec::new(),
        })
        .unwrap()
    }

    fn transport(port: u16, limits: TransportLimits) -> GoogleHttpTransport {
        let credential = GoogleCredential {
            access_token: AccessToken::new(TOKEN).unwrap(),
            quota_project: Some(QuotaProject::new("cantos-test-project").unwrap()),
        };
        let endpoint = format!("http://127.0.0.1:{port}/v1/text:synthesize");
        GoogleHttpTransport::build(endpoint, credential, limits, false).unwrap()
    }

    fn fast_limits() -> TransportLimits {
        TransportLimits {
            connect_timeout: Duration::from_secs(2),
            total_timeout: Duration::from_millis(400),
            max_response_bytes: 1_024,
        }
    }

    async fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4_096];
        loop {
            let read = stream.read(&mut buffer).await.unwrap();
            request.extend_from_slice(&buffer[..read]);
            let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") else {
                assert!(read > 0, "client closed before sending headers");
                continue;
            };
            let head = String::from_utf8_lossy(&request[..end]).to_ascii_lowercase();
            let length: usize = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .map_or(0, |value| value.trim().parse().unwrap());
            if request.len() >= end + 4 + length || read == 0 {
                return request;
            }
        }
    }

    async fn serve_once(reply: Reply) -> (u16, JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            send_reply(stream, reply).await;
            request
        });
        (port, server)
    }

    async fn send_reply(mut stream: TcpStream, reply: Reply) {
        match reply {
            Reply::Bytes(bytes) => {
                let _ = stream.write_all(&bytes).await;
                let _ = stream.shutdown().await;
            }
            Reply::Stall(duration) => tokio::time::sleep(duration).await,
            Reply::PartialThenStall(bytes, duration) => {
                let _ = stream.write_all(&bytes).await;
                tokio::time::sleep(duration).await;
            }
            Reply::Close => drop(stream),
        }
    }

    /// The server keeps an independent request ledger and remains available after the first
    /// outcome. A client resend would be observed rather than hidden by a closed listener.
    async fn serve_counted(reply: Reply) -> (u16, JoinHandle<Vec<Vec<u8>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut requests = vec![read_request(&mut stream).await];
            send_reply(stream, reply).await;
            while let Ok(Ok((mut stream, _))) =
                tokio::time::timeout(Duration::from_millis(500), listener.accept()).await
            {
                requests.push(read_request(&mut stream).await);
                drop(stream);
            }
            requests
        });
        (port, server)
    }

    fn response(status: &str, body: &[u8]) -> Vec<u8> {
        let mut bytes = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        bytes.extend_from_slice(body);
        bytes
    }

    #[tokio::test]
    async fn refused_connection_is_not_sent() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let observation = transport(port, fast_limits()).send(&sample_plan()).await;
        assert_eq!(observation, TransportObservation::NotSent);
    }

    #[tokio::test]
    async fn request_carries_exact_body_and_credential_headers_once() {
        let body = br#"{"error":{"code":429,"message":"echo","status":"RESOURCE_EXHAUSTED"}}"#;
        let (port, server) =
            serve_once(Reply::Bytes(response("429 Too Many Requests", body))).await;
        let plan = sample_plan();
        let observation = transport(port, fast_limits()).send(&plan).await;
        assert_eq!(
            observation,
            TransportObservation::Responded {
                status: 429,
                body: ResponseBody::Complete(body.to_vec()),
            }
        );
        let request = server.await.unwrap();
        let end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8(request[..end].to_vec()).unwrap();
        let head_lower = head.to_ascii_lowercase();
        assert!(head.starts_with("POST /v1/text:synthesize HTTP/1.1\r\n"));
        assert!(head.contains(&format!("authorization: Bearer {TOKEN}")));
        assert!(head_lower.contains("x-goog-user-project: cantos-test-project"));
        assert!(head_lower.contains("content-type: application/json; charset=utf-8"));
        assert_eq!(&request[end + 4..], plan.body());
    }

    #[tokio::test]
    async fn timeout_after_the_request_is_written_is_lost_not_unsent() {
        let (port, server) = serve_once(Reply::Stall(Duration::from_secs(2))).await;
        let observation = transport(port, fast_limits()).send(&sample_plan()).await;
        assert_eq!(
            observation,
            TransportObservation::LostAfterSend(LostCause::Timeout)
        );
        assert!(
            !server.await.unwrap().is_empty(),
            "the request reached the server"
        );
    }

    #[tokio::test]
    async fn connection_closed_after_the_request_is_lost() {
        let (port, server) = serve_once(Reply::Close).await;
        let observation = transport(port, fast_limits()).send(&sample_plan()).await;
        assert_eq!(
            observation,
            TransportObservation::LostAfterSend(LostCause::ConnectionError)
        );
        assert!(!server.await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn charged_or_uncertain_http_outcomes_are_never_replayed() {
        let partial = b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\n{\"audioContent\":".to_vec();
        let cases = [
            (
                Reply::Bytes(response(
                    "429 Too Many Requests",
                    br#"{"error":{"status":"RESOURCE_EXHAUSTED"}}"#,
                )),
                FailureClass::RateLimit,
            ),
            (
                Reply::Bytes(response(
                    "503 Service Unavailable",
                    br#"{"error":{"status":"UNAVAILABLE"}}"#,
                )),
                FailureClass::Permanent,
            ),
            (Reply::Close, FailureClass::Permanent),
            (
                Reply::Stall(Duration::from_millis(600)),
                FailureClass::Timeout,
            ),
            (
                Reply::PartialThenStall(partial.clone(), Duration::from_millis(600)),
                FailureClass::MalformedAudio,
            ),
            (Reply::Bytes(partial), FailureClass::MalformedAudio),
            (
                Reply::Bytes(response("200 OK", br#"{"audioContent":""}"#)),
                FailureClass::MalformedAudio,
            ),
        ];
        for (reply, expected_class) in cases {
            let (port, server) = serve_counted(reply).await;
            let plan = sample_plan();
            let outcome = synthesize_once(&transport(port, fast_limits()), &plan).await;
            let GoogleOutcome::Failure(failure) = outcome else {
                panic!("expected failure");
            };
            assert_eq!(failure.class, expected_class);
            assert_eq!(failure.delivery, DeliveryState::MayHaveExecuted);
            let requests = server.await.unwrap();
            assert_eq!(requests.len(), 1, "{failure:?}");
            let request = &requests[0];
            let end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
            assert_eq!(&request[end + 4..], plan.body());
        }
    }

    #[tokio::test]
    async fn http_success_is_decoded_and_inspected_by_the_production_core() {
        // Synthetic nonempty silence, not a provider sample or listening-quality oracle.
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&132_u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        for value in [1_u16, 1] {
            wav.extend_from_slice(&value.to_le_bytes());
        }
        for value in [24_000_u32, 48_000] {
            wav.extend_from_slice(&value.to_le_bytes());
        }
        for value in [2_u16, 16] {
            wav.extend_from_slice(&value.to_le_bytes());
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&96_u32.to_le_bytes());
        wav.extend_from_slice(&[0_u8; 96]);
        let encoded = base64::engine::general_purpose::STANDARD.encode(&wav);
        let body = format!(r#"{{"audioContent":"{encoded}"}}"#).into_bytes();
        let (port, server) = serve_once(Reply::Bytes(response("200 OK", &body))).await;
        let outcome = synthesize_once(&transport(port, fast_limits()), &sample_plan()).await;
        let GoogleOutcome::Output(audio) = outcome else {
            panic!("expected inspected output");
        };
        assert_eq!(audio.bytes(), wav.as_slice());
        assert_eq!(audio.inspected().frames(), 48);
        assert_eq!(audio.inspected().sample_rate_hz(), 24_000);
        assert_eq!(
            audio.inspected().output_profile_version(),
            "cantos-google-tts-pcm-1"
        );
        assert!(!server.await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn declared_oversized_body_is_refused_before_buffering() {
        let reply = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n", 1_025).into_bytes();
        let (port, _server) = serve_once(Reply::Bytes(reply)).await;
        let observation = transport(port, fast_limits()).send(&sample_plan()).await;
        assert_eq!(
            observation,
            TransportObservation::Responded {
                status: 200,
                body: ResponseBody::TooLarge,
            }
        );
    }

    #[tokio::test]
    async fn undeclared_oversized_body_stops_at_the_limit() {
        let mut reply = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n".to_vec();
        for _ in 0..3 {
            reply.extend_from_slice(b"200\r\n");
            reply.extend_from_slice(&[b'a'; 0x200]);
            reply.extend_from_slice(b"\r\n");
        }
        reply.extend_from_slice(b"0\r\n\r\n");
        let (port, _server) = serve_once(Reply::Bytes(reply)).await;
        let observation = transport(port, fast_limits()).send(&sample_plan()).await;
        assert_eq!(
            observation,
            TransportObservation::Responded {
                status: 200,
                body: ResponseBody::TooLarge,
            }
        );
    }

    #[tokio::test]
    async fn body_cut_off_after_the_status_is_interrupted() {
        let reply = b"HTTP/1.1 200 OK\r\ncontent-length: 100\r\n\r\n{\"audioContent\":".to_vec();
        let (port, _server) = serve_once(Reply::Bytes(reply)).await;
        let observation = transport(port, fast_limits()).send(&sample_plan()).await;
        assert_eq!(
            observation,
            TransportObservation::Responded {
                status: 200,
                body: ResponseBody::Interrupted,
            }
        );
    }

    #[tokio::test]
    async fn redirects_are_reported_not_followed() {
        let reply =
            b"HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:9/elsewhere\r\ncontent-length: 0\r\n\r\n"
                .to_vec();
        let (port, _server) = serve_once(Reply::Bytes(reply)).await;
        let observation = transport(port, fast_limits()).send(&sample_plan()).await;
        assert_eq!(
            observation,
            TransportObservation::Responded {
                status: 302,
                body: ResponseBody::Complete(Vec::new()),
            }
        );
    }

    #[tokio::test]
    async fn https_only_transport_refuses_plain_http_before_connecting() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let credential = GoogleCredential {
            access_token: AccessToken::new(TOKEN).unwrap(),
            quota_project: None,
        };
        let endpoint = format!("http://127.0.0.1:{port}/v1/text:synthesize");
        let transport =
            GoogleHttpTransport::build(endpoint, credential, fast_limits(), true).unwrap();
        assert_eq!(
            transport.send(&sample_plan()).await,
            TransportObservation::NotSent
        );
        let accepted = tokio::time::timeout(Duration::from_millis(200), listener.accept()).await;
        assert!(accepted.is_err(), "no connection may be opened");
    }

    #[test]
    fn production_transport_targets_the_documented_https_endpoint() {
        let credential = GoogleCredential {
            access_token: AccessToken::new(TOKEN).unwrap(),
            quota_project: None,
        };
        let transport = GoogleHttpTransport::new(credential).unwrap();
        assert_eq!(
            transport.endpoint,
            "https://texttospeech.googleapis.com/v1/text:synthesize"
        );
    }

    #[test]
    fn credentials_never_appear_in_debug_output() {
        let credential = GoogleCredential {
            access_token: AccessToken::new(TOKEN).unwrap(),
            quota_project: None,
        };
        let transport = GoogleHttpTransport::new(credential).unwrap();
        let rendered = format!("{transport:?}");
        assert!(!rendered.contains("sentinel-token"), "{rendered}");
        assert!(rendered.contains("AccessToken(<redacted>)"));
    }

    #[test]
    fn malformed_credentials_are_refused() {
        for token in ["", "has space", "line\nbreak", "tab\t", "ü-non-ascii"] {
            assert_eq!(
                AccessToken::new(token).map(|_| ()),
                Err(CredentialError::InvalidAccessToken),
                "{token:?}"
            );
        }
        assert!(AccessToken::new(&"a".repeat(MAX_ACCESS_TOKEN_BYTES + 1)).is_err());
        for project in [
            "short",
            "Upper-case1",
            "1starts-digit",
            "ends-with-",
            &"a".repeat(31),
        ] {
            assert_eq!(
                QuotaProject::new(project).map(|_| ()),
                Err(CredentialError::InvalidQuotaProject),
                "{project:?}"
            );
        }
        assert!(QuotaProject::new("cantos-dev-2026").is_ok());
    }
}
