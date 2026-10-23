//! Test-only native Server/Identity HTTP world. One real, private server.db and the production
//! router; no development session is admitted. Requests declare their socket peer. Turso has
//! no table roles: raw fixture SQL is a privileged test observation, never HTTP authority.
#![allow(dead_code)]

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header, HeaderMap, Method, Request, StatusCode},
    Router,
};
use cantos_server::test_support::ServerFixture;
use cantos_server::{
    adapters::inbound::http::{api_router, AppState},
    platform::{
        identity_auth::PeerPolicy,
        identity_auth::{config::IdentityAuthConfig, decision::Subject, IdentityAuth},
        store::{Params, Row, Store, StoreError},
    },
};
use http_body_util::BodyExt;
use std::{net::SocketAddr, path::PathBuf};
use tower::ServiceExt;

pub struct Database {
    pub admin: ServerFixture,
}

impl Database {
    pub async fn new() -> Self {
        Self {
            admin: ServerFixture::new().await,
        }
    }

    pub fn store(&self) -> Store {
        self.admin.store().clone().without_development_sessions()
    }
}

// SQL in callers is native and parameterized; this trait only keeps single-row assertions terse.
pub trait FixtureQueries {
    async fn query_one(&self, sql: &str, params: Params<'_>) -> Result<Row, StoreError>;
    async fn query(&self, sql: &str, params: Params<'_>) -> Result<Vec<Row>, StoreError>;
    async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64, StoreError>;
}

impl FixtureQueries for ServerFixture {
    async fn query_one(&self, sql: &str, params: Params<'_>) -> Result<Row, StoreError> {
        let mut rows = self.rows(sql, params).await?;
        assert_eq!(rows.len(), 1);
        Ok(rows.remove(0))
    }
    async fn query(&self, sql: &str, params: Params<'_>) -> Result<Vec<Row>, StoreError> {
        self.rows(sql, params).await
    }
    async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64, StoreError> {
        self.exec(sql, params).await
    }
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub json: serde_json::Value,
}

impl Reply {
    /// A top-level field the contract promises. Indexing a missing key yields `null`, so an
    /// equality between two replies on a misspelled name would pass vacuously (`null == null`);
    /// this fails at the first reply instead. The literal at the call site stays the oracle.
    pub fn field(&self, name: &str) -> &serde_json::Value {
        self.json
            .get(name)
            .unwrap_or_else(|| panic!("the reply has no `{name}` field"))
    }

    pub fn set_cookies(&self) -> Vec<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_owned())
            .collect()
    }

    /// `name=value` of the single Set-Cookie of this reply, as a browser would store it.
    pub fn cookie(&self) -> (String, String) {
        let cookies = self.set_cookies();
        // Counted, never printed: a Set-Cookie value is a live session secret.
        assert_eq!(cookies.len(), 1, "expected exactly one Set-Cookie header");
        let pair = cookies[0].split(';').next().unwrap();
        let (name, value) = pair.split_once('=').unwrap();
        (name.to_owned(), value.to_owned())
    }
}

pub struct App {
    router: Router,
    pub origin: &'static str,
}

impl App {
    /// Drop the router's file-owner clone before the fixture reopens server.db.
    pub fn release(&mut self) {
        self.router = Router::new();
    }

    pub fn new(
        db: &Database,
        origin: &'static str,
        socket: PathBuf,
        peer_uid: u32,
        studio: &[Subject],
    ) -> Self {
        let identity = IdentityAuth::new(
            IdentityAuthConfig {
                socket,
                peer_uid,
                peer_policy: PeerPolicy::DirectPeer,
                studio_subjects: studio.to_vec(),
            },
            origin.starts_with("https://"),
        );
        let store = db.store().with_identity_auth(identity);
        let router = api_router(AppState {
            store,
            web_origin: origin.to_owned(),
        });
        Self { router, origin }
    }

    pub async fn send(
        &self,
        method: Method,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<serde_json::Value>,
    ) -> Reply {
        let mut builder = Request::builder().method(method).uri(path);
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let body = match body {
            Some(value) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(value.to_string())
            }
            None => Body::empty(),
        };
        let mut request = builder.body(body).unwrap();
        request.extensions_mut().insert(ConnectInfo(
            "203.0.113.9:44321".parse::<SocketAddr>().unwrap(),
        ));
        let response = self.router.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        let json = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        Reply {
            status: parts.status,
            headers: parts.headers,
            json,
        }
    }

    pub async fn post(
        &self,
        path: &str,
        headers: &[(&str, &str)],
        body: serde_json::Value,
    ) -> Reply {
        self.send(Method::POST, path, headers, Some(body)).await
    }

    pub async fn get(&self, path: &str, headers: &[(&str, &str)]) -> Reply {
        self.send(Method::GET, path, headers, None).await
    }
}
