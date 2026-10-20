//! `database/migrations/2022_07_31_200647_create_sessions_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("sessions", |t| {
        // `string('id')->primary()`
        t.string("id");
        t.primary(&["id"]);
        t.foreign_id_for("user_id", Key::Uuid).nullable().column().index();
        t.string_len("ip_address", 45).nullable();
        t.text("user_agent").nullable();
        t.long_text("payload");
        t.integer("last_activity").index();
    });
}
