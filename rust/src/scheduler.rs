//! The scheduler (`routes/console.php`, K013): every minute, each task whose
//! `crons` row says it is due runs, in order. `CronEvent` decides when a task
//! is due, exactly as `app/Console/Scheduling/CronEvent.php` does.

use std::future::Future;
use std::pin::Pin;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::timestamp::Timestamp;
use crate::domains::contact::dav::jobs::CleanSyncToken;
use crate::domains::contact::manage_reminders::ProcessScheduledContactReminders;
use crate::entities::{crons, failed_jobs, job_batches, logs, sessions};
use crate::error::ApiResult;
use crate::http::state::AppState;

/// How often a task runs (`->hourly()`, `->daily()`, `->minutes(n)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    Minutes(i64),
    Hourly,
    Daily,
    Weekly,
}

impl Frequency {
    /// (`$minutes`, `$days`) of CronEvent.
    fn minutes_days(self) -> (i64, i64) {
        match self {
            Frequency::Minutes(m) => (m, 0),
            Frequency::Hourly => (60, 0),
            Frequency::Daily => (0, 1),
            Frequency::Weekly => (0, 7),
        }
    }
}

/// `CronEvent::isDue`: the next run is the start of the period after the
/// one of the last run; a task that never ran is due. When due, records
/// `now` as the last run.
pub async fn is_due<C: ConnectionTrait>(
    db: &C,
    command: &str,
    frequency: Frequency,
    now: NaiveDateTime,
) -> ApiResult<bool> {
    let cron = match crons::Entity::find().filter(crons::Column::Command.eq(command)).one(db).await? {
        Some(c) => c,
        None => crons::ActiveModel { command: Set(command.to_owned()), ..Default::default() }.insert(db).await?,
    };
    if let Some(t) = cron.last_run_at.map(|t| t.0) {
        let (minutes, days) = frequency.minutes_days();
        let next_run = if minutes != 0 {
            let minute = (t.minute() as i64 / minutes) * minutes;
            t.date().and_hms_opt(t.hour(), 0, 0).map(|d| d + Duration::minutes(minute + minutes))
        } else if days != 0 {
            let day = (t.day() as i64 / days) * days;
            // Carbon::create(…, day 0, …) is the last day of the previous month.
            NaiveDate::from_ymd_opt(t.year(), t.month(), 1)
                .map(|first| first + Duration::days(day - 1 + days))
                .and_then(|d| d.and_hms_opt(0, 0, 0))
        } else {
            None
        };
        match next_run {
            Some(next) if next <= now => {}
            _ => return Ok(false),
        }
    }
    let mut c: crons::ActiveModel = cron.into();
    c.last_run_at = Set(Some(Timestamp(now)));
    c.update(db).await?;
    Ok(true)
}

type TaskFuture<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;

/// A scheduled task: its `crons.command` name, frequency and body.
pub struct Task {
    pub command: &'static str,
    pub frequency: Frequency,
    pub run: for<'a> fn(&'a AppState) -> TaskFuture<'a>,
}

/// The schedule of `routes/console.php`, in its order (the command names
/// are its `crons.command` values), then the storage sweep (E009).
pub fn tasks() -> Vec<Task> {
    vec![
        Task { command: "model:prune", frequency: Frequency::Daily, run: |s| Box::pin(prune(s)) },
        Task {
            command: "queue:prune-batches --hours=48 --unfinished=72 --cancelled=72",
            frequency: Frequency::Daily,
            run: |s| Box::pin(prune_batches(s)),
        },
        Task {
            command: "queue:prune-failed --hours=48",
            frequency: Frequency::Daily,
            run: |s| Box::pin(prune_failed(s)),
        },
        Task {
            command: "App\\Domains\\Contact\\DavClient\\Jobs\\UpdateAddressBooks",
            frequency: Frequency::Hourly,
            run: |s| Box::pin(crate::domains::contact::dav_client::update_address_books(s)),
        },
        Task {
            command: "App\\Domains\\Contact\\ManageReminders\\Jobs\\ProcessScheduledContactReminders",
            frequency: Frequency::Minutes(1),
            run: |s| Box::pin(process_reminders(s)),
        },
        Task {
            command: "App\\Domains\\Contact\\Dav\\Jobs\\CleanSyncToken",
            frequency: Frequency::Daily,
            run: |s| Box::pin(clean_sync_tokens(s)),
        },
        Task { command: "App\\Logging\\CleanLogs", frequency: Frequency::Daily, run: |s| Box::pin(clean_logs(s)) },
        Task { command: "storage:sweep", frequency: Frequency::Daily, run: |s| Box::pin(sweep_storage(s)) },
    ]
}

/// Runs the tasks that are due at `now`.
pub async fn run_due(state: &AppState, now: NaiveDateTime) -> anyhow::Result<()> {
    for task in tasks() {
        if is_due(&state.db, task.command, task.frequency, now).await?
            && let Err(e) = (task.run)(state).await
        {
            tracing::error!(task = task.command, "scheduled task failed: {e:#}");
        }
    }
    Ok(())
}

/// Ticks at the start of every minute until the process ends.
pub async fn run_forever(state: AppState) {
    loop {
        let now = chrono::Utc::now();
        let wait = 60 - now.second() as u64;
        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        if let Err(e) = run_due(&state, crate::db::now().0).await {
            tracing::error!("scheduler: {e:#}");
        }
    }
}

/// `model:prune`: sessions past their lifetime (the remember-me lifetime
/// for the longest ones).
async fn prune(state: &AppState) -> anyhow::Result<()> {
    let cutoff = chrono::Utc::now().timestamp() - crate::http::session::REMEMBER_MINUTES * 60;
    sessions::Entity::delete_many().filter(sessions::Column::LastActivity.lt(cutoff as i32)).exec(&state.db).await?;
    Ok(())
}

/// `queue:prune-batches --hours=48 --unfinished=72 --cancelled=72` (J006).
async fn prune_batches(state: &AppState) -> anyhow::Result<()> {
    let now = chrono::Utc::now().timestamp();
    let hours = |h: i64| i32::try_from(now - h * 3600).unwrap_or(i32::MAX);
    job_batches::Entity::delete_many()
        .filter(job_batches::Column::FinishedAt.is_not_null())
        .filter(job_batches::Column::FinishedAt.lt(hours(48)))
        .exec(&state.db)
        .await?;
    job_batches::Entity::delete_many()
        .filter(job_batches::Column::FinishedAt.is_null())
        .filter(job_batches::Column::CreatedAt.lt(hours(72)))
        .exec(&state.db)
        .await?;
    job_batches::Entity::delete_many()
        .filter(job_batches::Column::CancelledAt.is_not_null())
        .filter(job_batches::Column::CreatedAt.lt(hours(72)))
        .exec(&state.db)
        .await?;
    Ok(())
}

/// `queue:prune-failed --hours=48` (J007).
async fn prune_failed(state: &AppState) -> anyhow::Result<()> {
    let cutoff = Timestamp(crate::db::now().0 - Duration::hours(48));
    failed_jobs::Entity::delete_many().filter(failed_jobs::Column::FailedAt.lt(cutoff)).exec(&state.db).await?;
    Ok(())
}

/// `ProcessScheduledContactReminders` (J001).
async fn process_reminders(state: &AppState) -> anyhow::Result<()> {
    ProcessScheduledContactReminders.execute(&state.db, &state.mailer, &state.config).await?;
    Ok(())
}

/// `CleanSyncToken` (J003).
async fn clean_sync_tokens(state: &AppState) -> anyhow::Result<()> {
    CleanSyncToken.execute(&state.db).await?;
    Ok(())
}

/// Deletes stored bytes no `files` row points at (E009).
async fn sweep_storage(state: &AppState) -> anyhow::Result<()> {
    let removed = state.storage.sweep(&state.db).await?;
    if removed > 0 {
        tracing::info!("storage sweep removed {removed} files");
    }
    Ok(())
}

/// `App\Logging\CleanLogs` (J004): logs older than 15 days.
async fn clean_logs(state: &AppState) -> anyhow::Result<()> {
    let cutoff = Timestamp(crate::db::now().0 - Duration::days(15));
    logs::Entity::delete_many().filter(logs::Column::CreatedAt.lt(cutoff)).exec(&state.db).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! `tests/Unit/Commands/Scheduling/CronEventTest.php`.

    use super::*;
    use crate::testing::TestDb;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
    }

    async fn last_run(db: &TestDb, command: &str) -> Option<NaiveDateTime> {
        crons::Entity::find()
            .filter(crons::Column::Command.eq(command))
            .one(&db.conn)
            .await
            .unwrap()
            .unwrap()
            .last_run_at
            .map(|t| t.0)
    }

    #[test]
    fn the_schedule_is_the_one_of_routes_console() {
        let schedule: Vec<(&str, Frequency)> = tasks().iter().map(|t| (t.command, t.frequency)).collect();
        assert_eq!(
            schedule,
            [
                ("model:prune", Frequency::Daily),
                ("queue:prune-batches --hours=48 --unfinished=72 --cancelled=72", Frequency::Daily),
                ("queue:prune-failed --hours=48", Frequency::Daily),
                ("App\\Domains\\Contact\\DavClient\\Jobs\\UpdateAddressBooks", Frequency::Hourly),
                (
                    "App\\Domains\\Contact\\ManageReminders\\Jobs\\ProcessScheduledContactReminders",
                    Frequency::Minutes(1)
                ),
                ("App\\Domains\\Contact\\Dav\\Jobs\\CleanSyncToken", Frequency::Daily),
                ("App\\Logging\\CleanLogs", Frequency::Daily),
                ("storage:sweep", Frequency::Daily),
            ]
        );
    }

    #[tokio::test]
    async fn due_tasks_run_and_clean_up() {
        let db = TestDb::migrated().await;
        let state = AppState::new(db.conn.clone(), crate::config::Config::for_tests("", std::env::temp_dir())).unwrap();
        let now = crate::db::now();
        let old = Timestamp(now.0 - Duration::hours(49));
        for (uuid, failed_at) in [("old", old), ("new", now)] {
            failed_jobs::ActiveModel {
                uuid: Set(uuid.into()),
                connection: Set("database".into()),
                queue: Set("default".into()),
                payload: Set("{}".into()),
                exception: Set("boom".into()),
                failed_at: Set(failed_at),
                ..Default::default()
            }
            .insert(&db.conn)
            .await
            .unwrap();
        }
        // queue:prune-batches: finished 48 h ago, unfinished or cancelled 72 h ago.
        let hours_ago = |h: i64| i32::try_from(now.0.and_utc().timestamp() - h * 3600).unwrap();
        for (id, created, finished, cancelled) in [
            ("finished-old", hours_ago(100), Some(hours_ago(49)), None),
            ("finished-new", hours_ago(100), Some(hours_ago(1)), None),
            ("unfinished-old", hours_ago(73), None, None),
            ("unfinished-new", hours_ago(1), None, None),
            ("cancelled-old", hours_ago(73), None, Some(hours_ago(73))),
        ] {
            job_batches::ActiveModel {
                id: Set(id.into()),
                name: Set(String::new()),
                total_jobs: Set(1),
                pending_jobs: Set(0),
                failed_jobs: Set(0),
                failed_job_ids: Set("[]".into()),
                options: Set(None),
                cancelled_at: Set(cancelled),
                created_at: Set(created),
                finished_at: Set(finished),
            }
            .insert(&db.conn)
            .await
            .unwrap();
        }
        // model:prune: sessions past the longest (remember me) lifetime.
        let stale = now.0.and_utc().timestamp() - crate::http::session::REMEMBER_MINUTES * 60 - 60;
        for (id, last_activity) in [("stale", stale), ("live", now.0.and_utc().timestamp())] {
            sessions::ActiveModel {
                id: Set(id.into()),
                user_id: Set(None),
                ip_address: Set(None),
                user_agent: Set(None),
                payload: Set(String::new()),
                last_activity: Set(i32::try_from(last_activity).unwrap()),
            }
            .insert(&db.conn)
            .await
            .unwrap();
        }
        // CleanLogs: logs older than 15 days.
        for (message, created_at) in [("old", Timestamp(now.0 - Duration::days(16))), ("new", now)] {
            logs::ActiveModel {
                group_id: Set(crate::db::id::Id(0)),
                level: Set("200".into()),
                level_name: Set("INFO".into()),
                channel: Set("testing".into()),
                message: Set(Some(message.into())),
                loggable_type: Set("App\\Models\\AddressBookSubscription".into()),
                loggable_id: Set(crate::db::uuid::Uuid::new()),
                logged_at: Set(created_at),
                created_at: Set(Some(created_at)),
                updated_at: Set(Some(created_at)),
                ..Default::default()
            }
            .insert(&db.conn)
            .await
            .unwrap();
        }

        run_due(&state, now.0).await.unwrap();

        let left: Vec<String> =
            failed_jobs::Entity::find().all(&db.conn).await.unwrap().into_iter().map(|f| f.uuid).collect();
        assert_eq!(left, ["new"]);
        let mut batches: Vec<String> =
            job_batches::Entity::find().all(&db.conn).await.unwrap().into_iter().map(|b| b.id).collect();
        batches.sort();
        assert_eq!(batches, ["finished-new", "unfinished-new"]);
        let sessions: Vec<String> =
            sessions::Entity::find().all(&db.conn).await.unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(sessions, ["live"]);
        let logs: Vec<Option<String>> =
            logs::Entity::find().all(&db.conn).await.unwrap().into_iter().map(|l| l.message).collect();
        assert_eq!(logs, [Some("new".to_owned())]);
        assert_eq!(crons::Entity::find().all(&db.conn).await.unwrap().len(), tasks().len());
        db.drop().await;
    }

    #[tokio::test]
    async fn it_get_the_right_command() {
        let db = TestDb::migrated().await;
        is_due(&db.conn, "test:command", Frequency::Minutes(1), at(2019, 5, 1, 7, 0)).await.unwrap();
        is_due(&db.conn, "test:command", Frequency::Minutes(1), at(2019, 5, 1, 7, 0)).await.unwrap();
        assert_eq!(crons::Entity::find().all(&db.conn).await.unwrap().len(), 1);
        db.drop().await;
    }

    #[tokio::test]
    async fn next_minute_is_due() {
        let db = TestDb::migrated().await;
        assert!(is_due(&db.conn, "c", Frequency::Minutes(1), at(2019, 5, 1, 7, 0)).await.unwrap(), "first run");
        assert!(!is_due(&db.conn, "c", Frequency::Minutes(1), at(2019, 5, 1, 7, 0)).await.unwrap(), "now_not_due");
        assert!(is_due(&db.conn, "c", Frequency::Minutes(1), at(2019, 5, 1, 7, 1)).await.unwrap());
        assert_eq!(last_run(&db, "c").await, Some(at(2019, 5, 1, 7, 1)));
        db.drop().await;
    }

    #[tokio::test]
    async fn hourly_cron() {
        let db = TestDb::migrated().await;
        is_due(&db.conn, "c", Frequency::Hourly, at(2019, 5, 1, 7, 0)).await.unwrap();
        assert!(!is_due(&db.conn, "c", Frequency::Hourly, at(2019, 5, 1, 7, 0)).await.unwrap());
        assert!(is_due(&db.conn, "c", Frequency::Hourly, at(2019, 5, 1, 8, 22)).await.unwrap());
        assert_eq!(last_run(&db, "c").await, Some(at(2019, 5, 1, 8, 22)));
        assert!(!is_due(&db.conn, "c", Frequency::Hourly, at(2019, 5, 1, 8, 59)).await.unwrap());
        assert!(is_due(&db.conn, "c", Frequency::Hourly, at(2019, 5, 1, 9, 1)).await.unwrap());
        assert_eq!(last_run(&db, "c").await, Some(at(2019, 5, 1, 9, 1)));
        db.drop().await;
    }

    #[tokio::test]
    async fn daily_cron() {
        let db = TestDb::migrated().await;
        is_due(&db.conn, "c", Frequency::Daily, at(2019, 5, 1, 7, 0)).await.unwrap();
        assert!(is_due(&db.conn, "c", Frequency::Daily, at(2019, 5, 2, 8, 10)).await.unwrap());
        assert_eq!(last_run(&db, "c").await, Some(at(2019, 5, 2, 8, 10)));
        assert!(!is_due(&db.conn, "c", Frequency::Daily, at(2019, 5, 2, 10, 0)).await.unwrap());
        assert!(is_due(&db.conn, "c", Frequency::Daily, at(2019, 5, 3, 0, 0)).await.unwrap());
        assert_eq!(last_run(&db, "c").await, Some(at(2019, 5, 3, 0, 0)));
        db.drop().await;
    }
}
