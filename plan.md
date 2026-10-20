# Monica Rust rewrite

## Goal

Replace Monica's Laravel backend with a Rust server that exposes one documented
HTTP API, and move the existing Vue frontend onto that API. Work happens in this
repo. No external services are required except SMTP.

## Starting point

- Laravel 12 + Inertia + Vue 3. The web UI gets its data from 103 ViewHelpers
  via ~350 routes in `routes/web.php`. `routes/api.php` only covers users and
  vaults.
- ViewHelpers return page-shaped data with server-built URLs (`route(...)`) and
  user-formatted dates, e.g.
  `app/Domains/Contact/ManageNotes/Web/ViewHelpers/ModuleNotesViewHelper.php`.
  The new API must not copy that shape.
- Business logic: 261 service classes under `app/Domains`, 69 models,
  74 migrations, 507 test files.
- CI tests against SQLite, MySQL and Postgres.
- There are no users or production data, so no data migration or
  Laravel compatibility (password hashes, `APP_KEY` encryption, Sanctum tokens)
  is needed.

## End state

1. **Server.** A Rust server in `rust/` (axum + tokio + SeaORM) that implements
   every action the current app can perform, minus the removed features below.
2. **API.** All actions are exposed under `/api/v1` as JSON, described by an
   OpenAPI 3.1 spec generated from the Rust code (utoipa). The web UI and a
   future mobile app use the same endpoints. No UI-only endpoints. Responses
   carry raw data (ids, ISO 8601 dates, enums), never server-built URLs or
   user-formatted strings.
3. **Auth.** Cookie session + CSRF for the browser, bearer tokens for other
   clients, on the same routes. Vault permissions enforced server-side on every
   endpoint. Email/password (argon2id), TOTP 2FA, email verification, password
   reset.
4. **Frontend.** The Vue app becomes a standalone Vite SPA that calls the API
   only through a typed client generated from the spec. Inertia, Ziggy and SSR
   are removed. No visual or behavioral change to any page; only the data layer
   of the components changes. Exceptions: the upload widget (item 7) and
   removing UI for removed features (item 8).
5. **Database.** The current schema (`database/migrations`) is the data model,
   reproduced as Rust migrations for SQLite, MySQL/MariaDB and Postgres. Tables
   used only by removed features (`webauthn_keys`, `user_tokens`) are left out.
   No other schema changes without asking.
6. **Feature parity**, including:
   - scheduler and jobs: reminders every minute, DAV sync hourly, daily
     cleanups (see `routes/console.php`)
   - email notifications
   - all 29 languages from `lang/`
   - CardDAV/CalDAV server and client
   - database full-text search
   - MultiAvatar default avatars (`app/Models/MultiAvatar.php`)
   - country, ISO-639 language and currency data
7. **Local file storage** (replaces Uploadcare):
   - multipart upload to the API, stored under a configurable data directory
   - served through an endpoint that checks vault permissions
   - server-side thumbnails (300x300 crop for photos, plus avatars), replacing
     the Uploadcare CDN transforms
   - account storage limit enforced
   - a local upload component replaces `resources/js/Components/Uploadcare.vue`
8. **Removed**, including their UI entry points:
   - OAuth/Socialite login, WebAuthn, Telegram notifications
   - Uploadcare, Mapbox static maps, LocationIQ geocoding
   - Meilisearch/Typesense, Redis/memcached
   - the customers.monicahq.com reference in the delete-account dialog
   - dev/monitoring tools: Pulse, Nightwatch, Telescope, Debugbar, Scribe,
     Sentry

## Phases

Stop for approval at each ★.

- [ ] **0. Inventory.** One table mapping every web route → controller →
  service → ViewHelper → Vue page, plus every job, command and integration,
  each marked port or remove. This is the parity checklist for the project.
- [ ] **1. API design.** ★ Claude drafts the OpenAPI spec from the inventory:
  resources, pagination, error format, permission model, file upload/serving,
  and which heavy pages (e.g. the contact page) get an aggregate endpoint.
  Astra reviews it once for gaps against the inventory and for
  inconsistencies. Claude triages; no review loops. The user approves the spec.
- [ ] **2. Skeleton.** ★ Config, DB layer and migrations on all 3 engines,
  auth, accounts, users, vaults, permissions, error handling, test harness.
- [ ] **3. Domains.** One module per commit: port each service and its PHP
  tests. Astra may port backend modules under the CLAUDE.md porting rules, in
  its own worktree; Claude reviews the diff and commits.
- [ ] **4. Frontend.** Migrate page by page from Inertia props to the API
  client, and remove the UI for removed features. Claude only; Astra never
  touches frontend.
- [ ] **5. Integrations.** ★ Scheduler/jobs, email, search, file storage, DAV.

## Done when

- Every inventory row is marked done (with a test) or removed.
- `cargo test` passes on SQLite, MySQL and Postgres.
- `cargo clippy -- -D warnings` and `cargo fmt --check` pass.
- `yarn lint` and `yarn build` pass; the frontend makes no request outside the
  generated client; no code for removed features is left in the frontend.
- On a fresh install with PHP not running: sign up, verify email, enable 2FA,
  create a vault and contacts, use every contact module, upload/view/delete a
  photo and a document, receive a reminder email, and sync contacts over
  CardDAV with a real client. Checked by a browser smoke test of every
  top-level page.

## Rules

- Agents allowed (Claude subagents; Astra per CLAUDE.md).
- Git: work on branch `rust-rewrite` in this repo (Astra in per-task
  worktrees). Commit freely there. Never commit to `main`. Never push.
- Ask before cutting any further feature or changing the DB schema.
- License stays AGPL-3.0.
