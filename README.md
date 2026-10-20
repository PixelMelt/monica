# Monica

Monica is a personal relationship manager: a private place to document your
life and keep track of the people you care about, their important dates,
your notes, calls, tasks and loans, and to be reminded of what matters.

This version is a single-user install of Monica with its backend rewritten in
Rust (`rust/`) and the Vue frontend served as a standalone app:

- The first visit opens a setup page that creates your account; you then
  sign in with a username and password (two-factor authentication
  available). There is no email.
- Reminders are sent to your phone through [Pushover](https://pushover.net).
- Contacts and calendars sync with any CardDAV/CalDAV client.
- Data lives in SQLite (default), MySQL or Postgres, and uploaded files on
  disk.
- An instance administrator (the first user) can create other accounts on
  the admin dashboard; each account has its own users, vaults and settings.

## Features

- Contacts with relationships, addresses, ways to contact them, important
  dates, labels, groups, pets, goals, life events, loans, tasks, calls and
  notes
- Reminders, including automatic birthday reminders
- A journal with posts, slices of life and mood tracking
- Photos and documents
- Several vaults, shared with the users of your account
- Customisable contact page templates, genders, pronouns, relationship types
  and more
- 29 languages

## Run it

With Docker:

```
yarn docker:build
yarn docker:run
```

Then open http://localhost:8080. From source, configuration and the
`monica` commands (such as `monica reset-password <username>`): see
[rust/README.md](rust/README.md).

## Develop

- `cargo test --manifest-path rust/Cargo.toml` runs the server's tests
  (SQLite by default; MySQL and Postgres as described in `rust/README.md`).
- `yarn dev` serves the frontend with hot reload against a running server
  (`MONICA_SERVER_URL`, default `http://localhost:8000`); `yarn lint` and
  `yarn build` check and build it.
- `tests/smoke/run.sh` drives the whole app in a headless browser.
- The API is documented in `resources/js/api/openapi.json`, generated from
  the server (`yarn api:generate` also regenerates the frontend's client).

## Credits and license

Based on [Monica](https://github.com/monicahq/monica) by Regis Freyd, Alexis
Saettler and the Monica contributors. Licensed under the GNU Affero General
Public License v3.0; see [LICENSE.md](LICENSE.md).
