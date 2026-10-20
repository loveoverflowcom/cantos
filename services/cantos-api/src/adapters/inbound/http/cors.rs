//! Opt-in, exact-origin browser transport for one surface. Authorization remains downstream.
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use cantos_contracts::web_api::{ApiOrigin, WebSurface};

#[derive(Clone)]
struct BrowserCors {
    origin: HeaderValue,
    surface: WebSurface,
}

pub fn with_browser_cors(
    app: Router,
    web_origin: &str,
    surface: WebSurface,
) -> Result<Router, &'static str> {
    ApiOrigin::parse(web_origin).map_err(|_| "invalid browser origin")?;
    let origin = HeaderValue::from_str(web_origin).map_err(|_| "invalid browser origin")?;
    Ok(app.layer(middleware::from_fn_with_state(
        BrowserCors { origin, surface },
        cors,
    )))
}

fn requested_headers(headers: &HeaderMap, surface: WebSurface) -> bool {
    let own = match surface {
        WebSurface::Studio => ["x-cantos-studio-session", "x-cantos-studio-stale-only"],
        WebSurface::Theatre => ["x-cantos-theatre-session", "x-cantos-theatre-stale-only"],
    };
    headers
        .get_all(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .iter()
        .all(|value| {
            value.to_str().is_ok_and(|value| {
                value.split(',').all(|name| {
                    let name = name.trim().to_ascii_lowercase();
                    matches!(
                        name.as_str(),
                        "accept" | "content-type" | "authorization" | "range"
                    ) || own.contains(&name.as_str())
                })
            })
        })
}

fn supported_method(value: &str) -> bool {
    matches!(value, "GET" | "HEAD" | "POST" | "PUT" | "DELETE")
}

fn refusal() -> Response {
    let mut response = super::failure(
        cantos_contracts::ErrorCode::Forbidden,
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
    let base = policy.surface.base();
    let path = request.uri().path();
    if headers.get_all(header::ORIGIN).iter().count() != 1
        || headers.get(header::ORIGIN) != Some(&policy.origin)
        || !(path == base || path.starts_with(&format!("{base}/")))
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
                .is_some_and(supported_method)
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
            WebSurface::Studio => "accept, content-type, authorization, range, x-cantos-studio-session, x-cantos-studio-stale-only",
            WebSurface::Theatre => "accept, content-type, authorization, range, x-cantos-theatre-session, x-cantos-theatre-stale-only",
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
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, policy.origin);
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        HeaderValue::from_static("true"),
    );
    let exposed = match policy.surface {
        WebSurface::Studio => {
            "x-cantos-studio-session, etag, content-range, accept-ranges, content-length, cantos-provider-revocation"
        }
        WebSurface::Theatre => {
            "x-cantos-theatre-session, etag, content-range, accept-ranges, content-length, cantos-provider-revocation"
        }
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
