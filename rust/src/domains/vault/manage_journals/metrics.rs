//! `CreateJournalMetric`, `CreatePostMetric`, `DestroyJournalMetric`,
//! `DestroyPostMetric`, `UpdateJournalMetric`, `UpdatePostMetric`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use super::journals::find_journal;
use super::posts::find_post;
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::{journal_metrics, post_metrics};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateJournalMetric`.
pub struct CreateJournalMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub label: String,
}

impl CreateJournalMetric {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<journal_metrics::Model> {
        let mut v = Validator::new();
        if self.label.trim().is_empty() {
            v.fail("label", "required", &[]);
        }
        v.max_len("label", Some(&self.label), 255);
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

        Ok(journal_metrics::ActiveModel {
            journal_id: Set(self.journal_id),
            label: Set(self.label),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `CreatePostMetric`.
pub struct CreatePostMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
    pub journal_metric_id: Id,
    pub value: i32,
    pub label: Option<String>,
}

impl CreatePostMetric {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<post_metrics::Model> {
        let mut v = Validator::new();
        if journal_metrics::Entity::find_by_id(self.journal_metric_id).one(db).await?.is_none() {
            v.fail("journal_metric_id", "exists", &[]);
        }
        v.max_len("label", self.label.as_deref(), 255);
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
        find_post(db, journal.id, self.post_id).await?;
        find_journal_metric(db, journal.id, self.journal_metric_id).await?;

        Ok(post_metrics::ActiveModel {
            journal_metric_id: Set(self.journal_metric_id),
            post_id: Set(self.post_id),
            value: Set(self.value),
            label: Set(value_or_null(self.label)),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `DestroyJournalMetric`.
pub struct DestroyJournalMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub journal_metric_id: Id,
}

impl DestroyJournalMetric {
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
        let journal_metric = find_journal_metric(db, journal.id, self.journal_metric_id).await?;

        journal_metrics::Entity::delete_by_id(journal_metric.id).exec(db).await?;
        Ok(())
    }
}

/// `DestroyPostMetric`.
pub struct DestroyPostMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
    pub post_metric_id: Id,
}

impl DestroyPostMetric {
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
        let post_metric = post_metrics::Entity::find_by_id(self.post_metric_id)
            .filter(post_metrics::Column::PostId.eq(post.id))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        post_metrics::Entity::delete_by_id(post_metric.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateJournalMetric`. `label` is `nullable` in the PHP, but the column
/// isn't: a null label fails on save.
pub struct UpdateJournalMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub journal_metric_id: Id,
    pub label: String,
}

impl UpdateJournalMetric {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<journal_metrics::Model> {
        let mut v = Validator::new();
        v.max_len("label", Some(&self.label), 255);
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
        let journal_metric = find_journal_metric(db, journal.id, self.journal_metric_id).await?;

        let mut journal_metric: journal_metrics::ActiveModel = journal_metric.into();
        journal_metric.label = Set(self.label);
        Ok(journal_metric.update(db).await?)
    }
}

/// `UpdatePostMetric`. `post_metric_id` isn't in the PHP's rules but is read.
pub struct UpdatePostMetric {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
    pub journal_metric_id: Id,
    pub post_metric_id: Id,
    pub value: i32,
    pub label: Option<String>,
}

impl UpdatePostMetric {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<post_metrics::Model> {
        let mut v = Validator::new();
        v.max_len("label", self.label.as_deref(), 255);
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
        find_post(db, journal.id, self.post_id).await?;
        let journal_metric = find_journal_metric(db, journal.id, self.journal_metric_id).await?;
        let post_metric = post_metrics::Entity::find_by_id(self.post_metric_id)
            .filter(post_metrics::Column::JournalMetricId.eq(journal_metric.id))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        let mut post_metric: post_metrics::ActiveModel = post_metric.into();
        post_metric.label = Set(value_or_null(self.label));
        post_metric.value = Set(self.value);
        Ok(post_metric.update(db).await?)
    }
}

/// `$journal->journalMetrics()->findOrFail($id)`.
async fn find_journal_metric<C: ConnectionTrait>(
    db: &C,
    journal_id: Id,
    journal_metric_id: Id,
) -> ApiResult<journal_metrics::Model> {
    journal_metrics::Entity::find_by_id(journal_metric_id)
        .filter(journal_metrics::Column::JournalId.eq(journal_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}
