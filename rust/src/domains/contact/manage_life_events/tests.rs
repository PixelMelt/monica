//! `tests/Unit/Domains/Contact/ManageLifeEvents/Services`.

use crate::crypto::random_string;
use crate::entities::{users, vaults};
use crate::error::Code;
use crate::testing::{TestDb, factory};

use super::*;

/// `TimelineEvent::factory()` in `vault_id`.
async fn timeline_event(db: &TestDb, vault_id: Uuid) -> timeline_events::Model {
    timeline_events::ActiveModel {
        vault_id: Set(vault_id),
        started_at: Set(crate::db::now().0.date()),
        label: Set(Some(format!("Sentence {}", random_string(10)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("timeline event")
}

/// `LifeEventCategory::factory()` in `vault_id`.
async fn life_event_category(db: &TestDb, vault_id: Uuid) -> life_event_categories::Model {
    life_event_categories::ActiveModel {
        vault_id: Set(vault_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        label_translation_key: Set(Some(format!("Name {}", random_string(6)))),
        can_be_deleted: Set(true),
        position: Set(Some(1)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("life event category")
}

/// `LifeEventType::factory()` in `life_event_category_id`.
async fn life_event_type(db: &TestDb, life_event_category_id: Id) -> life_event_types::Model {
    life_event_types::ActiveModel {
        life_event_category_id: Set(life_event_category_id),
        label: Set(Some(format!("Name {}", random_string(6)))),
        label_translation_key: Set(Some(format!("Name {}", random_string(6)))),
        can_be_deleted: Set(true),
        position: Set(Some(1)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("life event type")
}

/// `LifeEvent::factory()` in `timeline_event_id`, of `life_event_type_id`
/// (a new type when `None`), paid by a new contact.
async fn life_event(db: &TestDb, timeline_event_id: Id, life_event_type_id: Option<Id>) -> life_events::Model {
    let life_event_type_id = match life_event_type_id {
        Some(id) => id,
        None => life_event_type(db, life_event_category(db, other_vault(db).await.id).await.id).await.id,
    };
    let paid_by = other_contact(db).await;
    life_events::ActiveModel {
        timeline_event_id: Set(timeline_event_id),
        life_event_type_id: Set(life_event_type_id),
        summary: Set(Some(format!("Sentence {}", random_string(10)))),
        description: Set(Some(format!("Sentence {}", random_string(10)))),
        happened_at: Set(crate::db::now().0.date()),
        costs: Set(Some(rand::random_range(0..10))),
        currency_id: Set(None),
        paid_by_contact_id: Set(Some(paid_by.id)),
        duration_in_minutes: Set(Some(rand::random_range(0..100_000))),
        distance: Set(Some(rand::random_range(0..100_000))),
        from_place: Set(Some(format!("City {}", random_string(6)))),
        to_place: Set(Some(format!("City {}", random_string(6)))),
        place: Set(Some(format!("City {}", random_string(6)))),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("life event")
}

/// `Vault::factory()->create()`: a vault of a new account.
async fn other_vault(db: &TestDb) -> vaults::Model {
    let account = factory::account(&db.conn).await;
    factory::vault(&db.conn, account.id).await
}

/// `Contact::factory()->create()`: a contact in a new vault of a new account.
async fn other_contact(db: &TestDb) -> contacts::Model {
    let vault = other_vault(db).await;
    factory::contact(&db.conn, &vault).await
}

/// `$user` with `permission` in a new vault of their account.
async fn user_in_vault(db: &TestDb, permission: VaultPermission) -> (users::Model, vaults::Model) {
    let user = factory::create_user(&db.conn).await;
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    factory::set_permission_in_vault(&db.conn, &user, permission, &vault).await;
    (user, vault)
}

/// `$user` with edit permission in a new vault of another account.
async fn user_in_other_account_vault(db: &TestDb) -> (users::Model, vaults::Model) {
    let user = factory::create_user(&db.conn).await;
    let vault = other_vault(db).await;
    factory::set_permission_in_vault(&db.conn, &user, VaultPermission::Edit, &vault).await;
    (user, vault)
}

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

async fn life_event_row(db: &TestDb, id: Id) -> Option<life_events::Model> {
    life_events::Entity::find_by_id(id).one(&db.conn).await.unwrap()
}

async fn timeline_event_row(db: &TestDb, id: Id) -> Option<timeline_events::Model> {
    timeline_events::Entity::find_by_id(id).one(&db.conn).await.unwrap()
}

/// The life event has the values the PHP tests send, and `contact_id` takes part in it.
async fn assert_life_event(db: &TestDb, id: Id, timeline_event_id: Id, life_event_type_id: Id, contact_id: Uuid) {
    let row = life_event_row(db, id).await.unwrap();
    assert_eq!(row.timeline_event_id, timeline_event_id);
    assert_eq!(row.life_event_type_id, life_event_type_id);
    assert_eq!(row.summary, None);
    assert_eq!(row.happened_at, ymd(1982, 2, 4));
    let participant = life_event_participants::Entity::find_by_id((contact_id, id)).one(&db.conn).await.unwrap();
    assert!(participant.is_some());
}

// ---- CreateLifeEventTest

fn create_request(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    life_event_type: &life_event_types::Model,
    timeline_event: &timeline_events::Model,
) -> CreateLifeEvent {
    CreateLifeEvent {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        timeline_event_id: timeline_event.id,
        life_event_type_id: life_event_type.id,
        summary: None,
        description: None,
        happened_at: "1982-02-04".into(),
        costs: None,
        currency_id: None,
        paid_by_contact_id: None,
        duration_in_minutes: None,
        distance: None,
        distance_unit: None,
        from_place: None,
        to_place: None,
        place: None,
        participant_ids: vec![contact.id],
    }
}

async fn create_life_event(db: &TestDb, request: CreateLifeEvent) -> ApiResult<()> {
    let (timeline_event_id, life_event_type_id, contact_id) =
        (request.timeline_event_id, request.life_event_type_id, request.participant_ids[0]);
    let life_event = request.execute(&db.conn).await?;

    assert_life_event(db, life_event.id, timeline_event_id, life_event_type_id, contact_id).await;
    Ok(())
}

#[tokio::test]
async fn it_creates_a_life_event() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let request = create_request(&user, user.account_id, &vault, &contact, &r#type, &timeline_event);
    create_life_event(&db, request).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let request = create_request(&user, user.account_id, &vault, &contact, &r#type, &timeline_event);
    let err = CreateLifeEvent { happened_at: String::new(), participant_ids: vec![], ..request }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let request = create_request(&user, account.id, &vault, &contact, &r#type, &timeline_event);
    assert_eq!(create_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_other_account_vault(&db).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let request = create_request(&user, user.account_id, &vault, &contact, &r#type, &timeline_event);
    assert_eq!(create_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_isnt_vault_editor() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let request = create_request(&user, user.account_id, &vault, &contact, &r#type, &timeline_event);
    assert_eq!(create_life_event(&db, request).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_timeline_event_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let timeline_event = timeline_event(&db, other_vault(&db).await.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let request = create_request(&user, user.account_id, &vault, &contact, &r#type, &timeline_event);
    assert_eq!(create_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let request = create_request(&user, user.account_id, &vault, &contact, &r#type, &timeline_event);
    assert_eq!(create_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_life_event_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, other_vault(&db).await.id).await.id).await;
    let request = create_request(&user, user.account_id, &vault, &contact, &r#type, &timeline_event);
    assert_eq!(create_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- CreateTimelineEventTest

fn create_timeline_request(author: &users::Model, account_id: Uuid, vault: &vaults::Model) -> CreateTimelineEvent {
    CreateTimelineEvent {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        label: None,
        started_at: "1982-02-04".into(),
    }
}

async fn create_timeline_event(db: &TestDb, request: CreateTimelineEvent) -> ApiResult<()> {
    let vault_id = request.vault_id;
    let timeline_event = request.execute(&db.conn).await?;

    let row = timeline_event_row(db, timeline_event.id).await.unwrap();
    assert_eq!(row.vault_id, vault_id);
    assert_eq!(row.label, None);
    assert_eq!(row.started_at, ymd(1982, 2, 4));
    Ok(())
}

#[tokio::test]
async fn it_creates_a_timeline_event() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    create_timeline_event(&db, create_timeline_request(&user, user.account_id, &vault)).await.unwrap();
    db.drop().await;
}

/// F9: CreateTimelineEvent stores `label`.
#[tokio::test]
async fn create_timeline_event_stores_the_label() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let request = create_timeline_request(&user, user.account_id, &vault);
    let timeline_event =
        CreateTimelineEvent { label: Some("Trip to Japan".into()), ..request }.execute(&db.conn).await.unwrap();
    let row = timeline_event_row(&db, timeline_event.id).await.unwrap();
    assert_eq!(row.label.as_deref(), Some("Trip to Japan"));
    db.drop().await;
}

#[tokio::test]
async fn create_timeline_event_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let request = create_timeline_request(&user, user.account_id, &vault);
    let err = CreateTimelineEvent { started_at: String::new(), ..request }.execute(&db.conn).await.unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn create_timeline_event_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let request = create_timeline_request(&user, account.id, &vault);
    assert_eq!(create_timeline_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_timeline_event_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_other_account_vault(&db).await;
    let request = create_timeline_request(&user, user.account_id, &vault);
    assert_eq!(create_timeline_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn create_timeline_event_fails_if_user_isnt_vault_editor() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::View).await;
    let request = create_timeline_request(&user, user.account_id, &vault);
    assert_eq!(create_timeline_event(&db, request).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

// ---- DestroyLifeEventTest

async fn destroy_life_event(
    db: &TestDb,
    user: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    life_event: &life_events::Model,
    timeline_event: &timeline_events::Model,
) -> ApiResult<()> {
    DestroyLifeEvent {
        account_id,
        vault_id: vault.id,
        author_id: user.id,
        timeline_event_id: timeline_event.id,
        life_event_id: life_event.id,
    }
    .execute(&db.conn)
    .await?;

    assert!(life_event_row(db, life_event.id).await.is_none());
    assert!(timeline_event_row(db, timeline_event.id).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_contact_life_event() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    destroy_life_event(&db, &user, user.account_id, &vault, &life_event, &timeline_event).await.unwrap();
    db.drop().await;
}

// destroy_life_event_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_life_event_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    let err = destroy_life_event(&db, &user, account.id, &vault, &life_event, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_life_event_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_other_account_vault(&db).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    let err = destroy_life_event(&db, &user, user.account_id, &vault, &life_event, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_life_event_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::View).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    let err = destroy_life_event(&db, &user, user.account_id, &vault, &life_event, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_life_event_fails_if_timeline_event_does_not_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let timeline_event = timeline_event(&db, other_vault(&db).await.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    let err = destroy_life_event(&db, &user, user.account_id, &vault, &life_event, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_life_event_fails_if_life_events_doesn_belong_to_timeline_event() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let other_timeline_event = timeline_event(&db, other_vault(&db).await.id).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let life_event = life_event(&db, other_timeline_event.id, None).await;
    let err = destroy_life_event(&db, &user, user.account_id, &vault, &life_event, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---- DestroyTimelineEventTest

async fn destroy_timeline_event(
    db: &TestDb,
    user: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    timeline_event: &timeline_events::Model,
) -> ApiResult<()> {
    DestroyTimelineEvent { account_id, vault_id: vault.id, author_id: user.id, timeline_event_id: timeline_event.id }
        .execute(&db.conn)
        .await?;

    assert!(timeline_event_row(db, timeline_event.id).await.is_none());
    Ok(())
}

#[tokio::test]
async fn it_destroys_a_timeline_event() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    destroy_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap();
    db.drop().await;
}

// destroy_timeline_event_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn destroy_timeline_event_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = destroy_timeline_event(&db, &user, account.id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_timeline_event_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_other_account_vault(&db).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = destroy_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn destroy_timeline_event_fails_if_user_doesnt_have_right_permission_in_initial_vault() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::View).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = destroy_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn destroy_timeline_event_fails_if_timeline_event_does_not_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let timeline_event = timeline_event(&db, other_vault(&db).await.id).await;
    let err = destroy_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---- ToggleLifeEventTest

async fn toggle_life_event(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    life_event: &life_events::Model,
    timeline_event: &timeline_events::Model,
) -> ApiResult<()> {
    let life_event = ToggleLifeEvent {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        timeline_event_id: timeline_event.id,
        life_event_id: life_event.id,
    }
    .execute(&db.conn)
    .await?;

    let row = life_event_row(db, life_event.id).await.unwrap();
    assert_eq!(row.timeline_event_id, timeline_event.id);
    assert!(row.collapsed);
    Ok(())
}

#[tokio::test]
async fn toggle_life_event_updates_a_life_event() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    toggle_life_event(&db, &user, user.account_id, &vault, &life_event, &timeline_event).await.unwrap();
    db.drop().await;
}

// toggle_life_event_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn toggle_life_event_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    let err = toggle_life_event(&db, &user, account.id, &vault, &life_event, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_life_event_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_other_account_vault(&db).await;
    factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    let err = toggle_life_event(&db, &user, user.account_id, &vault, &life_event, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_life_event_fails_if_user_isnt_vault_editor() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::View).await;
    factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    let err = toggle_life_event(&db, &user, user.account_id, &vault, &life_event, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn toggle_life_event_fails_if_timeline_event_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, other_vault(&db).await.id).await;
    let life_event = life_event(&db, timeline_event.id, None).await;
    let err = toggle_life_event(&db, &user, user.account_id, &vault, &life_event, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---- ToggleTimelineEventTest

async fn toggle_timeline_event(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    timeline_event: &timeline_events::Model,
) -> ApiResult<()> {
    let timeline_event = ToggleTimelineEvent {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        timeline_event_id: timeline_event.id,
    }
    .execute(&db.conn)
    .await?;

    let row = timeline_event_row(db, timeline_event.id).await.unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert!(!row.collapsed);
    Ok(())
}

#[tokio::test]
async fn toggle_timeline_event_updates_a_timeline_event() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    toggle_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap();
    db.drop().await;
}

// toggle_timeline_event_fails_if_wrong_parameters_are_given: every field is typed (Uuid, Id); no invalid input can be built.

#[tokio::test]
async fn toggle_timeline_event_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = toggle_timeline_event(&db, &user, account.id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_timeline_event_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_other_account_vault(&db).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = toggle_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn toggle_timeline_event_fails_if_user_isnt_vault_editor() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::View).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = toggle_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn toggle_timeline_event_fails_if_timeline_event_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let timeline_event = timeline_event(&db, other_vault(&db).await.id).await;
    let err = toggle_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

// ---- UpdateLifeEventTest

fn update_request(
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    contact: &contacts::Model,
    life_event_type: &life_event_types::Model,
    life_event: &life_events::Model,
    timeline_event: &timeline_events::Model,
) -> UpdateLifeEvent {
    UpdateLifeEvent {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        timeline_event_id: timeline_event.id,
        life_event_type_id: life_event_type.id,
        life_event_id: life_event.id,
        summary: None,
        description: None,
        happened_at: "1982-02-04".into(),
        costs: None,
        currency_id: None,
        paid_by_contact_id: None,
        duration_in_minutes: None,
        distance: None,
        distance_unit: None,
        from_place: None,
        to_place: None,
        place: None,
        participant_ids: vec![contact.id],
    }
}

async fn update_life_event(db: &TestDb, request: UpdateLifeEvent) -> ApiResult<()> {
    let (timeline_event_id, life_event_type_id, contact_id) =
        (request.timeline_event_id, request.life_event_type_id, request.participant_ids[0]);
    let life_event = request.execute(&db.conn).await?;

    assert_life_event(db, life_event.id, timeline_event_id, life_event_type_id, contact_id).await;
    Ok(())
}

#[tokio::test]
async fn it_updates_a_life_event() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let life_event = life_event(&db, timeline_event.id, Some(r#type.id)).await;
    let request = update_request(&user, user.account_id, &vault, &contact, &r#type, &life_event, &timeline_event);
    update_life_event(&db, request).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_life_event_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let life_event = life_event(&db, timeline_event.id, Some(r#type.id)).await;
    let request = update_request(&user, user.account_id, &vault, &contact, &r#type, &life_event, &timeline_event);
    let err = UpdateLifeEvent { summary: Some("x".repeat(256)), happened_at: "04/02/1982".into(), ..request }
        .execute(&db.conn)
        .await
        .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_life_event_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let life_event = life_event(&db, timeline_event.id, Some(r#type.id)).await;
    let request = update_request(&user, account.id, &vault, &contact, &r#type, &life_event, &timeline_event);
    assert_eq!(update_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_life_event_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_other_account_vault(&db).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let life_event = life_event(&db, timeline_event.id, Some(r#type.id)).await;
    let request = update_request(&user, user.account_id, &vault, &contact, &r#type, &life_event, &timeline_event);
    assert_eq!(update_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_life_event_fails_if_user_isnt_vault_editor() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::View).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let life_event = life_event(&db, timeline_event.id, Some(r#type.id)).await;
    let request = update_request(&user, user.account_id, &vault, &contact, &r#type, &life_event, &timeline_event);
    assert_eq!(update_life_event(&db, request).await.unwrap_err().code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_life_event_fails_if_contact_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = other_contact(&db).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let life_event = life_event(&db, timeline_event.id, Some(r#type.id)).await;
    let request = update_request(&user, user.account_id, &vault, &contact, &r#type, &life_event, &timeline_event);
    assert_eq!(update_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_life_event_fails_if_life_event_type_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, other_vault(&db).await.id).await.id).await;
    let life_event = life_event(&db, timeline_event.id, Some(r#type.id)).await;
    let request = update_request(&user, user.account_id, &vault, &contact, &r#type, &life_event, &timeline_event);
    assert_eq!(update_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_life_event_fails_if_timeline_event_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let timeline_event = timeline_event(&db, other_vault(&db).await.id).await;
    let r#type = life_event_type(&db, life_event_category(&db, vault.id).await.id).await;
    let life_event = life_event(&db, timeline_event.id, Some(r#type.id)).await;
    let request = update_request(&user, user.account_id, &vault, &contact, &r#type, &life_event, &timeline_event);
    assert_eq!(update_life_event(&db, request).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// ---- UpdateTimelineEventTest

async fn update_timeline_event(
    db: &TestDb,
    author: &users::Model,
    account_id: Uuid,
    vault: &vaults::Model,
    timeline_event: &timeline_events::Model,
) -> ApiResult<()> {
    let timeline_event = UpdateTimelineEvent {
        account_id,
        vault_id: vault.id,
        author_id: author.id,
        timeline_event_id: timeline_event.id,
        label: None,
        started_at: "1982-02-04".into(),
    }
    .execute(&db.conn)
    .await?;

    let row = timeline_event_row(db, timeline_event.id).await.unwrap();
    assert_eq!(row.vault_id, vault.id);
    assert_eq!(row.label, None);
    assert_eq!(row.started_at, ymd(1982, 2, 4));
    Ok(())
}

#[tokio::test]
async fn it_updates_a_timeline_event() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    update_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap();
    db.drop().await;
}

#[tokio::test]
async fn update_timeline_event_fails_if_wrong_parameters_are_given() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = UpdateTimelineEvent {
        account_id: user.account_id,
        vault_id: vault.id,
        author_id: user.id,
        timeline_event_id: timeline_event.id,
        label: None,
        started_at: "1982-02-30".into(),
    }
    .execute(&db.conn)
    .await
    .unwrap_err();
    assert_eq!(err.code, Code::ValidationFailed);
    db.drop().await;
}

#[tokio::test]
async fn update_timeline_event_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let account = factory::create_account(&db.conn).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = update_timeline_event(&db, &user, account.id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_timeline_event_fails_if_vault_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_other_account_vault(&db).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = update_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn update_timeline_event_fails_if_user_isnt_vault_editor() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::View).await;
    let timeline_event = timeline_event(&db, vault.id).await;
    let err = update_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::Forbidden);
    db.drop().await;
}

#[tokio::test]
async fn update_timeline_event_fails_if_timeline_event_doesnt_belong_to_vault() {
    let db = TestDb::migrated().await;
    let (user, vault) = user_in_vault(&db, VaultPermission::Edit).await;
    let timeline_event = timeline_event(&db, other_vault(&db).await.id).await;
    let err = update_timeline_event(&db, &user, user.account_id, &vault, &timeline_event).await.unwrap_err();
    assert_eq!(err.code, Code::NotFound);
    db.drop().await;
}
