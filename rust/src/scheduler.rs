//! The scheduler (`routes/console.php`, K013): every minute, each task whose
//! `crons` row says it is due runs, in order. `CronEvent` decides when a task
//! is due, exactly as `app/Console/Scheduling/CronEvent.php` does.

use std::future::Future;
use std::pin::Pin;

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::db::timestamp::Timestamp;
use crate::entities::{crons, logs, sessions};
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

/// The schedule of `routes/console.php`, in its order.
pub fn tasks() -> Vec<Task> {
    vec![
        Task { command: "model:prune", frequency: Frequency::Daily, run: |s| Box::pin(prune(s)) },
        Task { command: "storage:sweep", frequency: Frequency::Daily, run: |s| Box::pin(sweep_storage(s)) },
        Task { command: "logs:clean", frequency: Frequency::Daily, run: |s| Box::pin(clean_logs(s)) },
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
