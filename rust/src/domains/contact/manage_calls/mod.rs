//! `app/Domains/Contact/ManageCalls/Services`.

#[cfg(test)]
mod tests;

use chrono::{NaiveDate, NaiveTime};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check, value_or_null};
use crate::entities::{call_reasons, calls, contacts, emotions, users};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateCall`.
pub struct CreateCall {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub call_reason_id: Option<Id>,
    pub called_at: String,
    pub duration: Option<i32>,
    pub description: Option<String>,
    pub r#type: String,
    pub answered: Option<bool>,
    pub who_initiated: String,
    pub emotion_id: Option<Id>,
}

impl CreateCall {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<calls::Model> {
        let (checked, called_at) = self.validate(db).await?;

        let call = calls::ActiveModel {
            contact_id: Set(self.contact_id),
            author_id: Set(Some(checked.author.id)),
            author_name: Set(name(&checked.author)),
            called_at: Set(called_at),
            duration: Set(self.duration.filter(|d| *d != 0)),
            call_reason_id: Set(self.call_reason_id.filter(|id| id.0 != 0)),
            emotion_id: Set(self.emotion_id.filter(|id| id.0 != 0)),
            description: Set(value_or_null(self.description)),
            r#type: Set(self.r#type),
            answered: Set(self.answered.unwrap_or(true)),
            who_initiated: Set(self.who_initiated),
            ..Default::default()
        }
        .insert(db)
        .await?;
        update_last_edited_date(db, checked.contact()).await?;

        Ok(call)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(Checked, Timestamp)> {
        let mut v = Validator::new();
        if let Some(id) = self.call_reason_id
            && call_reasons::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("call_reason_id", "exists", &[]);
        }
        let called_at = date_format(&mut v, "called_at", &self.called_at);
        v.max_len("description", self.description.as_deref(), 65535);
        if self.r#type.trim().is_empty() {
            v.fail("type", "required", &[]);
        }
        if self.who_initiated.trim().is_empty() {
            v.fail("who_initiated", "required", &[]);
        }
        if let Some(id) = self.emotion_id
            && emotions::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("emotion_id", "exists", &[]);
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

        if let Some(id) = self.emotion_id.filter(|id| id.0 != 0) {
            emotion_in_account(db, checked.author.account_id, id).await?;
        }
        Ok((checked, called_at.expect("validated")))
    }
}

/// `DestroyCall`.
pub struct DestroyCall {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub call_id: Id,
}

impl DestroyCall {
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

        let call = find(db, checked.contact(), self.call_id).await?;

        calls::Entity::delete_by_id(call.id).exec(db).await?;

        update_last_edited_date(db, checked.contact()).await
    }
}

/// `UpdateCall`.
pub struct UpdateCall {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub call_id: Id,
    pub call_reason_id: Option<Id>,
    pub called_at: String,
    pub duration: Option<i32>,
    /// F8: not in the PHP rules, which drop the description on update.
    pub description: Option<String>,
    pub r#type: String,
    pub answered: Option<bool>,
    pub who_initiated: String,
    pub emotion_id: Option<Id>,
}

impl UpdateCall {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<calls::Model> {
        let (checked, called_at) = self.validate(db).await?;

        let call = find(db, checked.contact(), self.call_id).await?;

        let mut call: calls::ActiveModel = call.into();
        call.called_at = Set(called_at);
        call.duration = Set(self.duration.filter(|d| *d != 0));
        call.call_reason_id = Set(self.call_reason_id.filter(|id| id.0 != 0));
        call.emotion_id = Set(self.emotion_id.filter(|id| id.0 != 0));
        call.description = Set(value_or_null(self.description));
        call.r#type = Set(self.r#type);
        call.answered = Set(self.answered.unwrap_or(true));
        call.who_initiated = Set(self.who_initiated);
        let call = call.update(db).await?;

        update_last_edited_date(db, checked.contact()).await?;

        Ok(call)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(Checked, Timestamp)> {
        let mut v = Validator::new();
        if let Some(id) = self.call_reason_id
            && call_reasons::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("call_reason_id", "exists", &[]);
        }
        let called_at = date_format(&mut v, "called_at", &self.called_at);
        v.max_len("description", self.description.as_deref(), 65535);
        if self.r#type.trim().is_empty() {
            v.fail("type", "required", &[]);
        }
        if self.who_initiated.trim().is_empty() {
            v.fail("who_initiated", "required", &[]);
        }
        if let Some(id) = self.emotion_id
            && emotions::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("emotion_id", "exists", &[]);
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

        if let Some(id) = self.emotion_id.filter(|id| id.0 != 0) {
            emotion_in_account(db, checked.author.account_id, id).await?;
        }
        Ok((checked, called_at.expect("validated")))
    }
}

/// `required|date_format:Y-m-d`: the date at midnight, when `value` is one.
fn date_format(v: &mut Validator, field: &str, value: &str) -> Option<Timestamp> {
    if value.trim().is_empty() {
        v.fail(field, "required", &[]);
        return None;
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok().filter(|d| d.format("%Y-%m-%d").to_string() == value);
    if date.is_none() {
        v.fail(field, "date_format", &[("format", "Y-m-d")]);
    }
    date.map(|d| Timestamp(d.and_time(NaiveTime::MIN)))
}

/// `$this->account()->emotions()->findOrFail($id)`.
async fn emotion_in_account<C: ConnectionTrait>(db: &C, account_id: Uuid, emotion_id: Id) -> ApiResult<()> {
    emotions::Entity::find_by_id(emotion_id)
        .filter(emotions::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(())
}

/// `$this->contact->calls()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, contact: &contacts::Model, call_id: Id) -> ApiResult<calls::Model> {
    calls::Entity::find_by_id(call_id)
        .filter(calls::Column::ContactId.eq(contact.id))
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
