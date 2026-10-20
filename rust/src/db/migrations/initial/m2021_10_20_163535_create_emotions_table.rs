//! `database/migrations/2021_10_20_163535_create_emotions_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("emotions", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid).constrained("accounts").cascade_on_delete();
        t.string("name").nullable();
        t.string("name_translation_key").nullable();
        t.string("type");
        t.timestamps();
    });
}
