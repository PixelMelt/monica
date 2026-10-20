//! `database/migrations/2013_04_25_155842_create_attributes_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("templates", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid)
            .constrained("accounts")
            .cascade_on_delete();
        t.string("name").nullable();
        t.string("name_translation_key").nullable();
        t.timestamps();
        // 2023_10_02_195229_add_can_be_deleted_to_templates
        t.boolean("can_be_deleted").default(true);
    });

    s.create("template_pages", |t| {
        t.id();
        t.foreign_id_for("template_id", Key::Int)
            .constrained("templates")
            .cascade_on_delete();
        t.string("name").nullable();
        t.string("name_translation_key").nullable();
        t.string("slug");
        t.integer("position").nullable();
        t.string("type").nullable();
        t.boolean("can_be_deleted").default(true);
        t.timestamps();
    });

    s.create("modules", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid)
            .constrained("accounts")
            .cascade_on_delete();
        t.string("name").nullable();
        t.string("name_translation_key").nullable();
        t.string("type").nullable();
        t.boolean("reserved_to_contact_information").default(false);
        t.boolean("can_be_deleted").default(true);
        t.integer("pagination").nullable();
        t.timestamps();
    });

    s.create("module_template_page", |t| {
        t.foreign_id_for("template_page_id", Key::Int)
            .constrained("template_pages")
            .cascade_on_delete();
        t.foreign_id_for("module_id", Key::Int)
            .constrained("modules")
            .cascade_on_delete();
        t.integer("position").nullable();
        t.timestamps();
    });

    s.create("module_rows", |t| {
        t.id();
        t.foreign_id_for("module_id", Key::Int)
            .constrained("modules")
            .cascade_on_delete();
        t.integer("position").nullable();
        t.timestamps();
    });

    s.create("module_row_fields", |t| {
        t.id();
        t.foreign_id_for("module_row_id", Key::Int)
            .constrained("module_rows")
            .cascade_on_delete();
        t.string("label");
        t.string("module_field_type");
        t.boolean("required").default(false);
        t.integer("position").nullable();
        t.timestamps();
    });
}
