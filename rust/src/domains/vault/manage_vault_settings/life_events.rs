//! `CreateLifeEventCategory`, `CreateLifeEventType`, `DestroyLifeEventCategory`,
//! `DestroyLifeEventType`, `UpdateLifeEventCategory`,
//! `UpdateLifeEventCategoryPosition`, `UpdateLifeEventType` and
//! `UpdateLifeEventTypePosition`.

use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, QueryFilter, QuerySelect,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::{life_event_categories, life_event_types};
use crate::error::{ApiError, ApiResult, Code};
use crate::validate::Validator;

/// `CreateLifeEventCategory`.
pub struct CreateLifeEventCategory {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub label: String,
    pub can_be_deleted: bool,
}

impl CreateLifeEventCategory {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_event_categories::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
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

        // determine the new position of the life event type
        let new_position = life_event_categories::Entity::find()
            .select_only()
            .expr(Expr::col(life_event_categories::Column::Position).max())
            .filter(life_event_categories::Column::VaultId.eq(checked.vault().id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .unwrap_or(0)
            + 1;

        Ok(life_event_categories::ActiveModel {
            vault_id: Set(self.vault_id),
            label: Set(Some(self.label)),
            can_be_deleted: Set(self.can_be_deleted),
            position: Set(Some(new_position)),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `CreateLifeEventType`.
pub struct CreateLifeEventType {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub life_event_category_id: Id,
    pub label: String,
    pub can_be_deleted: bool,
}

impl CreateLifeEventType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_event_types::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
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

        let category = find_category(db, checked.vault().id, self.life_event_category_id).await?;

        // determine the new position of the life event type
        let new_position = life_event_types::Entity::find()
            .select_only()
            .expr(Expr::col(life_event_types::Column::Position).max())
            .filter(life_event_types::Column::LifeEventCategoryId.eq(category.id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .unwrap_or(0)
            + 1;

        let r#type = life_event_types::ActiveModel {
            life_event_category_id: Set(category.id),
            label: Set(Some(self.label)),
            can_be_deleted: Set(self.can_be_deleted),
            position: Set(Some(new_position)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(r#type)
    }
}

/// `DestroyLifeEventCategory`.
pub struct DestroyLifeEventCategory {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub life_event_category_id: Id,
}

impl DestroyLifeEventCategory {
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

        let category = find_category(db, checked.vault().id, self.life_event_category_id).await?;

        if !category.can_be_deleted {
            return Err(ApiError::new(Code::Conflict));
        }

        life_event_categories::Entity::delete_by_id(category.id).exec(db).await?;
        Ok(())
    }
}

/// `DestroyLifeEventType`.
pub struct DestroyLifeEventType {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub life_event_category_id: Id,
    pub life_event_type_id: Id,
}

impl DestroyLifeEventType {
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

        let category = find_category(db, checked.vault().id, self.life_event_category_id).await?;

        let r#type = find_type(db, category.id, self.life_event_type_id).await?;

        if !r#type.can_be_deleted {
            return Err(ApiError::new(Code::Conflict));
        }

        life_event_types::Entity::delete_by_id(r#type.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateLifeEventCategory`.
pub struct UpdateLifeEventCategory {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub life_event_category_id: Id,
    pub label: String,
    pub can_be_deleted: bool,
}

impl UpdateLifeEventCategory {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_event_categories::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
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

        let category = find_category(db, checked.vault().id, self.life_event_category_id).await?;

        let mut category: life_event_categories::ActiveModel = category.into();
        category.label = Set(Some(self.label));
        category.can_be_deleted = Set(self.can_be_deleted);
        Ok(category.update(db).await?)
    }
}

/// `UpdateLifeEventCategoryPosition`.
pub struct UpdateLifeEventCategoryPosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    /// Not in `rules()`, but read by `vault_must_belong_to_account`.
    pub vault_id: Uuid,
    pub life_event_category_id: Id,
    pub new_position: i32,
}

impl UpdateLifeEventCategoryPosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_event_categories::Model> {
        let (checked, life_event_category, past_position) = self.validate(db).await?;
        let life_event_category = self.update_position(db, &checked, life_event_category, past_position).await?;

        Ok(life_event_category)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(Checked, life_event_categories::Model, i32)> {
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

        let life_event_category = find_category(db, checked.vault().id, self.life_event_category_id).await?;

        let past_position =
            life_event_category.position.ok_or_else(|| ApiError::internal("life_event_categories.position is null"))?;

        Ok((checked, life_event_category, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        checked: &Checked,
        life_event_category: life_event_categories::Model,
        past_position: i32,
    ) -> ApiResult<life_event_categories::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, checked, past_position).await?;
        } else {
            self.update_descending_position(db, checked, past_position).await?;
        }

        let mut life_event_category: life_event_categories::ActiveModel = life_event_category.into();
        life_event_category.position = Set(Some(self.new_position));
        Ok(life_event_category.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        checked: &Checked,
        past_position: i32,
    ) -> ApiResult<()> {
        life_event_categories::Entity::update_many()
            .col_expr(
                life_event_categories::Column::Position,
                Expr::col(life_event_categories::Column::Position).sub(1),
            )
            .col_expr(life_event_categories::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(life_event_categories::Column::VaultId.eq(checked.vault().id))
            .filter(life_event_categories::Column::Position.gt(past_position))
            .filter(life_event_categories::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        checked: &Checked,
        past_position: i32,
    ) -> ApiResult<()> {
        life_event_categories::Entity::update_many()
            .col_expr(
                life_event_categories::Column::Position,
                Expr::col(life_event_categories::Column::Position).add(1),
            )
            .col_expr(life_event_categories::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(life_event_categories::Column::VaultId.eq(checked.vault().id))
            .filter(life_event_categories::Column::Position.gte(self.new_position))
            .filter(life_event_categories::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `UpdateLifeEventType`.
pub struct UpdateLifeEventType {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub life_event_category_id: Id,
    pub life_event_type_id: Id,
    pub label: String,
    pub can_be_deleted: bool,
}

impl UpdateLifeEventType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_event_types::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
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

        let category = find_category(db, checked.vault().id, self.life_event_category_id).await?;

        let r#type = find_type(db, category.id, self.life_event_type_id).await?;

        let mut r#type: life_event_types::ActiveModel = r#type.into();
        r#type.label = Set(Some(self.label));
        r#type.can_be_deleted = Set(self.can_be_deleted);
        Ok(r#type.update(db).await?)
    }
}

/// `UpdateLifeEventTypePosition`.
pub struct UpdateLifeEventTypePosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    /// Not in `rules()`, but read by `vault_must_belong_to_account`.
    pub vault_id: Uuid,
    pub life_event_category_id: Id,
    pub life_event_type_id: Id,
    pub new_position: i32,
}

impl UpdateLifeEventTypePosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_event_types::Model> {
        let (life_event_category, life_event_type, past_position) = self.validate(db).await?;
        let life_event_type = self.update_position(db, &life_event_category, life_event_type, past_position).await?;

        Ok(life_event_type)
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(life_event_categories::Model, life_event_types::Model, i32)> {
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

        let life_event_category = find_category(db, checked.vault().id, self.life_event_category_id).await?;

        let life_event_type = find_type(db, life_event_category.id, self.life_event_type_id).await?;

        let past_position =
            life_event_type.position.ok_or_else(|| ApiError::internal("life_event_types.position is null"))?;

        Ok((life_event_category, life_event_type, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        life_event_category: &life_event_categories::Model,
        life_event_type: life_event_types::Model,
        past_position: i32,
    ) -> ApiResult<life_event_types::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, life_event_category, past_position).await?;
        } else {
            self.update_descending_position(db, life_event_category, past_position).await?;
        }

        let mut life_event_type: life_event_types::ActiveModel = life_event_type.into();
        life_event_type.position = Set(Some(self.new_position));
        Ok(life_event_type.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        life_event_category: &life_event_categories::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        life_event_types::Entity::update_many()
            .col_expr(life_event_types::Column::Position, Expr::col(life_event_types::Column::Position).sub(1))
            .col_expr(life_event_types::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(life_event_types::Column::LifeEventCategoryId.eq(life_event_category.id))
            .filter(life_event_types::Column::Position.gt(past_position))
            .filter(life_event_types::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        life_event_category: &life_event_categories::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        life_event_types::Entity::update_many()
            .col_expr(life_event_types::Column::Position, Expr::col(life_event_types::Column::Position).add(1))
            .col_expr(life_event_types::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(life_event_types::Column::LifeEventCategoryId.eq(life_event_category.id))
            .filter(life_event_types::Column::Position.gte(self.new_position))
            .filter(life_event_types::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `$this->vault->lifeEventCategories()->findOrFail($id)`.
async fn find_category<C: ConnectionTrait>(
    db: &C,
    vault_id: Uuid,
    life_event_category_id: Id,
) -> ApiResult<life_event_categories::Model> {
    life_event_categories::Entity::find_by_id(life_event_category_id)
        .filter(life_event_categories::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$category->lifeEventTypes()->findOrFail($id)`.
async fn find_type<C: ConnectionTrait>(
    db: &C,
    life_event_category_id: Id,
    life_event_type_id: Id,
) -> ApiResult<life_event_types::Model> {
    life_event_types::Entity::find_by_id(life_event_type_id)
        .filter(life_event_types::Column::LifeEventCategoryId.eq(life_event_category_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
