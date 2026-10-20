//! `CreateTag`, `DestroyTag` and `UpdateTag`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::tags;
use crate::error::{ApiError, ApiResult};
use crate::text::slug;
use crate::validate::Validator;

/// `CreateTag`.
pub struct CreateTag {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub name: String,
}

impl CreateTag {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<tags::Model> {
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
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let tag = tags::ActiveModel {
            vault_id: Set(self.vault_id),
            name: Set(self.name.clone()),
            slug: Set(slug(&self.name)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(tag)
    }
}

/// `DestroyTag`.
pub struct DestroyTag {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub tag_id: Id,
}

impl DestroyTag {
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

        let tag = find(db, checked.vault().id, self.tag_id).await?;

        tags::Entity::delete_by_id(tag.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateTag`.
pub struct UpdateTag {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub tag_id: Id,
    pub name: String,
}

impl UpdateTag {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<tags::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
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

        let tag = find(db, checked.vault().id, self.tag_id).await?;

        let mut tag: tags::ActiveModel = tag.into();
        tag.name = Set(self.name.clone());
        tag.slug = Set(slug(&self.name));
        Ok(tag.update(db).await?)
    }
}

/// `$this->vault->tags()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, vault_id: Uuid, tag_id: Id) -> ApiResult<tags::Model> {
    tags::Entity::find_by_id(tag_id)
        .filter(tags::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
