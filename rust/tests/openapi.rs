//! The OpenAPI document generated from the code (utoipa) agrees with the
//! approved draft, `docs/rewrite/openapi.yaml`, for every operation the
//! server implements: same path, method and operationId, same
//! `x-permission` and `x-inventory`, same success status codes, and the
//! document is OpenAPI 3.1.

use std::collections::BTreeSet;

use serde_json::Value;

fn draft() -> Value {
    let raw = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/rewrite/openapi.yaml")).unwrap();
    serde_yaml_ng::from_str(&raw).unwrap()
}

fn operations(doc: &Value) -> Vec<(String, String, Value)> {
    let mut out = Vec::new();
    for (path, item) in doc["paths"].as_object().unwrap() {
        for (method, op) in item.as_object().unwrap() {
            if ["get", "post", "put", "patch", "delete"].contains(&method.as_str()) {
                out.push((path.clone(), method.clone(), op.clone()));
            }
        }
    }
    out
}

fn success_codes(op: &Value) -> BTreeSet<String> {
    op["responses"].as_object().unwrap().keys().filter(|k| k.starts_with('2')).cloned().collect()
}

#[test]
fn generated_operations_match_the_draft() {
    let generated = serde_json::to_value(monica::api::openapi()).unwrap();
    assert_eq!(generated["openapi"], "3.1.0");
    let draft = draft();
    let mut problems = Vec::new();
    for (path, method, op) in operations(&generated) {
        let id = op["operationId"].as_str().unwrap_or_default();
        let Some(d) = draft["paths"].get(&path).and_then(|p| p.get(&method)) else {
            problems.push(format!("{method} {path} ({id}) is not in the draft"));
            continue;
        };
        if d["operationId"] != op["operationId"] {
            problems.push(format!("{method} {path}: operationId {id} vs draft {}", d["operationId"]));
        }
        for ext in ["x-permission", "x-inventory"] {
            if d[ext] != op[ext] {
                problems.push(format!("{method} {path} ({id}): {ext} {} vs draft {}", op[ext], d[ext]));
            }
        }
        if success_codes(d) != success_codes(&op) {
            problems.push(format!(
                "{method} {path} ({id}): success codes {:?} vs draft {:?}",
                success_codes(&op),
                success_codes(d)
            ));
        }
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}
