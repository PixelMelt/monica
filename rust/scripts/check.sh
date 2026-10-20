#!/usr/bin/env bash
# Runs what plan.md's "Done when" asks of the server: tests on SQLite, MySQL
# and Postgres, clippy with warnings as errors, and rustfmt. MySQL and
# Postgres need the test containers from README.md.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --check
cargo clippy --all-targets -- -D warnings
for engine in sqlite mysql postgres; do
    echo "== cargo test on $engine"
    MONICA_TEST_DB=$engine cargo test --quiet
done
