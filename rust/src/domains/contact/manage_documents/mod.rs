//! `app/Domains/Contact/ManageDocuments/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::{contacts, files};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `DestroyFile`.
pub struct DestroyFile {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub file_id: Id,
}

impl DestroyFile {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
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

        let file = files::Entity::find_by_id(self.file_id)
            .filter(files::Column::VaultId.eq(checked.vault().id))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        files::Entity::delete_by_id(file.id).exec(db).await?;

        update_last_edited_date(db, &file).await
    }
}

async fn update_last_edited_date<C: ConnectionTrait>(db: &C, file: &files::Model) -> ApiResult<()> {
    if file.fileable_type.as_deref() == Some("App\\Models\\Contact") {
        let contact = contacts::Entity::live()
            .filter(contacts::Column::Id.eq(file.ufileable_id))
            .one(db)
            .await?
            .ok_or_else(|| ApiError::internal("the file's contact is missing"))?;
        let mut contact: contacts::ActiveModel = contact.into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;
    }
    Ok(())
}

/// `UploadFile`.
pub struct UploadFile {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub uuid: String,
    pub name: String,
    pub mime_type: String,
    pub size: i32,
    pub r#type: String,
}

impl UploadFile {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<files::Model> {
        let mut v = Validator::new();
        for (field, value) in
            [("uuid", &self.uuid), ("name", &self.name), ("mime_type", &self.mime_type), ("type", &self.r#type)]
        {
            if value.trim().is_empty() {
                v.fail(field, "required", &[]);
            }
        }
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

        let file = files::ActiveModel {
            vault_id: Set(self.vault_id),
            uuid: Set(self.uuid),
            name: Set(self.name),
            original_url: Set(None),
            cdn_url: Set(None),
            mime_type: Set(self.mime_type),
            size: Set(self.size),
            r#type: Set(self.r#type),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(file)
    }
}
