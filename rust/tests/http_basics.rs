//! The HTTP shell: instance info, CSRF, errors, redirects.

mod common;

use common::TestApp;
use serde_json::json;

#[tokio::test]
async fn instance_is_public_and_lists_locales() {
    let mut app = TestApp::new().await;
    let res = app.get("/instance").await.assert(200);
    let body = res.json();
    assert_eq!(body["signup_enabled"], true);
    assert_eq!(body["locale"], "en");
    assert_eq!(body["locales"].as_array().unwrap().len(), 29);
    assert!(body["locales"].as_array().unwrap().iter().any(|l| l["id"] == "ar" && l["dir"] == "rtl"));
    assert_eq!(body["features"]["account_deletion"], false);

    let fr =
        app.request_with(axum::http::Method::GET, "/instance", None, &[("accept-language", "fr-FR,fr;q=0.9")]).await;
    assert_eq!(fr.json()["locale"], "fr");
    app.finish().await;
}

#[tokio::test]
async fn unsafe_requests_need_the_csrf_token() {
    let mut app = TestApp::new().await;
    // No session yet: rejected.
    let res = app.post("/auth/login", json!({"email": "a@b.c", "password": "x"})).await.assert(403);
    assert_eq!(res.code(), "csrf_token_mismatch");

    app.get("/auth/csrf").await.assert(204);
    assert!(app.cookie("XSRF-TOKEN").is_some_and(|t| t.len() == 40));
    assert!(app.cookie("monica_session").is_some());

    // A wrong header is rejected too.
    app.remove_cookie("XSRF-TOKEN");
    let res =
        app.request_with(axum::http::Method::POST, "/auth/logout", Some(json!({})), &[("x-xsrf-token", "nope")]).await;
    assert_eq!(res.status.as_u16(), 403);
    app.finish().await;
}

#[tokio::test]
async fn unknown_api_paths_are_problem_404s_and_well_known_redirects() {
    let mut app = TestApp::new().await;
    let res = app.get("/nope").await.assert(404);
    assert_eq!(res.headers["content-type"], "application/problem+json");
    assert_eq!(res.code(), "not_found");
    let res = app.get("/.well-known/carddav").await.assert(308);
    assert_eq!(res.headers["location"], "/dav");
    app.get("/up").await.assert(200);
    app.finish().await;
}
