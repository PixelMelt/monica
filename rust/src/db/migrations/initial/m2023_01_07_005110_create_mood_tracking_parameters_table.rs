//! `database/migrations/2023_01_07_005110_create_mood_tracking_parameters_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("mood_tracking_parameters", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid)
            .constrained("vaults")
            .cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.string("hex_color");
        t.integer("position").nullable();
        t.timestamps();
    });
}
