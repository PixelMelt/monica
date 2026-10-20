//! Contact modules, set A, over HTTP: important dates, goals and streaks,
//! labels, reminders, addresses, contact information, loans, job
//! information and religion, and their contact page module data.

mod common;

use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Utc};
use common::TestApp;
use monica::api::contact_modules_a as modules;
use monica::api::contact_page::ModuleContext;
use monica::db::id::Id;
use monica::db::timestamp::Timestamp;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::entities::{
    address_types, addresses, companies, contact_address, contact_information_types, contact_label,
    contact_reminder_scheduled, contacts, currencies, goals, labels, religions, streaks, user_notification_channels,
    users, vaults,
};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::{Value, json};

/// A logged-in vault manager with a contact in a vault created through the API.
struct World {
    app: TestApp,
    user: users::Model,
    vault: vaults::Model,
    contact: contacts::Model,
}

impl World {
    async fn new() -> World {
        let mut app = TestApp::new().await;
        let user = factory::user(&app.state.db, None).await;
        app.login(&user.username).await;
        let v = app.post("/vaults", json!({"name": "Friends"})).await.assert(201).json();
        let vault_id: Uuid = v["id"].as_str().unwrap().parse().unwrap();
        let vault = vaults::Entity::find_by_id(vault_id).one(&app.state.db).await.unwrap().unwrap();
        let contact = factory::contact(&app.state.db, &vault).await;
        World { app, user, vault, contact }
    }

    /// `/vaults/{vault}/contacts/{contact}{rest}`.
    fn path(&self, rest: &str) -> String {
        contact_path(&self.vault, &self.contact, rest)
    }

    /// Another contact of the vault.
    async fn other_contact(&self) -> contacts::Model {
        factory::contact(&self.app.state.db, &self.vault).await
    }

    /// The module context of the contact, as it is now.
    async fn ctx(&self) -> ModuleContext {
        let contact = contacts::Entity::find_by_id(self.contact.id).one(&self.app.state.db).await.unwrap().unwrap();
        ModuleContext { user: self.user.clone(), vault: self.vault.clone(), contact }
    }

    /// Logs in a viewer of the vault.
    async fn login_viewer(&mut self) {
        let viewer = factory::user(&self.app.state.db, Some(self.user.account_id)).await;
        factory::set_permission_in_vault(&self.app.state.db, &viewer, VaultPermission::View, &self.vault).await;
        self.app.login(&viewer.username).await;
    }

    /// Logs in a user of another account.
    async fn login_stranger(&mut self) {
        let stranger = factory::user(&self.app.state.db, None).await;
        self.app.login(&stranger.username).await;
    }

    /// A vault of another account, with a contact.
    async fn foreign_vault(&self) -> (vaults::Model, contacts::Model) {
        let account = factory::account(&self.app.state.db).await;
        let vault = factory::vault(&self.app.state.db, account.id).await;
        let contact = factory::contact(&self.app.state.db, &vault).await;
        (vault, contact)
    }

    async fn finish(self) {
        self.app.finish().await;
    }
}

fn contact_path(vault: &vaults::Model, contact: &contacts::Model, rest: &str) -> String {
    format!("/vaults/{}/contacts/{}{rest}", vault.id, contact.id)
}

fn today() -> NaiveDate {
    Utc::now().date_naive()
}

fn ymd(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn ids(list: &Value) -> Vec<i64> {
    list.as_array().unwrap().iter().map(|x| x["id"].as_i64().unwrap()).collect()
}

/// The `field` of each error of a 422.
fn error_fields(res: &common::TestResponse) -> Vec<String> {
    assert_eq!(res.status.as_u16(), 422, "body: {}", String::from_utf8_lossy(&res.body));
    res.json()["errors"].as_array().unwrap().iter().map(|e| e["field"].as_str().unwrap().to_owned()).collect()
}

fn error_codes(res: &common::TestResponse) -> Vec<String> {
    res.json()["errors"].as_array().unwrap().iter().map(|e| e["code"].as_str().unwrap().to_owned()).collect()
}

/// Whole years from `born` to today.
fn years_since(born: NaiveDate) -> i64 {
    let t = today();
    let mut years = i64::from(t.year() - born.year());
    if (t.month(), t.day()) < (born.month(), born.day()) {
        years -= 1;
    }
    years
}

// ---------------------------------------------------------------- important dates

#[tokio::test]
async fn manages_important_dates() {
    let mut w = World::new().await;
    let db = w.app.state.db.clone();
    let base = w.path("/important-dates");
    let birthdate = monica::entities::contact_important_date_types::Entity::find()
        .filter(monica::entities::contact_important_date_types::Column::VaultId.eq(w.vault.id))
        .filter(monica::entities::contact_important_date_types::Column::InternalType.eq("birthdate"))
        .one(&db)
        .await
        .unwrap()
        .expect("createVault seeds the birthdate type");

    let full = w
        .app
        .post(
            &base,
            json!({"label": "Birthday", "contact_important_date_type_id": birthdate.id, "day": 15, "month": 3, "year": 1990}),
        )
        .await
        .assert(201)
        .json();
    assert_eq!(full["label"], "Birthday");
    assert_eq!(full["contact_id"], w.contact.id.to_string());
    assert_eq!((full["day"].as_i64(), full["month"].as_i64(), full["year"].as_i64()), (Some(15), Some(3), Some(1990)));
    assert_eq!(full["age"], years_since(NaiveDate::from_ymd_opt(1990, 3, 15).unwrap()));
    assert_eq!(full["contact_important_date_type"]["id"], birthdate.id.0);
    assert_eq!(full["contact_important_date_type"]["internal_type"], "birthdate");
    assert!(full["created_at"].as_str().unwrap().ends_with('Z'));

    let month_day = w
        .app
        .post(
            &base,
            json!({"label": "Anniversary", "contact_important_date_type_id": null, "day": 1, "month": 2, "year": null}),
        )
        .await
        .assert(201)
        .json();
    assert!(month_day["age"].is_null());
    assert!(month_day["contact_important_date_type"].is_null());
    assert!(month_day["year"].is_null());

    let year_only = w
        .app
        .post(&base, json!({"label": "Graduation", "day": null, "month": null, "year": 2000}))
        .await
        .assert(201)
        .json();
    assert_eq!(year_only["age"], i64::from(today().year() - 2000));

    // Validation.
    let res = w.app.post(&base, json!({"label": " ", "day": 1, "month": 2, "year": null})).await;
    assert_eq!(error_fields(&res), ["label"]);
    let res = w.app.post(&base, json!({"label": "x", "day": 1, "month": 2})).await;
    assert_eq!(error_fields(&res), ["year"]);
    assert_eq!(error_codes(&res), ["required"]);
    let res = w.app.post(&base, json!({"label": "x", "day": 31, "month": 2, "year": 2021})).await;
    assert_eq!(error_fields(&res), ["day"]);
    assert_eq!(error_codes(&res), ["date"]);
    let res = w.app.post(&base, json!({"label": "x", "day": 5, "month": null, "year": null})).await;
    assert_eq!(error_fields(&res), ["month"]);
    let res = w.app.post(&base, json!({"label": "x", "day": null, "month": null, "year": null})).await;
    assert_eq!(error_fields(&res), ["year"]);
    let res = w.app.post(&base, json!({"label": "x", "day": 32, "month": 13, "year": null})).await;
    assert_eq!(error_fields(&res), ["day", "month"]);
    let res = w.app.post(&base, json!({"label": &"a".repeat(256), "day": 1, "month": 1, "year": null})).await;
    assert_eq!(error_codes(&res), ["max"]);
    let (foreign, _) = w.foreign_vault().await;
    let foreign_type = monica::entities::contact_important_date_types::ActiveModel {
        vault_id: Set(foreign.id),
        label: Set("Other".into()),
        can_be_deleted: Set(true),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let res = w
        .app
        .post(&base, json!({"label": "x", "contact_important_date_type_id": foreign_type.id, "day": 1, "month": 1, "year": null}))
        .await;
    assert_eq!(error_fields(&res), ["contact_important_date_type_id"]);
    assert_eq!(error_codes(&res), ["exists"]);

    let list = w.app.get(&base).await.assert(200).json();
    let (a, b, c) =
        (full["id"].as_i64().unwrap(), month_day["id"].as_i64().unwrap(), year_only["id"].as_i64().unwrap());
    assert_eq!(ids(&list["data"]), [a, b, c]);
    assert_eq!(list["data"][0]["contact_important_date_type"]["label"], birthdate.label);

    // Update: absent fields are kept; date parts go together.
    let up = w.app.patch(&format!("{base}/{a}"), json!({"label": "Bday"})).await.assert(200).json();
    assert_eq!(up["label"], "Bday");
    assert_eq!(up["day"], 15);
    assert_eq!(up["contact_important_date_type"]["id"], birthdate.id.0);
    let up = w.app.patch(&format!("{base}/{a}"), json!({"day": 16, "month": 4, "year": 1991})).await.assert(200).json();
    assert_eq!((up["day"].as_i64(), up["month"].as_i64(), up["year"].as_i64()), (Some(16), Some(4), Some(1991)));
    assert_eq!(up["age"], years_since(NaiveDate::from_ymd_opt(1991, 4, 16).unwrap()));
    assert_eq!(up["label"], "Bday");
    let res = w.app.patch(&format!("{base}/{a}"), json!({"day": 1})).await;
    assert_eq!(error_fields(&res), ["month", "year"]);
    assert_eq!(error_codes(&res), ["required_with", "required_with"]);
    let res = w.app.patch(&format!("{base}/{a}"), json!({"label": ""})).await;
    assert_eq!(error_fields(&res), ["label"]);
    let up =
        w.app.patch(&format!("{base}/{a}"), json!({"contact_important_date_type_id": null})).await.assert(200).json();
    assert!(up["contact_important_date_type"].is_null());
    let up =
        w.app.patch(&format!("{base}/{b}"), json!({"day": null, "month": null, "year": 1980})).await.assert(200).json();
    assert!(up["day"].is_null());
    assert_eq!(up["age"], i64::from(today().year() - 1980));

    // Another contact's date isn't reachable through this contact.
    let other = w.other_contact().await;
    let others = w
        .app
        .post(
            &contact_path(&w.vault, &other, "/important-dates"),
            json!({"label": "x", "day": 1, "month": 1, "year": null}),
        )
        .await
        .assert(201)
        .json();
    let other_id = others["id"].as_i64().unwrap();
    w.app.patch(&format!("{base}/{other_id}"), json!({"label": "y"})).await.assert(404);
    w.app.delete(&format!("{base}/{other_id}")).await.assert(404);
    w.app.patch(&format!("{base}/999999"), json!({"label": "y"})).await.assert(404);
    w.app.get(&format!("/vaults/{}/contacts/{}/important-dates", w.vault.id, Uuid::new())).await.assert(404);

    let module =
        serde_json::to_value(modules::important_dates_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(ids(&module["important_dates"]), [a, b, c]);

    w.app.delete(&format!("{base}/{c}")).await.assert(204);
    w.app.delete(&format!("{base}/{c}")).await.assert(404);
    w.app.patch(&format!("{base}/{c}"), json!({"label": "z"})).await.assert(404);
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [a, b]);

    w.login_viewer().await;
    w.app.get(&base).await.assert(200);
    w.app.post(&base, json!({"label": "x", "day": 1, "month": 1, "year": null})).await.assert(403);
    w.app.patch(&format!("{base}/{a}"), json!({"label": "y"})).await.assert(403);
    w.app.delete(&format!("{base}/{a}")).await.assert(403);

    w.login_stranger().await;
    w.app.get(&base).await.assert(404);
    w.app.post(&base, json!({"label": "x", "day": 1, "month": 1, "year": null})).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- goals

#[tokio::test]
async fn manages_goals_and_streaks() {
    let mut w = World::new().await;
    let db = w.app.state.db.clone();
    let base = w.path("/goals");
    let today = today();

    let goal = w.app.post(&base, json!({"name": "Run"})).await.assert(201).json();
    assert_eq!(goal["name"], "Run");
    assert_eq!(goal["active"], true);
    assert_eq!(goal["contact_id"], w.contact.id.to_string());
    // Q6: no streaks is 0, not 1.
    assert_eq!((goal["max_streak"].as_i64(), goal["current_streak"].as_i64()), (Some(0), Some(0)));
    let days = goal["last_7_days"].as_array().unwrap();
    assert_eq!(days.len(), 7);
    assert_eq!(days[0]["date"], ymd(today - Duration::days(6)));
    assert_eq!(days[6]["date"], ymd(today));
    assert!(days.iter().all(|d| d["has_streak"] == false));
    let id = goal["id"].as_i64().unwrap();
    let goal_path = format!("{base}/{id}");

    let res = w.app.post(&base, json!({"name": "  "})).await;
    assert_eq!(error_fields(&res), ["name"]);
    let res = w.app.post(&base, json!({"name": "a".repeat(256)})).await;
    assert_eq!(error_codes(&res), ["max"]);

    // Streaks: PUT and DELETE are idempotent; a run of n days is n (Q6).
    let streak = |d: NaiveDate| format!("{goal_path}/streaks/{}", ymd(d));
    let g = w.app.put(&streak(today), json!({})).await.assert(200).json();
    assert_eq!((g["max_streak"].as_i64(), g["current_streak"].as_i64()), (Some(1), Some(1)));
    assert_eq!(g["last_7_days"][6]["has_streak"], true);
    let g = w.app.put(&streak(today), json!({})).await.assert(200).json();
    assert_eq!(g["current_streak"], 1);
    let count =
        || async { streaks::Entity::find().filter(streaks::Column::GoalId.eq(Id(id))).count(&db).await.unwrap() };
    assert_eq!(count().await, 1);
    let g = w.app.put(&streak(today - Duration::days(1)), json!({})).await.assert(200).json();
    assert_eq!((g["max_streak"].as_i64(), g["current_streak"].as_i64()), (Some(2), Some(2)));
    let g = w.app.put(&streak(today - Duration::days(5)), json!({})).await.assert(200).json();
    assert_eq!((g["max_streak"].as_i64(), g["current_streak"].as_i64()), (Some(2), Some(2)));
    assert_eq!(g["last_7_days"][1]["has_streak"], true);
    let g = w.app.delete(&streak(today)).await.assert(200).json();
    assert_eq!((g["max_streak"].as_i64(), g["current_streak"].as_i64()), (Some(1), Some(0)));
    let g = w.app.delete(&streak(today)).await.assert(200).json();
    assert_eq!(g["current_streak"], 0);
    assert_eq!(count().await, 2);
    w.app.put(&format!("{goal_path}/streaks/2025-02-30"), json!({})).await.assert(400);

    // The goal page: streaks of the calendar (first of the month a year ago to today).
    streaks::ActiveModel {
        goal_id: Set(Id(id)),
        happened_at: Set(Timestamp((today - Duration::days(800)).and_time(NaiveTime::MIN))),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let detail = w.app.get(&goal_path).await.assert(200).json();
    let from = NaiveDate::from_ymd_opt(today.year() - 1, today.month(), 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(today.year() - 1, 3, 1).unwrap());
    assert_eq!(detail["streaks_from"], ymd(from));
    assert_eq!(detail["streaks_to"], ymd(today));
    let happened: Vec<&str> =
        detail["streaks"].as_array().unwrap().iter().map(|s| s["happened_at"].as_str().unwrap()).collect();
    assert_eq!(happened, [ymd(today - Duration::days(5)), ymd(today - Duration::days(1))]);
    assert_eq!(detail["streaks"][0]["goal_id"], id);
    assert_eq!(detail["max_streak"], 1);
    assert_eq!(detail["last_7_days"].as_array().unwrap().len(), 7);

    let g = w.app.patch(&goal_path, json!({"name": "Walk"})).await.assert(200).json();
    assert_eq!(g["name"], "Walk");
    assert_eq!(w.app.patch(&goal_path, json!({})).await.assert(200).json()["name"], "Walk");
    let res = w.app.patch(&goal_path, json!({"name": ""})).await;
    assert_eq!(error_fields(&res), ["name"]);

    // Filters and the module: a second, inactive goal (no operation deactivates one).
    let second = w.app.post(&base, json!({"name": "Read"})).await.assert(201).json();
    let second_id = second["id"].as_i64().unwrap();
    let mut inactive: goals::ActiveModel =
        goals::Entity::find_by_id(Id(second_id)).one(&db).await.unwrap().unwrap().into();
    inactive.active = Set(false);
    inactive.update(&db).await.unwrap();
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [id, second_id]);
    assert_eq!(ids(&w.app.get(&format!("{base}?active=true")).await.assert(200).json()["data"]), [id]);
    assert_eq!(ids(&w.app.get(&format!("{base}?active=false")).await.assert(200).json()["data"]), [second_id]);
    let module = serde_json::to_value(modules::goals_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(ids(&module["goals"]), [id]);
    assert_eq!(module["inactive_goals_count"], 1);
    assert_eq!(module["goals"][0]["max_streak"], 1);

    // Another contact's goal, a missing goal.
    let other = w.other_contact().await;
    let others = w.app.post(&contact_path(&w.vault, &other, "/goals"), json!({"name": "x"})).await.assert(201).json();
    let other_goal = format!("{base}/{}", others["id"]);
    w.app.get(&other_goal).await.assert(404);
    w.app.put(&format!("{other_goal}/streaks/{}", ymd(today)), json!({})).await.assert(404);
    w.app.delete(&format!("{other_goal}/streaks/{}", ymd(today))).await.assert(404);
    w.app.patch(&other_goal, json!({"name": "y"})).await.assert(404);
    w.app.delete(&other_goal).await.assert(404);
    w.app.get(&format!("{base}/999999")).await.assert(404);

    w.login_viewer().await;
    w.app.get(&goal_path).await.assert(200);
    w.app.get(&base).await.assert(200);
    w.app.post(&base, json!({"name": "x"})).await.assert(403);
    w.app.patch(&goal_path, json!({"name": "x"})).await.assert(403);
    w.app.put(&streak(today), json!({})).await.assert(403);
    w.app.delete(&streak(today)).await.assert(403);
    w.app.delete(&goal_path).await.assert(403);

    w.login_stranger().await;
    w.app.get(&base).await.assert(404);
    w.app.get(&goal_path).await.assert(404);
    w.app.put(&streak(today), json!({})).await.assert(404);

    w.app.login(&w.user.username.clone()).await;
    w.app.delete(&goal_path).await.assert(204);
    w.app.get(&goal_path).await.assert(404);
    assert_eq!(count().await, 0, "the streaks go with the goal");
    w.finish().await;
}

// ---------------------------------------------------------------- labels

async fn label(db: &sea_orm::DatabaseConnection, vault_id: Uuid, name: &str) -> labels::Model {
    labels::ActiveModel {
        vault_id: Set(vault_id),
        name: Set(name.into()),
        slug: Set(name.to_lowercase()),
        bg_color: Set("bg-neutral-200".into()),
        text_color: Set("text-neutral-800".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

#[tokio::test]
async fn assigns_and_removes_labels() {
    let mut w = World::new().await;
    let db = w.app.state.db.clone();
    let base = w.path("/labels");
    let friends = label(&db, w.vault.id, "Friends").await;
    let work = label(&db, w.vault.id, "Work").await;
    let (foreign, _) = w.foreign_vault().await;
    let foreign_label = label(&db, foreign.id, "Elsewhere").await;

    assert_eq!(w.app.get(&base).await.assert(200).json()["data"], json!([]));
    let l = w.app.put(&format!("{base}/{}", work.id), json!({})).await.assert(200).json();
    assert_eq!(l["id"], work.id.0);
    assert_eq!(l["name"], "Work");
    assert_eq!(l["slug"], "work");
    assert_eq!(l["vault_id"], w.vault.id.to_string());
    assert_eq!(l["bg_color"], "bg-neutral-200");
    assert!(l.get("contacts_count").is_none());
    w.app.put(&format!("{base}/{}", work.id), json!({})).await.assert(200);
    w.app.put(&format!("{base}/{}", friends.id), json!({})).await.assert(200);
    let links =
        contact_label::Entity::find().filter(contact_label::Column::ContactId.eq(w.contact.id)).count(&db).await;
    assert_eq!(links.unwrap(), 2, "assigning twice keeps one link");
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [friends.id.0, work.id.0]);

    w.app.put(&format!("{base}/{}", foreign_label.id), json!({})).await.assert(404);
    w.app.put(&format!("{base}/999999"), json!({})).await.assert(404);
    w.app.delete(&format!("{base}/{}", foreign_label.id)).await.assert(404);

    let module = serde_json::to_value(modules::labels_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(ids(&module["labels"]), [friends.id.0, work.id.0]);

    w.app.delete(&format!("{base}/{}", work.id)).await.assert(204);
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [friends.id.0]);
    assert!(labels::Entity::find_by_id(work.id).one(&db).await.unwrap().is_some(), "the label stays in the vault");

    w.login_viewer().await;
    w.app.get(&base).await.assert(200);
    w.app.put(&format!("{base}/{}", work.id), json!({})).await.assert(403);
    w.app.delete(&format!("{base}/{}", friends.id)).await.assert(403);

    w.login_stranger().await;
    w.app.get(&base).await.assert(404);
    w.app.put(&format!("{base}/{}", work.id), json!({})).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- reminders

/// The next `month`/`day` strictly after today.
fn next_yearly(month: u32, day: u32) -> NaiveDate {
    let t = today();
    let this_year = NaiveDate::from_ymd_opt(t.year(), month, day).unwrap();
    if this_year > t { this_year } else { NaiveDate::from_ymd_opt(t.year() + 1, month, day).unwrap() }
}

#[tokio::test]
async fn manages_reminders() {
    let mut w = World::new().await;
    let db = w.app.state.db.clone();
    let base = w.path("/reminders");
    user_notification_channels::ActiveModel {
        user_id: Set(Some(w.user.id)),
        r#type: Set("email".into()),
        label: Set(Some("Mail".into())),
        content: Set(w.user.username.clone()),
        active: Set(true),
        fails: Set(0),
        verified_at: Set(Some(monica::db::now())),
        preferred_time: Set(NaiveTime::from_hms_opt(9, 0, 0)),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let scheduled = |reminder: i64| {
        let db = db.clone();
        async move {
            contact_reminder_scheduled::Entity::find()
                .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(Id(reminder)))
                .count(&db)
                .await
                .unwrap()
        }
    };

    let next_year = today().year() + 1;
    let xmas = w
        .app
        .post(
            &base,
            json!({"label": "Xmas", "day": 25, "month": 12, "year": next_year, "type": "one_time", "frequency_number": 1}),
        )
        .await
        .assert(201)
        .json();
    assert_eq!(xmas["label"], "Xmas");
    assert_eq!(xmas["type"], "one_time");
    assert_eq!(xmas["frequency_number"], 1);
    assert_eq!(xmas["number_times_triggered"], 0);
    assert!(xmas["last_triggered_at"].is_null());
    assert_eq!(xmas["next_occurrence"], format!("{next_year}-12-25"));
    let xmas_id = xmas["id"].as_i64().unwrap();
    assert_eq!(scheduled(xmas_id).await, 1, "scheduled on the user's channel");

    let new_year = w
        .app
        .post(&base, json!({"label": "New year", "day": 1, "month": 1, "year": null, "type": "recurring_year", "frequency_number": null}))
        .await
        .assert(201)
        .json();
    assert!(new_year["year"].is_null());
    assert!(new_year["frequency_number"].is_null());
    assert_eq!(new_year["next_occurrence"], format!("{next_year}-01-01"));
    let new_year_id = new_year["id"].as_i64().unwrap();

    // Validation.
    let res = w.app.post(&base, json!({"label": "x", "day": 31, "month": 2, "year": 2025, "type": "one_time"})).await;
    assert_eq!(error_fields(&res), ["day"]);
    let res = w.app.post(&base, json!({"label": "x", "day": 1, "month": 2, "year": null, "type": "weekly"})).await;
    assert_eq!(error_fields(&res), ["type"]);
    let res = w.app.post(&base, json!({"label": "", "month": 2, "type": "one_time", "frequency_number": 0})).await;
    assert_eq!(error_fields(&res), ["label", "day", "year", "frequency_number"]);
    let res = w.app.post(&base, json!({"label": "x", "day": 1, "month": 13, "year": null})).await;
    assert_eq!(error_fields(&res), ["month", "type"]);

    // By month, then day.
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [new_year_id, xmas_id]);

    let up = w
        .app
        .patch(&format!("{base}/{xmas_id}"), json!({"label": "Christmas", "year": null, "type": "recurring_year"}))
        .await
        .assert(200)
        .json();
    assert_eq!(up["label"], "Christmas");
    assert!(up["year"].is_null());
    assert_eq!((up["day"].as_i64(), up["month"].as_i64()), (Some(25), Some(12)));
    assert_eq!(up["type"], "recurring_year");
    assert_eq!(up["frequency_number"], 1);
    assert_eq!(up["next_occurrence"], ymd(next_yearly(12, 25)));
    assert_eq!(scheduled(xmas_id).await, 1, "rescheduled, not doubled");
    let up = w.app.patch(&format!("{base}/{xmas_id}"), json!({"frequency_number": null})).await.assert(200).json();
    assert!(up["frequency_number"].is_null());
    let res = w.app.patch(&format!("{base}/{xmas_id}"), json!({"type": "nope"})).await;
    assert_eq!(error_fields(&res), ["type"]);
    let res = w.app.patch(&format!("{base}/{xmas_id}"), json!({"day": 31, "month": 2, "year": 2025})).await;
    assert_eq!(error_fields(&res), ["day"]);
    let res = w.app.patch(&format!("{base}/{xmas_id}"), json!({"label": " "})).await;
    assert_eq!(error_fields(&res), ["label"]);

    let module = serde_json::to_value(modules::reminders_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(ids(&module["reminders"]), [new_year_id, xmas_id]);

    let other = w.other_contact().await;
    let others = w
        .app
        .post(
            &contact_path(&w.vault, &other, "/reminders"),
            json!({"label": "x", "day": 1, "month": 1, "year": null, "type": "one_time"}),
        )
        .await
        .assert(201)
        .json();
    w.app.patch(&format!("{base}/{}", others["id"]), json!({"label": "y"})).await.assert(404);
    w.app.delete(&format!("{base}/{}", others["id"])).await.assert(404);

    w.app.delete(&format!("{base}/{xmas_id}")).await.assert(204);
    assert_eq!(scheduled(xmas_id).await, 0);
    w.app.delete(&format!("{base}/{xmas_id}")).await.assert(404);
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [new_year_id]);

    w.login_viewer().await;
    w.app.get(&base).await.assert(200);
    w.app.post(&base, json!({"label": "x", "day": 1, "month": 1, "year": null, "type": "one_time"})).await.assert(403);
    w.app.patch(&format!("{base}/{new_year_id}"), json!({"label": "y"})).await.assert(403);
    w.app.delete(&format!("{base}/{new_year_id}")).await.assert(403);

    w.login_stranger().await;
    w.app.get(&base).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- addresses

async fn address_type(db: &sea_orm::DatabaseConnection, account_id: Uuid, name: &str) -> address_types::Model {
    address_types::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(name.into())),
        r#type: Set(Some("home".into())),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

#[tokio::test]
async fn manages_addresses() {
    let mut w = World::new().await;
    let db = w.app.state.db.clone();
    let base = w.path("/addresses");
    let home = address_type(&db, w.user.account_id, "Home").await;
    let (foreign, foreign_contact) = w.foreign_vault().await;
    let foreign_type = address_type(&db, foreign.account_id, "Elsewhere").await;
    let other = w.other_contact().await;
    let other_base = contact_path(&w.vault, &other, "/addresses");

    let a = w
        .app
        .post(
            &base,
            json!({"address_type_id": home.id, "line_1": " 1 Main St ", "city": "Paris", "country": "France", "latitude": 48.85, "longitude": 2.35}),
        )
        .await
        .assert(201)
        .json();
    assert_eq!(a["line_1"], "1 Main St");
    assert_eq!(a["city"], "Paris");
    assert!(a["line_2"].is_null());
    assert_eq!(a["is_past_address"], false);
    assert_eq!(a["vault_id"], w.vault.id.to_string());
    assert_eq!(a["address_type"]["id"], home.id.0);
    assert_eq!(a["address_type"]["name"], "Home");
    assert_eq!(a["address_type"]["type"], "home");
    assert_eq!(a["latitude"], 48.85);
    let address_id = a["id"].as_i64().unwrap();

    // Linking an existing vault address.
    let res = w.app.post(&base, json!({"address_id": address_id})).await.assert(409);
    assert_eq!(res.code(), "conflict");
    let linked =
        w.app.post(&other_base, json!({"address_id": address_id, "is_past_address": true})).await.assert(201).json();
    assert_eq!(linked["id"], address_id);
    assert_eq!(linked["is_past_address"], true);
    assert_eq!(linked["city"], "Paris");

    // Validation.
    let res = w.app.post(&other_base, json!({"address_id": address_id, "line_1": "x"})).await;
    assert_eq!(error_fields(&res), ["line_1"]);
    assert_eq!(error_codes(&res), ["missing_with"]);
    let res = w.app.post(&base, json!({"address_id": 999999})).await;
    assert_eq!(error_fields(&res), ["address_id"]);
    let foreign_address =
        addresses::ActiveModel { vault_id: Set(foreign.id), city: Set(Some("Far".into())), ..Default::default() }
            .insert(&db)
            .await
            .unwrap();
    let res = w.app.post(&base, json!({"address_id": foreign_address.id})).await;
    assert_eq!(error_codes(&res), ["exists"]);
    let res = w
        .app
        .post(
            &base,
            json!({"address_type_id": foreign_type.id, "line_1": "a".repeat(256), "latitude": 91, "longitude": -181}),
        )
        .await;
    assert_eq!(error_fields(&res), ["address_type_id", "line_1", "latitude", "longitude"]);

    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [address_id]);
    assert_eq!(
        ids(&w.app.get(&format!("{base}?is_past_address=true")).await.assert(200).json()["data"]),
        Vec::<i64>::new()
    );
    assert_eq!(
        ids(&w.app.get(&format!("{other_base}?is_past_address=true")).await.assert(200).json()["data"]),
        [address_id]
    );

    // Address fields change the shared address; is_past_address only this link.
    let up = w
        .app
        .patch(&format!("{base}/{address_id}"), json!({"city": "Lyon", "is_past_address": true}))
        .await
        .assert(200)
        .json();
    assert_eq!(up["city"], "Lyon");
    assert_eq!(up["line_1"], "1 Main St");
    assert_eq!(up["latitude"], 48.85);
    assert_eq!(up["is_past_address"], true);
    assert_eq!(up["address_type"]["id"], home.id.0);
    let seen_by_other = w.app.get(&other_base).await.assert(200).json();
    assert_eq!(seen_by_other["data"][0]["city"], "Lyon");
    let up = w
        .app
        .patch(&format!("{base}/{address_id}"), json!({"address_type_id": null, "line_2": "Apt 4"}))
        .await
        .assert(200)
        .json();
    assert!(up["address_type"].is_null());
    assert_eq!(up["line_2"], "Apt 4");
    let up = w.app.patch(&format!("{base}/{address_id}"), json!({"is_past_address": false})).await.assert(200).json();
    assert_eq!(up["is_past_address"], false);
    assert_eq!(up["line_2"], "Apt 4");
    let res = w.app.patch(&format!("{base}/{address_id}"), json!({"postal_code": "1".repeat(256)})).await;
    assert_eq!(error_fields(&res), ["postal_code"]);

    // An address of the vault the contact isn't linked to.
    let only_other = w.app.post(&other_base, json!({"line_1": "2 Side St"})).await.assert(201).json();
    let only_other_id = only_other["id"].as_i64().unwrap();
    w.app.patch(&format!("{base}/{only_other_id}"), json!({"city": "x"})).await.assert(404);
    w.app.delete(&format!("{base}/{only_other_id}")).await.assert(404);

    w.app.patch(&format!("{base}/{address_id}"), json!({"is_past_address": true})).await.assert(200);
    let module = serde_json::to_value(modules::addresses_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(module["active_addresses"], json!([]));
    assert_eq!(ids(&module["inactive_addresses"]), [address_id]);
    assert_eq!(ids(&module["vault_addresses"]), [address_id, only_other_id]);
    assert!(module["vault_addresses"][0].get("is_past_address").is_none());

    // Unlinking deletes the address once no contact is linked to it.
    w.app.delete(&format!("{base}/{address_id}")).await.assert(204);
    assert!(addresses::Entity::find_by_id(Id(address_id)).one(&db).await.unwrap().is_some());
    w.app.delete(&format!("{base}/{address_id}")).await.assert(404);
    w.app.delete(&format!("{other_base}/{address_id}")).await.assert(204);
    assert!(addresses::Entity::find_by_id(Id(address_id)).one(&db).await.unwrap().is_none());
    assert_eq!(
        contact_address::Entity::find()
            .filter(contact_address::Column::AddressId.eq(Id(address_id)))
            .count(&db)
            .await
            .unwrap(),
        0
    );

    w.login_viewer().await;
    w.app.get(&other_base).await.assert(200);
    w.app.post(&other_base, json!({"line_1": "x"})).await.assert(403);
    w.app.patch(&format!("{other_base}/{only_other_id}"), json!({"city": "x"})).await.assert(403);
    w.app.delete(&format!("{other_base}/{only_other_id}")).await.assert(403);

    w.login_stranger().await;
    w.app.get(&other_base).await.assert(404);
    w.app.get(&contact_path(&foreign, &foreign_contact, "/addresses")).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- contact information

async fn information_type(
    db: &sea_orm::DatabaseConnection,
    account_id: Uuid,
    key: &str,
    r#type: &str,
    protocol: Option<&str>,
) -> contact_information_types::Model {
    contact_information_types::ActiveModel {
        account_id: Set(account_id),
        name_translation_key: Set(Some(key.into())),
        r#type: Set(Some(r#type.into())),
        protocol: Set(protocol.map(Into::into)),
        can_be_deleted: Set(r#type != "email"),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

#[tokio::test]
async fn manages_contact_information() {
    let mut w = World::new().await;
    let db = w.app.state.db.clone();
    let base = w.path("/contact-information");
    let email = information_type(&db, w.user.account_id, "Email address", "email", Some("mailto:")).await;
    let facebook = information_type(&db, w.user.account_id, "Facebook", "X-SOCIAL-PROFILE", None).await;
    let foreign = factory::account(&db).await;
    let foreign_type = information_type(&db, foreign.id, "Facebook", "X-SOCIAL-PROFILE", None).await;

    let mail = w
        .app
        .post(&base, json!({"type_id": email.id, "kind": "work", "data": " regis@example.com "}))
        .await
        .assert(201)
        .json();
    assert_eq!(mail["data"], "regis@example.com");
    assert_eq!(mail["kind"], "work");
    assert_eq!(mail["contact_id"], w.contact.id.to_string());
    assert_eq!(mail["type"]["id"], email.id.0);
    assert_eq!(mail["type"]["protocol"], "mailto:");
    assert_eq!(mail["type"]["type"], "email");
    assert_eq!(mail["type"]["can_be_deleted"], false);
    let profile = w.app.post(&base, json!({"type_id": facebook.id, "data": "regis"})).await.assert(201).json();
    assert!(profile["kind"].is_null());
    let (mail_id, profile_id) = (mail["id"].as_i64().unwrap(), profile["id"].as_i64().unwrap());

    let res = w.app.post(&base, json!({"data": "x"})).await;
    assert_eq!(error_fields(&res), ["type_id"]);
    let res = w.app.post(&base, json!({"type_id": foreign_type.id, "data": " ", "kind": "k".repeat(256)})).await;
    assert_eq!(error_fields(&res), ["type_id", "data", "kind"]);
    assert_eq!(error_codes(&res), ["exists", "required", "max"]);

    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [mail_id, profile_id]);

    let up = w.app.patch(&format!("{base}/{mail_id}"), json!({"kind": null})).await.assert(200).json();
    assert!(up["kind"].is_null());
    assert_eq!(up["data"], "regis@example.com");
    let up = w
        .app
        .patch(&format!("{base}/{mail_id}"), json!({"data": "troyat@example.com", "kind": "home"}))
        .await
        .assert(200)
        .json();
    assert_eq!(up["data"], "troyat@example.com");
    assert_eq!(up["kind"], "home");
    assert_eq!(up["type"]["id"], email.id.0);
    let up = w.app.patch(&format!("{base}/{profile_id}"), json!({"type_id": email.id})).await.assert(200).json();
    assert_eq!(up["type"]["id"], email.id.0);
    w.app.patch(&format!("{base}/{profile_id}"), json!({"type_id": facebook.id})).await.assert(200);
    let res = w.app.patch(&format!("{base}/{mail_id}"), json!({"type_id": foreign_type.id, "data": ""})).await;
    assert_eq!(error_fields(&res), ["type_id", "data"]);

    let module =
        serde_json::to_value(modules::contact_information_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(ids(&module["contact_information"]["email"]), [mail_id]);
    assert_eq!(ids(&module["contact_information"]["X-SOCIAL-PROFILE"]), [profile_id]);
    let groups: Vec<&str> = module["groups"].as_array().unwrap().iter().map(|g| g["type"].as_str().unwrap()).collect();
    assert_eq!(groups, ["email", "phone", "IMPP", "X-SOCIAL-PROFILE"]);
    assert_eq!(module["groups"][0]["name_translation_key"], "Email address");
    let mut email_kinds: Vec<&str> =
        module["kinds"]["email"].as_array().unwrap().iter().map(|k| k["id"].as_str().unwrap()).collect();
    email_kinds.sort_unstable();
    assert_eq!(email_kinds, ["home", "other", "personal", "work"]);
    assert_eq!(module["kinds"]["phone"].as_array().unwrap().len(), 6);
    let protocols = module["social_protocols"].as_array().unwrap();
    assert_eq!(protocols.len(), 11);
    assert_eq!(
        protocols[0],
        json!({"name_translation_key": "Facebook", "url": "https://www.facebook.com/", "type": "X-SOCIAL-PROFILE"})
    );
    assert_eq!(protocols[7]["url"], "");

    let other = w.other_contact().await;
    let others = w
        .app
        .post(
            &contact_path(&w.vault, &other, "/contact-information"),
            json!({"type_id": email.id, "data": "x@example.com"}),
        )
        .await
        .assert(201)
        .json();
    w.app.patch(&format!("{base}/{}", others["id"]), json!({"data": "y"})).await.assert(404);
    w.app.delete(&format!("{base}/{}", others["id"])).await.assert(404);

    w.app.delete(&format!("{base}/{profile_id}")).await.assert(204);
    w.app.delete(&format!("{base}/{profile_id}")).await.assert(404);
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [mail_id]);

    w.login_viewer().await;
    w.app.get(&base).await.assert(200);
    w.app.post(&base, json!({"type_id": email.id, "data": "x"})).await.assert(403);
    w.app.patch(&format!("{base}/{mail_id}"), json!({"data": "x"})).await.assert(403);
    w.app.delete(&format!("{base}/{mail_id}")).await.assert(403);

    w.login_stranger().await;
    w.app.get(&base).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- loans

#[tokio::test]
async fn manages_loans() {
    let mut w = World::new().await;
    let db = w.app.state.db.clone();
    let base = w.path("/loans");
    let other = w.other_contact().await;
    let third = w.other_contact().await;
    let (_, outsider) = w.foreign_vault().await;
    let eur = currencies::Entity::find().filter(currencies::Column::Code.eq("EUR")).one(&db).await.unwrap().unwrap();

    let rent = w
        .app
        .post(
            &base,
            json!({
                "type": "monetary", "name": " Rent ", "description": "June", "amount_lent": 12345,
                "currency_id": eur.id, "loaned_at": "2025-06-01",
                "loaner_ids": [w.contact.id], "loanee_ids": [other.id]
            }),
        )
        .await
        .assert(201)
        .json();
    assert_eq!(rent["name"], "Rent");
    assert_eq!(rent["type"], "monetary");
    assert_eq!(rent["description"], "June");
    assert_eq!(rent["amount_lent"], 12345);
    assert_eq!(rent["currency"], json!({"id": eur.id.0, "code": "EUR", "name": "Euro"}));
    assert_eq!(rent["loaned_at"], "2025-06-01");
    assert_eq!(rent["settled"], false);
    assert!(rent["settled_at"].is_null());
    assert_eq!(rent["vault_id"], w.vault.id.to_string());
    assert_eq!(rent["loaners"][0]["id"], w.contact.id.to_string());
    assert_eq!(rent["loaners"][0]["first_name"], "Regis");
    assert!(rent["loaners"][0]["groups"].is_array());
    assert!(rent["loaners"][0].get("age").is_some());
    assert_eq!(rent["loanees"].as_array().unwrap().len(), 1);
    assert_eq!(rent["loanees"][0]["id"], other.id.to_string());
    let rent_id = rent["id"].as_i64().unwrap();

    let book = w
        .app
        .post(&base, json!({"type": "object", "name": "Book", "loaned_at": null, "loaner_ids": [other.id], "loanee_ids": [w.contact.id]}))
        .await
        .assert(201)
        .json();
    assert!(book["loaned_at"].is_null());
    assert!(book["currency"].is_null());
    assert!(book["amount_lent"].is_null());
    let book_id = book["id"].as_i64().unwrap();

    // Validation.
    let res = w.app.post(&base, json!({"type": "gift", "loaner_ids": [], "loanee_ids": [outsider.id]})).await;
    assert_eq!(error_fields(&res), ["type", "name", "loaner_ids", "loanee_ids[0]"]);
    assert_eq!(error_codes(&res), ["in", "required", "required", "exists"]);
    let res = w
        .app
        .post(
            &base,
            json!({"type": "object", "name": "x", "loaner_ids": [w.contact.id], "loanee_ids": [other.id, other.id], "currency_id": 999999, "amount_lent": 3_000_000_000_i64}),
        )
        .await;
    assert_eq!(error_fields(&res), ["amount_lent", "currency_id", "loanee_ids[1]"]);
    assert_eq!(error_codes(&res), ["max", "exists", "distinct"]);
    let res = w
        .app
        .post(&base, json!({"type": "object", "name": "x", "loaner_ids": [other.id], "loanee_ids": [third.id]}))
        .await;
    assert_eq!(error_fields(&res), ["loaner_ids"]);
    let res = w.app.post(&base, json!({"type": "object", "name": "x", "loaned_at": "2025-13-01", "loaner_ids": [w.contact.id], "loanee_ids": [other.id]})).await;
    assert_eq!(error_fields(&res), ["loaned_at"]);

    // Loans of the contact as loaner or loanee, undated first.
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [book_id, rent_id]);
    let other_base = contact_path(&w.vault, &other, "/loans");
    assert_eq!(ids(&w.app.get(&other_base).await.assert(200).json()["data"]), [book_id, rent_id]);

    // Settling and unsettling (Q6: unsettling clears settled_at).
    let settled = w.app.patch(&format!("{base}/{rent_id}"), json!({"settled": true})).await.assert(200).json();
    assert_eq!(settled["settled"], true);
    assert!(settled["settled_at"].is_string());
    assert_eq!(settled["name"], "Rent");
    assert_eq!(ids(&w.app.get(&format!("{base}?settled=false")).await.assert(200).json()["data"]), [book_id]);
    assert_eq!(ids(&w.app.get(&format!("{base}?settled=true")).await.assert(200).json()["data"]), [rent_id]);
    let module = serde_json::to_value(modules::loans_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(ids(&module["loans"]), [book_id]);
    let unsettled = w.app.patch(&format!("{base}/{rent_id}"), json!({"settled": false})).await.assert(200).json();
    assert_eq!(unsettled["settled"], false);
    assert!(unsettled["settled_at"].is_null());

    // Editing keeps what isn't sent; the parties are replaced when sent.
    let up = w
        .app
        .patch(&format!("{base}/{rent_id}"), json!({"name": "Rent 2", "amount_lent": null, "currency_id": null}))
        .await
        .assert(200)
        .json();
    assert_eq!(up["name"], "Rent 2");
    assert!(up["amount_lent"].is_null());
    assert!(up["currency"].is_null());
    assert_eq!(up["description"], "June");
    assert_eq!(up["loaned_at"], "2025-06-01");
    assert_eq!(up["loaners"][0]["id"], w.contact.id.to_string());
    let up = w
        .app
        .patch(&format!("{base}/{rent_id}"), json!({"loanee_ids": [third.id], "settled": true}))
        .await
        .assert(200)
        .json();
    assert_eq!(up["loanees"][0]["id"], third.id.to_string());
    assert_eq!(up["loanees"].as_array().unwrap().len(), 1);
    assert_eq!(up["loaners"][0]["id"], w.contact.id.to_string());
    assert_eq!(up["settled"], true);
    assert_eq!(up["name"], "Rent 2");
    let res = w.app.patch(&format!("{base}/{rent_id}"), json!({"type": "gift", "name": "", "loaner_ids": []})).await;
    assert_eq!(error_fields(&res), ["type", "name", "loaner_ids"]);

    // F17: a loan of the vault the contact doesn't take part in.
    let between_others = w
        .app
        .post(&other_base, json!({"type": "object", "name": "Pen", "loaner_ids": [other.id], "loanee_ids": [third.id]}))
        .await
        .assert(201)
        .json();
    let pen = format!("{base}/{}", between_others["id"]);
    w.app.patch(&pen, json!({"name": "Mine"})).await.assert(404);
    w.app.delete(&pen).await.assert(404);
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [book_id, rent_id]);
    w.app.patch(&format!("{base}/999999"), json!({"name": "x"})).await.assert(404);

    w.app.delete(&format!("{base}/{rent_id}")).await.assert(204);
    w.app.delete(&format!("{base}/{rent_id}")).await.assert(404);
    assert_eq!(ids(&w.app.get(&base).await.assert(200).json()["data"]), [book_id]);

    w.login_viewer().await;
    w.app.get(&base).await.assert(200);
    w.app
        .post(&base, json!({"type": "object", "name": "x", "loaner_ids": [w.contact.id], "loanee_ids": [other.id]}))
        .await
        .assert(403);
    w.app.patch(&format!("{base}/{book_id}"), json!({"settled": true})).await.assert(403);
    w.app.delete(&format!("{base}/{book_id}")).await.assert(403);

    w.login_stranger().await;
    w.app.get(&base).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- job information

async fn company(db: &sea_orm::DatabaseConnection, vault_id: Uuid, name: &str) -> companies::Model {
    companies::ActiveModel {
        vault_id: Set(vault_id),
        name: Set(name.into()),
        r#type: Set(Some("company".into())),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

#[tokio::test]
async fn sets_and_clears_job_information() {
    let mut w = World::new().await;
    let db = w.app.state.db.clone();
    let path = w.path("/job-information");
    let acme = company(&db, w.vault.id, "Acme").await;
    let (foreign, _) = w.foreign_vault().await;
    let elsewhere = company(&db, foreign.id, "Elsewhere").await;

    let job = w.app.put(&path, json!({"company_id": acme.id, "job_position": " CTO "})).await.assert(200).json();
    assert_eq!(job["job_position"], "CTO");
    assert_eq!(job["company"]["id"], acme.id.0);
    assert_eq!(job["company"]["name"], "Acme");
    assert_eq!(job["company"]["type"], "company");
    assert_eq!(job["company"]["vault_id"], w.vault.id.to_string());
    let module = serde_json::to_value(modules::company_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(module, job);

    let res = w.app.put(&path, json!({"job_position": "x"})).await;
    assert_eq!(error_fields(&res), ["company_id"]);
    let res = w.app.put(&path, json!({"company_id": elsewhere.id, "job_position": "x".repeat(256)})).await;
    assert_eq!(error_fields(&res), ["company_id", "job_position"]);
    assert_eq!(error_codes(&res), ["exists", "max"]);

    let job = w.app.put(&path, json!({"company_id": null, "job_position": "Freelance"})).await.assert(200).json();
    assert!(job["company"].is_null());
    assert_eq!(job["job_position"], "Freelance");

    w.app.put(&path, json!({"company_id": acme.id, "job_position": "CTO"})).await.assert(200);
    w.app.delete(&path).await.assert(204);
    let module = serde_json::to_value(modules::company_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(module, json!({"company": null, "job_position": null}));
    assert!(companies::Entity::find_by_id(acme.id).one(&db).await.unwrap().is_some(), "the company stays");

    w.app
        .put(
            &format!("/vaults/{}/contacts/{}/job-information", w.vault.id, Uuid::new()),
            json!({"company_id": null, "job_position": null}),
        )
        .await
        .assert(404);

    w.login_viewer().await;
    w.app.put(&path, json!({"company_id": null, "job_position": null})).await.assert(403);
    w.app.delete(&path).await.assert(403);

    w.login_stranger().await;
    w.app.put(&path, json!({"company_id": null, "job_position": null})).await.assert(404);
    w.app.delete(&path).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- religion

async fn religion(db: &sea_orm::DatabaseConnection, account_id: Uuid, key: &str) -> religions::Model {
    religions::ActiveModel {
        account_id: Set(account_id),
        translation_key: Set(Some(key.into())),
        position: Set(Some(1)),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
}

#[tokio::test]
async fn sets_and_clears_the_religion() {
    let mut w = World::new().await;
    let db = w.app.state.db.clone();
    let path = w.path("/religion");
    let christian = religion(&db, w.user.account_id, "Christian").await;
    let foreign = factory::account(&db).await;
    let elsewhere = religion(&db, foreign.id, "Buddhist").await;

    let r = w.app.put(&path, json!({"religion_id": christian.id})).await.assert(200).json();
    assert_eq!(
        r["religion"],
        json!({"id": christian.id.0, "name": null, "translation_key": "Christian", "position": 1})
    );
    let module = serde_json::to_value(modules::religions_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(module, r);

    let res = w.app.put(&path, json!({})).await;
    assert_eq!(error_fields(&res), ["religion_id"]);
    let res = w.app.put(&path, json!({"religion_id": elsewhere.id})).await;
    assert_eq!(error_codes(&res), ["exists"]);

    // F10: clearing works.
    let r = w.app.put(&path, json!({"religion_id": null})).await.assert(200).json();
    assert!(r["religion"].is_null());
    let module = serde_json::to_value(modules::religions_module(&w.app.state, &w.ctx().await).await.unwrap()).unwrap();
    assert_eq!(module, json!({"religion": null}));

    w.login_viewer().await;
    w.app.put(&path, json!({"religion_id": null})).await.assert(403);

    w.login_stranger().await;
    w.app.put(&path, json!({"religion_id": null})).await.assert(404);
    w.finish().await;
}
