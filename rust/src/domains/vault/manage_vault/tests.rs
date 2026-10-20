//! `tests/Unit/Domains/Vault/ManageVault/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, PaginatorTrait};

use super::*;
use crate::crypto::random_string;
use crate::entities::contact_important_date_types;
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

async fn vault_row(db: &TestDb, id: Uuid) -> Option<vaults::Model> {
    vaults::Entity::find_by_id(id).one(&db.conn).await.unwrap()
}

// ---------------------------------------------------------------- CreateVaultTest

async fn create_vault(db: &TestDb, author: &users::Model, account_id: Uuid) -> ApiResult<()> {
    let vault = CreateVault {
        account_id,
        author_id: author.id,
        template_id: None,
        r#type: "personal".into(),
        name: "vault name".into(),
        description: None,
    }
    .execute(&db.conn)
    .await?;

    let row = vault_row(db, vault.id).await.unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name, "vault name");
    assert_eq!(row.r#type, "personal");

    assert_eq!(contacts::Entity::find().count(&db.conn).await.unwrap(), 1);

    let contact = contacts::Entity::find().one(&db.conn).await.unwrap().unwrap();

    assert!(!contact.can_be_deleted);

    let pivot = user_vault::Entity::find_by_id((vault.id, author.id)).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(pivot.contact_id, contact.id);

    for (label, internal_type) in [("Birthdate", "birthdate"), ("Deceased date", "deceased_date")] {
        let r#type = contact_important_date_types::Entity::find()
            .filter(contact_important_date_types::Column::VaultId.eq(vault.id))
            .filter(contact_important_date_types::Column::Label.eq(label))
            .one(&db.conn)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r#type.internal_type.as_deref(), Some(internal_type));
        assert!(!r#type.can_be_deleted);
    }
    Ok(())
}

#[tokio::test]
async fn it_creates_a_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    create_vault(&db, &ross, ross.account_id).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let err = CreateVault {
        account_id: ross.account_id,
        author_id: ross.id,
        template_id: None,
        r#type: String::new(),
        name: String::new(),
        description: None,
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
    let ross = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create_vault(&db, &ross, account.id).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyVaultTest

async fn destroy_vault(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    file: Option<&files::Model>,
) -> ApiResult<()> {
    DestroyVault { account_id, author_id: author.id, vault_id: vault.id }.execute(&db.conn).await?;

    assert!(vault_row(db, vault.id).await.is_none());

    if let Some(file) = file {
        assert!(files::Entity::find_by_id(file.id).one(&db.conn).await.unwrap().is_none());
    }
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let file = file(&db, vault.id).await;
    destroy_vault(&db, &ross, ross.account_id, &vault, Some(&file)).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, ross.account_id).await;
    assert_eq!(destroy_vault(&db, &ross, account.id, &vault, None).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::vault(&db.conn, factory::account(&db.conn).await.id).await;
    assert_eq!(destroy_vault(&db, &ross, ross.account_id, &vault, None).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    assert_eq!(destroy_vault(&db, &ross, ross.account_id, &vault, None).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateVaultTest

async fn update_vault(db: &TestDb, author: &users::Model, account_id: Uuid, vault: &vaults::Model) -> ApiResult<()> {
    let vault = UpdateVault {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        name: "vault name".into(),
        description: None,
    }
    .execute(&db.conn)
    .await?;

    let row = vault_row(db, vault.id).await.unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.name, "vault name");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    update_vault(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = UpdateVault {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        name: String::new(),
        description: None,
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
    let ross = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, ross.account_id).await;
    assert_eq!(update_vault(&db, &ross, account.id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::vault(&db.conn, factory::account(&db.conn).await.id).await;
    assert_eq!(update_vault(&db, &ross, ross.account_id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    assert_eq!(update_vault(&db, &ross, ross.account_id, &vault).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateVaultDashboardDefaultTabTest

async fn update_default_tab(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
) -> ApiResult<()> {
    let vault = UpdateVaultDashboardDefaultTab {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        default_activity_tab: "activity".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = vault_row(db, vault.id).await.unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.default_activity_tab, "activity");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_vault_default_tab_on_dashboard() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    update_default_tab(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_default_tab_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = UpdateVaultDashboardDefaultTab {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        default_activity_tab: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_default_tab_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, ross.account_id).await;
    assert_eq!(update_default_tab(&db, &ross, account.id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_default_tab_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::vault(&db.conn, factory::account(&db.conn).await.id).await;
    assert_eq!(update_default_tab(&db, &ross, ross.account_id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateVaultTabVisibilityTest

async fn update_tab_visibility(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
) -> ApiResult<()> {
    let vault = UpdateVaultTabVisibility {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        show_group_tab: true,
        show_tasks_tab: true,
        show_files_tab: true,
        show_journal_tab: true,
        show_companies_tab: true,
        show_reports_tab: true,
        show_calendar_tab: true,
    }
    .execute(&db.conn)
    .await?;

    let row = vault_row(db, vault.id).await.unwrap();
    assert_eq!(row.account_id, account_id);
    assert!(row.show_group_tab);
    assert!(row.show_tasks_tab);
    assert!(row.show_files_tab);
    assert!(row.show_journal_tab);
    assert!(row.show_companies_tab);
    assert!(row.show_reports_tab);
    assert!(row.show_calendar_tab);
    Ok(())
}

#[tokio::test]
async fn update_tab_visibility_updates_a_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    update_tab_visibility(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

// update_tab_visibility_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn update_tab_visibility_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, ross.account_id).await;
    assert_eq!(update_tab_visibility(&db, &ross, account.id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_tab_visibility_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::vault(&db.conn, factory::account(&db.conn).await.id).await;
    assert_eq!(update_tab_visibility(&db, &ross, ross.account_id, &vault).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_tab_visibility_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    assert_eq!(update_tab_visibility(&db, &ross, ross.account_id, &vault).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}
