//! `tests/Unit/Domains/Settings/ManageCurrencies/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};

use super::*;
use crate::entities::{accounts, users};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Currency::factory()`.
async fn currency(db: &TestDb) -> currencies::Model {
    currencies::ActiveModel { code: Set("CAD".into()), ..Default::default() }.insert(&db.conn).await.expect("currency")
}

/// `$account->currencies()->attach($currency->id, ['active' => $active])`.
async fn attach(db: &TestDb, account_id: Uuid, currency: &currencies::Model, active: bool) {
    account_currency::ActiveModel {
        currency_id: Set(currency.id),
        account_id: Set(account_id),
        active: Set(active),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("account_currency");
}

async fn account_currency_rows(db: &TestDb, account_id: Uuid) -> Vec<account_currency::Model> {
    account_currency::Entity::find()
        .filter(account_currency::Column::AccountId.eq(account_id))
        .all(&db.conn)
        .await
        .unwrap()
}

async fn account_of(db: &TestDb, user: &users::Model) -> accounts::Model {
    accounts::Entity::find_by_id(user.account_id).one(&db.conn).await.unwrap().unwrap()
}

// ---------------------------------------------------------------- DisableAllCurrenciesTest

async fn disable_all(db: &TestDb, author: &users::Model, account: &accounts::Model) -> ApiResult<()> {
    let currency = currency(db).await;
    attach(db, account.id, &currency, true).await;

    DisableAllCurrencies { account_id: account.id, author_id: author.id }.execute(&db.conn).await?;

    let rows = account_currency_rows(db, account.id).await;
    assert!(rows.iter().any(|r| !r.active));
    Ok(())
}

#[tokio::test]
async fn it_disables_all_currencies() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = account_of(&db, &ross).await;
    disable_all(&db, &ross, &account).await.unwrap();
    db.drop().await;
}

// it_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(disable_all(&db, &ross, &account).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let account = account_of(&db, &ross).await;
    assert_eq!(disable_all(&db, &ross, &account).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- EnableAllCurrenciesTest

async fn enable_all(db: &TestDb, author: &users::Model, account: &accounts::Model) -> ApiResult<()> {
    let currency = currency(db).await;
    attach(db, account.id, &currency, false).await;

    EnableAllCurrencies { account_id: account.id, author_id: author.id }.execute(&db.conn).await?;

    let rows = account_currency_rows(db, account.id).await;
    assert!(rows.iter().any(|r| r.active));
    Ok(())
}

#[tokio::test]
async fn it_enables_all_currencies() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = account_of(&db, &ross).await;
    enable_all(&db, &ross, &account).await.unwrap();
    db.drop().await;
}

// enable_all_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn enable_all_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(enable_all(&db, &ross, &account).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn enable_all_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let account = account_of(&db, &ross).await;
    assert_eq!(enable_all(&db, &ross, &account).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- ToggleCurrencyTest

async fn toggle(db: &TestDb, author: &users::Model, account: &accounts::Model) -> ApiResult<()> {
    let currency = currency(db).await;
    attach(db, account.id, &currency, false).await;

    ToggleCurrency { account_id: account.id, author_id: author.id, currency_id: currency.id }.execute(&db.conn).await?;

    let rows = account_currency_rows(db, account.id).await;
    assert!(rows.iter().any(|r| r.currency_id == currency.id && r.active));
    Ok(())
}

#[tokio::test]
async fn it_toggles_a_currency() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = account_of(&db, &ross).await;
    toggle(&db, &ross, &account).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = ToggleCurrency { account_id: ross.account_id, author_id: ross.id, currency_id: Id(i64::MAX) }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(toggle(&db, &ross, &account).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let account = account_of(&db, &ross).await;
    assert_eq!(toggle(&db, &ross, &account).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}
