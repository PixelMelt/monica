//! Integration test helpers: the browser-like [`TestApp`]. The database
//! harness and factories are `monica::testing`.

#![allow(dead_code)]

pub use monica::testing::TestDb;

// ---------------------------------------------------------------- HTTP

use std::collections::HashMap;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use monica::config::Config;
use monica::http::state::AppState;
use serde_json::Value;
use tower::ServiceExt;

/// The app on a fresh database, driven like a browser (cookies, CSRF
/// header) or, after [`TestApp::use_token`], like a bearer-token client.
pub struct TestApp {
    pub state: AppState,
    router: Router,
    cookies: HashMap<String, String>,
    bearer: Option<String>,
    db: Option<TestDb>,
    data_dir: std::path::PathBuf,
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: Vec<u8>,
}

impl TestResponse {
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    /// Panics with the body unless the status is `expected`.
    pub fn assert(self, expected: u16) -> Self {
        assert_eq!(self.status.as_u16(), expected, "body: {}", String::from_utf8_lossy(&self.body));
        self
    }

    /// The problem `code` of an error response.
    pub fn code(&self) -> String {
        self.json()["code"].as_str().unwrap_or_default().to_owned()
    }
}

impl TestApp {
    pub async fn new() -> TestApp {
        TestApp::with_config(|_| {}).await
    }

    pub async fn with_config(f: impl FnOnce(&mut Config)) -> TestApp {
        let db = TestDb::migrated().await;
        let data_dir = std::env::temp_dir().join(format!("monica_data_{}", uuid::Uuid::new_v4().simple()));
        let mut config = Config::for_tests("", data_dir.clone());
        f(&mut config);
        let state = AppState::new(db.conn.clone(), config).expect("state");
        let router = monica::http::app(state.clone());
        TestApp { state, router, cookies: HashMap::new(), bearer: None, db: Some(db), data_dir }
    }

    /// Sends requests with `Authorization: Bearer <token>` and no cookies.
    pub fn use_token(&mut self, token: &str) {
        self.bearer = Some(token.to_owned());
    }

    /// Back to the browser (cookie) client.
    pub fn use_cookies(&mut self) {
        self.bearer = None;
    }

    pub fn cookie(&self, name: &str) -> Option<&str> {
        self.cookies.get(name).map(String::as_str)
    }

    pub fn clear_cookies(&mut self) {
        self.cookies.clear();
    }

    pub fn remove_cookie(&mut self, name: &str) {
        self.cookies.remove(name);
    }

    pub async fn request(&mut self, method: Method, path: &str, body: Option<Value>) -> TestResponse {
        self.request_with(method, path, body, &[]).await
    }

    pub async fn request_with(
        &mut self,
        method: Method,
        path: &str,
        body: Option<Value>,
        headers: &[(&str, &str)],
    ) -> TestResponse {
        let uri = if path.starts_with("/api/")
            || !path.starts_with('/')
            || path.starts_with("/up")
            || path.starts_with("/.well-known")
        {
            path.to_owned()
        } else {
            format!("/api/v1{path}")
        };
        let mut req = Request::builder().method(method.clone()).uri(uri);
        if let Some(token) = &self.bearer {
            req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
        } else {
            if !self.cookies.is_empty() {
                let c = self.cookies.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ");
                req = req.header(header::COOKIE, c);
            }
            if !matches!(method, Method::GET | Method::HEAD)
                && let Some(xsrf) = self.cookies.get("XSRF-TOKEN")
            {
                req = req.header("x-xsrf-token", xsrf.clone());
            }
        }
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let req = match body {
            Some(b) => req.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())).unwrap(),
            None => req.body(Body::empty()).unwrap(),
        };
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        if self.bearer.is_none() {
            for v in headers.get_all(header::SET_COOKIE) {
                let raw = v.to_str().unwrap();
                let (pair, attrs) = raw.split_once(';').unwrap_or((raw, ""));
                let (k, val) = pair.split_once('=').unwrap();
                if val.is_empty() || attrs.to_lowercase().contains("max-age=0") {
                    self.cookies.remove(k);
                } else {
                    self.cookies.insert(k.to_owned(), val.to_owned());
                }
            }
        }
        let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
        TestResponse { status, headers, body }
    }

    pub async fn get(&mut self, path: &str) -> TestResponse {
        self.request(Method::GET, path, None).await
    }

    pub async fn post(&mut self, path: &str, body: Value) -> TestResponse {
        self.request(Method::POST, path, Some(body)).await
    }

    pub async fn put(&mut self, path: &str, body: Value) -> TestResponse {
        self.request(Method::PUT, path, Some(body)).await
    }

    pub async fn patch(&mut self, path: &str, body: Value) -> TestResponse {
        self.request(Method::PATCH, path, Some(body)).await
    }

    pub async fn delete(&mut self, path: &str) -> TestResponse {
        self.request(Method::DELETE, path, None).await
    }

    pub async fn finish(mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
        if let Some(db) = self.db.take() {
            db.drop().await;
        }
    }
}
