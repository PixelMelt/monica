//! `database/migrations/2019_12_14_000001_create_personal_access_tokens_table.php`

use crate::db::blueprint::{Engine, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("personal_access_tokens", |t| {
        t.id();
        t.morphs("tokenable");
        t.string("name");
        t.string_len("token", 64).unique();
        t.text("abilities").nullable();
        t.timestamp("last_used_at").nullable();
        t.timestamp("expires_at").nullable();
        t.timestamps();
    });
}
