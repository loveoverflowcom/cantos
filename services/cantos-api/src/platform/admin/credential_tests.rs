use super::{credential, domain::AdminError, CLIENT_ID};
use crate::platform::oidc::core::JwkSet;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use ring::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
use serde_json::{json, Value};
struct Signer(EcdsaKeyPair);
impl Signer {
    fn new() -> Self {
        let r = SystemRandom::new();
        let p = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &r).unwrap();
        Self(EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, p.as_ref(), &r).unwrap())
    }
    fn keys(&self) -> JwkSet {
        serde_json::from_value(self.jwks()).unwrap()
    }
    fn jwks(&self) -> Value {
        let k = self.0.public_key().as_ref();
        json!({"keys":[{"kid":"one","kty":"EC","crv":"P-256","alg":"ES256","x":B64.encode(&k[1..33]),"y":B64.encode(&k[33..65])}]})
    }
    fn sign(&self, claims: Value, header: Value) -> String {
        let m = format!(
            "{}.{}",
            B64.encode(serde_json::to_vec(&header).unwrap()),
            B64.encode(serde_json::to_vec(&claims).unwrap())
        );
        format!(
            "{m}.{}",
            B64.encode(
                self.0
                    .sign(&SystemRandom::new(), m.as_bytes())
                    .unwrap()
                    .as_ref()
            )
        )
    }
}
const ISSUER: &str = "https://idm.example/oauth2/openid/cantos-admin-api";
fn claims() -> Value {
    json!({"iss":ISSUER,"sub":"đặng-minh","aud":CLIENT_ID,"client_id":CLIENT_ID,"iat":100,"exp":400,"cantos_admin_groups":["cantos_identity_operators"]})
}
fn header() -> Value {
    json!({"alg":"ES256","kid":"one","typ":"at+jwt"})
}
#[test]
fn real_signature_and_purpose_are_checked_before_closed_role_mapping() {
    let s = Signer::new();
    let keys = s.keys();
    for (field, value, error) in [
        ("iss", json!("wrong"), AdminError::Credential),
        (
            "aud",
            json!("cantos-studio-web-bff"),
            AdminError::Credential,
        ),
        (
            "client_id",
            json!("cantos-theatre-web-bff"),
            AdminError::Credential,
        ),
        ("exp", json!(100), AdminError::Credential),
        ("iat", json!(101), AdminError::Credential),
        ("nbf", json!(101), AdminError::Credential),
        ("cantos_admin_groups", json!([]), AdminError::MissingRole),
        (
            "cantos_admin_groups",
            json!(["superadmin"]),
            AdminError::UnknownRole,
        ),
    ] {
        let mut c = claims();
        c[field] = value;
        assert_eq!(
            credential::verify(&s.sign(c, header()), &keys, ISSUER, 100).err(),
            Some(error)
        );
    }
    for (field, value) in [
        ("alg", json!("none")),
        ("alg", json!("HS256")),
        ("kid", json!("unknown")),
        ("typ", json!("JWT")),
    ] {
        let mut h = header();
        h[field] = value;
        assert_eq!(
            credential::verify(&s.sign(claims(), h), &keys, ISSUER, 100).err(),
            Some(AdminError::Credential)
        );
    }
    let wrong = Signer::new();
    assert_eq!(
        credential::verify(&wrong.sign(claims(), header()), &keys, ISSUER, 100).err(),
        Some(AdminError::Credential)
    );
    for field in [
        "iss",
        "sub",
        "aud",
        "client_id",
        "iat",
        "exp",
        "cantos_admin_groups",
    ] {
        let mut c = claims();
        c.as_object_mut().unwrap().remove(field);
        assert_eq!(
            credential::verify(&s.sign(c, header()), &keys, ISSUER, 100).err(),
            Some(if field == "cantos_admin_groups" {
                AdminError::MissingRole
            } else {
                AdminError::Credential
            })
        );
    }
}
#[test]
fn current_role_removal_demotion_and_expiry_invalidate_signed_admin_claims() {
    let s = Signer::new();
    let keys = s.keys();
    let token = s.sign(claims(), header());
    assert_eq!(
        credential::verify(&token, &keys, ISSUER, 100)
            .unwrap()
            .observe(&[], 400, 100)
            .err(),
        Some(AdminError::MissingRole)
    );
    assert_eq!(
        credential::verify(&token, &keys, ISSUER, 100)
            .unwrap()
            .observe(&["cantos_system_auditors".into()], 400, 100)
            .err(),
        Some(AdminError::StaleRole)
    );
    assert_eq!(
        credential::verify(&token, &keys, ISSUER, 100)
            .unwrap()
            .observe(&["cantos_identity_operators".into()], 100, 100)
            .err(),
        Some(AdminError::StaleRole)
    );
    assert!(credential::verify(&token, &keys, ISSUER, 100)
        .unwrap()
        .observe(&["cantos_identity_operators".into()], 400, 100)
        .is_ok());
}

#[tokio::test]
async fn admin_introspection_form_encodes_special_secret_before_basic_authentication() {
    use super::provider::{AdminConfig, AdminProvider};
    use axum::{
        extract::Form,
        http::{HeaderMap, StatusCode},
        routing::{get, post},
        Json, Router,
    };
    use base64::engine::general_purpose::STANDARD;
    use std::{collections::HashMap, sync::Arc};

    const SECRET: &str = "a+b%: đ&/?=#";
    let signer = Signer::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let issuer = format!("{origin}/oauth2/openid/{CLIENT_ID}");
    let now = super::now().unwrap();
    let mut signed_claims = claims();
    signed_claims["iss"] = json!(issuer);
    signed_claims["iat"] = json!(now);
    signed_claims["exp"] = json!(now + 200);
    let token = signer.sign(signed_claims, header());
    let expected_token = Arc::new(token.clone());
    let metadata = json!({
        "issuer":issuer,
        "jwks_uri":format!("{origin}/jwks"),
        "introspection_endpoint":format!("{origin}/introspect")
    });
    let keys = signer.jwks();
    let observation = json!({
        "active":true,"iss":issuer,"sub":"đặng-minh","aud":CLIENT_ID,
        "client_id":CLIENT_ID,"exp":now+200,
        "cantos_admin_groups":["cantos_identity_operators"]
    });
    let app = Router::new()
        .route(
            &format!("/oauth2/openid/{CLIENT_ID}/.well-known/openid-configuration"),
            get(move || {
                let metadata = metadata.clone();
                async move { Json(metadata) }
            }),
        )
        .route(
            "/jwks",
            get(move || {
                let keys = keys.clone();
                async move { Json(keys) }
            }),
        )
        .route(
            "/introspect",
            post(
                move |headers: HeaderMap, Form(form): Form<HashMap<String, String>>| {
                    let observation = observation.clone();
                    let expected_token = expected_token.clone();
                    async move {
                        // Decode the protocol at the mock provider boundary; do not copy the encoder.
                        let credentials = headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            .and_then(|value| value.strip_prefix("Basic "))
                            .and_then(|value| STANDARD.decode(value).ok())
                            .and_then(|value| String::from_utf8(value).ok());
                        let accepted = credentials
                            .as_deref()
                            .and_then(|value| value.split_once(':'))
                            .is_some_and(|(client, secret)| {
                                let decode = |value: &str| {
                                    reqwest::Url::parse(&format!(
                                        "https://fixture.invalid/?value={value}"
                                    ))
                                    .ok()
                                    .and_then(|url| {
                                        url.query_pairs()
                                            .next()
                                            .map(|(_, value)| value.into_owned())
                                    })
                                };
                                decode(client).as_deref() == Some(CLIENT_ID)
                                    && decode(secret).as_deref() == Some(SECRET)
                                    && form.get("token").map(String::as_str)
                                        == Some(expected_token.as_str())
                            });
                        if accepted {
                            (StatusCode::OK, Json(observation))
                        } else {
                            (StatusCode::UNAUTHORIZED, Json(json!({})))
                        }
                    }
                },
            ),
        );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let provider = AdminProvider::new(AdminConfig::new(&issuer, SECRET.into()).unwrap()).unwrap();
    let result = provider.authenticate(&token).await;
    server.abort();
    let proof = result
        .expect("compliant OAuth Basic authentication must admit the signed Admin credential");
    assert_eq!(proof.key().issuer(), issuer);
    assert_eq!(proof.key().subject(), "đặng-minh");
    assert!(proof.valid_until() > now);
}
