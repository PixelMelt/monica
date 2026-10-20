//! `app/Domains/Contact/ManageContact/Dav`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use super::services::ImportVCardContext;
use super::{
    VCardResource, contact, decode_uri, eq_or_null, escape, format_value, get_uid, import_uid, kind, parse_uuid,
    php_empty, refresh,
};
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::contact::manage_contact::{CreateContact, UpdateContact};
use crate::domains::contact::manage_contact_addresses::{AssociateAddressToContact, RemoveAddressFromContact};
use crate::domains::contact::manage_labels::{AssignLabel, RemoveLabel};
use crate::domains::contact::manage_reminders::format_contact_name;
use crate::domains::settings::manage_address_types::CreateAddressType;
use crate::domains::vault::manage_addresses::{CreateAddress, UpdateAddress};
use crate::domains::vault::manage_vault_settings::CreateLabel;
use crate::entities::{
    address_types, addresses, companies, contact_address, contact_label, contacts, genders, groups, labels, user_vault,
    users,
};
use crate::error::{ApiError, ApiResult, Code};
use crate::i18n;
use crate::vobject::{Document, Property};

/// `config('dav.default_gender')`.
const DEFAULT_GENDER: &str = "vCard";

/// `ExportAddress` (`#[Order(20)]`).
pub struct ExportAddress;

impl ExportAddress {
    pub async fn export<C: ConnectionTrait>(
        &self,
        db: &C,
        resource: &contacts::Model,
        vcard: &mut Document,
    ) -> ApiResult<()> {
        vcard.remove("ADR");

        let addresses = current_addresses(db, resource).await?;

        for address in addresses {
            let address_type = match address.address_type_id {
                Some(id) => address_types::Entity::find_by_id(id).one(db).await?,
                None => None,
            };
            // https://datatracker.ietf.org/doc/html/rfc6350#section-6.3.1
            vcard.add_property(
                "ADR",
                vec![
                    Some(String::new()),
                    address.line_1,
                    address.line_2,
                    address.city,
                    address.province,
                    address.postal_code,
                    address.country,
                ]
                .into(),
                match address_type {
                    Some(t) => vec![("TYPE", t.r#type)],
                    None => vec![],
                },
            )?;
        }
        Ok(())
    }
}

/// `$contact->addresses()->wherePivot('is_past_address', false)->get()`.
async fn current_addresses<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<Vec<addresses::Model>> {
    Ok(addresses::Entity::find()
        .inner_join(contact_address::Entity)
        .filter(contact_address::Column::ContactId.eq(contact.id))
        .filter(contact_address::Column::IsPastAddress.eq(false))
        .all(db)
        .await?)
}

/// `ExportGender` (`#[Order(10)]`).
pub struct ExportGender;

impl ExportGender {
    pub async fn export<C: ConnectionTrait>(
        &self,
        db: &C,
        resource: &contacts::Model,
        vcard: &mut Document,
    ) -> ApiResult<()> {
        vcard.remove("GENDER");

        let gender = match resource.gender_id {
            Some(id) => genders::Entity::find_by_id(id).one(db).await?,
            None => None,
        };
        let Some(gender) = gender else {
            return Ok(());
        };

        let mut r#type = gender.r#type.clone();
        if r#type.as_deref().is_none_or(php_empty) {
            let locale = i18n::current();
            let name = gender_name(&gender);
            r#type = Some(if name == Some(i18n::t(&locale, "Male", &[])) {
                "M".to_owned()
            } else if name == Some(i18n::t(&locale, "Female", &[])) {
                "F".to_owned()
            } else {
                "O".to_owned()
            });
        }

        // https://datatracker.ietf.org/doc/html/rfc6350#section-6.2.7
        vcard.add_property("GENDER", r#type.into(), vec![])?;
        Ok(())
    }
}

/// The `Gender::name` accessor: the name, else the translated key.
fn gender_name(gender: &genders::Model) -> Option<String> {
    match &gender.name {
        Some(name) if !php_empty(name) => Some(name.clone()),
        _ => gender.name_translation_key.as_deref().map(|key| i18n::t(&i18n::current(), key, &[])),
    }
}

/// `ExportLabels` (`#[Order(40)]`).
pub struct ExportLabels;

impl ExportLabels {
    pub async fn export<C: ConnectionTrait>(
        &self,
        db: &C,
        resource: &contacts::Model,
        vcard: &mut Document,
    ) -> ApiResult<()> {
        // https://datatracker.ietf.org/doc/html/rfc6350#section-6.7.1
        vcard.remove("CATEGORIES");

        let labels = contact_labels(db, resource).await?;
        if !labels.is_empty() {
            vcard.add_property(
                "CATEGORIES",
                labels.into_iter().map(|l| Some(l.name)).collect::<Vec<_>>().into(),
                vec![],
            )?;
        }
        Ok(())
    }
}

/// `$contact->labels`.
async fn contact_labels<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<Vec<labels::Model>> {
    Ok(labels::Entity::find()
        .inner_join(contact_label::Entity)
        .filter(contact_label::Column::ContactId.eq(contact.id))
        .all(db)
        .await?)
}

/// `ExportNames` (`#[Order(1)]`).
pub struct ExportNames;

impl ExportNames {
    /// `$resource->name` is the name formatted for `author` (`Auth::user()`).
    pub fn export(&self, author: &users::Model, resource: &contacts::Model, vcard: &mut Document) -> ApiResult<()> {
        vcard.remove("FN");
        vcard.remove("N");
        vcard.remove("NICKNAME");

        // https://datatracker.ietf.org/doc/html/rfc6350#section-6.2.1
        vcard.add_property("FN", escape(Some(&format_contact_name(author, resource))).into(), vec![])?;

        // https://datatracker.ietf.org/doc/html/rfc6350#section-6.2.2
        vcard.add_property(
            "N",
            vec![
                escape(resource.last_name.as_deref()),
                escape(resource.first_name.as_deref()),
                escape(resource.middle_name.as_deref()),
            ]
            .into(),
            vec![],
        )?;

        if !resource.nickname.as_deref().is_none_or(php_empty) {
            // https://datatracker.ietf.org/doc/html/rfc6350#section-6.2.3
            vcard.add_property("NICKNAME", escape(resource.nickname.as_deref()).into(), vec![])?;
        }
        Ok(())
    }
}

/// `ExportTimestamp` (`#[Order(1000)]`).
pub struct ExportTimestamp;

impl ExportTimestamp {
    pub fn export(&self, resource: &contacts::Model, vcard: &mut Document) -> ApiResult<()> {
        vcard.remove("REV");

        // https://datatracker.ietf.org/doc/html/rfc6350#section-6.7.4
        vcard.set_property("REV", resource.updated_at.map(|t| t.0.format("%Y%m%dT%H%M%SZ").to_string()).into())?;
        Ok(())
    }
}

/// `ExportWorkInformation` (`#[Order(40)]`).
pub struct ExportWorkInformation;

impl ExportWorkInformation {
    pub async fn export<C: ConnectionTrait>(
        &self,
        db: &C,
        resource: &contacts::Model,
        vcard: &mut Document,
    ) -> ApiResult<()> {
        vcard.remove("ORG");
        vcard.remove("TITLE");

        let company = match resource.company_id {
            Some(id) => companies::Entity::find_by_id(id).one(db).await?,
            None => None,
        };
        if let Some(company) = company {
            // https://datatracker.ietf.org/doc/html/rfc6350#section-6.6.4
            vcard.add_property("ORG", escape(Some(&company.name)).into(), vec![])?;
        }

        if !resource.job_position.as_deref().is_none_or(php_empty) {
            // https://datatracker.ietf.org/doc/html/rfc6350#section-6.6.1
            vcard.add_property("TITLE", escape(resource.job_position.as_deref()).into(), vec![])?;
        }
        Ok(())
    }
}

/// `ImportAddress` (`#[Order(40)]`).
pub struct ImportAddress;

impl ImportAddress {
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
        let contact = contact(result)?;

        let addresses = current_addresses(db, &contact).await?;
        let adr = vcard.properties("ADR");

        let mut i = 0;
        while i < adr.len() || i < addresses.len() {
            if i < adr.len() {
                let address_type = self.get_address_type(db, context, adr[i]).await?;

                if i < addresses.len() {
                    self.update_address(db, context, adr[i], &addresses[i], address_type.as_ref()).await?;
                } else {
                    self.create_address(db, context, &contact, adr[i], address_type.as_ref()).await?;
                }
            } else if i < addresses.len() {
                self.remove_address(db, context, &contact, &addresses[i]).await?;
            }
            i += 1;
        }

        Ok(Some(VCardResource::Contact(Box::new(refresh(db, &contact).await?))))
    }

    async fn get_address_type<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        adr: &Property,
    ) -> ApiResult<Option<address_types::Model>> {
        let r#type = adr.parameter("TYPE");

        if let Some(r#type) = r#type {
            let value = r#type.get_value();
            let found = address_types::Entity::find()
                .filter(address_types::Column::AccountId.eq(context.vault.account_id))
                .filter(eq_or_null(address_types::Column::Type, value.clone()))
                .one(db)
                .await?;
            match found {
                Some(t) => return Ok(Some(t)),
                None => {
                    match (CreateAddressType {
                        account_id: context.vault.account_id,
                        author_id: context.author.id,
                        name: value.clone(),
                        name_translation_key: None,
                        r#type: value,
                    })
                    .execute(db)
                    .await
                    {
                        Ok(t) => return Ok(Some(t)),
                        Err(e) if e.code == Code::Forbidden => {
                            // catch
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        Ok(None)
    }

    async fn update_address<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        adr: &Property,
        address: &addresses::Model,
        address_type: Option<&address_types::Model>,
    ) -> ApiResult<()> {
        let parts = adr.get_parts();
        let part = |i: usize| parts.get(i).cloned().flatten();
        UpdateAddress {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            address_id: address.id,
            address_type_id: address_type.map(|t| t.id),
            line_1: part(1),
            line_2: part(2),
            city: part(3),
            province: part(4),
            postal_code: part(5),
            country: part(6),
            latitude: None,
            longitude: None,
        }
        .execute(db)
        .await?;
        Ok(())
    }

    async fn create_address<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        contact: &contacts::Model,
        adr: &Property,
        address_type: Option<&address_types::Model>,
    ) -> ApiResult<()> {
        let parts = adr.get_parts();
        let part = |i: usize| parts.get(i).cloned().flatten();
        let address = addresses::Entity::find()
            .filter(addresses::Column::VaultId.eq(context.vault.id))
            .filter(eq_or_null(addresses::Column::AddressTypeId, address_type.map(|t| t.id)))
            .filter(eq_or_null(addresses::Column::Line1, part(1)))
            .filter(eq_or_null(addresses::Column::Line2, part(2)))
            .filter(eq_or_null(addresses::Column::City, part(3)))
            .filter(eq_or_null(addresses::Column::Province, part(4)))
            .filter(eq_or_null(addresses::Column::PostalCode, part(5)))
            .filter(eq_or_null(addresses::Column::Country, part(6)))
            .one(db)
            .await?;

        let address = match address {
            Some(address) => address,
            None => {
                CreateAddress {
                    account_id: context.vault.account_id,
                    vault_id: context.vault.id,
                    author_id: context.author.id,
                    address_type_id: address_type.map(|t| t.id),
                    line_1: part(1),
                    line_2: part(2),
                    city: part(3),
                    province: part(4),
                    postal_code: part(5),
                    country: part(6),
                    latitude: None,
                    longitude: None,
                }
                .execute(db)
                .await?
            }
        };

        AssociateAddressToContact {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id: contact.id,
            address_id: address.id,
            is_past_address: false,
        }
        .execute(db)
        .await?;
        Ok(())
    }

    async fn remove_address<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        contact: &contacts::Model,
        address: &addresses::Model,
    ) -> ApiResult<()> {
        RemoveAddressFromContact {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id: contact.id,
            address_id: address.id,
        }
        .execute(db)
        .await?;
        Ok(())
    }
}

/// `ImportContact::getContactData()`, and what the import adds to it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct ContactData {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Option<Uuid>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub middle_name: Option<String>,
    pub nickname: Option<String>,
    pub gender_id: Option<Id>,
    pub id: Option<String>,
}

/// `ImportContact` (`#[Order(1)]`).
pub struct ImportContact;

impl ImportContact {
    pub fn handle(&self, vcard: &Document) -> bool {
        kind(vcard) == "individual"
    }

    pub fn can(&self, vcard: &Document) -> bool {
        self.has_fn(vcard) || self.has_nickname(vcard) || self.has_firstname_in_n(vcard)
    }

    pub async fn import<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        vcard: &Document,
        _result: Option<VCardResource>,
    ) -> ApiResult<Option<VCardResource>> {
        let contact = self.get_existing_contact(db, context, vcard).await?;

        let mut data = self.get_contact_data(db, context, contact.as_ref()).await?;
        let original = data.clone();

        data.id = import_uid(context.external, data.id, get_uid(vcard));
        let data = self.import_names(context, data, vcard)?;
        let data = self.import_gender(db, context, data, vcard).await?;

        let mut contact = match contact {
            None => {
                CreateContact {
                    account_id: data.account_id,
                    id: data.id.as_deref().and_then(parse_uuid),
                    vault_id: data.vault_id,
                    author_id: data.author_id,
                    first_name: data.first_name,
                    last_name: data.last_name,
                    middle_name: data.middle_name,
                    nickname: data.nickname,
                    maiden_name: None,
                    prefix: None,
                    suffix: None,
                    gender_id: data.gender_id,
                    pronoun_id: None,
                    template_id: None,
                    listed: true,
                }
                .execute(db)
                .await?
            }
            Some(contact) if data != original => {
                UpdateContact {
                    account_id: data.account_id,
                    vault_id: data.vault_id,
                    author_id: data.author_id,
                    contact_id: contact.id,
                    first_name: data.first_name.unwrap_or_default(),
                    last_name: data.last_name,
                    middle_name: data.middle_name,
                    nickname: data.nickname,
                    maiden_name: None,
                    gender_id: data.gender_id,
                    pronoun_id: None,
                    suffix: None,
                    prefix: None,
                }
                .execute(db)
                .await?
            }
            Some(contact) => contact,
        };

        if context.external && contact.distant_uuid.is_none() {
            let mut am: contacts::ActiveModel = contact.clone().into();
            am.distant_uuid.set_ne(get_uid(vcard));
            if am.is_changed() {
                contact = am.update(db).await?;
            }
        }

        // Contact::withoutTimestamps
        let uri = context.uri.clone();
        if context.external {
            let mut am: contacts::ActiveModel = contact.clone().into();
            am.distant_etag.set_ne(context.etag.clone());
            am.distant_uri.set_ne(uri);

            if am.is_changed() {
                am.updated_at = Set(contact.updated_at);
                contact = am.update(db).await?;
            }
        }

        Ok(Some(VCardResource::Contact(Box::new(contact))))
    }

    async fn get_existing_contact<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        vcard: &Document,
    ) -> ApiResult<Option<contacts::Model>> {
        let mut contact = None;

        if let Some(uri) = &context.uri {
            contact = match card_dav_get_object(db, &context.author, context.vault.id, uri).await? {
                Some(VCardResource::Contact(c)) => Some(*c),
                Some(VCardResource::Group(_)) => return Err(ApiError::internal("the uri is a group, not a contact")),
                None => None,
            };

            if contact.is_none() {
                contact = contacts::Entity::live()
                    .filter(contacts::Column::VaultId.eq(context.vault.id))
                    .filter(contacts::Column::DistantUri.eq(uri))
                    .one(db)
                    .await?;
            }
        }

        if contact.is_none()
            && let Some(contact_id) = get_uid(vcard).as_deref().and_then(parse_uuid)
        {
            contact = contacts::Entity::live()
                .filter(contacts::Column::VaultId.eq(context.vault.id))
                .filter(contacts::Column::Id.eq(contact_id))
                .one(db)
                .await?;
        }

        if let Some(c) = &contact
            && c.vault_id != context.vault.id
        {
            return Err(ApiError::not_found());
        }

        Ok(contact)
    }

    /// Get contact data.
    async fn get_contact_data<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        contact: Option<&contacts::Model>,
    ) -> ApiResult<ContactData> {
        Ok(ContactData {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id: contact.map(|c| c.id),
            first_name: contact.and_then(|c| c.first_name.clone()),
            last_name: contact.and_then(|c| c.last_name.clone()),
            middle_name: contact.and_then(|c| c.middle_name.clone()),
            nickname: contact.and_then(|c| c.nickname.clone()),
            gender_id: match contact {
                Some(c) => c.gender_id,
                None => Some(self.get_gender(db, context, "O").await?.id),
            },
            id: None,
        })
    }

    /// Import names of the contact.
    pub(super) fn import_names(
        &self,
        context: &ImportVCardContext,
        contact_data: ContactData,
        entry: &Document,
    ) -> ApiResult<ContactData> {
        if self.has_firstname_in_n(entry) {
            Ok(self.import_name_from_n(contact_data, entry))
        } else if self.has_fn(entry) {
            Ok(self.import_name_from_fn(context, contact_data, entry))
        } else if self.has_nickname(entry) {
            Ok(self.import_name_from_nickname(contact_data, entry))
        } else {
            Err(ApiError::internal("Check if you can import entry!"))
        }
    }

    fn has_firstname_in_n(&self, entry: &Document) -> bool {
        entry.property("N").is_some_and(|n| !n.get_parts().get(1).cloned().flatten().as_deref().is_none_or(php_empty))
    }

    fn has_nickname(&self, entry: &Document) -> bool {
        !php_empty(&entry.property("NICKNAME").map(ToString::to_string).unwrap_or_default())
    }

    fn has_fn(&self, entry: &Document) -> bool {
        !php_empty(&entry.property("FN").map(ToString::to_string).unwrap_or_default())
    }

    fn import_name_from_n(&self, mut contact_data: ContactData, entry: &Document) -> ContactData {
        let parts = entry.property("N").map(Property::get_parts).unwrap_or_default();
        let part = |i: usize| parts.get(i).cloned().flatten();

        contact_data.last_name = format_value(part(0).as_deref());
        contact_data.first_name = format_value(part(1).as_deref());
        contact_data.middle_name = format_value(part(2).as_deref());
        // prefix [3]
        // suffix [4]

        if let Some(nickname) = entry.property("NICKNAME") {
            contact_data.nickname = format_value(Some(&nickname.to_string()));
        }

        contact_data
    }

    fn import_name_from_nickname(&self, mut contact_data: ContactData, entry: &Document) -> ContactData {
        contact_data.first_name = format_value(entry.property("NICKNAME").map(ToString::to_string).as_deref());

        contact_data
    }

    fn import_name_from_fn(
        &self,
        context: &ImportVCardContext,
        mut contact_data: ContactData,
        entry: &Document,
    ) -> ContactData {
        let full_name = entry.property("FN").map(ToString::to_string).unwrap_or_default();
        let fullname_parts = preg_split_whitespace(&full_name);

        if context.author.name_order.starts_with("%first_name% %last_name%") {
            contact_data.first_name = format_value(Some(fullname_parts[0]));
            if fullname_parts.len() > 1 {
                contact_data.last_name = format_value(Some(fullname_parts[1]));
            }
        } else if context.author.name_order.starts_with("%last_name% %first_name%") {
            contact_data.last_name = format_value(Some(fullname_parts[0]));
            if fullname_parts.len() > 1 {
                contact_data.first_name = format_value(Some(fullname_parts[1]));
            }
        } else {
            contact_data.first_name = format_value(Some(fullname_parts[0]));
        }

        if let Some(nickname) = entry.property("NICKNAME") {
            contact_data.nickname = format_value(Some(&nickname.to_string()));
        }

        contact_data
    }

    /// Import gender of the contact.
    async fn import_gender<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        mut contact_data: ContactData,
        entry: &Document,
    ) -> ApiResult<ContactData> {
        if let Some(gender) = entry.property("GENDER") {
            contact_data.gender_id = Some(self.get_gender(db, context, &gender.to_string()).await?.id);
        }

        Ok(contact_data)
    }

    /// Get or create the gender called "Vcard" that is associated with all
    /// imported contacts.
    async fn get_gender<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        gender_code: &str,
    ) -> ApiResult<genders::Model> {
        let mut gender = self.get_gender_by_type(db, context, gender_code).await?;

        if gender.is_none() {
            let locale = i18n::current();
            gender = match gender_code {
                "M" => match self.get_gender_by_name(db, context, &i18n::t(&locale, "Male", &[])).await? {
                    Some(g) => Some(g),
                    None => self.get_gender_by_name(db, context, DEFAULT_GENDER).await?,
                },
                "F" => match self.get_gender_by_name(db, context, &i18n::t(&locale, "Female", &[])).await? {
                    Some(g) => Some(g),
                    None => self.get_gender_by_name(db, context, DEFAULT_GENDER).await?,
                },
                _ => self.get_gender_by_name(db, context, DEFAULT_GENDER).await?,
            };
        }

        match gender {
            Some(gender) => Ok(gender),
            None => Ok(genders::ActiveModel {
                account_id: Set(context.vault.account_id),
                name: Set(Some(DEFAULT_GENDER.to_owned())),
                r#type: Set(Some("U".to_owned())),
                ..Default::default()
            }
            .insert(db)
            .await?),
        }
    }

    /// Get the gender by name.
    async fn get_gender_by_name<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        name: &str,
    ) -> ApiResult<Option<genders::Model>> {
        let genders = account_genders(db, context).await?;
        Ok(genders.into_iter().find(|g| gender_name(g).as_deref() == Some(name)))
    }

    /// Get the gender by type.
    async fn get_gender_by_type<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        r#type: &str,
    ) -> ApiResult<Option<genders::Model>> {
        let genders = account_genders(db, context).await?;
        Ok(genders.into_iter().find(|g| g.r#type.as_deref().unwrap_or_default() == r#type))
    }
}

/// `$this->account()->genders`.
async fn account_genders<C: ConnectionTrait>(db: &C, context: &ImportVCardContext) -> ApiResult<Vec<genders::Model>> {
    Ok(genders::Entity::find().filter(genders::Column::AccountId.eq(context.vault.account_id)).all(db).await?)
}

/// `preg_split('/\s+/', $value, 2)`.
fn preg_split_whitespace(value: &str) -> Vec<&str> {
    let is_space = |c: char| matches!(c, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r');
    match value.find(is_space) {
        Some(i) => vec![&value[..i], value[i..].trim_start_matches(is_space)],
        None => vec![value],
    }
}

/// `CardDAVBackend::withUser($author)->getObject($collectionId, $uri)`: the
/// contact or group of the uri's file name in one of the author's vaults.
pub async fn card_dav_get_object<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    collection_id: Uuid,
    uri: &str,
) -> ApiResult<Option<VCardResource>> {
    let uuid = decode_uri(uri);

    // `vaults($collectionId)->first()`, else NotEnoughPermissionException (not found).
    let member = user_vault::Entity::find()
        .filter(user_vault::Column::UserId.eq(author.id))
        .filter(user_vault::Column::VaultId.eq(collection_id))
        .one(db)
        .await?;
    let Some(member) = member else {
        return Ok(None);
    };
    let vault_id = member.vault_id;

    if let Some(c) = contacts::Entity::live()
        .filter(contacts::Column::VaultId.eq(vault_id))
        .filter(contacts::Column::DistantUuid.eq(&uuid))
        .one(db)
        .await?
    {
        return Ok(Some(VCardResource::Contact(Box::new(c))));
    }
    if let Some(g) = groups::Entity::live()
        .filter(groups::Column::VaultId.eq(vault_id))
        .filter(groups::Column::DistantUuid.eq(&uuid))
        .one(db)
        .await?
    {
        return Ok(Some(VCardResource::Group(Box::new(g))));
    }
    let Ok(id) = uuid.parse::<Uuid>() else {
        return Ok(None);
    };
    if let Some(c) = contacts::Entity::live()
        .filter(contacts::Column::VaultId.eq(vault_id))
        .filter(contacts::Column::Id.eq(id))
        .one(db)
        .await?
    {
        return Ok(Some(VCardResource::Contact(Box::new(c))));
    }
    Ok(groups::Entity::live()
        .filter(groups::Column::VaultId.eq(vault_id))
        .filter(groups::Column::Uuid.eq(id))
        .one(db)
        .await?
        .map(|g| VCardResource::Group(Box::new(g))))
}

/// `ImportLabels` (`#[Order(40)]`).
pub struct ImportLabels;

impl ImportLabels {
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
        let contact = contact(result)?;

        // `mapWithKeys(fn ($label) => [$label->name => $label])`.
        let mut labels: Vec<(String, labels::Model)> = Vec::new();
        for label in contact_labels(db, &contact).await? {
            match labels.iter_mut().find(|(name, _)| *name == label.name) {
                Some(entry) => entry.1 = label,
                None => labels.push((label.name.clone(), label)),
            }
        }
        let categories = self.get_categories(vcard);

        // `diffKeys`: the categories are keyed 0, 1, …, the labels by name.
        let to_add: Vec<String> = categories
            .iter()
            .enumerate()
            .filter(|(i, _)| !labels.iter().any(|(name, _)| *name == i.to_string()))
            .map(|(_, name)| name.clone().unwrap_or_default())
            .collect();
        let to_remove: Vec<&labels::Model> = labels
            .iter()
            .filter(|(name, _)| !(0..categories.len()).any(|i| *name == i.to_string()))
            .map(|(_, label)| label)
            .collect();

        let mut refresh_contact = false;
        for label in to_remove {
            self.remove_label(db, context, &contact, label).await?;
            refresh_contact = true;
        }
        for name in to_add {
            self.add_label(db, context, &contact, &name).await?;
            refresh_contact = true;
        }

        Ok(Some(VCardResource::Contact(Box::new(if refresh_contact { refresh(db, &contact).await? } else { contact }))))
    }

    fn get_categories(&self, vcard: &Document) -> Vec<Option<String>> {
        vcard.property("CATEGORIES").map(Property::get_parts).unwrap_or_default()
    }

    async fn add_label<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        contact: &contacts::Model,
        name: &str,
    ) -> ApiResult<()> {
        let label = match self.get_label(db, context, name).await? {
            Some(label) => label,
            None => self.create_label(db, context, name).await?,
        };

        AssignLabel {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id: contact.id,
            label_id: label.id,
        }
        .execute(db)
        .await?;
        Ok(())
    }

    async fn remove_label<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        contact: &contacts::Model,
        label: &labels::Model,
    ) -> ApiResult<()> {
        RemoveLabel {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id: contact.id,
            label_id: label.id,
        }
        .execute(db)
        .await?;
        Ok(())
    }

    async fn get_label<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        name: &str,
    ) -> ApiResult<Option<labels::Model>> {
        Ok(labels::Entity::find()
            .filter(labels::Column::VaultId.eq(context.vault.id))
            .filter(labels::Column::Name.eq(name))
            .one(db)
            .await?)
    }

    async fn create_label<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        name: &str,
    ) -> ApiResult<labels::Model> {
        CreateLabel {
            account_id: context.vault.account_id,
            author_id: context.author.id,
            vault_id: context.vault.id,
            name: name.to_owned(),
            description: None,
            bg_color: None,
            text_color: None,
        }
        .execute(db)
        .await
    }
}

#[cfg(test)]
#[path = "tests/manage_contact.rs"]
mod tests;
