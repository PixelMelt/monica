//! `database/migrations/2019_08_19_000000_create_failed_jobs_table.php`

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("failed_jobs", |t| {
        t.id();
        t.string("uuid").unique();
        t.text("connection");
        t.text("queue");
        t.long_text("payload");
        t.long_text("exception");
        t.timestamp("failed_at").use_current();
    });
}
