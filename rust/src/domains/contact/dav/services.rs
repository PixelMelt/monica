//! `app/Domains/Contact/Dav/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use sha2::{Digest, Sha256};

use super::{
    VCalendarResource, VCardResource, find_contact, manage_contact, manage_contact_important_dates,
    manage_contact_information, manage_groups, manage_tasks, php_empty, route_contact_show,
};
use crate::config::Config;
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, Checked, VaultPermission, check};
use crate::entities::{contact_important_dates, contact_tasks, contacts, groups, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;
use crate::vobject::{self, Document, Kind};

/// The `entry` of `ImportVCard` / `ImportVCalendar`: a string, or a parsed document.
#[derive(Debug, Clone)]
pub enum Entry {
    String(String),
    Document(Document),
}

/// `ExportVCalendar`.
pub struct ExportVCalendar {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub contact_important_date_id: Option<Id>,
    pub contact_task_id: Option<Id>,
}

impl ExportVCalendar {
    pub async fn execute<C: ConnectionTrait>(self, db: &C, config: &Config) -> ApiResult<Document> {
        let (checked, resource) = self.validate_rules(db).await?;

        let vcalendar = export_vcalendar(db, config, &checked.author, &resource).await?;

        save_vcalendar(db, resource, vcalendar.serialize()).await?;

        Ok(vcalendar)
    }

    /// `validateRules()`: the rules and permissions, then the resource.
    async fn validate_rules<C: ConnectionTrait>(&self, db: &C) -> ApiResult<(Checked, VCalendarResource)> {
        let mut v = Validator::new();
        if self.contact_important_date_id.is_none() && self.contact_task_id.is_none() {
            v.fail("contact_important_date_id", "required_if", &[("other", "contact task id"), ("value", "null")]);
            v.fail("contact_task_id", "required_if", &[("other", "contact important date id"), ("value", "null")]);
        }
        exists_calendar_resources(db, &mut v, self.contact_important_date_id, self.contact_task_id).await?;
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::View))),
                ..Default::default()
            },
        )
        .await?;

        let resource = if let Some(id) = self.contact_important_date_id {
            let date = find_important_date(db, id).await?;
            VCalendarResource::ContactImportantDate(date)
        } else if let Some(id) = self.contact_task_id {
            let task = find_task(db, id).await?;
            VCalendarResource::ContactTask(task)
        } else {
            return Err(ApiError::not_found());
        };
        if find_contact(db, resource.contact_id()).await?.vault_id != self.vault_id {
            return Err(ApiError::not_found());
        }

        Ok((checked, resource))
    }
}

/// `ExportVCalendar::export()`.
async fn export_vcalendar<C: ConnectionTrait>(
    db: &C,
    config: &Config,
    author: &users::Model,
    resource: &VCalendarResource,
) -> ApiResult<Document> {
    // The standard for most of these fields can be found on https://datatracker.ietf.org/doc/html/rfc5545
    let mut vcalendar = None;
    if let Some(source) = resource.vcalendar().filter(|v| !php_empty(v)) {
        vcalendar = ReadVObject { entry: source.to_owned() }.execute()?;
    }

    let mut vcalendar = match vcalendar {
        Some(vcalendar) => vcalendar,
        // Basic information
        None => Document::vcalendar(vec![
            ("SOURCE", get_vcalendar_source(db, config, resource).await?.into()),
            ("VERSION", "2.0".into()),
        ])?,
    };

    // `exporters($resource::class)`, by `Order`.
    match resource {
        VCalendarResource::ContactImportantDate(date) => {
            manage_contact_important_dates::ExportCalendarImportantDates
                .export(db, config, author, date, &mut vcalendar)
                .await?; // 40
        }
        VCalendarResource::ContactTask(task) => {
            manage_tasks::ExportContactTask.export(db, config, author, task, &mut vcalendar).await?; // 1
        }
    }

    Ok(vcalendar)
}

/// `ExportVCalendar::getSource()`.
async fn get_vcalendar_source<C: ConnectionTrait>(
    db: &C,
    config: &Config,
    resource: &VCalendarResource,
) -> ApiResult<String> {
    let contact = find_contact(db, resource.contact_id()).await?;
    Ok(route_contact_show(config, contact.vault_id, contact.id))
}

/// `$resource->vcalendar = $vcalendar; $resource->save();` without timestamps.
async fn save_vcalendar<C: ConnectionTrait>(
    db: &C,
    resource: VCalendarResource,
    vcalendar: String,
) -> ApiResult<VCalendarResource> {
    Ok(match resource {
        VCalendarResource::ContactImportantDate(date) => {
            let mut am: contact_important_dates::ActiveModel = date.clone().into();
            am.vcalendar.set_ne(Some(vcalendar));
            if !am.is_changed() {
                return Ok(VCalendarResource::ContactImportantDate(date));
            }
            am.updated_at = Set(date.updated_at);
            VCalendarResource::ContactImportantDate(am.update(db).await?)
        }
        VCalendarResource::ContactTask(task) => {
            let mut am: contact_tasks::ActiveModel = task.clone().into();
            am.vcalendar.set_ne(Some(vcalendar));
            if !am.is_changed() {
                return Ok(VCalendarResource::ContactTask(task));
            }
            am.updated_at = Set(task.updated_at);
            VCalendarResource::ContactTask(am.update(db).await?)
        }
    })
}

/// `ExportVCard`.
pub struct ExportVCard {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub contact_id: Option<Uuid>,
    pub group_id: Option<Id>,
}

impl ExportVCard {
    pub async fn execute<C: ConnectionTrait>(self, db: &C, config: &Config) -> ApiResult<Document> {
        let mut v = Validator::new();
        if self.contact_id.is_none() && self.group_id.is_none() {
            v.fail("contact_id", "required_if", &[("other", "group id"), ("value", "null")]);
            v.fail("group_id", "required_if", &[("other", "contact id"), ("value", "null")]);
        }
        if let Some(id) = self.contact_id
            && contacts::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("contact_id", "exists", &[]);
        }
        if let Some(id) = self.group_id
            && groups::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("group_id", "exists", &[]);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::View))),
                contact_id: self.contact_id,
                group_id: self.group_id,
                ..Default::default()
            },
        )
        .await?;

        let obj = if self.contact_id.is_some() {
            VCardResource::Contact(Box::new(checked.contact().clone()))
        } else if self.group_id.is_some() {
            VCardResource::Group(Box::new(checked.group().clone()))
        } else {
            return Err(ApiError::not_found());
        };

        let vcard = export_vcard(db, config, &checked.author, &obj).await?;

        save_vcard(db, obj, vcard.serialize()).await?;

        Ok(vcard)
    }
}

/// `ExportVCard::export()`.
async fn export_vcard<C: ConnectionTrait>(
    db: &C,
    config: &Config,
    author: &users::Model,
    resource: &VCardResource,
) -> ApiResult<Document> {
    // The standard for most of these fields can be found on https://datatracker.ietf.org/doc/html/rfc6350
    let mut vcard = None;
    if let Some(source) = resource.vcard().filter(|v| !php_empty(v)) {
        vcard = ReadVObject { entry: source.to_owned() }.execute()?;
        if let Some(vcard) = &mut vcard
            && vcard.property("UID").is_none()
        {
            let uid = match resource {
                VCardResource::Contact(c) => c.distant_uuid.clone().unwrap_or_else(|| c.id.to_string()),
                VCardResource::Group(g) => {
                    g.distant_uuid.clone().or_else(|| g.uuid.map(|u| u.to_string())).unwrap_or_else(|| g.id.to_string())
                }
            };
            vcard.set_property("UID", uid.into())?;
        }
    }

    let mut vcard = match vcard {
        Some(vcard) => vcard,
        // Basic information
        None => {
            let uid = match resource {
                VCardResource::Contact(c) => c.id.to_string(),
                VCardResource::Group(g) => g.uuid.map_or_else(|| g.id.to_string(), |u| u.to_string()),
            };
            Document::vcard(vec![
                ("UID", uid.into()),
                ("SOURCE", get_vcard_source(config, resource).into()),
                ("VERSION", "4.0".into()),
            ])?
        }
    };

    // `exporters($resource::class)`, by `Order` (same orders in file order).
    match resource {
        VCardResource::Contact(contact) => {
            manage_contact::ExportNames.export(author, contact, &mut vcard)?; // 1
            manage_contact::ExportGender.export(db, contact, &mut vcard).await?; // 10
            manage_contact::ExportAddress.export(db, contact, &mut vcard).await?; // 20
            manage_contact::ExportLabels.export(db, contact, &mut vcard).await?; // 40
            manage_contact::ExportWorkInformation.export(db, contact, &mut vcard).await?; // 40
            manage_contact_important_dates::ExportImportantDates.export(db, contact, &mut vcard).await?; // 40
            manage_contact_information::ExportContactInformation.export(db, contact, &mut vcard).await?; // 40
            manage_contact::ExportTimestamp.export(contact, &mut vcard)?; // 1000
        }
        VCardResource::Group(group) => {
            manage_groups::ExportKind.export(group, &mut vcard)?; // 1
            manage_groups::ExportNames.export(group, &mut vcard)?; // 10
            manage_groups::ExportMembers.export(db, group, &mut vcard).await?; // 20
            manage_groups::ExportTimestamp.export(group, &mut vcard)?; // 1000
        }
    }

    Ok(vcard)
}

/// `ExportVCard::getSource()`.
fn get_vcard_source(config: &Config, resource: &VCardResource) -> String {
    match resource {
        VCardResource::Contact(c) => route_contact_show(config, c.vault_id, c.id),
        VCardResource::Group(g) => format!("{}/vaults/{}/groups/{}", config.url, g.vault_id, g.id),
    }
}

/// `$obj->vcard = $vcard; $obj->save();` without timestamps.
async fn save_vcard<C: ConnectionTrait>(db: &C, resource: VCardResource, vcard: String) -> ApiResult<VCardResource> {
    Ok(match resource {
        VCardResource::Contact(contact) => {
            let mut am: contacts::ActiveModel = (*contact).clone().into();
            am.vcard.set_ne(Some(vcard));
            if !am.is_changed() {
                return Ok(VCardResource::Contact(contact));
            }
            am.updated_at = Set(contact.updated_at);
            VCardResource::Contact(Box::new(am.update(db).await?))
        }
        VCardResource::Group(group) => {
            let mut am: groups::ActiveModel = (*group).clone().into();
            am.vcard.set_ne(Some(vcard));
            if !am.is_changed() {
                return Ok(VCardResource::Group(group));
            }
            am.updated_at = Set(group.updated_at);
            VCardResource::Group(Box::new(am.update(db).await?))
        }
    })
}

/// `GetEtag`.
pub struct GetEtag {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub vcard: Option<VCardResource>,
    pub vcalendar: Option<VCalendarResource>,
}

impl GetEtag {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<String> {
        let mut v = Validator::new();
        if self.vcard.is_none() && self.vcalendar.is_none() {
            v.fail("vcard", "required_if", &[("other", "vcalendar"), ("value", "null")]);
            v.fail("vcalendar", "required_if", &[("other", "vcard"), ("value", "null")]);
        }
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::View))),
                ..Default::default()
            },
        )
        .await?;

        if let Some(entry) = self.vcalendar {
            if find_contact(db, entry.contact_id()).await?.vault_id != checked.vault().id {
                return Err(ApiError::not_found());
            }

            Ok(entry.distant_etag().map_or_else(|| sha256_etag(entry.vcalendar()), str::to_owned))
        } else if let Some(entry) = self.vcard {
            if entry.vault_id() != checked.vault().id {
                return Err(ApiError::not_found());
            }

            Ok(entry.distant_etag().map_or_else(|| sha256_etag(entry.vcard()), str::to_owned))
        } else {
            Err(ApiError::not_found())
        }
    }
}

/// `'"'.hash('sha256', $data).'"'`.
fn sha256_etag(data: Option<&str>) -> String {
    format!("\"{}\"", hex::encode(Sha256::digest(data.unwrap_or_default().as_bytes())))
}

/// The state of an `ImportVCalendar` run that its importers read (`$context`).
pub struct ImportVCalendarContext {
    pub author: users::Model,
    pub vault: vaults::Model,
    pub external: bool,
    /// `$data['uri']`.
    pub uri: Option<String>,
    /// `$data['etag']`.
    pub etag: Option<String>,
}

/// `ImportVCalendar`.
pub struct ImportVCalendar {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub contact_important_date_id: Option<Id>,
    pub contact_task_id: Option<Id>,
    pub entry: Entry,
    pub etag: Option<String>,
    pub uri: Option<String>,
    pub external: Option<bool>,
}

impl ImportVCalendar {
    /// The imported date or task, `None` when the calendar can't be imported.
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<Option<VCalendarResource>> {
        let mut v = Validator::new();
        exists_calendar_resources(db, &mut v, self.contact_important_date_id, self.contact_task_id).await?;
        entry_rule(&mut v, &self.entry, Kind::VCalendar, "a VCalendar");
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
        if let Some(id) = self.contact_important_date_id {
            let date = find_important_date(db, id).await?;
            if find_contact(db, date.contact_id).await?.vault_id != self.vault_id {
                return Err(ApiError::not_found());
            }
        } else if let Some(id) = self.contact_task_id {
            let task = find_task(db, id).await?;
            if find_contact(db, task.contact_id).await?.vault_id != self.vault_id {
                return Err(ApiError::not_found());
            }
        }

        let context = ImportVCalendarContext {
            author: checked.author,
            vault: checked.vault.expect("Check::vault was set"),
            external: self.external.unwrap_or(false),
            uri: self.uri,
            etag: self.etag,
        };

        process_vcalendar(db, &context, self.entry).await
    }
}

/// `ImportVCalendar::process()`.
async fn process_vcalendar<C: ConnectionTrait>(
    db: &C,
    context: &ImportVCalendarContext,
    entry: Entry,
) -> ApiResult<Option<VCalendarResource>> {
    let (entry, vcalendar) = get_entry(entry)?;

    let Some(entry) = entry else {
        return Ok(None);
    };
    if entry.kind != Kind::VCalendar {
        return Err(ApiError::internal("the entry is not a VCalendar"));
    }

    process_vcalendar_entry(db, context, &entry, vcalendar).await
}

/// `ImportVCalendar::processEntry()`.
async fn process_vcalendar_entry<C: ConnectionTrait>(
    db: &C,
    context: &ImportVCalendarContext,
    entry: &Document,
    vcalendar: String,
) -> ApiResult<Option<VCalendarResource>> {
    if !can_import_current_vcalendar(entry) {
        return Ok(None);
    }

    let Some(result) = import_vcalendar_entry(db, context, entry).await? else {
        return Ok(None);
    };

    // Save vcalendar content
    Ok(Some(save_vcalendar(db, result, vcalendar).await?))
}

/// The `ImportVCalendarResource` classes, by `Order`.
#[derive(Debug, Clone, Copy)]
enum VCalendarImporter {
    /// `ImportCalendarContactImportantDates`.
    CalendarContactImportantDates,
    /// `ImportContactTask`.
    ContactTask,
}

const VCALENDAR_IMPORTERS: [VCalendarImporter; 2] =
    [VCalendarImporter::CalendarContactImportantDates, VCalendarImporter::ContactTask];

impl VCalendarImporter {
    fn handle(self, vcalendar: &Document) -> bool {
        match self {
            VCalendarImporter::CalendarContactImportantDates => {
                manage_contact_important_dates::ImportCalendarContactImportantDates.handle(vcalendar)
            }
            VCalendarImporter::ContactTask => manage_tasks::ImportContactTask.handle(vcalendar),
        }
    }

    /// `VCalendarImporter::can()`.
    fn can(self, _vcalendar: &Document) -> bool {
        true
    }

    async fn import<C: ConnectionTrait>(
        self,
        db: &C,
        context: &ImportVCalendarContext,
        vcalendar: &Document,
        result: Option<VCalendarResource>,
    ) -> ApiResult<Option<VCalendarResource>> {
        match self {
            VCalendarImporter::CalendarContactImportantDates => {
                manage_contact_important_dates::ImportCalendarContactImportantDates
                    .import(db, context, vcalendar, result)
                    .await
            }
            VCalendarImporter::ContactTask => {
                manage_tasks::ImportContactTask.import(db, context, vcalendar, result).await
            }
        }
    }
}

/// `ImportVCalendar::canImportCurrentEntry()`.
fn can_import_current_vcalendar(entry: &Document) -> bool {
    let importers: Vec<VCalendarImporter> = VCALENDAR_IMPORTERS.into_iter().filter(|i| i.handle(entry)).collect();

    if importers.is_empty() {
        return false;
    }

    importers.into_iter().all(|i| i.can(entry))
}

/// `ImportVCalendar::importEntry()`.
async fn import_vcalendar_entry<C: ConnectionTrait>(
    db: &C,
    context: &ImportVCalendarContext,
    entry: &Document,
) -> ApiResult<Option<VCalendarResource>> {
    let mut result = None;

    for importer in VCALENDAR_IMPORTERS.into_iter().filter(|i| i.handle(entry)) {
        result = importer.import(db, context, entry, result).await?;
    }

    Ok(result)
}

/// The result of `ImportVCard`: `['error' => …, 'reason' => …]` or
/// `['id' => …, 'entry' => …]`.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportVCardResult {
    Error { error: &'static str, reason: &'static str },
    Entry(VCardResource),
}

/// `ImportVCard::BEHAVIOUR_ADD`.
pub const BEHAVIOUR_ADD: &str = "behaviour_add";

/// `ImportVCard::BEHAVIOUR_REPLACE`.
pub const BEHAVIOUR_REPLACE: &str = "behaviour_replace";

const ERROR_PARSER: &str = "ERROR_PARSER";

const ERROR_CARD_NOT_IMPORTABLE: &str = "ERROR_CARD_NOT_IMPORTABLE";

/// `ImportVCard::$errorResults` (`ERROR_CONTACT_EXIST` is never returned).
fn error_result(error: &'static str) -> ImportVCardResult {
    let reason = match error {
        ERROR_PARSER => "import_vcard_parse_error",
        ERROR_CARD_NOT_IMPORTABLE => "import_vcard_not_importable",
        _ => "import_vcard_contact_exist",
    };
    ImportVCardResult::Error { error, reason }
}

/// The state of an `ImportVCard` run that its importers read (`$context`).
pub struct ImportVCardContext {
    pub author: users::Model,
    pub vault: vaults::Model,
    pub external: bool,
    /// `$data['uri']`.
    pub uri: Option<String>,
    /// `$data['etag']`.
    pub etag: Option<String>,
}

/// `ImportVCard`.
pub struct ImportVCard {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub contact_id: Option<Uuid>,
    pub entry: Entry,
    pub behaviour: String,
    pub etag: Option<String>,
    pub uri: Option<String>,
    pub external: Option<bool>,
}

impl ImportVCard {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<ImportVCardResult> {
        let mut v = Validator::new();
        if let Some(id) = self.contact_id
            && contacts::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("contact_id", "exists", &[]);
        }
        entry_rule(&mut v, &self.entry, Kind::VCard, "a VCard");
        if self.behaviour.trim().is_empty() {
            v.fail("behaviour", "required", &[]);
        } else {
            v.one_of("behaviour", Some(&self.behaviour), &[BEHAVIOUR_ADD, BEHAVIOUR_REPLACE]);
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

        let context = ImportVCardContext {
            author: checked.author,
            vault: checked.vault.expect("Check::vault was set"),
            external: self.external.unwrap_or(false),
            uri: self.uri,
            etag: self.etag,
        };

        process_vcard(db, &context, self.entry).await
    }
}

/// `'entry' => ['required', fn …]`: a string or a document of the right kind.
fn entry_rule(v: &mut Validator, entry: &Entry, kind: Kind, object: &str) {
    match entry {
        Entry::String(s) if s.trim().is_empty() => v.fail("entry", "required", &[]),
        Entry::String(_) => {}
        Entry::Document(d) if d.kind == kind => {}
        Entry::Document(_) => {
            v.add("entry", "entry", format!("entry must be a string, a resource, or {object} object."))
        }
    }
}

/// `ImportVCard::process()`.
async fn process_vcard<C: ConnectionTrait>(
    db: &C,
    context: &ImportVCardContext,
    entry: Entry,
) -> ApiResult<ImportVCardResult> {
    let (entry, vcard) = get_entry(entry)?;

    let Some(entry) = entry else {
        return Ok(error_result(ERROR_PARSER));
    };
    if entry.kind != Kind::VCard {
        return Err(ApiError::internal("the entry is not a VCard"));
    }

    process_vcard_entry(db, context, &entry, vcard).await
}

/// `getEntry()` of `ImportVCard` and `ImportVCalendar`: the document and its text.
fn get_entry(entry: Entry) -> ApiResult<(Option<Document>, String)> {
    match entry {
        Entry::String(s) => {
            let document = ReadVObject { entry: s.clone() }.execute()?;
            Ok((document, s))
        }
        Entry::Document(d) => {
            let text = d.serialize();
            Ok((Some(d), text))
        }
    }
}

/// `ImportVCard::processEntry()`.
async fn process_vcard_entry<C: ConnectionTrait>(
    db: &C,
    context: &ImportVCardContext,
    entry: &Document,
    vcard: String,
) -> ApiResult<ImportVCardResult> {
    if !can_import_current_entry(entry) {
        return Ok(error_result(ERROR_CARD_NOT_IMPORTABLE));
    }

    process_entry_card(db, context, entry, vcard).await
}

/// `ImportVCard::processEntryCard()`.
async fn process_entry_card<C: ConnectionTrait>(
    db: &C,
    context: &ImportVCardContext,
    entry: &Document,
    vcard: String,
) -> ApiResult<ImportVCardResult> {
    let Some(result) = import_entry(db, context, entry).await? else {
        return Ok(error_result(ERROR_CARD_NOT_IMPORTABLE));
    };

    // Save vcard content
    let result = save_vcard(db, result, vcard).await?;

    Ok(ImportVCardResult::Entry(result))
}

/// The `ImportVCardResource` classes, by `Order` (same orders in file order).
#[derive(Debug, Clone, Copy)]
enum VCardImporter {
    /// `ImportContact`.
    Contact,
    /// `ImportGroup`.
    Group,
    /// `ImportMembers`.
    Members,
    /// `ImportAddress`.
    Address,
    /// `ImportLabels`.
    Labels,
    /// `ImportImportantDates`.
    ImportantDates,
    /// `ImportContactInformation`.
    ContactInformation,
}

const VCARD_IMPORTERS: [VCardImporter; 7] = [
    VCardImporter::Contact,            // 1
    VCardImporter::Group,              // 10
    VCardImporter::Members,            // 11
    VCardImporter::Address,            // 40
    VCardImporter::Labels,             // 40
    VCardImporter::ImportantDates,     // 40
    VCardImporter::ContactInformation, // 40
];

impl VCardImporter {
    fn handle(self, vcard: &Document) -> bool {
        match self {
            VCardImporter::Contact => manage_contact::ImportContact.handle(vcard),
            VCardImporter::Group => manage_groups::ImportGroup.handle(vcard),
            VCardImporter::Members => manage_groups::ImportMembers.handle(vcard),
            VCardImporter::Address => manage_contact::ImportAddress.handle(vcard),
            VCardImporter::Labels => manage_contact::ImportLabels.handle(vcard),
            VCardImporter::ImportantDates => manage_contact_important_dates::ImportImportantDates.handle(vcard),
            VCardImporter::ContactInformation => manage_contact_information::ImportContactInformation.handle(vcard),
        }
    }

    /// `can()`: `ImportContact`'s, else `Importer::can()`.
    fn can(self, vcard: &Document) -> bool {
        match self {
            VCardImporter::Contact => manage_contact::ImportContact.can(vcard),
            _ => true,
        }
    }

    async fn import<C: ConnectionTrait>(
        self,
        db: &C,
        context: &ImportVCardContext,
        vcard: &Document,
        result: Option<VCardResource>,
    ) -> ApiResult<Option<VCardResource>> {
        match self {
            VCardImporter::Contact => manage_contact::ImportContact.import(db, context, vcard, result).await,
            VCardImporter::Group => manage_groups::ImportGroup.import(db, context, vcard, result).await,
            VCardImporter::Members => manage_groups::ImportMembers.import(db, context, vcard, result).await,
            VCardImporter::Address => manage_contact::ImportAddress.import(db, context, vcard, result).await,
            VCardImporter::Labels => manage_contact::ImportLabels.import(db, context, vcard, result).await,
            VCardImporter::ImportantDates => {
                manage_contact_important_dates::ImportImportantDates.import(db, context, vcard, result).await
            }
            VCardImporter::ContactInformation => {
                manage_contact_information::ImportContactInformation.import(db, context, vcard, result).await
            }
        }
    }
}

/// `ImportVCard::canImportCurrentEntry()`.
fn can_import_current_entry(entry: &Document) -> bool {
    let importers: Vec<VCardImporter> = VCARD_IMPORTERS.into_iter().filter(|i| i.handle(entry)).collect();

    if importers.is_empty() {
        return false;
    }

    importers.into_iter().all(|i| i.can(entry))
}

/// `ImportVCard::importEntry()`.
async fn import_entry<C: ConnectionTrait>(
    db: &C,
    context: &ImportVCardContext,
    entry: &Document,
) -> ApiResult<Option<VCardResource>> {
    let mut result = None;

    for importer in VCARD_IMPORTERS.into_iter().filter(|i| i.handle(entry)) {
        result = importer.import(db, context, entry, result).await?;
    }

    Ok(result)
}

/// `ReadVObject`.
pub struct ReadVObject {
    pub entry: String,
}

impl ReadVObject {
    /// The document, `None` when it can't be parsed.
    pub fn execute(self) -> ApiResult<Option<Document>> {
        let mut v = Validator::new();
        if self.entry.trim().is_empty() {
            v.fail("entry", "required", &[]);
        }
        v.finish()?;

        match vobject::read(&self.entry, vobject::OPTION_FORGIVING + vobject::OPTION_IGNORE_INVALID_LINES) {
            Ok(document) => Ok(Some(document)),
            Err(vobject::Error::Parse(_)) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// `exists:contact_important_dates,id` and `exists:contact_tasks,id`.
async fn exists_calendar_resources<C: ConnectionTrait>(
    db: &C,
    v: &mut Validator,
    contact_important_date_id: Option<Id>,
    contact_task_id: Option<Id>,
) -> ApiResult<()> {
    if let Some(id) = contact_important_date_id
        && contact_important_dates::Entity::find_by_id(id).one(db).await?.is_none()
    {
        v.fail("contact_important_date_id", "exists", &[]);
    }
    if let Some(id) = contact_task_id
        && contact_tasks::Entity::find_by_id(id).one(db).await?.is_none()
    {
        v.fail("contact_task_id", "exists", &[]);
    }
    Ok(())
}

/// `ContactImportantDate::find($id)`; a miss is a 404 (the PHP reads `->contact` on it).
async fn find_important_date<C: ConnectionTrait>(db: &C, id: Id) -> ApiResult<contact_important_dates::Model> {
    contact_important_dates::Entity::live()
        .filter(contact_important_dates::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `ContactTask::find($id)`; a miss is a 404 (the PHP reads `->contact` on it).
async fn find_task<C: ConnectionTrait>(db: &C, id: Id) -> ApiResult<contact_tasks::Model> {
    contact_tasks::Entity::live()
        .filter(contact_tasks::Column::Id.eq(id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

#[cfg(test)]
#[path = "tests/services.rs"]
mod tests;
