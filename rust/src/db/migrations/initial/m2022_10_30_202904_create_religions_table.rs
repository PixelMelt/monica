//! `database/migrations/2022_10_30_202904_create_religions_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("religions", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid).constrained("accounts").cascade_on_delete();
        t.string("name").nullable();
        t.string("translation_key").nullable();
        t.integer("position").nullable();
        t.timestamps();
    });
}
