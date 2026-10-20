//! `app/Domains/Vault/ManageVaultSettings/Services`.

#[cfg(test)]
mod tests;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait,
};

use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::{
    contact_reminder_scheduled, contact_reminders, contacts, templates, user_notification_channels, user_vault, users,
    vaults,
};
use crate::error::{ApiError, ApiResult, Code};
use crate::validate::Validator;

/// `ChangeVaultAccess`.
pub struct ChangeVaultAccess {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub user_id: Uuid,
    pub permission: i32,
}

impl ChangeVaultAccess {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let (checked, user) = validate(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                vault: Some((self.vault_id, Some(VaultPermission::Manage))),
                ..Default::default()
            },
            self.user_id,
        )
        .await?;
        change(db, checked.vault(), &user, self.permission).await?;

        Ok(user)
    }
}

async fn change<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    user: &users::Model,
    permission: i32,
) -> ApiResult<()> {
    // We need to get the vault with pivot, $this->vault does not contains it.
    let pivot = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(user.id))
        .filter(user_vault::Column::VaultId.eq(vault.id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::internal("the user is not in the vault"))?;
    let mut pivot: user_vault::ActiveModel = pivot.into();
    pivot.permission = Set(permission);
    pivot.update(db).await?;
    Ok(())
}

/// `GrantVaultAccessToUser`.
pub struct GrantVaultAccessToUser {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub user_id: Uuid,
    pub permission: i32,
}

impl GrantVaultAccessToUser {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let (checked, user) = validate(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Manage))),
                ..Default::default()
            },
            self.user_id,
        )
        .await?;
        grant(db, checked.vault(), &user, self.permission).await?;
        schedule_contact_reminders(db, checked.vault(), &user).await?;

        Ok(user)
    }
}

async fn grant<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    user: &users::Model,
    permission: i32,
) -> ApiResult<()> {
    // F20: the PHP inserts a second membership and contact for an existing
    // member; the spec answers 409.
    let member = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(user.id))
        .filter(user_vault::Column::VaultId.eq(vault.id))
        .one(db)
        .await?;
    if member.is_some() {
        return Err(ApiError::conflict("The user is already a member of this vault."));
    }

    let contact = contacts::ActiveModel {
        id: Set(Uuid::new()),
        vault_id: Set(vault.id),
        first_name: Set(user.first_name.clone()),
        last_name: Set(user.last_name.clone()),
        can_be_deleted: Set(false),
        template_id: Set(vault.default_template_id),
        ..Default::default()
    }
    .insert(db)
    .await?;

    user_vault::ActiveModel {
        vault_id: Set(vault.id),
        user_id: Set(user.id),
        permission: Set(permission),
        contact_id: Set(contact.id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// When a user is granted access to a vault, we need to schedule all the
/// contact reminders for all the contacts in the vault, for the user.
async fn schedule_contact_reminders<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    user: &users::Model,
) -> ApiResult<()> {
    let contact_ids = contacts::Entity::live()
        .select_only()
        .column(contacts::Column::Id)
        .filter(contacts::Column::VaultId.eq(vault.id))
        .into_query();
    let contact_reminders = contact_reminders::Entity::find()
        .filter(contact_reminders::Column::ContactId.in_subquery(contact_ids))
        .all(db)
        .await?;

    for contact_reminder in contact_reminders {
        // if the contact reminder is a one time reminder, we need to make
        // sure the `number_times_triggered` is equal to 0, as otherwise,
        // the reminder will be scheduled again, which we don't want.
        if contact_reminder.r#type == "one_time" && contact_reminder.number_times_triggered != 0 {
            continue;
        }

        ScheduleContactReminderForUser { contact_reminder_id: contact_reminder.id, user_id: user.id }
            .execute(db)
            .await?;
    }
    Ok(())
}

/// `RemoveVaultAccess`.
pub struct RemoveVaultAccess {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub user_id: Uuid,
}

impl RemoveVaultAccess {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let (checked, user) = validate(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Manage))),
                ..Default::default()
            },
            self.user_id,
        )
        .await?;
        remove(db, checked.vault(), &user).await?;
        remove_all_reminders_for_this_user_in_this_vault(db, checked.vault(), &user).await
    }
}

async fn remove<C: ConnectionTrait>(db: &C, vault: &vaults::Model, user: &users::Model) -> ApiResult<()> {
    let pivot = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(user.id))
        .filter(user_vault::Column::VaultId.eq(vault.id))
        .one(db)
        .await?;

    if let Some(pivot) = pivot {
        let contact = contacts::Entity::live()
            .filter(contacts::Column::Id.eq(pivot.contact_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::internal("the user's contact in the vault is deleted"))?;
        let mut contact: contacts::ActiveModel = contact.into();
        contact.deleted_at = Set(Some(crate::db::now()));
        contact.update(db).await?;
        // F19: soft-deleting the contact doesn't cascade to `user_vault`, so
        // the PHP leaves the user a member; remove the membership itself.
        user_vault::Entity::delete_many()
            .filter(user_vault::Column::UserId.eq(user.id))
            .filter(user_vault::Column::VaultId.eq(vault.id))
            .exec(db)
            .await?;
    }
    Ok(())
}

/// We need to remove all the contact reminders that were scheduled for this
/// user in this vault.
async fn remove_all_reminders_for_this_user_in_this_vault<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    user: &users::Model,
) -> ApiResult<()> {
    let channel_ids = user_notification_channels::Entity::find()
        .select_only()
        .column(user_notification_channels::Column::Id)
        .filter(user_notification_channels::Column::UserId.eq(user.id))
        .into_query();
    let contact_ids = contacts::Entity::find()
        .select_only()
        .column(contacts::Column::Id)
        .filter(contacts::Column::VaultId.eq(vault.id))
        .into_query();
    let contact_reminder_ids = contact_reminders::Entity::find()
        .select_only()
        .column(contact_reminders::Column::Id)
        .filter(contact_reminders::Column::ContactId.in_subquery(contact_ids))
        .into_query();

    contact_reminder_scheduled::Entity::delete_many()
        .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.in_subquery(channel_ids))
        .filter(contact_reminder_scheduled::Column::ContactReminderId.in_subquery(contact_reminder_ids))
        .exec(db)
        .await?;
    Ok(())
}

/// `UpdateVaultDefaultTemplate`.
pub struct UpdateVaultDefaultTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub template_id: Id,
}

impl UpdateVaultDefaultTemplate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<vaults::Model> {
        let mut v = Validator::new();
        if templates::Entity::find_by_id(self.template_id).one(db).await?.is_none() {
            v.fail("template_id", "exists", &[]);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Manage))),
                ..Default::default()
            },
        )
        .await?;

        let template_id = (self.template_id.0 != 0).then_some(self.template_id);
        if let Some(id) = template_id {
            templates::Entity::find_by_id(id)
                .filter(templates::Column::AccountId.eq(checked.author.account_id))
                .one(db)
                .await?
                .ok_or_else(ApiError::not_found)?;
        }

        let mut vault: vaults::ActiveModel = checked.vault().clone().into();
        vault.default_template_id = Set(template_id);
        Ok(vault.update(db).await?)
    }
}

/// `validate()` of ChangeVaultAccess, GrantVaultAccessToUser and
/// RemoveVaultAccess: the rules, the permissions, then the user to act on.
async fn validate<C: ConnectionTrait>(db: &C, c: &Check, user_id: Uuid) -> ApiResult<(Checked, users::Model)> {
    let mut v = Validator::new();
    if users::Entity::find_by_id(user_id).one(db).await?.is_none() {
        v.fail("user_id", "exists", &[]);
    }
    v.finish()?;
    let checked = check(db, c).await?;

    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::AccountId.eq(checked.author.account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;

    if user.id == checked.author.id {
        return Err(ApiError::new(Code::Conflict));
    }
    Ok((checked, user))
}

/// `App\Domains\Contact\ManageReminders\Services\ScheduleContactReminderForUser`,
/// called by GrantVaultAccessToUser.
struct ScheduleContactReminderForUser {
    contact_reminder_id: Id,
    user_id: Uuid,
}

impl ScheduleContactReminderForUser {
    async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
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
        let vault = vaults::Entity::find_by_id(contact.vault_id).one(db).await?.ok_or_else(ApiError::not_found)?;
        if user.account_id != vault.account_id {
            return Err(ApiError::not_found());
        }

        let upcoming_date = get_date(&contact_reminder)?;
        schedule(db, &contact_reminder, &user, upcoming_date).await
    }
}

/// A ContactReminder can be either a complete date, or only a day/month.
/// If it is only a day/month, we need to add a fake year so we can still
/// manipulate the date as a Carbon object.
fn get_date(contact_reminder: &contact_reminders::Model) -> ApiResult<NaiveDateTime> {
    let year = match contact_reminder.year {
        None | Some(0) => 1900,
        Some(year) => year,
    };
    // `Carbon::parse("$year-$month-$day")`: day 0–31 and month 0–12 roll over.
    match (contact_reminder.month, contact_reminder.day) {
        (Some(month @ 0..=12), Some(day @ 0..=31)) => Ok(set_date(year, month, day).and_time(Default::default())),
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
        let (month, day) = (upcoming_date.month() as i32, upcoming_date.day() as i32);
        upcoming_date = set_date(now.year(), month, day).and_time(upcoming_date.time());
        if upcoming_date < now {
            upcoming_date = set_date(now.year() + 1, month, day).and_time(upcoming_date.time());
        }
    }

    let notification_channels = user_notification_channels::Entity::find()
        .filter(user_notification_channels::Column::UserId.eq(user.id))
        .order_by_asc(user_notification_channels::Column::Id)
        .all(db)
        .await?;
    for channel in notification_channels {
        let preferred_time = channel
            .preferred_time
            .ok_or_else(|| ApiError::internal("the notification channel has no preferred time"))?;
        upcoming_date = upcoming_date
            .date()
            .and_hms_opt(preferred_time.hour(), preferred_time.minute(), upcoming_date.second())
            .expect("a NaiveTime's hour and minute");
        let scheduled_at = Timestamp(upcoming_date);

        let scheduled = contact_reminder_scheduled::Entity::find()
            .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(contact_reminder.id))
            .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel.id))
            .one(db)
            .await?;
        if scheduled.is_none() {
            contact_reminder_scheduled::ActiveModel {
                user_notification_channel_id: Set(channel.id),
                contact_reminder_id: Set(contact_reminder.id),
                scheduled_at: Set(scheduled_at),
                ..Default::default()
            }
            .insert(db)
            .await?;
        } else {
            contact_reminder_scheduled::Entity::update_many()
                .col_expr(contact_reminder_scheduled::Column::ScheduledAt, Expr::value(scheduled_at))
                .col_expr(contact_reminder_scheduled::Column::UpdatedAt, Expr::value(crate::db::now()))
                .filter(contact_reminder_scheduled::Column::ContactReminderId.eq(contact_reminder.id))
                .filter(contact_reminder_scheduled::Column::UserNotificationChannelId.eq(channel.id))
                .exec(db)
                .await?;
        }
    }
    Ok(())
}

/// PHP's `setDate`: months and days past the end of the month roll over.
fn set_date(year: i32, month: i32, day: i32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year + (month - 1).div_euclid(12), (month - 1).rem_euclid(12) as u32 + 1, 1)
        .expect("the first of a month");
    first + Duration::days(i64::from(day) - 1)
}
