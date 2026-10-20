//! A fake Pushover API for tests: it records the messages it gets, accepts
//! any token and user key except user keys containing `invalid`, and can be
//! told to fail like an unreachable server.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use axum::Form;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::json;

/// A message the fake received: its form fields (`token`, `user`, `title`,
/// `message`).
pub type Sent = HashMap<String, String>;

#[derive(Default)]
struct Inner {
    sent: Mutex<Vec<Sent>>,
    down: AtomicBool,
}

pub struct FakePushover {
    url: String,
    inner: Arc<Inner>,
}

impl FakePushover {
    pub async fn start() -> FakePushover {
        let inner = Arc::new(Inner::default());
        let app = Router::new()
            .route("/1/messages.json", post(messages))
            .route("/1/users/validate.json", post(validate))
            .with_state(inner.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let url = format!("http://{}/1", listener.local_addr().expect("address"));
        tokio::spawn(async move { axum::serve(listener, app).await.expect("fake Pushover") });
        FakePushover { url, inner }
    }

    /// The `PUSHOVER_API_URL` to use.
    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn sent(&self) -> Vec<Sent> {
        self.inner.sent.lock().unwrap().clone()
    }

    /// Answers every request with a 503 and no JSON from now on.
    pub fn go_down(&self) {
        self.inner.down.store(true, Ordering::SeqCst);
    }
}

fn answer(inner: &Inner, form: &Sent) -> Option<Response> {
    if inner.down.load(Ordering::SeqCst) {
        return Some((StatusCode::SERVICE_UNAVAILABLE, "down").into_response());
    }
    if form.get("user").is_none_or(|u| u.contains("invalid")) {
        let body = json!({"user": "invalid", "errors": ["user key is invalid"], "status": 0});
        return Some((StatusCode::BAD_REQUEST, axum::Json(body)).into_response());
    }
    None
}

async fn messages(State(inner): State<Arc<Inner>>, Form(form): Form<Sent>) -> Response {
    if let Some(res) = answer(&inner, &form) {
        return res;
    }
    inner.sent.lock().unwrap().push(form);
    axum::Json(json!({"status": 1, "request": "647d2300-702c-4b38-8b2f-d56326ae460b"})).into_response()
}

async fn validate(State(inner): State<Arc<Inner>>, Form(form): Form<Sent>) -> Response {
    if let Some(res) = answer(&inner, &form) {
        return res;
    }
    axum::Json(json!({"status": 1, "devices": ["phone"], "request": "1f4a3b2c"})).into_response()
}
