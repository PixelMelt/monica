//! `database/migrations/2021_10_21_013005_create_notes_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, e: Engine) {
    s.create("notes", |t| {
        t.id();
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();
        t.foreign_id_for("author_id", Key::Uuid).nullable().constrained("users").null_on_delete();
        t.foreign_id_for("emotion_id", Key::Int).nullable().constrained("emotions").null_on_delete();
        t.string("title").nullable();
        t.text("body");
        t.timestamps();

        if e.full_text() {
            t.full_text(&["title"]);
            t.full_text(&["body"]);
        }
    });
}
