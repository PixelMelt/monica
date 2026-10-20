//! Contact modules, part B, over HTTP: relationships, pets, documents,
//! photos, tasks, calls, group membership, timeline and life events, mood
//! tracking events, and the contact page module data of each.

mod common;

use axum::http::Method;
use common::TestApp;
use monica::api::contact_modules_b as modules;
use monica::api::contact_page::ModuleContext;
use monica::db::id::Id;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::domains::contact::manage_groups::CreateGroup;
use monica::domains::settings::create_account::CreateAccount;
use monica::entities::{
    accounts, call_reason_types, call_reasons, contact_feed_items, contact_tasks, contacts, emotions, files,
    group_type_roles, group_types, life_event_categories, life_event_types, mood_tracking_parameters, pet_categories,
    relationship_group_types, relationship_types, timeline_events, users, vaults,
};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{Value, json};

/// A seeded account (`CreateAccount`), its first user logged in, a vault
/// made through the API and a contact in it.
struct Setup {
    app: TestApp,
    user: users::Model,
    vault: vaults::Model,
    contact: contacts::Model,
    /// `/vaults/{vault_id}/contacts/{contact_id}`.
    base: String,
}

async fn setup() -> Setup {
    let mut app = TestApp::new().await;
    let user = CreateAccount {
        first_name: "Ann".into(),
        last_name: "Smith".into(),
        email: format!("{}@example.com", uuid::Uuid::new_v4().simple()),
        password: Some(factory::PASSWORD.into()),
        storage_limit_in_mb: 50,
    }
    .execute(&app.state.db, &app.state.mailer, &app.state.config)
    .await
    .unwrap();
    let mut verified: users::ActiveModel = user.into();
    verified.email_verified_at = Set(Some(monica::db::now()));
    let user = verified.update(&app.state.db).await.unwrap();
    app.login(&user.email).await;
    let vault = new_vault(&mut app, "Family").await;
    let contact = factory::contact(&app.state.db, &vault).await;
    let base = format!("/vaults/{}/contacts/{}", vault.id, contact.id);
    Setup { app, user, vault, contact, base }
}

async fn new_vault(app: &mut TestApp, name: &str) -> vaults::Model {
    let v = app.post("/vaults", json!({ "name": name })).await.assert(201).json();
    let id: Uuid = v["id"].as_str().unwrap().parse().unwrap();
    vaults::Entity::find_by_id(id).one(&app.state.db).await.unwrap().unwrap()
}

impl Setup {
    async fn login_viewer(&mut self) {
        let viewer = factory::user(&self.app.state.db, Some(self.user.account_id)).await;
        factory::set_permission_in_vault(&self.app.state.db, &viewer, VaultPermission::View, &self.vault).await;
        self.app.login(&viewer.email).await;
    }

    async fn login_stranger(&mut self) {
        let stranger = factory::user(&self.app.state.db, None).await;
        self.app.login(&stranger.email).await;
    }

    fn ctx(&self, contact: &contacts::Model) -> ModuleContext {
        ModuleContext { user: self.user.clone(), vault: self.vault.clone(), contact: contact.clone() }
    }

    fn path_of(&self, contact: &contacts::Model, rest: &str) -> String {
        format!("/vaults/{}/contacts/{}{rest}", contact.vault_id, contact.id)
    }
}

fn fields(res: &Value) -> Vec<String> {
    res["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap().to_owned()).collect()
}

fn codes(res: &Value) -> Vec<String> {
    res["errors"].as_array().unwrap().iter().map(|e| e["code"].as_str().unwrap().to_owned()).collect()
}

fn len(v: &Value) -> usize {
    v.as_array().unwrap().len()
}

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_fn(4, 4, |x, y| image::Rgb([(x * 40) as u8, (y * 40) as u8, 128]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

// ---------------------------------------------------------------- relationships

#[tokio::test]
async fn sets_lists_and_removes_relationships() {
    let mut s = setup().await;
    let db = s.app.state.db.clone();
    let base = format!("{}/relationships", s.base);
    let group_ids: Vec<Id> = relationship_group_types::Entity::find()
        .filter(relationship_group_types::Column::AccountId.eq(s.user.account_id))
        .order_by_asc(relationship_group_types::Column::Id)
        .all(&db)
        .await
        .unwrap()
        .into_iter()
        .map(|g| g.id)
        .collect();
    let types = relationship_types::Entity::find()
        .filter(relationship_types::Column::RelationshipGroupTypeId.eq(group_ids[0]))
        .order_by_asc(relationship_types::Column::Id)
        .all(&db)
        .await
        .unwrap();
    let (first, second) = (&types[0], &types[1]);
    assert_eq!(len(&s.app.get(&base).await.assert(200).json()["data"]), 0);

    let other = factory::contact(&db, &s.vault).await;
    let r = s
        .app
        .post(&base, json!({"relationship_type_id": first.id, "other_contact_id": other.id}))
        .await
        .assert(201)
        .json();
    assert_eq!(r["contact_id"], s.contact.id.to_string());
    assert_eq!(r["related_contact_id"], other.id.to_string());
    assert_eq!(r["relationship_type_id"], first.id.0);
    assert_eq!(r["relationship_type"]["id"], first.id.0);
    assert_eq!(
        r["relationship_type"]["name_reverse_relationship_translation_key"],
        json!(first.name_reverse_relationship_translation_key)
    );
    assert_eq!(r["other_contact"]["id"], other.id.to_string());
    assert_eq!(r["other_contact"]["first_name"], "Regis");
    assert!(r["other_contact"]["age"].is_null());

    // Reverse, with a contact made for the relationship (composite).
    let kid = s
        .app
        .post(
            &base,
            json!({"relationship_type_id": second.id, "direction": "reverse",
                   "new_contact": {"first_name": " Kid ", "listed": false}}),
        )
        .await
        .assert(201)
        .json();
    assert_eq!(kid["related_contact_id"], s.contact.id.to_string());
    assert_eq!(kid["other_contact"]["first_name"], "Kid");
    assert_eq!(kid["other_contact"]["listed"], false);
    let kid_id: Uuid = kid["contact_id"].as_str().unwrap().parse().unwrap();
    let feed = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(kid_id))
        .filter(contact_feed_items::Column::Action.eq("contact_created"))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(feed.len(), 1);

    let list = s.app.get(&base).await.assert(200).json();
    assert_eq!(len(&list["data"]), 2);
    assert_eq!(list["data"][0]["id"], r["id"]);
    assert_eq!(list["data"][1]["id"], kid["id"]);
    // Seen from the other side.
    let theirs = s.app.get(&s.path_of(&other, "/relationships")).await.assert(200).json();
    assert_eq!(theirs["data"][0]["other_contact"]["id"], s.contact.id.to_string());

    let m =
        serde_json::to_value(modules::relationships_module(&s.app.state, &s.ctx(&s.contact)).await.unwrap()).unwrap();
    assert_eq!(m["number_of_defined_relations"], 2);
    assert_eq!(len(&m["relationship_group_types"]), group_ids.len());
    assert_eq!(m["relationship_group_types"][0]["relationship_group_type"]["id"], group_ids[0].0);
    assert_eq!(len(&m["relationship_group_types"][0]["relationship_group_type"]["types"]), types.len());
    assert_eq!(len(&m["relationship_group_types"][0]["relationships"]), 2);
    assert_eq!(len(&m["relationship_group_types"][1]["relationships"]), 0);

    // Validation.
    let res = s.app.post(&base, json!({})).await.assert(422).json();
    assert_eq!(fields(&res), ["relationship_type_id", "other_contact_id"]);
    let res = s
        .app
        .post(
            &base,
            json!({"relationship_type_id": first.id, "other_contact_id": other.id, "new_contact": {"listed": true}}),
        )
        .await
        .assert(422)
        .json();
    assert_eq!(codes(&res), ["prohibits"]);
    let res = s
        .app
        .post(&base, json!({"relationship_type_id": 999_999, "other_contact_id": other.id}))
        .await
        .assert(422)
        .json();
    assert_eq!((fields(&res), codes(&res)), (vec!["relationship_type_id".to_owned()], vec!["exists".to_owned()]));
    let elsewhere = new_vault(&mut s.app, "Work").await;
    let outsider = factory::contact(&db, &elsewhere).await;
    let res = s
        .app
        .post(&base, json!({"relationship_type_id": first.id, "other_contact_id": outsider.id}))
        .await
        .assert(422)
        .json();
    assert_eq!(fields(&res), ["other_contact_id"]);
    let res = s
        .app
        .post(&base, json!({"relationship_type_id": first.id, "new_contact": {"first_name": "x".repeat(256), "gender_id": 999_999}}))
        .await
        .assert(422)
        .json();
    assert_eq!(fields(&res), ["new_contact.first_name", "new_contact.listed", "new_contact.gender_id"]);
    let res = s
        .app
        .post(&base, json!({"relationship_type_id": first.id, "other_contact_id": other.id, "direction": "sideways"}))
        .await
        .assert(422)
        .json();
    assert_eq!(fields(&res), ["direction"]);

    // Removing a relationship deletes the unlisted contact made for it.
    s.app.delete(&format!("{base}/{}", kid["id"])).await.assert(204);
    let gone = contacts::Entity::find_by_id(kid_id).one(&db).await.unwrap().unwrap();
    assert!(gone.deleted_at.is_some());
    assert_eq!(len(&s.app.get(&base).await.json()["data"]), 1);
    s.app.delete(&format!("{base}/{}", kid["id"])).await.assert(404);
    // A relationship the path contact isn't part of.
    let third = factory::contact(&db, &s.vault).await;
    s.app.delete(&s.path_of(&third, &format!("/relationships/{}", r["id"]))).await.assert(404);
    s.app.get(&format!("/vaults/{}/contacts/{}/relationships", s.vault.id, outsider.id)).await.assert(404);

    s.login_viewer().await;
    assert_eq!(len(&s.app.get(&base).await.assert(200).json()["data"]), 1);
    s.app.post(&base, json!({"relationship_type_id": first.id, "other_contact_id": third.id})).await.assert(403);
    s.app.delete(&format!("{base}/{}", r["id"])).await.assert(403);
    s.login_stranger().await;
    s.app.get(&base).await.assert(404);
    s.app.post(&base, json!({"relationship_type_id": first.id, "other_contact_id": third.id})).await.assert(404);
    s.app.delete(&format!("{base}/{}", r["id"])).await.assert(404);
    s.app.finish().await;
}

// ---------------------------------------------------------------- pets

#[tokio::test]
async fn manages_pets() {
    let mut s = setup().await;
    let db = s.app.state.db.clone();
    let base = format!("{}/pets", s.base);
    let categories = pet_categories::Entity::find()
        .filter(pet_categories::Column::AccountId.eq(s.user.account_id))
        .order_by_asc(pet_categories::Column::Id)
        .all(&db)
        .await
        .unwrap();
    let foreign = pet_categories::ActiveModel {
        account_id: Set(factory::account(&db).await.id),
        name: Set(Some("Dragon".into())),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();

    assert_eq!(fields(&s.app.post(&base, json!({})).await.assert(422).json()), ["pet_category_id"]);
    let res = s.app.post(&base, json!({"pet_category_id": foreign.id})).await.assert(422).json();
    assert_eq!(codes(&res), ["exists"]);
    let res = s
        .app
        .post(&base, json!({"pet_category_id": categories[0].id, "name": "x".repeat(256)}))
        .await
        .assert(422)
        .json();
    assert_eq!(fields(&res), ["name"]);

    let pet = s.app.post(&base, json!({"pet_category_id": categories[0].id, "name": "Rex"})).await.assert(201).json();
    assert_eq!(pet["name"], "Rex");
    assert_eq!(pet["contact_id"], s.contact.id.to_string());
    assert_eq!(pet["pet_category"]["id"], categories[0].id.0);
    assert_eq!(pet["pet_category"]["name_translation_key"], json!(categories[0].name_translation_key));
    let feed = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(s.contact.id))
        .filter(contact_feed_items::Column::Action.eq("pet_created"))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(feed.len(), 1);
    let item = format!("{base}/{}", pet["id"]);

    // F7: the category can change.
    let p = s.app.patch(&item, json!({"pet_category_id": categories[1].id})).await.assert(200).json();
    assert_eq!((p["pet_category"]["id"].clone(), p["name"].clone()), (json!(categories[1].id.0), json!("Rex")));
    let p = s.app.patch(&item, json!({"name": null})).await.assert(200).json();
    assert!(p["name"].is_null());
    s.app.patch(&item, json!({"pet_category_id": foreign.id})).await.assert(422);

    let list = s.app.get(&base).await.assert(200).json();
    assert_eq!(len(&list["data"]), 1);
    assert_eq!(list["data"][0]["pet_category"]["id"], categories[1].id.0);
    let m = serde_json::to_value(modules::pets_module(&s.app.state, &s.ctx(&s.contact)).await.unwrap()).unwrap();
    assert_eq!(m["pets"][0]["id"], pet["id"]);

    let other = factory::contact(&db, &s.vault).await;
    s.app.patch(&s.path_of(&other, &format!("/pets/{}", pet["id"])), json!({"name": "Mine"})).await.assert(404);
    s.app.delete(&s.path_of(&other, &format!("/pets/{}", pet["id"]))).await.assert(404);

    s.login_viewer().await;
    assert_eq!(len(&s.app.get(&base).await.assert(200).json()["data"]), 1);
    s.app.post(&base, json!({"pet_category_id": categories[0].id})).await.assert(403);
    s.app.patch(&item, json!({"name": "Max"})).await.assert(403);
    s.app.delete(&item).await.assert(403);

    s.app.login(&s.user.email).await;
    s.app.delete(&item).await.assert(204);
    assert_eq!(len(&s.app.get(&base).await.json()["data"]), 0);
    s.app.delete(&item).await.assert(404);
    s.login_stranger().await;
    s.app.get(&base).await.assert(404);
    s.app.post(&base, json!({"pet_category_id": categories[0].id})).await.assert(404);
    s.app.patch(&item, json!({"name": "x"})).await.assert(404);
    s.app.delete(&item).await.assert(404);
    s.app.finish().await;
}

// ---------------------------------------------------------------- documents and photos

#[tokio::test]
async fn uploads_lists_and_deletes_documents() {
    let mut s = setup().await;
    let db = s.app.state.db.clone();
    let base = format!("{}/documents", s.base);

    let doc =
        s.app.upload(Method::POST, &base, "notes.pdf", "application/pdf", b"%PDF-1.4 hello").await.assert(201).json();
    assert_eq!(doc["type"], "document");
    assert_eq!(doc["name"], "notes.pdf");
    assert_eq!(doc["mime_type"], "application/pdf");
    assert_eq!(doc["size"], 14);
    let row = files::Entity::find_by_id(Id(doc["id"].as_i64().unwrap())).one(&db).await.unwrap().unwrap();
    assert_eq!(row.fileable_type.as_deref(), Some("App\\Models\\Contact"));
    assert_eq!(row.ufileable_id, Some(s.contact.id));
    assert_eq!(s.app.state.storage.get(s.vault.id, &row.uuid).await.unwrap(), b"%PDF-1.4 hello");
    let res = s.app.upload(Method::POST, &base, "empty.txt", "text/plain", b"").await.assert(422).json();
    assert_eq!(fields(&res), ["file"]);

    let list = s.app.get(&base).await.assert(200).json();
    assert_eq!(len(&list["data"]), 1);
    assert_eq!(list["data"][0]["id"], doc["id"]);
    assert_eq!(list["can_upload_file"], true);
    let m = serde_json::to_value(modules::documents_module(&s.app.state, &s.ctx(&s.contact)).await.unwrap()).unwrap();
    assert_eq!((len(&m["documents"]), m["can_upload_file"].clone()), (1, json!(true)));

    // F17: only the contact's documents.
    let photo = s
        .app
        .upload(Method::POST, &format!("{}/photos", s.base), "a.png", "image/png", &png())
        .await
        .assert(201)
        .json();
    s.app.delete(&format!("{base}/{}", photo["id"])).await.assert(404);
    let other = factory::contact(&db, &s.vault).await;
    s.app.delete(&s.path_of(&other, &format!("/documents/{}", doc["id"]))).await.assert(404);

    s.login_viewer().await;
    assert_eq!(len(&s.app.get(&base).await.assert(200).json()["data"]), 1);
    s.app.upload(Method::POST, &base, "b.txt", "text/plain", b"hello").await.assert(403);
    s.app.delete(&format!("{base}/{}", doc["id"])).await.assert(403);

    s.app.login(&s.user.email).await;
    s.app.delete(&format!("{base}/{}", doc["id"])).await.assert(204);
    assert!(files::Entity::find_by_id(row.id).one(&db).await.unwrap().is_none());
    assert!(s.app.state.storage.get(s.vault.id, &row.uuid).await.is_err(), "the bytes are deleted");
    assert_eq!(len(&s.app.get(&base).await.json()["data"]), 0);

    // X011: the account's storage limit.
    let mut account: accounts::ActiveModel =
        accounts::Entity::find_by_id(s.user.account_id).one(&db).await.unwrap().unwrap().into();
    account.storage_limit_in_mb = Set(1);
    account.update(&db).await.unwrap();
    files::ActiveModel {
        vault_id: Set(s.vault.id),
        uuid: Set(uuid::Uuid::new_v4().to_string()),
        name: Set("big.bin".into()),
        mime_type: Set("application/octet-stream".into()),
        r#type: Set("document".into()),
        size: Set(2 * 1024 * 1024),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let res = s.app.upload(Method::POST, &base, "c.txt", "text/plain", b"hello").await.assert(422);
    assert_eq!(res.code(), "storage_limit_exceeded");
    assert_eq!(s.app.get(&base).await.json()["can_upload_file"], false);

    s.login_stranger().await;
    s.app.get(&base).await.assert(404);
    s.app.upload(Method::POST, &base, "x.txt", "text/plain", b"x").await.assert(404);
    s.app.delete(&format!("{base}/{}", photo["id"])).await.assert(404);
    s.app.finish().await;
}

#[tokio::test]
async fn uploads_pages_shows_and_deletes_photos() {
    let mut s = setup().await;
    let db = s.app.state.db.clone();
    let base = format!("{}/photos", s.base);

    let res = s.app.upload(Method::POST, &base, "a.txt", "text/plain", b"not an image").await.assert(422).json();
    assert_eq!(fields(&res), ["file"]);
    let mut ids = Vec::new();
    for name in ["a.png", "b.png", "c.png"] {
        let photo = s.app.upload(Method::POST, &base, name, "image/png", &png()).await.assert(201).json();
        assert_eq!((photo["type"].clone(), photo["mime_type"].clone()), (json!("photo"), json!("image/png")));
        ids.push(photo["id"].clone());
    }

    let list = s.app.get(&base).await.assert(200).json();
    assert_eq!(len(&list["data"]), 3);
    assert_eq!(list["meta"], json!({"page": 1, "per_page": 30, "total": 3, "last_page": 1}));
    assert_eq!(list["can_upload_file"], true);
    let page = s.app.get(&format!("{base}?per_page=2&page=1")).await.assert(200).json();
    assert_eq!(len(&page["data"]), 2);
    assert_eq!(page["data"][0]["id"], ids[2], "newest first");
    assert_eq!(page["meta"]["last_page"], 2);
    let m = serde_json::to_value(modules::photos_module(&s.app.state, &s.ctx(&s.contact)).await.unwrap()).unwrap();
    assert_eq!(len(&m["photos"]), 3);
    assert_eq!(m["photos"][0]["id"], ids[2]);

    let photo = s.app.get(&format!("{base}/{}", ids[0])).await.assert(200).json();
    assert_eq!(photo["name"], "a.png");
    let doc = s
        .app
        .upload(Method::POST, &format!("{}/documents", s.base), "d.txt", "text/plain", b"doc")
        .await
        .assert(201)
        .json();
    s.app.get(&format!("{base}/{}", doc["id"])).await.assert(404);
    let other = factory::contact(&db, &s.vault).await;
    s.app.get(&s.path_of(&other, &format!("/photos/{}", ids[0]))).await.assert(404);
    s.app.delete(&s.path_of(&other, &format!("/photos/{}", ids[0]))).await.assert(404);

    s.login_viewer().await;
    s.app.get(&format!("{base}/{}", ids[0])).await.assert(200);
    s.app.upload(Method::POST, &base, "e.png", "image/png", &png()).await.assert(403);
    s.app.delete(&format!("{base}/{}", ids[0])).await.assert(403);

    s.app.login(&s.user.email).await;
    s.app.delete(&format!("{base}/{}", ids[0])).await.assert(204);
    s.app.get(&format!("{base}/{}", ids[0])).await.assert(404);
    assert_eq!(s.app.get(&base).await.json()["meta"]["total"], 2);
    s.login_stranger().await;
    s.app.get(&base).await.assert(404);
    s.app.upload(Method::POST, &base, "x.png", "image/png", &png()).await.assert(404);
    s.app.get(&format!("{base}/{}", ids[1])).await.assert(404);
    s.app.delete(&format!("{base}/{}", ids[1])).await.assert(404);
    s.app.finish().await;
}

// ---------------------------------------------------------------- tasks

#[tokio::test]
async fn manages_tasks() {
    let mut s = setup().await;
    let db = s.app.state.db.clone();
    let base = format!("{}/tasks", s.base);

    assert_eq!(fields(&s.app.post(&base, json!({"label": "  "})).await.assert(422).json()), ["label"]);
    let res = s.app.post(&base, json!({"label": "x".repeat(256)})).await.assert(422).json();
    assert_eq!(codes(&res), ["max"]);

    let late = s
        .app
        .post(&base, json!({"label": "Call mom", "description": "about **dinner**", "due_at": "2020-01-01"}))
        .await
        .assert(201)
        .json();
    // F8: the description is saved.
    assert_eq!(late["description"], "about **dinner**");
    assert_eq!(late["due_at"], "2020-01-01");
    assert_eq!(late["is_late"], true);
    assert_eq!(late["completed"], false);
    assert!(late["completed_at"].is_null());
    assert_eq!(late["author_id"], s.user.id.to_string());
    assert_eq!(late["author_name"], "Ann Smith");
    let later = s.app.post(&base, json!({"label": "Later", "due_at": "2999-01-01"})).await.assert(201).json();
    assert_eq!(later["is_late"], false);
    let undated = s.app.post(&base, json!({"label": "No date"})).await.assert(201).json();
    assert!(undated["due_at"].is_null());
    assert_eq!(undated["is_late"], false);

    let list = s.app.get(&base).await.assert(200).json();
    let order: Vec<Value> = list["data"].as_array().unwrap().iter().map(|t| t["id"].clone()).collect();
    assert_eq!(order, [undated["id"].clone(), later["id"].clone(), late["id"].clone()]);

    let item = format!("{base}/{}", late["id"]);
    let t = s.app.patch(&item, json!({"label": "Call dad", "description": "lunch"})).await.assert(200).json();
    assert_eq!(
        (t["label"].clone(), t["description"].clone(), t["due_at"].clone()),
        (json!("Call dad"), json!("lunch"), json!("2020-01-01"))
    );
    let t = s.app.patch(&item, json!({"due_at": null})).await.assert(200).json();
    assert!(t["due_at"].is_null());
    assert_eq!(t["description"], "lunch");
    assert_eq!(fields(&s.app.patch(&item, json!({"label": ""})).await.assert(422).json()), ["label"]);

    let t = s.app.patch(&item, json!({"completed": true})).await.assert(200).json();
    assert_eq!(t["completed"], true);
    assert!(t["completed_at"].is_string());
    let done = s.app.get(&format!("{base}?completed=true")).await.assert(200).json();
    assert_eq!(len(&done["data"]), 1);
    assert_eq!(done["data"][0]["id"], late["id"]);
    assert_eq!(len(&s.app.get(&base).await.json()["data"]), 2);
    let m = serde_json::to_value(modules::tasks_module(&s.app.state, &s.ctx(&s.contact)).await.unwrap()).unwrap();
    assert_eq!((len(&m["tasks"]), m["completed_tasks_count"].clone()), (2, json!(1)));
    // Q6: un-completing clears completed_at.
    let t = s.app.patch(&item, json!({"completed": false})).await.assert(200).json();
    assert_eq!(t["completed"], false);
    assert!(t["completed_at"].is_null());

    let other = factory::contact(&db, &s.vault).await;
    s.app.patch(&s.path_of(&other, &format!("/tasks/{}", late["id"])), json!({"label": "x"})).await.assert(404);

    s.login_viewer().await;
    assert_eq!(len(&s.app.get(&base).await.assert(200).json()["data"]), 3);
    s.app.post(&base, json!({"label": "Nope"})).await.assert(403);
    s.app.patch(&item, json!({"completed": true})).await.assert(403);
    s.app.delete(&item).await.assert(403);

    s.app.login(&s.user.email).await;
    s.app.delete(&item).await.assert(204);
    let row = contact_tasks::Entity::find_by_id(Id(late["id"].as_i64().unwrap())).one(&db).await.unwrap().unwrap();
    assert!(row.deleted_at.is_some(), "soft deleted");
    s.app.patch(&item, json!({"label": "Back"})).await.assert(404);
    s.app.delete(&item).await.assert(404);
    s.login_stranger().await;
    s.app.get(&base).await.assert(404);
    s.app.post(&base, json!({"label": "x"})).await.assert(404);
    s.app.patch(&format!("{base}/{}", later["id"]), json!({"label": "x"})).await.assert(404);
    s.app.delete(&format!("{base}/{}", later["id"])).await.assert(404);
    s.app.finish().await;
}

// ---------------------------------------------------------------- calls

#[tokio::test]
async fn logs_and_edits_calls() {
    let mut s = setup().await;
    let db = s.app.state.db.clone();
    let base = format!("{}/calls", s.base);
    let reason_type = call_reason_types::Entity::find()
        .filter(call_reason_types::Column::AccountId.eq(s.user.account_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let reason = call_reasons::Entity::find()
        .filter(call_reasons::Column::CallReasonTypeId.eq(reason_type.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let emotion = emotions::Entity::find()
        .filter(emotions::Column::AccountId.eq(s.user.account_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let foreign_emotion = emotions::ActiveModel {
        account_id: Set(factory::account(&db).await.id),
        name: Set(Some("Elsewhere".into())),
        r#type: Set("positive".into()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();

    let res = s.app.post(&base, json!({})).await.assert(422).json();
    assert_eq!(fields(&res), ["called_at", "type", "who_initiated"]);
    let res = s
        .app
        .post(&base, json!({"called_at": "2024-05-01", "type": "fax", "who_initiated": "me"}))
        .await
        .assert(422)
        .json();
    assert_eq!((fields(&res), codes(&res)), (vec!["type".to_owned()], vec!["in".to_owned()]));
    let res = s
        .app
        .post(&base, json!({"called_at": "2024-05-01", "type": "audio", "who_initiated": "me", "emotion_id": foreign_emotion.id, "call_reason_id": 999_999}))
        .await
        .assert(422)
        .json();
    assert_eq!(fields(&res), ["call_reason_id", "emotion_id"]);

    let call = s
        .app
        .post(
            &base,
            json!({"called_at": "2024-05-01", "type": "audio", "who_initiated": "contact", "description": "chat",
                   "call_reason_id": reason.id, "emotion_id": emotion.id}),
        )
        .await
        .assert(201)
        .json();
    assert_eq!(call["called_at"], "2024-05-01");
    assert_eq!(call["answered"], true);
    assert_eq!(call["who_initiated"], "contact");
    assert_eq!(call["description"], "chat");
    assert_eq!(call["call_reason"]["id"], reason.id.0);
    assert_eq!(call["call_reason"]["call_reason_type_id"], reason_type.id.0);
    assert_eq!(call["emotion"]["id"], emotion.id.0);
    assert_eq!(call["author_name"], "Ann Smith");
    let second = s
        .app
        .post(&base, json!({"called_at": "2024-06-01", "type": "video", "who_initiated": "me", "answered": false}))
        .await
        .assert(201)
        .json();
    assert_eq!(second["answered"], false);
    assert!(second["call_reason"].is_null() && second["emotion"].is_null());

    let list = s.app.get(&base).await.assert(200).json();
    assert_eq!(list["data"][0]["id"], second["id"]);
    assert_eq!(list["data"][1]["id"], call["id"]);

    let item = format!("{base}/{}", call["id"]);
    let c = s
        .app
        .patch(&item, json!({"description": "long chat", "call_reason_id": null, "answered": false}))
        .await
        .assert(200)
        .json();
    // F8: the description is saved.
    assert_eq!(c["description"], "long chat");
    assert!(c["call_reason"].is_null());
    assert_eq!(c["answered"], false);
    assert_eq!(
        (c["type"].clone(), c["emotion"]["id"].clone(), c["called_at"].clone()),
        (json!("audio"), json!(emotion.id.0), json!("2024-05-01"))
    );
    assert_eq!(
        fields(&s.app.patch(&item, json!({"who_initiated": "nobody"})).await.assert(422).json()),
        ["who_initiated"]
    );
    s.app.patch(&item, json!({"emotion_id": foreign_emotion.id})).await.assert(422);
    let m = serde_json::to_value(modules::calls_module(&s.app.state, &s.ctx(&s.contact)).await.unwrap()).unwrap();
    assert_eq!(len(&m["calls"]), 2);

    let other = factory::contact(&db, &s.vault).await;
    s.app.patch(&s.path_of(&other, &format!("/calls/{}", call["id"])), json!({"description": "x"})).await.assert(404);

    s.login_viewer().await;
    assert_eq!(len(&s.app.get(&base).await.assert(200).json()["data"]), 2);
    s.app.post(&base, json!({"called_at": "2024-05-01", "type": "audio", "who_initiated": "me"})).await.assert(403);
    s.app.patch(&item, json!({"description": "x"})).await.assert(403);
    s.app.delete(&item).await.assert(403);

    s.app.login(&s.user.email).await;
    s.app.delete(&item).await.assert(204);
    s.app.delete(&item).await.assert(404);
    assert_eq!(len(&s.app.get(&base).await.json()["data"]), 1);
    s.login_stranger().await;
    s.app.get(&base).await.assert(404);
    s.app.post(&base, json!({"called_at": "2024-05-01", "type": "audio", "who_initiated": "me"})).await.assert(404);
    s.app.patch(&format!("{base}/{}", second["id"]), json!({"description": "x"})).await.assert(404);
    s.app.delete(&format!("{base}/{}", second["id"])).await.assert(404);
    s.app.finish().await;
}

// ---------------------------------------------------------------- groups

#[tokio::test]
async fn adds_and_removes_the_contact_from_groups() {
    let mut s = setup().await;
    let db = s.app.state.db.clone();
    let base = format!("{}/groups", s.base);
    let family = group_types::Entity::find()
        .filter(group_types::Column::AccountId.eq(s.user.account_id))
        .order_by_asc(group_types::Column::Position)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let roles = group_type_roles::Entity::find()
        .filter(group_type_roles::Column::GroupTypeId.eq(family.id))
        .order_by_asc(group_type_roles::Column::Position)
        .all(&db)
        .await
        .unwrap();
    let group = |vault_id: Uuid, name: &str, group_type_id: Option<Id>| CreateGroup {
        account_id: s.user.account_id,
        vault_id,
        author_id: s.user.id,
        group_type_id,
        name: Some(name.into()),
    };
    let band = group(s.vault.id, "Band", Some(family.id)).execute(&db).await.unwrap();
    let alpha = group(s.vault.id, "Alpha", None).execute(&db).await.unwrap();
    let zed = group(s.vault.id, "Zed", None).execute(&db).await.unwrap();
    let work = new_vault(&mut s.app, "Work").await;
    let outside = group(work.id, "Outside", None).execute(&db).await.unwrap();
    let foreign_type = group_types::ActiveModel {
        account_id: Set(factory::account(&db).await.id),
        label: Set(Some("Other".into())),
        position: Set(1),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let foreign_role = group_type_roles::ActiveModel {
        group_type_id: Set(foreign_type.id),
        label: Set(Some("Boss".into())),
        position: Set(Some(1)),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();

    assert_eq!(fields(&s.app.post(&base, json!({})).await.assert(422).json()), ["group_id"]);
    let res = s
        .app
        .post(&base, json!({"group_id": outside.id, "group_type_role_id": foreign_role.id}))
        .await
        .assert(422)
        .json();
    assert_eq!(
        (fields(&res), codes(&res)),
        (vec!["group_id".to_owned(), "group_type_role_id".to_owned()], vec!["exists".to_owned(), "exists".to_owned()])
    );

    let g = s.app.post(&base, json!({"group_id": band.id, "group_type_role_id": roles[0].id})).await.assert(201).json();
    assert_eq!(g["name"], "Band");
    assert_eq!(g["vault_id"], s.vault.id.to_string());
    assert_eq!(g["group_type"]["id"], family.id.0);
    assert_eq!(len(&g["group_type"]["roles"]), roles.len());
    assert_eq!(g["group_type"]["roles"][0]["id"], roles[0].id.0);
    assert_eq!(len(&g["members"]), 1);
    assert_eq!(g["members"][0]["id"], s.contact.id.to_string());
    assert_eq!(g["members"][0]["group_type_role_id"], roles[0].id.0);
    // Already a member: the role is replaced.
    let g = s.app.post(&base, json!({"group_id": band.id})).await.assert(201).json();
    assert_eq!(len(&g["members"]), 1);
    assert!(g["members"][0]["group_type_role_id"].is_null());
    let g = s.app.post(&base, json!({"group_id": alpha.id})).await.assert(201).json();
    assert!(g["group_type"].is_null());
    let feed = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(s.contact.id))
        .filter(contact_feed_items::Column::Action.eq("added_to_group"))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(feed.len(), 3);

    let list = s.app.get(&base).await.assert(200).json();
    let names: Vec<&str> = list["data"].as_array().unwrap().iter().map(|g| g["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Alpha", "Band"]);
    let m = serde_json::to_value(modules::groups_module(&s.app.state, &s.ctx(&s.contact)).await.unwrap()).unwrap();
    assert_eq!(len(&m["groups"]), 2);
    assert_eq!(m["available_groups"], json!([{"id": zed.id.0, "name": "Zed", "group_type_id": null}]));

    s.app.delete(&format!("{base}/{}", outside.id)).await.assert(404);
    s.login_viewer().await;
    assert_eq!(len(&s.app.get(&base).await.assert(200).json()["data"]), 2);
    s.app.post(&base, json!({"group_id": zed.id})).await.assert(403);
    s.app.delete(&format!("{base}/{}", alpha.id)).await.assert(403);

    s.app.login(&s.user.email).await;
    s.app.delete(&format!("{base}/{}", alpha.id)).await.assert(204);
    assert_eq!(len(&s.app.get(&base).await.json()["data"]), 1);
    s.app.delete(&format!("{base}/{}", alpha.id)).await.assert(404);
    s.login_stranger().await;
    s.app.get(&base).await.assert(404);
    s.app.post(&base, json!({"group_id": zed.id})).await.assert(404);
    s.app.delete(&format!("{base}/{}", band.id)).await.assert(404);
    s.app.finish().await;
}

// ---------------------------------------------------------------- timeline and life events

#[tokio::test]
async fn manages_timeline_and_life_events() {
    let mut s = setup().await;
    let db = s.app.state.db.clone();
    let base = format!("{}/timeline-events", s.base);
    let category = life_event_categories::Entity::find()
        .filter(life_event_categories::Column::VaultId.eq(s.vault.id))
        .order_by_asc(life_event_categories::Column::Position)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let types = life_event_types::Entity::find()
        .filter(life_event_types::Column::LifeEventCategoryId.eq(category.id))
        .order_by_asc(life_event_types::Column::Id)
        .all(&db)
        .await
        .unwrap();
    let work = new_vault(&mut s.app, "Work").await;
    let work_category = life_event_categories::Entity::find()
        .filter(life_event_categories::Column::VaultId.eq(work.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let work_type = life_event_types::Entity::find()
        .filter(life_event_types::Column::LifeEventCategoryId.eq(work_category.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let outsider = factory::contact(&db, &work).await;
    let friend = factory::contact(&db, &s.vault).await;
    let third = factory::contact(&db, &s.vault).await;

    assert_eq!(fields(&s.app.post(&base, json!({})).await.assert(422).json()), ["life_event"]);
    let res = s.app.post(&base, json!({"life_event": {}})).await.assert(422).json();
    assert_eq!(fields(&res), ["life_event.life_event_type_id", "life_event.happened_at"]);
    let res = s
        .app
        .post(
            &base,
            json!({"life_event": {"life_event_type_id": work_type.id, "happened_at": "2024-01-01",
                   "distance_unit": "parsecs", "currency_id": 999_999, "participant_ids": [friend.id, outsider.id]}}),
        )
        .await
        .assert(422)
        .json();
    assert_eq!(
        fields(&res),
        [
            "life_event.life_event_type_id",
            "life_event.currency_id",
            "life_event.distance_unit",
            "life_event.participant_ids.1"
        ]
    );

    let te = s
        .app
        .post(
            &base,
            json!({"label": "Trip", "life_event": {"life_event_type_id": types[0].id, "happened_at": "2024-03-10",
                   "summary": "Drove to Paris", "distance": 500, "distance_unit": "km", "participant_ids": [friend.id]}}),
        )
        .await
        .assert(201)
        .json();
    // F9: the label is saved.
    assert_eq!(te["label"], "Trip");
    assert_eq!(te["vault_id"], s.vault.id.to_string());
    assert_eq!(te["started_at"], "2024-03-10");
    assert_eq!(te["collapsed"], true, "the column's default");
    assert_eq!(
        (te["first_happened_at"].clone(), te["last_happened_at"].clone()),
        (json!("2024-03-10"), json!("2024-03-10"))
    );
    assert_eq!(len(&te["life_events"]), 1);
    let first = &te["life_events"][0];
    assert_eq!(first["summary"], "Drove to Paris");
    assert_eq!((first["distance"].clone(), first["distance_unit"].clone()), (json!(500), json!("km")));
    assert_eq!(first["life_event_type"]["id"], types[0].id.0);
    assert_eq!(first["life_event_category"]["id"], category.id.0);
    assert_eq!(first["timeline_event_id"], te["id"]);
    let mut people: Vec<String> =
        first["participants"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap().to_owned()).collect();
    people.sort();
    let mut expected = vec![s.contact.id.to_string(), friend.id.to_string()];
    expected.sort();
    assert_eq!(people, expected, "the contact in the path is a participant");
    assert!(first["participants"][0]["groups"].is_array());

    let item = format!("{base}/{}", te["id"]);
    let second = s
        .app
        .post(&format!("{item}/life-events"), json!({"life_event_type_id": types[1].id, "happened_at": "2024-03-12"}))
        .await
        .assert(201)
        .json();
    assert_eq!(second["timeline_event_id"], te["id"]);
    assert_eq!(len(&second["participants"]), 1);
    assert_eq!(second["participants"][0]["id"], s.contact.id.to_string());
    let res = s.app.post(&format!("{item}/life-events"), json!({"happened_at": "2024-03-12"})).await.assert(422).json();
    assert_eq!(fields(&res), ["life_event_type_id"]);

    let list = s.app.get(&base).await.assert(200).json();
    assert_eq!(list["meta"], json!({"page": 1, "per_page": 15, "total": 1, "last_page": 1}));
    let t = &list["data"][0];
    assert_eq!(len(&t["life_events"]), 2);
    assert_eq!(
        (t["first_happened_at"].clone(), t["last_happened_at"].clone()),
        (json!("2024-03-10"), json!("2024-03-12"))
    );
    assert_eq!(t["life_events"][0]["id"], first["id"], "in creation order");
    assert_eq!(len(&s.app.get(&s.path_of(&friend, "/timeline-events")).await.json()["data"]), 1);
    let m = serde_json::to_value(modules::life_events_module(&s.app.state, &s.ctx(&s.contact)).await.unwrap()).unwrap();
    assert_eq!((len(&m["timeline_events"]), m["meta"]["total"].clone()), (1, json!(1)));

    // F17: a contact who doesn't take part gets a 404.
    assert_eq!(len(&s.app.get(&s.path_of(&third, "/timeline-events")).await.json()["data"]), 0);
    let theirs = s.path_of(&third, &format!("/timeline-events/{}", te["id"]));
    s.app.patch(&theirs, json!({"collapsed": true})).await.assert(404);
    s.app
        .post(&format!("{theirs}/life-events"), json!({"life_event_type_id": types[0].id, "happened_at": "2024-01-01"}))
        .await
        .assert(404);
    s.app.patch(&format!("{theirs}/life-events/{}", first["id"]), json!({"summary": "x"})).await.assert(404);
    s.app.delete(&format!("{theirs}/life-events/{}", first["id"])).await.assert(404);
    s.app.delete(&theirs).await.assert(404);

    let t = s.app.patch(&item, json!({"collapsed": false})).await.assert(200).json();
    assert_eq!(t["collapsed"], false);
    assert_eq!(len(&t["life_events"]), 2);
    assert_eq!(fields(&s.app.patch(&item, json!({})).await.assert(422).json()), ["collapsed"]);

    let life = format!("{item}/life-events/{}", first["id"]);
    let l = s.app.patch(&life, json!({"summary": "Flew", "collapsed": true})).await.assert(200).json();
    assert_eq!((l["summary"].clone(), l["collapsed"].clone()), (json!("Flew"), json!(true)));
    assert_eq!(len(&l["participants"]), 2, "the participants are kept");
    assert_eq!((l["distance"].clone(), l["happened_at"].clone()), (json!(500), json!("2024-03-10")));
    let l = s.app.patch(&life, json!({"participant_ids": [], "distance": null})).await.assert(200).json();
    assert_eq!(len(&l["participants"]), 1);
    assert_eq!(l["participants"][0]["id"], s.contact.id.to_string());
    assert!(l["distance"].is_null());
    assert_eq!(l["collapsed"], true);
    let l = s.app.patch(&life, json!({"collapsed": false})).await.assert(200).json();
    assert_eq!((l["collapsed"].clone(), l["summary"].clone()), (json!(false), json!("Flew")));
    let res = s
        .app
        .patch(&life, json!({"currency_id": 999_999, "life_event_type_id": work_type.id}))
        .await
        .assert(422)
        .json();
    assert_eq!(fields(&res), ["life_event_type_id", "currency_id"]);
    s.app.patch(&format!("{item}/life-events/999999"), json!({"summary": "x"})).await.assert(404);

    s.login_viewer().await;
    assert_eq!(len(&s.app.get(&base).await.assert(200).json()["data"]), 1);
    s.app
        .post(&base, json!({"life_event": {"life_event_type_id": types[0].id, "happened_at": "2024-01-01"}}))
        .await
        .assert(403);
    s.app.patch(&item, json!({"collapsed": false})).await.assert(403);
    s.app
        .post(&format!("{item}/life-events"), json!({"life_event_type_id": types[0].id, "happened_at": "2024-01-01"}))
        .await
        .assert(403);
    s.app.patch(&life, json!({"summary": "x"})).await.assert(403);
    s.app.delete(&life).await.assert(403);
    s.app.delete(&item).await.assert(403);

    s.app.login(&s.user.email).await;
    // Deleting the last life event deletes the timeline event.
    s.app.delete(&format!("{item}/life-events/{}", second["id"])).await.assert(204);
    s.app.delete(&life).await.assert(204);
    let id = Id(te["id"].as_i64().unwrap());
    assert!(timeline_events::Entity::find_by_id(id).one(&db).await.unwrap().is_none());
    assert_eq!(len(&s.app.get(&base).await.json()["data"]), 0);
    let te = s
        .app
        .post(&base, json!({"life_event": {"life_event_type_id": types[0].id, "happened_at": "2024-04-01"}}))
        .await
        .assert(201)
        .json();
    assert!(te["label"].is_null());
    s.app.delete(&format!("{base}/{}", te["id"])).await.assert(204);
    s.app.delete(&format!("{base}/{}", te["id"])).await.assert(404);
    s.login_stranger().await;
    s.app.get(&base).await.assert(404);
    s.app
        .post(&base, json!({"life_event": {"life_event_type_id": types[0].id, "happened_at": "2024-01-01"}}))
        .await
        .assert(404);
    s.app.patch(&format!("{base}/{}", te["id"]), json!({"collapsed": true})).await.assert(404);
    s.app.delete(&format!("{base}/{}", te["id"])).await.assert(404);
    s.app.post(&format!("{base}/{}/life-events", te["id"]), json!({})).await.assert(404);
    s.app.patch(&format!("{base}/{}/life-events/1", te["id"]), json!({})).await.assert(404);
    s.app.delete(&format!("{base}/{}/life-events/1", te["id"])).await.assert(404);
    s.app.finish().await;
}

// ---------------------------------------------------------------- mood tracking events

#[tokio::test]
async fn records_mood_tracking_events() {
    let mut s = setup().await;
    let db = s.app.state.db.clone();
    let base = format!("{}/mood-tracking-events", s.base);
    let parameter = mood_tracking_parameters::Entity::find()
        .filter(mood_tracking_parameters::Column::VaultId.eq(s.vault.id))
        .order_by_asc(mood_tracking_parameters::Column::Position)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let work = new_vault(&mut s.app, "Work").await;
    let elsewhere = mood_tracking_parameters::Entity::find()
        .filter(mood_tracking_parameters::Column::VaultId.eq(work.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();

    let res = s.app.post(&base, json!({})).await.assert(422).json();
    assert_eq!(fields(&res), ["mood_tracking_parameter_id", "rated_at"]);
    let res = s
        .app
        .post(&base, json!({"mood_tracking_parameter_id": elsewhere.id, "rated_at": "2024-01-01"}))
        .await
        .assert(422)
        .json();
    assert_eq!(codes(&res), ["exists"]);
    let res = s
        .app
        .post(&base, json!({"mood_tracking_parameter_id": parameter.id, "rated_at": "2024-01-01", "number_of_hours_slept": 99_999_999_999_i64}))
        .await
        .assert(422)
        .json();
    assert_eq!(fields(&res), ["number_of_hours_slept"]);

    let event = s
        .app
        .post(&base, json!({"mood_tracking_parameter_id": parameter.id, "rated_at": "2024-01-02", "note": " fine ", "number_of_hours_slept": 7}))
        .await
        .assert(201)
        .json();
    assert_eq!(event["contact_id"], s.contact.id.to_string());
    assert_eq!(event["rated_at"], "2024-01-02");
    assert_eq!(event["note"], "fine");
    assert_eq!(event["number_of_hours_slept"], 7);
    assert_eq!(event["mood_tracking_parameter"]["id"], parameter.id.0);
    assert_eq!(event["mood_tracking_parameter"]["hex_color"], parameter.hex_color);
    let feed = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(s.contact.id))
        .filter(contact_feed_items::Column::Action.eq("mood_tracking_event_added"))
        .all(&db)
        .await
        .unwrap();
    assert_eq!(feed.len(), 1);
    s.app
        .post(&format!("/vaults/{}/contacts/{}/mood-tracking-events", s.vault.id, uuid::Uuid::new_v4()), json!({}))
        .await
        .assert(404);

    s.login_viewer().await;
    s.app.post(&base, json!({"mood_tracking_parameter_id": parameter.id, "rated_at": "2024-01-02"})).await.assert(403);
    s.login_stranger().await;
    s.app.post(&base, json!({"mood_tracking_parameter_id": parameter.id, "rated_at": "2024-01-02"})).await.assert(404);
    s.app.finish().await;
}
