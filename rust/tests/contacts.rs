//! Contacts over HTTP: the list and search, create/read/update/delete,
//! views and most consulted contacts, moves, avatars, quick facts, the
//! feed, and the contact page module data of this area.

mod common;

use axum::http::Method;
use chrono::NaiveDate;
use common::TestApp;
use monica::api::contact_page::ModuleContext;
use monica::db::id::Id;
use monica::db::timestamp::Timestamp;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::domains::settings::create_account::SetupAccount;
use monica::entities::{
    address_types, addresses, contact_feed_items, contact_information, contact_information_types, contact_label,
    contact_post, contact_vault_user, contacts, files, genders, goals, journals, labels, mood_tracking_events,
    mood_tracking_parameters, pet_categories, pets, post_sections, posts, pronouns, relationship_group_types,
    relationship_types, relationships, streaks, templates, users, vault_quick_facts_templates, vaults,
};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{Value, json};

/// A set-up account's administrator, logged in, with a vault made through
/// the API (default template, quick fact templates, the user's contact).
struct World {
    app: TestApp,
    user: users::Model,
    vault: String,
    own_contact: String,
}

async fn world() -> World {
    world_with(|_| {}).await
}

async fn world_with(config: impl FnOnce(&mut monica::config::Config)) -> World {
    let mut app = TestApp::with_config(config).await;
    let user = factory::create_administrator(&app.state.db).await;
    SetupAccount { account_id: user.account_id, author_id: user.id }
        .execute(&app.state.db)
        .await
        .expect("setup account");
    app.login(&user.username).await;
    let v = app.post("/vaults", json!({"name": "Family"})).await.assert(201).json();
    World {
        vault: v["id"].as_str().unwrap().to_owned(),
        own_contact: v["user_contact_id"].as_str().unwrap().to_owned(),
        app,
        user,
    }
}

impl World {
    fn path(&self, rest: &str) -> String {
        format!("/vaults/{}{rest}", self.vault)
    }

    fn vault_id(&self) -> Uuid {
        self.vault.parse().unwrap()
    }

    async fn vault_model(&self) -> vaults::Model {
        vaults::Entity::find_by_id(self.vault_id()).one(&self.app.state.db).await.unwrap().unwrap()
    }

    async fn contact(&mut self, body: Value) -> Value {
        let path = self.path("/contacts");
        self.app.post(&path, body).await.assert(201).json()
    }

    /// Another user of the account with `permission` in the vault.
    async fn member(&self, permission: VaultPermission) -> users::Model {
        let u = factory::user(&self.app.state.db, Some(self.user.account_id)).await;
        factory::set_permission_in_vault(&self.app.state.db, &u, permission, &self.vault_model().await).await;
        u
    }

    async fn login_back(&mut self) {
        let email = self.user.username.clone();
        self.app.login(&email).await;
    }

    async fn feed_actions(&self, contact_id: &str) -> Vec<String> {
        contact_feed_items::Entity::find()
            .filter(contact_feed_items::Column::ContactId.eq(contact_id.parse::<Uuid>().unwrap()))
            .order_by_asc(contact_feed_items::Column::Id)
            .all(&self.app.state.db)
            .await
            .unwrap()
            .into_iter()
            .map(|i| i.action)
            .collect()
    }

    async fn contact_model(&self, id: &str) -> contacts::Model {
        contacts::Entity::find_by_id(id.parse::<Uuid>().unwrap()).one(&self.app.state.db).await.unwrap().unwrap()
    }

    async fn ctx(&self, contact_id: &str) -> ModuleContext {
        ModuleContext {
            user: self.user.clone(),
            vault: self.vault_model().await,
            contact: self.contact_model(contact_id).await,
        }
    }
}

fn ids(list: &Value) -> Vec<String> {
    list["data"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_owned()).collect()
}

fn v(xs: &[&String]) -> Vec<String> {
    xs.iter().map(|x| (*x).clone()).collect()
}

fn to_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

fn s(v: &Value) -> String {
    v["id"].as_str().unwrap().to_owned()
}

fn at(year: i32) -> Timestamp {
    Timestamp(NaiveDate::from_ymd_opt(year, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap())
}

fn png(w: u32, h: u32, blue: u8) -> Vec<u8> {
    let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 255) as u8, (y % 255) as u8, blue]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

async fn set_times(w: &World, id: &str, created: i32, updated: i32) {
    let mut c: contacts::ActiveModel = w.contact_model(id).await.into();
    c.created_at = Set(Some(at(created)));
    c.last_updated_at = Set(Some(at(updated)));
    c.update(&w.app.state.db).await.unwrap();
}

#[tokio::test]
async fn creates_reads_updates_and_deletes_a_contact() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let base = w.path("/contacts");
    let gender = genders::Entity::find()
        .filter(genders::Column::AccountId.eq(w.user.account_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let foreign_gender = factory::gender(&db, factory::create_account(&db).await.id).await;

    let res = w.app.post(&base, json!({"first_name": "x".repeat(256)})).await.assert(422).json();
    assert_eq!(
        (res["errors"][0]["field"].as_str(), res["errors"][0]["code"].as_str()),
        (Some("first_name"), Some("max"))
    );
    let res = w.app.post(&base, json!({"first_name": "A", "gender_id": foreign_gender.id})).await.assert(422).json();
    assert_eq!(
        (res["errors"][0]["field"].as_str(), res["errors"][0]["code"].as_str()),
        (Some("gender_id"), Some("exists"))
    );

    let c =
        w.contact(json!({"first_name": " John ", "last_name": "Doe", "nickname": "", "gender_id": gender.id})).await;
    assert_eq!(c["first_name"], "John");
    assert_eq!(c["last_name"], "Doe");
    assert!(c["nickname"].is_null());
    assert_eq!(c["gender_id"], gender.id.0);
    assert_eq!(c["vault_id"], w.vault);
    assert_eq!((c["listed"].as_bool(), c["is_favorite"].as_bool()), (Some(true), Some(false)));
    assert_eq!((c["can_be_deleted"].as_bool(), c["show_quick_facts"].as_bool()), (Some(true), Some(false)));
    assert_eq!(c["template_id"], w.vault_model().await.default_template_id.unwrap().0, "the vault's default template");
    assert!(c["last_updated_at"].is_string());
    assert!(c["avatar_file_id"].is_null());
    let path = format!("{base}/{}", s(&c));
    let got = w.app.get(&path).await.assert(200).json();
    assert_eq!((got["id"].clone(), got["first_name"].clone()), (c["id"].clone(), c["first_name"].clone()));

    let u = w.app.patch(&path, json!({"nickname": "JD"})).await.assert(200).json();
    assert_eq!(
        (u["first_name"].as_str(), u["last_name"].as_str(), u["nickname"].as_str()),
        (Some("John"), Some("Doe"), Some("JD"))
    );
    assert_eq!(u["gender_id"], gender.id.0, "absent fields are unchanged");
    let res = w.app.patch(&path, json!({"first_name": " "})).await.assert(422).json();
    assert_eq!(res["errors"][0]["code"], "required");
    w.app.patch(&path, json!({"middle_name": "m".repeat(256)})).await.assert(422);
    w.app.patch(&path, json!({"pronoun_id": 999_999})).await.assert(422);
    let u = w.app.patch(&path, json!({"last_name": null, "gender_id": null})).await.assert(200).json();
    assert!(u["last_name"].is_null() && u["gender_id"].is_null());
    assert_eq!(u["nickname"], "JD");

    let u = w.app.patch(&path, json!({"is_favorite": true, "show_quick_facts": true})).await.assert(200).json();
    assert_eq!((u["is_favorite"].as_bool(), u["show_quick_facts"].as_bool()), (Some(true), Some(true)));
    w.app.patch(&path, json!({"is_favorite": true, "show_quick_facts": true})).await.assert(200);
    let u = w.app.patch(&path, json!({"listed": false})).await.assert(200).json();
    assert_eq!(u["listed"], false);
    assert_eq!(
        w.feed_actions(&s(&c)).await,
        ["contact_created", "information_updated", "information_updated", "favorited", "archived"],
        "F6: the first favorite is `favorited`; equal booleans change nothing"
    );
    let u = w.app.patch(&path, json!({"is_favorite": false, "listed": true})).await.assert(200).json();
    assert_eq!((u["is_favorite"].as_bool(), u["listed"].as_bool()), (Some(false), Some(true)));

    let template = templates::ActiveModel {
        account_id: Set(w.user.account_id),
        name: Set(Some("Other".into())),
        can_be_deleted: Set(true),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let foreign = factory::create_account(&db).await;
    let foreign_template = templates::ActiveModel {
        account_id: Set(foreign.id),
        name: Set(Some("Theirs".into())),
        can_be_deleted: Set(true),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    assert_eq!(
        w.app.patch(&path, json!({"template_id": template.id})).await.assert(200).json()["template_id"],
        template.id.0
    );
    let res = w.app.patch(&path, json!({"template_id": foreign_template.id})).await.assert(422).json();
    assert_eq!(
        (res["errors"][0]["field"].as_str(), res["errors"][0]["code"].as_str()),
        (Some("template_id"), Some("exists"))
    );

    w.app.delete(&path).await.assert(204);
    w.app.get(&path).await.assert(404);
    w.app.patch(&path, json!({"nickname": "x"})).await.assert(404);
    w.app.delete(&path).await.assert(404);
    let own = w.path(&format!("/contacts/{}", w.own_contact));
    assert_eq!(w.app.delete(&own).await.assert(409).code(), "conflict", "a user's own contact");
    w.app.finish().await;
}

#[tokio::test]
async fn viewers_read_contacts_and_strangers_see_nothing() {
    let mut w = world().await;
    let c = w.contact(json!({"first_name": "Jane"})).await;
    let path = w.path(&format!("/contacts/{}", s(&c)));
    let viewer = w.member(VaultPermission::View).await;
    let stranger = factory::create_user(&w.app.state.db).await;

    w.app.login(&viewer.username).await;
    w.app.get(&path).await.assert(200);
    w.app.get(&w.path("/contacts")).await.assert(200);
    assert_eq!(w.app.post(&w.path("/contacts"), json!({"first_name": "No"})).await.assert(403).code(), "forbidden");
    w.app.patch(&path, json!({"is_favorite": true})).await.assert(403);
    w.app.delete(&path).await.assert(403);
    w.app.post(&format!("{path}/move"), json!({"other_vault_id": w.vault})).await.assert(403);

    w.app.login(&stranger.username).await;
    w.app.get(&path).await.assert(404);
    w.app.get(&w.path("/contacts")).await.assert(404);
    w.app.post(&w.path("/contacts"), json!({"first_name": "No"})).await.assert(404);
    w.app.get(&w.path("/contacts/most-consulted")).await.assert(404);

    w.login_back().await;
    w.app.get(&w.path(&format!("/contacts/{}", Uuid::new()))).await.assert(404);
    w.app.finish().await;
}

#[tokio::test]
async fn lists_searches_sorts_and_filters_contacts() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let base = w.path("/contacts");
    let alice = s(&w.contact(json!({"first_name": "Alice", "last_name": "Zed"})).await);
    let bob = s(&w.contact(json!({"first_name": "Bob", "last_name": "Young", "listed": false})).await);
    let carl = s(&w.contact(json!({"first_name": "Carl", "last_name": "Xavier", "nickname": "Ceej"})).await);
    let own = w.own_contact.clone();
    set_times(&w, &own, 2020, 2020).await;
    set_times(&w, &alice, 2021, 2024).await;
    set_times(&w, &bob, 2022, 2022).await;
    set_times(&w, &carl, 2023, 2023).await;

    let list = w.app.get(&base).await.assert(200).json();
    assert_eq!(ids(&list), v(&[&alice, &carl, &bob, &own]), "the user's sort order: last updated first");
    assert_eq!(list["meta"], json!({"page": 1, "per_page": 25, "total": 4, "last_page": 1}));
    assert_eq!(list["data"][0]["first_name"], "Alice");
    assert_eq!(list["data"][0]["listed"], true);
    assert!(list["data"][0].get("is_favorite").is_none(), "ContactSummary");
    assert_eq!(ids(&w.app.get(&format!("{base}?listed=false")).await.assert(200).json()), v(&[&bob]));
    assert_eq!(ids(&w.app.get(&format!("{base}?listed=true")).await.json()), v(&[&alice, &carl, &own]));

    assert_eq!(ids(&w.app.get(&format!("{base}?q=ALI")).await.assert(200).json()), v(&[&alice]), "X019, any case");
    assert_eq!(ids(&w.app.get(&format!("{base}?q=avie")).await.json()), v(&[&carl]), "last name");
    assert_eq!(
        ids(&w.app.get(&format!("{base}?q=ceej&sort=first_name&per_page=5")).await.json()),
        v(&[&carl]),
        "nickname"
    );
    assert!(ids(&w.app.get(&format!("{base}?q=zzz")).await.json()).is_empty());
    assert_eq!(ids(&w.app.get(&format!("{base}?q=%20%20")).await.json()).len(), 4, "a blank search doesn't filter");

    let page = w.app.get(&format!("{base}?sort=first_name&per_page=2")).await.assert(200).json();
    assert_eq!(ids(&page), v(&[&alice, &bob]));
    assert_eq!(page["meta"], json!({"page": 1, "per_page": 2, "total": 4, "last_page": 2}));
    assert_eq!(ids(&w.app.get(&format!("{base}?sort=first_name&per_page=2&page=2")).await.json()), v(&[&carl, &own]));
    assert_eq!(ids(&w.app.get(&format!("{base}?sort=created_at")).await.json()), v(&[&own, &alice, &bob, &carl]));
    assert_eq!(
        ids(&w.app.get(&format!("{base}?sort=desc")).await.json()),
        v(&[&own, &carl, &bob, &alice]),
        "first names Z to A"
    );
    assert_eq!(ids(&w.app.get(&format!("{base}?sort=asc")).await.json()), v(&[&alice, &bob, &carl, &own]));
    let mut u: users::ActiveModel = w.user.clone().into();
    u.name_order = Set("%last_name% %first_name%".into());
    u.contact_sort_order = Set("asc".into());
    u.update(&db).await.unwrap();
    assert_eq!(
        ids(&w.app.get(&base).await.json()),
        v(&[&own, &carl, &bob, &alice]),
        "the saved `asc` order, by the first part of the name order (last names; the user's is `Name …`)"
    );
    let res = w.app.get(&format!("{base}?sort=nope")).await.assert(422).json();
    assert_eq!((res["errors"][0]["field"].as_str(), res["errors"][0]["code"].as_str()), (Some("sort"), Some("in")));

    let label = labels::ActiveModel {
        vault_id: Set(w.vault_id()),
        name: Set("Friends".into()),
        slug: Set("friends".into()),
        bg_color: Set("bg-neutral-200".into()),
        text_color: Set("text-neutral-800".into()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    contact_label::ActiveModel { label_id: Set(label.id), contact_id: Set(bob.parse().unwrap()), ..Default::default() }
        .insert(&db)
        .await
        .unwrap();
    let other_vault = factory::create_vault(&db, w.user.account_id).await;
    let foreign_label = labels::ActiveModel {
        vault_id: Set(other_vault.id),
        name: Set("Other".into()),
        slug: Set("other".into()),
        bg_color: Set("bg-neutral-200".into()),
        text_color: Set("text-neutral-800".into()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    assert_eq!(
        ids(&w.app.get(&format!("{base}?label_id={}&sort=created_at&per_page=10", label.id)).await.json()),
        v(&[&bob])
    );
    let res = w.app.get(&format!("{base}?label_id={}", foreign_label.id)).await.assert(422).json();
    assert_eq!(
        (res["errors"][0]["field"].as_str(), res["errors"][0]["code"].as_str()),
        (Some("label_id"), Some("exists"))
    );
    w.app.get(&format!("{base}?label_id=999999")).await.assert(422);

    // Deleted contacts are gone from the list.
    w.app.delete(&w.path(&format!("/contacts/{carl}"))).await.assert(204);
    assert_eq!(w.app.get(&base).await.json()["meta"]["total"], 3);
    w.app.finish().await;
}

#[tokio::test]
async fn counts_views_for_the_most_consulted_contacts() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let mut made = Vec::new();
    for name in ["A", "B", "C", "D", "E", "F"] {
        made.push(s(&w.contact(json!({"first_name": name})).await));
    }
    let contacts_path = w.path("/contacts");
    let view = |id: &str| format!("{contacts_path}/{id}/view");
    for (i, id) in made.iter().enumerate() {
        for _ in 0..=i {
            let p = view(id);
            w.app.post(&p, json!({})).await.assert(204);
        }
    }
    let most = w.app.get(&w.path("/contacts/most-consulted")).await.assert(200).json();
    let expected: Vec<String> = made.iter().rev().take(5).cloned().collect();
    assert_eq!(ids(&most), expected, "five at most, most viewed first");
    assert_eq!(most["data"][0]["first_name"], "F");
    let row = contact_vault_user::Entity::find_by_id((made[5].parse::<Uuid>().unwrap(), w.vault_id(), w.user.id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.number_of_views, 6);

    // A contact without a template isn't counted (R041 shows the template picker).
    let blank = s(&w.contact(json!({"first_name": "Blank"})).await);
    let mut c: contacts::ActiveModel = w.contact_model(&blank).await.into();
    c.template_id = Set(None);
    c.update(&db).await.unwrap();
    w.app.post(&view(&blank), json!({})).await.assert(204);
    assert!(
        contact_vault_user::Entity::find_by_id((blank.parse::<Uuid>().unwrap(), w.vault_id(), w.user.id))
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );

    // Viewers count their own views; deleted contacts drop out.
    let viewer = w.member(VaultPermission::View).await;
    w.app.login(&viewer.username).await;
    w.app.post(&view(&made[0]), json!({})).await.assert(204);
    assert_eq!(ids(&w.app.get(&w.path("/contacts/most-consulted")).await.assert(200).json()), v(&[&made[0]]));
    w.app.post(&view(&Uuid::new().to_string()), json!({})).await.assert(404);
    w.login_back().await;
    w.app.delete(&w.path(&format!("/contacts/{}", made[5]))).await.assert(204);
    assert_eq!(ids(&w.app.get(&w.path("/contacts/most-consulted")).await.json())[0], made[4]);
    w.app.finish().await;
}

#[tokio::test]
async fn moves_a_contact_to_another_vault() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let c = s(&w.contact(json!({"first_name": "Mover"})).await);
    let path = w.path(&format!("/contacts/{c}/move"));
    let target = s(&w.app.post("/vaults", json!({"name": "Work"})).await.assert(201).json());

    let res = w.app.post(&path, json!({})).await.assert(422).json();
    assert_eq!(
        (res["errors"][0]["field"].as_str(), res["errors"][0]["code"].as_str()),
        (Some("other_vault_id"), Some("required"))
    );
    let foreign = factory::create_vault(&db, factory::create_account(&db).await.id).await;
    let res = w.app.post(&path, json!({"other_vault_id": foreign.id})).await.assert(422).json();
    assert_eq!(res["errors"][0]["code"], "exists");
    w.app.post(&path, json!({"other_vault_id": Uuid::new()})).await.assert(422);
    // A vault of the account where the user may only view.
    let owner = factory::user(&db, Some(w.user.account_id)).await;
    let viewed = factory::create_vault_user(&db, &owner, VaultPermission::Manage).await;
    factory::set_permission_in_vault(&db, &w.user, VaultPermission::View, &viewed).await;
    w.app.post(&path, json!({"other_vault_id": viewed.id})).await.assert(403);

    let avatar = w.path(&format!("/contacts/{c}/avatar"));
    w.app.upload(Method::PUT, &avatar, "me.png", "image/png", &png(40, 40, 7)).await.assert(200);
    let moved = w.app.post(&path, json!({"other_vault_id": target})).await.assert(200).json();
    assert_eq!((moved["id"].as_str(), moved["vault_id"].as_str()), (Some(c.as_str()), Some(target.as_str())));
    assert!(moved["last_updated_at"].is_string());
    w.app.get(&w.path(&format!("/contacts/{c}"))).await.assert(404);
    w.app.get(&format!("/vaults/{target}/contacts/{c}")).await.assert(200);
    let res = w.app.get(&format!("/vaults/{target}/contacts/{c}/avatar")).await.assert(200);
    assert_eq!(res.headers["content-type"], "image/jpeg", "the avatar moves along");
    w.app.post(&path, json!({"other_vault_id": target})).await.assert(404);
    w.app.finish().await;
}

#[tokio::test]
async fn serves_uploads_and_removes_avatars() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let c = s(&w.contact(json!({"first_name": "John", "last_name": "Doe"})).await);
    let avatar = w.path(&format!("/contacts/{c}/avatar"));

    let res = w.app.get(&avatar).await.assert(200);
    assert_eq!(res.headers["content-type"], "image/svg+xml");
    assert_eq!(res.headers["cache-control"], "private, no-cache");
    assert_eq!(String::from_utf8(res.body.clone()).unwrap(), monica::multiavatar::svg("John Doe"), "X021");
    let etag = res.headers["etag"].to_str().unwrap().to_owned();
    let again = w.app.request_with(Method::GET, &avatar, None, &[("if-none-match", etag.as_str())]).await.assert(304);
    assert_eq!(again.headers["etag"].to_str().unwrap(), etag);
    let nameless = s(&w.contact(json!({"last_name": "Nobody"})).await);
    let res = w.app.get(&w.path(&format!("/contacts/{nameless}/avatar"))).await.assert(200);
    assert!(String::from_utf8(res.body).unwrap().starts_with("<svg"), "a random name's avatar");

    let res = w.app.upload(Method::PUT, &avatar, "notes.txt", "text/plain", b"not an image").await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "file");
    let file = w.app.upload(Method::PUT, &avatar, "me.png", "image/png", &png(640, 480, 10)).await.assert(200).json();
    assert_eq!(
        (file["type"].as_str(), file["mime_type"].as_str(), file["name"].as_str()),
        (Some("avatar"), Some("image/png"), Some("me.png"))
    );
    let contact = w.app.get(&w.path(&format!("/contacts/{c}"))).await.json();
    assert_eq!(contact["avatar_file_id"], file["id"]);
    assert_eq!(w.feed_actions(&c).await.last().unwrap(), "changed_avatar");

    let res = w.app.get(&avatar).await.assert(200);
    assert_eq!(res.headers["content-type"], "image/jpeg", "the 300×300 thumbnail");
    assert_eq!(res.headers["cache-control"], "private, no-cache");
    let thumb = image::load_from_memory(&res.body).unwrap();
    assert_eq!((thumb.width(), thumb.height()), (300, 300));
    let etag = res.headers["etag"].to_str().unwrap().to_owned();
    let res = w.app.request_with(Method::GET, &avatar, None, &[("if-none-match", etag.as_str())]).await.assert(304);
    assert_eq!(res.headers["cache-control"], "private, no-cache");

    let second =
        w.app.upload(Method::PUT, &avatar, "new.png", "image/png", &png(320, 320, 200)).await.assert(200).json();
    let avatars = files::Entity::find().filter(files::Column::Type.eq("avatar")).all(&db).await.unwrap();
    assert_eq!(
        avatars.iter().map(|f| f.id.0).collect::<Vec<_>>(),
        [second["id"].as_i64().unwrap()],
        "the previous one is gone"
    );
    let res = w.app.get(&avatar).await.assert(200);
    assert_ne!(res.headers["etag"].to_str().unwrap(), etag, "a new upload, a new ETag");

    let viewer = w.member(VaultPermission::View).await;
    w.app.login(&viewer.username).await;
    w.app.get(&avatar).await.assert(200);
    w.app.upload(Method::PUT, &avatar, "me.png", "image/png", &png(10, 10, 0)).await.assert(403);
    w.app.delete(&avatar).await.assert(403);
    w.login_back().await;

    w.app.delete(&avatar).await.assert(204);
    assert!(w.app.get(&w.path(&format!("/contacts/{c}"))).await.json()["avatar_file_id"].is_null());
    assert!(files::Entity::find().filter(files::Column::Type.eq("avatar")).all(&db).await.unwrap().is_empty());
    assert_eq!(w.app.get(&avatar).await.assert(200).headers["content-type"], "image/svg+xml");
    w.app.get(&w.path(&format!("/contacts/{}/avatar", Uuid::new()))).await.assert(404);
    w.app
        .upload(Method::PUT, &w.path(&format!("/contacts/{}/avatar", Uuid::new())), "a.png", "image/png", &png(4, 4, 1))
        .await
        .assert(404);
    w.app.finish().await;
}

#[tokio::test]
async fn manages_quick_facts() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let c = s(&w.contact(json!({"first_name": "Jane"})).await);
    let base = w.path(&format!("/contacts/{c}/quick-facts"));
    let templates = vault_quick_facts_templates::Entity::find()
        .filter(vault_quick_facts_templates::Column::VaultId.eq(w.vault_id()))
        .order_by_asc(vault_quick_facts_templates::Column::Position)
        .all(&db)
        .await
        .unwrap();
    let (hobbies, food) = (templates[0].id, templates[1].id);
    let other_vault = factory::create_vault(&db, w.user.account_id).await;
    let foreign = vault_quick_facts_templates::ActiveModel {
        vault_id: Set(other_vault.id),
        label: Set(Some("Other".into())),
        position: Set(1),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();

    let res = w.app.post(&base, json!({"content": "Tea"})).await.assert(422).json();
    assert_eq!(
        (res["errors"][0]["field"].as_str(), res["errors"][0]["code"].as_str()),
        (Some("vault_quick_facts_template_id"), Some("required"))
    );
    let res = w
        .app
        .post(&base, json!({"vault_quick_facts_template_id": foreign.id, "content": "Tea"}))
        .await
        .assert(422)
        .json();
    assert_eq!(res["errors"][0]["code"], "exists");
    let res =
        w.app.post(&base, json!({"vault_quick_facts_template_id": hobbies, "content": "  "})).await.assert(422).json();
    assert_eq!(
        (res["errors"][0]["field"].as_str(), res["errors"][0]["code"].as_str()),
        (Some("content"), Some("required"))
    );
    w.app.post(&base, json!({"vault_quick_facts_template_id": hobbies, "content": "x".repeat(256)})).await.assert(422);

    let tea =
        w.app.post(&base, json!({"vault_quick_facts_template_id": food, "content": " Tea "})).await.assert(201).json();
    assert_eq!((tea["content"].as_str(), tea["contact_id"].as_str()), (Some("Tea"), Some(c.as_str())));
    assert_eq!(tea["vault_quick_facts_template_id"], food.0);
    let chess = w
        .app
        .post(&base, json!({"vault_quick_facts_template_id": hobbies, "content": "Chess"}))
        .await
        .assert(201)
        .json();
    let list = w.app.get(&base).await.assert(200).json();
    assert_eq!(
        list["data"].as_array().unwrap().iter().map(|q| q["content"].as_str().unwrap()).collect::<Vec<_>>(),
        ["Tea", "Chess"],
        "oldest first"
    );
    let only = w.app.get(&format!("{base}?vault_quick_facts_template_id={hobbies}")).await.assert(200).json();
    assert_eq!(only["data"].as_array().unwrap().len(), 1);
    assert_eq!(only["data"][0]["id"], chess["id"]);
    w.app.get(&format!("{base}?vault_quick_facts_template_id={}", foreign.id)).await.assert(404);

    let fact = format!("{base}/{}", tea["id"]);
    let u = w.app.patch(&fact, json!({"content": "Coffee"})).await.assert(200).json();
    assert_eq!((u["content"].as_str(), u["id"].clone()), (Some("Coffee"), tea["id"].clone()));
    w.app.patch(&fact, json!({"content": ""})).await.assert(422);
    assert_eq!(w.app.patch(&fact, json!({})).await.assert(200).json()["content"], "Coffee");
    let other = s(&w.contact(json!({"first_name": "Other"})).await);
    let elsewhere = w.path(&format!("/contacts/{other}/quick-facts/{}", tea["id"]));
    w.app.patch(&elsewhere, json!({"content": "Mine"})).await.assert(404);
    w.app.delete(&elsewhere).await.assert(404);

    let viewer = w.member(VaultPermission::View).await;
    w.app.login(&viewer.username).await;
    assert_eq!(w.app.get(&base).await.assert(200).json()["data"].as_array().unwrap().len(), 2);
    w.app.post(&base, json!({"vault_quick_facts_template_id": hobbies, "content": "No"})).await.assert(403);
    w.app.patch(&fact, json!({"content": "No"})).await.assert(403);
    w.app.delete(&fact).await.assert(403);
    w.login_back().await;

    w.app.delete(&fact).await.assert(204);
    w.app.delete(&fact).await.assert(404);
    assert_eq!(w.app.get(&base).await.json()["data"].as_array().unwrap().len(), 1);
    w.app.finish().await;
}

#[tokio::test]
async fn lists_the_feed_with_its_objects() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let c = s(&w.contact(json!({"first_name": "Feed", "last_name": "Me"})).await);
    let contact_path = w.path(&format!("/contacts/{c}"));
    let feed = format!("{contact_path}/feed");
    w.app.patch(&contact_path, json!({"nickname": "F"})).await.assert(200);
    let note = w
        .app
        .post(&format!("{contact_path}/notes"), json!({"body": "A long enough note body"}))
        .await
        .assert(201)
        .json();
    let gone = w.app.post(&format!("{contact_path}/notes"), json!({"body": "Soon gone"})).await.assert(201).json();
    w.app.delete(&format!("{contact_path}/notes/{}", gone["id"])).await.assert(204);
    let label = labels::ActiveModel {
        vault_id: Set(w.vault_id()),
        name: Set("Friends".into()),
        slug: Set("friends".into()),
        bg_color: Set("bg-neutral-200".into()),
        text_color: Set("text-neutral-800".into()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    for (action, feedable) in [("label_assigned", Some(label.id)), ("label_removed", Some(Id(999_999)))] {
        contact_feed_items::ActiveModel {
            author_id: Set(Some(w.user.id)),
            contact_id: Set(c.parse().unwrap()),
            action: Set(action.into()),
            description: Set(Some("Friends".into())),
            feedable_type: Set(Some("App\\Models\\Label".into())),
            feedable_id: Set(feedable),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }

    let list = w.app.get(&feed).await.assert(200).json();
    let items = list["data"].as_array().unwrap();
    let actions: Vec<&str> = items.iter().map(|i| i["action"].as_str().unwrap()).collect();
    assert_eq!(
        actions,
        [
            "label_removed",
            "label_assigned",
            "note_destroyed",
            "note_created",
            "note_created",
            "information_updated",
            "contact_created"
        ],
        "newest first"
    );
    assert_eq!(list["meta"], json!({"page": 1, "per_page": 15, "total": 7, "last_page": 1}));
    let first = &items[0];
    assert!(first["label"].is_null() && first.as_object().unwrap().contains_key("label"), "deleted since: null");
    assert_eq!(first["description"], "Friends");
    assert_eq!(items[1]["label"]["name"], "Friends");
    assert_eq!(items[1]["label"]["id"], label.id.0);
    assert!(items[1].get("note").is_none(), "only the action's object");
    assert!(items[2]["note"].is_null(), "the destroyed note");
    assert_eq!(items[2]["description"], "Soon gone");
    assert!(items[3]["note"].is_null(), "created, then destroyed");
    assert_eq!(items[4]["note"]["id"], note["id"]);
    assert_eq!(items[4]["note"]["body"], "A long enough note body");
    let created = &items[6];
    assert_eq!(created["author"]["id"], w.user.id.to_string());
    assert_eq!(created["author"]["contact"]["id"], w.own_contact);
    assert_eq!(created["contact"]["id"], c);
    assert!(created["contact"]["age"].is_null());
    assert!(created["contact"]["groups"].as_array().unwrap().is_empty());
    for key in ["label", "address", "contact_information", "pet", "note", "goal", "mood_tracking_event"] {
        assert!(created.get(key).is_none(), "{key}");
    }
    assert!(created["created_at"].as_str().unwrap().ends_with('Z'));

    let page = w.app.get(&format!("{feed}?per_page=2&page=2")).await.assert(200).json();
    assert_eq!(page["data"][0]["action"], "note_destroyed");
    assert_eq!(page["meta"]["last_page"], 4);

    // The deleted author is null; the module is the first page.
    let mut item: contact_feed_items::ActiveModel = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::Action.eq("contact_created"))
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into();
    item.author_id = Set(None);
    item.update(&db).await.unwrap();
    let module = monica::api::contacts::feed_module(&w.app.state, &w.ctx(&c).await).await.unwrap();
    let module = serde_json::to_value(module).unwrap();
    let list = w.app.get(&feed).await.json();
    assert_eq!(module["feed_items"], list["data"]);
    assert_eq!(module["meta"], list["meta"]);
    assert!(list["data"][6]["author"].is_null());

    let viewer = w.member(VaultPermission::View).await;
    w.app.login(&viewer.username).await;
    w.app.get(&feed).await.assert(200);
    let stranger = factory::create_user(&db).await;
    w.app.login(&stranger.username).await;
    w.app.get(&feed).await.assert(404);
    w.login_back().await;
    w.app.get(&w.path(&format!("/contacts/{}/feed", Uuid::new()))).await.assert(404);
    w.app.finish().await;
}

#[tokio::test]
async fn builds_the_module_data_of_the_contact_page() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let gender = genders::Entity::find()
        .filter(genders::Column::AccountId.eq(w.user.account_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let pronoun = pronouns::Entity::find()
        .filter(pronouns::Column::AccountId.eq(w.user.account_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let c = s(&w.contact(json!({"first_name": "Page", "gender_id": gender.id, "pronoun_id": pronoun.id})).await);
    let state = w.app.state.clone();

    // contact_names and avatar
    w.app.patch(&w.path(&format!("/contacts/{c}")), json!({"is_favorite": true})).await.assert(200);
    let names = to_json(&monica::api::contacts::contact_names_module(&state, &w.ctx(&c).await).await.unwrap());
    assert_eq!((names["first_name"].as_str(), names["is_favorite"].as_bool()), (Some("Page"), Some(true)));
    assert_eq!(names["id"], c);
    let avatar = to_json(&monica::api::contacts::avatar_module(&state, &w.ctx(&c).await).await.unwrap());
    assert!(avatar["avatar_file_id"].is_null());
    let file = w
        .app
        .upload(Method::PUT, &w.path(&format!("/contacts/{c}/avatar")), "a.png", "image/png", &png(20, 20, 3))
        .await
        .assert(200)
        .json();
    let avatar = to_json(&monica::api::contacts::avatar_module(&state, &w.ctx(&c).await).await.unwrap());
    assert_eq!(avatar["avatar_file_id"], file["id"]);

    // gender_pronoun
    let gp = to_json(&monica::api::contacts::gender_pronoun_module(&state, &w.ctx(&c).await).await.unwrap());
    assert_eq!(gp["gender"]["id"], gender.id.0);
    assert_eq!(gp["gender"]["name_translation_key"], json!(gender.name_translation_key));
    assert_eq!(gp["pronoun"]["id"], pronoun.id.0);
    let bare = s(&w.contact(json!({"first_name": "Bare"})).await);
    let gp = to_json(&monica::api::contacts::gender_pronoun_module(&state, &w.ctx(&bare).await).await.unwrap());
    assert!(gp["gender"].is_null() && gp["pronoun"].is_null());

    // family_summary: a partner (either direction) and a child; a friend isn't family.
    let partner = s(&w.contact(json!({"first_name": "Partner"})).await);
    let child = s(&w.contact(json!({"first_name": "Child", "listed": false})).await);
    let friend = s(&w.contact(json!({"first_name": "Friend"})).await);
    let type_in = |group: &'static str, ty: Option<&'static str>, name: &'static str| {
        let db = db.clone();
        let account = w.user.account_id;
        async move {
            let g = relationship_group_types::Entity::find()
                .filter(relationship_group_types::Column::AccountId.eq(account))
                .filter(relationship_group_types::Column::Type.eq(group))
                .one(&db)
                .await
                .unwrap()
                .unwrap();
            let mut q = relationship_types::Entity::find()
                .filter(relationship_types::Column::RelationshipGroupTypeId.eq(g.id))
                .filter(relationship_types::Column::NameTranslationKey.eq(name));
            if let Some(t) = ty {
                q = q.filter(relationship_types::Column::Type.eq(t));
            }
            q.one(&db).await.unwrap().unwrap()
        }
    };
    let spouse = type_in("love", Some("family"), "spouse").await;
    let parent = type_in("family", Some("child"), "parent").await;
    let lover = type_in("love", None, "lover").await;
    let rel = |ty: Id, from: &str, to: &str| relationships::ActiveModel {
        relationship_type_id: Set(ty),
        contact_id: Set(from.parse().unwrap()),
        related_contact_id: Set(to.parse().unwrap()),
        ..Default::default()
    };
    let r1 = rel(spouse.id, &partner, &c).insert(&db).await.unwrap();
    let r2 = rel(parent.id, &c, &child).insert(&db).await.unwrap();
    rel(lover.id, &c, &friend).insert(&db).await.unwrap();
    let family = to_json(&monica::api::contacts::family_summary_module(&state, &w.ctx(&c).await).await.unwrap());
    assert_eq!(family["love_relationships"].as_array().unwrap().len(), 1);
    assert_eq!(family["love_relationships"][0]["relationship_id"], r1.id.0);
    assert_eq!(family["love_relationships"][0]["relationship_type_id"], spouse.id.0);
    assert_eq!(family["love_relationships"][0]["contact"]["id"], partner);
    assert_eq!(family["family_relationships"][0]["relationship_id"], r2.id.0);
    assert_eq!(family["family_relationships"][0]["contact"]["id"], child);
    assert_eq!(family["family_relationships"][0]["contact"]["listed"], false);
    assert!(family["family_relationships"][0]["contact"]["groups"].is_array());
    w.app.delete(&w.path(&format!("/contacts/{partner}"))).await.assert(204);
    let family = to_json(&monica::api::contacts::family_summary_module(&state, &w.ctx(&c).await).await.unwrap());
    assert!(family["love_relationships"].as_array().unwrap().is_empty(), "deleted contacts are left out");

    // posts: newest created first, with the journal, excerpt and first photo.
    let journal =
        journals::ActiveModel { vault_id: Set(w.vault_id()), name: Set("Diary".into()), ..Default::default() }
            .insert(&db)
            .await
            .unwrap();
    let mut made = Vec::new();
    for (title, year) in [(Some("Old"), 2020), (None, 2021)] {
        let p = posts::ActiveModel {
            journal_id: Set(journal.id),
            published: Set(true),
            title: Set(title.map(Into::into)),
            view_count: Set(0),
            written_at: Set(at(year)),
            created_at: Set(Some(at(year))),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        contact_post::ActiveModel { post_id: Set(p.id), contact_id: Set(c.parse().unwrap()), ..Default::default() }
            .insert(&db)
            .await
            .unwrap();
        made.push(p);
    }
    for (position, content) in [(2, Some("second")), (1, None), (3, Some("third"))] {
        post_sections::ActiveModel {
            post_id: Set(made[0].id),
            position: Set(position),
            label: Set("Section".into()),
            content: Set(content.map(|c| c.repeat(if c == "second" { 50 } else { 1 }))),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }
    let photo = files::ActiveModel {
        vault_id: Set(w.vault_id()),
        fileable_type: Set(Some("App\\Models\\Post".into())),
        fileable_id: Set(Some(made[0].id)),
        uuid: Set(Uuid::new().to_string()),
        mime_type: Set("image/png".into()),
        name: Set("p.png".into()),
        r#type: Set("photo".into()),
        size: Set(10),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let module = to_json(&monica::api::contacts::posts_module(&state, &w.ctx(&c).await).await.unwrap());
    let posts = module["posts"].as_array().unwrap();
    assert_eq!(posts.iter().map(|p| p["id"].as_i64().unwrap()).collect::<Vec<_>>(), [made[1].id.0, made[0].id.0]);
    assert!(posts[0]["title"].is_null() && posts[0]["excerpt"].is_null() && posts[0]["first_photo"].is_null());
    assert_eq!(posts[1]["journal"], json!({"id": journal.id.0, "name": "Diary"}));
    assert_eq!(posts[1]["written_at"], "2020-01-01");
    assert_eq!(posts[1]["excerpt"], format!("{}...", "second".repeat(50).chars().take(200).collect::<String>()));
    assert_eq!(posts[1]["first_photo"]["id"], photo.id.0);
    w.app.finish().await;
}

#[tokio::test]
async fn embeds_each_kind_of_feed_object() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let c = s(&w.contact(json!({"first_name": "Objects"})).await);
    let contact_id: Uuid = c.parse().unwrap();
    let account = w.user.account_id;
    let category = pet_categories::Entity::find()
        .filter(pet_categories::Column::AccountId.eq(account))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let pet = pets::ActiveModel {
        contact_id: Set(contact_id),
        pet_category_id: Set(category.id),
        name: Set(Some("Rex".into())),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let goal = goals::ActiveModel {
        contact_id: Set(contact_id),
        name: Set("Run".into()),
        active: Set(true),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let today = chrono::Utc::now().date_naive();
    for back in [0, 1, 5] {
        let day = today - chrono::Duration::days(back);
        streaks::ActiveModel {
            goal_id: Set(goal.id),
            happened_at: Set(Timestamp(day.and_hms_opt(0, 0, 0).unwrap())),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }
    let address_type = address_types::Entity::find()
        .filter(address_types::Column::AccountId.eq(account))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let address = addresses::ActiveModel {
        vault_id: Set(w.vault_id()),
        address_type_id: Set(Some(address_type.id)),
        line_1: Set(Some("1 Main St".into())),
        city: Set(Some("Paris".into())),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let info_type = contact_information_types::Entity::find()
        .filter(contact_information_types::Column::AccountId.eq(account))
        .filter(contact_information_types::Column::Type.eq("email"))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let info = contact_information::ActiveModel {
        contact_id: Set(contact_id),
        type_id: Set(info_type.id),
        data: Set("a@example.com".into()),
        kind: Set(Some("work".into())),
        pref: Set(false),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let parameter = mood_tracking_parameters::Entity::find()
        .filter(mood_tracking_parameters::Column::VaultId.eq(w.vault_id()))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mood = mood_tracking_events::ActiveModel {
        contact_id: Set(contact_id),
        mood_tracking_parameter_id: Set(parameter.id),
        rated_at: Set(at(2025)),
        note: Set(Some("Fine".into())),
        number_of_hours_slept: Set(Some(7)),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    for (action, class, id) in [
        ("pet_created", "Pet", pet.id),
        ("goal_created", "Goal", goal.id),
        ("address_created", "Address", address.id),
        ("contact_information_created", "ContactInformation", info.id),
        ("mood_tracking_event_added", "MoodTrackingEvent", mood.id),
        ("pet_destroyed", "Pet", Id(999_999)),
    ] {
        contact_feed_items::ActiveModel {
            author_id: Set(Some(w.user.id)),
            contact_id: Set(contact_id),
            action: Set(action.into()),
            feedable_type: Set(Some(format!("App\\Models\\{class}"))),
            feedable_id: Set(Some(id)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }

    let items = w.app.get(&w.path(&format!("/contacts/{c}/feed"))).await.assert(200).json()["data"].clone();
    let item = |action: &str| items.as_array().unwrap().iter().find(|i| i["action"] == action).unwrap().clone();
    let p = item("pet_created");
    assert_eq!(
        (p["pet"]["name"].as_str(), p["pet"]["pet_category"]["id"].as_i64()),
        (Some("Rex"), Some(category.id.0))
    );
    assert!(item("pet_destroyed")["pet"].is_null());
    let g = item("goal_created")["goal"].clone();
    assert_eq!(
        (g["name"].as_str(), g["max_streak"].as_i64(), g["current_streak"].as_i64()),
        (Some("Run"), Some(2), Some(2))
    );
    let days = g["last_7_days"].as_array().unwrap();
    assert_eq!(days.len(), 7);
    assert_eq!(days[6], json!({"date": today.to_string(), "has_streak": true}));
    assert_eq!(days[0]["has_streak"], false);
    assert_eq!(days[1]["has_streak"], true, "five days ago");
    let a = item("address_created")["address"].clone();
    assert_eq!((a["line_1"].as_str(), a["city"].as_str()), (Some("1 Main St"), Some("Paris")));
    assert_eq!(a["address_type"]["id"], address_type.id.0);
    let i = item("contact_information_created")["contact_information"].clone();
    assert_eq!((i["data"].as_str(), i["kind"].as_str()), (Some("a@example.com"), Some("work")));
    assert_eq!(i["type"]["id"], info_type.id.0);
    let m = item("mood_tracking_event_added")["mood_tracking_event"].clone();
    assert_eq!(
        (m["rated_at"].as_str(), m["note"].as_str(), m["number_of_hours_slept"].as_i64()),
        (Some("2025-01-01"), Some("Fine"), Some(7))
    );
    assert_eq!(m["mood_tracking_parameter"]["id"], parameter.id.0);
    w.app.finish().await;
}

#[tokio::test]
async fn refuses_large_avatars_and_unknown_contacts() {
    let mut w = world_with(|c| c.max_upload_size_mb = 1).await;
    let c = s(&w.contact(json!({"first_name": "Big"})).await);
    let avatar = w.path(&format!("/contacts/{c}/avatar"));
    let big = vec![0u8; 1024 * 1024 + 1];
    assert_eq!(
        w.app.upload(Method::PUT, &avatar, "big.png", "image/png", &big).await.assert(413).code(),
        "payload_too_large"
    );
    let missing = Uuid::new();
    w.app.delete(&w.path(&format!("/contacts/{missing}/avatar"))).await.assert(404);
    w.app.post(&w.path(&format!("/contacts/{missing}/view")), json!({})).await.assert(404);
    w.app.post(&w.path(&format!("/contacts/{missing}/move")), json!({"other_vault_id": w.vault})).await.assert(404);
    w.app.get(&w.path(&format!("/contacts/{missing}/quick-facts"))).await.assert(404);
    w.app.post(&w.path(&format!("/contacts/{missing}/quick-facts")), json!({"content": "x"})).await.assert(404);
    let stranger = factory::create_user(&w.app.state.db).await;
    w.app.login(&stranger.username).await;
    w.app.post(&w.path(&format!("/contacts/{c}/view")), json!({})).await.assert(404);
    w.app.get(&avatar).await.assert(404);
    w.app.finish().await;
}
