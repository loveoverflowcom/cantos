//! Ephemeral signed mock. Actual HTTP + P-256 crypto, not a live Kanidm server.
#![allow(dead_code)]
use axum::{
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD as B64},
    Engine,
};
use ring::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
pub const CLIENT_ID: &str = "cantos-studio-web-bff";

pub struct Signer {
    key: EcdsaKeyPair,
    pub kid: String,
}
impl Signer {
    pub fn new(kid: &str) -> Self {
        let r = SystemRandom::new();
        let p = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &r).unwrap();
        Self {
            key: EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, p.as_ref(), &r)
                .unwrap(),
            kid: kid.into(),
        }
    }
    pub fn jwk(&self) -> Value {
        let key = self.key.public_key().as_ref();
        json!({"kid":self.kid,"kty":"EC","crv":"P-256","alg":"ES256","use":"sig","key_ops":["verify"],"x":B64.encode(&key[1..33]),"y":B64.encode(&key[33..65])})
    }
    pub fn sign(&self, claims: Value) -> String {
        self.sign_typed(claims, None)
    }
    pub fn sign_access(&self, claims: Value) -> String {
        self.sign_typed(claims, Some("at+jwt"))
    }
    fn sign_typed(&self, claims: Value, typ: Option<&str>) -> String {
        let mut header = json!({"kid":self.kid,"alg":"ES256"});
        if let Some(typ) = typ {
            header["typ"] = json!(typ);
        }
        let message = format!(
            "{}.{}",
            B64.encode(serde_json::to_vec(&header).unwrap()),
            B64.encode(serde_json::to_vec(&claims).unwrap())
        );
        let sig = self
            .key
            .sign(&SystemRandom::new(), message.as_bytes())
            .unwrap();
        format!("{message}.{}", B64.encode(sig.as_ref()))
    }
}
struct Code {
    nonce: String,
    challenge: String,
    redirect: String,
    subject: String,
    change: Option<(String, Value)>,
    expires: u64,
}
struct Access {
    subject: String,
    expires: u64,
}
pub struct TokenPause {
    pub entered: tokio::sync::Notify,
    pub release: tokio::sync::Notify,
}
pub struct Data {
    pub origin: String,
    pub client_id: &'static str,
    pub signer: Mutex<Signer>,
    codes: Mutex<HashMap<String, Code>>,
    access: Mutex<HashMap<String, Access>>,
    pub now: std::sync::atomic::AtomicU64,
    pub active: AtomicBool,
    pub revoke_fail: AtomicBool,
    pub revoke_pause: AtomicBool,
    pub revoke_started: tokio::sync::Notify,
    pub revoke_release: tokio::sync::Notify,
    pub token_fail: AtomicBool,
    pub bad_discovery: AtomicBool,
    pub bad_jwks: AtomicBool,
    pub jwks_calls: AtomicUsize,
    pub token_calls: AtomicUsize,
    pub id_override: Mutex<Option<String>>,
    pub last_id_token: Mutex<Option<String>>,
    pub introspection_calls: AtomicUsize,
    pub introspection_fail: AtomicBool,
    pub revoke_calls: AtomicUsize,
    pub expected_secret: Mutex<String>,
    pub token_pauses: Mutex<HashMap<String, Arc<TokenPause>>>,
}
pub struct MockProvider {
    pub data: Arc<Data>,
    task: tokio::task::JoinHandle<()>,
}
impl MockProvider {
    pub async fn start(now: u64) -> Self {
        Self::start_for(now, CLIENT_ID).await
    }
    pub async fn start_for(now: u64, client_id: &'static str) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let data = Arc::new(Data {
            origin,
            client_id,
            signer: Mutex::new(Signer::new("first")),
            codes: Mutex::new(HashMap::new()),
            access: Mutex::new(HashMap::new()),
            now: std::sync::atomic::AtomicU64::new(now),
            active: AtomicBool::new(true),
            revoke_fail: AtomicBool::new(false),
            revoke_pause: AtomicBool::new(false),
            revoke_started: tokio::sync::Notify::new(),
            revoke_release: tokio::sync::Notify::new(),
            token_fail: AtomicBool::new(false),
            bad_discovery: AtomicBool::new(false),
            bad_jwks: AtomicBool::new(false),
            jwks_calls: AtomicUsize::new(0),
            token_calls: AtomicUsize::new(0),
            id_override: Mutex::new(None),
            last_id_token: Mutex::new(None),
            introspection_calls: AtomicUsize::new(0),
            introspection_fail: AtomicBool::new(false),
            revoke_calls: AtomicUsize::new(0),
            expected_secret: Mutex::new("fixture".into()),
            token_pauses: Mutex::new(HashMap::new()),
        });
        let router = Router::new()
            .route(
                &format!("/oauth2/openid/{client_id}/.well-known/openid-configuration"),
                get(discovery),
            )
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .route("/introspect", post(introspect))
            .route("/revoke", post(revoke))
            .with_state(data.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self { data, task }
    }
    pub fn issuer(&self) -> String {
        format!("{}/oauth2/openid/{}", self.data.origin, self.data.client_id)
    }
    pub fn pause_code(&self, code: &str) -> Arc<TokenPause> {
        let pause = Arc::new(TokenPause {
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        self.data
            .token_pauses
            .lock()
            .unwrap()
            .insert(code.into(), pause.clone());
        pause
    }
    pub fn issue(
        &self,
        url: &str,
        subject: &str,
        change: Option<(String, Value)>,
    ) -> (String, String) {
        let url = reqwest::Url::parse(url).unwrap();
        let pairs: HashMap<_, _> = url
            .query_pairs()
            .map(|(a, b)| (a.into_owned(), b.into_owned()))
            .collect();
        assert_eq!(pairs["client_id"], self.data.client_id);
        assert_eq!(pairs["response_type"], "code");
        assert_eq!(pairs["scope"], "openid");
        assert_eq!(pairs["code_challenge_method"], "S256");
        let code = uuid::Uuid::new_v4().to_string();
        self.data.codes.lock().unwrap().insert(
            code.clone(),
            Code {
                nonce: pairs["nonce"].clone(),
                challenge: pairs["code_challenge"].clone(),
                redirect: pairs["redirect_uri"].clone(),
                subject: subject.into(),
                change,
                expires: self.data.now.load(Ordering::SeqCst) + 60,
            },
        );
        (pairs["state"].clone(), code)
    }
    pub fn expire_code(&self, code: &str) {
        self.data
            .codes
            .lock()
            .unwrap()
            .get_mut(code)
            .unwrap()
            .expires = self.data.now.load(Ordering::SeqCst);
    }
    pub fn pause_code(&self, code: &str) -> Arc<TokenPause> {
        let pause = Arc::new(TokenPause {
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        });
        self.data
            .token_pauses
            .lock()
            .unwrap()
            .insert(code.into(), pause.clone());
        pause
    }
    pub fn rotate(&self, kid: &str) {
        *self.data.signer.lock().unwrap() = Signer::new(kid);
    }
}
impl Drop for MockProvider {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn basic(headers: &HeaderMap, data: &Data) -> bool {
    let Some(encoded) = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Basic "))
    else {
        return false;
    };
    let Ok(decoded) = STANDARD.decode(encoded) else {
        return false;
    };
    let Ok(decoded) = String::from_utf8(decoded) else {
        return false;
    };
    let Some((client, password)) = decoded.split_once(':') else {
        return false;
    };
    // RFC6749 client_secret_basic decodes form encoding after Basic. This oracle exercises
    // a protocol implementation instead of copying the production header encoder.
    let Ok(url) = reqwest::Url::parse(&format!(
        "https://fixture.invalid/?client={client}&secret={password}"
    )) else {
        return false;
    };
    let pairs: HashMap<_, _> = url.query_pairs().collect();
    pairs
        .get("client")
        .is_some_and(|value| value == data.client_id)
        && pairs
            .get("secret")
            .is_some_and(|value| value.as_ref() == data.expected_secret.lock().unwrap().as_str())
}
async fn discovery(State(data): State<Arc<Data>>) -> Json<Value> {
    let o = &data.origin;
    let client_id = data.client_id;
    let authorization_path = if client_id == "cantos-theatre-native" {
        "/ui/oauth2"
    } else {
        "/authorize"
    };
    Json(
        json!({"issuer":if data.bad_discovery.load(Ordering::SeqCst){format!("{o}/wrong")}else{format!("{o}/oauth2/openid/{client_id}")},"authorization_endpoint":format!("{o}{authorization_path}"),"token_endpoint":format!("{o}/token"),"jwks_uri":format!("{o}/jwks"),"introspection_endpoint":format!("{o}/introspect"),"revocation_endpoint":format!("{o}/revoke"),"code_challenge_methods_supported":["S256"],"id_token_signing_alg_values_supported":["ES256"],"token_endpoint_auth_methods_supported":["client_secret_basic"],"introspection_endpoint_auth_methods_supported":["none"],"revocation_endpoint_auth_methods_supported":["none"],"response_types_supported":["code"]}),
    )
}
async fn jwks(State(data): State<Arc<Data>>) -> (StatusCode, Json<Value>) {
    data.jwks_calls.fetch_add(1, Ordering::SeqCst);
    if data.bad_jwks.load(Ordering::SeqCst) {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({})));
    }
    (
        StatusCode::OK,
        Json(json!({"keys":[data.signer.lock().unwrap().jwk()]})),
    )
}
async fn token(
    State(data): State<Arc<Data>>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    data.token_calls.fetch_add(1, Ordering::SeqCst);
    if if data.client_id == "cantos-theatre-native" {
        headers.contains_key("authorization")
            || form.get("client_id").map(String::as_str) != Some(data.client_id)
            || form.contains_key("client_secret")
    } else {
        !basic(&headers, &data)
    } {
        return (StatusCode::UNAUTHORIZED, Json(json!({})));
    }
    let code = data
        .codes
        .lock()
        .unwrap()
        .remove(form.get("code").map(String::as_str).unwrap_or(""));
    let Some(code) = code else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_grant"})),
        );
    };
    if data.token_fail.load(Ordering::SeqCst) {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({})));
    }
    let now = data.now.load(Ordering::SeqCst);
    if code.expires <= now
        || form.get("redirect_uri") != Some(&code.redirect)
        || form.get("grant_type").map(String::as_str) != Some("authorization_code")
        || form
            .get("code_verifier")
            .is_none_or(|v| B64.encode(Sha256::digest(v.as_bytes())) != code.challenge)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_grant"})),
        );
    }
    let pause = {
        data.token_pauses
            .lock()
            .unwrap()
            .remove(form.get("code").map(String::as_str).unwrap_or(""))
    };
    if let Some(pause) = pause {
        pause.entered.notify_one();
        pause.release.notified().await;
    }
    let mut claims = json!({"iss":format!("{}/oauth2/openid/{}",data.origin,data.client_id),"aud":data.client_id,"sub":code.subject,"nonce":code.nonce,"iat":now,"exp":now+200,"nbf":now});
    if let Some((field, value)) = code.change {
        claims[field] = value;
    }
    let access = if data.client_id == "cantos-theatre-native" {
        data.signer.lock().unwrap().sign_access(json!({"iss":format!("{}/oauth2/openid/{}",data.origin,data.client_id),"sub":code.subject,"aud":data.client_id,"client_id":data.client_id,"iat":now,"nbf":now,"exp":now+200,"jti":uuid::Uuid::new_v4().to_string(),"scope":"openid"}))
    } else {
        uuid::Uuid::new_v4().to_string()
    };
    data.access.lock().unwrap().insert(
        access.clone(),
        Access {
            subject: code.subject,
            expires: now + 200,
        },
    );
    let signed = data
        .id_override
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| data.signer.lock().unwrap().sign(claims));
    *data.last_id_token.lock().unwrap() = Some(signed.clone());
    (
        StatusCode::OK,
        Json(json!({"id_token":signed,"access_token":access,"token_type":"Bearer"})),
    )
}
async fn introspect(
    State(data): State<Arc<Data>>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    data.introspection_calls.fetch_add(1, Ordering::SeqCst);
    if data.introspection_fail.load(Ordering::SeqCst) {
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({})));
    }
    if if data.client_id == "cantos-theatre-native" {
        headers.contains_key("authorization") || form.contains_key("client_secret")
    } else {
        !basic(&headers, &data)
    } {
        return (StatusCode::UNAUTHORIZED, Json(json!({})));
    }
    let access = data.access.lock().unwrap();
    let value = access.get(form.get("token").map(String::as_str).unwrap_or(""));
    let Some(value) = value else {
        return (StatusCode::OK, Json(json!({"active":false})));
    };
    (
        StatusCode::OK,
        Json(
            json!({"active":data.active.load(Ordering::SeqCst),"sub":value.subject,"exp":value.expires,"aud":data.client_id,"client_id":data.client_id,"iss":format!("{}/oauth2/openid/{}",data.origin,data.client_id)}),
        ),
    )
}
async fn revoke(
    State(data): State<Arc<Data>>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> StatusCode {
    data.revoke_calls.fetch_add(1, Ordering::SeqCst);
    if if data.client_id == "cantos-theatre-native" {
        headers.contains_key("authorization") || form.contains_key("client_secret")
    } else {
        !basic(&headers, &data)
    } {
        return StatusCode::UNAUTHORIZED;
    }
    if data.revoke_fail.load(Ordering::SeqCst) {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    if data.revoke_pause.load(Ordering::SeqCst) {
        data.revoke_started.notify_one();
        data.revoke_release.notified().await;
    }
    data.access
        .lock()
        .unwrap()
        .remove(form.get("token").map(String::as_str).unwrap_or(""));
    StatusCode::OK
}
