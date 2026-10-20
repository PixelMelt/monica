//! `database/migrations/2020_02_19_173445_create_pronouns_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("pronouns", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid)
            .constrained("accounts")
            .cascade_on_delete();
        t.string("name").nullable();
        t.string("name_translation_key").nullable();
        t.timestamps();
    });
}
