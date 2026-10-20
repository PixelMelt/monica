# Browser smoke test

`tests/smoke/run.sh` checks the rewrite end to end, the way a user would use
it: it builds the SPA (`corepack yarn build`) and the Rust server, starts a
fake Pushover API and the server on a fresh SQLite data directory
(production mode, nothing of the Laravel app or PHP), and drives the SPA in
headless Chrome (`smoke.mjs`, `puppeteer-core` with `/usr/bin/google-chrome`).

Steps:

1. First run: any page leads to the setup page, which creates the first user
   (username and password); log out, check sign-up is closed, log in.
2. Enable two-factor authentication on the profile page (TOTP codes computed
   in Node), log out, log in again through the 2FA challenge.
3. Create a vault and two contacts.
4. Add, edit and delete an item of every module of the default contact
   template (and quick facts, mood, posts), checking each change after a
   reload.
5. Upload a photo and a document (generated PNG and PDF): thumbnail and full
   image load, the download has the uploaded bytes, both are deleted; upload
   and remove an avatar.
6. Add a Pushover channel in Settings → Notifications and send a test, then
   create a reminder for today, run `monica test-reminders` against the
   server's database, and wait for the reminder at the fake Pushover.
7. Visit every page of `resources/js/router/routes.js` a logged-in user
   reaches (following links, then the ids created above).
8. CardDAV/CalDAV with a real client: create an API token with `write` on
   the API tokens page, let `vdirsyncer` discover and pull the vault's
   address book and its dates and tasks calendars, then push a new card, an
   edit and a deletion, each checked in the UI.
9. Instance administration: add an account on the admin dashboard, then
   delete it.

Every step fails on a console error, a page error, a failed request or an
HTTP response ≥ 400 that the step doesn't expect; pages also fail when blank
or an error page.

The fake Pushover (`pushover.py`, standard library only) accepts any token
and user key except a user key containing `invalid`, and writes each message
to `output/pushover/`. `vdirsyncer` is installed in `tests/smoke/.venv` by
`run.sh`.

Output, in `tests/smoke/output/` (ignored by git): `screenshots/` (one per
step; `modules/`: each module after a change; `pages/`: step 7),
`pushover/`, `downloads/`, `server.log`, `pushover.log`, `results.json`.

Options: `SMOKE_PORT` (8095), `SMOKE_PUSHOVER_PORT` (8094), `SMOKE_SKIP_BUILD=1`,
`MONICA_BIN` (another server binary). For debugging: `SMOKE_STEPS=4,5` runs
those steps (after 1 and 3, which they need), `SMOKE_MODULES=notes,pets`
limits step 4, `SMOKE_TRACE=1` prints the API requests that change data and
`SMOKE_DEBUG=1` the stack of a failure.
