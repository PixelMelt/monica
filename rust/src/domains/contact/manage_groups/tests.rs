//! `tests/Unit/Domains/Contact/ManageGroups/Services`.

use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

use super::*;

/// `GroupType::factory()` in `account_id`.
async fn group_type_factory(db: &TestDb, account_id: Uuid) -> group_types::Model {
    group_types::ActiveModel {
        account_id: Set(account_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        position: Set(1),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap()
}

/// `GroupType::factory()->create()`: in a new account.
async fn group_type_elsewhere(db: &TestDb) -> group_types::Model {
    let account = factory::account(&db.conn).await;
    group_type_factory(db, account.id).await
}

/// `GroupTypeRole::factory()` of `group_type_id`.
async fn group_type_role_factory(db: &TestDb, group_type_id: Id) -> group_type_roles::Model {
    group_type_roles::ActiveModel {
        group_type_id: Set(group_type_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        position: Set(Some(1)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap()
}

/// `Group::factory()` in `vault_id` (a new vault when `None`) with
/// `group_type_id` (a group type in a new account when `None`).
async fn group_factory(db: &TestDb, vault_id: Option<Uuid>, group_type_id: Option<Id>) -> groups::Model {
    let vault_id = match vault_id {
        Some(id) => id,
        None => {
            let account = factory::account(&db.conn).await;
            factory::vault(&db.conn, account.id).await.id
        }
    };
    let group_type_id = match group_type_id {
        Some(id) => id,
        None => group_type_elsewhere(db).await.id,
    };
    groups::ActiveModel {
        uuid: Set(Some(Uuid::new())),
        vault_id: Set(vault_id),
        group_type_id: Set(Some(group_type_id)),
        name: Set(format!("Name {}", random_string(6))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap()
}

/// `Contact::factory()->create()`: in a new vault.
async fn contact_elsewhere(db: &TestDb) -> contacts::Model {
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `createUser()` with a vault where they have `permission`.
async fn user_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

/// `$group->contacts()->attach($contact->id)`.
async fn attach(db: &TestDb, group: &groups::Model, contact: &contacts::Model) {
    contact_group::ActiveModel {
        group_id: Set(group.id),
        contact_id: Set(contact.id),
        group_type_role_id: Set(None),
        created_at: Set(None),
        updated_at: Set(None),
    }
    .insert(&db.conn)
    .await
    .unwrap();
}

/// The `contact_group` row of `group` and `contact`.
async fn pivot(db: &TestDb, group: &groups::Model, contact: &contacts::Model) -> Option<contact_group::Model> {
    contact_group::Entity::find()
        .filter(contact_group::Column::GroupId.eq(group.id))
        .filter(contact_group::Column::ContactId.eq(contact.id))
        .one(&db.conn)
        .await
        .unwrap()
}

// ---------------------------------------------------------------- AddContactToGroupTest

async fn add(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    group: &groups::Model,
    group_type_role: &group_type_roles::Model,
) -> ApiResult<()> {
    AddContactToGroup {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        group_id: group.id,
        contact_id: contact.id,
        group_type_role_id: Some(group_type_role.id),
    }
    .execute(&db.conn)
    .await?;

    let row = pivot(db, group, contact).await.unwrap();
    assert_eq!(row.group_type_role_id, Some(group_type_role.id));
    Ok(())
}

#[tokio::test]
async fn it_adds_a_contact_to_a_group() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let group_type = group_type_factory(&db, vault.account_id).await;
    let group = group_factory(&db, Some(vault.id), Some(group_type.id)).await;
    let role = group_type_role_factory(&db, group_type.id).await;
    add(&db, &regis, regis.account_id, &vault, &contact, &group, &role).await.unwrap();
    db.drop().await;
}

// it_fails_if_wrong_parameters_are_given: every AddContactToGroup field is typed.

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let group_type = group_type_factory(&db, vault.account_id).await;
    let group = group_factory(&db, Some(vault.id), Some(group_type.id)).await;
    let role = group_type_role_factory(&db, group_type.id).await;
    let err = add(&db, &regis, account.id, &vault, &contact, &group, &role).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_group_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let group_type = group_type_factory(&db, vault.account_id).await;
    let group = group_factory(&db, None, Some(group_type.id)).await;
    let role = group_type_role_factory(&db, group_type.id).await;
    let err = add(&db, &regis, regis.account_id, &vault, &contact, &group, &role).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let group_type = group_type_factory(&db, vault.account_id).await;
    let group = group_factory(&db, Some(vault.id), Some(group_type.id)).await;
    let role = group_type_role_factory(&db, group_type.id).await;
    let err = add(&db, &regis, regis.account_id, &vault, &contact, &group, &role).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_group_type_role_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let group_type = group_type_factory(&db, vault.account_id).await;
    let group = group_factory(&db, Some(vault.id), Some(group_type.id)).await;
    let role = group_type_role_factory(&db, group_type_elsewhere(&db).await.id).await;
    let err = add(&db, &regis, regis.account_id, &vault, &contact, &group, &role).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let group_type = group_type_factory(&db, vault.account_id).await;
    let group = group_factory(&db, Some(vault.id), Some(group_type.id)).await;
    let role = group_type_role_factory(&db, group_type.id).await;
    let err = add(&db, &regis, regis.account_id, &vault, &contact, &group, &role).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateGroupTest

fn create_service(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    group_type: &group_types::Model,
) -> CreateGroup {
    CreateGroup {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        group_type_id: Some(group_type.id),
        name: Some("test".into()),
    }
}

async fn create(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    group_type: &group_types::Model,
) -> ApiResult<()> {
    let group = create_service(author, account_id, vault, group_type).execute(&db.conn).await?;

    let row = groups::Entity::find_by_id(group.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.group_type_id, Some(group_type.id));
    assert_eq!(row.name, "test");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_group() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let group_type = group_type_factory(&db, regis.account_id).await;
    create(&db, &regis, regis.account_id, &vault, &group_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let group_type = group_type_factory(&db, regis.account_id).await;
    let mut service = create_service(&regis, regis.account_id, &vault, &group_type);
    service.name = Some("x".repeat(256));
    assert_eq!(service.execute(&db.conn).await.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let group_type = group_type_factory(&db, regis.account_id).await;
    let err = create(&db, &regis, account.id, &vault, &group_type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let group_type = group_type_factory(&db, regis.account_id).await;
    let err = create(&db, &regis, regis.account_id, &vault, &group_type).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_group_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let group_type = group_type_elsewhere(&db).await;
    let err = create(&db, &regis, regis.account_id, &vault, &group_type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyGroupTest

async fn destroy(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    group: &groups::Model,
) -> ApiResult<()> {
    DestroyGroup { account_id, vault_id: vault.id, author_id: author.id, group_id: group.id }.execute(&db.conn).await?;

    let row = groups::Entity::find_by_id(group.id).one(&db.conn).await.unwrap().unwrap();
    assert!(row.deleted_at.is_some());
    Ok(())
}

#[tokio::test]
async fn it_deletes_a_group() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    destroy(&db, &regis, regis.account_id, &vault, &group).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every DestroyGroup field is typed.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    assert_eq!(destroy(&db, &regis, account.id, &vault, &group).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    assert_eq!(destroy(&db, &regis, regis.account_id, &vault, &group).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_couple_is_not_in_the_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let group = group_factory(&db, None, None).await;
    assert_eq!(destroy(&db, &regis, regis.account_id, &vault, &group).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- RemoveContactFromGroupTest

async fn remove(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    group: &groups::Model,
) -> ApiResult<()> {
    RemoveContactFromGroup {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        group_id: group.id,
        contact_id: contact.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(pivot(db, group, contact).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_removes_a_contact_from_a_group() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    attach(&db, &group, &contact).await;
    remove(&db, &regis, regis.account_id, &vault, &contact, &group).await.unwrap();
    db.drop().await;
}

// remove_fails_if_wrong_parameters_are_given: every RemoveContactFromGroup field is typed.

#[tokio::test]
async fn remove_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    attach(&db, &group, &contact).await;
    let err = remove(&db, &regis, account.id, &vault, &contact, &group).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_label_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let group = group_factory(&db, None, None).await;
    attach(&db, &group, &contact).await;
    let err = remove(&db, &regis, regis.account_id, &vault, &contact, &group).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    attach(&db, &group, &contact).await;
    let err = remove(&db, &regis, regis.account_id, &vault, &contact, &group).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_group_type_role_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    let err = remove(&db, &regis, regis.account_id, &vault, &contact, &group).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    attach(&db, &group, &contact).await;
    let err = remove(&db, &regis, regis.account_id, &vault, &contact, &group).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateGroupTest

fn update_service(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    group: &groups::Model,
    group_type: &group_types::Model,
) -> UpdateGroup {
    UpdateGroup {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        group_id: group.id,
        group_type_id: Some(group_type.id),
        name: Some("love".into()),
    }
}

async fn update(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    group: &groups::Model,
    group_type: &group_types::Model,
) -> ApiResult<()> {
    let group = update_service(author, account_id, vault, group, group_type).execute(&db.conn).await?;

    let row = groups::Entity::find_by_id(group.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.group_type_id, Some(group_type.id));
    assert_eq!(row.name, "love");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_group() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    let group_type = group_type_factory(&db, regis.account_id).await;
    update(&db, &regis, regis.account_id, &vault, &group, &group_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    let group_type = group_type_factory(&db, regis.account_id).await;
    let mut service = update_service(&regis, regis.account_id, &vault, &group, &group_type);
    service.name = Some("x".repeat(256));
    assert_eq!(service.execute(&db.conn).await.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    let group_type = group_type_factory(&db, regis.account_id).await;
    let err = update(&db, &regis, account.id, &vault, &group, &group_type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    let group_type = group_type_factory(&db, regis.account_id).await;
    let err = update(&db, &regis, regis.account_id, &vault, &group, &group_type).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_group_is_not_in_the_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let group = group_factory(&db, None, None).await;
    let group_type = group_type_factory(&db, regis.account_id).await;
    let err = update(&db, &regis, regis.account_id, &vault, &group, &group_type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_group_type_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let group = group_factory(&db, Some(vault.id), None).await;
    let group_type = group_type_elsewhere(&db).await;
    let err = update(&db, &regis, regis.account_id, &vault, &group, &group_type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
