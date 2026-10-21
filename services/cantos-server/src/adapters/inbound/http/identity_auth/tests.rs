use super::*;

fn state(origin: &str) -> AppState {
    AppState {
        store: crate::application::Store::new("host=127.0.0.1 user=x".parse().unwrap()).unwrap(),
        web_origin: origin.to_owned(),
    }
}

fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.append(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    map
}

fn status(result: Result<(), Rejection>) -> Option<StatusCode> {
    result
        .err()
        .map(|rejection| rejection.into_response().status())
}

#[test]
fn originless_native_mutations_are_exact_and_never_a_prefix_or_another_surface() {
    for path in ["signup", "signup/complete", "login", "refresh", "revoke"] {
        let full = format!("/api/v1/theatre/native/identity/{path}");
        assert!(native_mutation(&Method::POST, &full), "{full}");
        assert!(!native_mutation(&Method::PUT, &full));
        assert!(!native_mutation(&Method::DELETE, &full));
        assert!(!native_mutation(&Method::POST, &format!("{full}/x")));
        assert!(!native_mutation(&Method::POST, &format!("{full}/")));
        assert!(!native_mutation(
            &Method::POST,
            &format!("/api/v1/studio/native/identity/{path}")
        ));
        // The Web twin of each route is never originless.
        assert!(!native_mutation(
            &Method::POST,
            &format!("/api/v1/theatre/identity/{path}")
        ));
    }
    assert!(native_mutation(
        &Method::DELETE,
        "/api/v1/theatre/native/identity/session"
    ));
    assert!(!native_mutation(
        &Method::POST,
        "/api/v1/theatre/native/identity/session"
    ));
    assert!(!native_mutation(
        &Method::DELETE,
        "/api/v1/theatre/identity/session"
    ));
}

#[tokio::test]
async fn web_admission_requires_the_exact_origin_for_mutations_and_refuses_mixed_credentials() {
    const ORIGIN: &str = "https://theatre.example";
    let state = state(ORIGIN);
    let ok = headers(&[("origin", ORIGIN)]);
    assert_eq!(status(web_gate(&state, &ok, true)), None);
    assert_eq!(status(web_gate(&state, &HeaderMap::new(), false)), None);
    assert_eq!(
        status(web_gate(&state, &HeaderMap::new(), true)),
        Some(StatusCode::FORBIDDEN),
        "a mutation without Origin is a CSRF candidate"
    );
    for origin in [
        "https://evil.example",
        "http://theatre.example",
        "https://theatre.example:8443",
        "https://theatre.example/",
        "https://sibling.theatre.example",
        "null",
    ] {
        let wrong = headers(&[("origin", origin)]);
        for mutation in [true, false] {
            assert_eq!(
                status(web_gate(&state, &wrong, mutation)),
                Some(StatusCode::FORBIDDEN),
                "{origin} mutation={mutation}"
            );
        }
    }
    let mut twice = HeaderMap::new();
    twice.append(header::ORIGIN, HeaderValue::from_static(ORIGIN));
    twice.append(header::ORIGIN, HeaderValue::from_static(ORIGIN));
    assert_eq!(
        status(web_gate(&state, &twice, true)),
        Some(StatusCode::FORBIDDEN)
    );
    assert_eq!(
        status(web_gate(
            &state,
            &headers(&[("origin", ORIGIN), ("sec-fetch-site", "cross-site")]),
            true
        )),
        Some(StatusCode::FORBIDDEN)
    );
    assert_eq!(
        status(web_gate(
            &state,
            &headers(&[("origin", ORIGIN), ("sec-fetch-site", "same-site")]),
            true
        )),
        None
    );
    // A cookie request never also carries a Bearer.
    assert_eq!(
        status(web_gate(
            &state,
            &headers(&[("origin", ORIGIN), ("authorization", "Bearer x")]),
            true
        )),
        Some(StatusCode::UNAUTHORIZED)
    );
}

#[test]
fn native_admission_refuses_every_browser_signal_and_exactly_one_bearer_of_its_own_kind() {
    let token = format!("ci1_n_{}", "a".repeat(64));
    let bearer = |value: &str| {
        let mut map = HeaderMap::new();
        map.insert(header::AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        map
    };
    assert_eq!(status(native_gate(&HeaderMap::new(), false)), None);
    assert!(bearer_token(&bearer(&format!("Bearer {token}"))).is_ok());
    for signal in [
        ("origin", "https://app.example"),
        ("cookie", "a=b"),
        ("sec-fetch-site", "none"),
        ("sec-fetch-mode", "cors"),
    ] {
        let map = headers(&[signal]);
        assert_eq!(
            status(native_gate(&map, false)),
            Some(StatusCode::UNAUTHORIZED),
            "{signal:?}"
        );
        assert_eq!(
            status(native_gate(&map, true)),
            Some(StatusCode::UNAUTHORIZED)
        );
    }
    // Anonymous native routes carry no Authorization at all.
    assert_eq!(
        status(native_gate(&bearer("Bearer x"), false)),
        Some(StatusCode::UNAUTHORIZED)
    );
    let mut cookie_and_bearer = bearer(&format!("Bearer {token}"));
    cookie_and_bearer.insert(header::COOKIE, HeaderValue::from_static("a=b"));
    assert!(bearer_token(&cookie_and_bearer).is_err());
    let mut twice = bearer(&format!("Bearer {token}"));
    twice.append(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    assert!(bearer_token(&twice).is_err());
    for wrong in [
        format!("Bearer ci1_t_{}", "a".repeat(64)),
        format!("Bearer ci1_s_{}", "a".repeat(64)),
        format!("Bearer ci1_r_{}", "a".repeat(64)),
        format!("Bearer ca1_n_{}", "a".repeat(64)),
        format!("bearer {token}"),
        token.clone(),
        String::new(),
    ] {
        assert!(bearer_token(&bearer(&wrong)).is_err(), "{wrong}");
    }
}

#[test]
fn failures_project_to_generic_statuses_with_a_bounded_retry_after() {
    for (failure_kind, expected) in [
        (AuthFailure::Unauthenticated, StatusCode::UNAUTHORIZED),
        (AuthFailure::Forbidden, StatusCode::FORBIDDEN),
        (AuthFailure::InvalidRequest, StatusCode::BAD_REQUEST),
        (AuthFailure::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
    ] {
        let response = refusal(failure_kind);
        assert_eq!(response.status(), expected);
        assert!(response.headers().get(header::RETRY_AFTER).is_none());
    }
    let limited = refusal(AuthFailure::Throttled {
        retry_after_seconds: 42,
    });
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.headers()[header::RETRY_AFTER], "42");
}
