//! `tests/Unit/Domains/Settings/ManageAddressTypes/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `AddressType::factory()` in `account_id`.
async fn address_type(db: &TestDb, account_id: Uuid) -> address_types::Model {
    const LIST: [&str; 3] = ["home", "work", "other"];
    address_types::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        r#type: Set(Some(LIST[rand::random_range(0..LIST.len())].into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("address type")
}

// ---- CreateAddressTypeTest

async fn create_address_type(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let address_type = CreateAddressType {
        account_id,
        author_id: author.id,
        name: Some("type name".into()),
        name_translation_key: None,
        r#type: None,
    }
    .execute(&db.conn)
    .await?;

    let row = address_types::Entity::find_by_id(address_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_contact_information_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_address_type(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateAddressType {
        account_id: ross.account_id,
        author_id: ross.id,
        name: Some("x".repeat(256)),
        name_translation_key: None,
        r#type: None,
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
    assert_eq!(create_address_type(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    assert_eq!(create_address_type(&db, &ross, ross.account_id).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- DestroyAddressTypeTest

async fn destroy_address_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    address_type: &address_types::Model,
) -> ApiResult<()> {
    DestroyAddressType { account_id, author_id: author.id, address_type_id: address_type.id }.execute(&db.conn).await?;
    assert!(address_types::Entity::find_by_id(address_type.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let address_type = address_type(&db, ross.account_id).await;
    destroy_address_type(&db, &ross, ross.account_id, &address_type).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let address_type = address_type(&db, ross.account_id).await;
    assert_eq!(destroy_address_type(&db, &ross, account.id, &address_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let address_type = address_type(&db, other.id).await;
    assert_eq!(
        destroy_address_type(&db, &ross, ross.account_id, &address_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let address_type = address_type(&db, ross.account_id).await;
    assert_eq!(
        destroy_address_type(&db, &ross, ross.account_id, &address_type).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

// ---- UpdateAddressTypeTest

async fn update_address_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    address_type: &address_types::Model,
) -> ApiResult<()> {
    let address_type = UpdateAddressType {
        account_id,
        author_id: author.id,
        address_type_id: address_type.id,
        name: "type name".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = address_types::Entity::find_by_id(address_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let address_type = address_type(&db, ross.account_id).await;
    update_address_type(&db, &ross, ross.account_id, &address_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let address_type = address_type(&db, ross.account_id).await;
    let err = UpdateAddressType {
        account_id: ross.account_id,
        author_id: ross.id,
        address_type_id: address_type.id,
        name: String::new(),
    }
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
    let address_type = address_type(&db, ross.account_id).await;
    assert_eq!(update_address_type(&db, &ross, account.id, &address_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let address_type = address_type(&db, other.id).await;
    assert_eq!(update_address_type(&db, &ross, ross.account_id, &address_type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let address_type = address_type(&db, ross.account_id).await;
    assert_eq!(
        update_address_type(&db, &ross, ross.account_id, &address_type).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}
