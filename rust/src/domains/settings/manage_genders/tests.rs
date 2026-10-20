//! `tests/Unit/Domains/Settings/ManageGenders/Services`.

use sea_orm::EntityTrait;

use super::*;
use crate::entities::accounts;
use crate::error::Code;
use crate::testing::{TestDb, factory};

// ---------------------------------------------------------------- CreateGenderTest

async fn create_gender(
    db: &TestDb,
    author: &crate::entities::users::Model,
    account: &accounts::Model,
) -> ApiResult<()> {
    let gender = CreateGender {
        account_id: account.id,
        author_id: author.id,
        name: Some("gender name".into()),
        name_translation_key: None,
    }
    .execute(&db.conn)
    .await?;

    let row = genders::Entity::find_by_id(gender.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account.id);
    assert_eq!(row.name.as_deref(), Some("gender name"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_gender() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = accounts::Entity::find_by_id(ross.account_id).one(&db.conn).await.unwrap().unwrap();
    create_gender(&db, &ross, &account).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateGender {
        account_id: ross.account_id,
        author_id: ross.id,
        name: Some("x".repeat(256)),
        name_translation_key: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create_gender(&db, &ross, &account).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let account = accounts::Entity::find_by_id(ross.account_id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(create_gender(&db, &ross, &account).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateGenderTest

async fn update_gender(
    db: &TestDb,
    author: &crate::entities::users::Model,
    account_id: Uuid,
    gender: &genders::Model,
) -> ApiResult<()> {
    let gender = UpdateGender { account_id, author_id: author.id, gender_id: gender.id, name: "gender name".into() }
        .execute(&db.conn)
        .await?;

    let row = genders::Entity::find_by_id(gender.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("gender name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_gender() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gender = factory::gender(&db.conn, ross.account_id).await;
    update_gender(&db, &ross, ross.account_id, &gender).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gender = factory::gender(&db.conn, ross.account_id).await;
    let err =
        UpdateGender { account_id: ross.account_id, author_id: ross.id, gender_id: gender.id, name: String::new() }
            .execute(&db.conn)
            .await
            .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let gender = factory::gender(&db.conn, ross.account_id).await;
    assert_eq!(update_gender(&db, &ross, account.id, &gender).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_gender_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let gender = factory::gender(&db.conn, other.id).await;
    assert_eq!(update_gender(&db, &ross, ross.account_id, &gender).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let gender = factory::gender(&db.conn, ross.account_id).await;
    assert_eq!(update_gender(&db, &ross, ross.account_id, &gender).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyGenderTest

async fn destroy_gender(
    db: &TestDb,
    author: &crate::entities::users::Model,
    account_id: Uuid,
    gender: &genders::Model,
) -> ApiResult<()> {
    DestroyGender { account_id, author_id: author.id, gender_id: gender.id }.execute(&db.conn).await?;
    assert!(genders::Entity::find_by_id(gender.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_gender() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let gender = factory::gender(&db.conn, ross.account_id).await;
    destroy_gender(&db, &ross, ross.account_id, &gender).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let gender = factory::gender(&db.conn, ross.account_id).await;
    assert_eq!(destroy_gender(&db, &ross, account.id, &gender).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_gender_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let gender = factory::gender(&db.conn, other.id).await;
    assert_eq!(destroy_gender(&db, &ross, ross.account_id, &gender).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let gender = factory::gender(&db.conn, ross.account_id).await;
    assert_eq!(destroy_gender(&db, &ross, ross.account_id, &gender).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}
