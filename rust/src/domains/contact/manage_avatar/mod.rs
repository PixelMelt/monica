//! `app/Domains/Contact/ManageAvatar/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::{contact_feed_items, contacts, files, users};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `DestroyAvatar`.
pub struct DestroyAvatar {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
}

impl DestroyAvatar {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
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

        delete_current_avatar(db, checked.contact()).await?;
        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.file_id = Set(None);

        contact.last_updated_at = Set(Some(crate::db::now()));
        let contact = contact.update(db).await?;

        create_feed_item(db, &checked.author, &contact).await?;

        Ok(contact)
    }
}

/// `UpdatePhotoAsAvatar`.
pub struct UpdatePhotoAsAvatar {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub file_id: Option<Id>,
}

impl UpdatePhotoAsAvatar {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contacts::Model> {
        let mut v = Validator::new();
        if let Some(id) = self.file_id
            && files::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("file_id", "exists", &[]);
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

        let file = match self.file_id {
            Some(id) => {
                files::Entity::find_by_id(id)
                    .filter(files::Column::VaultId.eq(checked.vault().id))
                    .filter(files::Column::Type.eq("avatar"))
                    .one(db)
                    .await?
            }
            None => None,
        }
        .ok_or_else(ApiError::not_found)?;

        delete_current_avatar(db, checked.contact()).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.file_id = Set(Some(file.id));
        let contact = contact.update(db).await?;

        let mut file: files::ActiveModel = file.into();
        file.fileable_type = Set(Some("App\\Models\\Contact".into()));
        file.ufileable_id = Set(Some(contact.id));
        file.update(db).await?;

        let mut contact: contacts::ActiveModel = contact.into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        let contact = contact.update(db).await?;

        create_feed_item(db, &checked.author, &contact).await?;

        Ok(contact)
    }
}

/// `deleteCurrentAvatar`.
async fn delete_current_avatar<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<()> {
    if let Some(file_id) = contact.file_id {
        files::Entity::delete_by_id(file_id).exec(db).await?;
    }
    Ok(())
}

/// `createFeedItem`.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    contact: &contacts::Model,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author.id)),
        contact_id: Set(contact.id),
        action: Set("changed_avatar".into()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}
