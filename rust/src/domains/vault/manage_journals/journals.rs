//! `CreateJournal`, `DestroyJournal`, `UpdateJournal`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter};

use super::{post_photos, slice_covers};
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::{files, journals, posts, slice_of_lives};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateJournal`.
pub struct CreateJournal {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

impl CreateJournal {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<journals::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        v.max_len("description", self.description.as_deref(), 65535);
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

        Ok(journals::ActiveModel {
            vault_id: Set(self.vault_id),
            name: Set(self.name),
            description: Set(value_or_null(self.description)),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `DestroyJournal`. Q7: also deletes the `files` rows of its posts' photos
/// and of its slices' covers.
pub struct DestroyJournal {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
}

impl DestroyJournal {
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

        let posts = posts::Entity::find().filter(posts::Column::JournalId.eq(journal.id)).all(db).await?;
        let slices =
            slice_of_lives::Entity::find().filter(slice_of_lives::Column::JournalId.eq(journal.id)).all(db).await?;
        files::Entity::delete_many()
            .filter(Condition::any().add(post_photos(posts.iter().map(|p| p.id))).add(slice_covers(&slices)))
            .exec(db)
            .await?;

        journals::Entity::delete_by_id(journal.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateJournal`.
pub struct UpdateJournal {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub name: String,
    pub description: Option<String>,
}

impl UpdateJournal {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<journals::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        v.max_len("description", self.description.as_deref(), 65535);
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

        let mut journal: journals::ActiveModel = journal.into();
        journal.name = Set(self.name);
        journal.description = Set(value_or_null(self.description));
        Ok(journal.update(db).await?)
    }
}

/// `$this->vault->journals()->findOrFail($id)`.
pub(super) async fn find_journal<C: ConnectionTrait>(
    db: &C,
    vault_id: Uuid,
    journal_id: Id,
) -> ApiResult<journals::Model> {
    journals::Entity::find_by_id(journal_id)
        .filter(journals::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
