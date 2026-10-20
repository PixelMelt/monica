//! `app/Domains/Settings/ManageNotificationChannels/Services` and `Jobs`.

#[cfg(test)]
mod tests;

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, JoinType, QueryFilter, QuerySelect,
    RelationTrait,
};

use crate::config::Config;
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::{
    contact_reminder_scheduled, contact_reminders, contacts, user_notification_channels, user_notification_sent, users,
    vaults,
};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::mail::{Email, Mailer};
use crate::validate::Validator;

/// `UserNotificationChannel::TYPE_EMAIL`.
pub const TYPE_EMAIL: &str = "email";

/// `CreateUserNotificationChannel`.
pub struct CreateUserNotificationChannel {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub label: String,
    pub r#type: String,
    pub content: String,
    pub verify_email: Option<bool>,
    pub preferred_time: Option<String>,
}

impl CreateUserNotificationChannel {
    pub async fn execute<C: ConnectionTrait>(
        self,
        db: &C,
        mailer: &Mailer,
        config: &Config,
    ) -> ApiResult<user_notification_channels::Model> {
        self.validate(db).await?;
        let channel = self.create(db).await?;
        self.verify_channel(mailer, config, &channel).await?;
        Ok(channel)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        for (field, value, max) in
            [("label", &self.label, 255), ("type", &self.r#type, 255), ("content", &self.content, 65535)]
        {
            if value.trim().is_empty() {
                v.fail(field, "required", &[]);
            }
            v.max_len(field, Some(value), max);
        }
        v.finish()?;
        check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() }).await?;

        let exists = user_notification_channels::Entity::find()
            .filter(user_notification_channels::Column::Content.eq(&self.content))
            .one(db)
            .await?
            .is_some();

        if exists {
            return Err(ApiError::field(
                "content",
                "unique",
                i18n::t(&i18n::current(), "The email is already taken. Please choose another one.", &[]),
            ));
        }
        Ok(())
    }

    async fn create<C: ConnectionTrait>(&self, db: &C) -> ApiResult<user_notification_channels::Model> {
        let preferred_time = match self.preferred_time.as_deref() {
            None => None,
            Some(t) => Some(
                NaiveTime::parse_from_str(t, "%H:%M:%S")
                    .or_else(|_| NaiveTime::parse_from_str(t, "%H:%M"))
                    .map_err(|_| ApiError::internal(format!("could not parse preferred_time {t:?}")))?,
            ),
        };
        let channel = user_notification_channels::ActiveModel {
            user_id: Set(Some(self.author_id)),
            label: Set(Some(self.label.clone())),
            r#type: Set(self.r#type.clone()),
            content: Set(self.content.clone()),
            preferred_time: Set(preferred_time),
            ..Default::default()
        }
        .insert(db)
        .await?;

        let mut channel: user_notification_channels::ActiveModel = channel.into();
        channel.verification_token = Set(Some(uuid::Uuid::new_v4().to_string()));
        Ok(channel.update(db).await?)
    }

    async fn verify_channel(
        &self,
        mailer: &Mailer,
        config: &Config,
        channel: &user_notification_channels::Model,
    ) -> ApiResult<()> {
        if self.r#type == TYPE_EMAIL && self.verify_email == Some(true) {
            SendVerificationEmailChannel { channel: channel.clone() }.handle(mailer, config).await?;
        }
        Ok(())
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

/// `SendTestEmail`.
pub struct SendTestEmail {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub user_notification_channel_id: Id,
}

impl SendTestEmail {
    pub async fn execute<C: ConnectionTrait>(
        self,
        db: &C,
        mailer: &Mailer,
    ) -> ApiResult<user_notification_channels::Model> {
        let channel = self.validate(db).await?;
        Self::send(mailer, &channel).await?;
        Self::log(db, &channel).await?;
        Ok(channel)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<user_notification_channels::Model> {
        let checked =
            check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() }).await?;

        let channel = find(db, &checked.author, self.user_notification_channel_id).await?;

        if channel.r#type != TYPE_EMAIL {
            return Err(ApiError::conflict("Only email can be sent."));
        }
        Ok(channel)
    }

    async fn send(mailer: &Mailer, channel: &user_notification_channels::Model) -> ApiResult<()> {
        let l = i18n::current();
        let (text, html) = Mailer::render(
            &[
                i18n::t(&l, "This is a test email", &[]),
                i18n::t(
                    &l,
                    "This email is a test email to check if Monica can send an email to this email address.",
                    &[],
                ),
                i18n::t(&l, "So… it works 😼", &[]),
            ],
            None,
            &[i18n::t(&l, "Have a great day,", &[]), "Monica".into()],
        );
        mailer
            .send(Email { to: channel.content.clone(), subject: i18n::t(&l, "Test email for Monica", &[]), text, html })
            .await
            .map_err(ApiError::internal)
    }

    async fn log<C: ConnectionTrait>(db: &C, channel: &user_notification_channels::Model) -> ApiResult<()> {
        user_notification_sent::ActiveModel {
            user_notification_channel_id: Set(channel.id),
            sent_at: Set(crate::db::now()),
            subject_line: Set(i18n::t(&i18n::current(), "Test email for Monica", &[])),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(())
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
        let contact_reminders = contact_reminders::Entity::find()
            .inner_join(contact_reminder_scheduled::Entity)
            .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel.id))
            .all(db)
            .await?;
        for contact_reminder in contact_reminders {
            contact_reminders::Entity::delete_by_id(contact_reminder.id).exec(db).await?;
        }
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

/// `VerifyUserNotificationChannelEmailAddress`.
pub struct VerifyUserNotificationChannelEmailAddress {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub user_notification_channel_id: Id,
    pub uuid: Uuid,
}

impl VerifyUserNotificationChannelEmailAddress {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<user_notification_channels::Model> {
        let channel = self.validate(db).await?;
        let channel = Self::verify(db, channel).await?;
        self.rescheduled_reminders(db, &channel).await?;
        Ok(channel)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<user_notification_channels::Model> {
        let checked =
            check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() }).await?;

        let channel = find(db, &checked.author, self.user_notification_channel_id).await?;

        if channel.verification_token.as_deref() != Some(self.uuid.to_string().as_str()) {
            let mut v = Validator::new();
            v.fail("uuid", "in", &[]);
            v.finish()?;
        }
        Ok(channel)
    }

    async fn verify<C: ConnectionTrait>(
        db: &C,
        channel: user_notification_channels::Model,
    ) -> ApiResult<user_notification_channels::Model> {
        let mut channel: user_notification_channels::ActiveModel = channel.into();
        channel.verified_at = Set(Some(crate::db::now()));
        Ok(channel.update(db).await?)
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

/// `Jobs/SendVerificationEmailChannel`.
pub struct SendVerificationEmailChannel {
    pub channel: user_notification_channels::Model,
}

impl SendVerificationEmailChannel {
    pub async fn handle(self, mailer: &Mailer, config: &Config) -> ApiResult<()> {
        if self.channel.r#type != TYPE_EMAIL {
            return Ok(());
        }

        let l = i18n::current();
        let token = self
            .channel
            .verification_token
            .as_deref()
            .ok_or_else(|| ApiError::internal("missing verification_token"))?;
        let url = format!("{}/settings/notifications/{}/verify/{token}", config.url, self.channel.id);
        let (text, html) = Mailer::render(
            &[
                i18n::t(&l, "Please verify your email address", &[]),
                i18n::t(
                    &l,
                    "You’ve been invited to use this email address in Monica, an open source personal CRM, so we can use it to send you notifications.",
                    &[],
                ),
            ],
            Some((&i18n::t(&l, "Verify this email address", &[]), &url)),
            &[
                i18n::t(&l, "If you’ve received this invitation by mistake, please discard it.", &[]),
                i18n::t(&l, "Thanks,", &[]),
                "Monica".into(),
            ],
        );
        mailer
            .send(Email {
                to: self.channel.content.clone(),
                subject: i18n::t(&l, "Please validate your email address", &[]),
                text,
                html,
            })
            .await
            .map_err(ApiError::internal)
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
