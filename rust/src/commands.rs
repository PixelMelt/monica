//! Console commands that do more than call one function (`app/Console/Commands`).

use std::time::Duration;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::config::{Config, Environment};
use crate::domains::contact::manage_reminders::{
    RescheduleContactReminderForChannel, format_contact_name, reminder_triggered,
};
use crate::domains::settings::manage_notification_channels::TYPE_PUSHOVER;
use crate::domains::settings::manage_users::normalize_username;
use crate::entities::{contact_reminder_scheduled, contact_reminders, contacts, user_notification_channels, users};
use crate::error::ApiError;
use crate::http::state::AppState;
use crate::i18n;

/// `WaitForDb` (K005): connects, retrying every second, 30 times.
pub async fn wait_for_db(url: &str) -> anyhow::Result<DatabaseConnection> {
    // A missing SQLite file is created (`touch($e->path)`), with its directory.
    if let Some(dir) = url.strip_prefix("sqlite://").and_then(|p| std::path::Path::new(p).parent()) {
        std::fs::create_dir_all(dir)?;
    }
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

        if channel.r#type == TYPE_PUSHOVER && channel.active {
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
                    reminder_triggered(db, &state.pushover, &channel, &contact_reminder.label, &contact_name).await
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

/// `reset-password`: a new random password for `username`, returned to be
/// printed, and the end of their sessions and "remember me" logins (there
/// is no email to send a reset link to). `disable_two_factor` also turns
/// two-factor authentication off, for a lost authenticator.
pub async fn reset_password(state: &AppState, username: &str, disable_two_factor: bool) -> anyhow::Result<String> {
    let username = normalize_username(username);
    let user = users::Entity::find()
        .filter(users::Column::Username.eq(&username))
        .one(&state.db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("There is no user {username:?}."))?;
    let password = crate::crypto::random_string(20);
    let user_id = user.id;
    let mut u: users::ActiveModel = user.into();
    u.password = Set(Some(crate::password::hash(&password)));
    u.remember_token = Set(Some(crate::crypto::random_string(60)));
    if disable_two_factor {
        u.two_factor_secret = Set(None);
        u.two_factor_recovery_codes = Set(None);
        u.two_factor_confirmed_at = Set(None);
    }
    u.update(&state.db).await?;
    crate::http::session::delete_other_sessions(state, user_id, None).await?;
    Ok(password)
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, NaiveTime};
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};

    use super::*;
    use crate::domains::base::VaultPermission;
    use crate::domains::contact::manage_reminders::{CreateContactReminder, TYPE_RECURRING_YEAR};
    use crate::pushover::Credentials;
    use crate::testing::pushover::FakePushover;
    use crate::testing::{TestDb, factory};

    #[tokio::test]
    async fn test_reminders_sends_every_scheduled_reminder_now() {
        let db = TestDb::migrated().await;
        let pushover = FakePushover::start().await;
        let mut config = Config::for_tests("", std::env::temp_dir());
        config.pushover_api_url = pushover.url();
        let state = AppState::new(db.conn.clone(), config.clone()).unwrap();
        let user = factory::create_user(&db.conn).await;
        let vault = factory::create_vault_user(&db.conn, &user, VaultPermission::Manage).await;
        let contact = factory::contact(&db.conn, &vault).await;
        user_notification_channels::ActiveModel {
            user_id: Set(Some(user.id)),
            r#type: Set(TYPE_PUSHOVER.into()),
            label: Set(Some("me".into())),
            content: Set(Credentials { user_key: "u-me".into(), app_token: "a-monica".into() }.to_content()),
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
        let sent = pushover.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!((sent[0]["user"].as_str(), sent[0]["token"].as_str()), ("u-me", "a-monica"));
        assert_eq!(sent[0]["message"], "Birthday");
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
        assert_eq!(pushover.sent().len(), 2);

        let mut production = config.clone();
        production.env = Environment::Production;
        assert_eq!(test_reminders(&state, &production).await.unwrap(), 0);
        db.drop().await;
    }

    #[tokio::test]
    async fn reset_password_sets_a_new_password_and_can_turn_two_factor_off() {
        let db = TestDb::migrated().await;
        let state = AppState::new(db.conn.clone(), Config::for_tests("", std::env::temp_dir())).unwrap();
        let user = factory::create_user(&db.conn).await;
        let mut u: users::ActiveModel = user.clone().into();
        u.two_factor_secret = Set(Some("secret".into()));
        u.two_factor_confirmed_at = Set(Some(crate::db::now()));
        u.update(&db.conn).await.unwrap();

        let password = reset_password(&state, &user.username.to_uppercase(), false).await.unwrap();
        assert_eq!(password.len(), 20);
        let after = users::Entity::find_by_id(user.id).one(&db.conn).await.unwrap().unwrap();
        assert!(crate::password::verify(&password, after.password.as_deref().unwrap()));
        assert!(after.two_factor_confirmed_at.is_some(), "2FA stays on without the flag");

        reset_password(&state, &user.username, true).await.unwrap();
        let after = users::Entity::find_by_id(user.id).one(&db.conn).await.unwrap().unwrap();
        assert!(after.two_factor_secret.is_none() && after.two_factor_confirmed_at.is_none());
        assert!(reset_password(&state, "nobody", false).await.is_err());
        db.drop().await;
    }
}
