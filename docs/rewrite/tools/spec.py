#!/usr/bin/env python3
"""Assemble docs/rewrite/api/*.yaml into docs/rewrite/openapi.yaml and check it.

Usage: python3 docs/rewrite/tools/spec.py [--check] [--json PATH]

Checks: OpenAPI 3.1 structure (against docs/rewrite/tools/oas-3.1-schema.json,
from spec.openapis.org) and JSON Schema 2020-12 validity of every schema;
duplicate paths/methods, component names and operationIds; unresolved $refs;
undeclared path parameters; required operation fields (operationId, tags,
x-permission, x-inventory, x-services, responses); and that every inventory
row marked `port` in §2 and §3 of inventory.md is covered by an operation's
x-inventory or by x-not-api, and that no `remove` row is.
With --check the merged spec is not written. With --json the merged spec is
also written as JSON to PATH (the input of the frontend client generator).
"""
import glob
import json
import os
import re
import sys

import yaml

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
API = os.path.join(ROOT, "docs/rewrite/api")
OUT = os.path.join(ROOT, "docs/rewrite/openapi.yaml")
INVENTORY = os.path.join(ROOT, "docs/rewrite/inventory.md")

METHODS = ("get", "put", "post", "delete", "patch", "head", "options")
PERMISSIONS = {"public", "user", "account:member", "account:admin", "instance:admin", "vault:view", "vault:edit", "vault:manage"}
COMPONENT_KINDS = ("schemas", "parameters", "responses", "requestBodies", "headers", "securitySchemes")

errors = []


def err(msg):
    errors.append(msg)


def load(path):
    with open(path, encoding="utf-8") as f:
        return yaml.safe_load(f) or {}


files = sorted(glob.glob(os.path.join(API, "*.yaml")))
base_path = os.path.join(API, "00-base.yaml")
spec = load(base_path)
spec.setdefault("paths", {})
spec.setdefault("components", {})
not_api = list(spec.get("x-not-api", []))
origin = {}  # (kind, name) or path+method -> file

for f in files:
    if f == base_path:
        continue
    name = os.path.basename(f)
    frag = load(f)
    unknown = set(frag) - {"paths", "components", "x-not-api", "tags"}
    if unknown:
        err(f"{name}: unknown top-level keys {sorted(unknown)}")
    for t in frag.get("tags", []) or []:
        if t not in spec["tags"]:
            spec["tags"].append(t)
    not_api += frag.get("x-not-api", []) or []
    for p, item in (frag.get("paths") or {}).items():
        target = spec["paths"].setdefault(p, {})
        for k, v in item.items():
            if k in target and k != "parameters":
                err(f"{name}: {k.upper()} {p} already defined in {origin.get((p, k))}")
                continue
            if k == "parameters" and "parameters" in target:
                if target["parameters"] != v:
                    err(f"{name}: path-level parameters of {p} differ from {origin.get((p, 'parameters'))}")
                continue
            target[k] = v
            origin[(p, k)] = name
    for kind, defs in (frag.get("components") or {}).items():
        if kind not in COMPONENT_KINDS:
            err(f"{name}: unknown component kind {kind}")
            continue
        target = spec["components"].setdefault(kind, {})
        for n, d in (defs or {}).items():
            if n in target:
                err(f"{name}: components.{kind}.{n} already defined in {origin.get((kind, n), '00-base.yaml')}")
                continue
            target[n] = d
            origin[(kind, n)] = name

spec["x-not-api"] = not_api
spec["paths"] = dict(sorted(spec["paths"].items()))

# ------------------------------------------------------------------ $refs
def walk(node, fn, path=()):
    if isinstance(node, dict):
        fn(node, path)
        for k, v in node.items():
            walk(v, fn, path + (k,))
    elif isinstance(node, list):
        for i, v in enumerate(node):
            walk(v, fn, path + (i,))


def check_ref(node, path):
    ref = node.get("$ref")
    if not isinstance(ref, str):
        return
    m = re.match(r"^#/components/(\w+)/([^/]+)$", ref)
    if not m:
        err(f"bad $ref {ref} at {'/'.join(map(str, path))}")
        return
    kind, n = m.groups()
    if n not in spec["components"].get(kind, {}):
        err(f"unresolved $ref {ref} at {'/'.join(map(str, path[:3]))}")


walk(spec, check_ref)

# ------------------------------------------------------------------ operations
def resolve_param(p):
    if "$ref" in p:
        return spec["components"].get("parameters", {}).get(p["$ref"].split("/")[-1], {})
    return p


op_ids = {}
covered = {}
for p, item in spec["paths"].items():
    path_params = set(re.findall(r"{(\w+)}", p))
    level = {resolve_param(x).get("name") for x in item.get("parameters", []) if resolve_param(x).get("in") == "path"}
    for m, op in item.items():
        if m not in METHODS:
            continue
        where = f"{m.upper()} {p} ({origin.get((p, m))})"
        oid = op.get("operationId")
        if not oid:
            err(f"{where}: no operationId")
        elif oid in op_ids:
            err(f"{where}: operationId {oid} also used by {op_ids[oid]}")
        else:
            op_ids[oid] = where
        if not op.get("tags"):
            err(f"{where}: no tags")
        for t in op.get("tags", []):
            if t not in [x["name"] if isinstance(x, dict) else x for x in spec["tags"]]:
                err(f"{where}: unknown tag {t}")
        if op.get("x-permission") not in PERMISSIONS:
            err(f"{where}: x-permission {op.get('x-permission')!r} not one of {sorted(PERMISSIONS)}")
        if not isinstance(op.get("x-inventory"), list):
            err(f"{where}: x-inventory missing")
        for rid in op.get("x-inventory") or []:
            covered.setdefault(rid, []).append(oid)
        if not isinstance(op.get("x-services"), list):
            err(f"{where}: x-services missing")
        if not op.get("responses"):
            err(f"{where}: no responses")
        declared = level | {resolve_param(x).get("name") for x in op.get("parameters", []) if resolve_param(x).get("in") == "path"}
        if path_params != declared:
            err(f"{where}: path params {sorted(path_params)} but declared {sorted(declared)}")
        if op.get("x-permission") == "public" and op.get("security") != []:
            err(f"{where}: public operations need `security: []`")

# ------------------------------------------------------------------ coverage
rows = {}
with open(INVENTORY, encoding="utf-8") as f:
    for line in f:
        m = re.match(r"^\| ([RV]\d{3}) \|(.*)\|\s*$", line)
        if m:
            cells = [c.strip() for c in m.group(2).split("|")]
            disp = cells[-2]
            rows[m.group(1)] = "remove" if disp.startswith("remove") else "port"

not_api_ids = {x["id"] for x in not_api}
for rid, disp in sorted(rows.items()):
    if disp == "port" and rid not in covered and rid not in not_api_ids:
        err(f"inventory {rid} (port) is not covered by any operation or x-not-api")
    if disp == "remove" and (rid in covered or rid in not_api_ids):
        err(f"inventory {rid} is marked remove but is covered by {covered.get(rid) or 'x-not-api'}")
for rid in sorted(set(covered) | not_api_ids):
    if re.match(r"^[RV]\d{3}$", rid) and rid not in rows:
        err(f"x-inventory cites unknown row {rid}")

# ------------------------------------------------------------------ OpenAPI 3.1 structure
try:
    import json

    import jsonschema

    with open(os.path.join(os.path.dirname(__file__), "oas-3.1-schema.json"), encoding="utf-8") as f:
        oas = jsonschema.Draft202012Validator(json.load(f))
    for e in sorted(oas.iter_errors(spec), key=lambda e: list(map(str, e.path))):
        err(f"OpenAPI 3.1: {'/'.join(map(str, e.path))}: {e.message[:200]}")
    for n, s in spec["components"].get("schemas", {}).items():
        for e in jsonschema.Draft202012Validator(jsonschema.Draft202012Validator.META_SCHEMA).iter_errors(s):
            err(f"schema {n}: {'/'.join(map(str, e.path))}: {e.message[:200]}")
except ImportError:
    print("jsonschema not installed; skipping OpenAPI structure validation")

n_ops = sum(1 for item in spec["paths"].values() for m in item if m in METHODS)
print(f"{len(files)} fragments, {len(spec['paths'])} paths, {n_ops} operations, "
      f"{len(spec['components'].get('schemas', {}))} schemas; "
      f"{sum(1 for r, d in rows.items() if d == 'port' and (r in covered or r in not_api_ids))}/"
      f"{sum(1 for d in rows.values() if d == 'port')} port rows covered")
if errors:
    print(f"{len(errors)} problems:")
    for e in errors:
        print("  " + e)

if "--check" not in sys.argv:
    with open(OUT, "w", encoding="utf-8") as f:
        f.write("# Generated by docs/rewrite/tools/spec.py from docs/rewrite/api/*.yaml. Do not edit.\n")
        yaml.safe_dump(spec, f, sort_keys=False, allow_unicode=True, width=100)
if "--json" in sys.argv:
    with open(sys.argv[sys.argv.index("--json") + 1], "w", encoding="utf-8") as f:
        json.dump(spec, f, indent=2, ensure_ascii=False)
        f.write("\n")
sys.exit(1 if errors else 0)
