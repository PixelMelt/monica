//! `app/Domains/Contact/ManageLifeEvents/Services`.

#[cfg(test)]
mod tests;

use chrono::NaiveDate;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check, contact_in_vault, value_or_null};
use crate::entities::{
    account_currency, contacts, currencies, life_event_categories, life_event_participants, life_event_types,
    life_events, timeline_event_participants, timeline_events,
};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateLifeEvent`.
pub struct CreateLifeEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub timeline_event_id: Id,
    pub life_event_type_id: Id,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub happened_at: String,
    pub costs: Option<i32>,
    pub currency_id: Option<Id>,
    pub paid_by_contact_id: Option<Uuid>,
    pub duration_in_minutes: Option<i32>,
    pub distance: Option<i32>,
    pub distance_unit: Option<String>,
    pub from_place: Option<String>,
    pub to_place: Option<String>,
    pub place: Option<String>,
    pub participant_ids: Vec<Uuid>,
}

impl CreateLifeEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_events::Model> {
        let (timeline_event, participants, happened_at) = self.validate(db).await?;
        let life_event = self.store(db, happened_at).await?;
        associate_participants(db, &participants, &life_event, &timeline_event).await?;
        update_last_edited_date(db, &participants).await?;

        Ok(life_event)
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(timeline_events::Model, Vec<contacts::Model>, NaiveDate)> {
        let mut v = Validator::new();
        if life_event_types::Entity::find_by_id(self.life_event_type_id).one(db).await?.is_none() {
            v.fail("life_event_type_id", "exists", &[]);
        }
        v.max_len("summary", self.summary.as_deref(), 255);
        v.max_len("description", self.description.as_deref(), 65535);
        let happened_at = date_format(&mut v, "happened_at", &self.happened_at);
        if let Some(id) = self.currency_id
            && currencies::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("currency_id", "exists", &[]);
        }
        if let Some(id) = self.paid_by_contact_id
            && contacts::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("paid_by_contact_id", "exists", &[]);
        }
        v.max_len("distance_unit", self.distance_unit.as_deref(), 255);
        v.max_len("from_place", self.from_place.as_deref(), 255);
        v.max_len("to_place", self.to_place.as_deref(), 255);
        v.max_len("place", self.place.as_deref(), 255);
        if self.participant_ids.is_empty() {
            v.fail("participant_ids", "required", &[]);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let life_event_type = life_event_types::Entity::find_by_id(self.life_event_type_id)
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        let timeline_event = timeline_event_in_vault(db, checked.vault().id, self.timeline_event_id).await?;

        category_in_vault(db, checked.vault().id, life_event_type.life_event_category_id).await?;

        if let Some(id) = self.paid_by_contact_id {
            contact_in_vault(db, checked.vault().id, id).await?;
        }

        if let Some(id) = self.currency_id {
            currency_in_account(db, checked.author.account_id, id).await?;
        }

        let participants = participants(db, &checked, &self.participant_ids).await?;
        Ok((timeline_event, participants, happened_at.expect("validated")))
    }

    async fn store<C: ConnectionTrait>(self, db: &C, happened_at: NaiveDate) -> ApiResult<life_events::Model> {
        Ok(life_events::ActiveModel {
            timeline_event_id: Set(self.timeline_event_id),
            life_event_type_id: Set(self.life_event_type_id),
            summary: Set(value_or_null(self.summary)),
            description: Set(value_or_null(self.description)),
            happened_at: Set(happened_at),
            costs: Set(self.costs.filter(|n| *n != 0)),
            currency_id: Set(self.currency_id.filter(|id| id.0 != 0)),
            paid_by_contact_id: Set(self.paid_by_contact_id),
            duration_in_minutes: Set(self.duration_in_minutes.filter(|n| *n != 0)),
            distance: Set(self.distance.filter(|n| *n != 0)),
            distance_unit: Set(value_or_null(self.distance_unit)),
            from_place: Set(value_or_null(self.from_place)),
            to_place: Set(value_or_null(self.to_place)),
            place: Set(value_or_null(self.place)),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `CreateTimelineEvent`.
pub struct CreateTimelineEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub label: Option<String>,
    pub started_at: String,
}

impl CreateTimelineEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<timeline_events::Model> {
        let mut v = Validator::new();
        v.max_len("label", self.label.as_deref(), 255);
        let started_at = date_format(&mut v, "started_at", &self.started_at);
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

        Ok(timeline_events::ActiveModel {
            vault_id: Set(self.vault_id),
            label: Set(value_or_null(self.label)),
            started_at: Set(started_at.expect("validated")),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

/// `DestroyLifeEvent`.
pub struct DestroyLifeEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub timeline_event_id: Id,
    pub life_event_id: Id,
}

impl DestroyLifeEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let timeline_event = timeline_event_in_vault(db, checked.vault().id, self.timeline_event_id).await?;

        let life_event = life_event_in_timeline_event(db, &timeline_event, self.life_event_id).await?;

        life_events::Entity::delete_by_id(life_event.id).exec(db).await?;

        delete_timeline_event(db, &timeline_event).await
    }
}

/// `deleteTimelineEvent()`: a LifeEvent is always associated with a timeline
/// event; if we delete the last life event of the timeline event, we need to
/// delete the timeline event as well.
async fn delete_timeline_event<C: ConnectionTrait>(db: &C, timeline_event: &timeline_events::Model) -> ApiResult<()> {
    let count = life_events::Entity::find()
        .filter(life_events::Column::TimelineEventId.eq(timeline_event.id))
        .count(db)
        .await?;
    if count == 0 {
        timeline_events::Entity::delete_by_id(timeline_event.id).exec(db).await?;
    }
    Ok(())
}

/// `DestroyTimelineEvent`.
pub struct DestroyTimelineEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub timeline_event_id: Id,
}

impl DestroyTimelineEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let timeline_event = timeline_event_in_vault(db, checked.vault().id, self.timeline_event_id).await?;

        timeline_events::Entity::delete_by_id(timeline_event.id).exec(db).await?;
        Ok(())
    }
}

/// `ToggleLifeEvent`.
pub struct ToggleLifeEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub timeline_event_id: Id,
    pub life_event_id: Id,
}

impl ToggleLifeEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_events::Model> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let timeline_event = timeline_event_in_vault(db, checked.vault().id, self.timeline_event_id).await?;

        let life_event = life_event_in_timeline_event(db, &timeline_event, self.life_event_id).await?;

        let collapsed = !life_event.collapsed;
        let mut life_event: life_events::ActiveModel = life_event.into();
        life_event.collapsed = Set(collapsed);
        Ok(life_event.update(db).await?)
    }
}

/// `ToggleTimelineEvent`.
pub struct ToggleTimelineEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub timeline_event_id: Id,
}

impl ToggleTimelineEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<timeline_events::Model> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let timeline_event = timeline_event_in_vault(db, checked.vault().id, self.timeline_event_id).await?;

        let collapsed = !timeline_event.collapsed;
        let mut timeline_event: timeline_events::ActiveModel = timeline_event.into();
        timeline_event.collapsed = Set(collapsed);
        Ok(timeline_event.update(db).await?)
    }
}

/// `UpdateLifeEvent`.
pub struct UpdateLifeEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub timeline_event_id: Id,
    pub life_event_type_id: Id,
    pub life_event_id: Id,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub happened_at: String,
    pub costs: Option<i32>,
    pub currency_id: Option<Id>,
    pub paid_by_contact_id: Option<Uuid>,
    pub duration_in_minutes: Option<i32>,
    pub distance: Option<i32>,
    pub distance_unit: Option<String>,
    pub from_place: Option<String>,
    pub to_place: Option<String>,
    pub place: Option<String>,
    pub participant_ids: Vec<Uuid>,
}

impl UpdateLifeEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<life_events::Model> {
        let (timeline_event, life_event, participants, happened_at) = self.validate(db).await?;
        let life_event = self.update(db, life_event, happened_at).await?;
        dissociate_participants(db, &life_event).await?;
        associate_participants(db, &participants, &life_event, &timeline_event).await?;
        update_last_edited_date(db, &participants).await?;

        Ok(life_event)
    }

    async fn validate<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> ApiResult<(timeline_events::Model, life_events::Model, Vec<contacts::Model>, NaiveDate)> {
        let mut v = Validator::new();
        if life_event_types::Entity::find_by_id(self.life_event_type_id).one(db).await?.is_none() {
            v.fail("life_event_type_id", "exists", &[]);
        }
        v.max_len("summary", self.summary.as_deref(), 255);
        v.max_len("description", self.description.as_deref(), 65535);
        let happened_at = date_format(&mut v, "happened_at", &self.happened_at);
        if let Some(id) = self.currency_id
            && currencies::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("currency_id", "exists", &[]);
        }
        if let Some(id) = self.paid_by_contact_id
            && contacts::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("paid_by_contact_id", "exists", &[]);
        }
        v.max_len("distance_unit", self.distance_unit.as_deref(), 255);
        v.max_len("from_place", self.from_place.as_deref(), 255);
        v.max_len("to_place", self.to_place.as_deref(), 255);
        v.max_len("place", self.place.as_deref(), 255);
        if self.participant_ids.is_empty() {
            v.fail("participant_ids", "required", &[]);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let timeline_event = timeline_event_in_vault(db, checked.vault().id, self.timeline_event_id).await?;

        let life_event_type = life_event_types::Entity::find_by_id(self.life_event_type_id)
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        let life_event = life_event_in_timeline_event(db, &timeline_event, self.life_event_id).await?;

        category_in_vault(db, checked.vault().id, life_event_type.life_event_category_id).await?;

        if let Some(id) = self.paid_by_contact_id {
            contact_in_vault(db, checked.vault().id, id).await?;
        }

        if let Some(id) = self.currency_id {
            currency_in_account(db, checked.author.account_id, id).await?;
        }

        let participants = participants(db, &checked, &self.participant_ids).await?;
        Ok((timeline_event, life_event, participants, happened_at.expect("validated")))
    }

    async fn update<C: ConnectionTrait>(
        self,
        db: &C,
        life_event: life_events::Model,
        happened_at: NaiveDate,
    ) -> ApiResult<life_events::Model> {
        let mut life_event: life_events::ActiveModel = life_event.into();
        life_event.timeline_event_id = Set(self.timeline_event_id);
        life_event.life_event_type_id = Set(self.life_event_type_id);
        life_event.summary = Set(value_or_null(self.summary));
        life_event.description = Set(value_or_null(self.description));
        life_event.happened_at = Set(happened_at);
        life_event.costs = Set(self.costs.filter(|n| *n != 0));
        life_event.currency_id = Set(self.currency_id.filter(|id| id.0 != 0));
        life_event.paid_by_contact_id = Set(self.paid_by_contact_id);
        life_event.duration_in_minutes = Set(self.duration_in_minutes.filter(|n| *n != 0));
        life_event.distance = Set(self.distance.filter(|n| *n != 0));
        life_event.distance_unit = Set(value_or_null(self.distance_unit));
        life_event.from_place = Set(value_or_null(self.from_place));
        life_event.to_place = Set(value_or_null(self.to_place));
        life_event.place = Set(value_or_null(self.place));
        Ok(life_event.update(db).await?)
    }
}

/// `UpdateTimelineEvent`.
pub struct UpdateTimelineEvent {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub timeline_event_id: Id,
    pub label: Option<String>,
    pub started_at: String,
}

impl UpdateTimelineEvent {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<timeline_events::Model> {
        let mut v = Validator::new();
        v.max_len("label", self.label.as_deref(), 255);
        let started_at = date_format(&mut v, "started_at", &self.started_at);
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let timeline_event = timeline_event_in_vault(db, checked.vault().id, self.timeline_event_id).await?;

        let mut timeline_event: timeline_events::ActiveModel = timeline_event.into();
        timeline_event.label = Set(value_or_null(self.label));
        timeline_event.started_at = Set(started_at.expect("validated"));
        Ok(timeline_event.update(db).await?)
    }
}

/// `required|date|date_format:Y-m-d`: the date, when `value` is one.
fn date_format(v: &mut Validator, field: &str, value: &str) -> Option<NaiveDate> {
    if value.trim().is_empty() {
        v.fail(field, "required", &[]);
        return None;
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok().filter(|d| d.format("%Y-%m-%d").to_string() == value);
    if date.is_none() {
        v.fail(field, "date_format", &[("format", "Y-m-d")]);
    }
    date
}

/// `$this->vault->timelineEvents()->findOrFail($id)`.
async fn timeline_event_in_vault<C: ConnectionTrait>(
    db: &C,
    vault_id: Uuid,
    timeline_event_id: Id,
) -> ApiResult<timeline_events::Model> {
    timeline_events::Entity::find_by_id(timeline_event_id)
        .filter(timeline_events::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$timelineEvent->lifeEvents()->findOrFail($id)`.
async fn life_event_in_timeline_event<C: ConnectionTrait>(
    db: &C,
    timeline_event: &timeline_events::Model,
    life_event_id: Id,
) -> ApiResult<life_events::Model> {
    life_events::Entity::find_by_id(life_event_id)
        .filter(life_events::Column::TimelineEventId.eq(timeline_event.id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->vault->lifeEventCategories()->findOrFail($lifeEventType->lifeEventCategory->id)`.
async fn category_in_vault<C: ConnectionTrait>(db: &C, vault_id: Uuid, life_event_category_id: Id) -> ApiResult<()> {
    life_event_categories::Entity::find_by_id(life_event_category_id)
        .filter(life_event_categories::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(())
}

/// `$this->account()->currencies()->findOrFail($id)`.
async fn currency_in_account<C: ConnectionTrait>(db: &C, account_id: Uuid, currency_id: Id) -> ApiResult<()> {
    account_currency::Entity::find()
        .filter(account_currency::Column::AccountId.eq(account_id))
        .filter(account_currency::Column::CurrencyId.eq(currency_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(())
}

/// `$this->partipantsCollection`: each participant, a contact of the vault.
async fn participants<C: ConnectionTrait>(
    db: &C,
    checked: &Checked,
    participant_ids: &[Uuid],
) -> ApiResult<Vec<contacts::Model>> {
    let mut participants = Vec::with_capacity(participant_ids.len());
    for id in participant_ids {
        participants.push(contact_in_vault(db, checked.vault().id, *id).await?);
    }
    Ok(participants)
}

/// `dissociateParticipants()`: `$this->lifeEvent->participants()->detach()`.
async fn dissociate_participants<C: ConnectionTrait>(db: &C, life_event: &life_events::Model) -> ApiResult<()> {
    life_event_participants::Entity::delete_many()
        .filter(life_event_participants::Column::LifeEventId.eq(life_event.id))
        .exec(db)
        .await?;
    Ok(())
}

/// `associateParticipants()`: each participant takes part in the life event
/// (`attach`) and in its timeline event (`syncWithoutDetaching`). The pivots
/// have no timestamps.
async fn associate_participants<C: ConnectionTrait>(
    db: &C,
    participants: &[contacts::Model],
    life_event: &life_events::Model,
    timeline_event: &timeline_events::Model,
) -> ApiResult<()> {
    for participant in participants {
        life_event_participants::ActiveModel {
            contact_id: Set(participant.id),
            life_event_id: Set(life_event.id),
            created_at: Set(None),
            updated_at: Set(None),
        }
        .insert(db)
        .await?;

        let attached = timeline_event_participants::Entity::find_by_id((participant.id, timeline_event.id))
            .one(db)
            .await?
            .is_some();
        if !attached {
            timeline_event_participants::ActiveModel {
                contact_id: Set(participant.id),
                timeline_event_id: Set(timeline_event.id),
                created_at: Set(None),
                updated_at: Set(None),
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

/// `updateLastEditedDate()`: each participant's `last_updated_at`.
async fn update_last_edited_date<C: ConnectionTrait>(db: &C, participants: &[contacts::Model]) -> ApiResult<()> {
    for participant in participants {
        let mut participant: contacts::ActiveModel = participant.clone().into();
        participant.last_updated_at = Set(Some(crate::db::now()));
        participant.update(db).await?;
    }
    Ok(())
}
