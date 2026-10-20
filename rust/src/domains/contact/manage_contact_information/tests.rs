//! `tests/Unit/Domains/Contact/ManageContactInformation/Services`.

use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

use super::*;

/// `ContactInformationType::factory()` in `account_id`.
async fn type_factory(db: &TestDb, account_id: Uuid) -> contact_information_types::Model {
    contact_information_types::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        protocol: Set(Some("mailto:".into())),
        can_be_deleted: Set(false),
        r#type: Set(Some("email".into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap()
}

/// `ContactInformationType::factory()->create()`: in a new account.
async fn type_elsewhere(db: &TestDb) -> contact_information_types::Model {
    let account = factory::account(&db.conn).await;
    type_factory(db, account.id).await
}

/// `ContactInformation::factory()` of `contact_id` with `type_id`.
async fn information_factory(db: &TestDb, contact_id: Uuid, type_id: Id) -> contact_information::Model {
    contact_information::ActiveModel {
        contact_id: Set(contact_id),
        type_id: Set(type_id),
        data: Set(format!("{}@example.com", random_string(8).to_lowercase())),
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

/// `ContactInformation::factory()->create()`: of a new contact, with a new type.
async fn information_elsewhere(db: &TestDb) -> contact_information::Model {
    let contact = contact_elsewhere(db).await;
    let r#type = type_elsewhere(db).await;
    information_factory(db, contact.id, r#type.id).await
}

/// `createUser()` with a vault where they have `permission`.
async fn user_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

/// `assertDatabaseHas('contact_feed_items', [contact_id, action])`.
async fn assert_feed_item(db: &TestDb, contact_id: Uuid, action: &str) {
    let row = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(contact_id))
        .filter(contact_feed_items::Column::Action.eq(action))
        .one(&db.conn)
        .await
        .unwrap();
    assert!(row.is_some());
}

// ---------------------------------------------------------------- CreateContactInformationTest

async fn create(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    r#type: &contact_information_types::Model,
) -> ApiResult<()> {
    let info = CreateContactInformation {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_information_type_id: r#type.id,
        contact_information_kind: None,
        data: "45322322".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = contact_information::Entity::find_by_id(info.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.data, "45322322");
    assert_feed_item(db, contact.id, "contact_information_created").await;
    Ok(())
}

#[tokio::test]
async fn it_creates_a_contact_information() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    create(&db, &regis, regis.account_id, &vault, &contact, &r#type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let err = CreateContactInformation {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        contact_information_type_id: r#type.id,
        contact_information_kind: None,
        data: String::new(),
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
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let err = create(&db, &regis, account.id, &vault, &contact, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let err = create(&db, &regis, regis.account_id, &vault, &contact, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let err = create(&db, &regis, regis.account_id, &vault, &contact, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_type_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_elsewhere(&db).await;
    let err = create(&db, &regis, regis.account_id, &vault, &contact, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyContactInformationTest

async fn destroy(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    information: &contact_information::Model,
) -> ApiResult<()> {
    DestroyContactInformation {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_information_id: information.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(contact_information::Entity::find_by_id(information.id).one(&db.conn).await.unwrap().is_none());
    assert_feed_item(db, contact.id, "contact_information_destroyed").await;
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_contact_information() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    destroy(&db, &regis, regis.account_id, &vault, &contact, &information).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every DestroyContactInformation field is typed.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    let err = destroy(&db, &regis, account.id, &vault, &contact, &information).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    let err = destroy(&db, &regis, regis.account_id, &vault, &contact, &information).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    let err = destroy(&db, &regis, regis.account_id, &vault, &contact, &information).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_type_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_elsewhere(&db).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    let err = destroy(&db, &regis, regis.account_id, &vault, &contact, &information).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_information_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    type_elsewhere(&db).await;
    let information = information_elsewhere(&db).await;
    let err = destroy(&db, &regis, regis.account_id, &vault, &contact, &information).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateContactInformationTest

fn update_service(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    r#type: &contact_information_types::Model,
    information: &contact_information::Model,
) -> UpdateContactInformation {
    UpdateContactInformation {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_information_type_id: r#type.id,
        contact_information_id: information.id,
        contact_information_kind: None,
        data: "45322322".into(),
    }
}

async fn update(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    r#type: &contact_information_types::Model,
    information: &contact_information::Model,
) -> ApiResult<()> {
    let info = update_service(author, account_id, vault, contact, r#type, information).execute(&db.conn).await?;

    let row = contact_information::Entity::find_by_id(info.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.data, "45322322");
    assert_feed_item(db, contact.id, "contact_information_updated").await;
    Ok(())
}

#[tokio::test]
async fn it_updates_a_contact_information() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    update(&db, &regis, regis.account_id, &vault, &contact, &r#type, &information).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    let mut service = update_service(&regis, regis.account_id, &vault, &contact, &r#type, &information);
    service.data = String::new();
    assert_eq!(service.execute(&db.conn).await.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    let err = update(&db, &regis, account.id, &vault, &contact, &r#type, &information).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    let err = update(&db, &regis, regis.account_id, &vault, &contact, &r#type, &information).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_factory(&db, regis.account_id).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    let err = update(&db, &regis, regis.account_id, &vault, &contact, &r#type, &information).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_type_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_elsewhere(&db).await;
    let information = information_factory(&db, contact.id, r#type.id).await;
    let err = update(&db, &regis, regis.account_id, &vault, &contact, &r#type, &information).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_information_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let r#type = type_elsewhere(&db).await;
    let information = information_elsewhere(&db).await;
    let err = update(&db, &regis, regis.account_id, &vault, &contact, &r#type, &information).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
