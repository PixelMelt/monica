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

/// The shape of a schema: objects as `{field: shape}` (`?` marks fields
/// that are not required), arrays as `[shape]`, everything else as `"*"`.
/// `$ref`s are resolved, `allOf` merged, and nullable variants dropped,
/// so the generated and the drafted documents compare field by field.
fn shape(doc: &Value, schema: &Value, depth: usize) -> Value {
    if depth > 64 {
        return Value::from("*");
    }
    if let Some(r) = schema["$ref"].as_str() {
        let target = r.trim_start_matches("#/").split('/').fold(doc, |v, k| &v[k]);
        return shape(doc, target, depth + 1);
    }
    if let Some(all) = schema["allOf"].as_array() {
        let mut merged = serde_json::Map::new();
        for s in all {
            if let Value::Object(m) = shape(doc, s, depth + 1) {
                merged.extend(m);
            }
        }
        if let Value::Object(m) = shape(doc, &strip(schema, "allOf"), depth + 1) {
            merged.extend(m);
        }
        return Value::Object(merged);
    }
    for key in ["oneOf", "anyOf"] {
        // `oneOf` next to `properties` only constrains which are required.
        if schema.get("properties").is_some() {
            break;
        }
        if let Some(variants) = schema[key].as_array() {
            let real: Vec<&Value> = variants.iter().filter(|v| v["type"] != "null").collect();
            if real.len() == 1 {
                return shape(doc, real[0], depth + 1);
            }
            let mut merged = serde_json::Map::new();
            for v in real {
                if let Value::Object(m) = shape(doc, v, depth + 1) {
                    merged.extend(m);
                }
            }
            return Value::Object(merged);
        }
    }
    let types: Vec<&str> = match &schema["type"] {
        Value::String(t) => vec![t.as_str()],
        Value::Array(ts) => ts.iter().filter_map(Value::as_str).collect(),
        _ => vec![],
    };
    if types.contains(&"array") {
        return Value::Array(vec![shape(doc, &schema["items"], depth + 1)]);
    }
    if let Some(props) = schema["properties"].as_object() {
        let required: BTreeSet<&str> =
            schema["required"].as_array().map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        return Value::Object(
            props
                .iter()
                .map(|(k, v)| {
                    let key = if required.contains(k.as_str()) { k.clone() } else { format!("{k}?") };
                    (key, shape(doc, v, depth + 1))
                })
                .collect(),
        );
    }
    Value::from("*")
}

fn strip(schema: &Value, key: &str) -> Value {
    let mut s = schema.clone();
    if let Value::Object(m) = &mut s {
        m.remove(key);
    }
    s
}

/// Differences between two shapes, as `path: …` lines.
fn diff(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let bare = |k: &str| k.trim_end_matches('?').to_owned();
            let yk: BTreeSet<String> = y.keys().map(|k| bare(k)).collect();
            let xk: BTreeSet<String> = x.keys().map(|k| bare(k)).collect();
            for k in xk.difference(&yk) {
                out.push(format!("{path}.{k}: only in the code"));
            }
            for k in yk.difference(&xk) {
                out.push(format!("{path}.{k}: only in the draft"));
            }
            for (k, v) in x {
                let other = y.get(k).map(|w| (k.clone(), w)).or_else(|| {
                    let alt = if k.ends_with('?') { bare(k) } else { format!("{k}?") };
                    y.get(&alt).map(|w| (alt, w))
                });
                // Required-ness is not compared: utoipa leaves `Option` fields
                // out of `required` where the draft lists them as nullable.
                if let Some((_, w)) = other {
                    diff(&format!("{path}.{}", bare(k)), v, w, out);
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => diff(&format!("{path}[]"), &x[0], &y[0], out),
        (Value::String(_), Value::String(_)) => {}
        _ => out.push(format!("{path}: {} vs {}", kind(a), kind(b))),
    }
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        _ => "scalar",
    }
}

fn json_schema(content: &Value) -> Option<&Value> {
    content.as_object()?.iter().find(|(t, _)| t.contains("json")).map(|(_, c)| &c["schema"])
}

#[test]
fn generated_bodies_match_the_draft() {
    let generated = serde_json::to_value(monica::api::openapi()).unwrap();
    let draft = draft();
    let mut problems = Vec::new();
    for (path, method, op) in operations(&generated) {
        let Some(d) = draft["paths"].get(&path).and_then(|p| p.get(&method)) else { continue };
        let id = op["operationId"].as_str().unwrap_or_default().to_owned();
        let mut out = Vec::new();
        if let (Some(a), Some(b)) =
            (json_schema(&op["requestBody"]["content"]), json_schema(&d["requestBody"]["content"]))
        {
            diff("request", &shape(&generated, a, 0), &shape(&draft, b, 0), &mut out);
        }
        for code in success_codes(d) {
            if let (Some(a), Some(b)) =
                (json_schema(&op["responses"][&code]["content"]), json_schema(&d["responses"][&code]["content"]))
            {
                diff(&code, &shape(&generated, a, 0), &shape(&draft, b, 0), &mut out);
            }
        }
        problems.extend(out.into_iter().map(|p| format!("{id}: {p}")));
    }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
}
