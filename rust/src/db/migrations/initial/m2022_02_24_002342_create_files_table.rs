//! `database/migrations/2022_02_24_002342_create_files_table.php`
//!
//! The migration also adds `contacts.file_id`; that column is folded into
//! `contacts` by its own module.

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("files", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();

        t.nullable_numeric_morphs("fileable");
        t.uuid("ufileable_id").nullable();
        t.index(&["fileable_type", "ufileable_id"]);

        t.string("uuid");
        t.string("original_url").nullable();
        t.string("cdn_url").nullable();
        t.string("mime_type");
        t.string("name");
        t.string("type");
        t.integer("size");
        t.timestamps();
    });
}
