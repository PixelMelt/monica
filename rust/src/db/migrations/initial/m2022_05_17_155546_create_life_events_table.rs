//! `database/migrations/2022_05_17_155546_create_life_events_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("life_event_categories", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();
        t.integer("position").nullable();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.boolean("can_be_deleted").default(false);
        t.timestamps();
    });

    s.create("life_event_types", |t| {
        t.id();
        t.foreign_id_for("life_event_category_id", Key::Int).constrained("life_event_categories").cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.boolean("can_be_deleted").default(false);
        t.integer("position").nullable();
        t.timestamps();
    });

    s.create("timeline_events", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid).constrained("vaults").cascade_on_delete();
        t.date("started_at");
        t.string("label").nullable();
        t.boolean("collapsed").default(true);
        t.timestamps();
    });

    s.create("timeline_event_participants", |t| {
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.foreign_id_for("timeline_event_id", Key::Int).constrained("timeline_events").cascade_on_delete();
        t.timestamps();
    });

    s.create("life_events", |t| {
        t.id();
        t.foreign_id_for("timeline_event_id", Key::Int).constrained("timeline_events").cascade_on_delete();
        t.foreign_id_for("life_event_type_id", Key::Int).constrained("life_event_types").cascade_on_delete();
        t.foreign_id_for("emotion_id", Key::Int).nullable().constrained("emotions").null_on_delete();
        t.date("happened_at");
        t.boolean("collapsed").default(false);
        t.string("summary").nullable();
        t.text("description").nullable();
        t.integer("costs").nullable();
        t.foreign_id_for("currency_id", Key::Int).nullable().constrained("currencies").null_on_delete();
        t.foreign_id_for("paid_by_contact_id", Key::Uuid).nullable().constrained("contacts").null_on_delete();
        t.integer("duration_in_minutes").nullable();
        t.integer("distance").nullable();
        t.char("distance_unit", 2).nullable();
        t.string("from_place").nullable();
        t.string("to_place").nullable();
        t.string("place").nullable();
        t.timestamps();
    });

    s.create("life_event_participants", |t| {
        t.foreign_id_for("contact_id", Key::Uuid).constrained("contacts").cascade_on_delete();
        t.foreign_id_for("life_event_id", Key::Int).constrained("life_events").cascade_on_delete();
        t.timestamps();
    });
}
