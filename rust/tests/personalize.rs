//! Account personalization over HTTP: genders, pronouns, address types, pet
//! categories, contact information types, call reasons, gift occasions and
//! states, relationship types, religions, currencies and emotions.

mod common;

use common::TestApp;
use monica::domains::settings::create_account::SetupAccount;
use monica::entities::users;
use monica::testing::factory;
use serde_json::{Value, json};

/// An administrator of a set-up account (seeded lists), a plain member of
/// the same account, and the administrator of another account.
struct World {
    app: TestApp,
    admin: users::Model,
    member: users::Model,
    stranger: users::Model,
}

async fn world() -> World {
    let app = TestApp::new().await;
    let admin = factory::create_administrator(&app.state.db).await;
    let member = factory::user(&app.state.db, Some(admin.account_id)).await;
    let stranger = factory::create_administrator(&app.state.db).await;
    SetupAccount { account_id: admin.account_id, author_id: admin.id }
        .execute(&app.state.db)
        .await
        .expect("setup account");
    World { app, admin, member, stranger }
}

impl World {
    async fn as_admin(&mut self) {
        let email = self.admin.username.clone();
        self.app.login(&email).await;
    }

    async fn as_member(&mut self) {
        let email = self.member.username.clone();
        self.app.login(&email).await;
    }

    async fn as_stranger(&mut self) {
        let email = self.stranger.username.clone();
        self.app.login(&email).await;
    }
}

fn data(list: &Value) -> &Vec<Value> {
    list["data"].as_array().expect("a data array")
}

fn ids(list: &Value) -> Vec<i64> {
    data(list).iter().map(|i| i["id"].as_i64().unwrap()).collect()
}

/// The item of `list` whose `key` is `value`.
fn item<'a>(list: &'a Value, key: &str, value: &str) -> &'a Value {
    data(list).iter().find(|i| i[key] == value).unwrap_or_else(|| panic!("no {key} = {value} in {list}"))
}

fn sorted(v: &[i64]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}

/// The field and code of the first validation error.
fn first_error(res: common::TestResponse) -> (String, String) {
    let body = res.assert(422).json();
    assert_eq!(body["code"], "validation_failed");
    (body["errors"][0]["field"].as_str().unwrap().to_owned(), body["errors"][0]["code"].as_str().unwrap().to_owned())
}

/// A list of named rows (`TranslatableName` + id): members read it, an
/// administrator creates, renames and deletes, other accounts see nothing.
async fn manages_named_rows(path: &str, seeded_key: &str) -> (World, Value) {
    let mut w = world().await;
    w.as_member().await;
    let list = w.app.get(path).await.assert(200).json();
    assert!(sorted(&ids(&list)), "by id: {list}");
    let seeded = item(&list, "name_translation_key", seeded_key);
    assert!(seeded["name"].is_null());
    let seeded_id = seeded["id"].as_i64().unwrap();
    w.app.post(path, json!({"name": "Mine"})).await.assert(403);
    w.app.patch(&format!("{path}/{seeded_id}"), json!({"name": "Mine"})).await.assert(403);
    w.app.delete(&format!("{path}/{seeded_id}")).await.assert(403);

    w.as_admin().await;
    assert_eq!(first_error(w.app.post(path, json!({"name": "   "})).await), ("name".into(), "required".into()));
    assert_eq!(first_error(w.app.post(path, json!({})).await), ("name".into(), "required".into()));
    assert_eq!(first_error(w.app.post(path, json!({"name": "x".repeat(256)})).await), ("name".into(), "max".into()));
    let created = w.app.post(path, json!({"name": "  Custom  "})).await.assert(201).json();
    assert_eq!(created["name"], "Custom");
    assert!(created["name_translation_key"].is_null());
    let id = created["id"].as_i64().unwrap();
    let item_path = format!("{path}/{id}");

    let renamed = w.app.patch(&item_path, json!({"name": "Renamed"})).await.assert(200).json();
    assert_eq!(renamed["name"], "Renamed");
    assert_eq!(renamed["id"], id);
    assert_eq!(first_error(w.app.patch(&item_path, json!({})).await), ("name".into(), "required".into()));
    assert_eq!(first_error(w.app.patch(&item_path, json!({"name": ""})).await), ("name".into(), "required".into()));
    // A seeded row gets a name of its own.
    let named = w.app.patch(&format!("{path}/{seeded_id}"), json!({"name": "Seeded"})).await.assert(200).json();
    assert_eq!((named["name"].as_str(), named["name_translation_key"].as_str()), (Some("Seeded"), Some(seeded_key)));
    w.app.patch(&format!("{path}/999999"), json!({"name": "Nope"})).await.assert(404);
    w.app.delete(&format!("{path}/999999")).await.assert(404);

    w.as_stranger().await;
    assert!(!ids(&w.app.get(path).await.assert(200).json()).contains(&id));
    w.app.patch(&item_path, json!({"name": "Mine"})).await.assert(404);
    w.app.delete(&item_path).await.assert(404);

    w.as_admin().await;
    let list = w.app.get(path).await.json();
    assert_eq!(item(&list, "name", "Renamed")["id"], id);
    w.app.delete(&item_path).await.assert(204);
    assert!(!ids(&w.app.get(path).await.json()).contains(&id));
    w.app.patch(&item_path, json!({"name": "Gone"})).await.assert(404);
    (w, created)
}

#[tokio::test]
async fn manages_genders() {
    let (w, created) = manages_named_rows("/account/genders", "Male").await;
    assert!(created["type"].is_null(), "user-created genders have no vCard code");
    w.app.finish().await;
}

#[tokio::test]
async fn manages_pronouns() {
    let (w, _) = manages_named_rows("/account/pronouns", "she/her").await;
    w.app.finish().await;
}

#[tokio::test]
async fn manages_pet_categories() {
    let (w, _) = manages_named_rows("/account/pet-categories", "Dog").await;
    w.app.finish().await;
}

#[tokio::test]
async fn manages_address_types() {
    let (mut w, created) = manages_named_rows("/account/address-types", "🏢 Work").await;
    assert_eq!(created["type"], "custom", "the default type");
    let list = w.app.get("/account/address-types").await.json();
    assert_eq!(item(&list, "name_translation_key", "🏡 Home")["type"], "home");

    let typed = w.app.post("/account/address-types", json!({"name": "Boat", "type": "boat"})).await.assert(201).json();
    assert_eq!(typed["type"], "boat");
    let res = w.app.post("/account/address-types", json!({"name": "Boat", "type": "x".repeat(256)})).await;
    assert_eq!(first_error(res), ("type".into(), "max".into()));
    let renamed = w
        .app
        .patch(&format!("/account/address-types/{}", typed["id"]), json!({"name": "Ship"}))
        .await
        .assert(200)
        .json();
    assert_eq!((renamed["name"].as_str(), renamed["type"].as_str()), (Some("Ship"), Some("boat")));
    w.app.finish().await;
}

#[tokio::test]
async fn lists_emotions() {
    let mut w = world().await;
    w.as_member().await;
    let list = w.app.get("/account/emotions").await.assert(200).json();
    assert_eq!(data(&list).len(), 3);
    assert!(sorted(&ids(&list)));
    let negative = &data(&list)[0];
    assert_eq!(negative["type"], "negative");
    assert_eq!(negative["name_translation_key"], "😡 Negative");
    assert!(negative["name"].is_null());
    assert_eq!(data(&list)[2]["type"], "positive");

    w.as_stranger().await;
    assert!(data(&w.app.get("/account/emotions").await.assert(200).json()).is_empty());
    w.app.clear_cookies();
    w.app.get("/account/emotions").await.assert(401);
    w.app.finish().await;
}

#[tokio::test]
async fn manages_contact_information_types() {
    let mut w = world().await;
    let path = "/account/contact-information-types";
    w.as_member().await;
    let list = w.app.get(path).await.assert(200).json();
    assert!(sorted(&ids(&list)));
    let email = item(&list, "type", "email").clone();
    assert_eq!(email["protocol"], "mailto:");
    assert_eq!(email["name_translation_key"], "Email address");
    assert_eq!(email["can_be_deleted"], false);
    let facebook = item(&list, "name_translation_key", "Facebook").clone();
    assert_eq!(
        (facebook["type"].as_str(), facebook["can_be_deleted"].as_bool()),
        (Some("X-SOCIAL-PROFILE"), Some(true))
    );
    assert!(facebook["protocol"].is_null());
    w.app.post(path, json!({"name": "Signal", "type": "IMPP"})).await.assert(403);
    w.app.patch(&format!("{path}/{}", facebook["id"]), json!({"name": "FB"})).await.assert(403);
    w.app.delete(&format!("{path}/{}", facebook["id"])).await.assert(403);

    w.as_admin().await;
    let res = w.app.post(path, json!({"protocol": "x".repeat(256)})).await.assert(422).json();
    let fields: Vec<&str> = res["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap()).collect();
    assert_eq!(fields, ["name", "type", "protocol"]);
    let signal =
        w.app.post(path, json!({"name": "Signal", "type": "IMPP", "protocol": "sgnl:"})).await.assert(201).json();
    assert_eq!(
        (signal["name"].as_str(), signal["type"].as_str(), signal["protocol"].as_str()),
        (Some("Signal"), Some("IMPP"), Some("sgnl:"))
    );
    assert_eq!(signal["can_be_deleted"], true);
    assert!(signal["name_translation_key"].is_null());
    let signal_path = format!("{path}/{}", signal["id"]);

    // Absent fields keep their value; a null protocol clears it.
    let updated = w.app.patch(&signal_path, json!({"protocol": "signal:"})).await.assert(200).json();
    assert_eq!((updated["name"].as_str(), updated["type"].as_str()), (Some("Signal"), Some("IMPP")));
    assert_eq!(updated["protocol"], "signal:");
    let updated =
        w.app.patch(&signal_path, json!({"type": "X-SOCIAL-PROFILE", "protocol": null})).await.assert(200).json();
    assert_eq!((updated["name"].as_str(), updated["type"].as_str()), (Some("Signal"), Some("X-SOCIAL-PROFILE")));
    assert!(updated["protocol"].is_null());
    assert_eq!(first_error(w.app.patch(&signal_path, json!({})).await), ("name".into(), "required".into()));
    assert_eq!(first_error(w.app.patch(&signal_path, json!({"type": " "})).await), ("type".into(), "required".into()));
    // A seeded type has no name of its own: renaming needs one.
    let facebook_path = format!("{path}/{}", facebook["id"]);
    assert_eq!(
        first_error(w.app.patch(&facebook_path, json!({"protocol": "fb:"})).await),
        ("name".into(), "required".into())
    );
    let renamed = w.app.patch(&facebook_path, json!({"name": "FB"})).await.assert(200).json();
    assert_eq!((renamed["name"].as_str(), renamed["type"].as_str()), (Some("FB"), Some("X-SOCIAL-PROFILE")));
    w.app.patch(&format!("{path}/999999"), json!({"name": "Nope"})).await.assert(404);

    // F18: email and phone can't be deleted.
    assert_eq!(w.app.delete(&format!("{path}/{}", email["id"])).await.assert(409).code(), "conflict");
    w.app.delete(&format!("{path}/999999")).await.assert(404);

    w.as_stranger().await;
    w.app.patch(&signal_path, json!({"name": "Mine"})).await.assert(404);
    w.app.delete(&signal_path).await.assert(404);

    w.as_admin().await;
    w.app.delete(&signal_path).await.assert(204);
    assert!(!ids(&w.app.get(path).await.json()).contains(&signal["id"].as_i64().unwrap()));
    w.app.finish().await;
}

#[tokio::test]
async fn manages_call_reasons() {
    let mut w = world().await;
    let path = "/account/call-reason-types";
    w.as_member().await;
    let list = w.app.get(path).await.assert(200).json();
    assert!(sorted(&ids(&list)));
    let personal = item(&list, "label_translation_key", "Personal").clone();
    assert!(personal["label"].is_null());
    let reasons = personal["reasons"].as_array().unwrap();
    assert_eq!(reasons.len(), 5);
    assert_eq!(reasons[0]["label_translation_key"], "For advice");
    assert_eq!(reasons[0]["call_reason_type_id"], personal["id"]);
    let reason_ids: Vec<i64> = reasons.iter().map(|r| r["id"].as_i64().unwrap()).collect();
    assert!(sorted(&reason_ids));
    let business = item(&list, "label_translation_key", "Business").clone();
    let personal_path = format!("{path}/{}", personal["id"]);
    w.app.post(path, json!({"label": "Family"})).await.assert(403);
    w.app.patch(&personal_path, json!({"label": "Mine"})).await.assert(403);
    w.app.delete(&personal_path).await.assert(403);
    w.app.post(&format!("{personal_path}/reasons"), json!({"label": "Mine"})).await.assert(403);
    w.app.patch(&format!("{personal_path}/reasons/{}", reason_ids[0]), json!({"label": "Mine"})).await.assert(403);
    w.app.delete(&format!("{personal_path}/reasons/{}", reason_ids[0])).await.assert(403);

    w.as_admin().await;
    assert_eq!(first_error(w.app.post(path, json!({"label": ""})).await), ("label".into(), "required".into()));
    let family = w.app.post(path, json!({"label": "Family"})).await.assert(201).json();
    assert_eq!(family["label"], "Family");
    assert!(family["label_translation_key"].is_null());
    assert_eq!(family["reasons"], json!([]));
    let family_path = format!("{path}/{}", family["id"]);

    let renamed = w.app.patch(&personal_path, json!({"label": "Private"})).await.assert(200).json();
    assert_eq!(renamed["label"], "Private");
    assert_eq!(renamed["label_translation_key"], "Personal");
    assert_eq!(renamed["reasons"].as_array().unwrap().len(), 5, "with its reasons");
    assert_eq!(
        first_error(w.app.patch(&personal_path, json!({"label": "x".repeat(256)})).await),
        ("label".into(), "max".into())
    );
    w.app.patch(&format!("{path}/999999"), json!({"label": "Nope"})).await.assert(404);

    // Reasons.
    let reasons_path = format!("{family_path}/reasons");
    assert_eq!(first_error(w.app.post(&reasons_path, json!({})).await), ("label".into(), "required".into()));
    let reason = w.app.post(&reasons_path, json!({"label": "Birthday"})).await.assert(201).json();
    assert_eq!(reason["label"], "Birthday");
    assert_eq!(reason["call_reason_type_id"], family["id"]);
    assert!(reason["label_translation_key"].is_null());
    w.app.post(&format!("{path}/999999/reasons"), json!({"label": "Nope"})).await.assert(404);
    let reason_path = format!("{reasons_path}/{}", reason["id"]);
    let renamed = w.app.patch(&reason_path, json!({"label": "Anniversary"})).await.assert(200).json();
    assert_eq!(renamed["label"], "Anniversary");
    assert_eq!(
        first_error(w.app.patch(&reason_path, json!({"label": " "})).await),
        ("label".into(), "required".into())
    );
    // A reason under another type of the account is not found.
    let elsewhere = format!("{path}/{}/reasons/{}", business["id"], reason["id"]);
    w.app.patch(&elsewhere, json!({"label": "Nope"})).await.assert(404);
    w.app.delete(&elsewhere).await.assert(404);
    let listed = w.app.get(path).await.json();
    assert_eq!(item(&listed, "label", "Family")["reasons"][0]["label"], "Anniversary");

    w.as_stranger().await;
    assert!(data(&w.app.get(path).await.json()).is_empty());
    w.app.patch(&family_path, json!({"label": "Mine"})).await.assert(404);
    w.app.delete(&family_path).await.assert(404);
    w.app.post(&reasons_path, json!({"label": "Mine"})).await.assert(404);
    w.app.patch(&reason_path, json!({"label": "Mine"})).await.assert(404);
    w.app.delete(&reason_path).await.assert(404);

    w.as_admin().await;
    w.app.delete(&reason_path).await.assert(204);
    w.app.delete(&reason_path).await.assert(404);
    w.app.delete(&format!("{personal_path}/reasons/{}", reason_ids[0])).await.assert(204);
    let listed = w.app.get(path).await.json();
    assert_eq!(item(&listed, "label", "Private")["reasons"].as_array().unwrap().len(), 4);
    w.app.delete(&family_path).await.assert(204);
    w.app.delete(&family_path).await.assert(404);
    assert_eq!(data(&w.app.get(path).await.json()).len(), 2);
    w.app.finish().await;
}

/// Gift occasions, gift states and religions: rows kept in `position` order.
async fn manages_ordered_rows(path: &str, field: &str, key_field: &str, seeded: &[&str]) {
    let mut w = world().await;
    w.as_member().await;
    let list = w.app.get(path).await.assert(200).json();
    let keys: Vec<&str> = data(&list).iter().map(|i| i[key_field].as_str().unwrap()).collect();
    assert_eq!(keys, seeded);
    let positions: Vec<i64> = data(&list).iter().map(|i| i["position"].as_i64().unwrap()).collect();
    assert_eq!(positions, (1..=seeded.len() as i64).collect::<Vec<_>>());
    let first_id = data(&list)[0]["id"].as_i64().unwrap();
    w.app.post(path, json!({field: "Mine"})).await.assert(403);
    w.app.patch(&format!("{path}/{first_id}"), json!({"position": 2})).await.assert(403);
    w.app.delete(&format!("{path}/{first_id}")).await.assert(403);

    w.as_admin().await;
    assert_eq!(first_error(w.app.post(path, json!({field: "  "})).await), (field.into(), "required".into()));
    assert_eq!(first_error(w.app.post(path, json!({field: "x".repeat(256)})).await), (field.into(), "max".into()));
    let created = w.app.post(path, json!({field: " New "})).await.assert(201).json();
    assert_eq!(created[field], "New");
    assert!(created[key_field].is_null());
    let last = seeded.len() as i64 + 1;
    assert_eq!(created["position"], last, "goes last");
    let id = created["id"].as_i64().unwrap();
    let item_path = format!("{path}/{id}");

    let renamed = w.app.patch(&item_path, json!({field: "Renamed"})).await.assert(200).json();
    assert_eq!((renamed[field].as_str(), renamed["position"].as_i64()), (Some("Renamed"), Some(last)));
    // Moving it first shifts the others down.
    let moved = w.app.patch(&item_path, json!({"position": 1})).await.assert(200).json();
    assert_eq!((moved[field].as_str(), moved["position"].as_i64()), (Some("Renamed"), Some(1)));
    let list = w.app.get(path).await.json();
    assert_eq!(ids(&list)[0], id);
    assert_eq!(item(&list, key_field, seeded[0])["position"], 2);
    // Both at once.
    let both = w.app.patch(&item_path, json!({field: "Both", "position": 3})).await.assert(200).json();
    assert_eq!((both[field].as_str(), both["position"].as_i64()), (Some("Both"), Some(3)));
    let list = w.app.get(path).await.json();
    assert_eq!(ids(&list)[2], id);
    assert_eq!(item(&list, key_field, seeded[0])["position"], 1);
    let positions: Vec<i64> = data(&list).iter().map(|i| i["position"].as_i64().unwrap()).collect();
    assert_eq!(positions, (1..=last).collect::<Vec<_>>());

    assert_eq!(first_error(w.app.patch(&item_path, json!({})).await), (field.into(), "required".into()));
    assert_eq!(first_error(w.app.patch(&item_path, json!({"position": 0})).await), ("position".into(), "min".into()));
    assert_eq!(
        first_error(w.app.patch(&item_path, json!({"position": "x"})).await),
        ("position".into(), "type".into())
    );
    assert_eq!(
        first_error(w.app.patch(&item_path, json!({field: "", "position": 2})).await),
        (field.into(), "required".into())
    );
    w.app.patch(&format!("{path}/999999"), json!({"position": 1})).await.assert(404);
    w.app.delete(&format!("{path}/999999")).await.assert(404);

    w.as_stranger().await;
    assert!(data(&w.app.get(path).await.json()).is_empty());
    w.app.patch(&item_path, json!({field: "Mine"})).await.assert(404);
    w.app.delete(&item_path).await.assert(404);

    w.as_admin().await;
    w.app.delete(&item_path).await.assert(204);
    let list = w.app.get(path).await.json();
    assert!(!ids(&list).contains(&id));
    let positions: Vec<i64> = data(&list).iter().map(|i| i["position"].as_i64().unwrap()).collect();
    assert_eq!(positions, (1..=seeded.len() as i64).collect::<Vec<_>>(), "the ones after it move up");
    w.app.finish().await;
}

#[tokio::test]
async fn manages_gift_occasions() {
    manages_ordered_rows(
        "/account/gift-occasions",
        "label",
        "label_translation_key",
        &["Birthday", "Anniversary", "Christmas", "Just because", "Wedding"],
    )
    .await;
}

#[tokio::test]
async fn manages_gift_states() {
    manages_ordered_rows(
        "/account/gift-states",
        "label",
        "label_translation_key",
        &["Idea", "Searched", "Found", "Bought", "Offered"],
    )
    .await;
}

#[tokio::test]
async fn manages_religions() {
    manages_ordered_rows(
        "/account/religions",
        "name",
        "translation_key",
        &["Christian", "Muslim", "Hinduist", "Buddhist", "Shintoist", "Taoist", "Sikh", "Jew", "Atheist"],
    )
    .await;
}

#[tokio::test]
async fn manages_relationship_types() {
    let mut w = world().await;
    let path = "/account/relationship-group-types";
    w.as_member().await;
    let list = w.app.get(path).await.assert(200).json();
    assert!(sorted(&ids(&list)));
    let love = item(&list, "name_translation_key", "Love").clone();
    assert_eq!((love["type"].as_str(), love["can_be_deleted"].as_bool()), (Some("love"), Some(false)));
    let spouse = love["types"].as_array().unwrap().iter().find(|t| t["name_translation_key"] == "spouse").unwrap();
    assert_eq!(spouse["type"], "family");
    assert_eq!(spouse["can_be_deleted"], false);
    assert_eq!(spouse["relationship_group_type_id"], love["id"]);
    assert_eq!(spouse["name_reverse_relationship_translation_key"], "spouse");
    assert!(spouse["name"].is_null() && spouse["name_reverse_relationship"].is_null());
    let family = item(&list, "name_translation_key", "Family").clone();
    let parent = family["types"][0].clone();
    assert_eq!((parent["name_translation_key"].as_str(), parent["type"].as_str()), (Some("parent"), Some("child")));
    let friend = item(&list, "name_translation_key", "Friend").clone();
    assert!(friend["type"].is_null());
    let type_ids: Vec<i64> = friend["types"].as_array().unwrap().iter().map(|t| t["id"].as_i64().unwrap()).collect();
    assert!(sorted(&type_ids));
    let friend_path = format!("{path}/{}", friend["id"]);
    w.app.post(path, json!({"name": "Mine"})).await.assert(403);
    w.app.patch(&friend_path, json!({"name": "Mine"})).await.assert(403);
    w.app.delete(&friend_path).await.assert(403);
    let types_path = format!("{friend_path}/relationship-types");
    w.app.post(&types_path, json!({"name": "a", "name_reverse_relationship": "b"})).await.assert(403);
    w.app.patch(&format!("{types_path}/{}", type_ids[0]), json!({"name": "a"})).await.assert(403);
    w.app.delete(&format!("{types_path}/{}", type_ids[0])).await.assert(403);

    w.as_admin().await;
    assert_eq!(first_error(w.app.post(path, json!({"name": ""})).await), ("name".into(), "required".into()));
    let club = w.app.post(path, json!({"name": "Club"})).await.assert(201).json();
    assert_eq!(club["name"], "Club");
    assert_eq!((club["can_be_deleted"].as_bool(), club["types"].clone()), (Some(true), json!([])));
    assert!(club["type"].is_null() && club["name_translation_key"].is_null());
    let club_path = format!("{path}/{}", club["id"]);
    let renamed = w.app.patch(&friend_path, json!({"name": "Friends"})).await.assert(200).json();
    assert_eq!(renamed["name"], "Friends");
    assert_eq!(renamed["types"].as_array().unwrap().len(), 2, "with its types");
    assert_eq!(first_error(w.app.patch(&friend_path, json!({})).await), ("name".into(), "required".into()));
    w.app.patch(&format!("{path}/999999"), json!({"name": "Nope"})).await.assert(404);
    assert_eq!(w.app.delete(&format!("{path}/{}", love["id"])).await.assert(409).code(), "conflict");
    w.app.delete(&format!("{path}/999999")).await.assert(404);

    // Relationship types.
    let club_types = format!("{club_path}/relationship-types");
    let res = w.app.post(&club_types, json!({"name": " "})).await.assert(422).json();
    let fields: Vec<&str> = res["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap()).collect();
    assert_eq!(fields, ["name", "name_reverse_relationship"]);
    let member_of = w
        .app
        .post(&club_types, json!({"name": "member", "name_reverse_relationship": "club"}))
        .await
        .assert(201)
        .json();
    assert_eq!(
        (member_of["name"].as_str(), member_of["name_reverse_relationship"].as_str()),
        (Some("member"), Some("club"))
    );
    assert_eq!(
        (member_of["can_be_deleted"].as_bool(), member_of["relationship_group_type_id"].clone()),
        (Some(true), club["id"].clone())
    );
    assert!(member_of["type"].is_null() && member_of["name_translation_key"].is_null());
    w.app
        .post(&format!("{path}/999999/relationship-types"), json!({"name": "a", "name_reverse_relationship": "b"}))
        .await
        .assert(404);
    let member_path = format!("{club_types}/{}", member_of["id"]);
    let updated = w.app.patch(&member_path, json!({"name_reverse_relationship": "the club"})).await.assert(200).json();
    assert_eq!(
        (updated["name"].as_str(), updated["name_reverse_relationship"].as_str()),
        (Some("member"), Some("the club"))
    );
    assert_eq!(first_error(w.app.patch(&member_path, json!({})).await), ("name".into(), "required".into()));
    // A seeded type has no reverse name of its own.
    let seeded_path = format!("{types_path}/{}", type_ids[0]);
    assert_eq!(
        first_error(w.app.patch(&seeded_path, json!({"name": "pal"})).await),
        ("name_reverse_relationship".into(), "required".into())
    );
    let pal =
        w.app.patch(&seeded_path, json!({"name": "pal", "name_reverse_relationship": "pal"})).await.assert(200).json();
    assert_eq!((pal["name"].as_str(), pal["name_translation_key"].as_str()), (Some("pal"), Some("friend")));
    // The type must belong to the group in the path.
    w.app.patch(&format!("{types_path}/{}", member_of["id"]), json!({"name": "x"})).await.assert(404);
    w.app.delete(&format!("{types_path}/{}", member_of["id"])).await.assert(404);
    let family_types = format!("{path}/{}/relationship-types", family["id"]);
    assert_eq!(w.app.delete(&format!("{family_types}/{}", parent["id"])).await.assert(409).code(), "conflict");

    w.as_stranger().await;
    assert!(data(&w.app.get(path).await.json()).is_empty());
    w.app.patch(&club_path, json!({"name": "Mine"})).await.assert(404);
    w.app.delete(&club_path).await.assert(404);
    w.app.post(&club_types, json!({"name": "a", "name_reverse_relationship": "b"})).await.assert(404);
    w.app.patch(&member_path, json!({"name": "Mine"})).await.assert(404);
    w.app.delete(&member_path).await.assert(404);

    w.as_admin().await;
    w.app.delete(&member_path).await.assert(204);
    w.app.delete(&member_path).await.assert(404);
    w.app.delete(&club_path).await.assert(204);
    assert!(!ids(&w.app.get(path).await.json()).contains(&club["id"].as_i64().unwrap()));
    w.app.finish().await;
}

#[tokio::test]
async fn manages_account_currencies() {
    let mut w = world().await;
    w.as_member().await;
    let list = w.app.get("/account/currencies").await.assert(200).json();
    assert_eq!(data(&list).len(), 164);
    let codes: Vec<&str> = data(&list).iter().map(|c| c["code"].as_str().unwrap()).collect();
    let mut by_code = codes.clone();
    by_code.sort();
    assert_eq!(codes, by_code, "ordered by code");
    let usd = item(&list, "code", "USD").clone();
    assert_eq!((usd["name"].as_str(), usd["active"].as_bool()), (Some("US Dollar"), Some(true)));
    assert_eq!(item(&list, "code", "ZM")["name"], "currencies.ZM", "Laravel shows the key of a missing name");
    let usd_path = format!("/account/currencies/{}", usd["id"]);
    w.app.patch(&usd_path, json!({"active": false})).await.assert(403);
    w.app.post("/account/currencies/disable-all", json!({})).await.assert(403);
    w.app.post("/account/currencies/enable-all", json!({})).await.assert(403);

    w.as_admin().await;
    w.app.post("/account/currencies/disable-all", json!({})).await.assert(204);
    assert!(data(&w.app.get("/account/currencies?active=true").await.assert(200).json()).is_empty());
    assert_eq!(data(&w.app.get("/account/currencies?active=false").await.json()).len(), 164);

    let on = w.app.patch(&usd_path, json!({"active": true})).await.assert(200).json();
    assert_eq!(
        (on["code"].as_str(), on["name"].as_str(), on["active"].as_bool()),
        (Some("USD"), Some("US Dollar"), Some(true))
    );
    assert_eq!(on["id"], usd["id"]);
    // Sending the current state changes nothing (no toggle).
    assert_eq!(w.app.patch(&usd_path, json!({"active": true})).await.assert(200).json()["active"], true);
    let active = w.app.get("/account/currencies?active=true").await.json();
    assert_eq!(ids(&active), [usd["id"].as_i64().unwrap()]);
    assert_eq!(first_error(w.app.patch(&usd_path, json!({})).await), ("active".into(), "required".into()));
    assert_eq!(first_error(w.app.patch(&usd_path, json!({"active": "yes"})).await), ("active".into(), "type".into()));
    w.app.patch("/account/currencies/999999", json!({"active": true})).await.assert(404);

    w.app.post("/account/currencies/enable-all", json!({})).await.assert(204);
    assert_eq!(data(&w.app.get("/account/currencies?active=true").await.json()).len(), 164);
    let off = w.app.patch(&usd_path, json!({"active": false})).await.assert(200).json();
    assert_eq!(off["active"], false);
    assert_eq!(data(&w.app.get("/account/currencies?active=true").await.json()).len(), 163);

    // Another account has no currencies attached: none listed, none to change.
    w.as_stranger().await;
    assert!(data(&w.app.get("/account/currencies").await.json()).is_empty());
    w.app.patch(&usd_path, json!({"active": true})).await.assert(404);
    w.app.post("/account/currencies/enable-all", json!({})).await.assert(204);
    w.as_member().await;
    assert_eq!(data(&w.app.get("/account/currencies?active=true").await.json()).len(), 163, "untouched");
    w.app.finish().await;
}
