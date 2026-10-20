//! `tests/Unit/Domains/Contact/ManageNotes/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

use super::*;
use crate::crypto::random_string;
use crate::entities::vaults;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `Contact::factory()->create()`: a contact in a new vault.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    factory::contact(&db.conn, &vault).await
}

/// `Note::factory()`: a new contact and author unless given.
async fn note(db: &TestDb, contact_id: Option<Uuid>, author_id: Option<Uuid>) -> notes::Model {
    let contact_id = match contact_id {
        Some(id) => id,
        None => other_contact(db).await.id,
    };
    let account = factory::create_account(&db.conn).await;
    let vault = factory::create_vault(&db.conn, account.id).await;
    let author_id = match author_id {
        Some(id) => id,
        None => factory::create_user(&db.conn).await.id,
    };
    notes::ActiveModel {
        contact_id: Set(contact_id),
        vault_id: Set(vault.id),
        author_id: Set(Some(author_id)),
        title: Set(Some("Mr.".into())),
        body: Set(format!("Body {} of the note.", random_string(6))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("note")
}

/// `Emotion::factory()` in `account_id`.
async fn emotion(db: &TestDb, account_id: Uuid) -> emotions::Model {
    emotions::ActiveModel {
        account_id: Set(account_id),
        name: Set(Some(format!("Name {}", random_string(6)))),
        r#type: Set(format!("Type {}", random_string(6))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("emotion")
}

/// `ContactFeedItem::factory()` for `contact_id`, about the note `feedable_id`.
async fn feed_item(db: &TestDb, contact_id: Uuid, feedable_id: Id) -> contact_feed_items::Model {
    let author = factory::create_user(&db.conn).await;
    contact_feed_items::ActiveModel {
        author_id: Set(Some(author.id)),
        contact_id: Set(contact_id),
        action: Set(ACTION_NOTE_CREATED.into()),
        description: Set(Some(random_string(6))),
        feedable_id: Set(Some(feedable_id)),
        feedable_type: Set(Some("App\\Models\\Note".into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("feed item")
}

/// `createUser()`, `createVault()` and `setPermissionInVault()` with `permission`.
async fn regis_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

#[test]
fn words_follows_str_words() {
    assert_eq!(words("one two three", 10, "…"), "one two three");
    assert_eq!(words("  one two  ", 2, "…"), "  one two  ");
    assert_eq!(words("one two three four", 2, "…"), "one two…");
    assert_eq!(words("one  two\nthree", 2, "…"), "one  two…");
    assert_eq!(words("", 10, "…"), "");
}

// ---------------------------------------------------------------- CreateNoteTest

async fn create_note(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    emotion: Option<&emotions::Model>,
) -> ApiResult<()> {
    let note = CreateNote {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        emotion_id: emotion.map(|e| e.id),
        title: Some("super title".into()),
        body: "super body".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = notes::Entity::find_by_id(note.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.author_id, Some(author.id));
    assert_eq!(row.title.as_deref(), Some("super title"));
    assert_eq!(row.body, "super body");
    assert_eq!(row.emotion_id, emotion.map(|e| e.id));

    assert!(
        contact_feed_items::Entity::find()
            .filter(contact_feed_items::Column::ContactId.eq(contact.id))
            .filter(contact_feed_items::Column::FeedableId.eq(note.id))
            .filter(contact_feed_items::Column::FeedableType.eq("App\\Models\\Note"))
            .one(&db.conn)
            .await
            .unwrap()
            .is_some()
    );
    Ok(())
}

#[tokio::test]
async fn it_creates_a_note() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    create_note(&db, &regis, regis.account_id, &vault, &contact, None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_creates_a_note_with_emotion() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let emotion = emotion(&db, regis.account_id).await;
    create_note(&db, &regis, regis.account_id, &vault, &contact, Some(&emotion)).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = CreateNote {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        emotion_id: None,
        title: Some("Ross".into()),
        body: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = create_note(&db, &regis, account.id, &vault, &contact, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let err = create_note(&db, &regis, regis.account_id, &vault, &contact, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let err = create_note(&db, &regis, regis.account_id, &vault, &contact, None).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyNoteTest

async fn destroy_note(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    note: &notes::Model,
) -> ApiResult<()> {
    DestroyNote { account_id, vault_id: vault.id, author_id: author.id, contact_id: contact.id, note_id: note.id }
        .execute(&db.conn)
        .await?;

    assert!(notes::Entity::find_by_id(note.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_note() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, Some(contact.id), None).await;
    feed_item(&db, contact.id, note.id).await;
    destroy_note(&db, &regis, regis.account_id, &vault, &contact, &note).await.unwrap();
    db.drop().await;
}

// destroy_fails_if_wrong_parameters_are_given: every field is typed, no invalid input can be built.

#[tokio::test]
async fn destroy_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, Some(contact.id), None).await;
    let err = destroy_note(&db, &regis, account.id, &vault, &contact, &note).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let note = note(&db, Some(contact.id), None).await;
    let err = destroy_note(&db, &regis, regis.account_id, &vault, &contact, &note).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, Some(contact.id), None).await;
    let err = destroy_note(&db, &regis, regis.account_id, &vault, &contact, &note).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_fails_if_note_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, None, None).await;
    let err = destroy_note(&db, &regis, regis.account_id, &vault, &contact, &note).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateNoteTest

async fn update_note(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    note: &notes::Model,
    emotion: Option<&emotions::Model>,
) -> ApiResult<()> {
    let note = UpdateNote {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        contact_id: contact.id,
        note_id: note.id,
        emotion_id: emotion.map(|e| e.id),
        title: Some("super title".into()),
        body: "super body".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = notes::Entity::find_by_id(note.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.contact_id, contact.id);
    assert_eq!(row.author_id, Some(author.id));
    assert_eq!(row.title.as_deref(), Some("super title"));
    assert_eq!(row.body, "super body");
    assert_eq!(row.emotion_id, emotion.map(|e| e.id));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_note() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, Some(contact.id), Some(regis.id)).await;
    update_note(&db, &regis, regis.account_id, &vault, &contact, &note, None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_updates_a_note_with_emotion() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, Some(contact.id), Some(regis.id)).await;
    let emotion = emotion(&db, regis.account_id).await;
    update_note(&db, &regis, regis.account_id, &vault, &contact, &note, Some(&emotion)).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, Some(contact.id), Some(regis.id)).await;
    let err = UpdateNote {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        contact_id: contact.id,
        note_id: note.id,
        emotion_id: None,
        title: Some("Ross".into()),
        body: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, Some(contact.id), Some(regis.id)).await;
    let err = update_note(&db, &regis, account.id, &vault, &contact, &note, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let note = note(&db, Some(contact.id), Some(regis.id)).await;
    let err = update_note(&db, &regis, regis.account_id, &vault, &contact, &note, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, Some(contact.id), Some(regis.id)).await;
    let err = update_note(&db, &regis, regis.account_id, &vault, &contact, &note, None).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_fails_if_note_is_not_in_the_contact() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let note = note(&db, None, Some(regis.id)).await;
    let err = update_note(&db, &regis, regis.account_id, &vault, &contact, &note, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
