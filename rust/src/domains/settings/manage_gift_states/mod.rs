//! `app/Domains/Settings/ManageGiftStates/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::gift_states;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateGiftState`.
pub struct CreateGiftState {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub label: String,
}

impl CreateGiftState {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<gift_states::Model> {
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

    async fn create<C: ConnectionTrait>(self, db: &C) -> ApiResult<gift_states::Model> {
        // determine the new position
        let new_position = gift_states::Entity::find()
            .select_only()
            .expr(Expr::col(gift_states::Column::Position).max())
            .filter(gift_states::Column::AccountId.eq(self.account_id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .map_or(1, |p| p + 1);

        let gift_state = gift_states::ActiveModel {
            account_id: Set(self.account_id),
            label: Set(Some(self.label)),
            position: Set(Some(new_position)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(gift_state)
    }
}

/// `DestroyGiftState`.
pub struct DestroyGiftState {
    pub account_id: Uuid,
    pub gift_state_id: Id,
    pub author_id: Uuid,
}

impl DestroyGiftState {
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

        let gift_state = find(db, self.account_id, self.gift_state_id).await?;

        gift_states::Entity::delete_by_id(gift_state.id).exec(db).await?;

        self.reposition_everything(db, &gift_state).await
    }

    async fn reposition_everything<C: ConnectionTrait>(
        &self,
        db: &C,
        gift_state: &gift_states::Model,
    ) -> ApiResult<()> {
        gift_states::Entity::update_many()
            .col_expr(gift_states::Column::Position, Expr::col(gift_states::Column::Position).sub(1))
            .col_expr(gift_states::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(gift_states::Column::AccountId.eq(self.account_id))
            .filter(gift_states::Column::Position.gt(gift_state.position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `UpdateGiftState`.
pub struct UpdateGiftState {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub gift_state_id: Id,
    pub label: String,
}

impl UpdateGiftState {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<gift_states::Model> {
        let gift_state = self.validate(db).await?;
        self.update(db, gift_state).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<gift_states::Model> {
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
        find(db, self.account_id, self.gift_state_id).await
    }

    async fn update<C: ConnectionTrait>(self, db: &C, gift_state: gift_states::Model) -> ApiResult<gift_states::Model> {
        let mut gift_state: gift_states::ActiveModel = gift_state.into();
        gift_state.label = Set(Some(self.label));
        Ok(gift_state.update(db).await?)
    }
}

/// `UpdateGiftStatePosition`.
pub struct UpdateGiftStatePosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub gift_state_id: Id,
    pub new_position: i32,
}

impl UpdateGiftStatePosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<gift_states::Model> {
        let (gift_state, past_position) = self.validate(db).await?;
        self.update_position(db, gift_state, past_position).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(gift_states::Model, i32)> {
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

        let gift_state = find(db, self.account_id, self.gift_state_id).await?;

        let past_position = gift_state.position.ok_or_else(|| ApiError::internal("gift_states.position is null"))?;
        Ok((gift_state, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        gift_state: gift_states::Model,
        past_position: i32,
    ) -> ApiResult<gift_states::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, past_position).await?;
        } else {
            self.update_descending_position(db, past_position).await?;
        }

        let mut gift_state: gift_states::ActiveModel = gift_state.into();
        gift_state.position = Set(Some(self.new_position));
        Ok(gift_state.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        gift_states::Entity::update_many()
            .col_expr(gift_states::Column::Position, Expr::col(gift_states::Column::Position).sub(1))
            .col_expr(gift_states::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(gift_states::Column::AccountId.eq(self.account_id))
            .filter(gift_states::Column::Position.gt(past_position))
            .filter(gift_states::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        gift_states::Entity::update_many()
            .col_expr(gift_states::Column::Position, Expr::col(gift_states::Column::Position).add(1))
            .col_expr(gift_states::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(gift_states::Column::AccountId.eq(self.account_id))
            .filter(gift_states::Column::Position.gte(self.new_position))
            .filter(gift_states::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `$this->account()->giftStates()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, account_id: Uuid, gift_state_id: Id) -> ApiResult<gift_states::Model> {
    gift_states::Entity::find_by_id(gift_state_id)
        .filter(gift_states::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
