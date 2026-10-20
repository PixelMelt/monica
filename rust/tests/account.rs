//! Profile and preferences, Pushover notification channels and account
//! users over HTTP.

mod common;

use common::TestApp;
use monica::testing::factory;
use monica::testing::pushover::FakePushover;
use serde_json::json;

async fn login(app: &mut TestApp, username: &str, password: &str) {
    app.clear_cookies();
    app.use_cookies();
    app.get("/auth/csrf").await.assert(204);
    app.post("/auth/login", json!({"username": username, "password": password})).await.assert(200);
}

#[tokio::test]
async fn changes_name_and_username() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    let other = factory::user(&app.state.db, None).await;
    login(&mut app, &user.username, factory::PASSWORD).await;

    let res = app.patch("/me", json!({"username": other.username.to_uppercase()})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["code"], "unique");
    app.patch("/me", json!({"username": "with:colon"})).await.assert(422);
    let me = app.patch("/me", json!({"first_name": "Rossy", "username": "Rossy"})).await.assert(200).json();
    assert_eq!((me["first_name"].as_str(), me["username"].as_str()), (Some("Rossy"), Some("rossy")));
    assert!(me.get("email").is_none());
    login(&mut app, "rossy", factory::PASSWORD).await;
    app.finish().await;
}

#[tokio::test]
async fn updates_preferences() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.username, factory::PASSWORD).await;

    let p = app
        .patch(
            "/me/preferences",
            json!({"name_order": "%last_name% %first_name%", "date_format": "DD/MM/YYYY", "timezone": "Europe/Paris",
                   "distance_format": "km", "default_map_site": "open_street_maps", "locale": "fr", "help_shown": false,
                   "contact_sort_order": "asc", "number_format": "1 234,56"}),
        )
        .await
        .assert(200)
        .json();
    assert_eq!(p["name_order"], "%last_name% %first_name%");
    assert_eq!(p["timezone"], "Europe/Paris");
    assert_eq!(p["locale"], "fr");
    assert_eq!(p["help_shown"], false);
    assert_eq!(p["contact_sort_order"], "asc");
    assert_eq!(app.get("/me/preferences").await.json()["date_format"], "DD/MM/YYYY");

    // The user's locale now drives messages.
    let res = app.patch("/me/preferences", json!({"date_format": "nope", "timezone": "Mars/Base"})).await.assert(422);
    let body = res.json();
    let fields: Vec<&str> = body["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap()).collect();
    assert_eq!(fields, ["date_format", "timezone"]);
    assert!(!res.json()["title"].as_str().unwrap().is_empty());
    app.finish().await;
}

#[tokio::test]
async fn manages_pushover_channels() {
    let pushover = FakePushover::start().await;
    let mut app = TestApp::with_config(|c| c.pushover_api_url = pushover.url()).await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.username, factory::PASSWORD).await;
    let base = "/me/notification-channels";

    let res = app.post(base, json!({"label": "Phone", "preferred_time": "25:00"})).await.assert(422).json();
    let fields: Vec<&str> = res["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap()).collect();
    assert_eq!(fields, ["user_key", "app_token", "preferred_time"]);
    let keys = |user_key: &str| json!({"label": "Phone", "user_key": user_key, "app_token": "a-monica", "preferred_time": "08:30"});
    let res = app.post(base, keys("u-invalid")).await.assert(422).json();
    assert_eq!(res["errors"][0]["field"], "user_key");
    assert_eq!(res["errors"][0]["message"], "Pushover refused these keys: user key is invalid");

    // Checked with Pushover, so verified and on at once; the token never comes back.
    let c = app.post(base, keys("u-phone")).await.assert(201).json();
    let id = c["id"].as_i64().unwrap();
    assert_eq!((c["type"].as_str(), c["user_key"].as_str()), (Some("pushover"), Some("u-phone")));
    assert_eq!(c["active"], true);
    assert!(c["verified_at"].is_string());
    assert_eq!(c["preferred_time"], "08:30");
    assert!(!c.to_string().contains("a-monica"));
    assert!(!app.get(base).await.json().to_string().contains("a-monica"));
    app.post(&format!("{base}/{id}/verify"), json!({"token": uuid::Uuid::new_v4()})).await.assert(404);

    let c = app.patch(&format!("{base}/{id}"), json!({"active": false})).await.assert(200).json();
    assert_eq!(c["active"], false);
    app.patch(&format!("{base}/{id}"), json!({"active": true})).await.assert(200);

    app.post(&format!("{base}/{id}/test"), json!({})).await.assert(204);
    let sent = pushover.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0]["token"].as_str(), sent[0]["user"].as_str()), ("a-monica", "u-phone"));
    let name = format!("{} {}", user.first_name.unwrap(), user.last_name.unwrap());
    assert_eq!(sent[0]["message"], format!("This is a test notification for {name}"));
    let logs = app.get(&format!("{base}/{id}/logs")).await.assert(200).json();
    assert_eq!(logs["meta"]["total"], 1);
    assert_eq!(logs["data"][0]["subject_line"], format!("This is a test notification for {name}"));

    pushover.go_down();
    let res = app.post(&format!("{base}/{id}/test"), json!({})).await.assert(502);
    assert_eq!(res.code(), "notification_failed");
    assert_eq!(res.json()["detail"], "Pushover could not be reached. Try again later.");

    app.delete(&format!("{base}/{id}")).await.assert(204);
    app.get(&format!("{base}/{id}")).await.assert(404);
    app.finish().await;
}

/// No email invitations: an administrator gives a new user a username and
/// a first password.
#[tokio::test]
async fn administrators_create_users() {
    let mut app = TestApp::new().await;
    let admin = factory::create_administrator(&app.state.db).await;
    login(&mut app, &admin.username, factory::PASSWORD).await;

    let body = json!({"first_name": "New", "last_name": "Person", "username": "New.Person", "password": "abcd",
                      "password_confirmation": "abcd", "is_account_administrator": true});
    let res = app.post("/account/users", json!({"username": "x"})).await.assert(422).json();
    let fields: Vec<&str> = res["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap()).collect();
    assert_eq!(fields, ["first_name", "last_name", "password"]);
    let created = app.post("/account/users", body.clone()).await.assert(201).json();
    assert_eq!(created["username"], "new.person");
    assert_eq!(created["is_account_administrator"], true);
    assert!(created.get("email").is_none() && created.get("is_invitation_pending").is_none());
    assert_eq!(app.post("/account/users", body).await.assert(422).json()["errors"][0]["code"], "unique");
    let users = app.get("/account/users").await.json()["data"].clone();
    assert_eq!(users.as_array().unwrap().len(), 2);

    login(&mut app, "new.person", "abcd").await;
    let me = app.get("/me").await.assert(200).json();
    assert_eq!(
        (me["first_name"].as_str(), me["account_id"].as_str()),
        (Some("New"), Some(admin.account_id.to_string().as_str()))
    );
    app.clear_cookies();
    app.get(&format!("/invitations/{}", uuid::Uuid::new_v4())).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn administrators_manage_users() {
    let mut app = TestApp::new().await;
    let admin = factory::create_administrator(&app.state.db).await;
    let member = factory::user(&app.state.db, Some(admin.account_id)).await;
    let outsider = factory::user(&app.state.db, None).await;

    login(&mut app, &member.username, factory::PASSWORD).await;
    app.get("/account/users").await.assert(200);
    app.post("/account/users", json!({"username": "x"})).await.assert(403);
    app.patch(&format!("/account/users/{}", member.id), json!({"is_account_administrator": true})).await.assert(403);

    login(&mut app, &admin.username, factory::PASSWORD).await;
    app.get(&format!("/account/users/{}", outsider.id)).await.assert(404);
    let u = app
        .patch(&format!("/account/users/{}", member.id), json!({"is_account_administrator": true}))
        .await
        .assert(200)
        .json();
    assert_eq!(u["is_account_administrator"], true);
    app.patch(&format!("/account/users/{}", admin.id), json!({"is_account_administrator": false})).await.assert(409);

    let path = format!("/account/users/{}", member.id);
    let res = app.request(axum::http::Method::DELETE, &path, Some(json!({}))).await.assert(403);
    assert_eq!(res.code(), "password_confirmation_required");
    app.request(axum::http::Method::DELETE, &path, Some(json!({"password": factory::PASSWORD}))).await.assert(204);
    app.get(&path).await.assert(404);
    let own = format!("/account/users/{}", admin.id);
    app.request(axum::http::Method::DELETE, &own, Some(json!({"password": factory::PASSWORD}))).await.assert(409);
    app.finish().await;
}

/// No storage quota and no account cancellation on a single-user install.
#[tokio::test]
async fn storage_and_cancellation_are_gone() {
    let mut app = TestApp::new().await;
    let admin = factory::create_administrator(&app.state.db).await;
    login(&mut app, &admin.username, factory::PASSWORD).await;
    let account = app.get("/account").await.assert(200).json();
    assert_eq!(account["id"], admin.account_id.to_string());
    assert!(account.get("storage_limit_in_mb").is_none());
    app.get("/account/storage").await.assert(404);
    app.request(axum::http::Method::DELETE, "/account", Some(json!({"password": factory::PASSWORD}))).await.assert(405);
    app.get("/me").await.assert(200);
    app.finish().await;
}
