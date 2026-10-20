//! Native code-response verification groundwork. This is NOT resource admission.
//! No raw/native ID token can construct Library's authenticated witness through this API.
pub use crate::platform::oidc::config::{
    THEATRE_NATIVE_CLIENT_ID as NATIVE_CLIENT_ID, THEATRE_NATIVE_REDIRECT as NATIVE_REDIRECT,
};
use crate::platform::{
    identity::domain::ExternalIdentityKey,
    oidc::core::{verify_for_client, AuthError, JwkSet, VerifiedIdentity},
};

/// Verified OIDC identity for a pending native exchange; not a standing access credential.
pub struct VerifiedNativeCodeResponse(VerifiedIdentity);
impl VerifiedNativeCodeResponse {
    pub fn external_identity(&self) -> &ExternalIdentityKey {
        self.0.key()
    }
    pub fn expires_at(&self) -> u64 {
        self.0.expires()
    }
}
/// Consumer seam for the native token-exchange successor. JWKS/clock/nonce are trusted shell
/// inputs; keys are bounded and actual ES256 is verified. No registry or session is mutated.
pub fn verify_native_code_response(
    token: &str,
    jwks_json: &[u8],
    issuer: &str,
    expected_nonce: &str,
    now: u64,
) -> Result<VerifiedNativeCodeResponse, AuthError> {
    let url = reqwest::Url::parse(issuer).map_err(|_| AuthError::Configuration)?;
    if url.scheme() != "https"
        || url.path() != format!("/oauth2/openid/{NATIVE_CLIENT_ID}")
        || url.as_str() != issuer
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || jwks_json.len() > 65_536
    {
        return Err(AuthError::Configuration);
    }
    let keys: JwkSet = serde_json::from_slice(jwks_json).map_err(|_| AuthError::InvalidKey)?;
    if keys.keys.is_empty() || keys.keys.len() > 16 {
        return Err(AuthError::InvalidKey);
    }
    verify_for_client(token, &keys, issuer, NATIVE_CLIENT_ID, expected_nonce, now)
        .map(VerifiedNativeCodeResponse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::oidc::mock::Signer;
    use serde_json::json;
    #[test]
    fn actual_native_signature_nonce_issuer_client_and_expiry_are_verified_without_minting_authority(
    ) {
        let signer = Signer::new("native-key");
        let other = Signer::new("native-key");
        let issuer = "https://idm.example/oauth2/openid/cantos-theatre-native";
        let keys = serde_json::to_vec(&json!({"keys":[signer.jwk()]})).unwrap();
        let claims = json!({"iss":issuer,"aud":NATIVE_CLIENT_ID,"sub":"nguyễn-minh","nonce":"fresh-native-nonce","iat":100,"exp":300});
        let proof = verify_native_code_response(
            &signer.sign(claims.clone()),
            &keys,
            issuer,
            "fresh-native-nonce",
            100,
        )
        .unwrap();
        assert_eq!(proof.external_identity().subject(), "nguyễn-minh");
        for (field, value, error) in [
            ("aud", json!("cantos-theatre-web-bff"), AuthError::Audience),
            ("azp", json!("cantos-studio-web-bff"), AuthError::Audience),
            ("iss", json!("https://other.example"), AuthError::Issuer),
            ("nonce", json!("old"), AuthError::Nonce),
            ("exp", json!(100), AuthError::Expired),
        ] {
            let mut changed = claims.clone();
            changed[field] = value;
            assert_eq!(
                verify_native_code_response(
                    &signer.sign(changed),
                    &keys,
                    issuer,
                    "fresh-native-nonce",
                    100
                )
                .err(),
                Some(error)
            );
        }
        assert_eq!(
            verify_native_code_response(
                &other.sign(claims),
                &keys,
                issuer,
                "fresh-native-nonce",
                100
            )
            .err(),
            Some(AuthError::Signature)
        );
    }
}
