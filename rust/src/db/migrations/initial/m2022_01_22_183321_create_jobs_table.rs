//! `database/migrations/2022_01_22_183321_create_jobs_table.php`

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("jobs", |t| {
        t.big_increments("id");
        t.string("queue").index();
        t.long_text("payload");
        t.unsigned_tiny_integer("attempts");
        t.unsigned_integer("reserved_at").nullable();
        t.unsigned_integer("available_at");
        t.unsigned_integer("created_at");
    });
}
