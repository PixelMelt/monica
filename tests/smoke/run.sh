#!/usr/bin/env bash
# End-to-end smoke test: builds the SPA and the server, starts an SMTP sink
# and the server on a fresh SQLite data directory, drives the SPA in
# headless Chrome (smoke.mjs), and stops everything. Output (screenshots,
# received mail, logs, the data directory) goes to tests/smoke/output/.
#
# Environment: SMOKE_PORT (8095), SMOKE_SMTP_PORT (8094), SMOKE_SKIP_BUILD=1
# to reuse the existing builds, MONICA_BIN to use another server binary.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="$here/output"
port="${SMOKE_PORT:-8095}"
smtp_port="${SMOKE_SMTP_PORT:-8094}"
smtp_user="smoke"
smtp_password="smoke-sink-password"

for p in "$port" "$smtp_port"; do
  if (exec 3<>"/dev/tcp/127.0.0.1/$p") 2>/dev/null; then echo "Port $p is in use."; exit 1; fi
done

rm -rf "$out"
mkdir -p "$out/data" "$out/mail" "$out/screenshots"

pids=()
cleanup() {
  for pid in "${pids[@]}"; do kill "$pid" 2>/dev/null || true; done
  wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# Builds.
bin="${MONICA_BIN:-${CARGO_TARGET_DIR:-$root/rust/target}/debug/monica}"
if [ "${SMOKE_SKIP_BUILD:-}" != 1 ]; then
  echo "Building the SPA…"
  (cd "$root" && corepack yarn build) >"$out/build-spa.log" 2>&1 || { tail -30 "$out/build-spa.log"; exit 1; }
  if [ -z "${MONICA_BIN:-}" ]; then
    echo "Building the server…"
    cargo build --manifest-path "$root/rust/Cargo.toml" --bin monica >"$out/build-server.log" 2>&1 \
      || { tail -30 "$out/build-server.log"; exit 1; }
  fi
fi

# The SMTP sink, in its own venv.
if [ ! -x "$here/.venv/bin/python" ] || ! "$here/.venv/bin/python" -c 'import aiosmtpd' 2>/dev/null; then
  echo "Creating the Python venv…"
  python3 -m venv "$here/.venv"
  "$here/.venv/bin/pip" install --quiet --disable-pip-version-check aiosmtpd==1.4.6
fi
"$here/.venv/bin/python" -I -W ignore "$here/sink.py" 127.0.0.1 "$smtp_port" "$smtp_user" "$smtp_password" "$out/mail" \
  >"$out/sink.log" 2>&1 &
pids+=($!)
for _ in $(seq 1 40); do grep -q "SMTP sink on" "$out/sink.log" && break; sleep 0.25; done
grep -q "SMTP sink on" "$out/sink.log" || { echo "The SMTP sink did not start:"; cat "$out/sink.log"; exit 1; }

# The server: production mode, fresh SQLite database, real SMTP with
# credentials (so verification emails are sent, not skipped: E005), no PHP.
export APP_ENV=production
export APP_URL="http://127.0.0.1:$port"
export BIND="127.0.0.1:$port"
export DATA_DIR="$out/data"
export DATABASE_URL="sqlite://$out/data/monica.sqlite?mode=rwc"
export SPA_DIR="$root/dist"
export MAIL_MAILER=smtp MAIL_HOST=127.0.0.1 MAIL_PORT="$smtp_port" MAIL_ENCRYPTION=
export MAIL_USERNAME="$smtp_user" MAIL_PASSWORD="$smtp_password"
export MAIL_FROM_ADDRESS=monica@example.com MAIL_FROM_NAME=Monica
export WIKIPEDIA_URL=
APP_KEY="$("$bin" key-generate)"
export APP_KEY
(cd "$out" && exec "$bin" serve) >"$out/server.log" 2>&1 &
pids+=($!)

for _ in $(seq 1 60); do
  curl -sf "http://127.0.0.1:$port/up" >/dev/null && break
  sleep 0.5
done
curl -sf "http://127.0.0.1:$port/up" >/dev/null || { echo "The server did not start:"; tail -30 "$out/server.log"; exit 1; }

# The browser test. `monica test-reminders` runs against the same database
# (it refuses to run in production, hence APP_ENV=local for it only).
BASE="http://127.0.0.1:$port" OUT="$out" MAIL_DIR="$out/mail" MONICA_BIN="$bin" \
  node "$here/smoke.mjs"
