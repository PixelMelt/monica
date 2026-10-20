//! `database/migrations/2022_05_16_193917_create_calls_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("calls", |t| {
        t.id();
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.foreign_id_for("call_reason_id", Key::Int).nullable().constrained("call_reasons").cascade_on_delete();
        t.foreign_id_for("author_id", Key::Uuid).nullable().constrained("users").null_on_delete();
        t.foreign_id_for("emotion_id", Key::Int).nullable().constrained("emotions").null_on_delete();
        t.string("author_name");
        t.datetime("called_at");
        t.integer("duration").nullable();
        t.string("type");
        t.text("description").nullable();
        t.boolean("answered").default(true);
        t.string("who_initiated");
        t.timestamps();
    });
}
