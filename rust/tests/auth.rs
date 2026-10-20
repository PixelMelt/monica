//! Login, 2FA, tokens, password reset and email verification over HTTP.

mod common;

use common::TestApp;
use monica::auth::two_factor;
use monica::entities::users;
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel};
use serde_json::json;

async fn login(app: &mut TestApp, email: &str, password: &str) -> common::TestResponse {
    app.get("/auth/csrf").await.assert(204);
    app.post("/auth/login", json!({"email": email, "password": password})).await
}

fn query_param(url: &str, name: &str) -> String {
    let q = url.split_once('?').unwrap().1;
    q.split('&').find_map(|kv| kv.strip_prefix(&format!("{name}="))).unwrap().to_owned()
}

#[tokio::test]
async fn logs_in_and_out() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;

    let res = login(&mut app, &user.email, "wrong").await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "email");
    assert_eq!(res.json()["errors"][0]["message"], "These credentials do not match our records.");
    app.get("/me").await.assert(401);

    let res = login(&mut app, &user.email, factory::PASSWORD).await.assert(200);
    assert_eq!(res.json()["two_factor"], false);
    assert_eq!(res.json()["user"]["email"], user.email.as_str());
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
        login(&mut app, &user.email, "wrong").await.assert(422);
    }
    let res = login(&mut app, &user.email, factory::PASSWORD).await.assert(429);
    assert!(res.headers.contains_key("retry-after"));
    app.finish().await;
}

#[tokio::test]
async fn two_factor_enable_confirm_and_challenge() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.email, factory::PASSWORD).await.assert(200);

    // Sensitive: without a confirmation or the password in the body it's refused.
    let res = app.post("/me/two-factor", json!({})).await.assert(403);
    assert_eq!(res.code(), "password_confirmation_required");
    app.post("/auth/confirm-password", json!({"password": "nope"})).await.assert(422);
    app.post("/auth/confirm-password", json!({"password": factory::PASSWORD})).await.assert(204);
    assert_eq!(app.get("/auth/confirmed-password-status").await.json()["confirmed"], true);

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
    let res = login(&mut app, &user.email, factory::PASSWORD).await.assert(200);
    assert_eq!(res.json()["two_factor"], true);
    assert!(res.json()["user"].is_null());
    app.get("/me").await.assert(401);

    // The code for the next time step (the current one was used to confirm).
    let next = two_factor::code_at(&secret, now + 30);
    let me = app.post("/auth/two-factor-challenge", json!({"code": next})).await.assert(200).json();
    assert_eq!(me["email"], user.email.as_str());

    // A recovery code works once.
    app.post("/auth/logout", json!({})).await.assert(204);
    login(&mut app, &user.email, factory::PASSWORD).await.assert(200);
    let code = codes[0].as_str().unwrap();
    app.post("/auth/two-factor-challenge", json!({"recovery_code": code})).await.assert(200);
    app.post("/auth/logout", json!({})).await.assert(204);
    login(&mut app, &user.email, factory::PASSWORD).await.assert(200);
    app.post("/auth/two-factor-challenge", json!({"recovery_code": code})).await.assert(422);

    app.finish().await;
}

#[tokio::test]
async fn tokens_authenticate_with_their_abilities() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.email, factory::PASSWORD).await.assert(200);

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

#[tokio::test]
async fn resets_a_forgotten_password() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    app.get("/auth/csrf").await.assert(204);

    let res = app.post("/auth/forgot-password", json!({"email": "nobody@example.com"})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["message"], "We can't find a user with that email address.");
    app.post("/auth/forgot-password", json!({"email": user.email})).await.assert(204);
    app.post("/auth/forgot-password", json!({"email": user.email})).await.assert(422);

    let mail = app.state.mailer.sent().pop().unwrap();
    assert_eq!(mail.subject, "Reset Password Notification");
    let url = mail.text.lines().find(|l| l.contains("/reset-password/")).unwrap();
    let token = url.split("/reset-password/").nth(1).unwrap().split('?').next().unwrap().to_owned();

    let body =
        |t: &str| json!({"token": t, "email": user.email, "password": "newpass", "password_confirmation": "newpass"});
    app.post("/auth/reset-password", body("wrong")).await.assert(422);
    app.post("/auth/reset-password", body(&token)).await.assert(204);
    app.post("/auth/reset-password", body(&token)).await.assert(422);

    login(&mut app, &user.email, factory::PASSWORD).await.assert(422);
    login(&mut app, &user.email, "newpass").await.assert(200);
    app.finish().await;
}

#[tokio::test]
async fn verifies_the_email_address() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    let mut u = user.clone().into_active_model();
    u.email_verified_at = Set(None);
    u.update(&app.state.db).await.unwrap();
    login(&mut app, &user.email, factory::PASSWORD).await.assert(200);

    // Unverified users can reach /me but not the rest.
    app.get("/me").await.assert(200);
    let res = app.get("/me/tokens").await.assert(403);
    assert_eq!(res.code(), "email_not_verified");

    app.post("/auth/email/verification-notification", json!({})).await.assert(204);
    let mail = app.state.mailer.sent().pop().unwrap();
    assert_eq!(mail.subject, "Verify Email Address");
    let url = mail.text.lines().find(|l| l.contains("/email/verify/")).unwrap().split(": ").nth(1).unwrap().to_owned();
    let path = url.split("/email/verify/").nth(1).unwrap().split('?').next().unwrap();
    let (id, hash) = path.split_once('/').unwrap();
    let expires: i64 = query_param(&url, "expires").parse().unwrap();
    let signature = query_param(&url, "signature");

    let bad = json!({"id": id, "hash": hash, "expires": expires, "signature": "0".repeat(64)});
    app.post("/auth/email/verify", bad).await.assert(403);
    let good = json!({"id": id, "hash": hash, "expires": expires, "signature": signature});
    app.post("/auth/email/verify", good).await.assert(204);
    app.get("/me/tokens").await.assert(200);
    let row = users::Entity::find_by_id(user.id).one(&app.state.db).await.unwrap().unwrap();
    assert!(row.email_verified_at.is_some());
    app.finish().await;
}

#[tokio::test]
async fn changes_password_and_lists_sessions() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.email, factory::PASSWORD).await.assert(200);
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
    login(&mut app, &user.email, factory::PASSWORD).await.assert(200);
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
    login(&mut app, &user.email, factory::PASSWORD).await.assert(200);
    let mut other_browser = std::collections::HashMap::new();
    app.swap_cookies(&mut other_browser);
    login(&mut app, &user.email, factory::PASSWORD).await.assert(200);
    assert_eq!(app.get("/me/sessions").await.json()["data"].as_array().unwrap().len(), 2);

    app.post("/me/sessions/log-out-others", json!({"password": "wrong"})).await.assert(422);
    app.post("/me/sessions/log-out-others", json!({})).await.assert(403);
    app.post("/me/sessions/log-out-others", json!({"password": factory::PASSWORD})).await.assert(204);
    assert_eq!(app.get("/me/sessions").await.json()["data"].as_array().unwrap().len(), 1);
    app.swap_cookies(&mut other_browser);
    app.get("/me").await.assert(401);
    app.finish().await;
}

#[tokio::test]
async fn registers_a_new_account() {
    let mut app = TestApp::new().await;
    app.get("/auth/csrf").await.assert(204);
    let body = |email: &str| {
        json!({"first_name": "Ann", "last_name": "Smith", "email": email, "password": "abcd",
               "password_confirmation": "abcd", "terms": true})
    };
    let mut no_terms = body("ann@example.com");
    no_terms["terms"] = json!(false);
    let res = app.post("/auth/register", no_terms).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "terms");

    let me = app.post("/auth/register", body("ann@example.com")).await.assert(201).json();
    assert_eq!(me["is_account_administrator"], true);
    assert!(me["email_verified_at"].is_null());
    assert_eq!(me["preferences"]["timezone"], "UTC");
    // Logged in but unverified: only the verification flow and /me work.
    app.get("/me").await.assert(200);
    assert_eq!(app.get("/vaults").await.assert(403).code(), "email_not_verified");
    let mail = app.state.mailer.sent().pop().unwrap();
    assert_eq!((mail.to.as_str(), mail.subject.as_str()), ("ann@example.com", "Verify Email Address"));

    // The account was set up (SetupAccount): its default notification channel exists.
    let account_id: monica::db::uuid::Uuid = me["account_id"].as_str().unwrap().parse().unwrap();
    let users = monica::entities::users::Entity::find().all(&app.state.db).await.unwrap();
    assert_eq!(users.len(), 1);
    let genders = monica::entities::genders::Entity::find().all(&app.state.db).await.unwrap();
    assert!(genders.iter().any(|g| g.account_id == account_id));

    app.post("/auth/logout", json!({})).await.assert(204);
    app.get("/auth/csrf").await.assert(204);
    assert_eq!(
        app.post("/auth/register", body("ann@example.com")).await.assert(422).json()["errors"][0]["code"],
        "unique"
    );
    app.finish().await;
}

#[tokio::test]
async fn signup_can_be_disabled_after_the_first_account() {
    let mut app = TestApp::with_config(|c| c.disable_signup = true).await;
    app.get("/auth/csrf").await.assert(204);
    assert_eq!(app.get("/instance").await.json()["signup_enabled"], true, "the first account may always register");
    let body = |email: &str| json!({"first_name": "A", "last_name": "B", "email": email, "password": "abcd", "password_confirmation": "abcd", "terms": true});
    app.post("/auth/register", body("first@example.com")).await.assert(201);
    app.post("/auth/logout", json!({})).await.assert(204);
    app.get("/auth/csrf").await.assert(204);
    assert_eq!(app.get("/instance").await.json()["signup_enabled"], false);
    assert_eq!(app.post("/auth/register", body("second@example.com")).await.assert(403).code(), "signup_disabled");
    app.finish().await;
}
