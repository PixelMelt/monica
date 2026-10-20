//! `CreateMoodTrackingParameterTest`, `DestroyMoodTrackingParameterTest`,
//! `UpdateMoodTrackingParameterTest` and `UpdateMoodTrackingParameterPositionTest`.

use super::super::*;
use super::any_vault;
use crate::crypto::random_string;
use crate::entities::mood_tracking_parameters;
use crate::testing::{TestDb, factory};

/// `MoodTrackingParameter::factory()` in `vault_id`, to insert.
fn mood_tracking_parameter(vault_id: Uuid) -> mood_tracking_parameters::ActiveModel {
    mood_tracking_parameters::ActiveModel {
        vault_id: Set(vault_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        hex_color: Set(format!("Name {}", random_string(6))),
        position: Set(Some(1)),
        ..Default::default()
    }
}

async fn parameter_row(db: &TestDb, id: Id) -> mood_tracking_parameters::Model {
    mood_tracking_parameters::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap()
}

// ---------------------------------------------------------------- CreateMoodTrackingParameterTest

async fn create_parameter(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
) -> ApiResult<()> {
    let mood_tracking_parameter = CreateMoodTrackingParameter {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        label: "label name".into(),
        hex_color: "bg-zinc-700".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = parameter_row(db, mood_tracking_parameter.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label.as_deref(), Some("label name"));
    assert_eq!(row.hex_color, "bg-zinc-700");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_mood_tracking_parameter() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    create_parameter(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_parameter_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let err = CreateMoodTrackingParameter {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        label: String::new(),
        hex_color: "bg-zinc-700".into(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_parameter_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = create_parameter(&db, &ross, account.id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_parameter_fails_if_user_is_not_editor() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_parameter(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_parameter_fails_if_vault_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = any_vault(&db).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_parameter(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyMoodTrackingParameterTest

async fn destroy_parameter(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    mood_tracking_parameter: &mood_tracking_parameters::Model,
) -> ApiResult<()> {
    DestroyMoodTrackingParameter {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        mood_tracking_parameter_id: mood_tracking_parameter.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(
        mood_tracking_parameters::Entity::find_by_id(mood_tracking_parameter.id).one(&db.conn).await.unwrap().is_none()
    );
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_mood_tracking_parameter() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(vault.id).insert(&db.conn).await.unwrap();
    destroy_parameter(&db, &ross, ross.account_id, &vault, &mood_tracking_parameter).await.unwrap();
    db.drop().await;
}

// destroy_parameter_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_parameter_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_parameter(&db, &ross, account.id, &vault, &mood_tracking_parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_parameter_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy_parameter(&db, &ross, account.id, &vault, &mood_tracking_parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_parameter_fails_if_parameter_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy_parameter(&db, &ross, ross.account_id, &vault, &mood_tracking_parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_parameter_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(vault.id).insert(&db.conn).await.unwrap();
    let err = destroy_parameter(&db, &ross, ross.account_id, &vault, &mood_tracking_parameter).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateMoodTrackingParameterTest

async fn update_parameter(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    mood_tracking_parameter: &mood_tracking_parameters::Model,
) -> ApiResult<()> {
    let mood_tracking_parameter = UpdateMoodTrackingParameter {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        mood_tracking_parameter_id: mood_tracking_parameter.id,
        label: "label name".into(),
        hex_color: "bg-zinc-700".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = parameter_row(db, mood_tracking_parameter.id).await;
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label.as_deref(), Some("label name"));
    assert_eq!(row.hex_color, "bg-zinc-700");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_mood_tracking_parameter() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(vault.id).insert(&db.conn).await.unwrap();
    update_parameter(&db, &ross, ross.account_id, &vault, &mood_tracking_parameter).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_parameter_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    let mood_tracking_parameter = mood_tracking_parameter(vault.id).insert(&db.conn).await.unwrap();
    let err = UpdateMoodTrackingParameter {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        mood_tracking_parameter_id: mood_tracking_parameter.id,
        label: "label name".into(),
        hex_color: "x".repeat(256),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_parameter_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(vault.id).insert(&db.conn).await.unwrap();
    let err = update_parameter(&db, &ross, account.id, &vault, &mood_tracking_parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_parameter_fails_if_parameter_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err = update_parameter(&db, &ross, ross.account_id, &vault, &mood_tracking_parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_parameter_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(vault.id).insert(&db.conn).await.unwrap();
    let err = update_parameter(&db, &ross, ross.account_id, &vault, &mood_tracking_parameter).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateMoodTrackingParameterPositionTest

async fn update_parameter_position(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    mood_tracking_parameter: &mood_tracking_parameters::Model,
) -> ApiResult<()> {
    let at = |position: i32| mood_tracking_parameters::ActiveModel {
        position: Set(Some(position)),
        ..self::mood_tracking_parameter(vault.id)
    };
    let mood_tracking_parameter1 = at(1).insert(&db.conn).await.unwrap();
    let mood_tracking_parameter3 = at(3).insert(&db.conn).await.unwrap();
    let mood_tracking_parameter4 = at(4).insert(&db.conn).await.unwrap();

    let request = |new_position: i32| UpdateMoodTrackingParameterPosition {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        mood_tracking_parameter_id: mood_tracking_parameter.id,
        new_position,
    };
    request(3).execute(&db.conn).await?;

    for (id, position) in [
        (mood_tracking_parameter1.id, 1),
        (mood_tracking_parameter3.id, 2),
        (mood_tracking_parameter4.id, 4),
        (mood_tracking_parameter.id, 3),
    ] {
        let row = parameter_row(db, id).await;
        assert_eq!((row.vault_id, row.position), (vault.id, Some(position)));
    }

    request(2).execute(&db.conn).await?;

    for (id, position) in [
        (mood_tracking_parameter1.id, 1),
        (mood_tracking_parameter3.id, 3),
        (mood_tracking_parameter4.id, 4),
        (mood_tracking_parameter.id, 2),
    ] {
        let row = parameter_row(db, id).await;
        assert_eq!((row.vault_id, row.position), (vault.id, Some(position)));
    }
    Ok(())
}

#[tokio::test]
async fn it_updates_a_mood_tracking_parameter_position() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(vault.id).insert(&db.conn).await.unwrap();
    update_parameter_position(&db, &ross, ross.account_id, &vault, &mood_tracking_parameter).await.unwrap();
    db.drop().await;
}

// update_parameter_position_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn update_parameter_position_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(vault.id).insert(&db.conn).await.unwrap();
    let err = update_parameter_position(&db, &ross, account.id, &vault, &mood_tracking_parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_parameter_position_fails_if_parameter_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let mood_tracking_parameter = mood_tracking_parameter(any_vault(&db).await.id).insert(&db.conn).await.unwrap();
    let err =
        update_parameter_position(&db, &ross, ross.account_id, &vault, &mood_tracking_parameter).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
