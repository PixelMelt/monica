//! `app/Domains/Contact/Dav/Jobs`.

use chrono::Duration;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use super::services::{BEHAVIOUR_REPLACE, Entry, GetEtag, ImportVCalendar, ImportVCard, ImportVCardResult};
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::sync_tokens;
use crate::error::ApiResult;
use crate::validate::Validator;

/// `config('dav.sync_token_keep_days')`.
const SYNC_TOKEN_KEEP_DAYS: i64 = 30;

/// `CleanSyncToken`.
pub struct CleanSyncToken;

impl CleanSyncToken {
    /// Clean token list.
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let timefix = Timestamp(crate::db::now().0 + Duration::days(-SYNC_TOKEN_KEEP_DAYS));

        let tokens: Vec<(Uuid, String, Timestamp)> = sync_tokens::Entity::find()
            .select_only()
            .column(sync_tokens::Column::UserId)
            .column(sync_tokens::Column::Name)
            .column_as(sync_tokens::Column::Timestamp.max(), "timestamp")
            .group_by(sync_tokens::Column::UserId)
            .group_by(sync_tokens::Column::Name)
            .order_by_asc(sync_tokens::Column::UserId)
            .into_tuple()
            .all(db)
            .await?;
        for (user_id, name, timestamp) in tokens {
            self.handle_user_token(db, timefix, user_id, &name, timestamp).await?;
        }
        Ok(())
    }

    /// Handle tokens for a user.
    async fn handle_user_token<C: ConnectionTrait>(
        &self,
        db: &C,
        timefix: Timestamp,
        user_id: Uuid,
        token_name: &str,
        timestamp: Timestamp,
    ) -> ApiResult<()> {
        let tokens = sync_tokens::Entity::find()
            .filter(sync_tokens::Column::UserId.eq(user_id))
            .filter(sync_tokens::Column::Name.eq(token_name))
            .filter(sync_tokens::Column::Timestamp.lt(timestamp))
            .filter(sync_tokens::Column::Timestamp.lt(timefix))
            .order_by_desc(sync_tokens::Column::Timestamp)
            .all(db)
            .await?;

        for token in tokens {
            // TokenDeleteEvent::dispatch($token): the event has no listener.
            sync_tokens::Entity::delete_by_id(token.id).exec(db).await?;
        }
        Ok(())
    }
}

/// `UpdateVCalendar`.
pub struct UpdateVCalendar {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub uri: String,
    pub etag: Option<String>,
    pub external: Option<bool>,
    pub calendar: String,
}

impl UpdateVCalendar {
    /// Update or create a contact using the VCard data.
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<Option<String>> {
        let checked =
            validate_rules(db, self.account_id, self.author_id, self.vault_id, &self.uri, "calendar", &self.calendar)
                .await?;

        let newtag = self
            .update_calendar(db, &checked, &self.uri, &self.calendar, self.etag.clone(), self.external.unwrap_or(false))
            .await?;

        if let (Some(newtag), Some(etag)) = (&newtag, &self.etag)
            && newtag != etag
        {
            tracing::warn!(
                contacturl = self.uri,
                calendardata = self.calendar,
                "UpdateVCalendar execute wrong etag when updating contact. Expected [{etag}], got [{newtag}]"
            );
        }

        Ok(newtag)
    }

    /// Update the contact with the calendardata.
    async fn update_calendar<C: ConnectionTrait>(
        &self,
        db: &C,
        checked: &Checked,
        uri: &str,
        calendar: &str,
        etag: Option<String>,
        external: bool,
    ) -> ApiResult<Option<String>> {
        let result = async {
            let result = ImportVCalendar {
                account_id: checked.author.account_id,
                author_id: checked.author.id,
                vault_id: checked.vault().id,
                contact_important_date_id: None,
                contact_task_id: None,
                entry: Entry::String(calendar.to_owned()),
                etag,
                uri: Some(uri.to_owned()),
                external: Some(external),
            }
            .execute(db)
            .await?;

            // An ImportVCalendar result never has an `error`.
            GetEtag {
                account_id: checked.author.account_id,
                author_id: checked.author.id,
                vault_id: checked.vault().id,
                vcard: None,
                vcalendar: result,
            }
            .execute(db)
            .await
            .map(Some)
        }
        .await;

        result.inspect_err(|e| {
            tracing::error!(uri, calendardata = calendar, "UpdateVCalendar updateCalendar: {}", e.title);
        })
    }
}

/// `UpdateVCard`.
pub struct UpdateVCard {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub uri: String,
    pub etag: Option<String>,
    pub external: Option<bool>,
    pub card: String,
}

impl UpdateVCard {
    /// Update or create a contact using the VCard data.
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked =
            validate_rules(db, self.account_id, self.author_id, self.vault_id, &self.uri, "card", &self.card).await?;

        crate::i18n::LOCALE
            .scope(checked.author.locale.clone(), async {
                let newtag = self.update_card(db, &checked, &self.uri, &self.card).await?;

                if let (Some(newtag), Some(etag)) = (&newtag, &self.etag)
                    && newtag != etag
                {
                    tracing::warn!(
                        contacturl = self.uri,
                        carddata = self.card,
                        "UpdateVCard execute wrong etag when updating contact. Expected [{etag}], got [{newtag}]"
                    );
                }
                Ok(())
            })
            .await
    }

    /// Update the contact with the carddata.
    async fn update_card<C: ConnectionTrait>(
        &self,
        db: &C,
        checked: &Checked,
        uri: &str,
        card: &str,
    ) -> ApiResult<Option<String>> {
        let result = async {
            let result = ImportVCard {
                account_id: checked.author.account_id,
                author_id: checked.author.id,
                vault_id: checked.vault().id,
                contact_id: None,
                entry: Entry::String(card.to_owned()),
                behaviour: BEHAVIOUR_REPLACE.to_owned(),
                etag: self.etag.clone(),
                uri: Some(uri.to_owned()),
                external: Some(self.external.unwrap_or(false)),
            }
            .execute(db)
            .await?;

            if let ImportVCardResult::Entry(entry) = result {
                return GetEtag {
                    account_id: checked.author.account_id,
                    author_id: checked.author.id,
                    vault_id: checked.vault().id,
                    vcard: Some(entry),
                    vcalendar: None,
                }
                .execute(db)
                .await
                .map(Some);
            }

            Ok(None)
        }
        .await;

        result.inspect_err(|e| {
            tracing::error!(uri, carddata = card, "UpdateVCard updateCard: {}", e.title);
        })
    }
}

/// The `rules()` and `permissions()` of `UpdateVCard` and `UpdateVCalendar`.
async fn validate_rules<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    author_id: Uuid,
    vault_id: Uuid,
    uri: &str,
    field: &str,
    data: &str,
) -> ApiResult<Checked> {
    let mut v = Validator::new();
    if uri.trim().is_empty() {
        v.fail("uri", "required", &[]);
    }
    if data.trim().is_empty() {
        v.fail(field, "required", &[]);
    }
    v.finish()?;
    check(
        db,
        &Check { account_id, author_id, vault: Some((vault_id, Some(VaultPermission::Edit))), ..Default::default() },
    )
    .await
}

#[cfg(test)]
#[path = "tests/jobs.rs"]
mod tests;
