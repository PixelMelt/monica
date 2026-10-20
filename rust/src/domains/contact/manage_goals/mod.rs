//! `app/Domains/Contact/ManageGoals/Services`.

#[cfg(test)]
mod tests;

use chrono::{Duration, NaiveDate, NaiveTime};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::{contact_feed_items, contacts, goals, streaks};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `ContactFeedItem::ACTION_GOAL_CREATED`.
const ACTION_GOAL_CREATED: &str = "goal_created";
/// `ContactFeedItem::ACTION_GOAL_UPDATED`.
const ACTION_GOAL_UPDATED: &str = "goal_updated";
/// `ContactFeedItem::ACTION_GOAL_DESTROYED`.
const ACTION_GOAL_DESTROYED: &str = "goal_destroyed";

/// `CreateGoal`.
pub struct CreateGoal {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub name: Option<String>,
}

impl CreateGoal {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<goals::Model> {
        let checked = self.validate(db).await?;

        let goal = self.create(db, &checked).await?;
        create_feed_item(db, &checked, &goal, self.name.clone(), ACTION_GOAL_CREATED).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        Ok(goal)
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<Checked> {
        let mut v = Validator::new();
        v.max_len("name", self.name.as_deref(), 255);
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await
    }

    async fn create<C: ConnectionTrait>(&self, db: &C, checked: &Checked) -> ApiResult<goals::Model> {
        Ok(goals::ActiveModel {
            contact_id: Set(checked.contact().id),
            name: Set(self.name.clone().ok_or_else(|| ApiError::internal("goals.name can't be null"))?),
            active: Set(true),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `DestroyGoal`.
pub struct DestroyGoal {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub goal_id: Id,
}

impl DestroyGoal {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let goal = find(db, checked.contact().id, Some(self.goal_id)).await?;

        goals::Entity::delete_by_id(goal.id).exec(db).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        Self::create_feed_item(db, &checked, &goal).await?;

        Ok(())
    }

    async fn create_feed_item<C: ConnectionTrait>(db: &C, checked: &Checked, goal: &goals::Model) -> ApiResult<()> {
        contact_feed_items::ActiveModel {
            author_id: Set(Some(checked.author.id)),
            contact_id: Set(checked.contact().id),
            action: Set(ACTION_GOAL_DESTROYED.into()),
            description: Set(Some(goal.name.clone())),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(())
    }
}

/// `ToggleStreak`.
pub struct ToggleStreak {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub goal_id: Option<Id>,
    pub happened_at: String,
}

impl ToggleStreak {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let (checked, goal) = self.validate(db).await?;

        // `whereDate('happened_at', $date)`: within that day.
        let day = as_date_time(&self.happened_at)?;
        let entry = streaks::Entity::find()
            .filter(streaks::Column::GoalId.eq(goal.id))
            .filter(streaks::Column::HappenedAt.gte(day))
            .filter(streaks::Column::HappenedAt.lt(Timestamp(day.0 + Duration::days(1))))
            .one(db)
            .await?;

        if let Some(entry) = entry {
            Self::destroy_streak(db, entry).await?;
        } else {
            self.create_streak(db, &goal).await?;
        }

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        Ok(())
    }

    async fn validate<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(Checked, goals::Model)> {
        let mut v = Validator::new();
        if self.happened_at.trim().is_empty() {
            v.fail("happened_at", "required", &[]);
        } else {
            date_format(&mut v, "happened_at", &self.happened_at);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let goal = find(db, checked.contact().id, self.goal_id).await?;
        Ok((checked, goal))
    }

    async fn create_streak<C: ConnectionTrait>(&self, db: &C, goal: &goals::Model) -> ApiResult<()> {
        streaks::ActiveModel {
            goal_id: Set(goal.id),
            happened_at: Set(as_date_time(&self.happened_at)?),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(())
    }

    async fn destroy_streak<C: ConnectionTrait>(db: &C, streak: streaks::Model) -> ApiResult<()> {
        streaks::Entity::delete_by_id(streak.id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateGoal`.
pub struct UpdateGoal {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub goal_id: Option<Id>,
    pub name: Option<String>,
}

impl UpdateGoal {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<goals::Model> {
        let mut v = Validator::new();
        v.max_len("name", self.name.as_deref(), 255);
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;

        let goal = find(db, checked.contact().id, self.goal_id).await?;

        let mut goal: goals::ActiveModel = goal.into();
        goal.name = Set(self.name.clone().ok_or_else(|| ApiError::internal("goals.name can't be null"))?);
        let goal = goal.update(db).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        create_feed_item(db, &checked, &goal, self.name, ACTION_GOAL_UPDATED).await?;

        Ok(goal)
    }
}

/// `createFeedItem()` of CreateGoal and UpdateGoal, which differ only by the
/// action. `name` is `$this->data['name']`.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    checked: &Checked,
    goal: &goals::Model,
    name: Option<String>,
    action: &str,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(checked.author.id)),
        contact_id: Set(checked.contact().id),
        action: Set(action.into()),
        description: Set(name),
        feedable_type: Set(Some("App\\Models\\Goal".into())),
        feedable_id: Set(Some(goal.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// `$this->contact->goals()->findOrFail($id)` (`findOrFail(null)` fails too).
async fn find<C: ConnectionTrait>(db: &C, contact_id: Uuid, goal_id: Option<Id>) -> ApiResult<goals::Model> {
    let Some(goal_id) = goal_id else {
        return Err(ApiError::not_found());
    };
    goals::Entity::find_by_id(goal_id)
        .filter(goals::Column::ContactId.eq(contact_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `date_format:Y-m-d`: the value parses and formats back to itself.
fn date_format(v: &mut Validator, field: &str, value: &str) {
    let ok = NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok_and(|d| d.format("%Y-%m-%d").to_string() == value);
    if !ok {
        v.fail(field, "date_format", &[("format", "Y-m-d")]);
    }
}

/// The `datetime` cast of a `Y-m-d` string: its midnight.
fn as_date_time(value: &str) -> ApiResult<Timestamp> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(|d| Timestamp(d.and_time(NaiveTime::MIN)))
        .map_err(ApiError::internal)
}
