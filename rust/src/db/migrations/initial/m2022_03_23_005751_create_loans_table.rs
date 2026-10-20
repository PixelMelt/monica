//! `database/migrations/2022_03_23_005751_create_loans_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("loans", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid)
            .constrained("vaults")
            .cascade_on_delete();
        t.string("type");
        t.string("name");
        t.text("description").nullable();
        t.integer("amount_lent").nullable();
        t.foreign_id_for("currency_id", Key::Int)
            .nullable()
            .constrained("currencies")
            .null_on_delete();
        t.datetime("loaned_at").nullable();
        t.boolean("settled").default(false);
        t.datetime("settled_at").nullable();
        t.timestamps();
    });

    s.create("contact_loan", |t| {
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
