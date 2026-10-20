//! `app/Domains/Vault/ManageVaultImportantDateTypes/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::contact_important_date_types;
use crate::error::{ApiError, ApiResult, Code};
use crate::validate::Validator;

/// `CreateContactImportantDateType`.
pub struct CreateContactImportantDateType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    pub internal_type: Option<String>,
    pub can_be_deleted: Option<bool>,
}

impl CreateContactImportantDateType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_important_date_types::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.max_len("internal_type", self.internal_type.as_deref(), 255);
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let r#type = contact_important_date_types::ActiveModel {
            vault_id: Set(self.vault_id),
            label: Set(self.label),
            internal_type: Set(value_or_null(self.internal_type)),
            can_be_deleted: Set(self.can_be_deleted.unwrap_or(true)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(r#type)
    }
}

/// `DestroyContactImportantDateType`.
pub struct DestroyContactImportantDateType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub contact_important_date_type_id: Id,
}

impl DestroyContactImportantDateType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        exists(db, &mut v, self.contact_important_date_type_id).await?;
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let r#type = find(db, checked.vault().id, self.contact_important_date_type_id).await?;

        if !r#type.can_be_deleted {
            return Err(ApiError::new(Code::Conflict));
        }

        contact_important_date_types::Entity::delete_by_id(r#type.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateContactImportantDateType`.
pub struct UpdateContactImportantDateType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub contact_important_date_type_id: Id,
    pub label: String,
    pub internal_type: Option<String>,
    pub can_be_deleted: Option<bool>,
}

impl UpdateContactImportantDateType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_important_date_types::Model> {
        let mut v = Validator::new();
        exists(db, &mut v, self.contact_important_date_type_id).await?;
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.max_len("internal_type", self.internal_type.as_deref(), 255);
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let r#type = find(db, checked.vault().id, self.contact_important_date_type_id).await?;

        let mut r#type: contact_important_date_types::ActiveModel = r#type.into();
        r#type.label = Set(self.label);
        r#type.can_be_deleted = Set(self.can_be_deleted.unwrap_or(true));
        r#type.internal_type = Set(value_or_null(self.internal_type));
        Ok(r#type.update(db).await?)
    }
}

/// `exists:contact_important_date_types,id`.
async fn exists<C: ConnectionTrait>(db: &C, v: &mut Validator, id: Id) -> ApiResult<()> {
    if contact_important_date_types::Entity::find_by_id(id).one(db).await?.is_none() {
        v.fail("contact_important_date_type_id", "exists", &[]);
    }
    Ok(())
}

/// `$this->vault->contactImportantDateTypes()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, vault_id: Uuid, id: Id) -> ApiResult<contact_important_date_types::Model> {
    contact_important_date_types::Entity::find_by_id(id)
        .filter(contact_important_date_types::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `valueOrNull`: `null` when PHP's `empty()` holds (`""`, `"0"`).
fn value_or_null(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty() && s != "0")
}
