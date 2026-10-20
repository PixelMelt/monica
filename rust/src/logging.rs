//! The `database` log channel (`App\Logging\LoggingHandler`, C405/C406):
//! what the CardDAV client logs while it synchronizes a subscription is
//! stored in `logs`, attached to the subscription, with its `current_logid`
//! as the group. Other logs only go to the console.
//!
//! A `tracing` layer picks the events of the DAV client module that happen
//! inside an `addressbook_subscription` span (its `addressbook_subscription_id`
//! field), and a task writes them once [`start`] gave it the database.

use std::fmt::Write as _;
use std::sync::OnceLock;

use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use serde_json::{Map, Value};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use crate::db::timestamp::Timestamp;
use crate::db::uuid::Uuid;
use crate::entities::{addressbook_subscriptions, logs};

/// The module whose logs go to the `database` channel.
pub const TARGET: &str = "monica::domains::contact::dav_client";
/// `Loggable::class` of an address book subscription.
const LOGGABLE_TYPE: &str = "App\\Models\\AddressBookSubscription";

struct Record {
    subscription_id: Uuid,
    level: Level,
    message: String,
    context: Map<String, Value>,
    logged_at: Timestamp,
}

static SENDER: OnceLock<UnboundedSender<Record>> = OnceLock::new();

/// The layer to add to the subscriber; records nothing until [`start`].
pub fn layer() -> DatabaseLayer {
    DatabaseLayer
}

/// Starts writing the channel's records to `db`; `channel` is Monolog's
/// channel name (Laravel uses the environment's name).
pub fn start(db: DatabaseConnection, channel: String) {
    let (tx, mut rx) = unbounded_channel::<Record>();
    if SENDER.set(tx).is_err() {
        return;
    }
    tokio::spawn(async move {
        while let Some(record) = rx.recv().await {
            if let Err(e) = write(&db, &channel, record).await {
                eprintln!("database log channel: {e}");
            }
        }
    });
}

/// `LoggingHandler::write`: a row for the record, unless the subscription
/// is gone.
async fn write(db: &DatabaseConnection, channel: &str, record: Record) -> anyhow::Result<()> {
    let Some(subscription) = addressbook_subscriptions::Entity::find_by_id(record.subscription_id).one(db).await?
    else {
        return Ok(());
    };
    let (level, level_name) = monolog_level(record.level);
    let context = (!record.context.is_empty()).then(|| Value::Object(record.context.clone()).to_string());
    let formatted = format!(
        "[{}] {channel}.{level_name}: {} {} []\n",
        record.logged_at.0.format("%Y-%m-%dT%H:%M:%S%.6f+00:00"),
        record.message,
        context.as_deref().unwrap_or("[]"),
    );
    logs::ActiveModel {
        group_id: Set(subscription.current_logid.unwrap_or(crate::db::id::Id(0))),
        level: Set(level.to_string()),
        level_name: Set(level_name.into()),
        channel: Set(channel.into()),
        message: Set(Some(record.message)),
        context: Set(context),
        extra: Set(None),
        formatted: Set(Some(formatted)),
        loggable_type: Set(LOGGABLE_TYPE.into()),
        loggable_id: Set(subscription.id),
        logged_at: Set(record.logged_at),
        created_at: Set(Some(crate::db::now())),
        updated_at: Set(Some(crate::db::now())),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(())
}

/// Monolog's level number and name.
fn monolog_level(level: Level) -> (u16, &'static str) {
    match level {
        Level::ERROR => (400, "ERROR"),
        Level::WARN => (300, "WARNING"),
        Level::INFO => (200, "INFO"),
        _ => (100, "DEBUG"),
    }
}

pub struct DatabaseLayer;

/// The subscription a span belongs to.
struct SubscriptionId(Uuid);

#[derive(Default)]
struct Fields {
    message: String,
    context: Map<String, Value>,
    subscription_id: Option<Uuid>,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.record(field, Value::from(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record(field, Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record(field, Value::from(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record(field, Value::from(value));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let mut s = String::new();
        let _ = write!(s, "{value:?}");
        self.record(field, Value::from(s));
    }
}

impl Fields {
    fn record(&mut self, field: &Field, value: Value) {
        match field.name() {
            "message" => self.message = value.as_str().map(str::to_owned).unwrap_or_else(|| value.to_string()),
            "addressbook_subscription_id" => {
                self.subscription_id = value.as_str().and_then(|s| s.parse().ok());
                self.context.insert(field.name().into(), value);
            }
            name => {
                self.context.insert(name.into(), value);
            }
        }
    }
}

impl<S> Layer<S> for DatabaseLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        if let (Some(subscription), Some(span)) = (fields.subscription_id, ctx.span(id)) {
            span.extensions_mut().insert(SubscriptionId(subscription));
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let Some(sender) = SENDER.get() else { return };
        if !event.metadata().target().starts_with(TARGET) {
            return;
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        let subscription = fields.subscription_id.or_else(|| {
            ctx.event_scope(event)?.find_map(|span| span.extensions().get::<SubscriptionId>().map(|s| s.0))
        });
        let Some(subscription_id) = subscription else { return };
        fields.context.insert("addressbook_subscription_id".into(), Value::from(subscription_id.to_string()));
        let _ = sender.send(Record {
            subscription_id,
            level: *event.metadata().level(),
            message: fields.message,
            context: fields.context,
            logged_at: crate::db::now(),
        });
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ColumnTrait, QueryFilter};
    use tracing_subscriber::prelude::*;

    use super::*;
    use crate::db::id::Id;
    use crate::testing::{TestDb, factory};

    #[tokio::test]
    async fn stores_the_dav_client_logs_of_a_subscription() {
        let db = TestDb::migrated().await;
        let user = factory::user(&db.conn, None).await;
        let vault = factory::vault(&db.conn, user.account_id).await;
        let subscription = addressbook_subscriptions::ActiveModel {
            id: Set(Uuid::new()),
            user_id: Set(user.id),
            vault_id: Set(vault.id),
            uri: Set("https://dav.example.com/".into()),
            username: Set("me@example.com".into()),
            password: Set(String::new()),
            active: Set(true),
            sync_way: Set(3.into()),
            capabilities: Set("{}".into()),
            current_logid: Set(Some(Id(4))),
            frequency: Set(180),
            ..Default::default()
        }
        .insert(&db.conn)
        .await
        .unwrap();
        start(db.conn.clone(), "testing".into());

        let subscriber = tracing_subscriber::registry().with(layer());
        let _guard = tracing::subscriber::set_default(subscriber);
        let span = tracing::info_span!(target: TARGET, "addressbook_subscription", addressbook_subscription_id = %subscription.id);
        {
            let _entered = span.enter();
            tracing::info!(target: "monica::domains::contact::dav_client::jobs", "Synchronize addressbook 'Family'");
            tracing::info!(target: "monica::api", "not the database channel");
        }
        tracing::info!(target: "monica::domains::contact::dav_client::jobs", "outside any subscription");

        let mut rows = Vec::new();
        for _ in 0..50 {
            rows =
                logs::Entity::find().filter(logs::Column::LoggableId.eq(subscription.id)).all(&db.conn).await.unwrap();
            if !rows.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.message.as_deref(), Some("Synchronize addressbook 'Family'"));
        assert_eq!((row.level.as_str(), row.level_name.as_str(), row.channel.as_str()), ("200", "INFO", "testing"));
        assert_eq!(row.group_id, Id(4));
        assert_eq!(row.loggable_type, LOGGABLE_TYPE);
        assert!(row.context.as_deref().unwrap().contains(&subscription.id.to_string()));
        assert!(row.formatted.as_deref().unwrap().contains("testing.INFO: Synchronize addressbook 'Family'"));
        db.drop().await;
    }
}
