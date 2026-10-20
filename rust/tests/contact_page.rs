//! The contact page aggregate (`getContactPage`, R041/R057): header modules,
//! tabs, the modules of the active tab, groups and quick facts.

mod common;

use common::TestApp;
use monica::domains::base::VaultPermission;
use monica::domains::settings::create_account::SetupAccount;
use monica::entities::{contacts, users};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::{Value, json};

/// A set-up account's administrator, logged in, with a vault (default
/// template) and a contact in it.
async fn world() -> (TestApp, users::Model, String, String, String) {
    let mut app = TestApp::new().await;
    let user = factory::create_administrator(&app.state.db).await;
    SetupAccount { account_id: user.account_id, author_id: user.id }
        .execute(&app.state.db)
        .await
        .expect("setup account");
    app.login(&user.username).await;
    let v = app.post("/vaults", json!({"name": "Family"})).await.assert(201).json();
    let vault = v["id"].as_str().unwrap().to_owned();
    let own = v["user_contact_id"].as_str().unwrap().to_owned();
    let c = app.post(&format!("/vaults/{vault}/contacts"), json!({"first_name": "Ada"})).await.assert(201).json();
    (app, user, vault, own, c["id"].as_str().unwrap().to_owned())
}

fn types(modules: &Value) -> Vec<String> {
    modules.as_array().unwrap().iter().map(|m| m["type"].as_str().unwrap().to_owned()).collect()
}

#[tokio::test]
async fn shows_the_header_the_tabs_and_the_first_tab() {
    let (mut app, _, vault, own, contact) = world().await;
    let page = app.get(&format!("/vaults/{vault}/contacts/{contact}/page")).await.assert(200).json();

    assert_eq!(page["contact"]["first_name"], "Ada");
    assert_eq!(page["is_current_user_contact"], false);
    // The default template's contact information page, in position order.
    assert_eq!(
        types(&page["header_modules"]),
        [
            "avatar",
            "contact_names",
            "family_summary",
            "important_dates",
            "gender_pronoun",
            "labels",
            "company",
            "religions"
        ]
    );
    for m in page["header_modules"].as_array().unwrap() {
        assert!(m["id"].is_i64());
        assert!(m["name_translation_key"].is_string());
        assert!(m["data"].is_object(), "{m}");
    }
    let tabs = page["template_pages"].as_array().unwrap();
    assert!(tabs.len() >= 2, "the default template has several tabs");
    assert!(tabs.iter().all(|t| t["type"] != "contact_information"));
    assert_eq!(page["active_template_page_id"], tabs[0]["id"]);
    assert!(!page["modules"].as_array().unwrap().is_empty());
    assert_eq!(page["groups"], json!([]));
    assert_eq!(page["quick_fact_templates"].as_array().unwrap().len(), 2, "the vault's seeded quick fact templates");
    assert_eq!(page["quick_facts"], json!([]));

    // Another tab by its slug.
    let second = tabs[1].clone();
    let other = app
        .get(&format!("/vaults/{vault}/contacts/{contact}/page?template_page={}", second["slug"].as_str().unwrap()))
        .await
        .assert(200)
        .json();
    assert_eq!(other["active_template_page_id"], second["id"]);
    assert_ne!(types(&other["modules"]), types(&page["modules"]));

    app.get(&format!("/vaults/{vault}/contacts/{contact}/page?template_page=nope")).await.assert(404);

    // The user's own contact.
    let mine = app.get(&format!("/vaults/{vault}/contacts/{own}/page")).await.assert(200).json();
    assert_eq!(mine["is_current_user_contact"], true);
    app.finish().await;
}

#[tokio::test]
async fn shows_groups_and_quick_facts() {
    let (mut app, _, vault, _, contact) = world().await;
    let base = format!("/vaults/{vault}/contacts/{contact}");
    let page = app.get(&format!("{base}/page")).await.assert(200).json();
    let template = page["quick_fact_templates"][0]["id"].as_i64().unwrap();
    app.post(
        &format!("{base}/quick-facts"),
        json!({"vault_quick_facts_template_id": template, "content": "Likes tea"}),
    )
    .await
    .assert(201);
    for name in ["Zebras", "Ants"] {
        let group = app.post(&format!("/vaults/{vault}/groups"), json!({"name": name})).await.assert(201).json();
        app.post(&format!("{base}/groups"), json!({"group_id": group["id"]})).await.assert(201);
    }

    let page = app.get(&format!("{base}/page")).await.assert(200).json();
    assert_eq!(page["quick_facts"][0]["content"], "Likes tea");
    let names: Vec<&str> = page["groups"].as_array().unwrap().iter().map(|g| g["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Ants", "Zebras"]);
    app.finish().await;
}

#[tokio::test]
async fn a_contact_without_template_has_no_modules() {
    let (mut app, _, vault, _, contact) = world().await;
    let row = contacts::Entity::find_by_id(contact.parse::<monica::db::uuid::Uuid>().unwrap())
        .one(&app.state.db)
        .await
        .unwrap()
        .unwrap();
    let mut row: contacts::ActiveModel = row.into();
    row.template_id = Set(None);
    row.update(&app.state.db).await.unwrap();

    let page = app.get(&format!("/vaults/{vault}/contacts/{contact}/page")).await.assert(200).json();
    assert_eq!(page["header_modules"], json!([]));
    assert_eq!(page["template_pages"], json!([]));
    assert_eq!(page["modules"], json!([]));
    assert!(page["active_template_page_id"].is_null());
    app.finish().await;
}

#[tokio::test]
async fn viewers_see_the_page_and_strangers_dont() {
    let (mut app, user, vault, _, contact) = world().await;
    let vault_model = monica::entities::vaults::Entity::find_by_id(vault.parse::<monica::db::uuid::Uuid>().unwrap())
        .one(&app.state.db)
        .await
        .unwrap()
        .unwrap();
    let viewer = factory::user(&app.state.db, Some(user.account_id)).await;
    factory::set_permission_in_vault(&app.state.db, &viewer, VaultPermission::View, &vault_model).await;
    let stranger = factory::create_user(&app.state.db).await;

    app.login(&viewer.username).await;
    app.get(&format!("/vaults/{vault}/contacts/{contact}/page")).await.assert(200);
    app.login(&stranger.username).await;
    app.get(&format!("/vaults/{vault}/contacts/{contact}/page")).await.assert(404);
    app.finish().await;
}
