use super::core::{pkce_challenge, session_expiry, verify, JwkSet};
use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine};
use ring::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
use serde_json::{json, Value};

struct Signer {
    key: EcdsaKeyPair,
    kid: &'static str,
}
impl Signer {
    fn new(kid: &'static str) -> Self {
        let random = SystemRandom::new();
        let pkcs = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &random).unwrap();
        Self {
            key: EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs.as_ref(), &random)
                .unwrap(),
            kid,
        }
    }
    fn jwk(&self) -> Value {
        let key = self.key.public_key().as_ref();
        json!({"kid":self.kid,"kty":"EC","crv":"P-256","alg":"ES256","use":"sig","key_ops":["verify"],"x":B64.encode(&key[1..33]),"y":B64.encode(&key[33..65])})
    }
    fn token(&self, claims: Value, header: Value) -> String {
        let message = format!(
            "{}.{}",
            B64.encode(serde_json::to_vec(&header).unwrap()),
            B64.encode(serde_json::to_vec(&claims).unwrap())
        );
        let signature = self
            .key
            .sign(&SystemRandom::new(), message.as_bytes())
            .unwrap();
        format!("{message}.{}", B64.encode(signature.as_ref()))
    }
    fn sign(&self, claims: Value) -> String {
        self.token(claims, json!({"alg":"ES256","kid":self.kid}))
    }
}
fn claims() -> Value {
    json!({"iss":"https://idm.example/oauth2/openid/cantos-studio-web-bff","aud":CLIENT_ID,"sub":"đặng-minh","nonce":"nonce","iat":100,"exp":400,"nbf":100})
}
fn keys(signer: &Signer) -> JwkSet {
    serde_json::from_value(json!({"keys":[signer.jwk()]})).unwrap()
}
fn failure(token: &str, keys: &JwkSet, expected: AuthError, now: u64) {
    assert_eq!(
        verify(token, keys, claims()["iss"].as_str().unwrap(), "nonce", now).err(),
        Some(expected)
    );
}

#[test]
fn signed_identity_admission_partitions_use_exact_errors_and_injected_clock() {
    let signer = Signer::new("one");
    let keys = keys(&signer);
    let proof = verify(
        &signer.sign(claims()),
        &keys,
        claims()["iss"].as_str().unwrap(),
        "nonce",
        100,
    )
    .unwrap();
    assert_eq!(proof.key().subject(), "đặng-minh");
    for (field, value, error) in [
        ("iss", json!("https://other"), AuthError::Issuer),
        ("aud", json!("cantos-theatre"), AuthError::Audience),
        ("azp", json!("admin"), AuthError::Audience),
        ("nonce", json!("wrong"), AuthError::Nonce),
        ("sub", json!(""), AuthError::Subject),
        ("exp", json!(100), AuthError::Expired),
        ("iat", json!(101), AuthError::NotYetValid),
        ("nbf", json!(101), AuthError::NotYetValid),
    ] {
        let mut value_claims = claims();
        value_claims[field] = value;
        failure(&signer.sign(value_claims), &keys, error, 100);
    }
    failure(&signer.sign(claims()), &keys, AuthError::Expired, 400);
    let mut value = claims();
    value["exp"] = json!(9999);
    failure(&signer.sign(value), &keys, AuthError::Expired, 400);
    for field in ["iss", "sub", "aud", "nonce", "iat", "exp"] {
        let mut value = claims();
        value.as_object_mut().unwrap().remove(field);
        failure(&signer.sign(value), &keys, AuthError::InvalidToken, 100);
    }
    let mut value = claims();
    value["aud"] = json!([CLIENT_ID]);
    failure(&signer.sign(value), &keys, AuthError::InvalidToken, 100);
}

#[test]
fn actual_crypto_rejects_alg_kid_key_signature_and_duplicate_jwks() {
    let signer = Signer::new("one");
    let wrong = Signer::new("one");
    let keys = keys(&signer);
    failure(&wrong.sign(claims()), &keys, AuthError::Signature, 100);
    failure(
        &signer.token(claims(), json!({"alg":"HS256","kid":"one"})),
        &keys,
        AuthError::Algorithm,
        100,
    );
    failure(
        &signer.token(claims(), json!({"alg":"none","kid":"one"})),
        &keys,
        AuthError::Algorithm,
        100,
    );
    failure(
        &signer.token(claims(), json!({"alg":"ES256","kid":"missing"})),
        &keys,
        AuthError::UnknownKey,
        100,
    );
    failure(
        &signer.token(claims(), json!({"alg":"ES256","kid":"one","crit":["b64"]})),
        &keys,
        AuthError::Algorithm,
        100,
    );
    let duplicate: JwkSet =
        serde_json::from_value(json!({"keys":[signer.jwk(),signer.jwk()]})).unwrap();
    failure(
        &signer.sign(claims()),
        &duplicate,
        AuthError::InvalidKey,
        100,
    );
    for (field, value) in [
        ("alg", json!("RS256")),
        ("use", json!("enc")),
        ("key_ops", json!(["sign"])),
        ("crv", json!("P-384")),
        ("x", json!("AA")),
    ] {
        let mut jwk = signer.jwk();
        jwk[field] = value;
        let bad = serde_json::from_value(json!({"keys":[jwk]})).unwrap();
        failure(&signer.sign(claims()), &bad, AuthError::InvalidKey, 100);
    }
    failure("x.y.z", &keys, AuthError::InvalidToken, 100);
}

#[test]
fn pkce_rfc7636_vector_and_expiry_boundaries() {
    assert_eq!(
        pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    for now in 0..20 {
        for proof in 0..20 {
            for access in 0..20 {
                let result = session_expiry(now, proof, access);
                if proof <= now || access <= now {
                    assert_eq!(result, Err(AuthError::Expired));
                } else {
                    assert_eq!(result, Ok(proof.min(access)));
                }
            }
        }
    }
}

#[test]
fn config_and_cookie_policy_cannot_enable_remote_http_or_cross_origin_metadata() {
    for issuer in [
        "http://idm.example/oauth2/openid/cantos-studio-web-bff",
        "https://user@idm.example/oauth2/openid/cantos-studio-web-bff",
        "https://idm.example/oauth2/openid/cantos-theatre",
        "https://idm.example/oauth2/openid/cantos-studio-web-bff?x=1",
    ] {
        assert_eq!(
            OidcConfig::new(issuer, "https://studio.example", "fixture".into()).err(),
            Some(AuthError::Configuration)
        );
    }
    let config = OidcConfig::new(
        "https://idm.example/oauth2/openid/cantos-studio-web-bff",
        "https://studio.example",
        "fixture".into(),
    )
    .unwrap();
    assert_eq!(
        config.endpoint("https://evil.example/keys").err(),
        Some(AuthError::InvalidMetadata)
    );
    let oidc = StudioOidc::new(config).unwrap();
    let cookie = oidc.cookie(oidc.session_name(), "opaque", "Strict", 100);
    assert_eq!(cookie,"__Secure-cantos_studio=opaque; HttpOnly; SameSite=Strict; Path=/api/v1/studio; Max-Age=100; Secure");
    assert!(!cookie.contains("Domain="));
    assert_eq!(oidc.cookie(oidc.flow_name(),"opaque","Lax",300),"__Secure-cantos_oidc=opaque; HttpOnly; SameSite=Lax; Path=/api/v1/studio; Max-Age=300; Secure");
}

#[path = "../../../tests/support/oidc_provider.rs"]
mod mock;

#[test]
fn immutable_browser_inventory_is_bounded_duplicate_free_and_has_no_identity_authority() {
    let values = (0..MAX_BROWSER_FLOWS)
        .map(|i| (format!("{i:064x}"), "b".repeat(64)))
        .collect::<Vec<_>>();
    assert_eq!(
        BrowserInventory::new(values.clone()).unwrap().len(),
        MAX_BROWSER_FLOWS
    );
    let mut overflow = values.clone();
    overflow.push(("f".repeat(64), "c".repeat(64)));
    assert!(matches!(
        BrowserInventory::new(overflow),
        Err(AuthError::State)
    ));
    assert!(matches!(
        BrowserInventory::new(vec![values[0].clone(), values[0].clone()]),
        Err(AuthError::State)
    ));
    assert!(matches!(
        BrowserInventory::new(vec![("not-a-state".into(), "b".repeat(64))]),
        Err(AuthError::State)
    ));
    assert!(matches!(
        BrowserInventory::new(vec![("a".repeat(64), "raw-provider-token".into())]),
        Err(AuthError::State)
    ));
}

#[tokio::test]
async fn oauth_basic_form_encoding_handles_special_secret_at_every_endpoint() {
    let mock = mock::MockProvider::start(100).await;
    let secret = "a+b%: đ";
    *mock.data.expected_secret.lock().unwrap() = secret.into();
    let provider = provider::Provider::new(
        OidcConfig::new(&mock.issuer(), "http://127.0.0.1:8080", secret.into()).unwrap(),
    )
    .unwrap();
    let url = provider
        .authorization_url("state", "nonce", "verifier", 100)
        .await
        .unwrap();
    let (_, code) = mock.issue(&url, "one", None);
    let exchange = provider
        .exchange(&code, "verifier", "nonce", || Ok(100))
        .await
        .unwrap();
    assert_eq!(
        provider
            .introspect(&exchange.access_token, "one", || Ok(100))
            .await,
        Ok(300)
    );
    assert_eq!(provider.revoke(&exchange.access_token, 100).await, Ok(()));
    assert_eq!(
        provider
            .introspect(&exchange.access_token, "one", || Ok(100))
            .await,
        Err(AuthError::Inactive)
    );
}

#[tokio::test]
async fn real_http_exchange_pkce_rotation_bounded_refresh_and_provider_disable() {
    use std::sync::atomic::Ordering;
    let mock = mock::MockProvider::start(100).await;
    let config =
        OidcConfig::new(&mock.issuer(), "http://127.0.0.1:8080", "fixture".into()).unwrap();
    let provider = provider::Provider::new(config).unwrap();
    let url = provider
        .authorization_url("state", "nonce", "verifier", 100)
        .await
        .unwrap();
    let (_, code) = mock.issue(&url, "one", None);
    let exchange = provider
        .exchange(&code, "verifier", "nonce", || Ok(100))
        .await
        .unwrap();
    assert_eq!(exchange.proof.key().subject(), "one");
    assert_eq!(
        provider
            .exchange(&code, "verifier", "nonce", || Ok(100))
            .await
            .err(),
        Some(AuthError::ProviderUnavailable)
    );
    mock.rotate("second");
    for time in [100, 109] {
        let (_, code) = mock.issue(&url, "one", None);
        assert_eq!(
            provider
                .exchange(&code, "verifier", "nonce", || Ok(time))
                .await
                .err(),
            Some(AuthError::UnknownKey)
        );
    }
    assert_eq!(mock.data.jwks_calls.load(Ordering::SeqCst), 1);
    let (_, code) = mock.issue(&url, "one", None);
    assert_eq!(
        provider
            .exchange(&code, "verifier", "nonce", || Ok(110))
            .await
            .unwrap()
            .proof
            .key()
            .subject(),
        "one"
    );
    assert_eq!(mock.data.jwks_calls.load(Ordering::SeqCst), 2);
    mock.data.active.store(false, Ordering::SeqCst);
    assert_eq!(
        provider
            .introspect(&exchange.access_token, "one", || Ok(110))
            .await,
        Err(AuthError::Inactive)
    );
    mock.data.active.store(true, Ordering::SeqCst);
    mock.data.bad_jwks.store(true, Ordering::SeqCst);
    assert_eq!(
        provider.verify_metadata(400).await,
        Err(AuthError::ProviderUnavailable)
    );
    assert_eq!(
        provider.verify_metadata(401).await,
        Err(AuthError::ProviderUnavailable)
    );
    assert_eq!(mock.data.jwks_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn real_http_provider_refuses_redirect_pkce_nonce_and_metadata_mismatch() {
    use std::sync::atomic::Ordering;
    let mock = mock::MockProvider::start(100).await;
    let config =
        OidcConfig::new(&mock.issuer(), "http://127.0.0.1:8080", "fixture".into()).unwrap();
    let provider = provider::Provider::new(config).unwrap();
    let url = provider
        .authorization_url("state", "nonce", "verifier", 100)
        .await
        .unwrap();
    for (verifier, nonce, expected) in [
        ("wrong", "nonce", AuthError::ProviderUnavailable),
        ("verifier", "wrong", AuthError::Nonce),
    ] {
        let (_, code) = mock.issue(&url, "one", None);
        assert_eq!(
            provider
                .exchange(&code, verifier, nonce, || Ok(100))
                .await
                .err(),
            Some(expected)
        );
    }
    let bad_url = url.replace("8080", "8081");
    let (_, code) = mock.issue(&bad_url, "one", None);
    assert_eq!(
        provider
            .exchange(&code, "verifier", "nonce", || Ok(100))
            .await
            .err(),
        Some(AuthError::ProviderUnavailable)
    );
    let (_, code) = mock.issue(&url, "one", Some(("iss".into(), json!("https://wrong"))));
    assert_eq!(
        provider
            .exchange(&code, "verifier", "nonce", || Ok(100))
            .await
            .err(),
        Some(AuthError::Issuer)
    );
    mock.data.bad_discovery.store(true, Ordering::SeqCst);
    assert_eq!(
        provider.verify_metadata(400).await,
        Err(AuthError::InvalidMetadata)
    );
}

#[tokio::test]
async fn verifier_reads_clock_after_provider_mints_token_and_after_introspection() {
    let mock = mock::MockProvider::start(101).await;
    let provider = provider::Provider::new(
        OidcConfig::new(&mock.issuer(), "http://127.0.0.1:8080", "fixture".into()).unwrap(),
    )
    .unwrap();
    let url = provider
        .authorization_url("state", "nonce", "verifier", 100)
        .await
        .unwrap();
    let (_, code) = mock.issue(&url, "one", None);
    let calls = std::cell::Cell::new(0);
    let clock = || {
        let call = calls.get();
        calls.set(call + 1);
        Ok(if call == 0 { 100 } else { 101 })
    };
    let exchange = provider
        .exchange(&code, "verifier", "nonce", clock)
        .await
        .unwrap();
    assert_eq!(exchange.proof.key().subject(), "one");
    assert!(calls.get() >= 4);
    let calls = std::cell::Cell::new(0);
    let clock = || {
        let call = calls.get();
        calls.set(call + 1);
        Ok(if call == 0 { 101 } else { 301 })
    };
    assert_eq!(
        provider
            .introspect(&exchange.access_token, "one", clock)
            .await,
        Err(AuthError::Expired)
    );
}
