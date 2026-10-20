//! `CreateLabel`, `DestroyLabel` and `UpdateLabel`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use super::slug;
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::labels;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateLabel`.
pub struct CreateLabel {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub bg_color: Option<String>,
    pub text_color: Option<String>,
}

impl CreateLabel {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<labels::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        v.max_len("description", self.description.as_deref(), 65535);
        v.max_len("bg_color", self.bg_color.as_deref(), 255);
        v.max_len("text_color", self.text_color.as_deref(), 255);
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

        let label = labels::ActiveModel {
            vault_id: Set(self.vault_id),
            name: Set(self.name.clone()),
            description: Set(value_or_null(self.description)),
            slug: Set(slug(&self.name)),
            bg_color: Set(value_or_null(self.bg_color).unwrap_or_else(|| "bg-neutral-200".into())),
            text_color: Set(value_or_null(self.text_color).unwrap_or_else(|| "text-neutral-800".into())),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(label)
    }
}

/// `DestroyLabel`.
pub struct DestroyLabel {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub label_id: Id,
}

impl DestroyLabel {
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

        let label = find(db, checked.vault().id, self.label_id).await?;

        labels::Entity::delete_by_id(label.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateLabel`.
pub struct UpdateLabel {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub label_id: Id,
    pub name: String,
    pub description: Option<String>,
    pub bg_color: String,
    pub text_color: String,
}

impl UpdateLabel {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<labels::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        v.max_len("description", self.description.as_deref(), 65535);
        v.max_len("bg_color", Some(&self.bg_color), 255);
        v.max_len("text_color", Some(&self.text_color), 255);
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

        let label = find(db, checked.vault().id, self.label_id).await?;

        let mut label: labels::ActiveModel = label.into();
        label.name = Set(self.name.clone());
        label.bg_color = Set(self.bg_color);
        label.text_color = Set(self.text_color);
        label.description = Set(value_or_null(self.description));
        label.slug = Set(slug(&self.name));
        Ok(label.update(db).await?)
    }
}

/// `$this->vault->labels()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, vault_id: Uuid, label_id: Id) -> ApiResult<labels::Model> {
    labels::Entity::find_by_id(label_id)
        .filter(labels::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
