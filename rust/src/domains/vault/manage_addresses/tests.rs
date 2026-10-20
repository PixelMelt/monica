//! `tests/Unit/Domains/Vault/ManageAddresses/Services`.

use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

use super::*;

/// `AddressType::factory()` in `account_id`.
async fn address_type_factory(db: &TestDb, account_id: Uuid) -> address_types::Model {
    const TYPES: [&str; 3] = ["home", "work", "other"];
    address_types::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        r#type: Set(Some(TYPES[rand::random_range(0..TYPES.len())].into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap()
}

/// `AddressType::factory()->create()`: in a new account.
async fn address_type_elsewhere(db: &TestDb) -> address_types::Model {
    let account = factory::account(&db.conn).await;
    address_type_factory(db, account.id).await
}

/// `Address::factory()` in `vault_id` with `address_type_id`.
async fn address_factory(db: &TestDb, vault_id: Uuid, address_type_id: Id) -> addresses::Model {
    addresses::ActiveModel {
        vault_id: Set(vault_id),
        address_type_id: Set(Some(address_type_id)),
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

/// `Address::factory()->create()`: in a new vault, with a type in a new account.
async fn address_elsewhere(db: &TestDb) -> addresses::Model {
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, account.id).await;
    let address_type = address_type_elsewhere(db).await;
    address_factory(db, vault.id, address_type.id).await
}

/// `createUser()` with a vault where they have `permission`.
async fn user_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

/// `assertDatabaseHas('addresses', […])` with the values the tests send.
async fn assert_address(db: &TestDb, id: Id, vault: &vaults::Model, address_type: &address_types::Model) {
    let row = addresses::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.address_type_id, Some(address_type.id));
    assert_eq!(row.line_1.as_deref(), Some("25 grand rue"));
    assert_eq!(row.line_2.as_deref(), Some("Apartment 3"));
    assert_eq!(row.city.as_deref(), Some("paris"));
    assert_eq!(row.province.as_deref(), Some("67"));
    assert_eq!(row.postal_code.as_deref(), Some("12344"));
    assert_eq!(row.country.as_deref(), Some("FRA"));
    assert_eq!(row.latitude, Some(12345.0));
    assert_eq!(row.longitude, Some(12345.0));
}

// ---------------------------------------------------------------- CreateAddressTest

fn create_address(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    address_type_id: Id,
) -> CreateAddress {
    CreateAddress {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        address_type_id: Some(address_type_id),
        line_1: Some("25 grand rue".into()),
        line_2: Some("Apartment 3".into()),
        city: Some("paris".into()),
        province: Some("67".into()),
        postal_code: Some("12344".into()),
        country: Some("FRA".into()),
        latitude: Some(12345.0),
        longitude: Some(12345.0),
    }
}

async fn execute_create(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    address_type: &address_types::Model,
) -> ApiResult<()> {
    let address = create_address(author, account_id, vault, address_type.id).execute(&db.conn).await?;

    assert_address(db, address.id, vault, address_type).await;
    // Queue::assertPushed(GetGPSCoordinate): geocoding is removed (I007).
    Ok(())
}

#[tokio::test]
async fn it_creates_a_contact_address() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let address_type = address_type_factory(&db, regis.account_id).await;
    execute_create(&db, &regis, regis.account_id, &vault, &address_type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let address_type = address_type_factory(&db, regis.account_id).await;
    let mut service = create_address(&regis, regis.account_id, &vault, address_type.id);
    service.line_1 = Some("x".repeat(256));
    assert_eq!(service.execute(&db.conn).await.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let address_type = address_type_factory(&db, regis.account_id).await;
    let err = execute_create(&db, &regis, account.id, &vault, &address_type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let address_type = address_type_factory(&db, regis.account_id).await;
    let err = execute_create(&db, &regis, regis.account_id, &vault, &address_type).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_type_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let address_type = address_type_elsewhere(&db).await;
    let err = execute_create(&db, &regis, regis.account_id, &vault, &address_type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyAddressTest

async fn execute_destroy(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    address: &addresses::Model,
) -> ApiResult<()> {
    DestroyAddress { account_id, vault_id: vault.id, author_id: author.id, address_id: address.id }
        .execute(&db.conn)
        .await?;

    assert!(addresses::Entity::find_by_id(address.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

/// `Address::factory()->create(['vault_id' => …])`: with a type in a new account.
async fn address_in(db: &TestDb, vault: &vaults::Model) -> addresses::Model {
    let address_type = address_type_elsewhere(db).await;
    address_factory(db, vault.id, address_type.id).await
}

#[tokio::test]
async fn it_destroys_a_contact_address() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let address = address_in(&db, &vault).await;
    execute_destroy(&db, &regis, regis.account_id, &vault, &address).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every DestroyAddress field is typed.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let address = address_in(&db, &vault).await;
    assert_eq!(execute_destroy(&db, &regis, account.id, &vault, &address).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let address = address_in(&db, &vault).await;
    let err = execute_destroy(&db, &regis, regis.account_id, &vault, &address).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_address_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let address = address_elsewhere(&db).await;
    let err = execute_destroy(&db, &regis, regis.account_id, &vault, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// GetGPSCoordinateTest: geocoding is removed (I007).

// ---------------------------------------------------------------- UpdateAddressTest

fn update_address(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    address_type_id: Id,
    address_id: Id,
) -> UpdateAddress {
    UpdateAddress {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        address_id,
        address_type_id: Some(address_type_id),
        line_1: Some("25 grand rue".into()),
        line_2: Some("Apartment 3".into()),
        city: Some("paris".into()),
        province: Some("67".into()),
        postal_code: Some("12344".into()),
        country: Some("FRA".into()),
        latitude: Some(12345.0),
        longitude: Some(12345.0),
    }
}

async fn execute_update(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    address_type: &address_types::Model,
    address: &addresses::Model,
) -> ApiResult<()> {
    let address = update_address(author, account_id, vault, address_type.id, address.id).execute(&db.conn).await?;

    assert_address(db, address.id, vault, address_type).await;
    // Queue::assertPushed(GetGPSCoordinate): geocoding is removed (I007).
    Ok(())
}

#[tokio::test]
async fn it_updates_a_contact_address() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let address_type = address_type_factory(&db, regis.account_id).await;
    let address = address_factory(&db, vault.id, address_type.id).await;
    execute_update(&db, &regis, regis.account_id, &vault, &address_type, &address).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let address_type = address_type_factory(&db, regis.account_id).await;
    let address = address_factory(&db, vault.id, address_type.id).await;
    let mut service = update_address(&regis, regis.account_id, &vault, address_type.id, address.id);
    service.line_1 = Some("x".repeat(256));
    assert_eq!(service.execute(&db.conn).await.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let address_type = address_type_factory(&db, regis.account_id).await;
    let address = address_factory(&db, vault.id, address_type.id).await;
    let err = execute_update(&db, &regis, account.id, &vault, &address_type, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let address_type = address_type_factory(&db, regis.account_id).await;
    let address = address_factory(&db, vault.id, address_type.id).await;
    let err = execute_update(&db, &regis, regis.account_id, &vault, &address_type, &address).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_type_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let address_type = address_type_elsewhere(&db).await;
    let address = address_factory(&db, vault.id, address_type.id).await;
    let err = execute_update(&db, &regis, regis.account_id, &vault, &address_type, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_address_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let address_type = address_type_factory(&db, regis.account_id).await;
    let address = address_elsewhere(&db).await;
    let err = execute_update(&db, &regis, regis.account_id, &vault, &address_type, &address).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
