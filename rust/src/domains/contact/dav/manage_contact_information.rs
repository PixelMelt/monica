//! `app/Domains/Contact/ManageContactInformation/Dav`.

use sea_orm::sea_query::{Expr, ExprTrait, Func};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use super::services::ImportVCardContext;
use super::{VCardResource, contact, eq_or_null, escape, kind, refresh, str_is};
use crate::db::id::Id;
use crate::domains::contact::manage_contact_information::{
    CreateContactInformation, DestroyContactInformation, UpdateContactInformation,
};
use crate::domains::settings::manage_contact_information_types::CreateContactInformationType;
use crate::entities::{contact_information, contact_information_types, contacts};
use crate::error::{ApiError, ApiResult, Code};
use crate::i18n;
use crate::vobject::{Document, Property};

/// `ExportContactInformation` (`#[Order(40)]`).
pub struct ExportContactInformation;

impl ExportContactInformation {
    pub async fn export<C: ConnectionTrait>(
        &self,
        db: &C,
        resource: &contacts::Model,
        vcard: &mut Document,
    ) -> ApiResult<()> {
        let contact = resource;

        vcard.remove("TEL");
        vcard.remove("EMAIL");
        vcard.remove("X-SOCIAL-PROFILE");
        vcard.remove("IMPP");
        vcard.remove("URL");

        for contact_information in contact_informations(db, contact).await? {
            self.add_contact_information_to_vcard(db, vcard, &contact_information).await?;
        }
        Ok(())
    }

    async fn add_contact_information_to_vcard<C: ConnectionTrait>(
        &self,
        db: &C,
        vcard: &mut Document,
        contact_information: &contact_information::Model,
    ) -> ApiResult<()> {
        let information_type = information_type(db, contact_information).await?;
        let r#type = information_type.r#type.as_deref();

        if str_is(r#type, Some("email"), true) {
            // https://datatracker.ietf.org/doc/html/rfc6350#section-6.4.2
            let mut parameters = vec![];
            if let Some(kind) = &contact_information.kind {
                parameters.push(("TYPE", Some(kind.to_uppercase())));
            }
            if contact_information.pref {
                parameters.push(("PREF", Some("1".to_owned())));
            }

            vcard.add_property("EMAIL", contact_information.data.clone().into(), parameters)?;
        } else if str_is(r#type, Some("phone"), true) {
            // https://datatracker.ietf.org/doc/html/rfc6350#section-6.4.1
            let mut parameters = vec![];
            if let Some(kind) = &contact_information.kind {
                parameters.push(("TYPE", Some(kind.to_uppercase())));
            }
            if contact_information.pref {
                parameters.push(("PREF", Some("1".to_owned())));
            }

            vcard.add_property("TEL", contact_information.data.clone().into(), parameters)?;
        } else if str_is(r#type, Some("IMPP"), true) {
            // https://datatracker.ietf.org/doc/html/rfc4770
            vcard.add_property(
                "IMPP",
                contact_information.data.clone().into(),
                vec![("X-SERVICE-TYPE", type_name(&information_type))],
            )?;
        } else if str_is(r#type, Some("X-SOCIAL-PROFILE"), true) {
            vcard.add_property(
                "X-SOCIAL-PROFILE",
                "".into(),
                vec![("TYPE", type_name(&information_type)), ("X-USER", Some(contact_information.data.clone()))],
            )?;
        } else if let Some(r#type) = r#type.filter(|t| !t.is_empty() && *t != "0") {
            // If field isn't a supported social profile, but still has a protocol, then export it as a url.
            vcard.add_property(
                "URL",
                escape(Some(&format!("{type}{}", contact_information.data))).into(),
                vec![("TYPE", Some(r#type.to_owned()))],
            )?;
        }
        Ok(())
    }
}

/// `$contact->contactInformations`.
async fn contact_informations<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
) -> ApiResult<Vec<contact_information::Model>> {
    Ok(contact_information::Entity::find()
        .filter(contact_information::Column::ContactId.eq(contact.id))
        .all(db)
        .await?)
}

/// `$contactInformation->contactInformationType`.
async fn information_type<C: ConnectionTrait>(
    db: &C,
    contact_information: &contact_information::Model,
) -> ApiResult<contact_information_types::Model> {
    contact_information_types::Entity::find_by_id(contact_information.type_id)
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// The `ContactInformationType::name` accessor: the name, else the translated key.
fn type_name(information_type: &contact_information_types::Model) -> Option<String> {
    match &information_type.name {
        Some(name) if !name.is_empty() => Some(name.clone()),
        _ => information_type.name_translation_key.as_deref().map(|key| i18n::t(&i18n::current(), key, &[])),
    }
}

/// `ImportContactInformation` (`#[Order(40)]`).
pub struct ImportContactInformation;

/// `ImportContactInformation::$keys`.
const KEYS: [&str; 5] = ["EMAIL", "IMPP", "TEL", "URL", "X-SOCIAL-PROFILE"];

impl ImportContactInformation {
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

        let contact_informations = get_contact_informations(db, &contact).await?;

        for key in KEYS {
            self.import_contacts(db, context, vcard, &contact, key, &contact_informations).await?;
        }

        Ok(Some(VCardResource::Contact(Box::new(refresh(db, &contact).await?))))
    }

    async fn import_contacts<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        vcard: &Document,
        contact: &contacts::Model,
        key: &str,
        contact_informations: &[(String, Vec<contact_information::Model>)],
    ) -> ApiResult<()> {
        let card_infos = get_card_informations(vcard, key);
        // `[id, value, parameters.TYPE]`.
        let current: Vec<(Id, String, Option<String>)> = contact_informations
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, infos)| infos.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|info| (info.id, info.data.clone(), info.kind.clone()))
            .collect();

        let mut i = 0;
        while i < card_infos.len() || i < current.len() {
            let step = async {
                if i < card_infos.len() {
                    let Some(type_id) = self.get_type_id(db, context, card_infos[i]).await? else {
                        return Ok(()); // Skip unsupported types
                    };

                    if i < current.len() {
                        self.update_contact_information(db, context, contact, &current[i], card_infos[i], type_id).await
                    } else {
                        self.create_contact_information(db, context, contact, card_infos[i], type_id).await
                    }
                } else if i < current.len() {
                    self.remove_contact_information(db, context, contact, &current[i]).await
                } else {
                    Ok(())
                }
            };
            match step.await {
                Ok(()) => {}
                Err(e) if e.code == Code::Forbidden => {}
                Err(e) => return Err(e),
            }
            i += 1;
        }
        Ok(())
    }

    async fn get_type_id<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        property: &Property,
    ) -> ApiResult<Option<Id>> {
        let account_id = context.vault.account_id;
        let types = || {
            contact_information_types::Entity::find()
                .filter(contact_information_types::Column::AccountId.eq(account_id))
        };
        let upper_type_like = |name: &str| {
            Expr::expr(Func::upper(Expr::col(contact_information_types::Column::Type))).like(format!("%{name}%"))
        };
        let mut name = None;
        let mut r#type = if property.name == "EMAIL" {
            types().all(db).await?.into_iter().find(|t| t.r#type.as_deref() == Some("email"))
        } else if property.name == "TEL" {
            types().all(db).await?.into_iter().find(|t| t.r#type.as_deref() == Some("phone"))
        } else if property.name == "IMPP" {
            name = get_parameter(property, "X-SERVICE-TYPE");
            types()
                .filter(upper_type_like("IMPP"))
                .filter(eq_or_null(contact_information_types::Column::NameTranslationKey, name.clone()))
                .one(db)
                .await?
        } else if property.name == "X-SOCIAL-PROFILE" {
            name = get_parameter(property, "TYPE");
            types()
                .filter(upper_type_like("X-SOCIAL-PROFILE"))
                .filter(eq_or_null(contact_information_types::Column::NameTranslationKey, name.clone()))
                .one(db)
                .await?
        } else {
            // Type not supported
            return Ok(None);
        };

        if r#type.is_none() {
            r#type = types()
                .filter(upper_type_like(&property.name.to_uppercase()))
                .filter(eq_or_null(contact_information_types::Column::Name, name.clone()))
                .one(db)
                .await?;
        }

        if r#type.is_none() {
            r#type = Some(
                CreateContactInformationType {
                    account_id,
                    author_id: context.author.id,
                    r#type: property.name.to_uppercase(),
                    name,
                    name_translation_key: None,
                    protocol: None,
                }
                .execute(db)
                .await?,
            );
        }

        Ok(r#type.map(|t| t.id))
    }

    async fn create_contact_information<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        contact: &contacts::Model,
        data: &Property,
        type_id: Id,
    ) -> ApiResult<()> {
        CreateContactInformation {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id: contact.id,
            contact_information_type_id: type_id,
            contact_information_kind: get_parameter(data, "TYPE"),
            data: get_value(data),
        }
        .execute(db)
        .await?;
        Ok(())
    }

    async fn remove_contact_information<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        contact: &contacts::Model,
        info: &(Id, String, Option<String>),
    ) -> ApiResult<()> {
        DestroyContactInformation {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id: contact.id,
            contact_information_id: info.0,
        }
        .execute(db)
        .await
    }

    async fn update_contact_information<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        contact: &contacts::Model,
        info: &(Id, String, Option<String>),
        data: &Property,
        type_id: Id,
    ) -> ApiResult<()> {
        let kind = get_parameter(data, "TYPE");
        let value = get_value(data);

        if value != info.1 || !str_is(kind.as_deref(), info.2.as_deref(), true) {
            UpdateContactInformation {
                account_id: context.vault.account_id,
                vault_id: context.vault.id,
                author_id: context.author.id,
                contact_id: contact.id,
                contact_information_id: info.0,
                contact_information_type_id: type_id,
                contact_information_kind: kind,
                data: value,
            }
            .execute(db)
            .await?;
        }
        Ok(())
    }
}

/// `getContactInformations()`: `mapToGroups` by vCard property.
async fn get_contact_informations<C: ConnectionTrait>(
    db: &C,
    contact: &contacts::Model,
) -> ApiResult<Vec<(String, Vec<contact_information::Model>)>> {
    let mut groups: Vec<(String, Vec<contact_information::Model>)> = Vec::new();
    for info in contact_informations(db, contact).await? {
        let key = match information_type(db, &info).await?.r#type.as_deref() {
            Some("email") => "EMAIL".to_owned(),
            Some("phone") => "TEL".to_owned(),
            Some(r#type @ ("IMPP" | "X-SOCIAL-PROFILE")) => r#type.to_owned(),
            // For other types, we use a generic key
            _ => "OTHER".to_owned(),
        };
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, infos)) => infos.push(info),
            None => groups.push((key, vec![info])),
        }
    }
    Ok(groups)
}

/// `getValue()`: the `X-USER` of a social profile, else the value.
fn get_value(property: &Property) -> String {
    if property.name == "X-SOCIAL-PROFILE" {
        return get_parameter(property, "X-USER").or_else(|| property.get_value()).unwrap_or_default();
    }

    property.get_value().unwrap_or_default()
}

fn get_card_informations<'a>(vcard: &'a Document, key: &str) -> Vec<&'a Property> {
    vcard.properties(key)
}

/// `getParameter($property, $parameter)`.
fn get_parameter(property: &Property, parameter: &str) -> Option<String> {
    property.parameter(parameter).and_then(|p| p.get_value())
}

#[cfg(test)]
#[path = "tests/manage_contact_information.rs"]
mod tests;
