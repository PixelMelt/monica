//! `app/Domains/Vault/ManageVaultSettings/Services`.

mod labels;
mod life_events;
mod mood_tracking_parameters;
mod quick_fact_templates;
mod tags;
#[cfg(test)]
mod tests;

pub use labels::*;
pub use life_events::*;
pub use mood_tracking_parameters::*;
pub use quick_fact_templates::*;
pub use tags::*;

#[cfg(test)]
use chrono::{Datelike, NaiveDate};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, QueryTrait,
};

use crate::db::id::Id;
#[cfg(test)]
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::domains::contact::manage_reminders::ScheduleContactReminderForUser;
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
