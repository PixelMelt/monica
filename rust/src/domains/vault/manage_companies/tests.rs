//! `tests/Unit/Domains/Vault/ManageCompanies/Services`.

use sea_orm::EntityTrait;

use super::*;
use crate::domains::base::VaultPermission;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

// ---------------------------------------------------------------- CreateCompanyTest

async fn execute_service(db: &TestDb, author: &users::Model, account_id: Uuid, vault: &vaults::Model) -> ApiResult<()> {
    let company = CreateCompany {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        name: "company name".into(),
        r#type: Some("association".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = companies::Entity::find_by_id(company.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.name, "company name");
    assert_eq!(row.r#type.as_deref(), Some("association"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_company() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    execute_service(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = CreateCompany {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        name: String::new(),
        r#type: None,
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
    assert_eq!(execute_service(&db, &ross, account.id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_is_not_editor() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    assert_eq!(execute_service(&db, &ross, ross.account_id, &vault).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_vault_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    assert_eq!(execute_service(&db, &ross, ross.account_id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}
