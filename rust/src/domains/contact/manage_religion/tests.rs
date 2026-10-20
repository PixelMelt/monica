//! `tests/Unit/Domains/Contact/ManageReligion/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Religion::factory()` in `account_id`.
async fn religion(db: &TestDb, account_id: Uuid) -> religions::Model {
    religions::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        translation_key: Set(Some(random_string(6).to_lowercase())),
        position: Set(Some(1)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("religion")
}

/// `createVault($user->account)` then `setPermissionInVault($user, $permission, $vault)`.
async fn vault_for(db: &TestDb, user: &users::Model, permission: VaultPermission) -> vaults::Model {
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, user, permission, &vault).await;
    vault
}

async fn update_religion(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    religion: Option<&religions::Model>,
) -> ApiResult<()> {
    let contact = UpdateReligion {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        religion_id: religion.map(|r| r.id),
    }
    .execute(&db.conn)
    .await?;

    let row = contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.religion_id, religion.map(|r| r.id));
    Ok(())
}

#[tokio::test]
async fn it_updates_the_religion() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let religion = religion(&db, regis.account_id).await;
    update_religion(&db, &regis, regis.account_id, &vault, &contact, Some(&religion)).await.unwrap();

    // F10: a null religion clears it (the PHP answered 404).
    update_religion(&db, &regis, regis.account_id, &vault, &contact, None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = UpdateReligion {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        religion_id: Some(Id(i64::MAX)),
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
    let regis = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let religion = religion(&db, regis.account_id).await;
    let err = update_religion(&db, &regis, account.id, &vault, &contact, Some(&religion)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let other = factory::account(&db.conn).await;
    let contact = factory::contact(&db.conn, &factory::vault(&db.conn, other.id).await).await;
    let religion = religion(&db, regis.account_id).await;
    let err = update_religion(&db, &regis, regis.account_id, &vault, &contact, Some(&religion)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let religion = religion(&db, regis.account_id).await;
    let err = update_religion(&db, &regis, regis.account_id, &vault, &contact, Some(&religion)).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_religion_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let religion = religion(&db, factory::account(&db.conn).await.id).await;
    let err = update_religion(&db, &regis, regis.account_id, &vault, &contact, Some(&religion)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
