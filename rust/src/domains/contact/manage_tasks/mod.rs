//! `app/Domains/Contact/ManageTasks/Services`.

#[cfg(test)]
mod tests;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::{contact_tasks, contacts, users};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateContactTask`.
pub struct CreateContactTask {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub label: String,
    pub description: Option<String>,
    pub due_at: Option<String>,
}

impl CreateContactTask {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_tasks::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.max_len("description", self.description.as_deref(), 65535);
        date(&mut v, "due_at", self.due_at.as_deref());
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

        let task = contact_tasks::ActiveModel {
            contact_id: Set(self.contact_id),
            author_id: Set(Some(checked.author.id)),
            author_name: Set(name(&checked.author)),
            label: Set(self.label),
            description: Set(value_or_null(self.description)),
            due_at: Set(value_or_null(self.due_at).as_deref().and_then(parse_date)),
            uuid: Set(Some(Uuid::new())),
            ..Default::default()
        }
        .insert(db)
        .await?;
        update_last_edited_date(db, checked.contact()).await?;

        Ok(task)
    }
}

/// `DestroyContactTask`.
pub struct DestroyContactTask {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_task_id: Id,
}

impl DestroyContactTask {
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

        let task = find(db, checked.contact(), self.contact_task_id).await?;

        let mut task: contact_tasks::ActiveModel = task.into();
        task.deleted_at = Set(Some(crate::db::now()));
        task.update(db).await?;

        update_last_edited_date(db, checked.contact()).await
    }
}

/// `ToggleContactTask`.
pub struct ToggleContactTask {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_task_id: Id,
}

impl ToggleContactTask {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_tasks::Model> {
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

        let task = find(db, checked.contact(), self.contact_task_id).await?;

        let completed = !task.completed;
        let mut task: contact_tasks::ActiveModel = task.into();
        task.completed = Set(completed);
        task.completed_at = Set(completed.then(crate::db::now));
        let task = task.update(db).await?;

        update_last_edited_date(db, checked.contact()).await?;

        Ok(task)
    }
}

/// `UpdateContactTask`.
pub struct UpdateContactTask {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_task_id: Id,
    pub label: String,
    pub description: Option<String>,
    pub due_at: Option<String>,
}

impl UpdateContactTask {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_tasks::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.max_len("description", self.description.as_deref(), 65535);
        date(&mut v, "due_at", self.due_at.as_deref());
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

        let task = find(db, checked.contact(), self.contact_task_id).await?;

        let mut task: contact_tasks::ActiveModel = task.into();
        task.label = Set(self.label);
        task.description = Set(value_or_null(self.description));
        task.due_at = Set(value_or_null(self.due_at).as_deref().and_then(parse_date));
        let task = task.update(db).await?;

        update_last_edited_date(db, checked.contact()).await?;

        Ok(task)
    }
}

/// `nullable|date`, on the forms the app sends (`Y-m-d`, `Y-m-d H:i:s`).
fn date(v: &mut Validator, field: &str, value: Option<&str>) {
    if let Some(s) = value
        && !s.trim().is_empty()
        && parse_date(s).is_none()
    {
        v.fail(field, "date", &[]);
    }
}

/// The `datetime` cast of `due_at`: a `Y-m-d` date is midnight.
fn parse_date(value: &str) -> Option<Timestamp> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|d| d.and_time(NaiveTime::MIN))
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S"))
        .ok()
        .map(Timestamp)
}

/// `$this->contact->tasks()->findOrFail($id)`: soft-deleted tasks excluded.
async fn find<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
    contact_task_id: Id,
) -> ApiResult<contact_tasks::Model> {
    contact_tasks::Entity::live()
        .filter(contact_tasks::Column::Id.eq(contact_task_id))
        .filter(contact_tasks::Column::ContactId.eq(contact.id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `User::name`.
fn name(user: &users::Model) -> String {
    format!("{} {}", user.first_name.as_deref().unwrap_or(""), user.last_name.as_deref().unwrap_or(""))
}

/// `updateLastEditedDate()`: `$this->contact->last_updated_at = Carbon::now()`.
async fn update_last_edited_date<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<()> {
    let mut contact: contacts::ActiveModel = contact.clone().into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    contact.update(db).await?;
    Ok(())
}
