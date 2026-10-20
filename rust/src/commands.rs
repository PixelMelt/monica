//! Console commands that do more than call one function (`app/Console/Commands`).

use std::time::Duration;

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::config::{Config, Environment};
use crate::domains::contact::manage_reminders::{
    RescheduleContactReminderForChannel, format_contact_name, reminder_triggered,
};
use crate::entities::{contact_reminder_scheduled, contact_reminders, contacts, user_notification_channels, users};
use crate::error::ApiError;
use crate::http::state::AppState;
use crate::i18n;

/// `WaitForDb` (K005): connects, retrying every second, 30 times.
pub async fn wait_for_db(url: &str) -> anyhow::Result<DatabaseConnection> {
    let mut attempts = 30;
    loop {
        match crate::db::connect(url).await {
            Ok(db) => {
                tracing::info!("Database ready.");
                return Ok(db);
            }
            Err(e) if attempts > 1 => {
                tracing::warn!("Waiting for database to be ready… ({e})");
                attempts -= 1;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => return Err(anyhow::Error::new(e).context("Unable to contact your database.")),
        }
    }
}

/// `TestReminders` (K004, `test:send-reminders`): sends every scheduled
/// reminder that hasn't been sent, now, then schedules its next occurrence.
/// Not in production. Returns how many were sent.
pub async fn test_reminders(state: &AppState, config: &Config) -> anyhow::Result<usize> {
    if config.env == Environment::Production {
        return Ok(0);
    }
    let db = &state.db;
    let scheduled = contact_reminder_scheduled::Entity::find()
        .filter(contact_reminder_scheduled::Column::TriggeredAt.is_null())
        .all(db)
        .await?;
    let mut sent = 0;
    for scheduled_reminder in scheduled {
        let channel = user_notification_channels::Entity::find_by_id(scheduled_reminder.user_notification_channel_id)
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;
        let contact_reminder = contact_reminders::Entity::find_by_id(scheduled_reminder.contact_reminder_id)
            .one(db)
            .await?
            .ok_or_else(ApiError::not_found)?;

        if channel.r#type == "email" && channel.active {
            let contact = contacts::Entity::find_by_id(contact_reminder.contact_id)
                .one(db)
                .await?
                .ok_or_else(ApiError::not_found)?;
            let user = users::Entity::find_by_id(channel.user_id.ok_or_else(ApiError::not_found)?)
                .one(db)
                .await?
                .ok_or_else(ApiError::not_found)?;
            let contact_name = format_contact_name(&user, &contact);
            i18n::LOCALE
                .scope(user.locale.clone(), async {
                    reminder_triggered(db, &state.mailer, &channel, &contact_reminder.label, &contact_name).await
                })
                .await?;
            sent += 1;
        }

        let rescheduled = RescheduleContactReminderForChannel {
            contact_reminder_id: scheduled_reminder.contact_reminder_id,
            user_notification_channel_id: scheduled_reminder.user_notification_channel_id,
            contact_reminder_scheduled_id: scheduled_reminder.id,
        }
        .execute(db)
        .await;
        match rescheduled {
            Ok(()) => {}
            Err(e) if e.code == crate::error::Code::NotFound => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(sent)
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, NaiveTime};
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};

    use super::*;
    use crate::domains::base::VaultPermission;
    use crate::domains::contact::manage_reminders::{CreateContactReminder, TYPE_RECURRING_YEAR};
    use crate::testing::{TestDb, factory};

    #[tokio::test]
    async fn test_reminders_sends_every_scheduled_reminder_now() {
        let db = TestDb::migrated().await;
        let config = Config::for_tests("", std::env::temp_dir());
        let state = AppState::new(db.conn.clone(), config.clone()).unwrap();
        let user = factory::create_user(&db.conn).await;
        let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
        let contact = factory::contact(&db.conn, &vault).await;
        user_notification_channels::ActiveModel {
            user_id: Set(Some(user.id)),
            r#type: Set("email".into()),
            label: Set(Some("me".into())),
            content: Set("me@example.com".into()),
            active: Set(true),
            fails: Set(0),
            preferred_time: Set(NaiveTime::from_hms_opt(9, 0, 0)),
            ..Default::default()
        }
        .insert(&db.conn)
        .await
        .unwrap();
        let next_year = chrono::Utc::now().year() + 1;
        CreateContactReminder {
            account_id: user.account_id,
            vault_id: vault.id,
            author_id: user.id,
            contact_id: contact.id,
            label: "Birthday".into(),
            day: Some(1),
            month: Some(1),
            year: Some(next_year),
            r#type: TYPE_RECURRING_YEAR.into(),
            frequency_number: Some(1),
        }
        .execute(&db.conn)
        .await
        .unwrap();

        assert_eq!(test_reminders(&state, &config).await.unwrap(), 1);
        let sent = state.mailer.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].to, "me@example.com");
        assert!(sent[0].text.contains("Birthday"));
        let before = contact_reminder_scheduled::Entity::find().one(&db.conn).await.unwrap().unwrap();
        let rows = contact_reminder_scheduled::Entity::find().all(&db.conn).await.unwrap();
        assert_eq!(rows.len(), 1, "the schedule row is moved, as syncWithoutDetaching does");
        assert!(rows[0].triggered_at.is_none());
        assert_eq!(before.scheduled_at.0.month(), 1);

        // Sending again moves the schedule another year ahead.
        let year = rows[0].scheduled_at.0.year();
        test_reminders(&state, &config).await.unwrap();
        let after = contact_reminder_scheduled::Entity::find().one(&db.conn).await.unwrap().unwrap();
        assert_eq!(after.scheduled_at.0.year(), year + 1);
        assert_eq!(state.mailer.sent().len(), 2);

        let mut production = config.clone();
        production.env = Environment::Production;
        assert_eq!(test_reminders(&state, &production).await.unwrap(), 0);
        db.drop().await;
    }
}
