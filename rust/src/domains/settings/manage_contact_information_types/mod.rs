//! `app/Domains/Settings/ManageContactInformationTypes/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::contact_information_types;
use crate::error::{ApiError, ApiResult, Code};
use crate::validate::Validator;

/// `CreateContactInformationType`.
pub struct CreateContactInformationType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub r#type: String,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    pub protocol: Option<String>,
}

impl CreateContactInformationType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_information_types::Model> {
        let mut v = Validator::new();
        if self.r#type.trim().is_empty() {
            v.fail("type", "required", &[]);
        }
        v.max_len("type", Some(&self.r#type), 255);
        v.max_len("name", self.name.as_deref(), 255);
        v.max_len("name_translation_key", self.name_translation_key.as_deref(), 255);
        v.max_len("protocol", self.protocol.as_deref(), 255);
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

        Ok(contact_information_types::ActiveModel {
            account_id: Set(self.account_id),
            name: Set(self.name),
            name_translation_key: Set(self.name_translation_key),
            r#type: Set(value_or_null(Some(self.r#type))),
            protocol: Set(value_or_null(self.protocol)),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `DestroyContactInformationType`.
pub struct DestroyContactInformationType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub contact_information_type_id: Id,
}

impl DestroyContactInformationType {
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

        let info_type = find(db, self.account_id, self.contact_information_type_id).await?;
        if !info_type.can_be_deleted {
            return Err(ApiError::new(Code::Conflict));
        }

        contact_information_types::Entity::delete_by_id(info_type.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateContactInformationType`.
pub struct UpdateContactInformationType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub contact_information_type_id: Id,
    pub r#type: String,
    pub name: String,
    pub protocol: Option<String>,
}

impl UpdateContactInformationType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_information_types::Model> {
        let mut v = Validator::new();
        if self.r#type.trim().is_empty() {
            v.fail("type", "required", &[]);
        }
        v.max_len("type", Some(&self.r#type), 255);
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        v.max_len("protocol", self.protocol.as_deref(), 255);
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

        let info_type = find(db, self.account_id, self.contact_information_type_id).await?;

        let mut info_type: contact_information_types::ActiveModel = info_type.into();
        info_type.name = Set(Some(self.name));
        info_type.r#type = Set(value_or_null(Some(self.r#type)));
        info_type.protocol = Set(value_or_null(self.protocol));
        Ok(info_type.update(db).await?)
    }
}

/// `$this->account()->contactInformationTypes()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    contact_information_type_id: Id,
) -> ApiResult<contact_information_types::Model> {
    contact_information_types::Entity::find_by_id(contact_information_type_id)
        .filter(contact_information_types::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `valueOrNull`: `empty()` values become null.
fn value_or_null(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty() && v != "0")
}
