//! `app/Domains/Contact/ManageGroups/Dav`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, QueryFilter};

use super::services::ImportVCardContext;
use super::{VCardResource, escape, format_value, get_uid, import_uid, kind, php_empty};
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::contact::manage_groups::{AddContactToGroup, CreateGroup, RemoveContactFromGroup, UpdateGroup};
use crate::entities::{contact_group, contacts, groups};
use crate::error::{ApiError, ApiResult};
use crate::vobject::{Document, Property};

/// `ExportKind` (`#[Order(1)]`).
pub struct ExportKind;

impl ExportKind {
    pub fn export(&self, _resource: &groups::Model, vcard: &mut Document) -> ApiResult<()> {
        let kind = !vcard.properties("X-ADDRESSBOOKSERVER-KIND").is_empty();

        if kind {
            vcard.remove_child("X-ADDRESSBOOKSERVER-KIND", 0);
            vcard.add_property("X-ADDRESSBOOKSERVER-KIND", "group".into(), vec![])?;

            return Ok(());
        }

        vcard.remove("KIND");
        vcard.add_property("KIND", "group".into(), vec![])?;
        Ok(())
    }
}

/// `ExportMembers` (`#[Order(20)]`).
pub struct ExportMembers;

impl ExportMembers {
    pub async fn export<C: ConnectionTrait>(
        &self,
        db: &C,
        resource: &groups::Model,
        vcard: &mut Document,
    ) -> ApiResult<()> {
        let kind = !vcard.properties("X-ADDRESSBOOKSERVER-KIND").is_empty();

        // https://datatracker.ietf.org/doc/html/rfc6350#section-6.6.5
        self.export_type(db, resource, vcard, if kind { "X-ADDRESSBOOKSERVER-MEMBER" } else { "MEMBER" }).await
    }

    async fn export_type<C: ConnectionTrait>(
        &self,
        db: &C,
        resource: &groups::Model,
        vcard: &mut Document,
        r#type: &str,
    ) -> ApiResult<()> {
        let mut contacts: Vec<String> = group_contacts(db, resource)
            .await?
            .into_iter()
            .map(|contact| contact.distant_uuid.unwrap_or_else(|| contact.id.to_string()))
            .collect();
        contacts.sort();

        let current: Vec<String> = vcard.properties(r#type).into_iter().map(Property::to_string).collect();
        let members: Vec<Option<String>> = current.iter().map(|member| format_value(Some(member))).collect();

        // Add new members
        for contact in &contacts {
            if !members.iter().any(|m| m.as_deref() == Some(contact)) {
                vcard.add_property(r#type, contact.clone().into(), vec![])?;
            }
        }

        // Remove old members
        let old: Vec<usize> = current
            .iter()
            .enumerate()
            .filter(|(_, member)| !format_value(Some(member)).is_some_and(|m| contacts.contains(&m)))
            .map(|(index, _)| index)
            .collect();
        for index in old.into_iter().rev() {
            vcard.remove_child(r#type, index);
        }
        Ok(())
    }
}

/// `$group->contacts`.
async fn group_contacts<C: ConnectionTrait>(db: &C, group: &groups::Model) -> ApiResult<Vec<contacts::Model>> {
    Ok(contacts::Entity::live()
        .inner_join(contact_group::Entity)
        .filter(contact_group::Column::GroupId.eq(group.id))
        .all(db)
        .await?)
}

/// `ExportNames` (`#[Order(10)]`).
pub struct ExportNames;

impl ExportNames {
    pub fn export(&self, resource: &groups::Model, vcard: &mut Document) -> ApiResult<()> {
        vcard.remove("FN");
        vcard.remove("N");

        vcard.add_property("FN", escape(Some(&resource.name)).into(), vec![])?;

        vcard.add_property("N", vec![escape(Some(&resource.name))].into(), vec![])?;
        Ok(())
    }
}

/// `ExportTimestamp` (`#[Order(1000)]`).
pub struct ExportTimestamp;

impl ExportTimestamp {
    pub fn export(&self, resource: &groups::Model, vcard: &mut Document) -> ApiResult<()> {
        vcard.remove("REV");

        vcard.set_property("REV", resource.updated_at.map(|t| t.0.format("%Y%m%dT%H%M%SZ").to_string()).into())?;
        Ok(())
    }
}

/// `ImportGroup::getGroupData()`, and what the import adds to it.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct GroupData {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub group_id: Option<Id>,
    pub name: Option<String>,
    pub id: Option<String>,
}

/// `ImportGroup` (`#[Order(10)]`).
pub struct ImportGroup;

impl ImportGroup {
    pub fn handle(&self, vcard: &Document) -> bool {
        kind(vcard) == "group"
    }

    pub async fn import<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        vcard: &Document,
        _result: Option<VCardResource>,
    ) -> ApiResult<Option<VCardResource>> {
        let group = get_existing_group(db, context, vcard).await?;

        let mut data = self.get_group_data(context, group.as_ref());
        let original = data.clone();

        data.id = import_uid(context.external, data.id, get_uid(vcard));
        let data = self.import_names(data, vcard)?;

        let mut group = match group {
            None => {
                CreateGroup {
                    account_id: data.account_id,
                    vault_id: data.vault_id,
                    author_id: data.author_id,
                    group_type_id: None,
                    name: data.name,
                }
                .execute(db)
                .await?
            }
            Some(group) if data != original => {
                UpdateGroup {
                    account_id: data.account_id,
                    vault_id: data.vault_id,
                    author_id: data.author_id,
                    group_id: group.id,
                    group_type_id: None,
                    name: data.name,
                }
                .execute(db)
                .await?
            }
            Some(group) => group,
        };

        if context.external && group.distant_uuid.is_none() {
            let mut am: groups::ActiveModel = group.clone().into();
            am.distant_uuid.set_ne(get_uid(vcard));
            if am.is_changed() {
                group = am.update(db).await?;
            }
        }

        // Group::withoutTimestamps
        let uri = context.uri.clone();
        if context.external {
            let mut am: groups::ActiveModel = group.clone().into();
            am.distant_etag.set_ne(context.etag.clone());
            am.distant_uri.set_ne(uri);

            if am.is_changed() {
                am.updated_at = Set(group.updated_at);
                group = am.update(db).await?;
            }
        }

        Ok(Some(VCardResource::Group(Box::new(group))))
    }

    /// Get group data.
    fn get_group_data(&self, context: &ImportVCardContext, group: Option<&groups::Model>) -> GroupData {
        GroupData {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            group_id: group.map(|g| g.id),
            name: group.map(|g| g.name.clone()),
            id: None,
        }
    }

    /// Import names of the group.
    pub(super) fn import_names(&self, contact_data: GroupData, entry: &Document) -> ApiResult<GroupData> {
        if self.has_name_in_n(entry) {
            Ok(self.import_name_from_n(contact_data, entry))
        } else if self.has_fn(entry) {
            Ok(self.import_name_from_fn(contact_data, entry))
        } else {
            Err(ApiError::internal("Check if you can import entry!"))
        }
    }

    fn has_name_in_n(&self, entry: &Document) -> bool {
        entry.property("N").is_some_and(|n| !n.get_parts().first().cloned().flatten().as_deref().is_none_or(php_empty))
    }

    fn has_fn(&self, entry: &Document) -> bool {
        !php_empty(&entry.property("FN").map(ToString::to_string).unwrap_or_default())
    }

    fn import_name_from_n(&self, mut contact_data: GroupData, entry: &Document) -> GroupData {
        let parts = entry.property("N").map(Property::get_parts).unwrap_or_default();
        contact_data.name = format_value(parts.first().cloned().flatten().as_deref());

        contact_data
    }

    fn import_name_from_fn(&self, mut contact_data: GroupData, entry: &Document) -> GroupData {
        contact_data.name = Some(entry.property("FN").map(ToString::to_string).unwrap_or_default());

        contact_data
    }
}

/// `getExistingGroup()` (`ImportGroup` and `ImportMembers` have the same one).
async fn get_existing_group<C: ConnectionTrait>(
    db: &C,
    context: &ImportVCardContext,
    vcard: &Document,
) -> ApiResult<Option<groups::Model>> {
    let mut group = None;

    if let Some(uri) = &context.uri {
        group = groups::Entity::live()
            .filter(groups::Column::VaultId.eq(context.vault.id))
            .filter(groups::Column::DistantUri.eq(uri))
            .one(db)
            .await?;
    }

    if group.is_none()
        && let Some(group_id) = get_uid(vcard)
    {
        group = groups::Entity::live()
            .filter(groups::Column::VaultId.eq(context.vault.id))
            .filter(groups::Column::DistantUuid.eq(group_id))
            .one(db)
            .await?;
    }

    if let Some(g) = &group
        && g.vault_id != context.vault.id
    {
        return Err(ApiError::not_found());
    }

    Ok(group)
}

/// `ImportMembers` (`#[Order(11)]`).
pub struct ImportMembers;

impl ImportMembers {
    pub fn handle(&self, vcard: &Document) -> bool {
        kind(vcard) == "group"
    }

    pub async fn import<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        vcard: &Document,
        _result: Option<VCardResource>,
    ) -> ApiResult<Option<VCardResource>> {
        let group = get_existing_group(db, context, vcard).await?;

        if let Some(group) = &group {
            let members = self.import_members(vcard);

            self.update_group_members(db, context, group, &members).await?;
        }

        Ok(group.map(|g| VCardResource::Group(Box::new(g))))
    }

    /// Import members of the group.
    pub(super) fn import_members(&self, entry: &Document) -> Vec<String> {
        let mut members = entry.properties("MEMBER");

        if members.is_empty() {
            members = entry.properties("X-ADDRESSBOOKSERVER-MEMBER");
        }

        members.into_iter().map(|member| format_value(Some(&member.to_string())).unwrap_or_default()).collect()
    }

    /// Update group members.
    pub(super) async fn update_group_members<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCardContext,
        group: &groups::Model,
        members: &[String],
    ) -> ApiResult<()> {
        // Contacts to remove
        let contains = |value: Option<&str>| members.iter().any(|m| m == value.unwrap_or_default());
        let (keep, remove): (Vec<contacts::Model>, Vec<contacts::Model>) = group_contacts(db, group)
            .await?
            .into_iter()
            .partition(|contact| contains(contact.distant_uuid.as_deref()) || contains(Some(&contact.id.to_string())));

        for contact in remove {
            RemoveContactFromGroup {
                account_id: context.vault.account_id,
                vault_id: context.vault.id,
                author_id: context.author.id,
                group_id: group.id,
                contact_id: contact.id,
            }
            .execute(db)
            .await?;
        }

        // Contacts to add
        for member in members.iter().filter(|member| {
            !keep.iter().any(|c| c.distant_uuid.as_deref() == Some(member.as_str()))
                || !keep.iter().any(|c| c.id.to_string() == **member)
        }) {
            let mut contact = contacts::Entity::live().filter(contacts::Column::DistantUuid.eq(member)).one(db).await?;

            if contact.is_none()
                && let Ok(id) = member.parse::<Uuid>()
            {
                contact = contacts::Entity::live().filter(contacts::Column::Id.eq(id)).one(db).await?;
            }
            let Some(contact) = contact else {
                // Contact not found !
                continue;
            };

            AddContactToGroup {
                account_id: context.vault.account_id,
                vault_id: context.vault.id,
                author_id: context.author.id,
                group_id: group.id,
                contact_id: contact.id,
                group_type_role_id: None,
            }
            .execute(db)
            .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/manage_groups.rs"]
mod tests;
