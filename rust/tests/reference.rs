//! The reference lists over HTTP (`12-reference.yaml`): currencies,
//! countries, ISO 639-1 languages and timezones.

mod common;

use common::TestApp;
use monica::entities::users;
use monica::testing::factory;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, IntoActiveModel};
use serde_json::Value;

async fn logged_in(locale: &str) -> TestApp {
    let mut app = TestApp::new().await;
    let user = factory::create_user(&app.state.db).await;
    let mut u = user.clone().into_active_model();
    u.locale = Set(locale.into());
    let _: users::Model = u.update(&app.state.db).await.unwrap();
    app.login(&user.email).await;
    app
}

fn data(v: &Value) -> &Vec<Value> {
    v["data"].as_array().unwrap()
}

fn find<'a>(list: &'a [Value], key: &str, value: &str) -> &'a Value {
    list.iter().find(|x| x[key] == value).unwrap_or_else(|| panic!("no {key} {value}"))
}

#[tokio::test]
async fn lists_currencies_by_code() {
    let mut app = logged_in("fr").await;
    let res = app.get("/reference/currencies").await.assert(200).json();
    let list = data(&res);
    assert_eq!(list.len(), 164, "the codes the migration seeds");
    let codes: Vec<&str> = list.iter().map(|c| c["code"].as_str().unwrap()).collect();
    let mut sorted = codes.clone();
    sorted.sort_unstable();
    assert_eq!(codes, sorted);
    assert_eq!(list[0]["code"], "AED");
    assert_eq!(list[0]["name"], "United Arab Emirates Dirham", "only lang/en/currencies.php exists");
    let eur = find(list, "code", "EUR");
    assert_eq!(eur["name"], "Euro");
    assert!(eur["id"].is_i64());
    app.finish().await;
}

#[tokio::test]
async fn lists_countries_in_the_request_locale() {
    let mut app = logged_in("en").await;
    let res = app.get("/reference/countries").await.assert(200).json();
    let list = data(&res);
    assert_eq!(list.len(), 250);
    assert_eq!(list[0]["name"], "Afghanistan");
    assert_eq!(find(list, "code", "DE")["name"], "Germany");
    assert_eq!(find(list, "code", "AX")["name"], "Åland Islands");
    let names: Vec<&str> = list.iter().map(|c| c["name"].as_str().unwrap()).collect();
    let i = |n: &str| names.iter().position(|x| *x == n).unwrap();
    assert!(i("Åland Islands") < i("Albania"), "collator order, not byte order");
    app.finish().await;

    let mut app = logged_in("fr").await;
    let res = app.get("/reference/countries").await.assert(200).json();
    let list = data(&res);
    assert_eq!(find(list, "code", "DE")["name"], "Allemagne");
    assert_eq!(find(list, "code", "ES")["name"], "Espagne");
    assert_eq!(list[0]["name"], "Afghanistan");
    app.finish().await;

    // Native names count as translations; English otherwise.
    let mut app = logged_in("sv").await;
    let res = app.get("/reference/countries").await.assert(200).json();
    let list = data(&res);
    assert_eq!(find(list, "code", "SE")["name"], "Sverige");
    assert_eq!(find(list, "code", "DE")["name"], "Germany");
    app.finish().await;
}

#[tokio::test]
async fn lists_languages_by_name() {
    let mut app = logged_in("de").await;
    let res = app.get("/reference/languages").await.assert(200).json();
    let list = data(&res);
    assert_eq!(list.len(), 184, "the package's rows with an ISO 639-1 code");
    assert_eq!(list[0]["name"], "Abkhaz");
    let fr = find(list, "code", "fr");
    assert_eq!(fr["name"], "French", "no translations of language names exist");
    assert_eq!(fr["native_name"], "français, langue française");
    assert!(list.iter().all(|l| l["code"].as_str().unwrap().len() == 2));
    let c = monica::text::collator("de");
    let names: Vec<&str> = list.iter().map(|l| l["name"].as_str().unwrap()).collect();
    assert!(names.windows(2).all(|w| c.compare(w[0], w[1]).is_le()), "by name, with the collator");
    app.finish().await;
}

#[tokio::test]
async fn lists_timezones_with_their_offsets() {
    let mut app = logged_in("en").await;
    let res = app.get("/reference/timezones").await.assert(200).json();
    let list = data(&res);
    assert_eq!(list.len(), 420, "GMT plus DateTimeZone::listIdentifiers()");
    let ids: Vec<&str> = list.iter().map(|z| z["id"].as_str().unwrap()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
    assert_eq!(find(list, "id", "GMT")["utc_offset"], 0);
    assert_eq!(find(list, "id", "UTC")["utc_offset"], 0);
    assert_eq!(find(list, "id", "Asia/Kolkata")["utc_offset"], 330);
    assert_eq!(find(list, "id", "Asia/Kathmandu")["utc_offset"], 345);
    let ny = find(list, "id", "America/New_York")["utc_offset"].as_i64().unwrap();
    assert!(ny == -300 || ny == -240, "EST or EDT: {ny}");
    assert!(!ids.contains(&"US/Eastern"), "no backward-compatible aliases");
    // Every id is one chrono-tz knows, so updateMyPreferences accepts it.
    assert!(ids.iter().all(|z| z.parse::<chrono_tz::Tz>().is_ok()));
    app.finish().await;
}

#[tokio::test]
async fn needs_a_verified_user() {
    let mut app = TestApp::new().await;
    for path in ["/reference/currencies", "/reference/countries", "/reference/languages", "/reference/timezones"] {
        assert_eq!(app.get(path).await.assert(401).code(), "unauthenticated");
    }
    let user = factory::create_user(&app.state.db).await;
    let mut u = user.clone().into_active_model();
    u.email_verified_at = Set(None);
    let _: users::Model = u.update(&app.state.db).await.unwrap();
    app.login(&user.email).await;
    assert_eq!(app.get("/reference/countries").await.assert(403).code(), "email_not_verified");
    app.finish().await;
}
