#!/usr/bin/env python3
"""Generate the route -> controller -> service -> ViewHelper -> Vue page table.

Usage: python3 docs/rewrite/tools/inventory.py --write docs/rewrite/inventory.md
       (also: no args = route table, --classes = class table, --json = routes as JSON)
Requires `php docs/rewrite/tools/routes.php` to work (no Laravel boot needed).
"""
import json
import os
import re
import subprocess
import sys
from collections import defaultdict

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))


def rel(p):
    return os.path.relpath(p, ROOT)


def class_file(fqcn):
    if not fqcn.startswith("App\\"):
        return None
    p = os.path.join(ROOT, "app", *fqcn[4:].split("\\")) + ".php"
    return p if os.path.exists(p) else None


def read(p):
    with open(p, encoding="utf-8") as f:
        return f.read()


USE_RE = re.compile(r"^use\s+([A-Za-z0-9_\\]+)(?:\s+as\s+(\w+))?;", re.M)
NS_RE = re.compile(r"^namespace\s+([A-Za-z0-9_\\]+);", re.M)


def imports(src):
    m = NS_RE.search(src)
    ns = m.group(1) if m else ""
    out = {}
    for full, alias in USE_RE.findall(src):
        out[alias or full.split("\\")[-1]] = full
    return ns, out


def resolve(name, ns, uses):
    if name in uses:
        return uses[name]
    if name.startswith("\\"):
        return name[1:]
    return ns + "\\" + name


def method_bodies(src):
    """Return {method_name: body} using brace matching."""
    out = {}
    for m in re.finditer(r"(?:public|protected|private)\s+(?:static\s+)?function\s+(\w+)\s*\(", src):
        i = src.find("{", m.end())
        semi = src.find(";", m.end())
        if i < 0 or (0 <= semi < i):
            continue
        depth, j = 0, i
        while j < len(src):
            c = src[j]
            if c == "{":
                depth += 1
            elif c == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        out[m.group(1)] = src[i : j + 1]
    return out


def short(fqcn):
    return fqcn.split("\\")[-1]


# ---------------------------------------------------------------- routes
routes = [json.loads(l) for l in subprocess.check_output(["php", os.path.join(ROOT, "docs/rewrite/tools/routes.php")], text=True).splitlines()]

# ---------------------------------------------------------------- php index
php_files = []
for d, _, fs in os.walk(os.path.join(ROOT, "app")):
    php_files += [os.path.join(d, f) for f in fs if f.endswith(".php")]

class_src = {}
for p in php_files:
    src = read(p)
    ns, _ = imports(src)
    cls = os.path.basename(p)[:-4]
    class_src[ns + "\\" + cls] = (p, src)

# route('name' usages -> which classes build that url
url_builders = defaultdict(set)
for fq, (p, src) in class_src.items():
    for name in re.findall(r"route\(\s*'([A-Za-z0-9_.\-]+)'", src):
        url_builders[name].add(fq)


def refs_in(body, ns, uses, pattern):
    out = set()
    for name in re.findall(pattern, body):
        out.add(resolve(name, ns, uses))
    return out


# ViewHelper call graph at method granularity: (class, method) -> callees, urls
VH_CALL = r"\b([A-Z]\w*ViewHelper)::\w+\("
VH_METHOD_CALL = r"\b([A-Z]\w*ViewHelper|self|static)::(\w+)\("
vh_graph = {}
for fq, (p, src) in class_src.items():
    if fq.endswith("ViewHelper"):
        ns, uses = imports(src)
        for m, body in method_bodies(src).items():
            callees = set()
            for c, mm in re.findall(VH_METHOD_CALL, body):
                callees.add((fq if c in ("self", "static") else resolve(c, ns, uses), mm))
            for mm in re.findall(r"\$this->(\w+)\(", body):
                callees.add((fq, mm))
            urls = set(re.findall(r"route\(\s*'([A-Za-z0-9_.\-]+)'", body))
            vh_graph[(fq, m)] = (callees, urls)


def vh_urls(calls):
    """All route names built by the given (class, method) calls, transitively."""
    seen, stack, urls = set(), list(calls), set()
    while stack:
        k = stack.pop()
        if k in seen or k not in vh_graph:
            continue
        seen.add(k)
        callees, u = vh_graph[k]
        urls |= u
        stack += list(callees)
    return urls


# ---------------------------------------------------------------- controller analysis
def analyze(controller, action):
    info = {"services": set(), "jobs": set(), "vhs": set(), "vh_calls": set(), "page": None, "kind": "", "file": None, "urls": set()}
    f = class_file(controller)
    if not f:
        return info
    info["file"] = rel(f)
    src = read(f)
    ns, uses = imports(src)
    bodies = method_bodies(src)
    if action and action not in bodies:
        info["kind"] = "missing-action"
        return info
    body = bodies.get(action, "")
    # follow private helpers in the same controller
    for helper in re.findall(r"\$this->(\w+)\(", body):
        body += bodies.get(helper, "")
    for name in re.findall(r"new\s+\\?([A-Z][\w\\]*)", body):
        fq = resolve(name, ns, uses)
        if "\\Services\\" in fq or "\\Actions\\" in fq:
            info["services"].add(fq)
        elif "\\Jobs\\" in fq:
            info["jobs"].add(fq)
    for name in re.findall(r"app\(\s*\\?([A-Z][\w\\]*)::class", body):
        info["services"].add(resolve(name, ns, uses))
    for name in re.findall(r"\b([A-Z]\w*)::dispatch(?:Sync)?\(", body):
        fq = resolve(name, ns, uses)
        (info["jobs"] if "\\Jobs\\" in fq else info["services"]).add(fq)
    info["vhs"] = refs_in(body, ns, uses, VH_CALL)
    info["vh_calls"] = {(resolve(c, ns, uses), m) for c, m in re.findall(r"\b([A-Z]\w*ViewHelper)::(\w+)\(", body)}
    info["urls"] = set(re.findall(r"route\(\s*'([A-Za-z0-9_.\-]+)'", body))
    m = re.search(r"Inertia::render\(\s*'([^']+)'", body)
    if m:
        info["page"] = m.group(1)
        info["kind"] = "page"
    elif "response()->json" in body or "Resource" in body:
        info["kind"] = "json"
    elif re.search(r"redirect|Redirect::", body):
        info["kind"] = "redirect"
    elif "download" in body or "stream" in body:
        info["kind"] = "file"
    return info


analyses = {}
for r in routes:
    analyses[(r["controller"], r["action"])] = analyze(r["controller"], r["action"])

# Pages that consume each route. A page route seeds its own page; every non-page
# route whose URL that page's controller or ViewHelpers build gets the page as a
# consumer; JSON routes pass their consumers on to the non-page URLs their own
# ViewHelpers build (e.g. a JSON detail endpoint that returns edit/delete urls).
# Routes whose URL is built by the shared layout are marked "Layout".
LAYOUT = {("App\\Domains\\Vault\\ManageVault\\Web\\ViewHelpers\\VaultIndexViewHelper", "layoutData")}
layout_urls = vh_urls(LAYOUT)
page_routes = {r["name"] for r in routes if analyses[(r["controller"], r["action"])]["page"]}
consumers = defaultdict(set)
for r in routes:
    a = analyses[(r["controller"], r["action"])]
    if a["page"]:
        consumers[r["name"]].add(a["page"])


def built_by(a):
    return (a["urls"] | vh_urls(a["vh_calls"] - LAYOUT)) - page_routes


changed = True
while changed:
    changed = False
    for r in routes:
        if not r["name"] or not consumers.get(r["name"]):
            continue
        pages = consumers[r["name"]]
        for n in built_by(analyses[(r["controller"], r["action"])]):
            if not pages <= consumers[n]:
                consumers[n] |= pages
                changed = True
for n in layout_urls:
    consumers[n].add("Layout")


# Vue files that call a route by name through Ziggy
vue_refs = defaultdict(set)
for d, _, fs in os.walk(os.path.join(ROOT, "resources/js")):
    for f in fs:
        if f.endswith(".vue") or f.endswith(".js"):
            fp = os.path.join(d, f)
            for name in re.findall(r"route\(\s*'([A-Za-z0-9_.\-]+)'", read(fp)):
                vue_refs[name].add(rel(fp).replace("resources/js/", ""))


def pages_for_route(name):
    return consumers.get(name, set())


# ---------------------------------------------------------------- disposition
REMOVE_CONTROLLERS = {
    "SocialiteCallbackController": "OAuth/Socialite removed",
    "UserTokenController": "OAuth/Socialite removed",
    "TelegramWebhookController": "Telegram removed",
    "TelegramNotificationsController": "Telegram removed",
    "ContactModuleAddressImageController": "Mapbox static maps removed",
}


def disposition(r, a):
    c = short(r["controller"])
    if c in REMOVE_CONTROLLERS:
        return "remove", REMOVE_CONTROLLERS[c]
    if r["controller"].startswith("Redirect:"):
        return "port", "redirect"
    if a["kind"] == "missing-action":
        return "remove", "dead route (controller action does not exist)"
    if a["kind"] == "page" and not a["services"]:
        return "port", "read (aggregate/list)"
    return "port", ""


def fmt_set(s):
    return "<br>".join(sorted(short(x) for x in s)) or "—"


def route_table():
    out = [
        "| ID | Method | URI | Route name | Controller@action | Services / jobs | ViewHelpers | Vue page | Kind | Disposition | Status |",
        "|---|---|---|---|---|---|---|---|---|---|---|",
    ]
    for i, r in enumerate(routes, 1):
        a = analyses[(r["controller"], r["action"])]
        disp, why = disposition(r, a)
        page = a["page"] or ""
        if not page and r["name"]:
            page = "<br>".join("(" + p + ")" for p in sorted(pages_for_route(r["name"])))
        direct = sorted(vue_refs.get(r["name"], ())) if r["name"] else []
        if direct:
            page = "<br>".join(filter(None, [page] + ["`" + d + "`" for d in direct]))
        ctrl = short(r["controller"]) + ("@" + r["action"] if r["action"] and r["action"] != "__invoke" else "")
        mw = [m for m in r["middleware"] if m.startswith("can:")]
        uri = r["uri"].replace("|", "\\|")
        status = "todo" if disp == "port" else "todo (remove)"
        out.append(
            f"| R{i:03d} | {'/'.join(r['methods'])} | `{uri}` | `{r['name'] or '—'}` | {ctrl}{'<br>' + ', '.join(mw) if mw else ''} | "
            f"{fmt_set(a['services'] | a['jobs'])} | {fmt_set(a['vhs'])} | {page or '—'} | {a['kind'] or '—'} | {disp}{(': ' + why) if why else ''} | {status} |"
        )
    return out


# ---------------------------------------------------------------- non-route classes
test_files = {}
for d, _, fs in os.walk(os.path.join(ROOT, "tests")):
    for f in fs:
        if f.endswith("Test.php"):
            test_files.setdefault(f[:-8], []).append(rel(os.path.join(d, f)))

REMOVE_PATTERNS = [
    (r"Socialite|UserToken", "OAuth/Socialite removed"),
    (r"Webauthn", "WebAuthn removed"),
    (r"Telegram", "Telegram removed"),
    (r"Uploadcare", "Uploadcare removed (local storage)"),
    (r"GetGPSCoordinate|ContactModuleAddressImage", "Mapbox/LocationIQ removed"),
    (r"Scout|Typesense|Meilisearch", "Meilisearch/Typesense removed (DB search kept)"),
    (r"Telescope", "Telescope removed"),
    (r"SetupDocumentation", "Scribe removed"),
    (r"ZiggyVersionCheck|HandleInertiaRequests|TwoFactorChallengeView|UserProfile$", "Inertia/Ziggy removed (SPA)"),
    (r"VaultLifeEventController", "no route points at it (§11 F5)"),
]

# Classes that are ported but lose the parts that belong to removed features.
PARTIAL = {
    "MapHelper": "drop `getStaticImage` (Mapbox); keep `getMapLink`, `getAddressAsString`",
    "CreateAddress": "drop the `GetGPSCoordinate` dispatch (LocationIQ)",
    "UpdateAddress": "drop the `GetGPSCoordinate` dispatch (LocationIQ)",
    "UploadFile": "store the upload locally instead of recording an Uploadcare uuid/CDN URL",
    "DeleteFileInStorage": "delete from the local data directory instead of Uploadcare",
    "StorageHelper": "count local file sizes",
    "ProcessScheduledContactReminders": "drop the Telegram channel",
    "ReminderTriggered": "drop the Telegram channel",
    "UserNotificationChannel": "drop `TYPE_TELEGRAM`",
    "TestReminders": "drop Telegram",
    "User": "drop `webauthnKeys`, `userTokens` relations",
    "Contact": "drop Scout `Searchable` engine config; keep DB search columns",
    "Group": "drop Scout `Searchable` engine config; keep DB search columns",
    "Loan": "drop Scout `Searchable` engine config; keep DB search columns",
    "Note": "drop Scout `Searchable` engine config; keep DB search columns",
    "Vault": "drop Scout-related bits",
    "RedirectIfTwoFactorAuthenticatable": "drop the WebAuthn branch",
    "SetupApplication": "drop Scout setup",
    "SetupDummyAccount": "drop Scout/Uploadcare bits",
    "AppServiceProvider": "drop Socialite, Pulse, Telescope wiring",
}


def kind_of(fq, path):
    p = rel(path)
    for k, pat in [
        ("controller", "/Controllers/"), ("viewhelper", "/ViewHelpers/"), ("service", "/Services/"),
        ("job", "/Jobs/"), ("command", "/Console/"), ("model", "app/Models/"), ("helper", "app/Helpers/"),
        ("action", "app/Actions/"), ("listener", "app/Listeners/"), ("notification", "app/Notifications/"),
        ("mail", "app/Mail/"), ("middleware", "/Middleware/"), ("provider", "app/Providers/"),
        ("dav", "/Dav/"), ("dav", "/DAVACL/"), ("dav", "/Backend/"), ("exception", "app/Exceptions/"),
        ("resource", "/Resources/"), ("request", "/Requests/"), ("trait", "/Traits/"), ("logging", "app/Logging/"),
    ]:
        if pat in p:
            return k
    return "other"


def callers_of(fq):
    s = short(fq)
    out = set()
    for other, (p, src) in class_src.items():
        if other == fq:
            continue
        if re.search(r"^use\s+" + re.escape(fq) + r";", src, re.M) or (other.rsplit("\\", 1)[0] == fq.rsplit("\\", 1)[0] and re.search(r"\b" + s + r"\b", src)):
            out.add(other)
    return out


route_controllers = {r["controller"] for r in routes}


def class_rows():
    rows = []
    for fq, (p, src) in sorted(class_src.items(), key=lambda kv: rel(kv[1][0])):
        k = kind_of(fq, p)
        if fq in route_controllers or k == "viewhelper":
            continue
        disp = "port"
        for pat, why in REMOVE_PATTERNS:
            if re.search(pat, fq):
                disp = "remove: " + why
                break
        if disp == "port" and short(fq) in PARTIAL:
            disp = "port, " + PARTIAL[short(fq)]
        tests = test_files.get(short(fq), [])
        callers = callers_of(fq)
        rows.append((k, fq, rel(p), tests, callers, disp))
    return rows


def class_table():
    out = ["| ID | Kind | Class | PHP tests | Used by | Disposition | Status |", "|---|---|---|---|---|---|---|"]
    for i, (k, fq, p, tests, callers, disp) in enumerate(class_rows(), 1):
        used = ", ".join(sorted(short(c) for c in callers))
        if len(used) > 160:
            used = used[:157] + "…"
        status = "todo (remove)" if disp.startswith("remove") else "todo"
        out.append(f"| C{i:03d} | {k} | `{p}` | {'<br>'.join(t.replace('tests/', '') for t in tests) or '—'} | {used or '—'} | {disp} | {status} |")
    return out


def write(path):
    """Replace the generated blocks in the inventory, keeping each row's Status cell."""
    doc = read(path)
    for key, rows in (("routes", route_table()), ("classes", class_table())):
        begin, end = f"<!-- BEGIN GENERATED {key} -->", f"<!-- END GENERATED {key} -->"
        i, j = doc.index(begin) + len(begin), doc.index(end)
        old = {l.split("|")[1].strip(): l.rstrip("|").split("|")[-1].strip() for l in doc[i:j].strip().splitlines()[2:]}
        new = []
        for l in rows:
            rid = l.split("|")[1].strip()
            if rid in old and old[rid] and not old[rid].startswith("todo"):
                l = l[: l.rstrip("|").rstrip().rfind("|")] + f"| {old[rid]} |"
            new.append(l)
        doc = doc[:i] + "\n" + "\n".join(new) + "\n" + doc[j:]
    with open(path, "w", encoding="utf-8") as f:
        f.write(doc)


if __name__ == "__main__":
    if len(sys.argv) > 2 and sys.argv[1] == "--write":
        write(sys.argv[2])
    elif len(sys.argv) > 1 and sys.argv[1] == "--classes":
        print("\n".join(class_table()))
    elif len(sys.argv) > 1 and sys.argv[1] == "--json":
        out = []
        for i, r in enumerate(routes, 1):
            a = analyses[(r["controller"], r["action"])]
            out.append({"id": f"R{i:03d}", **r, "services": sorted(a["services"]), "jobs": sorted(a["jobs"]),
                        "viewhelpers": sorted(a["vhs"]), "page": a["page"],
                        "pages_via_urls": sorted(pages_for_route(r["name"])) if r["name"] else [],
                        "vue_direct": sorted(vue_refs.get(r["name"], ())) if r["name"] else [],
                        "kind": a["kind"], "disposition": " ".join(disposition(r, a)).strip(), "controller_file": a["file"]})
        json.dump(out, sys.stdout, indent=1)
    else:
        print("\n".join(route_table()))
