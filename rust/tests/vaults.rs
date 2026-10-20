//! Vaults and their members over HTTP.

mod common;

use common::TestApp;
use monica::domains::base::VaultPermission;
use monica::testing::factory;
use serde_json::json;

async fn login(app: &mut TestApp, email: &str) {
    app.clear_cookies();
    app.use_cookies();
    app.get("/auth/csrf").await.assert(204);
    app.post("/auth/login", json!({"email": email, "password": factory::PASSWORD})).await.assert(200);
}

#[tokio::test]
async fn creates_reads_updates_and_deletes_a_vault() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    login(&mut app, &user.email).await;

    app.post("/vaults", json!({"name": "  "})).await.assert(422);
    let vault = app.post("/vaults", json!({"name": "Friends", "description": "people"})).await.assert(201).json();
    let id = vault["id"].as_str().unwrap().to_owned();
    assert_eq!(vault["permission"], 100);
    assert_eq!(vault["type"], "personal");
    assert!(vault["user_contact_id"].is_string());

    let list = app.get("/vaults").await.assert(200).json();
    assert_eq!(list["data"].as_array().unwrap().len(), 1);
    assert_eq!(list["data"][0]["name"], "Friends");
    assert_eq!(list["data"][0]["contacts_count"], 1, "the user's own contact");

    let v = app
        .patch(&format!("/vaults/{id}"), json!({"name": "Family", "show_files_tab": false, "default_activity_tab": "life_events"}))
        .await
        .assert(200)
        .json();
    assert_eq!(v["name"], "Family");
    assert_eq!(v["description"], "people");
    assert_eq!(v["show_files_tab"], false);
    assert_eq!(v["show_group_tab"], true);
    assert_eq!(v["default_activity_tab"], "life_events");
    app.patch(&format!("/vaults/{id}"), json!({"description": null})).await.assert(200);
    assert!(app.get(&format!("/vaults/{id}")).await.assert(200).json()["description"].is_null());
    app.patch(&format!("/vaults/{id}"), json!({"default_activity_tab": "nope"})).await.assert(422);

    app.delete(&format!("/vaults/{id}")).await.assert(204);
    app.get(&format!("/vaults/{id}")).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn viewers_can_only_change_the_dashboard_tab_and_strangers_see_nothing() {
    let mut app = TestApp::new().await;
    let manager = factory::user(&app.state.db, None).await;
    let viewer = factory::user(&app.state.db, Some(manager.account_id)).await;
    let stranger = factory::user(&app.state.db, None).await;
    let vault = factory::create_vault_user(&app.state.db, &manager, VaultPermission::Manage).await;
    factory::set_permission_in_vault(&app.state.db, &viewer, VaultPermission::View, &vault).await;
    let path = format!("/vaults/{}", vault.id);

    login(&mut app, &viewer.email).await;
    assert_eq!(app.get(&path).await.assert(200).json()["permission"], 300);
    app.patch(&path, json!({"default_activity_tab": "life_metrics"})).await.assert(200);
    let res = app.patch(&path, json!({"name": "Mine now"})).await.assert(403);
    assert_eq!(res.code(), "forbidden");
    app.delete(&path).await.assert(403);
    app.get(&format!("{path}/members")).await.assert(403);

    login(&mut app, &stranger.email).await;
    app.get(&path).await.assert(404);
    app.patch(&path, json!({"default_activity_tab": "activity"})).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn manages_members() {
    let mut app = TestApp::new().await;
    let manager = factory::create_administrator(&app.state.db).await;
    let other = factory::user(&app.state.db, Some(manager.account_id)).await;
    let vault = factory::create_vault_user(&app.state.db, &manager, VaultPermission::Manage).await;
    let base = format!("/vaults/{}/members", vault.id);
    login(&mut app, &manager.email).await;

    let candidates = app.get(&format!("/vaults/{}/member-candidates", vault.id)).await.assert(200).json();
    assert_eq!(candidates["data"].as_array().unwrap().len(), 1);
    assert_eq!(candidates["data"][0]["id"], other.id.to_string());

    app.post(&base, json!({"user_id": other.id, "permission": 250})).await.assert(422);
    let m = app.post(&base, json!({"user_id": other.id, "permission": 300})).await.assert(201).json();
    assert_eq!(m["permission"], 300);
    assert_eq!(m["user"]["contact"]["first_name"], other.first_name.clone().unwrap());
    app.post(&base, json!({"user_id": other.id, "permission": 200})).await.assert(409);
    assert_eq!(app.get(&base).await.json()["data"].as_array().unwrap().len(), 2);

    let m = app.patch(&format!("{base}/{}", other.id), json!({"permission": 200})).await.assert(200).json();
    assert_eq!(m["permission"], 200);
    app.patch(&format!("{base}/{}", manager.id), json!({"permission": 300})).await.assert(409);

    app.delete(&format!("{base}/{}", other.id)).await.assert(204);
    assert_eq!(app.get(&base).await.json()["data"].as_array().unwrap().len(), 1);
    // F19: the removed user has no access left.
    login(&mut app, &other.email).await;
    app.get(&format!("/vaults/{}", vault.id)).await.assert(404);
    app.finish().await;
}
