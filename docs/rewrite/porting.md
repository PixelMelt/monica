# Porting a PHP domain to Rust

How a folder of `app/Domains/<Area>/<Domain>/Services` (and `Jobs`) becomes
`rust/src/domains/<area>/<domain>/`. The worked example is
`rust/src/domains/settings/manage_genders/` (from
`app/Domains/Settings/ManageGenders`); read it before porting anything.

## Sources

The PHP is the spec. Before writing a module, read in full:

- every service (and job) in the domain folder,
- every test file for them under `tests/Unit/Domains/<Area>/<Domain>/`,
- the factories those tests use (`database/factories/<Model>Factory.php`),
- the models they touch (`app/Models/<Model>.php`): constants, casts,
  relationships, `SoftDeletes`.

Don't fill gaps from general knowledge of Laravel or Monica; if the PHP
doesn't say it, the port doesn't do it.

## Layout

- `rust/src/domains/<area>/<snake_domain>/mod.rs`: one Rust struct per PHP
  service, named like the class, in the order of the PHP files (alphabetical).
  Module doc: `` //! `app/Domains/<Area>/<Domain>/Services`. ``
- `rust/src/domains/<area>/<snake_domain>/tests.rs`: the PHP tests, behind
  `#[cfg(test)] mod tests;` in `mod.rs`.
- Shared helpers stay private to the module unless another domain needs them.

## A service

```rust
/// `UpdateGender`.
pub struct UpdateGender {
    pub account_id: Uuid,
    pub author_id: Uuid,
    pub gender_id: Id,
    pub name: String,
}

impl UpdateGender {
    pub async fn execute<C: ConnectionTrait>(self, db: &C) -> ApiResult<genders::Model> { … }
}
```

- **Fields** are the keys of `rules()`, in the same order, same names.
  Types follow the rule:

  | PHP rule | Rust |
  |---|---|
  | `required\|uuid` | `Uuid` (`crate::db::uuid::Uuid`) |
  | `required\|integer` on an id | `Id` (`crate::db::id::Id`) |
  | `nullable\|integer\|exists:…` | `Option<Id>` |
  | other `required\|integer` | `i32` / `i64` like the column |
  | `required\|string` | `String` |
  | `nullable\|string` / `sometimes` | `Option<String>` |
  | `required\|boolean` / `nullable\|boolean` | `bool` / `Option<bool>` |
  | dates and times passed as strings (`date_format:Y-m-d`, `'09:00'`) | `String` / `Option<String>`, parsed in `execute` like the PHP does |
  | arrays | `Vec<…>` |

- **`execute`** takes `self` and `db: &C` where `C: ConnectionTrait` (so it
  runs inside a caller's transaction). A service that sends mail also takes
  `mailer: &crate::mail::Mailer` (and `config: &Config` for URLs). Return
  the entity `Model` where the PHP returns a model, `()` for `void`, the
  same value otherwise.
- **Order** inside `execute` matches the PHP: `validateRules` (rules, then
  permissions), then the body, statement by statement.
- **Rules**: everything the Rust type doesn't already guarantee is checked
  with `crate::validate::Validator` and `v.finish()?` before the permission
  checks:
  - `required` on a `String`: fail `required` when it is empty after trim
    (Laravel's `required` rejects `""`);
  - `max:n`, `min:n`, `email`, `in:…`, `date_format`, integer bounds;
  - `exists:table,id` on a value that references another row (an
    `emotion_id` on a note, a `template_id` on a vault): query that the row
    exists (in any account), else fail `exists` (Laravel's
    ValidationException). Not for the id of the row the service acts on
    (`gender_id` in UpdateGender) nor for `account_id`/`author_id`/
    `vault_id`/`contact_id`: those are found by the scoped lookup or
    `check()`, and a miss is a 404 (api-design.md §6).
- **Permissions**: `crate::domains::base::check(db, &Check { … })` with the
  service's `permissions()` (see `base.rs` for the mapping). Use the returned
  `Checked` for `$this->author`, `$this->vault`, `$this->contact`,
  `$this->group`.
- **Scoped lookups** (`$this->account()->genders()->findOrFail($id)`,
  `$this->vault->contacts()->findOrFail()`): filter by the parent and map a
  miss to `ApiError::not_found()`.
- **Soft deletes**: query `contacts`, `groups`, `contact_tasks`,
  `contact_important_dates` with `Entity::live()` unless the PHP uses
  `withTrashed()`. `->delete()` on them sets `deleted_at`.
- **Timestamps** are automatic (see `entities/mod.rs`). Use
  `crate::db::now()` for `Carbon::now()`; all times are UTC.
- **Helpers**: `valueOrNull` (empty → null), `valueOrFalse`, `valueOrTrue`
  are ported as written.
- **Translations**: `trans_key('X')` stores the key `"X"` as-is;
  `__('X')` / `trans('X')` becomes `crate::i18n::t(&crate::i18n::current(), "X", &[])`.
- **Jobs and queued services** run synchronously (call their `execute`).
- **Side effects** (feed items, `contacts.last_updated_at`, logs, other
  services called) are ported in the same order.

## Errors

| PHP | Rust (`crate::error::Code`) |
|---|---|
| `ValidationException` | `ValidationFailed` |
| `ModelNotFoundException`, `findOrFail` | `NotFound` |
| `NotEnoughPermissionException` | `Forbidden` |
| `EntryAlreadyExistException`, `SameUserException`, `CantBeDeletedException` | `Conflict` |
| `MaximumNumberOfUsersInVaultException` | `VaultUserLimitReached` |
| other `Exception` thrown on purpose | the closest code; say which in the report |

## Tests

- One `#[tokio::test]` per PHP test method, same name. When a module holds
  several PHP test files, prefix the names after the first file with the
  service (`update_fails_if_…`) and separate files with a
  `// ---- <PhpTestClass>` comment.
- Each test starts with `let db = TestDb::migrated().await;` and ends with
  `db.drop().await;` (`crate::testing::TestDb`).
- Factories: `crate::testing::factory` has account, user, administrator,
  vault, vault user, contact, gender. A factory the domain needs that isn't
  there goes in the domain's `tests.rs`, ported from
  `database/factories/<Model>Factory.php` (Faker values can be any value of
  the same shape).
- `assertDatabaseHas` → query the row and assert the same columns;
  `assertDatabaseMissing` → assert it's gone; `expectException(X)` →
  `assert_eq!(err.code, Code::…)` per the table above.
- `it_fails_if_wrong_parameters_are_given`: PHP passes a request with
  unknown keys. Port it by violating a rule the Rust types don't enforce
  (an empty required string, a too-long string). If every field is typed so
  that no invalid input can be built, leave a one-line comment in place of
  the test saying so.
- Tests use fixtures or values computed by hand from the PHP, never the
  port's own output.

## Correspondence

- Same functions, fields, constants and logic in the same order. A private
  PHP method becomes a private Rust function of the same name (snake_case).
- No additions: no helper layers, abstractions, config structures, logging,
  comments explaining Rust, TODOs, lint suppressions, fallback defaults or
  fields the PHP doesn't have.
- No omissions: every branch and field.
- A deviation is allowed only when the PHP is impossible to port as is
  (e.g. it relies on the database driver) or is a bug listed in
  `docs/rewrite/inventory.md` §11; list each deviation in the report.
- Size: the Rust module may be up to ~30% longer than the PHP it ports.

## Checks

From `rust/`, for your modules:

```
for e in sqlite mysql postgres; do MONICA_TEST_DB=$e cargo test --lib domains::<area>::<domain>; done
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
