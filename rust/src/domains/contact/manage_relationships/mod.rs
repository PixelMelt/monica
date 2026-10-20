//! `app/Domains/Contact/ManageRelationships/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, contact_in_vault};
use crate::entities::{contacts, relationship_group_types, relationship_types, relationships};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `SetRelationship`.
pub struct SetRelationship {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub relationship_type_id: Id,
    pub contact_id: Uuid,
    pub other_contact_id: Uuid,
}

impl SetRelationship {
    /// When a relationship is created (father -> son), we need to create
    /// the inverse relationship (son -> father) as well.
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let (contact, other_contact, relationship_type) = validate(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
            self.relationship_type_id,
            self.other_contact_id,
        )
        .await?;

        // create the relationships
        set_relationship(db, &contact, &other_contact, &relationship_type).await?;

        update_last_edited_date(db, contact).await
    }
}

/// `$contact->relationships()->syncWithoutDetaching([$otherContact->id => […]])`.
async fn set_relationship<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
    other_contact: &contacts::Model,
    relationship_type: &relationship_types::Model,
) -> ApiResult<()> {
    let attached = relationships::Entity::find()
        .filter(relationships::Column::ContactId.eq(contact.id))
        .filter(relationships::Column::RelatedContactId.eq(other_contact.id))
        .one(db)
        .await?
        .is_some();

    if attached {
        relationships::Entity::update_many()
            .col_expr(relationships::Column::RelationshipTypeId, Expr::value(relationship_type.id))
            .filter(relationships::Column::ContactId.eq(contact.id))
            .filter(relationships::Column::RelatedContactId.eq(other_contact.id))
            .exec(db)
            .await?;
    } else {
        relationships::ActiveModel {
            relationship_type_id: Set(relationship_type.id),
            contact_id: Set(contact.id),
            related_contact_id: Set(other_contact.id),
            created_at: Set(None),
            updated_at: Set(None),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// `UnsetRelationship`.
pub struct UnsetRelationship {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub relationship_type_id: Id,
    pub contact_id: Uuid,
    pub other_contact_id: Uuid,
}

impl UnsetRelationship {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let (contact, other_contact, _) = validate(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
            self.relationship_type_id,
            self.other_contact_id,
        )
        .await?;

        unset_relationship(db, &contact, &other_contact).await?;
        unset_relationship(db, &other_contact, &contact).await?;

        let contact = if contact.listed { contact } else { soft_delete(db, contact).await? };
        if !other_contact.listed {
            soft_delete(db, other_contact).await?;
        }

        update_last_edited_date(db, contact).await
    }
}

/// `$contact->relationships()->detach([$otherContact->id])`.
async fn unset_relationship<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
    other_contact: &contacts::Model,
) -> ApiResult<()> {
    relationships::Entity::delete_many()
        .filter(relationships::Column::ContactId.eq(contact.id))
        .filter(relationships::Column::RelatedContactId.eq(other_contact.id))
        .exec(db)
        .await?;
    Ok(())
}

/// The rules, the permissions, the other contact
/// (`$this->vault->contacts()->findOrFail`) and the relationship type, whose
/// group type must be in the account.
async fn validate<C: ConnectionTrait>(
    db: &C,
    c: &Check,
    relationship_type_id: Id,
    other_contact_id: Uuid,
) -> ApiResult<(contacts::Model, contacts::Model, relationship_types::Model)> {
    let mut v = Validator::new();
    if relationship_types::Entity::find_by_id(relationship_type_id).one(db).await?.is_none() {
        v.fail("relationship_type_id", "exists", &[]);
    }
    if contacts::Entity::find_by_id(other_contact_id).one(db).await?.is_none() {
        v.fail("other_contact_id", "exists", &[]);
    }
    v.finish()?;
    let checked = check(db, c).await?;

    let other_contact = contact_in_vault(db, checked.vault().id, other_contact_id).await?;

    let relationship_type =
        relationship_types::Entity::find_by_id(relationship_type_id).one(db).await?.ok_or_else(ApiError::not_found)?;
    let group_type = relationship_group_types::Entity::find_by_id(relationship_type.relationship_group_type_id)
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    if group_type.account_id != c.account_id {
        return Err(ApiError::not_found());
    }

    Ok((checked.contact().clone(), other_contact, relationship_type))
}

/// `$contact->delete()`.
async fn soft_delete<C: ConnectionTrait>(db: &C, contact: contacts::Model) -> ApiResult<contacts::Model> {
    let mut contact: contacts::ActiveModel = contact.into();
    contact.deleted_at = Set(Some(crate::db::now()));
    Ok(contact.update(db).await?)
}

/// `updateLastEditedDate`.
async fn update_last_edited_date<C: ConnectionTrait>(db: &C, contact: contacts::Model) -> ApiResult<()> {
    let mut contact: contacts::ActiveModel = contact.into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    contact.update(db).await?;
    Ok(())
}
