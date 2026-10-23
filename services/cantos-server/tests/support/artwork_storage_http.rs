//! Real loopback transport for file-backed Turso/private-filesystem artwork tests.
//! No listener, database, identity provider or storage process is started by ordinary tests.
use std::time::Duration;

use axum::{http::StatusCode, Router};
use reqwest::{header::HeaderMap, Client, Method};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use tokio::{net::TcpListener, task::JoinHandle};

pub struct HttpHost {
    pub client: Client,
    pub base: String,
    task: JoinHandle<()>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl HttpHost {
    pub async fn start(app: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (shutdown, stopping) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopping.await;
                })
                .await
                .unwrap();
        });
        Self {
            client: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(20))
                .build()
                .unwrap(),
            base,
            task,
            shutdown: Some(shutdown),
        }
    }

    pub async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        (&mut self.task).await.unwrap();
    }
    pub async fn json<T: Serialize>(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        fence: Option<&str>,
        body: Option<&T>,
    ) -> Reply {
        request(
            &self.client,
            &self.base,
            method,
            path,
            token,
            fence,
            Some("application/json"),
            body.map_or_else(Vec::new, |value| serde_json::to_vec(value).unwrap()),
            &[],
        )
        .await
    }
}

impl Drop for HttpHost {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub bytes: Vec<u8>,
}

impl Reply {
    pub fn success<T: DeserializeOwned>(&self, expected: StatusCode) -> T {
        assert_eq!(self.status, expected, "{:?}", self.bytes);
        assert_eq!(self.headers["content-type"], "application/json");
        serde_json::from_slice(&self.bytes).unwrap()
    }

    pub fn error(&self, expected: StatusCode, code: &str) -> Value {
        let value: Value = self.success(expected);
        assert_eq!(value["code"], code);
        value
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn request(
    client: &Client,
    base: &str,
    method: &str,
    path: &str,
    token: Option<&str>,
    fence: Option<&str>,
    content_type: Option<&str>,
    body: Vec<u8>,
    extra: &[(&str, &str)],
) -> Reply {
    let mut request = client
        .request(
            Method::from_bytes(method.as_bytes()).unwrap(),
            format!("{base}{path}"),
        )
        // The Studio host is configured with this exact loopback Origin. It is independent
        // of the ephemeral API listener port, as with a separately hosted Studio client.
        .header(
            "origin",
            extra
                .iter()
                .find(|(name, _)| *name == "origin")
                .map_or("http://127.0.0.1:8080", |(_, value)| *value),
        );
    if let Some(token) = token {
        request = request.header("cookie", format!("cantos_session={token}"));
    }
    if let Some(fence) = fence {
        request = request.header("x-cantos-studio-session", fence);
    }
    if let Some(content_type) = content_type {
        request = request.header("content-type", content_type);
    }
    for (name, value) in extra {
        if *name == "origin" {
            continue;
        }
        request = request.header(*name, *value);
    }
    let response = request.body(body).send().await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["referrer-policy"], "no-referrer");
    Reply {
        status,
        headers,
        bytes: response.bytes().await.unwrap().to_vec(),
    }
}
