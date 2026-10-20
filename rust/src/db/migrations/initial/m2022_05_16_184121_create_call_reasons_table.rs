//! `database/migrations/2022_05_16_184121_create_call_reasons_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("call_reason_types", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid).constrained("accounts").cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.timestamps();
    });

    s.create("call_reasons", |t| {
        t.id();
        t.foreign_id_for("call_reason_type_id", Key::Int).constrained("call_reason_types").cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.timestamps();
    });
}
