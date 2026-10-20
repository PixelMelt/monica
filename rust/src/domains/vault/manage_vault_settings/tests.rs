//! `tests/Unit/Domains/Vault/ManageVaultSettings/Services`.

use chrono::NaiveTime;
use sea_orm::{EntityTrait, PaginatorTrait};

use super::*;
use crate::crypto::random_string;
use crate::testing::{TestDb, factory};

/// `UserNotificationChannel::factory()` for `user_id`, to insert.
fn user_notification_channel(user_id: Uuid) -> user_notification_channels::ActiveModel {
    user_notification_channels::ActiveModel {
        user_id: Set(Some(user_id)),
        r#type: Set("email".into()),
        label: Set(Some(random_string(6))),
        content: Set("admin@admin.com".into()),
        active: Set(true),
        fails: Set(0),
        verified_at: Set(None),
        preferred_time: Set(NaiveTime::from_hms_opt(9, 0, 0)),
        ..Default::default()
    }
}

/// `ContactReminder::factory()` for `contact_id`, to insert.
fn contact_reminder(contact_id: Uuid) -> contact_reminders::ActiveModel {
    contact_reminders::ActiveModel {
        contact_id: Set(contact_id),
        label: Set(format!("Label {}", random_string(8))),
        day: Set(Some(29)),
        month: Set(Some(10)),
        year: Set(Some(1981)),
        r#type: Set("one_time".into()),
        frequency_number: Set(Some(1)),
        last_triggered_at: Set(None),
        number_times_triggered: Set(0),
        ..Default::default()
    }
}

/// `Template::factory()` in `account_id` (a new account when `None`).
async fn template(db: &TestDb, account_id: Option<Uuid>) -> templates::Model {
    let account_id = match account_id {
        Some(a) => a,
        None => factory::account(&db.conn).await.id,
    };
    templates::ActiveModel { account_id: Set(account_id), name: Set(Some("business".into())), ..Default::default() }
        .insert(&db.conn)
        .await
        .expect("template")
}

/// `Contact::factory()->create()`: in a new vault of a new account.
async fn any_contact(db: &TestDb) -> contacts::Model {
    let vault = factory::vault(&db.conn, factory::account(&db.conn).await.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `$vault->users()->save($user, ['permission' => Vault::PERMISSION_MANAGE, 'contact_id' => $contact_id])`.
async fn save_vault_manager(db: &TestDb, vault: &vaults::Model, user: &users::Model, contact_id: Uuid) {
    user_vault::ActiveModel {
        vault_id: Set(vault.id),
        user_id: Set(user.id),
        contact_id: Set(contact_id),
        permission: Set(VaultPermission::Manage as i32),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("user_vault");
}

// ---------------------------------------------------------------- ChangeVaultAccessTest

async fn change_access(
    db: &TestDb,
    account_id: Uuid,
    regis: &users::Model,
    another_user: &users::Model,
    vault: &vaults::Model,
) -> ApiResult<()> {
    ChangeVaultAccess {
        account_id,
        author_id: regis.id,
        vault_id: vault.id,
        user_id: another_user.id,
        permission: VaultPermission::View as i32,
    }
    .execute(&db.conn)
    .await?;

    let pivot = user_vault::Entity::find_by_id((vault.id, another_user.id)).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(pivot.permission, VaultPermission::View as i32);
    Ok(())
}

#[tokio::test]
async fn it_changes_the_permission_in_the_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_administrator(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(regis.account_id)).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Manage, &vault).await;
    save_vault_manager(&db, &vault, &another_user, any_contact(&db).await.id).await;
    change_access(&db, regis.account_id, &regis, &another_user, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(regis.account_id)).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Manage, &vault).await;
    let err = change_access(&db, account.id, &regis, &another_user, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_the_other_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_administrator(&db.conn).await;
    let another_user = factory::user(&db.conn, None).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Manage, &vault).await;
    let err = change_access(&db, regis.account_id, &regis, &another_user, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_is_not_vault_manager() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(regis.account_id)).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Edit, &vault).await;
    let err = change_access(&db, regis.account_id, &regis, &another_user, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Manage, &vault).await;
    let err = ChangeVaultAccess {
        account_id: regis.account_id,
        author_id: regis.id,
        vault_id: vault.id,
        user_id: Uuid::new(),
        permission: VaultPermission::View as i32,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

// ---------------------------------------------------------------- GrantVaultAccessToUserTest

async fn grant_access(
    db: &TestDb,
    account_id: Uuid,
    user: &users::Model,
    another_user: &users::Model,
    vault: &vaults::Model,
) -> ApiResult<()> {
    // Carbon::setTestNow(2018-01-01) has no equivalent: the expected date
    // below is the next October 2nd from today.
    let now = crate::db::now().0;
    let mut year = now.year();
    if NaiveDate::from_ymd_opt(year, 10, 2).unwrap().and_hms_opt(0, 0, 0).unwrap() < now {
        year += 1;
    }
    let expected = NaiveDate::from_ymd_opt(year, 10, 2).unwrap().and_hms_opt(9, 0, 0).unwrap();

    // we'll create contact reminders so we can check that this new user
    // has also contact reminders scheduled too
    let mut channel = user_notification_channel(another_user.id);
    channel.active = Set(false);
    let channel = channel.insert(&db.conn).await.unwrap();
    let other_contact = factory::contact(&db.conn, vault).await;
    let mut contact_reminder = contact_reminder(other_contact.id);
    contact_reminder.r#type = Set("one_time".into());
    contact_reminder.day = Set(Some(2));
    contact_reminder.month = Set(Some(10));
    contact_reminder.year = Set(Some(1090));
    contact_reminder.number_times_triggered = Set(0);
    let contact_reminder = contact_reminder.insert(&db.conn).await.unwrap();

    GrantVaultAccessToUser {
        account_id,
        author_id: user.id,
        vault_id: vault.id,
        user_id: another_user.id,
        permission: VaultPermission::View as i32,
    }
    .execute(&db.conn)
    .await?;

    assert_eq!(contacts::Entity::find().count(&db.conn).await.unwrap(), 3);

    let contact = contacts::Entity::find()
        .filter(contacts::Column::FirstName.eq(another_user.first_name.clone()))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();

    assert!(!contact.can_be_deleted);
    assert_eq!(
        format!("{} {}", another_user.first_name.clone().unwrap(), another_user.last_name.clone().unwrap()),
        format!("{} {}", contact.first_name.unwrap_or_default(), contact.last_name.unwrap_or_default())
    );

    let pivot = user_vault::Entity::find_by_id((vault.id, another_user.id)).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(pivot.contact_id, contact.id);

    let scheduled = contact_reminder_scheduled::Entity::find()
        .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel.id))
        .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(contact_reminder.id))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(scheduled.scheduled_at, Timestamp(expected));
    Ok(())
}

#[tokio::test]
async fn it_gives_the_right_to_access_the_vault_to_another_user() {
    let db = TestDb::migrated().await;
    let user = factory::create_administrator(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(user.account_id)).await;
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, &user, VaultPermission::Manage, &vault).await;
    grant_access(&db, user.account_id, &user, &another_user, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn grant_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let user = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(user.account_id)).await;
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, &user, VaultPermission::Manage, &vault).await;
    let err = grant_access(&db, account.id, &user, &another_user, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn grant_fails_if_the_other_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let user = factory::create_administrator(&db.conn).await;
    let another_user = factory::user(&db.conn, None).await;
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, &user, VaultPermission::Manage, &vault).await;
    let err = grant_access(&db, user.account_id, &user, &another_user, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn grant_fails_if_user_is_not_vault_manager() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(user.account_id)).await;
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, &user, VaultPermission::Edit, &vault).await;
    let err = grant_access(&db, user.account_id, &user, &another_user, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn grant_fails_if_user_and_the_other_user_are_the_same() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, &user, VaultPermission::Manage, &vault).await;
    let err = grant_access(&db, user.account_id, &user, &user, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Conflict);
    db.drop().await;
}

#[tokio::test]
async fn grant_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let user = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, &user, VaultPermission::Manage, &vault).await;
    let err = GrantVaultAccessToUser {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        user_id: Uuid::new(),
        permission: VaultPermission::View as i32,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

// ---------------------------------------------------------------- RemoveVaultAccessTest

async fn remove_access(
    db: &TestDb,
    account_id: Uuid,
    regis: &users::Model,
    another_user: &users::Model,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    // we'll create contact reminders so we can check that this new user
    // has also contact reminders scheduled too
    let mut channel = user_notification_channel(another_user.id);
    channel.active = Set(false);
    let channel = channel.insert(&db.conn).await.unwrap();
    let other_contact = factory::contact(&db.conn, vault).await;
    let mut contact_reminder = contact_reminder(other_contact.id);
    contact_reminder.r#type = Set("one_time".into());
    let contact_reminder = contact_reminder.insert(&db.conn).await.unwrap();
    contact_reminder_scheduled::ActiveModel {
        user_notification_channel_id: Set(channel.id),
        contact_reminder_id: Set(contact_reminder.id),
        scheduled_at: Set(crate::db::now()),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap();

    // Carbon::setTestNow(2018-01-01) has no equivalent: `deleted_at` is
    // checked against the time around the call.
    let before = crate::db::now();

    RemoveVaultAccess { account_id, author_id: regis.id, vault_id: vault.id, user_id: another_user.id }
        .execute(&db.conn)
        .await?;

    let after = crate::db::now();

    // F19: the membership itself is gone (the PHP test filtered on VIEW
    // while the row was MANAGE, so it passed with the row still there).
    assert!(
        user_vault::Entity::find()
            .filter(user_vault::Column::VaultId.eq(vault.id))
            .filter(user_vault::Column::UserId.eq(another_user.id))
            .one(&db.conn)
            .await
            .unwrap()
            .is_none()
    );

    let row = contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().unwrap();
    assert!(row.deleted_at.is_some_and(|t| before <= t && t <= after));

    assert!(
        contact_reminder_scheduled::Entity::find()
            .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel.id))
            .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(contact_reminder.id))
            .one(&db.conn)
            .await
            .unwrap()
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn it_removes_the_permission_in_the_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_administrator(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(regis.account_id)).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Manage, &vault).await;
    let contact = any_contact(&db).await;
    save_vault_manager(&db, &vault, &another_user, contact.id).await;
    remove_access(&db, regis.account_id, &regis, &another_user, &vault, &contact).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(regis.account_id)).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Manage, &vault).await;
    let contact = any_contact(&db).await;
    save_vault_manager(&db, &vault, &another_user, contact.id).await;
    let err = remove_access(&db, account.id, &regis, &another_user, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_the_other_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_administrator(&db.conn).await;
    let another_user = factory::user(&db.conn, None).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Manage, &vault).await;
    let contact = any_contact(&db).await;
    save_vault_manager(&db, &vault, &another_user, contact.id).await;
    let err = remove_access(&db, regis.account_id, &regis, &another_user, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_user_is_not_vault_manager() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(regis.account_id)).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Edit, &vault).await;
    let contact = any_contact(&db).await;
    save_vault_manager(&db, &vault, &another_user, contact.id).await;
    let err = remove_access(&db, regis.account_id, &regis, &another_user, &vault, &contact).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn remove_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Manage, &vault).await;
    let err = RemoveVaultAccess {
        account_id: regis.account_id,
        author_id: regis.id,
        vault_id: vault.id,
        user_id: Uuid::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateVaultDefaultTemplateTest

async fn update_default_template(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    template: &templates::Model,
) -> ApiResult<()> {
    UpdateVaultDefaultTemplate { account_id, author_id: author.id, vault_id: vault.id, template_id: template.id }
        .execute(&db.conn)
        .await?;

    let row = vaults::Entity::find_by_id(vault.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.account_id, account_id);
    assert_eq!(row.default_template_id, Some(template.id));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_vault_default_template() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let template = template(&db, Some(ross.account_id)).await;
    update_default_template(&db, &ross, ross.account_id, &vault, &template).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_default_template_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let err = UpdateVaultDefaultTemplate {
        account_id: ross.account_id,
        author_id: ross.id,
        vault_id: vault.id,
        template_id: Id(0),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_default_template_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = factory::vault(&db.conn, ross.account_id).await;
    let template = template(&db, Some(ross.account_id)).await;
    let err = update_default_template(&db, &ross, account.id, &vault, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_default_template_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::vault(&db.conn, factory::account(&db.conn).await.id).await;
    let template = template(&db, Some(ross.account_id)).await;
    let err = update_default_template(&db, &ross, ross.account_id, &vault, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_default_template_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::View, &vault).await;
    let template = template(&db, Some(ross.account_id)).await;
    let err = update_default_template(&db, &ross, ross.account_id, &vault, &template).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_default_template_fails_if_the_template_doesnt_belong_to_the_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, ross.account_id).await;
    factory::set_permission_in_vault(&db.conn, &ross, VaultPermission::Manage, &vault).await;
    let template = template(&db, None).await;
    let err = update_default_template(&db, &ross, ross.account_id, &vault, &template).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn grant_fails_if_the_user_is_already_a_member() {
    // F20: the PHP added a second membership and contact.
    let db = TestDb::migrated().await;
    let regis = factory::create_administrator(&db.conn).await;
    let another_user = factory::user(&db.conn, Some(regis.account_id)).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, VaultPermission::Manage, &vault).await;
    factory::set_permission_in_vault(&db.conn, &another_user, VaultPermission::View, &vault).await;
    let err = GrantVaultAccessToUser {
        account_id: regis.account_id,
        author_id: regis.id,
        vault_id: vault.id,
        user_id: another_user.id,
        permission: VaultPermission::Edit as i32,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, crate::error::Code::Conflict);
    let members = user_vault::Entity::find()
        .filter(user_vault::Column::VaultId.eq(vault.id))
        .filter(user_vault::Column::UserId.eq(another_user.id))
        .count(&db.conn)
        .await
        .unwrap();
    assert_eq!(members, 1);
    db.drop().await;
}
