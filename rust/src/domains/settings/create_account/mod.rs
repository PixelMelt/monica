//! `app/Domains/Settings/CreateAccount/Services` and `Jobs`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::uuid::Uuid;
use crate::domains::base::{Check, check};
use crate::domains::settings::manage_address_types::CreateAddressType;
use crate::domains::settings::manage_call_reasons::{CreateCallReason, CreateCallReasonType};
use crate::domains::settings::manage_contact_information_types::CreateContactInformationType;
use crate::domains::settings::manage_genders::CreateGender;
use crate::domains::settings::manage_group_types::{CreateGroupType, CreateGroupTypeRole};
use crate::domains::settings::manage_pet_categories::CreatePetCategory;
use crate::domains::settings::manage_post_templates::{CreatePostTemplate, CreatePostTemplateSection};
use crate::domains::settings::manage_pronouns::CreatePronoun;
use crate::domains::settings::manage_relationship_types::CreateRelationshipGroupType;
use crate::domains::settings::manage_templates::{
    AssociateModuleToTemplatePage, CreateModule, CreateTemplate, CreateTemplatePage,
};
use crate::domains::settings::manage_users::{normalize_username, validate_username};
use crate::entities::{
    account_currency, accounts, contact_information_types, currencies, emotions, gift_occasions, gift_states,
    relationship_types, religions, template_pages, templates, users,
};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::validate::Validator;

/// `Module` constants.
mod module {
    pub const TYPE_NOTES: &str = "notes";
    pub const TYPE_CONTACT_NAMES: &str = "contact_names";
    pub const TYPE_AVATAR: &str = "avatar";
    pub const TYPE_FAMILY_SUMMARY: &str = "family_summary";
    pub const TYPE_COMPANY: &str = "company";
    pub const TYPE_FEED: &str = "feed";
    pub const TYPE_GENDER_PRONOUN: &str = "gender_pronoun";
    pub const TYPE_IMPORTANT_DATES: &str = "important_dates";
    pub const TYPE_LABELS: &str = "labels";
    pub const TYPE_REMINDERS: &str = "reminders";
    pub const TYPE_LOANS: &str = "loans";
    pub const TYPE_RELATIONSHIPS: &str = "relationships";
    pub const TYPE_TASKS: &str = "tasks";
    pub const TYPE_CALLS: &str = "calls";
    pub const TYPE_PETS: &str = "pets";
    pub const TYPE_GOALS: &str = "goals";
    pub const TYPE_ADDRESSES: &str = "addresses";
    pub const TYPE_GROUPS: &str = "groups";
    pub const TYPE_CONTACT_INFORMATION: &str = "contact_information";
    pub const TYPE_DOCUMENTS: &str = "documents";
    pub const TYPE_PHOTOS: &str = "photos";
    pub const TYPE_POSTS: &str = "posts";
    pub const TYPE_RELIGIONS: &str = "religions";
    pub const TYPE_LIFE_EVENTS: &str = "life_events";
}

/// `TemplatePage::TYPE_CONTACT`.
const TEMPLATE_PAGE_TYPE_CONTACT: &str = "contact_information";

/// `Emotion` constants.
mod emotion {
    pub const TYPE_POSITIVE: &str = "positive";
    pub const TYPE_NEUTRAL: &str = "neutral";
    pub const TYPE_NEGATIVE: &str = "negative";
}

/// `RelationshipGroupType` constants.
mod relationship_group_type {
    pub const TYPE_FAMILY: &str = "family";
    pub const TYPE_LOVE: &str = "love";
}

/// `RelationshipType` constants.
mod relationship_type {
    pub const TYPE_LOVE: &str = "family";
    pub const TYPE_CHILD: &str = "child";
}

/// `config('app.social_protocols')`: the `name_translation_key` and `type`
/// of each entry.
const SOCIAL_PROTOCOLS: [(&str, &str); 11] = [
    ("Facebook", "X-SOCIAL-PROFILE"),
    ("Whatsapp", "X-SOCIAL-PROFILE"),
    ("Telegram", "IMPP"),
    ("LinkedIn", "X-SOCIAL-PROFILE"),
    ("Instagram", "X-SOCIAL-PROFILE"),
    ("Twitter", "IMPP"),
    ("Hangouts", "IMPP"),
    ("Mastodon", "X-SOCIAL-PROFILE"),
    ("Bluesky", "X-SOCIAL-PROFILE"),
    ("Threads", "X-SOCIAL-PROFILE"),
    ("GitHub", "X-SOCIAL-PROFILE"),
];

/// `CreateAccount`. The password's rules are checked by the caller.
pub struct CreateAccount {
    pub first_name: String,
    pub last_name: String,
    pub username: String,
    pub password: Option<String>,
}

impl CreateAccount {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<users::Model> {
        let mut v = Validator::new();
        for (field, value) in [("first_name", &self.first_name), ("last_name", &self.last_name)] {
            if value.trim().is_empty() {
                v.fail(field, "required", &[]);
            }
            v.max_len(field, Some(value), 255);
        }
        validate_username(db, &mut v, &normalize_username(&self.username), None).await?;
        v.finish()?;

        // `storage_limit_in_mb` 0: no limit.
        let account = accounts::ActiveModel { id: Set(Uuid::new()), storage_limit_in_mb: Set(0), ..Default::default() }
            .insert(db)
            .await?;

        let user = self.add_first_user(db, &account).await?;
        Self::setup_account(db, &account, &user).await?;
        Ok(user)
    }

    async fn add_first_user<C: ConnectionTrait>(&self, db: &C, account: &accounts::Model) -> ApiResult<users::Model> {
        Ok(users::ActiveModel {
            id: Set(Uuid::new()),
            account_id: Set(account.id),
            first_name: Set(Some(self.first_name.clone())),
            last_name: Set(Some(self.last_name.clone())),
            username: Set(normalize_username(&self.username)),
            password: Set(self.password.as_deref().map(crate::password::hash)),
            locale: Set(i18n::current()),
            is_account_administrator: Set(true),
            timezone: Set(Some("UTC".into())),
            ..Default::default()
        }
        .insert(db)
        .await?)
    }

    async fn setup_account<C: ConnectionTrait>(
        db: &C,
        account: &accounts::Model,
        user: &users::Model,
    ) -> ApiResult<()> {
        SetupAccount { account_id: account.id, author_id: user.id }.execute(db).await
    }
}

/// `Jobs/SetupAccount`.
pub struct SetupAccount {
    pub account_id: Uuid,
    pub author_id: Uuid,
}

impl SetupAccount {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        let author = check(db, &Check { account_id: self.account_id, author_id: self.author_id, ..Default::default() })
            .await?
            .author;

        populate_currencies(db, &author).await?;
        let template = add_template(db, &author).await?;
        add_template_page_contact_information(db, &author, &template).await?;
        add_template_page_feed(db, &author, &template).await?;
        add_template_page_contact(db, &author, &template).await?;
        add_template_page_social(db, &author, &template).await?;
        add_template_page_life_events(db, &author, &template).await?;
        add_template_page_information(db, &author, &template).await?;
        add_first_information(db, &author).await
    }
}

/// Populate currencies in the account.
async fn populate_currencies<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    let currencies = currencies::Entity::find().all(db).await?;
    for currency in currencies {
        account_currency::ActiveModel {
            currency_id: Set(currency.id),
            account_id: Set(author.account_id),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// Add the first template.
async fn add_template<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<templates::Model> {
    CreateTemplate {
        account_id: author.account_id,
        author_id: author.id,
        name: None,
        name_translation_key: Some("Default template".into()),
        can_be_deleted: Some(false),
    }
    .execute(db)
    .await
}

async fn add_template_page_contact_information<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    template: &templates::Model,
) -> ApiResult<()> {
    // the contact information page is automatically created when we
    // create the template
    let template_page_contact = template_pages::Entity::find()
        .filter(template_pages::Column::TemplateId.eq(template.id))
        .filter(template_pages::Column::Type.eq(TEMPLATE_PAGE_TYPE_CONTACT))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::internal("the template has no contact information page"))?;

    for (name_translation_key, r#type) in [
        ("Avatar", module::TYPE_AVATAR),
        ("Contact name", module::TYPE_CONTACT_NAMES),
        ("Family summary", module::TYPE_FAMILY_SUMMARY),
        ("Important dates", module::TYPE_IMPORTANT_DATES),
        ("Gender and pronoun", module::TYPE_GENDER_PRONOUN),
        ("Labels", module::TYPE_LABELS),
        ("Job information", module::TYPE_COMPANY),
        ("Religions", module::TYPE_RELIGIONS),
    ] {
        let module = CreateModule {
            account_id: author.account_id,
            author_id: author.id,
            name: None,
            name_translation_key: Some(name_translation_key.into()),
            r#type: Some(r#type.into()),
            can_be_deleted: false,
            reserved_to_contact_information: Some(true),
        }
        .execute(db)
        .await?;
        AssociateModuleToTemplatePage {
            account_id: author.account_id,
            author_id: author.id,
            template_id: template.id,
            template_page_id: template_page_contact.id,
            module_id: module.id,
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_template_page_feed<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    template: &templates::Model,
) -> ApiResult<()> {
    let template_page_feed = CreateTemplatePage {
        account_id: author.account_id,
        author_id: author.id,
        template_id: template.id,
        name: None,
        name_translation_key: Some("Activity feed".into()),
        r#type: None,
        can_be_deleted: Some(true),
    }
    .execute(db)
    .await?;
    let module = CreateModule {
        account_id: author.account_id,
        author_id: author.id,
        name: None,
        name_translation_key: Some("Contact feed".into()),
        r#type: Some(module::TYPE_FEED.into()),
        can_be_deleted: false,
        reserved_to_contact_information: None,
    }
    .execute(db)
    .await?;
    AssociateModuleToTemplatePage {
        account_id: author.account_id,
        author_id: author.id,
        template_id: template.id,
        template_page_id: template_page_feed.id,
        module_id: module.id,
    }
    .execute(db)
    .await?;
    Ok(())
}

async fn add_template_page_contact<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    template: &templates::Model,
) -> ApiResult<()> {
    let template_page = CreateTemplatePage {
        account_id: author.account_id,
        author_id: author.id,
        template_id: template.id,
        name: None,
        name_translation_key: Some("Ways to connect".into()),
        r#type: None,
        can_be_deleted: Some(true),
    }
    .execute(db)
    .await?;

    for (name_translation_key, r#type) in
        [("Addresses", module::TYPE_ADDRESSES), ("Contact information", module::TYPE_CONTACT_INFORMATION)]
    {
        let module = CreateModule {
            account_id: author.account_id,
            author_id: author.id,
            name: None,
            name_translation_key: Some(name_translation_key.into()),
            r#type: Some(r#type.into()),
            can_be_deleted: false,
            reserved_to_contact_information: None,
        }
        .execute(db)
        .await?;
        AssociateModuleToTemplatePage {
            account_id: author.account_id,
            author_id: author.id,
            template_id: template.id,
            template_page_id: template_page.id,
            module_id: module.id,
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_template_page_social<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    template: &templates::Model,
) -> ApiResult<()> {
    let template_page_social = CreateTemplatePage {
        account_id: author.account_id,
        author_id: author.id,
        template_id: template.id,
        name: None,
        name_translation_key: Some("Social".into()),
        r#type: None,
        can_be_deleted: Some(true),
    }
    .execute(db)
    .await?;

    for (name_translation_key, r#type) in
        [("Relationships", module::TYPE_RELATIONSHIPS), ("Pets", module::TYPE_PETS), ("Groups", module::TYPE_GROUPS)]
    {
        let module = CreateModule {
            account_id: author.account_id,
            author_id: author.id,
            name: None,
            name_translation_key: Some(name_translation_key.into()),
            r#type: Some(r#type.into()),
            can_be_deleted: false,
            reserved_to_contact_information: None,
        }
        .execute(db)
        .await?;
        AssociateModuleToTemplatePage {
            account_id: author.account_id,
            author_id: author.id,
            template_id: template.id,
            template_page_id: template_page_social.id,
            module_id: module.id,
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_template_page_life_events<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    template: &templates::Model,
) -> ApiResult<()> {
    let template_page_social = CreateTemplatePage {
        account_id: author.account_id,
        author_id: author.id,
        template_id: template.id,
        name: None,
        name_translation_key: Some("Life & goals".into()),
        r#type: None,
        can_be_deleted: Some(true),
    }
    .execute(db)
    .await?;

    for (name_translation_key, r#type) in [("Life", module::TYPE_LIFE_EVENTS), ("Goals", module::TYPE_GOALS)] {
        let module = CreateModule {
            account_id: author.account_id,
            author_id: author.id,
            name: None,
            name_translation_key: Some(name_translation_key.into()),
            r#type: Some(r#type.into()),
            can_be_deleted: false,
            reserved_to_contact_information: None,
        }
        .execute(db)
        .await?;
        AssociateModuleToTemplatePage {
            account_id: author.account_id,
            author_id: author.id,
            template_id: template.id,
            template_page_id: template_page_social.id,
            module_id: module.id,
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_template_page_information<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    template: &templates::Model,
) -> ApiResult<()> {
    let template_page_information = CreateTemplatePage {
        account_id: author.account_id,
        author_id: author.id,
        template_id: template.id,
        name: None,
        name_translation_key: Some("Information".into()),
        r#type: None,
        can_be_deleted: Some(true),
    }
    .execute(db)
    .await?;

    for (name_translation_key, r#type) in [
        ("Documents", module::TYPE_DOCUMENTS),
        ("Photos", module::TYPE_PHOTOS),
        ("Notes", module::TYPE_NOTES),
        ("Reminders", module::TYPE_REMINDERS),
        ("Loans", module::TYPE_LOANS),
        ("Tasks", module::TYPE_TASKS),
        ("Calls", module::TYPE_CALLS),
        ("Posts", module::TYPE_POSTS),
    ] {
        let module = CreateModule {
            account_id: author.account_id,
            author_id: author.id,
            name: None,
            name_translation_key: Some(name_translation_key.into()),
            r#type: Some(r#type.into()),
            can_be_deleted: false,
            reserved_to_contact_information: None,
        }
        .execute(db)
        .await?;
        AssociateModuleToTemplatePage {
            account_id: author.account_id,
            author_id: author.id,
            template_id: template.id,
            template_page_id: template_page_information.id,
            module_id: module.id,
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_first_information<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    add_genders(db, author).await?;
    add_pronouns(db, author).await?;
    add_group_types(db, author).await?;
    add_relationship_types(db, author).await?;
    add_address_types(db, author).await?;
    add_call_reason_types(db, author).await?;
    add_contact_information(db, author).await?;
    add_pet_categories(db, author).await?;
    add_emotions(db, author).await?;
    add_gift_occasions(db, author).await?;
    add_gift_states(db, author).await?;
    add_post_templates(db, author).await?;
    add_religions(db, author).await
}

/// Add the default genders in the account.
async fn add_genders<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    for r#type in ["Male", "Female", "Other"] {
        CreateGender {
            account_id: author.account_id,
            author_id: author.id,
            name: None,
            name_translation_key: Some(r#type.into()),
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

/// Add the default pronouns in the account.
async fn add_pronouns<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    for pronoun in ["he/him", "she/her", "they/them", "per/per", "ve/ver", "xe/xem", "ze/hir"] {
        CreatePronoun {
            account_id: author.account_id,
            author_id: author.id,
            name: None,
            name_translation_key: Some(pronoun.into()),
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

/// Add the default group types in the account.
async fn add_group_types<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    let group_type = CreateGroupType {
        account_id: author.account_id,
        author_id: author.id,
        label: None,
        label_translation_key: Some("Family".into()),
    }
    .execute(db)
    .await?;
    CreateGroupTypeRole {
        account_id: author.account_id,
        author_id: author.id,
        group_type_id: group_type.id,
        label: None,
        label_translation_key: Some("Parent".into()),
    }
    .execute(db)
    .await?;
    CreateGroupTypeRole {
        account_id: author.account_id,
        author_id: author.id,
        group_type_id: group_type.id,
        label: None,
        label_translation_key: Some("Child".into()),
    }
    .execute(db)
    .await?;

    let group_type = CreateGroupType {
        account_id: author.account_id,
        author_id: author.id,
        label: None,
        label_translation_key: Some("Couple".into()),
    }
    .execute(db)
    .await?;
    CreateGroupTypeRole {
        account_id: author.account_id,
        author_id: author.id,
        group_type_id: group_type.id,
        label: None,
        label_translation_key: Some("Partner".into()),
    }
    .execute(db)
    .await?;

    for label_translation_key in ["Club", "Association", "Roomates"] {
        CreateGroupType {
            account_id: author.account_id,
            author_id: author.id,
            label: None,
            label_translation_key: Some(label_translation_key.into()),
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_relationship_types<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    // Love type
    let group = CreateRelationshipGroupType {
        account_id: author.account_id,
        author_id: author.id,
        name: None,
        name_translation_key: Some("Love".into()),
        r#type: Some(relationship_group_type::TYPE_LOVE.into()),
        can_be_deleted: false,
    }
    .execute(db)
    .await?;

    relationship_types::Entity::insert_many(
        [
            ("significant other", "significant other", false, Some(relationship_type::TYPE_LOVE)),
            ("spouse", "spouse", false, Some(relationship_type::TYPE_LOVE)),
            ("date", "date", true, None),
            ("lover", "lover", true, None),
            ("in love with", "loved by", true, None),
            ("ex-boyfriend", "ex-boyfriend", true, None),
        ]
        .map(|(name, reverse, can_be_deleted, r#type)| relationship_types::ActiveModel {
            name_translation_key: Set(Some(name.into())),
            name_reverse_relationship_translation_key: Set(Some(reverse.into())),
            relationship_group_type_id: Set(group.id),
            can_be_deleted: Set(can_be_deleted),
            r#type: Set(r#type.map(Into::into)),
            ..Default::default()
        }),
    )
    .exec_without_returning(db)
    .await?;

    // Family type
    let group = CreateRelationshipGroupType {
        account_id: author.account_id,
        author_id: author.id,
        name: None,
        name_translation_key: Some("Family".into()),
        r#type: Some(relationship_group_type::TYPE_FAMILY.into()),
        can_be_deleted: false,
    }
    .execute(db)
    .await?;

    relationship_types::Entity::insert_many(
        [
            ("parent", "child", false, Some(relationship_type::TYPE_CHILD)),
            ("brother/sister", "brother/sister", true, None),
            ("grand parent", "grand child", true, None),
            ("uncle/aunt", "nephew/niece", true, None),
            ("cousin", "cousin", true, None),
            ("godparent", "godchild", true, None),
        ]
        .map(|(name, reverse, can_be_deleted, r#type)| relationship_types::ActiveModel {
            name_translation_key: Set(Some(name.into())),
            name_reverse_relationship_translation_key: Set(Some(reverse.into())),
            relationship_group_type_id: Set(group.id),
            can_be_deleted: Set(can_be_deleted),
            r#type: Set(r#type.map(Into::into)),
            ..Default::default()
        }),
    )
    .exec_without_returning(db)
    .await?;

    // Friend
    let group = CreateRelationshipGroupType {
        account_id: author.account_id,
        author_id: author.id,
        name: None,
        name_translation_key: Some("Friend".into()),
        r#type: None,
        can_be_deleted: true,
    }
    .execute(db)
    .await?;

    relationship_types::Entity::insert_many(
        [("friend", "friend", true, None::<&str>), ("best friend", "best friend", true, None)].map(
            |(name, reverse, can_be_deleted, r#type)| relationship_types::ActiveModel {
                name_translation_key: Set(Some(name.into())),
                name_reverse_relationship_translation_key: Set(Some(reverse.into())),
                relationship_group_type_id: Set(group.id),
                can_be_deleted: Set(can_be_deleted),
                r#type: Set(r#type.map(Into::into)),
                ..Default::default()
            },
        ),
    )
    .exec_without_returning(db)
    .await?;

    // Work
    let group = CreateRelationshipGroupType {
        account_id: author.account_id,
        author_id: author.id,
        name: None,
        name_translation_key: Some("Work".into()),
        r#type: None,
        can_be_deleted: true,
    }
    .execute(db)
    .await?;

    relationship_types::Entity::insert_many(
        [
            ("colleague", "colleague", true, None::<&str>),
            ("subordinate", "boss", true, None),
            ("mentor", "protege", true, None),
        ]
        .map(|(name, reverse, can_be_deleted, r#type)| relationship_types::ActiveModel {
            name_translation_key: Set(Some(name.into())),
            name_reverse_relationship_translation_key: Set(Some(reverse.into())),
            relationship_group_type_id: Set(group.id),
            can_be_deleted: Set(can_be_deleted),
            r#type: Set(r#type.map(Into::into)),
            ..Default::default()
        }),
    )
    .exec_without_returning(db)
    .await?;
    Ok(())
}

async fn add_address_types<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    for (r#type, label) in [
        ("home", "🏡 Home"),
        ("secondary", "🏠 Secondary residence"),
        ("work", "🏢 Work"),
        ("chalet", "🌳 Chalet"),
        ("other", "❔ Other"),
    ] {
        CreateAddressType {
            account_id: author.account_id,
            author_id: author.id,
            name: None,
            name_translation_key: Some(label.into()),
            r#type: Some(r#type.into()),
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_call_reason_types<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    let r#type = CreateCallReasonType {
        account_id: author.account_id,
        author_id: author.id,
        label: None,
        label_translation_key: Some("Personal".into()),
    }
    .execute(db)
    .await?;
    for label_translation_key in [
        "For advice",
        "Just to say hello",
        "To see if they need anything",
        "Out of respect and appreciation",
        "To hear their story",
    ] {
        CreateCallReason {
            account_id: author.account_id,
            author_id: author.id,
            call_reason_type_id: r#type.id,
            label: None,
            label_translation_key: Some(label_translation_key.into()),
        }
        .execute(db)
        .await?;
    }

    // business
    let r#type = CreateCallReasonType {
        account_id: author.account_id,
        author_id: author.id,
        label: None,
        label_translation_key: Some("Business".into()),
    }
    .execute(db)
    .await?;
    for label_translation_key in ["Discuss recent purchases", "Discuss partnership"] {
        CreateCallReason {
            account_id: author.account_id,
            author_id: author.id,
            call_reason_type_id: r#type.id,
            label: None,
            label_translation_key: Some(label_translation_key.into()),
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_contact_information<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    let information = CreateContactInformationType {
        account_id: author.account_id,
        author_id: author.id,
        r#type: "email".into(),
        name: None,
        name_translation_key: Some("Email address".into()),
        protocol: Some("mailto:".into()),
    }
    .execute(db)
    .await?;
    let mut information: contact_information_types::ActiveModel = information.into();
    information.can_be_deleted = Set(false);
    information.update(db).await?;

    let information = CreateContactInformationType {
        account_id: author.account_id,
        author_id: author.id,
        r#type: "phone".into(),
        name: None,
        name_translation_key: Some("Phone".into()),
        protocol: Some("tel:".into()),
    }
    .execute(db)
    .await?;
    let mut information: contact_information_types::ActiveModel = information.into();
    information.can_be_deleted = Set(false);
    information.update(db).await?;

    for (name_translation_key, r#type) in SOCIAL_PROTOCOLS {
        CreateContactInformationType {
            account_id: author.account_id,
            author_id: author.id,
            r#type: r#type.into(),
            name: None,
            name_translation_key: Some(name_translation_key.into()),
            protocol: None,
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_pet_categories<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    for category in ["Dog", "Cat", "Bird", "Fish", "Small animal", "Hamster", "Horse", "Rabbit", "Rat", "Reptile"] {
        CreatePetCategory {
            account_id: author.account_id,
            author_id: author.id,
            name: None,
            name_translation_key: Some(category.into()),
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_emotions<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    emotions::Entity::insert_many(
        [
            ("😡 Negative", emotion::TYPE_NEGATIVE),
            ("😶‍🌫️ Neutral", emotion::TYPE_NEUTRAL),
            ("😁 Positive", emotion::TYPE_POSITIVE),
        ]
        .map(|(name_translation_key, r#type)| emotions::ActiveModel {
            account_id: Set(author.account_id),
            name_translation_key: Set(Some(name_translation_key.into())),
            r#type: Set(r#type.into()),
            ..Default::default()
        }),
    )
    .exec_without_returning(db)
    .await?;
    Ok(())
}

async fn add_gift_occasions<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    gift_occasions::Entity::insert_many(
        [("Birthday", 1), ("Anniversary", 2), ("Christmas", 3), ("Just because", 4), ("Wedding", 5)].map(
            |(label_translation_key, position)| gift_occasions::ActiveModel {
                account_id: Set(author.account_id),
                label_translation_key: Set(Some(label_translation_key.into())),
                position: Set(Some(position)),
                ..Default::default()
            },
        ),
    )
    .exec_without_returning(db)
    .await?;
    Ok(())
}

async fn add_gift_states<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    gift_states::Entity::insert_many([("Idea", 1), ("Searched", 2), ("Found", 3), ("Bought", 4), ("Offered", 5)].map(
        |(label_translation_key, position)| gift_states::ActiveModel {
            account_id: Set(author.account_id),
            label_translation_key: Set(Some(label_translation_key.into())),
            position: Set(Some(position)),
            ..Default::default()
        },
    ))
    .exec_without_returning(db)
    .await?;
    Ok(())
}

async fn add_post_templates<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    // default template
    let post_template = CreatePostTemplate {
        account_id: author.account_id,
        author_id: author.id,
        label: None,
        label_translation_key: Some("Regular post".into()),
        can_be_deleted: false,
    }
    .execute(db)
    .await?;

    CreatePostTemplateSection {
        account_id: author.account_id,
        author_id: author.id,
        post_template_id: post_template.id,
        label: None,
        label_translation_key: Some("Content".into()),
        can_be_deleted: false,
    }
    .execute(db)
    .await?;

    // inspirational template
    let post_template = CreatePostTemplate {
        account_id: author.account_id,
        author_id: author.id,
        label: None,
        label_translation_key: Some("Inspirational post".into()),
        can_be_deleted: true,
    }
    .execute(db)
    .await?;

    for label_translation_key in [
        "I am grateful for",
        "Daily affirmation",
        "How could I have done this day better?",
        "What would make today great?",
        "Three things that happened today",
    ] {
        CreatePostTemplateSection {
            account_id: author.account_id,
            author_id: author.id,
            post_template_id: post_template.id,
            label: None,
            label_translation_key: Some(label_translation_key.into()),
            can_be_deleted: true,
        }
        .execute(db)
        .await?;
    }
    Ok(())
}

async fn add_religions<C: ConnectionTrait>(db: &C, author: &users::Model) -> ApiResult<()> {
    religions::Entity::insert_many(
        [
            ("Christian", 1),
            ("Muslim", 2),
            ("Hinduist", 3),
            ("Buddhist", 4),
            ("Shintoist", 5),
            ("Taoist", 6),
            ("Sikh", 7),
            ("Jew", 8),
            ("Atheist", 9),
        ]
        .map(|(translation_key, position)| religions::ActiveModel {
            account_id: Set(author.account_id),
            translation_key: Set(Some(translation_key.into())),
            position: Set(Some(position)),
            ..Default::default()
        }),
    )
    .exec_without_returning(db)
    .await?;
    Ok(())
}
