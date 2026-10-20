//! `AddContactToPost`, `AddPhotoToPost`, `AssignTag`, `CreatePost`,
//! `DestroyPost`, `IncrementPostReadCounter`, `RemoveContactFromPost`,
//! `RemoveTag`, `UpdatePost`.

use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, QueryFilter, QueryOrder,
};

use super::journals::find_journal;
use super::{POST, date_format, find_file, post_photos};
use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::{
    contact_feed_items, contact_post, contacts, files, post_sections, post_tag, post_template_sections, post_templates,
    posts, tags,
};
use crate::error::{ApiError, ApiResult};
use crate::i18n;
use crate::validate::{self, Validator};

/// `AddContactToPost`. F16: the feed item and `last_updated_at` are written
/// only when the contact wasn't in the post yet.
pub struct AddContactToPost {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
    pub contact_id: Uuid,
}

impl AddContactToPost {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<posts::Model> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id).await?;
        let contact = checked.contact();

        if contact_post::Entity::find_by_id((post.id, contact.id)).one(db).await?.is_none() {
            contact_post::Entity::insert(contact_post::ActiveModel {
                post_id: Set(post.id),
                contact_id: Set(contact.id),
                created_at: Set(None),
                updated_at: Set(None),
            })
            .exec_without_returning(db)
            .await?;

            create_feed_item(db, checked.author.id, contact.id, "added_to_post", &post).await?;
            update_last_edited_date(db, contact).await?;
        }

        Ok(post)
    }
}

/// `AddPhotoToPost`.
pub struct AddPhotoToPost {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Option<Id>,
    pub file_id: Id,
}

impl AddPhotoToPost {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<posts::Model> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id.ok_or_else(ApiError::not_found)?).await?;
        let file = find_file(db, self.vault_id, self.file_id).await?;

        let mut file: files::ActiveModel = file.into();
        file.fileable_id = Set(Some(post.id));
        file.fileable_type = Set(Some(POST.into()));
        file.r#type = Set("photo".into());
        file.update(db).await?;

        Ok(post)
    }
}

/// `AssignTag`.
pub struct AssignTag {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
    pub tag_id: Id,
}

impl AssignTag {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<tags::Model> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id).await?;
        let tag = find_tag(db, self.vault_id, self.tag_id).await?;

        if post_tag::Entity::find_by_id((tag.id, post.id)).one(db).await?.is_none() {
            post_tag::Entity::insert(post_tag::ActiveModel {
                tag_id: Set(tag.id),
                post_id: Set(post.id),
                created_at: Set(None),
                updated_at: Set(None),
            })
            .exec_without_returning(db)
            .await?;
        }

        Ok(tag)
    }
}

/// `CreatePost`.
pub struct CreatePost {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_template_id: Id,
    pub title: Option<String>,
    pub published: bool,
    pub written_at: Option<String>,
}

impl CreatePost {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<posts::Model> {
        let mut v = Validator::new();
        if post_templates::Entity::find_by_id(self.post_template_id).one(db).await?.is_none() {
            v.fail("post_template_id", "exists", &[]);
        }
        v.max_len("title", self.title.as_deref(), 255);
        let written_at = date_format(&mut v, "written_at", self.written_at.as_deref());
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        find_journal(db, self.vault_id, self.journal_id).await?;
        let post_template = post_templates::Entity::find_by_id(self.post_template_id)
            .filter(post_templates::Column::AccountId.eq(self.account_id))
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        let post = posts::ActiveModel {
            journal_id: Set(self.journal_id),
            title: Set(value_or_null(self.title)),
            published: Set(self.published),
            written_at: Set(written_at.unwrap_or_else(crate::db::now)),
            ..Default::default()
        }
        .insert(db)
        .await?;
        create_post_sections(db, &post, &post_template).await?;

        Ok(post)
    }
}

/// `createPostSections`: a blank section per section of the post template.
async fn create_post_sections<C: ConnectionTrait>(
    db: &C,
    post: &posts::Model,
    post_template: &post_templates::Model,
) -> ApiResult<()> {
    let post_template_sections = post_template_sections::Entity::find()
        .filter(post_template_sections::Column::PostTemplateId.eq(post_template.id))
        .order_by_asc(post_template_sections::Column::Position)
        .all(db)
        .await?;

    for post_template_section in post_template_sections {
        let label = match value_or_null(post_template_section.label) {
            Some(label) => label,
            None => post_template_section
                .label_translation_key
                .map(|key| i18n::t(&i18n::current(), &key, &[]))
                .ok_or_else(|| ApiError::internal("post template section without a label"))?,
        };
        post_sections::ActiveModel {
            post_id: Set(post.id),
            position: Set(post_template_section.position),
            label: Set(label),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

/// `DestroyPost`.
pub struct DestroyPost {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
}

impl DestroyPost {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id).await?;

        destroy_files(db, &post).await?;
        posts::Entity::delete_by_id(post.id).exec(db).await?;
        Ok(())
    }
}

/// `destroyFiles`.
async fn destroy_files<C: ConnectionTrait>(db: &C, post: &posts::Model) -> ApiResult<()> {
    files::Entity::delete_many().filter(post_photos([post.id])).exec(db).await?;
    Ok(())
}

/// `IncrementPostReadCounter`. F15: a vault viewer may count a read.
pub struct IncrementPostReadCounter {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
}

impl IncrementPostReadCounter {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<posts::Model> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::View))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id).await?;

        increment(db, post).await
    }
}

/// `increment`: `$post->increment('view_count')`.
async fn increment<C: ConnectionTrait>(db: &C, post: posts::Model) -> ApiResult<posts::Model> {
    let now = crate::db::now();
    posts::Entity::update_many()
        .col_expr(posts::Column::ViewCount, Expr::col(posts::Column::ViewCount).add(1))
        .col_expr(posts::Column::UpdatedAt, Expr::value(now))
        .filter(posts::Column::Id.eq(post.id))
        .exec(db)
        .await?;
    Ok(posts::Model { view_count: post.view_count + 1, updated_at: Some(now), ..post })
}

/// `RemoveContactFromPost`. F16: 404 when the contact isn't in the post, so
/// no `removed_from_post` is written for a contact that wasn't removed.
pub struct RemoveContactFromPost {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
    pub contact_id: Uuid,
}

impl RemoveContactFromPost {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<posts::Model> {
        let checked = check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                contact_id: Some(self.contact_id),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id).await?;
        let contact = checked.contact();

        if contact_post::Entity::delete_by_id((post.id, contact.id)).exec(db).await?.rows_affected == 0 {
            return Err(ApiError::not_found());
        }

        update_last_edited_date(db, contact).await?;
        create_feed_item(db, checked.author.id, contact.id, "removed_from_post", &post).await?;

        Ok(post)
    }
}

/// `RemoveTag`.
pub struct RemoveTag {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
    pub tag_id: Id,
}

impl RemoveTag {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<tags::Model> {
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;

        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id).await?;
        let tag = find_tag(db, self.vault_id, self.tag_id).await?;

        post_tag::Entity::delete_by_id((tag.id, post.id)).exec(db).await?;

        Ok(tag)
    }
}

/// `UpdatePost`.
pub struct UpdatePost {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub journal_id: Id,
    pub post_id: Id,
    pub title: Option<String>,
    pub sections: Vec<UpdatePostSection>,
    pub written_at: Option<String>,
}

/// An entry of `UpdatePost::sections`; `content` is `None` when the entry
/// has no `content` key (the section is left as is).
pub struct UpdatePostSection {
    pub id: Id,
    pub content: Option<Option<String>>,
}

impl UpdatePost {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<posts::Model> {
        let mut v = Validator::new();
        v.max_len("title", self.title.as_deref(), 255);
        if self.sections.is_empty() {
            v.fail("sections", "required", &[]);
        }
        let written_at = date_format(&mut v, "written_at", self.written_at.as_deref());
        v.finish()?;
        check(
            db,
            &Check {
                account_id: self.account_id,
                author_id: self.author_id,
                vault: Some((self.vault_id, Some(VaultPermission::Edit))),
                ..Default::default()
            },
        )
        .await?;
        let journal = find_journal(db, self.vault_id, self.journal_id).await?;
        let post = find_post(db, journal.id, self.post_id).await?;

        let mut post: posts::ActiveModel = post.into();
        post.title = Set(self.title);
        post.written_at = Set(written_at.unwrap_or_else(crate::db::now));
        let post = post.update(db).await?;
        update_sections(db, &post, self.sections).await?;

        Ok(post)
    }
}

/// `updateSections`. A section that isn't the post's is a 422 (the PHP
/// calls `update()` on null).
async fn update_sections<C: ConnectionTrait>(
    db: &C,
    post: &posts::Model,
    sections: Vec<UpdatePostSection>,
) -> ApiResult<()> {
    for (i, section) in sections.into_iter().enumerate() {
        let Some(content) = section.content else {
            continue;
        };

        let field = format!("sections[{i}].id");
        let row = post_sections::Entity::find_by_id(section.id)
            .filter(post_sections::Column::PostId.eq(post.id))
            .one(db)
            .await?
            .ok_or_else(|| {
                ApiError::field(&field, "exists", validate::message(&i18n::current(), "exists", &field, &[]))
            })?;
        let mut row: post_sections::ActiveModel = row.into();
        row.content = Set(content);
        row.update(db).await?;
    }
    Ok(())
}

/// `$journal->posts()->findOrFail($id)`.
pub(super) async fn find_post<C: ConnectionTrait>(db: &C, journal_id: Id, post_id: Id) -> ApiResult<posts::Model> {
    posts::Entity::find_by_id(post_id)
        .filter(posts::Column::JournalId.eq(journal_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->vault->tags()->findOrFail($id)`.
async fn find_tag<C: ConnectionTrait>(db: &C, vault_id: Uuid, tag_id: Id) -> ApiResult<tags::Model> {
    tags::Entity::find_by_id(tag_id)
        .filter(tags::Column::VaultId.eq(vault_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `createFeedItem`: an `$action` feed item on the contact, attached to the post.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    author_id: Uuid,
    contact_id: Uuid,
    action: &str,
    post: &posts::Model,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author_id)),
        contact_id: Set(contact_id),
        action: Set(action.into()),
        description: Set(Some(title(post))),
        feedable_type: Set(Some(POST.into())),
        feedable_id: Set(Some(post.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// `updateLastEditedDate`.
async fn update_last_edited_date<C: ConnectionTrait>(db: &C, contact: &contacts::Model) -> ApiResult<()> {
    let mut contact: contacts::ActiveModel = contact.clone().into();
    contact.last_updated_at = Set(Some(crate::db::now()));
    contact.update(db).await?;
    Ok(())
}

/// `Post::title()`: `Undefined` when the post has no title.
fn title(post: &posts::Model) -> String {
    value_or_null(post.title.clone()).unwrap_or_else(|| i18n::t(&i18n::current(), "Undefined", &[]))
}
