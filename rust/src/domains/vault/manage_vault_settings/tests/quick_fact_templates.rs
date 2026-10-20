//! `CreateQuickFactTemplateTest`, `DestroyQuickFactTemplateTest`,
//! `UpdateQuickFactTemplateTest` and `UpdateQuickFactTemplatePositionTest`.

use super::super::*;
use super::any_vault;
use crate::crypto::random_string;
use crate::entities::vault_quick_facts_templates;
use crate::testing::{TestDb, factory};

/// `VaultQuickFactsTemplate::factory()` in `vault_id`, to insert.
fn quick_fact_template(vault_id: Uuid) -> vault_quick_facts_templates::ActiveModel {
    vault_quick_facts_templates::ActiveModel {
        vault_id: Set(vault_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        position: Set(1),
        label_translation_key: Set(Some("vault_quick_fact_template.label_translation_key".into())),
        ..Default::default()
    }
}

async fn entry_row(db: &TestDb, id: Id) -> vault_quick_facts_templates::Model {
    vault_quick_facts_templates::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap()
}

// ---------------------------------------------------------------- CreateQuickFactTemplateTest

async fn create_quick_fact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
) -> ApiResult<()> {
    let quick_fact_template_entry =
        CreateQuickFactTemplate { account_id, author_id: author.id, vault_id: vault.id, label: "label".into() }
            .execute(&db.conn)
            .await?;

    let row = entry_row(db, quick_fact_template_entry.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label.as_deref(), Some("label"));
    assert_eq!(row.position, 1);
    Ok(())
}

#[tokio::test]
async fn it_creates_a_quick_fact_template_entry() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    create_quick_fact(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_quick_fact_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let err = CreateQuickFactTemplate {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        label: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_quick_fact_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = create_quick_fact(&db, &ross, account.id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_quick_fact_fails_if_user_is_not_editor() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_quick_fact(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_quick_fact_fails_if_vault_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = any_vault(&db).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_quick_fact(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyQuickFactTemplateTest

async fn destroy_quick_fact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    quick_fact_template_entry: &vault_quick_facts_templates::Model,
) -> ApiResult<()> {
    DestroyQuickFactTemplate {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        vault_quick_facts_template_id: quick_fact_template_entry.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(
        vault_quick_facts_templates::Entity::find_by_id(quick_fact_template_entry.id)
            .one(&db.conn)
            .await
            .unwrap()
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_quick_fact_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(vault.id).insert(&db.conn).await.unwrap();
    destroy_quick_fact(&db, &ross, ross.account_id, &vault, &quick_fact_template_entry).await.unwrap();
    db.drop().await;
}

// destroy_quick_fact_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_quick_fact_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_quick_fact(&db, &ross, account.id, &vault, &quick_fact_template_entry).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_quick_fact_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy_quick_fact(&db, &ross, account.id, &vault, &quick_fact_template_entry).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_quick_fact_fails_if_entry_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy_quick_fact(&db, &ross, ross.account_id, &vault, &quick_fact_template_entry).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_quick_fact_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let quick_fact_template_entry = quick_fact_template(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_quick_fact(&db, &ross, ross.account_id, &vault, &quick_fact_template_entry).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateQuickFactTemplateTest

async fn update_quick_fact(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    quick_fact_template_entry: &vault_quick_facts_templates::Model,
) -> ApiResult<()> {
    let quick_fact_template_entry = UpdateQuickFactTemplate {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        vault_quick_facts_template_id: quick_fact_template_entry.id,
        label: "label name".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = entry_row(db, quick_fact_template_entry.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label.as_deref(), Some("label name"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_quick_fact_template_entry() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(vault.id).insert(&db.conn).await.unwrap();
    update_quick_fact(&db, &ross, ross.account_id, &vault, &quick_fact_template_entry).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_quick_fact_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let quick_fact_template_entry = quick_fact_template(vault.id).insert(&db.conn).await.unwrap();
    let err = UpdateQuickFactTemplate {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        vault_quick_facts_template_id: quick_fact_template_entry.id,
        label: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_quick_fact_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(vault.id).insert(&db.conn).await.unwrap();
    let err = update_quick_fact(&db, &ross, account.id, &vault, &quick_fact_template_entry).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_quick_fact_fails_if_quick_fact_template_entry_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = update_quick_fact(&db, &ross, ross.account_id, &vault, &quick_fact_template_entry).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_quick_fact_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let quick_fact_template_entry = quick_fact_template(vault.id).insert(&db.conn).await.unwrap();
    let err = update_quick_fact(&db, &ross, ross.account_id, &vault, &quick_fact_template_entry).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateQuickFactTemplatePositionTest

async fn update_quick_fact_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    quick_fact_template_entry: &vault_quick_facts_templates::Model,
) -> ApiResult<()> {
    let at = |position: i32| vault_quick_facts_templates::ActiveModel {
        position: Set(position),
        ..quick_fact_template(vault.id)
    };
    let quick_fact_template_entry1 = at(1).insert(&db.conn).await.unwrap();
    let quick_fact_template_entry3 = at(3).insert(&db.conn).await.unwrap();
    let quick_fact_template_entry4 = at(4).insert(&db.conn).await.unwrap();

    let request = |new_position: i32| UpdateQuickFactTemplatePosition {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        vault_quick_facts_template_id: quick_fact_template_entry.id,
        new_position,
    };
    request(3).execute(&db.conn).await?;

    for (id, position) in [
        (quick_fact_template_entry1.id, 1),
        (quick_fact_template_entry3.id, 2),
        (quick_fact_template_entry4.id, 4),
        (quick_fact_template_entry.id, 3),
    ] {
        let row = entry_row(db, id).await;
        assert_eq!((row.vault_id, row.position), (vault.id, position));
    }

    request(2).execute(&db.conn).await?;

    for (id, position) in [
        (quick_fact_template_entry1.id, 1),
        (quick_fact_template_entry3.id, 3),
        (quick_fact_template_entry4.id, 4),
        (quick_fact_template_entry.id, 2),
    ] {
        let row = entry_row(db, id).await;
        assert_eq!((row.vault_id, row.position), (vault.id, position));
    }
    Ok(())
}

#[tokio::test]
async fn it_updates_a_quick_fact_template_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(vault.id).insert(&db.conn).await.unwrap();
    update_quick_fact_position(&db, &ross, ross.account_id, &vault, &quick_fact_template_entry).await.unwrap();
    db.drop().await;
}

// update_quick_fact_position_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn update_quick_fact_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(vault.id).insert(&db.conn).await.unwrap();
    let err = update_quick_fact_position(&db, &ross, account.id, &vault, &quick_fact_template_entry).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_quick_fact_position_fails_if_parameter_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let quick_fact_template_entry = quick_fact_template(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err =
        update_quick_fact_position(&db, &ross, ross.account_id, &vault, &quick_fact_template_entry).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
