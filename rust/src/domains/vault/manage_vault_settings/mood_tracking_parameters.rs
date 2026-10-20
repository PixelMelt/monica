//! `CreateMoodTrackingParameter`, `DestroyMoodTrackingParameter`,
//! `UpdateMoodTrackingParameter` and `UpdateMoodTrackingParameterPosition`.

use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, QueryFilter, QuerySelect,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::mood_tracking_parameters;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateMoodTrackingParameter`.
pub struct CreateMoodTrackingParameter {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
    pub hex_color: String,
}

impl CreateMoodTrackingParameter {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<mood_tracking_parameters::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.max_len("hex_color", Some(&self.hex_color), 255);
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

        // determine the new position
        let new_position = mood_tracking_parameters::Entity::find()
            .select_only()
            .expr(Expr::col(mood_tracking_parameters::Column::Position).max())
            .filter(mood_tracking_parameters::Column::VaultId.eq(checked.vault().id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .unwrap_or(0)
            + 1;

        let mood_tracking_parameter = mood_tracking_parameters::ActiveModel {
            vault_id: Set(self.vault_id),
            label: Set(Some(self.label)),
            hex_color: Set(self.hex_color),
            position: Set(Some(new_position)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(mood_tracking_parameter)
    }
}

/// `DestroyMoodTrackingParameter`.
pub struct DestroyMoodTrackingParameter {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub mood_tracking_parameter_id: Id,
}

impl DestroyMoodTrackingParameter {
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

        let mood_tracking_parameter = find(db, checked.vault().id, self.mood_tracking_parameter_id).await?;

        mood_tracking_parameters::Entity::delete_by_id(mood_tracking_parameter.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateMoodTrackingParameter`.
pub struct UpdateMoodTrackingParameter {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub mood_tracking_parameter_id: Id,
    pub label: String,
    pub hex_color: String,
}

impl UpdateMoodTrackingParameter {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<mood_tracking_parameters::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
        v.max_len("hex_color", Some(&self.hex_color), 255);
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

        let mood_tracking_parameter = find(db, checked.vault().id, self.mood_tracking_parameter_id).await?;

        let mut mood_tracking_parameter: mood_tracking_parameters::ActiveModel = mood_tracking_parameter.into();
        mood_tracking_parameter.label = Set(Some(self.label));
        mood_tracking_parameter.hex_color = Set(self.hex_color);
        Ok(mood_tracking_parameter.update(db).await?)
    }
}

/// `UpdateMoodTrackingParameterPosition`.
pub struct UpdateMoodTrackingParameterPosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub mood_tracking_parameter_id: Id,
    pub new_position: i32,
}

impl UpdateMoodTrackingParameterPosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<mood_tracking_parameters::Model> {
        let (checked, mood_tracking_parameter, past_position) = self.validate(db).await?;
        let mood_tracking_parameter =
            self.update_position(db, &checked, mood_tracking_parameter, past_position).await?;

        Ok(mood_tracking_parameter)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(Checked, mood_tracking_parameters::Model, i32)> {
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

        let mood_tracking_parameter = find(db, checked.vault().id, self.mood_tracking_parameter_id).await?;

        let past_position = mood_tracking_parameter
            .position
            .ok_or_else(|| ApiError::internal("mood_tracking_parameters.position is null"))?;

        Ok((checked, mood_tracking_parameter, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        checked: &Checked,
        mood_tracking_parameter: mood_tracking_parameters::Model,
        past_position: i32,
    ) -> ApiResult<mood_tracking_parameters::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, checked, past_position).await?;
        } else {
            self.update_descending_position(db, checked, past_position).await?;
        }

        let mut mood_tracking_parameter: mood_tracking_parameters::ActiveModel = mood_tracking_parameter.into();
        mood_tracking_parameter.position = Set(Some(self.new_position));
        Ok(mood_tracking_parameter.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        checked: &Checked,
        past_position: i32,
    ) -> ApiResult<()> {
        mood_tracking_parameters::Entity::update_many()
            .col_expr(
                mood_tracking_parameters::Column::Position,
                Expr::col(mood_tracking_parameters::Column::Position).sub(1),
            )
            .col_expr(mood_tracking_parameters::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(mood_tracking_parameters::Column::VaultId.eq(checked.vault().id))
            .filter(mood_tracking_parameters::Column::Position.gt(past_position))
            .filter(mood_tracking_parameters::Column::Position.lte(self.new_position))
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
        mood_tracking_parameters::Entity::update_many()
            .col_expr(
                mood_tracking_parameters::Column::Position,
                Expr::col(mood_tracking_parameters::Column::Position).add(1),
            )
            .col_expr(mood_tracking_parameters::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(mood_tracking_parameters::Column::VaultId.eq(checked.vault().id))
            .filter(mood_tracking_parameters::Column::Position.gte(self.new_position))
            .filter(mood_tracking_parameters::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `$this->vault->moodTrackingParameters()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(
    db: &C,
    vault_id: Uuid,
    mood_tracking_parameter_id: Id,
) -> ApiResult<mood_tracking_parameters::Model> {
    mood_tracking_parameters::Entity::find_by_id(mood_tracking_parameter_id)
        .filter(mood_tracking_parameters::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
