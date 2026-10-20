//! `tests/Unit/Domains/Contact/ManageJobInformation/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::vaults;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Company::factory()` in `vault_id`.
async fn company(db: &TestDb, vault_id: Uuid) -> companies::Model {
    companies::ActiveModel {
        vault_id: Set(vault_id),
        name: Set(format!("Name {}", random_string(6))),
        r#type: Set(Some("company".into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("company")
}

/// `Vault::factory()->create()`.
async fn vault_elsewhere(db: &TestDb) -> vaults::Model {
    let account = factory::account(&db.conn).await;
    factory::vault(&db.conn, account.id).await
}

/// `createVault($user->account)` then `setPermissionInVault($user, $permission, $vault)`.
async fn vault_for(db: &TestDb, user: &users::Model, permission: VaultPermission) -> vaults::Model {
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, user, permission, &vault).await;
    vault
}

/// `Contact::factory()->create(['vault_id' => …, 'company_id' => …, 'job_position' => 'tester'])`.
async fn employee(db: &TestDb, vault: &vaults::Model, company: &companies::Model) -> contacts::Model {
    let contact = factory::contact(&db.conn, vault).await;
    let mut contact: contacts::ActiveModel = contact.into();
    contact.company_id = Set(Some(company.id));
    contact.job_position = Set(Some("tester".into()));
    contact.update(&db.conn).await.unwrap()
}

async fn assert_job(db: &TestDb, contact: &contacts::Model, company_id: Option<Id>, job_position: Option<&str>) {
    let row = contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.company_id, company_id);
    assert_eq!(row.job_position.as_deref(), job_position);
}

// ---------------------------------------------------------------- ResetJobInformationTest

async fn reset_job(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    ResetJobInformation { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id }
        .execute(&db.conn)
        .await?;

    assert_job(db, contact, None, None).await;
    Ok(())
}

#[tokio::test]
async fn it_resets_the_job_information() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let company = company(&db, vault.id).await;
    let contact = employee(&db, &vault, &company).await;
    reset_job(&db, &regis, regis.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

// it_fails_if_wrong_parameters_are_given: every field is typed, no invalid
// input can be built.

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let company = company(&db, vault.id).await;
    let contact = employee(&db, &vault, &company).await;
    let err = reset_job(&db, &regis, account.id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let company = company(&db, vault.id).await;
    let contact = employee(&db, &vault_elsewhere(&db).await, &company).await;
    let err = reset_job(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let company = company(&db, vault.id).await;
    let contact = employee(&db, &vault, &company).await;
    let err = reset_job(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateJobInformationTest

async fn update_job(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    company: &companies::Model,
    job_position: &str,
) -> ApiResult<()> {
    let service = |company_id: Option<Id>, job_position: Option<String>| UpdateJobInformation {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        company_id,
        job_position,
    };

    let contact = service(Some(company.id), Some(job_position.into())).execute(&db.conn).await?;
    assert_job(db, &contact, Some(company.id), Some(job_position)).await;

    // a second time with null value, to see if it still works, as it should
    let contact = service(None, None).execute(&db.conn).await?;
    assert_job(db, &contact, None, None).await;
    Ok(())
}

#[tokio::test]
async fn it_updates_the_job_information() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let company = company(&db, vault.id).await;
    update_job(&db, &regis, regis.account_id, &vault, &contact, &company, "developer").await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = UpdateJobInformation {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        company_id: None,
        job_position: Some("x".repeat(256)),
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
    let regis = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let company = company(&db, vault.id).await;
    let err = update_job(&db, &regis, account.id, &vault, &contact, &company, "developer").await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault_elsewhere(&db).await).await;
    let company = company(&db, vault.id).await;
    let err = update_job(&db, &regis, regis.account_id, &vault, &contact, &company, "developer").await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let company = company(&db, vault.id).await;
    let err = update_job(&db, &regis, regis.account_id, &vault, &contact, &company, "developer").await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_company_is_not_in_the_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let company = company(&db, vault_elsewhere(&db).await.id).await;
    let err = update_job(&db, &regis, regis.account_id, &vault, &contact, &company, "developer").await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
