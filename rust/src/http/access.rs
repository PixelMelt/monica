//! Vault access for HTTP handlers (api-design.md §9): a vault the caller
//! isn't a member of is a 404; a member without the needed permission gets
//! a 403. Services check again (`domains::base`).

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use super::session::Identity;
use super::state::AppState;
use crate::db::uuid::Uuid;
use crate::domains::base::VaultPermission;
use crate::entities::{user_vault, vaults};
use crate::error::{ApiError, ApiResult};

/// The vault and the caller's membership row.
pub struct VaultAccess {
    pub vault: vaults::Model,
    pub membership: user_vault::Model,
}

impl VaultAccess {
    pub fn permission(&self) -> i32 {
        self.membership.permission
    }
}

/// Loads `vault_id` for `id`, requiring at least `needed`.
pub async fn vault(state: &AppState, id: &Identity, vault_id: Uuid, needed: VaultPermission) -> ApiResult<VaultAccess> {
    let (membership, vault) = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(id.user.id))
        .filter(user_vault::Column::VaultId.eq(vault_id))
        .find_also_related(vaults::Entity)
        .one(&state.db)
        .await?
        .and_then(|(m, v)| v.map(|v| (m, v)))
        .filter(|(_, v)| v.account_id == id.user.account_id)
        .ok_or_else(ApiError::not_found)?;
    if membership.permission > needed as i32 {
        return Err(ApiError::forbidden());
    }
    Ok(VaultAccess { vault, membership })
}

/// 403 unless the caller is an account administrator (gate `administrator`).
pub fn administrator(id: &Identity) -> ApiResult<()> {
    if id.user.is_account_administrator { Ok(()) } else { Err(ApiError::forbidden()) }
}
