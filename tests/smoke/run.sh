#!/usr/bin/env bash
# End-to-end smoke test: builds the SPA and the server, starts a fake
# Pushover API and the server on a fresh SQLite data directory, drives the
# SPA in headless Chrome (smoke.mjs), and stops everything. Output
# (screenshots, received notifications, logs, the data directory) goes to
# tests/smoke/output/.
#
# Environment: SMOKE_PORT (8095), SMOKE_PUSHOVER_PORT (8094),
# SMOKE_SKIP_BUILD=1 to reuse the existing builds, MONICA_BIN to use another
# server binary.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/../.." && pwd)"
out="$here/output"
port="${SMOKE_PORT:-8095}"
pushover_port="${SMOKE_PUSHOVER_PORT:-8094}"

for p in "$port" "$pushover_port"; do
  if (exec 3<>"/dev/tcp/127.0.0.1/$p") 2>/dev/null; then echo "Port $p is in use."; exit 1; fi
done

rm -rf "$out"
mkdir -p "$out/data" "$out/pushover" "$out/screenshots"

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

# vdirsyncer (step 8), in its own venv.
if [ ! -x "$here/.venv/bin/vdirsyncer" ]; then
  echo "Creating the Python venv…"
  python3 -m venv "$here/.venv"
  "$here/.venv/bin/pip" install --quiet --disable-pip-version-check vdirsyncer==0.21.0
fi

# The fake Pushover API (standard library only).
python3 -I "$here/pushover.py" 127.0.0.1 "$pushover_port" "$out/pushover" >"$out/pushover.log" 2>&1 &
pids+=($!)
for _ in $(seq 1 40); do grep -q "Fake Pushover on" "$out/pushover.log" && break; sleep 0.25; done
grep -q "Fake Pushover on" "$out/pushover.log" || { echo "The fake Pushover did not start:"; cat "$out/pushover.log"; exit 1; }

# The server: production mode, fresh SQLite database, the fake Pushover, no PHP.
export APP_ENV=production
export APP_URL="http://127.0.0.1:$port"
export BIND="127.0.0.1:$port"
export DATA_DIR="$out/data"
export DATABASE_URL="sqlite://$out/data/monica.sqlite?mode=rwc"
export SPA_DIR="$root/dist"
export PUSHOVER_API_URL="http://127.0.0.1:$pushover_port/1"
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
BASE="http://127.0.0.1:$port" OUT="$out" PUSHOVER_DIR="$out/pushover" MONICA_BIN="$bin" \
  VDIRSYNCER="$here/.venv/bin/vdirsyncer" node "$here/smoke.mjs"
