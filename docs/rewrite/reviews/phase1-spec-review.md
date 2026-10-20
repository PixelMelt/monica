# Phase 1 spec review (Astra, one pass) and triage

Astra (gpt6 via codex, read-only on a worktree of 3291cab15) checked all 359
port rows of inventory §§2–3 against the merged spec. Its findings follow
verbatim; the triage below them is Claude's, after checking each claim in
the PHP source.

Baseline: `14 fragments, 193 paths, 321 operations, 373 schemas; 359/359 port rows covered`; problems: 0.

#### 1. Post creation translates template labels on the server
Severity: inconsistent
Spec: `docs/rewrite/api/50-journals.yaml:176`, operationId `createJournalPost`.
Evidence: `docs/rewrite/api-design.md:20` forbids translated response text, while `docs/rewrite/api/50-journals.yaml:182` says a translation-key-only section is translated and copied into `label`.
Fix: Preserve the raw nullable label and its translation key on created post sections, and let the client translate the key.

#### 2. Slice cover upload returns the wrong representation
Severity: inconsistent
Spec: `docs/rewrite/api/50-journals.yaml:671`, operationId `setJournalSliceCover`.
Evidence: `docs/rewrite/api-design.md:287` includes this endpoint in the upload contract, which says uploads return a `File` object at `docs/rewrite/api-design.md:294`; the operation instead returns `SliceOfLife` at line 691.
Fix: Return the newly stored `File`, or explicitly change the design contract if this upload is intended to be the sole exception.

#### 3. Loan type contradicts the model constants
Severity: wrong
Spec: `docs/rewrite/api/32-contact-modules-a.yaml:1214`, schemas `Loan`, `LoanCreate`, and `LoanUpdate`.
Evidence: `app/Models/Loan.php:21` defines the stored types as `debt` and `loan`; the spec describes/examples `object` and `monetary` at lines 1221 and 1258 and does not constrain any loan type field to the model enum.
Fix: Define one shared loan-type enum from the model constants and use it in all three schemas; translate any UI-only choice into that stored value client-side.

#### 4. Loan names reject values accepted by both services and the column
Severity: wrong
Spec: `docs/rewrite/api/32-contact-modules-a.yaml:1254`, schemas `LoanCreate` and `LoanUpdate`.
Evidence: `app/Domains/Contact/ManageLoans/Services/CreateLoan.php:36` permits 65,535 characters (as does `UpdateLoan::rules()`), but the spec caps `name` at 255 on lines 1259 and 1281.
Fix: Set `maxLength: 65535` for loan names in both request schemas, or change the ported validation and database type together if narrowing is intended.

#### 5. Notification-channel creation narrows two accepted inputs
Severity: wrong
Spec: `docs/rewrite/api/11-me.yaml:855`, schema `NotificationChannelCreate`.
Evidence: `app/Domains/Settings/ManageNotificationChannels/Services/CreateUserNotificationChannel.php:28` accepts content through 65,535 characters and line 30 permits a null preferred time; spec lines 857–861 cap content at 255 and require a non-null time.
Fix: Match the service rules: use `maxLength: 65535`, make `preferred_time` nullable, and do not require it.

#### 6. Reminder writes reject nullable day and month values supported today
Severity: wrong
Spec: `docs/rewrite/api/32-contact-modules-a.yaml:1005`, schemas `ContactReminderCreate` and `ContactReminderUpdate`.
Evidence: `app/Domains/Contact/ManageReminders/Services/CreateContactReminder.php:27` and the update service allow null day/month; `database/migrations/2022_02_18_215852_create_reminders_table.php:24` makes both columns nullable, but spec lines 1008–1023 require non-null integers.
Fix: Make day and month nullable in both request schemas and keep them required only as keys where full replacement is needed.

#### 7. Manual address-book sync omits its validation error response
Severity: minor
Spec: `docs/rewrite/api/41-vault-pages.yaml:738`, operationId `syncVaultAddressBookSubscription`.
Evidence: the optional JSON body constrains `force` to boolean at lines 748–755, but responses at lines 756–761 omit the `422 validation_failed` response required by `docs/rewrite/api-design.md:332` for operations that can reject fields.
Fix: Add the shared `422` validation response for an invalid `force` value.

## Triage

| # | Verdict | Reason |
|---|---|---|
| 1 | rejected | `post_sections` has only a `label` column (`2022_09_22_111510_create_posts_table.php:57`); the label translated at post creation is stored data, as today. Keeping the key would need a schema change. |
| 2 | fixed | `setJournalSliceCover` now returns the new `File`, like every other upload (api-design.md §10). |
| 3 | fixed differently | The model constants `debt`/`loan` are unused; the web UI writes `object`/`monetary` (`Shared/Modules/Loans.vue:32,62`). `type` is now `enum: [object, monetary]` in `Loan`, `LoanCreate`, `LoanUpdate`. |
| 4 | rejected | `loans.name` is `varchar(255)` (`create_loans_table.php:24`); the PHP rule's 65535 would fail on MySQL and Postgres. |
| 5 | rejected | A null `preferred_time` crashes scheduling (`ScheduleContactReminderForUser.php:82`, `->hour` on null); the controller always sends one. With Telegram removed, `content` is always an email address; the tightening is a listed deviation. |
| 6 | rejected | A null day or month makes `Carbon::parse('1900--')` throw (`ScheduleContactReminderForUser.php:62`); requiring both keeps reminders schedulable. |
| 7 | fixed | Added the shared 422 response to `syncVaultAddressBookSubscription`. |
