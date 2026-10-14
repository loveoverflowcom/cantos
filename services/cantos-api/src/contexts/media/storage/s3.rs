//! Generic, private, bounded S3-compatible storage. There are no vendor branches, ACL changes,
//! bucket-policy changes or unsafe HEAD-then-PUT emulations in this adapter.

use std::{fmt, sync::Arc, time::Duration};

use bytes::Bytes;
use chrono::Utc;
use futures_util::StreamExt;
use reqwest::{header, redirect, Client, Method, Response, StatusCode, Url};
use sha2::{Digest, Sha256};

use crate::contexts::media::domain::object_storage::{
    NormalizedByteRange, ObjectIntegrity, ObjectKey, StorageCapability, StorageError,
    MAX_OBJECT_BYTES,
};
use crate::contexts::media::image_intake::VerifiedImage;

use super::{
    port::{PrivateStorage, StorageFuture, StorageRead, TemporaryRead, VerifiedPayload},
    sigv4::{canonical_query, Signer, EMPTY_PAYLOAD_SHA256},
};

// Historical audio locations use this fixed private namespace. Images share it so reads keep
// selecting only the recorded target/key; MIME never becomes a caller-selected path segment.
const PRIVATE_PREFIX: &str = "private/audio";

/// Non-secret transport options. The composition root resolves secret injection and chooses
/// targets; this adapter never reads environment variables or changes a target's identity.
#[derive(Clone)]
pub(crate) struct S3Settings {
    pub(crate) endpoint: String,
    pub(crate) region: String,
    pub(crate) bucket: String,
    pub(crate) force_path_style: bool,
    pub(crate) connect_timeout_ms: u64,
    pub(crate) request_timeout_ms: u64,
    pub(crate) conditional_put: bool,
    pub(crate) range_reads: bool,
    pub(crate) presigned_reads: bool,
}

impl fmt::Debug for S3Settings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("S3Settings([internal locations redacted])")
    }
}

impl S3Settings {
    /// Validate topology before credentials are resolved or a transport client is constructed.
    pub(crate) fn validate(&self) -> Result<(), StorageError> {
        validate_settings(self, false).map(|_| ())
    }
}

#[derive(Clone)]
pub(crate) struct S3Storage {
    client: Client,
    endpoint: Url,
    settings: S3Settings,
    signer: Signer,
}

impl fmt::Debug for S3Storage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("S3Storage([internal locations and credentials redacted])")
    }
}

/// Opaque transport validators, obtained only alongside a fully verified actual-byte GET.
/// An ETag is never interpreted as the object's SHA-256.
struct VerifiedLocation {
    version_id: Option<String>,
    etag: Option<String>,
}

/// Transport clones retain the sealed encoded body and its admission permit. Cancellation
/// cannot free a lane while reqwest/hyper still owns bytes from that admitted image.
struct ImageBody(Arc<VerifiedImage>);

impl AsRef<[u8]> for ImageBody {
    fn as_ref(&self) -> &[u8] {
        self.0.bytes()
    }
}

impl S3Storage {
    pub(crate) fn new(
        settings: S3Settings,
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
    ) -> Result<Self, StorageError> {
        Self::build(settings, access_key, secret_key, session_token, false)
    }

    fn build(
        settings: S3Settings,
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
        allow_test_loopback: bool,
    ) -> Result<Self, StorageError> {
        let endpoint = validate_settings(&settings, allow_test_loopback)?;
        let signer = Signer::new(
            settings.region.clone(),
            access_key,
            secret_key,
            session_token,
        )?;
        let client = Client::builder()
            .https_only(!allow_test_loopback)
            .redirect(redirect::Policy::none())
            .no_proxy()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .connect_timeout(Duration::from_millis(settings.connect_timeout_ms))
            .timeout(Duration::from_millis(settings.request_timeout_ms))
            .read_timeout(Duration::from_millis(settings.request_timeout_ms))
            .build()
            .map_err(|_| StorageError::Unavailable)?;
        Ok(Self {
            client,
            endpoint,
            settings,
            signer,
        })
    }

    fn object_url(&self, key: &ObjectKey) -> Result<Url, StorageError> {
        let mut url = self.endpoint.clone();
        if self.settings.force_path_style {
            url.set_path(&format!(
                "/{}/{PRIVATE_PREFIX}/{}",
                self.settings.bucket,
                key.as_str()
            ));
        } else {
            let host = url.host_str().ok_or(StorageError::InvalidTarget)?;
            let host = format!("{}.{host}", self.settings.bucket);
            url.set_host(Some(&host))
                .map_err(|_| StorageError::InvalidTarget)?;
            url.set_path(&format!("/{PRIVATE_PREFIX}/{}", key.as_str()));
        }
        Ok(url)
    }

    async fn request(
        &self,
        method: Method,
        url: Url,
        payload_hash: &str,
        additional: &[(&str, String)],
        body: Option<Bytes>,
    ) -> Result<Response, StorageError> {
        let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
        let headers =
            self.signer
                .headers(method.as_str(), &url, payload_hash, &timestamp, additional)?;
        let mut request = self.client.request(method, url);
        for (name, value) in headers {
            let name = header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| StorageError::InvalidTarget)?;
            let value =
                header::HeaderValue::from_str(&value).map_err(|_| StorageError::InvalidTarget)?;
            request = request.header(name, value);
        }
        if let Some(body) = body {
            request = request.body(body);
        }
        // Never propagate a reqwest error or response body: both can contain a secret URL,
        // provider details or credentials. Only closed safe reason codes cross this boundary.
        request.send().await.map_err(transport_error)
    }

    async fn put_bytes(
        &self,
        key: ObjectKey,
        integrity: ObjectIntegrity,
        bytes: impl Into<Bytes>,
        content_type: &'static str,
    ) -> Result<(), StorageError> {
        let bytes = bytes.into();
        validate_binding(&key, &integrity)?;
        integrity.verify_bytes(&bytes)?;
        if !self.settings.conditional_put {
            return Err(StorageError::UnsupportedCapability(
                StorageCapability::ConditionalCreate,
            ));
        }
        let response = self
            .request(
                Method::PUT,
                self.object_url(&key)?,
                integrity.sha256(),
                &[
                    ("if-none-match", "*".to_owned()),
                    ("content-type", content_type.to_owned()),
                    ("content-length", integrity.byte_length().to_string()),
                ],
                Some(bytes),
            )
            .await?;
        match response.status() {
            StatusCode::OK | StatusCode::PRECONDITION_FAILED => {}
            status => {
                return Err(status_error(
                    status,
                    Some(StorageCapability::ConditionalCreate),
                ))
            }
        }
        drop(response);
        // PUT success or a duplicate precondition is not proof of storage integrity. Hash the
        // actual bounded stored bytes before any metadata receipt can be accepted.
        self.verify_location(&key, &integrity).await?;
        Ok(())
    }

    async fn verify_location(
        &self,
        key: &ObjectKey,
        integrity: &ObjectIntegrity,
    ) -> Result<VerifiedLocation, StorageError> {
        validate_binding(key, integrity)?;
        let response = self
            .request(
                Method::GET,
                self.object_url(key)?,
                EMPTY_PAYLOAD_SHA256,
                &[],
                None,
            )
            .await?;
        if response.status() != StatusCode::OK {
            return Err(status_error(response.status(), None));
        }
        validate_response(&response, integrity.byte_length(), None)?;
        let location = VerifiedLocation {
            version_id: usable_version_id(response.headers()),
            etag: strong_etag(response.headers()),
        };
        let mut body = response.bytes_stream();
        let mut sha256 = Sha256::new();
        let mut observed = 0_u64;
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(body_error)?;
            observed = observed
                .checked_add(chunk.len() as u64)
                .ok_or(StorageError::LengthMismatch)?;
            if observed > integrity.byte_length() || observed > MAX_OBJECT_BYTES {
                return Err(StorageError::LengthMismatch);
            }
            sha256.update(&chunk);
        }
        integrity.verify_observed(&format!("{:x}", sha256.finalize()), observed)?;
        Ok(location)
    }

    async fn read_verified(
        &self,
        key: ObjectKey,
        integrity: ObjectIntegrity,
        range: Option<NormalizedByteRange>,
    ) -> Result<StorageRead, StorageError> {
        validate_binding(&key, &integrity)?;
        if let Some(range) = range {
            if range.total_byte_length() != integrity.byte_length() {
                return Err(StorageError::LengthMismatch);
            }
            if !self.settings.range_reads {
                return Err(StorageError::UnsupportedCapability(
                    StorageCapability::RangeRead,
                ));
            }
        }
        let location = self.verify_location(&key, &integrity).await?;
        let mut url = self.object_url(&key)?;
        let mut additional = Vec::new();
        if let Some(version_id) = &location.version_id {
            url.set_query(Some(&canonical_query([(
                "versionId".to_owned(),
                version_id.clone(),
            )])));
        } else if let Some(etag) = &location.etag {
            additional.push(("if-match", etag.clone()));
        } else {
            // A second unpinned request could return bytes changed after verification.
            return Err(StorageError::Unavailable);
        }
        let expected_length = match range {
            Some(range) => {
                additional.push(("range", format!("bytes={}-{}", range.first(), range.last())));
                range.byte_length()
            }
            None => integrity.byte_length(),
        };
        let response = self
            .request(Method::GET, url, EMPTY_PAYLOAD_SHA256, &additional, None)
            .await?;
        let expected_status = match range {
            Some(_) => StatusCode::PARTIAL_CONTENT,
            None => StatusCode::OK,
        };
        if response.status() != expected_status {
            let error = if response.status() == StatusCode::OK && range.is_some() {
                StorageError::UnsupportedCapability(StorageCapability::RangeRead)
            } else {
                status_error(
                    response.status(),
                    range.map(|_| StorageCapability::RangeRead),
                )
            };
            return Err(error);
        }
        validate_response(&response, expected_length, range)?;
        validate_read_pin(response.headers(), &location)?;
        let stream = response.bytes_stream();
        let state = (stream, 0_u64, Sha256::new(), integrity, range.is_none());
        let stream = futures_util::stream::try_unfold(
            state,
            move |(mut body, mut observed, mut hash, integrity, verify_digest)| async move {
                match body.next().await {
                    Some(chunk) => {
                        let chunk = chunk.map_err(body_error)?;
                        observed = observed
                            .checked_add(chunk.len() as u64)
                            .ok_or(StorageError::LengthMismatch)?;
                        if observed > expected_length {
                            return Err(StorageError::LengthMismatch);
                        }
                        if verify_digest {
                            hash.update(&chunk);
                        }
                        Ok(Some((
                            chunk,
                            (body, observed, hash, integrity, verify_digest),
                        )))
                    }
                    None => {
                        if observed != expected_length {
                            return Err(StorageError::LengthMismatch);
                        }
                        if verify_digest {
                            integrity
                                .verify_observed(&format!("{:x}", hash.finalize()), observed)?;
                        }
                        Ok(None)
                    }
                }
            },
        );
        // Dropping this stream drops the transport body and cancels further I/O; no detached
        // transfer keeps running after the requesting client cancels its read.
        Ok(StorageRead {
            stream: Box::pin(stream),
            byte_length: expected_length,
        })
    }

    async fn read_grant(
        &self,
        key: ObjectKey,
        integrity: ObjectIntegrity,
        ttl: u32,
    ) -> Result<Option<TemporaryRead>, StorageError> {
        if !(1..=300).contains(&ttl) {
            return Err(StorageError::InvalidTarget);
        }
        if !self.settings.presigned_reads {
            return Ok(None);
        }
        let location = self.verify_location(&key, &integrity).await?;
        let Some(version_id) = location.version_id else {
            // Ordinary unversioned presigns cannot pin the verified object across future GETs.
            return Ok(None);
        };
        let now = Utc::now();
        let signed = self.signer.presign_get(
            self.object_url(&key)?,
            &now.format("%Y%m%dT%H%M%SZ").to_string(),
            ttl,
            Some(&version_id),
        )?;
        let expires_at = now
            .timestamp()
            .checked_add(i64::from(ttl))
            .and_then(|value| u64::try_from(value).ok())
            .ok_or(StorageError::Unavailable)?;
        Ok(Some(TemporaryRead::new(signed.into(), expires_at)?))
    }
}

impl PrivateStorage for S3Storage {
    fn put_if_absent(&self, payload: VerifiedPayload) -> StorageFuture<()> {
        let storage = self.clone();
        Box::pin(async move {
            let integrity = ObjectIntegrity::new(payload.sha256(), payload.byte_length())?;
            let key = ObjectKey::parse(payload.sha256())?;
            let content_type = payload.content_type();
            match payload {
                VerifiedPayload::Audio(audio) => {
                    storage
                        .put_bytes(key, integrity, audio.into_bytes(), content_type)
                        .await
                }
                VerifiedPayload::Image(image) => {
                    // The local owner retains admission through actual-byte readback; the
                    // Bytes owner independently retains it for every surviving transport clone.
                    let image = Arc::new(image);
                    let body = Bytes::from_owner(ImageBody(image.clone()));
                    let result = storage.put_bytes(key, integrity, body, content_type).await;
                    drop(image);
                    result
                }
            }
        })
    }

    fn verify(&self, key: ObjectKey, integrity: ObjectIntegrity) -> StorageFuture<()> {
        let storage = self.clone();
        Box::pin(async move {
            storage.verify_location(&key, &integrity).await?;
            Ok(())
        })
    }

    fn open_verified(
        &self,
        key: ObjectKey,
        integrity: ObjectIntegrity,
        range: Option<NormalizedByteRange>,
    ) -> StorageFuture<StorageRead> {
        let storage = self.clone();
        Box::pin(async move { storage.read_verified(key, integrity, range).await })
    }

    fn temporary_read(
        &self,
        key: ObjectKey,
        integrity: ObjectIntegrity,
        ttl: u32,
    ) -> StorageFuture<Option<TemporaryRead>> {
        let storage = self.clone();
        Box::pin(async move { storage.read_grant(key, integrity, ttl).await })
    }
}

fn validate_settings(settings: &S3Settings, allow_loopback: bool) -> Result<Url, StorageError> {
    let endpoint = Url::parse(&settings.endpoint).map_err(|_| StorageError::InvalidTarget)?;
    let test_http = allow_loopback
        && endpoint.scheme() == "http"
        && endpoint
            .host_str()
            .and_then(ip_address)
            .is_some_and(|address| address.is_loopback());
    if (endpoint.scheme() != "https" && !test_http)
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || endpoint.path() != "/"
        || !(1..=300_000).contains(&settings.request_timeout_ms)
        || !(1..=60_000).contains(&settings.connect_timeout_ms)
        || settings.connect_timeout_ms > settings.request_timeout_ms
        || !valid_bucket(&settings.bucket)
        || (!settings.force_path_style
            && endpoint
                .host_str()
                .is_some_and(|host| ip_address(host).is_some()))
        || (settings.presigned_reads && !settings.range_reads)
    {
        return Err(StorageError::InvalidTarget);
    }
    // Keep the same checked region shape as the signer, without handling any credentials.
    if settings.region.is_empty()
        || settings.region.len() > 63
        || !settings.region.as_bytes()[0].is_ascii_alphanumeric()
        || !settings.region.as_bytes()[settings.region.len() - 1].is_ascii_alphanumeric()
        || !settings
            .region
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(StorageError::InvalidTarget);
    }
    Ok(endpoint)
}

fn ip_address(host: &str) -> Option<std::net::IpAddr> {
    host.strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
        .parse()
        .ok()
}

fn valid_bucket(bucket: &str) -> bool {
    (3..=63).contains(&bucket.len())
        && bucket.parse::<std::net::Ipv4Addr>().is_err()
        && bucket.split('.').all(|label| {
            !label.is_empty()
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn validate_binding(key: &ObjectKey, integrity: &ObjectIntegrity) -> Result<(), StorageError> {
    if key.as_str() != integrity.sha256() {
        return Err(StorageError::DigestMismatch);
    }
    Ok(())
}

fn validate_response(
    response: &Response,
    expected_length: u64,
    range: Option<NormalizedByteRange>,
) -> Result<(), StorageError> {
    let headers = response.headers();
    let length = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(StorageError::LengthMismatch)?;
    if length != expected_length {
        return Err(StorageError::LengthMismatch);
    }
    if headers.get(header::CONTENT_ENCODING).is_some_and(|value| {
        value
            .to_str()
            .map_or(true, |value| !value.eq_ignore_ascii_case("identity"))
    }) {
        return Err(StorageError::Unavailable);
    }
    match range {
        Some(range) => {
            let expected = format!(
                "bytes {}-{}/{}",
                range.first(),
                range.last(),
                range.total_byte_length()
            );
            if headers
                .get(header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                != Some(expected.as_str())
            {
                return Err(StorageError::LengthMismatch);
            }
        }
        None => {
            if headers.contains_key(header::CONTENT_RANGE) {
                return Err(StorageError::LengthMismatch);
            }
        }
    }
    Ok(())
}

fn usable_version_id(headers: &header::HeaderMap) -> Option<String> {
    if headers.get_all("x-amz-version-id").iter().count() != 1 {
        return None;
    }
    let version = headers.get("x-amz-version-id")?.to_str().ok()?;
    if version.is_empty()
        || version.len() > 1024
        || version.eq_ignore_ascii_case("null")
        || !version.bytes().all(|byte| (b'!'..=b'~').contains(&byte))
    {
        return None;
    }
    Some(version.to_owned())
}

fn strong_etag(headers: &header::HeaderMap) -> Option<String> {
    if headers.get_all(header::ETAG).iter().count() != 1 {
        return None;
    }
    let etag = headers.get(header::ETAG)?.to_str().ok()?;
    let opaque = etag.strip_prefix('"')?.strip_suffix('"')?;
    if opaque.is_empty()
        || opaque.len() > 1024
        || !opaque
            .bytes()
            .all(|byte| byte == b'!' || (b'#'..=b'~').contains(&byte))
    {
        return None;
    }
    Some(etag.to_owned())
}

fn validate_read_pin(
    headers: &header::HeaderMap,
    location: &VerifiedLocation,
) -> Result<(), StorageError> {
    let agrees = if let Some(version) = &location.version_id {
        usable_version_id(headers).as_ref() == Some(version)
    } else if let Some(etag) = &location.etag {
        strong_etag(headers).as_ref() == Some(etag)
    } else {
        false
    };
    if !agrees {
        // Some compatible stores ignore a conditional header or version query. Require the
        // response to confirm the exact verifier before exposing any range bytes.
        return Err(StorageError::Conflict);
    }
    Ok(())
}

fn transport_error(error: reqwest::Error) -> StorageError {
    if error.is_timeout() {
        StorageError::Timeout
    } else {
        StorageError::Unavailable
    }
}

fn body_error(error: reqwest::Error) -> StorageError {
    if error.is_timeout() {
        StorageError::Timeout
    } else if error.is_body() || error.is_decode() {
        StorageError::LengthMismatch
    } else {
        StorageError::Unavailable
    }
}

fn status_error(status: StatusCode, capability: Option<StorageCapability>) -> StorageError {
    match status {
        StatusCode::NOT_FOUND => StorageError::NotFound,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => StorageError::Forbidden,
        StatusCode::CONFLICT | StatusCode::PRECONDITION_FAILED => StorageError::Conflict,
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => StorageError::Timeout,
        StatusCode::NOT_IMPLEMENTED | StatusCode::METHOD_NOT_ALLOWED => match capability {
            Some(capability) => StorageError::UnsupportedCapability(capability),
            None => StorageError::Unavailable,
        },
        _ => StorageError::Unavailable,
    }
}

// Operator-run acceptance against a real provider; ignored and never selected by a gate.
#[cfg(test)]
#[path = "s3_live_acceptance.rs"]
mod live_acceptance;

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io::{Read, Write},
        sync::{Arc, Mutex},
        thread,
    };

    use axum::{
        body::{to_bytes, Body},
        extract::State,
        http::{Request, Response as HttpResponse},
        routing::any,
        Router,
    };
    use futures_util::TryStreamExt;
    use tokio::{net::TcpListener, task::JoinHandle};

    use super::*;
    use crate::contexts::media::domain::object_storage::ByteRange;

    #[derive(Clone)]
    struct Reply {
        status: StatusCode,
        body: Vec<u8>,
        headers: Vec<(&'static str, String)>,
        delay: Duration,
    }

    impl Reply {
        fn object(bytes: &[u8]) -> Self {
            Self {
                status: StatusCode::OK,
                body: bytes.to_vec(),
                headers: vec![
                    ("content-length", bytes.len().to_string()),
                    ("etag", "\"opaque-not-a-content-digest\"".to_owned()),
                ],
                delay: Duration::ZERO,
            }
        }

        fn status(status: StatusCode) -> Self {
            Self {
                status,
                body: Vec::new(),
                headers: Vec::new(),
                delay: Duration::ZERO,
            }
        }
    }

    struct ObservedRequest {
        method: Method,
        uri: String,
        headers: header::HeaderMap,
        body: Vec<u8>,
    }

    #[derive(Clone)]
    struct PeerState {
        replies: Arc<Mutex<VecDeque<Reply>>>,
        requests: Arc<Mutex<Vec<ObservedRequest>>>,
    }

    struct Peer {
        endpoint: String,
        state: PeerState,
        handle: JoinHandle<()>,
    }

    impl Drop for Peer {
        fn drop(&mut self) {
            self.handle.abort();
        }
    }

    impl Peer {
        async fn new(replies: Vec<Reply>) -> Self {
            let state = PeerState {
                replies: Arc::new(Mutex::new(replies.into())),
                requests: Arc::new(Mutex::new(Vec::new())),
            };
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let router = Router::new()
                .route("/{*path}", any(answer))
                .with_state(state.clone());
            let handle = tokio::spawn(async move {
                axum::serve(listener, router).await.unwrap();
            });
            Self {
                endpoint,
                state,
                handle,
            }
        }

        fn storage(&self) -> S3Storage {
            S3Storage::build(
                settings(&self.endpoint),
                "EXAMPLE".to_owned(),
                "secret-sentinel".to_owned(),
                None,
                true,
            )
            .unwrap()
        }
    }

    async fn answer(State(state): State<PeerState>, request: Request<Body>) -> HttpResponse<Body> {
        let (parts, body) = request.into_parts();
        let body = to_bytes(body, MAX_OBJECT_BYTES as usize).await.unwrap();
        state.requests.lock().unwrap().push(ObservedRequest {
            method: parts.method,
            uri: parts.uri.to_string(),
            headers: parts.headers,
            body: body.to_vec(),
        });
        let reply = state
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Reply::status(StatusCode::INTERNAL_SERVER_ERROR));
        if !reply.delay.is_zero() {
            tokio::time::sleep(reply.delay).await;
        }
        let mut response = HttpResponse::builder().status(reply.status);
        for (name, value) in reply.headers {
            response = response.header(name, value);
        }
        response.body(Body::from(reply.body)).unwrap()
    }

    fn settings(endpoint: &str) -> S3Settings {
        S3Settings {
            endpoint: endpoint.to_owned(),
            region: "auto".to_owned(),
            bucket: "private-bucket".to_owned(),
            force_path_style: true,
            connect_timeout_ms: 500,
            request_timeout_ms: 1000,
            conditional_put: true,
            range_reads: true,
            presigned_reads: true,
        }
    }

    fn expected(bytes: &[u8]) -> (ObjectKey, ObjectIntegrity) {
        (
            ObjectKey::for_bytes(bytes),
            ObjectIntegrity::for_bytes(bytes).unwrap(),
        )
    }

    async fn collect(read: StorageRead) -> Result<Vec<u8>, StorageError> {
        let chunks: Vec<_> = read.stream.try_collect().await?;
        Ok(chunks.into_iter().flatten().collect())
    }

    fn receive_request(socket: &mut std::net::TcpStream) {
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut received = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !received.windows(4).any(|value| value == b"\r\n\r\n") {
            let length = socket.read(&mut buffer).unwrap();
            assert!(length > 0);
            received.extend_from_slice(&buffer[..length]);
            assert!(received.len() <= 16 * 1024);
        }
    }

    fn raw_peer(replies: Vec<Vec<u8>>) -> (String, thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            for reply in replies {
                let (mut socket, _) = listener.accept().unwrap();
                receive_request(&mut socket);
                socket.write_all(&reply).unwrap();
                socket.shutdown(std::net::Shutdown::Both).unwrap();
            }
        });
        (endpoint, handle)
    }

    fn loopback_storage(endpoint: &str) -> S3Storage {
        S3Storage::build(
            settings(endpoint),
            "EXAMPLE".into(),
            "secret-sentinel".into(),
            None,
            true,
        )
        .unwrap()
    }

    #[test]
    fn production_configuration_is_https_checked_bounded_and_location_redacted() {
        let valid = settings("https://storage.example");
        assert!(validate_settings(&valid, false).is_ok());
        for endpoint in [
            "http://storage.example",
            "http://127.0.0.1:9000",
            "https://user:secret@storage.example",
            "https://storage.example/private",
            "https://storage.example?secret=value",
            "https://storage.example#fragment",
            "file:///tmp/private",
        ] {
            assert_eq!(
                validate_settings(&settings(endpoint), false),
                Err(StorageError::InvalidTarget)
            );
        }
        for bucket in [
            "",
            "ab",
            "Private",
            "../private",
            "a..b",
            "-private",
            "private-",
            "192.168.1.1",
        ] {
            let mut invalid = valid.clone();
            invalid.bucket = bucket.to_owned();
            assert_eq!(
                validate_settings(&invalid, false),
                Err(StorageError::InvalidTarget)
            );
        }
        for (connect, request) in [(0, 1000), (1001, 1000), (1, 300001), (60001, 60001)] {
            let mut invalid = valid.clone();
            invalid.connect_timeout_ms = connect;
            invalid.request_timeout_ms = request;
            assert_eq!(
                validate_settings(&invalid, false),
                Err(StorageError::InvalidTarget)
            );
        }
        let mut invalid = valid.clone();
        invalid.range_reads = false;
        assert_eq!(
            validate_settings(&invalid, false),
            Err(StorageError::InvalidTarget)
        );
        assert!(!format!("{valid:?}").contains("storage.example"));
        let storage = S3Storage::new(
            valid,
            "EXAMPLE".to_owned(),
            "secret-sentinel".to_owned(),
            None,
        )
        .unwrap();
        assert!(!format!("{storage:?}").contains("sentinel"));
        assert!(!format!("{storage:?}").contains("private-bucket"));
    }

    #[test]
    fn addressing_preserves_the_exact_server_derived_private_key() {
        let (key, _) = expected(b"synthetic bytes");
        let mut options = settings("https://storage.example:9443");
        let path =
            S3Storage::new(options.clone(), "EXAMPLE".into(), "secret".into(), None).unwrap();
        assert_eq!(
            path.object_url(&key).unwrap().as_str(),
            format!(
                "https://storage.example:9443/private-bucket/private/audio/{}",
                key.as_str()
            )
        );
        options.force_path_style = false;
        let virtual_host =
            S3Storage::new(options, "EXAMPLE".into(), "secret".into(), None).unwrap();
        assert_eq!(
            virtual_host.object_url(&key).unwrap().as_str(),
            format!(
                "https://private-bucket.storage.example:9443/private/audio/{}",
                key.as_str()
            )
        );
    }

    #[tokio::test]
    async fn put_and_duplicate_verify_actual_bytes_without_unconditional_retry() {
        let bytes = b"synthetic stored bytes";
        let peer = Peer::new(vec![
            Reply::status(StatusCode::OK),
            Reply::object(bytes),
            Reply::status(StatusCode::PRECONDITION_FAILED),
            Reply::object(bytes),
        ])
        .await;
        let storage = peer.storage();
        let (key, integrity) = expected(bytes);
        assert_eq!(
            storage
                .put_bytes(key.clone(), integrity.clone(), bytes.to_vec(), "audio/ogg")
                .await,
            Ok(())
        );
        assert_eq!(
            storage
                .put_bytes(key.clone(), integrity, bytes.to_vec(), "audio/ogg")
                .await,
            Ok(())
        );
        let requests = peer.state.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        for request in [&requests[0], &requests[2]] {
            assert_eq!(request.method, Method::PUT);
            assert_eq!(request.headers["if-none-match"], "*");
            assert_eq!(request.headers["content-type"], "audio/ogg");
            assert_eq!(request.body, bytes);
            assert!(request.uri.ends_with(key.as_str()));
            assert!(request.headers["authorization"]
                .to_str()
                .unwrap()
                .contains("if-none-match"));
        }
        assert_eq!(requests[1].method, Method::GET);
        assert_eq!(requests[3].method, Method::GET);
    }

    #[tokio::test]
    async fn inspected_png_and_jpeg_use_actual_mime_conditional_create_and_pinned_reads() {
        use crate::contexts::media::{domain::image::ImageMime, image_intake::ImageAdmission};

        for (bytes, mime) in [
            (
                include_bytes!("../image_intake/fixtures/python-rgb-2x3.png").as_slice(),
                ImageMime::Png,
            ),
            (
                include_bytes!("../image_intake/fixtures/pillow-rgb-2x3.jpg").as_slice(),
                ImageMime::Jpeg,
            ),
        ] {
            let peer = Peer::new(vec![
                Reply::status(StatusCode::OK),
                Reply::object(bytes),
                Reply::status(StatusCode::PRECONDITION_FAILED),
                Reply::object(bytes),
                Reply::object(bytes),
                Reply::object(bytes),
            ])
            .await;
            let storage = peer.storage();
            for _ in 0..2 {
                let image = ImageAdmission::from_test_lanes(std::sync::Arc::new(
                    tokio::sync::Semaphore::new(1),
                ))
                .unwrap()
                .inspect(bytes.to_vec(), mime)
                .await
                .unwrap();
                assert_eq!(
                    storage.put_if_absent(VerifiedPayload::Image(image)).await,
                    Ok(())
                );
            }
            let (key, integrity) = expected(bytes);
            let read = storage
                .open_verified(key.clone(), integrity, None)
                .await
                .unwrap();
            assert_eq!(collect(read).await.unwrap(), bytes);
            let requests = peer.state.requests.lock().unwrap();
            assert_eq!(requests.len(), 6);
            for request in [&requests[0], &requests[2]] {
                assert_eq!(request.method, Method::PUT);
                assert_eq!(
                    request.uri,
                    format!("/private-bucket/private/audio/{}", key.as_str())
                );
                assert_eq!(request.headers["content-type"], mime.as_str());
                assert_eq!(request.headers["content-length"], bytes.len().to_string());
                assert_eq!(request.headers["if-none-match"], "*");
                assert_eq!(request.body, bytes);
            }
            for request in [&requests[1], &requests[3], &requests[4], &requests[5]] {
                assert_eq!(request.method, Method::GET);
                assert!(request.body.is_empty());
            }
            assert_eq!(
                requests[5].headers["if-match"],
                "\"opaque-not-a-content-digest\""
            );
        }
    }

    #[tokio::test]
    async fn inspected_image_put_rejects_wrong_readback_without_retry_or_success() {
        use crate::contexts::media::{domain::image::ImageMime, image_intake::ImageAdmission};

        let bytes = include_bytes!("../image_intake/fixtures/python-rgb-2x3.png").as_slice();
        let mut corrupt = bytes.to_vec();
        corrupt[bytes.len() - 1] ^= 1;
        let peer = Peer::new(vec![Reply::status(StatusCode::OK), Reply::object(&corrupt)]).await;
        let image =
            ImageAdmission::from_test_lanes(std::sync::Arc::new(tokio::sync::Semaphore::new(1)))
                .unwrap()
                .inspect(bytes.to_vec(), ImageMime::Png)
                .await
                .unwrap();
        assert_eq!(
            peer.storage()
                .put_if_absent(VerifiedPayload::Image(image))
                .await,
            Err(StorageError::DigestMismatch)
        );
        let requests = peer.state.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].method, Method::PUT);
        assert_eq!(requests[0].headers["content-type"], "image/png");
        assert_eq!(requests[1].method, Method::GET);
    }

    #[tokio::test]
    async fn inspected_image_keeps_its_admission_lane_until_s3_readback_finishes() {
        use crate::contexts::media::{
            domain::image::ImageMime,
            image_intake::{ImageAdmission, ImageIntakeError},
        };
        use tokio::sync::{Notify, Semaphore};

        let bytes = include_bytes!("../image_intake/fixtures/python-rgb-2x3.png").as_slice();
        let lanes = Arc::new(Semaphore::new(1));
        let image = ImageAdmission::from_test_lanes(lanes.clone())
            .unwrap()
            .inspect(bytes.to_vec(), ImageMime::Png)
            .await
            .unwrap();
        let readback_started = Arc::new(Notify::new());
        let readback_release = Arc::new(Semaphore::new(0));
        let router = Router::new().route(
            "/{*path}",
            any({
                let started = readback_started.clone();
                let release = readback_release.clone();
                move |request: Request<Body>| {
                    let started = started.clone();
                    let release = release.clone();
                    async move {
                        if request.method() == Method::PUT {
                            let (parts, body) = request.into_parts();
                            assert_eq!(parts.headers["content-type"], "image/png");
                            assert_eq!(
                                to_bytes(body, MAX_OBJECT_BYTES as usize).await.unwrap(),
                                bytes
                            );
                            HttpResponse::builder()
                                .status(StatusCode::OK)
                                .body(Body::empty())
                                .unwrap()
                        } else {
                            assert_eq!(request.method(), Method::GET);
                            started.notify_one();
                            let _permit = release.acquire().await.unwrap();
                            HttpResponse::builder()
                                .status(StatusCode::OK)
                                .header("content-length", bytes.len().to_string())
                                .header("etag", "\"synthetic-verified-image\"")
                                .body(Body::from(bytes))
                                .unwrap()
                        }
                    }
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let storage = loopback_storage(&endpoint);
        let put = tokio::spawn(storage.put_if_absent(VerifiedPayload::Image(image)));
        tokio::time::timeout(Duration::from_secs(2), readback_started.notified())
            .await
            .unwrap();
        assert_eq!(lanes.available_permits(), 0);
        assert!(matches!(
            ImageAdmission::from_test_lanes(lanes.clone()),
            Err(ImageIntakeError::AdmissionBusy)
        ));
        readback_release.add_permits(1);
        assert_eq!(put.await.unwrap(), Ok(()));
        assert_eq!(lanes.available_permits(), 1);
        server.abort();
    }

    #[tokio::test]
    async fn last_transport_body_clone_retains_image_admission_after_outer_owner_drops() {
        use crate::contexts::media::{
            domain::image::ImageMime,
            image_intake::{ImageAdmission, ImageIntakeError},
        };
        use tokio::sync::Semaphore;

        let bytes = include_bytes!("../image_intake/fixtures/python-rgb-2x3.png").as_slice();
        let lanes = Arc::new(Semaphore::new(1));
        let image = Arc::new(
            ImageAdmission::from_test_lanes(lanes.clone())
                .unwrap()
                .inspect(bytes.to_vec(), ImageMime::Png)
                .await
                .unwrap(),
        );
        let body = Bytes::from_owner(ImageBody(image.clone()));
        assert_eq!(body.as_ptr(), image.bytes().as_ptr());
        assert_eq!(body.as_ref(), bytes);
        let cloned_body = body.clone();
        let final_transport_slice = body.slice(3..11);

        drop(image);
        assert_eq!(lanes.available_permits(), 0);
        drop(body);
        drop(cloned_body);
        assert_eq!(lanes.available_permits(), 0);
        assert!(matches!(
            ImageAdmission::from_test_lanes(lanes.clone()),
            Err(ImageIntakeError::AdmissionBusy)
        ));
        assert_eq!(final_transport_slice.as_ref(), &bytes[3..11]);
        drop(final_transport_slice);
        assert_eq!(lanes.available_permits(), 1);
    }

    #[tokio::test]
    async fn conflicting_or_corrupt_prior_objects_never_become_verified() {
        let bytes = b"synthetic stored bytes";
        let (key, integrity) = expected(bytes);
        let peer = Peer::new(vec![
            Reply::status(StatusCode::PRECONDITION_FAILED),
            Reply::object(&vec![b'X'; bytes.len()]),
        ])
        .await;
        assert_eq!(
            peer.storage()
                .put_bytes(key, integrity, bytes.to_vec(), "audio/ogg")
                .await,
            Err(StorageError::DigestMismatch)
        );
        assert_eq!(peer.state.requests.lock().unwrap().len(), 2);
        let peer = Peer::new(vec![
            Reply::status(StatusCode::OK),
            Reply::object(&bytes[..3]),
        ])
        .await;
        let (key, integrity) = expected(bytes);
        assert_eq!(
            peer.storage()
                .put_bytes(key, integrity, bytes.to_vec(), "audio/ogg")
                .await,
            Err(StorageError::LengthMismatch)
        );
    }

    #[tokio::test]
    async fn unsupported_conditional_write_and_conflict_do_not_emulate_or_retry() {
        let bytes = b"synthetic stored bytes";
        for (status, error) in [
            (StatusCode::CONFLICT, StorageError::Conflict),
            (
                StatusCode::NOT_IMPLEMENTED,
                StorageError::UnsupportedCapability(StorageCapability::ConditionalCreate),
            ),
            (StatusCode::FORBIDDEN, StorageError::Forbidden),
            (StatusCode::SERVICE_UNAVAILABLE, StorageError::Unavailable),
        ] {
            let peer = Peer::new(vec![Reply::status(status)]).await;
            let (key, integrity) = expected(bytes);
            assert_eq!(
                peer.storage()
                    .put_bytes(key, integrity, bytes.to_vec(), "audio/ogg")
                    .await,
                Err(error)
            );
            assert_eq!(peer.state.requests.lock().unwrap().len(), 1);
        }
        let peer = Peer::new(Vec::new()).await;
        let mut storage = peer.storage();
        storage.settings.conditional_put = false;
        let (key, integrity) = expected(bytes);
        assert_eq!(
            storage
                .put_bytes(key, integrity, bytes.to_vec(), "audio/ogg")
                .await,
            Err(StorageError::UnsupportedCapability(
                StorageCapability::ConditionalCreate
            ))
        );
        assert!(peer.state.requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn full_and_first_middle_final_ranges_pin_the_verified_etag() {
        let bytes = b"0123456789";
        for (raw, first, last) in [("bytes=0-2", 0, 2), ("bytes=3-5", 3, 5), ("bytes=-3", 7, 9)] {
            let range = ByteRange::parse(raw)
                .unwrap()
                .normalize(bytes.len() as u64)
                .unwrap();
            let mut selected = Reply::object(&bytes[first..=last]);
            selected.status = StatusCode::PARTIAL_CONTENT;
            selected
                .headers
                .push(("content-range", format!("bytes {first}-{last}/10")));
            let peer = Peer::new(vec![Reply::object(bytes), selected]).await;
            let (key, integrity) = expected(bytes);
            let read = peer
                .storage()
                .open_verified(key, integrity, Some(range))
                .await
                .unwrap();
            assert_eq!(read.byte_length, 3);
            assert_eq!(collect(read).await.unwrap(), &bytes[first..=last]);
            let requests = peer.state.requests.lock().unwrap();
            assert_eq!(
                requests[1].headers["if-match"],
                "\"opaque-not-a-content-digest\""
            );
            assert_eq!(
                requests[1].headers["range"],
                format!("bytes={first}-{last}")
            );
        }
        let peer = Peer::new(vec![Reply::object(bytes), Reply::object(bytes)]).await;
        let (key, integrity) = expected(bytes);
        assert_eq!(
            collect(
                peer.storage()
                    .open_verified(key, integrity, None)
                    .await
                    .unwrap()
            )
            .await
            .unwrap(),
            bytes
        );
    }

    #[tokio::test]
    async fn versioned_reads_prefer_exact_encoded_version_to_mutable_current() {
        let bytes = b"0123456789";
        let mut verified = Reply::object(bytes);
        verified
            .headers
            .push(("x-amz-version-id", "version/with+reserved=bytes".to_owned()));
        let mut selected = Reply::object(bytes);
        selected
            .headers
            .push(("x-amz-version-id", "version/with+reserved=bytes".to_owned()));
        let peer = Peer::new(vec![verified, selected]).await;
        let (key, integrity) = expected(bytes);
        assert_eq!(
            collect(
                peer.storage()
                    .open_verified(key, integrity, None)
                    .await
                    .unwrap()
            )
            .await
            .unwrap(),
            bytes
        );
        let requests = peer.state.requests.lock().unwrap();
        assert!(requests[1]
            .uri
            .ends_with("?versionId=version%2Fwith%2Breserved%3Dbytes"));
        assert!(!requests[1].headers.contains_key("if-match"));
    }

    #[tokio::test]
    async fn incorrect_range_status_headers_compression_and_changed_pin_fail_closed() {
        let bytes = b"0123456789";
        let range = ByteRange::parse("bytes=3-5")
            .unwrap()
            .normalize(10)
            .unwrap();
        let mut wrong_range = Reply::object(b"345");
        wrong_range.status = StatusCode::PARTIAL_CONTENT;
        wrong_range
            .headers
            .push(("content-range", "bytes 0-2/10".to_owned()));
        let mut compressed = Reply::object(bytes);
        compressed
            .headers
            .push(("content-encoding", "gzip".to_owned()));
        for (reply, error) in [
            (
                Reply::object(bytes),
                StorageError::UnsupportedCapability(StorageCapability::RangeRead),
            ),
            (wrong_range, StorageError::LengthMismatch),
            (
                Reply::status(StatusCode::PRECONDITION_FAILED),
                StorageError::Conflict,
            ),
        ] {
            let peer = Peer::new(vec![Reply::object(bytes), reply]).await;
            let (key, integrity) = expected(bytes);
            assert!(
                matches!(peer.storage().open_verified(key, integrity, Some(range)).await, Err(observed) if observed == error)
            );
        }
        let peer = Peer::new(vec![compressed]).await;
        let (key, integrity) = expected(bytes);
        assert_eq!(
            peer.storage().verify(key, integrity).await,
            Err(StorageError::Unavailable)
        );
    }

    #[tokio::test]
    async fn providers_ignoring_the_read_pin_cannot_expose_a_replacement_range() {
        let bytes = b"0123456789";
        let range = ByteRange::parse("bytes=3-5")
            .unwrap()
            .normalize(10)
            .unwrap();
        for versioned in [false, true] {
            let mut verified = Reply::object(bytes);
            if versioned {
                verified
                    .headers
                    .push(("x-amz-version-id", "verified-version".to_owned()));
            }
            let mut replacement = Reply::object(b"BAD");
            replacement.status = StatusCode::PARTIAL_CONTENT;
            replacement
                .headers
                .push(("content-range", "bytes 3-5/10".to_owned()));
            replacement.headers.retain(|(name, _)| *name != "etag");
            replacement
                .headers
                .push(("etag", "\"replacement\"".to_owned()));
            if versioned {
                replacement
                    .headers
                    .push(("x-amz-version-id", "replacement-version".to_owned()));
            }
            let peer = Peer::new(vec![verified, replacement]).await;
            let (key, integrity) = expected(bytes);
            assert!(matches!(
                peer.storage()
                    .open_verified(key, integrity, Some(range))
                    .await,
                Err(StorageError::Conflict)
            ));
            assert_eq!(peer.state.requests.lock().unwrap().len(), 2);
        }
    }

    #[tokio::test]
    async fn missing_forbidden_outage_timeout_and_redirect_remain_typed_and_sanitized() {
        let bytes = b"synthetic stored bytes";
        for (status, error) in [
            (StatusCode::NOT_FOUND, StorageError::NotFound),
            (StatusCode::FORBIDDEN, StorageError::Forbidden),
            (StatusCode::SERVICE_UNAVAILABLE, StorageError::Unavailable),
            (StatusCode::TEMPORARY_REDIRECT, StorageError::Unavailable),
        ] {
            let mut reply = Reply::status(status);
            reply.headers.push((
                "location",
                "https://credential-leak.invalid/secret-sentinel".to_owned(),
            ));
            let peer = Peer::new(vec![reply]).await;
            let (key, integrity) = expected(bytes);
            assert_eq!(peer.storage().verify(key, integrity).await, Err(error));
            assert_eq!(peer.state.requests.lock().unwrap().len(), 1);
            assert!(!format!("{error:?}").contains("sentinel"));
        }
        let mut delayed = Reply::object(bytes);
        delayed.delay = Duration::from_millis(200);
        let peer = Peer::new(vec![delayed]).await;
        let mut options = settings(&peer.endpoint);
        options.connect_timeout_ms = 10;
        options.request_timeout_ms = 50;
        let storage = S3Storage::build(
            options,
            "EXAMPLE".into(),
            "secret-sentinel".into(),
            None,
            true,
        )
        .unwrap();
        let (key, integrity) = expected(bytes);
        assert_eq!(
            storage.verify(key, integrity).await,
            Err(StorageError::Timeout)
        );
    }

    #[tokio::test]
    async fn unversioned_grants_fall_back_and_reads_without_any_strong_pin_are_unavailable() {
        let bytes = b"synthetic stored bytes";
        for version in [None, Some("null")] {
            let mut reply = Reply::object(bytes);
            if let Some(version) = version {
                reply.headers.push(("x-amz-version-id", version.to_owned()));
            }
            let peer = Peer::new(vec![reply]).await;
            let (key, integrity) = expected(bytes);
            assert!(matches!(
                peer.storage().temporary_read(key, integrity, 300).await,
                Ok(None)
            ));
        }
        let mut reply = Reply::object(bytes);
        reply.headers.retain(|(name, _)| *name != "etag");
        reply.headers.push(("etag", "W/\"weak\"".to_owned()));
        let peer = Peer::new(vec![reply]).await;
        let (key, integrity) = expected(bytes);
        assert!(matches!(
            peer.storage().open_verified(key, integrity, None).await,
            Err(StorageError::Unavailable)
        ));
        assert_eq!(peer.state.requests.lock().unwrap().len(), 1);
        let peer = Peer::new(Vec::new()).await;
        let (key, integrity) = expected(bytes);
        assert!(matches!(
            peer.storage().temporary_read(key, integrity, 301).await,
            Err(StorageError::InvalidTarget)
        ));
        assert!(peer.state.requests.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn truncated_actual_verification_and_range_bodies_are_never_successful() {
        let bytes = b"0123456789";
        let (endpoint, handle) = raw_peer(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nETag: \"opaque\"\r\nConnection: close\r\n\r\n012".to_vec(),
        ]);
        let (key, integrity) = expected(bytes);
        assert_eq!(
            loopback_storage(&endpoint).verify(key, integrity).await,
            Err(StorageError::LengthMismatch)
        );
        handle.join().unwrap();

        let (endpoint, handle) = raw_peer(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nETag: \"opaque\"\r\nConnection: close\r\n\r\n0123456789".to_vec(),
            b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/10\r\nETag: \"opaque\"\r\nConnection: close\r\n\r\n3".to_vec(),
        ]);
        let range = ByteRange::parse("bytes=3-5")
            .unwrap()
            .normalize(10)
            .unwrap();
        let (key, integrity) = expected(bytes);
        let read = loopback_storage(&endpoint)
            .open_verified(key, integrity, Some(range))
            .await
            .unwrap();
        assert_eq!(collect(read).await, Err(StorageError::LengthMismatch));
        handle.join().unwrap();
    }

    #[tokio::test]
    async fn cancelling_verification_drops_the_in_progress_transport() {
        let bytes = b"0123456789";
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (started_send, started_receive) = tokio::sync::oneshot::channel();
        let (closed_send, closed_receive) = tokio::sync::oneshot::channel();
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            receive_request(&mut socket);
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nETag: \"opaque\"\r\nConnection: close\r\n\r\n0").unwrap();
            socket.flush().unwrap();
            started_send.send(()).unwrap();
            let mut buffer = [0_u8; 1];
            let disconnected = match socket.read(&mut buffer) {
                Ok(0) => true,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => true,
                _ => false,
            };
            let _ = closed_send.send(disconnected);
        });
        let mut options = settings(&endpoint);
        options.request_timeout_ms = 5000;
        let storage = S3Storage::build(
            options,
            "EXAMPLE".into(),
            "secret-sentinel".into(),
            None,
            true,
        )
        .unwrap();
        let (key, integrity) = expected(bytes);
        let transfer = tokio::spawn(storage.verify(key, integrity));
        started_receive.await.unwrap();
        transfer.abort();
        assert!(transfer.await.unwrap_err().is_cancelled());
        assert!(tokio::time::timeout(Duration::from_secs(1), closed_receive)
            .await
            .unwrap()
            .unwrap());
        handle.join().unwrap();
    }

    #[tokio::test]
    async fn dropping_a_verified_read_stream_cancels_further_object_io() {
        let bytes = b"0123456789";
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (started_send, started_receive) = tokio::sync::oneshot::channel();
        let (closed_send, closed_receive) = tokio::sync::oneshot::channel();
        let handle = thread::spawn(move || {
            let (mut verify, _) = listener.accept().unwrap();
            receive_request(&mut verify);
            verify.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nETag: \"opaque\"\r\nConnection: close\r\n\r\n0123456789").unwrap();
            verify.shutdown(std::net::Shutdown::Both).unwrap();
            let (mut read, _) = listener.accept().unwrap();
            receive_request(&mut read);
            read.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nETag: \"opaque\"\r\nConnection: close\r\n\r\n0").unwrap();
            read.flush().unwrap();
            started_send.send(()).unwrap();
            let mut buffer = [0_u8; 1];
            let disconnected = match read.read(&mut buffer) {
                Ok(0) => true,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => true,
                _ => false,
            };
            let _ = closed_send.send(disconnected);
        });
        let mut options = settings(&endpoint);
        options.request_timeout_ms = 5000;
        let storage = S3Storage::build(
            options,
            "EXAMPLE".into(),
            "secret-sentinel".into(),
            None,
            true,
        )
        .unwrap();
        let (key, integrity) = expected(bytes);
        let read = storage.open_verified(key, integrity, None).await.unwrap();
        started_receive.await.unwrap();
        drop(read.stream);
        assert!(tokio::time::timeout(Duration::from_secs(1), closed_receive)
            .await
            .unwrap()
            .unwrap());
        handle.join().unwrap();
    }
}
