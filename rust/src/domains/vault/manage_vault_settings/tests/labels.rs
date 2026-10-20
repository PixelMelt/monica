//! `CreateLabelTest`, `UpdateLabelTest` and `DestroyLabelTest`.

use super::super::*;
use super::any_vault;
use crate::crypto::random_string;
use crate::entities::labels;
use crate::testing::{TestDb, factory};

/// `Label::factory()` in `vault_id`, to insert.
fn label(vault_id: Uuid) -> labels::ActiveModel {
    labels::ActiveModel {
        vault_id: Set(vault_id),
        name: Set(random_string(8)),
        slug: Set(random_string(8)),
        description: Set(Some(format!("Sentence {}.", random_string(12)))),
        ..Default::default()
    }
}

// ---------------------------------------------------------------- CreateLabelTest

async fn create_label(db: &TestDb, author: &users::Model, account_id: Uuid, vault: &vaults::Model) -> ApiResult<()> {
    let label = CreateLabel {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        name: "label name".into(),
        description: None,
        bg_color: Some("bg-zinc-700".into()),
        text_color: Some("bg-zinc-700".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = labels::Entity::find_by_id(label.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.name, "label name");
    assert_eq!(row.bg_color, "bg-zinc-700");
    assert_eq!(row.text_color, "bg-zinc-700");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_label() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    create_label(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_label_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let err = CreateLabel {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        name: String::new(),
        description: None,
        bg_color: None,
        text_color: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_label_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = create_label(&db, &ross, account.id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_label_fails_if_user_is_not_editor() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_label(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_label_fails_if_vault_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = any_vault(&db).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_label(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

/// Not in the PHP tests: the slug `Str::slug` gives for the name.
#[tokio::test]
async fn create_label_slugs_the_name() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let label = CreateLabel {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        name: " My_Label  name@home!".into(),
        description: Some(String::new()),
        bg_color: None,
        text_color: None,
    }
    .execute(&db.conn)
    .await
    .unwrap();
    let row = labels::Entity::find_by_id(label.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.slug, "my-label-name-at-home");
    assert_eq!(row.description, None);
    assert_eq!(row.bg_color, "bg-neutral-200");
    assert_eq!(row.text_color, "text-neutral-800");
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateLabelTest

async fn update_label(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    label: &labels::Model,
) -> ApiResult<()> {
    let label = UpdateLabel {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        label_id: label.id,
        name: "label name".into(),
        description: None,
        bg_color: "bg-zinc-700".into(),
        text_color: "bg-zinc-700".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = labels::Entity::find_by_id(label.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.name, "label name");
    assert_eq!(row.bg_color, "bg-zinc-700");
    assert_eq!(row.text_color, "bg-zinc-700");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_label() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let label = label(vault.id).insert(&db.conn).await.unwrap();
    update_label(&db, &ross, ross.account_id, &vault, &label).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_label_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let label = label(vault.id).insert(&db.conn).await.unwrap();
    let err = UpdateLabel {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        label_id: label.id,
        name: String::new(),
        description: None,
        bg_color: "bg-zinc-700".into(),
        text_color: "bg-zinc-700".into(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_label_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let label = label(vault.id).insert(&db.conn).await.unwrap();
    let err = update_label(&db, &ross, account.id, &vault, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_label_fails_if_label_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let label = label(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = update_label(&db, &ross, ross.account_id, &vault, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_label_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let label = label(vault.id).insert(&db.conn).await.unwrap();
    let err = update_label(&db, &ross, ross.account_id, &vault, &label).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyLabelTest

async fn destroy_label(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    label: &labels::Model,
) -> ApiResult<()> {
    DestroyLabel { account_id, author_id: author.id, label_id: label.id, vault_id: vault.id }.execute(&db.conn).await?;

    assert!(labels::Entity::find_by_id(label.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_label() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let label = label(vault.id).insert(&db.conn).await.unwrap();
    destroy_label(&db, &ross, ross.account_id, &vault, &label).await.unwrap();
    db.drop().await;
}

// destroy_label_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_label_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let label = label(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_label(&db, &ross, account.id, &vault, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_label_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let label = label(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy_label(&db, &ross, account.id, &vault, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_label_fails_if_label_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let label = label(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy_label(&db, &ross, ross.account_id, &vault, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_label_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let label = label(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_label(&db, &ross, ross.account_id, &vault, &label).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}
