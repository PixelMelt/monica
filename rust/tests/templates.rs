//! Contact templates, their pages and modules, the module catalogue, post
//! templates and group types over HTTP (`21-account-templates.yaml`).

mod common;

use common::TestApp;
use monica::db::id::Id;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::domains::settings::create_account::SetupAccount;
use monica::domains::settings::manage_post_templates::{CreatePostTemplate, CreatePostTemplateSection};
use monica::entities::{modules, users};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::{Value, json};

fn id(v: &Value) -> i64 {
    v["id"].as_i64().unwrap_or_else(|| panic!("no id in {v}"))
}

fn ids(list: &Value) -> Vec<i64> {
    list.as_array().unwrap().iter().map(id).collect()
}

async fn module(app: &TestApp, account_id: Uuid, name: &str, ty: &str, reserved: bool) -> modules::Model {
    modules::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(name.into())),
        r#type: Set(Some(ty.into())),
        reserved_to_contact_information: Set(reserved),
        can_be_deleted: Set(false),
        pagination: Set((ty == "notes").then_some(3)),
        ..Default::default()
    }
    .insert(&app.state.db)
    .await
    .unwrap()
}

async fn admin(app: &mut TestApp) -> users::Model {
    let user = factory::create_administrator(&app.state.db).await;
    app.login(&user.username).await;
    user
}

#[tokio::test]
async fn creates_reads_renames_and_deletes_templates() {
    let mut app = TestApp::new().await;
    admin(&mut app).await;

    let res = app.post("/account/templates", json!({"name": "  "})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["field"], "name");
    assert_eq!(res.json()["errors"][0]["code"], "required");
    let res = app.post("/account/templates", json!({"name": "x".repeat(256)})).await.assert(422);
    assert_eq!(res.json()["errors"][0]["code"], "max");

    let t = app.post("/account/templates", json!({"name": " Business "})).await.assert(201).json();
    assert_eq!(t["name"], "Business");
    assert!(t["name_translation_key"].is_null());
    assert_eq!(t["can_be_deleted"], true);
    let other = app.post("/account/templates", json!({"name": "Family"})).await.assert(201).json();
    let path = format!("/account/templates/{}", id(&t));

    let list = app.get("/account/templates").await.assert(200).json();
    assert_eq!(ids(&list["data"]), [id(&t), id(&other)], "ordered by id");

    let detail = app.get(&path).await.assert(200).json();
    assert_eq!(detail["name"], "Business");
    let pages = detail["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 1, "CreateTemplate adds the contact information page");
    assert_eq!(pages[0]["type"], "contact_information");
    assert_eq!(pages[0]["position"], 1);
    assert_eq!(pages[0]["can_be_deleted"], false);
    assert_eq!(pages[0]["name_translation_key"], "Contact information");
    assert_eq!(pages[0]["slug"], "contact-information");
    assert_eq!(pages[0]["template_id"], id(&t));

    let renamed = app.patch(&path, json!({"name": "Work"})).await.assert(200).json();
    assert_eq!((renamed["id"].as_i64(), renamed["name"].as_str()), (Some(id(&t)), Some("Work")));
    app.patch(&path, json!({})).await.assert(422);
    app.patch(&path, json!({"name": ""})).await.assert(422);
    app.patch(&path, json!({"name": 3})).await.assert(422);

    app.get("/account/templates/999999").await.assert(404);
    app.patch("/account/templates/999999", json!({"name": "x"})).await.assert(404);
    app.delete("/account/templates/999999").await.assert(404);

    app.delete(&path).await.assert(204);
    app.get(&path).await.assert(404);
    assert_eq!(ids(&app.get("/account/templates").await.json()["data"]), [id(&other)]);
    app.finish().await;
}

#[tokio::test]
async fn deleting_a_template_unsets_it_on_vaults() {
    let mut app = TestApp::new().await;
    let user = admin(&mut app).await;
    let vault = factory::create_vault_user(&app.state.db, &user, VaultPermission::Manage).await;
    let t = app.post("/account/templates", json!({"name": "Business"})).await.assert(201).json();
    let v = app.patch(&format!("/vaults/{}", vault.id), json!({"default_template_id": id(&t)})).await.assert(200);
    assert_eq!(v.json()["default_template_id"], id(&t));
    app.delete(&format!("/account/templates/{}", id(&t))).await.assert(204);
    assert!(app.get(&format!("/vaults/{}", vault.id)).await.json()["default_template_id"].is_null());
    app.finish().await;
}

#[tokio::test]
async fn manages_template_pages() {
    let mut app = TestApp::new().await;
    admin(&mut app).await;
    let t = app.post("/account/templates", json!({"name": "Business"})).await.assert(201).json();
    let base = format!("/account/templates/{}/pages", id(&t));
    let info = id(&app.get(&format!("/account/templates/{}", id(&t))).await.json()["pages"][0]);

    app.post(&base, json!({})).await.assert(422);
    app.post(&base, json!({"name": "y".repeat(256)})).await.assert(422);
    app.post("/account/templates/999999/pages", json!({"name": "Social"})).await.assert(404);

    let social = app.post(&base, json!({"name": "Social life"})).await.assert(201).json();
    assert_eq!(social["slug"], "social-life");
    assert_eq!(social["position"], 2);
    assert!(social["type"].is_null());
    assert_eq!(social["can_be_deleted"], true);
    assert_eq!(social["template_id"], id(&t));
    let work = app.post(&base, json!({"name": "Work"})).await.assert(201).json();
    assert_eq!(work["position"], 3);

    // Moving shifts the others; renaming regenerates the slug.
    let moved = app.patch(&format!("{base}/{}", id(&work)), json!({"position": 2})).await.assert(200).json();
    assert_eq!(moved["position"], 2);
    let pages = app.get(&format!("/account/templates/{}", id(&t))).await.json()["pages"].clone();
    assert_eq!(ids(&pages), [info, id(&work), id(&social)]);
    assert_eq!(pages[2]["position"], 3);
    let renamed =
        app.patch(&format!("{base}/{}", id(&work)), json!({"name": "Jobs & Career", "position": 3})).await.assert(200);
    let renamed = renamed.json();
    assert_eq!((renamed["name"].as_str(), renamed["slug"].as_str()), (Some("Jobs & Career"), Some("jobs-career")));
    assert_eq!(renamed["position"], 3);
    let unchanged = app.patch(&format!("{base}/{}", id(&work)), json!({})).await.assert(200).json();
    assert_eq!(unchanged["name"], "Jobs & Career");
    app.patch(&format!("{base}/{}", id(&work)), json!({"position": 0})).await.assert(422);
    app.patch(&format!("{base}/{}", id(&work)), json!({"name": " "})).await.assert(422);

    let detail = app.get(&format!("{base}/{}", id(&social))).await.assert(200).json();
    assert_eq!(detail["name"], "Social life");
    assert_eq!(detail["slug"], "social-life");
    assert_eq!(detail["modules"], json!([]));

    // A page of another template is not found through this one.
    let other = app.post("/account/templates", json!({"name": "Other"})).await.assert(201).json();
    app.get(&format!("/account/templates/{}/pages/{}", id(&other), id(&social))).await.assert(404);
    app.patch(&format!("/account/templates/{}/pages/{}", id(&other), id(&social)), json!({"name": "x"}))
        .await
        .assert(404);
    app.delete(&format!("/account/templates/{}/pages/{}", id(&other), id(&social))).await.assert(404);

    // F18: the contact information page can't be deleted.
    assert_eq!(app.delete(&format!("{base}/{info}")).await.assert(409).code(), "conflict");
    app.delete(&format!("{base}/{}", id(&social))).await.assert(204);
    app.get(&format!("{base}/{}", id(&social))).await.assert(404);
    let pages = app.get(&format!("/account/templates/{}", id(&t))).await.json()["pages"].clone();
    assert_eq!(ids(&pages), [info, id(&work)]);
    assert_eq!(pages[1]["position"], 3, "the other pages keep their positions");
    app.finish().await;
}

#[tokio::test]
async fn places_modules_on_template_pages() {
    let mut app = TestApp::new().await;
    let user = admin(&mut app).await;
    let avatar = module(&app, user.account_id, "Avatar", "avatar", true).await;
    let notes = module(&app, user.account_id, "Notes", "notes", false).await;
    let pets = module(&app, user.account_id, "Pets", "pets", false).await;
    let stranger = factory::create_administrator(&app.state.db).await;
    let foreign = module(&app, stranger.account_id, "Calls", "calls", false).await;

    let catalogue = app.get("/account/modules").await.assert(200).json();
    assert_eq!(ids(&catalogue["data"]), [avatar.id.0, notes.id.0, pets.id.0]);
    assert_eq!(catalogue["data"][1]["type"], "notes");
    assert_eq!(catalogue["data"][1]["pagination"], 3);
    assert_eq!(catalogue["data"][0]["reserved_to_contact_information"], true);
    assert_eq!(catalogue["data"][0]["can_be_deleted"], false);
    assert!(catalogue["data"][0]["name_translation_key"].is_null());

    let t = app.post("/account/templates", json!({"name": "Business"})).await.assert(201).json();
    let tpath = format!("/account/templates/{}", id(&t));
    let info = id(&app.get(&tpath).await.json()["pages"][0]);
    let social = id(&app.post(&format!("{tpath}/pages"), json!({"name": "Social"})).await.assert(201).json());
    let page = format!("{tpath}/pages/{social}");

    let detail = app.get(&page).await.assert(200).json();
    assert_eq!(
        ids(&detail["available_modules"]),
        [notes.id.0, pets.id.0],
        "modules not reserved to contact information"
    );
    assert_eq!(detail["available_modules"][0]["already_used"], false);
    let info_detail = app.get(&format!("{tpath}/pages/{info}")).await.assert(200).json();
    assert_eq!(info_detail["type"], "contact_information");
    assert_eq!(ids(&info_detail["available_modules"]), [avatar.id.0]);

    let placed = app.put(&format!("{page}/modules/{}", pets.id), json!({})).await.assert(200).json();
    assert_eq!((id(&placed), placed["position"].as_i64()), (pets.id.0, Some(1)));
    assert_eq!(placed["type"], "pets");
    let placed = app.put(&format!("{page}/modules/{}", notes.id), json!({})).await.assert(200).json();
    assert_eq!(placed["position"], 2);
    let again = app.put(&format!("{page}/modules/{}", pets.id), json!({})).await.assert(200).json();
    assert_eq!(again["position"], 1, "already on the page: nothing changes");

    let detail = app.get(&page).await.json();
    assert_eq!(ids(&detail["modules"]), [pets.id.0, notes.id.0]);
    assert_eq!(detail["modules"][1]["position"], 2);
    assert!(detail["available_modules"].as_array().unwrap().iter().all(|m| m["already_used"] == true));

    let moved = app.patch(&format!("{page}/modules/{}", notes.id), json!({"position": 1})).await.assert(200).json();
    assert_eq!(moved["position"], 1);
    let detail = app.get(&page).await.json();
    assert_eq!(ids(&detail["modules"]), [notes.id.0, pets.id.0]);
    assert_eq!(detail["modules"][1]["position"], 2);
    app.patch(&format!("{page}/modules/{}", notes.id), json!({})).await.assert(422);
    app.patch(&format!("{page}/modules/{}", notes.id), json!({"position": -1})).await.assert(422);
    app.patch(&format!("{page}/modules/{}", avatar.id), json!({"position": 1})).await.assert(404);

    app.put(&format!("{page}/modules/{}", foreign.id), json!({})).await.assert(404);
    app.put(&format!("{page}/modules/999999"), json!({})).await.assert(404);
    app.put(&format!("{tpath}/pages/999999/modules/{}", notes.id), json!({})).await.assert(404);

    app.delete(&format!("{page}/modules/{}", notes.id)).await.assert(204);
    // F11: removing a module that isn't on the page is a 404, not a re-add.
    app.delete(&format!("{page}/modules/{}", notes.id)).await.assert(404);
    let detail = app.get(&page).await.json();
    assert_eq!(ids(&detail["modules"]), [pets.id.0]);
    assert_eq!(detail["modules"][0]["position"], 2, "the others keep their positions");
    app.finish().await;
}

#[tokio::test]
async fn manages_post_templates_and_sections() {
    let mut app = TestApp::new().await;
    let user = admin(&mut app).await;
    assert_eq!(app.get("/account/post-templates").await.assert(200).json()["data"], json!([]));

    app.post("/account/post-templates", json!({"label": ""})).await.assert(422);
    let diary = app.post("/account/post-templates", json!({"label": "Diary"})).await.assert(201).json();
    assert_eq!(diary["label"], "Diary");
    assert!(diary["label_translation_key"].is_null());
    assert_eq!((diary["position"].as_i64(), diary["can_be_deleted"].as_bool()), (Some(1), Some(true)));
    assert_eq!(diary["sections"], json!([]));
    let travel = app.post("/account/post-templates", json!({"label": "Travel"})).await.assert(201).json();
    assert_eq!(travel["position"], 2);

    let sections = format!("/account/post-templates/{}/sections", id(&diary));
    app.post(&sections, json!({})).await.assert(422);
    app.post("/account/post-templates/999999/sections", json!({"label": "x"})).await.assert(404);
    let morning = app.post(&sections, json!({"label": "Morning"})).await.assert(201).json();
    assert_eq!(morning["post_template_id"], id(&diary));
    assert_eq!((morning["position"].as_i64(), morning["can_be_deleted"].as_bool()), (Some(1), Some(true)));
    let evening = app.post(&sections, json!({"label": "Evening"})).await.assert(201).json();
    assert_eq!(evening["position"], 2);

    let moved = app.patch(&format!("{sections}/{}", id(&evening)), json!({"position": 1})).await.assert(200).json();
    assert_eq!(moved["position"], 1);
    let renamed = app.patch(&format!("{sections}/{}", id(&evening)), json!({"label": "Night"})).await.assert(200);
    assert_eq!(renamed.json()["label"], "Night");
    app.patch(&format!("{sections}/{}", id(&evening)), json!({"label": ""})).await.assert(422);
    app.patch(&format!("{sections}/{}", id(&evening)), json!({"position": 0})).await.assert(422);
    let other_sections = format!("/account/post-templates/{}/sections", id(&travel));
    app.patch(&format!("{other_sections}/{}", id(&evening)), json!({"label": "x"})).await.assert(404);
    app.delete(&format!("{other_sections}/{}", id(&evening))).await.assert(404);

    let moved = app.patch(&format!("/account/post-templates/{}", id(&travel)), json!({"position": 1})).await;
    assert_eq!(moved.assert(200).json()["position"], 1);
    let renamed = app.patch(&format!("/account/post-templates/{}", id(&diary)), json!({"label": "Journal"})).await;
    let renamed = renamed.assert(200).json();
    assert_eq!((renamed["label"].as_str(), renamed["position"].as_i64()), (Some("Journal"), Some(2)));
    assert_eq!(ids(&renamed["sections"]), [id(&evening), id(&morning)], "sections by position");
    app.patch(&format!("/account/post-templates/{}", id(&diary)), json!({"label": " "})).await.assert(422);
    app.patch("/account/post-templates/999999", json!({"label": "x"})).await.assert(404);

    let list = app.get("/account/post-templates").await.json();
    assert_eq!(ids(&list["data"]), [id(&travel), id(&diary)]);
    assert_eq!(ids(&list["data"][1]["sections"]), [id(&evening), id(&morning)]);

    // F18: rows that can't be deleted answer 409.
    let fixed = CreatePostTemplate {
        account_id: user.account_id,
        author_id: user.id,
        label: None,
        label_translation_key: Some("Regular post".into()),
        can_be_deleted: false,
    }
    .execute(&app.state.db)
    .await
    .unwrap();
    let fixed_section = CreatePostTemplateSection {
        account_id: user.account_id,
        author_id: user.id,
        post_template_id: diary["id"].as_i64().map(Id).unwrap(),
        label: None,
        label_translation_key: Some("Content".into()),
        can_be_deleted: false,
    }
    .execute(&app.state.db)
    .await
    .unwrap();
    app.delete(&format!("/account/post-templates/{}", fixed.id)).await.assert(409);
    app.delete(&format!("{sections}/{}", fixed_section.id)).await.assert(409);

    app.delete(&format!("{sections}/{}", id(&morning))).await.assert(204);
    app.delete(&format!("/account/post-templates/{}", id(&travel))).await.assert(204);
    app.delete("/account/post-templates/999999").await.assert(404);
    let list = app.get("/account/post-templates").await.json();
    assert_eq!(ids(&list["data"]), [id(&diary), fixed.id.0]);
    assert_eq!(ids(&list["data"][0]["sections"]), [id(&evening), fixed_section.id.0]);
    app.finish().await;
}

#[tokio::test]
async fn manages_group_types_and_roles() {
    let mut app = TestApp::new().await;
    admin(&mut app).await;
    assert_eq!(app.get("/account/group-types").await.assert(200).json()["data"], json!([]));

    app.post("/account/group-types", json!({"label": "z".repeat(256)})).await.assert(422);
    let family = app.post("/account/group-types", json!({"label": "Family"})).await.assert(201).json();
    assert_eq!((family["label"].as_str(), family["position"].as_i64()), (Some("Family"), Some(1)));
    assert!(family["label_translation_key"].is_null());
    assert_eq!(family["roles"], json!([]));
    let club = app.post("/account/group-types", json!({"label": "Club"})).await.assert(201).json();
    assert_eq!(club["position"], 2);
    let team = app.post("/account/group-types", json!({"label": "Team"})).await.assert(201).json();

    let roles = format!("/account/group-types/{}/roles", id(&family));
    app.post(&roles, json!({"label": null})).await.assert(422);
    app.post("/account/group-types/999999/roles", json!({"label": "x"})).await.assert(404);
    let parent = app.post(&roles, json!({"label": "Parent"})).await.assert(201).json();
    assert_eq!((parent["group_type_id"].as_i64(), parent["position"].as_i64()), (Some(id(&family)), Some(1)));
    let child = app.post(&roles, json!({"label": "Child"})).await.assert(201).json();
    assert_eq!(child["position"], 2);
    let sibling = app.post(&roles, json!({"label": "Sibling"})).await.assert(201).json();

    let moved = app.patch(&format!("{roles}/{}", id(&child)), json!({"position": 1})).await.assert(200).json();
    assert_eq!(moved["position"], 1);
    let renamed = app.patch(&format!("{roles}/{}", id(&child)), json!({"label": "Kid"})).await.assert(200).json();
    assert_eq!((renamed["label"].as_str(), renamed["position"].as_i64()), (Some("Kid"), Some(1)));
    app.patch(&format!("{roles}/{}", id(&child)), json!({"label": ""})).await.assert(422);
    app.patch(&format!("/account/group-types/{}/roles/{}", id(&club), id(&child)), json!({"label": "x"}))
        .await
        .assert(404);
    app.delete(&format!("/account/group-types/{}/roles/{}", id(&club), id(&child))).await.assert(404);

    let moved = app.patch(&format!("/account/group-types/{}", id(&club)), json!({"position": 1})).await;
    assert_eq!(moved.assert(200).json()["position"], 1);
    let renamed = app.patch(&format!("/account/group-types/{}", id(&family)), json!({"label": "Kin"})).await;
    let renamed = renamed.assert(200).json();
    assert_eq!((renamed["label"].as_str(), renamed["position"].as_i64()), (Some("Kin"), Some(2)));
    assert_eq!(ids(&renamed["roles"]), [id(&child), id(&parent), id(&sibling)]);
    app.patch(&format!("/account/group-types/{}", id(&family)), json!({"position": 0})).await.assert(422);
    app.patch("/account/group-types/999999", json!({"label": "x"})).await.assert(404);

    // Deleting moves the ones after it up by one.
    app.delete(&format!("{roles}/{}", id(&child))).await.assert(204);
    app.delete(&format!("/account/group-types/{}", id(&club))).await.assert(204);
    app.delete("/account/group-types/999999").await.assert(404);
    let list = app.get("/account/group-types").await.json();
    assert_eq!(ids(&list["data"]), [id(&family), id(&team)]);
    assert_eq!((list["data"][0]["position"].as_i64(), list["data"][1]["position"].as_i64()), (Some(1), Some(2)));
    let kept = &list["data"][0]["roles"];
    assert_eq!(ids(kept), [id(&parent), id(&sibling)]);
    assert_eq!((kept[0]["position"].as_i64(), kept[1]["position"].as_i64()), (Some(1), Some(2)));
    app.finish().await;
}

#[tokio::test]
async fn members_read_and_only_administrators_write() {
    let mut app = TestApp::new().await;
    let user = admin(&mut app).await;
    let m = module(&app, user.account_id, "Notes", "notes", false).await;
    let t = id(&app.post("/account/templates", json!({"name": "Business"})).await.assert(201).json());
    let p = id(&app.post(&format!("/account/templates/{t}/pages"), json!({"name": "Social"})).await.json());
    app.put(&format!("/account/templates/{t}/pages/{p}/modules/{}", m.id), json!({})).await.assert(200);
    let pt = id(&app.post("/account/post-templates", json!({"label": "Diary"})).await.json());
    let s = id(&app.post(&format!("/account/post-templates/{pt}/sections"), json!({"label": "Body"})).await.json());
    let gt = id(&app.post("/account/group-types", json!({"label": "Family"})).await.json());
    let r = id(&app.post(&format!("/account/group-types/{gt}/roles"), json!({"label": "Parent"})).await.json());

    let reads = [
        "/account/templates".to_owned(),
        format!("/account/templates/{t}"),
        format!("/account/templates/{t}/pages/{p}"),
        "/account/modules".to_owned(),
        "/account/post-templates".to_owned(),
        "/account/group-types".to_owned(),
    ];
    let body = json!({"name": "x", "label": "x", "position": 1});
    let writes = [
        ("POST", "/account/templates".to_owned()),
        ("PATCH", format!("/account/templates/{t}")),
        ("DELETE", format!("/account/templates/{t}")),
        ("POST", format!("/account/templates/{t}/pages")),
        ("PATCH", format!("/account/templates/{t}/pages/{p}")),
        ("DELETE", format!("/account/templates/{t}/pages/{p}")),
        ("PUT", format!("/account/templates/{t}/pages/{p}/modules/{}", m.id)),
        ("PATCH", format!("/account/templates/{t}/pages/{p}/modules/{}", m.id)),
        ("DELETE", format!("/account/templates/{t}/pages/{p}/modules/{}", m.id)),
        ("POST", "/account/post-templates".to_owned()),
        ("PATCH", format!("/account/post-templates/{pt}")),
        ("DELETE", format!("/account/post-templates/{pt}")),
        ("POST", format!("/account/post-templates/{pt}/sections")),
        ("PATCH", format!("/account/post-templates/{pt}/sections/{s}")),
        ("DELETE", format!("/account/post-templates/{pt}/sections/{s}")),
        ("POST", "/account/group-types".to_owned()),
        ("PATCH", format!("/account/group-types/{gt}")),
        ("DELETE", format!("/account/group-types/{gt}")),
        ("POST", format!("/account/group-types/{gt}/roles")),
        ("PATCH", format!("/account/group-types/{gt}/roles/{r}")),
        ("DELETE", format!("/account/group-types/{gt}/roles/{r}")),
    ];

    // Q9: any member of the account reads, only administrators write.
    let member = factory::user(&app.state.db, Some(user.account_id)).await;
    app.login(&member.username).await;
    for path in &reads {
        app.get(path).await.assert(200);
    }
    for (method, path) in &writes {
        let res = app.request(method.parse().unwrap(), path, Some(body.clone())).await;
        assert_eq!(res.assert(403).code(), "forbidden", "{method} {path}");
    }

    // Another account's administrator sees none of it.
    let stranger = factory::create_administrator(&app.state.db).await;
    app.login(&stranger.username).await;
    assert_eq!(app.get("/account/templates").await.assert(200).json()["data"], json!([]));
    assert_eq!(app.get("/account/modules").await.assert(200).json()["data"], json!([]));
    assert_eq!(app.get("/account/post-templates").await.assert(200).json()["data"], json!([]));
    assert_eq!(app.get("/account/group-types").await.assert(200).json()["data"], json!([]));
    for path in &reads[1..3] {
        app.get(path).await.assert(404);
    }
    for (method, path) in writes.iter().filter(|(_, p)| p.matches('/').count() > 2) {
        let res = app.request(method.parse().unwrap(), path, Some(body.clone())).await;
        assert_eq!(res.assert(404).code(), "not_found", "{method} {path}");
    }

    app.clear_cookies();
    app.get("/account/templates").await.assert(401);
    app.finish().await;
}

#[tokio::test]
async fn the_seeded_defaults_cannot_be_deleted() {
    let mut app = TestApp::new().await;
    let user = admin(&mut app).await;
    SetupAccount { account_id: user.account_id, author_id: user.id }.execute(&app.state.db).await.unwrap();

    let templates = app.get("/account/templates").await.json()["data"].clone();
    assert_eq!(templates.as_array().unwrap().len(), 1);
    assert_eq!(templates[0]["can_be_deleted"], false);
    assert_eq!(templates[0]["name_translation_key"], "Default template");
    let t = id(&templates[0]);
    let pages = app.get(&format!("/account/templates/{t}")).await.json()["pages"].clone();
    assert_eq!(pages[0]["type"], "contact_information");
    let positions: Vec<i64> = pages.as_array().unwrap().iter().map(|p| p["position"].as_i64().unwrap()).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "pages by position: {positions:?}");
    let info = app.get(&format!("/account/templates/{t}/pages/{}", id(&pages[0]))).await.json();
    assert!(!info["modules"].as_array().unwrap().is_empty());
    assert!(info["modules"].as_array().unwrap().iter().all(|m| m["reserved_to_contact_information"] == true));
    assert!(info["available_modules"].as_array().unwrap().iter().all(|m| m["reserved_to_contact_information"] == true));

    let modules = app.get("/account/modules").await.json()["data"].clone();
    let notes = modules.as_array().unwrap().iter().find(|m| m["type"] == "notes").expect("a notes module");
    assert!(notes["name_translation_key"].is_string());

    assert_eq!(app.delete(&format!("/account/templates/{t}")).await.assert(409).code(), "conflict");
    let regular = app.get("/account/post-templates").await.json()["data"][0].clone();
    assert_eq!(regular["label_translation_key"], "Regular post");
    app.delete(&format!("/account/post-templates/{}", id(&regular))).await.assert(409);
    assert!(!app.get("/account/group-types").await.json()["data"].as_array().unwrap().is_empty());
    app.finish().await;
}
