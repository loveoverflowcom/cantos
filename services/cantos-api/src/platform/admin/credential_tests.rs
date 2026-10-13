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
        let k = self.0.public_key().as_ref();
        serde_json::from_value(json!({"keys":[{"kid":"one","kty":"EC","crv":"P-256","alg":"ES256","x":B64.encode(&k[1..33]),"y":B64.encode(&k[33..65])}]})).unwrap()
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
