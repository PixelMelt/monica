//! `tests/Unit/Domains/Contact/ManageDocuments/Services`.

use sea_orm::EntityTrait;

use super::*;
use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `File::factory()` in `vault_id` with `type`.
async fn file_factory(db: &TestDb, vault_id: Uuid, r#type: &str) -> files::Model {
    files::ActiveModel {
        vault_id: Set(vault_id),
        uuid: Set(Uuid::new().to_string()),
        original_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        cdn_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        name: Set(format!("Name {}", random_string(6))),
        mime_type: Set("avatar".into()),
        r#type: Set(r#type.into()),
        size: Set(rand::random_range(200 * 1024 * 1024..=500 * 1024 * 1024)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap()
}

/// `createUser()` with a vault where they have `permission`.
async fn user_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

// ---------------------------------------------------------------- DestroyFileTest

async fn destroy_file(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    file: &files::Model,
) -> ApiResult<()> {
    DestroyFile { account_id, vault_id: vault.id, author_id: author.id, file_id: file.id }.execute(&db.conn).await?;

    assert!(files::Entity::find_by_id(file.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_file() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let file = file_factory(&db, vault.id, "document").await;
    destroy_file(&db, &regis, regis.account_id, &vault, &file).await.unwrap();
    db.drop().await;
}

// it_fails_if_wrong_parameters_are_given: every DestroyFile field is typed.

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let file = file_factory(&db, vault.id, "document").await;
    assert_eq!(destroy_file(&db, &regis, account.id, &vault, &file).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    let file = file_factory(&db, vault.id, "document").await;
    assert_eq!(destroy_file(&db, &regis, regis.account_id, &vault, &file).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UploadFileTest

async fn upload_file(db: &TestDb, author: &users::Model, account_id: Uuid, vault: &vaults::Model) -> ApiResult<()> {
    let file = UploadFile {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        uuid: "017162da-e83b-46fc-89fc-3a7740db0a81".into(),
        name: "Twitter post.png".into(),
        mime_type: "image/jpg".into(),
        size: 390340,
        r#type: "avatar".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = files::Entity::find_by_id(file.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.uuid, "017162da-e83b-46fc-89fc-3a7740db0a81");
    assert_eq!(row.name, "Twitter post.png");
    assert_eq!(row.original_url, None);
    assert_eq!(row.cdn_url, None);
    assert_eq!(row.mime_type, "image/jpg");
    assert_eq!(row.size, 390340);
    assert_eq!(row.r#type, "avatar");
    Ok(())
}

#[tokio::test]
async fn it_uploads_a_file() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    upload_file(&db, &regis, regis.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn upload_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let err = UploadFile {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        uuid: "017162da-e83b-46fc-89fc-3a7740db0a81".into(),
        name: String::new(),
        mime_type: "image/jpg".into(),
        size: 390340,
        r#type: "avatar".into(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

// it_throws_an_exception_when_env_keys_are_not_set: Uploadcare is removed (I005).

#[tokio::test]
async fn upload_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    assert_eq!(upload_file(&db, &regis, account.id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn upload_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = user_in_vault(&db, VaultPermission::View).await;
    assert_eq!(upload_file(&db, &regis, regis.account_id, &vault).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}
