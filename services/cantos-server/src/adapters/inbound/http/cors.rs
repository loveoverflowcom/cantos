//! Opt-in, exact-origin browser transport for one surface. Authorization remains downstream.
use super::{policy::Surface, AppState};
use crate::{contract::routes, platform::host::is_browser_origin};
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};

/// The two browser surfaces this layer can admit; Admin browser transport is route-scoped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebSurface {
    Studio,
    Theatre,
}

#[derive(Clone)]
struct BrowserCors {
    origin: Option<HeaderValue>,
    surface: Surface,
    route_scoped: bool,
}

pub fn with_browser_cors(
    app: Router,
    web_origin: &str,
    surface: WebSurface,
) -> Result<Router, &'static str> {
    if !is_browser_origin(web_origin) {
        return Err("invalid browser origin");
    }
    let origin = HeaderValue::from_str(web_origin).map_err(|_| "invalid browser origin")?;
    Ok(app.layer(middleware::from_fn_with_state(
        BrowserCors {
            origin: Some(origin),
            surface: match surface {
                WebSurface::Studio => Surface::Studio,
                WebSurface::Theatre => Surface::Theatre,
            },
            route_scoped: false,
        },
        cors,
    )))
}

/// Applied only to a matched route group, never to the merged host. Missing Admin origin
/// disables browser admission without changing its dedicated credential verifier.
pub(super) fn boundary(
    app: Router<AppState>,
    origin: Option<&str>,
    surface: Surface,
) -> Router<AppState> {
    app.layer(middleware::from_fn_with_state(
        BrowserCors {
            origin: origin.and_then(|origin| HeaderValue::from_str(origin).ok()),
            surface,
            route_scoped: true,
        },
        cors,
    ))
}

fn requested_headers(headers: &HeaderMap, surface: Surface) -> bool {
    let own = match surface {
        Surface::Studio => &["x-cantos-studio-session", "x-cantos-studio-stale-only"][..],
        Surface::Theatre => &["x-cantos-theatre-session", "x-cantos-theatre-stale-only"][..],
        Surface::Admin => &[][..],
    };
    headers
        .get_all(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .iter()
        .all(|value| {
            value.to_str().is_ok_and(|value| {
                value.split(',').all(|name| {
                    let name = name.trim().to_ascii_lowercase();
                    matches!(name.as_str(), "accept" | "content-type" | "authorization")
                        || name == "range" && !matches!(surface, Surface::Admin)
                        || own.contains(&name.as_str())
                })
            })
        })
}

fn supported_method(value: &str, surface: Surface) -> bool {
    match surface {
        Surface::Studio | Surface::Theatre => {
            matches!(value, "GET" | "HEAD" | "POST" | "PUT" | "DELETE")
        }
        Surface::Admin => matches!(value, "GET" | "HEAD" | "POST"),
    }
}

fn refusal() -> Response {
    let mut response = super::errors::failure(
        crate::contract::studio::ErrorCode::Forbidden,
        StatusCode::FORBIDDEN,
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .append(header::VARY, HeaderValue::from_static("Origin"));
    response
}

async fn cors(State(policy): State<BrowserCors>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    if !headers.contains_key(header::ORIGIN) {
        return next.run(request).await;
    }
    let base = match policy.surface {
        Surface::Studio => routes::STUDIO_BASE,
        Surface::Theatre => routes::THEATRE_BASE,
        Surface::Admin => "/api/v1/admin",
    };
    let path = request.uri().path();
    if headers.get_all(header::ORIGIN).iter().count() != 1
        || policy.origin.is_none()
        || headers.get(header::ORIGIN) != policy.origin.as_ref()
        || !policy.route_scoped && !(path == base || path.starts_with(&format!("{base}/")))
    {
        return refusal();
    }
    let mut response = if request.method() == Method::OPTIONS {
        let method = headers.get(header::ACCESS_CONTROL_REQUEST_METHOD);
        if headers
            .get_all(header::ACCESS_CONTROL_REQUEST_METHOD)
            .iter()
            .count()
            != 1
            || !method
                .and_then(|v| v.to_str().ok())
                .is_some_and(|method| supported_method(method, policy.surface))
            || !requested_headers(headers, policy.surface)
        {
            return refusal();
        }
        let mut response = StatusCode::NO_CONTENT.into_response();
        if let Some(method) = method {
            response
                .headers_mut()
                .insert(header::ACCESS_CONTROL_ALLOW_METHODS, method.clone());
        }
        let allowed = match policy.surface {
            Surface::Studio => "accept, content-type, authorization, range, x-cantos-studio-session, x-cantos-studio-stale-only",
            Surface::Theatre => "accept, content-type, authorization, range, x-cantos-theatre-session, x-cantos-theatre-stale-only",
            Surface::Admin => "accept, content-type, authorization",
        };
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static(allowed),
        );
        response
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    if let Some(origin) = policy.origin {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        HeaderValue::from_static("true"),
    );
    // `retry-after` is not CORS-safelisted: without it a Web 429 hold falls back to the client's
    // conservative default instead of the wait the API asked for.
    let exposed = match policy.surface {
        Surface::Studio => {
            "x-cantos-studio-session, retry-after, etag, content-range, accept-ranges, content-length, cantos-provider-revocation"
        }
        Surface::Theatre => {
            "x-cantos-theatre-session, retry-after, etag, content-range, accept-ranges, content-length, cantos-provider-revocation"
        }
        Surface::Admin => "",
    };
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static(exposed),
    );
    headers.append(header::VARY, HeaderValue::from_static("Origin"));
    // The audio transport owns its private response policy; CORS only supplies a default.
    headers
        .entry(header::CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get};
    use tower::ServiceExt;
    fn app() -> Router {
        with_browser_cors(
            Router::new().route(
                "/api/v1/studio/session",
                get(|| async { StatusCode::UNAUTHORIZED }),
            ),
            "https://cantoss-studio.web.app",
            WebSurface::Studio,
        )
        .unwrap()
    }
    #[tokio::test]
    async fn exact_surface_origin_can_read_an_authentication_refusal_without_bypassing_it() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/session")
                    .header(header::ORIGIN, "https://cantoss-studio.web.app")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            "https://cantoss-studio.web.app"
        );
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_CREDENTIALS],
            "true"
        );
        assert_eq!(response.headers()[header::VARY], "Origin");
    }
    #[tokio::test]
    async fn only_declared_methods_headers_and_the_configured_surface_pass_preflight() {
        for (path, origin, method, headers, status) in [
            (
                "/api/v1/studio/session",
                "https://cantoss-studio.web.app",
                "DELETE",
                "x-cantos-studio-session",
                204,
            ),
            (
                "/api/v1/studio/session",
                "https://cantoss-theatre.web.app",
                "DELETE",
                "x-cantos-studio-session",
                403,
            ),
            (
                "/api/v1/theatre/session",
                "https://cantoss-studio.web.app",
                "DELETE",
                "x-cantos-studio-session",
                403,
            ),
            (
                "/api/v1/studio/session",
                "null",
                "DELETE",
                "x-cantos-studio-session",
                403,
            ),
            (
                "/api/v1/studio/session",
                "https://cantoss-studio.web.app.evil.example",
                "DELETE",
                "x-cantos-studio-session",
                403,
            ),
            (
                "/api/v1/studio/session",
                "https://cantoss-studio.web.app",
                "PATCH",
                "x-cantos-studio-session",
                403,
            ),
            (
                "/api/v1/studio/session",
                "https://cantoss-studio.web.app",
                "DELETE",
                "x-cantos-theatre-session",
                403,
            ),
            (
                "/api/v1/studio/session",
                "https://cantoss-studio.web.app",
                "DELETE",
                "x-unreviewed",
                403,
            ),
        ] {
            let response = app()
                .oneshot(
                    Request::builder()
                        .method(Method::OPTIONS)
                        .uri(path)
                        .header(header::ORIGIN, origin)
                        .header(header::ACCESS_CONTROL_REQUEST_METHOD, method)
                        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, headers)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status().as_u16(),
                status,
                "{path} {origin} {method} {headers}"
            );
            assert_eq!(
                response
                    .headers()
                    .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
                status == 204
            );
        }
    }
    #[tokio::test]
    async fn duplicate_origins_are_refused_and_non_browser_requests_gain_no_cors() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/session")
                    .header(header::ORIGIN, "https://cantoss-studio.web.app")
                    .header(header::ORIGIN, "https://cantoss-studio.web.app")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    }
    #[tokio::test]
    async fn each_web_surface_exposes_its_own_fence_and_retry_after_to_its_origin() {
        for (surface, base, origin, own, other) in [
            (
                WebSurface::Studio,
                "/api/v1/studio/identity/login",
                "https://studio.example.org",
                "x-cantos-studio-session",
                "x-cantos-theatre-session",
            ),
            (
                WebSurface::Theatre,
                "/api/v1/theatre/identity/login",
                "https://listen.example.org",
                "x-cantos-theatre-session",
                "x-cantos-studio-session",
            ),
        ] {
            let app = Router::new().route(
                base,
                get(|| async { (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "12")]) }),
            );
            let response = with_browser_cors(app, origin, surface)
                .unwrap()
                .oneshot(
                    Request::builder()
                        .uri(base)
                        .header(header::ORIGIN, origin)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.headers()[header::RETRY_AFTER], "12");
            let exposed: Vec<_> = response.headers()[header::ACCESS_CONTROL_EXPOSE_HEADERS]
                .to_str()
                .unwrap()
                .split(',')
                .map(str::trim)
                .collect();
            assert!(exposed.contains(&"retry-after"), "{base}");
            assert!(exposed.contains(&own), "{base}");
            assert!(!exposed.contains(&other), "{base}");
        }
    }
    #[tokio::test]
    async fn cors_preserves_transport_owned_cache_policy_and_its_own_surface_fence() {
        let app = Router::new().route(
            "/api/v1/theatre/works",
            get(|| async { ([(header::CACHE_CONTROL, "private, no-cache")], "response") }),
        );
        let response = with_browser_cors(app, "https://listen.example.org", WebSurface::Theatre)
            .unwrap()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/theatre/works")
                    .header(header::ORIGIN, "https://listen.example.org")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-cache"
        );
        assert!(!response.headers()[header::ACCESS_CONTROL_EXPOSE_HEADERS]
            .to_str()
            .unwrap()
            .contains("studio"));
    }
}
