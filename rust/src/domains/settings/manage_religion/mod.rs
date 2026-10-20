//! `app/Domains/Settings/ManageReligion/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::religions;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateReligion`.
pub struct CreateReligion {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub name: String,
}

impl CreateReligion {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<religions::Model> {
        self.validate(db).await?;
        self.create(db).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<()> {
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
        Ok(())
    }

    async fn create<C: ConnectionTrait>(self, db: &C) -> ApiResult<religions::Model> {
        // determine the new position of the religion
        let new_position = religions::Entity::find()
            .select_only()
            .expr(Expr::col(religions::Column::Position).max())
            .filter(religions::Column::AccountId.eq(self.account_id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .map_or(1, |p| p + 1);

        let religion = religions::ActiveModel {
            account_id: Set(self.account_id),
            name: Set(Some(self.name)),
            position: Set(Some(new_position)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(religion)
    }
}

/// `DestroyReligion`.
pub struct DestroyReligion {
    pub account_id: Uuid,
    pub religion_id: Id,
    pub author_id: Uuid,
}

impl DestroyReligion {
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

        let religion = find(db, self.account_id, self.religion_id).await?;

        religions::Entity::delete_by_id(religion.id).exec(db).await?;

        self.reposition_everything(db, &religion).await
    }

    async fn reposition_everything<C: ConnectionTrait>(&self, db: &C, religion: &religions::Model) -> ApiResult<()> {
        religions::Entity::update_many()
            .col_expr(religions::Column::Position, Expr::col(religions::Column::Position).sub(1))
            .col_expr(religions::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(religions::Column::AccountId.eq(self.account_id))
            .filter(religions::Column::Position.gt(religion.position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `UpdateReligion`.
pub struct UpdateReligion {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub religion_id: Id,
    pub name: String,
}

impl UpdateReligion {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<religions::Model> {
        let religion = self.validate(db).await?;
        self.update(db, religion).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<religions::Model> {
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
        find(db, self.account_id, self.religion_id).await
    }

    async fn update<C: ConnectionTrait>(self, db: &C, religion: religions::Model) -> ApiResult<religions::Model> {
        let mut religion: religions::ActiveModel = religion.into();
        religion.name = Set(Some(self.name));
        Ok(religion.update(db).await?)
    }
}

/// `UpdateReligionPosition`.
pub struct UpdateReligionPosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub religion_id: Id,
    pub new_position: i32,
}

impl UpdateReligionPosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<religions::Model> {
        let (religion, past_position) = self.validate(db).await?;
        self.update_position(db, religion, past_position).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(religions::Model, i32)> {
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

        let religion = find(db, self.account_id, self.religion_id).await?;

        let past_position = religion.position.ok_or_else(|| ApiError::internal("religions.position is null"))?;
        Ok((religion, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        religion: religions::Model,
        past_position: i32,
    ) -> ApiResult<religions::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, past_position).await?;
        } else {
            self.update_descending_position(db, past_position).await?;
        }

        let mut religion: religions::ActiveModel = religion.into();
        religion.position = Set(Some(self.new_position));
        Ok(religion.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        religions::Entity::update_many()
            .col_expr(religions::Column::Position, Expr::col(religions::Column::Position).sub(1))
            .col_expr(religions::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(religions::Column::AccountId.eq(self.account_id))
            .filter(religions::Column::Position.gt(past_position))
            .filter(religions::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        religions::Entity::update_many()
            .col_expr(religions::Column::Position, Expr::col(religions::Column::Position).add(1))
            .col_expr(religions::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(religions::Column::AccountId.eq(self.account_id))
            .filter(religions::Column::Position.gte(self.new_position))
            .filter(religions::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `$this->account()->religions()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, account_id: Uuid, religion_id: Id) -> ApiResult<religions::Model> {
    religions::Entity::find_by_id(religion_id)
        .filter(religions::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
