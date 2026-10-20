//! `tests/Unit/Domains/Contact/ManageContactAddresses/Services`.

use crate::crypto::random_string;
use crate::entities::{address_types, users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

use super::*;

/// `Address::factory()` in `vault_id` (with an `AddressType::factory()` in a new account).
async fn address_factory(db: &TestDb, vault_id: Uuid) -> addresses::Model {
    let account = factory::account(&db.conn).await;
    let address_type = address_types::ActiveModel {
        account_id: Set(account.id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        r#type: Set(Some("home".into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap();
    addresses::ActiveModel {
        vault_id: Set(vault_id),
        address_type_id: Set(Some(address_type.id)),
        line_1: Set(Some(format!("{} Main Street", rand::random_range(1..999)))),
        line_2: Set(Some(format!("{} Side Street", rand::random_range(1..999)))),
        city: Set(Some(format!("City {}", random_string(6)))),
        province: Set(Some(format!("Name {}", random_string(6)))),
        postal_code: Set(Some(format!("{:05}", rand::random_range(0..99999)))),
        country: Set(Some(rand::random_range(1..=3).to_string())),
        latitude: Set(Some(rand::random_range(-90.0..90.0))),
        longitude: Set(Some(rand::random_range(-180.0..180.0))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap()
}

/// `Address::factory()->create()`: in a new vault.
async fn address_elsewhere(db: &TestDb) -> addresses::Model {
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, account.id).await;
    address_factory(db, vault.id).await
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

/// The `contact_address` row of `contact_id` and `address_id`.
async fn pivot(db: &TestDb, contact_id: Uuid, address_id: Id) -> Option<contact_address::Model> {
    contact_address::Entity::find()
        .filter(contact_address::Column::ContactId.eq(contact_id))
        .filter(contact_address::Column::AddressId.eq(address_id))
        .one(&db.conn)
        .await
        .unwrap()
}

// ---------------------------------------------------------------- AssociateAddressToContactTest

async fn associate(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    address: &addresses::Model,
) -> ApiResult<()> {
    let address = AssociateAddressToContact {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        address_id: address.id,
        is_past_address: false,
    }
    .execute(&db.conn)
    .await?;

    let row = pivot(db, contact.id, address.id).await.unwrap();
    assert!(!row.is_past_address);
    assert_feed_item(db, contact.id, "address_created").await;
    Ok(())
}

#[tokio::test]
async fn it_associates_a_contact_to_an_address() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let address = address_factory(&db, vault.id).await;
    associate(&db, &regis, regis.account_id, &vault, &contact, &address).await.unwrap();
    db.drop().await;
}

// it_fails_if_wrong_parameters_are_given: every AssociateAddressToContact field is typed.

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let address = address_factory(&db, vault.id).await;
    let err = associate(&db, &regis, account.id, &vault, &contact, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let address = address_factory(&db, vault.id).await;
    let err = associate(&db, &regis, regis.account_id, &vault, &contact, &address).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_is_not_part_of_the_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let address = address_factory(&db, vault.id).await;
    let err = associate(&db, &regis, regis.account_id, &vault, &contact, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_address_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let address = address_elsewhere(&db).await;
    let err = associate(&db, &regis, regis.account_id, &vault, &contact, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- RemoveAddressFromContactTest

async fn remove(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    address: &addresses::Model,
) -> ApiResult<()> {
    contact_address::ActiveModel {
        contact_id: Set(contact.id),
        address_id: Set(address.id),
        is_past_address: Set(false),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap();

    let address = RemoveAddressFromContact {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        address_id: address.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(pivot(db, contact.id, address.id).await.is_none());
    assert_feed_item(db, contact.id, "address_destroyed").await;
    Ok(())
}

#[tokio::test]
async fn it_removes_a_contact_from_an_address() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let address = address_factory(&db, vault.id).await;
    remove(&db, &regis, regis.account_id, &vault, &contact, &address).await.unwrap();
    db.drop().await;
}

// remove_fails_if_wrong_parameters_are_given: every RemoveAddressFromContact field is typed.

#[tokio::test]
async fn remove_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let address = address_factory(&db, vault.id).await;
    let err = remove(&db, &regis, account.id, &vault, &contact, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let address = address_factory(&db, vault.id).await;
    let err = remove(&db, &regis, regis.account_id, &vault, &contact, &address).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_contact_is_not_part_of_the_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let address = address_factory(&db, vault.id).await;
    let err = remove(&db, &regis, regis.account_id, &vault, &contact, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_address_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let address = address_elsewhere(&db).await;
    let err = remove(&db, &regis, regis.account_id, &vault, &contact, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- MapHelperTest

#[test]
fn it_gets_the_address_as_a_string() {
    let address = addresses::Model {
        id: Id(1),
        vault_id: Uuid::new(),
        address_type_id: None,
        line_1: Some("123 main st".into()),
        line_2: Some("Apartment 4".into()),
        city: Some("montreal".into()),
        province: Some("quebec".into()),
        postal_code: Some("h1k 12k".into()),
        country: Some("Canada".into()),
        latitude: None,
        longitude: None,
        created_at: None,
        updated_at: None,
    };
    assert_eq!(get_address_as_string(&address), "123 main st Apartment 4 montreal quebec h1k 12k Canada");
}
