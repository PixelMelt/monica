//! `database/migrations/2023_08_30_202650_create_logs_table.php`

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("logs", |t| {
        t.id();
        t.unsigned_big_integer("group_id").index();
        t.string("level").index();
        t.string("level_name");
        t.string("channel").index();
        t.long_text("message").nullable();
        t.long_text("context").nullable();
        t.long_text("extra").nullable();
        t.long_text("formatted").nullable();
        t.morphs("loggable");
        t.datetime("logged_at");
        t.timestamps();
    });
}
