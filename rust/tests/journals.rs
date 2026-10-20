//! Journals, posts, slices of life and journal metrics over HTTP.

mod common;

use axum::http::Method;
use common::TestApp;
use monica::db::id::Id;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::entities::{
    contact_feed_items, contacts, files, mood_tracking_events, mood_tracking_parameters, post_template_sections,
    post_templates, users, vaults,
};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::{Value, json};

struct World {
    app: TestApp,
    user: users::Model,
    vault: vaults::Model,
    template: Id,
}

/// An editor of a vault, logged in, with a post template of two sections.
async fn world() -> World {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    let vault = factory::create_vault_user(&app.state.db, &user, VaultPermission::Edit).await;
    let template = post_template(&app, user.account_id).await;
    app.login(&user.username).await;
    World { app, user, vault, template }
}

/// A post template whose sections are "Story" (a translation key) then
/// "Highlights".
async fn post_template(app: &TestApp, account_id: Uuid) -> Id {
    let t = post_templates::ActiveModel {
        account_id: Set(account_id),
        label: Set(Some("Regular".into())),
        position: Set(1),
        can_be_deleted: Set(true),
        ..Default::default()
    }
    .insert(&app.state.db)
    .await
    .unwrap();
    for (label, key, position) in [(Some("Highlights"), None, 2), (None, Some("Story"), 1)] {
        post_template_sections::ActiveModel {
            post_template_id: Set(t.id),
            label: Set(label.map(Into::into)),
            label_translation_key: Set(key.map(Into::into)),
            position: Set(position),
            can_be_deleted: Set(true),
            ..Default::default()
        }
        .insert(&app.state.db)
        .await
        .unwrap();
    }
    t.id
}

fn png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(4, 4, image::Rgb([200, 10, 10]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

impl World {
    fn journals(&self) -> String {
        format!("/vaults/{}/journals", self.vault.id)
    }

    async fn journal(&mut self, name: &str) -> String {
        let path = self.journals();
        let j = self.app.post(&path, json!({"name": name})).await.assert(201).json();
        format!("{path}/{}", j["id"])
    }

    async fn post(&mut self, journal: &str, written_at: &str) -> (String, Value) {
        let body = json!({"post_template_id": self.template, "title": format!("Post of {written_at}"), "written_at": written_at});
        let p = self.app.post(&format!("{journal}/posts"), body).await.assert(201).json();
        (format!("{journal}/posts/{}", p["id"]), p)
    }

    async fn write(&mut self, post: &(String, Value), contents: [&str; 2]) {
        let sections: Vec<Value> = post.1["sections"]
            .as_array()
            .unwrap()
            .iter()
            .zip(contents)
            .map(|(s, c)| json!({"id": s["id"], "content": c}))
            .collect();
        self.app.patch(&post.0, json!({"sections": sections})).await.assert(200);
    }

    async fn contact(&self, first_name: &str) -> contacts::Model {
        let c = factory::contact(&self.app.state.db, &self.vault).await;
        let mut a: contacts::ActiveModel = c.into();
        a.first_name = Set(Some(first_name.into()));
        a.update(&self.app.state.db).await.unwrap()
    }

    async fn files(&self) -> u64 {
        files::Entity::find().filter(files::Column::VaultId.eq(self.vault.id)).count(&self.app.state.db).await.unwrap()
    }
}

fn titles(posts: &Value) -> Vec<String> {
    posts.as_array().unwrap().iter().map(|p| p["title"].as_str().unwrap_or("-").to_owned()).collect()
}

#[tokio::test]
async fn creates_lists_updates_and_deletes_journals() {
    let mut w = world().await;
    let base = w.journals();

    let res = w.app.post(&base, json!({"name": " "})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "name");
    let res = w.app.post(&base, json!({"name": "x".repeat(256)})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["code"], "max");
    let j = w.app.post(&base, json!({"name": "Zeta", "description": " Thoughts "})).await.assert(201).json();
    assert_eq!(j["name"], "Zeta");
    assert_eq!(j["description"], "Thoughts");
    assert_eq!(j["vault_id"], w.vault.id.to_string());
    assert!(j["last_post_written_at"].is_null());
    assert!(j["created_at"].is_string());
    let zeta = format!("{base}/{}", j["id"]);
    let alpha = w.journal("alpha").await;

    w.post(&alpha, "2024-03-05").await;
    w.post(&alpha, "2023-01-02").await;
    let list = w.app.get(&base).await.assert(200).json();
    let names: Vec<&str> = list["data"].as_array().unwrap().iter().map(|j| j["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["alpha", "Zeta"]);
    assert_eq!(list["data"][0]["last_post_written_at"], "2024-03-05");
    assert!(list["data"][1]["last_post_written_at"].is_null());

    let j = w.app.get(&zeta).await.assert(200).json();
    assert_eq!(j["description"], "Thoughts");
    let j = w.app.patch(&zeta, json!({"name": "Omega"})).await.assert(200).json();
    assert_eq!((j["name"].as_str(), j["description"].as_str()), (Some("Omega"), Some("Thoughts")));
    let j = w.app.patch(&zeta, json!({"description": null})).await.assert(200).json();
    assert_eq!(j["name"], "Omega");
    assert!(j["description"].is_null());
    w.app.patch(&zeta, json!({"name": ""})).await.assert(422);
    w.app.patch(&zeta, json!({"description": "d".repeat(65536)})).await.assert(422);

    w.app.delete(&zeta).await.assert(204);
    w.app.get(&zeta).await.assert(404);
    w.app.patch(&zeta, json!({"name": "Back"})).await.assert(404);
    w.app.delete(&zeta).await.assert(404);
    w.app.get(&format!("{base}/999999")).await.assert(404);
    w.app.finish().await;
}

#[tokio::test]
async fn writes_posts_from_templates() {
    let mut w = world().await;
    let journal = w.journal("Diary").await;
    let other_journal = w.journal("Other").await;
    let foreign_template = post_template(&w.app, factory::account(&w.app.state.db).await.id).await;
    let posts = format!("{journal}/posts");

    let res = w.app.post(&posts, json!({})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "post_template_id");
    assert_eq!(res.json()["errors"][0]["code"], "required");
    let res = w.app.post(&posts, json!({"post_template_id": foreign_template})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["code"], "exists");
    let res = w.app.post(&posts, json!({"post_template_id": w.template, "title": "t".repeat(256)})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "title");
    w.app
        .post(&format!("{}/journals/999999/posts", w.journals()), json!({"post_template_id": w.template}))
        .await
        .assert(404);

    // A draft dated today, a blank section per template section.
    let p = w.app.post(&posts, json!({"post_template_id": w.template})).await.assert(201).json();
    assert!(p["title"].is_null());
    assert_eq!(p["published"], false);
    assert_eq!(p["written_at"], chrono::Utc::now().date_naive().to_string());
    assert_eq!(p["journal"]["name"], "Diary");
    let labels: Vec<&str> = p["sections"].as_array().unwrap().iter().map(|s| s["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["Story", "Highlights"]);
    assert!(p["sections"][0]["content"].is_null());
    assert_eq!(p["sections"][0]["position"], 1);
    assert_eq!((p["view_count"].as_i64(), p["word_count"].as_i64()), (Some(0), Some(0)));
    assert_eq!(p["time_to_read_in_minute"], 1);
    for empty in ["tags", "contacts", "photos", "metrics", "mood_tracking_events"] {
        assert_eq!(p[empty], json!([]), "{empty}");
    }
    assert!(p["slice_of_life"].is_null() && p["previous_post"].is_null() && p["next_post"].is_null());

    let post = w.post(&journal, "2024-03-05").await;
    let path = post.0.clone();
    assert_eq!(post.1["title"], "Post of 2024-03-05");
    assert_eq!(post.1["written_at"], "2024-03-05");
    let s1 = post.1["sections"][0]["id"].clone();
    let s2 = post.1["sections"][1]["id"].clone();

    // Sections change by id; the statistics follow (`str_word_count`).
    let body = json!({
        "title": "  A day  ",
        "written_at": "2024-03-06",
        "sections": [{"id": s1, "content": "Hello brave new-world, it's me"}, {"id": s2, "content": null}],
    });
    let p = w.app.patch(&path, body).await.assert(200).json();
    assert_eq!(p["title"], "A day");
    assert_eq!(p["written_at"], "2024-03-06");
    assert_eq!(p["sections"][0]["content"], "Hello brave new-world, it's me");
    assert!(p["sections"][1]["content"].is_null());
    assert_eq!(p["word_count"], 5);
    assert_eq!(p["time_to_read_in_minute"], 1);
    let long = vec!["word"; 500].join(" ");
    let p = w.app.patch(&path, json!({"sections": [{"id": s2, "content": long}]})).await.assert(200).json();
    assert_eq!(p["word_count"], 505);
    assert_eq!(p["time_to_read_in_minute"], 3);
    assert_eq!(p["title"], "A day", "absent fields are kept");
    assert_eq!(p["written_at"], "2024-03-06");
    let p = w.app.patch(&path, json!({"title": null})).await.assert(200).json();
    assert!(p["title"].is_null());
    assert_eq!(p["word_count"], 505, "absent sections are kept");

    let other = w.post(&other_journal, "2024-01-01").await;
    let res = w
        .app
        .patch(&path, json!({"sections": [{"id": other.1["sections"][0]["id"], "content": "x"}]}))
        .await
        .assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "sections[0].id");
    assert_eq!(res.json()["errors"][0]["code"], "exists");
    let res = w.app.patch(&path, json!({"sections": [{"id": s1}]})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "sections[0].content");
    let res = w.app.patch(&path, json!({"sections": [{"content": "x"}]})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "sections[0].id");
    w.app.patch(&path, json!({"title": "t".repeat(256)})).await.assert(422);
    w.app.patch(&path, json!({"written_at": "06/03/2024"})).await.assert(422);
    // The post in the path must be in the journal of the path.
    w.app.get(&format!("{other_journal}/posts/{}", post.1["id"])).await.assert(404);
    w.app.patch(&format!("{other_journal}/posts/{}", post.1["id"]), json!({"title": "x"})).await.assert(404);

    // Neighbours are the closest posts written on other days.
    w.post(&journal, "2024-03-01").await;
    w.post(&journal, "2024-02-01").await;
    w.post(&journal, "2024-03-06").await;
    w.post(&journal, "2024-03-09").await;
    w.post(&journal, "2024-04-01").await;
    let p = w.app.get(&path).await.assert(200).json();
    assert_eq!(p["previous_post"]["written_at"], "2024-03-01");
    assert_eq!(p["previous_post"]["title"], "Post of 2024-03-01");
    assert_eq!(p["next_post"]["written_at"], "2024-03-09");

    // F15: reading counts for any member; GET has no side effect.
    let viewer = factory::user(&w.app.state.db, Some(w.user.account_id)).await;
    factory::set_permission_in_vault(&w.app.state.db, &viewer, VaultPermission::View, &w.vault).await;
    w.app.post(&format!("{path}/view"), json!({})).await.assert(204);
    w.app.login(&viewer.username).await;
    w.app.post(&format!("{path}/view"), json!({})).await.assert(204);
    assert_eq!(w.app.get(&path).await.assert(200).json()["view_count"], 2);
    assert_eq!(w.app.get(&path).await.json()["view_count"], 2);
    w.app.post(&format!("{other_journal}/posts/{}/view", post.1["id"]), json!({})).await.assert(404);
    w.app.login(&w.user.username).await;

    w.app.delete(&path).await.assert(204);
    w.app.get(&path).await.assert(404);
    w.app.delete(&path).await.assert(404);
    w.app.finish().await;
}

#[tokio::test]
async fn shows_a_journal_year() {
    let mut w = world().await;
    let journal = w.journal("Diary").await;
    let elsewhere = w.journal("Elsewhere").await;
    let a = w.post(&journal, "2024-03-05").await;
    w.post(&journal, "2024-03-20").await;
    let jan = w.post(&journal, "2024-01-10").await;
    w.post(&journal, "2023-07-01").await;
    let other = w.post(&elsewhere, "2024-03-01").await;
    w.write(&a, ["", &"é".repeat(250)]).await;
    w.write(&jan, ["Short", "Ignored"]).await;

    let tag = w.app.post(&format!("{}/tags", a.0), json!({"name": "Travel"})).await.assert(201).json();
    w.app.put(&format!("{}/tags/{}", other.0, tag["id"]), json!({})).await.assert(200);
    w.app.post(&format!("{}/tags", other.0), json!({"name": "Unused here"})).await.assert(201);
    w.app.post(&format!("{}/tags", jan.0), json!({"name": "Family"})).await.assert(201);
    let slice = w.app.post(&format!("{journal}/slices"), json!({"name": "Spring"})).await.assert(201).json();
    w.app.put(&format!("{}/slice", a.0), json!({"slice_of_life_id": slice["id"]})).await.assert(200);

    let o = w.app.get(&format!("{journal}/overview?year=2024")).await.assert(200).json();
    assert_eq!(o["journal"]["name"], "Diary");
    assert_eq!(o["journal"]["last_post_written_at"], "2024-03-20");
    assert_eq!(o["year"], 2024);
    let months = o["months"].as_array().unwrap();
    assert_eq!(months.len(), 12);
    let order: Vec<i64> = months.iter().map(|m| m["month"].as_i64().unwrap()).collect();
    assert_eq!(order, (1..=12).rev().collect::<Vec<i64>>());
    let march = &months[9];
    assert_eq!(march["post_count"], 2);
    assert_eq!(titles(&march["posts"]), ["Post of 2024-03-20", "Post of 2024-03-05"]);
    let p = &march["posts"][1];
    // The first section with content (an empty string is cleared), cut at 200.
    assert_eq!(p["excerpt"], format!("{}...", "é".repeat(200)));
    assert_eq!(p["journal"]["name"], "Diary");
    assert_eq!(p["slice_of_life_id"], slice["id"]);
    assert_eq!(p["published"], false);
    assert!(p["first_photo"].is_null());
    assert!(march["posts"][0]["excerpt"].is_null());
    assert_eq!(months[11]["posts"][0]["excerpt"], "Short");
    assert_eq!(months[0]["post_count"], 0);
    assert_eq!(o["years"], json!([{"year": 2024, "post_count": 3}, {"year": 2023, "post_count": 1}]));
    let tags = o["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!((tags[0]["name"].as_str(), tags[0]["post_count"].as_i64()), (Some("Family"), Some(1)));
    assert_eq!((tags[1]["name"].as_str(), tags[1]["post_count"].as_i64()), (Some("Travel"), Some(2)), "vault-wide");
    assert_eq!(tags[1]["slug"], "travel");
    assert!(tags[1].get("posts_count").is_none());
    assert_eq!(o["slices"][0]["name"], "Spring");
    assert_eq!(o["slices"][0]["oldest_post_written_at"], "2024-03-05");

    let o = w.app.get(&format!("{journal}/overview?year=2023")).await.assert(200).json();
    assert_eq!(o["months"][5]["post_count"], 1);
    let o = w.app.get(&format!("{journal}/overview")).await.assert(200).json();
    assert_eq!(o["year"], i64::from(chrono::Datelike::year(&chrono::Utc::now())));
    assert!(o["months"].as_array().unwrap().iter().all(|m| m["post_count"] == 0));
    w.app.get(&format!("{journal}/overview?year=0")).await.assert(422);
    w.app.get(&format!("{journal}/overview?year=10000")).await.assert(422);
    w.app.get(&format!("{}/999999/overview", w.journals())).await.assert(404);
    w.app.finish().await;
}

#[tokio::test]
async fn mentions_contacts_and_tags_posts() {
    let mut w = world().await;
    let journal = w.journal("Diary").await;
    let post = w.post(&journal, "2024-03-05").await;
    let path = post.0.clone();
    let regis = w.contact("Regis").await;
    let anna = w.contact("Anna").await;
    let stranger_vault = factory::create_vault(&w.app.state.db, w.user.account_id).await;
    let outsider = factory::contact(&w.app.state.db, &stranger_vault).await;
    let db = w.app.state.db.clone();
    let feed = |c: Uuid, action: &'static str| {
        let db = db.clone();
        async move {
            contact_feed_items::Entity::find()
                .filter(contact_feed_items::Column::ContactId.eq(c))
                .filter(contact_feed_items::Column::Action.eq(action))
                .count(&db)
                .await
                .unwrap()
        }
    };

    w.app.put(&format!("{path}/contacts/{}", regis.id), json!({})).await.assert(204);
    w.app.put(&format!("{path}/contacts/{}", regis.id), json!({})).await.assert(204);
    assert_eq!(feed(regis.id, "added_to_post").await, 1, "F16: idempotent, one feed item");
    w.app.put(&format!("{path}/contacts/{}", anna.id), json!({})).await.assert(204);
    w.app.put(&format!("{path}/contacts/{}", outsider.id), json!({})).await.assert(404);
    w.app.put(&format!("{path}/contacts/{}", Uuid::new()), json!({})).await.assert(404);
    let p = w.app.get(&path).await.assert(200).json();
    let mut mentioned: Vec<&str> =
        p["contacts"].as_array().unwrap().iter().map(|c| c["first_name"].as_str().unwrap()).collect();
    mentioned.sort();
    assert_eq!(mentioned, ["Anna", "Regis"]);
    assert_eq!(p["contacts"][0]["groups"], json!([]));
    assert!(p["contacts"][0].get("age").is_some());
    // F16: saving the post doesn't touch the mentions.
    w.app.patch(&path, json!({"title": "Saved"})).await.assert(200);
    assert_eq!(feed(regis.id, "added_to_post").await, 1);
    assert_eq!(w.app.get(&path).await.json()["contacts"].as_array().unwrap().len(), 2);

    w.app.delete(&format!("{path}/contacts/{}", regis.id)).await.assert(204);
    assert_eq!(feed(regis.id, "removed_from_post").await, 1);
    w.app.delete(&format!("{path}/contacts/{}", regis.id)).await.assert(404);
    assert_eq!(w.app.get(&path).await.json()["contacts"][0]["first_name"], "Anna");

    // Tags: created here as an editor, or an existing vault tag.
    let res = w.app.post(&format!("{path}/tags"), json!({"name": ""})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "name");
    w.app.post(&format!("{path}/tags"), json!({"name": "n".repeat(256)})).await.assert(422);
    let travel = w.app.post(&format!("{path}/tags"), json!({"name": "Travel plans"})).await.assert(201).json();
    assert_eq!(travel["name"], "Travel plans");
    assert_eq!(travel["slug"], "travel-plans");
    assert_eq!(travel["vault_id"], w.vault.id.to_string());
    let other = w.post(&journal, "2024-03-06").await;
    let books = w.app.post(&format!("{}/tags", other.0), json!({"name": "Books"})).await.assert(201).json();
    let t = w.app.put(&format!("{path}/tags/{}", books["id"]), json!({})).await.assert(200).json();
    assert_eq!(t["id"], books["id"]);
    w.app.put(&format!("{path}/tags/{}", books["id"]), json!({})).await.assert(200);
    let p = w.app.get(&path).await.json();
    let names: Vec<&str> = p["tags"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Books", "Travel plans"]);
    w.app.put(&format!("{path}/tags/999999"), json!({})).await.assert(404);
    w.app.delete(&format!("{path}/tags/{}", travel["id"])).await.assert(204);
    let p = w.app.get(&path).await.json();
    assert_eq!(p["tags"].as_array().unwrap().len(), 1);
    w.app.delete(&format!("{path}/tags/999999")).await.assert(404);
    w.app.finish().await;
}

#[tokio::test]
async fn slices_of_life_and_metrics() {
    let mut w = world().await;
    let journal = w.journal("Diary").await;
    let other_journal = w.journal("Other").await;
    let a = w.post(&journal, "2024-03-05").await;
    let b = w.post(&journal, "2024-05-01").await;
    w.post(&journal, "2024-06-01").await;
    let slices = format!("{journal}/slices");

    w.app.post(&slices, json!({"name": ""})).await.assert(422);
    w.app.post(&slices, json!({"name": "s".repeat(256)})).await.assert(422);
    let s = w.app.post(&slices, json!({"name": "summer"})).await.assert(201).json();
    assert_eq!(s["name"], "summer");
    assert!(s["oldest_post_written_at"].is_null() && s["file_cover_image_id"].is_null() && s["description"].is_null());
    let spring = w.app.post(&slices, json!({"name": "Spring"})).await.assert(201).json();
    let foreign = w.app.post(&format!("{other_journal}/slices"), json!({"name": "Theirs"})).await.assert(201).json();
    let list = w.app.get(&slices).await.assert(200).json();
    let names: Vec<&str> = list["data"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Spring", "summer"]);

    let res = w.app.put(&format!("{}/slice", a.0), json!({"slice_of_life_id": foreign["id"]})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["code"], "exists");
    w.app.put(&format!("{}/slice", a.0), json!({})).await.assert(422);
    let r = w.app.put(&format!("{}/slice", a.0), json!({"slice_of_life_id": spring["id"]})).await.assert(200).json();
    assert_eq!(r["id"], spring["id"]);
    assert_eq!(
        (r["oldest_post_written_at"].as_str(), r["newest_post_written_at"].as_str()),
        (Some("2024-03-05"), Some("2024-03-05"))
    );
    w.app.put(&format!("{}/slice", b.0), json!({"slice_of_life_id": spring["id"]})).await.assert(200);
    let p = w.app.get(&a.0).await.json();
    assert_eq!(p["slice_of_life"]["name"], "Spring");
    assert_eq!(p["slice_of_life"]["newest_post_written_at"], "2024-05-01");

    let regis = w.contact("Regis").await;
    let anna = w.contact("Anna").await;
    for (post, c) in [(&a, &regis), (&b, &anna), (&b, &regis)] {
        w.app.put(&format!("{}/contacts/{}", post.0, c.id), json!({})).await.assert(204);
    }
    let spring_path = format!("{slices}/{}", spring["id"]);
    let d = w.app.get(&spring_path).await.assert(200).json();
    assert_eq!(d["name"], "Spring");
    assert_eq!(d["journal"]["name"], "Diary");
    assert_eq!(titles(&d["posts"]), ["Post of 2024-05-01", "Post of 2024-03-05"]);
    let contacts: Vec<String> =
        d["contacts"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_owned()).collect();
    assert_eq!(contacts.len(), 2, "each contact once");
    assert!(contacts.contains(&regis.id.to_string()) && contacts.contains(&anna.id.to_string()));
    assert_eq!(d["oldest_post_written_at"], "2024-03-05");
    // F1: a slice of another journal is a 404.
    w.app.get(&format!("{slices}/{}", foreign["id"])).await.assert(404);
    w.app.patch(&format!("{slices}/{}", foreign["id"]), json!({"name": "x"})).await.assert(404);

    let u = w.app.patch(&spring_path, json!({"description": "Flowers"})).await.assert(200).json();
    assert_eq!((u["name"].as_str(), u["description"].as_str()), (Some("Spring"), Some("Flowers")));
    let u = w.app.patch(&spring_path, json!({"name": "Early spring", "description": null})).await.assert(200).json();
    assert_eq!(u["name"], "Early spring");
    assert!(u["description"].is_null());
    w.app.patch(&spring_path, json!({"name": " "})).await.assert(422);
    w.app.patch(&spring_path, json!({"description": "d".repeat(256)})).await.assert(422);

    w.app.delete(&format!("{}/slice", a.0)).await.assert(204);
    w.app.delete(&format!("{}/slice", a.0)).await.assert(204);
    assert!(w.app.get(&a.0).await.json()["slice_of_life"].is_null());
    w.app.delete(&spring_path).await.assert(204);
    w.app.get(&spring_path).await.assert(404);
    assert!(w.app.get(&b.0).await.json()["slice_of_life"].is_null(), "posts stay, out of the slice");

    // Journal metrics, and their values on posts.
    let metrics = format!("{journal}/metrics");
    w.app.post(&metrics, json!({})).await.assert(422);
    let sleep = w.app.post(&metrics, json!({"label": "sleep"})).await.assert(201).json();
    assert_eq!(sleep["label"], "sleep");
    let km = w.app.post(&metrics, json!({"label": "Km walked"})).await.assert(201).json();
    let theirs = w.app.post(&format!("{other_journal}/metrics"), json!({"label": "Theirs"})).await.assert(201).json();
    let list = w.app.get(&metrics).await.assert(200).json();
    let labels: Vec<&str> = list["data"].as_array().unwrap().iter().map(|m| m["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["Km walked", "sleep"]);

    let values = format!("{}/metrics", a.0);
    let res = w.app.post(&values, json!({"journal_metric_id": theirs["id"], "value": 3})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["code"], "exists");
    let res = w.app.post(&values, json!({"journal_metric_id": km["id"]})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "value");
    w.app.post(&values, json!({"journal_metric_id": km["id"], "value": 3_000_000_000_i64})).await.assert(422);
    w.app.post(&values, json!({"journal_metric_id": km["id"], "value": 1, "label": "l".repeat(256)})).await.assert(422);
    let v1 = w
        .app
        .post(&values, json!({"journal_metric_id": km["id"], "value": 3, "label": " morning "}))
        .await
        .assert(201)
        .json();
    assert_eq!((v1["value"].as_i64(), v1["label"].as_str()), (Some(3), Some("morning")));
    assert_eq!(v1["journal_metric_id"], km["id"]);
    w.app.post(&values, json!({"journal_metric_id": km["id"], "value": 4})).await.assert(201);
    let p = w.app.get(&a.0).await.json();
    let m = p["metrics"].as_array().unwrap();
    assert_eq!(m.len(), 2, "every metric of the journal");
    assert_eq!(m[0]["journal_metric"]["label"], "sleep");
    assert_eq!(m[0]["total"], 0);
    assert_eq!(m[1]["total"], 7);
    assert_eq!(m[1]["post_metrics"].as_array().unwrap().len(), 2);
    w.app.delete(&format!("{}/metrics/{}", b.0, v1["id"])).await.assert(404);
    w.app.delete(&format!("{values}/{}", v1["id"])).await.assert(204);
    w.app.delete(&format!("{values}/{}", v1["id"])).await.assert(404);
    assert_eq!(w.app.get(&a.0).await.json()["metrics"][1]["total"], 4);
    w.app.delete(&format!("{metrics}/{}", theirs["id"])).await.assert(404);
    w.app.delete(&format!("{metrics}/{}", km["id"])).await.assert(204);
    assert_eq!(w.app.get(&a.0).await.json()["metrics"].as_array().unwrap().len(), 1);
    w.app.finish().await;
}

#[tokio::test]
async fn uploads_post_photos_and_slice_covers() {
    let mut w = world().await;
    let journal = w.journal("Diary").await;
    let a = w.post(&journal, "2024-03-05").await;
    let b = w.post(&journal, "2024-04-05").await;
    w.post(&journal, "2024-05-05").await;

    let res =
        w.app.upload(Method::POST, &format!("{}/photos", a.0), "notes.txt", "text/plain", b"hello").await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "file");
    let photo = w
        .app
        .upload(Method::POST, &format!("{}/photos", a.0), "beach.png", "image/png", &png())
        .await
        .assert(201)
        .json();
    assert_eq!(photo["name"], "beach.png");
    assert_eq!(photo["mime_type"], "image/png");
    assert_eq!(photo["type"], "photo");
    assert_eq!(photo["size"], png().len());
    let uuid = photo["uuid"].as_str().unwrap().to_owned();
    assert!(w.app.state.storage.get(w.vault.id, &uuid).await.is_ok());
    let second =
        w.app.upload(Method::POST, &format!("{}/photos", a.0), "b.png", "image/png", &png()).await.assert(201).json();
    let on_b =
        w.app.upload(Method::POST, &format!("{}/photos", b.0), "c.png", "image/png", &png()).await.assert(201).json();
    w.app
        .upload(Method::POST, &format!("{journal}/posts/999999/photos"), "c.png", "image/png", &png())
        .await
        .assert(404);

    let p = w.app.get(&a.0).await.json();
    let ids: Vec<&Value> = p["photos"].as_array().unwrap().iter().map(|f| &f["id"]).collect();
    assert_eq!(ids, [&photo["id"], &second["id"]]);
    let list = w.app.get(&format!("{journal}/photos")).await.assert(200).json();
    let got: Vec<(Value, Value)> =
        list["data"].as_array().unwrap().iter().map(|f| (f["id"].clone(), f["post_id"].clone())).collect();
    assert_eq!(
        got,
        [
            (on_b["id"].clone(), b.1["id"].clone()),
            (photo["id"].clone(), a.1["id"].clone()),
            (second["id"].clone(), a.1["id"].clone())
        ],
        "by post date descending, then file id"
    );
    assert_eq!(list["data"][1]["uuid"], uuid);
    let o = w.app.get(&format!("{journal}/overview?year=2024")).await.json();
    assert_eq!(o["months"][9]["posts"][0]["first_photo"]["id"], photo["id"]);

    // F17: only a photo of the post in the path.
    w.app.delete(&format!("{}/photos/{}", b.0, photo["id"])).await.assert(404);
    w.app.delete(&format!("{}/photos/{}", a.0, photo["id"])).await.assert(204);
    assert!(w.app.state.storage.get(w.vault.id, &uuid).await.is_err(), "the bytes are gone");
    w.app.delete(&format!("{}/photos/{}", a.0, photo["id"])).await.assert(404);
    assert_eq!(w.app.get(&a.0).await.json()["photos"].as_array().unwrap().len(), 1);

    // Slice covers; Q7: a replaced or removed cover's file is deleted.
    let s = w.app.post(&format!("{journal}/slices"), json!({"name": "Spring"})).await.assert(201).json();
    let slice = format!("{journal}/slices/{}", s["id"]);
    w.app.upload(Method::PUT, &format!("{slice}/cover"), "x.txt", "text/plain", b"nope").await.assert(422);
    let cover =
        w.app.upload(Method::PUT, &format!("{slice}/cover"), "cover.png", "image/png", &png()).await.assert(200).json();
    assert_eq!(cover["type"], "photo");
    assert_eq!(w.app.get(&slice).await.json()["file_cover_image_id"], cover["id"]);
    let before = w.files().await;
    let newer =
        w.app.upload(Method::PUT, &format!("{slice}/cover"), "new.png", "image/png", &png()).await.assert(200).json();
    assert_eq!(w.files().await, before, "the old cover went");
    assert!(w.app.state.storage.get(w.vault.id, cover["uuid"].as_str().unwrap()).await.is_err());
    assert_eq!(w.app.get(&slice).await.json()["file_cover_image_id"], newer["id"]);
    assert_eq!(
        w.app.get(&format!("{journal}/overview?year=2024")).await.json()["slices"][0]["file_cover_image_id"],
        newer["id"]
    );
    w.app.delete(&format!("{slice}/cover")).await.assert(204);
    assert!(w.app.get(&slice).await.json()["file_cover_image_id"].is_null());
    assert_eq!(w.files().await, before - 1);
    w.app.delete(&format!("{slice}/cover")).await.assert(204);
    w.app.delete(&format!("{journal}/slices/999999/cover")).await.assert(404);

    let last =
        w.app.upload(Method::PUT, &format!("{slice}/cover"), "last.png", "image/png", &png()).await.assert(200).json();
    let count = w.files().await;
    w.app.delete(&slice).await.assert(204);
    assert_eq!(w.files().await, count - 1, "Q7: the slice's cover went with it");
    assert!(w.app.state.storage.get(w.vault.id, last["uuid"].as_str().unwrap()).await.is_err());

    // Deleting a post or a journal deletes its photos.
    w.app.delete(&b.0).await.assert(204);
    assert!(w.app.state.storage.get(w.vault.id, on_b["uuid"].as_str().unwrap()).await.is_err());
    assert_eq!(w.files().await, 1);
    w.app.upload(Method::PUT, &format!("{slice}/cover"), "x.png", "image/png", &png()).await.assert(404);
    let s = w.app.post(&format!("{journal}/slices"), json!({"name": "Autumn"})).await.assert(201).json();
    w.app
        .upload(Method::PUT, &format!("{journal}/slices/{}/cover", s["id"]), "x.png", "image/png", &png())
        .await
        .assert(200);
    assert_eq!(w.files().await, 2);
    w.app.delete(&journal).await.assert(204);
    assert_eq!(w.files().await, 0, "Q7: the journal's photos and covers went with it");
    w.app.finish().await;
}

#[tokio::test]
async fn shows_the_callers_mood_on_the_post_day() {
    let mut w = world().await;
    let journal = w.journal("Diary").await;
    let post = w.post(&journal, "2024-03-05").await;
    let vault = w.app.get(&format!("/vaults/{}", w.vault.id)).await.json();
    let own: Uuid = vault["user_contact_id"].as_str().unwrap().parse().unwrap();
    let other = w.contact("Other").await;
    let parameter = mood_tracking_parameters::ActiveModel {
        vault_id: Set(w.vault.id),
        label: Set(Some("Great".into())),
        hex_color: Set("bg-lime-500".into()),
        position: Set(Some(1)),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap();
    for (contact, at) in [(own, "2024-03-05T22:30:00"), (own, "2024-03-06T00:00:00"), (other.id, "2024-03-05T10:00:00")]
    {
        mood_tracking_events::ActiveModel {
            contact_id: Set(contact),
            mood_tracking_parameter_id: Set(parameter.id),
            rated_at: Set(monica::db::timestamp::Timestamp(at.parse().unwrap())),
            note: Set(Some("ok".into())),
            number_of_hours_slept: Set(Some(7)),
            ..Default::default()
        }
        .insert(&w.app.state.db)
        .await
        .unwrap();
    }
    let p = w.app.get(&post.0).await.assert(200).json();
    let events = p["mood_tracking_events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["rated_at"], "2024-03-05");
    assert_eq!(events[0]["note"], "ok");
    assert_eq!(events[0]["number_of_hours_slept"], 7);
    assert_eq!(events[0]["mood_tracking_parameter"]["label"], "Great");
    assert_eq!(events[0]["mood_tracking_parameter"]["hex_color"], "bg-lime-500");

    // No own contact any more: null.
    let c = contacts::Entity::find_by_id(own).one(&w.app.state.db).await.unwrap().unwrap();
    let mut c: contacts::ActiveModel = c.into();
    c.deleted_at = Set(Some(monica::db::now()));
    c.update(&w.app.state.db).await.unwrap();
    assert!(w.app.get(&post.0).await.json()["mood_tracking_events"].is_null());
    w.app.finish().await;
}

#[tokio::test]
async fn viewers_only_read_and_strangers_see_nothing() {
    let mut w = world().await;
    let journal = w.journal("Diary").await;
    let post = w.post(&journal, "2024-03-05").await;
    let p = post.0.clone();
    let slice = w.app.post(&format!("{journal}/slices"), json!({"name": "Spring"})).await.assert(201).json();
    let s = format!("{journal}/slices/{}", slice["id"]);
    let metric = w.app.post(&format!("{journal}/metrics"), json!({"label": "Km"})).await.assert(201).json();
    let tag = w.app.post(&format!("{p}/tags"), json!({"name": "Tag"})).await.assert(201).json();
    let value = w
        .app
        .post(&format!("{p}/metrics"), json!({"journal_metric_id": metric["id"], "value": 1}))
        .await
        .assert(201)
        .json();
    let photo =
        w.app.upload(Method::POST, &format!("{p}/photos"), "a.png", "image/png", &png()).await.assert(201).json();
    let contact = w.contact("Regis").await;
    let section = post.1["sections"][0]["id"].clone();
    let template = w.template;

    let reads = [
        w.journals(),
        journal.clone(),
        format!("{journal}/overview"),
        format!("{journal}/photos"),
        p.clone(),
        format!("{journal}/slices"),
        s.clone(),
        format!("{journal}/metrics"),
    ];
    let writes: Vec<(Method, String, Value)> = vec![
        (Method::POST, w.journals(), json!({"name": "J"})),
        (Method::PATCH, journal.clone(), json!({"name": "J"})),
        (Method::DELETE, journal.clone(), Value::Null),
        (Method::POST, format!("{journal}/posts"), json!({"post_template_id": template})),
        (Method::PATCH, p.clone(), json!({"sections": [{"id": section, "content": "x"}]})),
        (Method::DELETE, p.clone(), Value::Null),
        (Method::PUT, format!("{p}/contacts/{}", contact.id), Value::Null),
        (Method::DELETE, format!("{p}/contacts/{}", contact.id), Value::Null),
        (Method::DELETE, format!("{p}/photos/{}", photo["id"]), Value::Null),
        (Method::POST, format!("{p}/tags"), json!({"name": "T"})),
        (Method::PUT, format!("{p}/tags/{}", tag["id"]), Value::Null),
        (Method::DELETE, format!("{p}/tags/{}", tag["id"]), Value::Null),
        (Method::PUT, format!("{p}/slice"), json!({"slice_of_life_id": slice["id"]})),
        (Method::DELETE, format!("{p}/slice"), Value::Null),
        (Method::POST, format!("{p}/metrics"), json!({"journal_metric_id": metric["id"], "value": 2})),
        (Method::DELETE, format!("{p}/metrics/{}", value["id"]), Value::Null),
        (Method::POST, format!("{journal}/slices"), json!({"name": "S"})),
        (Method::PATCH, s.clone(), json!({"name": "S"})),
        (Method::DELETE, s.clone(), Value::Null),
        (Method::DELETE, format!("{s}/cover"), Value::Null),
        (Method::POST, format!("{journal}/metrics"), json!({"label": "L"})),
        (Method::DELETE, format!("{journal}/metrics/{}", metric["id"]), Value::Null),
    ];

    let viewer = factory::user(&w.app.state.db, Some(w.user.account_id)).await;
    factory::set_permission_in_vault(&w.app.state.db, &viewer, VaultPermission::View, &w.vault).await;
    w.app.login(&viewer.username).await;
    for path in &reads {
        w.app.get(path).await.assert(200);
    }
    for (method, path, body) in &writes {
        let body = if body.is_null() { None } else { Some(body.clone()) };
        let res = w.app.request(method.clone(), path, body).await;
        assert_eq!(res.status.as_u16(), 403, "{method} {path}: {}", String::from_utf8_lossy(&res.body));
    }
    w.app.upload(Method::POST, &format!("{p}/photos"), "a.png", "image/png", &png()).await.assert(403);
    w.app.upload(Method::PUT, &format!("{s}/cover"), "a.png", "image/png", &png()).await.assert(403);

    let stranger = factory::user(&w.app.state.db, None).await;
    w.app.login(&stranger.username).await;
    for path in &reads {
        w.app.get(path).await.assert(404);
    }
    w.app.post(&format!("{p}/view"), json!({})).await.assert(404);
    for (method, path, body) in &writes {
        let body = if body.is_null() { None } else { Some(body.clone()) };
        let res = w.app.request(method.clone(), path, body).await;
        assert_eq!(res.status.as_u16(), 404, "{method} {path}");
    }
    w.app.upload(Method::POST, &format!("{p}/photos"), "a.png", "image/png", &png()).await.assert(404);
    w.app.upload(Method::PUT, &format!("{s}/cover"), "a.png", "image/png", &png()).await.assert(404);
    w.app.finish().await;
}
