//! `app/Helpers/StorageHelper.php` and the totals of the storage page
//! (`ManageStorage/Web/ViewHelpers`): bytes used by the files of an
//! account's vaults (X011).

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect};

use crate::db::uuid::Uuid;
use crate::entities::{accounts, files, vaults};
use crate::error::ApiResult;

pub const TYPE_PHOTO: &str = "photo";
pub const TYPE_DOCUMENT: &str = "document";
pub const TYPE_AVATAR: &str = "avatar";

/// Count and bytes of one type of file.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TypeUsage {
    pub count: u64,
    pub size: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub photo: TypeUsage,
    pub document: TypeUsage,
    pub avatar: TypeUsage,
}

impl Usage {
    pub fn total_size(&self) -> u64 {
        self.photo.size + self.document.size + self.avatar.size
    }
}

async fn files_of<C: ConnectionTrait>(db: &C, account_id: Uuid) -> ApiResult<Vec<(String, i32)>> {
    let vault_ids: Vec<Uuid> = vaults::Entity::find()
        .filter(vaults::Column::AccountId.eq(account_id))
        .all(db)
        .await?
        .into_iter()
        .map(|v| v.id)
        .collect();
    if vault_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(files::Entity::find()
        .select_only()
        .column(files::Column::Type)
        .column(files::Column::Size)
        .filter(files::Column::VaultId.is_in(vault_ids))
        .into_tuple::<(String, i32)>()
        .all(db)
        .await?)
}

/// What the storage page counts.
pub async fn usage<C: ConnectionTrait>(db: &C, account_id: Uuid) -> ApiResult<Usage> {
    let mut u = Usage::default();
    for (ty, size) in files_of(db, account_id).await? {
        let t = match ty.as_str() {
            TYPE_PHOTO => &mut u.photo,
            TYPE_DOCUMENT => &mut u.document,
            TYPE_AVATAR => &mut u.avatar,
            _ => continue,
        };
        t.count += 1;
        t.size += u64::try_from(size).unwrap_or(0);
    }
    Ok(u)
}

/// `StorageHelper::canUploadFile`: under the account's limit (0 = unlimited).
pub async fn can_upload_file<C: ConnectionTrait>(db: &C, account: &accounts::Model) -> ApiResult<bool> {
    if account.storage_limit_in_mb == 0 {
        return Ok(true);
    }
    let total: i64 = files_of(db, account.id).await?.iter().map(|(_, s)| i64::from(*s)).sum();
    Ok(total < i64::from(account.storage_limit_in_mb) * 1024 * 1024)
}
