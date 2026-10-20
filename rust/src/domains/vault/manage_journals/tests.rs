//! `tests/Unit/Domains/Vault/ManageJournals/Services`.

use chrono::NaiveDate;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

use super::*;
use crate::crypto::random_string;
use crate::domains::base::VaultPermission;
use crate::entities::{
    contact_feed_items, contact_post, contacts, journal_metrics, journals, post_metrics, post_sections, post_tag,
    post_template_sections, post_templates, posts, tags, users, vaults,
};
use crate::error::Code;
use crate::testing::{TestDb, factory};

fn name() -> String {
    format!("Name {}", random_string(6))
}

fn sentence() -> String {
    format!("Sentence {}.", random_string(12))
}

/// `createUser()`, `createVault($regis->account)` and
/// `setPermissionInVault($regis, $permission, $vault)`.
async fn regis(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    factory::set_permission_in_vault(&db.conn, &regis, permission, &vault).await;
    (regis, vault)
}

/// `Vault::factory()`: a vault in a new account.
async fn other_vault(db: &TestDb) -> vaults::Model {
    let account = factory::account(&db.conn).await;
    factory::vault(&db.conn, account.id).await
}

/// `Contact::factory()` in `vault` (a new vault when `None`).
async fn contact(db: &TestDb, vault: Option<&vaults::Model>) -> contacts::Model {
    match vault {
        Some(v) => factory::contact(&db.conn, v).await,
        None => factory::contact(&db.conn, &other_vault(db).await).await,
    }
}

/// `Journal::factory()` in `vault_id` (a new vault when `None`).
async fn journal(db: &TestDb, vault_id: Option<Uuid>) -> journals::Model {
    let vault_id = match vault_id {
        Some(id) => id,
        None => other_vault(db).await.id,
    };
    journals::ActiveModel {
        vault_id: Set(vault_id),
        name: Set(name()),
        description: Set(Some(name())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("journal")
}

/// `Post::factory()` in `journal_id` (a new journal when `None`).
async fn post(db: &TestDb, journal_id: Option<Id>) -> posts::Model {
    let journal_id = match journal_id {
        Some(id) => id,
        None => journal(db, None).await.id,
    };
    let written_at =
        NaiveDate::from_ymd_opt(rand::random_range(2000..2026), rand::random_range(1..=12), rand::random_range(1..=28))
            .and_then(|d| d.and_hms_opt(rand::random_range(0..24), rand::random_range(0..60), 0))
            .expect("date");
    posts::ActiveModel {
        journal_id: Set(journal_id),
        title: Set(Some(sentence())),
        published: Set(false),
        written_at: Set(Timestamp(written_at)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("post")
}

/// `Post::factory()->create(['journal_id' => …, 'slice_of_life_id' => …])`.
async fn post_in_slice(db: &TestDb, journal_id: Option<Id>, slice_of_life_id: Id) -> posts::Model {
    let mut p: posts::ActiveModel = post(db, journal_id).await.into();
    p.slice_of_life_id = Set(Some(slice_of_life_id));
    p.update(&db.conn).await.expect("post")
}

/// `PostSection::factory()` of `post_id`.
async fn post_section(db: &TestDb, post_id: Id, content: &str) -> post_sections::Model {
    post_sections::ActiveModel {
        post_id: Set(post_id),
        label: Set("label".into()),
        position: Set(rand::random_range(1..=10)),
        content: Set(Some(content.into())),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("post section")
}

/// `SliceOfLife::factory()` in `journal_id` (a new journal when `None`).
async fn slice_of_life(db: &TestDb, journal_id: Option<Id>) -> slice_of_lives::Model {
    let journal_id = match journal_id {
        Some(id) => id,
        None => journal(db, None).await.id,
    };
    slice_of_lives::ActiveModel { journal_id: Set(journal_id), name: Set(name()), ..Default::default() }
        .insert(&db.conn)
        .await
        .expect("slice of life")
}

/// `JournalMetric::factory()` in `journal_id` (a new journal when `None`).
async fn journal_metric(db: &TestDb, journal_id: Option<Id>) -> journal_metrics::Model {
    let journal_id = match journal_id {
        Some(id) => id,
        None => journal(db, None).await.id,
    };
    journal_metrics::ActiveModel { journal_id: Set(journal_id), label: Set(sentence()), ..Default::default() }
        .insert(&db.conn)
        .await
        .expect("journal metric")
}

/// `PostMetric::factory()` of `post_id` and `journal_metric_id` (new ones when `None`).
async fn post_metric(db: &TestDb, post_id: Option<Id>, journal_metric_id: Option<Id>) -> post_metrics::Model {
    let post_id = match post_id {
        Some(id) => id,
        None => post(db, None).await.id,
    };
    let journal_metric_id = match journal_metric_id {
        Some(id) => id,
        None => journal_metric(db, None).await.id,
    };
    post_metrics::ActiveModel {
        post_id: Set(post_id),
        journal_metric_id: Set(journal_metric_id),
        label: Set(Some(sentence())),
        value: Set(rand::random_range(0..100_000)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("post metric")
}

/// `File::factory()` in `vault_id` (a new vault when `None`).
async fn file(db: &TestDb, vault_id: Option<Uuid>) -> files::Model {
    let vault_id = match vault_id {
        Some(id) => id,
        None => other_vault(db).await.id,
    };
    files::ActiveModel {
        vault_id: Set(vault_id),
        uuid: Set(Uuid::new().to_string()),
        original_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        cdn_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        name: Set(name()),
        mime_type: Set("avatar".into()),
        r#type: Set("avatar".into()),
        size: Set(rand::random_range(200 * 1024 * 1024..=500 * 1024 * 1024)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("file")
}

/// `Tag::factory()` in `vault_id` (a new vault when `None`).
async fn tag(db: &TestDb, vault_id: Option<Uuid>) -> tags::Model {
    let vault_id = match vault_id {
        Some(id) => id,
        None => other_vault(db).await.id,
    };
    tags::ActiveModel { vault_id: Set(vault_id), name: Set(name()), slug: Set(name()), ..Default::default() }
        .insert(&db.conn)
        .await
        .expect("tag")
}

/// `PostTemplate::factory()` in `account_id` (a new account when `None`).
async fn post_template(db: &TestDb, account_id: Option<Uuid>) -> post_templates::Model {
    let account_id = match account_id {
        Some(id) => id,
        None => factory::account(&db.conn).await.id,
    };
    post_templates::ActiveModel {
        account_id: Set(account_id),
        label: Set(Some("business".into())),
        position: Set(1),
        can_be_deleted: Set(false),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("post template")
}

async fn file_row(db: &TestDb, id: Id) -> Option<files::Model> {
    files::Entity::find_by_id(id).one(&db.conn).await.unwrap()
}

async fn slice_row(db: &TestDb, id: Id) -> Option<slice_of_lives::Model> {
    slice_of_lives::Entity::find_by_id(id).one(&db.conn).await.unwrap()
}

async fn post_row(db: &TestDb, id: Id) -> Option<posts::Model> {
    posts::Entity::find_by_id(id).one(&db.conn).await.unwrap()
}

// ---------------------------------------------------------------- AddContactToPostTest

async fn add_contact_to_post(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    journal: &journals::Model,
    post: &posts::Model,
) -> ApiResult<()> {
    AddContactToPost {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
        contact_id: contact.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(contact_post::Entity::find_by_id((post.id, contact.id)).one(&db.conn).await.unwrap().is_some());
    Ok(())
}

#[tokio::test]
async fn it_adds_a_contact_to_a_post() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    add_contact_to_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap();
    db.drop().await;
}

// it_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = add_contact_to_post(&db, &regis, account.id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let err = add_contact_to_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, None).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = add_contact_to_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, None).await;
    let err = add_contact_to_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = add_contact_to_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn add_contact_to_post_writes_the_feed_item_only_when_the_contact_is_new() {
    // F16: the PHP wrote an `added_to_post` feed item on every call.
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    add_contact_to_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap();
    add_contact_to_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap();

    let items = contact_feed_items::Entity::find()
        .filter(contact_feed_items::Column::ContactId.eq(contact.id))
        .all(&db.conn)
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].author_id, Some(regis.id));
    assert_eq!(items[0].action, "added_to_post");
    assert_eq!(items[0].description, post.title);
    assert_eq!(items[0].feedable_type.as_deref(), Some("App\\Models\\Post"));
    assert_eq!(items[0].feedable_id, Some(post.id));
    let row = contacts::Entity::find_by_id(contact.id).one(&db.conn).await.unwrap().unwrap();
    assert!(row.last_updated_at.is_some());
    let mentions =
        contact_post::Entity::find().filter(contact_post::Column::PostId.eq(post.id)).count(&db.conn).await.unwrap();
    assert_eq!(mentions, 1);
    db.drop().await;
}

// ---------------------------------------------------------------- AddPhotoToPostTest

async fn add_photo_to_post(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    file: &files::Model,
    post: &posts::Model,
) -> ApiResult<()> {
    AddPhotoToPost {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: Some(post.id),
        file_id: file.id,
    }
    .execute(&db.conn)
    .await?;

    let row = file_row(db, file.id).await.unwrap();
    assert_eq!(row.fileable_id, Some(post.id));
    assert_eq!(row.fileable_type.as_deref(), Some("App\\Models\\Post"));
    assert_eq!(row.r#type, "photo");
    Ok(())
}

#[tokio::test]
async fn it_adds_an_image() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let file = file(&db, Some(vault.id)).await;
    add_photo_to_post(&db, &regis, regis.account_id, &vault, &journal, &file, &post).await.unwrap();
    db.drop().await;
}

// add_photo_to_post_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn add_photo_to_post_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let file = file(&db, Some(vault.id)).await;
    let err = add_photo_to_post(&db, &regis, account.id, &vault, &journal, &file, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn add_photo_to_post_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let file = file(&db, Some(vault.id)).await;
    let err = add_photo_to_post(&db, &regis, regis.account_id, &vault, &journal, &file, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn add_photo_to_post_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, None).await;
    let file = file(&db, Some(vault.id)).await;
    let err = add_photo_to_post(&db, &regis, regis.account_id, &vault, &journal, &file, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn add_photo_to_post_fails_if_file_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let file = file(&db, None).await;
    let err = add_photo_to_post(&db, &regis, regis.account_id, &vault, &journal, &file, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn add_photo_to_post_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let file = file(&db, Some(vault.id)).await;
    let err = add_photo_to_post(&db, &regis, regis.account_id, &vault, &journal, &file, &post).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- AddPostToSliceOfLifeTest

async fn add_post_to_slice_of_life(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
    slice: &slice_of_lives::Model,
) -> ApiResult<()> {
    AddPostToSliceOfLife {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
        slice_of_life_id: slice.id,
    }
    .execute(&db.conn)
    .await?;

    assert_eq!(post_row(db, post.id).await.unwrap().slice_of_life_id, Some(slice.id));
    Ok(())
}

#[tokio::test]
async fn it_adds_a_post_to_a_slice_of_life() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post(&db, Some(journal.id)).await;
    add_post_to_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &post, &slice).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn add_post_to_slice_of_life_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = AddPostToSliceOfLife {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        post_id: post.id,
        slice_of_life_id: Id(0),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn add_post_to_slice_of_life_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = add_post_to_slice_of_life(&db, &regis, account.id, &vault, &journal, &post, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn add_post_to_slice_of_life_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err =
        add_post_to_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &post, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn add_post_to_slice_of_life_fails_if_slice_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let err =
        add_post_to_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &post, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn add_post_to_slice_of_life_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post(&db, None).await;
    let err =
        add_post_to_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &post, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn add_post_to_slice_of_life_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err =
        add_post_to_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &post, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- AssignTagTest

async fn assign_tag(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
    tag: &tags::Model,
) -> ApiResult<()> {
    AssignTag {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
        tag_id: tag.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(post_tag::Entity::find_by_id((tag.id, post.id)).one(&db.conn).await.unwrap().is_some());
    Ok(())
}

#[tokio::test]
async fn it_assigns_a_tag() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    assign_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap();
    db.drop().await;
}

// assign_tag_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn assign_tag_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = assign_tag(&db, &regis, account.id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn assign_tag_fails_if_tag_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let tag = tag(&db, None).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = assign_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn assign_tag_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let err = assign_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn assign_tag_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, None).await;
    let err = assign_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn assign_tag_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = assign_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateJournalMetricTest

async fn create_journal_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
) -> ApiResult<()> {
    let journal_metric = CreateJournalMetric {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        label: "this is a title".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = journal_metrics::Entity::find_by_id(journal_metric.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.journal_id, journal.id);
    assert_eq!(row.label, "this is a title");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_journal_metric() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    create_journal_metric(&db, &regis, regis.account_id, &vault, &journal).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_journal_metric_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = CreateJournalMetric {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        label: String::new(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_journal_metric_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = create_journal_metric(&db, &regis, account.id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_journal_metric_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = create_journal_metric(&db, &regis, regis.account_id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_journal_metric_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let err = create_journal_metric(&db, &regis, regis.account_id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateJournalTest

async fn create_journal(db: &TestDb, author: &users::Model, account_id: Uuid, vault: &vaults::Model) -> ApiResult<()> {
    let journal = CreateJournal {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        name: "super journal".into(),
        description: None,
    }
    .execute(&db.conn)
    .await?;

    let row = journals::Entity::find_by_id(journal.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.name, "super journal");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    create_journal(&db, &regis, regis.account_id, &vault).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_journal_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let err = CreateJournal {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        name: String::new(),
        description: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_journal_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let err = create_journal(&db, &regis, account.id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_journal_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let err = create_journal(&db, &regis, regis.account_id, &vault).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- CreatePostMetricTest

async fn create_post_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
    journal_metric: &journal_metrics::Model,
) -> ApiResult<()> {
    let post_metric = CreatePostMetric {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
        journal_metric_id: journal_metric.id,
        value: 1,
        label: Some("this is a title".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = post_metrics::Entity::find_by_id(post_metric.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.journal_metric_id, journal_metric.id);
    assert_eq!(row.post_id, post.id);
    assert_eq!(row.value, 1);
    assert_eq!(row.label.as_deref(), Some("this is a title"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_post_metric() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    create_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &journal_metric).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_post_metric_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err = CreatePostMetric {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        post_id: post.id,
        journal_metric_id: journal_metric.id,
        value: 1,
        label: Some("x".repeat(256)),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_post_metric_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err = create_post_metric(&db, &regis, account.id, &vault, &journal, &post, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_post_metric_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err =
        create_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_post_metric_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err =
        create_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_post_metric_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, None).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err =
        create_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_post_metric_fails_if_journal_metric_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, None).await;
    let err =
        create_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- CreatePostTest

async fn create_post(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post_template: Option<&post_templates::Model>,
) -> ApiResult<()> {
    let post_template = match post_template {
        Some(t) => t.clone(),
        None => self::post_template(db, Some(account_id)).await,
    };

    let post = CreatePost {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_template_id: post_template.id,
        title: Some("title".into()),
        published: true,
        written_at: None,
    }
    .execute(&db.conn)
    .await?;

    let row = post_row(db, post.id).await.unwrap();
    assert_eq!(row.journal_id, journal.id);
    assert_eq!(row.title.as_deref(), Some("title"));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_post() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    create_post(&db, &regis, regis.account_id, &vault, &journal, None).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_post_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post_template = post_template(&db, Some(regis.account_id)).await;
    let err = CreatePost {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        post_template_id: post_template.id,
        title: None,
        published: true,
        written_at: Some("2022-13-01".into()),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_post_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = create_post(&db, &regis, account.id, &vault, &journal, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_post_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let err = create_post(&db, &regis, regis.account_id, &vault, &journal, None).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_post_fails_if_post_template_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let post_template = post_template(&db, None).await;
    let err = create_post(&db, &regis, regis.account_id, &vault, &journal, Some(&post_template)).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_post_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = create_post(&db, &regis, regis.account_id, &vault, &journal, None).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_post_creates_the_sections_of_the_template() {
    // Not in the PHP tests: `createPostSections`, with the label accessor.
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post_template = post_template(&db, Some(regis.account_id)).await;
    for (position, label, key) in [(2, None, Some("Undefined")), (1, Some("first"), None)] {
        post_template_sections::ActiveModel {
            post_template_id: Set(post_template.id),
            label: Set(label.map(Into::into)),
            label_translation_key: Set(key.map(Into::into)),
            position: Set(position),
            can_be_deleted: Set(true),
            ..Default::default()
        }
        .insert(&db.conn)
        .await
        .unwrap();
    }
    let post = CreatePost {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        post_template_id: post_template.id,
        title: None,
        published: false,
        written_at: Some("2022-03-04".into()),
    }
    .execute(&db.conn)
    .await
    .unwrap();

    let row = post_row(&db, post.id).await.unwrap();
    assert_eq!(row.title, None);
    assert_eq!(row.written_at.to_string(), "2022-03-04T00:00:00Z");
    let mut sections = post_sections::Entity::find()
        .filter(post_sections::Column::PostId.eq(post.id))
        .all(&db.conn)
        .await
        .unwrap()
        .into_iter()
        .map(|s| (s.position, s.label, s.content))
        .collect::<Vec<_>>();
    sections.sort();
    assert_eq!(sections, vec![(1, "first".to_owned(), None), (2, "Undefined".to_owned(), None)]);
    db.drop().await;
}

// ---------------------------------------------------------------- CreateSliceOfLifeTest

async fn create_slice_of_life(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
) -> ApiResult<()> {
    let slice = CreateSliceOfLife {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        name: "this is a title".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = slice_row(db, slice.id).await.unwrap();
    assert_eq!(row.journal_id, journal.id);
    assert_eq!(row.name, "this is a title");
    Ok(())
}

#[tokio::test]
async fn it_creates_a_slice_of_life() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    create_slice_of_life(&db, &regis, regis.account_id, &vault, &journal).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn create_slice_of_life_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = CreateSliceOfLife {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        name: "x".repeat(256),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_slice_of_life_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = create_slice_of_life(&db, &regis, account.id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_slice_of_life_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = create_slice_of_life(&db, &regis, regis.account_id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn create_slice_of_life_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let err = create_slice_of_life(&db, &regis, regis.account_id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyJournalMetricTest

async fn destroy_journal_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    journal_metric: &journal_metrics::Model,
) -> ApiResult<()> {
    DestroyJournalMetric {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        journal_metric_id: journal_metric.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(journal_metrics::Entity::find_by_id(journal_metric.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_journal_metric() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    destroy_journal_metric(&db, &regis, regis.account_id, &vault, &journal, &journal_metric).await.unwrap();
    db.drop().await;
}

// destroy_journal_metric_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_journal_metric_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err = destroy_journal_metric(&db, &regis, account.id, &vault, &journal, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_journal_metric_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err =
        destroy_journal_metric(&db, &regis, regis.account_id, &vault, &journal, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_journal_metric_fails_if_journal_metric_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let journal_metric = journal_metric(&db, None).await;
    let err =
        destroy_journal_metric(&db, &regis, regis.account_id, &vault, &journal, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_journal_metric_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err =
        destroy_journal_metric(&db, &regis, regis.account_id, &vault, &journal, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_journal_metric_fails_if_reminder_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err =
        destroy_journal_metric(&db, &regis, regis.account_id, &vault, &journal, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyJournalTest

async fn destroy_journal(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
) -> ApiResult<()> {
    DestroyJournal { account_id, vault_id: vault.id, author_id: author.id, journal_id: journal.id }
        .execute(&db.conn)
        .await?;

    assert!(journals::Entity::find_by_id(journal.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    destroy_journal(&db, &regis, regis.account_id, &vault, &journal).await.unwrap();
    db.drop().await;
}

// destroy_journal_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_journal_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = destroy_journal(&db, &regis, account.id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_journal_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let err = destroy_journal(&db, &regis, regis.account_id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_journal_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = destroy_journal(&db, &regis, regis.account_id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_journal_fails_if_reminder_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let err = destroy_journal(&db, &regis, regis.account_id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_journal_deletes_the_files_of_its_photos_and_covers() {
    // Q7: the PHP left these `files` rows behind.
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let photo = file(&db, Some(vault.id)).await;
    AddPhotoToPost {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        post_id: Some(post.id),
        file_id: photo.id,
    }
    .execute(&db.conn)
    .await
    .unwrap();
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let cover = file(&db, Some(vault.id)).await;
    let mut s: slice_of_lives::ActiveModel = slice.into();
    s.file_cover_image_id = Set(Some(cover.id));
    s.update(&db.conn).await.unwrap();
    let other_journal = self::journal(&db, Some(vault.id)).await;
    let other_post = self::post(&db, Some(other_journal.id)).await;
    let other_photo = file(&db, Some(vault.id)).await;
    AddPhotoToPost {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: other_journal.id,
        post_id: Some(other_post.id),
        file_id: other_photo.id,
    }
    .execute(&db.conn)
    .await
    .unwrap();
    let unrelated = file(&db, Some(vault.id)).await;

    destroy_journal(&db, &regis, regis.account_id, &vault, &journal).await.unwrap();

    assert!(file_row(&db, photo.id).await.is_none());
    assert!(file_row(&db, cover.id).await.is_none());
    assert!(file_row(&db, other_photo.id).await.is_some());
    assert!(file_row(&db, unrelated.id).await.is_some());
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyPostMetricTest

async fn destroy_post_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
    post_metric: &post_metrics::Model,
) -> ApiResult<()> {
    DestroyPostMetric {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
        post_metric_id: post_metric.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(post_metrics::Entity::find_by_id(post_metric.id).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_post_metric() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, Some(post.id), None).await;
    destroy_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &post_metric).await.unwrap();
    db.drop().await;
}

// destroy_post_metric_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_post_metric_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, Some(post.id), None).await;
    let err = destroy_post_metric(&db, &regis, account.id, &vault, &journal, &post, &post_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_post_metric_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, Some(post.id), None).await;
    let err =
        destroy_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &post_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_post_metric_fails_if_post_metric_doesnt_belong_to_post() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, None, None).await;
    let err =
        destroy_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &post_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_post_metric_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, Some(post.id), None).await;
    let err =
        destroy_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &post_metric).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroyPostTest

async fn destroy_post(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
) -> ApiResult<()> {
    DestroyPost { account_id, vault_id: vault.id, author_id: author.id, journal_id: journal.id, post_id: post.id }
        .execute(&db.conn)
        .await?;

    assert!(post_row(db, post.id).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_post() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    destroy_post(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap();
    db.drop().await;
}

// destroy_post_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_post_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = destroy_post(&db, &regis, account.id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_post_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let err = destroy_post(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_post_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, None).await;
    let err = destroy_post(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_post_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = destroy_post(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_post_fails_if_reminder_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let err = destroy_post(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---------------------------------------------------------------- DestroySliceOfLifeTest

async fn destroy_slice_of_life(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    slice: &slice_of_lives::Model,
) -> ApiResult<()> {
    DestroySliceOfLife {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        slice_of_life_id: slice.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(slice_row(db, slice.id).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_slice_of_life() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    destroy_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap();
    db.drop().await;
}

// destroy_slice_of_life_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_slice_of_life_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err = destroy_slice_of_life(&db, &regis, account.id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_slice_of_life_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err = destroy_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_slice_of_life_fails_if_slice_of_life_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, None).await;
    let err = destroy_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_slice_of_life_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err = destroy_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_slice_of_life_fails_if_reminder_does_not_exist() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err = destroy_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_slice_of_life_deletes_the_files_of_its_covers() {
    // Q7: the PHP left the cover's `files` row behind.
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let earlier_cover = file(&db, Some(vault.id)).await;
    let mut f: files::ActiveModel = earlier_cover.clone().into();
    f.fileable_type = Set(Some("App\\Models\\SliceOfLife".into()));
    f.fileable_id = Set(Some(slice.id));
    f.update(&db.conn).await.unwrap();
    let cover = file(&db, Some(vault.id)).await;
    let mut s: slice_of_lives::ActiveModel = slice.clone().into();
    s.file_cover_image_id = Set(Some(cover.id));
    s.update(&db.conn).await.unwrap();
    let unrelated = file(&db, Some(vault.id)).await;

    destroy_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap();

    assert!(file_row(&db, cover.id).await.is_none());
    assert!(file_row(&db, earlier_cover.id).await.is_none());
    assert!(file_row(&db, unrelated.id).await.is_some());
    db.drop().await;
}

// ---------------------------------------------------------------- IncrementPostReadCounterTest

async fn increment_post_read_counter(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
) -> ApiResult<()> {
    let post = IncrementPostReadCounter {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
    }
    .execute(&db.conn)
    .await?;

    let row = post_row(db, post.id).await.unwrap();
    assert_eq!(row.journal_id, journal.id);
    assert_eq!(row.view_count, 1);
    Ok(())
}

#[tokio::test]
async fn it_increments_a_post_counter() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    increment_post_read_counter(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap();
    db.drop().await;
}

// increment_post_read_counter_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn increment_post_read_counter_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = increment_post_read_counter(&db, &regis, account.id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn increment_post_read_counter_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let err = increment_post_read_counter(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn increment_post_read_counter_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, None).await;
    let err = increment_post_read_counter(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn increment_post_read_counter_fails_if_user_doesnt_have_right_permission_in_vault() {
    // F15: the PHP refused a viewer; now only a user outside the vault is refused.
    let db = TestDb::migrated().await;
    let regis = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, regis.account_id).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = increment_post_read_counter(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn increment_post_read_counter_counts_a_read_by_a_vault_viewer() {
    // F15: the PHP required a vault editor.
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    increment_post_read_counter(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap();
    db.drop().await;
}

// ---------------------------------------------------------------- RemoveContactFromPostTest

async fn remove_contact_from_post(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    journal: &journals::Model,
    post: &posts::Model,
) -> ApiResult<()> {
    RemoveContactFromPost {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
        contact_id: contact.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(contact_post::Entity::find_by_id((post.id, contact.id)).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

/// `$post->contacts()->attach($contact->id)`.
async fn attach_contact(db: &TestDb, post: &posts::Model, contact: &contacts::Model) {
    contact_post::Entity::insert(contact_post::ActiveModel {
        post_id: Set(post.id),
        contact_id: Set(contact.id),
        created_at: Set(None),
        updated_at: Set(None),
    })
    .exec_without_returning(&db.conn)
    .await
    .unwrap();
}

#[tokio::test]
async fn it_removes_a_contact_from_a_post() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    attach_contact(&db, &post, &contact).await;
    remove_contact_from_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap();
    db.drop().await;
}

// remove_contact_from_post_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn remove_contact_from_post_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    attach_contact(&db, &post, &contact).await;
    let err = remove_contact_from_post(&db, &regis, account.id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_contact_from_post_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, None).await;
    attach_contact(&db, &post, &contact).await;
    let err =
        remove_contact_from_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_contact_from_post_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    attach_contact(&db, &post, &contact).await;
    let err =
        remove_contact_from_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_contact_from_post_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, None).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    attach_contact(&db, &post, &contact).await;
    let err =
        remove_contact_from_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_contact_from_post_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    attach_contact(&db, &post, &contact).await;
    let err =
        remove_contact_from_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn remove_contact_from_post_writes_a_feed_item_only_for_a_mentioned_contact() {
    // F16: the PHP wrote `removed_from_post` whether or not the contact was in the post.
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let contact = contact(&db, Some(&vault)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err =
        remove_contact_from_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    let feed = contact_feed_items::Entity::find().filter(contact_feed_items::Column::ContactId.eq(contact.id));
    assert_eq!(feed.clone().count(&db.conn).await.unwrap(), 0);

    attach_contact(&db, &post, &contact).await;
    remove_contact_from_post(&db, &regis, regis.account_id, &vault, &contact, &journal, &post).await.unwrap();
    let items = feed.all(&db.conn).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].action, "removed_from_post");
    assert_eq!(items[0].description, post.title);
    assert_eq!(items[0].feedable_id, Some(post.id));
    db.drop().await;
}

// ---------------------------------------------------------------- RemovePostFromSliceOfLifeTest

async fn remove_post_from_slice_of_life(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
) -> ApiResult<()> {
    RemovePostFromSliceOfLife {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
    }
    .execute(&db.conn)
    .await?;

    assert_eq!(post_row(db, post.id).await.unwrap().slice_of_life_id, None);
    Ok(())
}

#[tokio::test]
async fn it_removes_a_post_from_a_slice_of_life() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post_in_slice(&db, Some(journal.id), slice.id).await;
    remove_post_from_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap();
    db.drop().await;
}

// remove_post_from_slice_of_life_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn remove_post_from_slice_of_life_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post_in_slice(&db, Some(journal.id), slice.id).await;
    let err = remove_post_from_slice_of_life(&db, &regis, account.id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_post_from_slice_of_life_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post_in_slice(&db, Some(journal.id), slice.id).await;
    let err = remove_post_from_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_post_from_slice_of_life_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post_in_slice(&db, None, slice.id).await;
    let err = remove_post_from_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_post_from_slice_of_life_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let post = post_in_slice(&db, Some(journal.id), slice.id).await;
    let err = remove_post_from_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- RemoveSliceOfLifeCoverImageTest

async fn remove_slice_of_life_cover_image(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    slice: &slice_of_lives::Model,
) -> ApiResult<()> {
    let file = file(db, Some(vault.id)).await;
    let mut s: slice_of_lives::ActiveModel = slice.clone().into();
    s.file_cover_image_id = Set(Some(file.id));
    s.update(&db.conn).await.unwrap();

    RemoveSliceOfLifeCoverImage {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        slice_of_life_id: Some(slice.id),
    }
    .execute(&db.conn)
    .await?;

    assert_eq!(slice_row(db, slice.id).await.unwrap().file_cover_image_id, None);
    assert!(file_row(db, file.id).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_removes_a_cover_image() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    remove_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap();
    db.drop().await;
}

// remove_slice_of_life_cover_image_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn remove_slice_of_life_cover_image_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err = remove_slice_of_life_cover_image(&db, &regis, account.id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_slice_of_life_cover_image_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err =
        remove_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_slice_of_life_cover_image_fails_if_slice_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, None).await;
    let err =
        remove_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_slice_of_life_cover_image_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err =
        remove_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- RemoveTagTest

async fn remove_tag(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
    tag: &tags::Model,
) -> ApiResult<()> {
    RemoveTag {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
        tag_id: tag.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(post_tag::Entity::find_by_id((tag.id, post.id)).one(&db.conn).await.unwrap().is_none());
    Ok(())
}

/// `$post->tags()->syncWithoutDetaching($tag)`.
async fn attach_tag(db: &TestDb, post: &posts::Model, tag: &tags::Model) {
    post_tag::Entity::insert(post_tag::ActiveModel {
        tag_id: Set(tag.id),
        post_id: Set(post.id),
        created_at: Set(None),
        updated_at: Set(None),
    })
    .exec_without_returning(&db.conn)
    .await
    .unwrap();
}

#[tokio::test]
async fn it_removes_a_tag() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    attach_tag(&db, &post, &tag).await;
    remove_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap();
    db.drop().await;
}

// remove_tag_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn remove_tag_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    attach_tag(&db, &post, &tag).await;
    let err = remove_tag(&db, &regis, account.id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_tag_fails_if_tag_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let tag = tag(&db, None).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    attach_tag(&db, &post, &tag).await;
    let err = remove_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_tag_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    attach_tag(&db, &post, &tag).await;
    let err = remove_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_tag_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, None).await;
    attach_tag(&db, &post, &tag).await;
    let err = remove_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn remove_tag_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let tag = tag(&db, Some(vault.id)).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    attach_tag(&db, &post, &tag).await;
    let err = remove_tag(&db, &regis, regis.account_id, &vault, &journal, &post, &tag).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- SetSliceOfLifeCoverImageTest

async fn set_slice_of_life_cover_image(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    file: &files::Model,
    slice: &slice_of_lives::Model,
) -> ApiResult<()> {
    SetSliceOfLifeCoverImage {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        slice_of_life_id: Some(slice.id),
        file_id: file.id,
    }
    .execute(&db.conn)
    .await?;

    assert_eq!(slice_row(db, slice.id).await.unwrap().file_cover_image_id, Some(file.id));
    let row = file_row(db, file.id).await.unwrap();
    assert_eq!(row.fileable_id, Some(slice.id));
    assert_eq!(row.fileable_type.as_deref(), Some("App\\Models\\SliceOfLife"));
    Ok(())
}

#[tokio::test]
async fn it_adds_a_cover_image() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let file = file(&db, Some(vault.id)).await;
    set_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &file, &slice).await.unwrap();
    db.drop().await;
}

// set_slice_of_life_cover_image_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn set_slice_of_life_cover_image_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let file = file(&db, Some(vault.id)).await;
    let err =
        set_slice_of_life_cover_image(&db, &regis, account.id, &vault, &journal, &file, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn set_slice_of_life_cover_image_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let file = file(&db, Some(vault.id)).await;
    let err = set_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &file, &slice)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn set_slice_of_life_cover_image_fails_if_slice_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, None).await;
    let file = file(&db, Some(vault.id)).await;
    let err = set_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &file, &slice)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn set_slice_of_life_cover_image_fails_if_file_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let file = file(&db, None).await;
    let err = set_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &file, &slice)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn set_slice_of_life_cover_image_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let file = file(&db, Some(vault.id)).await;
    let err = set_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &file, &slice)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn set_slice_of_life_cover_image_deletes_the_previous_cover() {
    // Q7: the PHP kept the replaced cover's `files` row.
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let first = file(&db, Some(vault.id)).await;
    let second = file(&db, Some(vault.id)).await;
    set_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &first, &slice).await.unwrap();
    set_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &first, &slice).await.unwrap();
    assert!(file_row(&db, first.id).await.is_some());

    set_slice_of_life_cover_image(&db, &regis, regis.account_id, &vault, &journal, &second, &slice).await.unwrap();
    assert!(file_row(&db, first.id).await.is_none());
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateJournalMetricTest

async fn update_journal_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    journal_metric: &journal_metrics::Model,
) -> ApiResult<()> {
    let journal_metric = UpdateJournalMetric {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        journal_metric_id: journal_metric.id,
        label: "title".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = journal_metrics::Entity::find_by_id(journal_metric.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.journal_id, journal.id);
    assert_eq!(row.label, "title");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_journal_metric() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    update_journal_metric(&db, &regis, regis.account_id, &vault, &journal, &journal_metric).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_journal_metric_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err = UpdateJournalMetric {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        journal_metric_id: journal_metric.id,
        label: "x".repeat(256),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_journal_metric_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err = update_journal_metric(&db, &regis, account.id, &vault, &journal, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_journal_metric_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err =
        update_journal_metric(&db, &regis, regis.account_id, &vault, &journal, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_journal_metric_fails_if_journal_metric_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let journal_metric = journal_metric(&db, None).await;
    let err =
        update_journal_metric(&db, &regis, regis.account_id, &vault, &journal, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_journal_metric_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let err =
        update_journal_metric(&db, &regis, regis.account_id, &vault, &journal, &journal_metric).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateJournalTest

async fn update_journal(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
) -> ApiResult<()> {
    let journal = UpdateJournal {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        name: "super journal".into(),
        description: None,
    }
    .execute(&db.conn)
    .await?;

    let row = journals::Entity::find_by_id(journal.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.name, "super journal");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    update_journal(&db, &regis, regis.account_id, &vault, &journal).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_journal_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = UpdateJournal {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        name: String::new(),
        description: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_journal_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = update_journal(&db, &regis, account.id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_journal_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let err = update_journal(&db, &regis, regis.account_id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_journal_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let err = update_journal(&db, &regis, regis.account_id, &vault, &journal).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdatePostMetricTest

async fn update_post_metric(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
    post_metric: &post_metrics::Model,
) -> ApiResult<()> {
    let journal_metric_id = post_metric.journal_metric_id;
    let post_metric = UpdatePostMetric {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
        journal_metric_id,
        post_metric_id: post_metric.id,
        value: 1,
        label: Some("title".into()),
    }
    .execute(&db.conn)
    .await?;

    let row = post_metrics::Entity::find_by_id(post_metric.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.post_id, post.id);
    assert_eq!(row.journal_metric_id, journal_metric_id);
    assert_eq!(row.label.as_deref(), Some("title"));
    assert_eq!(row.value, 1);
    Ok(())
}

#[tokio::test]
async fn it_updates_a_post_metric() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, Some(post.id), Some(journal_metric.id)).await;
    update_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &post_metric).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_post_metric_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, Some(post.id), Some(journal_metric.id)).await;
    let err = UpdatePostMetric {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        post_id: post.id,
        journal_metric_id: journal_metric.id,
        post_metric_id: post_metric.id,
        value: 1,
        label: Some("x".repeat(256)),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_post_metric_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, Some(post.id), Some(journal_metric.id)).await;
    let err = update_post_metric(&db, &regis, account.id, &vault, &journal, &post, &post_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_post_metric_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, Some(post.id), Some(journal_metric.id)).await;
    let err =
        update_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &post_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_post_metric_fails_if_journal_metric_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, None).await;
    let post_metric = post_metric(&db, Some(post.id), Some(journal_metric.id)).await;
    let err =
        update_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &post_metric).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_post_metric_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let journal_metric = journal_metric(&db, Some(journal.id)).await;
    let post_metric = post_metric(&db, Some(post.id), Some(journal_metric.id)).await;
    let err =
        update_post_metric(&db, &regis, regis.account_id, &vault, &journal, &post, &post_metric).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

// ---------------------------------------------------------------- UpdatePostTest

async fn update_post(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    post: &posts::Model,
) -> ApiResult<()> {
    let section = post_section(db, post.id, "this is a content").await;

    let post = UpdatePost {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        post_id: post.id,
        title: Some("title".into()),
        sections: vec![UpdatePostSection { id: section.id, content: Some(Some("this is a content".into())) }],
        written_at: None,
    }
    .execute(&db.conn)
    .await?;

    let row = post_row(db, post.id).await.unwrap();
    assert_eq!(row.journal_id, journal.id);
    assert_eq!(row.title.as_deref(), Some("title"));
    let row = post_sections::Entity::find_by_id(section.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.post_id, post.id);
    assert_eq!(row.content.as_deref(), Some("this is a content"));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_post() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    update_post(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_post_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = UpdatePost {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        post_id: post.id,
        title: Some("title".into()),
        sections: vec![],
        written_at: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_post_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = update_post(&db, &regis, account.id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_post_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let post = post(&db, Some(journal.id)).await;
    let err = update_post(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_post_fails_if_post_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, None).await;
    let err = update_post(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_post_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let err = update_post(&db, &regis, regis.account_id, &vault, &journal, &post).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_post_fails_if_a_section_isnt_one_of_the_post() {
    // The PHP calls `update()` on null for a section of another post.
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let post = post(&db, Some(journal.id)).await;
    let other = post_section(&db, self::post(&db, None).await.id, "other content").await;
    let err = UpdatePost {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        post_id: post.id,
        title: Some("title".into()),
        sections: vec![
            UpdatePostSection { id: Id(0), content: None },
            UpdatePostSection { id: other.id, content: Some(Some("changed".into())) },
        ],
        written_at: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    let row = post_sections::Entity::find_by_id(other.id).one(&db.conn).await.unwrap().unwrap();
    assert_eq!(row.content.as_deref(), Some("other content"));
    db.drop().await;
}

// ---------------------------------------------------------------- UpdateSliceOfLifeTest

async fn update_slice_of_life(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    journal: &journals::Model,
    slice: &slice_of_lives::Model,
) -> ApiResult<()> {
    let slice = UpdateSliceOfLife {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        journal_id: journal.id,
        slice_of_life_id: slice.id,
        name: "name".into(),
        description: None,
    }
    .execute(&db.conn)
    .await?;

    let row = slice_row(db, slice.id).await.unwrap();
    assert_eq!(row.journal_id, journal.id);
    assert_eq!(row.name, "name");
    Ok(())
}

#[tokio::test]
async fn it_updates_a_slice_of_life() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    update_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_slice_of_life_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err = UpdateSliceOfLife {
        account_id: regis.account_id,
        vault_id: vault.id,
        author_id: regis.id,
        journal_id: journal.id,
        slice_of_life_id: slice.id,
        name: "x".repeat(256),
        description: None,
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_slice_of_life_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let account = factory::account(&db.conn).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err = update_slice_of_life(&db, &regis, account.id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_slice_of_life_fails_if_journal_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, None).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err = update_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_slice_of_life_fails_if_slice_of_life_doesnt_belong_to_journal() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::Edit).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, None).await;
    let err = update_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_slice_of_life_fails_if_user_doesnt_have_right_permission_in_vault() {
    let db = TestDb::migrated().await;
    let (regis, vault) = regis(&db, VaultPermission::View).await;
    let journal = journal(&db, Some(vault.id)).await;
    let slice = slice_of_life(&db, Some(journal.id)).await;
    let err = update_slice_of_life(&db, &regis, regis.account_id, &vault, &journal, &slice).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}
