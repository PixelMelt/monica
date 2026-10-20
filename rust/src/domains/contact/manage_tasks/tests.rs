//! `tests/Unit/Domains/Contact/ManageTasks/Services`.

use chrono::NaiveDate;
use sea_orm::EntityTrait;

use super::*;
use crate::crypto::random_string;
use crate::entities::vaults;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `ContactTask::factory()` for `contact`, with a new author.
async fn contact_task(db: &TestDb, contact: &contacts::Model) -> contact_tasks::Model {
    let author = factory::create_user(&db.conn).await;
    contact_tasks::ActiveModel {
        contact_id: Set(contact.id),
        author_id: Set(Some(author.id)),
        author_name: Set(format!("Name {}", random_string(6))),
        label: Set(format!("Sentence {}", random_string(10))),
        completed: Set(false),
        due_at: Set(Some(crate::db::now())),
        uuid: Set(Some(Uuid::new())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("contact task")
}

/// `Contact::factory()->create()`: a contact in a new vault of a new account.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `$regis` with `permission` in a new vault of their account.
async fn regis_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

async fn task_row(db: &TestDb, id: Id) -> contact_tasks::Model {
    contact_tasks::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap()
}

// ---- CreateContactTaskTest

async fn create_task(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    let task = CreateContactTask {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        label: "birthdate".into(),
        description: None,
        due_at: Some("1990-01-01".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = task_row(db, task.id).await;
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.label, "birthdate");
    assert_eq!(row.due_at, Some(Timestamp(NaiveDate::from_ymd_opt(1990, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap())));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_task() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    create_task(&db, &regis, regis.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = CreateContactTask {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        label: String::new(),
        description: None,
        due_at: Some("not a date".into()),
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
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = create_task(&db, &regis, account.id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let err = create_task(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = create_task(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---- DestroyContactTaskTest

async fn destroy_task(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    task: &contact_tasks::Model,
) -> ApiResult<()> {
    let before = crate::db::now();
    DestroyContactTask {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_task_id: task.id,
    }
    .execute(&db.conn)
    .await?;

    let deleted_at = task_row(db, task.id).await.deleted_at.unwrap();
    assert!(before <= deleted_at && deleted_at <= crate::db::now());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_task() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    destroy_task(&db, &regis, regis.account_id, &vault, &contact, &task).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    let err = destroy_task(&db, &regis, account.id, &vault, &contact, &task).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let task = contact_task(&db, &contact).await;
    let err = destroy_task(&db, &regis, regis.account_id, &vault, &contact, &task).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    let err = destroy_task(&db, &regis, regis.account_id, &vault, &contact, &task).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_reminder_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &other_contact(&db).await).await;
    let err = destroy_task(&db, &regis, regis.account_id, &vault, &contact, &task).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---- ToggleContactTaskTest

async fn toggle_task(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    task: &contact_tasks::Model,
) -> ApiResult<()> {
    let task = ToggleContactTask {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_task_id: task.id,
    }
    .execute(&db.conn)
    .await?;

    let row = task_row(db, task.id).await;
    assert_eq!(row.contact_id, contact.id);
    assert!(row.completed);
    Ok(())
}

#[tokio::test]
async fn it_toggles_a_task() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    toggle_task(&db, &regis, regis.account_id, &vault, &contact, &task).await.unwrap();
    db.drop().await;
}

/// Q6: toggling a task back to not completed clears `completed_at`.
#[tokio::test]
async fn toggle_clears_completed_at_when_the_task_is_not_completed_anymore() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    let toggle = || ToggleContactTask {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        contact_task_id: task.id,
    };
    let completed = toggle().execute(&db.conn).await.unwrap();
    assert!(completed.completed && completed.completed_at.is_some());
    toggle().execute(&db.conn).await.unwrap();
    let row = task_row(&db, task.id).await;
    assert!(!row.completed);
    assert_eq!(row.completed_at, None);
    db.drop().await;
}

// toggle_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn toggle_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    let err = toggle_task(&db, &regis, account.id, &vault, &contact, &task).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let task = contact_task(&db, &contact).await;
    let err = toggle_task(&db, &regis, regis.account_id, &vault, &contact, &task).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    let err = toggle_task(&db, &regis, regis.account_id, &vault, &contact, &task).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn toggle_fails_if_reminder_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &other_contact(&db).await).await;
    let err = toggle_task(&db, &regis, regis.account_id, &vault, &contact, &task).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---- UpdateContactTaskTest

fn update_request(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    task: &contact_tasks::Model,
) -> UpdateContactTask {
    UpdateContactTask {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_task_id: task.id,
        label: "birthdate".into(),
        description: None,
        due_at: None,
    }
}

async fn update_task(db: &TestDb, request: UpdateContactTask) -> ApiResult<()> {
    let contact_id = request.contact_id;
    let task = request.execute(&db.conn).await?;

    let row = task_row(db, task.id).await;
    assert_eq!(row.contact_id, contact_id);
    assert_eq!(row.label, "birthdate");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_task() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    update_task(&db, update_request(&regis, regis.account_id, &vault, &contact, &task)).await.unwrap();
    db.drop().await;
}

/// F8: UpdateContactTask takes `description` like CreateContactTask.
#[tokio::test]
async fn update_saves_the_description() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &task);
    update_task(&db, UpdateContactTask { description: Some("call the plumber".into()), ..request }).await.unwrap();
    assert_eq!(task_row(&db, task.id).await.description.as_deref(), Some("call the plumber"));
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    let request = update_request(&regis, regis.account_id, &vault, &contact, &task);
    let err = UpdateContactTask { label: "x".repeat(256), ..request }.execute(&db.conn).await.unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    let err = update_task(&db, update_request(&regis, account.id, &vault, &contact, &task)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let task = contact_task(&db, &contact).await;
    let err = update_task(&db, update_request(&regis, regis.account_id, &vault, &contact, &task)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &contact).await;
    let err = update_task(&db, update_request(&regis, regis.account_id, &vault, &contact, &task)).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_reminder_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let task = contact_task(&db, &other_contact(&db).await).await;
    let err = update_task(&db, update_request(&regis, regis.account_id, &vault, &contact, &task)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
