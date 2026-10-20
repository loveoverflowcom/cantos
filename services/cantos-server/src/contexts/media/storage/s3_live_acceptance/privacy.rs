//! Anonymous private-object diagnostics. Never retain a URL, header value, body or error.
//!
//! A separately constructed client is essential: reqwest adds opaque client-default headers
//! after `RequestBuilder::build`, so auditing a caller-supplied client's request is insufficient.

use futures_util::StreamExt;
use reqwest::{header, redirect, Client, Method, StatusCode, Url};
use sha2::{Digest, Sha256};
use std::{net::IpAddr, time::Duration};

use super::{ObjectIntegrity, Outcome};

const BODY_LIMIT: u64 = 64 * 1024;
const CASE_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub(super) struct PrivacyDiagnostics {
    pub(super) outcome: Outcome,
    pub(super) observations: Vec<(&'static str, String)>,
}

#[derive(Clone, Copy)]
enum Case {
    Exact,
    NoCache,
    Query,
}

impl Case {
    fn label(self) -> &'static str {
        match self {
            Self::Exact => "anonymous_exact",
            Self::NoCache => "anonymous_exact_no_cache",
            Self::Query => "anonymous_query_variant",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CacheStatus {
    Absent,
    Hit,
    Miss,
    Bypass,
    Other,
}

impl CacheStatus {
    fn from_headers(headers: &header::HeaderMap) -> Self {
        let mut values = headers.get_all("x-cache-status").iter();
        let Some(value) = values.next() else {
            return Self::Absent;
        };
        if values.next().is_some() {
            return Self::Other;
        }
        match value.to_str() {
            Ok(value) if value.eq_ignore_ascii_case("HIT") => Self::Hit,
            Ok(value) if value.eq_ignore_ascii_case("MISS") => Self::Miss,
            Ok(value) if value.eq_ignore_ascii_case("BYPASS") => Self::Bypass,
            _ => Self::Other,
        }
    }
}

#[derive(Debug)]
struct ResponseFacts {
    status: u16,
    cache_status: CacheStatus,
    server_present: bool,
    cache_control_present: bool,
    age_present: bool,
    via_present: bool,
    vary_present: bool,
}

impl ResponseFacts {
    fn from_response(response: &reqwest::Response) -> Self {
        let headers = response.headers();
        Self {
            status: response.status().as_u16(),
            cache_status: CacheStatus::from_headers(headers),
            server_present: headers.contains_key(header::SERVER),
            cache_control_present: headers.contains_key(header::CACHE_CONTROL),
            age_present: headers.contains_key(header::AGE),
            via_present: headers.contains_key(header::VIA),
            vary_present: headers.contains_key(header::VARY),
        }
    }

    fn observation(&self) -> String {
        format!(
            "http_status={} cache_status={:?} server_present={} cache_control_present={} age_present={} via_present={} vary_present={}",
            self.status, self.cache_status, self.server_present, self.cache_control_present,
            self.age_present, self.via_present, self.vary_present,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BodyState {
    CompleteMatch,
    CompleteMismatch,
    Incomplete,
    ExceededBound,
}

#[derive(Debug)]
struct BodyFacts {
    state: BodyState,
    observed_bytes: u64,
}

impl BodyFacts {
    fn observation(&self) -> String {
        format!(
            "body={:?} observed_bytes={} sha256_and_length_match={}",
            self.state,
            self.observed_bytes,
            self.state == BodyState::CompleteMatch,
        )
    }
}

async fn bounded_body(response: reqwest::Response, integrity: &ObjectIntegrity) -> BodyFacts {
    let bound = BODY_LIMIT;
    let mut stream = response.bytes_stream();
    let mut observed = 0_u64;
    let mut hash = Sha256::new();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else {
            return BodyFacts {
                state: BodyState::Incomplete,
                observed_bytes: observed,
            };
        };
        let remaining = bound - observed;
        if chunk.len() as u64 > remaining {
            // Do not collect or hash an unbounded response. One sentinel byte proves overrun.
            return BodyFacts {
                state: BodyState::ExceededBound,
                observed_bytes: bound + 1,
            };
        }
        observed += chunk.len() as u64;
        hash.update(&chunk);
    }
    let matches = observed == integrity.byte_length()
        && format!("{:x}", hash.finalize()) == integrity.sha256();
    BodyFacts {
        state: if matches {
            BodyState::CompleteMatch
        } else {
            BodyState::CompleteMismatch
        },
        observed_bytes: observed,
    }
}

fn valid_object_url(url: &Url) -> bool {
    let transport_valid = url.scheme() == "https"
        || (url.scheme() == "http"
            && url
                .host_str()
                .and_then(|host| host.parse::<IpAddr>().ok())
                .is_some_and(|ip| ip.is_loopback()));
    transport_valid
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn request_is_anonymous(request: &reqwest::Request, case: Case) -> bool {
    request.method() == Method::GET
        && request.url().username().is_empty()
        && request.url().password().is_none()
        && request.headers().keys().all(|name| {
            name == header::USER_AGENT
                || (matches!(case, Case::NoCache) && name == header::CACHE_CONTROL)
        })
}

async fn check_case(
    client: &Client,
    url: Url,
    integrity: &ObjectIntegrity,
    case: Case,
) -> (Outcome, String) {
    let mut builder = client
        .get(url)
        .header(header::USER_AGENT, "cantos-private-storage-diagnostic/1");
    if matches!(case, Case::NoCache) {
        builder = builder.header(header::CACHE_CONTROL, "no-cache");
    }
    let Ok(request) = builder.build() else {
        return (
            Outcome::Fail("anonymous_request_build_failed"),
            "request_built=false".to_owned(),
        );
    };
    if !request_is_anonymous(&request, case) {
        return (
            Outcome::Fail("anonymous_request_not_unsigned"),
            "request_unsigned=false".to_owned(),
        );
    }
    // These are the actual built headers of the module-owned client, not an inferred credential
    // flag. The client has no default auth/cookie headers to inject after this audit.
    let audit = format!(
        "request_unsigned=true built_header_count={} authorization_present={} cookie_present={} proxy_authorization_present={} security_token_present={}",
        request.headers().len(),
        request.headers().contains_key(header::AUTHORIZATION),
        request.headers().contains_key(header::COOKIE),
        request.headers().contains_key(header::PROXY_AUTHORIZATION),
        request.headers().contains_key("x-amz-security-token"),
    );
    let Ok(response) = client.execute(request).await else {
        return (
            Outcome::Fail("anonymous_request_transport_failed"),
            format!("{audit} response_received=false"),
        );
    };
    let status = response.status();
    let facts = ResponseFacts::from_response(&response);
    let body = bounded_body(response, integrity).await;
    let outcome = if body.state == BodyState::CompleteMatch {
        // Bytes are exposed even if a broken gateway labels the body 401/403.
        Outcome::Fail("anonymous_object_bytes_exposed")
    } else if status.is_success() {
        // A public success is never privacy acceptance, even for wrong/truncated bytes.
        Outcome::Fail("anonymous_success_response")
    } else {
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                if body.state == BodyState::CompleteMismatch {
                    Outcome::Pass
                } else {
                    Outcome::Fail("anonymous_denial_body_unverified")
                }
            }
            StatusCode::NOT_FOUND => Outcome::Review("anonymous_object_not_found"),
            status if status.is_redirection() => Outcome::Review("anonymous_redirect_not_followed"),
            _ => Outcome::Review("anonymous_unexpected_status"),
        }
    };
    (
        outcome,
        format!("{audit} {} {}", facts.observation(), body.observation()),
    )
}

fn severity(outcome: Outcome) -> u8 {
    match outcome {
        Outcome::Fail(_) => 3,
        Outcome::Review(_) => 2,
        Outcome::NotRun(_) | Outcome::Unsupported(_) | Outcome::UnsupportedByPermission(_) => 1,
        Outcome::Pass => 0,
    }
}

/// Three read-only requests, each with its own total deadline. The original exact URL's
/// failures remain binding; a query-string denial can never turn an exposed object into PASS.
pub(super) async fn anonymous_object_check(
    url: Url,
    integrity: &ObjectIntegrity,
) -> PrivacyDiagnostics {
    let mut diagnostic = PrivacyDiagnostics {
        outcome: Outcome::Pass,
        observations: Vec::new(),
    };
    if !valid_object_url(&url) {
        diagnostic.outcome = Outcome::NotRun("anonymous_object_url_not_unsigned");
        return diagnostic;
    }
    let Ok(client) = Client::builder()
        .https_only(url.scheme() == "https")
        .redirect(redirect::Policy::none())
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .connect_timeout(Duration::from_secs(5))
        .timeout(CASE_DEADLINE)
        .build()
    else {
        diagnostic.outcome = Outcome::NotRun("anonymous_client_unavailable");
        return diagnostic;
    };
    let mut nonce = [0_u8; 16];
    let query_nonce = getrandom::fill(&mut nonce).ok().map(|()| {
        nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    });
    for case in [Case::Exact, Case::NoCache, Case::Query] {
        let mut target = url.clone();
        if matches!(case, Case::Query) {
            let Some(nonce) = &query_nonce else {
                let outcome = Outcome::NotRun("anonymous_query_nonce_unavailable");
                if severity(outcome) > severity(diagnostic.outcome) {
                    diagnostic.outcome = outcome;
                }
                diagnostic
                    .observations
                    .push((case.label(), "request_sent=false".to_owned()));
                continue;
            };
            target
                .query_pairs_mut()
                .append_pair("cantos_privacy_check", nonce);
        }
        let (outcome, observation) =
            match tokio::time::timeout(CASE_DEADLINE, check_case(&client, target, integrity, case))
                .await
            {
                Ok(result) => result,
                Err(_) => (
                    Outcome::Fail("anonymous_request_deadline"),
                    "deadline_exceeded=true".to_owned(),
                ),
            };
        if severity(outcome) > severity(diagnostic.outcome) {
            diagnostic.outcome = outcome;
        }
        diagnostic.observations.push((case.label(), observation));
    }
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::State,
        http::{Request, Response},
        routing::any,
        Router,
    };
    use std::sync::{Arc, Mutex};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    #[derive(Clone)]
    struct Behavior {
        exact_status: StatusCode,
        query_status: StatusCode,
        bytes: Vec<u8>,
        headers: Vec<(&'static str, &'static str)>,
        seen: Arc<Mutex<Vec<(Method, bool, header::HeaderMap)>>>,
    }

    struct Running {
        url: Url,
        seen: Arc<Mutex<Vec<(Method, bool, header::HeaderMap)>>>,
        handle: JoinHandle<()>,
    }

    impl Drop for Running {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    async fn reply(State(state): State<Behavior>, request: Request<Body>) -> Response<Body> {
        let query = request.uri().query().is_some();
        state.seen.lock().unwrap().push((
            request.method().clone(),
            query,
            request.headers().clone(),
        ));
        let status = if query {
            state.query_status
        } else {
            state.exact_status
        };
        let mut response = Response::builder().status(status);
        for (name, value) in &state.headers {
            response = response.header(*name, *value);
        }
        let bytes = if query && !status.is_success() {
            vec![]
        } else {
            state.bytes
        };
        response.body(Body::from(bytes)).unwrap()
    }

    async fn server(
        status: StatusCode,
        query_status: StatusCode,
        bytes: Vec<u8>,
        headers: Vec<(&'static str, &'static str)>,
    ) -> Running {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let state = Behavior {
            exact_status: status,
            query_status,
            bytes,
            headers,
            seen: seen.clone(),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!(
            "http://{}/private-object-sentinel",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().fallback(any(reply)).with_state(state),
            )
            .await
            .unwrap();
        });
        Running { url, seen, handle }
    }

    #[tokio::test]
    async fn exact_cached_private_bytes_remain_a_failure_when_query_is_denied() {
        let bytes = b"owned private probe".to_vec();
        let peer = server(
            StatusCode::OK,
            StatusCode::FORBIDDEN,
            bytes.clone(),
            vec![("x-cache-status", "HIT")],
        )
        .await;
        let integrity = ObjectIntegrity::for_bytes(&bytes).unwrap();
        let result = anonymous_object_check(peer.url.clone(), &integrity).await;
        assert_eq!(
            result.outcome,
            Outcome::Fail("anonymous_object_bytes_exposed")
        );
        assert_eq!(result.observations.len(), 3);
        assert!(result.observations[0].1.contains("cache_status=Hit"));
        assert!(result.observations[0]
            .1
            .contains("sha256_and_length_match=true"));
        assert!(result.observations[2].1.contains("http_status=403"));
        let seen = peer.seen.lock().unwrap();
        assert_eq!(seen.len(), 3);
        for (method, _, headers) in seen.iter() {
            assert_eq!(*method, Method::GET);
            for forbidden in [
                "authorization",
                "cookie",
                "proxy-authorization",
                "x-amz-security-token",
            ] {
                assert!(!headers.contains_key(forbidden));
            }
        }
        assert!(!seen[0].1 && !seen[1].1 && seen[2].1);
        assert!(!seen[0].2.contains_key(header::CACHE_CONTROL));
        assert_eq!(seen[1].2[header::CACHE_CONTROL], "no-cache");
    }

    #[tokio::test]
    async fn complete_denials_pass_but_not_found_and_redirects_never_pass() {
        let integrity = ObjectIntegrity::for_bytes(b"private").unwrap();
        for status in [
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::FOUND,
        ] {
            let peer = server(
                status,
                status,
                vec![],
                vec![("location", "/redirect-private-sentinel")],
            )
            .await;
            let result = anonymous_object_check(peer.url.clone(), &integrity).await;
            if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
                assert_eq!(result.outcome, Outcome::Pass);
            } else {
                assert!(matches!(result.outcome, Outcome::Review(_)));
            }
            assert_eq!(peer.seen.lock().unwrap().len(), 3);
            assert!(!format!("{result:?}").contains("redirect-private-sentinel"));
        }
    }

    #[tokio::test]
    async fn every_success_status_fails_even_for_wrong_or_oversized_bytes() {
        let integrity = ObjectIntegrity::for_bytes(b"private").unwrap();
        for (status, bytes) in [
            (StatusCode::OK, b"wrong!!".to_vec()),
            (StatusCode::PARTIAL_CONTENT, b"private".to_vec()),
            (StatusCode::NO_CONTENT, vec![]),
            (StatusCode::OK, vec![0; BODY_LIMIT as usize + 1]),
        ] {
            let peer = server(status, StatusCode::FORBIDDEN, bytes, vec![]).await;
            let result = anonymous_object_check(peer.url.clone(), &integrity).await;
            assert!(matches!(result.outcome, Outcome::Fail(_)));
            assert_eq!(peer.seen.lock().unwrap().len(), 3);
            if status == StatusCode::OK {
                assert!(!matches!(result.outcome, Outcome::Pass));
            }
        }
    }

    #[tokio::test]
    async fn denial_status_cannot_hide_exact_private_bytes_or_an_oversized_body() {
        let integrity = ObjectIntegrity::for_bytes(b"private").unwrap();
        for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
            let peer = server(status, status, b"private".to_vec(), vec![]).await;
            let result = anonymous_object_check(peer.url.clone(), &integrity).await;
            assert_eq!(
                result.outcome,
                Outcome::Fail("anonymous_object_bytes_exposed")
            );
            let peer = server(status, status, vec![0; BODY_LIMIT as usize + 1], vec![]).await;
            let result = anonymous_object_check(peer.url.clone(), &integrity).await;
            assert_eq!(
                result.outcome,
                Outcome::Fail("anonymous_denial_body_unverified")
            );
        }
    }

    #[tokio::test]
    async fn absolute_body_limit_bounds_even_a_larger_expected_object() {
        let bytes = vec![0; BODY_LIMIT as usize + 2];
        let integrity = ObjectIntegrity::for_bytes(&bytes).unwrap();
        let peer = server(StatusCode::OK, StatusCode::FORBIDDEN, bytes, vec![]).await;
        let result = anonymous_object_check(peer.url.clone(), &integrity).await;
        assert_eq!(result.outcome, Outcome::Fail("anonymous_success_response"));
        assert!(result.observations[0]
            .1
            .contains("body=ExceededBound observed_bytes=65537"));
        assert!(!result.observations[0]
            .1
            .contains("sha256_and_length_match=true"));
    }

    #[tokio::test]
    async fn truncated_success_body_is_never_verified_or_accepted() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!(
            "http://{}/private-object",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let task = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut received = Vec::new();
                while !received.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut chunk = [0_u8; 1024];
                    let count = socket.read(&mut chunk).await.unwrap();
                    assert_ne!(count, 0);
                    received.extend_from_slice(&chunk[..count]);
                    assert!(received.len() <= 8192);
                }
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\npri",
                    )
                    .await
                    .unwrap();
                socket.shutdown().await.unwrap();
            }
        });
        let integrity = ObjectIntegrity::for_bytes(b"private").unwrap();
        let result = anonymous_object_check(url, &integrity).await;
        task.await.unwrap();
        assert_eq!(result.outcome, Outcome::Fail("anonymous_success_response"));
        assert!(result
            .observations
            .iter()
            .all(|(_, text)| text.contains("body=Incomplete")));
    }

    #[tokio::test]
    async fn truncated_denial_body_is_not_privacy_acceptance() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = Url::parse(&format!(
            "http://{}/private-object",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let task = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut received = Vec::new();
                while !received.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut chunk = [0_u8; 1024];
                    let count = socket.read(&mut chunk).await.unwrap();
                    assert_ne!(count, 0);
                    received.extend_from_slice(&chunk[..count]);
                    assert!(received.len() <= 8192);
                }
                socket.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 7\r\nConnection: close\r\n\r\npri").await.unwrap();
                socket.shutdown().await.unwrap();
            }
        });
        let integrity = ObjectIntegrity::for_bytes(b"private").unwrap();
        let result = anonymous_object_check(url, &integrity).await;
        task.await.unwrap();
        assert_eq!(
            result.outcome,
            Outcome::Fail("anonymous_denial_body_unverified")
        );
        assert!(result
            .observations
            .iter()
            .all(|(_, text)| text.contains("body=Incomplete")));
    }

    #[test]
    fn duplicate_or_reflected_cache_headers_are_only_other() {
        let mut headers = header::HeaderMap::new();
        assert_eq!(CacheStatus::from_headers(&headers), CacheStatus::Absent);
        headers.append("x-cache-status", header::HeaderValue::from_static("HIT"));
        headers.append(
            "x-cache-status",
            header::HeaderValue::from_static("credential-sentinel"),
        );
        assert_eq!(CacheStatus::from_headers(&headers), CacheStatus::Other);
        assert!(
            !format!("{:?}", CacheStatus::from_headers(&headers)).contains("credential-sentinel")
        );
    }

    #[tokio::test]
    async fn reflected_headers_and_error_bodies_never_enter_observations() {
        const SENTINEL: &str = "credential-reflection-sentinel";
        let peer = server(
            StatusCode::OK,
            StatusCode::FORBIDDEN,
            SENTINEL.as_bytes().to_vec(),
            vec![
                ("server", SENTINEL),
                ("x-cache-status", SENTINEL),
                ("age", SENTINEL),
                ("via", SENTINEL),
                ("vary", SENTINEL),
                ("cache-control", SENTINEL),
                ("set-cookie", SENTINEL),
                (
                    "location",
                    "https://secret.example/credential-reflection-sentinel",
                ),
            ],
        )
        .await;
        let integrity = ObjectIntegrity::for_bytes(b"private").unwrap();
        let result = anonymous_object_check(peer.url.clone(), &integrity).await;
        let output = format!("{result:?}");
        assert!(!output.contains(SENTINEL));
        assert!(!output.contains("secret.example"));
        assert!(!output.contains("private-object-sentinel"));
        assert!(output.contains("cache_status=Other"));
        assert!(output.contains("age_present=true via_present=true vary_present=true"));
    }

    #[tokio::test]
    async fn input_userinfo_query_and_fragment_are_rejected_before_any_request() {
        let peer = server(StatusCode::FORBIDDEN, StatusCode::FORBIDDEN, vec![], vec![]).await;
        let integrity = ObjectIntegrity::for_bytes(b"private").unwrap();
        let mut userinfo = peer.url.clone();
        userinfo.set_username("secret-sentinel").unwrap();
        let mut query = peer.url.clone();
        query.set_query(Some("X-Amz-Signature=secret-sentinel"));
        let mut fragment = peer.url.clone();
        fragment.set_fragment(Some("secret-sentinel"));
        for url in [userinfo, query, fragment] {
            let result = anonymous_object_check(url, &integrity).await;
            assert_eq!(
                result.outcome,
                Outcome::NotRun("anonymous_object_url_not_unsigned")
            );
            assert!(!format!("{result:?}").contains("secret-sentinel"));
        }
        assert!(peer.seen.lock().unwrap().is_empty());
    }
}
