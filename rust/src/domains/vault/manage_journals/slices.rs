//! `AddPostToSliceOfLife`, `CreateSliceOfLife`, `DestroySliceOfLife`,
//! `RemovePostFromSliceOfLife`, `RemoveSliceOfLifeCoverImage`,
//! `SetSliceOfLifeCoverImage`, `UpdateSliceOfLife`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use super::journals::find_journal;
use super::posts::find_post;
use super::{SLICE_OF_LIFE, find_file, slice_covers};
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::{files, posts, slice_of_lives};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `AddPostToSliceOfLife`. `slice_of_life_id` is `nullable` in the PHP, but
/// without it the service reads an unset `$this->slice` and fails.
pub struct AddPostToSliceOfLife {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
    pub slice_of_life_id: Id,
}

impl AddPostToSliceOfLife {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<slice_of_lives::Model> {
        let mut v = Validator::new();
        if slice_of_lives::Entity::find_by_id(self.slice_of_life_id).one(db).await?.is_none() {
            v.fail("slice_of_life_id", "exists", &[]);
        }
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id).await?;
        let slice = find_slice(db, journal.id, self.slice_of_life_id).await?;

        let mut post: posts::ActiveModel = post.into();
        post.slice_of_life_id = Set(Some(slice.id));
        post.update(db).await?;

        Ok(slice)
    }
}

/// `CreateSliceOfLife`. `name` is `nullable` in the PHP, but the column
/// isn't: a null name fails on insert.
pub struct CreateSliceOfLife {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub name: String,
}

impl CreateSliceOfLife {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<slice_of_lives::Model> {
        let mut v = Validator::new();
        v.max_len("name", Some(&self.name), 255);
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        find_journal(db, self.vault_id, self.journal_id).await?;

        Ok(slice_of_lives::ActiveModel { journal_id: Set(self.journal_id), name: Set(self.name), ..Default::default() }
            .insert(db)
            .await?)
    }
}

/// `DestroySliceOfLife`. Q7: also deletes the `files` rows of its covers.
pub struct DestroySliceOfLife {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub slice_of_life_id: Id,
}

impl DestroySliceOfLife {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let slice = find_slice(db, journal.id, self.slice_of_life_id).await?;

        files::Entity::delete_many().filter(slice_covers(std::slice::from_ref(&slice))).exec(db).await?;
        slice_of_lives::Entity::delete_by_id(slice.id).exec(db).await?;
        Ok(())
    }
}

/// `RemovePostFromSliceOfLife`.
pub struct RemovePostFromSliceOfLife {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
}

impl RemovePostFromSliceOfLife {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id).await?;

        let mut post: posts::ActiveModel = post.into();
        post.slice_of_life_id = Set(None);
        post.update(db).await?;
        Ok(())
    }
}

/// `RemoveSliceOfLifeCoverImage`.
pub struct RemoveSliceOfLifeCoverImage {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub slice_of_life_id: Option<Id>,
}

impl RemoveSliceOfLifeCoverImage {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<slice_of_lives::Model> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let slice = find_slice(db, journal.id, self.slice_of_life_id.ok_or_else(ApiError::not_found)?).await?;

        if let Some(file_id) = slice.file_cover_image_id {
            files::Entity::delete_by_id(file_id).exec(db).await?;
        }

        let mut slice: slice_of_lives::ActiveModel = slice.into();
        slice.file_cover_image_id = Set(None);
        Ok(slice.update(db).await?)
    }
}

/// `SetSliceOfLifeCoverImage`. Q7: the previous cover's `files` row is deleted.
pub struct SetSliceOfLifeCoverImage {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub slice_of_life_id: Option<Id>,
    pub file_id: Id,
}

impl SetSliceOfLifeCoverImage {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<slice_of_lives::Model> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let slice = find_slice(db, journal.id, self.slice_of_life_id.ok_or_else(ApiError::not_found)?).await?;
        let file = find_file(db, self.vault_id, self.file_id).await?;
        let previous_cover = slice.file_cover_image_id;

        let mut slice: slice_of_lives::ActiveModel = slice.into();
        slice.file_cover_image_id = Set(Some(file.id));
        let slice = slice.update(db).await?;

        let file_id = file.id;
        let mut file: files::ActiveModel = file.into();
        file.fileable_id = Set(Some(slice.id));
        file.fileable_type = Set(Some(SLICE_OF_LIFE.into()));
        file.update(db).await?;

        if let Some(previous_cover) = previous_cover
            && previous_cover != file_id
        {
            files::Entity::delete_by_id(previous_cover).exec(db).await?;
        }

        Ok(slice)
    }
}

/// `UpdateSliceOfLife`.
pub struct UpdateSliceOfLife {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub slice_of_life_id: Id,
    pub name: String,
    pub description: Option<String>,
}

impl UpdateSliceOfLife {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<slice_of_lives::Model> {
        let mut v = Validator::new();
        v.max_len("name", Some(&self.name), 255);
        v.max_len("description", self.description.as_deref(), 255);
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let slice = find_slice(db, journal.id, self.slice_of_life_id).await?;

        let mut slice: slice_of_lives::ActiveModel = slice.into();
        slice.name = Set(self.name);
        slice.description = Set(value_or_null(self.description));
        Ok(slice.update(db).await?)
    }
}

/// `$journal->slicesOfLife()->findOrFail($id)`.
async fn find_slice<C: ConnectionTrait>(db: &C, journal_id: Id, slice_id: Id) -> ApiResult<slice_of_lives::Model> {
    slice_of_lives::Entity::find_by_id(slice_id)
        .filter(slice_of_lives::Column::JournalId.eq(journal_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
