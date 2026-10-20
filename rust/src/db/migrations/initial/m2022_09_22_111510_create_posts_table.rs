//! `database/migrations/2022_09_22_111510_create_posts_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("post_templates", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid).constrained("accounts").cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.integer("position");
        t.boolean("can_be_deleted").default(true);
        t.timestamps();
    });

    s.create("post_template_sections", |t| {
        t.id();
        t.foreign_id_for("post_template_id", Key::Int).constrained("post_templates").cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.integer("position");
        t.boolean("can_be_deleted").default(true);
        t.timestamps();
    });

    s.create("posts", |t| {
        t.id();
        t.foreign_id_for("journal_id", Key::Int).constrained("journals").cascade_on_delete();
        t.boolean("published").default(false);
        t.string("title").nullable();
        t.integer("view_count").default(0);
        t.datetime("written_at");
        t.timestamps();
        // 2022_12_15_004442_create_slices_of_life_table
        t.foreign_id_for("slice_of_life_id", Key::Int).nullable().constrained("slice_of_lives").null_on_delete();
    });

    s.create("post_sections", |t| {
        t.id();
        t.foreign_id_for("post_id", Key::Int).constrained("posts").cascade_on_delete();
        t.integer("position");
        t.string("label");
        t.text("content").nullable();
        t.timestamps();
    });

    s.create("contact_post", |t| {
        t.foreign_id_for("post_id", Key::Int).constrained("posts").cascade_on_delete();
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.timestamps();
    });

    s.create("tags", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();
        t.string("name");
        t.string("slug");
        t.timestamps();
    });

    s.create("post_tag", |t| {
        t.foreign_id_for("tag_id", Key::Int).constrained("tags").cascade_on_delete();
        t.foreign_id_for("post_id", Key::Int).constrained("posts").cascade_on_delete();
        t.timestamps();
    });
}
