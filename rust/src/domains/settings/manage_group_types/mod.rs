//! `app/Domains/Settings/ManageGroupTypes/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::{group_type_roles, group_types};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateGroupType`.
pub struct CreateGroupType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
}

impl CreateGroupType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<group_types::Model> {
        self.validate(db).await?;
        self.create(db).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        v.max_len("label", self.label.as_deref(), 255);
        v.max_len("label_translation_key", self.label_translation_key.as_deref(), 255);
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
        Ok(())
    }

    async fn create<C: ConnectionTrait>(self, db: &C) -> ApiResult<group_types::Model> {
        let new_position = group_types::Entity::find()
            .select_only()
            .expr(Expr::col(group_types::Column::Position).max())
            .filter(group_types::Column::AccountId.eq(self.account_id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .map_or(1, |p| p + 1);

        let group_type = group_types::ActiveModel {
            account_id: Set(self.account_id),
            label: Set(self.label),
            label_translation_key: Set(self.label_translation_key),
            position: Set(new_position),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(group_type)
    }
}

/// `CreateGroupTypeRole`.
pub struct CreateGroupTypeRole {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub group_type_id: Id,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
}

impl CreateGroupTypeRole {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<group_type_roles::Model> {
        self.validate(db).await?;
        self.create(db).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        v.max_len("label", self.label.as_deref(), 255);
        v.max_len("label_translation_key", self.label_translation_key.as_deref(), 255);
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
        Ok(())
    }

    async fn create<C: ConnectionTrait>(self, db: &C) -> ApiResult<group_type_roles::Model> {
        let group_type = find_group_type(db, self.account_id, self.group_type_id).await?;

        let new_position = group_type_roles::Entity::find()
            .select_only()
            .expr(Expr::col(group_type_roles::Column::Position).max())
            .filter(group_type_roles::Column::GroupTypeId.eq(group_type.id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .map_or(1, |p| p + 1);

        let group_type_role = group_type_roles::ActiveModel {
            group_type_id: Set(self.group_type_id),
            label: Set(self.label),
            label_translation_key: Set(self.label_translation_key),
            position: Set(Some(new_position)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(group_type_role)
    }
}

/// `DestroyGroupType`.
pub struct DestroyGroupType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub group_type_id: Id,
}

impl DestroyGroupType {
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

        let group_type = find_group_type(db, self.account_id, self.group_type_id).await?;

        group_types::Entity::delete_by_id(group_type.id).exec(db).await?;

        self.reposition_everything(db, &group_type).await
    }

    async fn reposition_everything<C: ConnectionTrait>(
        &self,
        db: &C,
        group_type: &group_types::Model,
    ) -> ApiResult<()> {
        group_types::Entity::update_many()
            .col_expr(group_types::Column::Position, Expr::col(group_types::Column::Position).sub(1))
            .col_expr(group_types::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(group_types::Column::AccountId.eq(self.account_id))
            .filter(group_types::Column::Position.gt(group_type.position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `DestroyGroupTypeRole`.
pub struct DestroyGroupTypeRole {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub group_type_id: Id,
    pub group_type_role_id: Id,
}

impl DestroyGroupTypeRole {
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

        let group_type = find_group_type(db, self.account_id, self.group_type_id).await?;

        let group_type_role = find_group_type_role(db, group_type.id, self.group_type_role_id).await?;

        group_type_roles::Entity::delete_by_id(group_type_role.id).exec(db).await?;

        self.reposition_everything(db, &group_type, &group_type_role).await
    }

    async fn reposition_everything<C: ConnectionTrait>(
        &self,
        db: &C,
        group_type: &group_types::Model,
        group_type_role: &group_type_roles::Model,
    ) -> ApiResult<()> {
        let position =
            group_type_role.position.ok_or_else(|| ApiError::internal("group_type_roles.position is null"))?;
        group_type_roles::Entity::update_many()
            .col_expr(group_type_roles::Column::Position, Expr::col(group_type_roles::Column::Position).sub(1))
            .col_expr(group_type_roles::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(group_type_roles::Column::GroupTypeId.eq(group_type.id))
            .filter(group_type_roles::Column::Position.gt(position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `UpdateGroupType`.
pub struct UpdateGroupType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub group_type_id: Id,
    pub label: String,
}

impl UpdateGroupType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<group_types::Model> {
        let group_type = self.validate(db).await?;
        self.update(db, group_type).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<group_types::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
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
        find_group_type(db, self.account_id, self.group_type_id).await
    }

    async fn update<C: ConnectionTrait>(self, db: &C, group_type: group_types::Model) -> ApiResult<group_types::Model> {
        let mut group_type: group_types::ActiveModel = group_type.into();
        group_type.label = Set(Some(self.label));
        Ok(group_type.update(db).await?)
    }
}

/// `UpdateGroupTypePosition`.
pub struct UpdateGroupTypePosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub group_type_id: Id,
    pub new_position: i32,
}

impl UpdateGroupTypePosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<group_types::Model> {
        let (group_type, past_position) = self.validate(db).await?;
        self.update_position(db, group_type, past_position).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(group_types::Model, i32)> {
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

        let group_type = find_group_type(db, self.account_id, self.group_type_id).await?;

        let past_position = group_type.position;
        Ok((group_type, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        group_type: group_types::Model,
        past_position: i32,
    ) -> ApiResult<group_types::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, past_position).await?;
        } else {
            self.update_descending_position(db, past_position).await?;
        }

        let mut group_type: group_types::ActiveModel = group_type.into();
        group_type.position = Set(self.new_position);
        Ok(group_type.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        group_types::Entity::update_many()
            .col_expr(group_types::Column::Position, Expr::col(group_types::Column::Position).sub(1))
            .col_expr(group_types::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(group_types::Column::AccountId.eq(self.account_id))
            .filter(group_types::Column::Position.gt(past_position))
            .filter(group_types::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        group_types::Entity::update_many()
            .col_expr(group_types::Column::Position, Expr::col(group_types::Column::Position).add(1))
            .col_expr(group_types::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(group_types::Column::AccountId.eq(self.account_id))
            .filter(group_types::Column::Position.gte(self.new_position))
            .filter(group_types::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `UpdateGroupTypeRole`.
pub struct UpdateGroupTypeRole {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub group_type_id: Id,
    pub group_type_role_id: Id,
    pub label: String,
}

impl UpdateGroupTypeRole {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<group_type_roles::Model> {
        let group_type_role = self.validate(db).await?;
        self.update(db, group_type_role).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<group_type_roles::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
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

        let group_type = find_group_type(db, self.account_id, self.group_type_id).await?;

        find_group_type_role(db, group_type.id, self.group_type_role_id).await
    }

    async fn update<C: ConnectionTrait>(
        self,
        db: &C,
        group_type_role: group_type_roles::Model,
    ) -> ApiResult<group_type_roles::Model> {
        let mut group_type_role: group_type_roles::ActiveModel = group_type_role.into();
        group_type_role.label = Set(Some(self.label));
        Ok(group_type_role.update(db).await?)
    }
}

/// `UpdateGroupTypeRolePosition`.
pub struct UpdateGroupTypeRolePosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub group_type_id: Id,
    pub group_type_role_id: Id,
    pub new_position: i32,
}

impl UpdateGroupTypeRolePosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<group_type_roles::Model> {
        let (group_type, group_type_role, past_position) = self.validate(db).await?;
        self.update_position(db, &group_type, group_type_role, past_position).await
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(group_types::Model, group_type_roles::Model, i32)> {
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

        let group_type = find_group_type(db, self.account_id, self.group_type_id).await?;

        let group_type_role = find_group_type_role(db, group_type.id, self.group_type_role_id).await?;

        let past_position =
            group_type_role.position.ok_or_else(|| ApiError::internal("group_type_roles.position is null"))?;
        Ok((group_type, group_type_role, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        group_type: &group_types::Model,
        group_type_role: group_type_roles::Model,
        past_position: i32,
    ) -> ApiResult<group_type_roles::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, group_type, past_position).await?;
        } else {
            self.update_descending_position(db, group_type, past_position).await?;
        }

        let mut group_type_role: group_type_roles::ActiveModel = group_type_role.into();
        group_type_role.position = Set(Some(self.new_position));
        Ok(group_type_role.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        group_type: &group_types::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        group_type_roles::Entity::update_many()
            .col_expr(group_type_roles::Column::Position, Expr::col(group_type_roles::Column::Position).sub(1))
            .col_expr(group_type_roles::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(group_type_roles::Column::GroupTypeId.eq(group_type.id))
            .filter(group_type_roles::Column::Position.gt(past_position))
            .filter(group_type_roles::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        group_type: &group_types::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        group_type_roles::Entity::update_many()
            .col_expr(group_type_roles::Column::Position, Expr::col(group_type_roles::Column::Position).add(1))
            .col_expr(group_type_roles::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(group_type_roles::Column::GroupTypeId.eq(group_type.id))
            .filter(group_type_roles::Column::Position.gte(self.new_position))
            .filter(group_type_roles::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `$this->account()->groupTypes()->findOrFail($id)`.
async fn find_group_type<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    group_type_id: Id,
) -> ApiResult<group_types::Model> {
    group_types::Entity::find_by_id(group_type_id)
        .filter(group_types::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$groupType->groupTypeRoles()->findOrFail($id)`.
async fn find_group_type_role<C: ConnectionTrait>(
    db: &C,
    group_type_id: Id,
    group_type_role_id: Id,
) -> ApiResult<group_type_roles::Model> {
    group_type_roles::Entity::find_by_id(group_type_role_id)
        .filter(group_type_roles::Column::GroupTypeId.eq(group_type_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
