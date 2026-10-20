//! A contact's notes over HTTP, and the notes module of the contact page.

mod common;

use common::TestApp;
use monica::api::contact_page::ModuleContext;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::domains::settings::create_account::SetupAccount;
use monica::entities::{contact_feed_items, contacts, emotions, notes, users, vaults};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};

/// A set-up account's administrator, logged in, with a vault and a contact
/// made through the API.
struct World {
    app: TestApp,
    user: users::Model,
    vault: String,
    own_contact: String,
    contact: String,
}

async fn world() -> World {
    let mut app = TestApp::new().await;
    let user = factory::create_administrator(&app.state.db).await;
    SetupAccount { account_id: user.account_id, author_id: user.id }
        .execute(&app.state.db)
        .await
        .expect("setup account");
    app.login(&user.username).await;
    let v = app.post("/vaults", json!({"name": "Family"})).await.assert(201).json();
    let vault = v["id"].as_str().unwrap().to_owned();
    let c = app.post(&format!("/vaults/{vault}/contacts"), json!({"first_name": "Jane"})).await.assert(201).json();
    World {
        own_contact: v["user_contact_id"].as_str().unwrap().to_owned(),
        contact: c["id"].as_str().unwrap().to_owned(),
        vault,
        app,
        user,
    }
}

impl World {
    fn notes(&self) -> String {
        format!("/vaults/{}/contacts/{}/notes", self.vault, self.contact)
    }

    fn note(&self, note: &Value) -> String {
        format!("{}/{}", self.notes(), note["id"])
    }

    async fn emotion(&self, account_id: Uuid) -> emotions::Model {
        emotions::Entity::find()
            .filter(emotions::Column::AccountId.eq(account_id))
            .one(&self.app.state.db)
            .await
            .unwrap()
            .unwrap()
    }

    async fn member(&self, permission: VaultPermission) -> users::Model {
        let db = &self.app.state.db;
        let u = factory::user(db, Some(self.user.account_id)).await;
        let vault = vaults::Entity::find_by_id(self.vault.parse::<Uuid>().unwrap()).one(db).await.unwrap().unwrap();
        factory::set_permission_in_vault(db, &u, permission, &vault).await;
        u
    }

    async fn contact_model(&self) -> contacts::Model {
        contacts::Entity::find_by_id(self.contact.parse::<Uuid>().unwrap())
            .one(&self.app.state.db)
            .await
            .unwrap()
            .unwrap()
    }
}

fn first_error(res: &Value) -> (String, String) {
    let e = &res["errors"][0];
    (e["field"].as_str().unwrap().to_owned(), e["code"].as_str().unwrap().to_owned())
}

#[tokio::test]
async fn creates_lists_updates_and_deletes_notes() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let emotion = w.emotion(w.user.account_id).await;
    let other_account = factory::create_administrator(&db).await;
    SetupAccount { account_id: other_account.account_id, author_id: other_account.id }.execute(&db).await.unwrap();
    let foreign_emotion = w.emotion(other_account.account_id).await;
    let base = w.notes();

    let res = w.app.post(&base, json!({"title": "Empty", "body": "   "})).await.assert(422).json();
    assert_eq!(first_error(&res), ("body".into(), "required".into()));
    let res = w.app.post(&base, json!({"title": "t".repeat(256), "body": "b"})).await.assert(422).json();
    assert_eq!(first_error(&res), ("title".into(), "max".into()));
    let res = w.app.post(&base, json!({"body": "b".repeat(65_536)})).await.assert(422).json();
    assert_eq!(first_error(&res), ("body".into(), "max".into()));
    let res = w.app.post(&base, json!({"body": "b", "emotion_id": foreign_emotion.id})).await.assert(422).json();
    assert_eq!(first_error(&res), ("emotion_id".into(), "exists".into()));
    w.app.post(&base, json!({"body": "b", "emotion_id": 999_999})).await.assert(422);

    let before = w.contact_model().await.last_updated_at;
    let note = w
        .app
        .post(&base, json!({"title": " Hi ", "body": " First body ", "emotion_id": emotion.id}))
        .await
        .assert(201)
        .json();
    assert_eq!((note["title"].as_str(), note["body"].as_str()), (Some("Hi"), Some("First body")));
    assert_eq!(note["contact_id"], w.contact);
    assert_eq!(note["emotion"]["id"], emotion.id.0);
    assert_eq!(note["emotion"]["type"], emotion.r#type);
    assert_eq!(note["emotion"]["name_translation_key"], json!(emotion.name_translation_key));
    assert_eq!(note["author"]["id"], w.user.id.to_string());
    assert_eq!(note["author"]["first_name"], json!(w.user.first_name));
    assert_eq!(note["author"]["contact"]["id"], w.own_contact, "the author's contact in the vault");
    assert!(note["created_at"].as_str().unwrap().ends_with('Z'));
    assert!(w.contact_model().await.last_updated_at >= before, "X013");
    let item = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::Action.eq("note_created"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(item.feedable_id.map(|i| i.0), note["id"].as_i64());

    for i in 0..11 {
        w.app.post(&base, json!({"body": format!("Note {i}")})).await.assert(201);
    }
    let list = w.app.get(&base).await.assert(200).json();
    assert_eq!(list["meta"], json!({"page": 1, "per_page": 10, "total": 12, "last_page": 2}));
    assert_eq!(list["data"][0]["body"], "Note 10", "newest first");
    assert!(list["data"][0]["title"].is_null() && list["data"][0]["emotion"].is_null());
    let last = w.app.get(&format!("{base}?page=2")).await.assert(200).json();
    assert_eq!(last["data"].as_array().unwrap().len(), 2);
    assert_eq!(last["data"][1]["id"], note["id"]);
    assert_eq!(w.app.get(&format!("{base}?per_page=3")).await.json()["data"].as_array().unwrap().len(), 3);

    let path = w.note(&note);
    let u = w.app.patch(&path, json!({"title": null})).await.assert(200).json();
    assert!(u["title"].is_null());
    assert_eq!(
        (u["body"].as_str(), u["emotion"]["id"].as_i64()),
        (Some("First body"), Some(emotion.id.0)),
        "absent fields are kept"
    );
    let u = w.app.patch(&path, json!({"body": "New body", "emotion_id": null})).await.assert(200).json();
    assert_eq!(u["body"], "New body");
    assert!(u["emotion"].is_null());
    let res = w.app.patch(&path, json!({"body": ""})).await.assert(422).json();
    assert_eq!(first_error(&res), ("body".into(), "required".into()));
    let res = w.app.patch(&path, json!({"emotion_id": foreign_emotion.id})).await.assert(422).json();
    assert_eq!(first_error(&res), ("emotion_id".into(), "exists".into()));
    w.app.patch(&format!("{base}/999999"), json!({"body": "x"})).await.assert(404);
    let other =
        w.app.post(&format!("/vaults/{}/contacts", w.vault), json!({"first_name": "Other"})).await.assert(201).json();
    let elsewhere = format!("/vaults/{}/contacts/{}/notes/{}", w.vault, other["id"].as_str().unwrap(), note["id"]);
    w.app.patch(&elsewhere, json!({"body": "x"})).await.assert(404);
    w.app.delete(&elsewhere).await.assert(404);

    // An author who was deleted since is null.
    let mut row: notes::ActiveModel = notes::Entity::find_by_id(monica::db::id::Id(note["id"].as_i64().unwrap()))
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into();
    row.author_id = Set(None);
    row.update(&db).await.unwrap();
    let list = w.app.get(&format!("{base}?page=2")).await.json();
    assert!(list["data"][1]["author"].is_null());

    w.app.delete(&path).await.assert(204);
    w.app.delete(&path).await.assert(404);
    assert_eq!(w.app.get(&base).await.json()["meta"]["total"], 11);
    let destroyed = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::Action.eq("note_destroyed"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(destroyed.description.as_deref(), Some("New body"));
    w.app.finish().await;
}

#[tokio::test]
async fn viewers_read_notes_and_strangers_see_nothing() {
    let mut w = world().await;
    let base = w.notes();
    let note = w.app.post(&base, json!({"body": "Hello"})).await.assert(201).json();
    let path = w.note(&note);
    let viewer = w.member(VaultPermission::View).await;
    let stranger = factory::create_user(&w.app.state.db).await;

    w.app.login(&viewer.username).await;
    let list = w.app.get(&base).await.assert(200).json();
    assert_eq!(list["data"][0]["id"], note["id"]);
    assert_eq!(w.app.post(&base, json!({"body": "No"})).await.assert(403).code(), "forbidden");
    w.app.patch(&path, json!({"body": "No"})).await.assert(403);
    w.app.delete(&path).await.assert(403);

    w.app.login(&stranger.username).await;
    w.app.get(&base).await.assert(404);
    w.app.post(&base, json!({"body": "No"})).await.assert(404);
    w.app.patch(&path, json!({"body": "No"})).await.assert(404);
    w.app.delete(&path).await.assert(404);

    let email = w.user.username.clone();
    w.app.login(&email).await;
    let missing = format!("/vaults/{}/contacts/{}/notes", w.vault, Uuid::new());
    w.app.get(&missing).await.assert(404);
    w.app.post(&missing, json!({"body": "x"})).await.assert(404);
    w.app.finish().await;
}

#[tokio::test]
async fn builds_the_notes_module() {
    let mut w = world().await;
    let base = w.notes();
    for i in 0..5 {
        w.app.post(&base, json!({"body": format!("Note {i}")})).await.assert(201);
    }
    let ctx = ModuleContext {
        user: w.user.clone(),
        vault: vaults::Entity::find_by_id(w.vault.parse::<Uuid>().unwrap())
            .one(&w.app.state.db)
            .await
            .unwrap()
            .unwrap(),
        contact: w.contact_model().await,
    };
    let module =
        serde_json::to_value(monica::api::contact_notes::notes_module(&w.app.state, &ctx).await.unwrap()).unwrap();
    assert_eq!(module["total"], 5);
    let first_page = w.app.get(&format!("{base}?per_page=3")).await.json();
    assert_eq!(module["notes"], first_page["data"], "as listContactNotes?per_page=3");
    assert_eq!(module["notes"][0]["body"], "Note 4");
    w.app.finish().await;
}
