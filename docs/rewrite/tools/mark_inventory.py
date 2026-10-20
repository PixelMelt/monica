#!/usr/bin/env python3
"""Set the Status column of docs/rewrite/inventory.md from the code.

Usage: python3 docs/rewrite/tools/mark_inventory.py [--write]

A row becomes `done (test: …)` when its port and a test of it exist, and
`removed` when its disposition is `remove` (the removed features have no
code in rust/ or resources/js). Without --write, prints what would change
and the rows still open.

- Routes (R, V): the operations whose `x-inventory` lists the row, in the
  generated spec of the Rust code; the test is the HTTP test file of the
  module that implements the operation. Rows in `x-not-api` and the other
  sections map through MANUAL.
- Classes (C): the Rust item named after the PHP class (`pub struct Name`,
  or a doc comment naming it) under rust/src; the test is the tests file
  of its module. Other classes map through MANUAL.
"""

import glob
import os
import re
import subprocess
import sys

import yaml

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
INVENTORY = os.path.join(ROOT, "docs/rewrite/inventory.md")
RUST = os.path.join(ROOT, "rust")

# HTTP test file of each API module.
API_TESTS = {
    "account": "tests/account.rs",
    "auth": "tests/auth.rs",
    "me": "tests/auth.rs",
    "instance": "tests/http_basics.rs",
    "notification_channels": "tests/account.rs",
    "vaults": "tests/vaults.rs",
}

SMOKE = "tests/smoke/smoke.mjs"

# Rows whose port isn't found by the rules above: row id → test.
MANUAL = {}


def load_manual():
    path = os.path.join(os.path.dirname(__file__), "inventory_manual.yaml")
    if os.path.exists(path):
        MANUAL.update(yaml.safe_load(open(path)) or {})


def operations():
    """operationId → module file stem, and row id → operationIds, from the
    code's spec and the draft's x-inventory."""
    impl = {}
    for path in glob.glob(os.path.join(RUST, "src/api/*.rs")):
        for oid in re.findall(r'operation_id = "(\w+)"', open(path).read()):
            impl[oid] = os.path.basename(path)[:-3]
    draft = yaml.safe_load(open(os.path.join(ROOT, "docs/rewrite/openapi.yaml")))
    rows = {}
    for item in draft["paths"].values():
        for op in item.values():
            if isinstance(op, dict) and "operationId" in op:
                for row in op.get("x-inventory", []):
                    rows.setdefault(row, []).append(op["operationId"])
    not_api = {x["id"] for x in draft.get("x-not-api", [])}
    return impl, rows, not_api


def rust_items():
    """PHP class name → tests file of the Rust module that ports it."""
    found = {}
    for path in glob.glob(os.path.join(RUST, "src/**/*.rs"), recursive=True):
        rel = os.path.relpath(path, RUST)
        if "/tests" in rel or rel.endswith("tests.rs"):
            continue
        text = open(path).read()
        names = set(re.findall(r"^pub (?:struct|enum|fn|async fn) (\w+)", text, re.M))
        names |= set(re.findall(r"^\s*//[/!] `(?:App\\\\[\w\\\\]*\\\\)?(\w+)`", text, re.M))
        mod_dir = os.path.dirname(path)
        test = None
        for candidate in (os.path.join(mod_dir, "tests.rs"), os.path.join(mod_dir, "tests")):
            if os.path.exists(candidate):
                test = os.path.relpath(candidate, RUST)
                break
        if test is None and re.search(r"#\[cfg\(test\)\]\s*mod tests \{", text):
            test = rel
        if test:
            for n in names:
                found.setdefault(n, test)
    return found


def main():
    load_manual()
    impl, by_row, not_api = operations()
    items = rust_items()
    lines = open(INVENTORY).read().split("\n")
    changed, open_rows = [], []
    for i, line in enumerate(lines):
        m = re.match(r"^\| ([A-Z]\d{3}) \|", line)
        if not m:
            continue
        rid = m.group(1)
        cells = line.split("|")
        status = cells[-2].strip()
        if status.startswith("done") or status.startswith("removed"):
            continue
        new = None
        if "remove" in status:
            new = "removed"
        elif rid in MANUAL and os.path.exists(os.path.join(RUST, MANUAL[rid])):
            new = f"done (test: `{os.path.normpath(os.path.join('rust', MANUAL[rid]))}`)"
        elif rid[0] in "RV" and rid in by_row:
            tests = sorted({API_TESTS.get(impl[o], f"tests/{impl[o]}.rs") for o in by_row[rid] if o in impl})
            missing = [o for o in by_row[rid] if o not in impl]
            if tests and not missing and all(os.path.exists(os.path.join(RUST, t)) for t in tests):
                new = "done (test: " + ", ".join(f"`{t}`" for t in tests) + ")"
        elif rid[0] == "C":
            cls = re.search(r"`app/[\w/]*?(\w+)\.php`", line)
            if cls and cls.group(1) in items:
                new = f"done (test: `{items[cls.group(1)]}`)"
        if new:
            cells[-2] = f" {new} "
            lines[i] = "|".join(cells)
            changed.append((rid, new))
        else:
            open_rows.append(line[:160])
    print(f"{len(changed)} rows marked, {len(open_rows)} still open")
    for row in open_rows:
        print("  open:", row)
    if "--write" in sys.argv:
        open(INVENTORY, "w").write("\n".join(lines))


main()
