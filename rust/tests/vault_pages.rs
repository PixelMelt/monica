//! Vault pages over HTTP: dashboard, feed, reminders, tasks, calendar,
//! reports, life metrics, files, companies and search.

mod common;

use chrono::{Datelike, Duration, NaiveDate, Utc};
use common::TestApp;
use monica::db::id::Id;
use monica::db::timestamp::Timestamp;
use monica::db::uuid::Uuid;
use monica::domains::base::VaultPermission;
use monica::domains::contact::manage_life_events::{CreateLifeEvent, CreateTimelineEvent};
use monica::domains::vault::manage_journals::CreateJournal;
use monica::entities::{
    addresses, companies, contact_address, contact_feed_items, contact_important_dates, contact_life_metric,
    contact_reminder_scheduled, contact_reminders, contact_tasks, contact_vault_user, contacts, files, groups,
    life_event_types, mood_tracking_events, mood_tracking_parameters, notes, post_sections, posts,
    user_notification_channels, users, vaults,
};
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{Value, json};

/// A logged-in user with a vault made through the API (so it has the
/// seeded date types, mood parameters and life event categories).
struct World {
    app: TestApp,
    user: users::Model,
    vault: vaults::Model,
    /// The user's own contact in the vault.
    me: Uuid,
    base: String,
}

async fn world() -> World {
    let mut app = TestApp::new().await;
    let user = factory::user(&app.state.db, None).await;
    app.login(&user.username).await;
    let v = app.post("/vaults", json!({"name": "Family"})).await.assert(201).json();
    let id: Uuid = v["id"].as_str().unwrap().parse().unwrap();
    let me: Uuid = v["user_contact_id"].as_str().unwrap().parse().unwrap();
    let vault = vaults::Entity::find_by_id(id).one(&app.state.db).await.unwrap().unwrap();
    let base = format!("/vaults/{id}");
    World { app, user, vault, me, base }
}

impl World {
    async fn contact(&self, first: &str, last: &str) -> contacts::Model {
        let c = factory::contact(&self.app.state.db, &self.vault).await;
        let mut a: contacts::ActiveModel = c.into();
        a.first_name = Set(Some(first.into()));
        a.last_name = Set(Some(last.into()));
        a.update(&self.app.state.db).await.unwrap()
    }

    /// A user of the same account with `permission` in the vault.
    async fn member(&self, permission: VaultPermission) -> users::Model {
        let u = factory::user(&self.app.state.db, Some(self.user.account_id)).await;
        factory::set_permission_in_vault(&self.app.state.db, &u, permission, &self.vault).await;
        u
    }

    /// Someone of another account.
    async fn stranger(&self) -> users::Model {
        let u = factory::user(&self.app.state.db, None).await;
        factory::create_vault_user(&self.app.state.db, &u, VaultPermission::Manage).await;
        u
    }

    async fn finish(self) {
        self.app.finish().await;
    }
}

fn ts(d: chrono::NaiveDateTime) -> Timestamp {
    Timestamp(d)
}

fn midnight(d: NaiveDate) -> Timestamp {
    Timestamp(d.and_hms_opt(0, 0, 0).unwrap())
}

fn ids(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x["id"].to_string().trim_matches('"').to_owned()).collect()
}

async fn task(w: &World, contact: Uuid, label: &str, due: Option<Timestamp>, completed: bool) -> contact_tasks::Model {
    contact_tasks::ActiveModel {
        contact_id: Set(contact),
        author_id: Set(Some(w.user.id)),
        author_name: Set("Me".into()),
        label: Set(label.into()),
        completed: Set(completed),
        due_at: Set(due),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap()
}

async fn channel(w: &World, user: Uuid) -> Id {
    user_notification_channels::ActiveModel {
        user_id: Set(Some(user)),
        r#type: Set("email".into()),
        label: Set(Some("mail".into())),
        content: Set("me@example.com".into()),
        active: Set(true),
        fails: Set(0),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap()
    .id
}

async fn reminder(w: &World, contact: Uuid, label: &str) -> Id {
    contact_reminders::ActiveModel {
        contact_id: Set(contact),
        label: Set(label.into()),
        r#type: Set("one_time".into()),
        number_times_triggered: Set(0),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap()
    .id
}

async fn schedule(w: &World, channel: Id, reminder: Id, at: Timestamp, triggered: bool) {
    contact_reminder_scheduled::ActiveModel {
        user_notification_channel_id: Set(channel),
        contact_reminder_id: Set(reminder),
        scheduled_at: Set(at),
        triggered_at: Set(triggered.then(monica::db::now)),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap();
}

async fn mood(w: &World, contact: Uuid, at: Timestamp) -> mood_tracking_events::Model {
    let p = mood_tracking_parameters::Entity::find()
        .filter(mood_tracking_parameters::Column::VaultId.eq(w.vault.id))
        .order_by_asc(mood_tracking_parameters::Column::Position)
        .one(&w.app.state.db)
        .await
        .unwrap()
        .unwrap();
    mood_tracking_events::ActiveModel {
        contact_id: Set(contact),
        mood_tracking_parameter_id: Set(p.id),
        rated_at: Set(at),
        note: Set(Some("fine".into())),
        number_of_hours_slept: Set(Some(7)),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap()
}

async fn important_date(w: &World, contact: Uuid, label: &str, day: i32, month: i32, year: Option<i32>) -> Id {
    contact_important_dates::ActiveModel {
        contact_id: Set(contact),
        label: Set(label.into()),
        day: Set(Some(day)),
        month: Set(Some(month)),
        year: Set(year),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap()
    .id
}

async fn soft_delete(w: &World, c: contacts::Model) {
    let mut a: contacts::ActiveModel = c.into();
    a.deleted_at = Set(Some(monica::db::now()));
    a.update(&w.app.state.db).await.unwrap();
}

// ---------------------------------------------------------------- dashboard

#[tokio::test]
async fn the_dashboard_gathers_the_vault_and_the_users_data() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let now = monica::db::now().0;
    let ross = w.contact("Ross", "Geller").await;
    let rachel = w.contact("Rachel", "Green").await;
    contact_vault_user::ActiveModel {
        contact_id: Set(rachel.id),
        vault_id: Set(w.vault.id),
        user_id: Set(w.user.id),
        number_of_views: Set(0),
        is_favorite: Set(true),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();

    // Reminders: one reminder on two channels, one too far, one triggered.
    let (a, b) = (channel(&w, w.user.id).await, channel(&w, w.user.id).await);
    let soon = reminder(&w, ross.id, "Call Ross").await;
    schedule(&w, a, soon, ts(now + Duration::days(3)), false).await;
    schedule(&w, b, soon, ts(now + Duration::days(2)), false).await;
    let late = reminder(&w, ross.id, "Later").await;
    schedule(&w, a, late, ts(now + Duration::days(40)), false).await;
    let done = reminder(&w, rachel.id, "Done").await;
    schedule(&w, a, done, ts(now + Duration::days(1)), true).await;
    let past = reminder(&w, rachel.id, "Missed").await;
    schedule(&w, a, past, ts(now - Duration::days(5)), false).await;

    // Tasks: due soon, undated, overdue, too far, completed.
    let due = task(&w, ross.id, "soon", Some(midnight(now.date() + Duration::days(10))), false).await;
    let undated = task(&w, rachel.id, "whenever", None, false).await;
    let overdue = task(&w, rachel.id, "overdue", Some(midnight(now.date() - Duration::days(2))), false).await;
    task(&w, ross.id, "far", Some(midnight(now.date() + Duration::days(60))), false).await;
    task(&w, ross.id, "done", Some(midnight(now.date())), true).await;

    // A life event of the user's contact.
    let types = life_event_types::Entity::find().all(&db).await.unwrap();
    let timeline = CreateTimelineEvent {
        account_id: w.user.account_id,
        vault_id: w.vault.id,
        author_id: w.user.id,
        label: Some("Trip".into()),
        started_at: "2024-05-01".into(),
    }
    .execute(&db)
    .await
    .unwrap();
    CreateLifeEvent {
        account_id: w.user.account_id,
        vault_id: w.vault.id,
        author_id: w.user.id,
        timeline_event_id: timeline.id,
        life_event_type_id: types[0].id,
        summary: Some("Rome".into()),
        description: None,
        happened_at: "2024-05-02".into(),
        costs: None,
        currency_id: None,
        paid_by_contact_id: None,
        duration_in_minutes: None,
        distance: None,
        distance_unit: None,
        from_place: None,
        to_place: None,
        place: None,
        participant_ids: vec![w.me, ross.id],
    }
    .execute(&db)
    .await
    .unwrap();

    let path = format!("{}/dashboard", w.base);
    let d = w.app.get(&path).await.assert(200).json();
    assert_eq!(d["default_activity_tab"], "activity");
    assert_eq!(d["user_contact"]["id"], w.me.to_string());
    assert_eq!(ids(&d["favorites"]), vec![rachel.id.to_string()]);
    assert_eq!(d["last_updated_contacts"].as_array().unwrap().len(), 3);

    let reminders = d["upcoming_reminders"].as_array().unwrap();
    let labels: Vec<&str> = reminders.iter().map(|r| r["label"].as_str().unwrap()).collect();
    assert_eq!(labels, ["Missed", "Call Ross"], "past untriggered first, one per reminder");
    assert_eq!(reminders[1]["contact_reminder_id"], soon.0);
    assert_eq!(reminders[1]["contact"]["first_name"], "Ross");
    assert_eq!(reminders[1]["scheduled_at"], Timestamp(now + Duration::days(2)).to_string());

    let tasks = d["due_tasks"].as_array().unwrap();
    assert_eq!(ids(&d["due_tasks"]), vec![undated.id.to_string(), overdue.id.to_string(), due.id.to_string()]);
    assert!(tasks[0]["due_at"].is_null());
    assert_eq!(tasks[0]["is_late"], false);
    assert_eq!(tasks[1]["is_late"], true);
    assert_eq!(tasks[2]["is_late"], false);
    assert_eq!(tasks[2]["due_at"], (now.date() + Duration::days(10)).to_string());
    assert_eq!(tasks[2]["contact"]["id"], ross.id.to_string());

    let params = d["mood_tracking_parameters"].as_array().unwrap();
    assert_eq!(params.len(), 5);
    assert!(params.windows(2).all(|p| p[0]["position"].as_i64() < p[1]["position"].as_i64()));
    assert_eq!(d["current_date"], Utc::now().date_naive().to_string());

    let events = d["life_events"]["timeline_events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["label"], "Trip");
    assert_eq!(events[0]["first_happened_at"], "2024-05-02");
    assert_eq!(events[0]["life_events"][0]["summary"], "Rome");
    assert_eq!(events[0]["life_events"][0]["participants"].as_array().unwrap().len(), 2);
    assert_eq!(d["life_events"]["meta"]["total"], 1);
    assert_eq!(d["life_metrics"], json!([]));

    let stranger = w.stranger().await;
    w.app.login(&stranger.username).await;
    w.app.get(&path).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- feed

#[tokio::test]
async fn the_feed_pages_the_items_of_every_live_contact_newest_first() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let ross = w.contact("Ross", "Geller").await;
    let gone = w.contact("Gone", "Away").await;
    let now = monica::db::now().0;
    for (i, contact) in [ross.id, w.me, ross.id, gone.id].into_iter().enumerate() {
        contact_feed_items::ActiveModel {
            author_id: Set(Some(w.user.id)),
            contact_id: Set(contact),
            action: Set("contact_created".into()),
            description: Set(Some(format!("item {i}"))),
            created_at: Set(Some(ts(now - Duration::hours(10 - i as i64)))),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }
    soft_delete(&w, gone).await;

    let path = format!("{}/feed", w.base);
    let page = w.app.get(&path).await.assert(200).json();
    let descriptions: Vec<&str> =
        page["data"].as_array().unwrap().iter().map(|i| i["description"].as_str().unwrap()).collect();
    assert_eq!(descriptions, ["item 2", "item 1", "item 0"]);
    assert_eq!(page["meta"], json!({"page": 1, "per_page": 15, "total": 3, "last_page": 1}));
    assert_eq!(page["data"][0]["contact"]["first_name"], "Ross");
    assert_eq!(page["data"][0]["author"]["id"], w.user.id.to_string());
    assert_eq!(page["data"][0]["author"]["contact"]["id"], w.me.to_string());

    let second = w.app.get(&format!("{path}?per_page=2&page=2")).await.assert(200).json();
    assert_eq!(second["data"].as_array().unwrap().len(), 1);
    assert_eq!(second["meta"]["last_page"], 2);

    let stranger = w.stranger().await;
    w.app.login(&stranger.username).await;
    w.app.get(&path).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- reminders and tasks

#[tokio::test]
async fn reminders_are_grouped_into_the_next_twelve_months() {
    let mut w = world().await;
    let ross = w.contact("Ross", "Geller").await;
    let today = Utc::now().date_naive();
    let first = today.with_day(1).unwrap();
    let next_month = if first.month() == 12 {
        NaiveDate::from_ymd_opt(first.year() + 1, 1, 15).unwrap()
    } else {
        NaiveDate::from_ymd_opt(first.year(), first.month() + 1, 15).unwrap()
    };
    let (a, b) = (channel(&w, w.user.id).await, channel(&w, w.user.id).await);
    let this = reminder(&w, ross.id, "this month").await;
    schedule(&w, a, this, midnight(first), false).await;
    schedule(&w, b, this, midnight(first), false).await;
    let next = reminder(&w, ross.id, "next month").await;
    schedule(&w, a, next, midnight(next_month), false).await;
    // F14: last year's untriggered reminder of this month is not this month's.
    let old = reminder(&w, ross.id, "last year").await;
    schedule(&w, a, old, midnight(NaiveDate::from_ymd_opt(first.year() - 1, first.month(), 1).unwrap()), false).await;

    let list = w.app.get(&format!("{}/reminders", w.base)).await.assert(200).json();
    let months = list["data"].as_array().unwrap();
    assert_eq!(months.len(), 12);
    assert_eq!(months[0]["year"], today.year());
    assert_eq!(months[0]["month"], today.month());
    assert_eq!(months[1]["month"], next_month.month());
    let labels = |m: &Value| -> Vec<String> {
        m["reminders"].as_array().unwrap().iter().map(|r| r["label"].as_str().unwrap().to_owned()).collect()
    };
    assert_eq!(labels(&months[0]), ["this month"]);
    assert_eq!(labels(&months[1]), ["next month"]);
    assert!(months[2..].iter().all(|m| labels(m).is_empty()));

    let stranger = w.stranger().await;
    w.app.login(&stranger.username).await;
    w.app.get(&format!("{}/reminders", w.base)).await.assert(404);
    w.finish().await;
}

#[tokio::test]
async fn open_tasks_are_grouped_by_contact_by_last_name() {
    let mut w = world().await;
    let zed = w.contact("Zed", "Zimmer").await;
    let amy = w.contact("Amy", "Adams").await;
    let idle = w.contact("Idle", "Busy").await;
    let later = task(&w, amy.id, "later", Some(midnight(NaiveDate::from_ymd_opt(2030, 1, 1).unwrap())), false).await;
    let sooner = task(&w, amy.id, "sooner", Some(midnight(NaiveDate::from_ymd_opt(2020, 1, 1).unwrap())), false).await;
    let open = task(&w, zed.id, "open", None, false).await;
    task(&w, idle.id, "done", None, true).await;

    let list = w.app.get(&format!("{}/tasks", w.base)).await.assert(200).json();
    let groups = list["data"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0]["contact"]["last_name"], "Adams");
    assert_eq!(ids(&groups[0]["tasks"]), vec![sooner.id.to_string(), later.id.to_string()]);
    assert_eq!(groups[0]["tasks"][0]["is_late"], true);
    assert_eq!(groups[0]["tasks"][0]["contact"]["id"], amy.id.to_string());
    assert_eq!(groups[1]["contact"]["last_name"], "Zimmer");
    assert_eq!(ids(&groups[1]["tasks"]), vec![open.id.to_string()]);

    let stranger = w.stranger().await;
    w.app.login(&stranger.username).await;
    w.app.get(&format!("{}/tasks", w.base)).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- calendar

async fn post_on(w: &World, journal: Id, title: &str, day: NaiveDate) -> posts::Model {
    let p = posts::ActiveModel {
        journal_id: Set(journal),
        published: Set(true),
        title: Set(Some(title.into())),
        view_count: Set(0),
        written_at: Set(midnight(day)),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap();
    for (position, content) in [(1, None), (2, Some("x".repeat(250)))] {
        post_sections::ActiveModel {
            post_id: Set(p.id),
            position: Set(position),
            label: Set("Section".into()),
            content: Set(content),
            ..Default::default()
        }
        .insert(&w.app.state.db)
        .await
        .unwrap();
    }
    p
}

#[tokio::test]
async fn the_calendar_shows_a_month_grid_and_a_day() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let ross = w.contact("Ross", "Geller").await;
    let rachel = w.contact("Rachel", "Green").await;
    important_date(&w, ross.id, "Birthday", 14, 2, Some(1990)).await;
    important_date(&w, ross.id, "Second", 14, 2, None).await;
    important_date(&w, rachel.id, "Wedding", 1, 3, None).await;
    mood(&w, w.me, midnight(NaiveDate::from_ymd_opt(2026, 2, 14).unwrap())).await;
    let journal = CreateJournal {
        account_id: w.user.account_id,
        vault_id: w.vault.id,
        author_id: w.user.id,
        name: "Diary".into(),
        description: None,
    }
    .execute(&db)
    .await
    .unwrap();
    let post = post_on(&w, journal.id, "Valentine", NaiveDate::from_ymd_opt(2026, 2, 14).unwrap()).await;

    // February 2026 starts on a Sunday and ends on a Saturday: 5 weeks.
    let m = w.app.get(&format!("{}/calendar/2026/2", w.base)).await.assert(200).json();
    assert_eq!((m["year"].as_i64(), m["month"].as_i64()), (Some(2026), Some(2)));
    let weeks = m["weeks"].as_array().unwrap();
    assert_eq!(weeks.len(), 5);
    assert!(weeks.iter().all(|w| w.as_array().unwrap().len() == 7));
    assert_eq!(weeks[0][0]["date"], "2026-01-26");
    assert_eq!(weeks[0][0]["is_in_month"], false);
    assert_eq!(weeks[0][6]["date"], "2026-02-01");
    assert_eq!(weeks[4][6]["date"], "2026-03-01");
    let valentine = &weeks[2][5];
    assert_eq!(valentine["date"], "2026-02-14");
    assert_eq!(valentine["is_in_month"], true);
    assert_eq!(valentine["is_today"], false);
    let dates = valentine["important_dates"].as_array().unwrap();
    assert_eq!(dates.len(), 1, "one per contact");
    assert_eq!(dates[0]["label"], "Birthday");
    assert_eq!(dates[0]["contact"]["first_name"], "Ross");
    assert!(dates[0]["age"].is_i64());
    assert_eq!(valentine["mood_tracking_events"][0]["note"], "fine");
    assert!(valentine["mood_tracking_events"][0]["mood_tracking_parameter"]["hex_color"].is_string());
    assert_eq!(valentine["posts"][0]["id"], post.id.0);
    assert_eq!(valentine["posts"][0]["journal"]["name"], "Diary");
    assert_eq!(valentine["posts"][0]["excerpt"], format!("{}...", "x".repeat(200)));
    assert_eq!(weeks[4][6]["important_dates"][0]["label"], "Wedding");
    assert_eq!(weeks[4][6]["is_in_month"], false);

    let today = Utc::now().date_naive();
    let now = w.app.get(&format!("{}/calendar/{}/{}", w.base, today.year(), today.month())).await.assert(200).json();
    let todays: Vec<&Value> = now["weeks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|w| w.as_array().unwrap())
        .filter(|d| d["is_today"] == true)
        .collect();
    assert_eq!(todays.len(), 1);
    assert_eq!(todays[0]["date"], today.to_string());

    let day = w.app.get(&format!("{}/calendar/2026/2/14", w.base)).await.assert(200).json();
    assert_eq!(day["date"], "2026-02-14");
    assert_eq!(day["is_in_month"], true);
    assert_eq!(day["important_dates"][0]["label"], "Birthday");
    assert_eq!(day["posts"][0]["title"], "Valentine");

    let bad = w.app.get(&format!("{}/calendar/2026/13", w.base)).await.assert(422).json();
    assert_eq!(bad["errors"][0]["field"], "month");
    w.app.get(&format!("{}/calendar/0/1", w.base)).await.assert(422);
    let bad = w.app.get(&format!("{}/calendar/2026/2/30", w.base)).await.assert(422).json();
    assert_eq!(bad["errors"][0]["field"], "day");
    w.app.get(&format!("{}/calendar/2026/2/32", w.base)).await.assert(422);

    let stranger = w.stranger().await;
    w.app.login(&stranger.username).await;
    w.app.get(&format!("{}/calendar/2026/2", w.base)).await.assert(404);
    w.app.get(&format!("{}/calendar/2026/2/14", w.base)).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- reports

async fn address(w: &World, city: Option<&str>, country: Option<&str>, residents: &[Uuid]) -> Id {
    let a = addresses::ActiveModel {
        vault_id: Set(w.vault.id),
        line_1: Set(Some("1 Main St".into())),
        city: Set(city.map(Into::into)),
        country: Set(country.map(Into::into)),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap();
    for c in residents {
        contact_address::ActiveModel {
            contact_id: Set(*c),
            address_id: Set(a.id),
            is_past_address: Set(false),
            ..Default::default()
        }
        .insert(&w.app.state.db)
        .await
        .unwrap();
    }
    a.id
}

#[tokio::test]
async fn the_address_reports_group_places_case_insensitively() {
    let mut w = world().await;
    let ross = w.contact("Ross", "Geller").await;
    let rachel = w.contact("Rachel", "Green").await;
    let gone = w.contact("Gone", "Away").await;
    let first = address(&w, Some("Paris"), Some("France"), &[ross.id]).await;
    let second = address(&w, Some("paris"), Some("FRANCE"), &[rachel.id, gone.id]).await;
    address(&w, Some("Lyon"), None, &[ross.id]).await;
    address(&w, None, Some("Italy"), &[]).await;
    soft_delete(&w, gone).await;

    let r = w.app.get(&format!("{}/reports/addresses", w.base)).await.assert(200).json();
    assert_eq!(
        r["cities"],
        json!([{"city": "Paris", "contacts_count": 2}, {"city": "Lyon", "contacts_count": 1}]),
        "F14: every address of the city counts"
    );
    assert_eq!(
        r["countries"],
        json!([{"country": "France", "contacts_count": 2}, {"country": "Italy", "contacts_count": 0}])
    );

    let city = w.app.get(&format!("{}/reports/addresses/by-city?city=PARIS", w.base)).await.assert(200).json();
    assert_eq!(city["name"], "PARIS");
    assert_eq!(ids(&city["addresses"]), vec![first.to_string(), second.to_string()]);
    assert_eq!(ids(&city["addresses"][0]["contacts"]), vec![ross.id.to_string()]);
    assert_eq!(ids(&city["addresses"][1]["contacts"]), vec![rachel.id.to_string()]);
    assert!(city["addresses"][0]["contacts"][0]["groups"].is_array());
    assert_eq!(city["addresses"][0]["line_1"], "1 Main St");

    let country = w.app.get(&format!("{}/reports/addresses/by-country?country=italy", w.base)).await.assert(200).json();
    assert_eq!(country["addresses"].as_array().unwrap().len(), 1);
    assert_eq!(country["addresses"][0]["contacts"], json!([]));

    let bad = w.app.get(&format!("{}/reports/addresses/by-city", w.base)).await.assert(422).json();
    assert_eq!(bad["errors"][0]["field"], "city");
    assert_eq!(bad["errors"][0]["code"], "required");
    let long = "a".repeat(256);
    w.app.get(&format!("{}/reports/addresses/by-country?country={long}", w.base)).await.assert(422);

    let stranger = w.stranger().await;
    w.app.login(&stranger.username).await;
    w.app.get(&format!("{}/reports/addresses", w.base)).await.assert(404);
    w.app.get(&format!("{}/reports/addresses/by-city?city=Paris", w.base)).await.assert(404);
    w.app.get(&format!("{}/reports/addresses/by-country?country=France", w.base)).await.assert(404);
    w.finish().await;
}

#[tokio::test]
async fn the_mood_report_has_every_day_of_the_year() {
    let mut w = world().await;
    let other = w.contact("Ross", "Geller").await;
    let jan31 = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
    mood(&w, w.me, midnight(jan31)).await;
    let latest = mood(&w, w.me, Timestamp(jan31.and_hms_opt(20, 0, 0).unwrap())).await;
    let march = mood(&w, w.me, midnight(NaiveDate::from_ymd_opt(2025, 3, 2).unwrap())).await;
    mood(&w, other.id, midnight(NaiveDate::from_ymd_opt(2025, 3, 3).unwrap())).await;

    let r = w.app.get(&format!("{}/reports/mood-tracking-events?year=2025", w.base)).await.assert(200).json();
    assert_eq!(r["year"], 2025);
    let months = r["months"].as_array().unwrap();
    assert_eq!(months.len(), 12);
    let days: Vec<usize> = months.iter().map(|m| m["days"].as_array().unwrap().len()).collect();
    assert_eq!(days, [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31], "F14: the last day is there");
    assert_eq!(months[0]["days"][30]["day"], 31);
    assert_eq!(months[0]["days"][30]["mood_tracking_event"]["id"], latest.id.0);
    assert!(months[0]["days"][30]["mood_tracking_event"]["hex_color"].is_string());
    assert_eq!(months[2]["days"][1]["mood_tracking_event"]["id"], march.id.0);
    assert!(months[2]["days"][2]["mood_tracking_event"].is_null(), "another contact's mood");

    let current = w.app.get(&format!("{}/reports/mood-tracking-events", w.base)).await.assert(200).json();
    assert_eq!(current["year"], Utc::now().year());
    let bad = w.app.get(&format!("{}/reports/mood-tracking-events?year=0", w.base)).await.assert(422).json();
    assert_eq!(bad["errors"][0]["field"], "year");

    let stranger = w.stranger().await;
    w.app.login(&stranger.username).await;
    w.app.get(&format!("{}/reports/mood-tracking-events", w.base)).await.assert(404);
    w.finish().await;
}

#[tokio::test]
async fn the_important_date_report_covers_the_next_twelve_months() {
    let mut w = world().await;
    let ross = w.contact("Ross", "Geller").await;
    let gone = w.contact("Gone", "Away").await;
    let today = Utc::now().date_naive();
    let this_month = today.month() as i32;
    let next_month = if this_month == 12 { 1 } else { this_month + 1 };
    let late = important_date(&w, ross.id, "late", 20, this_month, Some(2000)).await;
    let early = important_date(&w, ross.id, "early", 2, this_month, None).await;
    let next = important_date(&w, ross.id, "next", 5, next_month, None).await;
    important_date(&w, gone.id, "gone", 3, this_month, None).await;
    soft_delete(&w, gone).await;

    let r = w.app.get(&format!("{}/reports/important-dates", w.base)).await.assert(200).json();
    let months = r["months"].as_array().unwrap();
    assert_eq!(months.len(), 12);
    assert_eq!(
        (months[0]["year"].as_i64(), months[0]["month"].as_i64()),
        (Some(today.year().into()), Some(this_month.into()))
    );
    assert_eq!(ids(&months[0]["important_dates"]), vec![early.to_string(), late.to_string()]);
    assert_eq!(ids(&months[1]["important_dates"]), vec![next.to_string()]);
    let late_item = &months[0]["important_dates"][1];
    assert_eq!(late_item["contact"]["id"], ross.id.to_string());
    assert_eq!(late_item["year"], 2000);
    assert!(late_item["age"].is_i64());
    assert!(months[0]["important_dates"][0]["age"].is_null());
    assert_eq!(late_item["contact_important_date_type"], Value::Null);

    let stranger = w.stranger().await;
    w.app.login(&stranger.username).await;
    w.app.get(&format!("{}/reports/important-dates", w.base)).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- life metrics

#[tokio::test]
async fn life_metrics_are_created_counted_renamed_and_deleted() {
    let mut w = world().await;
    let path = format!("{}/life-metrics", w.base);

    w.app.post(&path, json!({"label": "  "})).await.assert(422);
    let long = w.app.post(&path, json!({"label": "a".repeat(256)})).await.assert(422).json();
    assert_eq!(long["errors"][0]["code"], "max");
    let m = w.app.post(&path, json!({"label": "Coffee"})).await.assert(201).json();
    let id = m["id"].as_i64().unwrap();
    assert_eq!(m["label"], "Coffee");
    assert_eq!(m["vault_id"], w.vault.id.to_string());
    assert_eq!(m["stats"], json!({"weekly_events": 0, "monthly_events": 0, "yearly_events": 0}));
    assert_eq!(m["months"].as_array().unwrap().len(), 12);
    assert_eq!(m["year"], Utc::now().year());
    assert_eq!(m["max_number_of_events"], 0);

    let m = w.app.post(&format!("{path}/{id}/increment"), json!({})).await.assert(200).json();
    assert_eq!(m["stats"], json!({"weekly_events": 1, "monthly_events": 1, "yearly_events": 1}));
    assert_eq!(m["years"], json!([Utc::now().year()]));
    assert_eq!(m["max_number_of_events"], 1);
    assert_eq!(m["months"][Utc::now().month0() as usize]["events"], 1);

    // An event from 2019.
    contact_life_metric::ActiveModel {
        contact_id: Set(w.me),
        life_metric_id: Set(Id(id)),
        created_at: Set(Some(midnight(NaiveDate::from_ymd_opt(2019, 4, 3).unwrap()))),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap();
    let list = w.app.get(&format!("{path}?year=2019")).await.assert(200).json();
    assert_eq!(list["data"].as_array().unwrap().len(), 1);
    let old = &list["data"][0];
    assert_eq!(old["year"], 2019);
    assert_eq!(old["months"][3], json!({"month": 4, "events": 1}));
    assert_eq!(old["years"], json!([Utc::now().year(), 2019]));
    assert_eq!(old["stats"]["yearly_events"], 1);
    w.app.get(&format!("{path}?year=10000")).await.assert(422);

    let renamed = w.app.patch(&format!("{path}/{id}"), json!({"label": "Tea"})).await.assert(200).json();
    assert_eq!(renamed["label"], "Tea");
    assert_eq!(renamed["stats"]["yearly_events"], 1);
    w.app.patch(&format!("{path}/{id}"), json!({})).await.assert(422);
    w.app.patch(&format!("{path}/999999"), json!({"label": "x"})).await.assert(404);

    let dashboard = w.app.get(&format!("{}/dashboard", w.base)).await.assert(200).json();
    assert_eq!(dashboard["life_metrics"][0]["label"], "Tea");

    let viewer = w.member(VaultPermission::View).await;
    let stranger = w.stranger().await;
    w.app.login(&viewer.username).await;
    let seen = w.app.get(&path).await.assert(200).json();
    assert_eq!(seen["data"][0]["stats"]["yearly_events"], 0, "the viewer's own counts");
    w.app.post(&path, json!({"label": "Mine"})).await.assert(403);
    w.app.patch(&format!("{path}/{id}"), json!({"label": "Mine"})).await.assert(403);
    w.app.post(&format!("{path}/{id}/increment"), json!({})).await.assert(403);
    w.app.delete(&format!("{path}/{id}")).await.assert(403);
    w.app.login(&stranger.username).await;
    w.app.get(&path).await.assert(404);
    w.app.post(&format!("{path}/{id}/increment"), json!({})).await.assert(404);

    w.app.login(&w.user.username.clone()).await;
    w.app.delete(&format!("{path}/{id}")).await.assert(204);
    w.app.delete(&format!("{path}/{id}")).await.assert(404);
    w.app.post(&format!("{path}/{id}/increment"), json!({})).await.assert(404);
    assert_eq!(w.app.get(&path).await.json()["data"], json!([]));
    w.finish().await;
}

// ---------------------------------------------------------------- files

const PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99, 0x3D,
    0x1D, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

async fn stored_file(w: &World, vault: Uuid, ty: &str, bytes: &[u8], owner: Option<Uuid>, age: i64) -> files::Model {
    let uuid = w.app.state.storage.put(vault, bytes).await.unwrap();
    files::ActiveModel {
        vault_id: Set(vault),
        uuid: Set(uuid),
        name: Set(format!("{ty}.bin")),
        mime_type: Set(if ty == "document" { "application/pdf".into() } else { "image/png".into() }),
        r#type: Set(ty.into()),
        size: Set(bytes.len() as i32),
        fileable_type: Set(owner.map(|_| "App\\Models\\Contact".into())),
        ufileable_id: Set(owner),
        created_at: Set(Some(ts(monica::db::now().0 - Duration::minutes(age)))),
        ..Default::default()
    }
    .insert(&w.app.state.db)
    .await
    .unwrap()
}

#[tokio::test]
async fn vault_files_are_listed_served_and_deleted() {
    let mut w = world().await;
    let ross = w.contact("Ross", "Geller").await;
    let photo = stored_file(&w, w.vault.id, "photo", PNG, Some(ross.id), 3).await;
    let document = stored_file(&w, w.vault.id, "document", b"%PDF-1.4 hello", Some(ross.id), 2).await;
    let avatar = stored_file(&w, w.vault.id, "avatar", PNG, None, 1).await;
    let elsewhere = factory::create_vault(&w.app.state.db, w.user.account_id).await;
    let foreign = stored_file(&w, elsewhere.id, "photo", PNG, None, 0).await;
    let path = format!("{}/files", w.base);

    let list = w.app.get(&path).await.assert(200).json();
    assert_eq!(ids(&list["data"]), vec![avatar.id.to_string(), document.id.to_string(), photo.id.to_string()]);
    assert_eq!(list["counts"], json!({"all": 3, "photo": 1, "document": 1, "avatar": 1}));
    assert_eq!(list["meta"]["per_page"], 25);
    assert_eq!(list["data"][1]["contact"]["id"], ross.id.to_string());
    assert!(list["data"][0]["contact"].is_null());
    assert_eq!(list["data"][1]["uuid"], document.uuid);
    assert_eq!(list["data"][1]["size"], 14);
    let photos = w.app.get(&format!("{path}?type=photo")).await.assert(200).json();
    assert_eq!(ids(&photos["data"]), vec![photo.id.to_string()]);
    assert_eq!(photos["counts"]["all"], 3);
    let bad = w.app.get(&format!("{path}?type=video")).await.assert(422).json();
    assert_eq!(bad["errors"][0]["field"], "type");

    let res = w.app.get(&format!("{path}/{}/content", document.id)).await.assert(200);
    assert_eq!(res.body, b"%PDF-1.4 hello");
    assert_eq!(res.headers["content-type"], "application/pdf");
    assert!(res.headers["content-disposition"].to_str().unwrap().starts_with("attachment"));
    let etag = res.headers["etag"].to_str().unwrap().to_owned();
    let cached = w
        .app
        .request_with(
            axum::http::Method::GET,
            &format!("{path}/{}/content", document.id),
            None,
            &[("if-none-match", &etag)],
        )
        .await;
    assert_eq!(cached.status.as_u16(), 304);
    let thumb = w.app.get(&format!("{path}/{}/content?variant=thumbnail", photo.id)).await.assert(200);
    assert!(thumb.headers["content-type"].to_str().unwrap().starts_with("image/"));
    w.app.get(&format!("{path}/{}/content?variant=huge", photo.id)).await.assert(422);
    w.app.get(&format!("{path}/{}/content", foreign.id)).await.assert(404);

    let viewer = w.member(VaultPermission::View).await;
    let stranger = w.stranger().await;
    w.app.login(&viewer.username).await;
    w.app.get(&format!("{path}/{}/content", photo.id)).await.assert(200);
    w.app.delete(&format!("{path}/{}", photo.id)).await.assert(403);
    w.app.login(&stranger.username).await;
    w.app.get(&path).await.assert(404);
    w.app.get(&format!("{path}/{}/content", photo.id)).await.assert(404);
    w.app.delete(&format!("{path}/{}", photo.id)).await.assert(404);

    w.app.login(&w.user.username.clone()).await;
    w.app.delete(&format!("{path}/{}", foreign.id)).await.assert(404);
    w.app.delete(&format!("{path}/{}", photo.id)).await.assert(204);
    assert!(!w.app.state.storage.path(w.vault.id, &photo.uuid).exists(), "E009: the bytes are gone");
    w.app.get(&format!("{path}/{}/content", photo.id)).await.assert(404);
    w.app.delete(&format!("{path}/{}", photo.id)).await.assert(404);
    let touched = contacts::Entity::find_by_id(ross.id).one(&w.app.state.db).await.unwrap().unwrap();
    assert!(touched.last_updated_at.is_some(), "X013: the contact was updated");
    assert_eq!(w.app.get(&path).await.json()["counts"]["all"], 2);
    w.finish().await;
}

// ---------------------------------------------------------------- companies

#[tokio::test]
async fn companies_are_created_and_listed_by_name() {
    let mut w = world().await;
    let path = format!("{}/companies", w.base);
    w.app.post(&path, json!({"name": " "})).await.assert(422);
    let bad = w.app.post(&path, json!({"name": "Acme", "type": "cartel"})).await.assert(422).json();
    assert_eq!(bad["errors"][0]["field"], "type");
    w.app.post(&path, json!({"name": "a".repeat(256)})).await.assert(422);

    let zeta = w.app.post(&path, json!({"name": "Zeta", "type": "company"})).await.assert(201).json();
    assert_eq!(zeta["name"], "Zeta");
    assert_eq!(zeta["type"], "company");
    assert_eq!(zeta["vault_id"], w.vault.id.to_string());
    let alpha = w.app.post(&path, json!({"name": "Éclair", "type": null})).await.assert(201).json();
    assert!(alpha["type"].is_null());
    let ross = w.contact("Ross", "Geller").await;
    let mut a: contacts::ActiveModel = ross.clone().into();
    a.company_id = Set(Some(Id(zeta["id"].as_i64().unwrap())));
    a.update(&w.app.state.db).await.unwrap();

    let list = w.app.get(&path).await.assert(200).json();
    let names: Vec<&str> = list["data"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Éclair", "Zeta"]);
    assert_eq!(ids(&list["data"][1]["contacts"]), vec![ross.id.to_string()]);
    assert_eq!(list["data"][0]["contacts"], json!([]));
    assert!(companies::Entity::find().all(&w.app.state.db).await.unwrap().len() == 2);

    let viewer = w.member(VaultPermission::View).await;
    let stranger = w.stranger().await;
    w.app.login(&viewer.username).await;
    w.app.get(&path).await.assert(200);
    w.app.post(&path, json!({"name": "Mine"})).await.assert(403);
    w.app.login(&stranger.username).await;
    w.app.get(&path).await.assert(404);
    w.app.post(&path, json!({"name": "Mine"})).await.assert(404);
    w.finish().await;
}

// ---------------------------------------------------------------- search

#[tokio::test]
async fn search_matches_contacts_notes_and_groups() {
    let mut w = world().await;
    let db = w.app.state.db.clone();
    let ross = w.contact("Ross", "Geller").await;
    let mut a: contacts::ActiveModel = ross.clone().into();
    a.nickname = Set(Some("Dinosaur".into()));
    a.update(&db).await.unwrap();
    let rachel = w.contact("Rachel", "Green").await;
    let gone = w.contact("Saurus", "Gone").await;
    soft_delete(&w, gone).await;
    let note = notes::ActiveModel {
        contact_id: Set(rachel.id),
        vault_id: Set(w.vault.id),
        author_id: Set(Some(w.user.id)),
        title: Set(Some("Museum".into())),
        body: Set("Ross loves the SAURUS bones".into()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    notes::ActiveModel {
        contact_id: Set(rachel.id),
        vault_id: Set(w.vault.id),
        title: Set(None),
        body: Set("nothing here".into()),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let group =
        groups::ActiveModel { vault_id: Set(w.vault.id), name: Set("Saurus club".into()), ..Default::default() }
            .insert(&db)
            .await
            .unwrap();
    let path = format!("{}/search", w.base);

    let r = w.app.get(&format!("{path}?q=saur")).await.assert(200).json();
    assert_eq!(ids(&r["contacts"]), vec![ross.id.to_string()]);
    assert_eq!(ids(&r["notes"]), vec![note.id.to_string()]);
    assert_eq!(r["notes"][0]["contact"]["id"], rachel.id.to_string());
    assert_eq!(r["notes"][0]["author"]["id"], w.user.id.to_string());
    assert_eq!(r["notes"][0]["title"], "Museum");
    assert_eq!(r["groups"], json!([{"id": group.id.0, "name": "Saurus club"}]));

    let by_last = w.app.get(&format!("{path}?q=GREEN")).await.assert(200).json();
    assert_eq!(ids(&by_last["contacts"]), vec![rachel.id.to_string()]);
    let by_title = w.app.get(&format!("{path}?q=museum")).await.assert(200).json();
    assert_eq!(by_title["notes"].as_array().unwrap().len(), 1);

    let empty = w.app.get(&path).await.assert(200).json();
    assert_eq!(empty, json!({"contacts": [], "notes": [], "groups": []}));
    let bad = w.app.get(&format!("{path}?q={}", "a".repeat(256))).await.assert(422).json();
    assert_eq!(bad["errors"][0]["field"], "q");

    let stranger = w.stranger().await;
    w.app.login(&stranger.username).await;
    w.app.get(&format!("{path}?q=saur")).await.assert(404);
    w.finish().await;
}
