//! Read models that every area embeds, and their builders: the contact
//! hover card (`ContactCardHelper::data`), files and emotions.

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate, Utc};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use utoipa::ToSchema;

use super::base::{Emotion, File};
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::entities::{
    contact_group, contact_important_date_types, contact_important_dates, contacts, emotions, files,
};
use crate::error::ApiResult;

/// A contact with what the hover card shows (today `ContactCardHelper`):
/// age and the groups the contact belongs to.
#[derive(Debug, Serialize, ToSchema)]
pub struct ContactCard {
    #[schema(value_type = uuid::Uuid)]
    pub id: Uuid,
    #[schema(value_type = uuid::Uuid)]
    pub vault_id: Uuid,
    pub first_name: Option<String>,
    pub middle_name: Option<String>,
    pub last_name: Option<String>,
    pub nickname: Option<String>,
    pub maiden_name: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    #[schema(value_type = Option<i64>)]
    pub avatar_file_id: Option<Id>,
    /// False for contacts hidden from lists (e.g. created via a relationship).
    pub listed: bool,
    /// Computed: whole years since the contact's birthdate important
    /// date; null when there is none or it has no year.
    pub age: Option<i64>,
    /// The contact's groups, by name.
    pub groups: Vec<ContactCardGroup>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ContactCardGroup {
    #[schema(value_type = i64)]
    pub id: Id,
    pub name: String,
}

impl ContactCard {
    /// `ContactCardHelper::data` of one contact.
    pub async fn load<C: ConnectionTrait>(db: &C, contact: contacts::Model) -> ApiResult<ContactCard> {
        Ok(Self::load_many(db, vec![contact]).await?.pop().expect("one card per contact"))
    }

    /// `ContactCardHelper::data` of many contacts, in their order, with one
    /// query for the ages and one for the groups.
    pub async fn load_many<C: ConnectionTrait>(db: &C, contacts: Vec<contacts::Model>) -> ApiResult<Vec<ContactCard>> {
        let mut ages = ages(db, &contacts).await?;
        let ids: Vec<Uuid> = contacts.iter().map(|c| c.id).collect();
        let mut groups: HashMap<Uuid, Vec<ContactCardGroup>> = HashMap::new();
        if !ids.is_empty() {
            let rows = contact_group::Entity::find()
                .filter(contact_group::Column::ContactId.is_in(ids))
                .find_also_related(crate::entities::groups::Entity)
                .all(db)
                .await?;
            for (link, group) in rows {
                if let Some(g) = group.filter(|g| g.deleted_at.is_none()) {
                    groups.entry(link.contact_id).or_default().push(ContactCardGroup { id: g.id, name: g.name });
                }
            }
        }
        Ok(contacts
            .into_iter()
            .map(|c| {
                let mut gs = groups.remove(&c.id).unwrap_or_default();
                crate::text::sort_by_collator(&mut gs, |g| g.name.clone());
                ContactCard {
                    age: ages.remove(&c.id).flatten(),
                    groups: gs,
                    id: c.id,
                    vault_id: c.vault_id,
                    first_name: c.first_name,
                    middle_name: c.middle_name,
                    last_name: c.last_name,
                    nickname: c.nickname,
                    maiden_name: c.maiden_name,
                    prefix: c.prefix,
                    suffix: c.suffix,
                    avatar_file_id: c.file_id,
                    listed: c.listed,
                }
            })
            .collect())
    }
}

/// `ImportantDateHelper::getAge` on `today`: whole years since a full date,
/// the current year minus `year` for a year-only date, none for a day and
/// month, or for an empty date.
pub fn age(day: Option<i32>, month: Option<i32>, year: Option<i32>, today: NaiveDate) -> Option<i64> {
    let (day, month, year) = (day.filter(|d| *d != 0), month.filter(|m| *m != 0), year.filter(|y| *y != 0));
    match (day, month, year) {
        (Some(d), Some(m), Some(y)) => {
            // Carbon::parse rolls an impossible day into the next month.
            let first = NaiveDate::from_ymd_opt(y, u32::try_from(m).ok()?, 1)?;
            let born = first + chrono::Duration::days(i64::from(d) - 1);
            let mut years = i64::from(today.year() - born.year());
            if (today.month(), today.day()) < (born.month(), born.day()) {
                years -= 1;
            }
            Some(years)
        }
        // `Carbon::createFromFormat('Y', $year)` is today's date in that year.
        (None, None, Some(y)) => Some(i64::from(today.year() - y)),
        _ => None,
    }
}

/// `Contact::age` of many contacts: the age of each one's birthdate (the
/// important date of its vault's `birthdate` type).
pub async fn ages<C: ConnectionTrait>(db: &C, contacts: &[contacts::Model]) -> ApiResult<HashMap<Uuid, Option<i64>>> {
    let mut out = HashMap::new();
    if contacts.is_empty() {
        return Ok(out);
    }
    let vault_ids: Vec<Uuid> = contacts.iter().map(|c| c.vault_id).collect();
    let types: Vec<Id> = contact_important_date_types::Entity::find()
        .filter(contact_important_date_types::Column::VaultId.is_in(vault_ids))
        .filter(contact_important_date_types::Column::InternalType.eq("birthdate"))
        .all(db)
        .await?
        .into_iter()
        .map(|t| t.id)
        .collect();
    if types.is_empty() {
        return Ok(out);
    }
    let dates = contact_important_dates::Entity::find()
        .filter(contact_important_dates::Column::ContactId.is_in(contacts.iter().map(|c| c.id).collect::<Vec<_>>()))
        .filter(contact_important_dates::Column::ContactImportantDateTypeId.is_in(types))
        .filter(contact_important_dates::Column::DeletedAt.is_null())
        .all(db)
        .await?;
    let today = Utc::now().date_naive();
    for d in dates {
        // `firstWhere`: the first birthdate of the contact.
        out.entry(d.contact_id).or_insert_with(|| age(d.day, d.month, d.year, today));
    }
    Ok(out)
}

impl From<files::Model> for File {
    fn from(f: files::Model) -> Self {
        File {
            id: f.id,
            uuid: f.uuid,
            name: f.name,
            mime_type: f.mime_type,
            size: i64::from(f.size),
            r#type: f.r#type,
            created_at: f.created_at.unwrap_or_else(crate::db::now),
        }
    }
}

impl From<emotions::Model> for Emotion {
    fn from(e: emotions::Model) -> Self {
        Emotion { id: e.id, name: e.name, name_translation_key: e.name_translation_key, r#type: e.r#type }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_ages_like_carbon() {
        let today = NaiveDate::from_ymd_opt(2026, 3, 15).unwrap();
        assert_eq!(age(Some(15), Some(3), Some(2000), today), Some(26));
        assert_eq!(age(Some(16), Some(3), Some(2000), today), Some(25));
        assert_eq!(age(None, None, Some(1990), today), Some(36));
        assert_eq!(age(Some(1), Some(2), None, today), None);
        assert_eq!(age(None, None, None, today), None);
        assert_eq!(age(Some(30), Some(2), Some(2001), today), Some(25), "Feb 30 is March 2");
    }
}
