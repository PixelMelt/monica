//! `database/migrations/2020_03_20_213318_create_address_types_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("address_types", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid).constrained("accounts").cascade_on_delete();
        t.string("name").nullable();
        t.string("type").nullable();
        t.string("name_translation_key").nullable();
        t.timestamps();
    });
}
