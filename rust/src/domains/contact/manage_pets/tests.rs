//! `tests/Unit/Domains/Contact/ManagePets/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::*;
use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Contact::factory()->create()`: a contact in a new vault.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `PetCategory::factory()` in `account_id` (a new account when `None`).
async fn pet_category(db: &TestDb, account_id: Option<Uuid>) -> pet_categories::Model {
    let account_id = match account_id {
        Some(id) => id,
        None => factory::create_account(&db.conn).await.id,
    };
    pet_categories::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("pet category")
}

/// `Pet::factory()`: a new contact and category unless given.
async fn pet(db: &TestDb, contact_id: Option<Uuid>, pet_category_id: Option<Id>) -> pets::Model {
    let pet_category_id = match pet_category_id {
        Some(id) => id,
        None => pet_category(db, None).await.id,
    };
    let contact_id = match contact_id {
        Some(id) => id,
        None => other_contact(db).await.id,
    };
    pets::ActiveModel {
        pet_category_id: Set(pet_category_id),
        contact_id: Set(contact_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("pet")
}

/// `createUser()`, `createVault()` and `setPermissionInVault()` with `permission`.
async fn regis_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

async fn has_feed_item(db: &TestDb, contact: &contacts::Model, action: &str) -> bool {
    contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(contact.id))
        .filter(contact_feed_items::Column::Action.eq(action))
        .one(&db.conn)
        .await
        .unwrap()
        .is_some()
}

// ---------------------------------------------------------------- CreatePetTest

async fn create_pet(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    pet_category: &pet_categories::Model,
) -> ApiResult<()> {
    let pet = CreatePet {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        pet_category_id: pet_category.id,
        name: Some("boubou".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = pets::Entity::find_by_id(pet.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.pet_category_id, pet_category.id);
    assert_eq!(row.name.as_deref(), Some("boubou"));

    assert!(has_feed_item(db, contact, ACTION_PET_CREATED).await);
    Ok(())
}

#[tokio::test]
async fn it_creates_a_pet() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    create_pet(&db, &regis, regis.account_id, &vault, &contact, &pet_category).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let err = CreatePet {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        pet_category_id: pet_category.id,
        name: Some("x".repeat(256)),
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
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let err = create_pet(&db, &regis, account.id, &vault, &contact, &pet_category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let err = create_pet(&db, &regis, regis.account_id, &vault, &contact, &pet_category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let err = create_pet(&db, &regis, regis.account_id, &vault, &contact, &pet_category).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_pet_category_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, None).await;
    let err = create_pet(&db, &regis, regis.account_id, &vault, &contact, &pet_category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyPetTest

async fn destroy_pet(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    pet: &pets::Model,
) -> ApiResult<()> {
    DestroyPet { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id, pet_id: pet.id }
        .execute(&db.conn)
        .await?;

    assert!(pets::Entity::find_by_id(pet.id).one(&db.conn).await.unwrap().is_none());
    assert!(has_feed_item(db, contact, ACTION_PET_DESTROYED).await);
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_pet() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet = pet(&db, Some(contact.id), None).await;
    destroy_pet(&db, &regis, regis.account_id, &vault, &contact, &pet).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet = pet(&db, Some(contact.id), None).await;
    let err = destroy_pet(&db, &regis, account.id, &vault, &contact, &pet).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let pet = pet(&db, Some(contact.id), None).await;
    let err = destroy_pet(&db, &regis, regis.account_id, &vault, &contact, &pet).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet = pet(&db, Some(contact.id), None).await;
    let err = destroy_pet(&db, &regis, regis.account_id, &vault, &contact, &pet).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_pet_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet = pet(&db, None, None).await;
    let err = destroy_pet(&db, &regis, regis.account_id, &vault, &contact, &pet).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdatePetTest

async fn update_pet(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    pet: &pets::Model,
    pet_category: &pet_categories::Model,
) -> ApiResult<()> {
    let pet = UpdatePet {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        pet_id: pet.id,
        pet_category_id: pet_category.id,
        name: Some("boubou".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = pets::Entity::find_by_id(pet.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.pet_category_id, pet_category.id);
    assert_eq!(row.name.as_deref(), Some("boubou"));

    assert!(has_feed_item(db, contact, ACTION_PET_UPDATED).await);
    Ok(())
}

#[tokio::test]
async fn it_updates_a_pet() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let pet = pet(&db, Some(contact.id), Some(pet_category.id)).await;
    update_pet(&db, &regis, regis.account_id, &vault, &contact, &pet, &pet_category).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let pet = pet(&db, Some(contact.id), Some(pet_category.id)).await;
    let err = UpdatePet {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        pet_id: pet.id,
        pet_category_id: pet_category.id,
        name: Some("x".repeat(256)),
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
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let pet = pet(&db, Some(contact.id), Some(pet_category.id)).await;
    let err = update_pet(&db, &regis, account.id, &vault, &contact, &pet, &pet_category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let pet = pet(&db, Some(contact.id), Some(pet_category.id)).await;
    let err = update_pet(&db, &regis, regis.account_id, &vault, &contact, &pet, &pet_category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let pet = pet(&db, Some(contact.id), Some(pet_category.id)).await;
    let err = update_pet(&db, &regis, regis.account_id, &vault, &contact, &pet, &pet_category).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_pet_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, Some(regis.account_id)).await;
    let pet = pet(&db, None, Some(pet_category.id)).await;
    let err = update_pet(&db, &regis, regis.account_id, &vault, &contact, &pet, &pet_category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_pet_category_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pet_category = pet_category(&db, None).await;
    let pet = pet(&db, Some(contact.id), Some(pet_category.id)).await;
    let err = update_pet(&db, &regis, regis.account_id, &vault, &contact, &pet, &pet_category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

/// F7: the category can change (the PHP looked the pet up by the new one).
#[tokio::test]
async fn update_changes_the_pet_category() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let old_category = pet_category(&db, Some(regis.account_id)).await;
    let new_category = pet_category(&db, Some(regis.account_id)).await;
    let pet = pet(&db, Some(contact.id), Some(old_category.id)).await;
    update_pet(&db, &regis, regis.account_id, &vault, &contact, &pet, &new_category).await.unwrap();

    let feed_item = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(contact.id))
        .filter(contact_feed_items::Column::Action.eq(ACTION_PET_UPDATED))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(feed_item.description, new_category.name);
    db.drop().await;
}
