//! Records Monica's version and commit at build time, as `config/monica.php`
//! read them: `rust/.version` / `rust/.commit`, else `git describe --abbrev=0
//! --tags` / the HEAD commit. `APP_VERSION` and `APP_COMMIT` override them at
//! run time.

use std::process::Command;

fn read(file: &str, git: &[&str]) -> Option<String> {
    if let Ok(content) = std::fs::read_to_string(file) {
        return Some(content.trim().to_owned());
    }
    let out = Command::new("git").args(git).output().ok()?;
    let s = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (out.status.success() && !s.is_empty()).then_some(s)
}

fn main() {
    let version = read(".version", &["describe", "--abbrev=0", "--tags"]).unwrap_or_else(|| "0.0.0".into());
    println!("cargo:rustc-env=MONICA_VERSION={version}");
    if let Some(commit) = read(".commit", &["log", "--pretty=%H", "-n1", "HEAD"]) {
        println!("cargo:rustc-env=MONICA_COMMIT={commit}");
    }
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs/tags");
    println!("cargo:rerun-if-changed=.version");
    println!("cargo:rerun-if-changed=.commit");
}
