//! `tests/Unit/Domains/Contact/ManageLabels/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::*;
use crate::crypto::random_string;
use crate::entities::vaults;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Contact::factory()->create()`: a contact in a new vault.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `Label::factory()` in `vault_id` (a new vault when `None`).
async fn label(db: &TestDb, vault_id: Option<Uuid>) -> labels::Model {
    let vault_id = match vault_id {
        Some(id) => id,
        None => factory::create_vault(&db.conn, factory::create_account(&db.conn).await.id).await.id,
    };
    labels::ActiveModel {
        vault_id: Set(vault_id),
        name: Set(format!("word{}", random_string(6))),
        slug: Set(format!("word{}", random_string(6))),
        description: Set(Some(format!("Sentence {}.", random_string(8)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("label")
}

/// `$contact->labels()->syncWithoutDetaching($label)`.
async fn attach(db: &TestDb, contact: &contacts::Model, label: &labels::Model) {
    contact_label::Entity::insert(contact_label::ActiveModel {
        label_id: Set(label.id),
        contact_id: Set(contact.id),
        ..Default::default()
    })
    .exec_without_returning(&db.conn)
    .await
    .expect("contact_label");
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

async fn is_attached(db: &TestDb, contact: &contacts::Model, label: &labels::Model) -> bool {
    contact_label::Entity::find()
        .filter(contact_label::Column::ContactId.eq(contact.id))
        .filter(contact_label::Column::LabelId.eq(label.id))
        .one(&db.conn)
        .await
        .unwrap()
        .is_some()
}

// ---------------------------------------------------------------- AssignLabelTest

async fn assign_label(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    label: &labels::Model,
) -> ApiResult<()> {
    AssignLabel { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id, label_id: label.id }
        .execute(&db.conn)
        .await?;

    assert!(is_attached(db, contact, label).await);
    assert!(has_feed_item(db, contact, ACTION_LABEL_ASSIGNED).await);
    Ok(())
}

#[tokio::test]
async fn it_assigns_a_label() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let label = label(&db, Some(vault.id)).await;
    assign_label(&db, &regis, regis.account_id, &vault, &contact, &label).await.unwrap();
    db.drop().await;
}

// assign_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn assign_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let label = label(&db, Some(vault.id)).await;
    let err = assign_label(&db, &regis, account.id, &vault, &contact, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn assign_fails_if_label_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let label = label(&db, None).await;
    let err = assign_label(&db, &regis, regis.account_id, &vault, &contact, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn assign_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let label = label(&db, Some(vault.id)).await;
    let err = assign_label(&db, &regis, regis.account_id, &vault, &contact, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn assign_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let label = label(&db, Some(vault.id)).await;
    let err = assign_label(&db, &regis, regis.account_id, &vault, &contact, &label).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- RemoveLabelTest

async fn remove_label(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    label: &labels::Model,
) -> ApiResult<()> {
    RemoveLabel { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id, label_id: label.id }
        .execute(&db.conn)
        .await?;

    assert!(!is_attached(db, contact, label).await);
    assert!(has_feed_item(db, contact, ACTION_LABEL_REMOVED).await);
    Ok(())
}

#[tokio::test]
async fn it_removes_a_label() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let label = label(&db, Some(vault.id)).await;
    attach(&db, &contact, &label).await;
    remove_label(&db, &regis, regis.account_id, &vault, &contact, &label).await.unwrap();
    db.drop().await;
}

// remove_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn remove_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let label = label(&db, Some(vault.id)).await;
    attach(&db, &contact, &label).await;
    let err = remove_label(&db, &regis, account.id, &vault, &contact, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_label_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let label = label(&db, None).await;
    attach(&db, &contact, &label).await;
    let err = remove_label(&db, &regis, regis.account_id, &vault, &contact, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let label = label(&db, Some(vault.id)).await;
    attach(&db, &contact, &label).await;
    let err = remove_label(&db, &regis, regis.account_id, &vault, &contact, &label).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let label = label(&db, Some(vault.id)).await;
    attach(&db, &contact, &label).await;
    let err = remove_label(&db, &regis, regis.account_id, &vault, &contact, &label).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}
