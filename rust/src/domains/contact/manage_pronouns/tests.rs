//! `tests/Unit/Domains/Contact/ManagePronouns/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Pronoun::factory()` in `account_id`.
async fn pronoun(db: &TestDb, account_id: Uuid) -> pronouns::Model {
    pronouns::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("pronoun")
}

/// `createVault($user->account)` then `setPermissionInVault($user, $permission, $vault)`.
async fn vault_for(db: &TestDb, user: &users::Model, permission: VaultPermission) -> vaults::Model {
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, user, permission, &vault).await;
    vault
}

/// `Contact::factory()->create()`: in a vault of another account.
async fn contact_elsewhere(db: &TestDb) -> contacts::Model {
    let account = factory::account(&db.conn).await;
    factory::contact(&db.conn, &factory::vault(&db.conn, account.id).await).await
}

async fn pronoun_of(db: &TestDb, contact: &contacts::Model) -> Option<Id> {
    contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().unwrap().pronoun_id
}

// ---------------------------------------------------------------- RemovePronounTest

async fn remove_pronoun(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    RemovePronoun { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id }
        .execute(&db.conn)
        .await?;

    assert_eq!(pronoun_of(db, contact).await, None);
    Ok(())
}

#[tokio::test]
async fn it_unsets_pronoun() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    remove_pronoun(&db, &regis, regis.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    // The PHP test calls SetPronoun without a pronoun.
    let err = SetPronoun {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        pronoun_id: Id(i64::MAX),
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
    let err = remove_pronoun(&db, &regis, account.id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let err = remove_pronoun(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = remove_pronoun(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- SetPronounTest

async fn set_pronoun(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    pronoun: &pronouns::Model,
) -> ApiResult<()> {
    SetPronoun { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id, pronoun_id: pronoun.id }
        .execute(&db.conn)
        .await?;

    assert_eq!(pronoun_of(db, contact).await, Some(pronoun.id));
    Ok(())
}

#[tokio::test]
async fn it_sets_pronoun() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pronoun = pronoun(&db, regis.account_id).await;
    set_pronoun(&db, &regis, regis.account_id, &vault, &contact, &pronoun).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn set_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = SetPronoun {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        pronoun_id: Id(i64::MAX),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn set_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pronoun = pronoun(&db, regis.account_id).await;
    let err = set_pronoun(&db, &regis, account.id, &vault, &contact, &pronoun).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn set_fails_if_pronoun_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pronoun = pronoun(&db, factory::account(&db.conn).await.id).await;
    let err = set_pronoun(&db, &regis, regis.account_id, &vault, &contact, &pronoun).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn set_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let pronoun = pronoun(&db, regis.account_id).await;
    let err = set_pronoun(&db, &regis, regis.account_id, &vault, &contact, &pronoun).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn set_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let pronoun = pronoun(&db, regis.account_id).await;
    let err = set_pronoun(&db, &regis, regis.account_id, &vault, &contact, &pronoun).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}
