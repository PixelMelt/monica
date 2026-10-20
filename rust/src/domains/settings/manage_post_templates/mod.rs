//! `app/Domains/Settings/ManagePostTemplates/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::{post_template_sections, post_templates};
use crate::error::{ApiError, ApiResult, Code};
use crate::validate::Validator;

/// `CreatePostTemplate`.
pub struct CreatePostTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
    pub can_be_deleted: bool,
}

impl CreatePostTemplate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<post_templates::Model> {
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

        let new_position = post_templates::Entity::find()
            .select_only()
            .expr(Expr::col(post_templates::Column::Position).max())
            .filter(post_templates::Column::AccountId.eq(self.account_id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .map_or(1, |p| p + 1);

        let post_template = post_templates::ActiveModel {
            account_id: Set(self.account_id),
            label: Set(self.label),
            label_translation_key: Set(self.label_translation_key),
            position: Set(new_position),
            can_be_deleted: Set(self.can_be_deleted),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(post_template)
    }
}

/// `CreatePostTemplateSection`.
pub struct CreatePostTemplateSection {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub post_template_id: Id,
    pub label: Option<String>,
    pub label_translation_key: Option<String>,
    pub can_be_deleted: bool,
}

impl CreatePostTemplateSection {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<post_template_sections::Model> {
        let mut v = Validator::new();
        if post_templates::Entity::find_by_id(self.post_template_id).one(db).await?.is_none() {
            v.fail("post_template_id", "exists", &[]);
        }
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

        let post_template = find_post_template(db, self.account_id, self.post_template_id).await?;

        let new_position = post_template_sections::Entity::find()
            .select_only()
            .expr(Expr::col(post_template_sections::Column::Position).max())
            .filter(post_template_sections::Column::PostTemplateId.eq(post_template.id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .map_or(1, |p| p + 1);

        let post_template_section = post_template_sections::ActiveModel {
            post_template_id: Set(self.post_template_id),
            label: Set(self.label),
            label_translation_key: Set(self.label_translation_key),
            position: Set(new_position),
            can_be_deleted: Set(self.can_be_deleted),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(post_template_section)
    }
}

/// `DestroyPostTemplate`.
pub struct DestroyPostTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub post_template_id: Id,
}

impl DestroyPostTemplate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        if post_templates::Entity::find_by_id(self.post_template_id).one(db).await?.is_none() {
            v.fail("post_template_id", "exists", &[]);
        }
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

        let post_template = find_post_template(db, self.account_id, self.post_template_id).await?;

        if !post_template.can_be_deleted {
            return Err(ApiError::new(Code::Conflict));
        }

        post_templates::Entity::delete_by_id(post_template.id).exec(db).await?;
        Ok(())
    }
}

/// `DestroyPostTemplateSection`.
pub struct DestroyPostTemplateSection {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub post_template_id: Id,
    pub post_template_section_id: Id,
}

impl DestroyPostTemplateSection {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let mut v = Validator::new();
        if post_templates::Entity::find_by_id(self.post_template_id).one(db).await?.is_none() {
            v.fail("post_template_id", "exists", &[]);
        }
        if post_template_sections::Entity::find_by_id(self.post_template_section_id).one(db).await?.is_none() {
            v.fail("post_template_section_id", "exists", &[]);
        }
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

        let post_template = find_post_template(db, self.account_id, self.post_template_id).await?;

        let post_template_section =
            find_post_template_section(db, post_template.id, self.post_template_section_id).await?;

        if !post_template_section.can_be_deleted {
            return Err(ApiError::new(Code::Conflict));
        }

        post_template_sections::Entity::delete_by_id(post_template_section.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdatePostTemplate`.
pub struct UpdatePostTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub post_template_id: Id,
    pub label: String,
}

impl UpdatePostTemplate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<post_templates::Model> {
        let mut v = Validator::new();
        if post_templates::Entity::find_by_id(self.post_template_id).one(db).await?.is_none() {
            v.fail("post_template_id", "exists", &[]);
        }
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

        let post_template = find_post_template(db, self.account_id, self.post_template_id).await?;

        let mut post_template: post_templates::ActiveModel = post_template.into();
        post_template.label = Set(Some(self.label));
        Ok(post_template.update(db).await?)
    }
}

/// `UpdatePostTemplatePosition`.
pub struct UpdatePostTemplatePosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub post_template_id: Id,
    pub new_position: i32,
}

impl UpdatePostTemplatePosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<post_templates::Model> {
        let (post_template, past_position) = self.validate(db).await?;
        self.update_position(db, post_template, past_position).await
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(post_templates::Model, i32)> {
        let mut v = Validator::new();
        if post_templates::Entity::find_by_id(self.post_template_id).one(db).await?.is_none() {
            v.fail("post_template_id", "exists", &[]);
        }
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

        let post_template = find_post_template(db, self.account_id, self.post_template_id).await?;

        let past_position = post_template.position;
        Ok((post_template, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        post_template: post_templates::Model,
        past_position: i32,
    ) -> ApiResult<post_templates::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, past_position).await?;
        } else {
            self.update_descending_position(db, past_position).await?;
        }

        let mut post_template: post_templates::ActiveModel = post_template.into();
        post_template.position = Set(self.new_position);
        Ok(post_template.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        post_templates::Entity::update_many()
            .col_expr(post_templates::Column::Position, Expr::col(post_templates::Column::Position).sub(1))
            .col_expr(post_templates::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(post_templates::Column::AccountId.eq(self.account_id))
            .filter(post_templates::Column::Position.gt(past_position))
            .filter(post_templates::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(&self, db: &C, past_position: i32) -> ApiResult<()> {
        post_templates::Entity::update_many()
            .col_expr(post_templates::Column::Position, Expr::col(post_templates::Column::Position).add(1))
            .col_expr(post_templates::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(post_templates::Column::AccountId.eq(self.account_id))
            .filter(post_templates::Column::Position.gte(self.new_position))
            .filter(post_templates::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `UpdatePostTemplateSection`.
pub struct UpdatePostTemplateSection {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub post_template_id: Id,
    pub post_template_section_id: Id,
    pub label: String,
}

impl UpdatePostTemplateSection {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<post_template_sections::Model> {
        let mut v = Validator::new();
        if post_templates::Entity::find_by_id(self.post_template_id).one(db).await?.is_none() {
            v.fail("post_template_id", "exists", &[]);
        }
        if post_template_sections::Entity::find_by_id(self.post_template_section_id).one(db).await?.is_none() {
            v.fail("post_template_section_id", "exists", &[]);
        }
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

        let post_template = find_post_template(db, self.account_id, self.post_template_id).await?;

        let post_template_section =
            find_post_template_section(db, post_template.id, self.post_template_section_id).await?;

        let mut post_template_section: post_template_sections::ActiveModel = post_template_section.into();
        post_template_section.label = Set(Some(self.label));
        Ok(post_template_section.update(db).await?)
    }
}

/// `UpdatePostTemplateSectionPosition`.
pub struct UpdatePostTemplateSectionPosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub post_template_id: Id,
    pub post_template_section_id: Id,
    pub new_position: i32,
}

impl UpdatePostTemplateSectionPosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<post_template_sections::Model> {
        let (post_template, post_template_section, past_position) = self.validate(db).await?;
        self.update_position(db, &post_template, post_template_section, past_position).await
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(post_templates::Model, post_template_sections::Model, i32)> {
        let mut v = Validator::new();
        if post_templates::Entity::find_by_id(self.post_template_id).one(db).await?.is_none() {
            v.fail("post_template_id", "exists", &[]);
        }
        if post_template_sections::Entity::find_by_id(self.post_template_section_id).one(db).await?.is_none() {
            v.fail("post_template_section_id", "exists", &[]);
        }
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

        let post_template = find_post_template(db, self.account_id, self.post_template_id).await?;

        let post_template_section =
            find_post_template_section(db, post_template.id, self.post_template_section_id).await?;

        let past_position = post_template_section.position;
        Ok((post_template, post_template_section, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        post_template: &post_templates::Model,
        post_template_section: post_template_sections::Model,
        past_position: i32,
    ) -> ApiResult<post_template_sections::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, post_template, past_position).await?;
        } else {
            self.update_descending_position(db, post_template, past_position).await?;
        }

        let mut post_template_section: post_template_sections::ActiveModel = post_template_section.into();
        post_template_section.position = Set(self.new_position);
        Ok(post_template_section.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        post_template: &post_templates::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        post_template_sections::Entity::update_many()
            .col_expr(
                post_template_sections::Column::Position,
                Expr::col(post_template_sections::Column::Position).sub(1),
            )
            .col_expr(post_template_sections::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(post_template_sections::Column::PostTemplateId.eq(post_template.id))
            .filter(post_template_sections::Column::Position.gt(past_position))
            .filter(post_template_sections::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        post_template: &post_templates::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        post_template_sections::Entity::update_many()
            .col_expr(
                post_template_sections::Column::Position,
                Expr::col(post_template_sections::Column::Position).add(1),
            )
            .col_expr(post_template_sections::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(post_template_sections::Column::PostTemplateId.eq(post_template.id))
            .filter(post_template_sections::Column::Position.gte(self.new_position))
            .filter(post_template_sections::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `$this->account()->postTemplates()->findOrFail($id)`.
async fn find_post_template<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    post_template_id: Id,
) -> ApiResult<post_templates::Model> {
    post_templates::Entity::find_by_id(post_template_id)
        .filter(post_templates::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$postTemplate->postTemplateSections()->findOrFail($id)`.
async fn find_post_template_section<C: ConnectionTrait>(
    db: &C,
    post_template_id: Id,
    post_template_section_id: Id,
) -> ApiResult<post_template_sections::Model> {
    post_template_sections::Entity::find_by_id(post_template_section_id)
        .filter(post_template_sections::Column::PostTemplateId.eq(post_template_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
