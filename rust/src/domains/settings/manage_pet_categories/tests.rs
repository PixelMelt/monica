//! `tests/Unit/Domains/Settings/ManagePetCategories/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `PetCategory::factory()` in `account_id`.
async fn pet_category(db: &TestDb, account_id: Uuid) -> pet_categories::Model {
    pet_categories::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("pet category")
}

// ---- CreatePetCategoryTest

async fn create_pet_category(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let pet_category = CreatePetCategory {
        account_id,
        author_id: author.id,
        name: Some("gender name".into()),
        name_translation_key: None,
    }
    .execute(&db.conn)
    .await?;

    let row = pet_categories::Entity::find_by_id(pet_category.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("gender name"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_pet_category() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    create_pet_category(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let err = CreatePetCategory {
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
    assert_eq!(create_pet_category(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    assert_eq!(create_pet_category(&db, &ross, ross.account_id).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- DestroyPetCategoryTest

async fn destroy_pet_category(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    pet_category: &pet_categories::Model,
) -> ApiResult<()> {
    DestroyPetCategory { account_id, author_id: author.id, pet_category_id: pet_category.id }.execute(&db.conn).await?;
    assert!(pet_categories::Entity::find_by_id(pet_category.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_gender() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let pet_category = pet_category(&db, ross.account_id).await;
    destroy_pet_category(&db, &ross, ross.account_id, &pet_category).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let pet_category = pet_category(&db, ross.account_id).await;
    assert_eq!(destroy_pet_category(&db, &ross, account.id, &pet_category).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_gender_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let pet_category = pet_category(&db, other.id).await;
    assert_eq!(
        destroy_pet_category(&db, &ross, ross.account_id, &pet_category).await.unwrap_err().code,
        Code::NotFound
    );
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let pet_category = pet_category(&db, ross.account_id).await;
    assert_eq!(
        destroy_pet_category(&db, &ross, ross.account_id, &pet_category).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}

// ---- UpdatePetCategoryTest

async fn update_pet_category(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    pet_category: &pet_categories::Model,
) -> ApiResult<()> {
    let pet_category = UpdatePetCategory {
        account_id,
        author_id: author.id,
        pet_category_id: pet_category.id,
        name: "gender name".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = pet_categories::Entity::find_by_id(pet_category.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name.as_deref(), Some("gender name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_pet_category() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let pet_category = pet_category(&db, ross.account_id).await;
    update_pet_category(&db, &ross, ross.account_id, &pet_category).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let pet_category = pet_category(&db, ross.account_id).await;
    let err = UpdatePetCategory {
        account_id: ross.account_id,
        author_id: ross.id,
        pet_category_id: pet_category.id,
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
    let pet_category = pet_category(&db, ross.account_id).await;
    assert_eq!(update_pet_category(&db, &ross, account.id, &pet_category).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_gender_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let other = factory::create_account(&db.conn).await;
    let pet_category = pet_category(&db, other.id).await;
    assert_eq!(update_pet_category(&db, &ross, ross.account_id, &pet_category).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let pet_category = pet_category(&db, ross.account_id).await;
    assert_eq!(
        update_pet_category(&db, &ross, ross.account_id, &pet_category).await.unwrap_err().code,
        Code::Forbidden
    );
    db.drop().await;
}
