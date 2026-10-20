//! Login by username, 2FA, tokens and the first-run setup over HTTP.

mod common;

use axum::http::Method;
use common::TestApp;
use monica::auth::two_factor;
use monica::testing::factory;
use sea_orm::EntityTrait;
use serde_json::json;

async fn login(app: &mut TestApp, username: &str, password: &str) -> common::TestResponse {
    app.get("/auth/csrf").await.assert(204);
    app.post("/auth/login", json!({"username": username, "password": password})).await
}

#[tokio::test]
async fn logs_in_and_out() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;

    let res = login(&mut app, &user.username, "wrong").await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "username");
    assert_eq!(res.json()["errors"][0]["message"], "These credentials do not match our records.");
    app.get("/me").await.assert(401);

    // Usernames ignore case.
    let res = login(&mut app, &format!(" {} ", user.username.to_uppercase()), factory::PASSWORD).await.assert(200);
    assert_eq!(res.json()["two_factor"], false);
    assert_eq!(res.json()["user"]["username"], user.username.as_str());
    let me = app.get("/me").await.assert(200).json();
    assert_eq!(me["id"], user.id.to_string());
    assert_eq!(me["preferences"]["name_order"], "%first_name% %last_name%");

    app.post("/auth/logout", json!({})).await.assert(204);
    app.get("/me").await.assert(401);
    app.finish().await;
}

#[tokio::test]
async fn login_is_rate_limited() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    for _ in 0..5 {
        login(&mut app, &user.username, "wrong").await.assert(422);
    }
    let res = login(&mut app, &user.username, factory::PASSWORD).await.assert(429);
    assert!(res.headers.contains_key("retry-after"));
    app.finish().await;
}

#[tokio::test]
async fn two_factor_enable_confirm_and_challenge() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.username, factory::PASSWORD).await.assert(200);

    // Sensitive: without a confirmation or the password in the body it's refused.
    let res = app.post("/me/two-factor", json!({})).await.assert(403);
    assert_eq!(res.code(), "password_confirmation_required");
    app.post("/auth/confirm-password", json!({"password": "nope"})).await.assert(422);
    app.post("/auth/confirm-password", json!({"password": factory::PASSWORD})).await.assert(204);
    assert_eq!(app.get("/auth/confirmed-password-status").await.json()["confirmed"], true);
    // Jetstream's `seconds`: the confirmation is older than an empty window.
    assert_eq!(app.get("/auth/confirmed-password-status?seconds=0").await.json()["confirmed"], false);

    let setup = app.post("/me/two-factor", json!({})).await.assert(200).json();
    let secret = setup["secret"].as_str().unwrap().to_owned();
    assert!(setup["qr_code_svg"].as_str().unwrap().contains("<svg"));
    assert_eq!(setup["recovery_codes"].as_array().unwrap().len(), 8);
    assert_eq!(app.get("/me").await.json()["two_factor_enabled"], false, "not before confirmation");

    let now = chrono::Utc::now().timestamp() as u64;
    app.post("/me/two-factor/confirm", json!({"code": "000000"})).await.assert(422);
    app.post("/me/two-factor/confirm", json!({"code": two_factor::code_at(&secret, now)})).await.assert(204);
    assert_eq!(app.get("/me").await.json()["two_factor_enabled"], true);
    let codes = app.get("/me/two-factor/recovery-codes").await.assert(200).json()["recovery_codes"].clone();

    app.post("/auth/logout", json!({})).await.assert(204);
    let res = login(&mut app, &user.username, factory::PASSWORD).await.assert(200);
    assert_eq!(res.json()["two_factor"], true);
    assert!(res.json()["user"].is_null());
    app.get("/me").await.assert(401);

    // The code for the next time step (the current one was used to confirm).
    let next = two_factor::code_at(&secret, now + 30);
    let me = app.post("/auth/two-factor-challenge", json!({"code": next})).await.assert(200).json();
    assert_eq!(me["username"], user.username.as_str());

    // A recovery code works once.
    app.post("/auth/logout", json!({})).await.assert(204);
    login(&mut app, &user.username, factory::PASSWORD).await.assert(200);
    let code = codes[0].as_str().unwrap();
    app.post("/auth/two-factor-challenge", json!({"recovery_code": code})).await.assert(200);
    app.post("/auth/logout", json!({})).await.assert(204);
    login(&mut app, &user.username, factory::PASSWORD).await.assert(200);
    app.post("/auth/two-factor-challenge", json!({"recovery_code": code})).await.assert(422);

    app.finish().await;
}

#[tokio::test]
async fn tokens_authenticate_with_their_abilities() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.username, factory::PASSWORD).await.assert(200);

    let res = app.post("/me/tokens", json!({"name": "phone", "abilities": ["read"]})).await.assert(201).json();
    let read_only = res["plain_text_token"].as_str().unwrap().to_owned();
    assert_eq!(res["token"]["abilities"], json!(["read"]));
    let rw = app.post("/me/tokens", json!({"name": "cli", "abilities": ["read", "write"]})).await.assert(201).json();
    let rw = rw["plain_text_token"].as_str().unwrap().to_owned();
    app.post("/me/tokens", json!({"name": "bad", "abilities": ["admin"]})).await.assert(422);
    assert_eq!(app.get("/me/tokens").await.json()["data"].as_array().unwrap().len(), 2);

    app.use_token(&read_only);
    app.get("/me").await.assert(200);
    let res = app.patch("/me", json!({"first_name": "X"})).await.assert(403);
    assert_eq!(res.code(), "token_ability_missing");

    app.use_token(&rw);
    app.patch("/me", json!({"first_name": "Rossy"})).await.assert(200);
    // Writes with a bearer token need no CSRF header, but tokens can't mint tokens.
    app.post("/me/tokens", json!({"name": "more"})).await.assert(403);

    app.use_token("1|not-a-token");
    app.get("/me").await.assert(401);
    app.finish().await;
}

/// No email: the verification and password reset flows are gone (a
/// forgotten password is reset with `monica reset-password`), and so is
/// deleting your own user.
#[tokio::test]
async fn the_email_flows_and_self_deletion_are_gone() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    app.get("/auth/csrf").await.assert(204);
    for path in ["/auth/forgot-password", "/auth/reset-password", "/auth/email/verify"] {
        app.post(path, json!({"email": "a@example.com"})).await.assert(404);
    }
    app.login(&user.username).await;
    app.post("/auth/email/verification-notification", json!({})).await.assert(404);
    app.request(Method::DELETE, "/me", Some(json!({"password": factory::PASSWORD}))).await.assert(405);
    app.get("/me/tokens").await.assert(200);
    app.finish().await;
}

#[tokio::test]
async fn changes_password_and_lists_sessions() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.username, factory::PASSWORD).await.assert(200);
    let sessions = app.get("/me/sessions").await.assert(200).json();
    assert_eq!(sessions["data"].as_array().unwrap().len(), 1);
    assert_eq!(sessions["data"][0]["is_current_device"], true);

    let res = app
        .put("/me/password", json!({"current_password": "nope", "password": "abcd", "password_confirmation": "abcd"}))
        .await
        .assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "current_password");
    app.put(
        "/me/password",
        json!({"current_password": factory::PASSWORD, "password": "abcd", "password_confirmation": "abcd"}),
    )
    .await
    .assert(204);
    app.get("/me").await.assert(200);
    app.finish().await;
}

#[tokio::test]
async fn bootstraps_the_layout() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.username, factory::PASSWORD).await.assert(200);
    app.post("/vaults", json!({"name": "Zeta"})).await.assert(201);
    app.post("/vaults", json!({"name": "alpha"})).await.assert(201);
    let b = app.get("/me/bootstrap").await.assert(200).json();
    assert_eq!(b["user"]["id"], user.id.to_string());
    let names: Vec<&str> = b["vaults"].as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["alpha", "Zeta"]);
    assert_eq!(b["vaults"][0]["permission"], 100);
    assert_eq!(b["instance"]["locales"].as_array().unwrap().len(), 29);
    app.finish().await;
}

#[tokio::test]
async fn logs_out_other_sessions() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.username, factory::PASSWORD).await.assert(200);
    let mut other_browser = std::collections::HashMap::new();
    app.swap_cookies(&mut other_browser);
    login(&mut app, &user.username, factory::PASSWORD).await.assert(200);
    assert_eq!(app.get("/me/sessions").await.json()["data"].as_array().unwrap().len(), 2);

    app.post("/me/sessions/log-out-others", json!({"password": "wrong"})).await.assert(422);
    app.post("/me/sessions/log-out-others", json!({})).await.assert(403);
    app.post("/me/sessions/log-out-others", json!({"password": factory::PASSWORD})).await.assert(204);
    assert_eq!(app.get("/me/sessions").await.json()["data"].as_array().unwrap().len(), 1);
    app.swap_cookies(&mut other_browser);
    app.get("/me").await.assert(401);
    app.finish().await;
}

/// The first visitor creates the account and its administrator; sign-up
/// is closed after that, and nothing waits for a verification.
#[tokio::test]
async fn sets_up_the_first_user_then_closes_sign_up() {
    let mut app = TestApp::new().await;
    app.get("/auth/csrf").await.assert(204);
    assert_eq!(app.get("/instance").await.json()["setup_required"], true);
    let body = |username: &str| {
        json!({"first_name": "Ann", "last_name": "Smith", "username": username, "password": "abcd",
               "password_confirmation": "abcd"})
    };
    let res = app.post("/auth/register", body("ann smith")).await.assert(422);
    assert_eq!(
        (res.json()["errors"][0]["field"].as_str(), res.json()["errors"][0]["code"].as_str()),
        (Some("username"), Some("regex"))
    );
    let mut mismatch = body("ann");
    mismatch["password_confirmation"] = json!("other");
    assert_eq!(app.post("/auth/register", mismatch).await.assert(422).json()["errors"][0]["field"], "password");

    let me = app.post("/auth/register", body("Ann")).await.assert(201).json();
    assert_eq!(me["username"], "ann");
    assert_eq!(me["is_account_administrator"], true);
    assert_eq!(me["preferences"]["timezone"], "UTC");
    app.get("/vaults").await.assert(200);
    assert_eq!(app.get("/me/notification-channels").await.json()["data"], json!([]));

    // The account was set up (SetupAccount).
    let account_id: monica::db::uuid::Uuid = me["account_id"].as_str().unwrap().parse().unwrap();
    let genders = monica::entities::genders::Entity::find().all(&app.state.db).await.unwrap();
    assert!(genders.iter().any(|g| g.account_id == account_id));

    app.post("/auth/logout", json!({})).await.assert(204);
    app.get("/auth/csrf").await.assert(204);
    assert_eq!(app.get("/instance").await.json()["setup_required"], false);
    assert_eq!(app.post("/auth/register", body("bob")).await.assert(403).code(), "signup_disabled");
    login(&mut app, "ann", "abcd").await.assert(200);
    app.finish().await;
}
