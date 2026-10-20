//! Preferences, notification channels, account users, invitations and
//! account cancellation over HTTP.

mod common;

use common::TestApp;
use monica::testing::factory;
use serde_json::json;

async fn login(app: &mut TestApp, email: &str, password: &str) {
    app.clear_cookies();
    app.use_cookies();
    app.get("/auth/csrf").await.assert(204);
    app.post("/auth/login", json!({"email": email, "password": password})).await.assert(200);
}

#[tokio::test]
async fn updates_preferences() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.email, factory::PASSWORD).await;

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
async fn manages_notification_channels() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.email, factory::PASSWORD).await;

    app.post("/me/notification-channels", json!({"label": "Work", "content": "nope", "preferred_time": "25:00"}))
        .await
        .assert(422);
    let c = app
        .post(
            "/me/notification-channels",
            json!({"label": "Work", "content": "work@example.com", "preferred_time": "08:30"}),
        )
        .await
        .assert(201)
        .json();
    let id = c["id"].as_i64().unwrap();
    assert_eq!(c["active"], false);
    assert!(c["verified_at"].is_null());
    assert_eq!(c["preferred_time"], "08:30");

    let mail = app.state.mailer.sent().pop().unwrap();
    assert_eq!(mail.to, "work@example.com");
    let token = mail.text.split(&format!("/settings/notifications/{id}/verify/")).nth(1).unwrap()[..36].to_owned();
    app.post(&format!("/me/notification-channels/{id}/verify"), json!({"token": uuid::Uuid::new_v4()}))
        .await
        .assert(422);
    let c =
        app.post(&format!("/me/notification-channels/{id}/verify"), json!({"token": token})).await.assert(200).json();
    assert!(c["verified_at"].is_string());

    let c = app.patch(&format!("/me/notification-channels/{id}"), json!({"active": true})).await.assert(200).json();
    assert_eq!(c["active"], true);
    app.post(&format!("/me/notification-channels/{id}/test"), json!({})).await.assert(204);
    let logs = app.get(&format!("/me/notification-channels/{id}/logs")).await.assert(200).json();
    assert_eq!(logs["meta"]["total"], 1);
    assert_eq!(logs["data"].as_array().unwrap().len(), 1);
    assert_eq!(app.get("/me/notification-channels").await.json()["data"].as_array().unwrap().len(), 1);

    app.delete(&format!("/me/notification-channels/{id}")).await.assert(204);
    app.get(&format!("/me/notification-channels/{id}")).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn invites_users_who_accept_and_log_in() {
    let mut app = TestApp::new().await;
    let admin = factory::create_administrator(&app.state.db).await;
    login(&mut app, &admin.email, factory::PASSWORD).await;

    let invited = app
        .post("/account/users", json!({"email": "new@example.com", "is_account_administrator": true}))
        .await
        .assert(201)
        .json();
    assert_eq!(invited["is_invitation_pending"], true);
    app.post("/account/users", json!({"email": "new@example.com"})).await.assert(422);
    assert_eq!(app.get("/account/users").await.json()["data"].as_array().unwrap().len(), 2);

    let mail = app.state.mailer.sent().pop().unwrap();
    assert_eq!(mail.to, "new@example.com");
    let code = mail.text.split("/invitation/").nth(1).unwrap()[..36].to_owned();

    app.clear_cookies();
    assert_eq!(app.get(&format!("/invitations/{code}")).await.assert(200).json()["email"], "new@example.com");
    app.get(&format!("/invitations/{}", uuid::Uuid::new_v4())).await.assert(404);
    app.get("/auth/csrf").await.assert(204);
    let body = json!({"first_name": "New", "last_name": "Person", "password": "abcd", "password_confirmation": "abcd"});
    let me = app.post(&format!("/invitations/{code}/accept"), body.clone()).await.assert(200).json();
    assert_eq!(me["email"], "new@example.com");
    assert!(me["email_verified_at"].is_string());
    assert_eq!(me["is_account_administrator"], true, "Q5: invited as administrator");
    assert_eq!(app.get("/me").await.assert(200).json()["first_name"], "New");
    app.post(&format!("/invitations/{code}/accept"), body).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn administrators_manage_users() {
    let mut app = TestApp::new().await;
    let admin = factory::create_administrator(&app.state.db).await;
    let member = factory::user(&app.state.db, Some(admin.account_id)).await;
    let outsider = factory::user(&app.state.db, None).await;

    login(&mut app, &member.email, factory::PASSWORD).await;
    app.get("/account/users").await.assert(200);
    app.post("/account/users", json!({"email": "x@example.com"})).await.assert(403);
    app.patch(&format!("/account/users/{}", member.id), json!({"is_account_administrator": true})).await.assert(403);

    login(&mut app, &admin.email, factory::PASSWORD).await;
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

    let storage = app.get("/account/storage").await.assert(200).json();
    assert_eq!(storage["total_size"], 0);
    assert_eq!(storage["storage_limit_in_mb"], 10);
    app.finish().await;
}

#[tokio::test]
async fn cancels_the_account() {
    let mut app = TestApp::new().await;
    let admin = factory::create_administrator(&app.state.db).await;
    login(&mut app, &admin.email, factory::PASSWORD).await;
    app.post("/vaults", json!({"name": "Mine"})).await.assert(201);

    app.request(axum::http::Method::DELETE, "/account", Some(json!({"password": "nope"}))).await.assert(422);
    app.request(axum::http::Method::DELETE, "/account", Some(json!({"password": factory::PASSWORD}))).await.assert(202);
    app.get("/me").await.assert(401);
    app.get("/auth/csrf").await.assert(204);
    app.post("/auth/login", json!({"email": admin.email, "password": factory::PASSWORD})).await.assert(422);
    app.finish().await;
}

#[tokio::test]
async fn self_deletion_is_off_by_default() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.email, factory::PASSWORD).await;
    app.request(axum::http::Method::DELETE, "/me", Some(json!({"password": factory::PASSWORD}))).await.assert(404);
    app.finish().await;

    let mut app = TestApp::with_config(|c| c.enable_account_deletion = true).await;
    let admin = factory::create_administrator(&app.state.db).await;
    login(&mut app, &admin.email, factory::PASSWORD).await;
    app.request(axum::http::Method::DELETE, "/me", Some(json!({"password": factory::PASSWORD}))).await.assert(409);
    let user = factory::user(&app.state.db, Some(admin.account_id)).await;
    login(&mut app, &user.email, factory::PASSWORD).await;
    app.request(axum::http::Method::DELETE, "/me", Some(json!({"password": factory::PASSWORD}))).await.assert(204);
    app.get("/me").await.assert(401);
    app.finish().await;
}
