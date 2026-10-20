//! `database/migrations/2022_09_20_183401_create_journal_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("journals", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();
        t.string("name");
        t.text("description").nullable();
        t.timestamps();
    });
}
