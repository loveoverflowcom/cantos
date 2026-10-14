use super::*;
use axum::body::to_bytes;
use bytes::Bytes;
use futures_util::Stream;
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
use tokio::sync::Semaphore;

const SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn headers(name: header::HeaderName, value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(name, HeaderValue::from_str(value).unwrap());
    headers
}

fn get_plan(headers: &HeaderMap, length: u64) -> Result<AudioResponsePlan, AudioTransportError> {
    plan(headers, &Method::GET, SHA256, length)
}

fn lane() -> (Arc<Semaphore>, OwnedSemaphorePermit) {
    let lanes = Arc::new(Semaphore::new(1));
    let permit = lanes.clone().try_acquire_owned().unwrap();
    (lanes, permit)
}

fn rejection() -> Response {
    let mut response = Response::new(Body::from("caller-owned rejection"));
    *response.status_mut() = StatusCode::IM_A_TEAPOT;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    response
}

fn assert_private(headers: &HeaderMap) {
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "private, no-store, no-transform"
    );
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(!headers.contains_key(header::CONTENT_ENCODING));
    assert!(!headers.contains_key(header::LAST_MODIFIED));
}

#[test]
fn single_ranges_cover_closed_open_suffix_case_and_unsatisfiable_requests() {
    for (value, first, last) in [
        ("bytes=0-2", 0, 2),
        ("Bytes=3-5", 3, 5),
        ("BYTES=7-9", 7, 9),
        ("bytes=7-", 7, 9),
        ("bytes=-3", 7, 9),
        ("bytes=7-999", 7, 9),
        ("bytes=-999", 0, 9),
        ("bytes=0-0", 0, 0),
        ("bytes= 2-4 ", 2, 4),
    ] {
        let plan = get_plan(&headers(header::RANGE, value), 10).unwrap();
        assert_eq!(plan.status(), StatusCode::PARTIAL_CONTENT, "{value}");
        let range = plan.range().unwrap();
        assert_eq!((range.first(), range.last()), (first, last), "{value}");
        assert_eq!(range.byte_length(), last - first + 1);
    }
    for value in ["bytes=10-", "bytes=10-11", "bytes=-0"] {
        let plan = get_plan(&headers(header::RANGE, value), 10).unwrap();
        assert_eq!(plan.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(plan.range(), None);
        assert!(!plan.requires_storage());
    }
}

#[test]
fn a_valid_multi_range_is_ignored_whole_never_fake_multipart_or_partial_content() {
    for value in [
        "bytes=0-2,5-7",
        "bytes=0-2, 5-",
        "bytes=-2,0-2",
        "bytes=999-1000,2000-",
        "bytes=0-2,, 5-7,",
        "items=an-unknown-unit",
    ] {
        let plan = get_plan(&headers(header::RANGE, value), 10).unwrap();
        assert_eq!(plan.status(), StatusCode::OK, "{value}");
        assert_eq!(plan.range(), None);
        assert!(plan.requires_storage());
    }
    // Tolerated empty list elements do not turn one actual member into multi-range.
    let plan = get_plan(&headers(header::RANGE, "bytes=, 2-4,,"), 10).unwrap();
    assert_eq!(plan.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(plan.range().unwrap().first(), 2);
}

#[test]
fn malformed_ranges_and_every_invalid_multi_range_member_are_typed_400_inputs() {
    for value in [
        "",
        "bytes",
        "bytes =0-2",
        "=0-2",
        "bytes=-",
        "bytes=2-1",
        "bytes=+1-2",
        "bytes=1 -2",
        "bytes=0--1",
        "bytes=18446744073709551616-",
        "bytes=0-18446744073709551616",
        "bytes=0-2,broken",
        "bytes=broken,5-7",
        "bytes=,,,",
    ] {
        assert_eq!(
            get_plan(&headers(header::RANGE, value), 10),
            Err(AudioTransportError::MalformedRange),
            "{value}"
        );
    }
    let excessive = format!("bytes={}", vec!["0-0"; MAX_LIST_ITEMS + 1].join(","));
    assert_eq!(
        get_plan(&headers(header::RANGE, &excessive), 10),
        Err(AudioTransportError::MalformedRange)
    );
    let mut fields = headers(header::RANGE, "bytes=0-2");
    fields.append(header::RANGE, HeaderValue::from_static("bytes=5-7"));
    assert_eq!(
        get_plan(&fields, 10),
        Err(AudioTransportError::MalformedRange)
    );
    let mut fields = HeaderMap::new();
    fields.insert(
        header::RANGE,
        HeaderValue::from_bytes(b"bytes=\xff-2").unwrap(),
    );
    assert_eq!(
        get_plan(&fields, 10),
        Err(AudioTransportError::MalformedRange)
    );
}

#[test]
fn head_ignores_range_and_if_range_even_when_duplicate_or_malformed() {
    let mut fields = headers(header::RANGE, "broken");
    fields.append(header::RANGE, HeaderValue::from_static("bytes=999-"));
    fields.insert(header::IF_RANGE, HeaderValue::from_static("broken"));
    fields.append(header::IF_RANGE, HeaderValue::from_static("also broken"));
    let plan = plan(&fields, &Method::HEAD, SHA256, 10).unwrap();
    assert_eq!(plan.status(), StatusCode::OK);
    assert_eq!(plan.range(), None);
}

#[test]
fn if_range_requires_exact_strong_tag_and_never_invents_a_timestamp() {
    let mut fields = headers(header::RANGE, "bytes=2-4");
    fields.insert(
        header::IF_RANGE,
        HeaderValue::from_str(&format!("\"{SHA256}\"")).unwrap(),
    );
    assert_eq!(
        get_plan(&fields, 10).unwrap().status(),
        StatusCode::PARTIAL_CONTENT
    );
    for value in [
        format!("W/\"{SHA256}\""),
        "\"different\"".to_owned(),
        "Sun, 06 Nov 1994 08:49:37 GMT".to_owned(),
        "Sunday, 06-Nov-94 08:49:37 GMT".to_owned(),
        "Sun Nov  6 08:49:37 1994".to_owned(),
    ] {
        fields.insert(header::IF_RANGE, HeaderValue::from_str(&value).unwrap());
        assert_eq!(
            get_plan(&fields, 10).unwrap().status(),
            StatusCode::OK,
            "{value}"
        );
    }
    fields.insert(
        header::IF_RANGE,
        HeaderValue::from_static("not-a-validator"),
    );
    assert_eq!(
        get_plan(&fields, 10),
        Err(AudioTransportError::MalformedIfRange)
    );
    fields.insert(header::IF_RANGE, HeaderValue::from_static("\"different\""));
    fields.append(header::IF_RANGE, HeaderValue::from_static("\"different\""));
    assert_eq!(
        get_plan(&fields, 10),
        Err(AudioTransportError::MalformedIfRange)
    );
    // If-Range is inapplicable without Range, including duplicate/invalid field lines.
    fields.remove(header::RANGE);
    assert_eq!(get_plan(&fields, 10).unwrap().status(), StatusCode::OK);
}

#[test]
fn etag_preconditions_observe_strong_if_match_and_weak_if_none_match() {
    for (name, value, expected) in [
        (header::IF_MATCH, "*".to_owned(), StatusCode::OK),
        (header::IF_MATCH, format!("\"{SHA256}\""), StatusCode::OK),
        (
            header::IF_MATCH,
            format!("W/\"{SHA256}\""),
            StatusCode::PRECONDITION_FAILED,
        ),
        (
            header::IF_MATCH,
            "\"other\"".to_owned(),
            StatusCode::PRECONDITION_FAILED,
        ),
        (
            header::IF_MATCH,
            format!("\"other,tag\", \"{SHA256}\""),
            StatusCode::OK,
        ),
        (
            header::IF_NONE_MATCH,
            "*".to_owned(),
            StatusCode::NOT_MODIFIED,
        ),
        (
            header::IF_NONE_MATCH,
            format!("\"{SHA256}\""),
            StatusCode::NOT_MODIFIED,
        ),
        (
            header::IF_NONE_MATCH,
            format!("W/\"{SHA256}\""),
            StatusCode::NOT_MODIFIED,
        ),
        (
            header::IF_NONE_MATCH,
            "\"other\"".to_owned(),
            StatusCode::OK,
        ),
        (
            header::IF_MATCH,
            "".to_owned(),
            StatusCode::PRECONDITION_FAILED,
        ),
        (header::IF_NONE_MATCH, "".to_owned(), StatusCode::OK),
    ] {
        for method in [Method::GET, Method::HEAD] {
            let plan = plan(&headers(name.clone(), &value), &method, SHA256, 10).unwrap();
            assert_eq!(plan.status(), expected, "{name}: {value} ({method})");
        }
    }
    for name in [header::IF_MATCH, header::IF_NONE_MATCH] {
        let mut fields = headers(name.clone(), "\"other\"");
        fields.append(
            name.clone(),
            HeaderValue::from_str(&format!("\"{SHA256}\"")).unwrap(),
        );
        let expected = if name == header::IF_MATCH {
            StatusCode::OK
        } else {
            StatusCode::NOT_MODIFIED
        };
        assert_eq!(get_plan(&fields, 10).unwrap().status(), expected);
    }
}

#[test]
fn precondition_order_precedes_range_and_short_circuits_inapplicable_fields() {
    let mut fields = headers(header::RANGE, "malformed");
    fields.insert(header::IF_MATCH, HeaderValue::from_static("\"other\""));
    fields.insert(header::IF_NONE_MATCH, HeaderValue::from_static("malformed"));
    assert_eq!(
        get_plan(&fields, 10).unwrap().status(),
        StatusCode::PRECONDITION_FAILED
    );
    fields.insert(header::IF_MATCH, HeaderValue::from_static("*"));
    fields.insert(
        header::IF_NONE_MATCH,
        HeaderValue::from_str(&format!("W/\"{SHA256}\"")).unwrap(),
    );
    fields.append(header::RANGE, HeaderValue::from_static("also malformed"));
    assert_eq!(
        get_plan(&fields, 10).unwrap().status(),
        StatusCode::NOT_MODIFIED
    );
    fields.insert(header::IF_NONE_MATCH, HeaderValue::from_static("\"other\""));
    assert_eq!(
        get_plan(&fields, 10),
        Err(AudioTransportError::MalformedRange)
    );
    fields.insert(header::RANGE, HeaderValue::from_static("bytes=2-4"));
    assert_eq!(
        get_plan(&fields, 10).unwrap().status(),
        StatusCode::PARTIAL_CONTENT
    );
    // No Last-Modified fact exists, so date preconditions do not override entity tags.
    fields.insert(
        header::IF_MODIFIED_SINCE,
        HeaderValue::from_static("Sun, 06 Nov 1994 08:49:37 GMT"),
    );
    fields.insert(
        header::IF_UNMODIFIED_SINCE,
        HeaderValue::from_static("Sun, 06 Nov 1994 08:49:37 GMT"),
    );
    assert_eq!(
        get_plan(&fields, 10).unwrap().status(),
        StatusCode::PARTIAL_CONTENT
    );
}

#[test]
fn malformed_tag_fields_fail_exactly_and_valid_opaque_commas_or_obs_text_do_not() {
    for (name, error) in [
        (header::IF_MATCH, AudioTransportError::MalformedIfMatch),
        (
            header::IF_NONE_MATCH,
            AudioTransportError::MalformedIfNoneMatch,
        ),
    ] {
        for value in [
            "bare",
            "\"unterminated",
            "w/\"tag\"",
            "\"bad tag\"",
            "*, \"tag\"",
            "\"tag\" trailing",
        ] {
            assert_eq!(
                get_plan(&headers(name.clone(), value), 10),
                Err(error),
                "{value}"
            );
        }
        let mut fields = headers(name.clone(), "*");
        fields.append(name.clone(), HeaderValue::from_static("\"tag\""));
        assert_eq!(get_plan(&fields, 10), Err(error));
        let mut fields = HeaderMap::new();
        fields.insert(
            name.clone(),
            HeaderValue::from_bytes(b"\"\xff,opaque\"").unwrap(),
        );
        let expected = if name == header::IF_MATCH {
            StatusCode::PRECONDITION_FAILED
        } else {
            StatusCode::OK
        };
        assert_eq!(get_plan(&fields, 10).unwrap().status(), expected);
    }
}

#[test]
fn evaluated_header_budgets_cover_below_at_and_above_without_an_rfc_size_claim() {
    for (name, error, accepted_status) in [
        (
            header::RANGE,
            AudioTransportError::MalformedRange,
            StatusCode::PARTIAL_CONTENT,
        ),
        (
            header::IF_RANGE,
            AudioTransportError::MalformedIfRange,
            StatusCode::PARTIAL_CONTENT,
        ),
        (
            header::IF_MATCH,
            AudioTransportError::MalformedIfMatch,
            StatusCode::OK,
        ),
        (
            header::IF_NONE_MATCH,
            AudioTransportError::MalformedIfNoneMatch,
            StatusCode::NOT_MODIFIED,
        ),
    ] {
        for cost in [MAX_HEADER_BYTES - 1, MAX_HEADER_BYTES, MAX_HEADER_BYTES + 1] {
            let value_length = cost - FIELD_LINE_COST;
            let value = if name == header::RANGE {
                // Large but valid numeric spelling; all leading zeroes still mean byte1.
                format!("bytes={}1-1", "0".repeat(value_length - 9))
            } else {
                let tag = format!("\"{SHA256}\"");
                format!("{}{}", " ".repeat(value_length - tag.len()), tag)
            };
            assert_eq!(value.len() + FIELD_LINE_COST, cost);
            let mut fields = headers(name.clone(), &value);
            if name == header::IF_RANGE {
                fields.insert(header::RANGE, HeaderValue::from_static("bytes=1-1"));
            }
            let result = get_plan(&fields, 3);
            if cost <= MAX_HEADER_BYTES {
                assert_eq!(result.unwrap().status(), accepted_status, "{name}: {cost}");
            } else {
                assert_eq!(result, Err(error), "{name}: {cost}");
            }
        }
    }
}

#[test]
fn repeated_etag_field_lines_share_one_budget_including_empty_line_cost() {
    for (name, error, accepted_status) in [
        (
            header::IF_MATCH,
            AudioTransportError::MalformedIfMatch,
            StatusCode::PRECONDITION_FAILED,
        ),
        (
            header::IF_NONE_MATCH,
            AudioTransportError::MalformedIfNoneMatch,
            StatusCode::OK,
        ),
    ] {
        for cost in [MAX_HEADER_BYTES - 1, MAX_HEADER_BYTES, MAX_HEADER_BYTES + 1] {
            let mut fields = HeaderMap::new();
            let first = if cost % FIELD_LINE_COST == 0 { "" } else { " " };
            fields.append(name.clone(), HeaderValue::from_static(first));
            let other_lines = (cost - first.len() - FIELD_LINE_COST) / FIELD_LINE_COST;
            for _ in 0..other_lines {
                fields.append(name.clone(), HeaderValue::from_static(""));
            }
            let result = get_plan(&fields, 3);
            if cost <= MAX_HEADER_BYTES {
                assert_eq!(result.unwrap().status(), accepted_status, "{name}: {cost}");
            } else {
                assert_eq!(result, Err(error), "{name}: {cost}");
            }
        }
    }
}

#[test]
fn ignored_range_bypasses_its_budget_after_conditionals_if_range_mismatch_or_head() {
    let oversized = format!("bytes={}1-1", "0".repeat(MAX_HEADER_BYTES));
    let mut fields = headers(header::RANGE, &oversized);
    fields.insert(header::IF_RANGE, HeaderValue::from_static("\"different\""));
    assert_eq!(get_plan(&fields, 3).unwrap().status(), StatusCode::OK);
    fields.insert(
        header::IF_RANGE,
        HeaderValue::from_str(&format!("\"{SHA256}\"")).unwrap(),
    );
    assert_eq!(
        get_plan(&fields, 3),
        Err(AudioTransportError::MalformedRange)
    );
    assert_eq!(
        plan(&fields, &Method::HEAD, SHA256, 3).unwrap().status(),
        StatusCode::OK
    );
    fields.insert(header::IF_NONE_MATCH, HeaderValue::from_static("*"));
    assert_eq!(
        get_plan(&fields, 3).unwrap().status(),
        StatusCode::NOT_MODIFIED
    );
    fields.insert(header::IF_MATCH, HeaderValue::from_static("\"different\""));
    fields.insert(
        header::IF_NONE_MATCH,
        HeaderValue::from_str(&" ".repeat(MAX_HEADER_BYTES)).unwrap(),
    );
    assert_eq!(
        get_plan(&fields, 3).unwrap().status(),
        StatusCode::PRECONDITION_FAILED
    );
    // If-Range has no meaning without Range, so its own oversized field is then ignored.
    fields.remove(header::IF_MATCH);
    fields.remove(header::IF_NONE_MATCH);
    fields.remove(header::RANGE);
    fields.insert(
        header::IF_RANGE,
        HeaderValue::from_str(&" ".repeat(MAX_HEADER_BYTES)).unwrap(),
    );
    assert_eq!(get_plan(&fields, 3).unwrap().status(), StatusCode::OK);
}

#[test]
fn finite_http_range_space_agrees_with_independent_byte_membership_oracle() {
    for total in 1_u64..=16 {
        for first in 0_u64..=20 {
            for last in first..=20 {
                let plan = get_plan(
                    &headers(header::RANGE, &format!("bytes={first}-{last}")),
                    total,
                )
                .unwrap();
                let expected: Vec<_> = (0..total)
                    .filter(|index| first <= *index && *index <= last)
                    .collect();
                assert_range_membership(&plan, &expected);
            }
            let plan =
                get_plan(&headers(header::RANGE, &format!("bytes={first}-")), total).unwrap();
            let expected: Vec<_> = (0..total).filter(|index| *index >= first).collect();
            assert_range_membership(&plan, &expected);
        }
        for suffix in 0_u64..=20 {
            let plan =
                get_plan(&headers(header::RANGE, &format!("bytes=-{suffix}")), total).unwrap();
            let expected: Vec<_> = (0..total)
                .rev()
                .take(suffix as usize)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            assert_range_membership(&plan, &expected);
        }
    }
}

fn assert_range_membership(plan: &AudioResponsePlan, expected: &[u64]) {
    if expected.is_empty() {
        assert_eq!(plan.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(plan.range(), None);
    } else {
        assert_eq!(plan.status(), StatusCode::PARTIAL_CONTENT);
        let range = plan.range().unwrap();
        assert_eq!((range.first()..=range.last()).collect::<Vec<_>>(), expected);
        assert_eq!(range.byte_length(), expected.len() as u64);
    }
}

#[derive(Default)]
struct Observations {
    polls: AtomicUsize,
    drops: AtomicUsize,
}

struct Source {
    chunks: VecDeque<Result<Bytes, StorageError>>,
    pending: bool,
    observations: Arc<Observations>,
}

impl Stream for Source {
    type Item = Result<Bytes, StorageError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.observations.polls.fetch_add(1, Ordering::SeqCst);
        if self.pending {
            Poll::Pending
        } else {
            Poll::Ready(self.chunks.pop_front())
        }
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        self.observations.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn make_source(
    chunks: Vec<Result<Bytes, StorageError>>,
    pending: bool,
) -> (StorageStream, Arc<Observations>) {
    let observations = Arc::new(Observations::default());
    let source = Source {
        chunks: chunks.into(),
        pending,
        observations: observations.clone(),
    };
    (Box::pin(source), observations)
}

#[tokio::test]
async fn successful_get_and_range_bodies_are_exact_binary_bytes_without_metadata_negotiation() {
    for (range, expected, content_range) in [
        (None, b"abc".as_slice(), None),
        (Some("bytes=1-2"), b"bc".as_slice(), Some("bytes 1-2/3")),
    ] {
        let mut fields = headers(header::ACCEPT, "application/msgpack");
        fields.insert(
            header::ACCEPT_ENCODING,
            HeaderValue::from_static("gzip, br"),
        );
        if let Some(range) = range {
            fields.insert(header::RANGE, HeaderValue::from_static(range));
        }
        let plan = get_plan(&fields, 3).unwrap();
        let expected_status = plan.status();
        let (source, observations) = make_source(vec![Ok(Bytes::copy_from_slice(expected))], false);
        let (lanes, permit) = lane();
        let response = response(
            StorageRead {
                stream: source,
                byte_length: expected.len() as u64,
            },
            plan,
            &Method::GET,
            permit,
        )
        .unwrap();
        assert_eq!(response.status(), expected_status);
        assert_private(response.headers());
        assert_eq!(response.headers()[header::CONTENT_TYPE], "audio/ogg");
        assert_eq!(
            response.headers()[header::CONTENT_LENGTH],
            expected.len().to_string()
        );
        assert_eq!(response.headers()[header::ACCEPT_RANGES], "bytes");
        assert_eq!(response.headers()[header::ETAG], format!("\"{SHA256}\""));
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_RANGE)
                .map(|value| value.to_str().unwrap()),
            content_range
        );
        assert_eq!(observations.polls.load(Ordering::SeqCst), 0);
        assert_eq!(lanes.available_permits(), 0);
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap().as_ref(),
            expected
        );
        assert_eq!(observations.drops.load(Ordering::SeqCst), 1);
        assert_eq!(lanes.available_permits(), 1);
    }
}

#[tokio::test]
async fn head_has_whole_length_without_polling_source_and_releases_its_lane() {
    let fields = headers(header::RANGE, "bytes=1-2");
    let plan = plan(&fields, &Method::HEAD, SHA256, 3).unwrap();
    let (source, observations) = make_source(vec![], true);
    let (lanes, permit) = lane();
    let response = response(
        StorageRead {
            stream: source,
            byte_length: 3,
        },
        plan,
        &Method::HEAD,
        permit,
    )
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_private(response.headers());
    assert_eq!(response.headers()[header::CONTENT_LENGTH], "3");
    assert!(!response.headers().contains_key(header::CONTENT_RANGE));
    assert!(to_bytes(response.into_body(), 1024)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(observations.polls.load(Ordering::SeqCst), 0);
    assert_eq!(observations.drops.load(Ordering::SeqCst), 1);
    assert_eq!(lanes.available_permits(), 1);
}

#[tokio::test]
async fn conditional_and_416_metadata_are_private_and_preserve_caller_owned_error_bodies() {
    for (fields, status, range) in [
        (
            headers(header::IF_NONE_MATCH, "*"),
            StatusCode::NOT_MODIFIED,
            None,
        ),
        (
            headers(header::IF_MATCH, "\"other\""),
            StatusCode::PRECONDITION_FAILED,
            None,
        ),
        (
            headers(header::RANGE, "bytes=3-"),
            StatusCode::RANGE_NOT_SATISFIABLE,
            Some("bytes */3"),
        ),
    ] {
        let plan = get_plan(&fields, 3).unwrap();
        let (lanes, permit) = lane();
        let response = metadata_response(plan, &Method::GET, permit, rejection()).unwrap();
        assert_eq!(response.status(), status);
        assert_private(response.headers());
        assert_eq!(response.headers()[header::ETAG], format!("\"{SHA256}\""));
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_RANGE)
                .map(|value| value.to_str().unwrap()),
            range
        );
        if status == StatusCode::NOT_MODIFIED {
            assert!(!response.headers().contains_key(header::CONTENT_LENGTH));
        }
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        if status == StatusCode::NOT_MODIFIED {
            assert!(body.is_empty());
        } else {
            assert_eq!(body.as_ref(), b"caller-owned rejection");
        }
        assert_eq!(lanes.available_permits(), 1);
    }
}

#[tokio::test]
async fn inconsistent_plan_or_declared_length_is_rejected_before_polling_with_no_lane_leak() {
    let (source, observations) = make_source(vec![], true);
    let (lanes, permit) = lane();
    let error = response(
        StorageRead {
            stream: source,
            byte_length: 2,
        },
        get_plan(&HeaderMap::new(), 3).unwrap(),
        &Method::GET,
        permit,
    )
    .unwrap_err();
    assert_eq!(error, AudioTransportError::DeclaredLengthMismatch);
    assert_eq!(observations.polls.load(Ordering::SeqCst), 0);
    assert_eq!(observations.drops.load(Ordering::SeqCst), 1);
    assert_eq!(lanes.available_permits(), 1);
    let (_, permit) = lane();
    assert_eq!(
        metadata_response(
            get_plan(&HeaderMap::new(), 3).unwrap(),
            &Method::GET,
            permit,
            rejection(),
        )
        .unwrap_err(),
        AudioTransportError::InconsistentPlan
    );
    let (source, _) = make_source(vec![], true);
    let (_, permit) = lane();
    assert_eq!(
        response(
            StorageRead {
                stream: source,
                byte_length: 3
            },
            get_plan(&HeaderMap::new(), 3).unwrap(),
            &Method::HEAD,
            permit
        )
        .unwrap_err(),
        AudioTransportError::InconsistentPlan
    );
}

#[tokio::test]
async fn streaming_is_demand_driven_pass_through_and_releases_only_after_eof_or_drop() {
    let chunks = vec![
        Ok(Bytes::from_static(b"ab")),
        Ok(Bytes::new()),
        Ok(Bytes::from_static(b"c")),
    ];
    let (source, observations) = make_source(chunks, false);
    let (lanes, permit) = lane();
    let mut stream = bounded_stream(source, 3, permit);
    assert_eq!(observations.polls.load(Ordering::SeqCst), 0);
    for expected in [b"ab".as_slice(), b"".as_slice(), b"c".as_slice()] {
        assert_eq!(stream.next().await.unwrap().unwrap().as_ref(), expected);
        assert_eq!(lanes.available_permits(), 0);
        assert_eq!(observations.drops.load(Ordering::SeqCst), 0);
    }
    assert_eq!(observations.polls.load(Ordering::SeqCst), 3);
    assert!(stream.next().await.is_none());
    assert_eq!(lanes.available_permits(), 1);
    assert_eq!(observations.drops.load(Ordering::SeqCst), 1);
    assert!(stream.next().await.is_none());
    assert_eq!(observations.polls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn caller_drop_cancels_source_before_first_poll_and_mid_body_without_detached_work() {
    for poll_first in [false, true] {
        let (source, observations) = make_source(
            vec![Ok(Bytes::from_static(b"a")), Ok(Bytes::from_static(b"bc"))],
            false,
        );
        let (lanes, permit) = lane();
        let mut stream = bounded_stream(source, 3, permit);
        if poll_first {
            assert_eq!(
                stream.next().await.unwrap().unwrap(),
                Bytes::from_static(b"a")
            );
        }
        drop(stream);
        assert_eq!(lanes.available_permits(), 1);
        assert_eq!(observations.drops.load(Ordering::SeqCst), 1);
        assert_eq!(
            observations.polls.load(Ordering::SeqCst),
            usize::from(poll_first)
        );
    }
    let (source, observations) = make_source(vec![], true);
    let (lanes, permit) = lane();
    let response = response(
        StorageRead {
            stream: source,
            byte_length: 3,
        },
        get_plan(&HeaderMap::new(), 3).unwrap(),
        &Method::GET,
        permit,
    )
    .unwrap();
    drop(response);
    assert_eq!(lanes.available_permits(), 1);
    assert_eq!(observations.drops.load(Ordering::SeqCst), 1);
    assert_eq!(observations.polls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn truncation_overshoot_source_error_and_timeout_are_terminal_and_release_immediately() {
    for (chunks, expected) in [
        (vec![], StorageError::LengthMismatch),
        (
            vec![Ok(Bytes::from_static(b"abcd"))],
            StorageError::LengthMismatch,
        ),
        (
            vec![
                Err(StorageError::Unavailable),
                Ok(Bytes::from_static(b"abc")),
            ],
            StorageError::Unavailable,
        ),
        (vec![Err(StorageError::Cancelled)], StorageError::Cancelled),
    ] {
        let (source, observations) = make_source(chunks, false);
        let (lanes, permit) = lane();
        let mut stream = bounded_stream(source, 3, permit);
        assert_eq!(stream.next().await.unwrap(), Err(expected));
        assert_eq!(lanes.available_permits(), 1);
        assert_eq!(observations.drops.load(Ordering::SeqCst), 1);
        assert!(stream.next().await.is_none());
        assert_eq!(observations.polls.load(Ordering::SeqCst), 1);
    }
    let (source, observations) = make_source(vec![Ok(Bytes::from_static(b"ab"))], false);
    let (lanes, permit) = lane();
    let mut stream = bounded_stream(source, 3, permit);
    assert_eq!(
        stream.next().await.unwrap().unwrap(),
        Bytes::from_static(b"ab")
    );
    assert_eq!(
        stream.next().await.unwrap(),
        Err(StorageError::LengthMismatch)
    );
    assert_eq!(lanes.available_permits(), 1);
    assert_eq!(observations.drops.load(Ordering::SeqCst), 1);
    assert!(stream.next().await.is_none());
    let (source, observations) = make_source(vec![], true);
    let (lanes, permit) = lane();
    let mut stream =
        bounded_stream_until(source, 3, permit, Instant::now() - Duration::from_secs(1));
    assert_eq!(stream.next().await.unwrap(), Err(StorageError::Timeout));
    assert_eq!(lanes.available_permits(), 1);
    assert_eq!(observations.drops.load(Ordering::SeqCst), 1);
    assert_eq!(observations.polls.load(Ordering::SeqCst), 0);
    assert!(stream.next().await.is_none());
}

#[test]
fn representation_facts_and_method_are_validated_without_forging_a_read_scope() {
    for (sha, length) in [("not-a-sha", 3), (SHA256, 0)] {
        assert_eq!(
            plan(&HeaderMap::new(), &Method::GET, sha, length),
            Err(AudioTransportError::InvalidRepresentation)
        );
    }
    assert_eq!(
        plan(&HeaderMap::new(), &Method::POST, SHA256, 3),
        Err(AudioTransportError::UnsupportedMethod)
    );
}

#[test]
fn private_response_enforces_cache_and_encoding_without_borrowing_a_wire_error_contract() {
    let mut response = rejection();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    let response = private_response(response);
    assert_private(response.headers());
    assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
}
