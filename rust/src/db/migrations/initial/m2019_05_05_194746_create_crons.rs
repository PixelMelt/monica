//! `database/migrations/2019_05_05_194746_create_crons.php`

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("crons", |t| {
        t.increments("id");
        t.string("command").unique();
        t.timestamp("last_run_at").nullable();
        t.timestamps();
    });
}
