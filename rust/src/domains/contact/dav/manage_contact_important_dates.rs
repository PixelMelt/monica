//! `app/Domains/Contact/ManageContactImportantDates/Dav`.

use chrono::{Datelike, Duration, NaiveDate};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter};

use super::services::{ImportVCalendarContext, ImportVCardContext};
use super::{
    VCalendarResource, VCardResource, carbon_parse, contact, decode_uri, find_contact, format_value,
    get_contact_in_vault, get_uid, import_uid, kind, parse_uuid, refresh, route_contact_show, timestamp,
};
use crate::config::Config;
use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::contact::manage_contact_important_dates::{
    CreateContactImportantDate, DestroyContactImportantDate, UpdateContactImportantDate,
};
use crate::domains::contact::manage_reminders::format_contact_name;
use crate::entities::{contact_important_date_types, contact_important_dates, contacts, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::vobject::{self, Component, Document, Input, Node, VCardDateTime};

/// `ContactImportantDate::TYPE_BIRTHDATE`.
const TYPE_BIRTHDATE: &str = "birthdate";

/// `ExportCalendarImportantDates` (`#[Order(40)]`).
pub struct ExportCalendarImportantDates;

impl ExportCalendarImportantDates {
    /// `Auth::user()` is `author`.
    pub async fn export<C: ConnectionTrait>(
        &self,
        db: &C,
        config: &Config,
        author: &users::Model,
        resource: &contact_important_dates::Model,
        vcalendar: &mut Document,
    ) -> ApiResult<()> {
        if vcalendar.component("VEVENT").is_none() {
            let vevent = vcalendar.create_component("VEVENT", vec![], true)?;
            vcalendar.add(Node::Component(vevent));
        }

        self.export_timezone(author, vcalendar)?;
        self.export_date(db, config, author, resource, vcalendar).await
    }

    fn export_timezone(&self, author: &users::Model, vcalendar: &mut Document) -> ApiResult<()> {
        let vtimezone =
            vcalendar.create_component("VTIMEZONE", vec![("TZID", author.timezone.clone().into())], true)?;
        vcalendar.add(Node::Component(vtimezone));
        Ok(())
    }

    async fn export_date<C: ConnectionTrait>(
        &self,
        db: &C,
        config: &Config,
        author: &users::Model,
        important_date: &contact_important_dates::Model,
        vcalendar: &mut Document,
    ) -> ApiResult<()> {
        let mut properties = Vec::new();
        let p = |name: &str, value: Input| vcalendar.create_property(name, value, vec![]);

        properties.push(p(
            "UID",
            important_date.distant_uuid.clone().or_else(|| important_date.uuid.map(|u| u.to_string())).into(),
        )?);
        properties.push(p("SUMMARY", important_date.label.clone().into())?);
        let mut dtstart = p("DTSTART", date(important_date).format("%Y%m%d").to_string().into())?;
        dtstart.set_param("VALUE", Some("DATE".into()))?;
        properties.push(dtstart);
        let mut dtend = p("DTEND", (date(important_date) + Duration::days(1)).format("%Y%m%d").to_string().into())?;
        dtend.set_param("VALUE", Some("DATE".into()))?;
        properties.push(dtend);

        if date_type(db, important_date).await?.and_then(|t| t.internal_type).as_deref() == Some(TYPE_BIRTHDATE) {
            let (month, day) = (php_int(important_date.month), php_int(important_date.day));
            properties.push(p("RRULE", format!("FREQ=YEARLY;BYMONTH={month};BYMONTHDAY={day}").into())?);
        }

        properties.push(p("DTSTAMP", timestamp(important_date.created_at))?);
        properties.push(p("CREATED", timestamp(important_date.created_at))?);
        properties.push(p("LAST-MODIFIED", timestamp(important_date.updated_at))?);

        let contact = find_contact(db, important_date.contact_id).await?;
        let url = route_contact_show(config, contact.vault_id, contact.id);

        // $name = $contact->name;
        properties.push(p("ATTACH", url.clone().into())?);
        properties.push(p(
            "DESCRIPTION",
            i18n::t(
                &i18n::current(),
                "See :name profile :url",
                &[("name", &format_contact_name(author, &contact)), ("url", &url)],
            )
            .into(),
        )?);

        let vevent = vcalendar.component_mut("VEVENT").expect("the VEVENT was added");
        for property in properties {
            let name = property.name.clone();
            vevent.set(&name, Node::Property(property));
        }
        Ok(())
    }
}

/// An integer interpolated in a PHP string (`""` for `null`).
fn php_int(value: Option<i32>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// The `ContactImportantDate::date` attribute: `Carbon::create($year, $month, $day)`,
/// a missing part being today's.
fn date(important_date: &contact_important_dates::Model) -> NaiveDate {
    let today = chrono::Utc::now().date_naive();
    let year = important_date.year.unwrap_or(today.year());
    let month = important_date.month.map_or(today.month(), |m| m as u32);
    let day = important_date.day.map_or(i64::from(today.day()), i64::from);
    let first = NaiveDate::from_ymd_opt(year, month.clamp(1, 12), 1).unwrap_or(today);
    first + Duration::days(day - 1)
}

/// `$importantDate->contactImportantDateType`.
async fn date_type<C: ConnectionTrait>(
    db: &C,
    important_date: &contact_important_dates::Model,
) -> ApiResult<Option<contact_important_date_types::Model>> {
    Ok(match important_date.contact_important_date_type_id {
        Some(id) => contact_important_date_types::Entity::find_by_id(id).one(db).await?,
        None => None,
    })
}

/// `ContactImportantDate::getVCardDate()`.
/// @see https://datatracker.ietf.org/doc/html/rfc6350#section-6.2.5
fn get_vcard_date(important_date: &contact_important_dates::Model) -> String {
    let mut date = match important_date.year.filter(|y| *y != 0) {
        Some(year) => format!("{year:0>2}"),
        None => "--".to_owned(),
    };
    if important_date.month.is_none() && important_date.day.is_none() {
        return date;
    }

    match important_date.month.filter(|m| *m != 0) {
        Some(month) => date.push_str(&format!("{month:0>2}")),
        None => date.push('-'),
    }
    if let Some(day) = important_date.day.filter(|d| *d != 0) {
        date.push_str(&format!("{day:0>2}"));
    }

    date
}

/// `ExportImportantDates` (`#[Order(40)]`).
pub struct ExportImportantDates;

impl ExportImportantDates {
    pub async fn export<C: ConnectionTrait>(
        &self,
        db: &C,
        resource: &contacts::Model,
        vcard: &mut Document,
    ) -> ApiResult<()> {
        vcard.remove("BDAY");

        for important_date in important_dates(db, resource).await? {
            self.add_important_date_to_vcard(db, vcard, &important_date).await?;
        }
        Ok(())
    }

    pub async fn add_important_date_to_vcard<C: ConnectionTrait>(
        &self,
        db: &C,
        vcard: &mut Document,
        important_date: &contact_important_dates::Model,
    ) -> ApiResult<()> {
        if date_type(db, important_date).await?.and_then(|t| t.internal_type).as_deref() == Some(TYPE_BIRTHDATE) {
            // https://datatracker.ietf.org/doc/html/rfc6350#section-6.2.5
            vcard.set_property("BDAY", get_vcard_date(important_date).into())?;
        }
        Ok(())
    }
}

/// `$contact->importantDates`.
async fn important_dates<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
) -> ApiResult<Vec<contact_important_dates::Model>> {
    Ok(contact_important_dates::Entity::live()
        .filter(contact_important_dates::Column::ContactId.eq(contact.id))
        .all(db)
        .await?)
}

/// `ImportCalendarContactImportantDates::getContactData()`, as the import changes it.
#[derive(Debug, Clone, PartialEq)]
struct ImportantDateData {
    account_id: Uuid,
    vault_id: Uuid,
    author_id: Uuid,
    contact_id: Uuid,
    contact_important_date_id: Option<Id>,
    contact_important_date_type_id: Option<Id>,
    label: Option<String>,
    day: Option<i32>,
    month: Option<i32>,
    year: Option<i32>,
}

/// `ImportCalendarContactImportantDates` (`#[Order(1)]`).
pub struct ImportCalendarContactImportantDates;

impl ImportCalendarContactImportantDates {
    pub fn handle(&self, vcalendar: &Document) -> bool {
        vcalendar.component("VEVENT").is_some()
    }

    pub async fn import<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCalendarContext,
        vcalendar: &Document,
        _result: Option<VCalendarResource>,
    ) -> ApiResult<Option<VCalendarResource>> {
        let important_date = self.get_existing_important_date(db, context, vcalendar).await?;

        let data = self.get_contact_data(db, context, important_date.as_ref()).await?;
        let original = data.clone();

        let vevent = vcalendar.component("VEVENT").expect("handle() found a VEVENT");
        let data = self.import_summary(data, vevent);
        let data = self.import_date(data, vevent)?;

        let mut updated = false;
        let mut important_date = match important_date {
            None => {
                let created = CreateContactImportantDate {
                    account_id: data.account_id,
                    vault_id: data.vault_id,
                    author_id: data.author_id,
                    contact_id: data.contact_id,
                    contact_important_date_type_id: data.contact_important_date_type_id,
                    label: data.label.unwrap_or_default(),
                    day: data.day,
                    month: data.month,
                    year: data.year,
                }
                .execute(db)
                .await?;
                updated = true;
                created
            }
            Some(important_date) if data != original => {
                UpdateContactImportantDate {
                    account_id: data.account_id,
                    vault_id: data.vault_id,
                    author_id: data.author_id,
                    contact_id: data.contact_id,
                    contact_important_date_id: important_date.id,
                    contact_important_date_type_id: data.contact_important_date_type_id,
                    label: data.label.unwrap_or_default(),
                    day: data.day,
                    month: data.month,
                    year: data.year,
                }
                .execute(db)
                .await?
            }
            Some(important_date) => important_date,
        };
        let mut am: contact_important_dates::ActiveModel = important_date.clone().into();
        if updated {
            // `$importantDate->uuid = $this->getUid($vcalendar)`.
            am.uuid.set_ne(self.get_uid(vcalendar).and_then(|uid| uid.parse().ok().or(important_date.uuid)));
        }

        updated = self.import_timestamp(&important_date, &mut am, vevent)? || updated;

        if context.external && important_date.distant_uuid.is_none() {
            am.distant_uuid.set_ne(self.get_uid(vcalendar));
            updated = true;
        }

        if updated && am.is_changed() {
            important_date = am.update(db).await?;
        }

        // ContactImportantDate::withoutTimestamps
        let uri = context.uri.clone();
        if context.external {
            let mut am: contact_important_dates::ActiveModel = important_date.clone().into();
            am.distant_etag.set_ne(context.etag.clone());
            am.distant_uri.set_ne(uri);

            if am.is_changed() {
                am.updated_at = Set(important_date.updated_at);
                important_date = am.update(db).await?;
            }
        }

        Ok(Some(VCalendarResource::ContactImportantDate(important_date)))
    }

    async fn get_existing_important_date<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCalendarContext,
        vcalendar: &Document,
    ) -> ApiResult<Option<contact_important_dates::Model>> {
        let mut important_date = None;

        if let Some(uri) = &context.uri {
            important_date = cal_dav_dates_get_object(db, &context.vault, uri).await?;

            if important_date.is_none() {
                important_date = vault_important_date(
                    db,
                    &context.vault,
                    Condition::all().add(contact_important_dates::Column::DistantUri.eq(uri)),
                )
                .await?;
            }
        }

        if important_date.is_none()
            && let Some(important_date_id) = self.get_uid(vcalendar).as_deref().and_then(parse_uuid)
        {
            important_date = vault_important_date(
                db,
                &context.vault,
                Condition::all().add(contact_important_dates::Column::Uuid.eq(important_date_id)),
            )
            .await?;
        }

        if let Some(d) = &important_date
            && find_contact(db, d.contact_id).await?.vault_id != context.vault.id
        {
            return Err(ApiError::not_found());
        }

        Ok(important_date)
    }

    /// Get uid of the task.
    fn get_uid(&self, entry: &Document) -> Option<String> {
        entry.component("VEVENT").and_then(get_uid)
    }

    /// `VCalendarImporter::importUid()`.
    pub fn import_uid(&self, context: &ImportVCalendarContext, id: Option<String>, entry: &Document) -> Option<String> {
        import_uid(context.external, id, self.get_uid(entry))
    }

    /// Get contact data.
    async fn get_contact_data<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCalendarContext,
        important_date: Option<&contact_important_dates::Model>,
    ) -> ApiResult<ImportantDateData> {
        let contact_id = match important_date {
            Some(d) => d.contact_id,
            None => {
                get_contact_in_vault(db, &context.author, &context.vault)
                    .await?
                    .ok_or_else(|| ApiError::internal("the author has no contact in the vault"))?
                    .id
            }
        };
        Ok(ImportantDateData {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id,
            contact_important_date_id: important_date.map(|d| d.id),
            contact_important_date_type_id: important_date.and_then(|d| d.contact_important_date_type_id),
            label: important_date.map(|d| d.label.clone()),
            day: important_date.and_then(|d| d.day),
            month: important_date.and_then(|d| d.month),
            year: important_date.and_then(|d| d.year),
        })
    }

    fn import_summary(&self, mut data: ImportantDateData, entry: &Component) -> ImportantDateData {
        data.label = format_value(entry.property("SUMMARY").map(ToString::to_string).as_deref());

        data
    }

    fn import_date(&self, mut data: ImportantDateData, entry: &Component) -> ApiResult<ImportantDateData> {
        if let Some(dtstart) = entry.property("DTSTART") {
            let date = carbon_parse(dtstart.get_date_time()?);
            data.day = Some(date.day() as i32);
            data.month = Some(date.month() as i32);
            data.year = Some(date.year());
        }

        Ok(data)
    }

    fn import_timestamp(
        &self,
        important_date: &contact_important_dates::Model,
        am: &mut contact_important_dates::ActiveModel,
        entry: &Component,
    ) -> ApiResult<bool> {
        if important_date.created_at.is_none() {
            let mut created_at = None;
            if let Some(dtstamp) = entry.property("DTSTAMP") {
                created_at = Some(carbon_parse(dtstamp.get_date_time()?));
            } else if let Some(created) = entry.property("CREATED") {
                created_at = Some(carbon_parse(created.get_date_time()?));
            }

            if created_at.is_some() {
                am.created_at = Set(created_at.map(Timestamp));
                return Ok(true);
            }
        }

        Ok(false)
    }
}

/// `$vault->contacts->map(fn ($contact) => $contact->importantDates()->where(…)->get())->flatten(1)->filter()->first()`.
async fn vault_important_date<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    condition: Condition,
) -> ApiResult<Option<contact_important_dates::Model>> {
    Ok(contact_important_dates::Entity::live()
        .inner_join(contacts::Entity)
        .filter(contacts::Column::VaultId.eq(vault.id))
        .filter(contacts::Column::DeletedAt.is_null())
        .filter(condition)
        .one(db)
        .await?)
}

/// `(new CalDAVDates)->withVault($vault)->getObject($vault->id, $uri)`: the
/// date whose uuid is the uri's file name.
async fn cal_dav_dates_get_object<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    uri: &str,
) -> ApiResult<Option<contact_important_dates::Model>> {
    let Some(uuid) = decode_uri(uri).parse::<Uuid>().ok() else {
        return Ok(None);
    };
    vault_important_date(db, vault, Condition::all().add(contact_important_dates::Column::Uuid.eq(uuid))).await
}

/// `ImportImportantDates` (`#[Order(40)]`).
pub struct ImportImportantDates;

impl ImportImportantDates {
    pub fn handle(&self, vcard: &Document) -> bool {
        kind(vcard) == "individual"
    }

    pub async fn import<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        vcard: &Document,
        result: Option<VCardResource>,
    ) -> ApiResult<Option<VCardResource>> {
        let birthdate_type = self.get_birthdate_type(db, context).await?;

        let contact = contact(result)?;

        let contact_important_dates = self.get_important_dates(db, &contact).await?;
        let bdays = self.get_bday(vcard)?;

        let in_bdays = |k: &String| bdays.iter().any(|(key, _)| key == k);
        let to_add: Vec<&VCardDateTime> = bdays
            .iter()
            .filter(|(k, _)| !contact_important_dates.iter().any(|(key, _)| key == k))
            .map(|(_, v)| v)
            .collect();
        let to_remove: Vec<_> = contact_important_dates.iter().filter(|(k, _)| !in_bdays(k)).map(|(_, v)| v).collect();
        let intersect: Vec<_> = contact_important_dates.iter().filter(|(k, _)| in_bdays(k)).map(|(_, v)| v).collect();

        let mut refresh_contact = false;
        for important_date in to_remove {
            self.remove_contact_important_date(db, context, &contact, important_date).await?;
            refresh_contact = true;
        }
        for bday in to_add {
            self.create_contact_important_date(db, context, birthdate_type.as_ref(), &contact, bday).await?;
            refresh_contact = true;
        }
        for current in intersect {
            refresh_contact =
                self.update_contact_important_date(db, context, birthdate_type.as_ref(), &contact, current).await?
                    || refresh_contact;
        }

        Ok(Some(VCardResource::Contact(Box::new(if refresh_contact { refresh(db, &contact).await? } else { contact }))))
    }

    /// The contact's birthdates (or dates without a type), keyed by `getVCardDate()`.
    async fn get_important_dates<C: ConnectionTrait>(
        &self,
        db: &C,
        contact: &contacts::Model,
    ) -> ApiResult<Vec<(String, contact_important_dates::Model)>> {
        let mut dates: Vec<(String, contact_important_dates::Model)> = Vec::new();
        for important_date in important_dates(db, contact).await? {
            let date_type = date_type(db, &important_date).await?;
            if !(date_type.is_none() || date_type.and_then(|t| t.internal_type).as_deref() == Some(TYPE_BIRTHDATE)) {
                continue;
            }
            let key = get_vcard_date(&important_date);
            match dates.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = important_date,
                None => dates.push((key, important_date)),
            }
        }
        Ok(dates)
    }

    /// The card's `BDAY`s, keyed by their value.
    fn get_bday(&self, vcard: &Document) -> ApiResult<Vec<(String, VCardDateTime)>> {
        let mut bdays: Vec<(String, VCardDateTime)> = Vec::new();
        for bday in vcard.properties("BDAY") {
            let key = bday.get_value().unwrap_or_default();
            let value = vobject::parse_vcard_date_time(&key)?;
            match bdays.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => bdays.push((key, value)),
            }
        }
        Ok(bdays)
    }

    async fn get_birthdate_type<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
    ) -> ApiResult<Option<contact_important_date_types::Model>> {
        Ok(contact_important_date_types::Entity::find()
            .filter(contact_important_date_types::Column::VaultId.eq(context.vault.id))
            .all(db)
            .await?
            .into_iter()
            .find(|t| t.internal_type.as_deref() == Some(TYPE_BIRTHDATE)))
    }

    async fn create_contact_important_date<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        birthdate_type: Option<&contact_important_date_types::Model>,
        contact: &contacts::Model,
        date: &VCardDateTime,
    ) -> ApiResult<()> {
        let intval = |v: &Option<String>| v.as_deref().map(|v| v.parse().unwrap_or(0));
        CreateContactImportantDate {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id: contact.id,
            contact_important_date_type_id: birthdate_type.map(|t| t.id),
            label: i18n::t(&context.author.locale, "Birthday", &[]),
            day: intval(&date.date),
            month: intval(&date.month),
            year: intval(&date.year),
        }
        .execute(db)
        .await?;
        Ok(())
    }

    async fn update_contact_important_date<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        birthdate_type: Option<&contact_important_date_types::Model>,
        contact: &contacts::Model,
        important_date: &contact_important_dates::Model,
    ) -> ApiResult<bool> {
        if let Some(birthdate_type) = birthdate_type
            && date_type(db, important_date).await?.is_none()
        {
            UpdateContactImportantDate {
                account_id: context.vault.account_id,
                vault_id: context.vault.id,
                author_id: context.author.id,
                contact_id: contact.id,
                contact_important_date_id: important_date.id,
                contact_important_date_type_id: Some(birthdate_type.id),
                label: important_date.label.clone(),
                day: important_date.day,
                month: important_date.month,
                year: important_date.year,
            }
            .execute(db)
            .await?;

            return Ok(true);
        }

        Ok(false)
    }

    async fn remove_contact_important_date<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        contact: &contacts::Model,
        important_date: &contact_important_dates::Model,
    ) -> ApiResult<()> {
        DestroyContactImportantDate {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id: contact.id,
            contact_important_date_id: important_date.id,
        }
        .execute(db)
        .await
    }
}

#[cfg(test)]
#[path = "tests/manage_contact_important_dates.rs"]
mod tests;
