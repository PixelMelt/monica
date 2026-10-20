//! `tests/Unit/Domains/Contact/ManageGoals/Services`.

use chrono::NaiveDate;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::*;
use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Contact::factory()->create()`: a contact in a new vault.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `Goal::factory()` for `contact_id` (a new contact when `None`).
async fn goal(db: &TestDb, contact_id: Option<Uuid>) -> goals::Model {
    let contact_id = match contact_id {
        Some(id) => id,
        None => other_contact(db).await.id,
    };
    goals::ActiveModel {
        contact_id: Set(contact_id),
        name: Set(format!("word{}", random_string(6))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("goal")
}

/// `createUser()`, `createVault()` and `setPermissionInVault()` with `permission`.
async fn regis_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

// ---------------------------------------------------------------- CreateGoalTest

async fn create_goal(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    let goal = CreateGoal {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        name: Some("super title".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = goals::Entity::find_by_id(goal.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.name, "super title");

    assert!(
        contact_feed_items::Entity::find()
            .filter(contact_feed_items::Column::ContactId.eq(contact.id))
            .filter(contact_feed_items::Column::FeedableId.eq(goal.id))
            .filter(contact_feed_items::Column::FeedableType.eq("App\\Models\\Goal"))
            .filter(contact_feed_items::Column::Description.eq("super title"))
            .one(&db.conn)
            .await
            .unwrap()
            .is_some()
    );
    Ok(())
}

#[tokio::test]
async fn it_creates_a_goal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    create_goal(&db, &regis, regis.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = CreateGoal {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        name: Some("x".repeat(256)),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = create_goal(&db, &regis, account.id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let err = create_goal(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = create_goal(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyGoalTest

async fn destroy_goal(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    goal: &goals::Model,
) -> ApiResult<()> {
    DestroyGoal { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id, goal_id: goal.id }
        .execute(&db.conn)
        .await?;

    assert!(goals::Entity::find_by_id(goal.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_goal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    destroy_goal(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = destroy_goal(&db, &regis, account.id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = destroy_goal(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = destroy_goal(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_note_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, None).await;
    let err = destroy_goal(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- ToggleStreakTest

async fn toggle_streak(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    goal: &goals::Model,
) -> ApiResult<()> {
    let request = || ToggleStreak {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        goal_id: Some(goal.id),
        happened_at: "1900-01-01".into(),
    };
    let happened_at = Timestamp(NaiveDate::from_ymd_opt(1900, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap());
    let streak = || {
        streaks::Entity::find()
            .filter(streaks::Column::GoalId.eq(goal.id))
            .filter(streaks::Column::HappenedAt.eq(happened_at))
            .one(&db.conn)
    };

    request().execute(&db.conn).await?;
    assert!(streak().await.unwrap().is_some());

    request().execute(&db.conn).await?;
    assert!(streak().await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_creates_a_streak() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    toggle_streak(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn toggle_streak_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = ToggleStreak {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        goal_id: Some(goal.id),
        happened_at: "1900-1-1".into(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn toggle_streak_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = toggle_streak(&db, &regis, account.id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_streak_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = toggle_streak(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_streak_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = toggle_streak(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn toggle_streak_fails_if_goal_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, None).await;
    let err = toggle_streak(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateGoalTest

async fn update_goal(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    goal: &goals::Model,
) -> ApiResult<()> {
    let goal = UpdateGoal {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        goal_id: Some(goal.id),
        name: Some("super title".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = goals::Entity::find_by_id(goal.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.name, "super title");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_note() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    update_goal(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = UpdateGoal {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        goal_id: Some(goal.id),
        name: Some("x".repeat(256)),
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
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = update_goal(&db, &regis, account.id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = update_goal(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, Some(contact.id)).await;
    let err = update_goal(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_goal_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let goal = goal(&db, None).await;
    let err = update_goal(&db, &regis, regis.account_id, &vault, &contact, &goal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
