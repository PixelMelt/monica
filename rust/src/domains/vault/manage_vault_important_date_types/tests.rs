//! `tests/Unit/Domains/Vault/ManageVaultImportantDateTypes/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};

use super::*;
use crate::entities::{users, vaults};
use crate::testing::{TestDb, factory};

/// `ContactImportantDateType::factory()` in `vault_id` (a new vault when `None`).
async fn contact_important_date_type(db: &TestDb, vault_id: Option<Uuid>) -> contact_important_date_types::Model {
    let vault_id = match vault_id {
        Some(v) => v,
        None => factory::vault(&db.conn, factory::account(&db.conn).await.id).await.id,
    };
    contact_important_date_types::ActiveModel {
        vault_id: Set(vault_id),
        label: Set("Birthdate".into()),
        internal_type: Set(Some("birthdate".into())),
        can_be_deleted: Set(true),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("contact important date type")
}

// ---------------------------------------------------------------- CreateContactImportantDateTypeTest

async fn create_type(db: &TestDb, author: &users::Model, account_id: Uuid, vault: &vaults::Model) -> ApiResult<()> {
    let r#type = CreateContactImportantDateType {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        label: "label name".into(),
        internal_type: None,
        can_be_deleted: Some(false),
    }
    .execute(&db.conn)
    .await?;

    let row = contact_important_date_types::Entity::find_by_id(r#type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label, "label name");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    create_type(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = CreateContactImportantDateType {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        label: String::new(),
        internal_type: None,
        can_be_deleted: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    assert_eq!(create_type(&db, &ross, account.id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_is_not_editor() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    assert_eq!(create_type(&db, &ross, ross.account_id, &vault).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_vault_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, factory::account(&db.conn).await.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    assert_eq!(create_type(&db, &ross, ross.account_id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyContactImportantDateTypeTest

async fn destroy_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    r#type: &contact_important_date_types::Model,
) -> ApiResult<()> {
    DestroyContactImportantDateType {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        contact_important_date_type_id: r#type.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(contact_important_date_types::Entity::find_by_id(r#type.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_label() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let r#type = contact_important_date_type(&db, Some(vault.id)).await;
    destroy_type(&db, &ross, ross.account_id, &vault, &r#type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = DestroyContactImportantDateType {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        contact_important_date_type_id: Id(0),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let r#type = contact_important_date_type(&db, Some(vault.id)).await;
    assert_eq!(destroy_type(&db, &ross, account.id, &vault, &r#type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let r#type = contact_important_date_type(&db, None).await;
    assert_eq!(destroy_type(&db, &ross, account.id, &vault, &r#type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_type_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let r#type = contact_important_date_type(&db, None).await;
    assert_eq!(destroy_type(&db, &ross, ross.account_id, &vault, &r#type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let r#type = contact_important_date_type(&db, Some(vault.id)).await;
    assert_eq!(destroy_type(&db, &ross, ross.account_id, &vault, &r#type).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateContactImportantDateTypeTest

async fn update_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    r#type: &contact_important_date_types::Model,
) -> ApiResult<()> {
    let r#type = UpdateContactImportantDateType {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        contact_important_date_type_id: r#type.id,
        label: "label name".into(),
        internal_type: None,
        can_be_deleted: Some(false),
    }
    .execute(&db.conn)
    .await?;

    let row = contact_important_date_types::Entity::find_by_id(r#type.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label, "label name");
    assert!(!row.can_be_deleted);
    Ok(())
}

#[tokio::test]
async fn it_updates_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let r#type = contact_important_date_type(&db, Some(vault.id)).await;
    update_type(&db, &ross, ross.account_id, &vault, &r#type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let r#type = contact_important_date_type(&db, Some(vault.id)).await;
    let err = UpdateContactImportantDateType {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        contact_important_date_type_id: r#type.id,
        label: String::new(),
        internal_type: None,
        can_be_deleted: None,
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
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let r#type = contact_important_date_type(&db, Some(vault.id)).await;
    assert_eq!(update_type(&db, &ross, account.id, &vault, &r#type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_type_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let r#type = contact_important_date_type(&db, None).await;
    assert_eq!(update_type(&db, &ross, ross.account_id, &vault, &r#type).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let r#type = contact_important_date_type(&db, Some(vault.id)).await;
    assert_eq!(update_type(&db, &ross, ross.account_id, &vault, &r#type).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}
