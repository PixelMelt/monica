//! `app/Services/BaseService.php`: the permission checks every service runs
//! before its work (X002), as [`check`] and the [`Check`] it takes.
//!
//! PHP services list permission names; their dependencies are fixed (e.g.
//! `author_must_be_vault_editor` needs `vault_must_belong_to_account` and
//! `author_must_belong_to_account`). Here a [`Check`] names what the
//! service needs and [`check`] runs the checks in BaseService's order:
//! author in account, administrator, vault in account, vault permission,
//! contact in vault, group in vault.
//!
//! Errors map as: `ModelNotFoundException` → 404, `NotEnoughPermissionException`
//! → 403.

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::entities::{contacts, groups, user_vault, users, vaults};
use crate::error::{ApiError, ApiResult};

/// `Vault::PERMISSION_*`: lower is more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VaultPermission {
    Manage = 100,
    Edit = 200,
    View = 300,
}

impl VaultPermission {
    pub fn from_value(v: i32) -> Option<VaultPermission> {
        match v {
            100 => Some(VaultPermission::Manage),
            200 => Some(VaultPermission::Edit),
            300 => Some(VaultPermission::View),
            _ => None,
        }
    }
}

/// The checks a service asks for (its `permissions()`).
#[derive(Debug, Clone, Default)]
pub struct Check {
    /// `author_must_belong_to_account` (always).
    pub account_id: Uuid,
    pub author_id: Uuid,
    /// `author_must_be_account_administrator`.
    pub administrator: bool,
    /// `vault_must_belong_to_account`, plus `author_must_be_vault_manager` /
    /// `_editor` / `author_must_be_in_vault` when the permission is given.
    pub vault: Option<(Uuid, Option<VaultPermission>)>,
    /// `contact_must_belong_to_vault`.
    pub contact_id: Option<Uuid>,
    /// `group_must_belong_to_vault`.
    pub group_id: Option<Id>,
}

/// What the checks loaded (BaseService's `$author`, `$vault`, `$contact`, `$group`).
#[derive(Debug, Clone)]
pub struct Checked {
    pub author: users::Model,
    pub vault: Option<vaults::Model>,
    pub contact: Option<contacts::Model>,
    pub group: Option<groups::Model>,
}

impl Checked {
    pub fn vault(&self) -> &vaults::Model {
        self.vault.as_ref().expect("Check::vault was set")
    }

    pub fn contact(&self) -> &contacts::Model {
        self.contact.as_ref().expect("Check::contact_id was set")
    }

    pub fn group(&self) -> &groups::Model {
        self.group.as_ref().expect("Check::group_id was set")
    }
}

/// Runs the checks; see the module docs.
pub async fn check<C: ConnectionTrait>(db: &C, c: &Check) -> ApiResult<Checked> {
    let author = users::Entity::find_by_id(c.author_id)
        .filter(users::Column::AccountId.eq(c.account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    if c.administrator && !author.is_account_administrator {
        return Err(ApiError::forbidden());
    }
    let mut vault = None;
    if let Some((vault_id, permission)) = c.vault {
        let v = vaults::Entity::find_by_id(vault_id)
            .filter(vaults::Column::AccountId.eq(c.account_id))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;
        if let Some(required) = permission {
            user_permission_in_vault(db, author.id, v.id, required).await?;
        }
        vault = Some(v);
    }
    let mut contact = None;
    if let (Some(contact_id), Some(v)) = (c.contact_id, &vault) {
        contact = Some(contact_in_vault(db, v.id, contact_id).await?);
    }
    let mut group = None;
    if let (Some(group_id), Some(v)) = (c.group_id, &vault) {
        group = Some(
            groups::Entity::live()
                .filter(groups::Column::Id.eq(group_id))
                .filter(groups::Column::VaultId.eq(v.id))
                .one(db)
                .await?
                .ok_or_else(ApiError::not_found)?,
        );
    }
    Ok(Checked { author, vault, contact, group })
}

/// `validateUserPermissionInVault`: the user's permission in the vault is
/// `required` or more; 403 otherwise (also when the user isn't a member).
pub async fn user_permission_in_vault<C: ConnectionTrait>(
    db: &C,
    user_id: Uuid,
    vault_id: Uuid,
    required: VaultPermission,
) -> ApiResult<()> {
    let ok = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(user_id))
        .filter(user_vault::Column::VaultId.eq(vault_id))
        .filter(user_vault::Column::Permission.lte(required as i32))
        .one(db)
        .await?
        .is_some();
    if ok { Ok(()) } else { Err(ApiError::forbidden()) }
}

/// `$vault->contacts()->findOrFail($id)`: soft-deleted contacts excluded.
pub async fn contact_in_vault<C: ConnectionTrait>(
    db: &C,
    vault_id: Uuid,
    contact_id: Uuid,
) -> ApiResult<contacts::Model> {
    contacts::Entity::live()
        .filter(contacts::Column::Id.eq(contact_id))
        .filter(contacts::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `BaseService::valueOrNull`: `None` for what PHP's `empty()` calls empty
/// (no value, `""` or `"0"`).
pub fn value_or_null(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty() && v != "0")
}
