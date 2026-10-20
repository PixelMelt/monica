//! `tests/Unit/Domains/Settings/CancelAccount/Services`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set};

use super::*;
use crate::crypto::random_string;
use crate::entities::users;
use crate::error::Code;
use crate::testing::{TestDb, factory};

/// `File::factory()` in `vault_id`.
async fn file(db: &TestDb, vault_id: Uuid) -> files::Model {
    files::ActiveModel {
        vault_id: Set(vault_id),
        uuid: Set(uuid::Uuid::new_v4().to_string()),
        original_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        cdn_url: Set(Some(format!("https://example.com/{}", random_string(8)))),
        name: Set(format!("Name {}", random_string(6))),
        mime_type: Set("avatar".into()),
        r#type: Set("avatar".into()),
        size: Set(rand::random_range(200 * 1024 * 1024..=500 * 1024 * 1024)),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("file")
}

async fn cancel_account(
    db: &TestDb,
    account_id: Uuid,
    user: &users::Model,
    file: Option<&files::Model>,
) -> ApiResult<()> {
    CancelAccount { account_id, author_id: user.id }.execute(&db.conn).await?;

    assert!(accounts::Entity::find_by_id(account_id).one(&db.conn).await.unwrap().is_none());
    if let Some(file) = file {
        assert!(files::Entity::find_by_id(file.id).one(&db.conn).await.unwrap().is_none());
    }
    Ok(())
}

#[tokio::test]
async fn it_destroys_an_account() {
    let db = TestDb::migrated().await;
    let user = factory::create_administrator(&db.conn).await;
    let vault = factory::create_vault(&db.conn, user.account_id).await;
    let contact = factory::contact(&db.conn, &vault).await;
    let file = file(&db, contact.vault_id).await;
    cancel_account(&db, user.account_id, &user, Some(&file)).await.unwrap();
    db.drop().await;
}

// it_queues_destroying_an_account: queued services run synchronously here, nothing is queued.

#[tokio::test]
async fn it_fails_if_user_doesnt_belong_to_account() {
    let db = TestDb::migrated().await;
    let user = factory::create_administrator(&db.conn).await;
    let account = factory::account(&db.conn).await;
    assert_eq!(cancel_account(&db, account.id, &user, None).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

#[tokio::test]
async fn it_fails_if_user_is_not_account_administrator() {
    let db = TestDb::migrated().await;
    let user = factory::create_user(&db.conn).await;
    let account = factory::account(&db.conn).await;
    assert_eq!(cancel_account(&db, account.id, &user, None).await.unwrap_err().code, Code::NotFound);
    db.drop().await;
}

// it_fails_if_wrong_parameters_are_given: both fields are `Uuid`s, no invalid input can be built.
