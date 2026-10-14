//! Shared binary Ogg HTTP mechanics, not a read capability or an authorization policy.
//!
//! Callers resolve the exact immutable object and finish their owning authorization checks
//! before returning any response. A plan, digest or `StorageRead` grants no Studio, Theatre
//! or publication access. Studio is the only production consumer of this module today.

use crate::contexts::media::read::{
    ByteRange, NormalizedByteRange, RangeError, StorageError, StorageRead, StorageStream,
};
use axum::{
    body::Body,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::Response,
};
use chrono::NaiveDateTime;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::{sync::OwnedSemaphorePermit, time::Instant};

const STREAM_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_LIST_ITEMS: usize = 128;
// Local parser/formatting budget, not an RFC field-size limit or a whole-request memory claim.
// Each evaluated field-set permits 8 KiB total, including two bytes per line for bookkeeping.
const MAX_HEADER_BYTES: usize = 8 * 1024;
const FIELD_LINE_COST: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AudioTransportError {
    MalformedRange,
    MalformedIfRange,
    MalformedIfMatch,
    MalformedIfNoneMatch,
    UnsupportedMethod,
    InvalidRepresentation,
    InconsistentPlan,
    DeclaredLengthMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Representation(Option<NormalizedByteRange>),
    NotModified,
    PreconditionFailed,
    Unsatisfiable,
}

/// Private fields bind the transport outcome to its method, accepted digest and whole length.
/// This is an HTTP plan only; it deliberately contains no authority or storage location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AudioResponsePlan {
    method: Method,
    etag: HeaderValue,
    whole_length: u64,
    outcome: Outcome,
}

impl AudioResponsePlan {
    pub(crate) fn requires_storage(&self) -> bool {
        matches!(self.outcome, Outcome::Representation(_))
    }

    pub(crate) fn range(&self) -> Option<NormalizedByteRange> {
        match self.outcome {
            Outcome::Representation(range) => range,
            Outcome::NotModified | Outcome::PreconditionFailed | Outcome::Unsatisfiable => None,
        }
    }

    pub(crate) fn status(&self) -> StatusCode {
        match self.outcome {
            Outcome::Representation(None) => StatusCode::OK,
            Outcome::Representation(Some(_)) => StatusCode::PARTIAL_CONTENT,
            Outcome::NotModified => StatusCode::NOT_MODIFIED,
            Outcome::PreconditionFailed => StatusCode::PRECONDITION_FAILED,
            Outcome::Unsatisfiable => StatusCode::RANGE_NOT_SATISFIABLE,
        }
    }
}

/// RFC 9110 entity-tag preconditions precede Range. No modification timestamp is invented:
/// date preconditions are ignored, and a valid date If-Range cannot match without that fact.
pub(crate) fn plan(
    headers: &HeaderMap,
    method: &Method,
    sha256: &str,
    whole_length: u64,
) -> Result<AudioResponsePlan, AudioTransportError> {
    if *method != Method::GET && *method != Method::HEAD {
        return Err(AudioTransportError::UnsupportedMethod);
    }
    if whole_length == 0
        || sha256.len() != 64
        || !sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(AudioTransportError::InvalidRepresentation);
    }
    let etag = HeaderValue::from_str(&format!("\"{sha256}\""))
        .map_err(|_| AudioTransportError::InvalidRepresentation)?;
    let outcome = if entity_tag_condition(
        headers,
        header::IF_MATCH,
        sha256.as_bytes(),
        true,
        AudioTransportError::MalformedIfMatch,
    )? == Some(false)
    {
        Outcome::PreconditionFailed
    } else if entity_tag_condition(
        headers,
        header::IF_NONE_MATCH,
        sha256.as_bytes(),
        false,
        AudioTransportError::MalformedIfNoneMatch,
    )? == Some(true)
    {
        Outcome::NotModified
    } else {
        selected_range(headers, method, sha256.as_bytes(), whole_length)?
    };
    Ok(AudioResponsePlan {
        method: method.clone(),
        etag,
        whole_length,
        outcome,
    })
}

/// Accept only a declared verified read obtained by the owning application. This verifies
/// length, not digest, MIME, object scope, publication readiness or authorization.
pub(crate) fn response(
    stored: StorageRead,
    plan: AudioResponsePlan,
    method: &Method,
    permit: OwnedSemaphorePermit,
) -> Result<Response, AudioTransportError> {
    if plan.method != *method || !plan.requires_storage() {
        return Err(AudioTransportError::InconsistentPlan);
    }
    let expected = plan
        .range()
        .map_or(plan.whole_length, NormalizedByteRange::byte_length);
    if stored.byte_length != expected {
        return Err(AudioTransportError::DeclaredLengthMismatch);
    }
    let body = if *method == Method::HEAD {
        // HEAD still came through the owning verified-open/recheck flow, but consumes no bytes.
        drop(stored);
        drop(permit);
        Body::empty()
    } else {
        Body::from_stream(bounded_stream(stored.stream, expected, permit))
    };
    let mut response = Response::new(body);
    *response.status_mut() = plan.status();
    let headers = response.headers_mut();
    representation_headers(headers, &plan);
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("audio/ogg"));
    insert_header(headers, header::CONTENT_LENGTH, &expected.to_string())?;
    if let Some(range) = plan.range() {
        insert_header(
            headers,
            header::CONTENT_RANGE,
            &format!(
                "bytes {}-{}/{}",
                range.first(),
                range.last(),
                plan.whole_length
            ),
        )?;
    }
    Ok(response)
}

/// Conditional/unsatisfiable responses need no source. The caller must already have resolved
/// and authorized this exact representation; this helper performs no admission or recheck.
pub(crate) fn metadata_response(
    plan: AudioResponsePlan,
    method: &Method,
    permit: OwnedSemaphorePermit,
    rejection: Response,
) -> Result<Response, AudioTransportError> {
    if plan.method != *method || plan.requires_storage() {
        return Err(AudioTransportError::InconsistentPlan);
    }
    let mut response = match plan.outcome {
        Outcome::NotModified => {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::NOT_MODIFIED;
            response
        }
        Outcome::PreconditionFailed | Outcome::Unsatisfiable => rejection,
        Outcome::Representation(_) => return Err(AudioTransportError::InconsistentPlan),
    };
    // The route group owns its error vocabulary/body, while the plan owns HTTP status.
    *response.status_mut() = plan.status();
    if *method == Method::HEAD {
        *response.body_mut() = Body::empty();
    }
    representation_headers(response.headers_mut(), &plan);
    if plan.outcome == Outcome::Unsatisfiable {
        insert_header(
            response.headers_mut(),
            header::CONTENT_RANGE,
            &format!("bytes */{}", plan.whole_length),
        )?;
    }
    drop(permit);
    Ok(response)
}

/// Private cache/encoding mechanics also apply to caller-owned rejection responses.
pub(crate) fn private_response(mut response: Response) -> Response {
    private_headers(response.headers_mut());
    response
}

fn representation_headers(headers: &mut HeaderMap, plan: &AudioResponsePlan) {
    private_headers(headers);
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(header::ETAG, plan.etag.clone());
}

fn private_headers(headers: &mut HeaderMap) {
    // No cache-policy selection is delegated to a listener. Future publication delivery must
    // supply a separately reviewed policy, not repurpose these private preview mechanics.
    // TODO(issue120): publication/media owner must integrate a publication-class read only
    // after verified promotion and current-public admission; never widen private storage.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store, no-transform"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.remove(header::CONTENT_ENCODING);
    // TODO(issue120): deployment owner must probe the actual CDN/proxy path for no encoding,
    // cache isolation and Theatre CORS/exposed range headers before public delivery acceptance.
}

fn insert_header(
    headers: &mut HeaderMap,
    name: header::HeaderName,
    value: &str,
) -> Result<(), AudioTransportError> {
    let value =
        HeaderValue::from_str(value).map_err(|_| AudioTransportError::InvalidRepresentation)?;
    headers.insert(name, value);
    Ok(())
}

fn entity_tag_condition(
    headers: &HeaderMap,
    name: header::HeaderName,
    sha256: &[u8],
    strong: bool,
    error: AudioTransportError,
) -> Result<Option<bool>, AudioTransportError> {
    check_header_budget(headers, name.clone(), error)?;
    let values = headers.get_all(name);
    let field_count = values.iter().count();
    if field_count == 0 {
        return Ok(None);
    }
    let mut matched = false;
    let mut item_count = 0;
    for value in values.iter() {
        let mut remaining = trim_ows(value.as_bytes());
        if remaining == b"*" {
            return if field_count == 1 {
                Ok(Some(true))
            } else {
                Err(error)
            };
        }
        // These are list-valued fields. Repeated lines combine; commas inside an opaque tag
        // do not split it, and a bounded number of empty list elements is tolerated (5.6.1).
        while !remaining.is_empty() {
            item_count += 1;
            if item_count > MAX_LIST_ITEMS {
                return Err(error);
            }
            if remaining[0] == b',' {
                remaining = trim_ows(&remaining[1..]);
                continue;
            }
            let (opaque, weak, rest) = entity_tag(remaining).ok_or(error)?;
            matched |= opaque == sha256 && (!strong || !weak);
            remaining = trim_ows(rest);
            if !remaining.is_empty() {
                if remaining[0] != b',' {
                    return Err(error);
                }
                remaining = trim_ows(&remaining[1..]);
            }
        }
    }
    Ok(Some(matched))
}

fn check_header_budget(
    headers: &HeaderMap,
    name: header::HeaderName,
    error: AudioTransportError,
) -> Result<(), AudioTransportError> {
    let mut total = 0_usize;
    for value in headers.get_all(name).iter() {
        total = total
            .checked_add(value.as_bytes().len())
            .and_then(|total| total.checked_add(FIELD_LINE_COST))
            .ok_or(error)?;
        if total > MAX_HEADER_BYTES {
            return Err(error);
        }
    }
    Ok(())
}

fn entity_tag(value: &[u8]) -> Option<(&[u8], bool, &[u8])> {
    let (value, weak) = match value.strip_prefix(b"W/") {
        Some(value) => (value, true),
        None => (value, false),
    };
    let value = value.strip_prefix(b"\"")?;
    let end = value.iter().position(|byte| *byte == b'"')?;
    let opaque = &value[..end];
    if !opaque
        .iter()
        .all(|byte| *byte == 0x21 || (0x23..=0x7e).contains(byte) || *byte >= 0x80)
    {
        return None;
    }
    Some((opaque, weak, &value[end + 1..]))
}

fn trim_ows(value: &[u8]) -> &[u8] {
    let first = value
        .iter()
        .position(|byte| !matches!(byte, b' ' | b'\t'))
        .unwrap_or(value.len());
    let last = value
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t'))
        .map_or(first, |index| index + 1);
    &value[first..last]
}

fn single_field(
    headers: &HeaderMap,
    name: header::HeaderName,
    error: AudioTransportError,
) -> Result<Option<&HeaderValue>, AudioTransportError> {
    let all = headers.get_all(name);
    let mut values = all.iter();
    let first = values.next();
    if values.next().is_some() {
        return Err(error);
    }
    Ok(first)
}

fn selected_range(
    headers: &HeaderMap,
    method: &Method,
    sha256: &[u8],
    length: u64,
) -> Result<Outcome, AudioTransportError> {
    // RFC 9110 14.2 defines Range only for GET. HEAD ignores even malformed Range fields.
    if *method == Method::HEAD {
        return Ok(Outcome::Representation(None));
    }
    let Some(value) = single_field(headers, header::RANGE, AudioTransportError::MalformedRange)?
    else {
        return Ok(Outcome::Representation(None));
    };
    if !if_range_matches(headers, sha256)? {
        return Ok(Outcome::Representation(None));
    }
    // Inapplicable Range is ignored before scanning or copying a large member.
    check_header_budget(headers, header::RANGE, AudioTransportError::MalformedRange)?;
    let value = value
        .to_str()
        .map_err(|_| AudioTransportError::MalformedRange)?;
    let (unit, specification) = value
        .trim_matches([' ', '\t'])
        .split_once('=')
        .ok_or(AudioTransportError::MalformedRange)?;
    if unit.is_empty()
        || !unit
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
    {
        return Err(AudioTransportError::MalformedRange);
    }
    // Unknown range units are ignored, never interpreted as bytes.
    if !unit.eq_ignore_ascii_case("bytes") {
        return Ok(Outcome::Representation(None));
    }
    let mut selected = None;
    let mut count = 0;
    for (index, specification) in specification.split(',').enumerate() {
        if index >= MAX_LIST_ITEMS {
            return Err(AudioTransportError::MalformedRange);
        }
        let specification = specification.trim_matches([' ', '\t']);
        if specification.is_empty() {
            continue;
        }
        let range = ByteRange::parse(&format!("bytes={specification}"))
            .map_err(|_| AudioTransportError::MalformedRange)?;
        count += 1;
        selected = Some(range);
    }
    let range = selected.ok_or(AudioTransportError::MalformedRange)?;
    if count > 1 {
        // RFC 9110 14.2 permits ignoring Range. Validate every member, then ignore the whole
        // unsupported multi-range request: full 200, never a fake multipart/single 206.
        return Ok(Outcome::Representation(None));
    }
    match range.normalize(length) {
        Ok(range) => Ok(Outcome::Representation(Some(range))),
        Err(RangeError::Unsatisfiable) => Ok(Outcome::Unsatisfiable),
        Err(RangeError::Invalid | RangeError::MultipleRanges | RangeError::Overflow) => {
            Err(AudioTransportError::MalformedRange)
        }
    }
}

fn if_range_matches(headers: &HeaderMap, sha256: &[u8]) -> Result<bool, AudioTransportError> {
    let Some(value) = single_field(
        headers,
        header::IF_RANGE,
        AudioTransportError::MalformedIfRange,
    )?
    else {
        return Ok(true);
    };
    check_header_budget(
        headers,
        header::IF_RANGE,
        AudioTransportError::MalformedIfRange,
    )?;
    let value = trim_ows(value.as_bytes());
    if let Some((opaque, weak, rest)) = entity_tag(value) {
        if trim_ows(rest).is_empty() {
            return Ok(!weak && opaque == sha256);
        }
    }
    let date = std::str::from_utf8(value).map_err(|_| AudioTransportError::MalformedIfRange)?;
    if [
        "%a, %d %b %Y %H:%M:%S GMT",
        "%A, %d-%b-%y %H:%M:%S GMT",
        "%a %b %e %H:%M:%S %Y",
    ]
    .into_iter()
    .any(|format| NaiveDateTime::parse_from_str(date, format).is_ok())
    {
        return Ok(false);
    }
    Err(AudioTransportError::MalformedIfRange)
}

fn bounded_stream(
    source: StorageStream,
    length: u64,
    permit: OwnedSemaphorePermit,
) -> StorageStream {
    bounded_stream_until(source, length, permit, Instant::now() + STREAM_TIMEOUT)
}

fn bounded_stream_until(
    source: StorageStream,
    length: u64,
    permit: OwnedSemaphorePermit,
    deadline: Instant,
) -> StorageStream {
    // Demand-driven pass-through: at most one upstream chunk is handled per downstream poll,
    // each accepted chunk fits the remaining exact length. No aggregation, prefetch or spawned
    // transfer exists. The source controls chunk allocation; this is not an RSS or zero-copy
    // guarantee, and slice backing allocations/HTTP/socket buffers remain adapter concerns.
    // TODO(issue120): media/runtime owner must measure peak memory with the actual source,
    // host/proxy and concurrent read lanes before accepting a production memory budget.
    // The deadline is demand-polled, not an independent idle-client lane-eviction watchdog;
    // deployment must also accept its transport idle/slow-client policy before activation.
    let state = Some((source, length, permit));
    Box::pin(
        futures_util::stream::unfold(state, move |state| async move {
            let (mut source, remaining, permit) = state?;
            if Instant::now() >= deadline {
                return Some((Err(StorageError::Timeout), None));
            }
            let next = tokio::time::timeout_at(deadline, source.next()).await;
            match next {
                Ok(Some(Ok(bytes))) if bytes.len() as u64 <= remaining => {
                    let remaining = remaining - bytes.len() as u64;
                    Some((Ok(bytes), Some((source, remaining, permit))))
                }
                Ok(Some(Ok(_))) => Some((Err(StorageError::LengthMismatch), None)),
                Ok(Some(Err(error))) => Some((Err(error), None)),
                Ok(None) if remaining == 0 => None,
                Ok(None) => Some((Err(StorageError::LengthMismatch), None)),
                Err(_) => Some((Err(StorageError::Timeout), None)),
            }
            // Terminal outcomes drop both the source and permit before exposing the error/EOF.
        })
        .fuse(),
    )
}

#[cfg(test)]
mod tests;
