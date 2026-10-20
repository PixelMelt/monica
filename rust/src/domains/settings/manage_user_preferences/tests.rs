//! `tests/Unit/Domains/Settings/ManageUserPreferences/Services`.

use sea_orm::EntityTrait;

use super::*;
use crate::error::Code;
use crate::testing::{TestDb, factory};

async fn row(db: &TestDb, user: &users::Model) -> users::Model {
    users::Entity::find_by_id(user.id).one(&db.conn).await.unwrap().unwrap()
}

// ---- StoreDateFormatPreferenceTest

async fn store_date_format(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let user = StoreDateFormatPreference { account_id, author_id: author.id, date_format: "Y".into() }
        .execute(&db.conn)
        .await?;

    let row = row(db, &user).await;
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.date_format, "Y");
    Ok(())
}

#[tokio::test]
async fn it_stores_the_date_format_preference() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    store_date_format(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn date_format_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let err = StoreDateFormatPreference { account_id: ross.account_id, author_id: ross.id, date_format: String::new() }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn date_format_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(store_date_format(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- StoreDistanceFormatPreferenceTest

async fn store_distance_format(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let user = StoreDistanceFormatPreference { account_id, author_id: author.id, distance_format: "km".into() }
        .execute(&db.conn)
        .await?;

    let row = row(db, &user).await;
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.distance_format, "km");
    Ok(())
}

#[tokio::test]
async fn it_stores_the_distance_format_preference() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    store_distance_format(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn distance_format_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let err = StoreDistanceFormatPreference {
        account_id: ross.account_id,
        author_id: ross.id,
        distance_format: "x".repeat(256),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn distance_format_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(store_distance_format(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- StoreHelpPreferenceTest

async fn store_help(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let user = StoreHelpPreference { account_id, author_id: author.id, visibility: false }.execute(&db.conn).await?;

    let row = row(db, &user).await;
    assert_eq!(row.account_id, account_id);
    assert!(!row.help_shown);
    Ok(())
}

#[tokio::test]
async fn it_stores_the_help_preferences() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    store_help(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

// help_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn help_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(store_help(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- StoreLocaleTest

async fn store_locale(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let user = StoreLocale { account_id, author_id: author.id, locale: "fr".into() }.execute(&db.conn).await?;

    let row = row(db, &user).await;
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.locale, "fr");
    Ok(())
}

#[tokio::test]
async fn it_stores_the_locale() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    store_locale(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn locale_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let err = StoreLocale { account_id: ross.account_id, author_id: ross.id, locale: "xx".into() }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn locale_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(store_locale(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- StoreMapsPreferenceTest

async fn store_maps(db: &TestDb, author: &users::Model, maps_site: &str, account_id: Uuid) -> ApiResult<()> {
    let user =
        StoreMapsPreference { account_id, author_id: author.id, maps_site: maps_site.into() }.execute(&db.conn).await?;

    let row = row(db, &user).await;
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.default_map_site, maps_site);
    Ok(())
}

#[tokio::test]
async fn it_stores_the_maps_preferences() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    store_maps(&db, &ross, "google_maps", ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn maps_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let err = StoreMapsPreference { account_id: ross.account_id, author_id: ross.id, maps_site: String::new() }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn maps_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(store_maps(&db, &ross, "google_maps", account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- StoreNameOrderPreferenceTest

async fn store_name_order(db: &TestDb, author: &users::Model, name_order: &str, account_id: Uuid) -> ApiResult<()> {
    let user = StoreNameOrderPreference { account_id, author_id: author.id, name_order: name_order.into() }
        .execute(&db.conn)
        .await?;

    let row = row(db, &user).await;
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name_order, name_order);
    Ok(())
}

#[tokio::test]
async fn it_stores_the_name_order_preference() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    store_name_order(&db, &ross, "%name%", ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn name_order_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let err = StoreNameOrderPreference { account_id: ross.account_id, author_id: ross.id, name_order: "x".repeat(256) }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn name_order_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(store_name_order(&db, &ross, "%user%", account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn name_order_fails_if_name_order_has_no_variable() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    assert_eq!(store_name_order(&db, &ross, "", ross.account_id).await.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn name_order_fails_if_name_order_has_no_closing_percent_symbol() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    assert_eq!(store_name_order(&db, &ross, "%", ross.account_id).await.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

// ---- StoreNumberFormatPreferenceTest

async fn store_number_format(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let user = StoreNumberFormatPreference { account_id, author_id: author.id, number_format: "1,234.56".into() }
        .execute(&db.conn)
        .await?;

    let row = row(db, &user).await;
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.number_format, "1,234.56");
    Ok(())
}

#[tokio::test]
async fn it_stores_the_number_format_preference() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    store_number_format(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn number_format_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let err =
        StoreNumberFormatPreference { account_id: ross.account_id, author_id: ross.id, number_format: "1".into() }
            .execute(&db.conn)
            .await
            .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn number_format_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(store_number_format(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- StoreTimezoneTest

async fn store_timezone(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let user = StoreTimezone { account_id, author_id: author.id, timezone: "UTC".into() }.execute(&db.conn).await?;

    let row = row(db, &user).await;
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.timezone.as_deref(), Some("UTC"));
    Ok(())
}

#[tokio::test]
async fn it_stores_the_timezone() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    store_timezone(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn timezone_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let err = StoreTimezone { account_id: ross.account_id, author_id: ross.id, timezone: String::new() }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn timezone_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(store_timezone(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}
