//! Test-harness-only: a disposable PostgreSQL database with the production schema, grants and
//! runtime role, and the production router with Identity-backed sessions mounted. Requests are
//! dispatched in process; the socket peer is declared per request like a real connection.
#![allow(dead_code)]

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header, HeaderMap, Method, Request, StatusCode},
    Router,
};
use cantos_server::{
    adapters::inbound::http::{api_router, AppState},
    platform::{
        identity_auth::{config::IdentityAuthConfig, decision::Subject, IdentityAuth},
        postgres::{local_config, migrate, Store},
        identity_auth::PeerPolicy,
    },
};
use http_body_util::BodyExt;
use std::{env, net::SocketAddr, path::PathBuf};
use tokio_postgres::Config;
use tower::ServiceExt;

pub async fn connect(config: &Config) -> tokio_postgres::Client {
    let (client, connection) = config.connect(tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

pub struct Database {
    pub admin: tokio_postgres::Client,
    /// The operator role's configuration (explicit local operator tools, never the HTTP role).
    pub operator: Config,
    pub runtime: Config,
}

impl Database {
    /// A fresh database per test, migrated by the operator role and used by the runtime role.
    pub async fn new() -> Self {
        let mut config = local_config(
            &env::var("CANTOS_TEST_CLUSTER_URL").expect("cargo xtask verify postgres"),
        )
        .unwrap();
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let name = format!("cantos_test_{}", uuid::Uuid::new_v4().simple());
        connect(&config)
            .await
            .batch_execute(&format!("CREATE DATABASE {name}"))
            .await
            .unwrap();
        config.dbname(&name);
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        admin
            .batch_execute(include_str!(
                "../../../../infra/vps/postgres/runtime-grants.sql"
            ))
            .await
            .unwrap();
        let operator = config.clone();
        config.user("cantos_app");
        Self {
            admin,
            operator,
            runtime: config,
        }
    }

    pub fn store(&self) -> Store {
        Store::new(self.runtime.clone())
            .unwrap()
            .without_development_sessions()
    }
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub json: serde_json::Value,
}

impl Reply {
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
        assert_eq!(cookies.len(), 1, "{cookies:?}");
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
