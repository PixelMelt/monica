//! `app/Domains/Settings/ManagePronouns/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::pronouns;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreatePronoun`.
pub struct CreatePronoun {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
}

impl CreatePronoun {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<pronouns::Model> {
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

        let pronoun = pronouns::ActiveModel {
            account_id: Set(self.account_id),
            name: Set(self.name),
            name_translation_key: Set(self.name_translation_key),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(pronoun)
    }
}

/// `DestroyPronoun`.
pub struct DestroyPronoun {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub pronoun_id: Id,
}

impl DestroyPronoun {
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

        let pronoun = find(db, self.account_id, self.pronoun_id).await?;

        pronouns::Entity::delete_by_id(pronoun.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdatePronoun`.
pub struct UpdatePronoun {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub pronoun_id: Id,
    pub name: String,
}

impl UpdatePronoun {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<pronouns::Model> {
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

        let pronoun = find(db, self.account_id, self.pronoun_id).await?;

        let mut pronoun: pronouns::ActiveModel = pronoun.into();
        pronoun.name = Set(Some(self.name));
        Ok(pronoun.update(db).await?)
    }
}

/// `$this->account()->pronouns()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, account_id: Uuid, pronoun_id: Id) -> ApiResult<pronouns::Model> {
    pronouns::Entity::find_by_id(pronoun_id)
        .filter(pronouns::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
