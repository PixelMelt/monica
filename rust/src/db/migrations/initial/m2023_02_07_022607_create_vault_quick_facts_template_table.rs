//! `database/migrations/2023_02_07_022607_create_vault_quick_facts_template_table.php`
//! (its `contacts.show_quick_facts` belongs to `contacts`).

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("vault_quick_facts_templates", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid)
            .constrained("vaults")
            .cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.integer("position");
        t.timestamps();
    });

    s.create("quick_facts", |t| {
        t.id();
        t.foreign_id_for("vault_quick_facts_template_id", Key::Int)
            .constrained("vault_quick_facts_templates")
            .cascade_on_delete();
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.string("content");
        t.timestamps();
    });
}
