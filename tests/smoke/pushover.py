"""Fake Pushover API for the smoke test (https://pushover.net/api).

Answers POST /1/users/validate.json and POST /1/messages.json like Pushover:
any application token and user key are valid except a user key containing
"invalid". Each message is written to <dir>/<n>.json (its form fields).

Usage: python pushover.py HOST PORT DIR
"""

import itertools
import json
import pathlib
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs

host, port, out = sys.argv[1:4]
out = pathlib.Path(out)
out.mkdir(parents=True, exist_ok=True)
counter = itertools.count(1)


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        form = {k: v[0] for k, v in parse_qs(self.rfile.read(length).decode()).items()}
        if self.path not in ("/1/messages.json", "/1/users/validate.json"):
            return self.answer(404, {"status": 0, "errors": ["not found"]})
        if not form.get("token"):
            return self.answer(400, {"token": "invalid", "errors": ["application token is invalid"], "status": 0})
        if "invalid" in form.get("user", "invalid"):
            return self.answer(400, {"user": "invalid", "errors": ["user key is invalid"], "status": 0})
        if self.path == "/1/messages.json":
            path = out / f"{next(counter):03}.json"
            path.with_suffix(".tmp").write_text(json.dumps(form))
            path.with_suffix(".tmp").rename(path)
            print(f"message {path.name}: {form.get('title', '')} / {form.get('message', '')}", flush=True)
        self.answer(200, {"status": 1, "request": "smoke"})

    def answer(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, *args):
        pass


server = ThreadingHTTPServer((host, int(port)), Handler)
print(f"Fake Pushover on {host}:{port}, writing to {out}", flush=True)
server.serve_forever()
