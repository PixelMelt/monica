//! `tests/Unit/Domains/Contact/ManageAvatar/Services`.

use super::*;
use crate::crypto::random_string;
use crate::entities::vaults;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `File::factory()` in `vault_id`.
async fn file(db: &TestDb, vault_id: Uuid) -> files::Model {
    files::ActiveModel {
        vault_id: Set(vault_id),
        uuid: Set(Uuid::new().to_string()),
        original_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        cdn_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        name: Set(format!("Name {}", random_string(6))),
        mime_type: Set("avatar".into()),
        r#type: Set("avatar".into()),
        size: Set(rand::random_range(200 * 1024 * 1024..=500 * 1024 * 1024)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("file")
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

async fn contact_row(db: &TestDb, id: Uuid) -> contacts::Model {
    contacts::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap()
}

// ---------------------------------------------------------------- DestroyAvatarTest

async fn destroy_avatar(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    DestroyAvatar { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id }
        .execute(&db.conn)
        .await?;

    assert_eq!(contact_row(db, contact.id).await.file_id, None);
    Ok(())
}

#[tokio::test]
async fn it_deletes_an_avatar_if_its_a_photo() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &user, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let account = factory::account(&db.conn).await;
    let file = file(&db, factory::vault(&db.conn, account.id).await.id).await;
    let mut c: contacts::ActiveModel = contact.into();
    c.file_id = Set(Some(file.id));
    let contact = c.update(&db.conn).await.unwrap();

    destroy_avatar(&db, &user, user.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

// it_fails_if_wrong_parameters_are_given: every field is typed, no invalid
// input can be built.

// ---------------------------------------------------------------- UpdatePhotoAsAvatarTest

async fn update_photo_as_avatar(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    file: &files::Model,
) -> ApiResult<()> {
    let contact = UpdatePhotoAsAvatar {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        file_id: Some(file.id),
    }
    .execute(&db.conn)
    .await?;

    assert_eq!(contact_row(db, contact.id).await.file_id, Some(file.id));
    let row = files::Entity::find_by_id(file.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    Ok(())
}

#[tokio::test]
async fn it_uploads_a_photo_as_avatar() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &user, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let file = file(&db, vault.id).await;
    update_photo_as_avatar(&db, &user, user.account_id, &vault, &contact, &file).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &user, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = UpdatePhotoAsAvatar {
        account_id: user.account_id,
        vault_id: vault.id,
        author_id: user.id,
        contact_id: contact.id,
        file_id: Some(Id(i64::MAX)),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = vault_for(&db, &user, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let file = file(&db, vault.id).await;
    let err = update_photo_as_avatar(&db, &user, account.id, &vault, &contact, &file).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &user, VaultPermission::Edit).await;
    let contact = contact_elsewhere(&db).await;
    let file = file(&db, vault.id).await;
    let err = update_photo_as_avatar(&db, &user, user.account_id, &vault, &contact, &file).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = vault_for(&db, &user, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let file = file(&db, vault.id).await;
    let err = update_photo_as_avatar(&db, &user, user.account_id, &vault, &contact, &file).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}
