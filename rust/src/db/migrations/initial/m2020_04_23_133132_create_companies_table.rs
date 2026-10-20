//! `database/migrations/2020_04_23_133132_create_companies_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("companies", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid)
            .constrained("vaults")
            .cascade_on_delete();
        t.string("name");
        t.string("type").nullable();
        t.timestamps();
    });
}
