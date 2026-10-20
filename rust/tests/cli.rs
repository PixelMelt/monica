//! The console commands of the binary (inventory §5): version, key
//! generation, migrations on a new SQLite file (with its directory created,
//! as `WaitForDb` does), the OpenAPI document, and test-reminders.

use std::process::Command;

fn monica() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_monica"));
    cmd.env_remove("DATABASE_URL").env_remove("RUST_LOG");
    cmd
}

fn run(cmd: &mut Command) -> String {
    let out = cmd.output().expect("run monica");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn prints_the_version() {
    let out = run(monica().arg("--version"));
    assert!(out.starts_with("monica "), "{out}");
}

#[test]
fn generates_a_key_and_migrates_a_new_database() {
    let key = run(monica().arg("key-generate")).trim().to_owned();
    assert_eq!(base64_len(&key), 32, "32 bytes of key");

    let dir = std::env::temp_dir().join(format!("monica-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let db = dir.join("nested").join("monica.sqlite");
    let url = format!("sqlite://{}?mode=rwc", db.display());
    for _ in 0..2 {
        run(monica().arg("migrate").env("APP_KEY", &key).env("DATABASE_URL", &url).env("DATA_DIR", &dir));
    }
    assert!(db.exists());

    let out = run(monica()
        .arg("test-reminders")
        .env("APP_KEY", &key)
        .env("APP_ENV", "production")
        .env("DATABASE_URL", &url)
        .env("DATA_DIR", &dir));
    assert_eq!(out.trim(), "0 reminders sent.");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn prints_the_openapi_document() {
    let out = run(monica().arg("openapi"));
    let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(doc["openapi"], "3.1.0");
    assert!(doc["paths"].as_object().unwrap().len() > 150);
}

fn base64_len(key: &str) -> usize {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(key.trim_start_matches("base64:")).map(|k| k.len()).unwrap_or(0)
}
