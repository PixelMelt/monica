//! `app/Domains/Settings/ManageCallReasons/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::{call_reason_types, call_reasons};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateCallReason`.
pub struct CreateCallReason {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub call_reason_type_id: Id,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
}

impl CreateCallReason {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<call_reasons::Model> {
        let mut v = Validator::new();
        v.max_len("label", self.label.as_deref(), 255);
        v.max_len("label_translation_key", self.label_translation_key.as_deref(), 255);
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let reason_type = find_type(db, self.account_id, self.call_reason_type_id).await?;

        let call_reason = call_reasons::ActiveModel {
            call_reason_type_id: Set(reason_type.id),
            label: Set(self.label),
            label_translation_key: Set(self.label_translation_key),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(call_reason)
    }
}

/// `CreateCallReasonType`.
pub struct CreateCallReasonType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
}

impl CreateCallReasonType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<call_reason_types::Model> {
        let mut v = Validator::new();
        v.max_len("label", self.label.as_deref(), 255);
        v.max_len("label_translation_key", self.label_translation_key.as_deref(), 255);
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let reason_type = call_reason_types::ActiveModel {
            account_id: Set(self.account_id),
            label: Set(self.label),
            label_translation_key: Set(self.label_translation_key),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(reason_type)
    }
}

/// `DestroyCallReason`.
pub struct DestroyCallReason {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub call_reason_type_id: Id,
    pub call_reason_id: Id,
}

impl DestroyCallReason {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let reason_type = find_type(db, self.account_id, self.call_reason_type_id).await?;

        let reason = find_reason(db, reason_type.id, self.call_reason_id).await?;

        call_reasons::Entity::delete_by_id(reason.id).exec(db).await?;
        Ok(())
    }
}

/// `DestroyCallReasonType`.
pub struct DestroyCallReasonType {
    pub account_id: Uuid,
    pub call_reason_type_id: Id,
    pub author_id: Uuid,
}

impl DestroyCallReasonType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let reason_type = find_type(db, self.account_id, self.call_reason_type_id).await?;

        call_reason_types::Entity::delete_by_id(reason_type.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateCallReason`.
pub struct UpdateCallReason {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub call_reason_type_id: Id,
    pub call_reason_id: Id,
    pub label: String,
}

impl UpdateCallReason {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<call_reasons::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let reason_type = find_type(db, self.account_id, self.call_reason_type_id).await?;

        let reason = find_reason(db, reason_type.id, self.call_reason_id).await?;

        let mut reason: call_reasons::ActiveModel = reason.into();
        reason.label = Set(Some(self.label));
        Ok(reason.update(db).await?)
    }
}

/// `UpdateCallReasonType`.
pub struct UpdateCallReasonType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub call_reason_type_id: Id,
    pub label: String,
}

impl UpdateCallReasonType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<call_reason_types::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                administrator: true,
                ..Default::default()
            },
        )
        .await?;

        let reason_type = find_type(db, self.account_id, self.call_reason_type_id).await?;

        let mut reason_type: call_reason_types::ActiveModel = reason_type.into();
        reason_type.label = Set(Some(self.label));
        Ok(reason_type.update(db).await?)
    }
}

/// `$this->account()->callReasonTypes()->findOrFail($id)`.
async fn find_type<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    call_reason_type_id: Id,
) -> ApiResult<call_reason_types::Model> {
    call_reason_types::Entity::find_by_id(call_reason_type_id)
        .filter(call_reason_types::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$reasonType->callReasons()->findOrFail($id)`.
async fn find_reason<C: ConnectionTrait>(
    db: &C,
    call_reason_type_id: Id,
    call_reason_id: Id,
) -> ApiResult<call_reasons::Model> {
    call_reasons::Entity::find_by_id(call_reason_id)
        .filter(call_reasons::Column::CallReasonTypeId.eq(call_reason_type_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
