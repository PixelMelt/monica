//! `app/Domains/Contact/ManageMoodTrackingEvents/Services`.

#[cfg(test)]
mod tests;

use chrono::{NaiveDate, NaiveTime};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::{contact_feed_items, contacts, mood_tracking_events, mood_tracking_parameters, users};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::validate::Validator;

/// `ContactFeedItem::ACTION_MOOD_TRACKING_EVENT_CREATED`.
const ACTION_MOOD_TRACKING_EVENT_CREATED: &str = "mood_tracking_event_added";
/// `ContactFeedItem::ACTION_MOOD_TRACKING_EVENT_UPDATED`.
const ACTION_MOOD_TRACKING_EVENT_UPDATED: &str = "mood_tracking_event_updated";
/// `ContactFeedItem::ACTION_MOOD_TRACKING_EVENT_DESTROYED`.
const ACTION_MOOD_TRACKING_EVENT_DESTROYED: &str = "mood_tracking_event_deleted";

/// `CreateMoodTrackingEvent`.
pub struct CreateMoodTrackingEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub mood_tracking_parameter_id: Id,
    pub rated_at: String,
    pub note: Option<String>,
    pub number_of_hours_slept: Option<i32>,
}

impl CreateMoodTrackingEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<mood_tracking_events::Model> {
        let mut v = Validator::new();
        if mood_tracking_parameters::Entity::find_by_id(self.mood_tracking_parameter_id).one(db).await?.is_none() {
            v.fail("mood_tracking_parameter_id", "exists", &[]);
        }
        let rated_at = date_format(&mut v, "rated_at", &self.rated_at);
        v.max_len("note", self.note.as_deref(), 65535);
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

        let parameter = parameter_in_vault(db, checked.vault().id, self.mood_tracking_parameter_id).await?;

        let event = mood_tracking_events::ActiveModel {
            contact_id: Set(checked.contact().id),
            mood_tracking_parameter_id: Set(self.mood_tracking_parameter_id),
            rated_at: Set(rated_at.expect("validated")),
            note: Set(value_or_null(self.note)),
            number_of_hours_slept: Set(self.number_of_hours_slept.filter(|n| *n != 0)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        update_last_edited_date(db, checked.contact()).await?;

        create_feed_item(
            db,
            &checked.author,
            checked.contact(),
            ACTION_MOOD_TRACKING_EVENT_CREATED,
            &parameter,
            Some(&event),
        )
        .await?;

        Ok(event)
    }
}

/// `DestroyMoodTrackingEvent`.
pub struct DestroyMoodTrackingEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub mood_tracking_event_id: Id,
}

impl DestroyMoodTrackingEvent {
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

        let event = mood_tracking_events::Entity::find_by_id(self.mood_tracking_event_id)
            .filter(mood_tracking_events::Column::ContactId.eq(checked.contact().id))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        mood_tracking_events::Entity::delete_by_id(event.id).exec(db).await?;

        update_last_edited_date(db, checked.contact()).await?;

        let parameter = mood_tracking_parameters::Entity::find_by_id(event.mood_tracking_parameter_id)
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;
        create_feed_item(db, &checked.author, checked.contact(), ACTION_MOOD_TRACKING_EVENT_DESTROYED, &parameter, None)
            .await
    }
}

/// `UpdateMoodTrackingEvent`.
pub struct UpdateMoodTrackingEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub mood_tracking_parameter_id: Id,
    pub mood_tracking_event_id: Id,
    pub rated_at: String,
    pub note: Option<String>,
    pub number_of_hours_slept: Option<i32>,
}

impl UpdateMoodTrackingEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<mood_tracking_events::Model> {
        let mut v = Validator::new();
        if mood_tracking_parameters::Entity::find_by_id(self.mood_tracking_parameter_id).one(db).await?.is_none() {
            v.fail("mood_tracking_parameter_id", "exists", &[]);
        }
        let rated_at = date_format(&mut v, "rated_at", &self.rated_at);
        v.max_len("note", self.note.as_deref(), 65535);
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

        let parameter = parameter_in_vault(db, checked.vault().id, self.mood_tracking_parameter_id).await?;

        let event = mood_tracking_events::Entity::find_by_id(self.mood_tracking_event_id)
            .filter(mood_tracking_events::Column::ContactId.eq(checked.contact().id))
            .filter(mood_tracking_events::Column::MoodTrackingParameterId.eq(parameter.id))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        let mut event: mood_tracking_events::ActiveModel = event.into();
        event.mood_tracking_parameter_id = Set(self.mood_tracking_parameter_id);
        event.rated_at = Set(rated_at.expect("validated"));
        event.note = Set(value_or_null(self.note));
        event.number_of_hours_slept = Set(self.number_of_hours_slept.filter(|n| *n != 0));
        let event = event.update(db).await?;

        update_last_edited_date(db, checked.contact()).await?;

        create_feed_item(
            db,
            &checked.author,
            checked.contact(),
            ACTION_MOOD_TRACKING_EVENT_UPDATED,
            &parameter,
            Some(&event),
        )
        .await?;

        Ok(event)
    }
}

/// `required|date_format:Y-m-d`: the date at midnight, when `value` is one.
fn date_format(v: &mut Validator, field: &str, value: &str) -> Option<Timestamp> {
    if value.trim().is_empty() {
        v.fail(field, "required", &[]);
        return None;
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok().filter(|d| d.format("%Y-%m-%d").to_string() == value);
    if date.is_none() {
        v.fail(field, "date_format", &[("format", "Y-m-d")]);
    }
    date.map(|d| Timestamp(d.and_time(NaiveTime::MIN)))
}

/// `$this->vault->moodTrackingParameters()->findOrFail($id)`.
async fn parameter_in_vault<C: ConnectionTrait>(
    db: &C,
    vault_id: Uuid,
    mood_tracking_parameter_id: Id,
) -> ApiResult<mood_tracking_parameters::Model> {
    mood_tracking_parameters::Entity::find_by_id(mood_tracking_parameter_id)
        .filter(mood_tracking_parameters::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->contact->last_updated_at = Carbon::now()`.
async fn update_last_edited_date<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<()> {
    let mut contact: contacts::ActiveModel = contact.clone().into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    contact.update(db).await?;
    Ok(())
}

/// `createFeedItem()`: described by the parameter's label, attached to the
/// event (`$this->moodTrackingEvent->feedItem()->save()`) when there is one.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    contact: &contacts::Model,
    action: &str,
    parameter: &mood_tracking_parameters::Model,
    event: Option<&mood_tracking_events::Model>,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author.id)),
        contact_id: Set(contact.id),
        action: Set(action.into()),
        description: Set(label(parameter)),
        feedable_type: Set(event.map(|_| "App\\Models\\MoodTrackingEvent".into())),
        feedable_id: Set(event.map(|e| e.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// `MoodTrackingParameter::label`: the label, else its translation key translated.
fn label(parameter: &mood_tracking_parameters::Model) -> Option<String> {
    value_or_null(parameter.label.clone())
        .or_else(|| parameter.label_translation_key.as_deref().map(|key| i18n::t(&i18n::current(), key, &[])))
}
