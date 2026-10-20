//! `app/Domains/Vault/ManageVault/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::domains::vault::manage_vault_important_date_types::CreateContactImportantDateType;
use crate::entities::{
    contacts, files, life_event_categories, life_event_types, mood_tracking_parameters, templates, user_vault, users,
    vault_quick_facts_templates, vaults,
};
use crate::error::ApiResult;
use crate::i18n;
use crate::validate::Validator;

/// `CreateVault`.
pub struct CreateVault {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub template_id: Option<Id>,
    pub r#type: String,
    pub name: String,
    pub description: Option<String>,
}

impl CreateVault {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<vaults::Model> {
        let mut v = Validator::new();
        if let Some(id) = self.template_id
            && templates::Entity::find_by_id(id).one(db).await?.is_none()
        {
            v.fail("template_id", "exists", &[]);
        }
        if self.r#type.trim().is_empty() {
            v.fail("type", "required", &[]);
        }
        v.max_len("type", Some(&self.r#type), 255);
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        v.max_len("description", self.description.as_deref(), 255);
        v.finish()?;
        let checked =
            check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() }).await?;
        let author = checked.author;
        let account_id = self.account_id;

        let vault = self.create_vault(db, &author).await?;
        create_user_contact(db, &author, &vault).await?;
        populate_default_contact_important_date_types(db, account_id, &author, &vault).await?;
        populate_mood_tracking_parameters(db, &vault).await?;
        populate_default_life_event_categories(db, &vault).await?;
        populate_default_quick_vault_template_entries(db, &vault).await?;

        Ok(vault)
    }

    async fn create_vault<C: ConnectionTrait>(self, db: &C, author: &users::Model) -> ApiResult<vaults::Model> {
        // the vault default's template should be the first template in the
        // account, if it exists
        let template = templates::Entity::find()
            .filter(templates::Column::AccountId.eq(author.account_id))
            .order_by_asc(templates::Column::Id)
            .one(db)
            .await?;

        Ok(vaults::ActiveModel {
            id: Set(Uuid::new()),
            account_id: Set(self.account_id),
            r#type: Set(self.r#type),
            name: Set(self.name),
            description: Set(value_or_null(self.description)),
            default_template_id: Set(template.map(|t| t.id)),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }
}

async fn create_user_contact<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    vault: &vaults::Model,
) -> ApiResult<()> {
    let contact = contacts::ActiveModel {
        id: Set(Uuid::new()),
        vault_id: Set(vault.id),
        first_name: Set(author.first_name.clone()),
        last_name: Set(author.last_name.clone()),
        can_be_deleted: Set(false),
        template_id: Set(vault.default_template_id),
        ..Default::default()
    }
    .insert(db)
    .await?;

    user_vault::ActiveModel {
        vault_id: Set(vault.id),
        user_id: Set(author.id),
        permission: Set(VaultPermission::Manage as i32),
        contact_id: Set(contact.id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn populate_default_contact_important_date_types<C: ConnectionTrait>(
    db: &C,
    account_id: Uuid,
    author: &users::Model,
    vault: &vaults::Model,
) -> ApiResult<()> {
    CreateContactImportantDateType {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        label: i18n::t(&i18n::current(), "Birthdate", &[]),
        internal_type: Some("birthdate".into()),
        can_be_deleted: Some(false),
    }
    .execute(db)
    .await?;

    CreateContactImportantDateType {
        account_id,
        author_id: author.id,
        vault_id: vault.id,
        label: i18n::t(&i18n::current(), "Deceased date", &[]),
        internal_type: Some("deceased_date".into()),
        can_be_deleted: Some(false),
    }
    .execute(db)
    .await?;
    Ok(())
}

async fn populate_mood_tracking_parameters<C: ConnectionTrait>(db: &C, vault: &vaults::Model) -> ApiResult<()> {
    for (label_translation_key, position, hex_color) in [
        ("🥳 Awesome", 1, "bg-lime-500"),
        ("😀 Good", 2, "bg-lime-300"),
        ("😐 Meh", 3, "bg-cyan-600"),
        ("😔 Bad", 4, "bg-orange-300"),
        ("😩 Awful", 5, "bg-red-700"),
    ] {
        mood_tracking_parameters::ActiveModel {
            vault_id: Set(vault.id),
            label: Set(None),
            label_translation_key: Set(Some(label_translation_key.into())),
            position: Set(Some(position)),
            hex_color: Set(hex_color.into()),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

async fn populate_default_life_event_categories<C: ConnectionTrait>(db: &C, vault: &vaults::Model) -> ApiResult<()> {
    for (position, label_translation_key, types) in [
        // transportation category
        (1, "Transportation", &["Rode a bike", "Drove", "Walked", "Took the bus", "Took the metro"][..]),
        // social category
        (2, "Social", &["Ate", "Drank", "Went to a bar", "Watched a movie", "Watched TV", "Watched a tv show"][..]),
        // sport category
        (3, "Sport", &["Ran", "Played soccer", "Played basketball", "Played golf", "Played tennis"][..]),
        // work category
        (4, "Work", &["Took a new job", "Quit job", "Got fired", "Had a promotion"][..]),
    ] {
        let category = life_event_categories::ActiveModel {
            vault_id: Set(vault.id),
            position: Set(Some(position)),
            label: Set(None),
            label_translation_key: Set(Some(label_translation_key.into())),
            can_be_deleted: Set(true),
            ..Default::default()
        }
        .insert(db)
        .await?;

        for (position, label_translation_key) in (1..).zip(types) {
            life_event_types::ActiveModel {
                life_event_category_id: Set(category.id),
                position: Set(Some(position)),
                label: Set(None),
                label_translation_key: Set(Some((*label_translation_key).into())),
                can_be_deleted: Set(true),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}

async fn populate_default_quick_vault_template_entries<C: ConnectionTrait>(
    db: &C,
    vault: &vaults::Model,
) -> ApiResult<()> {
    for (label_translation_key, position) in [("Hobbies", 1), ("Food preferences", 2)] {
        vault_quick_facts_templates::ActiveModel {
            vault_id: Set(vault.id),
            label_translation_key: Set(Some(label_translation_key.into())),
            position: Set(position),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// `DestroyVault`.
pub struct DestroyVault {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
}

impl DestroyVault {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Manage))),
                ..Default::default()
            },
        )
        .await?;

        // we need to delete all the files in the vault this way, so the event
        // FileDeleted is called and delete the file from storage
        for file in files::Entity::find().filter(files::Column::VaultId.eq(self.vault_id)).all(db).await? {
            files::Entity::delete_by_id(file.id).exec(db).await?;
        }

        vaults::Entity::delete_by_id(checked.vault().id).exec(db).await?;
        Ok(())
    }
}

/// `UpdateVault`.
pub struct UpdateVault {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

impl UpdateVault {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<vaults::Model> {
        let mut v = Validator::new();
        if self.name.trim().is_empty() {
            v.fail("name", "required", &[]);
        }
        v.max_len("name", Some(&self.name), 255);
        v.max_len("description", self.description.as_deref(), 255);
        v.finish()?;
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Manage))),
                ..Default::default()
            },
        )
        .await?;

        let mut vault: vaults::ActiveModel = checked.vault().clone().into();
        vault.name = Set(self.name);
        vault.description = Set(value_or_null(self.description));
        Ok(vault.update(db).await?)
    }
}

/// `UpdateVaultDashboardDefaultTab`.
pub struct UpdateVaultDashboardDefaultTab {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub default_activity_tab: String,
}

impl UpdateVaultDashboardDefaultTab {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<vaults::Model> {
        let mut v = Validator::new();
        if self.default_activity_tab.trim().is_empty() {
            v.fail("default_activity_tab", "required", &[]);
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

        let mut vault: vaults::ActiveModel = checked.vault().clone().into();
        vault.default_activity_tab = Set(self.default_activity_tab);
        Ok(vault.update(db).await?)
    }
}

/// `UpdateVaultTabVisibility`.
pub struct UpdateVaultTabVisibility {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub vault_id: Uuid,
    pub show_group_tab: bool,
    pub show_tasks_tab: bool,
    pub show_files_tab: bool,
    pub show_journal_tab: bool,
    pub show_companies_tab: bool,
    pub show_reports_tab: bool,
    pub show_calendar_tab: bool,
}

impl UpdateVaultTabVisibility {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<vaults::Model> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Manage))),
                ..Default::default()
            },
        )
        .await?;

        let mut vault: vaults::ActiveModel = checked.vault().clone().into();
        vault.show_group_tab = Set(self.show_group_tab);
        vault.show_tasks_tab = Set(self.show_tasks_tab);
        vault.show_files_tab = Set(self.show_files_tab);
        vault.show_journal_tab = Set(self.show_journal_tab);
        vault.show_companies_tab = Set(self.show_companies_tab);
        vault.show_reports_tab = Set(self.show_reports_tab);
        vault.show_calendar_tab = Set(self.show_calendar_tab);
        Ok(vault.update(db).await?)
    }
}
