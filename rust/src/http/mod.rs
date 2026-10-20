//! The HTTP application: `/api/v1`, the redirects and health check outside
//! it, and the SPA.

pub mod access;
pub mod auth;
pub mod files;
pub mod json;
pub mod ratelimit;
pub mod session;
pub mod state;
pub mod tokens;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware::from_fn_with_state;
use axum::response::Redirect;
use axum::routing::get;
use tower_http::services::{ServeDir, ServeFile};

use crate::error::ApiError;
use state::AppState;

/// The whole server.
pub fn app(state: AppState) -> Router {
    let (api, _) = crate::api::router().split_for_parts();
    // Uploads are checked against MAX_UPLOAD_SIZE in `files::receive`; the
    // body limit leaves room for the multipart framing.
    let body_limit = usize::try_from(state.config.max_upload_size_mb * 1024 * 1024 + 64 * 1024).unwrap_or(usize::MAX);
    let api = api
        .fallback(|| async { ApiError::not_found() })
        .layer(from_fn_with_state(state.clone(), session::layer))
        .layer(DefaultBodyLimit::max(body_limit));
    let mut app = Router::new()
        .nest("/api/v1", api)
        .route("/up", get(|| async { "OK" }))
        .route("/.well-known/carddav", get(|| async { Redirect::permanent("/dav") }))
        .route("/.well-known/caldav", get(|| async { Redirect::permanent("/dav") }))
        .route("/.well-known/security.txt", get(|| async { Redirect::permanent("/security.txt") }));
    if let Some(dir) = &state.config.spa_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    }
    app.with_state(state)
}
