//! `app/Domains/Vault/ManageJournals/Services`.

mod journals;
mod metrics;
mod posts;
mod slices;
#[cfg(test)]
mod tests;

pub use journals::*;
pub use metrics::*;
pub use posts::*;
pub use slices::*;

use chrono::{NaiveDate, NaiveTime};
use sea_orm::{ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::entities::{files, slice_of_lives};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `Post::class`, as `fileable_type` and `feedable_type`.
const POST: &str = "App\\Models\\Post";

/// `SliceOfLife::class`, as `fileable_type`.
const SLICE_OF_LIFE: &str = "App\\Models\\SliceOfLife";

/// `date_format:Y-m-d`. Returns the date as Eloquent's `datetime` cast
/// stores it (midnight).
fn date_format(v: &mut Validator, field: &str, value: Option<&str>) -> Option<Timestamp> {
    let s = value?;
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().filter(|d| d.format("%Y-%m-%d").to_string() == s);
    if date.is_none() {
        v.fail(field, "date_format", &[("format", "Y-m-d")]);
    }
    date.map(|d| Timestamp(d.and_time(NaiveTime::MIN)))
}

/// `$this->vault->files()->findOrFail($id)`.
async fn find_file<C: ConnectionTrait>(db: &C, vault_id: Uuid, file_id: Id) -> ApiResult<files::Model> {
    files::Entity::find_by_id(file_id)
        .filter(files::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$post->files` of these posts.
fn post_photos(post_ids: impl IntoIterator<Item = Id>) -> Condition {
    Condition::all().add(files::Column::FileableType.eq(POST)).add(files::Column::FileableId.is_in(post_ids))
}

/// Q7: the covers of these slices: `file_cover_image_id`, and earlier covers
/// still attached to the slice by `SetSliceOfLifeCoverImage`.
fn slice_covers(slices: &[slice_of_lives::Model]) -> Condition {
    Condition::any().add(files::Column::Id.is_in(slices.iter().filter_map(|s| s.file_cover_image_id))).add(
        Condition::all()
            .add(files::Column::FileableType.eq(SLICE_OF_LIFE))
            .add(files::Column::FileableId.is_in(slices.iter().map(|s| s.id))),
    )
}
