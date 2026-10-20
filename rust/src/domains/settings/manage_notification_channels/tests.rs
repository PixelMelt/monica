//! `tests/Unit/Domains/Settings/ManageNotificationChannels/{Services,Jobs}`.

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::*;
use crate::crypto::random_string;
use crate::domains::base::VaultPermission;
use crate::error::Code;
use crate::testing::{TestDb, factory};

const UUID: &str = "21f74a1a-a157-11ec-b909-0242ac120002";

fn mail() -> (Mailer, Config) {
    let config = Config::for_tests("", std::env::temp_dir());
    (Mailer::new(&config).expect("mailer"), config)
}

/// `UserNotificationChannel::factory()` for `user_id` (a new user when
/// `None`), to adjust and insert.
async fn user_notification_channel(db: &TestDb, user_id: Option<Uuid>) -> user_notification_channels::ActiveModel {
    let user_id = match user_id {
        Some(u) => u,
        None => factory::create_user(&db.conn).await.id,
    };
    user_notification_channels::ActiveModel {
        user_id: Set(Some(user_id)),
        r#type: Set(TYPE_EMAIL.into()),
        label: Set(Some(random_string(8))),
        content: Set("admin@admin.com".into()),
        active: Set(true),
        fails: Set(0),
        verified_at: Set(None),
        preferred_time: Set(NaiveTime::from_hms_opt(9, 0, 0)),
        ..Default::default()
    }
}

/// `ContactReminder::factory()` for `contact_id`, to adjust and insert.
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

/// A new vault of `user`'s account with `user` as editor, a contact in it
/// and a one-time reminder on 2 October `year` for the contact.
async fn reminder_in_new_vault(db: &TestDb, user: &users::Model, year: i32) -> contact_reminders::Model {
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, user, VaultPermission::Edit, &vault).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let mut reminder = contact_reminder(contact.id);
    reminder.r#type = Set("one_time".into());
    reminder.day = Set(Some(2));
    reminder.month = Set(Some(10));
    reminder.year = Set(Some(year));
    reminder.insert(&db.conn).await.expect("contact reminder")
}

/// The PHP tests freeze time at 2018-01-01, so the reminders fall on
/// 2 October 2018. Time isn't frozen here: 2 October of this year, or of next
/// year once 2 October has begun.
fn next_october_second(hour: u32) -> NaiveDateTime {
    let today = chrono::Utc::now().date_naive();
    let year =
        if today < NaiveDate::from_ymd_opt(today.year(), 10, 2).unwrap() { today.year() } else { today.year() + 1 };
    NaiveDate::from_ymd_opt(year, 10, 2).unwrap().and_hms_opt(hour, 0, 0).unwrap()
}

async fn scheduled(
    db: &TestDb,
    reminder: &contact_reminders::Model,
    channel: &user_notification_channels::Model,
) -> Option<contact_reminder_scheduled::Model> {
    contact_reminder_scheduled::Entity::find()
        .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(reminder.id))
        .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel.id))
        .one(&db.conn)
        .await
        .unwrap()
}

async fn row(db: &TestDb, channel: &user_notification_channels::Model) -> Option<user_notification_channels::Model> {
    user_notification_channels::Entity::find_by_id(channel.id).one(&db.conn).await.unwrap()
}

// ---- CreateUserNotificationChannelTest

async fn create_channel(db: &TestDb, author: &users::Model, account_id: Uuid, channel_type: &str) -> ApiResult<()> {
    let (mailer, config) = mail();
    let channel = CreateUserNotificationChannel {
        account_id,
        author_id: author.id,
        label: "label".into(),
        r#type: channel_type.into(),
        content: "admin@admin.com".into(),
        verify_email: Some(true),
        preferred_time: Some("09:00".into()),
    }
    .execute(&db.conn, &mailer, &config)
    .await?;

    let row = row(db, &channel).await.unwrap();
    assert_eq!(row.user_id, Some(author.id));
    assert_eq!(row.label.as_deref(), Some("label"));
    assert_eq!(row.r#type, channel_type);
    assert_eq!(row.content, "admin@admin.com");

    if channel_type == TYPE_EMAIL {
        let sent = mailer.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].to, "admin@admin.com");
    }
    Ok(())
}

#[tokio::test]
async fn it_creates_the_channel() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    create_channel(&db, &ross, ross.account_id, "slack").await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_creates_the_channel_with_email() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    create_channel(&db, &ross, ross.account_id, "email").await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let (mailer, config) = mail();
    let err = CreateUserNotificationChannel {
        account_id: ross.account_id,
        author_id: ross.id,
        label: String::new(),
        r#type: "email".into(),
        content: "admin@admin.com".into(),
        verify_email: None,
        preferred_time: None,
    }
    .execute(&db.conn, &mailer, &config)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    assert_eq!(create_channel(&db, &ross, account.id, "slack").await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_email_already_exists_in_the_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let mut other = user_notification_channel(&db, None).await;
    other.content = Set("admin@admin.com".into());
    other.insert(&db.conn).await.unwrap();
    assert_eq!(create_channel(&db, &ross, ross.account_id, "email").await.unwrap_err().code, Code::ValidationFailed);
    db.drop().await;
}

// ---- DestroyUserNotificationChannelTest

async fn destroy_channel(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    channel: &user_notification_channels::Model,
) -> ApiResult<()> {
    DestroyUserNotificationChannel { account_id, author_id: author.id, user_notification_channel_id: channel.id }
        .execute(&db.conn)
        .await?;

    assert!(row(db, channel).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_deletes_the_channel() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let channel = user_notification_channel(&db, Some(ross.id)).await.insert(&db.conn).await.unwrap();
    destroy_channel(&db, &ross, ross.account_id, &channel).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let account = factory::create_account(&db.conn).await;
    let channel = user_notification_channel(&db, Some(ross.id)).await.insert(&db.conn).await.unwrap();
    assert_eq!(destroy_channel(&db, &ross, account.id, &channel).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_channel_doesnt_belong_to_user() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let channel = user_notification_channel(&db, None).await.insert(&db.conn).await.unwrap();
    assert_eq!(destroy_channel(&db, &ross, ross.account_id, &channel).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- ScheduleAllContactRemindersForNotificationChannelTest

#[tokio::test]
async fn it_schedules_reminders() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let mut user: users::ActiveModel = regis.into();
    user.timezone = Set(Some("UTC".into()));
    let regis = user.update(&db.conn).await.unwrap();

    let contact_reminder_a = reminder_in_new_vault(&db, &regis, 2000).await;
    let contact_reminder_b = reminder_in_new_vault(&db, &regis, 1090).await;
    let mut channel = user_notification_channel(&db, Some(regis.id)).await;
    channel.preferred_time = Set(NaiveTime::from_hms_opt(18, 0, 0));
    let channel = channel.insert(&db.conn).await.unwrap();

    ScheduleAllContactRemindersForNotificationChannel {
        account_id: regis.account_id,
        author_id: regis.id,
        user_notification_channel_id: channel.id,
    }
    .execute(&db.conn)
    .await
    .unwrap();

    for reminder in [&contact_reminder_a, &contact_reminder_b] {
        assert_eq!(scheduled(&db, reminder, &channel).await.unwrap().scheduled_at.0, next_october_second(18));
    }
    db.drop().await;
}

// ---- SendTestEmailTest

async fn send_test_email(
    db: &TestDb,
    author: &users::Model,
    channel: &user_notification_channels::Model,
) -> ApiResult<()> {
    let (mailer, _) = mail();
    let channel =
        SendTestEmail { account_id: author.account_id, author_id: author.id, user_notification_channel_id: channel.id }
            .execute(&db.conn, &mailer)
            .await?;

    let sent = mailer.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "admin@admin.com");
    assert_eq!(sent[0].subject, "Test email for Monica");
    let log = user_notification_sent::Entity::find()
        .filter(user_notification_sent::Column::UserNotificationChannelId.eq(channel.id))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(log.subject_line, "Test email for Monica");
    Ok(())
}

#[tokio::test]
async fn it_sends_a_test_email() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let mut channel = user_notification_channel(&db, Some(ross.id)).await;
    channel.content = Set("admin@admin.com".into());
    channel.r#type = Set(TYPE_EMAIL.into());
    let channel = channel.insert(&db.conn).await.unwrap();
    send_test_email(&db, &ross, &channel).await.unwrap();
    db.drop().await;
}

// send_test_email_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn send_test_email_fails_if_notification_channel_doesnt_belong_to_user() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let channel = user_notification_channel(&db, None).await.insert(&db.conn).await.unwrap();
    assert_eq!(send_test_email(&db, &ross, &channel).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn send_test_email_fails_if_notification_channel_type_is_not_email() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let mut channel = user_notification_channel(&db, Some(ross.id)).await;
    channel.content = Set("admin@admin.com".into());
    channel.r#type = Set("slack".into());
    let channel = channel.insert(&db.conn).await.unwrap();
    assert_eq!(send_test_email(&db, &ross, &channel).await.unwrap_err().code, Code::Conflict);
    db.drop().await;
}

// SendTestTelegramNotificationTest: Telegram is removed (I009).

// ---- ToggleUserNotificationChannelTest

async fn toggle_channel(
    db: &TestDb,
    author: &users::Model,
    channel: &user_notification_channels::Model,
) -> ApiResult<()> {
    ToggleUserNotificationChannel {
        account_id: author.account_id,
        author_id: author.id,
        user_notification_channel_id: channel.id,
    }
    .execute(&db.conn)
    .await?;
    Ok(())
}

#[tokio::test]
async fn it_toggles_the_channel_if_the_channel_was_inactive() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let mut channel = user_notification_channel(&db, Some(ross.id)).await;
    channel.active = Set(false);
    let channel = channel.insert(&db.conn).await.unwrap();
    let contact_reminder = reminder_in_new_vault(&db, &ross, 2000).await;
    toggle_channel(&db, &ross, &channel).await.unwrap();

    let row = row(&db, &channel).await.unwrap();
    assert_eq!(row.user_id, Some(ross.id));
    assert!(row.active);

    assert_eq!(scheduled(&db, &contact_reminder, &channel).await.unwrap().scheduled_at.0, next_october_second(9));
    db.drop().await;
}

#[tokio::test]
async fn it_toggles_the_channel_if_the_channel_was_active() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let mut channel = user_notification_channel(&db, Some(ross.id)).await;
    channel.active = Set(true);
    let channel = channel.insert(&db.conn).await.unwrap();
    let contact_reminder = reminder_in_new_vault(&db, &ross, 2000).await;
    toggle_channel(&db, &ross, &channel).await.unwrap();

    let row = row(&db, &channel).await.unwrap();
    assert_eq!(row.user_id, Some(ross.id));
    assert!(!row.active);

    assert!(scheduled(&db, &contact_reminder, &channel).await.is_none());
    db.drop().await;
}

// toggle_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn toggle_fails_if_notification_channel_doesnt_belong_to_user() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let mut channel = user_notification_channel(&db, None).await;
    channel.active = Set(false);
    let channel = channel.insert(&db.conn).await.unwrap();
    assert_eq!(toggle_channel(&db, &ross, &channel).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- VerifyUserNotificationChannelEmailAddressTest

async fn verify_channel(
    db: &TestDb,
    author: &users::Model,
    channel: &user_notification_channels::Model,
) -> ApiResult<()> {
    let before = crate::db::now();
    let channel = VerifyUserNotificationChannelEmailAddress {
        account_id: author.account_id,
        author_id: author.id,
        user_notification_channel_id: channel.id,
        uuid: UUID.parse().unwrap(),
    }
    .execute(&db.conn)
    .await?;
    let after = crate::db::now();

    let verified_at = row(db, &channel).await.unwrap().verified_at.unwrap();
    assert!(before <= verified_at && verified_at <= after);
    Ok(())
}

#[tokio::test]
async fn it_verifies_the_user_notification_channel() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let mut channel = user_notification_channel(&db, Some(ross.id)).await;
    channel.content = Set("admin@admin.com".into());
    channel.r#type = Set(TYPE_EMAIL.into());
    channel.verification_token = Set(Some(UUID.into()));
    let channel = channel.insert(&db.conn).await.unwrap();
    verify_channel(&db, &ross, &channel).await.unwrap();
    db.drop().await;
}

// verify_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn verify_fails_if_notification_channel_doesnt_belong_to_user() {
    let db = TestDb::migrated().await;
    let ross = factory::create_administrator(&db.conn).await;
    let channel = user_notification_channel(&db, None).await.insert(&db.conn).await.unwrap();
    assert_eq!(verify_channel(&db, &ross, &channel).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn verify_fails_if_the_token_is_not_the_channel_token() {
    let db = TestDb::migrated().await;
    let ross = factory::create_user(&db.conn).await;
    let mut channel = user_notification_channel(&db, Some(ross.id)).await;
    channel.verification_token = Set(Some("8a1d1f2e-0b5c-4e8f-9d6a-3c2b1a0f9e8d".into()));
    let channel = channel.insert(&db.conn).await.unwrap();
    assert_eq!(verify_channel(&db, &ross, &channel).await.unwrap_err().code, Code::ValidationFailed);
    assert!(row(&db, &channel).await.unwrap().verified_at.is_none());
    db.drop().await;
}

// ---- Jobs/SendVerificationEmailChannelTest

#[tokio::test]
async fn it_sends_a_verification_email() {
    let db = TestDb::migrated().await;
    let (mailer, config) = mail();
    let mut channel = user_notification_channel(&db, None).await;
    channel.r#type = Set(TYPE_EMAIL.into());
    channel.content = Set("admin@admin.com".into());
    channel.verification_token = Set(Some(UUID.into()));
    let channel = channel.insert(&db.conn).await.unwrap();

    SendVerificationEmailChannel { channel: channel.clone() }.handle(&mailer, &config).await.unwrap();

    let sent = mailer.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, "admin@admin.com");
    assert!(sent[0].text.contains(&format!("http://localhost/settings/notifications/{}/verify/{UUID}", channel.id)));
    db.drop().await;
}
