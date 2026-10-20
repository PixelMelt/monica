//! `app/Domains/Contact/ManageGroups/Services`.

#[cfg(test)]
mod tests;

use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check, value_or_null};
use crate::entities::{contact_feed_items, contact_group, contacts, group_type_roles, group_types, groups};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `AddContactToGroup`.
pub struct AddContactToGroup {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub group_id: Id,
    pub contact_id: Uuid,
    pub group_type_role_id: Option<Id>,
}

impl AddContactToGroup {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                contact_id: Some(self.contact_id),
                group_id: Some(self.group_id),
                ..editor(self.account_id, self.author_id, self.vault_id)
            },
        )
        .await?;

        let mut role = None;
        if let Some(group_type_role_id) = self.group_type_role_id
            && group_type_role_id != Id(0)
        {
            let r = group_type_roles::Entity::find_by_id(group_type_role_id)
                .one(db)
                .await?
                .ok_or_else(ApiError::not_found)?;

            find_group_type(db, self.account_id, r.group_type_id).await?;
            role = Some(r);
        }

        let (group, contact) = (checked.group(), checked.contact());
        let attached = contact_group::Entity::find()
            .filter(contact_group::Column::GroupId.eq(group.id))
            .filter(contact_group::Column::ContactId.eq(contact.id))
            .one(db)
            .await?;
        let group_type_role_id = role.map(|r| r.id);
        if attached.is_none() {
            contact_group::ActiveModel {
                group_id: Set(group.id),
                contact_id: Set(contact.id),
                group_type_role_id: Set(group_type_role_id),
                created_at: Set(None),
                updated_at: Set(None),
            }
            .insert(db)
            .await?;
        } else {
            contact_group::Entity::update_many()
                .col_expr(contact_group::Column::GroupTypeRoleId, Expr::value(group_type_role_id))
                .filter(contact_group::Column::GroupId.eq(group.id))
                .filter(contact_group::Column::ContactId.eq(contact.id))
                .exec(db)
                .await?;
        }

        touch(db, group).await?;

        update_last_edited_date(db, contact).await?;
        create_feed_item(db, &checked, "added_to_group").await
    }
}

/// `CreateGroup`.
pub struct CreateGroup {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub group_type_id: Option<Id>,
    pub name: Option<String>,
}

impl CreateGroup {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<groups::Model> {
        let mut v = Validator::new();
        group_type_exists(db, &mut v, self.group_type_id).await?;
        v.max_len("name", self.name.as_deref(), 255);
        v.finish()?;
        check(db, &editor(self.account_id, self.author_id, self.vault_id)).await?;

        if let Some(group_type_id) = self.group_type_id {
            find_group_type(db, self.account_id, group_type_id).await?;
        }

        let group = groups::ActiveModel {
            uuid: Set(Some(Uuid::new())),
            group_type_id: Set(self.group_type_id),
            vault_id: Set(self.vault_id),
            name: Set(name_or_fail(self.name)?),
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(group)
    }
}

/// `DestroyGroup`.
pub struct DestroyGroup {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub group_id: Id,
}

impl DestroyGroup {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check { group_id: Some(self.group_id), ..editor(self.account_id, self.author_id, self.vault_id) },
        )
        .await?;

        let mut group: groups::ActiveModel = checked.group().clone().into();
        group.deleted_at = Set(Some(crate::db::now()));
        group.update(db).await?;
        Ok(())
    }
}

/// `RemoveContactFromGroup`.
pub struct RemoveContactFromGroup {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub group_id: Id,
    pub contact_id: Uuid,
}

impl RemoveContactFromGroup {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                contact_id: Some(self.contact_id),
                group_id: Some(self.group_id),
                ..editor(self.account_id, self.author_id, self.vault_id)
            },
        )
        .await?;

        let (group, contact) = (checked.group(), checked.contact());
        contact_group::Entity::delete_many()
            .filter(contact_group::Column::GroupId.eq(group.id))
            .filter(contact_group::Column::ContactId.eq(contact.id))
            .exec(db)
            .await?;
        touch(db, group).await?;

        update_last_edited_date(db, contact).await?;
        create_feed_item(db, &checked, "removed_from_group").await
    }
}

/// `UpdateGroup`.
pub struct UpdateGroup {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub group_id: Id,
    pub group_type_id: Option<Id>,
    pub name: Option<String>,
}

impl UpdateGroup {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<groups::Model> {
        let mut v = Validator::new();
        group_type_exists(db, &mut v, self.group_type_id).await?;
        v.max_len("name", self.name.as_deref(), 255);
        v.finish()?;
        let checked = check(
            db,
            &Check { group_id: Some(self.group_id), ..editor(self.account_id, self.author_id, self.vault_id) },
        )
        .await?;

        if let Some(group_type_id) = self.group_type_id {
            find_group_type(db, self.account_id, group_type_id).await?;
        }

        let mut group: groups::ActiveModel = checked.group().clone().into();
        group.name = Set(name_or_fail(self.name)?);
        group.group_type_id = Set(self.group_type_id);
        Ok(group.update(db).await?)
    }
}

/// `permissions()`: the author is an editor of the vault.
fn editor(account_id: Uuid, author_id: Uuid, vault_id: Uuid) -> Check {
    Check { account_id, author_id, vault: Some((vault_id, Some(VaultPermission::Edit))), ..Default::default() }
}

/// `exists:group_types,id`.
async fn group_type_exists<C: ConnectionTrait>(db: &C, v: &mut Validator, id: Option<Id>) -> ApiResult<()> {
    if let Some(id) = id
        && group_types::Entity::find_by_id(id).one(db).await?.is_none()
    {
        v.fail("group_type_id", "exists", &[]);
    }
    Ok(())
}

/// `$this->account()->groupTypes()->findOrFail($id)`.
async fn find_group_type<C: ConnectionTrait>(db: &C, account_id: Uuid, id: Id) -> ApiResult<group_types::Model> {
    group_types::Entity::find_by_id(id)
        .filter(group_types::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `valueOrNull($data, 'name')` into `groups.name`, which is not nullable:
/// the insert or update fails like the PHP's does.
fn name_or_fail(name: Option<String>) -> ApiResult<String> {
    value_or_null(name).ok_or_else(|| ApiError::internal("groups.name cannot be null"))
}

/// `$this->group->touch()`.
async fn touch<C: ConnectionTrait>(db: &C, group: &groups::Model) -> ApiResult<()> {
    let mut group: groups::ActiveModel = group.clone().into();
    group.updated_at = Set(Some(crate::db::now()));
    group.update(db).await?;
    Ok(())
}

async fn update_last_edited_date<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<()> {
    let mut contact: contacts::ActiveModel = contact.clone().into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    contact.update(db).await?;
    Ok(())
}

async fn create_feed_item<C: ConnectionTrait>(db: &C, checked: &Checked, action: &str) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(checked.author.id)),
        contact_id: Set(checked.contact().id),
        action: Set(action.into()),
        description: Set(Some(checked.group().name.clone())),
        feedable_type: Set(Some("App\\Models\\Group".into())),
        feedable_id: Set(Some(checked.group().id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}
