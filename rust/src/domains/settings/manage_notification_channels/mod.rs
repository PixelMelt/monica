//! `app/Domains/Settings/ManageNotificationChannels/Services`, with
//! Pushover in place of email and Telegram.

#[cfg(test)]
mod tests;

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, JoinType, QueryFilter, QuerySelect,
    RelationTrait,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::{
    contact_reminder_scheduled, contact_reminders, contacts, user_notification_channels, user_notification_sent, users,
    vaults,
};
use crate::error::{ApiError, ApiResult, Code};
use crate::i18n;
use crate::pushover::{self, Credentials, Message, Pushover};
use crate::validate::Validator;

/// The type of a Pushover channel.
pub const TYPE_PUSHOVER: &str = "pushover";

/// `CreateUserNotificationChannel` for Pushover: the keys are checked with
/// Pushover, so the channel is verified and active from the start and its
/// reminders are scheduled.
pub struct CreateUserNotificationChannel {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub label: String,
    pub credentials: Credentials,
    pub preferred_time: String,
}

impl CreateUserNotificationChannel {
    pub async fn execute<C: ConnectionTrait>(
        self,
        db: &C,
        pushover: &Pushover,
    ) -> ApiResult<user_notification_channels::Model> {
        self.validate(db, pushover).await?;
        let channel = self.create(db).await?;
        ScheduleAllContactRemindersForNotificationChannel {
            account_id: self.account_id,
            author_id: self.author_id,
            user_notification_channel_id: channel.id,
        }
        .execute(db)
        .await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C, pushover: &Pushover) -> ApiResult<()> {
        let mut v = Validator::new();
        for (field, value) in [
            ("label", &self.label),
            ("user_key", &self.credentials.user_key),
            ("app_token", &self.credentials.app_token),
        ] {
            if value.trim().is_empty() {
                v.fail(field, "required", &[]);
            }
            v.max_len(field, Some(value), 255);
        }
        v.finish()?;
        check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() }).await?;

        pushover.validate(&self.credentials).await.map_err(|e| ApiError::field("user_key", "pushover", failure(&e)))
    }

    async fn create<C: ConnectionTrait>(&self, db: &C) -> ApiResult<user_notification_channels::Model> {
        let t = &self.preferred_time;
        let preferred_time = NaiveTime::parse_from_str(t, "%H:%M:%S")
            .or_else(|_| NaiveTime::parse_from_str(t, "%H:%M"))
            .map_err(|_| ApiError::internal(format!("could not parse preferred_time {t:?}")))?;
        Ok(user_notification_channels::ActiveModel {
            user_id: Set(Some(self.author_id)),
            label: Set(Some(self.label.clone())),
            r#type: Set(TYPE_PUSHOVER.into()),
            content: Set(self.credentials.to_content()),
            preferred_time: Set(Some(preferred_time)),
            active: Set(true),
            verified_at: Set(Some(crate::db::now())),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `DestroyUserNotificationChannel`.
pub struct DestroyUserNotificationChannel {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub user_notification_channel_id: Id,
}

impl DestroyUserNotificationChannel {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let channel = self.validate(db).await?;
        Self::destroy(db, &channel).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<user_notification_channels::Model> {
        let checked =
            check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() }).await?;

        find(db, &checked.author, self.user_notification_channel_id).await
    }

    async fn destroy<C: ConnectionTrait>(db: &C, channel: &user_notification_channels::Model) -> ApiResult<()> {
        user_notification_channels::Entity::delete_by_id(channel.id).exec(db).await?;
        Ok(())
    }
}

/// `ScheduleAllContactRemindersForNotificationChannel`.
pub struct ScheduleAllContactRemindersForNotificationChannel {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub user_notification_channel_id: Id,
}

impl ScheduleAllContactRemindersForNotificationChannel {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<user_notification_channels::Model> {
        let (author, channel) = self.validate(db).await?;
        Self::schedule(db, &author, &channel).await?;
        Ok(channel)
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(users::Model, user_notification_channels::Model)> {
        let checked =
            check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() }).await?;

        let channel = find(db, &checked.author, self.user_notification_channel_id).await?;
        Ok((checked.author, channel))
    }

    async fn schedule<C: ConnectionTrait>(
        db: &C,
        author: &users::Model,
        channel: &user_notification_channels::Model,
    ) -> ApiResult<()> {
        let vaults: Vec<Uuid> = vaults::Entity::find()
            .filter(vaults::Column::AccountId.eq(author.account_id))
            .all(db)
            .await?
            .into_iter()
            .map(|v| v.id)
            .collect();

        let contact_reminders = contact_reminders::Entity::find()
            .join(JoinType::InnerJoin, contact_reminders::Relation::Contacts.def())
            .join(JoinType::InnerJoin, contacts::Relation::Vaults.def())
            .filter(vaults::Column::Id.is_in(vaults))
            .all(db)
            .await?;

        for contact_reminder in contact_reminders {
            let mut upcoming_date = match contact_reminder.year {
                None | Some(0) => date(1900, contact_reminder.month, contact_reminder.day)?,
                Some(year) => date(year, contact_reminder.month, contact_reminder.day)?,
            };

            let now = crate::db::now().0;
            if upcoming_date < now {
                upcoming_date = date(now.year(), Some(upcoming_date.month() as i32), Some(upcoming_date.day() as i32))?;

                if upcoming_date < now {
                    upcoming_date =
                        date(now.year() + 1, Some(upcoming_date.month() as i32), Some(upcoming_date.day() as i32))?;
                }
            }

            let preferred_time =
                channel.preferred_time.ok_or_else(|| ApiError::internal("the channel has no preferred_time"))?;
            let upcoming_date = upcoming_date.date().and_time(
                NaiveTime::from_hms_opt(preferred_time.hour(), preferred_time.minute(), 0).expect("a valid time"),
            );
            let scheduled_at = shift_timezone(upcoming_date, author.timezone.as_deref().unwrap_or("UTC"))?;

            contact_reminder_scheduled::ActiveModel {
                user_notification_channel_id: Set(channel.id),
                contact_reminder_id: Set(contact_reminder.id),
                scheduled_at: Set(scheduled_at.into()),
                created_at: Set(None),
                updated_at: Set(None),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
        Ok(())
    }
}

/// `SendTestEmail`, through Pushover.
pub struct SendTestNotification {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub user_notification_channel_id: Id,
}

impl SendTestNotification {
    pub async fn execute<C: ConnectionTrait>(
        self,
        db: &C,
        pushover: &Pushover,
    ) -> ApiResult<user_notification_channels::Model> {
        let checked =
            check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() }).await?;
        let channel = find(db, &checked.author, self.user_notification_channel_id).await?;
        let credentials = Credentials::from_content(&channel.content).map_err(ApiError::internal)?;

        let name = format!(
            "{} {}",
            checked.author.first_name.as_deref().unwrap_or_default(),
            checked.author.last_name.as_deref().unwrap_or_default()
        );
        let text = i18n::t(&i18n::current(), "This is a test notification for :name", &[("name", name.trim())]);
        pushover
            .send(&credentials, &Message { title: None, message: text.clone() })
            .await
            .map_err(|e| ApiError::new(Code::NotificationFailed).with_detail(failure(&e)))?;

        user_notification_sent::ActiveModel {
            user_notification_channel_id: Set(channel.id),
            sent_at: Set(crate::db::now()),
            subject_line: Set(text),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(channel)
    }
}

/// `ToggleUserNotificationChannel`.
pub struct ToggleUserNotificationChannel {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub user_notification_channel_id: Id,
}

impl ToggleUserNotificationChannel {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<user_notification_channels::Model> {
        let channel = self.validate(db).await?;
        let channel = Self::toggle(db, channel).await?;
        self.update_scheduled_reminders(db, &channel).await?;
        Ok(channel)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<user_notification_channels::Model> {
        let checked =
            check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() }).await?;
        find(db, &checked.author, self.user_notification_channel_id).await
    }

    async fn toggle<C: ConnectionTrait>(
        db: &C,
        channel: user_notification_channels::Model,
    ) -> ApiResult<user_notification_channels::Model> {
        let active = !channel.active;
        let mut channel: user_notification_channels::ActiveModel = channel.into();
        channel.active = Set(active);
        if active {
            channel.fails = Set(0);
        }
        Ok(channel.update(db).await?)
    }

    async fn update_scheduled_reminders<C: ConnectionTrait>(
        &self,
        db: &C,
        channel: &user_notification_channels::Model,
    ) -> ApiResult<()> {
        if channel.active {
            self.rescheduled_reminders(db, channel).await
        } else {
            Self::delete_scheduled_reminders(db, channel).await
        }
    }

    async fn delete_scheduled_reminders<C: ConnectionTrait>(
        db: &C,
        channel: &user_notification_channels::Model,
    ) -> ApiResult<()> {
        // F21: the PHP calls `->contactReminders->each->delete()`, which
        // deletes the contacts' reminders themselves; only this channel's
        // schedules should go.
        contact_reminder_scheduled::Entity::delete_many()
            .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel.id))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn rescheduled_reminders<C: ConnectionTrait>(
        &self,
        db: &C,
        channel: &user_notification_channels::Model,
    ) -> ApiResult<()> {
        ScheduleAllContactRemindersForNotificationChannel {
            account_id: self.account_id,
            author_id: self.author_id,
            user_notification_channel_id: channel.id,
        }
        .execute(db)
        .await?;
        Ok(())
    }
}

/// Why Pushover didn't take a request, for the user.
fn failure(e: &pushover::Error) -> String {
    let l = i18n::current();
    match e {
        pushover::Error::Rejected(errors) => i18n::t(&l, "Pushover refused these keys: :errors", &[("errors", errors)]),
        pushover::Error::Unreachable(_) => i18n::t(&l, "Pushover could not be reached. Try again later.", &[]),
    }
}

/// `$this->author->notificationChannels()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    id: Id,
) -> ApiResult<user_notification_channels::Model> {
    user_notification_channels::Entity::find_by_id(id)
        .filter(user_notification_channels::Column::UserId.eq(author.id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `Carbon::parse("$year-$month-$day")` at midnight: like PHP, day 0 and days
/// past the end of the month roll over, a missing part or a month above 12 or
/// day above 31 fails.
fn date(year: i32, month: Option<i32>, day: Option<i32>) -> ApiResult<NaiveDateTime> {
    let (Some(month @ 0..=12), Some(day @ 0..=31)) = (month, day) else {
        return Err(ApiError::internal(format!("could not parse {year}-{month:?}-{day:?}")));
    };
    let months = year * 12 + month - 1;
    NaiveDate::from_ymd_opt(months.div_euclid(12), months.rem_euclid(12) as u32 + 1, 1)
        .and_then(|d| d.checked_add_signed(chrono::Duration::days(i64::from(day) - 1)))
        .map(|d| d.and_time(NaiveTime::MIN))
        .ok_or_else(|| ApiError::internal(format!("could not parse {year}-{month}-{day}")))
}

/// `->shiftTimezone($timezone)->tz('UTC')`: the UTC time of the wall-clock
/// time `local` in `timezone`. No timezone database is linked, so only UTC
/// is known.
fn shift_timezone(local: NaiveDateTime, timezone: &str) -> ApiResult<NaiveDateTime> {
    if timezone.eq_ignore_ascii_case("UTC") {
        Ok(local)
    } else {
        Err(ApiError::internal(format!("unknown timezone {timezone}")))
    }
}
