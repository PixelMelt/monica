//! `app/Domains/Settings/ManageGenders/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::genders;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateGender`.
pub struct CreateGender {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
}

impl CreateGender {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<genders::Model> {
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

        let gender = genders::ActiveModel {
            account_id: Set(self.account_id),
            name: Set(self.name),
            name_translation_key: Set(self.name_translation_key),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(gender)
    }
}

/// `UpdateGender`.
pub struct UpdateGender {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub gender_id: Id,
    pub name: String,
}

impl UpdateGender {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<genders::Model> {
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

        let gender = find(db, self.account_id, self.gender_id).await?;

        let mut gender: genders::ActiveModel = gender.into();
        gender.name = Set(Some(self.name));
        Ok(gender.update(db).await?)
    }
}

/// `DestroyGender`.
pub struct DestroyGender {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub gender_id: Id,
}

impl DestroyGender {
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

        let gender = find(db, self.account_id, self.gender_id).await?;

        genders::Entity::delete_by_id(gender.id).exec(db).await?;
        Ok(())
    }
}

/// `$this->account()->genders()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, account_id: Uuid, gender_id: Id) -> ApiResult<genders::Model> {
    genders::Entity::find_by_id(gender_id)
        .filter(genders::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
