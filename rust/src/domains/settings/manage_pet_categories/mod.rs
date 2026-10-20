//! `app/Domains/Settings/ManagePetCategories/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::pet_categories;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreatePetCategory`.
pub struct CreatePetCategory {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
}

impl CreatePetCategory {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<pet_categories::Model> {
        let mut v = Validator::new();
        v.max_len("name", self.name.as_deref(), 255);
        v.max_len("name_translation_key", self.name_translation_key.as_deref(), 255);
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

        let category = pet_categories::ActiveModel {
            account_id: Set(self.account_id),
            name: Set(self.name),
            name_translation_key: Set(self.name_translation_key),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(category)
    }
}

/// `DestroyPetCategory`.
pub struct DestroyPetCategory {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub pet_category_id: Id,
}

impl DestroyPetCategory {
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

        let pet_category = find(db, self.account_id, self.pet_category_id).await?;

        pet_categories::Entity::delete_by_id(pet_category.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdatePetCategory`.
pub struct UpdatePetCategory {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub pet_category_id: Id,
    pub name: String,
}

impl UpdatePetCategory {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<pet_categories::Model> {
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

        let pet_category = find(db, self.account_id, self.pet_category_id).await?;

        let mut pet_category: pet_categories::ActiveModel = pet_category.into();
        pet_category.name = Set(Some(self.name));
        Ok(pet_category.update(db).await?)
    }
}

/// `$this->account()->petCategories()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, account_id: Uuid, pet_category_id: Id) -> ApiResult<pet_categories::Model> {
    pet_categories::Entity::find_by_id(pet_category_id)
        .filter(pet_categories::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
