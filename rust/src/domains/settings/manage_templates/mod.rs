//! `app/Domains/Settings/ManageTemplates/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, QueryFilter, QuerySelect,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::entities::{module_template_page, modules, template_pages, templates};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `TemplatePage::TYPE_CONTACT`.
const TYPE_CONTACT: &str = "contact_information";

/// `AssociateModuleToTemplatePage`.
pub struct AssociateModuleToTemplatePage {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Id,
    pub template_page_id: Id,
    pub module_id: Id,
}

impl AssociateModuleToTemplatePage {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<modules::Model> {
        let (module, template_page) = self.validate(db).await?;

        let new_position = module_template_page::Entity::find()
            .select_only()
            .expr(Expr::col(module_template_page::Column::Position).max())
            .filter(module_template_page::Column::TemplatePageId.eq(template_page.id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .unwrap_or(0)
            + 1;

        let attached = module_template_page::Entity::find()
            .filter(module_template_page::Column::TemplatePageId.eq(template_page.id))
            .filter(module_template_page::Column::ModuleId.eq(module.id))
            .one(db)
            .await?;
        if attached.is_none() {
            module_template_page::ActiveModel {
                template_page_id: Set(template_page.id),
                module_id: Set(module.id),
                position: Set(Some(new_position)),
                ..Default::default()
            }
            .insert(db)
            .await?;
        } else {
            module_template_page::Entity::update_many()
                .col_expr(module_template_page::Column::Position, Expr::value(new_position))
                .col_expr(module_template_page::Column::UpdatedAt, Expr::value(crate::db::now()))
                .filter(module_template_page::Column::TemplatePageId.eq(template_page.id))
                .filter(module_template_page::Column::ModuleId.eq(module.id))
                .exec(db)
                .await?;
        }

        Ok(module)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(modules::Model, template_pages::Model)> {
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

        let module = find_module(db, self.account_id, self.module_id).await?;

        let template = find_template(db, self.account_id, self.template_id).await?;

        let template_page = find_page(db, template.id, self.template_page_id).await?;

        Ok((module, template_page))
    }
}

/// `CreateModule`.
pub struct CreateModule {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    pub r#type: Option<String>,
    pub can_be_deleted: bool,
    pub reserved_to_contact_information: Option<bool>,
}

impl CreateModule {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<modules::Model> {
        let mut v = Validator::new();
        v.max_len("name", self.name.as_deref(), 255);
        v.max_len("name_translation_key", self.name_translation_key.as_deref(), 255);
        v.max_len("type", self.r#type.as_deref(), 255);
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

        let module = modules::ActiveModel {
            account_id: Set(self.account_id),
            name: Set(self.name),
            name_translation_key: Set(self.name_translation_key),
            r#type: Set(self.r#type.filter(|t| !t.is_empty() && t != "0")),
            can_be_deleted: Set(self.can_be_deleted),
            reserved_to_contact_information: Set(self.reserved_to_contact_information.unwrap_or(false)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(module)
    }
}

/// `CreateTemplate`.
pub struct CreateTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    pub can_be_deleted: Option<bool>,
}

impl CreateTemplate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<templates::Model> {
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

        let template = templates::ActiveModel {
            account_id: Set(self.account_id),
            name: Set(self.name),
            name_translation_key: Set(self.name_translation_key),
            can_be_deleted: Set(self.can_be_deleted.unwrap_or(true)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        CreateTemplatePage {
            account_id: self.account_id,
            author_id: self.author_id,
            template_id: template.id,
            name: None,
            name_translation_key: Some("Contact information".into()),
            r#type: Some(TYPE_CONTACT.into()),
            can_be_deleted: Some(false),
        }
        .execute(db)
        .await?;

        Ok(template)
    }
}

/// `CreateTemplatePage`.
pub struct CreateTemplatePage {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Id,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    pub r#type: Option<String>,
    pub can_be_deleted: Option<bool>,
}

impl CreateTemplatePage {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<template_pages::Model> {
        let mut v = Validator::new();
        v.max_len("name", self.name.as_deref(), 255);
        v.max_len("name_translation_key", self.name_translation_key.as_deref(), 255);
        v.max_len("type", self.r#type.as_deref(), 255);
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

        let template = find_template(db, self.account_id, self.template_id).await?;

        let new_position = template_pages::Entity::find()
            .select_only()
            .expr(Expr::col(template_pages::Column::Position).max())
            .filter(template_pages::Column::TemplateId.eq(template.id))
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten()
            .unwrap_or(0)
            + 1;

        let slug = slug(self.name.as_deref().or(self.name_translation_key.as_deref()).unwrap_or_default());
        let template_page = template_pages::ActiveModel {
            template_id: Set(self.template_id),
            name: Set(self.name),
            name_translation_key: Set(self.name_translation_key),
            slug: Set(slug),
            r#type: Set(self.r#type.filter(|t| !t.is_empty() && t != "0")),
            position: Set(Some(new_position)),
            can_be_deleted: Set(self.can_be_deleted.unwrap_or(true)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(template_page)
    }
}

/// `DestroyModule`.
pub struct DestroyModule {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub module_id: Id,
}

impl DestroyModule {
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

        let module = find_module(db, self.account_id, self.module_id).await?;

        if !module.can_be_deleted {
            return Err(ApiError::conflict("The module cannot be deleted."));
        }

        modules::Entity::delete_by_id(module.id).exec(db).await?;
        Ok(())
    }
}

/// `DestroyTemplate`.
pub struct DestroyTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Id,
}

impl DestroyTemplate {
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

        let template = find_template(db, self.account_id, self.template_id).await?;

        if !template.can_be_deleted {
            return Err(ApiError::conflict("The template cannot be deleted."));
        }

        templates::Entity::delete_by_id(template.id).exec(db).await?;
        Ok(())
    }
}

/// `DestroyTemplatePage`.
pub struct DestroyTemplatePage {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Id,
    pub template_page_id: Id,
}

impl DestroyTemplatePage {
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

        let template = find_template(db, self.account_id, self.template_id).await?;

        let template_page = find_page(db, template.id, self.template_page_id).await?;

        if !template_page.can_be_deleted {
            return Err(ApiError::conflict("The template page cannot be deleted."));
        }

        template_pages::Entity::delete_by_id(template_page.id).exec(db).await?;
        Ok(())
    }
}

/// `RemoveModuleFromTemplatePage`.
pub struct RemoveModuleFromTemplatePage {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Id,
    pub template_page_id: Id,
    pub module_id: Id,
}

impl RemoveModuleFromTemplatePage {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<modules::Model> {
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

        let module = find_module(db, self.account_id, self.module_id).await?;

        let template = find_template(db, self.account_id, self.template_id).await?;

        let template_page = find_page(db, template.id, self.template_page_id).await?;

        let detached = module_template_page::Entity::delete_many()
            .filter(module_template_page::Column::TemplatePageId.eq(template_page.id))
            .filter(module_template_page::Column::ModuleId.eq(module.id))
            .exec(db)
            .await?;
        if detached.rows_affected == 0 {
            return Err(ApiError::not_found());
        }

        Ok(module)
    }
}

/// `UpdateModule`.
pub struct UpdateModule {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub module_id: Id,
    pub name: String,
}

impl UpdateModule {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<modules::Model> {
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

        let module = find_module(db, self.account_id, self.module_id).await?;

        let mut module: modules::ActiveModel = module.into();
        module.name = Set(Some(self.name));
        Ok(module.update(db).await?)
    }
}

/// `UpdateModulePosition`.
pub struct UpdateModulePosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Id,
    pub template_page_id: Id,
    pub module_id: Id,
    pub new_position: i32,
}

impl UpdateModulePosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<modules::Model> {
        let (template_page, module, past_position) = self.validate(db).await?;
        self.update_position(db, &template_page, &module, past_position).await?;

        Ok(module)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(template_pages::Model, modules::Model, i32)> {
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

        let template = find_template(db, self.account_id, self.template_id).await?;

        let template_page = find_page(db, template.id, self.template_page_id).await?;

        let Some((module, Some(pivot))) = modules::Entity::find_by_id(self.module_id)
            .find_also_related(module_template_page::Entity)
            .filter(module_template_page::Column::TemplatePageId.eq(template_page.id))
            .one(db)
            .await?
        else {
            return Err(ApiError::not_found());
        };

        let past_position =
            pivot.position.ok_or_else(|| ApiError::internal("module_template_page.position is null"))?;

        Ok((template_page, module, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        template_page: &template_pages::Model,
        module: &modules::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        if self.new_position > past_position {
            self.update_ascending_position(db, template_page, past_position).await?;
        } else {
            self.update_descending_position(db, template_page, past_position).await?;
        }

        module_template_page::Entity::update_many()
            .col_expr(module_template_page::Column::Position, Expr::value(self.new_position))
            .col_expr(module_template_page::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(module_template_page::Column::TemplatePageId.eq(template_page.id))
            .filter(module_template_page::Column::ModuleId.eq(module.id))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_ascending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        template_page: &template_pages::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        module_template_page::Entity::update_many()
            .col_expr(module_template_page::Column::Position, Expr::col(module_template_page::Column::Position).sub(1))
            .filter(module_template_page::Column::TemplatePageId.eq(template_page.id))
            .filter(module_template_page::Column::Position.gt(past_position))
            .filter(module_template_page::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        template_page: &template_pages::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        module_template_page::Entity::update_many()
            .col_expr(module_template_page::Column::Position, Expr::col(module_template_page::Column::Position).add(1))
            .filter(module_template_page::Column::TemplatePageId.eq(template_page.id))
            .filter(module_template_page::Column::Position.gte(self.new_position))
            .filter(module_template_page::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `UpdateTemplate`.
pub struct UpdateTemplate {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Id,
    pub name: String,
}

impl UpdateTemplate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<templates::Model> {
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

        let template = find_template(db, self.account_id, self.template_id).await?;

        let mut template: templates::ActiveModel = template.into();
        template.name = Set(Some(self.name));
        Ok(template.update(db).await?)
    }
}

/// `UpdateTemplatePage`.
pub struct UpdateTemplatePage {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Id,
    pub template_page_id: Id,
    pub name: String,
}

impl UpdateTemplatePage {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<template_pages::Model> {
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

        let template = find_template(db, self.account_id, self.template_id).await?;

        let template_page = find_page(db, template.id, self.template_page_id).await?;

        let mut template_page: template_pages::ActiveModel = template_page.into();
        template_page.name = Set(Some(self.name.clone()));
        template_page.slug = Set(slug(&self.name));
        Ok(template_page.update(db).await?)
    }
}

/// `UpdateTemplatePagePosition`.
pub struct UpdateTemplatePagePosition {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Id,
    pub template_page_id: Id,
    pub new_position: i32,
}

impl UpdateTemplatePagePosition {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<template_pages::Model> {
        let (template, template_page, past_position) = self.validate(db).await?;
        let template_page = self.update_position(db, &template, template_page, past_position).await?;

        Ok(template_page)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(templates::Model, template_pages::Model, i32)> {
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

        let template = find_template(db, self.account_id, self.template_id).await?;

        let template_page = find_page(db, template.id, self.template_page_id).await?;

        let past_position =
            template_page.position.ok_or_else(|| ApiError::internal("template_pages.position is null"))?;

        Ok((template, template_page, past_position))
    }

    async fn update_position<C: ConnectionTrait>(
        &self,
        db: &C,
        template: &templates::Model,
        template_page: template_pages::Model,
        past_position: i32,
    ) -> ApiResult<template_pages::Model> {
        if self.new_position > past_position {
            self.update_ascending_position(db, template, past_position).await?;
        } else {
            self.update_descending_position(db, template, past_position).await?;
        }

        let mut template_page: template_pages::ActiveModel = template_page.into();
        template_page.position = Set(Some(self.new_position));
        Ok(template_page.update(db).await?)
    }

    async fn update_ascending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        template: &templates::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        template_pages::Entity::update_many()
            .col_expr(template_pages::Column::Position, Expr::col(template_pages::Column::Position).sub(1))
            .col_expr(template_pages::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(template_pages::Column::TemplateId.eq(template.id))
            .filter(template_pages::Column::Position.gt(past_position))
            .filter(template_pages::Column::Position.lte(self.new_position))
            .exec(db)
            .await?;
        Ok(())
    }

    async fn update_descending_position<C: ConnectionTrait>(
        &self,
        db: &C,
        template: &templates::Model,
        past_position: i32,
    ) -> ApiResult<()> {
        template_pages::Entity::update_many()
            .col_expr(template_pages::Column::Position, Expr::col(template_pages::Column::Position).add(1))
            .col_expr(template_pages::Column::UpdatedAt, Expr::value(crate::db::now()))
            .filter(template_pages::Column::TemplateId.eq(template.id))
            .filter(template_pages::Column::Position.gte(self.new_position))
            .filter(template_pages::Column::Position.lt(past_position))
            .exec(db)
            .await?;
        Ok(())
    }
}

/// `$this->account()->modules()->findOrFail($id)`.
async fn find_module<C: ConnectionTrait>(db: &C, account_id: Uuid, module_id: Id) -> ApiResult<modules::Model> {
    modules::Entity::find_by_id(module_id)
        .filter(modules::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->account()->templates()->findOrFail($id)`.
async fn find_template<C: ConnectionTrait>(db: &C, account_id: Uuid, template_id: Id) -> ApiResult<templates::Model> {
    templates::Entity::find_by_id(template_id)
        .filter(templates::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$template->pages()->findOrFail($id)`.
async fn find_page<C: ConnectionTrait>(
    db: &C,
    template_id: Id,
    template_page_id: Id,
) -> ApiResult<template_pages::Model> {
    template_pages::Entity::find_by_id(template_page_id)
        .filter(template_pages::Column::TemplateId.eq(template_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `Str::slug($title, '-')` without `Str::ascii` (what it does for a null language).
fn slug(title: &str) -> String {
    let title = title.replace('_', "-").replace('@', "-at-").to_lowercase();
    let title: String = title.chars().filter(|c| *c == '-' || c.is_alphanumeric() || c.is_whitespace()).collect();
    title.split(|c: char| c == '-' || c.is_whitespace()).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-")
}
