//! `app/Domains/Settings/ManageRelationshipTypes/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check, value_or_null};
use crate::entities::{relationship_group_types, relationship_types};
use crate::error::{ApiError, ApiResult, Code};
use crate::validate::Validator;

/// `CreateRelationshipGroupType`.
pub struct CreateRelationshipGroupType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    pub r#type: Option<String>,
    pub can_be_deleted: bool,
}

impl CreateRelationshipGroupType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<relationship_group_types::Model> {
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

        let group = relationship_group_types::ActiveModel {
            account_id: Set(self.account_id),
            name: Set(self.name),
            name_translation_key: Set(self.name_translation_key),
            r#type: Set(value_or_null(self.r#type)),
            can_be_deleted: Set(self.can_be_deleted),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(group)
    }
}

/// `CreateRelationshipType`.
pub struct CreateRelationshipType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub relationship_group_type_id: Id,
    pub name: String,
    pub name_reverse_relationship: String,
    pub can_be_deleted: bool,
    pub r#type: Option<String>,
}

impl CreateRelationshipType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<relationship_types::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        if self.name_reverse_relationship.trim().is_empty() {
            v.fail("name_reverse_relationship", "required", &[]);
        }
        v.max_len("name_reverse_relationship", Some(&self.name_reverse_relationship), 255);
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

        let group = find_group(db, self.account_id, self.relationship_group_type_id).await?;

        let relationship_type = relationship_types::ActiveModel {
            relationship_group_type_id: Set(group.id),
            name: Set(Some(self.name)),
            name_reverse_relationship: Set(Some(self.name_reverse_relationship)),
            can_be_deleted: Set(self.can_be_deleted),
            r#type: Set(value_or_null(self.r#type)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(relationship_type)
    }
}

/// `DestroyRelationshipGroupType`.
pub struct DestroyRelationshipGroupType {
    pub account_id: Uuid,
    pub relationship_group_type_id: Id,
    pub author_id: Uuid,
}

impl DestroyRelationshipGroupType {
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

        let group = find_group(db, self.account_id, self.relationship_group_type_id).await?;
        if !group.can_be_deleted {
            return Err(ApiError::new(Code::Conflict));
        }

        relationship_group_types::Entity::delete_by_id(group.id).exec(db).await?;
        Ok(())
    }
}

/// `DestroyRelationshipType`.
pub struct DestroyRelationshipType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub relationship_group_type_id: Id,
    pub relationship_type_id: Id,
}

impl DestroyRelationshipType {
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

        let group = find_group(db, self.account_id, self.relationship_group_type_id).await?;

        let relationship_type = find_type(db, group.id, self.relationship_type_id).await?;
        if !relationship_type.can_be_deleted {
            return Err(ApiError::new(Code::Conflict));
        }

        relationship_types::Entity::delete_by_id(relationship_type.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateRelationshipGroupType`.
pub struct UpdateRelationshipGroupType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub relationship_group_type_id: Id,
    pub name: String,
}

impl UpdateRelationshipGroupType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<relationship_group_types::Model> {
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

        let group = find_group(db, self.account_id, self.relationship_group_type_id).await?;

        let mut group: relationship_group_types::ActiveModel = group.into();
        group.name = Set(Some(self.name));
        Ok(group.update(db).await?)
    }
}

/// `UpdateRelationshipType`.
pub struct UpdateRelationshipType {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub relationship_group_type_id: Id,
    pub relationship_type_id: Id,
    pub name: String,
    pub name_reverse_relationship: String,
}

impl UpdateRelationshipType {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<relationship_types::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        if self.name_reverse_relationship.trim().is_empty() {
            v.fail("name_reverse_relationship", "required", &[]);
        }
        v.max_len("name_reverse_relationship", Some(&self.name_reverse_relationship), 255);
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

        let group = find_group(db, self.account_id, self.relationship_group_type_id).await?;

        let relationship_type = find_type(db, group.id, self.relationship_type_id).await?;

        let mut relationship_type: relationship_types::ActiveModel = relationship_type.into();
        relationship_type.name = Set(Some(self.name));
        relationship_type.name_reverse_relationship = Set(Some(self.name_reverse_relationship));
        Ok(relationship_type.update(db).await?)
    }
}

/// `$this->account()->relationshipGroupTypes()->findOrFail($id)`.
async fn find_group<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    relationship_group_type_id: Id,
) -> ApiResult<relationship_group_types::Model> {
    relationship_group_types::Entity::find_by_id(relationship_group_type_id)
        .filter(relationship_group_types::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$group->types()->findOrFail($id)`.
async fn find_type<C: ConnectionTrait>(
    db: &C,
    relationship_group_type_id: Id,
    relationship_type_id: Id,
) -> ApiResult<relationship_types::Model> {
    relationship_types::Entity::find_by_id(relationship_type_id)
        .filter(relationship_types::Column::RelationshipGroupTypeId.eq(relationship_group_type_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
