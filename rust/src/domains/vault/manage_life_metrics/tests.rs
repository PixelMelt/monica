//! `tests/Unit/Domains/Vault/ManageLifeMetrics/Services`.

use super::*;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `LifeMetric::factory()` in `vault_id` (a new vault of a new account when `None`).
async fn life_metric(db: &TestDb, vault_id: Option<Uuid>) -> life_metrics::Model {
    let vault_id = match vault_id {
        Some(v) => v,
        None => factory::vault(&db.conn, factory::account(&db.conn).await.id).await.id,
    };
    life_metrics::ActiveModel {
        vault_id: Set(vault_id),
        label: Set(format!("Name {}", crate::crypto::random_string(6))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("life_metric")
}

// ---------------------------------------------------------------- CreateLifeMetricTest

async fn create_life_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
) -> ApiResult<()> {
    let life_metric =
        CreateLifeMetric { account_id, author_id: author.id, vault_id: vault.id, label: "label name".into() }
            .execute(&db.conn)
            .await?;

    let row = life_metrics::Entity::find_by_id(life_metric.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label, "label name");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_life_metric() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    create_life_metric(&db, &ross, ross.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err =
        CreateLifeMetric { account_id: ross.account_id, author_id: ross.id, vault_id: vault.id, label: "".into() }
            .execute(&db.conn)
            .await
            .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = create_life_metric(&db, &ross, account.id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_is_not_editor() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_life_metric(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_vault_is_not_in_the_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, factory::account(&db.conn).await.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let err = create_life_metric(&db, &ross, ross.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyLifeMetricTest

async fn destroy_life_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    life_metric: &life_metrics::Model,
) -> ApiResult<()> {
    DestroyLifeMetric { account_id, author_id: author.id, life_metric_id: life_metric.id, vault_id: vault.id }
        .execute(&db.conn)
        .await?;

    assert!(life_metrics::Entity::find_by_id(life_metric.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_life_metric() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    destroy_life_metric(&db, &ross, ross.account_id, &vault, &life_metric).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    let err = destroy_life_metric(&db, &ross, account.id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, None).await;
    let err = destroy_life_metric(&db, &ross, account.id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_life_metric_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, None).await;
    let err = destroy_life_metric(&db, &ross, ross.account_id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    let err = destroy_life_metric(&db, &ross, ross.account_id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- IncrementLifeMetricTest

async fn increment_life_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    life_metric: &life_metrics::Model,
) -> ApiResult<()> {
    IncrementLifeMetric { account_id, author_id: author.id, vault_id: vault.id, life_metric_id: life_metric.id }
        .execute(&db.conn)
        .await?;

    let contact_id =
        user_vault::Entity::find_by_id((vault.id, author.id)).one(&db.conn).await.unwrap().unwrap().contact_id;
    let row = contact_life_metric::Entity::find()
        .filter(contact_life_metric::Column::ContactId.eq(contact_id))
        .filter(contact_life_metric::Column::LifeMetricId.eq(life_metric.id))
        .one(&db.conn)
        .await
        .unwrap();
    assert!(row.is_some());
    Ok(())
}

#[tokio::test]
async fn it_increments_a_life_metric() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    increment_life_metric(&db, &ross, ross.account_id, &vault, &life_metric).await.unwrap();
    db.drop().await;
}

// increment_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn increment_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    let err = increment_life_metric(&db, &ross, account.id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn increment_fails_if_life_metric_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, None).await;
    let err = increment_life_metric(&db, &ross, ross.account_id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn increment_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    let err = increment_life_metric(&db, &ross, ross.account_id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateLifeMetricTest

async fn update_life_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    life_metric: &life_metrics::Model,
) -> ApiResult<()> {
    let life_metric = UpdateLifeMetric {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        life_metric_id: life_metric.id,
        label: "label name".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = life_metrics::Entity::find_by_id(life_metric.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label, "label name");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_life_metric() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    update_life_metric(&db, &ross, ross.account_id, &vault, &life_metric).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    let err = UpdateLifeMetric {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        life_metric_id: life_metric.id,
        label: "x".repeat(256),
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
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    let err = update_life_metric(&db, &ross, account.id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_life_metric_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let life_metric = life_metric(&db, None).await;
    let err = update_life_metric(&db, &ross, ross.account_id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let life_metric = life_metric(&db, Some(vault.id)).await;
    let err = update_life_metric(&db, &ross, ross.account_id, &vault, &life_metric).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}
