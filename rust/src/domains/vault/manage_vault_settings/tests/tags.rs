//! `CreateTagTest`, `UpdateTagTest` and `DestroyTagTest`.

use super::super::*;
use super::any_vault;
use crate::crypto::random_string;
use crate::entities::tags;
use crate::testing::{TestDb, factory};

/// `Tag::factory()` in `vault_id`, to insert.
fn tag(vault_id: Uuid) -> tags::ActiveModel {
    tags::ActiveModel {
        vault_id: Set(vault_id),
        name: Set(format!("Name {}", random_string(6))),
        slug: Set(format!("Name {}", random_string(6))),
        ..Default::default()
    }
}

// ---------------------------------------------------------------- CreateTagTest

async fn create_tag(db: &TestDb, author: &users::Model, account_id: Uuid, vault: &vaults::Model) -> ApiResult<()> {
    let tag = CreateTag { account_id, author_id: author.id, vault_id: vault.id, name: "tag name".into() }
        .execute(&db.conn)
        .await?;

    let row = tags::Entity::find_by_id(tag.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.name, "tag name");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_tag() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    create_tag(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_tag_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let err = CreateTag { account_id: ross.account_id, author_id: ross.id, vault_id: vault.id, name: String::new() }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_tag_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = create_tag(&db, &ross, account.id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_tag_fails_if_user_is_not_editor() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_tag(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_tag_fails_if_vault_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = any_vault(&db).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_tag(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateTagTest

async fn update_tag(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    tag: &tags::Model,
) -> ApiResult<()> {
    let tag =
        UpdateTag { account_id, author_id: author.id, vault_id: vault.id, tag_id: tag.id, name: "label name".into() }
            .execute(&db.conn)
            .await?;

    let row = tags::Entity::find_by_id(tag.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.name, "label name");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_tag() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let tag = tag(vault.id).insert(&db.conn).await.unwrap();
    update_tag(&db, &ross, ross.account_id, &vault, &tag).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_tag_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let tag = tag(vault.id).insert(&db.conn).await.unwrap();
    let err = UpdateTag {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        tag_id: tag.id,
        name: "x".repeat(256),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_tag_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let tag = tag(vault.id).insert(&db.conn).await.unwrap();
    let err = update_tag(&db, &ross, account.id, &vault, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_tag_fails_if_label_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let tag = tag(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = update_tag(&db, &ross, ross.account_id, &vault, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_tag_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let tag = tag(vault.id).insert(&db.conn).await.unwrap();
    let err = update_tag(&db, &ross, ross.account_id, &vault, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyTagTest

async fn destroy_tag(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    tag: &tags::Model,
) -> ApiResult<()> {
    DestroyTag { account_id, author_id: author.id, tag_id: tag.id, vault_id: vault.id }.execute(&db.conn).await?;

    assert!(tags::Entity::find_by_id(tag.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_tag() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let tag = tag(vault.id).insert(&db.conn).await.unwrap();
    destroy_tag(&db, &ross, ross.account_id, &vault, &tag).await.unwrap();
    db.drop().await;
}

// destroy_tag_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_tag_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let tag = tag(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_tag(&db, &ross, account.id, &vault, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_tag_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let tag = tag(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy_tag(&db, &ross, account.id, &vault, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_tag_fails_if_label_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let tag = tag(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy_tag(&db, &ross, ross.account_id, &vault, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_tag_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let tag = tag(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_tag(&db, &ross, ross.account_id, &vault, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}
