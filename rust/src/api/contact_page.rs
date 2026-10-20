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
use crate::db::id::Id;
use crate::db::uuid::Uuid;
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
// TODO(endpoints): hand-written: a oneOf of the variants in the draft.
pub type ContactPageModule = serde_json::Value;

/// Data of the contact page (Vault/Contact/Show), api-design.md §12.
#[derive(Serialize, ToSchema)]
pub struct ContactPage {
    pub contact: Contact,
    /// Computed: this is the current user's own contact in the vault (the page hides archive and delete for it).
    pub is_current_user_contact: bool,
    /// Computed: the account is below its storage limit (X011).
    pub can_upload_file: bool,
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
    todo!("getContactPage")
}

/// The operations of this module.
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_contact_page))
}
