//! `app/Domains/Contact/ManageTasks/Dav`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, ConnectionTrait, QueryFilter};

use super::services::ImportVCalendarContext;
use super::{
    VCalendarResource, carbon_parse, decode_uri, find_contact, format_value, get_contact_in_vault, get_uid, import_uid,
    parse_uuid, route_contact_show, timestamp,
};
use crate::config::Config;
use crate::db::id::Id;
use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::domains::contact::manage_tasks::{CreateContactTask, UpdateContactTask};
use crate::entities::{contact_tasks, contacts, users, vaults};
use crate::error::{ApiError, ApiResult};
use crate::vobject::{Component, Document, Input, Node};

/// `ExportContactTask` (`#[Order(1)]`).
pub struct ExportContactTask;

impl ExportContactTask {
    /// `Auth::user()` is `author`.
    pub async fn export<C: ConnectionTrait>(
        &self,
        db: &C,
        config: &Config,
        author: &users::Model,
        resource: &contact_tasks::Model,
        vcalendar: &mut Document,
    ) -> ApiResult<()> {
        if vcalendar.component("VTODO").is_none() {
            let vtodo = vcalendar.create_component("VTODO", vec![], true)?;
            vcalendar.add(Node::Component(vtodo));
        }

        // The standard for most of these fields can be found on https://datatracker.ietf.org/doc/html/rfc5545

        self.export_timezone(author, vcalendar)?;
        self.export_vtodo(db, config, resource, vcalendar).await
    }

    fn export_timezone(&self, author: &users::Model, vcalendar: &mut Document) -> ApiResult<()> {
        let vtimezone =
            vcalendar.create_component("VTIMEZONE", vec![("TZID", author.timezone.clone().into())], true)?;
        vcalendar.add(Node::Component(vtimezone));
        Ok(())
    }

    async fn export_vtodo<C: ConnectionTrait>(
        &self,
        db: &C,
        config: &Config,
        task: &contact_tasks::Model,
        vcalendar: &mut Document,
    ) -> ApiResult<()> {
        let mut properties = Vec::new();
        let p = |name: &str, value: Input| vcalendar.create_property(name, value, vec![]);

        properties.push(p("UID", task.distant_uuid.clone().or_else(|| task.uuid.map(|u| u.to_string())).into())?);
        properties.push(p("SUMMARY", task.label.clone().into())?);

        properties.push(p("DTSTAMP", timestamp(task.created_at))?);
        properties.push(p("CREATED", timestamp(task.created_at))?);
        properties.push(p("LAST-MODIFIED", timestamp(task.updated_at))?);

        if let Some(description) = task.description.as_ref().filter(|d| !d.is_empty()) {
            properties.push(p("DESCRIPTION", description.clone().into())?);
        }
        if task.due_at.is_some() {
            properties.push(p("DUE", timestamp(task.due_at))?);
        }

        let contact = find_contact(db, task.contact_id).await?;
        properties.push(p("ATTACH", route_contact_show(config, contact.vault_id, contact.id).into())?);

        if task.completed {
            properties.push(p("STATUS", "COMPLETED".into())?);
            properties.push(p("COMPLETED", timestamp(task.completed_at))?);
        }

        let vtodo = vcalendar.component_mut("VTODO").expect("the VTODO was added");
        for property in properties {
            let name = property.name.clone();
            vtodo.set(&name, Node::Property(property));
        }
        Ok(())
    }
}

/// `ImportContactTask::getContactTaskData()`, as the import changes it.
#[derive(Debug, Clone, PartialEq)]
struct TaskData {
    account_id: Uuid,
    vault_id: Uuid,
    author_id: Uuid,
    contact_id: Uuid,
    contact_task_id: Option<Id>,
    label: Option<String>,
    description: Option<String>,
    /// A `Y-m-d H:i:s` date.
    due_at: Option<String>,
}

/// `ImportContactTask` (`#[Order(1)]`).
pub struct ImportContactTask;

impl ImportContactTask {
    pub fn handle(&self, vcalendar: &Document) -> bool {
        vcalendar.component("VTODO").is_some()
    }

    pub async fn import<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCalendarContext,
        vcalendar: &Document,
        _result: Option<VCalendarResource>,
    ) -> ApiResult<Option<VCalendarResource>> {
        let task = self.get_existing_task(db, context, vcalendar).await?;

        let data = self.get_contact_task_data(db, context, task.as_ref()).await?;
        let original = data.clone();

        let vtodo = vcalendar.component("VTODO").expect("handle() found a VTODO");
        let data = self.import_summary(data, vtodo);
        let data = self.import_description(data, vtodo);
        let data = self.import_due(data, vtodo)?;

        let mut updated = false;
        let mut task = match task {
            None => {
                let created = CreateContactTask {
                    account_id: data.account_id,
                    vault_id: data.vault_id,
                    author_id: data.author_id,
                    contact_id: data.contact_id,
                    label: data.label.unwrap_or_default(),
                    description: data.description,
                    due_at: data.due_at,
                }
                .execute(db)
                .await?;
                updated = true;
                created
            }
            Some(task) if data != original => {
                UpdateContactTask {
                    account_id: data.account_id,
                    vault_id: data.vault_id,
                    author_id: data.author_id,
                    contact_id: data.contact_id,
                    contact_task_id: task.id,
                    label: data.label.unwrap_or_default(),
                    description: data.description,
                    due_at: data.due_at,
                }
                .execute(db)
                .await?
            }
            Some(task) => task,
        };
        let mut am: contact_tasks::ActiveModel = task.clone().into();
        if updated {
            // `$task->uuid = $this->getUid($vcalendar)`.
            am.uuid.set_ne(self.get_uid(vcalendar).and_then(|uid| uid.parse().ok().or(task.uuid)));
        }

        updated = self.import_completed(db, &task, &mut am, vtodo).await? || updated;
        updated = self.import_timestamp(&task, &mut am, vtodo)? || updated;

        if context.external && task.distant_uuid.is_none() {
            am.distant_uuid.set_ne(self.get_uid(vcalendar));
            updated = true;
        }

        if updated && am.is_changed() {
            task = am.update(db).await?;
        }

        // ContactTask::withoutTimestamps
        let uri = context.uri.clone();
        if context.external {
            let mut am: contact_tasks::ActiveModel = task.clone().into();
            am.distant_etag.set_ne(context.etag.clone());
            am.distant_uri.set_ne(uri);

            if am.is_changed() {
                am.updated_at = Set(task.updated_at);
                task = am.update(db).await?;
            }
        }

        Ok(Some(VCalendarResource::ContactTask(task)))
    }

    async fn get_existing_task<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCalendarContext,
        vcalendar: &Document,
    ) -> ApiResult<Option<contact_tasks::Model>> {
        let mut task = None;

        if let Some(uri) = &context.uri {
            task = cal_dav_tasks_get_object(db, &context.vault, uri).await?;

            if task.is_none() {
                task = vault_task(db, &context.vault, Condition::all().add(contact_tasks::Column::DistantUri.eq(uri)))
                    .await?;
            }
        }

        if task.is_none()
            && let Some(task_id) = self.get_uid(vcalendar).as_deref().and_then(parse_uuid)
        {
            task =
                vault_task(db, &context.vault, Condition::all().add(contact_tasks::Column::Uuid.eq(task_id))).await?;
        }

        if let Some(t) = &task
            && find_contact(db, t.contact_id).await?.vault_id != context.vault.id
        {
            return Err(ApiError::not_found());
        }

        Ok(task)
    }

    /// Get uid of the task.
    fn get_uid(&self, entry: &Document) -> Option<String> {
        entry.component("VTODO").and_then(get_uid)
    }

    /// `VCalendarImporter::importUid()`.
    pub fn import_uid(&self, context: &ImportVCalendarContext, id: Option<String>, entry: &Document) -> Option<String> {
        import_uid(context.external, id, self.get_uid(entry))
    }

    /// Get contact data.
    async fn get_contact_task_data<C: ConnectionTrait>(
        &self,
        db: &C,
        context: &ImportVCalendarContext,
        task: Option<&contact_tasks::Model>,
    ) -> ApiResult<TaskData> {
        let contact_id = match task {
            Some(t) => t.contact_id,
            None => {
                get_contact_in_vault(db, &context.author, &context.vault)
                    .await?
                    .ok_or_else(|| ApiError::internal("the author has no contact in the vault"))?
                    .id
            }
        };
        Ok(TaskData {
            account_id: context.vault.account_id,
            vault_id: context.vault.id,
            author_id: context.author.id,
            contact_id,
            contact_task_id: task.map(|t| t.id),
            label: task.map(|t| t.label.clone()),
            description: task.and_then(|t| t.description.clone()),
            due_at: task.and_then(|t| t.due_at).map(|d| d.0.format("%Y-%m-%d %H:%M:%S").to_string()),
        })
    }

    fn import_summary(&self, mut data: TaskData, entry: &Component) -> TaskData {
        data.label = format_value(entry.property("SUMMARY").map(ToString::to_string).as_deref());

        data
    }

    fn import_description(&self, mut data: TaskData, entry: &Component) -> TaskData {
        if let Some(description) = entry.property("DESCRIPTION") {
            data.description = format_value(Some(&description.to_string()));
        }

        data
    }

    fn import_due(&self, mut data: TaskData, entry: &Component) -> ApiResult<TaskData> {
        if let Some(due) = entry.property("DUE") {
            data.due_at = Some(carbon_parse(due.get_date_time()?).format("%Y-%m-%d %H:%M:%S").to_string());
        }

        Ok(data)
    }

    fn import_timestamp(
        &self,
        task: &contact_tasks::Model,
        am: &mut contact_tasks::ActiveModel,
        entry: &Component,
    ) -> ApiResult<bool> {
        if task.created_at.is_none() {
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

    async fn import_completed<C: ConnectionTrait>(
        &self,
        db: &C,
        task: &contact_tasks::Model,
        am: &mut contact_tasks::ActiveModel,
        entry: &Component,
    ) -> ApiResult<bool> {
        let completed = entry.property("STATUS").map(ToString::to_string).unwrap_or_default() == "COMPLETED";

        if completed != task.completed {
            if !task.completed {
                am.completed_at = Set(None);
            } else if let Some(completed) = entry.property("COMPLETED") {
                am.completed_at = Set(Some(Timestamp(carbon_parse(completed.get_date_time()?))));
            }

            let mut contact: contacts::ActiveModel = find_contact(db, task.contact_id).await?.into();
            contact.last_updated_at = Set(Some(crate::db::now()));
            contact.update(db).await?;

            return Ok(true);
        }

        Ok(false)
    }
}

/// `$vault->contacts->map(fn ($contact) => $contact->tasks()->where(…)->get())->flatten(1)->filter()->first()`.
async fn vault_task<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    condition: Condition,
) -> ApiResult<Option<contact_tasks::Model>> {
    Ok(contact_tasks::Entity::live()
        .inner_join(contacts::Entity)
        .filter(contacts::Column::VaultId.eq(vault.id))
        .filter(contacts::Column::DeletedAt.is_null())
        .filter(condition)
        .one(db)
        .await?)
}

/// `(new CalDAVTasks)->withVault($vault)->getObject($vault->id, $uri)`: the
/// task whose uuid is the uri's file name.
pub async fn cal_dav_tasks_get_object<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
    uri: &str,
) -> ApiResult<Option<contact_tasks::Model>> {
    let Some(uuid) = decode_uri(uri).parse::<Uuid>().ok() else {
        return Ok(None);
    };
    vault_task(db, vault, Condition::all().add(contact_tasks::Column::Uuid.eq(uuid))).await
}

#[cfg(test)]
#[path = "tests/manage_tasks.rs"]
mod tests;
