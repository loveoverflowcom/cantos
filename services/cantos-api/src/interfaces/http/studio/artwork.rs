//! Private owner Work-cover transport. Only metadata negotiates JSON/Protobuf; encoded
//! image bytes stay unchanged and acquire bounded admission before body extraction.
use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Extension, FromRequest, FromRequestParts, Query, Request, State},
    http::{header, request::Parts, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use cantos_contracts::{self as dto, protobuf::ProtobufDto, wire::WireFormat};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::{catalog, StudioPath};
use crate::{
    contexts::{
        catalog::CatalogStoreError,
        media::{
            domain::{artwork::ArtworkError, image::ImageError},
            image_intake::ImageIntakeError,
            ArtworkApplicationError, ArtworkReceipt, ArtworkService, ImageObjectError,
        },
    },
    interfaces::http::{cookie_token, error_response, wire, AppState},
    platform::postgres::StoreError,
};

const METADATA_LIMIT: usize = 32 * 1024;
const IMAGE_LIMIT: usize = 8 * 1024 * 1024;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/works/{work}/image-intents", post(prepare))
        .route("/works/{work}/image-intents/{intent}", get(intent))
        .route(
            "/works/{work}/image-intents/{intent}/bytes",
            put(upload).layer(DefaultBodyLimit::max(IMAGE_LIMIT)),
        )
        .route("/works/{work}/image-assets/{asset}", get(asset))
        .route("/works/{work}/image-assets/{asset}/bytes", get(preview))
        .route("/works/{work}/artwork", get(artwork).patch(set_artwork))
        .method_not_allowed_fallback(|| async {
            failure(
                StatusCode::METHOD_NOT_ALLOWED,
                dto::ErrorCode::InvalidRequest,
                None,
            )
        })
        .layer(DefaultBodyLimit::max(METADATA_LIMIT))
}

struct ArtworkAccepts(WireFormat);
impl<S: Send + Sync> FromRequestParts<S> for ArtworkAccepts {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Response> {
        wire::artwork_response_format(wire::accept_header(&parts.headers).as_deref())
            .map(Self)
            .map_err(|_| {
                failure(
                    StatusCode::NOT_ACCEPTABLE,
                    dto::ErrorCode::InvalidRequest,
                    Some(("accept", "not_acceptable")),
                )
            })
    }
}

struct ArtworkBody<T>(T);
impl<S: Send + Sync, T: DeserializeOwned + ProtobufDto> FromRequest<S> for ArtworkBody<T> {
    type Rejection = Response;
    async fn from_request(request: Request, state: &S) -> Result<Self, Response> {
        let format = wire::artwork_request_format(
            request
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
        )
        .map_err(|_| {
            failure(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                dto::ErrorCode::InvalidRequest,
                Some(("content-type", "unsupported_media_type")),
            )
        })?;
        if request.headers().contains_key(header::CONTENT_ENCODING) {
            return Err(failure(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                dto::ErrorCode::InvalidRequest,
                Some(("content-encoding", "unsupported_content_encoding")),
            ));
        }
        let bytes = Bytes::from_request(request, state).await.map_err(|e| {
            let status = if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            };
            failure(status, dto::ErrorCode::InvalidRequest, None)
        })?;
        dto::wire::decode_artwork(format, &bytes)
            .map(Self)
            .map_err(|_| {
                failure(
                    StatusCode::BAD_REQUEST,
                    dto::ErrorCode::InvalidRequest,
                    None,
                )
            })
    }
}

fn private_headers(mut response: Response, varies: bool) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if varies {
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("accept"));
    }
    response
}
fn failure(status: StatusCode, code: dto::ErrorCode, issue: Option<(&str, &str)>) -> Response {
    private_headers(
        (
            status,
            Json(dto::ApiError {
                code,
                current_revision: None,
                issues: issue
                    .into_iter()
                    .map(|(path, rule)| dto::FieldIssue {
                        path: path.into(),
                        rule: rule.into(),
                    })
                    .collect(),
            }),
        )
            .into_response(),
        true,
    )
}
fn encoded(format: WireFormat, bytes: Vec<u8>, status: StatusCode) -> Response {
    private_headers(
        (status, [(header::CONTENT_TYPE, format.media_type())], bytes).into_response(),
        true,
    )
}
fn receipt<T: Serialize + ProtobufDto>(
    format: WireFormat,
    value: ArtworkReceipt<T>,
    status: StatusCode,
) -> Response {
    if format == WireFormat::Json {
        return encoded(format, value.into_parts().1, status);
    }
    respond(format, value.value(), status)
}
fn respond<T: Serialize + ProtobufDto>(
    format: WireFormat,
    value: &T,
    status: StatusCode,
) -> Response {
    match dto::wire::encode_artwork(format, value) {
        Ok(bytes) => encoded(format, bytes, status),
        Err(_) => private_headers(error_response(StoreError::Unavailable), true),
    }
}
fn admitted_image_response(
    format: WireFormat,
    image: ArtworkReceipt<dto::ImageAssetDocument>,
) -> Response {
    receipt(format, image, StatusCode::OK)
}
fn error(error: ArtworkApplicationError) -> Response {
    let response = match error {
        ArtworkApplicationError::Storage(e) => error_response(e),
        ArtworkApplicationError::Catalog(e) => catalog::catalog_error(e),
        ArtworkApplicationError::Image(e) => image_error(e),
        ArtworkApplicationError::Artwork(e) => match e {
            ArtworkError::Image(e) => image_error(e),
            ArtworkError::Catalog(e) => catalog::catalog_error(CatalogStoreError::Domain(e)),
            ArtworkError::BlankAltText
            | ArtworkError::AltTextTooLong
            | ArtworkError::AltTextControlCharacter => failure(
                StatusCode::UNPROCESSABLE_ENTITY,
                dto::ErrorCode::InvalidRequest,
                Some(("artwork.alt_text", "invalid_alt_text")),
            ),
            ArtworkError::AssetBindingMismatch
            | ArtworkError::MissingAsset
            | ArtworkError::UnexpectedAsset => failure(
                StatusCode::UNPROCESSABLE_ENTITY,
                dto::ErrorCode::InvalidRequest,
                Some(("artwork", "asset_binding_mismatch")),
            ),
            ArtworkError::InvalidRecordedBy
            | ArtworkError::DuplicateRevision
            | ArtworkError::InvalidResponse => error_response(StoreError::CorruptRevision),
        },
        ArtworkApplicationError::Intake(e) => match e {
            ImageIntakeError::AdmissionBusy | ImageIntakeError::InspectionUnavailable => {
                error_response(StoreError::Unavailable)
            }
            ImageIntakeError::ImageTooLarge => failure(
                StatusCode::PAYLOAD_TOO_LARGE,
                dto::ErrorCode::InvalidRequest,
                Some(("bytes", e.code())),
            ),
            e => failure(
                StatusCode::UNPROCESSABLE_ENTITY,
                dto::ErrorCode::InvalidRequest,
                Some(("bytes", e.code())),
            ),
        },
        ArtworkApplicationError::Object(e) => match e {
            ImageObjectError::NotFound => error_response(StoreError::NotFound),
            ImageObjectError::Unavailable => error_response(StoreError::Unavailable),
            ImageObjectError::Corrupt => error_response(StoreError::CorruptRevision),
        },
    };
    private_headers(response, true)
}
fn image_error(error: ImageError) -> Response {
    use ImageError::*;
    match error {
        OperationReused => failure(StatusCode::CONFLICT, dto::ErrorCode::OperationReused, None),
        DuplicateIntent => failure(
            StatusCode::CONFLICT,
            dto::ErrorCode::InvalidRequest,
            Some(("id", "duplicate_id")),
        ),
        IntentExpired => failure(
            StatusCode::CONFLICT,
            dto::ErrorCode::InvalidRequest,
            Some(("intent", "intent_expired")),
        ),
        ImageTooLarge | DocumentTooLarge => failure(
            StatusCode::PAYLOAD_TOO_LARGE,
            dto::ErrorCode::InvalidRequest,
            None,
        ),
        ExpectedHashMismatch => failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            dto::ErrorCode::InvalidRequest,
            Some(("bytes", "expected_hash_mismatch")),
        ),
        ExpectedLengthMismatch => failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            dto::ErrorCode::InvalidRequest,
            Some(("bytes", "expected_length_mismatch")),
        ),
        ExpectedMimeMismatch | BindingMismatch => failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            dto::ErrorCode::InvalidRequest,
            Some(("bytes", "image_binding_mismatch")),
        ),
        Encoding => error_response(StoreError::CorruptRevision),
        InvalidIntentId
        | InvalidAssetId
        | InvalidOperationId
        | InvalidWorkId
        | InvalidOwnerId
        | InvalidHash
        | InvalidDigest
        | UnsupportedMime
        | UnsupportedProfile
        | UnsupportedSchema
        | EmptyImage
        | InvalidDimensions
        | UnsupportedLayout
        | DecodedImageTooLarge
        | OutputSizeOverflow
        | BlankProvenance
        | ProvenanceTooLong
        | ControlCharacter
        | MissingRightsRefs
        | TooManyRightsRefs
        | InvalidRightsRef
        | DuplicateRightsRef
        | NoncanonicalRightsRefs
        | InvalidTimestamp
        | InvalidExpiry
        | IntentNotYetCreated => failure(
            StatusCode::UNPROCESSABLE_ENTITY,
            dto::ErrorCode::InvalidRequest,
            Some(("image", "invalid_image_metadata")),
        ),
    }
}

async fn prepare(
    Extension(service): Extension<ArtworkService>,
    StudioPath(work): StudioPath<String>,
    ArtworkAccepts(format): ArtworkAccepts,
    headers: HeaderMap,
    ArtworkBody(request): ArtworkBody<dto::CreateImageIntentRequest>,
) -> Response {
    let token = match cookie_token(&headers) {
        Ok(t) => t,
        Err(e) => return headers_error(e),
    };
    match service.prepare(&token, &work, request).await {
        Ok(v) => receipt(format, v, StatusCode::CREATED),
        Err(e) => error(e),
    }
}
async fn intent(
    Extension(service): Extension<ArtworkService>,
    StudioPath((work, intent)): StudioPath<(String, String)>,
    ArtworkAccepts(format): ArtworkAccepts,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers) {
        Ok(t) => t,
        Err(e) => return headers_error(e),
    };
    match service.intent(&token, &work, &intent).await {
        Ok(v) => respond(format, &v, StatusCode::OK),
        Err(e) => error(e),
    }
}
async fn upload(
    State(state): State<AppState>,
    Extension(service): Extension<ArtworkService>,
    StudioPath((work, intent)): StudioPath<(String, String)>,
    ArtworkAccepts(format): ArtworkAccepts,
    request: Request,
) -> Response {
    let token = match cookie_token(request.headers()) {
        Ok(t) => t,
        Err(e) => return headers_error(e),
    };
    let mime = match request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
    {
        Some("image/png") if !request.headers().contains_key(header::CONTENT_ENCODING) => {
            dto::ImageMime::Png
        }
        Some("image/jpeg") if !request.headers().contains_key(header::CONTENT_ENCODING) => {
            dto::ImageMime::Jpeg
        }
        _ => {
            return failure(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                dto::ErrorCode::InvalidRequest,
                Some(("content-type", "unsupported_media_type")),
            )
        }
    };
    let ticket = match service.begin_upload(&token, &work, &intent, mime).await {
        Ok(t) => t,
        Err(e) => return error(e),
    };
    let body = match Bytes::from_request(request, &state).await {
        Ok(b) => b,
        Err(e) => {
            return failure(
                if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                },
                dto::ErrorCode::InvalidRequest,
                None,
            )
        }
    };
    match ticket.admit(body.to_vec()).await {
        Ok(v) => admitted_image_response(format, v),
        Err(e) => error(e),
    }
}
async fn asset(
    Extension(service): Extension<ArtworkService>,
    StudioPath((work, asset)): StudioPath<(String, String)>,
    ArtworkAccepts(format): ArtworkAccepts,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers) {
        Ok(t) => t,
        Err(e) => return headers_error(e),
    };
    match service.asset(&token, &work, &asset).await {
        Ok(v) => receipt(format, v, StatusCode::OK),
        Err(e) => error(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtworkQuery {
    work_revision: Option<u64>,
}
async fn artwork(
    Extension(service): Extension<ArtworkService>,
    StudioPath(work): StudioPath<String>,
    ArtworkAccepts(format): ArtworkAccepts,
    query: Result<Query<ArtworkQuery>, axum::extract::rejection::QueryRejection>,
    headers: HeaderMap,
) -> Response {
    let query = match query {
        Ok(Query(q)) => q,
        Err(_) => {
            return failure(
                StatusCode::BAD_REQUEST,
                dto::ErrorCode::InvalidRequest,
                None,
            )
        }
    };
    let token = match cookie_token(&headers) {
        Ok(t) => t,
        Err(e) => return headers_error(e),
    };
    match service.artwork(&token, &work, query.work_revision).await {
        Ok(v) => respond(format, &v, StatusCode::OK),
        Err(e) => error(e),
    }
}
async fn set_artwork(
    Extension(service): Extension<ArtworkService>,
    StudioPath(work): StudioPath<String>,
    ArtworkAccepts(format): ArtworkAccepts,
    headers: HeaderMap,
    ArtworkBody(request): ArtworkBody<dto::SetWorkArtworkRequest>,
) -> Response {
    let token = match cookie_token(&headers) {
        Ok(t) => t,
        Err(e) => return headers_error(e),
    };
    match service.set_artwork(&token, &work, request).await {
        Ok(v) => receipt(format, v, StatusCode::OK),
        Err(e) => error(e),
    }
}
async fn preview(
    Extension(service): Extension<ArtworkService>,
    StudioPath((work, asset)): StudioPath<(String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers) {
        Ok(t) => t,
        Err(e) => return headers_error(e),
    };
    let image = match service.preview(&token, &work, &asset).await {
        Ok(v) => v,
        Err(e) => return error(e),
    };
    let metadata = image.metadata;
    // HEAD still performs verified open and current authorization, then drops the stream.
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(image.stream)
    };
    let mut response = body.into_response();
    for (name, value) in [
        (
            header::CONTENT_TYPE,
            metadata.content_type().as_str().to_owned(),
        ),
        (header::CONTENT_LENGTH, metadata.byte_length().to_string()),
        (header::ETAG, format!("\"{}\"", metadata.sha256())),
    ] {
        let Ok(value) = HeaderValue::from_str(&value) else {
            return headers_error(StoreError::CorruptRevision);
        };
        response.headers_mut().insert(name, value);
    }
    private_headers(response, false)
}
fn headers_error(error: StoreError) -> Response {
    private_headers(error_response(error), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tower::ServiceExt;

    fn request() -> dto::SetWorkArtworkRequest {
        dto::SetWorkArtworkRequest {
            operation_id: "00000000-0000-4000-8000-000000000009".into(),
            expected_work_revision: 7,
            artwork: None,
        }
    }
    fn probe() -> (Router, Arc<AtomicUsize>) {
        async fn handler(
            State(calls): State<Arc<AtomicUsize>>,
            ArtworkAccepts(format): ArtworkAccepts,
            ArtworkBody(value): ArtworkBody<dto::SetWorkArtworkRequest>,
        ) -> Response {
            calls.fetch_add(1, Ordering::SeqCst);
            respond(format, &value, StatusCode::OK)
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/probe", post(handler))
            .layer(DefaultBodyLimit::max(METADATA_LIMIT))
            .with_state(calls.clone());
        (app, calls)
    }
    async fn send(
        app: &Router,
        content_type: &str,
        accept: Option<&str>,
        body: Vec<u8>,
    ) -> Response {
        let mut request = HttpRequest::post("/probe").header(header::CONTENT_TYPE, content_type);
        if let Some(accept) = accept {
            request = request.header(header::ACCEPT, accept);
        }
        app.clone()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap()
    }
    #[tokio::test]
    async fn each_metadata_codec_pair_carries_explicit_clear_and_safe_headers() {
        let (app, calls) = probe();
        for sent in [WireFormat::Json, WireFormat::Protobuf] {
            for wanted in [WireFormat::Json, WireFormat::Protobuf] {
                let response = send(
                    &app,
                    sent.media_type(),
                    Some(wanted.media_type()),
                    dto::wire::encode_artwork(sent, &request()).unwrap(),
                )
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(
                    response.headers()[header::CONTENT_TYPE],
                    wanted.media_type()
                );
                assert_eq!(response.headers()[header::VARY], "accept");
                assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                assert_eq!(
                    response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                    "nosniff"
                );
                let bytes = response.into_body().collect().await.unwrap().to_bytes();
                assert_eq!(
                    dto::wire::decode_artwork::<dto::SetWorkArtworkRequest>(wanted, &bytes)
                        .unwrap(),
                    request()
                );
            }
        }
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }
    #[tokio::test]
    async fn json_default_is_exact_and_original_receipt_bytes_survive_replay() {
        let (app, _) = probe();
        let bytes = serde_json::to_vec(&request()).unwrap();
        for accept in [None, Some("*/*"), Some("application/json")] {
            let response = send(&app, "application/json", accept, bytes.clone()).await;
            assert_eq!(
                response.into_body().collect().await.unwrap().to_bytes(),
                bytes
            );
        }
        let original = b"{\"artwork\":null,\"expected_work_revision\":7,\"operation_id\":\"00000000-0000-4000-8000-000000000009\"}".to_vec();
        let response = receipt(
            WireFormat::Json,
            ArtworkReceipt {
                value: request(),
                json_bytes: original.clone(),
            },
            StatusCode::OK,
        );
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            original
        );
    }
    #[tokio::test]
    async fn put_admission_and_equal_replay_are_200_in_both_metadata_formats() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/contracts/fixtures/artwork/v1/goldens.json"
        )))
        .unwrap();
        let asset = corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["message"] == "ImageAssetDocument")
            .unwrap()["value"]
            .clone();
        let asset: dto::ImageAssetDocument = serde_json::from_value(asset).unwrap();
        let original = serde_json::to_vec(&asset).unwrap();
        for format in [WireFormat::Json, WireFormat::Protobuf] {
            for _ in 0..2 {
                let response = admitted_image_response(
                    format,
                    ArtworkReceipt {
                        value: asset.clone(),
                        json_bytes: original.clone(),
                    },
                );
                assert_eq!(response.status(), StatusCode::OK);
                let bytes = response.into_body().collect().await.unwrap().to_bytes();
                assert_eq!(
                    dto::wire::decode_artwork::<dto::ImageAssetDocument>(format, &bytes).unwrap(),
                    asset
                );
                if format == WireFormat::Json {
                    assert_eq!(bytes, original);
                }
            }
        }
    }
    #[tokio::test]
    async fn hostile_metadata_refuses_before_handler_and_always_uses_json_envelope() {
        let (app, calls) = probe();
        let valid = serde_json::to_vec(&request()).unwrap();
        let mut proto_trailing =
            dto::wire::encode_artwork(WireFormat::Protobuf, &request()).unwrap();
        proto_trailing.push(0);
        let cases = [
            (
                "application/msgpack",
                None,
                valid.clone(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
            (
                "application/protobuf;version=2",
                None,
                valid.clone(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
            (
                "application/json",
                Some("application/msgpack"),
                valid.clone(),
                StatusCode::NOT_ACCEPTABLE,
            ),
            (
                "application/json",
                Some("application/protobuf;version=2,application/json;q=0,*/*"),
                valid.clone(),
                StatusCode::NOT_ACCEPTABLE,
            ),
            (
                "text/plain",
                Some("text/html"),
                valid.clone(),
                StatusCode::NOT_ACCEPTABLE,
            ),
            (
                "application/json",
                None,
                b"{\"operation_id\":\"x\",\"expected_work_revision\":7}".to_vec(),
                StatusCode::BAD_REQUEST,
            ),
            (
                "application/json",
                None,
                b"{\"artwork\":null,\"artwork\":null}".to_vec(),
                StatusCode::BAD_REQUEST,
            ),
            (
                "application/json",
                None,
                b"{} {}".to_vec(),
                StatusCode::BAD_REQUEST,
            ),
            (
                "application/protobuf",
                None,
                proto_trailing,
                StatusCode::BAD_REQUEST,
            ),
            (
                "application/json",
                None,
                vec![b' '; METADATA_LIMIT + 1],
                StatusCode::PAYLOAD_TOO_LARGE,
            ),
        ];
        for (ct, accept, bytes, expected) in cases {
            let response = send(&app, ct, accept, bytes).await;
            assert_eq!(response.status(), expected, "{ct} {accept:?}");
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(
                response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                "nosniff"
            );
            assert_eq!(response.headers()[header::VARY], "accept");
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            assert_eq!(
                serde_json::from_slice::<dto::ApiError>(&bytes)
                    .unwrap()
                    .code,
                dto::ErrorCode::InvalidRequest
            );
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn encoded_metadata_is_refused_before_handler() {
        let (app, calls) = probe();
        let response = app
            .oneshot(
                HttpRequest::post("/probe")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::CONTENT_ENCODING, "gzip")
                    .body(Body::from(serde_json::to_vec(&request()).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn binary_route_limit_overrides_metadata_limit_without_unbounded_buffering() {
        async fn bytes(
            body: Result<Bytes, axum::extract::rejection::BytesRejection>,
        ) -> StatusCode {
            match body {
                Ok(_) => StatusCode::OK,
                Err(e) => e.status(),
            }
        }
        let app = Router::new()
            .route(
                "/image",
                put(bytes).layer(DefaultBodyLimit::max(IMAGE_LIMIT)),
            )
            .layer(DefaultBodyLimit::max(METADATA_LIMIT));
        for (length, status) in [
            (METADATA_LIMIT + 1, StatusCode::OK),
            (IMAGE_LIMIT, StatusCode::OK),
            (IMAGE_LIMIT + 1, StatusCode::PAYLOAD_TOO_LARGE),
        ] {
            let response = app
                .clone()
                .oneshot(
                    HttpRequest::put("/image")
                        .body(Body::from(vec![0; length]))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status);
        }
    }
}
