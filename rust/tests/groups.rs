//! Vault groups over HTTP.

mod common;

use common::TestApp;
use monica::db::id::Id;
use monica::domains::base::VaultPermission;
use monica::domains::contact::manage_groups::AddContactToGroup;
use monica::entities::{
    contact_important_date_types, contact_important_dates, contacts, group_type_roles, group_types, users, vaults,
};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::{Value, json};

async fn group_type(app: &TestApp, account_id: monica::db::uuid::Uuid, label: &str) -> group_types::Model {
    group_types::ActiveModel {
        account_id: Set(account_id),
        label: Set(Some(label.into())),
        position: Set(1),
        ..Default::default()
    }
    .insert(&app.state.db)
    .await
    .unwrap()
}

async fn role(app: &TestApp, group_type_id: Id, label: &str, position: i32) -> group_type_roles::Model {
    group_type_roles::ActiveModel {
        group_type_id: Set(group_type_id),
        label: Set(Some(label.into())),
        position: Set(Some(position)),
        ..Default::default()
    }
    .insert(&app.state.db)
    .await
    .unwrap()
}

async fn named_contact(app: &TestApp, vault: &vaults::Model, first_name: &str) -> contacts::Model {
    let c = factory::contact(&app.state.db, vault).await;
    let mut a: contacts::ActiveModel = c.into();
    a.first_name = Set(Some(first_name.into()));
    a.update(&app.state.db).await.unwrap()
}

async fn add_to_group(
    app: &TestApp,
    user: &users::Model,
    vault: &vaults::Model,
    group_id: i64,
    contact: &contacts::Model,
    role: Option<Id>,
) {
    AddContactToGroup {
        account_id: user.account_id,
        vault_id: vault.id,
        author_id: user.id,
        group_id: Id(group_id),
        contact_id: contact.id,
        group_type_role_id: role,
    }
    .execute(&app.state.db)
    .await
    .unwrap();
}

fn names(members: &Value) -> Vec<String> {
    members.as_array().unwrap().iter().map(|m| m["first_name"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test]
async fn creates_lists_shows_updates_and_deletes_groups() {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    let vault = factory::create_vault_user(&app.state.db, &user, VaultPermission::Edit).await;
    let family = group_type(&app, user.account_id, "Family").await;
    let child = role(&app, family.id, "Child", 2).await;
    let parent = role(&app, family.id, "Parent", 1).await;
    let elsewhere = group_type(&app, factory::account(&app.state.db).await.id, "Other").await;
    app.login(&user.email).await;
    let base = format!("/vaults/{}/groups", vault.id);

    let res = app.post(&base, json!({"name": "  "})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "name");
    let res = app.post(&base, json!({"name": "x".repeat(256)})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["code"], "max");
    let res = app.post(&base, json!({"name": "Friends", "group_type_id": elsewhere.id})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "group_type_id");
    assert_eq!(res.json()["errors"][0]["code"], "exists");

    let g = app.post(&base, json!({"name": "Troyats", "group_type_id": family.id})).await.assert(201).json();
    assert_eq!(g["name"], "Troyats");
    assert_eq!(g["vault_id"], vault.id.to_string());
    assert_eq!(g["members"], json!([]));
    assert_eq!(g["group_type"]["id"], family.id.0);
    assert_eq!(g["group_type"]["label"], "Family");
    let roles: Vec<&str> =
        g["group_type"]["roles"].as_array().unwrap().iter().map(|r| r["label"].as_str().unwrap()).collect();
    assert_eq!(roles, ["Parent", "Child"]);
    let group_id = g["id"].as_i64().unwrap();
    let plain = app.post(&base, json!({"name": "Book club"})).await.assert(201).json();
    assert!(plain["group_type"].is_null());

    // Members: by first name, with age and role; deleted contacts left out.
    let zoe = named_contact(&app, &vault, "Zoé").await;
    let anna = named_contact(&app, &vault, "anna").await;
    let emile = named_contact(&app, &vault, "Émile").await;
    let gone = named_contact(&app, &vault, "Gone").await;
    add_to_group(&app, &user, &vault, group_id, &zoe, Some(parent.id)).await;
    add_to_group(&app, &user, &vault, group_id, &anna, None).await;
    add_to_group(&app, &user, &vault, group_id, &emile, Some(child.id)).await;
    add_to_group(&app, &user, &vault, group_id, &gone, None).await;
    let mut a: contacts::ActiveModel = gone.into();
    a.deleted_at = Set(Some(monica::db::now()));
    a.update(&app.state.db).await.unwrap();
    let birthdate = contact_important_date_types::ActiveModel {
        vault_id: Set(vault.id),
        label: Set("Birthdate".into()),
        internal_type: Set(Some("birthdate".into())),
        can_be_deleted: Set(false),
        ..Default::default()
    }
    .insert(&app.state.db)
    .await
    .unwrap();
    contact_important_dates::ActiveModel {
        contact_id: Set(anna.id),
        contact_important_date_type_id: Set(Some(birthdate.id)),
        label: Set("Birthdate".into()),
        year: Set(Some(2000)),
        ..Default::default()
    }
    .insert(&app.state.db)
    .await
    .unwrap();

    let list = app.get(&base).await.assert(200).json();
    let groups: Vec<&str> = list["data"].as_array().unwrap().iter().map(|g| g["name"].as_str().unwrap()).collect();
    assert_eq!(groups, ["Book club", "Troyats"]);
    let troyats = &list["data"][1];
    assert_eq!(names(&troyats["members"]), ["anna", "Émile", "Zoé"]);
    let anna_json = &troyats["members"][0];
    assert_eq!(anna_json["id"], anna.id.to_string());
    assert_eq!(anna_json["age"], chrono::Utc::now().format("%Y").to_string().parse::<i64>().unwrap() - 2000);
    assert!(anna_json["group_type_role_id"].is_null());
    assert_eq!(anna_json["listed"], true);
    assert!(anna_json["avatar_file_id"].is_null());
    assert_eq!(troyats["members"][2]["group_type_role_id"], parent.id.0);

    // Buckets: the type's roles by position, then the members without one.
    let detail = app.get(&format!("{base}/{group_id}")).await.assert(200).json();
    assert_eq!(detail["name"], "Troyats");
    assert_eq!(names(&detail["members"]), ["anna", "Émile", "Zoé"]);
    let buckets = detail["roles"].as_array().unwrap();
    assert_eq!(buckets.len(), 3);
    assert_eq!(buckets[0]["role"]["label"], "Parent");
    assert_eq!(buckets[0]["role"]["position"], 1);
    assert_eq!(buckets[0]["role"]["group_type_id"], family.id.0);
    assert_eq!(names(&buckets[0]["members"]), ["Zoé"]);
    assert_eq!(buckets[1]["role"]["label"], "Child");
    assert_eq!(names(&buckets[1]["members"]), ["Émile"]);
    assert!(buckets[2]["role"].is_null());
    assert_eq!(names(&buckets[2]["members"]), ["anna"]);
    let empty = app.get(&format!("{base}/{}", plain["id"])).await.assert(200).json();
    assert_eq!(empty["roles"], json!([]), "no null bucket without members");

    let g = app.patch(&format!("{base}/{group_id}"), json!({"name": "The Troyats"})).await.assert(200).json();
    assert_eq!(g["name"], "The Troyats");
    assert_eq!(g["group_type"]["id"], family.id.0, "absent type is kept");
    assert_eq!(g["members"].as_array().unwrap().len(), 3);
    app.patch(&format!("{base}/{group_id}"), json!({"name": ""})).await.assert(422);
    let res = app.patch(&format!("{base}/{group_id}"), json!({"group_type_id": elsewhere.id})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["code"], "exists");
    let g = app.patch(&format!("{base}/{group_id}"), json!({"group_type_id": null})).await.assert(200).json();
    assert!(g["group_type"].is_null());
    assert_eq!(g["name"], "The Troyats");
    let detail = app.get(&format!("{base}/{group_id}")).await.assert(200).json();
    let buckets = detail["roles"].as_array().unwrap();
    assert_eq!(buckets.len(), 1, "members with a role of another type have none");
    assert!(buckets[0]["role"].is_null());
    assert_eq!(names(&buckets[0]["members"]), ["anna", "Émile", "Zoé"]);

    app.delete(&format!("{base}/{group_id}")).await.assert(204);
    app.get(&format!("{base}/{group_id}")).await.assert(404);
    app.patch(&format!("{base}/{group_id}"), json!({"name": "Back"})).await.assert(404);
    app.delete(&format!("{base}/{group_id}")).await.assert(404);
    assert_eq!(app.get(&base).await.json()["data"].as_array().unwrap().len(), 1);
    app.finish().await;
}

#[tokio::test]
async fn viewers_only_read_and_strangers_see_nothing() {
    let mut app = TestApp::new().await;
    let editor = factory::user(&app.state.db, None).await;
    let viewer = factory::user(&app.state.db, Some(editor.account_id)).await;
    let stranger = factory::user(&app.state.db, None).await;
    let vault = factory::create_vault_user(&app.state.db, &editor, VaultPermission::Edit).await;
    factory::set_permission_in_vault(&app.state.db, &viewer, VaultPermission::View, &vault).await;
    let other = factory::create_vault_user(&app.state.db, &editor, VaultPermission::Edit).await;
    let base = format!("/vaults/{}/groups", vault.id);

    app.login(&editor.email).await;
    let g = app.post(&base, json!({"name": "Friends"})).await.assert(201).json();
    let path = format!("{base}/{}", g["id"]);
    let foreign = app.post(&format!("/vaults/{}/groups", other.id), json!({"name": "Theirs"})).await.assert(201).json();
    app.get(&format!("{base}/{}", foreign["id"])).await.assert(404);
    app.get(&format!("{base}/999999")).await.assert(404);

    app.login(&viewer.email).await;
    assert_eq!(app.get(&base).await.assert(200).json()["data"][0]["name"], "Friends");
    app.get(&path).await.assert(200);
    assert_eq!(app.post(&base, json!({"name": "Mine"})).await.assert(403).code(), "forbidden");
    app.patch(&path, json!({"name": "Mine"})).await.assert(403);
    app.delete(&path).await.assert(403);

    app.login(&stranger.email).await;
    app.get(&base).await.assert(404);
    app.get(&path).await.assert(404);
    app.post(&base, json!({"name": "Mine"})).await.assert(404);
    app.patch(&path, json!({"name": "Mine"})).await.assert(404);
    app.delete(&path).await.assert(404);
    app.finish().await;
}
