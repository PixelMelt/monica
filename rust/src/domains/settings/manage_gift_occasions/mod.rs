//! `app/Domains/Settings/ManageGiftOccasions/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::gift_occasions;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateGiftOccasion`.
pub struct CreateGiftOccasion {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub label: String,
}

impl CreateGiftOccasion {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<gift_occasions::Model> {
        self.validate(db).await?;
        self.create(db).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<()> {
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
        Ok(())
    }

    async fn create<C: ConnectionTrait>(self, db: &C) -> ApiResult<gift_occasions::Model> {
        // determine the new position of the template page
        let new_position = gift_occasions::Entity::find()
            .select_only()
            .expr(Expr::col(gift_occasions::Column::Position).max())
            .filter(gift_occasions::Column::AccountId.eq(self.account_id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .map_or(1, |p| p + 1);

        let gift_occasion = gift_occasions::ActiveModel {
            account_id: Set(self.account_id),
            label: Set(Some(self.label)),
            position: Set(Some(new_position)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(gift_occasion)
    }
}

/// `DestroyGiftOccasion`.
pub struct DestroyGiftOccasion {
    pub account_id: Uuid,
    pub gift_occasion_id: Id,
    pub author_id: Uuid,
}

impl DestroyGiftOccasion {
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

        let gift_occasion = find(db, self.account_id, self.gift_occasion_id).await?;

        gift_occasions::Entity::delete_by_id(gift_occasion.id).exec(db).await?;

        self.reposition_everything(db, &gift_occasion).await
    }

    async fn reposition_everything<C: ConnectionTrait>(
        &self,
        db: &C,
        gift_occasion: &gift_occasions::Model,
    ) -> ApiResult<()> {
        gift_occasions::Entity::update_many()
            .col_expr(gift_occasions::Column::Position, Expr::col(gift_occasions::Column::Position).sub(1))
            .col_expr(gift_occasions::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(gift_occasions::Column::AccountId.eq(self.account_id))
            .filter(gift_occasions::Column::Position.gt(gift_occasion.position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `UpdateGiftOccasion`.
pub struct UpdateGiftOccasion {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub gift_occasion_id: Id,
    pub label: String,
}

impl UpdateGiftOccasion {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<gift_occasions::Model> {
        let gift_occasion = self.validate(db).await?;
        self.update(db, gift_occasion).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<gift_occasions::Model> {
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
        find(db, self.account_id, self.gift_occasion_id).await
    }

    async fn update<C: ConnectionTrait>(
        self,
        db: &C,
        gift_occasion: gift_occasions::Model,
    ) -> ApiResult<gift_occasions::Model> {
        let mut gift_occasion: gift_occasions::ActiveModel = gift_occasion.into();
        gift_occasion.label = Set(Some(self.label));
        Ok(gift_occasion.update(db).await?)
    }
}

/// `UpdateGiftOccasionPosition`.
pub struct UpdateGiftOccasionPosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub gift_occasion_id: Id,
    pub new_position: i32,
}

impl UpdateGiftOccasionPosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<gift_occasions::Model> {
        let (gift_occasion, past_position) = self.validate(db).await?;
        self.update_position(db, gift_occasion, past_position).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(gift_occasions::Model, i32)> {
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

        let gift_occasion = find(db, self.account_id, self.gift_occasion_id).await?;

        let past_position =
            gift_occasion.position.ok_or_else(|| ApiError::internal("gift_occasions.position is null"))?;
        Ok((gift_occasion, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        gift_occasion: gift_occasions::Model,
        past_position: i32,
    ) -> ApiResult<gift_occasions::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, past_position).await?;
        } else {
            self.update_descending_position(db, past_position).await?;
        }

        let mut gift_occasion: gift_occasions::ActiveModel = gift_occasion.into();
        gift_occasion.position = Set(Some(self.new_position));
        Ok(gift_occasion.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        gift_occasions::Entity::update_many()
            .col_expr(gift_occasions::Column::Position, Expr::col(gift_occasions::Column::Position).sub(1))
            .col_expr(gift_occasions::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(gift_occasions::Column::AccountId.eq(self.account_id))
            .filter(gift_occasions::Column::Position.gt(past_position))
            .filter(gift_occasions::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        gift_occasions::Entity::update_many()
            .col_expr(gift_occasions::Column::Position, Expr::col(gift_occasions::Column::Position).add(1))
            .col_expr(gift_occasions::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(gift_occasions::Column::AccountId.eq(self.account_id))
            .filter(gift_occasions::Column::Position.gte(self.new_position))
            .filter(gift_occasions::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `$this->account()->giftOccasions()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, account_id: Uuid, gift_occasion_id: Id) -> ApiResult<gift_occasions::Model> {
    gift_occasions::Entity::find_by_id(gift_occasion_id)
        .filter(gift_occasions::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
