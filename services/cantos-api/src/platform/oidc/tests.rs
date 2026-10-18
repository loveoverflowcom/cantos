//! Real-signature fixtures exercise the production kernels outside the pure core.
use super::core::{verify_for_client, verify_native_access, AuthError, JwkSet};
use super::mock::Signer;
use serde_json::json;
#[test]
fn resource_witness_requires_actual_access_signature_purpose_client_scope_and_current_time() {
    let signer = Signer::new("native-resource");
    let attacker = Signer::new("native-resource");
    let issuer = "https://idm.example/oauth2/openid/cantos-theatre-native";
    let client = "cantos-theatre-native";
    let keys: JwkSet = serde_json::from_value(json!({"keys":[signer.jwk()]})).unwrap();
    let claims = json!({"iss":issuer,"sub":"nguyễn-minh","aud":client,"client_id":client,"iat":100,"nbf":100,"exp":300,"jti":"unique","scope":"openid"});
    let proof = verify_native_access(
        &signer.sign_access(claims.clone()),
        &keys,
        issuer,
        client,
        100,
    )
    .unwrap();
    assert_eq!(proof.key().subject(), "nguyễn-minh");
    assert_eq!(proof.expires(), 300);
    assert_eq!(
        verify_native_access(&signer.sign(claims.clone()), &keys, issuer, client, 100).err(),
        Some(AuthError::InvalidToken)
    );
    assert_eq!(
        verify_native_access(
            &attacker.sign_access(claims.clone()),
            &keys,
            issuer,
            client,
            100
        )
        .err(),
        Some(AuthError::Signature)
    );
    for (field, value, error) in [
        ("aud", json!("cantos-theatre-web-bff"), AuthError::Audience),
        (
            "client_id",
            json!("cantos-studio-web-bff"),
            AuthError::Audience,
        ),
        ("iss", json!("https://other.example"), AuthError::Issuer),
        ("exp", json!(100), AuthError::Expired),
        ("nbf", json!(101), AuthError::NotYetValid),
        ("scope", json!("profile"), AuthError::InvalidToken),
    ] {
        let mut changed = claims.clone();
        changed[field] = value;
        assert_eq!(
            verify_native_access(&signer.sign_access(changed), &keys, issuer, client, 100).err(),
            Some(error)
        );
    }
    let mut id_claims = claims.clone();
    id_claims["nonce"] = json!("native-nonce");
    assert_eq!(
        verify_for_client(
            &signer.sign_access(id_claims),
            &keys,
            issuer,
            client,
            "native-nonce",
            100
        )
        .err(),
        Some(AuthError::InvalidToken)
    );
}

#[test]
fn signed_auth_time_is_distinct_from_iat_and_future_authentication_is_rejected() {
    let signer = Signer::new("recent-proof");
    let issuer = "https://idm.example/oauth2/openid/cantos-studio-web-bff";
    let client = "cantos-studio-web-bff";
    let keys: JwkSet = serde_json::from_value(json!({"keys":[signer.jwk()]})).unwrap();
    let claims = json!({"iss":issuer,"sub":"alice","aud":client,"iat":100,"exp":400,"nonce":"n"});
    assert_eq!(
        verify_for_client(
            &signer.sign(claims.clone()),
            &keys,
            issuer,
            client,
            "n",
            100
        )
        .unwrap()
        .authenticated_at(),
        None
    );
    let mut recent = claims.clone();
    recent["auth_time"] = json!(99);
    assert_eq!(
        verify_for_client(&signer.sign(recent), &keys, issuer, client, "n", 100)
            .unwrap()
            .authenticated_at(),
        Some(99)
    );
    let mut future = claims;
    future["auth_time"] = json!(101);
    assert_eq!(
        verify_for_client(&signer.sign(future), &keys, issuer, client, "n", 100).err(),
        Some(AuthError::NotYetValid)
    );
}
