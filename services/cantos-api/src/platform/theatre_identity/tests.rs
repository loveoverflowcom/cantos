use super::*;
use crate::platform::oidc::{config::THEATRE_WEB_CLIENT_ID, core::verify_for_client};
use serde_json::json;
#[path = "../../../tests/support/oidc_provider.rs"]
mod mock;

#[test]
fn theatre_configuration_rejects_studio_native_and_admin_purposes() {
    for client in [
        "cantos-studio-web-bff",
        "cantos-theatre-native",
        "cantos-admin-web-bff",
    ] {
        assert_eq!(
            TheatreIdentity::new(
                &format!("https://idm.example/oauth2/openid/{client}"),
                "https://cantos.example",
                "fixture".into()
            )
            .err(),
            Some(AuthError::Configuration)
        );
    }
    let oidc = TheatreIdentity::new(
        "https://idm.example/oauth2/openid/cantos-theatre-web-bff",
        "https://cantos.example",
        "fixture".into(),
    )
    .unwrap();
    assert_eq!(
        oidc.config.redirect_uri(),
        "https://cantos.example/api/v1/theatre/auth/callback"
    );
    assert_eq!(oidc.cookie(oidc.session_name(), "opaque", "Strict", 100), "__Secure-cantos_theatre=opaque; HttpOnly; SameSite=Strict; Path=/api/v1/theatre; Max-Age=100; Secure");
}

#[tokio::test]
async fn theatre_signed_http_boundary_never_accepts_studio_proof_or_scope_escalation() {
    let mock = mock::MockProvider::start_for(100, THEATRE_WEB_CLIENT_ID).await;
    let oidc =
        TheatreIdentity::new(&mock.issuer(), "http://127.0.0.1:8080", "fixture".into()).unwrap();
    let url = oidc
        .provider
        .authorization_url("state", "nonce", "verifier", 100)
        .await
        .unwrap();
    let (_, code) = mock.issue(&url, "nguyễn-minh", None);
    let exchange = oidc
        .provider
        .exchange(&code, "verifier", "nonce", || Ok(100))
        .await
        .unwrap();
    assert_eq!(exchange.proof.key().subject(), "nguyễn-minh");
    for (field, value, expected) in [
        ("aud", json!("cantos-studio-web-bff"), AuthError::Audience),
        ("azp", json!("cantos-admin-web-bff"), AuthError::Audience),
        ("nonce", json!("old-flow"), AuthError::Nonce),
        ("exp", json!(100), AuthError::Expired),
    ] {
        let (_, code) = mock.issue(&url, "nguyễn-minh", Some((field.into(), value)));
        assert_eq!(
            oidc.provider
                .exchange(&code, "verifier", "nonce", || Ok(100))
                .await
                .err(),
            Some(expected)
        );
    }
    let keys =
        serde_json::from_value(json!({"keys":[mock.data.signer.lock().unwrap().jwk()]})).unwrap();
    let token = mock.data.last_id_token.lock().unwrap().clone().unwrap();
    assert_eq!(
        verify_for_client(
            &token,
            &keys,
            &mock.issuer(),
            "cantos-studio-web-bff",
            "nonce",
            100
        )
        .err(),
        Some(AuthError::Audience)
    );
}
