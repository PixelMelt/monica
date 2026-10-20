//! `CreateQuickFactTemplate`, `DestroyQuickFactTemplate`,
//! `UpdateQuickFactTemplate` and `UpdateQuickFactTemplatePosition`.

use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, QueryFilter, QuerySelect,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::vault_quick_facts_templates;
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateQuickFactTemplate`.
pub struct CreateQuickFactTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub label: String,
}

impl CreateQuickFactTemplate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<vault_quick_facts_templates::Model> {
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

        // determine the new position of the quick fact template entry
        let new_position = vault_quick_facts_templates::Entity::find()
            .select_only()
            .expr(Expr::col(vault_quick_facts_templates::Column::Position).max())
            .filter(vault_quick_facts_templates::Column::VaultId.eq(checked.vault().id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .unwrap_or(0)
            + 1;

        let quick_fact_template_entry = vault_quick_facts_templates::ActiveModel {
            vault_id: Set(self.vault_id),
            label: Set(Some(self.label)),
            position: Set(new_position),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(quick_fact_template_entry)
    }
}

/// `DestroyQuickFactTemplate`.
pub struct DestroyQuickFactTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub vault_quick_facts_template_id: Id,
}

impl DestroyQuickFactTemplate {
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

        let entry = find(db, checked.vault().id, self.vault_quick_facts_template_id).await?;

        vault_quick_facts_templates::Entity::delete_by_id(entry.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateQuickFactTemplate`.
pub struct UpdateQuickFactTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub vault_quick_facts_template_id: Id,
    pub label: String,
}

impl UpdateQuickFactTemplate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<vault_quick_facts_templates::Model> {
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

        let quick_fact_template_entry = find(db, checked.vault().id, self.vault_quick_facts_template_id).await?;

        let mut quick_fact_template_entry: vault_quick_facts_templates::ActiveModel = quick_fact_template_entry.into();
        quick_fact_template_entry.label = Set(Some(self.label));
        Ok(quick_fact_template_entry.update(db).await?)
    }
}

/// `UpdateQuickFactTemplatePosition`.
pub struct UpdateQuickFactTemplatePosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub vault_quick_facts_template_id: Id,
    pub new_position: i32,
}

impl UpdateQuickFactTemplatePosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<vault_quick_facts_templates::Model> {
        let (checked, quick_fact_template, past_position) = self.validate(db).await?;
        let quick_fact_template = self.update_position(db, &checked, quick_fact_template, past_position).await?;

        Ok(quick_fact_template)
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(Checked, vault_quick_facts_templates::Model, i32)> {
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

        let quick_fact_template = find(db, checked.vault().id, self.vault_quick_facts_template_id).await?;

        let past_position = quick_fact_template.position;

        Ok((checked, quick_fact_template, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        checked: &Checked,
        quick_fact_template: vault_quick_facts_templates::Model,
        past_position: i32,
    ) -> ApiResult<vault_quick_facts_templates::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, checked, past_position).await?;
        } else {
            self.update_descending_position(db, checked, past_position).await?;
        }

        let mut quick_fact_template: vault_quick_facts_templates::ActiveModel = quick_fact_template.into();
        quick_fact_template.position = Set(self.new_position);
        Ok(quick_fact_template.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        checked: &Checked,
        past_position: i32,
    ) -> ApiResult<()> {
        vault_quick_facts_templates::Entity::update_many()
            .col_expr(
                vault_quick_facts_templates::Column::Position,
                Expr::col(vault_quick_facts_templates::Column::Position).sub(1),
            )
            .col_expr(vault_quick_facts_templates::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(vault_quick_facts_templates::Column::VaultId.eq(checked.vault().id))
            .filter(vault_quick_facts_templates::Column::Position.gt(past_position))
            .filter(vault_quick_facts_templates::Column::Position.lte(self.new_position))
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
        vault_quick_facts_templates::Entity::update_many()
            .col_expr(
                vault_quick_facts_templates::Column::Position,
                Expr::col(vault_quick_facts_templates::Column::Position).add(1),
            )
            .col_expr(vault_quick_facts_templates::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(vault_quick_facts_templates::Column::VaultId.eq(checked.vault().id))
            .filter(vault_quick_facts_templates::Column::Position.gte(self.new_position))
            .filter(vault_quick_facts_templates::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `$this->vault->quickFactsTemplateEntries()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(
    db: &C,
    vault_id: Uuid,
    vault_quick_facts_template_id: Id,
) -> ApiResult<vault_quick_facts_templates::Model> {
    vault_quick_facts_templates::Entity::find_by_id(vault_quick_facts_template_id)
        .filter(vault_quick_facts_templates::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
