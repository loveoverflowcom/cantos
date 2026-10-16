//! Candidate REST shapes exercised only against literal loopback fixtures. Neither REST
//! response proves the local project, operation or epoch. An opaque API key cannot prove
//! its project association; operator/provider acceptance remains a separate prerequisite.
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};

use reqwest::{header, Client, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;
use url::Url;

use super::core::{self, ActionProof, Binding, ConfirmationError, Record, Refusal};

const CANDIDATE_ENDPOINT: &str = "https://identitytoolkit.googleapis.com/v1/";
const MAX_RESPONSE_BYTES: usize = 16_384;

struct ApiKey(String);
struct ActionCode(String);

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey([REDACTED])")
    }
}

impl std::fmt::Debug for ActionCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ActionCode([REDACTED])")
    }
}

impl ActionCode {
    fn parse(value: &str) -> Result<Self, Refusal> {
        if value.is_empty() || value.len() > 4096 || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(Refusal::InvalidCode);
        }
        Ok(Self(value.to_owned()))
    }
}

#[derive(Debug)]
struct CandidateTransport {
    client: Client,
    base: Url,
    key: ApiKey,
}

impl CandidateTransport {
    fn candidate(api_key: &str) -> Result<Self, Refusal> {
        if api_key.is_empty()
            || api_key.len() > 512
            || !api_key.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Refusal::Protocol);
        }
        Self::build(
            Url::parse(CANDIDATE_ENDPOINT).map_err(|_| Refusal::Protocol)?,
            ApiKey(api_key.to_owned()),
            Duration::from_secs(5),
        )
    }

    fn build(base: Url, key: ApiKey, timeout: Duration) -> Result<Self, Refusal> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(timeout)
            // Default rustls certificate/hostname verification is retained.
            .build()
            .map_err(|_| Refusal::Protocol)?;
        Ok(Self { client, base, key })
    }

    /// Private test seam takes a parsed socket, not an arbitrary URL, hostname or env var.
    fn fixture(address: SocketAddr, timeout: Duration) -> Result<Self, Refusal> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(Refusal::Protocol);
        }
        Self::build(
            Url::parse(&format!("http://{address}/v1/")).map_err(|_| Refusal::Protocol)?,
            ApiKey("synthetic-fixture-key".into()),
            timeout,
        )
    }

    fn endpoint(&self, consuming: bool) -> Url {
        let mut url = self.base.clone();
        url.set_path(if consuming {
            "/v1/accounts:update"
        } else {
            "/v1/accounts:resetPassword"
        });
        url.query_pairs_mut().append_pair("key", &self.key.0);
        url
    }

    async fn request<T: DeserializeOwned>(
        &self,
        consuming: bool,
        body: &[u8],
    ) -> Result<T, Refusal> {
        let response = self
            .client
            .post(self.endpoint(consuming))
            .header(header::ACCEPT, "application/json")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CACHE_CONTROL, "no-store")
            .body(body.to_vec())
            .send()
            .await
            .map_err(transport_failure)?;
        decode(response).await
    }

    async fn confirm(
        &self,
        local: &LocalFence,
        binding: &Binding,
        code: ActionCode,
        clock: &FixtureClock,
    ) -> Result<ActionProof, ConfirmationError> {
        {
            let mut record = local
                .0
                .lock()
                .map_err(|_| ConfirmationError::BeforeConsume(Refusal::Protocol))?;
            *record = core::reserve(&record, binding, clock.now())
                .map_err(ConfirmationError::BeforeConsume)?;
        }
        // Exactly the same serialized code is used for the non-consuming purpose/email
        // inspection and the consuming update. No ID token is accepted by this interface.
        let body = serde_json::to_vec(&json!({ "oobCode": code.0 }))
            .map_err(|_| ConfirmationError::BeforeConsume(Refusal::Protocol))?;
        let inspected = self.request::<Inspection>(false, &body).await;
        let checked = {
            let mut record = local
                .0
                .lock()
                .map_err(|_| ConfirmationError::BeforeConsume(Refusal::Protocol))?;
            let result = inspected.and_then(|value| {
                core::inspect(
                    &record,
                    binding,
                    clock.now(),
                    &value.request_type,
                    &value.email,
                )
            });
            match result {
                Ok((next, action)) => {
                    *record = next;
                    action
                }
                Err(error) => {
                    *record = record.release_inspection(binding);
                    return Err(ConfirmationError::BeforeConsume(error));
                }
            }
        };
        // There is deliberately no retry loop. A dropped future leaves Consuming fenced.
        let consumed = self.request::<Consumption>(true, &body).await;
        let mut record = local
            .0
            .lock()
            .map_err(|_| ConfirmationError::ConsumeUncertain(Refusal::Protocol))?;
        let result = consumed.and_then(|value| {
            core::finish(
                &record,
                checked,
                clock.now(),
                &value.local_id,
                &value.email,
                value.email_verified,
            )
        });
        match result {
            Ok((next, proof)) => {
                *record = next;
                Ok(proof)
            }
            Err(error) => {
                *record = record.consume_uncertain(binding);
                Err(ConfirmationError::ConsumeUncertain(error))
            }
        }
    }
}

// Mutable fixture infrastructure only. These are not Repository implementations and
// provide no cross-process, transactional, cancellation rollback or crash guarantees.
struct LocalFence(Mutex<Record>);
struct FixtureClock(AtomicU64);
impl FixtureClock {
    fn now(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Inspection {
    email: String,
    request_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Consumption {
    local_id: String,
    email: String,
    email_verified: Option<bool>,
}

fn transport_failure(error: reqwest::Error) -> Refusal {
    // Never format reqwest::Error: its URL includes the API key.
    if error.is_timeout() {
        Refusal::Timeout
    } else {
        Refusal::Network
    }
}

async fn decode<T: DeserializeOwned>(mut response: Response) -> Result<T, Refusal> {
    let status = response.status();
    if status.is_redirection() {
        return Err(Refusal::Redirect);
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
    {
        return Err(Refusal::TooLarge);
    }
    let json_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json_type {
        return Err(Refusal::NotJson);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport_failure)? {
        if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
            return Err(Refusal::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(Refusal::Quota);
    }
    if status.is_server_error() {
        return Err(Refusal::Provider);
    }
    if !status.is_success() {
        #[derive(Deserialize)]
        struct Envelope {
            error: ProviderError,
        }
        #[derive(Deserialize)]
        struct ProviderError {
            message: String,
        }
        let error: Envelope = serde_json::from_slice(&bytes).map_err(|_| Refusal::Protocol)?;
        return Err(match error.error.message.as_str() {
            "INVALID_OOB_CODE" => Refusal::InvalidCode,
            "EXPIRED_OOB_CODE" => Refusal::Expired,
            "TOO_MANY_ATTEMPTS_TRY_LATER" | "QUOTA_EXCEEDED" => Refusal::Quota,
            _ => Refusal::Rejected,
        });
    }
    serde_json::from_slice(&bytes).map_err(|_| Refusal::Protocol)
}

#[cfg(test)]
mod fixture_tests;
