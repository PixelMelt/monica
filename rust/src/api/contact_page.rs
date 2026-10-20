//! `GET /vaults/{vault_id}/contacts/{contact_id}/page`: the contact page
//! aggregate (api-design.md §12), built from the module data of every
//! contact module area.
use super::contact_modules_a::AddressesModule;
use super::contact_modules_a::CompanyModule;
use super::contact_modules_a::ContactInformationModule;
use super::contact_modules_a::GoalsModule;
use super::contact_modules_a::ImportantDatesModule;
use super::contact_modules_a::LabelsModule;
use super::contact_modules_a::LoansModule;
use super::contact_modules_a::ReligionsModule;
use super::contact_modules_a::RemindersModule;
use super::contact_modules_b::CallsModule;
use super::contact_modules_b::DocumentsModule;
use super::contact_modules_b::GroupsModule;
use super::contact_modules_b::LifeEventsModule;
use super::contact_modules_b::PetsModule;
use super::contact_modules_b::PhotosModule;
use super::contact_modules_b::RelationshipsModule;
use super::contact_modules_b::TasksModule;
use super::contact_notes::NotesModule;
use super::contacts::{
    AvatarModule, Contact, ContactNamesModule, FamilySummaryModule, FeedModule, GenderPronounModule, PostsModule,
    QuickFact,
};
use super::templates::TemplatePage;
use super::vault_settings::QuickFactTemplate;
use std::collections::HashMap;

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::entities::{
    contact_group, contacts, groups, module_template_page, modules, quick_facts, template_pages,
    vault_quick_facts_templates,
};
use crate::error::ApiError;
use crate::error::ApiResult;
use crate::http::access;
use crate::http::auth::Auth;
use crate::http::json::Json;
use crate::http::state::AppState;
use axum::extract::Path;
use axum::extract::Query;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// What the data of one contact page module is built from: each area
/// exposes `pub async fn <module type>_module(state: &AppState, ctx:
/// &ModuleContext) -> ApiResult<<Type>Module>` for the module types of its
/// fragment (`contact_notes::notes_module`, `contacts::avatar_module`,
/// `contact_modules_a::important_dates_module`, …).
pub struct ModuleContext {
    /// The viewer.
    pub user: crate::entities::users::Model,
    pub vault: crate::entities::vaults::Model,
    pub contact: crate::entities::contacts::Model,
}

/// One module block of the contact page; `data` depends on `type`.
#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub enum ContactPageModule {
    Notes(ContactPageModuleNotes),
    ContactNames(ContactPageModuleContactNames),
    Avatar(ContactPageModuleAvatar),
    FamilySummary(ContactPageModuleFamilySummary),
    Company(ContactPageModuleCompany),
    Feed(ContactPageModuleFeed),
    GenderPronoun(ContactPageModuleGenderPronoun),
    ImportantDates(ContactPageModuleImportantDates),
    Labels(ContactPageModuleLabels),
    Reminders(ContactPageModuleReminders),
    Loans(ContactPageModuleLoans),
    Relationships(ContactPageModuleRelationships),
    Tasks(ContactPageModuleTasks),
    Calls(ContactPageModuleCalls),
    Pets(ContactPageModulePets),
    Goals(ContactPageModuleGoals),
    Addresses(ContactPageModuleAddresses),
    Groups(ContactPageModuleGroups),
    ContactInformation(ContactPageModuleContactInformation),
    Documents(ContactPageModuleDocuments),
    Photos(ContactPageModulePhotos),
    Posts(ContactPageModulePosts),
    Religions(ContactPageModuleReligions),
    LifeEvents(ContactPageModuleLifeEvents),
}

/// Data of the contact page (Vault/Contact/Show), api-design.md §12.
#[derive(Serialize, ToSchema)]
pub struct ContactPage {
    pub contact: Contact,
    /// Computed: this is the current user's own contact in the vault (the page hides archive and delete for it).
    pub is_current_user_contact: bool,
    /// Modules of the template's `contact_information` page, in position
    /// order (left column: avatar, contact_names, family_summary,
    /// gender_pronoun, important_dates, labels, company, religions).
    pub header_modules: Vec<ContactPageModule>,
    /// Groups the contact is part of, by name.
    pub groups: Vec<ContactPageGroup>,
    /// The vault's quick fact templates, in position order (the block is hidden when empty).
    pub quick_fact_templates: Vec<QuickFactTemplate>,
    /// The contact's quick facts for the first template.
    pub quick_facts: Vec<QuickFact>,
    /// The template's pages (tabs) other than `contact_information`, in position order.
    pub template_pages: Vec<TemplatePage>,
    /// The page whose modules are in `modules`; null when there is none.
    pub active_template_page_id: Option<i64>,
    /// Modules of the active page, in position order. Modules without a known type are left out.
    pub modules: Vec<ContactPageModule>,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleBase {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleNotes {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: NotesModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleContactNames {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: ContactNamesModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleAvatar {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: AvatarModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleFamilySummary {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: FamilySummaryModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleCompany {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: CompanyModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleFeed {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: FeedModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleGenderPronoun {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: GenderPronounModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleImportantDates {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: ImportantDatesModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleLabels {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: LabelsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleReminders {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: RemindersModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleLoans {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: LoansModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleRelationships {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: RelationshipsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleTasks {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: TasksModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleCalls {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: CallsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModulePets {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: PetsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleGoals {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: GoalsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleAddresses {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: AddressesModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleGroups {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: GroupsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleContactInformation {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: ContactInformationModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleDocuments {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: DocumentsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModulePhotos {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: PhotosModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModulePosts {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: PostsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleReligions {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: ReligionsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageModuleLifeEvents {
    pub name: Option<String>,
    pub name_translation_key: Option<String>,
    /// The module id.
    #[schema(value_type = i64)]
    pub id: Id,
    pub r#type: String,
    pub data: LifeEventsModule,
}

#[derive(Serialize, ToSchema)]
pub struct ContactPageGroup {
    #[schema(value_type = i64)]
    pub id: Id,
    pub name: String,
}

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GetContactPageQuery {
    /// Slug of the template page (tab) to show. Default: the first page
    /// by `position` that isn't the `contact_information` page. 404 when
    /// the contact's template has no page with this slug.
    pub template_page: Option<String>,
}

#[utoipa::path(
    get,
    path = "/vaults/{vault_id}/contacts/{contact_id}/page",
    operation_id = "getContactPage",
    tag = "Contacts",
    summary = "Everything the contact page shows (aggregate, api-design.md §12)",
    params(("vault_id" = uuid::Uuid, Path), ("contact_id" = uuid::Uuid, Path), GetContactPageQuery),
    responses((status = 200, description = "The page data.", body = ContactPage), (status = 401, description = "No valid session or token (`unauthenticated`)."), (status = 403, description = "`forbidden`, `csrf_token_mismatch`, `email_not_verified`, `token_ability_missing`, `password_confirmation_required` or `signup_disabled`."), (status = 404, description = "Doesn't exist or isn't visible to the user (`not_found`).")),
    extensions(("x-permission" = json!("vault:view")), ("x-inventory" = json!(["R041", "R057"])), ("x-services" = json!([])))
)]
pub async fn get_contact_page(
    State(state): State<AppState>,
    Auth(id): Auth,
    Path((vault_id, contact_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<GetContactPageQuery>,
) -> ApiResult<Json<ContactPage>> {
    let access = access::vault(&state, &id, vault_id, crate::domains::base::VaultPermission::View).await?;
    let db = &state.db;
    let contact = contacts::Entity::live()
        .filter(contacts::Column::Id.eq(contact_id))
        .filter(contacts::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let ctx = ModuleContext { user: id.user.clone(), vault: access.vault.clone(), contact: contact.clone() };

    // `GroupsViewHelper::summary`.
    let mut groups: Vec<ContactPageGroup> = contact_group::Entity::find()
        .filter(contact_group::Column::ContactId.eq(contact.id))
        .find_also_related(groups::Entity)
        .all(db)
        .await?
        .into_iter()
        .filter_map(|(_, g)| g.filter(|g| g.deleted_at.is_none()))
        .map(|g| ContactPageGroup { id: g.id, name: g.name })
        .collect();
    crate::text::sort_by_collator(&mut groups, |g| g.name.clone());

    // `ContactShowViewHelper::quickFacts`: the vault's templates, and the
    // contact's quick facts of the first one.
    let templates = vault_quick_facts_templates::Entity::find()
        .filter(vault_quick_facts_templates::Column::VaultId.eq(vault_id))
        .order_by_asc(vault_quick_facts_templates::Column::Position)
        .order_by_asc(vault_quick_facts_templates::Column::Id)
        .all(db)
        .await?;
    let quick_facts = match templates.first() {
        Some(first) => quick_facts::Entity::find()
            .filter(quick_facts::Column::ContactId.eq(contact.id))
            .filter(quick_facts::Column::VaultQuickFactsTemplateId.eq(first.id))
            .order_by_asc(quick_facts::Column::Id)
            .all(db)
            .await?
            .into_iter()
            .map(QuickFact::from)
            .collect(),
        None => Vec::new(),
    };

    // The template's pages: the `contact_information` page gives the header,
    // the requested page (else the first other page) gives the modules.
    let (mut header_modules, mut modules, mut template_pages, mut active_template_page_id) =
        (Vec::new(), Vec::new(), Vec::new(), None);
    if let Some(template_id) = contact.template_id {
        let pages = template_pages::Entity::find()
            .filter(template_pages::Column::TemplateId.eq(template_id))
            .order_by_asc(template_pages::Column::Position)
            .order_by_asc(template_pages::Column::Id)
            .all(db)
            .await?;
        let is_contact_page = |p: &template_pages::Model| p.r#type.as_deref() == Some(TYPE_CONTACT);
        let active = match &query.template_page {
            Some(slug) => Some(pages.iter().find(|p| &p.slug == slug).ok_or_else(ApiError::not_found)?),
            None => pages.iter().find(|p| !is_contact_page(p)),
        };
        if let Some(page) = pages.iter().find(|p| is_contact_page(p)) {
            header_modules = page_modules(&state, &ctx, page.id).await?;
        }
        if let Some(page) = active {
            active_template_page_id = Some(page.id.0);
            modules = page_modules(&state, &ctx, page.id).await?;
        }
        template_pages = pages.into_iter().filter(|p| !is_contact_page(p)).map(TemplatePage::from).collect();
    }

    Ok(Json(ContactPage {
        is_current_user_contact: access.membership.contact_id == contact.id,
        contact: Contact::load(db, id.user.id, contact).await?,
        header_modules,
        groups,
        quick_fact_templates: templates.into_iter().map(QuickFactTemplate::from).collect(),
        quick_facts,
        template_pages,
        active_template_page_id,
        modules,
    }))
}

/// `TemplatePage::TYPE_CONTACT`.
const TYPE_CONTACT: &str = "contact_information";

/// The modules of a template page, in position order, with their data
/// (`ContactShowViewHelper::getContactInformation` and `::modules`); a
/// module of a type the page doesn't show is left out.
async fn page_modules(state: &AppState, ctx: &ModuleContext, page_id: Id) -> ApiResult<Vec<ContactPageModule>> {
    let links = module_template_page::Entity::find()
        .filter(module_template_page::Column::TemplatePageId.eq(page_id))
        .order_by_asc(module_template_page::Column::Position)
        .order_by_asc(module_template_page::Column::ModuleId)
        .all(&state.db)
        .await?;
    let ids: Vec<Id> = links.iter().map(|l| l.module_id).collect();
    let mut by_id: HashMap<Id, modules::Model> = modules::Entity::find()
        .filter(modules::Column::Id.is_in(ids.clone()))
        .all(&state.db)
        .await?
        .into_iter()
        .map(|m| (m.id, m))
        .collect();
    let mut out = Vec::new();
    for id in ids {
        if let Some(module) = by_id.remove(&id)
            && let Some(block) = module_block(state, ctx, module).await?
        {
            out.push(block);
        }
    }
    Ok(out)
}

/// One module with the data of its type.
async fn module_block(
    state: &AppState,
    ctx: &ModuleContext,
    module: modules::Model,
) -> ApiResult<Option<ContactPageModule>> {
    use super::{contact_modules_a as a, contact_modules_b as b, contact_notes, contacts};
    let Some(kind) = module.r#type.clone() else { return Ok(None) };
    macro_rules! block {
        ($variant:ident, $schema:ident, $data:expr) => {
            ContactPageModule::$variant($schema {
                name: module.name,
                name_translation_key: module.name_translation_key,
                id: module.id,
                r#type: kind.clone(),
                data: $data,
            })
        };
    }
    Ok(Some(match kind.as_str() {
        "notes" => block!(Notes, ContactPageModuleNotes, contact_notes::notes_module(state, ctx).await?),
        "contact_names" => {
            block!(ContactNames, ContactPageModuleContactNames, contacts::contact_names_module(state, ctx).await?)
        }
        "avatar" => block!(Avatar, ContactPageModuleAvatar, contacts::avatar_module(state, ctx).await?),
        "family_summary" => {
            block!(FamilySummary, ContactPageModuleFamilySummary, contacts::family_summary_module(state, ctx).await?)
        }
        "company" => block!(Company, ContactPageModuleCompany, a::company_module(state, ctx).await?),
        "feed" => block!(Feed, ContactPageModuleFeed, contacts::feed_module(state, ctx).await?),
        "gender_pronoun" => {
            block!(GenderPronoun, ContactPageModuleGenderPronoun, contacts::gender_pronoun_module(state, ctx).await?)
        }
        "important_dates" => {
            block!(ImportantDates, ContactPageModuleImportantDates, a::important_dates_module(state, ctx).await?)
        }
        "labels" => block!(Labels, ContactPageModuleLabels, a::labels_module(state, ctx).await?),
        "reminders" => block!(Reminders, ContactPageModuleReminders, a::reminders_module(state, ctx).await?),
        "loans" => block!(Loans, ContactPageModuleLoans, a::loans_module(state, ctx).await?),
        "relationships" => {
            block!(Relationships, ContactPageModuleRelationships, b::relationships_module(state, ctx).await?)
        }
        "tasks" => block!(Tasks, ContactPageModuleTasks, b::tasks_module(state, ctx).await?),
        "calls" => block!(Calls, ContactPageModuleCalls, b::calls_module(state, ctx).await?),
        "pets" => block!(Pets, ContactPageModulePets, b::pets_module(state, ctx).await?),
        "goals" => block!(Goals, ContactPageModuleGoals, a::goals_module(state, ctx).await?),
        "addresses" => block!(Addresses, ContactPageModuleAddresses, a::addresses_module(state, ctx).await?),
        "groups" => block!(Groups, ContactPageModuleGroups, b::groups_module(state, ctx).await?),
        "contact_information" => block!(
            ContactInformation,
            ContactPageModuleContactInformation,
            a::contact_information_module(state, ctx).await?
        ),
        "documents" => block!(Documents, ContactPageModuleDocuments, b::documents_module(state, ctx).await?),
        "photos" => block!(Photos, ContactPageModulePhotos, b::photos_module(state, ctx).await?),
        "posts" => block!(Posts, ContactPageModulePosts, contacts::posts_module(state, ctx).await?),
        "religions" => block!(Religions, ContactPageModuleReligions, a::religions_module(state, ctx).await?),
        "life_events" => block!(LifeEvents, ContactPageModuleLifeEvents, b::life_events_module(state, ctx).await?),
        _ => return Ok(None),
    }))
}

/// The operations of this module.
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_contact_page))
}
