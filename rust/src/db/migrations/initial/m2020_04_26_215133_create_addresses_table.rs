//! `database/migrations/2020_04_26_215133_create_addresses_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("addresses", |t| {
        t.id();
        t.foreign_id_for("vault_id", Key::Uuid)
            .constrained("vaults")
            .cascade_on_delete();
        t.foreign_id_for("address_type_id", Key::Int)
            .nullable()
            .constrained("address_types")
            .null_on_delete();
        t.string("line_1").nullable();
        t.string("line_2").nullable();
        t.string("city").nullable();
        t.string("province").nullable();
        t.string("postal_code").nullable();
        t.string("country").nullable();
        t.double("latitude").nullable();
        t.double("longitude").nullable();
        t.timestamps();
    });

    s.create("contact_address", |t| {
        t.foreign_id_for("contact_id", Key::Uuid)
            .constrained("contacts")
            .cascade_on_delete();
        t.foreign_id_for("address_id", Key::Int)
            .constrained("addresses")
            .cascade_on_delete();
        t.boolean("is_past_address").default(false);
        t.timestamps();
    });
}
