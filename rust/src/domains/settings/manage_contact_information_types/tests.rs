//! `tests/Unit/Domains/Settings/ManageContactInformationTypes/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::testing::{TestDb, factory};

/// `ContactInformationType::factory()` in `account_id`.
async fn contact_information_type(db: &TestDb, account_id: Uuid) -> contact_information_types::Model {
    contact_information_types::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        protocol: Set(Some("mailto:".into())),
        can_be_deleted: Set(false),
        r#type: Set(Some("email".into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("contact information type")
}

// ---- CreateContactInformationTypeTest

async fn create_contact_information_type(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let info_type = CreateContactInformationType {
        account_id,
        author_id: author.id,
        r#type: "IMPP".into(),
        name: Some("type name".into()),
        name_translation_key: None,
        protocol: None,
    }
    .execute(&db.conn)
    .await?;

    let row = contact_information_types::Entity::find_by_id(info_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_contact_information_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_contact_information_type(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreateContactInformationType {
        account_id: ross.account_id,
        author_id: ross.id,
        r#type: String::new(),
        name: None,
        name_translation_key: None,
        protocol: None,
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
    assert_eq!(create_contact_information_type(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    assert_eq!(create_contact_information_type(&db, &ross, ross.account_id).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- DestroyContactInformationTypeTest

async fn destroy_contact_information_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    info_type: &contact_information_types::Model,
) -> ApiResult<()> {
    DestroyContactInformationType { account_id, author_id: author.id, contact_information_type_id: info_type.id }
        .execute(&db.conn)
        .await?;
    assert!(contact_information_types::Entity::find_by_id(info_type.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    // F18: the factory's type can't be deleted.
    let mut info_type: contact_information_types::ActiveModel =
        contact_information_type(&db, ross.account_id).await.into();
    info_type.can_be_deleted = Set(true);
    let info_type = info_type.update(&db.conn).await.unwrap();
    destroy_contact_information_type(&db, &ross, ross.account_id, &info_type).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let info_type = contact_information_type(&db, ross.account_id).await;
    assert_eq!(
        destroy_contact_information_type(&db, &ross, account.id, &info_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let info_type = contact_information_type(&db, other.id).await;
    assert_eq!(
        destroy_contact_information_type(&db, &ross, ross.account_id, &info_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let info_type = contact_information_type(&db, ross.account_id).await;
    assert_eq!(
        destroy_contact_information_type(&db, &ross, ross.account_id, &info_type).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

/// F18: not in the PHP tests.
#[tokio::test]
async fn destroy_fails_if_type_cannot_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let info_type = contact_information_type(&db, ross.account_id).await;
    assert_eq!(
        destroy_contact_information_type(&db, &ross, ross.account_id, &info_type).await.unwrap_err().code,
        Code::Conflict
    );
    assert!(contact_information_types::Entity::find_by_id(info_type.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}

// ---- UpdateContactInformationTypeTest

async fn update_contact_information_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    info_type: &contact_information_types::Model,
) -> ApiResult<()> {
    let info_type = UpdateContactInformationType {
        account_id,
        author_id: author.id,
        contact_information_type_id: info_type.id,
        r#type: info_type.r#type.clone().unwrap(),
        name: "type name".into(),
        protocol: None,
    }
    .execute(&db.conn)
    .await?;

    let row = contact_information_types::Entity::find_by_id(info_type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("type name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let info_type = contact_information_type(&db, ross.account_id).await;
    update_contact_information_type(&db, &ross, ross.account_id, &info_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let info_type = contact_information_type(&db, ross.account_id).await;
    let err = UpdateContactInformationType {
        account_id: ross.account_id,
        author_id: ross.id,
        contact_information_type_id: info_type.id,
        r#type: "email".into(),
        name: String::new(),
        protocol: None,
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
    let info_type = contact_information_type(&db, ross.account_id).await;
    assert_eq!(
        update_contact_information_type(&db, &ross, account.id, &info_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let info_type = contact_information_type(&db, other.id).await;
    assert_eq!(
        update_contact_information_type(&db, &ross, ross.account_id, &info_type).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let info_type = contact_information_type(&db, ross.account_id).await;
    assert_eq!(
        update_contact_information_type(&db, &ross, ross.account_id, &info_type).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}
