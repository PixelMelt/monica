//! `tests/Unit/Domains/Contact/DAV/Jobs`.

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use sha2::{Digest, Sha256};

use super::*;
use crate::db::id::Id;
use crate::domains::base::VaultPermission;
use crate::entities::{contact_important_dates, contact_tasks, contacts, sync_tokens};
use crate::testing::{TestDb, factory};

/// `SyncToken::create([...])`.
async fn sync_token(db: &TestDb, account_id: Uuid, user_id: Uuid, timestamp: Timestamp) -> Id {
    sync_tokens::ActiveModel {
        account_id: Set(account_id),
        user_id: Set(user_id),
        name: Set("contacts".into()),
        timestamp: Set(timestamp),
        ..Default::default()
    }
    .insert(&db.conn)
    .await
    .expect("sync token")
    .id
}

fn etag(data: &str) -> String {
    format!("\"{}\"", hex::encode(Sha256::digest(data.as_bytes())))
}

// ---- CleanSyncTokenTest

#[tokio::test]
async fn tokenclean_left_one_token() {
    let db = TestDb::migrated().await;
    let account = factory::account(&db.conn).await;
    let user = factory::user(&db.conn, Some(account.id)).await;

    sync_token(&db, account.id, user.id, crate::db::now()).await;
    CleanSyncToken.execute(&db.conn).await.unwrap();

    let rows = sync_tokens::Entity::find()
        .filter(sync_tokens::Column::AccountId.eq(account.id))
        .filter(sync_tokens::Column::UserId.eq(user.id))
        .filter(sync_tokens::Column::Name.eq("contacts"))
        .all(&db.conn)
        .await
        .unwrap();
    assert!(!rows.is_empty());
    db.drop().await;
}

#[tokio::test]
async fn tokenclean_left_all_token() {
    let db = TestDb::migrated().await;
    let account = factory::account(&db.conn).await;
    let user = factory::user(&db.conn, Some(account.id)).await;

    let now = crate::db::now().0;
    let s1 = sync_token(&db, account.id, user.id, Timestamp(now)).await;
    let s2 = sync_token(&db, account.id, user.id, Timestamp(now + Duration::days(-31))).await;
    let s3 = sync_token(&db, account.id, user.id, Timestamp(now + Duration::days(-45))).await;
    let s4 = sync_token(&db, account.id, user.id, Timestamp(now + Duration::days(-1))).await;
    CleanSyncToken.execute(&db.conn).await.unwrap();

    let exists = |id: Id| {
        let conn = &db.conn;
        async move { sync_tokens::Entity::find_by_id(id).one(conn).await.unwrap().is_some() }
    };
    assert!(exists(s1).await);
    assert!(!exists(s2).await);
    assert!(!exists(s3).await);
    assert!(exists(s4).await);
    db.drop().await;
}

// ---- UpdateVCalendarTest

#[tokio::test]
async fn update_vcalendar_creates_a_task() {
    let db = TestDb::migrated().await;
    let user = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;

    let calendar = "BEGIN:VCALENDAR
PRODID:-//Sabre//Sabre VObject 4.5.3//EN
VERSION:2.0
BEGIN:VTODO
CREATED:20250729T000000Z
LAST-MODIFIED:20250814T000000Z
DTSTAMP:20250814T000000Z
UID:3a7baf23-50b6-43dd-b441-7d70362f6356
SUMMARY:My Task
END:VTODO
END:VCALENDAR
";
    let etag = etag(calendar);

    UpdateVCalendar {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        uri: "https://test/dav/3a7baf23-50b6-43dd-b441-7d70362f6356".into(),
        etag: Some(etag.clone()),
        calendar: calendar.into(),
        external: Some(true),
    }
    .execute(&db.conn)
    .await
    .unwrap();

    let row = contact_tasks::Entity::find()
        .filter(contact_tasks::Column::Label.eq("My Task"))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.vcalendar.as_deref(), Some(calendar));
    assert_eq!(row.distant_etag, Some(etag));
    assert_eq!(row.distant_uuid.as_deref(), Some("3a7baf23-50b6-43dd-b441-7d70362f6356"));
    db.drop().await;
}

#[tokio::test]
async fn update_vcalendar_creates_a_date() {
    let db = TestDb::migrated().await;
    let user = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;

    let calendar = "BEGIN:VCALENDAR
PRODID:-//Sabre//Sabre VObject 4.5.3//EN
VERSION:2.0
BEGIN:VEVENT
CREATED:20250814T000000Z
LAST-MODIFIED:20250814T000000Z
DTSTAMP:20250814T000000Z
UID:36ee6e82-5262-404f-aea1-98859c631892
SUMMARY:Réunion importante
DTSTART;VALUE=DATE:20250813
DTEND;VALUE=DATE:20250814
TRANSP:TRANSPARENT
END:VEVENT
END:VCALENDAR
";
    let etag = etag(calendar);

    UpdateVCalendar {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        uri: "https://test/dav/36ee6e82-5262-404f-aea1-98859c631892".into(),
        etag: Some(etag.clone()),
        calendar: calendar.into(),
        external: Some(true),
    }
    .execute(&db.conn)
    .await
    .unwrap();

    let row = contact_important_dates::Entity::find()
        .filter(contact_important_dates::Column::Label.eq("Réunion importante"))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.vcalendar.as_deref(), Some(calendar));
    assert_eq!(row.distant_etag, Some(etag));
    assert_eq!(row.distant_uuid.as_deref(), Some("36ee6e82-5262-404f-aea1-98859c631892"));
    db.drop().await;
}

// ---- UpdateVCardTest

#[tokio::test]
async fn update_vcard_creates_a_contact() {
    let db = TestDb::migrated().await;
    let user = factory::user(&db.conn, None).await;
    let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;

    let card = "BEGIN:VCARD
VERSION:4.0
PRODID:-//Sabre//Sabre VObject 4.5.3//EN
UID:affacde9-b2fe-4371-9acb-6612aaee6971
SOURCE:
FN:Test
N:;Test;;;
REV:20230715T112647Z
END:VCARD";
    let etag = etag(card);

    UpdateVCard {
        account_id: user.account_id,
        author_id: user.id,
        vault_id: vault.id,
        uri: "https://test/dav/affacde9-b2fe-4371-9acb-6612aaee6971".into(),
        etag: Some(etag.clone()),
        card: card.into(),
        external: Some(true),
    }
    .execute(&db.conn)
    .await
    .unwrap();

    let row = contacts::Entity::find()
        .filter(contacts::Column::FirstName.eq("Test"))
        .filter(contacts::Column::VaultId.eq(vault.id))
        .one(&db.conn)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.vcard.as_deref(), Some(card));
    assert_eq!(row.distant_etag, Some(etag));
    assert_eq!(row.distant_uuid.as_deref(), Some("affacde9-b2fe-4371-9acb-6612aaee6971"));
    db.drop().await;
}
