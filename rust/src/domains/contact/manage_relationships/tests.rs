//! `tests/Unit/Domains/Contact/ManageRelationships/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `RelationshipGroupType::factory()` in `account_id`.
async fn relationship_group_type(db: &TestDb, account_id: Uuid) -> relationship_group_types::Model {
    relationship_group_types::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("relationship group type")
}

/// `RelationshipType::factory()` in `relationship_group_type_id`, named `name`.
async fn relationship_type(
    db: &TestDb,
    relationship_group_type_id: Id,
    name: Option<String>,
) -> relationship_types::Model {
    relationship_types::ActiveModel {
        relationship_group_type_id: Set(relationship_group_type_id),
        name: Set(Some(name.unwrap_or_else(|| format!("Name {}", random_string(6))))),
        name_reverse_relationship: Set(Some(format!("Name {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("relationship type")
}

/// The type and its reverse in a group type of `account_id` (a new account
/// when `None`).
async fn types(db: &TestDb, account_id: Option<Uuid>) -> relationship_types::Model {
    let account_id = match account_id {
        Some(a) => a,
        None => factory::account(&db.conn).await.id,
    };
    let group_type = relationship_group_type(db, account_id).await;
    let r#type = relationship_type(db, group_type.id, None).await;
    relationship_type(db, group_type.id, r#type.name_reverse_relationship.clone()).await;
    r#type
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

async fn relationship(
    db: &TestDb,
    r#type: &relationship_types::Model,
    contact: &contacts::Model,
    other_contact: &contacts::Model,
) -> Option<relationships::Model> {
    relationships::Entity::find()
        .filter(relationships::Column::RelationshipTypeId.eq(r#type.id))
        .filter(relationships::Column::ContactId.eq(contact.id))
        .filter(relationships::Column::RelatedContactId.eq(other_contact.id))
        .one(&db.conn)
        .await
        .unwrap()
}

// ---------------------------------------------------------------- SetRelationshipTest

async fn set(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    r#type: &relationship_types::Model,
    vault: &vaults::Model,
    contact: &contacts::Model,
    other_contact: &contacts::Model,
) -> ApiResult<()> {
    SetRelationship {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        relationship_type_id: r#type.id,
        contact_id: contact.id,
        other_contact_id: other_contact.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(relationship(db, r#type, contact, other_contact).await.is_some());
    Ok(())
}

#[tokio::test]
async fn it_sets_relationship() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = types(&db, Some(regis.account_id)).await;
    set(&db, &regis, regis.account_id, &r#type, &vault, &contact, &other_contact).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = types(&db, Some(regis.account_id)).await;
    let err = SetRelationship {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        relationship_type_id: r#type.id,
        contact_id: contact.id,
        other_contact_id: Uuid::new(),
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
    let account = factory::account(&db.conn).await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = types(&db, Some(regis.account_id)).await;
    let err = set(&db, &regis, account.id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_relationship_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = types(&db, None).await;
    let err = set(&db, &regis, account.id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = contact_elsewhere(&db).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = types(&db, Some(regis.account_id)).await;
    let err = set(&db, &regis, regis.account_id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_other_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = contact_elsewhere(&db).await;
    let r#type = types(&db, Some(regis.account_id)).await;
    let err = set(&db, &regis, regis.account_id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = types(&db, Some(regis.account_id)).await;
    let err = set(&db, &regis, regis.account_id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UnsetRelationshipTest

/// The type and its reverse, and `$contact->relationships()->syncWithoutDetaching()`.
async fn related(
    db: &TestDb,
    account_id: Option<Uuid>,
    contact: &contacts::Model,
    other_contact: &contacts::Model,
) -> relationship_types::Model {
    let r#type = types(db, account_id).await;
    relationships::ActiveModel {
        relationship_type_id: Set(r#type.id),
        contact_id: Set(contact.id),
        related_contact_id: Set(other_contact.id),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("relationship");
    r#type
}

async fn unset(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    r#type: &relationship_types::Model,
    vault: &vaults::Model,
    contact: &contacts::Model,
    other_contact: &contacts::Model,
) -> ApiResult<()> {
    UnsetRelationship {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        relationship_type_id: r#type.id,
        contact_id: contact.id,
        other_contact_id: other_contact.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(relationship(db, r#type, contact, other_contact).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_unsets_relationship() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = related(&db, Some(regis.account_id), &contact, &other_contact).await;
    unset(&db, &regis, regis.account_id, &r#type, &vault, &contact, &other_contact).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn unset_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let err = UnsetRelationship {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        relationship_type_id: Id(i64::MAX),
        contact_id: contact.id,
        other_contact_id: other_contact.id,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn unset_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let account = factory::account(&db.conn).await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = related(&db, Some(regis.account_id), &contact, &other_contact).await;
    let err = unset(&db, &regis, account.id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn unset_fails_if_relationship_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = related(&db, None, &contact, &other_contact).await;
    let err = unset(&db, &regis, account.id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn unset_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = contact_elsewhere(&db).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = related(&db, Some(regis.account_id), &contact, &other_contact).await;
    let err = unset(&db, &regis, regis.account_id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn unset_fails_if_other_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::Manage).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = contact_elsewhere(&db).await;
    let r#type = related(&db, Some(regis.account_id), &contact, &other_contact).await;
    let err = unset(&db, &regis, regis.account_id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn unset_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let other_contact = factory::contact(&db.conn, &vault).await;
    let r#type = related(&db, Some(regis.account_id), &contact, &other_contact).await;
    let err = unset(&db, &regis, regis.account_id, &r#type, &vault, &contact, &other_contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}
