//! `CreateLifeEventCategoryTest`, `CreateLifeEventTypeTest`,
//! `DestroyLifeEventCategoryTest`, `DestroyLifeEventTypeTest`,
//! `UpdateLifeEventCategoryTest`, `UpdateLifeEventCategoryPositionTest`,
//! `UpdateLifeEventTypeTest` and `UpdateLifeEventTypePositionTest`.

use super::super::*;
use super::any_vault;
use crate::crypto::random_string;
use crate::entities::{life_event_categories, life_event_types};
use crate::testing::{TestDb, factory};

/// `LifeEventCategory::factory()` in `vault_id`, to insert.
fn life_event_category(vault_id: Uuid) -> life_event_categories::ActiveModel {
    life_event_categories::ActiveModel {
        vault_id: Set(vault_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        label_translation_key: Set(Some(format!("Name {}", random_string(6)))),
        can_be_deleted: Set(true),
        position: Set(Some(1)),
        ..Default::default()
    }
}

/// `LifeEventType::factory()` in `life_event_category_id`, to insert.
fn life_event_type(life_event_category_id: Id) -> life_event_types::ActiveModel {
    life_event_types::ActiveModel {
        life_event_category_id: Set(life_event_category_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        label_translation_key: Set(Some(format!("Name {}", random_string(6)))),
        can_be_deleted: Set(true),
        position: Set(Some(1)),
        ..Default::default()
    }
}

/// `LifeEventCategory::factory()->create()`: in a new vault of a new account.
async fn any_category(db: &TestDb) -> life_event_categories::Model {
    life_event_category(any_vault(db).await.id).insert(&db.conn).await.unwrap()
}

async fn category_row(db: &TestDb, id: Id) -> life_event_categories::Model {
    life_event_categories::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap()
}

async fn type_row(db: &TestDb, id: Id) -> life_event_types::Model {
    life_event_types::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap()
}

// ---------------------------------------------------------------- CreateLifeEventCategoryTest

async fn create_category(db: &TestDb, author: &users::Model, account_id: Uuid, vault: &vaults::Model) -> ApiResult<()> {
    let r#type = CreateLifeEventCategory {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        label: "type name".into(),
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await?;

    let row = category_row(db, r#type.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    assert!(row.can_be_deleted);
    Ok(())
}

#[tokio::test]
async fn it_creates_a_life_event_category() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    create_category(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_category_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let err = CreateLifeEventCategory {
        account_id: ross.account_id,
        vault_id: vault.id,
        author_id: ross.id,
        label: String::new(),
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_category_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let err = create_category(&db, &ross, account.id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_category_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let err = create_category(&db, &ross, vault.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_category_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_category(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateLifeEventTypeTest

async fn create_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    category: &life_event_categories::Model,
) -> ApiResult<()> {
    let r#type = CreateLifeEventType {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        life_event_category_id: category.id,
        label: "type name".into(),
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await?;

    let row = type_row(db, r#type.id).await;
    assert_eq!(row.life_event_category_id, category.id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    assert!(row.can_be_deleted);
    Ok(())
}

#[tokio::test]
async fn it_creates_a_life_event_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    create_type(&db, &ross, ross.account_id, &vault, &category).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_type_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let err = CreateLifeEventType {
        account_id: ross.account_id,
        vault_id: vault.id,
        author_id: ross.id,
        life_event_category_id: category.id,
        label: "x".repeat(256),
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let err = create_type(&db, &ross, account.id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_type_fails_if_user_is_not_administrator() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let err = create_type(&db, &ross, ross.account_id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_type_fails_if_category_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = any_category(&db).await;
    let err = create_type(&db, &ross, account.id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyLifeEventCategoryTest

async fn destroy_category(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    category: &life_event_categories::Model,
) -> ApiResult<()> {
    DestroyLifeEventCategory {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        life_event_category_id: category.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(life_event_categories::Entity::find_by_id(category.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_category() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    destroy_category(&db, &ross, ross.account_id, &vault, &category).await.unwrap();
    db.drop().await;
}

// destroy_category_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_category_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_category(&db, &ross, account.id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_category_fails_if_category_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = any_category(&db).await;
    let err = destroy_category(&db, &ross, ross.account_id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_category_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_category(&db, &ross, ross.account_id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

/// Not in the PHP tests: `CantBeDeletedException` (F18) is a 409.
#[tokio::test]
async fn destroy_category_fails_if_it_cant_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_categories::ActiveModel { can_be_deleted: Set(false), ..life_event_category(vault.id) }
        .insert(&db.conn)
        .await
        .unwrap();
    let err = destroy_category(&db, &ross, ross.account_id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::Conflict);
    assert!(life_event_categories::Entity::find_by_id(category.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyLifeEventTypeTest

async fn destroy_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    category: &life_event_categories::Model,
    r#type: &life_event_types::Model,
) -> ApiResult<()> {
    DestroyLifeEventType {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        life_event_category_id: category.id,
        life_event_type_id: r#type.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(life_event_types::Entity::find_by_id(r#type.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_type(category.id).insert(&db.conn).await.unwrap();
    destroy_type(&db, &ross, ross.account_id, &vault, &category, &r#type).await.unwrap();
    db.drop().await;
}

// destroy_type_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_type(category.id).insert(&db.conn).await.unwrap();
    let err = destroy_type(&db, &ross, account.id, &vault, &category, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_type_fails_if_type_doesnt_belong_to_category() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_type(any_category(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy_type(&db, &ross, ross.account_id, &vault, &category, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_type_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_type(category.id).insert(&db.conn).await.unwrap();
    let err = destroy_type(&db, &ross, ross.account_id, &vault, &category, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_type_fails_if_category_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = any_category(&db).await;
    let r#type = life_event_type(category.id).insert(&db.conn).await.unwrap();
    let err = destroy_type(&db, &ross, ross.account_id, &vault, &category, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

/// Not in the PHP tests: `CantBeDeletedException` (F18) is a 409.
#[tokio::test]
async fn destroy_type_fails_if_it_cant_be_deleted() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_types::ActiveModel { can_be_deleted: Set(false), ..life_event_type(category.id) }
        .insert(&db.conn)
        .await
        .unwrap();
    let err = destroy_type(&db, &ross, ross.account_id, &vault, &category, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::Conflict);
    assert!(life_event_types::Entity::find_by_id(r#type.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateLifeEventCategoryTest

async fn update_category(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    category: &life_event_categories::Model,
) -> ApiResult<()> {
    let category = UpdateLifeEventCategory {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        life_event_category_id: category.id,
        label: "type name".into(),
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await?;

    let row = category_row(db, category.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    assert!(row.can_be_deleted);
    Ok(())
}

#[tokio::test]
async fn it_updates_a_category() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    update_category(&db, &ross, ross.account_id, &vault, &category).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_category_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let err = UpdateLifeEventCategory {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        life_event_category_id: category.id,
        label: String::new(),
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_category_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let err = update_category(&db, &ross, account.id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_category_fails_if_category_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = any_category(&db).await;
    let err = update_category(&db, &ross, ross.account_id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_category_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let err = update_category(&db, &ross, ross.account_id, &vault, &category).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateLifeEventCategoryPositionTest

async fn update_category_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    life_event_category: &life_event_categories::Model,
) -> ApiResult<()> {
    let at = |position: i32| life_event_categories::ActiveModel {
        position: Set(Some(position)),
        ..self::life_event_category(vault.id)
    };
    let life_event_category1 = at(1).insert(&db.conn).await.unwrap();
    let life_event_category3 = at(3).insert(&db.conn).await.unwrap();
    let life_event_category4 = at(4).insert(&db.conn).await.unwrap();

    let request = |new_position: i32| UpdateLifeEventCategoryPosition {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        life_event_category_id: life_event_category.id,
        new_position,
    };
    request(3).execute(&db.conn).await?;

    for (id, position) in [
        (life_event_category1.id, 1),
        (life_event_category3.id, 2),
        (life_event_category4.id, 4),
        (life_event_category.id, 3),
    ] {
        let row = category_row(db, id).await;
        assert_eq!((row.vault_id, row.position), (vault.id, Some(position)));
    }

    request(2).execute(&db.conn).await?;

    for (id, position) in [
        (life_event_category1.id, 1),
        (life_event_category3.id, 3),
        (life_event_category4.id, 4),
        (life_event_category.id, 2),
    ] {
        let row = category_row(db, id).await;
        assert_eq!((row.vault_id, row.position), (vault.id, Some(position)));
    }
    Ok(())
}

#[tokio::test]
async fn it_updates_a_life_event_category_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let life_event_category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    update_category_position(&db, &ross, ross.account_id, &vault, &life_event_category).await.unwrap();
    db.drop().await;
}

// update_category_position_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn update_category_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let life_event_category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let err = update_category_position(&db, &ross, account.id, &vault, &life_event_category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_category_position_fails_if_life_event_category_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let life_event_category = any_category(&db).await;
    let err = update_category_position(&db, &ross, ross.account_id, &vault, &life_event_category).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateLifeEventTypeTest

async fn update_type(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    category: &life_event_categories::Model,
    r#type: &life_event_types::Model,
) -> ApiResult<()> {
    let r#type = UpdateLifeEventType {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        life_event_category_id: category.id,
        life_event_type_id: r#type.id,
        label: "type name".into(),
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await?;

    let row = type_row(db, r#type.id).await;
    assert_eq!(row.life_event_category_id, category.id);
    assert_eq!(row.label.as_deref(), Some("type name"));
    assert!(row.can_be_deleted);
    Ok(())
}

#[tokio::test]
async fn it_updates_a_type() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_type(category.id).insert(&db.conn).await.unwrap();
    update_type(&db, &ross, ross.account_id, &vault, &category, &r#type).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_type_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_type(category.id).insert(&db.conn).await.unwrap();
    let err = UpdateLifeEventType {
        account_id: ross.account_id,
        vault_id: vault.id,
        author_id: ross.id,
        life_event_category_id: category.id,
        life_event_type_id: r#type.id,
        label: String::new(),
        can_be_deleted: true,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_type_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_type(category.id).insert(&db.conn).await.unwrap();
    let err = update_type(&db, &ross, account.id, &vault, &category, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_type_fails_if_category_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = any_category(&db).await;
    let r#type = life_event_type(category.id).insert(&db.conn).await.unwrap();
    let err = update_type(&db, &ross, ross.account_id, &vault, &category, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_type_fails_if_type_doesnt_belong_to_category() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_type(any_category(&db).await.id).insert(&db.conn).await.unwrap();
    let err = update_type(&db, &ross, ross.account_id, &vault, &category, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_type_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let r#type = life_event_type(category.id).insert(&db.conn).await.unwrap();
    let err = update_type(&db, &ross, ross.account_id, &vault, &category, &r#type).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateLifeEventTypePositionTest

async fn update_type_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    life_event_category: &life_event_categories::Model,
    life_event_type: &life_event_types::Model,
) -> ApiResult<()> {
    let at = |position: i32| life_event_types::ActiveModel {
        position: Set(Some(position)),
        ..self::life_event_type(life_event_category.id)
    };
    let life_event_type1 = at(1).insert(&db.conn).await.unwrap();
    let life_event_type3 = at(3).insert(&db.conn).await.unwrap();
    let life_event_type4 = at(4).insert(&db.conn).await.unwrap();

    let request = |new_position: i32| UpdateLifeEventTypePosition {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        life_event_category_id: life_event_category.id,
        life_event_type_id: life_event_type.id,
        new_position,
    };
    request(3).execute(&db.conn).await?;

    for (id, position) in
        [(life_event_type1.id, 1), (life_event_type3.id, 2), (life_event_type4.id, 4), (life_event_type.id, 3)]
    {
        let row = type_row(db, id).await;
        assert_eq!((row.life_event_category_id, row.position), (life_event_category.id, Some(position)));
    }

    request(2).execute(&db.conn).await?;

    for (id, position) in
        [(life_event_type1.id, 1), (life_event_type3.id, 3), (life_event_type4.id, 4), (life_event_type.id, 2)]
    {
        let row = type_row(db, id).await;
        assert_eq!((row.life_event_category_id, row.position), (life_event_category.id, Some(position)));
    }
    Ok(())
}

#[tokio::test]
async fn it_updates_a_life_event_type_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let life_event_category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let life_event_type =
        life_event_types::ActiveModel { position: Set(Some(2)), ..life_event_type(life_event_category.id) }
            .insert(&db.conn)
            .await
            .unwrap();
    update_type_position(&db, &ross, ross.account_id, &vault, &life_event_category, &life_event_type).await.unwrap();
    db.drop().await;
}

// update_type_position_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn update_type_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let life_event_category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let life_event_type = life_event_type(life_event_category.id).insert(&db.conn).await.unwrap();
    let err =
        update_type_position(&db, &ross, account.id, &vault, &life_event_category, &life_event_type).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_type_position_fails_if_life_event_type_doesnt_belong_to_category() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let life_event_category = life_event_category(vault.id).insert(&db.conn).await.unwrap();
    let life_event_type = life_event_type(any_category(&db).await.id).insert(&db.conn).await.unwrap();
    let err = update_type_position(&db, &ross, ross.account_id, &vault, &life_event_category, &life_event_type)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_type_position_fails_if_life_event_category_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Edit, &vault).await;
    let life_event_category = any_category(&db).await;
    let life_event_type = life_event_type(life_event_category.id).insert(&db.conn).await.unwrap();
    let err = update_type_position(&db, &ross, ross.account_id, &vault, &life_event_category, &life_event_type)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
