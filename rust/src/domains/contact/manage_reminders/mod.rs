//! `app/Domains/Contact/ManageReminders/Services` and `Jobs`.

#[cfg(test)]
mod tests;

use chrono::{Datelike, Duration, MappedLocalTime, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Timelike};
use chrono_tz::Tz;
use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::config::Config;
use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::domains::settings::manage_notification_channels::TYPE_EMAIL;
use crate::entities::{
    contact_reminder_scheduled, contact_reminders, contacts, user_notification_channels, user_notification_sent,
    user_vault, users, vaults,
};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::mail::{Email, Mailer};
use crate::validate::Validator;

/// `ContactReminder::TYPE_ONE_TIME`.
pub const TYPE_ONE_TIME: &str = "one_time";
/// `ContactReminder::TYPE_RECURRING_DAY`.
pub const TYPE_RECURRING_DAY: &str = "recurring_day";
/// `ContactReminder::TYPE_RECURRING_MONTH`.
pub const TYPE_RECURRING_MONTH: &str = "recurring_month";
/// `ContactReminder::TYPE_RECURRING_YEAR`.
pub const TYPE_RECURRING_YEAR: &str = "recurring_year";

/// `config('monica.max_notification_failures', 10)`.
const MAX_NOTIFICATION_FAILURES: i32 = 10;

/// `CreateContactReminder`.
pub struct CreateContactReminder {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub label: String,
    pub day: Option<i32>,
    pub month: Option<i32>,
    pub year: Option<i32>,
    pub r#type: String,
    pub frequency_number: Option<i32>,
}

impl CreateContactReminder {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_reminders::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        if self.r#type.trim().is_empty() {
            v.fail("type", "required", &[]);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let reminder = self.create_contact_reminder(db).await?;
        update_last_updated_date(db, checked.contact()).await?;
        scheduled_reminder_for_all_users_in_vault(db, checked.vault(), &reminder).await?;

        Ok(reminder)
    }

    async fn create_contact_reminder<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_reminders::Model> {
        let reminder = contact_reminders::ActiveModel {
            contact_id: Set(self.contact_id),
            label: Set(self.label),
            day: Set(value_or_null_int(self.day)),
            month: Set(value_or_null_int(self.month)),
            year: Set(value_or_null_int(self.year)),
            r#type: Set(self.r#type),
            frequency_number: Set(value_or_null_int(self.frequency_number)),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(reminder)
    }
}

/// `DestroyReminder`.
pub struct DestroyReminder {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_reminder_id: Id,
}

impl DestroyReminder {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let reminder = find(db, checked.contact(), self.contact_reminder_id).await?;

        contact_reminders::Entity::delete_by_id(reminder.id).exec(db).await?;

        update_last_updated_date(db, checked.contact()).await
    }
}

/// `RescheduleContactReminderForChannel`.
pub struct RescheduleContactReminderForChannel {
    pub contact_reminder_id: Id,
    pub user_notification_channel_id: Id,
    pub contact_reminder_scheduled_id: Id,
}

impl RescheduleContactReminderForChannel {
    /// Schedule another occurence of the contact reminder, as the
    /// previous iteration has been sent, for the given channel.
    /// Before sending this occurence, we need to make the user notification
    /// channel is still active.
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let contact_reminder = contact_reminders::Entity::find_by_id(self.contact_reminder_id).one(db).await?;
        let channel = user_notification_channels::Entity::find_by_id(self.user_notification_channel_id).one(db).await?;
        let record = contact_reminder_scheduled::Entity::find_by_id(self.contact_reminder_scheduled_id).one(db).await?;
        let mut v = Validator::new();
        if contact_reminder.is_none() {
            v.fail("contact_reminder_id", "exists", &[]);
        }
        if channel.is_none() {
            v.fail("user_notification_channel_id", "exists", &[]);
        }
        if record.is_none() {
            v.fail("contact_reminder_scheduled_id", "exists", &[]);
        }
        v.finish()?;
        let (Some(contact_reminder), Some(channel), Some(record)) = (contact_reminder, channel, record) else {
            return Err(ApiError::not_found());
        };

        if !channel.active {
            return Err(ApiError::not_found().with_detail("The user notification channel is not active anymore."));
        }

        if contact_reminder.r#type != TYPE_ONE_TIME {
            Self::schedule(db, &contact_reminder, &channel, &record).await
        } else {
            contact_reminder_scheduled::Entity::delete_by_id(self.contact_reminder_scheduled_id).exec(db).await?;
            Ok(())
        }
    }

    async fn schedule<C: ConnectionTrait>(
        db: &C,
        contact_reminder: &contact_reminders::Model,
        channel: &user_notification_channels::Model,
        record: &contact_reminder_scheduled::Model,
    ) -> ApiResult<()> {
        let upcoming_date = record.scheduled_at.0;

        // Carbon's `addMonth` / `addYear`: a day past the end of the month rolls over.
        let upcoming_date = match contact_reminder.r#type.as_str() {
            TYPE_RECURRING_DAY => upcoming_date + Duration::days(1),
            TYPE_RECURRING_MONTH => {
                set_date(upcoming_date.year(), upcoming_date.month() as i32 + 1, upcoming_date.day() as i32)
                    .and_time(upcoming_date.time())
            }
            TYPE_RECURRING_YEAR => {
                set_date(upcoming_date.year() + 1, upcoming_date.month() as i32, upcoming_date.day() as i32)
                    .and_time(upcoming_date.time())
            }
            _ => return Err(ApiError::internal("Invalid contact reminder type.")),
        };

        sync_without_detaching(db, contact_reminder.id, channel.id, upcoming_date).await
    }
}

/// `ScheduleContactReminderForUser`.
pub struct ScheduleContactReminderForUser {
    pub contact_reminder_id: Id,
    pub user_id: Uuid,
}

impl ScheduleContactReminderForUser {
    /// Schedule a contact reminder for the given user, on his timezone.
    /// For each user in the vault, a scheduled reminder is created.
    /// This service SHOULD NOT BE CALLED FROM THE CLIENTS, ever.
    /// It is called by other services.
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let contact_reminder = contact_reminders::Entity::find_by_id(self.contact_reminder_id).one(db).await?;
        let user = users::Entity::find_by_id(self.user_id).one(db).await?;
        let mut v = Validator::new();
        if contact_reminder.is_none() {
            v.fail("contact_reminder_id", "exists", &[]);
        }
        if user.is_none() {
            v.fail("user_id", "exists", &[]);
        }
        v.finish()?;
        let (Some(contact_reminder), Some(user)) = (contact_reminder, user) else {
            return Err(ApiError::not_found());
        };

        let contact = contacts::Entity::live()
            .filter(contacts::Column::Id.eq(contact_reminder.contact_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::internal("the contact of the reminder is deleted"))?;
        let vault = vaults::Entity::find_by_id(contact.vault_id)
            .one(db)
            .await?
            .ok_or_else(|| ApiError::internal("the vault of the contact is missing"))?;
        if user.account_id != vault.account_id {
            return Err(ApiError::not_found().with_detail("The user does not belong to the vault's account."));
        }

        let upcoming_date = Self::get_date(&contact_reminder)?;
        Self::schedule(db, &contact_reminder, &user, upcoming_date).await
    }

    /// A ContactReminder can be either a complete date, or only a day/month.
    /// If it is only a day/month, we need to add a fake year so we can still
    /// manipulate the date as a Carbon object.
    fn get_date(contact_reminder: &contact_reminders::Model) -> ApiResult<NaiveDateTime> {
        let year = match contact_reminder.year {
            None | Some(0) => 1900,
            Some(year) => year,
        };
        // `Carbon::parse("$year-$month-$day")`: month 0–12 and day 0–31 roll
        // over, anything else fails.
        match (contact_reminder.month, contact_reminder.day) {
            (Some(month @ 0..=12), Some(day @ 0..=31)) => Ok(set_date(year, month, day).and_time(NaiveTime::MIN)),
            _ => Err(ApiError::internal("the contact reminder date can't be parsed")),
        }
    }

    async fn schedule<C: ConnectionTrait>(
        db: &C,
        contact_reminder: &contact_reminders::Model,
        user: &users::Model,
        mut upcoming_date: NaiveDateTime,
    ) -> ApiResult<()> {
        // is the date in the past? if so, we need to schedule the reminder
        // for next year
        let now = crate::db::now().0;
        if upcoming_date < now {
            upcoming_date = set_date(now.year(), upcoming_date.month() as i32, upcoming_date.day() as i32)
                .and_time(upcoming_date.time());
            if upcoming_date < now {
                upcoming_date = set_date(now.year() + 1, upcoming_date.month() as i32, upcoming_date.day() as i32)
                    .and_time(upcoming_date.time());
            }
        }

        let notification_channels = user_notification_channels::Entity::find()
            .filter(user_notification_channels::Column::UserId.eq(user.id))
            .order_by_asc(user_notification_channels::Column::Id)
            .all(db)
            .await?;
        for channel in notification_channels {
            // `shiftTimezone()` keeps the wall-clock time, `->hour` and
            // `->minute` set it, `->tz('UTC')` converts it (and stays for
            // the next channel).
            let preferred_time = channel
                .preferred_time
                .ok_or_else(|| ApiError::internal("the notification channel has no preferred time"))?;
            let local = upcoming_date
                .date()
                .and_hms_opt(preferred_time.hour(), preferred_time.minute(), upcoming_date.second())
                .expect("a NaiveTime's hour and minute");
            upcoming_date = to_utc(local, timezone(user.timezone.as_deref()));

            sync_without_detaching(db, contact_reminder.id, channel.id, upcoming_date).await?;
        }
        Ok(())
    }
}

/// `UpdateContactReminder`.
pub struct UpdateContactReminder {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_reminder_id: Id,
    pub label: String,
    pub day: Option<i32>,
    pub month: Option<i32>,
    pub year: Option<i32>,
    pub r#type: String,
    pub frequency_number: Option<i32>,
}

impl UpdateContactReminder {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_reminders::Model> {
        let (contact, vault, reminder) = self.validate(db).await?;

        let reminder = self.update(db, reminder).await?;
        Self::delete_old_scheduled_reminders(db, &reminder).await?;
        update_last_updated_date(db, &contact).await?;
        scheduled_reminder_for_all_users_in_vault(db, &vault, &reminder).await?;

        Ok(reminder)
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(contacts::Model, vaults::Model, contact_reminders::Model)> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        if self.r#type.trim().is_empty() {
            v.fail("type", "required", &[]);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let reminder = find(db, checked.contact(), self.contact_reminder_id).await?;
        Ok((checked.contact().clone(), checked.vault().clone(), reminder))
    }

    async fn update<C: ConnectionTrait>(
        self,
        db: &C,
        reminder: contact_reminders::Model,
    ) -> ApiResult<contact_reminders::Model> {
        let mut reminder: contact_reminders::ActiveModel = reminder.into();
        reminder.label = Set(self.label);
        reminder.day = Set(self.day);
        reminder.month = Set(self.month);
        reminder.year = Set(self.year);
        reminder.r#type = Set(self.r#type);
        reminder.frequency_number = Set(value_or_null_int(self.frequency_number));
        Ok(reminder.update(db).await?)
    }

    /// `$this->reminder->userNotificationChannels()->detach()`.
    async fn delete_old_scheduled_reminders<C: ConnectionTrait>(
        db: &C,
        reminder: &contact_reminders::Model,
    ) -> ApiResult<()> {
        contact_reminder_scheduled::Entity::delete_many()
            .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(reminder.id))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `Jobs/ProcessScheduledContactReminders`.
pub struct ProcessScheduledContactReminders;

impl ProcessScheduledContactReminders {
    pub async fn execute<C: ConnectionTrait>(self, db: &C, mailer: &Mailer, _config: &Config) -> ApiResult<()> {
        // this cron job runs every five minutes, so we must make sure that
        // the date we run this cron against, has no seconds, otherwise getting
        // the scheduled reminders will not return any results
        let current_date = crate::db::now().0.with_second(0).expect("0 is a valid second");

        let scheduled_contact_reminders = contact_reminder_scheduled::Entity::find()
            .filter(contact_reminder_scheduled::Column::ScheduledAt.lte(Timestamp(current_date)))
            .all(db)
            .await?;

        for scheduled_reminder in scheduled_contact_reminders {
            let user_notification_channel =
                user_notification_channels::Entity::find_by_id(scheduled_reminder.user_notification_channel_id)
                    .one(db)
                    .await?
                    .ok_or_else(ApiError::not_found)?;

            let result: anyhow::Result<()> = async {
                let contact_reminder = contact_reminders::Entity::find_by_id(scheduled_reminder.contact_reminder_id)
                    .one(db)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("the contact reminder doesn't exist"))?;
                let contact = contacts::Entity::live()
                    .filter(contacts::Column::Id.eq(contact_reminder.contact_id))
                    .one(db)
                    .await?;

                if let Some(contact) = contact {
                    Self::trigger_notification(
                        db,
                        mailer,
                        &user_notification_channel,
                        &contact,
                        &contact_reminder,
                        &scheduled_reminder,
                    )
                    .await?;
                }

                Self::update_scheduled_contact_reminder_triggered_at(db, &scheduled_reminder).await
            }
            .await;

            if let Err(e) = result {
                // we don't want to stop the process if one of the notifications fails
                // we just want to log the error and continue with the next scheduled reminder
                tracing::error!(message = %e, scheduled_reminder = ?scheduled_reminder, "Error sending reminder");
                user_notification_sent::ActiveModel {
                    user_notification_channel_id: Set(user_notification_channel.id),
                    sent_at: Set(crate::db::now()),
                    subject_line: Set(String::new()),
                    error: Set(Some(e.to_string())),
                    ..Default::default()
                }
                .insert(db)
                .await?;

                let user_notification_channel =
                    user_notification_channels::Entity::find_by_id(user_notification_channel.id)
                        .one(db)
                        .await?
                        .ok_or_else(ApiError::not_found)?;
                let fails = user_notification_channel.fails + 1;
                let channel_id = user_notification_channel.id;
                let mut user_notification_channel: user_notification_channels::ActiveModel =
                    user_notification_channel.into();
                user_notification_channel.fails = Set(fails);

                if fails >= MAX_NOTIFICATION_FAILURES {
                    user_notification_channel.active = Set(false);
                    // F21: only this channel's schedules go, not the reminders.
                    contact_reminder_scheduled::Entity::delete_many()
                        .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel_id))
                        .exec(db)
                        .await?;
                }

                user_notification_channel.update(db).await?;
            }
        }
        Ok(())
    }

    async fn update_scheduled_contact_reminder_triggered_at<C: ConnectionTrait>(
        db: &C,
        scheduled_reminder: &contact_reminder_scheduled::Model,
    ) -> anyhow::Result<()> {
        contact_reminder_scheduled::Entity::update_many()
            .col_expr(contact_reminder_scheduled::Column::TriggeredAt, Expr::value(crate::db::now()))
            .filter(contact_reminder_scheduled::Column::Id.eq(scheduled_reminder.id))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_number_of_times_triggered<C: ConnectionTrait>(db: &C, id: Id) -> anyhow::Result<()> {
        contact_reminders::Entity::update_many()
            .col_expr(
                contact_reminders::Column::NumberTimesTriggered,
                Expr::col(contact_reminders::Column::NumberTimesTriggered).add(1),
            )
            .filter(contact_reminders::Column::Id.eq(id))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn trigger_notification<C: ConnectionTrait>(
        db: &C,
        mailer: &Mailer,
        channel: &user_notification_channels::Model,
        contact: &contacts::Model,
        contact_reminder: &contact_reminders::Model,
        scheduled_reminder: &contact_reminder_scheduled::Model,
    ) -> anyhow::Result<()> {
        if !channel.active {
            return Ok(());
        }

        let user = match channel.user_id {
            Some(user_id) => users::Entity::find_by_id(user_id).one(db).await?,
            None => None,
        }
        .ok_or_else(|| anyhow::anyhow!("the notification channel has no user"))?;
        let contact_name = format_contact_name(&user, contact);

        // I009: Telegram is removed; any type but email is unknown.
        if channel.r#type != TYPE_EMAIL {
            return Ok(());
        }

        i18n::LOCALE
            .scope(user.locale.clone(), reminder_triggered(db, mailer, channel, &contact_reminder.label, &contact_name))
            .await?;

        Self::update_number_of_times_triggered(db, scheduled_reminder.contact_reminder_id).await?;

        RescheduleContactReminderForChannel {
            contact_reminder_id: scheduled_reminder.contact_reminder_id,
            user_notification_channel_id: scheduled_reminder.user_notification_channel_id,
            contact_reminder_scheduled_id: scheduled_reminder.id,
        }
        .execute(db)
        .await?;
        Ok(())
    }
}

/// `App\Notifications\ReminderTriggered` on the mail channel: `toMail`
/// logs the notification, then the message goes to the channel's address.
pub async fn reminder_triggered<C: ConnectionTrait>(
    db: &C,
    mailer: &Mailer,
    channel: &user_notification_channels::Model,
    content: &str,
    contact_name: &str,
) -> anyhow::Result<()> {
    user_notification_sent::ActiveModel {
        user_notification_channel_id: Set(channel.id),
        sent_at: Set(crate::db::now()),
        subject_line: Set(content.to_owned()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    let l = i18n::current();
    let (text, html) = Mailer::render(
        &[
            i18n::t(&l, "You wanted to be reminded of the following:", &[]),
            content.to_owned(),
            i18n::t(&l, "for", &[]),
            contact_name.to_owned(),
            i18n::t(&l, "Test email for Monica", &[]),
        ],
        None,
        &[],
    );
    mailer
        .send(Email {
            to: channel.content.clone(),
            subject: i18n::t(&l, "Reminder for :name", &[("name", contact_name)]),
            text,
            html,
        })
        .await
}

/// `$this->contact->reminders()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
    contact_reminder_id: Id,
) -> ApiResult<contact_reminders::Model> {
    contact_reminders::Entity::find_by_id(contact_reminder_id)
        .filter(contact_reminders::Column::ContactId.eq(contact.id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `updateLastUpdatedDate`: `$this->contact->last_updated_at = Carbon::now()`.
async fn update_last_updated_date<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<()> {
    let mut contact: contacts::ActiveModel = contact.clone().into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    contact.update(db).await?;
    Ok(())
}

/// `scheduledReminderForAllUsersInVault`.
async fn scheduled_reminder_for_all_users_in_vault<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    reminder: &contact_reminders::Model,
) -> ApiResult<()> {
    let users = user_vault::Entity::find().filter(user_vault::Column::VaultId.eq(vault.id)).all(db).await?;

    for user in users {
        ScheduleContactReminderForUser { contact_reminder_id: reminder.id, user_id: user.user_id }.execute(db).await?;
    }
    Ok(())
}

/// `$contactReminder->userNotificationChannels()->syncWithoutDetaching([$channelId => ['scheduled_at' => $date]])`.
async fn sync_without_detaching<C: ConnectionTrait>(
    db: &C,
    contact_reminder_id: Id,
    user_notification_channel_id: Id,
    scheduled_at: NaiveDateTime,
) -> ApiResult<()> {
    let scheduled_at = Timestamp(scheduled_at);
    let attached = contact_reminder_scheduled::Entity::find()
        .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(contact_reminder_id))
        .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(user_notification_channel_id))
        .one(db)
        .await?
        .is_some();
    if attached {
        contact_reminder_scheduled::Entity::update_many()
            .col_expr(contact_reminder_scheduled::Column::ScheduledAt, Expr::value(scheduled_at))
            .col_expr(contact_reminder_scheduled::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(contact_reminder_id))
            .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(user_notification_channel_id))
            .exec(db)
            .await?;
    } else {
        contact_reminder_scheduled::ActiveModel {
            user_notification_channel_id: Set(user_notification_channel_id),
            contact_reminder_id: Set(contact_reminder_id),
            scheduled_at: Set(scheduled_at),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// `valueOrNull` on an integer: `None` for 0.
fn value_or_null_int(value: Option<i32>) -> Option<i32> {
    value.filter(|v| *v != 0)
}

/// PHP's `setDate`: months and days past the end of the month roll over.
fn set_date(year: i32, month: i32, day: i32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year + (month - 1).div_euclid(12), (month - 1).rem_euclid(12) as u32 + 1, 1)
        .expect("the first of a month");
    first + Duration::days(i64::from(day) - 1)
}

/// `$user->timezone ?? config('app.timezone')`: UTC when the user has no
/// timezone or an unknown one.
fn timezone(name: Option<&str>) -> Tz {
    name.and_then(|n| n.parse().ok()).unwrap_or(Tz::UTC)
}

/// `->tz('UTC')` of the wall-clock time `local` in `tz`, resolved like PHP:
/// an ambiguous time takes the earlier offset, a time in a gap the offset
/// before the gap.
fn to_utc(local: NaiveDateTime, tz: Tz) -> NaiveDateTime {
    match tz.from_local_datetime(&local) {
        MappedLocalTime::Single(t) | MappedLocalTime::Ambiguous(t, _) => t.naive_utc(),
        MappedLocalTime::None => {
            let before = tz.offset_from_utc_datetime(&(local - Duration::days(1))).fix();
            local - Duration::seconds(i64::from(before.local_minus_utc()))
        }
    }
}

/// Stand-in for `App\Helpers\NameHelper::formatContactName` until the
/// helpers are ported; a `%variable%` is one of the contact's name columns.
pub fn format_contact_name(user: &users::Model, contact: &contacts::Model) -> String {
    const PHP_TRIM: [char; 6] = [' ', '\t', '\n', '\r', '\0', '\x0B'];
    let mut variable_found = false;
    let mut variable_name = String::new();
    let mut complete_name = String::new();

    if let Some(prefix) = value_or_null(contact.prefix.clone()) {
        complete_name = format!("{prefix} ");
    }

    for char in user.name_order.chars() {
        if char == '%' && !variable_found {
            // a variable has been found
            variable_found = true;
            variable_name.push(char);
        } else if char != '%' && variable_found {
            variable_name.push(char);
        } else if char == '%' {
            // the variable has ended
            // get rid of the first %
            let value = match &variable_name[1..] {
                "first_name" => contact.first_name.as_deref(),
                "middle_name" => contact.middle_name.as_deref(),
                "last_name" => contact.last_name.as_deref(),
                "nickname" => contact.nickname.as_deref(),
                "maiden_name" => contact.maiden_name.as_deref(),
                "prefix" => contact.prefix.as_deref(),
                "suffix" => contact.suffix.as_deref(),
                "job_position" => contact.job_position.as_deref(),
                _ => None,
            };
            complete_name.push_str(value.unwrap_or(""));

            // reset the variable
            variable_found = false;
            variable_name.clear();
        } else {
            // this is a normal character
            complete_name.push(char);
        }
    }

    // in some cases, the user will add parenthesis to add, for instance,
    // the nickname, but if the nickname is not set, we need to remove them
    // from being displayed
    let mut complete_name = complete_name.replace("()", "").trim_end_matches(PHP_TRIM).to_owned();

    if let Some(suffix) = value_or_null(contact.suffix.clone()) {
        complete_name = format!("{complete_name} {suffix}");
    }

    if complete_name.trim_matches(PHP_TRIM).is_empty() {
        complete_name = i18n::t(&i18n::current(), "Unknown name", &[]);
    }

    complete_name
}
