//! `tests/Unit/Domains/Contact/ManageReminders/Services` and `Jobs`.

use sea_orm::PaginatorTrait;

use super::*;
use crate::config::Config;
use crate::crypto::random_string;
use crate::error::Code;
use crate::testing::pushover::FakePushover;
use crate::testing::{TestDb, factory};

/// A fake Pushover and a client of it.
async fn pushover() -> (FakePushover, Pushover) {
    let fake = FakePushover::start().await;
    let mut config = Config::for_tests("", std::env::temp_dir());
    config.pushover_api_url = fake.url();
    let client = Pushover::new(&config);
    (fake, client)
}

/// `UserNotificationChannel::factory()` for `user_id`, to insert.
fn user_notification_channel(user_id: Uuid) -> user_notification_channels::ActiveModel {
    user_notification_channels::ActiveModel {
        user_id: Set(Some(user_id)),
        r#type: Set(TYPE_PUSHOVER.into()),
        label: Set(Some(random_string(6))),
        content: Set(Credentials { user_key: "u-admin".into(), app_token: "a-monica".into() }.to_content()),
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
        r#type: Set(TYPE_ONE_TIME.into()),
        frequency_number: Set(Some(1)),
        last_triggered_at: Set(None),
        number_times_triggered: Set(0),
        ..Default::default()
    }
}

/// `Contact::factory()->create()`: in a new vault of a new account.
async fn any_contact(db: &TestDb) -> contacts::Model {
    let vault = factory::vault(&db.conn, factory::account(&db.conn).await.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `createVault($user->account)` then `setPermissionInVault($user, $permission, $vault)`.
async fn vault_with(db: &TestDb, user: &users::Model, permission: VaultPermission) -> vaults::Model {
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, user, permission, &vault).await;
    vault
}

/// `DB::table('contact_reminder_scheduled')->insertGetId([...])`.
async fn insert_scheduled(
    db: &TestDb,
    channel: &user_notification_channels::Model,
    contact_reminder: &contact_reminders::Model,
    scheduled_at: NaiveDateTime,
) -> Id {
    contact_reminder_scheduled::ActiveModel {
        user_notification_channel_id: Set(channel.id),
        contact_reminder_id: Set(contact_reminder.id),
        scheduled_at: Set(Timestamp(scheduled_at)),
        triggered_at: Set(None),
        created_at: Set(None),
        updated_at: Set(None),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .unwrap()
    .id
}

async fn scheduled(db: &TestDb, channel_id: Id, contact_reminder_id: Id) -> Vec<contact_reminder_scheduled::Model> {
    contact_reminder_scheduled::Entity::find()
        .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel_id))
        .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(contact_reminder_id))
        .all(&db.conn)
        .await
        .unwrap()
}

fn at(value: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap()
}

/// The PHP tests freeze time at 2018-01-01, so a reminder on `month`/`day`
/// of a past year falls on that day of 2018. Time isn't frozen here: the
/// next such day from today.
fn next(month: u32, day: u32) -> NaiveDate {
    let now = crate::db::now().0;
    let this_year = NaiveDate::from_ymd_opt(now.year(), month, day).unwrap();
    if this_year.and_time(NaiveTime::MIN) < now {
        NaiveDate::from_ymd_opt(now.year() + 1, month, day).unwrap()
    } else {
        this_year
    }
}

async fn set_timezone(db: &TestDb, user: users::Model, timezone: &str) -> users::Model {
    let mut user: users::ActiveModel = user.into();
    user.timezone = Set(Some(timezone.into()));
    user.update(&db.conn).await.unwrap()
}

// ---------------------------------------------------------------- CreateContactReminderTest

async fn create(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    let user_notification_channel = user_notification_channel(author.id).insert(&db.conn).await.unwrap();

    let contact_reminder = CreateContactReminder {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        label: "birthdate".into(),
        day: Some(29),
        month: Some(10),
        year: Some(1981),
        r#type: TYPE_ONE_TIME.into(),
        frequency_number: None,
    }
    .execute(&db.conn)
    .await?;

    let row = contact_reminders::Entity::find_by_id(contact_reminder.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.label, "birthdate");
    assert_eq!((row.day, row.month, row.year), (Some(29), Some(10), Some(1981)));
    assert_eq!(row.r#type, TYPE_ONE_TIME);
    assert_eq!(row.frequency_number, None);

    let rows = scheduled(db, user_notification_channel.id, contact_reminder.id).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].scheduled_at.0, next(10, 29).and_hms_opt(9, 0, 0).unwrap());
    Ok(())
}

#[tokio::test]
async fn it_creates_a_reminder() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    create(&db, &regis, regis.account_id, &vault, &contact).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = CreateContactReminder {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        label: String::new(),
        day: None,
        month: None,
        year: None,
        r#type: TYPE_ONE_TIME.into(),
        frequency_number: None,
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
    let regis = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    assert_eq!(create(&db, &regis, account.id, &vault, &contact).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = any_contact(&db).await;
    assert_eq!(create(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    assert_eq!(create(&db, &regis, regis.account_id, &vault, &contact).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyReminderTest

async fn destroy(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    reminder: &contact_reminders::Model,
) -> ApiResult<()> {
    DestroyReminder {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_reminder_id: reminder.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(contact_reminders::Entity::find_by_id(reminder.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_reminder() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    destroy(&db, &regis, regis.account_id, &vault, &contact, &reminder).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    let err = destroy(&db, &regis, account.id, &vault, &contact, &reminder).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = any_contact(&db).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    let err = destroy(&db, &regis, regis.account_id, &vault, &contact, &reminder).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    let err = destroy(&db, &regis, regis.account_id, &vault, &contact, &reminder).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_reminder_does_not_exist() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(any_contact(&db).await.id).insert(&db.conn).await.unwrap();
    let err = destroy(&db, &regis, regis.account_id, &vault, &contact, &reminder).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- RescheduleContactReminderForChannelTest

/// A reminder of `type` on 2 October 2000, an email channel (`active`), and
/// its schedule at 2018-01-01 00:00:00; then the service.
async fn reschedule(
    db: &TestDb,
    r#type: &str,
    active: bool,
) -> (ApiResult<()>, user_notification_channels::Model, contact_reminders::Model) {
    let mut contact_reminder = contact_reminder(any_contact(db).await.id);
    contact_reminder.r#type = Set(r#type.into());
    contact_reminder.day = Set(Some(2));
    contact_reminder.month = Set(Some(10));
    contact_reminder.year = Set(Some(2000));
    let contact_reminder = contact_reminder.insert(&db.conn).await.unwrap();

    let mut channel = user_notification_channel(factory::create_user(&db.conn).await.id);
    channel.r#type = Set(TYPE_PUSHOVER.into());
    channel.content = Set("admin@admin.com".into());
    channel.active = Set(active);
    let channel = channel.insert(&db.conn).await.unwrap();
    let id = insert_scheduled(db, &channel, &contact_reminder, at("2018-01-01 00:00:00")).await;

    let result = RescheduleContactReminderForChannel {
        contact_reminder_id: contact_reminder.id,
        user_notification_channel_id: channel.id,
        contact_reminder_scheduled_id: id,
    }
    .execute(&db.conn)
    .await;
    (result, channel, contact_reminder)
}

async fn assert_rescheduled_at(db: &TestDb, channel: Id, contact_reminder: Id, expected: &str) {
    let rows = scheduled(db, channel, contact_reminder).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].scheduled_at.0, at(expected));
    assert_eq!(rows[0].triggered_at, None);
}

#[tokio::test]
async fn it_doesnt_schedules_a_one_time_contact_reminder_again() {
    let db = TestDb::migrated().await;
    let (result, channel, contact_reminder) = reschedule(&db, TYPE_ONE_TIME, true).await;
    result.unwrap();
    assert!(scheduled(&db, channel.id, contact_reminder.id).await.is_empty());
    db.drop().await;
}

#[tokio::test]
async fn it_schedules_a_daily_contact_reminder_again() {
    let db = TestDb::migrated().await;
    let (result, channel, contact_reminder) = reschedule(&db, TYPE_RECURRING_DAY, true).await;
    result.unwrap();
    assert_rescheduled_at(&db, channel.id, contact_reminder.id, "2018-01-02 00:00:00").await;
    db.drop().await;
}

#[tokio::test]
async fn it_schedules_a_monthly_contact_reminder_again() {
    let db = TestDb::migrated().await;
    let (result, channel, contact_reminder) = reschedule(&db, TYPE_RECURRING_MONTH, true).await;
    result.unwrap();
    assert_rescheduled_at(&db, channel.id, contact_reminder.id, "2018-02-01 00:00:00").await;
    db.drop().await;
}

#[tokio::test]
async fn it_schedules_a_yearly_contact_reminder_again() {
    let db = TestDb::migrated().await;
    let (result, channel, contact_reminder) = reschedule(&db, TYPE_RECURRING_YEAR, true).await;
    result.unwrap();
    assert_rescheduled_at(&db, channel.id, contact_reminder.id, "2019-01-01 00:00:00").await;
    db.drop().await;
}

#[tokio::test]
async fn it_doesnt_schedules_a_yearly_contact_reminder_again_if_channel_is_inactive() {
    let db = TestDb::migrated().await;
    let (result, _, _) = reschedule(&db, TYPE_RECURRING_YEAR, false).await;
    assert_eq!(result.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- ScheduleContactReminderForUserTest

/// A user in `timezone` with a reminder on 2 October `year` and an 18:00
/// channel; then the service and the schedule it wrote.
async fn schedule(db: &TestDb, timezone: &str, year: i32) -> NaiveDateTime {
    let regis = set_timezone(db, factory::create_user(&db.conn).await, timezone).await;
    let vault = vault_with(db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;

    let mut contact_reminder = contact_reminder(contact.id);
    contact_reminder.r#type = Set(TYPE_ONE_TIME.into());
    contact_reminder.day = Set(Some(2));
    contact_reminder.month = Set(Some(10));
    contact_reminder.year = Set(Some(year));
    let contact_reminder = contact_reminder.insert(&db.conn).await.unwrap();
    let mut channel = user_notification_channel(regis.id);
    channel.preferred_time = Set(NaiveTime::from_hms_opt(18, 0, 0));
    let channel = channel.insert(&db.conn).await.unwrap();

    ScheduleContactReminderForUser { contact_reminder_id: contact_reminder.id, user_id: regis.id }
        .execute(&db.conn)
        .await
        .unwrap();

    let rows = scheduled(db, channel.id, contact_reminder.id).await;
    assert_eq!(rows.len(), 1);
    rows[0].scheduled_at.0
}

#[tokio::test]
async fn it_schedules_a_reminder_of_date_in_the_past_in_utc() {
    let db = TestDb::migrated().await;
    assert_eq!(schedule(&db, "UTC", 2000).await, next(10, 2).and_hms_opt(18, 0, 0).unwrap());
    db.drop().await;
}

#[tokio::test]
async fn it_schedules_a_reminder_of_date_in_the_past_in_another_timezone() {
    let db = TestDb::migrated().await;
    assert_eq!(schedule(&db, "America/New_York", 2000).await, next(10, 2).and_hms_opt(22, 0, 0).unwrap());
    db.drop().await;
}

#[tokio::test]
async fn it_schedules_a_reminder_of_date_in_the_past_in_another_timezone_again() {
    let db = TestDb::migrated().await;
    assert_eq!(schedule(&db, "Asia/Sakhalin", 2000).await, next(10, 2).and_hms_opt(7, 0, 0).unwrap());
    db.drop().await;
}

#[tokio::test]
async fn it_schedules_a_reminder_of_date_in_the_future_in_utc() {
    let db = TestDb::migrated().await;
    assert_eq!(schedule(&db, "Asia/Sakhalin", 2200).await, at("2200-10-02 07:00:00"));
    db.drop().await;
}

#[tokio::test]
async fn schedule_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let err = ScheduleContactReminderForUser { contact_reminder_id: Id(i64::MAX), user_id: regis.id }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

/// Deviation: PHP throws on a timezone it doesn't know; the port uses UTC.
#[tokio::test]
async fn schedule_uses_utc_for_an_unknown_timezone() {
    let db = TestDb::migrated().await;
    assert_eq!(schedule(&db, "Not/AZone", 2200).await, at("2200-10-02 18:00:00"));
    db.drop().await;
}

/// `shiftTimezone()` / `->tz('UTC')` mutate the date, so the next channel
/// starts from the previous channel's UTC date (Carbon, at 2018-01-01:
/// 2018-10-03 02:00 then 2018-10-03 13:00).
#[tokio::test]
async fn schedule_starts_each_channel_from_the_previous_utc_date() {
    let db = TestDb::migrated().await;
    let regis = set_timezone(&db, factory::create_user(&db.conn).await, "America/New_York").await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let mut contact_reminder = contact_reminder(contact.id);
    contact_reminder.day = Set(Some(2));
    contact_reminder.year = Set(Some(2000));
    let contact_reminder = contact_reminder.insert(&db.conn).await.unwrap();
    let mut late = user_notification_channel(regis.id);
    late.preferred_time = Set(NaiveTime::from_hms_opt(22, 0, 0));
    let late = late.insert(&db.conn).await.unwrap();
    let early = user_notification_channel(regis.id).insert(&db.conn).await.unwrap();

    ScheduleContactReminderForUser { contact_reminder_id: contact_reminder.id, user_id: regis.id }
        .execute(&db.conn)
        .await
        .unwrap();

    let day_after = next(10, 2).succ_opt().unwrap();
    assert_eq!(
        scheduled(&db, late.id, contact_reminder.id).await[0].scheduled_at.0,
        day_after.and_hms_opt(2, 0, 0).unwrap()
    );
    assert_eq!(
        scheduled(&db, early.id, contact_reminder.id).await[0].scheduled_at.0,
        day_after.and_hms_opt(13, 0, 0).unwrap()
    );
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateContactReminderTest

async fn update(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    reminder: &contact_reminders::Model,
) -> ApiResult<()> {
    let mut channel = user_notification_channel(author.id);
    channel.preferred_time = Set(NaiveTime::from_hms_opt(18, 0, 0));
    channel.insert(&db.conn).await.unwrap();

    let reminder = UpdateContactReminder {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        contact_reminder_id: reminder.id,
        label: "birthdate".into(),
        day: Some(29),
        month: Some(10),
        year: Some(1981),
        r#type: TYPE_ONE_TIME.into(),
        frequency_number: None,
    }
    .execute(&db.conn)
    .await?;

    let row = contact_reminders::Entity::find_by_id(reminder.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.label, "birthdate");
    assert_eq!((row.day, row.month, row.year), (Some(29), Some(10), Some(1981)));
    assert_eq!(row.r#type, TYPE_ONE_TIME);
    assert_eq!(row.frequency_number, None);

    let count = contact_reminder_scheduled::Entity::find()
        .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(reminder.id))
        .count(&db.conn)
        .await
        .unwrap();
    assert!(count > 0);
    Ok(())
}

#[tokio::test]
async fn it_updates_a_reminder() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    update(&db, &regis, regis.account_id, &vault, &contact, &reminder).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_persists_notification_channels() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    let mut channel = user_notification_channel(regis.id);
    channel.preferred_time = Set(NaiveTime::from_hms_opt(18, 0, 0));
    channel.insert(&db.conn).await.unwrap();

    UpdateContactReminder {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        contact_reminder_id: reminder.id,
        label: "birthdate".into(),
        day: Some(29),
        month: Some(10),
        year: Some(1981),
        r#type: TYPE_ONE_TIME.into(),
        frequency_number: None,
    }
    .execute(&db.conn)
    .await
    .unwrap();

    let channels = user_notification_channels::Entity::find()
        .filter(user_notification_channels::Column::UserId.eq(regis.id))
        .count(&db.conn)
        .await
        .unwrap();
    assert!(channels > 0);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    let err = UpdateContactReminder {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        contact_reminder_id: reminder.id,
        label: "x".repeat(256),
        day: None,
        month: None,
        year: None,
        r#type: TYPE_ONE_TIME.into(),
        frequency_number: None,
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
    let regis = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    let err = update(&db, &regis, account.id, &vault, &contact, &reminder).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = any_contact(&db).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    let err = update(&db, &regis, regis.account_id, &vault, &contact, &reminder).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(contact.id).insert(&db.conn).await.unwrap();
    let err = update(&db, &regis, regis.account_id, &vault, &contact, &reminder).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_reminder_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = vault_with(&db, &regis, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let reminder = contact_reminder(any_contact(&db).await.id).insert(&db.conn).await.unwrap();
    let err = update(&db, &regis, regis.account_id, &vault, &contact, &reminder).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- ProcessScheduledContactRemindersTest

/// A reminder of `type` labelled "test" for a new contact, `channel`, and
/// its schedule at `scheduled_at`.
async fn scheduled_reminder(
    db: &TestDb,
    r#type: &str,
    channel: user_notification_channels::ActiveModel,
    scheduled_at: NaiveDateTime,
) -> (contact_reminders::Model, user_notification_channels::Model, Id) {
    let mut contact_reminder = contact_reminder(any_contact(db).await.id);
    contact_reminder.r#type = Set(r#type.into());
    contact_reminder.label = Set("test".into());
    let contact_reminder = contact_reminder.insert(&db.conn).await.unwrap();
    let channel = channel.insert(&db.conn).await.unwrap();
    let id = insert_scheduled(db, &channel, &contact_reminder, scheduled_at).await;
    (contact_reminder, channel, id)
}

/// `Carbon::setTestNow(2018-01-01 00:00:00)` then `Carbon::now()`: the job
/// compares against the current minute, so the schedule is this minute.
fn this_minute() -> NaiveDateTime {
    crate::db::now().0.with_second(0).unwrap()
}

async fn pushover_channel(db: &TestDb) -> user_notification_channels::ActiveModel {
    user_notification_channel(factory::create_user(&db.conn).await.id)
}

#[tokio::test]
async fn it_processes_all_the_scheduled_contact_reminders() {
    let db = TestDb::migrated().await;
    let (fake, pushover) = pushover().await;
    let channel = pushover_channel(&db).await;
    let (_, channel, _) = scheduled_reminder(&db, TYPE_RECURRING_DAY, channel, this_minute()).await;

    ProcessScheduledContactReminders.execute(&db.conn, &pushover).await.unwrap();

    let sent = fake.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0]["user"].as_str(), sent[0]["token"].as_str()), ("u-admin", "a-monica"));
    assert_eq!(sent[0]["title"], "Reminder for Dr. Regis Troyat III");
    assert_eq!(sent[0]["message"], "test");
    let logged = user_notification_sent::Entity::find()
        .filter(user_notification_sent::Column::UserNotificationChannelId.eq(channel.id))
        .all(&db.conn)
        .await
        .unwrap();
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0].subject_line, "test");
    db.drop().await;
}

#[tokio::test]
async fn it_cant_process_the_scheduled_contact_reminders() {
    let db = TestDb::migrated().await;
    let (fake, pushover) = pushover().await;
    let channel = pushover_channel(&db).await;
    scheduled_reminder(&db, TYPE_ONE_TIME, channel, this_minute() + Duration::minutes(10)).await;

    ProcessScheduledContactReminders.execute(&db.conn, &pushover).await.unwrap();

    assert!(fake.sent().is_empty());
    db.drop().await;
}

#[tokio::test]
async fn it_does_not_process_reminders_for_deleted_contacts() {
    let db = TestDb::migrated().await;
    let (fake, pushover) = pushover().await;
    let channel = pushover_channel(&db).await;
    let (contact_reminder, _, _) = scheduled_reminder(&db, TYPE_RECURRING_DAY, channel, this_minute()).await;
    let contact = contacts::Entity::find_by_id(contact_reminder.contact_id).one(&db.conn).await.unwrap().unwrap();
    let mut contact: contacts::ActiveModel = contact.into();
    contact.deleted_at = Set(Some(crate::db::now()));
    contact.update(&db.conn).await.unwrap();

    ProcessScheduledContactReminders.execute(&db.conn, &pushover).await.unwrap();

    assert!(fake.sent().is_empty());
    db.drop().await;
}

#[tokio::test]
async fn it_does_not_reschudle_a_failing_reminder() {
    let db = TestDb::migrated().await;
    let (_fake, pushover) = pushover().await;
    let mut channel = user_notification_channel(factory::create_user(&db.conn).await.id);
    channel.r#type = Set("telegram".into());
    channel.content = Set("0".into());
    channel.fails = Set(10);
    channel.active = Set(false);
    let (_, channel, _) = scheduled_reminder(&db, TYPE_RECURRING_DAY, channel, this_minute()).await;

    ProcessScheduledContactReminders.execute(&db.conn, &pushover).await.unwrap();

    let channel = user_notification_channels::Entity::find_by_id(channel.id).one(&db.conn).await.unwrap().unwrap();
    assert!(!channel.active);
    db.drop().await;
}

/// The notification is in the channel user's locale (`->locale($channel->user->locale)`).
#[tokio::test]
async fn process_sends_the_reminder_in_the_users_locale() {
    let db = TestDb::migrated().await;
    let (fake, pushover) = pushover().await;
    let user = factory::create_user(&db.conn).await;
    let mut user: users::ActiveModel = user.into();
    user.locale = Set("fr".into());
    let user = user.update(&db.conn).await.unwrap();
    let channel = user_notification_channel(user.id);
    scheduled_reminder(&db, TYPE_RECURRING_DAY, channel, this_minute()).await;

    ProcessScheduledContactReminders.execute(&db.conn, &pushover).await.unwrap();

    let sent = fake.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["title"], "Rappel pour Dr. Regis Troyat III");
    db.drop().await;
}

/// Pushover is the only kind of channel: an active channel of another type
/// (Telegram, I009) sends nothing and its schedule is only marked triggered.
#[tokio::test]
async fn process_skips_a_channel_of_another_type() {
    let db = TestDb::migrated().await;
    let (fake, pushover) = pushover().await;
    let mut channel = user_notification_channel(factory::create_user(&db.conn).await.id);
    channel.r#type = Set("telegram".into());
    let scheduled_at = this_minute();
    let (contact_reminder, channel, id) = scheduled_reminder(&db, TYPE_RECURRING_DAY, channel, scheduled_at).await;

    ProcessScheduledContactReminders.execute(&db.conn, &pushover).await.unwrap();

    assert!(fake.sent().is_empty());
    let sent_rows = user_notification_sent::Entity::find().count(&db.conn).await.unwrap();
    assert_eq!(sent_rows, 0);
    let row = contact_reminder_scheduled::Entity::find_by_id(id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.scheduled_at.0, scheduled_at);
    assert!(row.triggered_at.is_some());
    let reminder = contact_reminders::Entity::find_by_id(contact_reminder.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(reminder.number_times_triggered, 0);
    let channel = user_notification_channels::Entity::find_by_id(channel.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(channel.fails, 0);
    db.drop().await;
}

/// F21: a channel that reaches 10 failed sends is deactivated and loses its
/// schedules; the contact reminders themselves stay.
#[tokio::test]
async fn process_deactivates_a_channel_after_ten_failures_and_keeps_the_reminders() {
    let db = TestDb::migrated().await;
    let (fake, pushover) = pushover().await;
    fake.go_down();
    let mut channel = user_notification_channel(factory::create_user(&db.conn).await.id);
    channel.fails = Set(9);
    let (contact_reminder, channel, _) = scheduled_reminder(&db, TYPE_RECURRING_DAY, channel, this_minute()).await;

    ProcessScheduledContactReminders.execute(&db.conn, &pushover).await.unwrap();

    let channel = user_notification_channels::Entity::find_by_id(channel.id).one(&db.conn).await.unwrap().unwrap();
    assert!(!channel.active);
    assert_eq!(channel.fails, 10);
    let schedules = contact_reminder_scheduled::Entity::find()
        .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel.id))
        .count(&db.conn)
        .await
        .unwrap();
    assert_eq!(schedules, 0);
    assert!(contact_reminders::Entity::find_by_id(contact_reminder.id).one(&db.conn).await.unwrap().is_some());
    db.drop().await;
}
