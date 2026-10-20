//! `app/Domains/Settings/ManageAddressTypes/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check, value_or_null};
use crate::entities::address_types;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateAddressType`.
pub struct CreateAddressType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    pub r#type: Option<String>,
}

impl CreateAddressType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<address_types::Model> {
        let mut v = Validator::new();
        v.max_len("name", self.name.as_deref(), 255);
        v.max_len("name_translation_key", self.name_translation_key.as_deref(), 255);
        v.max_len("type", self.r#type.as_deref(), 255);
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

        let address_type = address_types::ActiveModel {
            account_id: Set(self.account_id),
            name: Set(value_or_null(self.name)),
            name_translation_key: Set(value_or_null(self.name_translation_key)),
            r#type: Set(Some(value_or_null(self.r#type).unwrap_or_else(|| "custom".into()))),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(address_type)
    }
}

/// `DestroyAddressType`.
pub struct DestroyAddressType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub address_type_id: Id,
}

impl DestroyAddressType {
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

        let address_type = find(db, self.account_id, self.address_type_id).await?;

        address_types::Entity::delete_by_id(address_type.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateAddressType`.
pub struct UpdateAddressType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub address_type_id: Id,
    pub name: String,
}

impl UpdateAddressType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<address_types::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
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

        let address_type = find(db, self.account_id, self.address_type_id).await?;

        let mut address_type: address_types::ActiveModel = address_type.into();
        address_type.name = Set(Some(self.name));
        Ok(address_type.update(db).await?)
    }
}

/// `$this->account()->addressTypes()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, account_id: Uuid, address_type_id: Id) -> ApiResult<address_types::Model> {
    address_types::Entity::find_by_id(address_type_id)
        .filter(address_types::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
