//! `database/migrations/2022_06_09_173049_create_gifts_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("gift_occasions", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid)
            .constrained("accounts")
            .cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.integer("position").nullable();
        t.timestamps();
    });

    s.create("gift_states", |t| {
        t.id();
        t.foreign_id_for("account_id", Key::Uuid)
            .constrained("accounts")
            .cascade_on_delete();
        t.string("label").nullable();
        t.string("label_translation_key").nullable();
        t.integer("position").nullable();
        t.timestamps();
    });

    s.create("gifts", |t| {
        t.id();
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.string("type");
        t.string("name");
        t.text("description").nullable();
        t.integer("estimated_price").nullable();
        t.foreign_id_for("currency_id", Key::Int)
            .nullable()
            .constrained("currencies")
            .null_on_delete();
        t.datetime("received_at").nullable();
        t.datetime("given_at").nullable();
        t.datetime("bought_at").nullable();
        t.timestamps();
    });

    s.create("contact_gift", |t| {
        t.foreign_id_for("loan_id", Key::Int)
            .constrained("loans")
            .cascade_on_delete();
        t.foreign_id_for("loaner_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.foreign_id_for("loanee_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.timestamps();
    });
}
