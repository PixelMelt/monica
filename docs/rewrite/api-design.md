# Monica API v1: design

Status: draft for the phase 1 ★ approval. The spec itself is
`docs/rewrite/openapi.yaml`, assembled from `docs/rewrite/api/*.yaml` by
`docs/rewrite/tools/spec.py`. Once the Rust server exists, utoipa generates the
spec from the code and this draft becomes the target the generated spec is
checked against.

Inventory IDs (`R…`, `V…`, `X…`, see `inventory.md`) are cited where a rule
comes from current behaviour.

## 1. Scope

- One HTTP API under `/api/v1`, JSON in and out, used by the web SPA and any
  other client. No endpoint exists only for the web UI.
- Outside `/api/v1` the server also serves: the SPA (`/`, any non-API path
  falls back to `index.html`), `/dav` (CardDAV/CalDAV, §11), `/.well-known/carddav`
  and `/.well-known/caldav` (301 to `/dav`), `/.well-known/security.txt` (301
  to `/security.txt`) and `/up` (health).
- Responses carry raw data: ids, ISO 8601 dates, numbers, enums, user-entered
  text. They never carry server-built URLs or text formatted for a user
  (formatted dates, "3 days ago", formatted names, money strings, translated
  labels). The client formats.

## 2. Resources and paths

- Paths are nouns, plural, kebab-case: `/vaults/{vault_id}/life-event-categories`.
- Ownership is expressed by nesting, at most two levels below the vault:
  - account level: `/account/...` (account settings; reads open to every
    account member, writes need account administrator),
  - current user: `/me/...`,
  - vault level: `/vaults/{vault_id}/...`,
  - contact level: `/vaults/{vault_id}/contacts/{contact_id}/...`,
  - journal level: `/vaults/{vault_id}/journals/{journal_id}/...` and
    `/vaults/{vault_id}/journals/{journal_id}/posts/{post_id}/...`.
- Path parameters are named `{<resource>_id}`. Ids keep their database type:
  UUID strings for accounts, users, vaults, contacts and address book
  subscriptions; 64-bit integers for everything else.
- The server checks every id in the path against its parent (contact belongs to
  vault, post belongs to journal, …). A mismatch is a 404.

## 3. Methods

| Method | Use | Success |
|---|---|---|
| `GET` collection | list | 200, list envelope (§5) |
| `GET` item | read | 200, the object |
| `POST` collection | create | 201, the created object |
| `PATCH` item | partial update (JSON merge: absent = unchanged, `null` = clear a nullable field) | 200, the updated object |
| `PUT` item | set a sub-resource that has no id of its own (e.g. a contact's job, a label assignment) | 200 with the object, or 204 |
| `DELETE` item | delete | 204 |
| `POST /…/{id}/<verb>` | an action that isn't a field change (move a contact, send a test notification, regenerate recovery codes) | 200 or 202 with a body, or 204 |

Rules that follow from this:

- Toggle endpoints become explicit state. `PUT contacts/{id}/toggle-favorite`
  (R052) becomes `PATCH contacts/{id}` with `{"is_favorite": true}`. The server
  never flips a value the client didn't send.
- Reordering becomes `PATCH` of `position` on the item. The server shifts the
  siblings, as the `Update…Position` services do today.
- A controller action that chained two services for one form (create a label
  and assign it, R074) becomes two calls. Exceptions are marked
  `x-composite: true` where one call is needed so nothing is left half-made:
  `createContactAddress` (an address can't exist without a contact),
  `setContactAvatar` (upload + set), `createContactRelationship` (may create
  the other contact), `createContactTimelineEvent` (a timeline event never
  exists without a life event), `createJournalPostTag` (create a tag while
  writing a post, as an editor).
- GET never has side effects. View counters that today increment on page
  load become explicit actions the SPA calls when it opens the page:
  `recordContactView` (most consulted contacts, X014) and
  `recordJournalPostView`.
- One sub-resource is keyed by a date instead of an id: a goal's streak days,
  `PUT`/`DELETE …/goals/{goal_id}/streaks/{date}`, which return the updated
  goal so the module can redraw its stats.

## 4. Representation

- JSON fields are `snake_case` and keep the column name where there is one.
- Timestamps are RFC 3339 in UTC with a `Z` suffix:
  `"2025-08-09T22:32:20Z"`. Date-only values are `"YYYY-MM-DD"`.
  Partial dates keep their parts: `{"day": 3, "month": 5, "year": null}`
  (important dates, R061–R064).
- Enums are the string values stored in the database (`"birthdate"`,
  `"email"`, `"photo"`). Each enum is listed in the spec.
- Money is an integer in the currency's minor unit plus a currency id.
- Labels that the account seeds from a translation key keep both columns:
  `name` (user-entered, nullable) and `name_translation_key` (nullable). The
  client shows `name` if set, else translates the key. The server never
  translates in an API response.
- Related objects are embedded as small summaries (`ContactSummary`,
  `UserSummary`, `FileSummary`, `LabelSummary`, …) when the page needs them,
  and referenced by id otherwise.
- Values that come from domain logic are computed by the server and returned
  raw: a contact's age in years, the next occurrence of a reminder, a goal's
  streaks, calendar grids, report totals. Formatting them is the client's job.
- `ContactSummary` is `{id, vault_id, first_name, middle_name, last_name,
  nickname, maiden_name, prefix, suffix, avatar_file_id, listed}`; the client
  builds the display name from the user's `name_order`. Where the UI shows
  the contact hover card, the response embeds `ContactCard` (summary + age +
  groups).
- Avatars: a contact's avatar is always fetched from
  `GET /vaults/{vault_id}/contacts/{contact_id}/avatar`, which returns the
  uploaded avatar (300×300 crop) or the generated MultiAvatar SVG (X021). The
  SVG is no longer inlined in every response.
- Markdown fields (note bodies, post sections, goal and task descriptions…)
  are returned as stored. The client renders them with the same rules as
  `Str::markdownExternalLink` (X016).

## 5. Lists and pagination

Every collection response is an envelope:

```json
{
  "data": [ ... ],
  "meta": { "page": 1, "per_page": 15, "total": 42, "last_page": 3 }
}
```

- Paginated collections take `page` (1-based) and `per_page` (default per
  endpoint, matching today's page size; max 100) and always include `meta`.
- Small bounded collections (an account's genders, a vault's labels) are not
  paginated; `meta` is omitted.
- Filters are query parameters named after the field (`label_id`,
  `is_archived`, `type`). Sorting uses `sort`, with values listed per
  endpoint (`last_updated`, `-last_updated`, `first_name`, …). Where the user
  has a saved sort (contacts, R038), the endpoint uses it when `sort` is absent.
- Full-text search uses `q` (X019): `LIKE` matching on the columns the Scout
  `database` engine searches today.

## 6. Errors

Errors use RFC 9457 problem details, `Content-Type: application/problem+json`:

```json
{
  "type": "about:blank",
  "status": 422,
  "code": "validation_failed",
  "title": "The given data was invalid.",
  "errors": [
    { "field": "title", "code": "max", "message": "The title may not be greater than 255 characters.", "params": { "max": 255 } }
  ]
}
```

- `code` is stable and machine-readable; `title` and `message` are in the
  request locale (§8).
- `errors[].field` is a JSON path into the request body (`"emails[2].value"`).

| Status | `code` | When |
|---|---|---|
| 400 | `bad_request` | malformed JSON, wrong content type |
| 401 | `unauthenticated` | no valid session or token |
| 403 | `csrf_token_mismatch` | cookie-authenticated unsafe request without a valid CSRF header |
| 403 | `email_not_verified` | the user hasn't verified their email |
| 403 | `forbidden` | authenticated, can see the resource, lacks the permission (e.g. viewer trying to edit) |
| 403 | `token_ability_missing` | the bearer token lacks `write` |
| 403 | `password_confirmation_required` | a sensitive action without a recent password confirmation (§7.4) |
| 403 | `signup_disabled` | registration while signup is off (X007) |
| 404 | `not_found` | doesn't exist, or belongs to a vault/account the user can't see |
| 409 | `conflict` | uniqueness or state conflicts (`EntryAlreadyExistException`, `SameUserException`, `CantBeDeletedException`); deleting a row whose `can_be_deleted` is false |
| 413 | `payload_too_large` | upload above the per-file limit |
| 422 | `validation_failed` | field validation (X003) |
| 422 | `storage_limit_exceeded` | upload would pass the account storage limit (X011) |
| 422 | `vault_user_limit_reached` | `MaximumNumberOfUsersInVaultException` |
| 429 | `rate_limited` | rate limits (X005); `Retry-After` header set |
| 500 | `internal_error` | anything else; no details leaked |

A resource in a vault the user isn't a member of is a 404, not a 403, so ids
don't leak across accounts.

## 7. Authentication

### 7.1 Two credentials, same routes

Every `/api/v1` route accepts either:

- **Session cookie** (browser). `monica_session`, `HttpOnly`, `Secure` when
  served over HTTPS, `SameSite=Lax`. Sessions live in the `sessions` table.
  "Remember me" extends the cookie lifetime (5 years, as Laravel's remember
  cookie does today).
- **Bearer token** (`Authorization: Bearer <token>`). Personal access tokens
  in `personal_access_tokens`, with abilities `read` and `write` (X027).
  `read` allows `GET`/`HEAD`; anything else needs `write`. Tokens are shown
  once at creation and stored hashed (SHA-256).

If both are present, the bearer token wins and CSRF is not checked.
Creating or changing tokens needs a cookie session, so a token can't mint
tokens or widen its own abilities.

### 7.2 CSRF

Cookie-authenticated requests with an unsafe method (`POST`, `PUT`, `PATCH`,
`DELETE`) must send `X-XSRF-TOKEN` equal to the `XSRF-TOKEN` cookie (readable
by JS, `SameSite=Lax`). The server sets the cookie on any response that
doesn't carry one; `GET /api/v1/auth/csrf` exists to prime it before the
first login. This is the axios default, so the generated client needs no
extra code. Requests from other origins are not given CORS headers.

### 7.3 Flows

| Flow | Endpoint(s) |
|---|---|
| Register (X007 gate) | `POST /auth/register` → session cookie; verification email sent |
| Log in | `POST /auth/login` `{email, password, remember}` → `200 {two_factor: false, user}` with session, or `200 {two_factor: true}` with a pending-2FA session |
| 2FA challenge | `POST /auth/two-factor-challenge` `{code}` or `{recovery_code}` |
| Log out | `POST /auth/logout` |
| Verify email | link in the email opens the SPA at `/email/verify/{user_id}/{hash}?expires=&signature=`, which calls `POST /auth/email/verify` with those values |
| Resend verification | `POST /auth/email/verification-notification` |
| Forgot / reset password | `POST /auth/forgot-password` `{email}`; `POST /auth/reset-password` `{token, email, password, password_confirmation}` |
| Confirm password (§7.4) | `POST /auth/confirm-password`; `GET /auth/confirmed-password-status` |
| Accept invitation | `GET /invitations/{code}`; `POST /invitations/{code}/accept` |
| Instance info (public) | `GET /instance`: signup enabled, deletion notice, locales, version, feature flags |

Bearer tokens are created from a logged-in session (`POST /me/tokens`); there
is no password grant. A mobile app gets its first token by logging in with
the cookie flow and creating one.

Rate limits (X005): login 5/min per email+IP, 2FA 5/min per pending login,
email verification 6/min, everything else 60/min per user (or IP when
anonymous).

### 7.4 Sensitive actions

Enabling/disabling 2FA, viewing or regenerating recovery codes, logging out
other sessions, cancelling the account, deleting a user and deleting yourself
need a recent password confirmation, as Fortify does today for the first
four. The request passes if either:

- the cookie session confirmed the password in the last 3 hours
  (`POST /auth/confirm-password`), or
- the body carries `"password": "<current password>"` (the way bearer clients
  do it).

Otherwise the server answers 403 `password_confirmation_required`, which the
SPA's `ConfirmsPassword` component handles the same way it does today.

### 7.5 Passwords and 2FA

- argon2id for new hashes. Rules as X006.
- TOTP (RFC 6238, SHA-1, 6 digits, 30 s, ±1 step), the same as Fortify. The
  secret and recovery codes are encrypted at rest with a server key
  (`APP_KEY`-equivalent, new format; no Laravel compatibility needed).
  Enabling returns `secret`, `otpauth_url` and `qr_code_svg`; 2FA is active
  only after `POST /me/two-factor/confirm` with a valid code.

## 8. Localization

- The request locale is the user's `locale`; for anonymous requests it is
  negotiated from `Accept-Language` against the 29 supported locales, falling
  back to `en` (X009).
- Only error messages and emails are translated by the server. The SPA keeps
  shipping `lang/*.json`.
- `GET /instance` lists the supported locales; `GET /reference/…` returns
  countries, currencies and languages with names in the request locale (D001–D003).

## 9. Permissions

Each operation in the spec has an `x-permission`:

| Value | Meaning |
|---|---|
| `public` | no authentication |
| `user` | any authenticated user (verified email unless noted) |
| `account:member` | any user of the account that owns the resource |
| `account:admin` | `is_account_administrator` (gate `administrator`) |
| `vault:view` | vault permission ≤ 300 (any member) |
| `vault:edit` | vault permission ≤ 200 |
| `vault:manage` | vault permission ≤ 100 |

When one operation's fields need different permissions, `x-field-permissions`
maps field name to permission (only `updateVault`: any member may set
`default_activity_tab`, the rest needs `vault:manage`).

Today the route gates check membership (`vault-viewer`, X001) and the services
check edit/manage (`author_must_be_vault_editor`, X002). The API checks the
stricter of the two at the HTTP layer, and the Rust services keep their own
checks. Every read in a vault needs `vault:view`; every write needs
`vault:edit` unless the service asks for `vault:manage`.

## 10. Files (local storage)

Replaces Uploadcare (I005, X020).

- Upload is `multipart/form-data` with one `file` part, sent to the endpoint
  of the thing it belongs to:
  - `POST /vaults/{vault_id}/contacts/{contact_id}/photos`
  - `POST /vaults/{vault_id}/contacts/{contact_id}/documents`
  - `PUT /vaults/{vault_id}/contacts/{contact_id}/avatar`
  - `POST /vaults/{vault_id}/journals/{journal_id}/posts/{post_id}/photos`
  - `PUT /vaults/{vault_id}/journals/{journal_id}/slices/{slice_id}/cover`
- The server stores the bytes under `<DATA_DIR>/files/<vault_id>/<uuid>`,
  writes a `files` row (the existing columns: `uuid` is the storage key,
  `original_url`/`cdn_url` stay null, `name`, `mime_type`, `size`, `type`),
  and returns a `File` object `{id, uuid, name, mime_type, size, type,
  created_at}`.
- Limits: per-file `MAX_UPLOAD_SIZE` (default 10 MB) → 413; account total
  against `storage_limit_in_mb` (default 50, X011) → 422
  `storage_limit_exceeded`. Photos and avatars must be JPEG, PNG, GIF or WebP
  (HEIC rejected with 422).
- Serving: `GET /vaults/{vault_id}/files/{file_id}/content?variant=` with
  `variant` = `original` (default) or `thumbnail` (300×300 centre crop,
  generated on first request and cached on disk, JPEG or PNG). Needs
  `vault:view`. Sends `Content-Type`, `Content-Length`, `ETag`,
  `Cache-Control: private, max-age=31536000, immutable`, and
  `Content-Disposition: inline` for images, `attachment` for documents.
  Range requests are supported for originals.
- Contact avatars are also served by `GET …/contacts/{contact_id}/avatar`
  (§4).
- The UI showed Uploadcare crops at 75, 100, 200, 300 and 400 px squares,
  200×100 and 800×100 banners and a 1700 px wide view. The SPA uses the
  `thumbnail` variant for squares up to 300 px and `original` with CSS
  `object-fit: cover` for the rest, so the pages look the same.
- Responses that back an upload widget carry a computed `can_upload_file`
  (storage used < limit), as the pages' `canUploadFile` props do today:
  `ContactPage`, the contact photo and document lists and modules, `Post`,
  `SliceOfLifeDetail`. `GET /account/storage` gives the full usage.
- Deleting the `files` row deletes the bytes and cached variants (E009).
- The vault files page lists files with `GET /vaults/{vault_id}/files?type=`.

## 11. CardDAV / CalDAV

Not part of the OpenAPI spec. `/dav` keeps today's surface (V038): principal
`/dav/principals/{email}`, address books per vault, calendars per vault,
`PROPFIND`, `REPORT` (addressbook-multiget, addressbook-query,
calendar-multiget, sync-collection), `GET`/`PUT`/`DELETE` of `.vcf`/`.ics`,
VCF/ICS export, sync tokens. Authentication is what
`EnsureDavRequestsAreStateful` and `AuthenticateWithTokenOnBasicAuth` allow
today: a bearer personal access token, HTTP Basic with the user's email and a
personal access token as the password (the token needs both `read` and
`write`), or the browser session. The account password is not accepted. Address book subscriptions
(the DAV client, I003) are managed through the API under
`/vaults/{vault_id}/address-book-subscriptions` (today only the console
command K003 creates them).

## 12. Aggregate endpoints

A few pages show data from many resources at once. Fetching each resource
separately would cost 10–25 requests per page view, so these pages get one
read endpoint that returns what the page needs. They are reads only; every
write still goes to the resource endpoints.

| Endpoint | Page | Contents |
|---|---|---|
| `GET /me/bootstrap` | layout (every page) | current user + preferences, account flags, the user's vaults with permissions, the data `VaultIndexViewHelper::layoutData` builds today |
| `GET /vaults/{vault_id}/dashboard` | Vault/Dashboard/Index | default tab, favorites, last updated contacts, upcoming reminders, due tasks, mood tracking events, life metrics |
| `GET /vaults/{vault_id}/contacts/{contact_id}/page?template_page=` | Vault/Contact/Show | contact header (names, avatar, labels, quick facts, family summary, gender/pronoun, important dates, company, religion), the template's pages (tabs), and for the requested page, every module's data as a `oneOf` keyed by `module.type` |
| `GET /vaults/{vault_id}/journals/{journal_id}/overview?year=` | Vault/Journal/Show | posts grouped by month for a year, years with posts, slices of life, tags |
| `GET /vaults/{vault_id}/calendar/{year}/{month}` | Vault/Calendar/Index | month grid with important dates, mood events, posts per day |

Each aggregate's module/section shapes reuse the schemas of the matching
resource endpoints, so a client that prefers per-resource calls gets the same
objects.

## 13. Spec conventions (for `docs/rewrite/api/*.yaml`)

- `operationId`: `<verb><Resource>`, camelCase, unique: `listContactNotes`,
  `createContactNote`, `updateContactNote`, `deleteContactNote`,
  `moveContact`.
- One tag per area: `Auth`, `Me`, `Account`, `Vaults`, `Contacts`,
  `Contact modules`, `Groups`, `Journals`, `Reference`, `Files`.
- Every operation carries:
  - `x-permission` (§9),
  - `x-inventory`: the inventory rows it covers (`[R066]`), so coverage can be
    checked mechanically,
  - `x-services`: the PHP services it ports (`[CreateNote]`),
  - optionally `x-composite` (§3) and `x-field-permissions` (§9),
  - responses for success and every error status it can return, via the
    shared `components/responses`.
- Rows that are ported without an operation (SPA-only pages, redirects,
  `/dav`) are listed in `x-not-api` with the reason, in `00-base.yaml` or in
  the area's fragment. Operations with no inventory row (list endpoints a
  module reuses, the reference lists) have `x-inventory: []`.
- Schemas: `<Thing>` for the read model, `<Thing>Create` and `<Thing>Update`
  for bodies (`Update` has no required fields, matching PATCH). Validation
  limits from the PHP `rules()` become `maxLength`, `minimum`, `format`,
  `enum`.
