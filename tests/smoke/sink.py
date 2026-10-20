"""SMTP sink for the smoke test.

Accepts mail only from an authenticated client (AUTH PLAIN/LOGIN without
TLS, one user and password), and writes each message to <dir>/<n>.eml.

Usage: python sink.py HOST PORT USER PASSWORD DIR
"""

import itertools
import pathlib
import signal
import sys

from aiosmtpd.controller import Controller
from aiosmtpd.smtp import AuthResult, LoginPassword

host, port, user, password, out = sys.argv[1:6]
user, password = user.encode(), password.encode()
out = pathlib.Path(out)
out.mkdir(parents=True, exist_ok=True)
counter = itertools.count(1)


class Handler:
    async def handle_DATA(self, server, session, envelope):
        path = out / f"{next(counter):03}.eml"
        tmp = path.with_suffix(".tmp")
        tmp.write_bytes(envelope.original_content or envelope.content)
        tmp.rename(path)
        print(f"received {path.name} for {', '.join(envelope.rcpt_tos)}", flush=True)
        return "250 Message accepted"


def authenticator(server, session, envelope, mechanism, auth_data):
    ok = isinstance(auth_data, LoginPassword) and auth_data.login == user and auth_data.password == password
    return AuthResult(success=ok)


controller = Controller(
    Handler(),
    hostname=host,
    port=int(port),
    authenticator=authenticator,
    auth_required=True,
    auth_require_tls=False,
)
controller.start()
print(f"SMTP sink on {host}:{port}, writing to {out}", flush=True)
signal.signal(signal.SIGTERM, lambda *_: (controller.stop(), sys.exit(0)))
signal.pause()
