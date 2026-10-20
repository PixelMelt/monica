//! `app/Domains/Contact/ManageContactImportantDates/Services`.

#[cfg(test)]
mod tests;

use chrono::NaiveDate;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check};
use crate::entities::{contact_feed_items, contact_important_date_types, contact_important_dates, contacts, users};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `CreateContactImportantDate`.
pub struct CreateContactImportantDate {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_important_date_type_id: Option<Id>,
    pub label: String,
    pub day: Option<i32>,
    pub month: Option<i32>,
    pub year: Option<i32>,
}

impl CreateContactImportantDate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_important_dates::Model> {
        let mut v = Validator::new();
        exists_type(db, &mut v, self.contact_important_date_type_id).await?;
        label_rules(&mut v, &self.label);
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

        // make sure the vault matches
        if let Some(id) = self.contact_important_date_type_id {
            find_type(db, checked.vault().id, id).await?;
        }

        let date = contact_important_dates::ActiveModel {
            contact_id: Set(self.contact_id),
            contact_important_date_type_id: Set(self.contact_important_date_type_id),
            label: Set(self.label),
            day: Set(self.day.filter(|d| *d != 0)),
            month: Set(self.month.filter(|m| *m != 0)),
            year: Set(self.year.filter(|y| *y != 0)),
            uuid: Set(Some(Uuid::new())),
            ..Default::default()
        }
        .insert(db)
        .await?;

        update_last_edited_date(db, checked.contact()).await?;
        create_feed_item(db, &checked.author, &date, "important_date_created", true).await?;

        Ok(date)
    }
}

/// `DestroyContactImportantDate`.
pub struct DestroyContactImportantDate {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_important_date_id: Id,
}

impl DestroyContactImportantDate {
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

        let date = find_date(db, checked.contact(), self.contact_important_date_id).await?;

        let mut deleted: contact_important_dates::ActiveModel = date.clone().into();
        deleted.deleted_at = Set(Some(crate::db::now()));
        deleted.update(db).await?;

        update_last_edited_date(db, checked.contact()).await?;

        create_feed_item(db, &checked.author, &date, "important_date_destroyed", false).await
    }
}

/// `UpdateContactImportantDate`.
pub struct UpdateContactImportantDate {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub contact_important_date_id: Id,
    pub contact_important_date_type_id: Option<Id>,
    pub label: String,
    pub day: Option<i32>,
    pub month: Option<i32>,
    pub year: Option<i32>,
}

impl UpdateContactImportantDate {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<contact_important_dates::Model> {
        let mut v = Validator::new();
        exists_type(db, &mut v, self.contact_important_date_type_id).await?;
        label_rules(&mut v, &self.label);
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

        let date = find_date(db, checked.contact(), self.contact_important_date_id).await?;

        // make sure the vault matches
        if let Some(id) = self.contact_important_date_type_id {
            find_type(db, checked.vault().id, id).await?;
        }

        let mut date: contact_important_dates::ActiveModel = date.into();
        date.label = Set(self.label);
        date.contact_important_date_type_id = Set(self.contact_important_date_type_id);
        date.day = Set(self.day.filter(|d| *d != 0));
        date.month = Set(self.month.filter(|m| *m != 0));
        date.year = Set(self.year.filter(|y| *y != 0));
        let date = date.update(db).await?;

        update_last_edited_date(db, checked.contact()).await?;

        create_feed_item(db, &checked.author, &date, "important_date_updated", true).await?;

        Ok(date)
    }
}

/// `exists:contact_important_date_types,id`.
async fn exists_type<C: ConnectionTrait>(db: &C, v: &mut Validator, id: Option<Id>) -> ApiResult<()> {
    if let Some(id) = id
        && contact_important_date_types::Entity::find_by_id(id).one(db).await?.is_none()
    {
        v.fail("contact_important_date_type_id", "exists", &[]);
    }
    Ok(())
}

/// `'label' => 'required|string|max:255'`.
fn label_rules(v: &mut Validator, label: &str) {
    if label.trim().is_empty() {
        v.fail("label", "required", &[]);
    }
    v.max_len("label", Some(label), 255);
}

/// `$this->vault->contactImportantDateTypes()->findOrFail($id)`.
async fn find_type<C: ConnectionTrait>(
    db: &C,
    vault_id: Uuid,
    id: Id,
) -> ApiResult<contact_important_date_types::Model> {
    contact_important_date_types::Entity::find_by_id(id)
        .filter(contact_important_date_types::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->contact->importantDates()->findOrFail($id)`.
async fn find_date<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
    id: Id,
) -> ApiResult<contact_important_dates::Model> {
    contact_important_dates::Entity::live()
        .filter(contact_important_dates::Column::Id.eq(id))
        .filter(contact_important_dates::Column::ContactId.eq(contact.id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->contact->last_updated_at = Carbon::now(); $this->contact->save()`.
async fn update_last_edited_date<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<()> {
    let mut contact: contacts::ActiveModel = contact.clone().into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    contact.update(db).await?;
    Ok(())
}

/// `createFeedItem`; `feedable` is `$this->date->feedItem()->save($feedItem)`.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    date: &contact_important_dates::Model,
    action: &str,
    feedable: bool,
) -> ApiResult<()> {
    let description = format!("{} {}", date.label, format_date(date, author)?.unwrap_or_default());
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author.id)),
        contact_id: Set(date.contact_id),
        action: Set(action.into()),
        description: Set(Some(description)),
        feedable_type: Set(feedable.then(|| "App\\Models\\ContactImportantDate".into())),
        feedable_id: Set(feedable.then_some(date.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// Stand-in for `App\Helpers\ImportantDateHelper::formatDate` until the
/// helpers are ported.
fn format_date(date: &contact_important_dates::Model, user: &users::Model) -> ApiResult<Option<String>> {
    let (day, month, year) = (date.day.unwrap_or(0), date.month.unwrap_or(0), date.year.unwrap_or(0));
    let mut date_as_string = String::new();

    match determine_type(date) {
        Some("full_date") => date_as_string = iso_format(parse(year, month, day)?, &user.date_format),
        Some("year") => date_as_string = year.to_string(),
        Some("month_day") => {
            let date = parse(1900, month, day)?;
            date_as_string = match user.date_format.as_str() {
                "MMM DD, YYYY" => iso_format(date, "MMM DD"),
                "DD MMM YYYY" => iso_format(date, "DD MMM"),
                "YYYY/MM/DD" => iso_format(date, "MM/DD"),
                _ => iso_format(date, "DD/MM"),
            };
        }
        _ => {}
    }

    // case: date is empty. this shouldn't happen, but we'll cover the case
    // nonetheless
    if day == 0 && month == 0 && year == 0 {
        return Ok(None);
    }

    Ok(Some(date_as_string))
}

/// Stand-in for `App\Helpers\ImportantDateHelper::determineType`.
fn determine_type(date: &contact_important_dates::Model) -> Option<&'static str> {
    let set = |v: Option<i32>| v.is_some_and(|v| v != 0);
    let (day, month, year) = (set(date.day), set(date.month), set(date.year));

    // case: date is empty. this shouldn't happen, but we'll cover the case
    // nonetheless
    if !day && !month && !year {
        return None;
    }

    Some(match (day, month, year) {
        (true, true, true) => "full_date",
        (false, false, true) => "year",
        // case: only know the month and day. In this case, we can't calculate
        // the age at all
        (true, true, false) => "month_day",
        _ => "",
    })
}

/// `Carbon::parse("$year-$month-$day")`: PHP takes a month up to 12 and a
/// day up to 31, rolling a day past the month's end over (`2021-2-30` is
/// March 2); anything else throws.
fn parse(year: i32, month: i32, day: i32) -> ApiResult<NaiveDate> {
    let invalid = || ApiError::internal(format!("invalid date {year}-{month}-{day}"));
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(invalid());
    }
    NaiveDate::from_ymd_opt(year, month as u32, 1)
        .and_then(|d| d.checked_add_signed(chrono::TimeDelta::days(i64::from(day) - 1)))
        .ok_or_else(invalid)
}

/// Carbon's `isoFormat` for the tokens of the date format preferences
/// (`YYYY`, `MMM`, `MM`, `DD`), with English month names.
fn iso_format(date: NaiveDate, format: &str) -> String {
    let pattern =
        format.replace('%', "%%").replace("YYYY", "%Y").replace("MMM", "%b").replace("MM", "%m").replace("DD", "%d");
    date.format(&pattern).to_string()
}
