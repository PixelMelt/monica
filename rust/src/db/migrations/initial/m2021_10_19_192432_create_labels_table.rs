//! `database/migrations/2021_10_19_192432_create_labels_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("labels", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();
        t.string("name");
        t.string("slug");
        t.text("description").nullable();
        t.string("bg_color").default("bg-zinc-200");
        t.string("text_color").default("text-zinc-700");
        t.timestamps();
    });

    s.create("contact_label", |t| {
        t.foreign_id_for("label_id", Key::Int).constrained("labels").cascade_on_delete();
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.timestamps();
    });
}
