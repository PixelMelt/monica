//! `app/Domains/Contact/ManageNotes/Services`.

#[cfg(test)]
mod tests;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::id::Id;
use crate::db::uuid::Uuid;
use crate::domains::base::{Check, VaultPermission, check, value_or_null};
use crate::entities::{contact_feed_items, contacts, emotions, notes, users};
use crate::error::{ApiError, ApiResult};
use crate::validate::Validator;

/// `ContactFeedItem::ACTION_NOTE_CREATED`.
const ACTION_NOTE_CREATED: &str = "note_created";
/// `ContactFeedItem::ACTION_NOTE_UPDATED`.
const ACTION_NOTE_UPDATED: &str = "note_updated";
/// `ContactFeedItem::ACTION_NOTE_DESTROYED`.
const ACTION_NOTE_DESTROYED: &str = "note_destroyed";

/// `CreateNote`.
pub struct CreateNote {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub emotion_id: Option<Id>,
    pub title: Option<String>,
    pub body: String,
}

impl CreateNote {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<notes::Model> {
        let mut v = Validator::new();
        emotion_exists(db, &mut v, self.emotion_id).await?;
        v.max_len("title", self.title.as_deref(), 255);
        body_rules(&mut v, &self.body);
        v.finish()?;
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

        if let Some(emotion_id) = self.emotion_id {
            find_emotion(db, checked.author.account_id, emotion_id).await?;
        }

        let note = notes::ActiveModel {
            contact_id: Set(checked.contact().id),
            vault_id: Set(self.vault_id),
            author_id: Set(Some(checked.author.id)),
            title: Set(value_or_null(self.title)),
            body: Set(self.body),
            emotion_id: Set(self.emotion_id),
            ..Default::default()
        }
        .insert(db)
        .await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        create_feed_item(db, &checked.author, checked.contact(), &note, ACTION_NOTE_CREATED).await?;

        Ok(note)
    }
}

/// `DestroyNote`.
pub struct DestroyNote {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub note_id: Id,
}

impl DestroyNote {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<()> {
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

        let note = find(db, checked.contact().id, self.note_id).await?;

        Self::create_feed_item(db, &checked.author, checked.contact(), &note).await?;
        notes::Entity::delete_by_id(note.id).exec(db).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        Ok(())
    }

    async fn create_feed_item<C: ConnectionTrait>(
        db: &C,
        author: &users::Model,
        contact: &contacts::Model,
        note: &notes::Model,
    ) -> ApiResult<()> {
        contact_feed_items::ActiveModel {
            author_id: Set(Some(author.id)),
            contact_id: Set(contact.id),
            action: Set(ACTION_NOTE_DESTROYED.into()),
            description: Set(Some(words(&note.body, 10, "…"))),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(())
    }
}

/// `UpdateNote`.
pub struct UpdateNote {
    pub account_id: Uuid,
    pub vault_id: Uuid,
    pub author_id: Uuid,
    pub contact_id: Uuid,
    pub note_id: Id,
    pub emotion_id: Option<Id>,
    pub title: Option<String>,
    pub body: String,
}

impl UpdateNote {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<notes::Model> {
        let mut v = Validator::new();
        emotion_exists(db, &mut v, self.emotion_id).await?;
        v.max_len("title", self.title.as_deref(), 255);
        body_rules(&mut v, &self.body);
        v.finish()?;
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

        let note = find(db, checked.contact().id, self.note_id).await?;

        if let Some(emotion_id) = self.emotion_id {
            find_emotion(db, checked.author.account_id, emotion_id).await?;
        }

        let mut note: notes::ActiveModel = note.into();
        note.body = Set(self.body);
        note.title = Set(value_or_null(self.title));
        note.emotion_id = Set(self.emotion_id);
        let note = note.update(db).await?;

        let mut contact: contacts::ActiveModel = checked.contact().clone().into();
        contact.last_updated_at = Set(Some(crate::db::now()));
        contact.update(db).await?;

        create_feed_item(db, &checked.author, checked.contact(), &note, ACTION_NOTE_UPDATED).await?;

        Ok(note)
    }
}

/// `createFeedItem()` of CreateNote and UpdateNote, which differ only by the
/// action.
async fn create_feed_item<C: ConnectionTrait>(
    db: &C,
    author: &users::Model,
    contact: &contacts::Model,
    note: &notes::Model,
    action: &str,
) -> ApiResult<()> {
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author.id)),
        contact_id: Set(contact.id),
        action: Set(action.into()),
        description: Set(Some(words(&note.body, 10, "…"))),
        feedable_type: Set(Some("App\\Models\\Note".into())),
        feedable_id: Set(Some(note.id)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// `'emotion_id' => 'nullable|integer|exists:emotions,id'`.
async fn emotion_exists<C: ConnectionTrait>(db: &C, v: &mut Validator, emotion_id: Option<Id>) -> ApiResult<()> {
    if let Some(id) = emotion_id
        && emotions::Entity::find_by_id(id).one(db).await?.is_none()
    {
        v.fail("emotion_id", "exists", &[]);
    }
    Ok(())
}

/// `'body' => 'required|string|max:65535'`.
fn body_rules(v: &mut Validator, body: &str) {
    if body.trim().is_empty() {
        v.fail("body", "required", &[]);
    }
    v.max_len("body", Some(body), 65535);
}

/// `$this->account()->emotions()->findOrFail($id)`.
async fn find_emotion<C: ConnectionTrait>(db: &C, account_id: Uuid, emotion_id: Id) -> ApiResult<emotions::Model> {
    emotions::Entity::find_by_id(emotion_id)
        .filter(emotions::Column::AccountId.eq(account_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `$this->contact->notes()->findOrFail($id)`.
async fn find<C: ConnectionTrait>(db: &C, contact_id: Uuid, note_id: Id) -> ApiResult<notes::Model> {
    notes::Entity::find_by_id(note_id)
        .filter(notes::Column::ContactId.eq(contact_id))
        .one(db)
        .await?
        .ok_or_else(ApiError::not_found)
}

/// `Str::words($value, $words, $end)`: the first `words` words of `value`
/// followed by `end`, or `value` itself when it has no more words.
fn words(value: &str, words: usize, end: &str) -> String {
    let mut matched = value.len() - value.trim_start().len();
    let mut count = 0;
    while count < words {
        let rest = &value[matched..];
        let word = rest.len() - rest.trim_start_matches(|c: char| !c.is_whitespace()).len();
        if word == 0 {
            break;
        }
        let after = &rest[word..];
        matched += word + after.len() - after.trim_start().len();
        count += 1;
    }
    if count == 0 || matched == value.len() {
        return value.to_owned();
    }
    format!("{}{end}", value[..matched].trim_end_matches([' ', '\t', '\n', '\r', '\0', '\x0B']))
}
