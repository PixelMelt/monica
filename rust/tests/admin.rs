//! The instance administrator's dashboard: the accounts of the install.

mod common;

use axum::http::Method;
use common::TestApp;
use monica::entities::{accounts, users};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::json;

async fn instance_administrator(app: &TestApp) -> users::Model {
    let admin = factory::create_administrator(&app.state.db).await;
    let mut u: users::ActiveModel = admin.into();
    u.is_instance_administrator = Set(true);
    u.update(&app.state.db).await.unwrap()
}

#[tokio::test]
async fn only_instance_administrators_manage_accounts() {
    let mut app = TestApp::new().await;
    let account_admin = factory::create_administrator(&app.state.db).await;
    app.login(&account_admin.username).await;
    app.get("/admin/accounts").await.assert(403);
    app.post("/admin/accounts", json!({"username": "x"})).await.assert(403);
    let path = format!("/admin/accounts/{}", account_admin.account_id);
    app.request(Method::DELETE, &path, Some(json!({"password": factory::PASSWORD}))).await.assert(403);
    app.finish().await;
}

#[tokio::test]
async fn creates_lists_and_deletes_accounts() {
    let mut app = TestApp::new().await;
    let admin = instance_administrator(&app).await;
    app.login(&admin.username).await;

    let res = app.post("/admin/accounts", json!({"username": "Grace"})).await.assert(422).json();
    let fields: Vec<&str> = res["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap()).collect();
    assert_eq!(fields, ["first_name", "last_name", "password"]);
    let password = factory::PASSWORD;
    let body = json!({"first_name": "Grace", "last_name": "Hopper", "username": "Grace", "password": password,
                      "password_confirmation": password});
    let created = app.post("/admin/accounts", body.clone()).await.assert(201).json();
    assert_ne!(created["id"], admin.account_id.to_string());
    assert_eq!(created["users"][0]["username"], "grace");
    assert_eq!(created["users"][0]["is_account_administrator"], true);
    assert_eq!(created["users"][0]["is_instance_administrator"], false);
    assert_eq!((created["vault_count"].as_u64(), created["contact_count"].as_u64()), (Some(0), Some(0)));
    assert_eq!(app.post("/admin/accounts", body).await.assert(422).json()["errors"][0]["code"], "unique");

    let list = app.get("/admin/accounts").await.assert(200).json()["data"].clone();
    let ids: Vec<&str> = list.as_array().unwrap().iter().map(|a| a["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&admin.account_id.to_string().as_str()) && ids.contains(&created["id"].as_str().unwrap()));

    // The new account is set up and separate: its user sees only their own vaults.
    app.login("grace").await;
    app.get("/admin/accounts").await.assert(403);
    let vault = app.post("/vaults", json!({"name": "Navy"})).await.assert(201).json();
    assert_eq!(app.get("/vaults").await.json()["data"].as_array().unwrap().len(), 1);
    app.login(&admin.username).await;
    let list = app.get("/admin/accounts").await.json()["data"].clone();
    let grace = list.as_array().unwrap().iter().find(|a| a["id"] == created["id"]).unwrap().clone();
    assert_eq!(grace["vault_count"], 1);
    assert_eq!(grace["contact_count"], 1, "the user's own contact in the vault");

    let own = format!("/admin/accounts/{}", admin.account_id);
    app.request(Method::DELETE, &own, Some(json!({"password": factory::PASSWORD}))).await.assert(409);
    let path = format!("/admin/accounts/{}", created["id"].as_str().unwrap());
    let res = app.request(Method::DELETE, &path, Some(json!({}))).await.assert(403);
    assert_eq!(res.code(), "password_confirmation_required");
    app.request(Method::DELETE, &path, Some(json!({"password": "wrong"}))).await.assert(422);
    app.request(Method::DELETE, &path, Some(json!({"password": factory::PASSWORD}))).await.assert(204);
    let id: monica::db::uuid::Uuid = created["id"].as_str().unwrap().parse().unwrap();
    assert!(accounts::Entity::find_by_id(id).one(&app.state.db).await.unwrap().is_none());
    let vault_id = vault["id"].as_str().unwrap();
    assert!(!app.state.config.data_dir.join("files").join(vault_id).exists());
    app.request(Method::DELETE, &path, Some(json!({"password": factory::PASSWORD}))).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn the_first_user_is_the_instance_administrator() {
    let mut app = TestApp::new().await;
    app.get("/auth/csrf").await.assert(204);
    let me = app
        .post(
            "/auth/register",
            json!({"first_name": "Ann", "last_name": "Smith", "username": "ann", "password": "abcd",
                   "password_confirmation": "abcd"}),
        )
        .await
        .assert(201)
        .json();
    assert_eq!(me["is_instance_administrator"], true);
    app.get("/admin/accounts").await.assert(200);
    app.finish().await;
}
