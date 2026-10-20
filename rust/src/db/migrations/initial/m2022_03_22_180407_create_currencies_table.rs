//! `database/migrations/2022_03_22_180407_create_currencies_table.php`

use sea_orm::{ConnectionTrait, DbErr};

use crate::db::blueprint::{Engine, Key, Schema};

pub fn up(s: &mut Schema, _e: Engine) {
    s.create("currencies", |t| {
        t.id();
        t.string("code");
        t.timestamps();
    });

    s.create("account_currency", |t| {
        t.id();
        t.foreign_id_for("currency_id", Key::Int).constrained("currencies").cascade_on_delete();
        t.foreign_id_for("account_id", Key::Uuid).constrained("accounts").cascade_on_delete();
        t.boolean("active").default(true);
        t.timestamps();
    });
}

/// The currency codes the migration inserts, in insertion order (ids 1..=164).
pub const CODES: [&str; 164] = [
    "AFA", "ALL", "DZD", "AOA", "ARS", "AMD", "AWG", "AUD", "AZN", "BSD", "BHD", "BDT",
    "BBD", "BYR", "BEF", "BZD", "BMD", "BTN", "BTC", "BOB", "BAM", "BWP", "BRL", "GBP",
    "BND", "BGN", "BIF", "KHR", "CAD", "CVE", "KYD", "XOF", "XAF", "XPF", "CLP", "CNY",
    "COP", "KMF", "CDF", "CRC", "HRK", "CUC", "CZK", "DKK", "DJF", "DOP", "XCD", "EGP",
    "ERN", "EEK", "ETB", "EUR", "FKP", "FJD", "GMD", "GEL", "DEM", "GHS", "GIP", "GRD",
    "GTQ", "GNF", "GYD", "HTG", "HNL", "HKD", "HUF", "ISK", "INR", "IDR", "IRR", "IQD",
    "ILS", "ITL", "JMD", "JPY", "JOD", "KZT", "KES", "KWD", "KGS", "LAK", "LVL", "LBP",
    "LSL", "LRD", "LYD", "LTL", "MOP", "MKD", "MGA", "MWK", "MYR", "MVR", "MRO", "MUR",
    "MXN", "MDL", "MNT", "MAD", "MZM", "MMK", "NAD", "NPR", "ANG", "TWD", "NZD", "NIO",
    "NGN", "KPW", "NOK", "OMR", "PKR", "PAB", "PGK", "PYG", "PEN", "PHP", "PLN", "QAR",
    "RON", "RUB", "RWF", "SVC", "WST", "SAR", "RSD", "SCR", "SLL", "SGD", "SKK", "SBD",
    "SOS", "ZAR", "KRW", "XDR", "LKR", "SHP", "SDG", "SRD", "SZL", "SEK", "CHF", "SYP",
    "STD", "TJS", "TZS", "THB", "TOP", "TTD", "TND", "TRY", "TMT", "UGX", "UAH", "AED",
    "UYU", "USD", "UZS", "VUV", "VEF", "VND", "YER", "ZM",
];

pub async fn seed<C: ConnectionTrait>(db: &C, _e: Engine) -> Result<(), DbErr> {
    let values: Vec<String> = CODES.iter().map(|c| format!("('{c}')")).collect();
    db.execute_unprepared(&format!("insert into currencies (code) values {}", values.join(", "))).await?;
    Ok(())
}
