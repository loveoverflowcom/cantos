//! Synthetic backend-owned catalog through Studio HTTP, immutable publication gates and real
//! private/publication filesystem adapters. The optional persistent lane uses only an existing
//! app connection/session, with a private write-ahead journal for safe replay; it never creates,
//! migrates, grants, drops or resets a persistent database.
use super::*;
use axum::http::header;
use cantos_server::adapters::inbound::http::server_router;
use cantos_server::contract::theatre::discovery::{
    MetadataUnitPage as UnitPage, MetadataWorkPage as WorkPage, MetadataWorkSummary as WorkSummary,
    PublicationManifest,
};
const THEATRE_BROWSER_ORIGIN: &str = "http://127.0.0.1:8082";
use cantos_server::contexts::media::{
    domain::playback_grant::GrantKey, FilesystemPublicationStorage, OriginPlayback,
    PublicMediaOrigin,
};
use std::cell::RefCell;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

const DATASET: &str = "theatre-catalog-v2";

/// Requests are recorded and fsynced before effects; successful responses are recorded before
/// advancing. An interrupted request reuses its exact operation ID and immutable payload.
struct SeedJournal {
    path: std::path::PathBuf,
    value: Value,
    next: usize,
}
impl SeedJournal {
    fn open(path: std::path::PathBuf, identity: Value, now: i64) -> Self {
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(include_bytes!("theatre_dev_seed_http_turso.rs"))
        );
        let value = if path.exists() {
            let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(
                value["dataset"], DATASET,
                "journal dataset differs; reconcile before changing it"
            );
            assert_eq!(
                value["fingerprint"], fingerprint,
                "seed source changed; reconcile existing journal explicitly"
            );
            assert_eq!(
                value["identity"], identity,
                "journal target/actor/roots differ; refusing another environment"
            );
            assert!(
                value["valid_until"].as_i64().unwrap() > now,
                "recorded seed rights expired; requires a new reviewed dataset revision"
            );
            value
        } else {
            json!({"dataset":DATASET,"fingerprint":fingerprint,"identity":identity,
                   "valid_until":now+RIGHTS_WINDOW_MS,"operations":[]})
        };
        let journal = Self {
            path,
            value,
            next: 0,
        };
        journal.save();
        journal
    }
    /// Session proof is refreshed before each run. It is audit history, not dataset identity:
    /// a newly authenticated session for the same bound actor can replay original operations.
    fn record_session(&mut self, binding: &SeedSessionBinding) {
        assert_eq!(
            self.value["identity"]["actor"], binding.actor,
            "verified session actor differs from dataset owner"
        );
        if self.value.get("validated_sessions").is_none() {
            self.value["validated_sessions"] = json!([]);
        }
        let sessions = self.value["validated_sessions"].as_array_mut().unwrap();
        if sessions
            .last()
            .is_none_or(|previous| previous["fence"] != binding.fence)
        {
            sessions.push(json!({"actor":binding.actor,"fence":binding.fence}));
            self.save();
        }
    }
    fn save(&self) {
        let temporary = self.path.with_extension("pending");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temporary)
            .unwrap();
        file.write_all(&serde_json::to_vec_pretty(&self.value).unwrap())
            .unwrap();
        file.sync_all().unwrap();
        fs::rename(&temporary, &self.path).unwrap();
        fs::File::open(self.path.parent().unwrap())
            .unwrap()
            .sync_all()
            .unwrap();
    }
    fn begin(
        &mut self,
        method: &str,
        path: &str,
        proposed: Value,
    ) -> (usize, Value, Option<Reply>) {
        let index = self.next;
        self.next += 1;
        if index == self.value["operations"].as_array().unwrap().len() {
            self.value["operations"]
                .as_array_mut()
                .unwrap()
                .push(json!({"method":method,"path":path,"request":proposed}));
            self.save();
        }
        let entry = &self.value["operations"][index];
        assert_eq!(entry["method"], method, "journal sequence differs");
        assert_eq!(entry["path"], path, "journal sequence differs");
        let reply = entry.get("status").map(|status| Reply {
            status: StatusCode::from_u16(status.as_u64().unwrap() as u16).unwrap(),
            body: entry["response"].clone(),
        });
        (index, entry["request"].clone(), reply)
    }
    fn complete(&mut self, index: usize, reply: &Reply) {
        if reply.status.is_success() {
            self.value["operations"][index]["status"] = json!(reply.status.as_u16());
            self.value["operations"][index]["response"] = reply.body.clone();
            self.save();
        }
    }
}

/// Long enough for a development session; the record says when to seed again.
const RIGHTS_WINDOW_MS: i64 = 30 * 86_400_000;
/// More than the default listener page (20), so Load more is exercised by real data.
const PODCAST_EPISODES: u16 = 23;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fate {
    /// Public Work; every unit is published unless the unit itself is withdrawn.
    Public,
    /// Public Work whose only release was published and then withdrawn.
    Withdrawn,
    /// Published release of a private Work: never anonymous.
    Private,
    /// Approved candidate that was never published.
    Unpublished,
}

struct SeedUnit {
    title: String,
    description: String,
    withdraw: bool,
    seconds: u8,
}
struct SeedWork {
    number: u16,
    kind: &'static str,
    title: &'static str,
    synopsis: &'static str,
    language: &'static str,
    fate: Fate,
    units: Vec<SeedUnit>,
}
fn work_id(number: u16) -> String {
    format!("5eed{number:04}-0000-4000-8000-000000000000")
}
fn unit_id(work: u16, unit: usize) -> String {
    format!("5eed{work:04}-{:04}-4000-8000-000000000000", unit + 1)
}
fn unit(title: &str, description: &str, seconds: u8) -> SeedUnit {
    SeedUnit {
        title: title.into(),
        description: description.into(),
        withdraw: false,
        seconds,
    }
}
/// Display order on the public shelf is newest first publication, so publish in reverse.
fn catalog() -> Vec<SeedWork> {
    let mut lantern = vec![
        unit(
            "Ngọn đèn đầu tiên",
            "Cô bé Thảo học cách dán khung tre từ bà ngoại.",
            3,
        ),
        unit(
            "Đêm rằm bị hoãn",
            "Chương đã rút khỏi phát hành để kiểm tra hàng bị ẩn.",
            2,
        ),
        unit(
            "Ánh sáng qua lớp giấy",
            "Chiếc đèn cuối cùng được thả trôi trên sông.",
            4,
        ),
    ];
    lantern[1].withdraw = true;
    vec![
        SeedWork {
            number: 1,
            kind: "novel",
            title: "Mùa gió chướng",
            synopsis: "Một gia đình làm muối ở cửa biển chờ người cha đi biển trở về, qua bốn mùa gió chướng thổi ngược dòng sông.",
            language: "vi-VN",
            fate: Fate::Public,
            units: vec![
                unit("Chương một: Bến cũ", "Ruộng muối, chiếc ghe và lời hứa trước mùa gió.", 125),
                unit("Chương hai: Muối kết hạt", "Nắng gắt làm hạt muối trắng và lòng người mặn.", 4),
                unit("Chương ba: Tin từ ngoài khơi", "Một chiếc radio cũ bắt được giọng nói quen.", 5),
                unit("Chương bốn: Gió đổi chiều", "Cả xóm ra bến đón những cánh buồm đầu tiên.", 3),
            ],
        },
        SeedWork {
            number: 2,
            kind: "story",
            title: "Chiếc đèn lồng giấy",
            synopsis: "Truyện ngắn về ba đêm làm đèn lồng trước Tết Trung thu ở một con hẻm Hội An.",
            language: "vi-VN",
            fate: Fate::Public,
            units: lantern,
        },
        SeedWork {
            number: 3,
            kind: "podcast",
            title: "Chuyện đêm khuya",
            synopsis: "Mỗi tập là một câu chuyện ngắn kể chậm cho người khó ngủ, từ những chuyến xe đêm đến tiếng mưa trên mái tôn.",
            language: "vi-VN",
            fate: Fate::Public,
            units: (1..=PODCAST_EPISODES)
                .map(|n| SeedUnit {
                    title: format!("Tập {n}: Chuyến xe số {}", 10 + n),
                    description: format!("Câu chuyện thứ {n} trên tuyến xe đêm qua thành phố."),
                    withdraw: false,
                    seconds: 2 + (n % 3) as u8,
                })
                .collect(),
        },
        SeedWork {
            number: 4,
            kind: "story",
            title: "The Lantern Keeper's Notes",
            synopsis: "A short English story about a keeper who writes one note for every lantern on the river.",
            language: "en-US",
            fate: Fate::Public,
            units: vec![
                unit("First note", "The keeper finds a lantern without a name.", 2),
                unit("Last note", "Every lantern returns to the riverbank at dawn.", 3),
            ],
        },
        SeedWork {
            number: 5,
            kind: "podcast",
            title: "Tín hiệu đã tắt",
            synopsis: "Podcast đã phát hành rồi rút lại; phải luôn trả not_found công khai.",
            language: "vi-VN",
            fate: Fate::Withdrawn,
            units: vec![unit("Tập duy nhất", "Bản phát hành đã bị rút.", 2)],
        },
        SeedWork {
            number: 6,
            kind: "novel",
            title: "Bản thảo riêng tư",
            synopsis: "Tác phẩm riêng tư có bản phát hành; khán giả ẩn danh không được thấy.",
            language: "vi-VN",
            fate: Fate::Private,
            units: vec![unit("Chương riêng tư", "Không bao giờ xuất hiện trong danh mục công khai.", 2)],
        },
        SeedWork {
            number: 7,
            kind: "story",
            title: "Truyện chưa phát hành",
            synopsis: "Ứng viên đã duyệt nhưng chưa xuất bản; không có bản phát hành công khai.",
            language: "vi-VN",
            fate: Fate::Unpublished,
            units: vec![unit("Chương chờ phát hành", "Chưa có bản phát hành.", 2)],
        },
    ]
}

struct Seeded {
    id: String,
    position: usize,
    publication: Option<String>,
    pointer_version: u64,
    withdrawn: bool,
    rendition: String,
    sha256: String,
    byte_length: u64,
}
/// The persistent lane uses the configured server's own authentication/composition, including
/// real OIDC-backed sessions. It never authenticates through a newly constructed local Store.
#[derive(Debug, PartialEq, Eq)]
enum SeedBindingError {
    MissingFence,
    InvalidFence,
    ActorMismatch,
    SessionMismatch,
}
#[derive(Debug)]
struct SeedSessionBinding {
    actor: String,
    fence: String,
}
impl SeedSessionBinding {
    fn checked(
        headers: &axum::http::HeaderMap,
        body: &Value,
        db_actor: &str,
        db_fence: &str,
    ) -> Result<Self, SeedBindingError> {
        let mut values = headers.get_all("x-cantos-studio-session").iter();
        let fence = values
            .next()
            .and_then(|v| v.to_str().ok())
            .ok_or(SeedBindingError::MissingFence)?;
        if values.next().is_some() {
            return Err(SeedBindingError::InvalidFence);
        }
        let parsed = Uuid::parse_str(fence).map_err(|_| SeedBindingError::InvalidFence)?;
        if parsed.is_nil() || parsed.to_string() != fence {
            return Err(SeedBindingError::InvalidFence);
        }
        let actor = body["actor_id"]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or(SeedBindingError::ActorMismatch)?;
        if actor != db_actor {
            return Err(SeedBindingError::ActorMismatch);
        }
        if fence != db_fence {
            return Err(SeedBindingError::SessionMismatch);
        }
        Ok(Self {
            actor: actor.into(),
            fence: fence.into(),
        })
    }
}

/// Mirrors the closed StudioOidc::session_name mapping: secure production vs explicit local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedCookieName {
    Secure,
    Development,
}
impl SeedCookieName {
    fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "__Secure-cantos_studio" => Ok(Self::Secure),
            "cantos_session" => Ok(Self::Development),
            _ => Err("unsupported Studio cookie name"),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Self::Secure => "__Secure-cantos_studio",
            Self::Development => "cantos_session",
        }
    }
    fn header(self, token: &str) -> Result<String, &'static str> {
        if token.is_empty()
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-._~".contains(&byte))
        {
            return Err("invalid Studio cookie value");
        }
        Ok(format!("{}={token}", self.as_str()))
    }
}

enum SeedAuthentication<'a> {
    Anonymous,
    Studio(&'a str),
    Producer(&'a str),
}
struct SeedRemote {
    client: reqwest::Client,
    api_origin: String,
    web_origin: String,
    session: Option<SeedSessionBinding>,
    cookie_name: SeedCookieName,
}
impl SeedRemote {
    fn new(api_origin: String, web_origin: String, cookie_name: SeedCookieName) -> Self {
        for value in [&api_origin, &web_origin] {
            let parsed = url::Url::parse(value).expect("valid explicit origin required");
            let loopback = matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
            assert!(
                parsed.origin().ascii_serialization() == *value
                    && (parsed.scheme() == "https" || parsed.scheme() == "http" && loopback)
                    && parsed.username().is_empty()
                    && parsed.password().is_none(),
                "HTTPS origin required; HTTP only for local development"
            );
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        Self {
            client,
            api_origin,
            web_origin,
            session: None,
            cookie_name,
        }
    }
    async fn bind_session(&mut self, token: &str, db_actor: &str, db_fence: &str) -> String {
        let (status, headers, bytes) = self
            .request(
                "GET",
                "/api/v1/studio/session",
                SeedAuthentication::Studio(token),
                None,
                "",
                None,
            )
            .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "configured API must authenticate the selected Studio session"
        );
        let body: Value = serde_json::from_slice(&bytes).expect("Studio session JSON required");
        let binding = SeedSessionBinding::checked(&headers, &body, db_actor, db_fence)
            .expect("API/DB Studio session binding mismatch; refusing before any seed mutation");
        let actor = binding.actor.clone();
        self.session = Some(binding);
        actor
    }
    async fn response(
        mut response: reqwest::Response,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let status = response.status();
        let headers = response.headers().clone();
        const LIMIT: usize = 4 * 1024 * 1024;
        assert!(
            response.content_length().is_none_or(|n| n <= LIMIT as u64),
            "API response exceeds seed bound"
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .unwrap_or_else(|_| panic!("configured API response failed (details redacted)"))
        {
            assert!(
                bytes.len() + chunk.len() <= LIMIT,
                "API response exceeds seed bound"
            );
            bytes.extend_from_slice(&chunk);
        }
        (status, headers, bytes)
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        authentication: SeedAuthentication<'_>,
        body: Option<Vec<u8>>,
        content_type: &str,
        range: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        assert!(
            path.starts_with("/api/v1/") && !path.starts_with("//") && !path.contains(['\r', '\n']),
            "API path required"
        );
        let mut request = self
            .client
            .request(
                method.parse::<reqwest::Method>().unwrap(),
                format!("{}{path}", self.api_origin),
            )
            .header("accept", "application/json");
        request = match authentication {
            SeedAuthentication::Anonymous => request,
            SeedAuthentication::Studio(token) => {
                assert!(path.starts_with("/api/v1/studio/"));
                let request = request
                    .header(
                        "cookie",
                        self.cookie_name
                            .header(token)
                            .expect("valid Studio cookie value required"),
                    )
                    .header("origin", &self.web_origin);
                if let Some(session) = &self.session {
                    request.header("x-cantos-studio-session", &session.fence)
                } else {
                    assert!(
                        method == "GET" && path == "/api/v1/studio/session",
                        "bind API/DB session before Studio requests"
                    );
                    request
                }
            }
            SeedAuthentication::Producer(token) => {
                assert!(path.starts_with("/api/v1/studio/producer/"));
                // The existing audio PUT is still behind the Studio Origin guard; only
                // append delegations are currently exempt from it. Keep the exact Studio
                // origin while sending no cookie or session fence with the upload bearer.
                request
                    .bearer_auth(token)
                    .header("origin", &self.web_origin)
            }
        };
        if let Some(body) = body {
            request = request.header("content-type", content_type).body(body);
        }
        if let Some(range) = range {
            request = request.header("range", range);
        }
        let response = request
            .send()
            .await
            .unwrap_or_else(|_| panic!("configured API transport failed (details redacted)"));
        Self::response(response).await
    }
    /// Follow the advertised HTTPS delivery path itself. Only disposable in-process evidence
    /// substitutes a grant path onto its local router; named acceptance never rewrites origins.
    async fn media(
        &self,
        method: &str,
        url: &str,
        range: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let parsed = url::Url::parse(url).expect("valid playback URL required");
        assert!(
            url.starts_with("https://")
                && !url.contains('\\')
                && parsed.scheme() == "https"
                && parsed.host_str().is_some()
                && parsed.username().is_empty()
                && parsed.password().is_none()
                && parsed.fragment().is_none()
                && parsed.query().is_none()
                && parsed.path().starts_with("/api/v1/theatre/media/")
                && url.len() <= 4096
                && !url.chars().any(|c| c.is_control() || c.is_whitespace()),
            "advertised HTTPS media URL required"
        );
        let mut request = self
            .client
            .request(method.parse::<reqwest::Method>().unwrap(), parsed)
            .header("accept", "audio/ogg");
        if let Some(range) = range {
            request = request.header("range", range);
        }
        let response = request
            .send()
            .await
            .unwrap_or_else(|_| panic!("advertised media transport failed (URL redacted)"));
        Self::response(response).await
    }
    async fn json(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
        let (status, _, bytes) = self
            .request(
                method,
                path,
                token.map_or(SeedAuthentication::Anonymous, SeedAuthentication::Studio),
                body.map(|v| serde_json::to_vec(&v).unwrap()),
                "application/json",
                None,
            )
            .await;
        let body = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
            panic!(
                "configured API returned non-JSON status {}",
                status.as_u16()
            )
        });
        Reply { status, body }
    }
}

struct SeedWorld {
    h: Harness,
    root: PrivateRoot,
    publication_root: PrivateRoot,
    token: String,
    rights_ref: String,
    journal: Option<RefCell<SeedJournal>>,
    remote: Option<SeedRemote>,
}
impl SeedWorld {
    async fn new() -> Self {
        let mut h = Harness::new().await;
        let token = ALICE.to_owned();
        assert_eq!(h.store.authenticate(&token).await.unwrap(), "alice");
        let rights_ref = "catalog-rights".to_owned();
        let root = PrivateRoot::new();
        let publication_root = PrivateRoot::new();
        h.store = h.store.clone().with_audio_storage(&root.0).unwrap();
        let delivery = Arc::new(FilesystemPublicationStorage::new(&publication_root.0).unwrap());
        let playback = OriginPlayback::new(
            PublicMediaOrigin::parse("https://seed.example.invalid").unwrap(),
            GrantKey::new(&[83; 32]).unwrap(),
            delivery.clone(),
        );
        let origins = cantos_server::platform::host::SurfaceOrigins::new(
            ORIGIN,
            THEATRE_BROWSER_ORIGIN,
            None,
        )
        .unwrap();
        h.app = server_router(
            AppState {
                store: h.store.clone(),
                web_origin: ORIGIN.into(),
            },
            &origins,
            None,
            Some((
                cantos_server::contexts::media::PublicationService::new(h.store.clone())
                    .with_origin_publication(delivery),
                playback,
            )),
        );
        let journal = None;
        let remote = None;
        Self {
            h,
            root,
            publication_root,
            token,
            rights_ref,
            journal,
            remote,
        }
    }
    async fn studio(&self, method: &str, path: &str, body: Option<Value>) -> Reply {
        if let Some(remote) = &self.remote {
            return remote
                .json(
                    method,
                    &format!("/api/v1/studio{path}"),
                    Some(&self.token),
                    body,
                )
                .await;
        }
        self.h.json(method, path, Some(&self.token), body).await
    }
    async fn theatre(&self, method: &str, path: &str) -> Reply {
        if let Some(remote) = &self.remote {
            return remote
                .json(method, &format!("/api/v1/theatre{path}"), None, None)
                .await;
        }
        let response = self
            .h
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(format!("/api/v1/theatre{path}"))
                    .header(header::ORIGIN, THEATRE_BROWSER_ORIGIN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            THEATRE_BROWSER_ORIGIN
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let status = response.status();
        let body =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        Reply { status, body }
    }
    async fn media(
        &self,
        method: &str,
        url: &str,
        range: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let path = url::Url::parse(url).unwrap().path().to_owned();
        assert!(path.starts_with("/api/v1/theatre/media/"));
        if let Some(remote) = &self.remote {
            return remote.media(method, url, range).await;
        }
        let mut request = Request::builder()
            .method(method)
            .uri(&path)
            .header(header::ORIGIN, THEATRE_BROWSER_ORIGIN);
        if let Some(range) = range {
            request = request.header("range", range);
        }
        let response = self
            .h
            .app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            THEATRE_BROWSER_ORIGIN
        );
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_CREDENTIALS],
            "true"
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-store, no-transform"
        );
        (
            response.status(),
            response.headers().clone(),
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
    }
    async fn json(&self, method: &str, path: &str, body: Option<Value>) -> Reply {
        let Some(journal) = &self.journal else {
            return self.studio(method, path, body).await;
        };
        let (index, request, completed) =
            journal
                .borrow_mut()
                .begin(method, path, body.unwrap_or(Value::Null));
        if let Some(reply) = completed {
            return reply;
        }
        let reply = self.studio(method, path, Some(request)).await;
        journal.borrow_mut().complete(index, &reply);
        reply
    }
    async fn upload(&self, submission: &str, token: &str, bytes: Vec<u8>) -> Reply {
        let Some(journal) = &self.journal else {
            return upload(self.h.app.clone(), submission, token, bytes).await;
        };
        let (index, _, completed) = journal.borrow_mut().begin(
            "PUT",
            submission,
            json!({"sha256":format!("{:x}",Sha256::digest(&bytes))}),
        );
        if let Some(reply) = completed {
            return reply;
        }
        let reply = if let Some(remote) = &self.remote {
            let (status, _, bytes) = remote
                .request(
                    "PUT",
                    &format!("/api/v1/studio/producer/audio-submissions/{submission}/bytes"),
                    SeedAuthentication::Producer(token),
                    Some(bytes),
                    "application/octet-stream",
                    None,
                )
                .await;
            Reply {
                status,
                body: serde_json::from_slice(&bytes).unwrap(),
            }
        } else {
            upload(self.h.app.clone(), submission, token, bytes).await
        };
        journal.borrow_mut().complete(index, &reply);
        reply
    }
    async fn approve(
        &self,
        unit: &str,
        submission: &str,
        rendition: &Value,
        valid_until: i64,
    ) -> Value {
        let path = format!("/content-units/{unit}/audio-submissions/{submission}");
        for kind in ["qc", "rights", "editorial"] {
            self.json(
                "POST",
                &format!("{path}/reviews"),
                Some(review_request(rendition, kind, "approved")),
            )
            .await
            .success(StatusCode::CREATED);
        }
        let candidate = self
            .json(
                "POST",
                &format!("{path}/release-candidates"),
                Some(json!({"operation_id":operation(),"rendition_digest":rendition["digest"]})),
            )
            .await
            .success(StatusCode::CREATED)["candidate"]
            .clone();
        let base = format!(
            "/content-units/{unit}/release-candidates/{}",
            candidate["id"].as_str().unwrap()
        );
        if self.journal.is_none() {
            let ready = self
                .studio("GET", &format!("{base}/readiness"), None)
                .await
                .success(StatusCode::OK);
            assert!(!ready["blockers"].as_array().unwrap().is_empty());
            assert!(
                !self
                    .publication_root
                    .0
                    .join("objects")
                    .join(rendition["verified_media"]["sha256"].as_str().unwrap())
                    .exists(),
                "unapproved candidate must not be promoted"
            );
        }
        let clearance = self.json("POST",&format!("{base}/distribution-rights"),Some(json!({"operation_id":operation(),"candidate_digest":candidate["digest"],"decision":"cleared","rights_refs":[self.rights_ref],"valid_until":valid_until,"evidence_refs":[self.rights_ref],"notes":"Owned synthetic signal only; human listening NOT_RUN"}))).await.success(StatusCode::CREATED);
        self.json("POST",&format!("{base}/publication-approvals"),Some(json!({"operation_id":operation(),"candidate_digest":candidate["digest"],"clearance_digest":clearance["digest"],"decision":"approved","notes":"Synthetic test publication; no real manuscript or voice"}))).await.success(StatusCode::CREATED);
        if self.journal.is_none() {
            let ready = self
                .studio("GET", &format!("{base}/readiness"), None)
                .await
                .success(StatusCode::OK);
            assert_eq!(ready["blockers"], json!([]));
            let bytes = fs::read(
                self.publication_root
                    .0
                    .join("objects")
                    .join(rendition["verified_media"]["sha256"].as_str().unwrap()),
            )
            .unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                rendition["verified_media"]["sha256"].as_str().unwrap()
            );
        }
        candidate
    }
    /// A distinct synthetic Ogg/Opus signal per unit: real bytes for the actual intake check.
    fn signal(&self, seconds: u8, frequency: u32) -> Vec<u8> {
        let path = self
            .root
            .0
            .join(format!("theatre-seed-{seconds}-{frequency}.opus"));
        if path.is_file() {
            return fs::read(&path).unwrap();
        }
        let source = format!("sine=frequency={frequency}:sample_rate=48000:duration={seconds}");
        let output = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
            .arg(source)
            .args(["-ac", "1", "-c:a", "libopus", "-b:a", "24k", "-vbr", "off"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read(path).unwrap()
    }
    async fn seed(&self, spec: &SeedWork, valid_until: i64) -> Vec<Seeded> {
        let id = work_id(spec.number);
        let mut request = work_request(spec.kind);
        request["id"] = json!(id);
        request["details"]["title"] = json!(spec.title);
        request["details"]["synopsis"] = json!(spec.synopsis);
        request["details"]["language"] = json!(spec.language);
        request["details"]["visibility"] = json!(if spec.fate == Fate::Private {
            "private"
        } else {
            "public"
        });
        request["details"]["rights_refs"] = json!([self.rights_ref]);
        let mut work = self
            .json("POST", "/works", Some(request))
            .await
            .success(StatusCode::CREATED)["work"]
            .clone();
        for (index, unit) in spec.units.iter().enumerate() {
            let mut request = unit_request(&work, &unit.title);
            request["id"] = json!(unit_id(spec.number, index));
            request["details"]["description"] = json!(unit.description);
            work = self
                .json("POST", &format!("/works/{id}/content-units"), Some(request))
                .await
                .success(StatusCode::CREATED)["work"]
                .clone();
        }
        // Content writes advance the Work revision; finish them before any candidate exists so
        // every release pins the final metadata revision.
        let mut contents = Vec::new();
        for index in 0..spec.units.len() {
            let mut request = content_request(
                0,
                work["revision"].as_u64().unwrap(),
                if spec.kind == "podcast" {
                    json!({"kind":"recording"})
                } else {
                    json!({"kind":"text","text":format!("{}\n{}\nNội dung tiếng Việt do Cantos tự tạo cho kiểm thử; âm thanh là tín hiệu tổng hợp, không phải lời đọc.",spec.title,spec.units[index].description),"source":null})
                },
            );
            request["details"]["language"] = json!(spec.language);
            let response = self
                .json(
                    "POST",
                    &format!(
                        "/content-units/{}/content/revisions",
                        unit_id(spec.number, index)
                    ),
                    Some(request),
                )
                .await
                .success(StatusCode::CREATED);
            work["revision"] = response["work_revision"].clone();
            contents.push(response["content"].clone());
        }
        // The explicit synthetic fixture publishes metadata independently of producing audio.
        if matches!(spec.fate, Fate::Public | Fate::Withdrawn) {
            let selected: Vec<_> = (0..spec.units.len())
                .map(|index| json!({"unit_id":unit_id(spec.number,index),"revision":1}))
                .collect();
            self.json("POST", &format!("/works/{id}/metadata-publication"), Some(json!({"action":"publish","operation_id":operation(),"expected_version":0,"work_revision":work["revision"],"units":selected,"confirm_metadata_rights":true}))).await.success(StatusCode::OK);
        }
        let mut seeded = Vec::new();
        for (index, (unit, content)) in spec.units.iter().zip(contents).enumerate() {
            let uid = unit_id(spec.number, index);
            let brief = self
                .json(
                    "POST",
                    &format!("/content-units/{uid}/production-briefs"),
                    Some(brief_http::brief_request(
                        &content,
                        0,
                        spec.kind == "podcast",
                    )),
                )
                .await
                .success(StatusCode::CREATED)["brief"]
                .clone();
            let bytes = self.signal(
                unit.seconds,
                220 + 20 * u32::from(spec.number) + 7 * index as u32,
            );
            // Upload capabilities are unique within the shared database.
            let token = format!("{:x}", Sha256::digest(operation().as_bytes()));
            let mut request = submission_request(&self.h, &uid, &brief, &bytes, &token).await;
            request["declarations"]["rights_refs"] = json!([self.rights_ref]);
            // The journal fixes both the generated submission identity and upload token.
            let prepared = self
                .json(
                    "POST",
                    &format!("/content-units/{uid}/audio-submissions"),
                    Some(request),
                )
                .await
                .success(StatusCode::CREATED);
            let prepared: cantos_server::contract::studio::AudioSubmissionResponse =
                serde_json::from_value(prepared)
                    .expect("Studio returns the AudioSubmissionResponse envelope");
            let submission = prepared.submission.id;
            // The upload capability is part of the persisted preparation request.
            let token = self
                .journal
                .as_ref()
                .map(|j| {
                    let j = j.borrow();
                    j.value["operations"][j.next - 1]["request"]["upload_token"]
                        .as_str()
                        .unwrap()
                        .to_owned()
                })
                .unwrap_or(token);
            let rendition = self
                .upload(&submission, &token, bytes)
                .await
                .success(StatusCode::OK);
            let candidate = self
                .approve(&uid, &submission, &rendition, valid_until)
                .await;
            let mut row = Seeded {
                id: uid.clone(),
                position: index,
                publication: None,
                pointer_version: 0,
                withdrawn: false,
                rendition: rendition["id"].as_str().unwrap().to_owned(),
                sha256: rendition["verified_media"]["sha256"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
                byte_length: rendition["verified_media"]["byte_length"].as_u64().unwrap(),
            };
            if spec.fate != Fate::Unpublished {
                let published = self.json("POST", &format!("/content-units/{uid}/release-candidates/{}/publish", candidate["id"].as_str().unwrap()), Some(json!({"operation_id":operation(),"candidate_digest":candidate["digest"],"expected_pointer_version":0,"expected_active_release_id":null}))).await.success(StatusCode::CREATED);
                row.pointer_version = published["pointer_version"].as_u64().unwrap();
                row.publication = Some(published["release"]["id"].as_str().unwrap().to_owned());
            }
            if unit.withdraw || spec.fate == Fate::Withdrawn {
                // First-run evidence: a previously granted capability loses access immediately
                // after withdrawal. Persistent replay already has the immutable withdrawal.
                let grant = if self.journal.is_none() {
                    Some(
                        self.theatre(
                            "POST",
                            &format!(
                                "/publications/{}/playback-authorizations",
                                row.publication.as_deref().unwrap()
                            ),
                        )
                        .await
                        .success(StatusCode::OK)["url"]
                            .as_str()
                            .unwrap()
                            .to_owned(),
                    )
                } else {
                    None
                };
                let withdrawn = self.json("POST", &format!("/content-units/{uid}/publication/withdraw"), Some(json!({"operation_id":operation(),"release_id":row.publication,"expected_pointer_version":row.pointer_version,"reason":"Development seed: withdrawn release must stay hidden"}))).await.success(StatusCode::OK);
                row.pointer_version = withdrawn["pointer_version"].as_u64().unwrap();
                row.withdrawn = true;
                if let Some(grant) = grant {
                    let (status, _, _) = self.media("GET", &grant, None).await;
                    assert_eq!(status, StatusCode::NOT_FOUND);
                }
            }
            seeded.push(row);
        }
        if spec.fate == Fate::Withdrawn {
            let path = format!("/works/{id}/metadata-publication");
            let state = self
                .studio("GET", &path, None)
                .await
                .success(StatusCode::OK);
            self.json("POST",&path,Some(json!({"action":"withdraw","operation_id":operation(),"expected_version":state["version"]}))).await.success(StatusCode::OK);
        }
        seeded
    }
    async fn get(&self, path: &str) -> (StatusCode, Value) {
        let reply = self.theatre("GET", path).await;
        (reply.status, reply.body)
    }
    /// Strict decoding through the shared v1 DTOs proves no extra (private) field leaked.
    async fn decode<T: serde::de::DeserializeOwned>(&self, path: &str) -> T {
        let (status, body) = self.get(path).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        serde_json::from_value(body).unwrap_or_else(|e| panic!("{path}: {e}"))
    }
    async fn not_found(&self, path: &str) {
        let (status, body) = self.get(path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
        assert_eq!(body, json!({"code":"not_found","retryable":false}));
    }
    async fn units(&self, work: &str) -> Vec<UnitPage> {
        let mut page: UnitPage = self.decode(&format!("/works/{work}/units")).await;
        let mut pages = Vec::new();
        for _ in 0..8 {
            assert_eq!(page.work_id, work);
            let next = page.next_cursor.clone();
            pages.push(page);
            let Some(cursor) = next else {
                return pages;
            };
            page = self
                .decode(&format!("/works/{work}/units?cursor={cursor}"))
                .await;
        }
        panic!("bounded seeded unit walk did not terminate");
    }
}

#[tokio::test]
async fn theatre_development_catalog_seed() {
    let _media_lane = native_sql::media_lane().await;
    let world = SeedWorld::new().await;
    let valid_until = world
        .journal
        .as_ref()
        .map(|j| j.borrow().value["valid_until"].as_i64().unwrap())
        .unwrap_or(now(&world.h).await + RIGHTS_WINDOW_MS);
    let specs = catalog();
    let mut seeded = Vec::new();
    for spec in specs.iter().rev() {
        seeded.push(world.seed(spec, valid_until).await);
    }
    seeded.reverse();

    // The shelf: only the four public Works, newest first publication first.
    let public: Vec<String> = specs
        .iter()
        .filter(|s| s.fate == Fate::Public)
        .map(|s| work_id(s.number))
        .collect();
    let shelf: WorkPage = world.decode("/works?limit=20").await;
    if world.journal.is_none() {
        assert_eq!(
            shelf
                .items
                .iter()
                .map(|w| w.work_id.clone())
                .collect::<Vec<_>>(),
            public
        );
        assert_eq!(shelf.next_cursor, None);
        let first: WorkPage = world.decode("/works?limit=2").await;
        let cursor = first
            .next_cursor
            .clone()
            .expect("four works need a second page of two");
        let second: WorkPage = world
            .decode(&format!("/works?limit=2&cursor={cursor}"))
            .await;
        assert_eq!(second.next_cursor, None);
        assert_eq!(
            first
                .items
                .iter()
                .chain(&second.items)
                .cloned()
                .collect::<Vec<_>>(),
            shelf.items
        );
    }
    for (spec, units) in specs.iter().zip(&seeded) {
        // Read current Studio facts, rather than treating a cached operation response as state.
        let current = world
            .studio("GET", &format!("/works/{}", work_id(spec.number)), None)
            .await
            .success(StatusCode::OK);
        assert_eq!(
            current["details"]["title"], spec.title,
            "seed-owned metadata drift requires reconciliation"
        );
        assert_eq!(
            current["revision"].as_u64(),
            Some(1 + 2 * spec.units.len() as u64)
        );
        let wid = work_id(spec.number);
        for (unit, spec_unit) in units.iter().zip(&spec.units) {
            let current = world
                .studio("GET", &format!("/content-units/{}", unit.id), None)
                .await
                .success(StatusCode::OK);
            assert_eq!(current["details"]["title"], spec_unit.title);
            assert_eq!(current["details"]["description"], spec_unit.description);
            assert_eq!(current["revision"], 1);
            let current = world
                .studio(
                    "GET",
                    &format!("/content-units/{}/publication/active", unit.id),
                    None,
                )
                .await
                .success(StatusCode::OK);
            assert_eq!(current["pointer_version"], unit.pointer_version);
            if unit.withdrawn || unit.publication.is_none() {
                assert!(current["release"].is_null());
            } else {
                assert_eq!(
                    current["release"]["id"],
                    unit.publication.as_deref().unwrap()
                );
            }
        }
        if spec.fate != Fate::Public {
            world.not_found(&format!("/works/{wid}")).await;
            world.not_found(&format!("/works/{wid}/units")).await;
            for unit in units {
                if let Some(publication) = &unit.publication {
                    world
                        .not_found(&format!("/publications/{publication}"))
                        .await;
                }
            }
            continue;
        }
        let detail: WorkSummary = world.decode(&format!("/works/{wid}")).await;
        if world.journal.is_none() {
            assert_eq!(Some(&detail), shelf.items.iter().find(|w| w.work_id == wid));
        }
        assert_eq!(
            (detail.title.as_str(), detail.language.as_str()),
            (spec.title, spec.language)
        );
        let pages = world.units(&wid).await;
        let rows: Vec<_> = pages.iter().flat_map(|p| p.items.clone()).collect();
        let expected: Vec<&Seeded> = units.iter().collect();
        assert_eq!(
            rows.iter()
                .map(|r| (r.unit_id.as_str(), r.position as usize))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|u| (u.id.as_str(), u.position + 1))
                .collect::<Vec<_>>(),
            "audio withdrawal keeps metadata membership and published positions"
        );
        for (row, (unit, spec_unit)) in rows.iter().zip(units.iter().zip(&spec.units)) {
            if unit.withdrawn {
                assert_eq!(row.playback, None);
            } else {
                let audio = row.playback.as_ref().unwrap();
                assert_eq!(Some(&audio.publication_id), unit.publication.as_ref());
                let expected_ms = u64::from(spec_unit.seconds) * 1000;
                assert!(audio.duration_ms.abs_diff(expected_ms) <= 100, "{row:?}");
            }
            assert_eq!(row.title, spec_unit.title);
            assert_eq!(row.description, spec_unit.description);
        }
        if spec.kind == "podcast" {
            assert_eq!(pages.len(), 2, "23 episodes are two default pages");
            assert_eq!(pages[0].items.len(), 20);
        }
    }

    // Manifest, signed grant and full/Range bytes share the real promoted filesystem object.
    let novel = &seeded[0][0];
    let publication = novel.publication.as_deref().unwrap();
    let manifest: PublicationManifest = world.decode(&format!("/publications/{publication}")).await;
    assert_eq!(
        (
            manifest.work_id.as_str(),
            manifest.unit_id.as_str(),
            manifest.publication_id.as_str()
        ),
        (work_id(1).as_str(), novel.id.as_str(), publication)
    );
    let grant = world
        .theatre(
            "POST",
            &format!("/publications/{publication}/playback-authorizations"),
        )
        .await
        .success(StatusCode::OK);
    let grant: cantos_server::contract::theatre::discovery::PlaybackAuthorization =
        serde_json::from_value(grant).unwrap();
    assert_eq!(grant.sha256, novel.sha256);
    let (status, headers, bytes) = world.media("GET", &grant.url, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "audio/ogg");
    assert_eq!(bytes.len() as u64, novel.byte_length);
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), novel.sha256);
    let (status, headers, body) = world.media("HEAD", &grant.url, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-length"], novel.byte_length.to_string());
    assert!(body.is_empty());
    let (status, _, body) = world.media("GET", &grant.url, Some("bytes=100-199")).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, &bytes[100..200]);
    if world.remote.is_none() {
        // The same canonical host must preserve owner-only private transport and cache
        // headers while serving public grants from its separate publication bytes.
        let submission: String = world
            .h
            .admin
            .query_one(
                "SELECT submission_id FROM audio_renditions WHERE id=?1",
                &[&novel.rendition],
            )
            .await
            .unwrap()
            .get(0);
        let private_path = format!(
            "/api/v1/studio/content-units/{}/audio-submissions/{submission}/renditions/{}/bytes",
            novel.id, novel.rendition
        );
        let response = world
            .h
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&private_path)
                    .header(header::ORIGIN, ORIGIN)
                    .header(header::COOKIE, format!("cantos_session={}", world.token))
                    .header(header::RANGE, "bytes=100-199")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-store, no-transform"
        );
        assert_eq!(
            response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            ORIGIN
        );
        assert_eq!(
            response.into_body().collect().await.unwrap().to_bytes(),
            &bytes[100..200]
        );
        for (origin, cookie, status) in [
            (ORIGIN, None, StatusCode::UNAUTHORIZED),
            (
                THEATRE_BROWSER_ORIGIN,
                Some(world.token.as_str()),
                StatusCode::FORBIDDEN,
            ),
        ] {
            let mut request = Request::builder()
                .uri(&private_path)
                .header(header::ORIGIN, origin);
            if let Some(cookie) = cookie {
                request = request.header(header::COOKIE, format!("cantos_session={cookie}"));
            }
            let response = world
                .h
                .app
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
    }
    let invalid_range = format!("bytes={}-", bytes.len());
    let (status, _, _) = world.media("GET", &grant.url, Some(&invalid_range)).await;
    assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);
    world
        .not_found("/works/5eed9999-0000-4000-8000-000000000000")
        .await;
    let (status, body) = world
        .get(&format!(
            "/works/{}/units?cursor=lc1_{}",
            work_id(3),
            "0".repeat(64)
        ))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({"code":"invalid_cursor","retryable":false}));
}

#[test]
fn seed_write_ahead_replay_preserves_operation_after_lost_response() {
    let root = PrivateRoot::new();
    let path = root.0.join("journal.json");
    let identity = json!({"database":"synthetic","actor":"owner"});
    let mut first = SeedJournal::open(path.clone(), identity.clone(), 0);
    let (_, request, reply) = first.begin(
        "POST",
        "/works",
        json!({"operation_id":"first","title":"Ánh đèn"}),
    );
    assert!(reply.is_none());
    assert_eq!(request["operation_id"], "first");
    drop(first); // Crash after the DB may have committed, before response checkpoint.
    let mut resumed = SeedJournal::open(path.clone(), identity.clone(), 1);
    let (index, request, reply) =
        resumed.begin("POST", "/works", json!({"operation_id":"different"}));
    assert!(reply.is_none());
    assert_eq!(request["operation_id"], "first");
    resumed.complete(
        index,
        &Reply {
            status: StatusCode::CREATED,
            body: json!({"id":"owned"}),
        },
    );
    drop(resumed);
    let mut completed = SeedJournal::open(path, identity, 2);
    let (_, _, reply) = completed.begin("POST", "/works", Value::Null);
    assert_eq!(reply.unwrap().body, json!({"id":"owned"}));
}

#[test]
#[should_panic(expected = "journal target/actor/roots differ")]
fn seed_journal_refuses_a_different_environment_before_replay() {
    let root = PrivateRoot::new();
    let path = root.0.join("journal.json");
    SeedJournal::open(path.clone(), json!({"environment":"first"}), 0);
    SeedJournal::open(path, json!({"environment":"second"}), 1);
}

#[test]
fn persistent_session_binding_refuses_wrong_database_actor_or_session_before_effects() {
    let fence = "42000000-0000-4000-8000-000000000042";
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-cantos-studio-session", fence.parse().unwrap());
    let body = json!({"actor_id":"oidc-actor"});
    assert!(matches!(
        SeedSessionBinding::checked(
            &headers,
            &body,
            "oidc-actor",
            "43000000-0000-4000-8000-000000000043"
        ),
        Err(SeedBindingError::SessionMismatch)
    ));
    assert!(matches!(
        SeedSessionBinding::checked(&headers, &body, "other-db-actor", fence),
        Err(SeedBindingError::ActorMismatch)
    ));
    assert!(matches!(
        SeedSessionBinding::checked(&axum::http::HeaderMap::new(), &body, "oidc-actor", fence),
        Err(SeedBindingError::MissingFence)
    ));
    headers.append("x-cantos-studio-session", fence.parse().unwrap());
    assert!(matches!(
        SeedSessionBinding::checked(&headers, &body, "oidc-actor", fence),
        Err(SeedBindingError::InvalidFence)
    ));
}

#[tokio::test]
async fn persistent_http_transport_binds_real_session_and_public_reads_cross_actual_cors_anonymously(
) {
    let _media_lane = native_sql::media_lane().await;
    use axum::{
        routing::{get, post, put},
        Json,
    };
    const FENCE: &str = "42000000-0000-4000-8000-000000000042";
    async fn session(headers: axum::http::HeaderMap) -> (axum::http::HeaderMap, Json<Value>) {
        assert_eq!(
            headers["cookie"],
            "__Secure-cantos_studio=opaque-oidc-session"
        );
        assert_eq!(headers["origin"], "https://studio.example.org");
        assert!(!headers.contains_key("authorization"));
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-cantos-studio-session", FENCE.parse().unwrap());
        (headers, Json(json!({"actor_id":"real-oidc-actor"})))
    }
    async fn mutation(headers: axum::http::HeaderMap) -> Json<Value> {
        assert_eq!(
            headers["cookie"],
            "__Secure-cantos_studio=opaque-oidc-session"
        );
        assert_eq!(headers["origin"], "https://studio.example.org");
        assert_eq!(headers["x-cantos-studio-session"], FENCE);
        Json(json!({"id":"synthetic-only"}))
    }
    async fn public(headers: axum::http::HeaderMap) -> Json<Value> {
        for name in [
            "cookie",
            "authorization",
            "origin",
            "x-cantos-studio-session",
        ] {
            assert!(!headers.contains_key(name));
        }
        Json(json!({"items":[],"next_cursor":null}))
    }
    async fn producer(headers: axum::http::HeaderMap) -> Json<Value> {
        assert_eq!(headers["authorization"], "Bearer synthetic-upload");
        for name in ["cookie", "x-cantos-studio-session"] {
            assert!(!headers.contains_key(name));
        }
        assert_eq!(headers["origin"], "https://studio.example.org");
        Json(json!({"accepted":true}))
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/api/v1/studio/session", get(session))
        .route("/api/v1/studio/works", post(mutation))
        .route("/api/v1/studio/producer/test", put(producer))
        .route("/api/v1/theatre/works", get(public))
        .route(
            "/api/v1/studio/redirect",
            get(|| async {
                axum::response::Redirect::temporary("https://unrelated.example.invalid")
            }),
        );
    let app = cantos_server::adapters::inbound::http::with_browser_cors(
        app,
        "https://studio.example.org",
        cantos_server::adapters::inbound::http::WebSurface::Studio,
    )
    .unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut remote = SeedRemote::new(
        origin,
        "https://studio.example.org".into(),
        SeedCookieName::Secure,
    );
    let actor = remote
        .bind_session("opaque-oidc-session", "real-oidc-actor", FENCE)
        .await;
    assert_eq!(actor, "real-oidc-actor");
    remote
        .json(
            "POST",
            "/api/v1/studio/works",
            Some("opaque-oidc-session"),
            Some(json!({"synthetic":true})),
        )
        .await
        .success(StatusCode::OK);
    let public = remote
        .json("GET", "/api/v1/theatre/works", None, None)
        .await
        .success(StatusCode::OK);
    assert_eq!(public["items"], json!([]));
    let (status, _, _) = remote
        .request(
            "PUT",
            "/api/v1/studio/producer/test",
            SeedAuthentication::Producer("synthetic-upload"),
            Some(vec![1]),
            "application/octet-stream",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = remote
        .request(
            "GET",
            "/api/v1/studio/redirect",
            SeedAuthentication::Studio("opaque-oidc-session"),
            None,
            "",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    server.abort();
}

#[tokio::test]
#[should_panic(expected = "advertised HTTPS media URL required")]
async fn persistent_media_refuses_rewriting_a_plaintext_grant_onto_the_api_origin() {
    let _media_lane = native_sql::media_lane().await;
    let remote = SeedRemote::new(
        "http://127.0.0.1:1".into(),
        "https://studio.example.org".into(),
        SeedCookieName::Secure,
    );
    remote
        .media(
            "GET",
            "http://wrong-origin.example.invalid/api/v1/theatre/media/invalid",
            None,
        )
        .await;
}

#[test]
fn fresh_validated_session_for_same_dataset_reuses_operation_ids_and_responses() {
    fn binding(actor: &str, fence: &str) -> SeedSessionBinding {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-cantos-studio-session", fence.parse().unwrap());
        SeedSessionBinding::checked(&headers, &json!({"actor_id":actor}), actor, fence).unwrap()
    }
    let root = PrivateRoot::new();
    let path = root.0.join("journal.json");
    let identity = json!({"environment":"same","database":"same","actor":"same-owner","api_origin":"https://api.example.org"});
    let first = binding("same-owner", "42000000-0000-4000-8000-000000000042");
    let mut journal = SeedJournal::open(path.clone(), identity.clone(), 0);
    journal.record_session(&first);
    let (index, _, _) = journal.begin(
        "POST",
        "/works",
        json!({"operation_id":"original","title":"Ánh đèn"}),
    );
    journal.complete(
        index,
        &Reply {
            status: StatusCode::CREATED,
            body: json!({"id":"owned-once"}),
        },
    );
    drop(journal);
    let next = binding("same-owner", "43000000-0000-4000-8000-000000000043");
    let mut resumed = SeedJournal::open(path, identity, 1);
    resumed.record_session(&next);
    let (_, request, reply) = resumed.begin(
        "POST",
        "/works",
        json!({"operation_id":"new-must-not-be-used"}),
    );
    assert_eq!(request["operation_id"], "original");
    assert_eq!(reply.unwrap().body["id"], "owned-once");
    assert_eq!(resumed.value["operations"].as_array().unwrap().len(), 1);
    assert_eq!(
        resumed.value["validated_sessions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
#[should_panic(expected = "verified session actor differs from dataset owner")]
fn new_session_for_a_different_actor_cannot_replay_an_owned_dataset() {
    let root = PrivateRoot::new();
    let mut journal = SeedJournal::open(
        root.0.join("journal.json"),
        json!({"actor":"original-owner"}),
        0,
    );
    let fence = "42000000-0000-4000-8000-000000000042";
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-cantos-studio-session", fence.parse().unwrap());
    let other = SeedSessionBinding::checked(
        &headers,
        &json!({"actor_id":"other-owner"}),
        "other-owner",
        fence,
    )
    .unwrap();
    journal.record_session(&other);
}

#[test]
fn studio_cookie_name_is_closed_and_refuses_cookie_header_injection() {
    for token in ["", "opaque;evil=x", "opaque\r\nX-Evil: x", "two cookies"] {
        assert_eq!(
            SeedCookieName::Secure.header(token),
            Err("invalid Studio cookie value")
        );
    }

    assert_eq!(
        SeedCookieName::parse("__Secure-cantos_studio"),
        Ok(SeedCookieName::Secure)
    );
    assert_eq!(
        SeedCookieName::parse("cantos_session"),
        Ok(SeedCookieName::Development)
    );
    for value in [
        "",
        "arbitrary",
        "cantos_session;evil=x",
        "cantos_session\r\nX-Evil: x",
    ] {
        assert_eq!(
            SeedCookieName::parse(value),
            Err("unsupported Studio cookie name")
        );
    }
}

#[tokio::test]
async fn configured_studio_cookie_name_sends_one_exact_cookie_without_fallback() {
    let _media_lane = native_sql::media_lane().await;
    use axum::{extract::State, routing::get, Json};
    const FENCE: &str = "42000000-0000-4000-8000-000000000042";
    async fn session(
        State(expected): State<&'static str>,
        headers: axum::http::HeaderMap,
    ) -> (axum::http::HeaderMap, Json<Value>) {
        assert_eq!(headers.get_all("cookie").iter().count(), 1);
        assert_eq!(headers["cookie"], format!("{expected}=opaque-session"));
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-cantos-studio-session", FENCE.parse().unwrap());
        (headers, Json(json!({"actor_id":"same-owner"})))
    }
    for name in [SeedCookieName::Secure, SeedCookieName::Development] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/api/v1/studio/session", get(session))
            .with_state(name.as_str());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut remote = SeedRemote::new(origin, "https://studio.example.org".into(), name);
        assert_eq!(
            remote
                .bind_session("opaque-session", "same-owner", FENCE)
                .await,
            "same-owner"
        );
        remote
            .json(
                "GET",
                "/api/v1/studio/session",
                Some("opaque-session"),
                None,
            )
            .await
            .success(StatusCode::OK);
        server.abort();
    }
}
