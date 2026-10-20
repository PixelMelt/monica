//! `database/migrations/2020_04_25_133132_create_contacts_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, e: Engine) {
    s.create("contacts", |t| {
        t.uuid("id");
        t.primary(&["id"]);
        t.foreign_id_for("vault_id", Key::Uuid)
            .constrained("vaults")
            .cascade_on_delete();
        t.foreign_id_for("gender_id", Key::Int)
            .nullable()
            .constrained("genders")
            .null_on_delete();
        t.foreign_id_for("pronoun_id", Key::Int)
            .nullable()
            .constrained("pronouns")
            .null_on_delete();
        t.foreign_id_for("template_id", Key::Int)
            .nullable()
            .constrained("templates")
            .null_on_delete();
        t.foreign_id_for("company_id", Key::Int)
            .nullable()
            .constrained("companies")
            .null_on_delete();
        t.string("first_name").nullable();
        t.string("middle_name").nullable();
        t.string("last_name").nullable();
        t.string("nickname").nullable();
        t.string("maiden_name").nullable();
        t.string("suffix").nullable();
        t.string("prefix").nullable();
        t.string("job_position").nullable();
        t.boolean("can_be_deleted").default(true);
        t.boolean("listed").default(true);

        t.medium_text("vcard").nullable();
        t.string_len("distant_uuid", 256).nullable();
        t.string_len("distant_etag", 256).nullable();
        t.string_len("distant_uri", 2096).nullable();

        t.datetime("last_updated_at").nullable();
        t.soft_deletes();
        t.timestamps();

        t.index(&["vault_id", "id"]);

        if e.full_text() {
            t.full_text(&["first_name"]);
            t.full_text(&["last_name"]);
            t.full_text(&["middle_name"]);
            t.full_text(&["nickname"]);
            t.full_text(&["maiden_name"]);
        }
        // 2022_02_24_002342_create_files_table
        t.foreign_id_for("file_id", Key::Int)
            .nullable()
            .constrained("files")
            .null_on_delete();
        // 2022_11_01_174411_add_religion_to_contact
        t.foreign_id_for("religion_id", Key::Int)
            .nullable()
            .constrained("religions")
            .null_on_delete();
        // 2023_02_07_022607_create_vault_quick_facts_template_table
        t.boolean("show_quick_facts").default(false);
    });

    s.create("user_vault", |t| {
        t.foreign_id_for("vault_id", Key::Uuid)
            .constrained("vaults")
            .cascade_on_delete();
        t.foreign_id_for("user_id", Key::Uuid)
            .constrained("users")
            .cascade_on_delete();
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.integer("permission");
        t.timestamps();
    });

    s.create("contact_vault_user", |t| {
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.foreign_id_for("vault_id", Key::Uuid)
            .constrained("vaults")
            .cascade_on_delete();
        t.foreign_id_for("user_id", Key::Uuid)
            .constrained("users")
            .cascade_on_delete();
        t.integer("number_of_views");
        t.boolean("is_favorite").default(false);
        t.timestamps();
    });
}
