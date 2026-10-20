//! Vault settings over HTTP: labels, tags, important date types, mood
//! tracking parameters, life event categories and types, quick fact
//! templates.

mod common;

use common::TestApp;
use monica::db::id::Id;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::entities::{
    contact_label, contacts, journals, life_event_categories, life_event_types, mood_tracking_events, post_tag, posts,
    users, vaults,
};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::{Value, json};

/// A manager logged in, with a vault created through the API (so it has
/// the seeded settings). Returns the vault and its path.
async fn setup() -> (TestApp, users::Model, vaults::Model, String) {
    let mut app = TestApp::new().await;
    let manager = factory::user(&app.state.db, None).await;
    app.login(&manager.username).await;
    let vault = app.post("/vaults", json!({"name": "Family"})).await.assert(201).json();
    let id: Uuid = vault["id"].as_str().unwrap().parse().unwrap();
    let vault = vaults::Entity::find_by_id(id).one(&app.state.db).await.unwrap().unwrap();
    let base = format!("/vaults/{}", vault.id);
    (app, manager, vault, base)
}

fn data(v: &Value) -> Vec<Value> {
    v["data"].as_array().unwrap().clone()
}

fn field(res: &Value, i: usize) -> &str {
    res["errors"][i]["field"].as_str().unwrap()
}

fn rule(res: &Value, i: usize) -> &str {
    res["errors"][i]["code"].as_str().unwrap()
}

/// Another user of the manager's account with `permission` in the vault.
async fn member(app: &TestApp, manager: &users::Model, vault: &vaults::Model, p: VaultPermission) -> users::Model {
    let u = factory::user(&app.state.db, Some(manager.account_id)).await;
    factory::set_permission_in_vault(&app.state.db, &u, p, vault).await;
    u
}

async fn label_contact(app: &TestApp, vault: &vaults::Model, label_id: i64, deleted: bool) {
    let c = factory::contact(&app.state.db, vault).await;
    if deleted {
        let mut a: contacts::ActiveModel = c.clone().into();
        a.deleted_at = Set(Some(monica::db::now()));
        a.update(&app.state.db).await.unwrap();
    }
    contact_label::ActiveModel { label_id: Set(label_id.into()), contact_id: Set(c.id), ..Default::default() }
        .insert(&app.state.db)
        .await
        .unwrap();
}

#[tokio::test]
async fn manages_labels() {
    let (mut app, _, vault, base) = setup().await;
    let labels = format!("{base}/labels");

    let res = app.post(&labels, json!({"name": "  "})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("name", "required"));
    let res = app.post(&labels, json!({"name": "x".repeat(256), "bg_color": "y".repeat(256)})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("name", "max"));
    assert_eq!((field(&res, 1), rule(&res, 1)), ("bg_color", "max"));

    let work = app.post(&labels, json!({"name": " Work stuff ", "description": ""})).await.assert(201).json();
    assert_eq!(work["name"], "Work stuff");
    assert_eq!(work["slug"], "work-stuff");
    assert_eq!(work["vault_id"], vault.id.to_string());
    assert!(work["description"].is_null());
    assert_eq!(work["bg_color"], "bg-neutral-200");
    assert_eq!(work["text_color"], "text-neutral-800");
    assert_eq!(work["contacts_count"], 0);
    assert!(work["created_at"].as_str().unwrap().ends_with('Z'));
    let alpha = app
        .post(
            &labels,
            json!({"name": "alpha", "description": "first", "bg_color": "bg-red-200", "text_color": "text-red-600"}),
        )
        .await
        .assert(201)
        .json();
    assert_eq!(alpha["description"], "first");
    assert_eq!(alpha["bg_color"], "bg-red-200");

    let work_id = work["id"].as_i64().unwrap();
    label_contact(&app, &vault, work_id, false).await;
    label_contact(&app, &vault, work_id, false).await;
    label_contact(&app, &vault, work_id, true).await;

    let list = app.get(&labels).await.assert(200).json();
    let names: Vec<_> = data(&list).iter().map(|l| l["name"].as_str().unwrap().to_owned()).collect();
    assert_eq!(names, ["alpha", "Work stuff"], "by name, locale collation");
    assert_eq!(data(&list)[0]["contacts_count"], 0);
    assert_eq!(data(&list)[1]["contacts_count"], 2, "soft-deleted contacts left out");

    let item = format!("{labels}/{work_id}");
    let l = app.patch(&item, json!({"name": "Office", "bg_color": "bg-sky-200"})).await.assert(200).json();
    assert_eq!(l["name"], "Office");
    assert_eq!(l["slug"], "office");
    assert_eq!(l["bg_color"], "bg-sky-200");
    assert_eq!(l["text_color"], "text-neutral-800", "absent fields are kept");
    assert_eq!(l["contacts_count"], 2);
    let l = app.patch(&item, json!({"description": "desk"})).await.assert(200).json();
    assert_eq!((l["name"].as_str(), l["description"].as_str()), (Some("Office"), Some("desk")));
    let l = app.patch(&item, json!({"text_color": "text-sky-600"})).await.assert(200).json();
    assert_eq!(l["description"], "desk");
    let l = app.patch(&item, json!({"description": null})).await.assert(200).json();
    assert!(l["description"].is_null());
    let res = app.patch(&item, json!({"name": "", "bg_color": " "})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("name", "required"));
    assert_eq!((field(&res, 1), rule(&res, 1)), ("bg_color", "string"));
    app.patch(&item, json!({"description": "d".repeat(65536)})).await.assert(422);
    app.patch(&format!("{labels}/999999"), json!({"name": "x"})).await.assert(404);

    app.delete(&item).await.assert(204);
    assert_eq!(data(&app.get(&labels).await.json()).len(), 1);
    assert!(contact_label::Entity::find().all(&app.state.db).await.unwrap().is_empty(), "removed from every contact");
    app.delete(&item).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn manages_tags() {
    let (mut app, _, vault, base) = setup().await;
    let tags = format!("{base}/tags");

    let res = app.post(&tags, json!({})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("name", "required"));
    app.post(&tags, json!({"name": "t".repeat(256)})).await.assert(422);
    let travel = app.post(&tags, json!({"name": "Travel Notes"})).await.assert(201).json();
    assert_eq!(travel["name"], "Travel Notes");
    assert_eq!(travel["slug"], "travel-notes");
    assert_eq!(travel["vault_id"], vault.id.to_string());
    assert_eq!(travel["posts_count"], 0);
    app.post(&tags, json!({"name": "éte"})).await.assert(201);
    app.post(&tags, json!({"name": "Zoo"})).await.assert(201);

    let tag_id = travel["id"].as_i64().unwrap();
    let journal = journals::ActiveModel { vault_id: Set(vault.id), name: Set("Diary".into()), ..Default::default() }
        .insert(&app.state.db)
        .await
        .unwrap();
    for _ in 0..2 {
        let post = posts::ActiveModel {
            journal_id: Set(journal.id),
            published: Set(true),
            title: Set(Some("Trip".into())),
            view_count: Set(0),
            written_at: Set(monica::db::now()),
            ..Default::default()
        }
        .insert(&app.state.db)
        .await
        .unwrap();
        post_tag::ActiveModel { tag_id: Set(tag_id.into()), post_id: Set(post.id), ..Default::default() }
            .insert(&app.state.db)
            .await
            .unwrap();
    }

    let list = app.get(&tags).await.assert(200).json();
    let names: Vec<_> = data(&list).iter().map(|t| t["name"].as_str().unwrap().to_owned()).collect();
    assert_eq!(names, ["éte", "Travel Notes", "Zoo"]);
    assert_eq!(data(&list)[1]["posts_count"], 2);

    let item = format!("{tags}/{tag_id}");
    let t = app.patch(&item, json!({"name": "Trips"})).await.assert(200).json();
    assert_eq!((t["name"].as_str(), t["slug"].as_str()), (Some("Trips"), Some("trips")));
    assert_eq!(t["posts_count"], 2);
    let res = app.patch(&item, json!({"name": " "})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("name", "required"));
    app.patch(&format!("{tags}/999999"), json!({"name": "x"})).await.assert(404);

    app.delete(&item).await.assert(204);
    assert_eq!(data(&app.get(&tags).await.json()).len(), 2);
    assert!(post_tag::Entity::find().all(&app.state.db).await.unwrap().is_empty(), "removed from every post");
    app.delete(&item).await.assert(404);
    app.finish().await;
}

#[tokio::test]
async fn manages_contact_important_date_types() {
    let (mut app, _, vault, base) = setup().await;
    let types = format!("{base}/contact-important-date-types");

    let list = app.get(&types).await.assert(200).json();
    let seeded = data(&list);
    assert_eq!(seeded.len(), 2);
    assert_eq!(seeded[0]["label"], "Birthdate");
    assert_eq!(seeded[0]["internal_type"], "birthdate");
    assert_eq!(seeded[0]["can_be_deleted"], false);
    assert_eq!(seeded[0]["vault_id"], vault.id.to_string());
    assert_eq!(seeded[1]["internal_type"], "deceased_date");

    let res = app.post(&types, json!({"label": ""})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("label", "required"));
    app.post(&types, json!({"label": "l".repeat(256)})).await.assert(422);
    let created = app.post(&types, json!({"label": " Anniversary "})).await.assert(201).json();
    assert_eq!(created["label"], "Anniversary");
    assert!(created["internal_type"].is_null());
    assert_eq!(created["can_be_deleted"], true);
    let ids: Vec<_> = data(&app.get(&types).await.json()).iter().map(|t| t["id"].as_i64().unwrap()).collect();
    assert_eq!(ids.len(), 3);
    assert!(ids.windows(2).all(|w| w[0] < w[1]), "by id");

    // Renaming a seeded type keeps what makes it special.
    let birthdate = format!("{types}/{}", seeded[0]["id"]);
    let t = app.patch(&birthdate, json!({"label": "Born"})).await.assert(200).json();
    assert_eq!(t["label"], "Born");
    assert_eq!(t["internal_type"], "birthdate");
    assert_eq!(t["can_be_deleted"], false);
    let res = app.patch(&birthdate, json!({})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("label", "required"));
    app.patch(&format!("{types}/999999"), json!({"label": "x"})).await.assert(404);

    assert_eq!(app.delete(&birthdate).await.assert(409).code(), "conflict");
    app.delete(&format!("{types}/{}", created["id"])).await.assert(204);
    app.delete(&format!("{types}/{}", created["id"])).await.assert(404);
    assert_eq!(data(&app.get(&types).await.json()).len(), 2);
    app.finish().await;
}

#[tokio::test]
async fn manages_mood_tracking_parameters() {
    let (mut app, manager, _, base) = setup().await;
    let params = format!("{base}/mood-tracking-parameters");

    let seeded = data(&app.get(&params).await.assert(200).json());
    assert_eq!(seeded.len(), 5);
    assert!(seeded[0]["label"].is_null());
    assert_eq!(seeded[0]["label_translation_key"], "🥳 Awesome");
    assert_eq!(seeded[0]["hex_color"], "bg-lime-500");
    let positions: Vec<_> = seeded.iter().map(|p| p["position"].as_i64().unwrap()).collect();
    assert_eq!(positions, [1, 2, 3, 4, 5]);

    let res = app.post(&params, json!({"label": "Tired"})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("hex_color", "required"));
    let p = app.post(&params, json!({"label": "Tired", "hex_color": "bg-stone-400"})).await.assert(201).json();
    assert_eq!(p["label"], "Tired");
    assert!(p["label_translation_key"].is_null());
    assert_eq!(p["hex_color"], "bg-stone-400");
    assert_eq!(p["position"], 6, "after the last one");

    let item = format!("{params}/{}", p["id"]);
    let u = app.patch(&item, json!({"label": "Exhausted"})).await.assert(200).json();
    assert_eq!((u["label"].as_str(), u["hex_color"].as_str()), (Some("Exhausted"), Some("bg-stone-400")));
    let u = app.patch(&item, json!({"hex_color": "bg-stone-700", "position": 2})).await.assert(200).json();
    assert_eq!((u["label"].as_str(), u["hex_color"].as_str()), (Some("Exhausted"), Some("bg-stone-700")));
    assert_eq!(u["position"], 2);
    let order: Vec<_> = data(&app.get(&params).await.json())
        .iter()
        .map(|x| (x["id"].as_i64().unwrap(), x["position"].as_i64().unwrap()))
        .collect();
    let mut expected: Vec<_> = seeded.iter().map(|x| x["id"].as_i64().unwrap()).collect();
    expected.insert(1, p["id"].as_i64().unwrap());
    assert_eq!(order.iter().map(|o| o.0).collect::<Vec<_>>(), expected, "the siblings in between shifted");
    assert_eq!(order.iter().map(|o| o.1).collect::<Vec<_>>(), [1, 2, 3, 4, 5, 6]);

    // A seeded row given only a colour takes its translated label, as the
    // PHP form sent it.
    let awesome = format!("{params}/{}", seeded[0]["id"]);
    let u = app.patch(&awesome, json!({"hex_color": "bg-lime-300"})).await.assert(200).json();
    assert_eq!(u["label"], "🥳 Awesome");
    assert_eq!(u["hex_color"], "bg-lime-300");

    let res = app.patch(&item, json!({"label": " ", "position": 0})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("label", "required"));
    assert_eq!((field(&res, 1), rule(&res, 1)), ("position", "min"));
    app.patch(&item, json!({"position": "first"})).await.assert(422);
    app.patch(&format!("{params}/999999"), json!({"label": "x"})).await.assert(404);

    let contact_id = monica::entities::user_vault::Entity::find()
        .all(&app.state.db)
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.user_id == manager.id)
        .unwrap()
        .contact_id;
    mood_tracking_events::ActiveModel {
        contact_id: Set(contact_id),
        mood_tracking_parameter_id: Set(p["id"].as_i64().unwrap().into()),
        rated_at: Set(monica::db::now()),
        ..Default::default()
    }
    .insert(&app.state.db)
    .await
    .unwrap();
    app.delete(&item).await.assert(204);
    assert!(mood_tracking_events::Entity::find().all(&app.state.db).await.unwrap().is_empty());
    app.delete(&item).await.assert(404);
    assert_eq!(data(&app.get(&params).await.json()).len(), 5);
    app.finish().await;
}

#[tokio::test]
async fn manages_life_event_categories_and_types() {
    let (mut app, _, vault, base) = setup().await;
    let categories = format!("{base}/life-event-categories");

    let seeded = data(&app.get(&categories).await.assert(200).json());
    let keys: Vec<_> = seeded.iter().map(|c| c["label_translation_key"].as_str().unwrap().to_owned()).collect();
    assert_eq!(keys, ["Transportation", "Social", "Sport", "Work"]);
    assert!(seeded[0]["label"].is_null());
    assert_eq!(seeded[0]["vault_id"], vault.id.to_string());
    assert_eq!(seeded[0]["can_be_deleted"], true);
    let transport = seeded[0]["life_event_types"].as_array().unwrap();
    assert_eq!(transport.len(), 5);
    assert_eq!(transport[0]["label_translation_key"], "Rode a bike");
    assert_eq!(transport[0]["life_event_category_id"], seeded[0]["id"]);
    let positions: Vec<_> = transport.iter().map(|t| t["position"].as_i64().unwrap()).collect();
    assert_eq!(positions, [1, 2, 3, 4, 5]);

    let res = app.post(&categories, json!({"label": null})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("label", "required"));
    let c = app.post(&categories, json!({"label": "Health"})).await.assert(201).json();
    assert_eq!(c["label"], "Health");
    assert_eq!(c["position"], 5);
    assert_eq!(c["can_be_deleted"], true);
    assert_eq!(c["life_event_types"], json!([]));
    let category = format!("{categories}/{}", c["id"]);

    // types
    let types = format!("{category}/life-event-types");
    let res = app.post(&types, json!({"label": ""})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("label", "required"));
    let t1 = app.post(&types, json!({"label": "Got sick"})).await.assert(201).json();
    assert_eq!(t1["label"], "Got sick");
    assert_eq!(t1["position"], 1);
    assert_eq!(t1["can_be_deleted"], true);
    assert_eq!(t1["life_event_category_id"], c["id"]);
    let t2 = app.post(&types, json!({"label": "Recovered"})).await.assert(201).json();
    assert_eq!(t2["position"], 2);
    app.post(&format!("{categories}/999999/life-event-types"), json!({"label": "x"})).await.assert(404);

    let type2 = format!("{types}/{}", t2["id"]);
    let t = app.patch(&type2, json!({"label": "Healed", "position": 1})).await.assert(200).json();
    assert_eq!((t["label"].as_str(), t["position"].as_i64()), (Some("Healed"), Some(1)));
    let t = app.patch(&type2, json!({"position": 2})).await.assert(200).json();
    assert_eq!((t["label"].as_str(), t["position"].as_i64()), (Some("Healed"), Some(2)));
    let res = app.patch(&type2, json!({"position": -1})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("position", "min"));
    // A type is reached through its own category only.
    let wrong = format!("{categories}/{}/life-event-types/{}", seeded[0]["id"], t2["id"]);
    app.patch(&wrong, json!({"label": "x"})).await.assert(404);
    app.delete(&wrong).await.assert(404);

    // categories
    let u = app.patch(&category, json!({"label": "Wellbeing", "position": 1})).await.assert(200).json();
    assert_eq!((u["label"].as_str(), u["position"].as_i64()), (Some("Wellbeing"), Some(1)));
    let labels: Vec<_> = u["life_event_types"].as_array().unwrap().iter().map(|t| t["label"].clone()).collect();
    assert_eq!(labels, [json!("Got sick"), json!("Healed")], "the types by position");
    let list = data(&app.get(&categories).await.json());
    let order: Vec<_> = list.iter().map(|c| (c["id"].clone(), c["position"].as_i64().unwrap())).collect();
    assert_eq!(order[0], (c["id"].clone(), 1));
    assert_eq!(order[1], (seeded[0]["id"].clone(), 2));
    assert_eq!(order.iter().map(|o| o.1).collect::<Vec<_>>(), [1, 2, 3, 4, 5]);
    let res = app.patch(&category, json!({"label": "  "})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("label", "required"));
    app.patch(&format!("{categories}/999999"), json!({"label": "x"})).await.assert(404);

    // can_be_deleted: false is kept by a rename and answers 409 on delete.
    let fixed_type =
        life_event_types::Entity::find_by_id(Id(t1["id"].as_i64().unwrap())).one(&app.state.db).await.unwrap().unwrap();
    let mut a: life_event_types::ActiveModel = fixed_type.into();
    a.can_be_deleted = Set(false);
    a.update(&app.state.db).await.unwrap();
    let type1 = format!("{types}/{}", t1["id"]);
    let t = app.patch(&type1, json!({"label": "Fell ill"})).await.assert(200).json();
    assert_eq!(t["can_be_deleted"], false);
    assert_eq!(app.delete(&type1).await.assert(409).code(), "conflict");
    app.delete(&type2).await.assert(204);
    app.delete(&type2).await.assert(404);

    let social =
        life_event_categories::Entity::find_by_id(Id(seeded[1]["id"].as_i64().unwrap())).one(&app.state.db).await;
    let mut a: life_event_categories::ActiveModel = social.unwrap().unwrap().into();
    a.can_be_deleted = Set(false);
    a.update(&app.state.db).await.unwrap();
    let social = format!("{categories}/{}", seeded[1]["id"]);
    let s = app.patch(&social, json!({"label": "Friends"})).await.assert(200).json();
    assert_eq!(s["can_be_deleted"], false);
    assert_eq!(s["life_event_types"].as_array().unwrap().len(), 6);
    assert_eq!(app.delete(&social).await.assert(409).code(), "conflict");

    app.delete(&category).await.assert(204);
    app.delete(&category).await.assert(404);
    assert_eq!(data(&app.get(&categories).await.json()).len(), 4);
    app.finish().await;
}

#[tokio::test]
async fn manages_quick_fact_templates() {
    let (mut app, _, vault, base) = setup().await;
    let templates = format!("{base}/quick-fact-templates");

    let seeded = data(&app.get(&templates).await.assert(200).json());
    let keys: Vec<_> = seeded.iter().map(|t| t["label_translation_key"].as_str().unwrap().to_owned()).collect();
    assert_eq!(keys, ["Hobbies", "Food preferences"]);
    assert!(seeded[0]["label"].is_null());
    assert_eq!(seeded[0]["position"], 1);
    assert_eq!(seeded[0]["vault_id"], vault.id.to_string());

    let res = app.post(&templates, json!({"label": " "})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("label", "required"));
    app.post(&templates, json!({"label": "q".repeat(256)})).await.assert(422);
    let t = app.post(&templates, json!({"label": "Pets"})).await.assert(201).json();
    assert_eq!(t["label"], "Pets");
    assert!(t["label_translation_key"].is_null());
    assert_eq!(t["position"], 3);

    let item = format!("{templates}/{}", t["id"]);
    let u = app.patch(&item, json!({"label": "Animals"})).await.assert(200).json();
    assert_eq!((u["label"].as_str(), u["position"].as_i64()), (Some("Animals"), Some(3)));
    let u = app.patch(&item, json!({"position": 1})).await.assert(200).json();
    assert_eq!((u["label"].as_str(), u["position"].as_i64()), (Some("Animals"), Some(1)));
    let order: Vec<_> = data(&app.get(&templates).await.json())
        .iter()
        .map(|x| (x["id"].clone(), x["position"].as_i64().unwrap()))
        .collect();
    assert_eq!(order, [(t["id"].clone(), 1), (seeded[0]["id"].clone(), 2), (seeded[1]["id"].clone(), 3)]);
    // A seeded template moves without being renamed.
    let food = format!("{templates}/{}", seeded[1]["id"]);
    let f = app.patch(&food, json!({"position": 1})).await.assert(200).json();
    assert!(f["label"].is_null());
    assert_eq!(f["position"], 1);
    let res = app.patch(&item, json!({"label": "", "position": 0})).await.assert(422).json();
    assert_eq!((field(&res, 0), rule(&res, 0)), ("label", "required"));
    assert_eq!((field(&res, 1), rule(&res, 1)), ("position", "min"));
    app.patch(&format!("{templates}/999999"), json!({"label": "x"})).await.assert(404);

    app.delete(&item).await.assert(204);
    app.delete(&item).await.assert(404);
    assert_eq!(data(&app.get(&templates).await.json()).len(), 2);
    app.finish().await;
}

#[tokio::test]
async fn viewers_read_editors_add_labels_and_tags_and_strangers_see_nothing() {
    let (mut app, manager, vault, base) = setup().await;
    let label = app.post(&format!("{base}/labels"), json!({"name": "Work"})).await.assert(201).json();
    let tag = app.post(&format!("{base}/tags"), json!({"name": "Trips"})).await.assert(201).json();
    let date_type = data(&app.get(&format!("{base}/contact-important-date-types")).await.json())[0]["id"].clone();
    let mood = data(&app.get(&format!("{base}/mood-tracking-parameters")).await.json())[0]["id"].clone();
    let category = data(&app.get(&format!("{base}/life-event-categories")).await.json())[0].clone();
    let life_type = category["life_event_types"][0]["id"].clone();
    let category = category["id"].clone();
    let quick = data(&app.get(&format!("{base}/quick-fact-templates")).await.json())[0]["id"].clone();

    let lists = [
        "labels",
        "tags",
        "contact-important-date-types",
        "mood-tracking-parameters",
        "life-event-categories",
        "quick-fact-templates",
    ];
    let creates = [
        ("contact-important-date-types".to_owned(), json!({"label": "x"})),
        ("mood-tracking-parameters".to_owned(), json!({"label": "x", "hex_color": "bg-red-400"})),
        ("life-event-categories".to_owned(), json!({"label": "x"})),
        (format!("life-event-categories/{category}/life-event-types"), json!({"label": "x"})),
        ("quick-fact-templates".to_owned(), json!({"label": "x"})),
    ];
    let items = [
        (format!("labels/{}", label["id"]), json!({"name": "x"})),
        (format!("tags/{}", tag["id"]), json!({"name": "x"})),
        (format!("contact-important-date-types/{date_type}"), json!({"label": "x"})),
        (format!("mood-tracking-parameters/{mood}"), json!({"label": "x"})),
        (format!("life-event-categories/{category}"), json!({"label": "x"})),
        (format!("life-event-categories/{category}/life-event-types/{life_type}"), json!({"label": "x"})),
        (format!("quick-fact-templates/{quick}"), json!({"label": "x"})),
    ];

    let viewer = member(&app, &manager, &vault, VaultPermission::View).await;
    app.login(&viewer.username).await;
    for l in lists {
        app.get(&format!("{base}/{l}")).await.assert(200);
    }
    app.post(&format!("{base}/labels"), json!({"name": "x"})).await.assert(403);
    app.post(&format!("{base}/tags"), json!({"name": "x"})).await.assert(403);
    for (path, body) in &creates {
        assert_eq!(app.post(&format!("{base}/{path}"), body.clone()).await.assert(403).code(), "forbidden");
    }
    for (path, body) in &items {
        app.patch(&format!("{base}/{path}"), body.clone()).await.assert(403);
        app.delete(&format!("{base}/{path}")).await.assert(403);
    }

    let editor = member(&app, &manager, &vault, VaultPermission::Edit).await;
    app.login(&editor.username).await;
    assert_eq!(app.post(&format!("{base}/labels"), json!({"name": "Home"})).await.assert(201).json()["name"], "Home");
    assert_eq!(app.post(&format!("{base}/tags"), json!({"name": "Food"})).await.assert(201).json()["name"], "Food");
    for (path, body) in &creates {
        app.post(&format!("{base}/{path}"), body.clone()).await.assert(403);
    }
    for (path, body) in &items {
        app.patch(&format!("{base}/{path}"), body.clone()).await.assert(403);
        app.delete(&format!("{base}/{path}")).await.assert(403);
    }

    let stranger = factory::user(&app.state.db, None).await;
    app.login(&stranger.username).await;
    for l in lists {
        assert_eq!(app.get(&format!("{base}/{l}")).await.assert(404).code(), "not_found");
    }
    app.post(&format!("{base}/labels"), json!({"name": "x"})).await.assert(404);
    app.post(&format!("{base}/tags"), json!({"name": "x"})).await.assert(404);
    for (path, body) in &creates {
        app.post(&format!("{base}/{path}"), body.clone()).await.assert(404);
    }
    for (path, body) in &items {
        app.patch(&format!("{base}/{path}"), body.clone()).await.assert(404);
        app.delete(&format!("{base}/{path}")).await.assert(404);
    }

    // The settings of another vault aren't reachable through this one.
    app.login(&manager.username).await;
    let other = app.post("/vaults", json!({"name": "Work"})).await.assert(201).json();
    let other = format!("/vaults/{}", other["id"].as_str().unwrap());
    for (path, body) in &items {
        app.patch(&format!("{other}/{path}"), body.clone()).await.assert(404);
        app.delete(&format!("{other}/{path}")).await.assert(404);
    }
    app.post(&format!("{other}/life-event-categories/{category}/life-event-types"), json!({"label": "x"}))
        .await
        .assert(404);
    app.finish().await;
}
