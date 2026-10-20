# Browser smoke test

`tests/smoke/run.sh` checks the rewrite end to end, the way a user would use
it: it builds the SPA (`corepack yarn build`) and the Rust server, starts an
SMTP sink and the server on a fresh SQLite data directory (production mode,
nothing of the Laravel app or PHP), and drives the SPA in headless Chrome
(`smoke.mjs`, `puppeteer-core` with `/usr/bin/google-chrome`).

Steps:

1. Sign up, follow the link of the verification email received by the sink.
2. Enable two-factor authentication on the profile page (TOTP codes computed
   in Node), log out, log in again through the 2FA challenge.
3. Create a vault and two contacts.
4. Add, edit and delete an item of every module of the default contact
   template (and quick facts, mood, posts), checking each change after a
   reload.
5. Upload a photo and a document (generated PNG and PDF): thumbnail and full
   image load, the download has the uploaded bytes, both are deleted; upload
   and remove an avatar.
6. Create a reminder for today, run `monica test-reminders` against the
   server's database, and wait for the reminder email at the sink.
7. Visit every page of `resources/js/router/routes.js` a logged-in user
   reaches (following links, then the ids created above).

Every step fails on a console error, a page error, a failed request or an
HTTP response ≥ 400 that the step doesn't expect; pages also fail when blank
or an error page.

The SMTP sink (`sink.py`, `aiosmtpd` in `tests/smoke/.venv`, created by
`run.sh`) requires AUTH, so the server sends the verification email instead of
marking the user verified (E005). Messages are written to `output/mail/`.

Output, in `tests/smoke/output/` (ignored by git): `screenshots/` (one per
step; `modules/`: each module after a change; `pages/`: step 7), `mail/`,
`downloads/`, `server.log`, `sink.log`, `results.json`.

Options: `SMOKE_PORT` (8095), `SMOKE_SMTP_PORT` (8094), `SMOKE_SKIP_BUILD=1`,
`MONICA_BIN` (another server binary). For debugging: `SMOKE_STEPS=4,5` runs
those steps (after 1 and 3, which they need), `SMOKE_MODULES=notes,pets`
limits step 4, `SMOKE_TRACE=1` prints the API requests that change data and
`SMOKE_DEBUG=1` the stack of a failure.
