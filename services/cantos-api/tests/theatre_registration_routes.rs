//! The actual router reports registration readiness without dialing PostgreSQL or an issuer.
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use cantos_api::{
    interfaces::http::{router, AppState},
    platform::{
        postgres::{local_config, Store},
        theatre_identity::{NativeIdentity, TheatreIdentity},
    },
};
use cantos_contracts::theatre_identity::{
    Configuration, LoginMethod, Registration, RegistrationUnavailableReason,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

const ORIGIN: &str = "http://127.0.0.1:8080";

fn store() -> Store {
    Store::new(local_config("postgresql://cantos_app@127.0.0.1:9/cantos").unwrap()).unwrap()
}

fn app(store: Store) -> Router {
    router(
        AppState {
            store,
            web_origin: ORIGIN.into(),
        },
        "/nonexistent",
    )
}

fn web_store() -> Store {
    store().with_theatre_identity(
        TheatreIdentity::new(
            "https://idm.example/oauth2/openid/cantos-theatre-web-bff",
            ORIGIN,
            "isolated-fixture-secret".into(),
        )
        .unwrap(),
    )
}

async fn configuration(app: Router) -> Configuration {
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/theatre/auth/configuration")
                // Cookies and caller-selected URLs cannot turn metadata into account authority.
                .header(
                    header::COOKIE,
                    "cantos_session=studio; cantos_theatre=untrusted",
                )
                .header(header::AUTHORIZATION, "Bearer untrusted")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert!(!response.headers().contains_key(header::LOCATION));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn absent_web_identity_reports_unconfigured_without_account_or_provider_access() {
    assert_eq!(
        configuration(app(store())).await,
        Configuration {
            login: LoginMethod::Unconfigured,
            registration: Registration::Unavailable {
                reason: RegistrationUnavailableReason::IdentityUnconfigured,
            },
        }
    );
    let native_only = store().with_theatre_native(
        NativeIdentity::new("https://idm.example/oauth2/openid/cantos-theatre-native").unwrap(),
    );
    assert_eq!(
        configuration(app(native_only)).await.login,
        LoginMethod::Unconfigured
    );
}

#[tokio::test]
async fn oidc_login_configuration_does_not_imply_provider_self_registration() {
    assert_eq!(
        configuration(app(web_store())).await,
        Configuration {
            login: LoginMethod::Oidc,
            registration: Registration::Unavailable {
                reason: RegistrationUnavailableReason::ProviderUnsupported,
            },
        }
    );
}

#[tokio::test]
async fn unsupported_registration_has_no_password_create_or_cross_surface_route() {
    for configured in [false, true] {
        let app = app(if configured { web_store() } else { store() });
        for path in [
            "/api/v1/theatre/auth/register",
            "/api/v1/theatre/auth/signup",
            "/api/v1/theatre/register",
            "/api/v1/admin/auth/configuration",
            "/api/v1/auth/configuration",
        ] {
            for method in ["GET", "POST"] {
                let response = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method(method)
                            .uri(path)
                            .header(header::ORIGIN, ORIGIN)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(
                                r#"{"password":"fixture","role":"studio_admin"}"#,
                            ))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
                assert!(!response.headers().contains_key(header::SET_COOKIE));
            }
        }
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/theatre/auth/configuration")
                    .header(header::ORIGIN, ORIGIN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
}
