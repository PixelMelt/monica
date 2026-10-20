//! `database/migrations/2014_10_12_000000_create_users_table.php`

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("users", |t| {
        t.uuid("id");
        t.primary(&["id"]);
        t.foreign_id_for("account_id", Key::Uuid)
            .constrained("accounts")
            .cascade_on_delete();
        t.string("first_name").nullable();
        t.string("last_name").nullable();
        t.string("password").nullable();
        t.string("email").unique();
        t.timestamp("email_verified_at").nullable();
        t.string("name_order").default("%first_name% %last_name%");
        t.string("date_format").default("MMM DD, YYYY");
        t.string("timezone").nullable();
        t.string_len("number_format", 8).default("locale");
        t.string("default_map_site").default("open_street_maps");
        t.string("distance_format").default("mi");
        t.boolean("is_account_administrator").default(false);
        t.boolean("help_shown").default(true);
        t.string("invitation_code").nullable();
        t.datetime("invitation_accepted_at").nullable();
        t.string("locale").default("en");
        t.remember_token();
        t.timestamps();
        // 2014_10_12_200000_add_two_factor_columns_to_users_table
        t.text("two_factor_secret").nullable();
        t.text("two_factor_recovery_codes").nullable();
        t.timestamp("two_factor_confirmed_at").nullable();
        // 2023_05_06_125432_add_contact_sort_order_to_users
        t.string("contact_sort_order").default("last_updated");
        // 2023_06_12_093907_add_instance_administrator
        t.boolean("is_instance_administrator").default(false);
    });
}
