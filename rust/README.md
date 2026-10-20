# Monica server (Rust)

The Rust rewrite of Monica's backend: one HTTP API under `/api/v1` (see
`docs/rewrite/api-design.md` and `docs/rewrite/openapi.yaml`), CardDAV/CalDAV
at `/dav`, and the SPA. Plan and status: `plan.md`, `docs/rewrite/inventory.md`.

## Run

```
export APP_KEY=$(cargo run -q -- key-generate)
cargo run              # = cargo run -- serve; migrates, then listens on BIND (0.0.0.0:8080)
```

This is a single-user install. The first visit opens the setup page, which
creates the account and its administrator (username and password); sign-up
is closed after that, and the administrator adds any other users. There is
no email: reminders go to Pushover (Settings → Notifications: your Pushover
user key and the token of a Pushover application you create at
pushover.net), and a forgotten password is reset on the server with
`monica reset-password <username>`, which prints a new one
(`--disable-two-factor` also turns 2FA off).

Configuration is read from the environment (or `.env`):

| Variable                    | Default                                     |                                                       |
| --------------------------- | ------------------------------------------- | ----------------------------------------------------- |
| `APP_ENV`                   | `production`                                | `production`, `local` or `testing`                    |
| `APP_URL`                   | `http://localhost:8080`                     | https makes cookies `Secure`                          |
| `APP_KEY`                   | (required)                                  | 32 bytes, base64; `monica key-generate`               |
| `DATABASE_URL`              | `sqlite://$DATA_DIR/monica.sqlite?mode=rwc` | or `mysql://…`, `postgres://…`                        |
| `DATA_DIR`                  | `data`                                      | uploaded files, default SQLite file                   |
| `SPA_DIR`                   | —                                           | built frontend served at `/`                          |
| `BIND`                      | `0.0.0.0:8080`                              |                                                       |
| `APP_LOCALE`                | `en`                                        |                                                       |
| `MAX_UPLOAD_SIZE`           | `10`                                        | MB per file                                           |
| `SESSION_LIFETIME`          | `120`                                       | minutes                                               |
| `SESSION_SECURE_COOKIE`     | https `APP_URL`                             |                                                       |
| `PUSHOVER_API_URL`          | `https://api.pushover.net/1`                | the Pushover API (tests point it at a fake)           |
| `WIKIPEDIA_URL`             | `https://{lang}.wikipedia.org/w/api.php`    | summaries on the city/country reports; empty disables |
| `APP_VERSION`, `APP_COMMIT` | from git at build time                      | shown in the footer                                   |

Other subcommands: `migrate`, `openapi` (prints the generated OpenAPI
document), `key-generate`, `reset-password <username> [--disable-two-factor]`,
`schema-dump --database-url …`, `test-reminders` (sends every scheduled
reminder now; not in production), `dummy [--migrate] [--force]` (wipes the
database and creates demo accounts `admin`/`admin123` and `blank`/`blank123`).

## Test

`cargo test` runs on SQLite. For MySQL and Postgres, start the throwaway
servers the harness expects, then select the engine with `MONICA_TEST_DB`:

```
docker run -d --name monica-test-mysql --tmpfs /var/lib/mysql:rw -e MYSQL_ROOT_PASSWORD=root \
  -p 127.0.0.1:33306:3306 mysql:8.4 --skip-log-bin --innodb-flush-log-at-trx-commit=0 --innodb-doublewrite=0 \
  --table-definition-cache=20000 --table-open-cache=20000 --max-connections=1000
docker run -d --name monica-test-pg --tmpfs /var/lib/postgresql/data:rw -e POSTGRES_PASSWORD=postgres \
  -p 127.0.0.1:55432:5432 postgres:17 -c max_locks_per_transaction=1024 -c fsync=off -c max_connections=500

MONICA_TEST_DB=mysql cargo test
MONICA_TEST_DB=postgres cargo test
```

`MONICA_TEST_MYSQL_URL` / `MONICA_TEST_POSTGRES_URL` override the server
URLs. Each test gets an empty migrated database: SQLite uses a temp file;
on MySQL and Postgres a finished test's database is emptied and reused by
the next test of the same process, and databases left by dead processes
are dropped.

Lint: `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`.

## Schema

`src/db/migrations` builds the schema of `database/migrations` (see the
module docs). `tests/schema_parity.rs` compares it with
`tests/fixtures/schema/*.json`, which are `monica schema-dump` of databases
migrated by Laravel itself. To regenerate them, run `php artisan migrate`
(e.g. in a `serversideup/php:8.4-cli` container with the repo mounted,
`SCOUT_DRIVER=database`) against each engine, dump them, and drop the tables
of removed features (`migrations`, `pulse_*`, `telescope_*`,
`webauthn_keys`, `user_tokens`).

Entities in `src/entities` were generated with `sea-orm-cli generate entity`
from such a database and adapted (see `src/entities/mod.rs`).

## Translations

`src/i18n.rs` embeds `lang/<locale>.json` and `locales/<locale>.json`; the
latter are generated from Laravel's PHP arrays with `php locales/generate.php`.
