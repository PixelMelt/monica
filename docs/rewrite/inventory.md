# Monica Rust rewrite: inventory (phase 0)

This is the parity checklist from `plan.md`. Every row is either ported (status
becomes `done` with the test that covers it) or removed (status becomes
`removed` once the code and UI entry points are gone).

The route table (§2) and the class table (§9) are generated from the PHP source
by `docs/rewrite/tools/inventory.py`. Running it again keeps any status that is
not `todo`:

```
python3 docs/rewrite/tools/inventory.py --write docs/rewrite/inventory.md
```

The other sections are written by hand.

ID prefixes: `R` web/API route, `V` route registered by a vendor package,
`J` scheduled task or queued job, `K` console command, `E` event, mail or
notification, `I` external integration, `X` cross-cutting behaviour, `D`
reference data, `P` frontend page or component change, `C` PHP class.

## 1. Summary

| What | Count | Port | Remove |
|---|---|---|---|
| Routes in `routes/web.php` + `routes/api.php` (§2) | 336 (328 web, 8 api) | 321 | 15 |
| Routes registered by vendor packages (§3) | 53 | 38 | 15 |
| Scheduled tasks and queued jobs (§4) | 19 | 17 | 2 |
| Console commands (§5) | 16 (10 app, 6 framework/vendor) | 11 | 5 |
| Events, mail, notifications (§6) | 9 | 7 | 2 |
| External integrations (§7) | 13 | 3 (+1 open, Q1) | 9 |
| Cross-cutting behaviours (§8) | 27 | 27 | 0 |
| Reference data sets (§8b) | 9 | 9 | 0 |
| PHP classes outside routed controllers and ViewHelpers (§9) | 491 | 474 | 17 |
| Frontend changes beyond the data layer (§10) | 16 | 10 | 6 |

The generated route table reports `Kind`:

- `page` renders an Inertia page (78 routes, 70 distinct pages);
- `json` returns JSON to a Vue component (229);
- `redirect` (13), `file` (1), and `—` for closures, redirects and actions
  that return an empty 200;
- `missing-action`: the route points at a controller method that does not
  exist, so calling it gives a 500. There are 8 of these. Their URLs are built
  by ViewHelpers but no Vue code calls them. They are marked `remove` (see §11).

The `Vue page` column holds the Inertia page a route renders. For JSON routes
it holds, in parentheses, the pages whose props contain that route's URL
(traced through the controller and ViewHelper call graph), plus any Vue file
that calls the route by name through Ziggy. `Layout` means the URL comes from
the shared layout (`VaultIndexViewHelper::layoutData`).

## 2. Web and API routes

Disposition `port` means the action moves to `/api/v1` and the page becomes an
SPA route. `port: read (aggregate/list)` marks GET pages that call no service;
their data comes from ViewHelpers and becomes API read endpoints. The
`can:` gates are listed under the controller. The services they call check the
same permissions again (`BaseService::permissions()`).

<!-- BEGIN GENERATED routes -->
| ID | Method | URI | Route name | Controller@action | Services / jobs | ViewHelpers | Vue page | Kind | Disposition | Status |
|---|---|---|---|---|---|---|---|---|---|---|
| R001 | GET | `/` | `home` | Closure | — | — | (Auth/AcceptInvitation) | — | port | todo |
| R002 | POST | `/closeBeta` | `close_beta` | LoginController@closeBeta | — | — | `Pages/Auth/Beta.vue` | — | port | todo |
| R003 | GET | `/.well-known/carddav` | `—` | Redirect:/dav | — | — | — | — | port: redirect | todo |
| R004 | GET | `/.well-known/caldav` | `—` | Redirect:/dav | — | — | — | — | port: redirect | todo |
| R005 | GET | `/.well-known/security.txt` | `—` | Redirect:/security.txt | — | — | — | — | port: redirect | todo |
| R006 | GET | `/auth/{driver}` | `login.provider` | SocialiteCallbackController@login | — | — | `Pages/Auth/ExternalProviders.vue`<br>`Pages/Profile/Partials/UpdateProviders.vue` | redirect | remove: OAuth/Socialite removed | todo (remove) |
| R007 | GET | `/auth/{driver}/callback` | `—` | SocialiteCallbackController@callback | — | — | — | redirect | remove: OAuth/Socialite removed | todo (remove) |
| R008 | POST | `/auth/{driver}/callback` | `—` | SocialiteCallbackController@callback | — | — | — | redirect | remove: OAuth/Socialite removed | todo (remove) |
| R009 | GET | `/invitation/{code}` | `invitation.show` | AcceptInvitationController@show | — | AcceptInvitationShowViewHelper | Auth/AcceptInvitation | page | port: read (aggregate/list) | done (test: `tests/account.rs`) |
| R010 | POST | `/invitation` | `invitation.store` | AcceptInvitationController@store | AcceptInvitation | — | (Auth/AcceptInvitation) | json | port | done (test: `tests/account.rs`) |
| R011 | POST | `/telegram/webhook/{webhook}` | `—` | TelegramWebhookController@store | ScheduleAllContactRemindersForNotificationChannel | — | — | json | remove: Telegram removed | todo (remove) |
| R012 | GET | `/vaults` | `vault.index` | VaultController@index | — | VaultIndexViewHelper | Vault/Index<br>`Layouts/AppLayout.vue` | page | port: read (aggregate/list) | done (test: `tests/vaults.rs`) |
| R013 | GET | `/vaults/create` | `vault.create` | VaultController@create | — | VaultCreateViewHelper<br>VaultIndexViewHelper | Vault/Create | page | port: read (aggregate/list) | todo |
| R014 | POST | `/vaults` | `vault.store` | VaultController@store | CreateVault | — | (Vault/Create) | json | port | done (test: `tests/vaults.rs`) |
| R015 | GET | `/vaults/{vault}` | `vault.show` | VaultController@show | — | ModuleLifeEventViewHelper<br>VaultIndexViewHelper<br>VaultLifeMetricsViewHelper<br>VaultShowViewHelper | Vault/Dashboard/Index | page | port: read (aggregate/list) | todo |
| R016 | GET | `/vaults/{vault}/edit` | `vault.edit` | VaultController@edit | — | VaultEditViewHelper<br>VaultIndexViewHelper | Vault/Edit | page | port: read (aggregate/list) | done (test: `tests/vaults.rs`) |
| R017 | PUT | `/vaults/{vault}` | `vault.update` | VaultController@update | UpdateVault | — | (Vault/Edit) | json | port | done (test: `tests/vaults.rs`) |
| R018 | DELETE | `/vaults/{vault}` | `vault.destroy` | VaultController@destroy | DestroyVault | — | (Vault/Settings/Index) | json | port | done (test: `tests/vaults.rs`) |
| R019 | PUT | `/vaults/{vault}/defaultTab` | `vault.default_tab.update` | VaultDefaultTabOnDashboardController@update<br>can:vault-viewer,vault | UpdateVaultDashboardDefaultTab | — | (Vault/Dashboard/Index) | json | port | done (test: `tests/vaults.rs`) |
| R020 | GET | `/vaults/{vault}/calendar` | `vault.calendar.index` | VaultCalendarController@index<br>can:vault-viewer,vault | — | VaultCalendarIndexViewHelper<br>VaultIndexViewHelper | Vault/Calendar/Index | page | port: read (aggregate/list) | todo |
| R021 | GET | `/vaults/{vault}/calendar/years/{year}/months/{month}` | `vault.calendar.month` | VaultCalendarController@month<br>can:vault-viewer,vault | — | VaultCalendarIndexViewHelper<br>VaultIndexViewHelper | Vault/Calendar/Index | page | port: read (aggregate/list) | todo |
| R022 | GET | `/vaults/{vault}/calendar/years/{year}/months/{month}/days/{day}` | `vault.calendar.day` | VaultCalendarController@day<br>can:vault-viewer,vault | — | VaultCalendarIndexViewHelper | (Vault/Calendar/Index) | json | port | todo |
| R023 | GET | `/vaults/{vault}/reminders` | `vault.reminder.index` | VaultReminderController@index<br>can:vault-viewer,vault | — | VaultIndexViewHelper<br>VaultReminderIndexViewHelper | Vault/Dashboard/Reminder/Index | page | port: read (aggregate/list) | todo |
| R024 | GET | `/vaults/{vault}/feed` | `vault.feed.show` | VaultFeedController@show<br>can:vault-viewer,vault | — | ModuleFeedViewHelper | (Vault/Dashboard/Index) | json | port | todo |
| R025 | GET | `/vaults/{vault}/tasks` | `vault.tasks.index` | VaultTaskController@index<br>can:vault-viewer,vault | — | VaultIndexViewHelper<br>VaultTasksIndexViewHelper | Vault/Dashboard/Task/Index | page | port: read (aggregate/list) | todo |
| R026 | GET | `/vaults/{vault}/reports` | `vault.reports.index` | ReportIndexController@index<br>can:vault-viewer,vault | — | ReportIndexViewHelper<br>VaultIndexViewHelper | Vault/Reports/Index | page | port: read (aggregate/list) | todo |
| R027 | GET | `/vaults/{vault}/reports/addresses` | `vault.reports.addresses.index` | ReportAddressesController@index<br>can:vault-viewer,vault | — | ReportAddressIndexViewHelper<br>VaultIndexViewHelper | Vault/Reports/Address/Index | page | port: read (aggregate/list) | todo |
| R028 | GET | `/vaults/{vault}/reports/addresses/city/{city}` | `vault.reports.addresses.cities.show` | ReportAddressesCitiesController@show<br>can:vault-viewer,vault | — | ReportCitiesShowViewHelper<br>VaultIndexViewHelper | Vault/Reports/Address/Cities/Index | page | port: read (aggregate/list) | todo |
| R029 | GET | `/vaults/{vault}/reports/addresses/country/{country}` | `vault.reports.addresses.countries.show` | ReportAddressesCountriesController@show<br>can:vault-viewer,vault | — | ReportCountriesShowViewHelper<br>VaultIndexViewHelper | Vault/Reports/Address/Countries/Index | page | port: read (aggregate/list) | todo |
| R030 | GET | `/vaults/{vault}/reports/moodTrackingEvents` | `vault.reports.mood_tracking_events.index` | ReportMoodTrackingEventController@index<br>can:vault-viewer,vault | — | ReportMoodTrackingEventIndexViewHelper<br>VaultIndexViewHelper | Vault/Reports/MoodTrackingEvents/Index | page | port: read (aggregate/list) | todo |
| R031 | GET | `/vaults/{vault}/reports/importantDates` | `vault.reports.important_dates.index` | ReportImportantDateSummaryController@index<br>can:vault-viewer,vault | — | ReportImportantDateSummaryIndexViewHelper<br>VaultIndexViewHelper | Vault/Reports/ImportantDate/Index | page | port: read (aggregate/list) | todo |
| R032 | POST | `/vaults/{vault}/lifeMetrics` | `vault.life_metrics.store` | LifeMetricController@store<br>can:vault-viewer,vault | CreateLifeMetric | VaultLifeMetricsViewHelper | (Vault/Dashboard/Index) | json | port | todo |
| R033 | PUT | `/vaults/{vault}/lifeMetrics/{metric}` | `vault.life_metrics.update` | LifeMetricController@update<br>can:vault-viewer,vault | UpdateLifeMetric | VaultLifeMetricsViewHelper | (Vault/Dashboard/Index) | json | port | todo |
| R034 | POST | `/vaults/{vault}/lifeMetrics/{metric}` | `vault.life_metrics.contact.store` | LifeMetricContactController@store<br>can:vault-viewer,vault | IncrementLifeMetric | VaultLifeMetricsViewHelper | (Vault/Dashboard/Index) | json | port | todo |
| R035 | DELETE | `/vaults/{vault}/lifeMetrics/{metric}` | `vault.life_metrics.destroy` | LifeMetricController@destroy<br>can:vault-viewer,vault | DestroyLifeMetric | — | (Vault/Dashboard/Index) | json | port | todo |
| R036 | GET | `/vaults/{vault}/contacts` | `contact.index` | ContactController@index<br>can:vault-viewer,vault | — | ContactIndexViewHelper<br>VaultIndexViewHelper | Vault/Contact/Index | page | port: read (aggregate/list) | todo |
| R037 | GET | `/vaults/{vault}/contacts/labels/{label}` | `contact.label.index` | ContactLabelController@index<br>can:vault-viewer,vault | — | ContactIndexViewHelper<br>VaultIndexViewHelper | Vault/Contact/Index | page | port: read (aggregate/list) | todo |
| R038 | PUT | `/vaults/{vault}/contacts/sort` | `contact.sort.update` | ContactSortController@update<br>can:vault-viewer,vault | UpdateContactSortOrder | — | (Vault/Contact/Index) | json | port | done (test: `tests/account.rs`) |
| R039 | GET | `/vaults/{vault}/contacts/create` | `contact.create` | ContactController@create<br>can:vault-viewer,vault | — | ContactCreateViewHelper<br>VaultIndexViewHelper | Vault/Contact/Create | page | port: read (aggregate/list) | todo |
| R040 | POST | `/vaults/{vault}/contacts` | `contact.store` | ContactController@store<br>can:vault-viewer,vault | CreateContact | — | (Vault/Contact/Create) | json | port | todo |
| R041 | GET | `/vaults/{vault}/contacts/{contact}` | `contact.show` | ContactController@show<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateContactView | ContactShowViewHelper<br>VaultIndexViewHelper | Vault/Contact/Show | page | port | todo |
| R042 | GET | `/vaults/{vault}/contacts/{contact}/edit` | `contact.edit` | ContactController@edit<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ContactEditViewHelper<br>VaultIndexViewHelper | Vault/Contact/Names/Edit | page | port: read (aggregate/list) | todo |
| R043 | POST | `/vaults/{vault}/contacts/{contact}` | `contact.update` | ContactController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateContact | — | (Vault/Contact/Names/Edit) | json | port | todo |
| R044 | DELETE | `/vaults/{vault}/contacts/{contact}` | `contact.destroy` | ContactController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyContact | — | (Vault/Contact/Show) | json | port | todo |
| R045 | POST | `/vaults/{vault}/contacts/{contact}/vcard` | `contact.vcard.download` | ContactVCardController@download<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ExportVCard | — | (Vault/Contact/Show) | redirect | port | todo |
| R046 | GET | `/vaults/{vault}/contacts/{contact}/quickFacts/{template}` | `contact.quick_fact.show` | ContactQuickFactController@show<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ContactModuleQuickFactViewHelper | (Vault/Contact/Show) | json | port | todo |
| R047 | POST | `/vaults/{vault}/contacts/{contact}/quickFacts/{template}` | `contact.quick_fact.store` | ContactQuickFactController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateQuickFact | ContactModuleQuickFactViewHelper | (Vault/Contact/Show) | json | port | todo |
| R048 | PUT | `/vaults/{vault}/contacts/{contact}/quickFacts/toggle` | `contact.quick_fact.toggle` | ContactQuickFactToggleController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ToggleQuickFactModule | — | (Vault/Contact/Show) | json | port | todo |
| R049 | PUT | `/vaults/{vault}/contacts/{contact}/quickFacts/{template}/{quickFact}` | `contact.quick_fact.update` | ContactQuickFactController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateQuickFact | ContactModuleQuickFactViewHelper | (Vault/Contact/Show) | json | port | todo |
| R050 | DELETE | `/vaults/{vault}/contacts/{contact}/quickFacts/{template}/{quickFact}` | `contact.quick_fact.destroy` | ContactQuickFactController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyQuickFact | — | (Vault/Contact/Show) | json | port | todo |
| R051 | PUT | `/vaults/{vault}/contacts/{contact}/toggle` | `contact.archive.update` | ContactArchiveController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ToggleArchiveContact | — | (Vault/Contact/Show) | json | port | todo |
| R052 | PUT | `/vaults/{vault}/contacts/{contact}/toggle-favorite` | `contact.favorite.update` | ContactFavoriteController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ToggleFavoriteContact | ModuleContactNameViewHelper | (Vault/Contact/Show) | json | port | todo |
| R053 | GET | `/vaults/{vault}/contacts/{contact}/move` | `contact.move.show` | ContactMoveController@show<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ContactShowMoveViewHelper<br>VaultIndexViewHelper | Vault/Contact/Move | page | port: read (aggregate/list) | todo |
| R054 | POST | `/vaults/{vault}/contacts/{contact}/move` | `contact.move.store` | ContactMoveController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | MoveContactToAnotherVault | — | (Vault/Contact/Move) | redirect | port | todo |
| R055 | GET | `/vaults/{vault}/contacts/{contact}/update-template` | `contact.blank` | ContactNoTemplateController@show<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ContactShowBlankViewHelper<br>VaultIndexViewHelper | Vault/Contact/Blank | page | port: read (aggregate/list) | todo |
| R056 | PUT | `/vaults/{vault}/contacts/{contact}/template` | `contact.template.update` | ContactTemplateController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateContactTemplate | — | (Vault/Contact/Blank) | json | port | todo |
| R057 | GET | `/vaults/{vault}/contacts/{contact}/tabs/{slug}` | `contact.page.show` | ContactPageController@show<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateContactView | ContactShowViewHelper<br>VaultIndexViewHelper | Vault/Contact/Show | page | port | todo |
| R058 | PUT | `/vaults/{vault}/contacts/{contact}/avatar` | `contact.avatar.update` | ModuleAvatarController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdatePhotoAsAvatar<br>UploadFile | — | (Vault/Contact/Show) | json | port | todo |
| R059 | DELETE | `/vaults/{vault}/contacts/{contact}/avatar` | `contact.avatar.destroy` | ModuleAvatarController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyAvatar | — | (Vault/Contact/Show) | json | port | todo |
| R060 | GET | `/vaults/{vault}/contacts/{contact}/feed` | `contact.feed.show` | ContactFeedController@show<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ModuleFeedViewHelper | (Vault/Contact/Show) | json | port | todo |
| R061 | GET | `/vaults/{vault}/contacts/{contact}/dates` | `contact.date.index` | ContactImportantDatesController@index<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ContactImportantDatesViewHelper<br>VaultIndexViewHelper | Vault/Contact/ImportantDates/Index | page | port: read (aggregate/list) | todo |
| R062 | POST | `/vaults/{vault}/contacts/{contact}/dates` | `contact.date.store` | ContactImportantDatesController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateContactImportantDate<br>CreateContactReminder | ContactImportantDatesViewHelper | (Vault/Contact/ImportantDates/Index) | json | port | todo |
| R063 | PUT | `/vaults/{vault}/contacts/{contact}/dates/{date}` | `contact.date.update` | ContactImportantDatesController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateContactImportantDate<br>UpdateContactReminder | ContactImportantDatesViewHelper | (Vault/Contact/ImportantDates/Index) | json | port | todo |
| R064 | DELETE | `/vaults/{vault}/contacts/{contact}/dates/{date}` | `contact.date.destroy` | ContactImportantDatesController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyContactImportantDate | — | (Vault/Contact/ImportantDates/Index) | json | port | todo |
| R065 | GET | `/vaults/{vault}/contacts/{contact}/notes` | `contact.note.index` | ContactNotesController@index<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | NotesIndexViewHelper<br>VaultIndexViewHelper | Vault/Contact/Notes/Index | page | port: read (aggregate/list) | todo |
| R066 | POST | `/vaults/{vault}/contacts/{contact}/notes` | `contact.note.store` | ContactModuleNoteController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateNote | ModuleNotesViewHelper | (Vault/Contact/Notes/Index)<br>(Vault/Contact/Show) | json | port | todo |
| R067 | PUT | `/vaults/{vault}/contacts/{contact}/notes/{note}` | `contact.note.update` | ContactModuleNoteController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateNote | ModuleNotesViewHelper | (Vault/Contact/Notes/Index)<br>(Vault/Contact/Show) | json | port | todo |
| R068 | DELETE | `/vaults/{vault}/contacts/{contact}/notes/{note}` | `contact.note.destroy` | ContactModuleNoteController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyNote | — | (Vault/Contact/Notes/Index)<br>(Vault/Contact/Show) | json | port | todo |
| R069 | GET | `/vaults/{vault}/contacts/{contact}/goals/{goal}` | `contact.goal.show` | ContactGoalController@show<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | GoalShowViewHelper<br>VaultIndexViewHelper | Vault/Contact/Goals/Show | page | port: read (aggregate/list) | todo |
| R070 | POST | `/vaults/{vault}/contacts/{contact}/goals` | `contact.goal.store` | ContactModuleGoalController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateGoal | ModuleGoalsViewHelper | (Vault/Contact/Show) | json | port | todo |
| R071 | PUT | `/vaults/{vault}/contacts/{contact}/goals/{goal}` | `contact.goal.update` | ContactGoalController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateGoal | GoalShowViewHelper | (Vault/Contact/Goals/Show)<br>(Vault/Contact/Show) | json | port | todo |
| R072 | PUT | `/vaults/{vault}/contacts/{contact}/goals/{goal}/streaks` | `contact.goal.streak.update` | ContactModuleStreakController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ToggleStreak | ModuleGoalsViewHelper | (Vault/Contact/Show) | json | port | todo |
| R073 | DELETE | `/vaults/{vault}/contacts/{contact}/goals/{goal}` | `contact.goal.destroy` | ContactGoalController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyGoal | — | (Vault/Contact/Goals/Show)<br>(Vault/Contact/Show) | json | port | todo |
| R074 | POST | `/vaults/{vault}/contacts/{contact}/labels` | `contact.label.store` | ContactModuleLabelController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | AssignLabel<br>CreateLabel | ModuleLabelViewHelper | (Vault/Contact/Show) | json | port | todo |
| R075 | PUT | `/vaults/{vault}/contacts/{contact}/labels/{label}` | `contact.label.update` | ContactModuleLabelController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | AssignLabel | ModuleLabelViewHelper | (Vault/Contact/Show) | json | port | todo |
| R076 | DELETE | `/vaults/{vault}/contacts/{contact}/labels/{label}` | `contact.label.destroy` | ContactModuleLabelController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | RemoveLabel | ModuleLabelViewHelper | (Vault/Contact/Show) | json | port | todo |
| R077 | POST | `/vaults/{vault}/contacts/{contact}/reminders` | `contact.reminder.store` | ContactModuleReminderController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateContactReminder | ModuleRemindersViewHelper | (Vault/Contact/Show) | json | port | todo |
| R078 | PUT | `/vaults/{vault}/contacts/{contact}/reminders/{reminder}` | `contact.reminder.update` | ContactModuleReminderController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateContactReminder | ModuleRemindersViewHelper | (Vault/Contact/Show) | json | port | todo |
| R079 | DELETE | `/vaults/{vault}/contacts/{contact}/reminders/{reminder}` | `contact.reminder.destroy` | ContactModuleReminderController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyReminder | — | (Vault/Contact/Show) | json | port | todo |
| R080 | POST | `/vaults/{vault}/contacts/{contact}/addresses` | `contact.address.store` | ContactModuleAddressController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | AssociateAddressToContact<br>CreateAddress | ModuleContactAddressesViewHelper | (Vault/Contact/Show) | json | port | todo |
| R081 | PUT | `/vaults/{vault}/contacts/{contact}/addresses/{address}` | `contact.address.update` | ContactModuleAddressController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateAddress | ModuleContactAddressesViewHelper | (Vault/Contact/Show) | json | port | todo |
| R082 | DELETE | `/vaults/{vault}/contacts/{contact}/addresses/{address}` | `contact.address.destroy` | ContactModuleAddressController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | RemoveAddressFromContact | — | (Vault/Contact/Show) | json | port | todo |
| R083 | GET | `/vaults/{vault}/contacts/{contact}/addresses/{address}/image/{width}x{height}` | `contact.address.image.show` | ContactModuleAddressImageController@show<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | — | — | file | remove: Mapbox static maps removed | todo (remove) |
| R084 | POST | `/vaults/{vault}/contacts/{contact}/contactInformation` | `contact.contact_information.store` | ContactInformationController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateContactInformation | ModuleContactInformationViewHelper | (Vault/Contact/Show) | json | port | todo |
| R085 | PUT | `/vaults/{vault}/contacts/{contact}/contactInformation/{info}` | `contact.contact_information.update` | ContactInformationController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateContactInformation | ModuleContactInformationViewHelper | (Vault/Contact/Show) | json | port | todo |
| R086 | DELETE | `/vaults/{vault}/contacts/{contact}/contactInformation/{info}` | `contact.contact_information.destroy` | ContactInformationController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyContactInformation | — | (Vault/Contact/Show) | json | port | todo |
| R087 | POST | `/vaults/{vault}/contacts/{contact}/loans` | `contact.loan.store` | ContactModuleLoanController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateLoan | ModuleLoanViewHelper | (Vault/Contact/Show) | json | port | todo |
| R088 | PUT | `/vaults/{vault}/contacts/{contact}/loans/{loan}` | `contact.loan.update` | ContactModuleLoanController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateLoan | ModuleLoanViewHelper | (Vault/Contact/Show) | json | port | todo |
| R089 | PUT | `/vaults/{vault}/contacts/{contact}/loans/{loan}/toggle` | `contact.loan.toggle` | ContactModuleToggleLoanController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ToggleLoan | ModuleLoanViewHelper | (Vault/Contact/Show) | json | port | todo |
| R090 | DELETE | `/vaults/{vault}/contacts/{contact}/loans/{loan}` | `contact.loan.destroy` | ContactModuleLoanController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyLoan | — | (Vault/Contact/Show) | json | port | todo |
| R091 | GET | `/vaults/{vault}/contacts/{contact}/companies/list` | `contact.companies.list.index` | ContactModuleJobInformationController@index<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ModuleCompanyViewHelper | (Vault/Contact/Show) | json | port | todo |
| R092 | PUT | `/vaults/{vault}/contacts/{contact}/jobInformation` | `contact.job_information.update` | ContactModuleJobInformationController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateCompany<br>UpdateJobInformation | ModuleCompanyViewHelper | (Vault/Contact/Show) | json | port | todo |
| R093 | DELETE | `/vaults/{vault}/contacts/{contact}/jobInformation` | `contact.job_information.destroy` | ContactModuleJobInformationController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ResetJobInformation | ModuleCompanyViewHelper | (Vault/Contact/Show) | json | port | todo |
| R094 | PUT | `/vaults/{vault}/contacts/{contact}/religion` | `contact.religion.update` | ContactModuleReligionController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateReligion | ModuleReligionViewHelper | (Vault/Contact/Show) | json | port | todo |
| R095 | GET | `/vaults/{vault}/contacts/{contact}/relationships/create` | `contact.relationships.create` | ContactRelationshipsController@create<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ContactRelationshipsCreateViewHelper<br>VaultIndexViewHelper | Vault/Contact/Relationships/Create | page | port: read (aggregate/list) | todo |
| R096 | POST | `/vaults/{vault}/contacts/{contact}/relationships` | `contact.relationships.store` | ContactRelationshipsController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateContact<br>SetRelationship | — | (Vault/Contact/Relationships/Create) | json | port | todo |
| R097 | PUT | `/vaults/{vault}/contacts/{contact}/relationships/{relationship}` | `contact.relationships.update` | ContactRelationshipsController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UnsetRelationship | ModuleRelationshipViewHelper | (Vault/Contact/Show) | json | port | todo |
| R098 | POST | `/vaults/{vault}/contacts/{contact}/pets` | `contact.pet.store` | ContactModulePetController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreatePet | ModulePetsViewHelper | (Vault/Contact/Show) | json | port | todo |
| R099 | PUT | `/vaults/{vault}/contacts/{contact}/pets/{pet}` | `contact.pet.update` | ContactModulePetController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdatePet | ModulePetsViewHelper | (Vault/Contact/Show) | json | port | todo |
| R100 | DELETE | `/vaults/{vault}/contacts/{contact}/pets/{pet}` | `contact.pet.destroy` | ContactModulePetController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyPet | — | (Vault/Contact/Show) | json | port | todo |
| R101 | POST | `/vaults/{vault}/contacts/{contact}/documents` | `contact.document.store` | ContactModuleDocumentController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UploadFile | ModuleDocumentsViewHelper | (Vault/Contact/Show) | json | port | todo |
| R102 | DELETE | `/vaults/{vault}/contacts/{contact}/documents/{document}` | `contact.document.destroy` | ContactModuleDocumentController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyFile | — | (Vault/Contact/Show) | json | port | todo |
| R103 | GET | `/vaults/{vault}/contacts/{contact}/photos` | `contact.photo.index` | ContactPhotoController@index<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ContactPhotosIndexViewHelper<br>VaultIndexViewHelper | Vault/Contact/Photos/Index | page | port: read (aggregate/list) | todo |
| R104 | GET | `/vaults/{vault}/contacts/{contact}/photos/{photo}` | `contact.photo.show` | ContactPhotoController@show<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ContactPhotosShowViewHelper<br>VaultIndexViewHelper | Vault/Contact/Photos/Show | page | port: read (aggregate/list) | todo |
| R105 | POST | `/vaults/{vault}/contacts/{contact}/photos` | `contact.photo.store` | ContactModulePhotoController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UploadFile | ModulePhotosViewHelper | (Vault/Contact/Photos/Index)<br>(Vault/Contact/Show) | json | port | todo |
| R106 | DELETE | `/vaults/{vault}/contacts/{contact}/photos/{photo}` | `contact.photo.destroy` | ContactModulePhotoController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyFile | — | (Vault/Contact/Photos/Index)<br>(Vault/Contact/Photos/Show)<br>(Vault/Contact/Show) | json | port | todo |
| R107 | GET | `/vaults/{vault}/contacts/{contact}/tasks/completed` | `contact.task.index` | ContactModuleTaskController@index<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ModuleContactTasksViewHelper | (Vault/Contact/Show) | json | port | todo |
| R108 | POST | `/vaults/{vault}/contacts/{contact}/tasks` | `contact.task.store` | ContactModuleTaskController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateContactTask | ModuleContactTasksViewHelper | (Vault/Contact/Show) | json | port | todo |
| R109 | PUT | `/vaults/{vault}/contacts/{contact}/tasks/{task}` | `contact.task.update` | ContactModuleTaskController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateContactTask | ModuleContactTasksViewHelper | (Vault/Contact/Show)<br>(Vault/Dashboard/Index)<br>(Vault/Dashboard/Task/Index) | json | port | todo |
| R110 | PUT | `/vaults/{vault}/contacts/{contact}/tasks/{task}/toggle` | `contact.task.toggle` | ContactModuleTaskController@toggle<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ToggleContactTask | ModuleContactTasksViewHelper | (Vault/Contact/Show)<br>(Vault/Dashboard/Index)<br>(Vault/Dashboard/Task/Index) | json | port | todo |
| R111 | DELETE | `/vaults/{vault}/contacts/{contact}/tasks/{task}` | `contact.task.destroy` | ContactModuleTaskController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyContactTask | — | (Vault/Contact/Show)<br>(Vault/Dashboard/Index)<br>(Vault/Dashboard/Task/Index) | json | port | todo |
| R112 | POST | `/vaults/{vault}/contacts/{contact}/calls` | `contact.call.store` | ContactModuleCallController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateCall | ModuleCallsViewHelper | (Vault/Contact/Show) | json | port | todo |
| R113 | PUT | `/vaults/{vault}/contacts/{contact}/calls/{call}` | `contact.call.update` | ContactModuleCallController@update<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateCall | ModuleCallsViewHelper | (Vault/Contact/Show) | json | port | todo |
| R114 | DELETE | `/vaults/{vault}/contacts/{contact}/calls/{call}` | `contact.call.destroy` | ContactModuleCallController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyCall | — | (Vault/Contact/Show) | json | port | todo |
| R115 | POST | `/vaults/{vault}/contacts/{contact}/groups` | `contact.group.store` | ContactModuleGroupController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | AddContactToGroup<br>CreateGroup | ModuleGroupsViewHelper | (Vault/Contact/Show) | json | port | todo |
| R116 | DELETE | `/vaults/{vault}/contacts/{contact}/groups/{group}` | `contact.group.destroy` | ContactModuleGroupController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | RemoveContactFromGroup | ModuleGroupsViewHelper | (Vault/Contact/Show) | json | port | todo |
| R117 | GET | `/vaults/{vault}/contacts/{contact}/timelineEvents` | `contact.timeline_event.index` | ContactModuleTimelineEventController@index<br>can:vault-viewer,vault, can:contact-owner,vault,contact | — | ModuleLifeEventViewHelper | (Vault/Contact/Show)<br>(Vault/Dashboard/Index) | json | port | todo |
| R118 | POST | `/vaults/{vault}/contacts/{contact}/timelineEvents` | `contact.timeline_event.store` | ContactModuleTimelineEventController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateLifeEvent<br>CreateTimelineEvent | ModuleLifeEventViewHelper | (Vault/Contact/Show)<br>(Vault/Dashboard/Index) | json | port | todo |
| R119 | POST | `/vaults/{vault}/contacts/{contact}/timelineEvents/{timelineEvent}/toggle` | `contact.timeline_event.toggle` | ToggleTimelineEventController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ToggleTimelineEvent | — | (Vault/Contact/Show)<br>(Vault/Dashboard/Index) | json | port | todo |
| R120 | POST | `/vaults/{vault}/contacts/{contact}/timelineEvents/{timelineEvent}/lifeEvents` | `contact.life_event.store` | ContactModuleLifeEventController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateLifeEvent | ModuleLifeEventViewHelper | (Vault/Contact/Show)<br>(Vault/Dashboard/Index) | json | port | todo |
| R121 | DELETE | `/vaults/{vault}/contacts/{contact}/timelineEvents/{timelineEvent}` | `contact.timeline_event.destroy` | ContactModuleTimelineEventController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyTimelineEvent | — | (Vault/Contact/Show)<br>(Vault/Dashboard/Index) | json | port | todo |
| R122 | PUT | `/vaults/{vault}/contacts/{contact}/timelineEvents/{timelineEvent}/lifeEvents/{lifeEvent}` | `contact.life_event.edit` | ContactModuleLifeEventController@edit<br>can:vault-viewer,vault, can:contact-owner,vault,contact | UpdateLifeEvent | ModuleLifeEventViewHelper | (Vault/Contact/Show)<br>(Vault/Dashboard/Index) | json | port | todo |
| R123 | POST | `/vaults/{vault}/contacts/{contact}/timelineEvents/{timelineEvent}/lifeEvents/{lifeEvent}/toggle` | `contact.life_event.toggle` | ToggleLifeEventController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | ToggleLifeEvent | — | (Vault/Contact/Show)<br>(Vault/Dashboard/Index) | json | port | todo |
| R124 | DELETE | `/vaults/{vault}/contacts/{contact}/timelineEvents/{timelineEvent}/lifeEvents/{lifeEvent}` | `contact.life_event.destroy` | ContactModuleLifeEventController@destroy<br>can:vault-viewer,vault, can:contact-owner,vault,contact | DestroyLifeEvent | — | (Vault/Contact/Show)<br>(Vault/Dashboard/Index) | json | port | todo |
| R125 | POST | `/vaults/{vault}/contacts/{contact}/moodTrackingEvents` | `contact.mood_tracking_event.store` | ContactMoodTrackingEventsController@store<br>can:vault-viewer,vault, can:contact-owner,vault,contact | CreateMoodTrackingEvent | VaultShowViewHelper | (Vault/Dashboard/Index) | json | port | todo |
| R126 | GET | `/vaults/{vault}/groups` | `group.index` | GroupController@index<br>can:vault-viewer,vault | — | GroupIndexViewHelper<br>VaultIndexViewHelper | Vault/Group/Index | page | port: read (aggregate/list) | todo |
| R127 | GET | `/vaults/{vault}/groups/{group}` | `group.show` | GroupController@show<br>can:vault-viewer,vault, can:group-owner,vault,group | — | GroupShowViewHelper<br>VaultIndexViewHelper | Vault/Group/Show | page | port: read (aggregate/list) | todo |
| R128 | GET | `/vaults/{vault}/groups/{group}/edit` | `group.edit` | GroupController@edit<br>can:vault-viewer,vault, can:group-owner,vault,group | — | GroupEditViewHelper<br>VaultIndexViewHelper | Vault/Group/Edit | page | port: read (aggregate/list) | todo |
| R129 | PUT | `/vaults/{vault}/groups/{group}` | `group.update` | GroupController@update<br>can:vault-viewer,vault, can:group-owner,vault,group | UpdateGroup | — | (Vault/Group/Edit) | json | port | todo |
| R130 | DELETE | `/vaults/{vault}/groups/{group}` | `group.destroy` | GroupController@destroy<br>can:vault-viewer,vault, can:group-owner,vault,group | DestroyGroup | — | (Vault/Group/Show) | json | port | todo |
| R131 | GET | `/vaults/{vault}/journals` | `journal.index` | JournalController@index<br>can:vault-viewer,vault | — | JournalIndexViewHelper<br>VaultIndexViewHelper | Vault/Journal/Index | page | port: read (aggregate/list) | todo |
| R132 | GET | `/vaults/{vault}/journals/create` | `journal.create` | JournalController@create<br>can:vault-viewer,vault | — | JournalCreateViewHelper<br>VaultIndexViewHelper | Vault/Journal/Create | page | port: read (aggregate/list) | todo |
| R133 | POST | `/vaults/{vault}/journals` | `journal.store` | JournalController@store<br>can:vault-viewer,vault | CreateJournal | — | (Vault/Journal/Create) | redirect | port | todo |
| R134 | GET | `/vaults/{vault}/journals/{journal}` | `journal.show` | JournalController@show<br>can:vault-viewer,vault, can:journal-owner,vault,journal | — | JournalShowViewHelper<br>VaultIndexViewHelper | Vault/Journal/Show | page | port: read (aggregate/list) | todo |
| R135 | GET | `/vaults/{vault}/journals/{journal}/photos` | `journal.photo.index` | JournalPhotoController@index<br>can:vault-viewer,vault, can:journal-owner,vault,journal | — | JournalPhotoIndexViewHelper<br>VaultIndexViewHelper | Vault/Journal/Photo/Index | page | port: read (aggregate/list) | todo |
| R136 | GET | `/vaults/{vault}/journals/{journal}/years/{year}` | `journal.year` | JournalController@year<br>can:vault-viewer,vault, can:journal-owner,vault,journal | — | JournalShowViewHelper<br>VaultIndexViewHelper | Vault/Journal/Show | page | port: read (aggregate/list) | todo |
| R137 | GET | `/vaults/{vault}/journals/{journal}/edit` | `journal.edit` | JournalController@edit<br>can:vault-viewer,vault, can:journal-owner,vault,journal | — | JournalEditViewHelper<br>VaultIndexViewHelper | Vault/Journal/Edit | page | port: read (aggregate/list) | todo |
| R138 | PUT | `/vaults/{vault}/journals/{journal}` | `journal.update` | JournalController@update<br>can:vault-viewer,vault, can:journal-owner,vault,journal | UpdateJournal | — | (Vault/Journal/Edit) | redirect | port | todo |
| R139 | DELETE | `/vaults/{vault}/journals/{journal}` | `journal.destroy` | JournalController@destroy<br>can:vault-viewer,vault, can:journal-owner,vault,journal | DestroyJournal | — | (Vault/Journal/Show) | redirect | port | todo |
| R140 | GET | `/vaults/{vault}/journals/{journal}/posts/create` | `post.create` | PostController@create<br>can:vault-viewer,vault, can:journal-owner,vault,journal | — | PostCreateViewHelper<br>VaultIndexViewHelper | Vault/Journal/Post/Template | page | port: read (aggregate/list) | todo |
| R141 | GET | `/vaults/{vault}/journals/{journal}/posts/template/{template}` | `post.store` | PostController@store<br>can:vault-viewer,vault, can:journal-owner,vault,journal | CreatePost | — | (Vault/Journal/Post/Template) | redirect | port | todo |
| R142 | GET | `/vaults/{vault}/journals/{journal}/posts/{post}` | `post.show` | PostController@show<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | IncrementPostReadCounter | PostShowViewHelper<br>VaultIndexViewHelper | Vault/Journal/Post/Show | page | port | todo |
| R143 | GET | `/vaults/{vault}/journals/{journal}/posts/{post}/edit` | `post.edit` | PostController@edit<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | — | PostEditViewHelper<br>VaultIndexViewHelper | Vault/Journal/Post/Edit | page | port: read (aggregate/list) | todo |
| R144 | PUT | `/vaults/{vault}/journals/{journal}/posts/{post}/update` | `post.update` | PostController@update<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | AddContactToPost<br>UpdatePost | — | (Vault/Journal/Post/Edit) | json | port | todo |
| R145 | POST | `/vaults/{vault}/journals/{journal}/posts/{post}/photos` | `post.photos.store` | PostPhotoController@store<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | AddPhotoToPost<br>UploadFile | PostEditViewHelper | (Vault/Journal/Post/Edit) | json | port | todo |
| R146 | DELETE | `/vaults/{vault}/journals/{journal}/posts/{post}/photos/{photo}` | `post.photos.destroy` | PostPhotoController@destroy<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | DestroyFile | — | (Vault/Journal/Post/Edit) | json | port | todo |
| R147 | DELETE | `/vaults/{vault}/journals/{journal}/posts/{post}` | `post.destroy` | PostController@destroy<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | DestroyPost | — | (Vault/Journal/Post/Edit) | redirect | port | todo |
| R148 | POST | `/vaults/{vault}/journals/{journal}/posts/{post}/tags` | `post.tag.store` | PostTagController@store<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | AssignTag<br>CreateTag | PostEditViewHelper | (Vault/Journal/Post/Edit) | json | port | todo |
| R149 | PUT | `/vaults/{vault}/journals/{journal}/posts/{post}/tags/{tag}` | `post.tag.update` | PostTagController@update<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | AssignTag | PostEditViewHelper | (Vault/Journal/Post/Edit) | json | port | todo |
| R150 | DELETE | `/vaults/{vault}/journals/{journal}/posts/{post}/tags/{tag}` | `post.tag.destroy` | PostTagController@destroy<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | RemoveTag | PostEditViewHelper | (Vault/Journal/Post/Edit) | json | port | todo |
| R151 | PUT | `/vaults/{vault}/journals/{journal}/posts/{post}/slices` | `post.slices.update` | PostSliceOfLifeController@update<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | AddPostToSliceOfLife | SliceOfLifeShowViewHelper | (Vault/Journal/Post/Edit) | json | port | todo |
| R152 | DELETE | `/vaults/{vault}/journals/{journal}/posts/{post}/slices` | `post.slices.destroy` | PostSliceOfLifeController@destroy<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | RemovePostFromSliceOfLife | — | (Vault/Journal/Post/Edit) | json | port | todo |
| R153 | POST | `/vaults/{vault}/journals/{journal}/posts/{post}/metrics` | `post.metrics.store` | PostMetricController@store<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | CreatePostMetric | PostEditViewHelper | (Vault/Journal/Post/Edit)<br>(Vault/Journal/Post/Show) | json | port | todo |
| R154 | DELETE | `/vaults/{vault}/journals/{journal}/posts/{post}/metrics/{metric}` | `post.metrics.destroy` | PostMetricController@destroy<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:post-owner,journal,post | DestroyPostMetric | — | (Vault/Journal/Post/Edit)<br>(Vault/Journal/Post/Show) | json | port | todo |
| R155 | GET | `/vaults/{vault}/journals/{journal}/slices` | `slices.index` | SliceOfLifeController@index<br>can:vault-viewer,vault, can:journal-owner,vault,journal | — | SliceOfLifeIndexViewHelper<br>VaultIndexViewHelper | Vault/Journal/Slices/Index | page | port: read (aggregate/list) | todo |
| R156 | POST | `/vaults/{vault}/journals/{journal}/slices` | `slices.store` | SliceOfLifeController@store<br>can:vault-viewer,vault, can:journal-owner,vault,journal | CreateSliceOfLife | SliceOfLifeIndexViewHelper | (Vault/Journal/Slices/Index) | json | port | todo |
| R157 | GET | `/vaults/{vault}/journals/{journal}/slices/{slice}` | `slices.show` | SliceOfLifeController@show<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:slice-owner,journal,slice | — | SliceOfLifeShowViewHelper<br>VaultIndexViewHelper | Vault/Journal/Slices/Show | page | port: read (aggregate/list) | todo |
| R158 | GET | `/vaults/{vault}/journals/{journal}/slices/{slice}/edit` | `slices.edit` | SliceOfLifeController@edit<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:slice-owner,journal,slice | — | SliceOfLifeEditViewHelper<br>VaultIndexViewHelper | Vault/Journal/Slices/Edit | page | port: read (aggregate/list) | todo |
| R159 | PUT | `/vaults/{vault}/journals/{journal}/slices/{slice}` | `slices.update` | SliceOfLifeController@update<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:slice-owner,journal,slice | UpdateSliceOfLife | — | (Vault/Journal/Slices/Edit) | redirect | port | todo |
| R160 | PUT | `/vaults/{vault}/journals/{journal}/slices/{slice}/cover` | `slices.cover.update` | SliceOfLifeCoverImageController@update<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:slice-owner,journal,slice | SetSliceOfLifeCoverImage<br>UploadFile | SliceOfLifeShowViewHelper | (Vault/Journal/Post/Edit)<br>(Vault/Journal/Slices/Show) | json | port | todo |
| R161 | DELETE | `/vaults/{vault}/journals/{journal}/slices/{slice}/cover` | `slices.cover.destroy` | SliceOfLifeCoverImageController@destroy<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:slice-owner,journal,slice | RemoveSliceOfLifeCoverImage | SliceOfLifeShowViewHelper | (Vault/Journal/Post/Edit)<br>(Vault/Journal/Slices/Show) | json | port | todo |
| R162 | DELETE | `/vaults/{vault}/journals/{journal}/slices/{slice}` | `slices.destroy` | SliceOfLifeController@destroy<br>can:vault-viewer,vault, can:journal-owner,vault,journal, can:slice-owner,journal,slice | DestroySliceOfLife | — | (Vault/Journal/Post/Edit)<br>(Vault/Journal/Slices/Show) | redirect | port | todo |
| R163 | GET | `/vaults/{vault}/journals/{journal}/metrics` | `journal_metrics.index` | JournalMetricController@index<br>can:vault-viewer,vault, can:journal-owner,vault,journal | — | JournalMetricIndexViewHelper<br>VaultIndexViewHelper | Vault/Journal/Metrics/Index | page | port: read (aggregate/list) | todo |
| R164 | POST | `/vaults/{vault}/journals/{journal}/metrics` | `journal_metrics.store` | JournalMetricController@store<br>can:vault-viewer,vault, can:journal-owner,vault,journal | CreateJournalMetric | JournalMetricIndexViewHelper | (Vault/Journal/Metrics/Index) | json | port | todo |
| R165 | DELETE | `/vaults/{vault}/journals/{journal}/metrics/{metric}` | `journal_metrics.destroy` | JournalMetricController@destroy<br>can:vault-viewer,vault, can:journal-owner,vault,journal | DestroyJournalMetric | — | (Vault/Journal/Metrics/Index) | json | port | todo |
| R166 | GET | `/vaults/{vault}/files` | `vault.files.index` | VaultFileController@index<br>can:vault-viewer,vault | — | VaultFileIndexViewHelper<br>VaultIndexViewHelper | Vault/Files/Index | page | port: read (aggregate/list) | todo |
| R167 | GET | `/vaults/{vault}/files/photos` | `vault.files.photos` | VaultFileController@photos<br>can:vault-viewer,vault | — | VaultFileIndexViewHelper<br>VaultIndexViewHelper | Vault/Files/Index | page | port: read (aggregate/list) | todo |
| R168 | GET | `/vaults/{vault}/files/documents` | `vault.files.documents` | VaultFileController@documents<br>can:vault-viewer,vault | — | VaultFileIndexViewHelper<br>VaultIndexViewHelper | Vault/Files/Index | page | port: read (aggregate/list) | todo |
| R169 | GET | `/vaults/{vault}/files/avatars` | `vault.files.avatars` | VaultFileController@avatars<br>can:vault-viewer,vault | — | VaultFileIndexViewHelper<br>VaultIndexViewHelper | Vault/Files/Index | page | port: read (aggregate/list) | todo |
| R170 | DELETE | `/vaults/{vault}/files/{file}` | `vault.files.destroy` | VaultFileController@destroy<br>can:vault-viewer,vault | DestroyFile | — | (Vault/Files/Index) | json | port | todo |
| R171 | GET | `/vaults/{vault}/companies` | `vault.companies.index` | VaultCompanyController@index<br>can:vault-viewer,vault | — | CompanyIndexViewHelper<br>VaultIndexViewHelper | Vault/Companies/Index | page | port: read (aggregate/list) | todo |
| R172 | GET | `/vaults/{vault}/companies/{company}` | `vault.companies.show` | VaultCompanyController@show<br>can:vault-viewer,vault | — | — | (Vault/Companies/Index) | missing-action | remove: dead route (controller action does not exist) | todo (remove) |
| R173 | GET | `/vaults/{vault}/settings` | `vault.settings.index` | VaultSettingsController@index<br>can:vault-viewer,vault, can:vault-manager,vault | — | VaultIndexViewHelper<br>VaultSettingsIndexViewHelper | Vault/Settings/Index | page | port: read (aggregate/list) | done (test: `tests/vaults.rs`) |
| R174 | PUT | `/vaults/{vault}/settings` | `vault.settings.update` | VaultSettingsController@update<br>can:vault-viewer,vault, can:vault-manager,vault | — | — | (Vault/Settings/Index) | missing-action | remove: dead route (controller action does not exist) | todo (remove) |
| R175 | PUT | `/vaults/{vault}/settings/template` | `vault.settings.template.update` | VaultSettingsTemplateController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateVaultDefaultTemplate | — | (Vault/Settings/Index) | json | port | done (test: `tests/vaults.rs`) |
| R176 | POST | `/vaults/{vault}/settings/users` | `vault.settings.user.store` | VaultSettingsUserController@store<br>can:vault-viewer,vault, can:vault-manager,vault | GrantVaultAccessToUser | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | done (test: `tests/vaults.rs`) |
| R177 | PUT | `/vaults/{vault}/settings/users/{user}` | `vault.settings.user.update` | VaultSettingsUserController@update<br>can:vault-viewer,vault, can:vault-manager,vault | ChangeVaultAccess | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | done (test: `tests/vaults.rs`) |
| R178 | DELETE | `/vaults/{vault}/settings/users/{user}` | `vault.settings.user.destroy` | VaultSettingsUserController@destroy<br>can:vault-viewer,vault, can:vault-manager,vault | RemoveVaultAccess | — | (Vault/Settings/Index) | json | port | done (test: `tests/vaults.rs`) |
| R179 | GET | `/vaults/{vault}/settings/labels` | `vault.settings.label.index` | VaultSettingsLabelController@index<br>can:vault-viewer,vault, can:vault-manager,vault | — | — | — | missing-action | remove: dead route (controller action does not exist) | todo (remove) |
| R180 | POST | `/vaults/{vault}/settings/labels` | `vault.settings.label.store` | VaultSettingsLabelController@store<br>can:vault-viewer,vault, can:vault-manager,vault | CreateLabel | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R181 | PUT | `/vaults/{vault}/settings/labels/{label}` | `vault.settings.label.update` | VaultSettingsLabelController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateLabel | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R182 | DELETE | `/vaults/{vault}/settings/labels/{label}` | `vault.settings.label.destroy` | VaultSettingsLabelController@destroy<br>can:vault-viewer,vault, can:vault-manager,vault | DestroyLabel | — | (Vault/Settings/Index) | json | port | todo |
| R183 | GET | `/vaults/{vault}/settings/tags` | `vault.settings.tag.index` | VaultSettingsTagController@index<br>can:vault-viewer,vault, can:vault-manager,vault | — | — | — | missing-action | remove: dead route (controller action does not exist) | todo (remove) |
| R184 | POST | `/vaults/{vault}/settings/tags` | `vault.settings.tag.store` | VaultSettingsTagController@store<br>can:vault-viewer,vault, can:vault-manager,vault | CreateTag | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R185 | PUT | `/vaults/{vault}/settings/tags/{tag}` | `vault.settings.tag.update` | VaultSettingsTagController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateTag | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R186 | DELETE | `/vaults/{vault}/settings/tags/{tag}` | `vault.settings.tag.destroy` | VaultSettingsTagController@destroy<br>can:vault-viewer,vault, can:vault-manager,vault | DestroyTag | — | (Vault/Settings/Index) | json | port | todo |
| R187 | POST | `/vaults/{vault}/settings/contactImportantDateTypes` | `vault.settings.important_date_type.store` | VaultSettingsContactImportantDateTypeController@store<br>can:vault-viewer,vault, can:vault-manager,vault | CreateContactImportantDateType | VaultImportantDateTypesViewHelper | (Vault/Settings/Index) | json | port | todo |
| R188 | PUT | `/vaults/{vault}/settings/contactImportantDateTypes/{type}` | `vault.settings.important_date_type.update` | VaultSettingsContactImportantDateTypeController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateContactImportantDateType | VaultImportantDateTypesViewHelper | (Vault/Settings/Index) | json | port | todo |
| R189 | DELETE | `/vaults/{vault}/settings/contactImportantDateTypes/{type}` | `vault.settings.important_date_type.destroy` | VaultSettingsContactImportantDateTypeController@destroy<br>can:vault-viewer,vault, can:vault-manager,vault | DestroyContactImportantDateType | — | (Vault/Settings/Index) | json | port | todo |
| R190 | PUT | `/vaults/{vault}/settings/visibility` | `vault.settings.tab.update` | VaultSettingsTabVisibilityController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateVaultTabVisibility | — | (Vault/Settings/Index) | — | port | done (test: `tests/vaults.rs`) |
| R191 | POST | `/vaults/{vault}/settings/moodTrackingParameters` | `vault.settings.mood_tracking_parameter.store` | VaultSettingsMoodTrackingParameterController@store<br>can:vault-viewer,vault, can:vault-manager,vault | CreateMoodTrackingParameter | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R192 | PUT | `/vaults/{vault}/settings/moodTrackingParameters/{parameter}` | `vault.settings.mood_tracking_parameter.update` | VaultSettingsMoodTrackingParameterController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateMoodTrackingParameter | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R193 | PUT | `/vaults/{vault}/settings/moodTrackingParameters/{parameter}/order` | `vault.settings.mood_tracking_parameter.order.update` | VaultSettingsMoodTrackingParameterPositionController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateMoodTrackingParameterPosition | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R194 | DELETE | `/vaults/{vault}/settings/moodTrackingParameters/{parameter}` | `vault.settings.mood_tracking_parameter.destroy` | VaultSettingsMoodTrackingParameterController@destroy<br>can:vault-viewer,vault, can:vault-manager,vault | DestroyMoodTrackingParameter | — | (Vault/Settings/Index) | json | port | todo |
| R195 | POST | `/vaults/{vault}/settings/lifeEventCategories` | `vault.settings.life_event_categories.store` | VaultSettingsLifeEventCategoriesController@store<br>can:vault-viewer,vault, can:vault-manager,vault | CreateLifeEventCategory | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R196 | PUT | `/vaults/{vault}/settings/lifeEventCategories/{lifeEventCategory}` | `vault.settings.life_event_categories.update` | VaultSettingsLifeEventCategoriesController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateLifeEventCategory | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R197 | DELETE | `/vaults/{vault}/settings/lifeEventCategories/{lifeEventCategory}` | `vault.settings.life_event_categories.destroy` | VaultSettingsLifeEventCategoriesController@destroy<br>can:vault-viewer,vault, can:vault-manager,vault | DestroyLifeEventCategory | — | (Vault/Settings/Index) | json | port | todo |
| R198 | POST | `/vaults/{vault}/settings/lifeEventCategories/{lifeEventCategory}/order` | `vault.settings.life_event_categories.order.update` | VaultSettingsLifeEventCategoriesPositionController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateLifeEventCategoryPosition | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R199 | POST | `/vaults/{vault}/settings/lifeEventCategories/{lifeEventCategory}/lifeEventTypes` | `vault.settings.life_event_types.store` | VaultSettingsLifeEventTypesController@store<br>can:vault-viewer,vault, can:vault-manager,vault | CreateLifeEventType | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R200 | PUT | `/vaults/{vault}/settings/lifeEventCategories/{lifeEventCategory}/lifeEventTypes/{lifeEventType}` | `vault.settings.life_event_types.update` | VaultSettingsLifeEventTypesController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateLifeEventType | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R201 | DELETE | `/vaults/{vault}/settings/lifeEventCategories/{lifeEventCategory}/lifeEventTypes/{lifeEventType}` | `vault.settings.life_event_types.destroy` | VaultSettingsLifeEventTypesController@destroy<br>can:vault-viewer,vault, can:vault-manager,vault | DestroyLifeEventType | — | (Vault/Settings/Index) | json | port | todo |
| R202 | POST | `/vaults/{vault}/settings/lifeEventCategories/{lifeEventCategory}/lifeEventTypes/{lifeEventType}/order` | `vault.settings.life_event_types.order.update` | VaultSettingsLifeEventTypesPositionController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateLifeEventTypePosition | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R203 | POST | `/vaults/{vault}/settings/quickFactTemplates` | `vault.settings.quick_fact_templates.store` | VaultSettingsQuickFactTemplateController@store<br>can:vault-viewer,vault, can:vault-manager,vault | CreateQuickFactTemplate | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R204 | PUT | `/vaults/{vault}/settings/quickFactTemplates/{template}` | `vault.settings.quick_fact_templates.update` | VaultSettingsQuickFactTemplateController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateQuickFactTemplate | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R205 | PUT | `/vaults/{vault}/settings/quickFactTemplates/{template}/order` | `vault.settings.quick_fact_templates.order.update` | VaultSettingsQuickFactTemplatePositionController@update<br>can:vault-viewer,vault, can:vault-manager,vault | UpdateQuickFactTemplatePosition | VaultSettingsIndexViewHelper | (Vault/Settings/Index) | json | port | todo |
| R206 | DELETE | `/vaults/{vault}/settings/quickFactTemplates/{template}` | `vault.settings.quick_fact_templates.destroy` | VaultSettingsQuickFactTemplateController@destroy<br>can:vault-viewer,vault, can:vault-manager,vault | DestroyQuickFactTemplate | — | (Vault/Settings/Index) | json | port | todo |
| R207 | GET | `/vaults/{vault}/search` | `vault.search.index` | VaultSearchController@index<br>can:vault-viewer,vault | — | VaultIndexViewHelper<br>VaultSearchIndexViewHelper | Vault/Search/Index | page | port: read (aggregate/list) | todo |
| R208 | POST | `/vaults/{vault}/search` | `vault.search.show` | VaultSearchController@show<br>can:vault-viewer,vault | — | VaultSearchIndexViewHelper | (Vault/Search/Index) | json | port | todo |
| R209 | GET | `/vaults/{vault}/search/user/contact/mostConsulted` | `vault.user.search.mostconsulted` | VaultMostConsultedContactsController@index<br>can:vault-viewer,vault | — | VaultMostConsultedViewHelper | (Layout) | json | port | todo |
| R210 | POST | `/vaults/{vault}/search/user/contacts` | `vault.user.search.index` | VaultContactSearchController@index<br>can:vault-viewer,vault | — | VaultContactSearchViewHelper | (Layout) | json | port | todo |
| R211 | GET | `/settings` | `settings.index` | SettingsController@index | — | SettingsIndexViewHelper<br>VaultIndexViewHelper | Settings/Index<br>`Pages/API/Index.vue`<br>`Pages/Profile/Show.vue` | page | port: read (aggregate/list) | todo |
| R212 | GET | `/settings/preferences` | `settings.preferences.index` | PreferencesController@index | — | UserPreferencesIndexViewHelper<br>VaultIndexViewHelper | Settings/Preferences/Index | page | port: read (aggregate/list) | done (test: `tests/account.rs`) |
| R213 | POST | `/settings/preferences/name` | `settings.preferences.name.store` | PreferencesNameOrderController@store | StoreNameOrderPreference | UserPreferencesIndexViewHelper | (Settings/Preferences/Index) | json | port | done (test: `tests/account.rs`) |
| R214 | POST | `/settings/preferences/date` | `settings.preferences.date.store` | PreferencesDateFormatController@store | StoreDateFormatPreference | UserPreferencesIndexViewHelper | (Settings/Preferences/Index) | json | port | done (test: `tests/account.rs`) |
| R215 | POST | `/settings/preferences/timezone` | `settings.preferences.timezone.store` | PreferencesTimezoneController@store | StoreTimezone | UserPreferencesIndexViewHelper | (Settings/Preferences/Index) | json | port | done (test: `tests/account.rs`) |
| R216 | POST | `/settings/preferences/number` | `settings.preferences.number.store` | PreferencesNumberFormatController@store | StoreNumberFormatPreference | UserPreferencesIndexViewHelper | (Settings/Preferences/Index) | json | port | done (test: `tests/account.rs`) |
| R217 | POST | `/settings/preferences/distance` | `settings.preferences.distance.store` | PreferencesDistanceFormatController@store | StoreDistanceFormatPreference | UserPreferencesIndexViewHelper | (Settings/Preferences/Index) | json | port | done (test: `tests/account.rs`) |
| R218 | POST | `/settings/preferences/maps` | `settings.preferences.maps.store` | PreferencesMapsPreferenceController@store | StoreMapsPreference | UserPreferencesIndexViewHelper | (Settings/Preferences/Index) | json | port | done (test: `tests/account.rs`) |
| R219 | POST | `/settings/preferences/locale` | `settings.preferences.locale.store` | PreferencesLocaleController@store | StoreLocale | UserPreferencesIndexViewHelper | (Settings/Preferences/Index) | json | port | done (test: `tests/account.rs`) |
| R220 | POST | `/settings/preferences/help` | `settings.preferences.help.store` | PreferencesHelpController@store | StoreHelpPreference | UserPreferencesIndexViewHelper | (Settings/Preferences/Index) | json | port | done (test: `tests/account.rs`) |
| R221 | GET | `/settings/notifications` | `settings.notifications.index` | NotificationsController@index | — | NotificationsIndexViewHelper<br>VaultIndexViewHelper | Settings/Notifications/Index | page | port: read (aggregate/list) | done (test: `tests/account.rs`) |
| R222 | POST | `/settings/notifications` | `settings.notifications.store` | NotificationsController@store | CreateUserNotificationChannel | NotificationsIndexViewHelper | (Settings/Notifications/Index) | json | port | done (test: `tests/account.rs`) |
| R223 | POST | `/settings/notifications/telegram` | `settings.notifications.telegram.store` | TelegramNotificationsController@store | CreateUserNotificationChannel | NotificationsIndexViewHelper | (Settings/Notifications/Index) | json | remove: Telegram removed | todo (remove) |
| R224 | GET | `/settings/notifications/{notification}/verify/{uuid}` | `settings.notifications.verification.store` | NotificationsVerificationController@store | VerifyUserNotificationChannelEmailAddress | NotificationsIndexViewHelper<br>VaultIndexViewHelper | Settings/Notifications/Index | page | port | done (test: `tests/account.rs`) |
| R225 | POST | `/settings/notifications/{notification}/test` | `settings.notifications.test.store` | NotificationsTestController@store | SendTestEmail<br>SendTestTelegramNotification | — | (Settings/Notifications/Index) | json | port | done (test: `tests/account.rs`) |
| R226 | PUT | `/settings/notifications/{notification}/toggle` | `settings.notifications.toggle.update` | NotificationsToggleController@update | ToggleUserNotificationChannel | NotificationsIndexViewHelper | (Settings/Notifications/Index) | json | port | done (test: `tests/account.rs`) |
| R227 | DELETE | `/settings/notifications/{notification}` | `settings.notifications.destroy` | NotificationsController@destroy | DestroyUserNotificationChannel | — | (Settings/Notifications/Index) | json | port | done (test: `tests/account.rs`) |
| R228 | GET | `/settings/notifications/{notification}/logs` | `settings.notifications.log.index` | NotificationsLogController@index | — | NotificationsLogIndexViewHelper<br>VaultIndexViewHelper | Settings/Notifications/Logs/Index | page | port: read (aggregate/list) | done (test: `tests/account.rs`) |
| R229 | GET | `/settings/users` | `settings.user.index` | UserController@index<br>can:administrator | — | UserIndexViewHelper<br>VaultIndexViewHelper | Settings/Users/Index | page | port: read (aggregate/list) | done (test: `tests/account.rs`) |
| R230 | GET | `/settings/users/create` | `settings.user.create` | UserController@create<br>can:administrator | — | UserCreateViewHelper<br>VaultIndexViewHelper | Settings/Users/Create | page | port: read (aggregate/list) | todo |
| R231 | POST | `/settings/users` | `settings.user.store` | UserController@store<br>can:administrator | InviteUser | — | (Settings/Users/Create) | json | port | done (test: `tests/account.rs`) |
| R232 | GET | `/settings/users/{user}` | `settings.user.show` | UserController@show<br>can:administrator | — | — | (Settings/Users/Index) | missing-action | remove: dead route (controller action does not exist) | todo (remove) |
| R233 | PUT | `/settings/users/{user}` | `settings.user.update` | UserController@update<br>can:administrator | GiveAdministratorPrivilege<br>RemoveAdministratorPrivilege | UserIndexViewHelper | (Settings/Users/Index) | json | port | done (test: `tests/account.rs`) |
| R234 | DELETE | `/settings/users/{user}` | `settings.user.destroy` | UserController@destroy<br>can:administrator | DestroyUser | — | (Settings/Users/Index) | json | port | done (test: `tests/account.rs`) |
| R235 | GET | `/settings/personalize` | `settings.personalize.index` | PersonalizeController@index<br>can:administrator | — | PersonalizeIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/Index | page | port: read (aggregate/list) | todo |
| R236 | GET | `/settings/personalize/relationships` | `settings.personalize.relationship.index` | PersonalizeRelationshipController@index<br>can:administrator | — | PersonalizeRelationshipIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/Relationships/Index | page | port: read (aggregate/list) | todo |
| R237 | POST | `/settings/personalize/relationships` | `settings.personalize.relationship.grouptype.store` | PersonalizeRelationshipController@store<br>can:administrator | CreateRelationshipGroupType | PersonalizeRelationshipIndexViewHelper | (Settings/Personalize/Relationships/Index) | json | port | todo |
| R238 | PUT | `/settings/personalize/relationships/{groupType}` | `settings.personalize.relationship.grouptype.update` | PersonalizeRelationshipController@update<br>can:administrator | UpdateRelationshipGroupType | PersonalizeRelationshipIndexViewHelper | (Settings/Personalize/Relationships/Index) | json | port | todo |
| R239 | DELETE | `/settings/personalize/relationships/{groupType}` | `settings.personalize.relationship.grouptype.destroy` | PersonalizeRelationshipController@destroy<br>can:administrator | DestroyRelationshipGroupType | — | (Settings/Personalize/Relationships/Index) | json | port | todo |
| R240 | POST | `/settings/personalize/relationships/{groupType}/types` | `settings.personalize.relationship.type.store` | PersonalizeRelationshipTypeController@store<br>can:administrator | CreateRelationshipType | PersonalizeRelationshipIndexViewHelper | (Settings/Personalize/Relationships/Index) | json | port | todo |
| R241 | PUT | `/settings/personalize/relationships/{groupType}/types/{type}` | `settings.personalize.relationship.type.update` | PersonalizeRelationshipTypeController@update<br>can:administrator | UpdateRelationshipType | PersonalizeRelationshipIndexViewHelper | (Settings/Personalize/Relationships/Index) | json | port | todo |
| R242 | DELETE | `/settings/personalize/relationships/{groupType}/types/{type}` | `settings.personalize.relationship.type.destroy` | PersonalizeRelationshipTypeController@destroy<br>can:administrator | DestroyRelationshipType | — | (Settings/Personalize/Relationships/Index) | json | port | todo |
| R243 | GET | `/settings/personalize/callReasonTypes` | `settings.personalize.call_reasons.index` | PersonalizeCallReasonTypesController@index<br>can:administrator | — | PersonalizeCallReasonsIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/CallReasons/Index | page | port: read (aggregate/list) | todo |
| R244 | POST | `/settings/personalize/callReasonTypes` | `settings.personalize.call_reasons.type.store` | PersonalizeCallReasonTypesController@store<br>can:administrator | CreateCallReasonType | PersonalizeCallReasonsIndexViewHelper | (Settings/Personalize/CallReasons/Index) | json | port | todo |
| R245 | PUT | `/settings/personalize/callReasonTypes/{callReasonType}` | `settings.personalize.call_reasons.type.update` | PersonalizeCallReasonTypesController@update<br>can:administrator | UpdateCallReasonType | PersonalizeCallReasonsIndexViewHelper | (Settings/Personalize/CallReasons/Index) | json | port | todo |
| R246 | DELETE | `/settings/personalize/callReasonTypes/{callReasonType}` | `settings.personalize.call_reasons.type.destroy` | PersonalizeCallReasonTypesController@destroy<br>can:administrator | DestroyCallReasonType | — | (Settings/Personalize/CallReasons/Index) | json | port | todo |
| R247 | POST | `/settings/personalize/callReasonTypes/{callReasonType}/reasons` | `settings.personalize.call_reasons.store` | PersonalizeCallReasonsController@store<br>can:administrator | CreateCallReason | PersonalizeCallReasonsIndexViewHelper | (Settings/Personalize/CallReasons/Index) | json | port | todo |
| R248 | PUT | `/settings/personalize/callReasonTypes/{callReasonType}/reasons/{reason}` | `settings.personalize.call_reasons.update` | PersonalizeCallReasonsController@update<br>can:administrator | UpdateCallReason | PersonalizeCallReasonsIndexViewHelper | (Settings/Personalize/CallReasons/Index) | json | port | todo |
| R249 | DELETE | `/settings/personalize/callReasonTypes/{callReasonType}/reasons/{reason}` | `settings.personalize.call_reasons.destroy` | PersonalizeCallReasonsController@destroy<br>can:administrator | DestroyCallReason | — | (Settings/Personalize/CallReasons/Index) | json | port | todo |
| R250 | GET | `/settings/personalize/giftOccasions` | `settings.personalize.gift_occasions.index` | PersonalizeGiftOccasionController@index<br>can:administrator | — | PersonalizeGiftOccasionViewHelper<br>VaultIndexViewHelper | Settings/Personalize/GiftOccasions/Index | page | port: read (aggregate/list) | todo |
| R251 | POST | `/settings/personalize/giftOccasions` | `settings.personalize.gift_occasions.store` | PersonalizeGiftOccasionController@store<br>can:administrator | CreateGiftOccasion | PersonalizeGiftOccasionViewHelper | (Settings/Personalize/GiftOccasions/Index) | json | port | todo |
| R252 | PUT | `/settings/personalize/giftOccasions/{giftOccasion}` | `settings.personalize.gift_occasions.update` | PersonalizeGiftOccasionController@update<br>can:administrator | UpdateGiftOccasion | PersonalizeGiftOccasionViewHelper | (Settings/Personalize/GiftOccasions/Index) | json | port | todo |
| R253 | DELETE | `/settings/personalize/giftOccasions/{giftOccasion}` | `settings.personalize.gift_occasions.destroy` | PersonalizeGiftOccasionController@destroy<br>can:administrator | DestroyGiftOccasion | — | (Settings/Personalize/GiftOccasions/Index) | json | port | todo |
| R254 | POST | `/settings/personalize/giftOccasions/{giftOccasion}/position` | `settings.personalize.gift_occasions.order.update` | PersonalizeGiftOccasionsPositionController@update<br>can:administrator | UpdateGiftOccasionPosition | PersonalizeGiftOccasionViewHelper | (Settings/Personalize/GiftOccasions/Index) | json | port | todo |
| R255 | GET | `/settings/personalize/giftStates` | `settings.personalize.gift_states.index` | PersonalizeGiftStateController@index<br>can:administrator | — | PersonalizeGiftStateViewHelper<br>VaultIndexViewHelper | Settings/Personalize/GiftStates/Index | page | port: read (aggregate/list) | todo |
| R256 | POST | `/settings/personalize/giftStates` | `settings.personalize.gift_states.store` | PersonalizeGiftStateController@store<br>can:administrator | CreateGiftState | PersonalizeGiftStateViewHelper | (Settings/Personalize/GiftStates/Index) | json | port | todo |
| R257 | PUT | `/settings/personalize/giftStates/{giftState}` | `settings.personalize.gift_states.update` | PersonalizeGiftStateController@update<br>can:administrator | UpdateGiftState | PersonalizeGiftStateViewHelper | (Settings/Personalize/GiftStates/Index) | json | port | todo |
| R258 | DELETE | `/settings/personalize/giftStates/{giftState}` | `settings.personalize.gift_states.destroy` | PersonalizeGiftStateController@destroy<br>can:administrator | DestroyGiftState | — | (Settings/Personalize/GiftStates/Index) | json | port | todo |
| R259 | POST | `/settings/personalize/giftStates/{giftState}/position` | `settings.personalize.gift_states.order.update` | PersonalizeGiftStatesPositionController@update<br>can:administrator | UpdateGiftStatePosition | PersonalizeGiftStateViewHelper | (Settings/Personalize/GiftStates/Index) | json | port | todo |
| R260 | GET | `/settings/personalize/postTemplates` | `settings.personalize.post_templates.index` | PersonalizePostTemplateController@index<br>can:administrator | — | PersonalizePostTemplateViewHelper<br>VaultIndexViewHelper | Settings/Personalize/PostTemplates/Index | page | port: read (aggregate/list) | todo |
| R261 | POST | `/settings/personalize/postTemplates` | `settings.personalize.post_templates.store` | PersonalizePostTemplateController@store<br>can:administrator | CreatePostTemplate | PersonalizePostTemplateViewHelper | (Settings/Personalize/PostTemplates/Index) | json | port | todo |
| R262 | PUT | `/settings/personalize/postTemplates/{postTemplate}` | `settings.personalize.post_templates.update` | PersonalizePostTemplateController@update<br>can:administrator | UpdatePostTemplate | PersonalizePostTemplateViewHelper | (Settings/Personalize/PostTemplates/Index) | json | port | todo |
| R263 | DELETE | `/settings/personalize/postTemplates/{postTemplate}` | `settings.personalize.post_templates.destroy` | PersonalizePostTemplateController@destroy<br>can:administrator | DestroyPostTemplate | — | (Settings/Personalize/PostTemplates/Index) | json | port | todo |
| R264 | POST | `/settings/personalize/postTemplates/{postTemplate}/position` | `settings.personalize.post_templates.order.update` | PersonalizePostTemplatePositionController@update<br>can:administrator | UpdatePostTemplatePosition | PersonalizePostTemplateViewHelper | (Settings/Personalize/PostTemplates/Index) | json | port | todo |
| R265 | POST | `/settings/personalize/postTemplates/{postTemplate}/sections` | `settings.personalize.post_templates.section.store` | PersonalizePostTemplateSectionController@store<br>can:administrator | CreatePostTemplateSection | PersonalizePostTemplateViewHelper | (Settings/Personalize/PostTemplates/Index) | json | port | todo |
| R266 | PUT | `/settings/personalize/postTemplates/{postTemplate}/sections/{section}` | `settings.personalize.post_templates.section.update` | PersonalizePostTemplateSectionController@update<br>can:administrator | UpdatePostTemplateSection | PersonalizePostTemplateViewHelper | (Settings/Personalize/PostTemplates/Index) | json | port | todo |
| R267 | DELETE | `/settings/personalize/postTemplates/{postTemplate}/sections/{section}` | `settings.personalize.post_templates.section.destroy` | PersonalizePostTemplateSectionController@destroy<br>can:administrator | DestroyPostTemplateSection | — | (Settings/Personalize/PostTemplates/Index) | json | port | todo |
| R268 | POST | `/settings/personalize/postTemplates/{postTemplate}/sections/{section}/position` | `settings.personalize.post_templates.section.order.update` | PersonalizePostTemplateSectionPositionController@update<br>can:administrator | UpdatePostTemplateSectionPosition | PersonalizePostTemplateViewHelper | (Settings/Personalize/PostTemplates/Index) | json | port | todo |
| R269 | GET | `/settings/personalize/groupTypes` | `settings.personalize.group_types.index` | PersonalizeGroupTypeController@index<br>can:administrator | — | PersonalizeGroupTypeViewHelper<br>VaultIndexViewHelper | Settings/Personalize/GroupTypes/Index | page | port: read (aggregate/list) | todo |
| R270 | POST | `/settings/personalize/groupTypes` | `settings.personalize.group_types.store` | PersonalizeGroupTypeController@store<br>can:administrator | CreateGroupType | PersonalizeGroupTypeViewHelper | (Settings/Personalize/GroupTypes/Index) | json | port | todo |
| R271 | PUT | `/settings/personalize/groupTypes/{type}` | `settings.personalize.group_types.update` | PersonalizeGroupTypeController@update<br>can:administrator | UpdateGroupType | PersonalizeGroupTypeViewHelper | (Settings/Personalize/GroupTypes/Index) | json | port | todo |
| R272 | DELETE | `/settings/personalize/groupTypes/{type}` | `settings.personalize.group_types.destroy` | PersonalizeGroupTypeController@destroy<br>can:administrator | DestroyGroupType | — | (Settings/Personalize/GroupTypes/Index) | json | port | todo |
| R273 | POST | `/settings/personalize/groupTypes/{type}/position` | `settings.personalize.group_types.order.update` | PersonalizeGroupTypePositionController@update<br>can:administrator | UpdateGroupTypePosition | PersonalizeGroupTypeViewHelper | (Settings/Personalize/GroupTypes/Index) | json | port | todo |
| R274 | POST | `/settings/personalize/groupTypes/{type}/groupTypeRoles` | `settings.personalize.group_types.roles.store` | PersonalizeGroupTypeRoleController@store<br>can:administrator | CreateGroupTypeRole | PersonalizeGroupTypeViewHelper | (Settings/Personalize/GroupTypes/Index) | json | port | todo |
| R275 | PUT | `/settings/personalize/groupTypes/{type}/groupTypeRoles/{role}` | `settings.personalize.group_types.roles.update` | PersonalizeGroupTypeRoleController@update<br>can:administrator | UpdateGroupTypeRole | PersonalizeGroupTypeViewHelper | (Settings/Personalize/GroupTypes/Index) | json | port | todo |
| R276 | DELETE | `/settings/personalize/groupTypes/{type}/groupTypeRoles/{role}` | `settings.personalize.group_types.roles.destroy` | PersonalizeGroupTypeRoleController@destroy<br>can:administrator | DestroyGroupTypeRole | — | (Settings/Personalize/GroupTypes/Index) | json | port | todo |
| R277 | POST | `/settings/personalize/groupTypes/{type}/groupTypeRoles/{role}/position` | `settings.personalize.group_types.roles.order.update` | PersonalizeGroupTypeRolePositionController@update<br>can:administrator | UpdateGroupTypeRolePosition | PersonalizeGroupTypeViewHelper | (Settings/Personalize/GroupTypes/Index) | json | port | todo |
| R278 | GET | `/settings/personalize/genders` | `settings.personalize.gender.index` | ManageGenderController@index<br>can:administrator | — | ManageGenderIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/Genders/Index | page | port: read (aggregate/list) | todo |
| R279 | POST | `/settings/personalize/genders` | `settings.personalize.gender.store` | ManageGenderController@store<br>can:administrator | CreateGender | ManageGenderIndexViewHelper | (Settings/Personalize/Genders/Index) | json | port | todo |
| R280 | PUT | `/settings/personalize/genders/{gender}` | `settings.personalize.gender.update` | ManageGenderController@update<br>can:administrator | UpdateGender | ManageGenderIndexViewHelper | (Settings/Personalize/Genders/Index) | json | port | todo |
| R281 | DELETE | `/settings/personalize/genders/{gender}` | `settings.personalize.gender.destroy` | ManageGenderController@destroy<br>can:administrator | DestroyGender | — | (Settings/Personalize/Genders/Index) | json | port | todo |
| R282 | GET | `/settings/personalize/pronouns` | `settings.personalize.pronoun.index` | PersonalizePronounController@index<br>can:administrator | — | PersonalizePronounIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/Pronouns/Index | page | port: read (aggregate/list) | todo |
| R283 | POST | `/settings/personalize/pronouns` | `settings.personalize.pronoun.store` | PersonalizePronounController@store<br>can:administrator | CreatePronoun | PersonalizePronounIndexViewHelper | (Settings/Personalize/Pronouns/Index) | json | port | todo |
| R284 | PUT | `/settings/personalize/pronouns/{pronoun}` | `settings.personalize.pronoun.update` | PersonalizePronounController@update<br>can:administrator | UpdatePronoun | PersonalizePronounIndexViewHelper | (Settings/Personalize/Pronouns/Index) | json | port | todo |
| R285 | DELETE | `/settings/personalize/pronouns/{pronoun}` | `settings.personalize.pronoun.destroy` | PersonalizePronounController@destroy<br>can:administrator | DestroyPronoun | — | (Settings/Personalize/Pronouns/Index) | json | port | todo |
| R286 | GET | `/settings/personalize/addressTypes` | `settings.personalize.address_type.index` | PersonalizeAddressTypeController@index<br>can:administrator | — | PersonalizeAddressTypeIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/AddressTypes/Index | page | port: read (aggregate/list) | todo |
| R287 | POST | `/settings/personalize/addressTypes` | `settings.personalize.address_type.store` | PersonalizeAddressTypeController@store<br>can:administrator | CreateAddressType | PersonalizeAddressTypeIndexViewHelper | (Settings/Personalize/AddressTypes/Index) | json | port | todo |
| R288 | PUT | `/settings/personalize/addressTypes/{addressType}` | `settings.personalize.address_type.update` | PersonalizeAddressTypeController@update<br>can:administrator | UpdateAddressType | PersonalizeAddressTypeIndexViewHelper | (Settings/Personalize/AddressTypes/Index) | json | port | todo |
| R289 | DELETE | `/settings/personalize/addressTypes/{addressType}` | `settings.personalize.address_type.destroy` | PersonalizeAddressTypeController@destroy<br>can:administrator | DestroyAddressType | — | (Settings/Personalize/AddressTypes/Index) | json | port | todo |
| R290 | GET | `/settings/personalize/petCategories` | `settings.personalize.pet_category.index` | PersonalizePetCategoriesController@index<br>can:administrator | — | PersonalizePetCategoriesIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/PetCategories/Index | page | port: read (aggregate/list) | todo |
| R291 | POST | `/settings/personalize/petCategories` | `settings.personalize.pet_category.store` | PersonalizePetCategoriesController@store<br>can:administrator | CreatePetCategory | PersonalizePetCategoriesIndexViewHelper | (Settings/Personalize/PetCategories/Index) | json | port | todo |
| R292 | PUT | `/settings/personalize/petCategories/{petCategory}` | `settings.personalize.pet_category.update` | PersonalizePetCategoriesController@update<br>can:administrator | UpdatePetCategory | PersonalizePetCategoriesIndexViewHelper | (Settings/Personalize/PetCategories/Index) | json | port | todo |
| R293 | DELETE | `/settings/personalize/petCategories/{petCategory}` | `settings.personalize.pet_category.destroy` | PersonalizePetCategoriesController@destroy<br>can:administrator | DestroyPetCategory | — | (Settings/Personalize/PetCategories/Index) | json | port | todo |
| R294 | GET | `/settings/personalize/contactInformationType` | `settings.personalize.contact_information_type.index` | PersonalizeContatInformationTypesController@index<br>can:administrator | — | PersonalizeContactInformationTypeIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/ContactInformationTypes/Index | page | port: read (aggregate/list) | todo |
| R295 | POST | `/settings/personalize/contactInformationType` | `settings.personalize.contact_information_type.store` | PersonalizeContatInformationTypesController@store<br>can:administrator | CreateContactInformationType | PersonalizeContactInformationTypeIndexViewHelper | (Settings/Personalize/ContactInformationTypes/Index) | json | port | todo |
| R296 | PUT | `/settings/personalize/contactInformationType/{type}` | `settings.personalize.contact_information_type.update` | PersonalizeContatInformationTypesController@update<br>can:administrator | UpdateContactInformationType | PersonalizeContactInformationTypeIndexViewHelper | (Settings/Personalize/ContactInformationTypes/Index) | json | port | todo |
| R297 | DELETE | `/settings/personalize/contactInformationType/{type}` | `settings.personalize.contact_information_type.destroy` | PersonalizeContatInformationTypesController@destroy<br>can:administrator | DestroyContactInformationType | — | (Settings/Personalize/ContactInformationTypes/Index) | json | port | todo |
| R298 | GET | `/settings/personalize/templates` | `settings.personalize.template.index` | PersonalizeTemplatesController@index<br>can:administrator | — | PersonalizeTemplateIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/Templates/Index | page | port: read (aggregate/list) | todo |
| R299 | POST | `/settings/personalize/templates` | `settings.personalize.template.store` | PersonalizeTemplatesController@store<br>can:administrator | CreateTemplate | PersonalizeTemplateIndexViewHelper | (Settings/Personalize/Templates/Index) | json | port | todo |
| R300 | PUT | `/settings/personalize/templates/{template}` | `settings.personalize.template.update` | PersonalizeTemplatesController@update<br>can:administrator | UpdateTemplate | PersonalizeTemplateIndexViewHelper | (Settings/Personalize/Templates/Index) | json | port | todo |
| R301 | DELETE | `/settings/personalize/templates/{template}` | `settings.personalize.template.destroy` | PersonalizeTemplatesController@destroy<br>can:administrator | DestroyTemplate | — | (Settings/Personalize/Templates/Index) | json | port | todo |
| R302 | GET | `/settings/personalize/templates/{template}` | `settings.personalize.template.show` | PersonalizeTemplatesController@show<br>can:administrator | — | PersonalizeTemplateShowViewHelper<br>VaultIndexViewHelper | Settings/Personalize/Templates/Show | page | port: read (aggregate/list) | todo |
| R303 | POST | `/settings/personalize/templates/{template}` | `settings.personalize.template.template_page.store` | PersonalizeTemplatePagesController@store<br>can:administrator | CreateTemplatePage | PersonalizeTemplateShowViewHelper | (Settings/Personalize/Templates/Show) | json | port | todo |
| R304 | PUT | `/settings/personalize/templates/{template}/template_pages/{page}` | `settings.personalize.template.template_page.update` | PersonalizeTemplatePagesController@update<br>can:administrator | UpdateTemplatePage | PersonalizeTemplateShowViewHelper | (Settings/Personalize/Templates/Show) | json | port | todo |
| R305 | DELETE | `/settings/personalize/templates/{template}/template_pages/{page}` | `settings.personalize.template.template_page.destroy` | PersonalizeTemplatePagesController@destroy<br>can:administrator | DestroyTemplatePage | — | (Settings/Personalize/Templates/Show) | json | port | todo |
| R306 | GET | `/settings/personalize/templates/{template}/template_pages/{page}` | `settings.personalize.template.template_page.show` | PersonalizeTemplatePagesController@show<br>can:administrator | — | PersonalizeTemplatePageShowViewHelper | (Settings/Personalize/Templates/Show) | json | port | todo |
| R307 | POST | `/settings/personalize/templates/{template}/template_pages/{page}/order` | `settings.personalize.template.template_page.order.update` | PersonalizeTemplatePagePositionController@update<br>can:administrator | UpdateTemplatePagePosition | PersonalizeTemplateShowViewHelper | (Settings/Personalize/Templates/Show) | json | port | todo |
| R308 | POST | `/settings/personalize/templates/{template}/template_pages/{page}/modules` | `settings.personalize.template.template_page.module.store` | PersonalizeTemplatePageModulesController@store<br>can:administrator | AssociateModuleToTemplatePage | PersonalizeTemplatePageShowViewHelper | (Settings/Personalize/Templates/Show) | json | port | todo |
| R309 | POST | `/settings/personalize/templates/{template}/template_pages/{page}/modules/{module}/order` | `settings.personalize.template.template_page.module.order.update` | PersonalizeTemplatePageModulesPositionController@update<br>can:administrator | UpdateModulePosition | PersonalizeTemplatePageShowViewHelper | (Settings/Personalize/Templates/Show) | json | port | todo |
| R310 | DELETE | `/settings/personalize/templates/{template}/template_pages/{page}/modules/{module}` | `settings.personalize.template.template_page.module.destroy` | PersonalizeTemplatePageModulesController@destroy<br>can:administrator | RemoveModuleFromTemplatePage | PersonalizeTemplatePageShowViewHelper | (Settings/Personalize/Templates/Show) | json | port | todo |
| R311 | GET | `/settings/personalize/modules` | `settings.personalize.module.index` | PersonalizeModulesController@index<br>can:administrator | — | PersonalizeModuleIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/Modules/Index | page | port: read (aggregate/list) | todo |
| R312 | POST | `/settings/personalize/modules` | `settings.personalize.module.store` | PersonalizeModulesController@store<br>can:administrator | — | — | (Settings/Personalize/Modules/Index) | missing-action | remove: dead route (controller action does not exist) | todo (remove) |
| R313 | PUT | `/settings/personalize/modules/{module}` | `settings.personalize.module.update` | PersonalizeModulesController@update<br>can:administrator | — | — | (Settings/Personalize/Modules/Index) | missing-action | remove: dead route (controller action does not exist) | todo (remove) |
| R314 | DELETE | `/settings/personalize/modules/{module}` | `settings.personalize.module.destroy` | PersonalizeModulesController@destroy<br>can:administrator | — | — | (Settings/Personalize/Modules/Index) | missing-action | remove: dead route (controller action does not exist) | todo (remove) |
| R315 | GET | `/settings/personalize/currencies` | `settings.personalize.currency.index` | PersonalizeCurrencyController@index<br>can:administrator | — | PersonalizeCurrencyIndexViewHelper<br>VaultIndexViewHelper | Settings/Personalize/Currencies/Index | page | port: read (aggregate/list) | todo |
| R316 | PUT | `/settings/personalize/currencies/{currency}` | `settings.personalize.currency.update` | PersonalizeCurrencyController@update<br>can:administrator | ToggleCurrency | — | (Settings/Personalize/Currencies/Index) | json | port | todo |
| R317 | POST | `/settings/personalize/currencies` | `settings.personalize.currency.store` | PersonalizeCurrencyController@store<br>can:administrator | EnableAllCurrencies | — | (Settings/Personalize/Currencies/Index) | json | port | todo |
| R318 | DELETE | `/settings/personalize/currencies` | `settings.personalize.currency.destroy` | PersonalizeCurrencyController@destroy<br>can:administrator | DisableAllCurrencies | — | (Settings/Personalize/Currencies/Index) | json | port | todo |
| R319 | GET | `/settings/personalize/religions` | `settings.personalize.religions.index` | PersonalizeReligionController@index<br>can:administrator | — | PersonalizeReligionViewHelper<br>VaultIndexViewHelper | Settings/Personalize/Religions/Index | page | port: read (aggregate/list) | todo |
| R320 | POST | `/settings/personalize/religions` | `settings.personalize.religions.store` | PersonalizeReligionController@store<br>can:administrator | CreateReligion | PersonalizeReligionViewHelper | (Settings/Personalize/Religions/Index) | json | port | todo |
| R321 | PUT | `/settings/personalize/religions/{religion}` | `settings.personalize.religions.update` | PersonalizeReligionController@update<br>can:administrator | UpdateReligion | PersonalizeReligionViewHelper | (Settings/Personalize/Religions/Index) | json | port | todo |
| R322 | DELETE | `/settings/personalize/religions/{religion}` | `settings.personalize.religions.destroy` | PersonalizeReligionController@destroy<br>can:administrator | DestroyReligion | — | (Settings/Personalize/Religions/Index) | json | port | todo |
| R323 | POST | `/settings/personalize/religions/{religion}/position` | `settings.personalize.religions.order.update` | PersonalizeReligionsPositionController@update<br>can:administrator | UpdateReligionPosition | PersonalizeReligionViewHelper | (Settings/Personalize/Religions/Index) | json | port | todo |
| R324 | GET | `/settings/storage` | `settings.storage.index` | AccountStorageController@index<br>can:administrator | — | StorageIndexViewHelper<br>VaultIndexViewHelper | Settings/Storage/Index | page | port: read (aggregate/list) | done (test: `tests/account.rs`) |
| R325 | GET | `/settings/cancel` | `settings.cancel.index` | CancelAccountController@index<br>can:administrator | — | CancelAccountViewHelper<br>VaultIndexViewHelper | Settings/CancelAccount/Index | page | port: read (aggregate/list) | todo |
| R326 | PUT | `/settings/cancel` | `settings.cancel.destroy` | CancelAccountController@destroy<br>can:administrator | CancelAccount | — | (Settings/CancelAccount/Index) | json | port | done (test: `tests/account.rs`) |
| R327 | GET | `/currencies` | `currencies.index` | CurrencyController@index | — | CurrencyIndexViewHelper | (Vault/Contact/Show) | json | port | todo |
| R328 | DELETE | `/auth/{driver}` | `provider.delete` | UserTokenController@destroy | — | — | `Pages/Profile/Partials/UpdateProviders.vue` | redirect | remove: OAuth/Socialite removed | todo (remove) |
| R329 | GET | `/api/user` | `api.` | UserController@user | — | — | — | json | port | done (test: `tests/auth.rs`) |
| R330 | GET | `/api/users` | `api.users.index` | UserController@index | — | — | — | json | port | done (test: `tests/account.rs`) |
| R331 | GET | `/api/users/{user}` | `api.users.show` | UserController@show | — | — | — | json | port | done (test: `tests/account.rs`) |
| R332 | GET | `/api/vaults` | `api.vaults.index` | VaultController@index | — | — | — | json | port | done (test: `tests/vaults.rs`) |
| R333 | POST | `/api/vaults` | `api.vaults.store` | VaultController@store | CreateVault | — | — | json | port | done (test: `tests/vaults.rs`) |
| R334 | GET | `/api/vaults/{vault}` | `api.vaults.show` | VaultController@show | — | — | — | json | port | done (test: `tests/vaults.rs`) |
| R335 | PUT/PATCH | `/api/vaults/{vault}` | `api.vaults.update` | VaultController@update | UpdateVault | — | — | json | port | done (test: `tests/vaults.rs`) |
| R336 | DELETE | `/api/vaults/{vault}` | `api.vaults.destroy` | VaultController@destroy | DestroyVault | — | — | — | port | done (test: `tests/vaults.rs`) |
<!-- END GENERATED routes -->

## 3. Routes registered by vendor packages

These routes are not in `routes/*.php`; they come from Fortify, Jetstream,
Sanctum, laravel-webauthn, laravel-sabre and the dev tools. Fortify features
on: registration, password reset, email verification, profile information,
password update, 2FA (`confirm` and `confirmPassword` both true). Jetstream
features on: terms and privacy policy, API tokens. Profile photos, teams and
account deletion are off.

| ID | Method | URI | Name | Handler (app code) | Vue page | Disposition | Status |
|---|---|---|---|---|---|---|---|
| V001 | GET | `/login` | `login` | `LoginController::__invoke`: signup enabled, reset enabled, status, random wallpaper, providers, WebAuthn userless options, beta cookie, deletion notice | Auth/Login | port: SPA page, data from instance info endpoint | done (test: `tests/http_basics.rs`) |
| V002 | POST | `/login` | `login.store` | Fortify pipeline `RedirectIfTwoFactorAuthenticatable`, `AttemptToAuthenticate`, `PrepareAuthenticatedSession`; rate limit 5/min per email+IP | Auth/Login | port | done (test: `tests/auth.rs`) |
| V003 | POST | `/logout` | `logout` | Fortify | Layout | port | done (test: `tests/auth.rs`) |
| V004 | GET | `/forgot-password` | `password.request` | Fortify view | Auth/ForgotPassword | port: SPA page | todo |
| V005 | POST | `/forgot-password` | `password.email` | Fortify, sends reset link email | Auth/ForgotPassword | port | done (test: `tests/auth.rs`) |
| V006 | GET | `/reset-password/{token}` | `password.reset` | Fortify view | Auth/ResetPassword | port: SPA page | todo |
| V007 | POST | `/reset-password` | `password.update` | `App\Actions\Fortify\ResetUserPassword` | Auth/ResetPassword | port | done (test: `tests/auth.rs`) |
| V008 | GET | `/register` | `register` | `RegisterController::__invoke` (providers, beta cookie); `monica.signup_is_enabled` | Auth/Register | port: SPA page | done (test: `tests/http_basics.rs`) |
| V009 | POST | `/register` | `register.store` | `App\Actions\Fortify\CreateNewUser` (creates account via `CreateAccount`); `monica.signup_is_enabled` | Auth/Register | port | done (test: `tests/auth.rs`) |
| V010 | GET | `/email/verify` | `verification.notice` | Fortify view | Auth/VerifyEmail | port: SPA page | todo |
| V011 | GET | `/email/verify/{id}/{hash}` | `verification.verify` | Fortify, signed URL, throttle 6/min | — (email link) | port | done (test: `tests/auth.rs`) |
| V012 | POST | `/email/verification-notification` | `verification.send` | Fortify, `User::sendEmailVerificationNotification` | Auth/VerifyEmail | port | done (test: `tests/auth.rs`) |
| V013 | PUT | `/user/profile-information` | `user-profile-information.update` | `App\Actions\Fortify\UpdateUserProfileInformation` (name, email, locale; re-verify on email change) | Profile/Show | port | done (test: `tests/auth.rs`) |
| V014 | PUT | `/user/password` | `user-password.update` | `App\Actions\Fortify\UpdateUserPassword` | Profile/Show | port | done (test: `tests/auth.rs`) |
| V015 | GET | `/user/confirm-password` | `password.confirm` | Fortify view | Auth/ConfirmPassword | port: SPA page | todo |
| V016 | GET | `/user/confirmed-password-status` | `password.confirmation` | Fortify | `Components/Jetstream/ConfirmsPassword.vue` | port | done (test: `tests/auth.rs`) |
| V017 | POST | `/user/confirm-password` | `password.confirm.store` | Fortify, `FortifyServiceProvider` confirm callback | `Components/Jetstream/ConfirmsPassword.vue` | port | done (test: `tests/auth.rs`) |
| V018 | GET | `/two-factor-challenge` | `two-factor.login` | `App\Actions\Fortify\TwoFactorChallengeView` | Auth/TwoFactorChallenge | port: SPA page | todo |
| V019 | POST | `/two-factor-challenge` | `two-factor.login.store` | Fortify; rate limit 5/min per `login.id` | Auth/TwoFactorChallenge | port | done (test: `tests/auth.rs`) |
| V020 | POST | `/user/two-factor-authentication` | `two-factor.enable` | Fortify; password confirm required | Profile/Show | port | done (test: `tests/auth.rs`) |
| V021 | POST | `/user/confirmed-two-factor-authentication` | `two-factor.confirm` | Fortify | Profile/Show | port | done (test: `tests/auth.rs`) |
| V022 | DELETE | `/user/two-factor-authentication` | `two-factor.disable` | Fortify; password confirm required | Profile/Show | port | done (test: `tests/auth.rs`) |
| V023 | GET | `/user/two-factor-qr-code` | `two-factor.qr-code` | Fortify, SVG QR code | Profile/Show | port | done (test: `tests/auth.rs`) |
| V024 | GET | `/user/two-factor-secret-key` | `two-factor.secret-key` | Fortify | Profile/Show | port | done (test: `tests/auth.rs`) |
| V025 | GET | `/user/two-factor-recovery-codes` | `two-factor.recovery-codes` | Fortify | Profile/Show | port | done (test: `tests/auth.rs`) |
| V026 | POST | `/user/two-factor-recovery-codes` | — | Fortify, regenerate | Profile/Show | port | done (test: `tests/auth.rs`) |
| V027 | GET | `/terms-of-service` | `terms.show` | Jetstream, renders `resources/markdown/terms.md` | TermsOfService | port: static SPA page | todo |
| V028 | GET | `/privacy-policy` | `policy.show` | Jetstream, renders `resources/markdown/policy.md` | PrivacyPolicy | port: static SPA page | todo |
| V029 | GET | `/user/profile` | `profile.show` | Jetstream + `App\Actions\Jetstream\UserProfile` (providers, user tokens, WebAuthn keys, locales, layout); sessions list | Profile/Show | port, minus providers and WebAuthn | done (test: `tests/auth.rs`) |
| V030 | DELETE | `/user/other-browser-sessions` | `other-browser-sessions.destroy` | Jetstream, needs password | Profile/Show | port | done (test: `tests/auth.rs`) |
| V031 | DELETE | `/user/profile-photo` | `current-user-photo.destroy` | Jetstream; profile photos feature is off | — | remove: feature off, not reachable | todo (remove) |
| V032 | DELETE | `/user` | `current-user.destroy` | `App\Actions\Jetstream\DeleteUser`; registered only when account deletion is on (it is off) | Profile/Show (`DeleteUserForm`, hidden) | port behind a config flag that defaults to off (see §11, Q4) | done (test: `tests/auth.rs`) |
| V033 | GET | `/user/api-tokens` | `api-tokens.index` | Jetstream; permissions `read`, `write`, default `read` | API/Index | port: bearer tokens | done (test: `tests/auth.rs`) |
| V034 | POST | `/user/api-tokens` | `api-tokens.store` | Jetstream | API/Index | port | done (test: `tests/auth.rs`) |
| V035 | PUT | `/user/api-tokens/{token}` | `api-tokens.update` | Jetstream | API/Index | port | done (test: `tests/auth.rs`) |
| V036 | DELETE | `/user/api-tokens/{token}` | `api-tokens.destroy` | Jetstream | API/Index | port | done (test: `tests/auth.rs`) |
| V037 | GET | `/sanctum/csrf-cookie` | `sanctum.csrf-cookie` | Sanctum | `bootstrap.js` (axios) | port: replaced by the API's CSRF scheme | done (test: `tests/auth.rs`) |
| V038 | ANY (15 verbs incl. PROPFIND, REPORT) | `/dav/{path?}` | `sabre.dav` | laravel-sabre + `DAVServiceProvider`: principals, CardDAV, CalDAV, VCF/ICS export, sync (RFC 6578), ACL; auth by `AuthenticateWithTokenOnBasicAuth` (basic auth, or bearer with matching email); middleware `abilities:read,write` | — (DAV clients) | port | todo |
| V039 | GET | `/up` | — | Laravel health route | — | port | todo |
| V040 | GET | `/webauthn/auth` | `webauthn.login` | laravel-webauthn | Webauthn/WebauthnLogin | remove: WebAuthn | todo (remove) |
| V041 | POST | `/webauthn/auth/options` | `webauthn.auth.options` | laravel-webauthn | Auth/Login, Webauthn/WebauthnLogin | remove: WebAuthn | todo (remove) |
| V042 | POST | `/webauthn/auth` | `webauthn.auth` | laravel-webauthn + `AttemptToAuthenticateWebauthn` | Auth/Login | remove: WebAuthn | todo (remove) |
| V043 | GET | `/webauthn/keys/create` | `webauthn.create` | laravel-webauthn | Webauthn/WebauthnKeys | remove: WebAuthn | todo (remove) |
| V044 | POST | `/webauthn/keys/options` | `webauthn.store.options` | laravel-webauthn | Webauthn/Partials/RegisterKey | remove: WebAuthn | todo (remove) |
| V045 | POST | `/webauthn/keys` | `webauthn.store` | laravel-webauthn | Webauthn/Partials/RegisterKey | remove: WebAuthn | todo (remove) |
| V046 | DELETE | `/webauthn/keys/{id}` | `webauthn.destroy` | laravel-webauthn | Webauthn/Partials/DeleteKeyModal | remove: WebAuthn | todo (remove) |
| V047 | PUT | `/webauthn/keys/{id}` | `webauthn.update` | laravel-webauthn | Webauthn/Partials/UpdateKey | remove: WebAuthn | todo (remove) |
| V048 | POST | `/webauthn/confirm-key` | `webauthn.key.confirm` | laravel-webauthn | `Components/Jetstream/ConfirmsPassword.vue` | remove: WebAuthn | todo (remove) |
| V049 | POST | `/sentry/tunnel` | — | laravel-sentry-tunnel | `sentry.js` | remove: Sentry | todo (remove) |
| V050 | GET | `/pulse` (+ Livewire) | — | Laravel Pulse | — | remove: Pulse | todo (remove) |
| V051 | GET | `/telescope/*` | — | Laravel Telescope | — | remove: Telescope | todo (remove) |
| V052 | GET | `/docs`, `/docs.postman`, `/docs.openapi` | — | Scribe | — | remove: Scribe (the utoipa spec replaces it) | todo (remove) |
| V053 | GET | `/_debugbar/*` | — | Debugbar (dev only) | — | remove: Debugbar | todo (remove) |

V001–V039 are ported except V031. V004, V006, V010, V015, V018, V027 and V028
only render a page and become SPA routes; the rest become API endpoints.
V031 and V040–V053 are removed.

## 4. Scheduled tasks and queued jobs

From `routes/console.php` and the `Jobs` folders. Queue connection defaults to
`sync`; `database` uses the `jobs`, `failed_jobs` and `job_batches` tables.

| ID | Schedule | Job / command | What it does | Disposition | Status |
|---|---|---|---|---|---|
| J001 | every minute | `ProcessScheduledContactReminders` | Sends due `contact_reminder_scheduled` rows to each user notification channel (email; Telegram is removed), reschedules recurring reminders, counts failures and disables a channel after `monica.max_notification_failures` (10) | port | todo |
| J002 | hourly | `DavClient\Jobs\UpdateAddressBooks` | For each due `addressbook_subscriptions` row (by `frequency`), dispatches `SynchronizeAddressBooks` | port | todo |
| J003 | daily | `Dav\Jobs\CleanSyncToken` | Deletes old sync tokens, keeping the latest per user/name | port | todo |
| J004 | daily | `Logging\CleanLogs` | Deletes old `logs` rows | port | todo |
| J005 | daily | `model:prune` | No model in `app/Models` is `Prunable`; Sanctum/vendor models only | port: nothing app-specific to prune; keep a daily prune hook for expired sessions, tokens and password resets | todo |
| J006 | daily | `queue:prune-batches --hours=48 --unfinished=72 --cancelled=72` | Laravel queue housekeeping | port: same retention on the Rust job tables | todo |
| J007 | daily | `queue:prune-failed --hours=48` | Laravel queue housekeeping | port: same retention | todo |
| J008 | daily (if enabled) | `telescope:prune` | Telescope housekeeping | remove: Telescope | todo (remove) |
| J009 | queued | `DavClient\Jobs\SynchronizeAddressBooks` | Runs `AddressBookSynchronizer` for one subscription (sync-collection or full sync, push local changes) | port | todo |
| J010 | queued | `DavClient\Jobs\GetVCard`, `GetMultipleVCard` | Fetch remote vCards and import them (`ImportVCard`) | port | todo |
| J011 | queued | `DavClient\Jobs\PushVCard` | PUT a local contact to the remote address book | port | todo |
| J012 | queued | `DavClient\Jobs\DeleteVCard`, `DeleteMultipleVCard`, `DeleteLocalVCard` | Remote/local deletions during sync | port | todo |
| J013 | queued | `Dav\Jobs\UpdateVCard`, `UpdateVCalendar` | Server side: apply a PUT vCard / vCalendar from a DAV client | port | todo |
| J014 | queued | `CreateAccount\Jobs\SetupAccount` | Seeds a new account: default templates, pages, modules, genders, pronouns, relationship types, call reasons, gift states/occasions, post templates, religions, currencies, life event categories/types, emotions, address types, contact information types, pet categories, group types, a default vault user notification channel | port | todo |
| J015 | queued | `ManageNotificationChannels\Jobs\SendVerificationEmailChannel` | Emails the verification link for a new email channel | port | todo |
| J016 | queued | `ManageContact\Services\DestroyContact` (QueuableService) | Contact deletion dispatched from `ContactController@destroy` | port | todo |
| J017 | queued | `ManageGroups\Services\DestroyGroup`, `AddContactToGroup`, `RemoveContactFromGroup` | Dispatched from group controllers | port | todo |
| J018 | queued | `CancelAccount\Services\CancelAccount` | Dispatched from `CancelAccountController@destroy` | port | todo |
| J019 | queued | `laravel/scout` index jobs | Search index updates | remove: Scout drivers (DB search kept, §8 X019) | todo (remove) |

## 5. Console commands

| ID | Command | Class | What it does | Disposition | Status |
|---|---|---|---|---|---|
| K001 | `monica:getversion` | `GetVersion` | Prints the version | port: `--version` flag on the binary | todo |
| K002 | `monica:setup` | `SetupApplication` | Install/update: migrations, cache, storage link, optional Scout setup | port: migrations on start and a `migrate` subcommand | todo |
| K003 | `monica:newaddressbooksubscription` | `NewAddressBookSubscription` | Adds a CardDAV client subscription for a user (`CreateAddressBookSubscription`) | port | todo |
| K004 | `test:send-reminders` | `TestReminders` | Sends all scheduled reminders now | port | todo |
| K005 | `waitfordb` | `WaitForDb` | Waits for the DB connection | port: retry on start | todo |
| K006 | `monica:dummy` | `Local\SetupDummyAccount` | Seeds a demo account with fake data (local only) | port (dev tool, behind a subcommand) | todo |
| K007 | `monica:updateaddressbooksubscription` | `Local\UpdateAddressBookSubscription` | Edits a subscription (local only) | port | todo |
| K008 | `monica:localize` | `Local\MonicaLocalize` | Extracts and machine-translates strings into `lang/*.json` (Google Translate) | port: kept as a dev script for the frontend locale files; not part of the server | todo |
| K009 | `scout:setup` | `SetupScout` | Creates Meilisearch/Typesense indexes | remove: search engines | todo (remove) |
| K010 | `monica:documentation` | `SetupDocumentation` | Runs Scribe | remove: Scribe | todo (remove) |
| K011 | `migrate`, `db:seed` | framework | Migrations; `database/seeders` | port: `migrate` subcommand | todo |
| K012 | `queue:work` | framework | Queue worker | port: in-process worker | todo |
| K013 | `schedule:run` / `schedule:work` | framework + `App\Console\Scheduling` (`CronEvent`, `crons` table) | Scheduler with `crons` table for last run | port: in-process scheduler that uses the `crons` table | todo |
| K014 | `telescope:*`, `pulse:*` | vendor | dev tools | remove | todo (remove) |
| K015 | `scribe:*` | vendor | docs | remove | todo (remove) |
| K016 | `ide-helper:*`, `debugbar:*` | vendor (dev) | dev tools | remove | todo (remove) |

## 6. Events, mail and notifications

| ID | Kind | Class | Trigger | Disposition | Status |
|---|---|---|---|---|---|
| E001 | notification | `Notifications\ReminderTriggered` | J001; mail channel writes a `user_notification_sent` row; template `emails/notifications/reminder.blade.php` | port (mail only) | todo |
| E002 | mail | `Mail\UserInvited` | `InviteUser` service; `emails/user/invitation.blade.php` | port | todo |
| E003 | mail | `Mail\TestEmailSent` | `SendTestEmail` service; `emails/notifications/test-notification.blade.php` | port | todo |
| E004 | mail | `Mail\UserNotificationChannelEmailCreated` | J015; `emails/notifications/validate-email.blade.php` | port | todo |
| E005 | notification | Laravel `VerifyEmail` | Registration and email change. `User::sendEmailVerificationNotification` marks the user verified without sending when the mailer is SMTP with no username/password | port | todo |
| E006 | notification | Laravel `ResetPassword` | V005 | port | todo |
| E007 | listener | `Listeners\LoginListener` | On login with "remember" and WebAuthn keys, sets `return` cookie | remove: WebAuthn | todo (remove) |
| E008 | listener | `Listeners\WebauthnAuthenticateListener` | WebAuthn login | remove: WebAuthn | todo (remove) |
| E009 | listener | `Domains\Vault\ManageFiles\Listeners\DeleteFileInStorage` on `FileDeleted` | Deletes the stored file when a `files` row is deleted | port: delete from the local data directory | todo |

## 7. External integrations

| ID | Integration | Where | Disposition | Status |
|---|---|---|---|---|
| I001 | SMTP (and the other Laravel mail drivers: mailgun, postmark, ses, log) | `config/mail.php`, `config/services.php` | port: SMTP plus a log/file transport for tests | todo |
| I002 | CardDAV/CalDAV server (sabre/dav via monicahq/laravel-sabre) | V038, `app/Domains/Contact/Dav` | port | todo |
| I003 | CardDAV client (Guzzle + sabre/vobject, DNS SRV discovery via the `Http::getDnsRecord` macro) | `app/Domains/Contact/DavClient` | port | todo |
| I004 | Wikipedia API (`WikipediaHelper`) | City and country report pages (`ReportCitiesShowViewHelper`, `ReportCountriesShowViewHelper`) fetch a summary and thumbnail from `{lang}.wikipedia.org` | **open question Q1** (§11): plan says no external services except SMTP | todo |
| I005 | Uploadcare (`uploadcare-php`, `Uploadcare.vue`, CDN transforms) | photos, documents, avatars, slice cover, post photos | remove: replaced by local storage (§8 X020) | todo (remove) |
| I006 | Mapbox static maps | R083, `MapHelper` | remove | todo (remove) |
| I007 | LocationIQ geocoding | `GetGPSCoordinate` (called from `CreateAddress`, `UpdateAddress`) | remove | todo (remove) |
| I008 | Socialite OAuth (Azure, Facebook, GitHub, Google, LinkedIn, Kanidm, Keycloak, SAML2) | R006–R008, R328, `UserToken`, `AttemptToAuthenticateSocialite` | remove | todo (remove) |
| I009 | Telegram bot (`laravel-notification-channels/telegram`) | R011, R223, `SendTestTelegramNotification`, `Telegram.vue` | remove | todo (remove) |
| I010 | Meilisearch / Typesense (Scout) | `config/scout.php`, `ScoutHelper`, model `Searchable` traits | remove; the Scout `database` driver behaviour is kept (X019) | todo (remove) |
| I011 | Redis / memcached cache, queue and session drivers | `config/cache.php`, `config/queue.php`, `config/session.php` | remove: DB-backed or in-process only | todo (remove) |
| I012 | Sentry, Nightwatch, Pulse, Telescope, Debugbar, Clockwork config, Scribe | `config/*.php`, `sentry.js`, `app.js`, `ssr.js` | remove | todo (remove) |
| I013 | GeoIP (`stevebauman/location`) | in `composer.json`, no call sites in `app/` or `config/` | remove: unused dependency | todo (remove) |

## 8. Cross-cutting behaviour

Behaviour that lives outside a single route or service and has to be rebuilt.

| ID | Behaviour | Source | Status |
|---|---|---|---|
| X001 | Authorization gates `administrator`, `vault-viewer` (any permission), `vault-editor` (≤200), `vault-manager` (≤100), `contact-owner`, `group-owner`, `journal-owner`, `post-owner`, `sliceOfLife-owner` | `AuthServiceProvider` | todo |
| X002 | Service permission checks `author_must_belong_to_account`, `author_must_be_account_administrator`, `vault_must_belong_to_account`, `author_must_be_vault_manager/editor`, `author_must_be_in_vault`, `contact_must_belong_to_vault`, `group_must_belong_to_vault`, with their dependency rules | `BaseService` | todo |
| X003 | Validation rules per service (`rules()`), e.g. `uuid`, `exists:table,id`, `max:255`, `max:65535`, date formats | every service | todo |
| X004 | Exceptions → HTTP: `NotEnoughPermissionException`, `ModelNotFoundException`, `ValidationException`, `CantBeDeletedException`, `EntryAlreadyExistException`, `MaximumNumberOfUsersInVaultException`, `SameUserException`, `EnvVariablesNotSetException` | `app/Exceptions` | todo |
| X005 | Rate limits: `api` 60/min per user or IP, `login` 5/min per email+IP, `two-factor` 5/min per `login.id`, email verification 6/min | `AppServiceProvider`, `FortifyServiceProvider` | todo |
| X006 | Password rules: production min 8 with letters, mixed case, numbers and symbols; otherwise min 4 | `AppServiceProvider::boot` `Password::defaults` | todo |
| X007 | Signup can be disabled (`APP_DISABLE_SIGNUP`); first user always allowed | `SignupHelper`, `EnsureSignupIsEnabled` | todo |
| X008 | Instance deletion notice on login (`APP_SHOW_DELETION_NOTICE`), beta banner cookie | `LoginController`, R002 | todo |
| X009 | Locale selection: user `locale`, else browser `Accept-Language` within `localizer.supported_locales`, stored in session/cookie | `codezero/laravel-localizer` `SetLocale` middleware, `config/localizer.php` | todo |
| X010 | Instance administrator (`is_instance_administrator`) | `2023_06_12_093907_add_instance_administrator`; used for Pulse gate only | todo |
| X011 | Account storage limit (`DEFAULT_STORAGE_LIMIT`, 50 MB) and `StorageHelper::canUploadFile` | `StorageHelper`, `AccountStorageController` | todo |
| X012 | Contact feed items written by services (`ContactFeedItem::ACTION_*`) and rendered by `ManageContactFeed/Web/ViewHelpers/Actions/*` | services, feed ViewHelpers | todo |
| X013 | `contacts.last_updated_at` bumped by every contact-module service | services | todo |
| X014 | Most consulted contacts (`contact_vault_user.number_of_views`), `UpdateContactView` on contact show (R041) | R209, `VaultMostConsultedContactsController` | todo |
| X015 | Contact name formatting by user `name_order` (`NameHelper`), date formatting by user `date_format` and timezone (`DateHelper`), number formatting (`MonetaryNumberHelper`, `number_format`), distance unit (`DistanceHelper`) | helpers | todo: the API returns raw values; formatting moves to the frontend |
| X016 | Markdown rendering for notes, posts, goals etc. (`Str::markdownExternalLink` macro) | `AppServiceProvider` | todo: move to the frontend or return both raw and rendered? see Q3 |
| X017 | Collation-aware sorting (`sortByCollator`) | `AppServiceProvider` | todo |
| X018 | Pagination (`PaginatorHelper::getData`) | helpers | todo |
| X019 | Full-text search across contacts, notes, groups, loans (Scout `database` engine: `LIKE` on `toSearchableArray` columns) | models `Searchable`, `VaultSearchController`, `VaultContactSearchController` | todo |
| X020 | File storage: today `files` rows hold Uploadcare uuid, CDN URL, mime type, size, type (photo/document/avatar); `FileHelper` formats sizes | `ManageFiles`, `ManagePhotos`, `ManageDocuments`, `ManageAvatar`, journals | todo: local storage, thumbnails, serving endpoint |
| X021 | Default avatar: MultiAvatar SVG generated from the contact name (`App\Models\MultiAvatar`, `AvatarHelper`) | models, helpers | todo |
| X022 | Vault tab visibility flags and default dashboard tab | `UpdateVaultTabVisibility` (R190), R019 | todo |
| X023 | Contact templates: template → pages → modules decide which modules a contact page shows | `ManageTemplates`, `ContactShowViewHelper` | todo |
| X024 | DAV sync tokens (`synctokens`), `distant_uri`/`distant_etag` on contacts and groups, vCard/iCal export and import (`ExportVCard`, `ImportVCard`, `ExportVCalendar`, `ImportVCalendar`, `DAVFormat` trait) | `app/Domains/Contact/Dav` | todo |
| X025 | vCard download of a contact (R045) | `ContactVCardController` | todo |
| X026 | Sessions: DB session table, "logout other browser sessions" | `sessions` table, V030 | todo |
| X027 | API tokens with abilities `read` / `write` (Sanctum `personal_access_tokens`) | Jetstream, V033–V036 | todo |

### 8b. Reference data

| ID | Data | Source | Status |
|---|---|---|---|
| D001 | Countries (names per locale, ISO codes) | `rinvex/countries` | todo |
| D002 | ISO-639 languages | `matriphe/iso-639` | todo |
| D003 | Currencies (code list + per-locale names) | `currencies` table seeded in migration `2022_03_22_180407_create_currencies_table`, `lang/*/currencies.php`, `moneyphp/money` for formatting | todo |
| D004 | Timezones | PHP `DateTimeZone::listIdentifiers` (preferences page) | todo |
| D005 | Translations, 29 locales (`lang/*.json`, `lang/*/*.php`) | `lang/` | todo |
| D006 | Emotions seeded per account | `SetupAccount` | todo |
| D007 | Default account data (templates, modules, life event types, …), translated at creation | `SetupAccount` (J014) | todo |
| D008 | Login wallpapers `public/img/image1..20.png` | `WallpaperHelper` | todo (frontend asset) |
| D009 | Help links (`config/monica.php` `help_links`, `help_center_url`) | config | todo (frontend constant) |

## 9. PHP classes

Every class under `app/` except the controllers that routes point at and the
ViewHelpers (both covered by §2). `PHP tests` lists the test files with the
same base name; phase 3 ports each service together with these tests.
`port, drop …` marks classes that keep their behaviour minus the parts that
belong to removed features.

<!-- BEGIN GENERATED classes -->
| ID | Kind | Class | PHP tests | Used by | Disposition | Status |
|---|---|---|---|---|---|---|
| C001 | action | `app/Actions/AttemptToAuthenticateSocialite.php` | — | SocialiteCallbackController | remove: OAuth/Socialite removed | todo (remove) |
| C002 | action | `app/Actions/AttemptToAuthenticateWebauthn.php` | Unit/Actions/AttemptToAuthenticateWebauthnTest.php | — | remove: WebAuthn removed | todo (remove) |
| C003 | action | `app/Actions/Fortify/CreateNewUser.php` | — | AttemptToAuthenticateSocialite, FortifyServiceProvider | port | todo |
| C004 | action | `app/Actions/Fortify/PasswordValidationRules.php` | — | CreateAccount, CreateNewUser, ResetUserPassword, UpdateUserPassword | port | todo |
| C005 | action | `app/Actions/Fortify/RedirectIfTwoFactorAuthenticatable.php` | — | — | port, drop the WebAuthn branch | todo |
| C006 | action | `app/Actions/Fortify/ResetUserPassword.php` | — | FortifyServiceProvider | port | todo |
| C007 | action | `app/Actions/Fortify/TwoFactorChallengeView.php` | — | FortifyServiceProvider | remove: Inertia/Ziggy removed (SPA) | todo (remove) |
| C008 | action | `app/Actions/Fortify/UpdateUserPassword.php` | — | FortifyServiceProvider | port | todo |
| C009 | action | `app/Actions/Fortify/UpdateUserProfileInformation.php` | — | FortifyServiceProvider | port | todo |
| C010 | action | `app/Actions/Jetstream/DeleteUser.php` | — | JetstreamServiceProvider | port | todo |
| C011 | action | `app/Actions/Jetstream/UserProfile.php` | Unit/Actions/Jetstream/UserProfileTest.php | JetstreamServiceProvider | remove: Inertia/Ziggy removed (SPA) | todo (remove) |
| C012 | command | `app/Console/Commands/GetVersion.php` | Unit/Commands/GetVersionTest.php | — | port | todo |
| C013 | command | `app/Console/Commands/Local/MonicaLocalize.php` | — | — | port | todo |
| C014 | command | `app/Console/Commands/Local/SetupDummyAccount.php` | — | — | port, drop Scout/Uploadcare bits | todo |
| C015 | command | `app/Console/Commands/Local/UpdateAddressBookSubscription.php` | — | — | port | todo |
| C016 | command | `app/Console/Commands/NewAddressBookSubscription.php` | — | — | port | todo |
| C017 | command | `app/Console/Commands/SetupApplication.php` | — | — | port, drop Scout setup | todo |
| C018 | command | `app/Console/Commands/SetupDocumentation.php` | Unit/Commands/SetupDocumentationTest.php | — | remove: Scribe removed | todo (remove) |
| C019 | command | `app/Console/Commands/SetupScout.php` | — | — | remove: Meilisearch/Typesense removed (DB search kept) | todo (remove) |
| C020 | command | `app/Console/Commands/TestReminders.php` | — | — | port, drop Telegram | todo |
| C021 | command | `app/Console/Commands/WaitForDb.php` | Unit/Commands/WaitForDbTest.php | — | port | todo |
| C022 | command | `app/Console/Scheduling/CronEvent.php` | Unit/Commands/Scheduling/CronEventTest.php | Schedule | port | todo |
| C023 | command | `app/Console/Scheduling/Schedule.php` | — | — | port | todo |
| C024 | dav | `app/Domains/Contact/Dav/Event/TokenDeleteEvent.php` | — | CleanSyncToken | port | todo |
| C025 | dav | `app/Domains/Contact/Dav/ExportVCalendarResource.php` | — | ExportCalendarImportantDates, ExportContactTask, ExportVCalendar | port | todo |
| C026 | dav | `app/Domains/Contact/Dav/ExportVCardResource.php` | — | ExportAddress, ExportContactInformation, ExportGender, ExportImportantDates, ExportKind, ExportLabels, ExportMembers, ExportNames, ExportNames, ExportTimesta… | port | todo |
| C027 | dav | `app/Domains/Contact/Dav/Exporter.php` | — | ExportCalendarImportantDates, ExportContactInformation, ExportContactTask, ExportImportantDates, ExportKind, ExportLabels, ExportMembers, ExportNames, Export… | port | todo |
| C028 | dav | `app/Domains/Contact/Dav/IDavResource.php` | — | AbstractCalDAVBackend, CalDAVDates, CalDAVTasks, CardDAVBackend, SyncDAVBackend, VCalendarResource, VCardResource | port | todo |
| C029 | dav | `app/Domains/Contact/Dav/ImportResource.php` | — | — | port | todo |
| C030 | dav | `app/Domains/Contact/Dav/ImportVCalendarResource.php` | — | ImportCalendarContactImportantDates, ImportContactTask, ImportVCalendar, VCalendarImporter | port | todo |
| C031 | dav | `app/Domains/Contact/Dav/ImportVCardResource.php` | — | ImportAddress, ImportContact, ImportContactInformation, ImportGroup, ImportImportantDates, ImportLabels, ImportMembers, ImportVCard, Importer | port | todo |
| C032 | dav | `app/Domains/Contact/Dav/Importer.php` | — | ImportAddress, ImportContact, ImportContactInformation, ImportGroup, ImportImportantDates, ImportLabels, ImportMembers | port | todo |
| C033 | job | `app/Domains/Contact/Dav/Jobs/CleanSyncToken.php` | Unit/Domains/Contact/DAV/Jobs/CleanSyncTokenTest.php | — | port | todo |
| C034 | job | `app/Domains/Contact/Dav/Jobs/UpdateVCalendar.php` | Unit/Domains/Contact/DAV/Jobs/UpdateVCalendarTest.php | AbstractCalDAVBackend | port | todo |
| C035 | job | `app/Domains/Contact/Dav/Jobs/UpdateVCard.php` | Unit/Domains/Contact/DAV/Jobs/UpdateVCardTest.php | CardDAVBackend, GetMultipleVCard, GetVCard | port | todo |
| C036 | dav | `app/Domains/Contact/Dav/Order.php` | — | ExportAddress, ExportCalendarImportantDates, ExportContactInformation, ExportContactTask, ExportGender, ExportImportantDates, ExportKind, ExportLabels, Expor… | port | todo |
| C037 | service | `app/Domains/Contact/Dav/Services/ExportVCalendar.php` | Unit/Domains/Contact/DAV/Services/ExportVCalendarTest.php | CalDAVDates, CalDAVTasks | port | todo |
| C038 | service | `app/Domains/Contact/Dav/Services/ExportVCard.php` | Unit/Domains/Contact/DAV/Services/ExportVCardTest.php | CardDAVBackend, ContactVCardController | port | todo |
| C039 | service | `app/Domains/Contact/Dav/Services/GetEtag.php` | Unit/Domains/Contact/DAV/Services/GetEtagTest.php | AbstractCalDAVBackend, CardDAVBackend, UpdateVCalendar, UpdateVCard | port | todo |
| C040 | service | `app/Domains/Contact/Dav/Services/ImportVCalendar.php` | — | ImportVCalendarResource, UpdateVCalendar, VCalendarImporter | port | todo |
| C041 | service | `app/Domains/Contact/Dav/Services/ImportVCard.php` | Unit/Domains/Contact/DAV/Services/ImportVCardTest.php | ImportVCardResource, Importer, UpdateVCard | port | todo |
| C042 | service | `app/Domains/Contact/Dav/Services/ReadVObject.php` | — | CalDAVDates, CalDAVTasks, CardDAVBackend, ExportVCalendar, ExportVCard, ImportVCalendar, ImportVCard | port | todo |
| C043 | dav | `app/Domains/Contact/Dav/VCalendarImporter.php` | — | ImportCalendarContactImportantDates, ImportContactTask | port | todo |
| C044 | dav | `app/Domains/Contact/Dav/VCalendarResource.php` | — | AbstractCalDAVBackend, ContactImportantDate, ContactTask, ExportVCalendar, ExportVCalendarResource, GetEtag, ImportCalendarContactImportantDates, ImportConta… | port | todo |
| C045 | dav | `app/Domains/Contact/Dav/VCardResource.php` | — | AddressBookSynchronizer, CardDAVBackend, Contact, ExportVCard, ExportVCardResource, GetEtag, Group, ImportAddress, ImportContact, ImportContactInformation, I… | port | todo |
| C046 | dav | `app/Domains/Contact/Dav/Web/Auth/AuthBackend.php` | — | DAVServiceProvider | port | todo |
| C047 | dav | `app/Domains/Contact/Dav/Web/Backend/CalDAV/AbstractCalDAVBackend.php` | — | CalDAVBackend, CalDAVDates, CalDAVTasks | port | todo |
| C048 | dav | `app/Domains/Contact/Dav/Web/Backend/CalDAV/CalDAVBackend.php` | — | DAVServiceProvider | port | todo |
| C049 | dav | `app/Domains/Contact/Dav/Web/Backend/CalDAV/CalDAVDates.php` | Unit/Domains/Contact/DAV/Services/CalDAVDatesTest.php | CalDAVBackend, ImportCalendarContactImportantDates | port | todo |
| C050 | dav | `app/Domains/Contact/Dav/Web/Backend/CalDAV/CalDAVTasks.php` | Unit/Domains/Contact/DAV/Services/CalDAVTasksTest.php | CalDAVBackend, ImportContactTask | port | todo |
| C051 | dav | `app/Domains/Contact/Dav/Web/Backend/CalDAV/ICalDAVBackend.php` | — | AbstractCalDAVBackend | port | todo |
| C052 | dav | `app/Domains/Contact/Dav/Web/Backend/CardDAV/AddressBook.php` | — | AddressBookHome | port | todo |
| C053 | dav | `app/Domains/Contact/Dav/Web/Backend/CardDAV/AddressBookHome.php` | — | AddressBookRoot | port | todo |
| C054 | dav | `app/Domains/Contact/Dav/Web/Backend/CardDAV/AddressBookRoot.php` | — | DAVServiceProvider | port | todo |
| C055 | dav | `app/Domains/Contact/Dav/Web/Backend/CardDAV/CardDAVBackend.php` | — | AddressBook, DAVServiceProvider, DeleteLocalVCard, HasSubscription, ImportContact, UpdateSubscriptionLocalSyncToken | port | todo |
| C056 | dav | `app/Domains/Contact/Dav/Web/Backend/GetVaults.php` | — | CalDAVBackend, CardDAVBackend | port | todo |
| C057 | dav | `app/Domains/Contact/Dav/Web/Backend/IDAVBackend.php` | — | AbstractCalDAVBackend, CalDAVBackend, CardDAVBackend | port | todo |
| C058 | dav | `app/Domains/Contact/Dav/Web/Backend/SyncDAVBackend.php` | — | AbstractCalDAVBackend, CardDAVBackend | port | todo |
| C059 | dav | `app/Domains/Contact/Dav/Web/Backend/WithUser.php` | — | AbstractCalDAVBackend, CalDAVBackend, CardDAVBackend | port | todo |
| C060 | dav | `app/Domains/Contact/Dav/Web/DAVACL/PrincipalBackend.php` | — | AbstractCalDAVBackend, AuthBackend, CardDAVBackend, DAVServiceProvider | port | todo |
| C061 | dav | `app/Domains/Contact/Dav/Web/DAVRedirect.php` | — | DAVServiceProvider | port | todo |
| C062 | job | `app/Domains/Contact/DavClient/Jobs/DeleteLocalVCard.php` | Unit/Domains/Contact/DavClient/Jobs/DeleteLocalVCardTest.php | DeleteMultipleVCard, PrepareJobsContactUpdater | port | todo |
| C063 | job | `app/Domains/Contact/DavClient/Jobs/DeleteMultipleVCard.php` | Unit/Domains/Contact/DavClient/Jobs/DeleteMultipleVCardTest.php | PrepareJobsContactUpdater | port | todo |
| C064 | job | `app/Domains/Contact/DavClient/Jobs/DeleteVCard.php` | Unit/Domains/Contact/DavClient/Jobs/DeleteVCardTest.php | AddressBookSynchronizer, PrepareJobsContactPush | port | todo |
| C065 | job | `app/Domains/Contact/DavClient/Jobs/GetMultipleVCard.php` | Unit/Domains/Contact/DavClient/Jobs/GetMultipleVCardTest.php | PrepareJobsContactUpdater | port | todo |
| C066 | job | `app/Domains/Contact/DavClient/Jobs/GetVCard.php` | Unit/Domains/Contact/DavClient/Jobs/GetVCardTest.php | PrepareJobsContactUpdater | port | todo |
| C067 | job | `app/Domains/Contact/DavClient/Jobs/PushVCard.php` | Unit/Domains/Contact/DavClient/Jobs/PushVCardTest.php | AddressBookSynchronizer, PrepareJobsContactPush, PrepareJobsContactPushMissed | port | todo |
| C068 | job | `app/Domains/Contact/DavClient/Jobs/SynchronizeAddressBooks.php` | Unit/Domains/Contact/DavClient/Jobs/SynchronizeAddressBooksTest.php | NewAddressBookSubscription, UpdateAddressBookSubscription, UpdateAddressBooks | port | todo |
| C069 | job | `app/Domains/Contact/DavClient/Jobs/UpdateAddressBooks.php` | Unit/Domains/Contact/DavClient/Jobs/UpdateAddressBooksTest.php | — | port | todo |
| C070 | service | `app/Domains/Contact/DavClient/Services/CreateAddressBookSubscription.php` | — | NewAddressBookSubscription | port | todo |
| C071 | service | `app/Domains/Contact/DavClient/Services/SynchronizeAddressBook.php` | Unit/Domains/Contact/DavClient/Services/SynchronizeAddressBookTest.php | SynchronizeAddressBooks | port | todo |
| C072 | service | `app/Domains/Contact/DavClient/Services/UpdateSubscriptionLocalSyncToken.php` | Unit/Domains/Contact/DavClient/Services/UpdateSubscriptionLocalSyncTokenTest.php | AddressBookSynchronizer | port | todo |
| C073 | service | `app/Domains/Contact/DavClient/Services/Utils/AddressBookGetter.php` | Unit/Domains/Contact/DavClient/Services/Utils/AddressBookGetterTest.php | CreateAddressBookSubscription | port | todo |
| C074 | service | `app/Domains/Contact/DavClient/Services/Utils/AddressBookSynchronizer.php` | Unit/Domains/Contact/DavClient/Services/Utils/AddressBookSynchronizerTest.php | SynchronizeAddressBook | port | todo |
| C075 | service | `app/Domains/Contact/DavClient/Services/Utils/Dav/DavClient.php` | Unit/Domains/Contact/DavClient/Services/Utils/Dav/DavClientTest.php | AddressBookSubscription, AddressBookSynchronizer, CreateAddressBookSubscription, DavClientException, DavServerNotCompliantException, HasClient, ServiceUrlQuery | port | todo |
| C076 | service | `app/Domains/Contact/DavClient/Services/Utils/Dav/DavClientException.php` | — | AddressBookGetter, CreateAddressBookSubscription, DavClient, DavServerNotCompliantException | port | todo |
| C077 | service | `app/Domains/Contact/DavClient/Services/Utils/Dav/DavServerNotCompliantException.php` | — | AddressBookGetter | port | todo |
| C078 | service | `app/Domains/Contact/DavClient/Services/Utils/Dav/ServiceUrlQuery.php` | Unit/Domains/Contact/DavClient/Services/Utils/Dav/ServiceUrlQueryTest.php | DavClient | port | todo |
| C079 | service | `app/Domains/Contact/DavClient/Services/Utils/Model/ContactDeleteDto.php` | — | AddressBookSynchronizer, PrepareJobsContactUpdater | port | todo |
| C080 | service | `app/Domains/Contact/DavClient/Services/Utils/Model/ContactDto.php` | — | AddressBookSynchronizer, ContactDeleteDto, GetVCard, PrepareJobsContactPush, PrepareJobsContactPushMissed, PrepareJobsContactUpdater | port | todo |
| C081 | service | `app/Domains/Contact/DavClient/Services/Utils/PrepareJobsContactPush.php` | Unit/Domains/Contact/DavClient/Services/Utils/PrepareJobsContactPushTest.php | AddressBookSynchronizer, PrepareJobsContactPushMissed | port | todo |
| C082 | service | `app/Domains/Contact/DavClient/Services/Utils/PrepareJobsContactPushMissed.php` | Unit/Domains/Contact/DavClient/Services/Utils/PrepareJobsContactPushMissedTest.php | AddressBookSynchronizer | port | todo |
| C083 | service | `app/Domains/Contact/DavClient/Services/Utils/PrepareJobsContactUpdater.php` | Unit/Domains/Contact/DavClient/Services/Utils/PrepareJobsContactUpdaterTest.php | AddressBookSynchronizer | port | todo |
| C084 | service | `app/Domains/Contact/DavClient/Services/Utils/Traits/HasCapability.php` | — | AddressBookSynchronizer, PrepareJobsContactUpdater | port | todo |
| C085 | service | `app/Domains/Contact/DavClient/Services/Utils/Traits/HasClient.php` | — | AddressBookGetter, ServiceUrlQuery | port | todo |
| C086 | service | `app/Domains/Contact/DavClient/Services/Utils/Traits/HasSubscription.php` | — | AddressBookSynchronizer, PrepareJobsContactPush, PrepareJobsContactPushMissed, PrepareJobsContactUpdater | port | todo |
| C087 | service | `app/Domains/Contact/ManageAvatar/Services/DestroyAvatar.php` | Unit/Domains/Contact/ManageAvatar/Services/DestroyAvatarTest.php | ModuleAvatarController | port | todo |
| C088 | service | `app/Domains/Contact/ManageAvatar/Services/UpdatePhotoAsAvatar.php` | Unit/Domains/Contact/ManageAvatar/Services/UpdatePhotoAsAvatarTest.php | ModuleAvatarController | port | todo |
| C089 | service | `app/Domains/Contact/ManageCalls/Services/CreateCall.php` | Unit/Domains/Contact/ManageCalls/Services/CreateCallTest.php | ContactModuleCallController | port | todo |
| C090 | service | `app/Domains/Contact/ManageCalls/Services/DestroyCall.php` | Unit/Domains/Contact/ManageCalls/Services/DestroyCallTest.php | ContactModuleCallController | port | todo |
| C091 | service | `app/Domains/Contact/ManageCalls/Services/UpdateCall.php` | Unit/Domains/Contact/ManageCalls/Services/UpdateCallTest.php | ContactModuleCallController | port | todo |
| C092 | dav | `app/Domains/Contact/ManageContact/Dav/ExportAddress.php` | Unit/Domains/Contact/ManageContact/Dav/ExportAddressTest.php | — | port | todo |
| C093 | dav | `app/Domains/Contact/ManageContact/Dav/ExportGender.php` | Unit/Domains/Contact/ManageContact/Dav/ExportGenderTest.php | — | port | todo |
| C094 | dav | `app/Domains/Contact/ManageContact/Dav/ExportLabels.php` | Unit/Domains/Contact/ManageContact/Dav/ExportLabelsTest.php | — | port | todo |
| C095 | dav | `app/Domains/Contact/ManageContact/Dav/ExportNames.php` | Unit/Domains/Contact/ManageContact/Dav/ExportNamesTest.php<br>Unit/Domains/Contact/ManageGroups/Dav/ExportNamesTest.php | — | port | todo |
| C096 | dav | `app/Domains/Contact/ManageContact/Dav/ExportTimestamp.php` | — | — | port | todo |
| C097 | dav | `app/Domains/Contact/ManageContact/Dav/ExportWorkInformation.php` | Unit/Domains/Contact/ManageContact/Dav/ExportWorkInformationTest.php | — | port | todo |
| C098 | dav | `app/Domains/Contact/ManageContact/Dav/ImportAddress.php` | Unit/Domains/Contact/ManageContact/Dav/ImportAddressTest.php | — | port | todo |
| C099 | dav | `app/Domains/Contact/ManageContact/Dav/ImportContact.php` | Unit/Domains/Contact/ManageContact/Dav/ImportContactTest.php | — | port | todo |
| C100 | dav | `app/Domains/Contact/ManageContact/Dav/ImportLabels.php` | Unit/Domains/Contact/ManageContact/Dav/ImportLabelsTest.php | — | port | todo |
| C101 | service | `app/Domains/Contact/ManageContact/Services/CopyContactToAnotherVault.php` | Unit/Domains/Contact/ManageContact/Services/CopyContactToAnotherVaultTest.php | — | port | todo |
| C102 | service | `app/Domains/Contact/ManageContact/Services/CreateContact.php` | Unit/Domains/Contact/ManageContact/Services/CreateContactTest.php | ContactController, ContactRelationshipsController, ImportContact, SetupDummyAccount | port | todo |
| C103 | service | `app/Domains/Contact/ManageContact/Services/DestroyContact.php` | Unit/Domains/Contact/ManageContact/Services/DestroyContactTest.php | CardDAVBackend, ContactController | port | todo |
| C104 | service | `app/Domains/Contact/ManageContact/Services/MoveContactToAnotherVault.php` | Unit/Domains/Contact/ManageContact/Services/MoveContactToAnotherVaultTest.php | ContactMoveController | port | todo |
| C105 | service | `app/Domains/Contact/ManageContact/Services/ToggleArchiveContact.php` | Unit/Domains/Contact/ManageContact/Services/ToggleArchiveContactTest.php | ContactArchiveController | port | todo |
| C106 | service | `app/Domains/Contact/ManageContact/Services/ToggleFavoriteContact.php` | Unit/Domains/Contact/ManageContact/Services/ToggleFavoriteContactTest.php | ContactFavoriteController | port | todo |
| C107 | service | `app/Domains/Contact/ManageContact/Services/UpdateContact.php` | Unit/Domains/Contact/ManageContact/Services/UpdateContactTest.php | ContactController, ImportContact | port | todo |
| C108 | service | `app/Domains/Contact/ManageContact/Services/UpdateContactSortOrder.php` | Unit/Domains/Contact/ManageContact/Services/UpdateContactSortOrderTest.php | ContactSortController | port | todo |
| C109 | service | `app/Domains/Contact/ManageContact/Services/UpdateContactTemplate.php` | Unit/Domains/Contact/ManageContact/Services/UpdateContactTemplateTest.php | ContactTemplateController | port | todo |
| C110 | service | `app/Domains/Contact/ManageContact/Services/UpdateContactView.php` | Unit/Domains/Contact/ManageContact/Services/UpdateContactViewTest.php | ContactController, ContactPageController | port | todo |
| C111 | service | `app/Domains/Contact/ManageContactAddresses/Services/AssociateAddressToContact.php` | Unit/Domains/Contact/ManageContactAddresses/Services/AssociateAddressToContactTest.php | ContactModuleAddressController, ImportAddress | port | todo |
| C112 | service | `app/Domains/Contact/ManageContactAddresses/Services/RemoveAddressFromContact.php` | Unit/Domains/Contact/ManageContactAddresses/Services/RemoveAddressFromContactTest.php | ContactModuleAddressController, ImportAddress | port | todo |
| C113 | dav | `app/Domains/Contact/ManageContactImportantDates/Dav/ExportCalendarImportantDates.php` | — | — | port | todo |
| C114 | dav | `app/Domains/Contact/ManageContactImportantDates/Dav/ExportImportantDates.php` | Unit/Domains/Contact/ManageContactImportantDates/Dav/ExportImportantDatesTest.php | — | port | todo |
| C115 | dav | `app/Domains/Contact/ManageContactImportantDates/Dav/ImportCalendarContactImportantDates.php` | Unit/Domains/Contact/ManageContactImportantDates/Dav/ImportCalendarContactImportantDatesTest.php | — | port | todo |
| C116 | dav | `app/Domains/Contact/ManageContactImportantDates/Dav/ImportImportantDates.php` | Unit/Domains/Contact/ManageContactImportantDates/Dav/ImportImportantDatesTest.php | — | port | todo |
| C117 | service | `app/Domains/Contact/ManageContactImportantDates/Services/CreateContactImportantDate.php` | Unit/Domains/Contact/ManageContactImportantDates/Services/CreateContactImportantDateTest.php | ContactImportantDatesController, ImportCalendarContactImportantDates, ImportImportantDates, SetupDummyAccount | port | todo |
| C118 | service | `app/Domains/Contact/ManageContactImportantDates/Services/DestroyContactImportantDate.php` | Unit/Domains/Contact/ManageContactImportantDates/Services/DestroyContactImportantDateTest.php | CalDAVDates, ContactImportantDatesController, ImportImportantDates | port | todo |
| C119 | service | `app/Domains/Contact/ManageContactImportantDates/Services/UpdateContactImportantDate.php` | Unit/Domains/Contact/ManageContactImportantDates/Services/UpdateContactImportantDateTest.php | ContactImportantDatesController, ImportCalendarContactImportantDates, ImportImportantDates | port | todo |
| C120 | dav | `app/Domains/Contact/ManageContactInformation/Dav/ExportContactInformation.php` | Unit/Domains/Contact/ManageContactInformation/Dav/ExportContactInformationTest.php | — | port | todo |
| C121 | dav | `app/Domains/Contact/ManageContactInformation/Dav/ImportContactInformation.php` | Unit/Domains/Contact/ManageContactInformation/Dav/ImportContactInformationTest.php | — | port | todo |
| C122 | service | `app/Domains/Contact/ManageContactInformation/Services/CreateContactInformation.php` | Unit/Domains/Contact/ManageContactInformation/Services/CreateContactInformationTest.php | ContactInformationController, ImportContactInformation | port | todo |
| C123 | service | `app/Domains/Contact/ManageContactInformation/Services/DestroyContactInformation.php` | Unit/Domains/Contact/ManageContactInformation/Services/DestroyContactInformationTest.php | ContactInformationController, ImportContactInformation | port | todo |
| C124 | service | `app/Domains/Contact/ManageContactInformation/Services/UpdateContactInformation.php` | Unit/Domains/Contact/ManageContactInformation/Services/UpdateContactInformationTest.php | ContactInformationController, ImportContactInformation | port | todo |
| C125 | other | `app/Domains/Contact/ManageDocuments/Events/FileDeleted.php` | — | AppServiceProvider, DeleteFileInStorage, File | port | todo |
| C126 | other | `app/Domains/Contact/ManageDocuments/Listeners/DeleteFileInStorage.php` | — | AppServiceProvider | port, delete from the local data directory instead of Uploadcare | todo |
| C127 | service | `app/Domains/Contact/ManageDocuments/Services/DestroyFile.php` | Unit/Domains/Contact/ManageDocuments/Services/DestroyFileTest.php | ContactModuleDocumentController, ContactModulePhotoController, PostPhotoController, VaultFileController | port | todo |
| C128 | service | `app/Domains/Contact/ManageDocuments/Services/UploadFile.php` | Unit/Domains/Contact/ManageDocuments/Services/UploadFileTest.php | ContactModuleDocumentController, ContactModulePhotoController, ModuleAvatarController, PostPhotoController, SliceOfLifeCoverImageController | port, store the upload locally instead of recording an Uploadcare uuid/CDN URL | todo |
| C129 | service | `app/Domains/Contact/ManageGoals/Services/CreateGoal.php` | Unit/Domains/Contact/ManageGoals/Services/CreateGoalTest.php | ContactModuleGoalController, SetupDummyAccount | port | todo |
| C130 | service | `app/Domains/Contact/ManageGoals/Services/DestroyGoal.php` | Unit/Domains/Contact/ManageGoals/Services/DestroyGoalTest.php | ContactGoalController | port | todo |
| C131 | service | `app/Domains/Contact/ManageGoals/Services/ToggleStreak.php` | Unit/Domains/Contact/ManageGoals/Services/ToggleStreakTest.php | ContactModuleStreakController, SetupDummyAccount | port | todo |
| C132 | service | `app/Domains/Contact/ManageGoals/Services/UpdateGoal.php` | Unit/Domains/Contact/ManageGoals/Services/UpdateGoalTest.php | ContactGoalController | port | todo |
| C133 | dav | `app/Domains/Contact/ManageGroups/Dav/ExportKind.php` | — | — | port | todo |
| C134 | dav | `app/Domains/Contact/ManageGroups/Dav/ExportMembers.php` | Unit/Domains/Contact/ManageGroups/Dav/ExportMembersTest.php | — | port | todo |
| C135 | dav | `app/Domains/Contact/ManageGroups/Dav/ExportNames.php` | Unit/Domains/Contact/ManageContact/Dav/ExportNamesTest.php<br>Unit/Domains/Contact/ManageGroups/Dav/ExportNamesTest.php | — | port | todo |
| C136 | dav | `app/Domains/Contact/ManageGroups/Dav/ExportTimestamp.php` | — | — | port | todo |
| C137 | dav | `app/Domains/Contact/ManageGroups/Dav/ImportGroup.php` | Unit/Domains/Contact/ManageGroups/Dav/ImportGroupTest.php | — | port | todo |
| C138 | dav | `app/Domains/Contact/ManageGroups/Dav/ImportMembers.php` | Unit/Domains/Contact/ManageGroups/Dav/ImportMembersTest.php | — | port | todo |
| C139 | service | `app/Domains/Contact/ManageGroups/Services/AddContactToGroup.php` | Unit/Domains/Contact/ManageGroups/Services/AddContactToGroupTest.php | ContactModuleGroupController, ImportMembers, SetupDummyAccount | port | todo |
| C140 | service | `app/Domains/Contact/ManageGroups/Services/CreateGroup.php` | Unit/Domains/Contact/ManageGroups/Services/CreateGroupTest.php | ContactModuleGroupController, ImportGroup, SetupDummyAccount | port | todo |
| C141 | service | `app/Domains/Contact/ManageGroups/Services/DestroyGroup.php` | Unit/Domains/Contact/ManageGroups/Services/DestroyGroupTest.php | CardDAVBackend, GroupController | port | todo |
| C142 | service | `app/Domains/Contact/ManageGroups/Services/RemoveContactFromGroup.php` | Unit/Domains/Contact/ManageGroups/Services/RemoveContactFromGroupTest.php | ContactModuleGroupController, ImportMembers | port | todo |
| C143 | service | `app/Domains/Contact/ManageGroups/Services/UpdateGroup.php` | Unit/Domains/Contact/ManageGroups/Services/UpdateGroupTest.php | GroupController, ImportGroup | port | todo |
| C144 | service | `app/Domains/Contact/ManageJobInformation/Services/ResetJobInformation.php` | Unit/Domains/Contact/ManageJobInformation/Services/ResetJobInformationTest.php | ContactModuleJobInformationController | port | todo |
| C145 | service | `app/Domains/Contact/ManageJobInformation/Services/UpdateJobInformation.php` | Unit/Domains/Contact/ManageJobInformation/Services/UpdateJobInformationTest.php | ContactModuleJobInformationController | port | todo |
| C146 | service | `app/Domains/Contact/ManageLabels/Services/AssignLabel.php` | Unit/Domains/Contact/ManageLabels/Services/AssignLabelTest.php | ContactModuleLabelController, ImportLabels | port | todo |
| C147 | service | `app/Domains/Contact/ManageLabels/Services/RemoveLabel.php` | Unit/Domains/Contact/ManageLabels/Services/RemoveLabelTest.php | ContactModuleLabelController, ImportLabels | port | todo |
| C148 | service | `app/Domains/Contact/ManageLifeEvents/Services/CreateLifeEvent.php` | Unit/Domains/Contact/ManageLifeEvents/Services/CreateLifeEventTest.php | ContactModuleLifeEventController, ContactModuleTimelineEventController | port | todo |
| C149 | service | `app/Domains/Contact/ManageLifeEvents/Services/CreateTimelineEvent.php` | Unit/Domains/Contact/ManageLifeEvents/Services/CreateTimelineEventTest.php | ContactModuleTimelineEventController | port | todo |
| C150 | service | `app/Domains/Contact/ManageLifeEvents/Services/DestroyLifeEvent.php` | Unit/Domains/Contact/ManageLifeEvents/Services/DestroyLifeEventTest.php | ContactModuleLifeEventController | port | todo |
| C151 | service | `app/Domains/Contact/ManageLifeEvents/Services/DestroyTimelineEvent.php` | Unit/Domains/Contact/ManageLifeEvents/Services/DestroyTimelineEventTest.php | ContactModuleTimelineEventController | port | todo |
| C152 | service | `app/Domains/Contact/ManageLifeEvents/Services/ToggleLifeEvent.php` | Unit/Domains/Contact/ManageLifeEvents/Services/ToggleLifeEventTest.php | ToggleLifeEventController | port | todo |
| C153 | service | `app/Domains/Contact/ManageLifeEvents/Services/ToggleTimelineEvent.php` | Unit/Domains/Contact/ManageLifeEvents/Services/ToggleTimelineEventTest.php | ToggleTimelineEventController | port | todo |
| C154 | service | `app/Domains/Contact/ManageLifeEvents/Services/UpdateLifeEvent.php` | Unit/Domains/Contact/ManageLifeEvents/Services/UpdateLifeEventTest.php | ContactModuleLifeEventController | port | todo |
| C155 | service | `app/Domains/Contact/ManageLifeEvents/Services/UpdateTimelineEvent.php` | Unit/Domains/Contact/ManageLifeEvents/Services/UpdateTimelineEventTest.php | — | port | todo |
| C156 | service | `app/Domains/Contact/ManageLoans/Services/CreateLoan.php` | Unit/Domains/Contact/ManageLoans/Services/CreateLoanTest.php | ContactModuleLoanController | port | todo |
| C157 | service | `app/Domains/Contact/ManageLoans/Services/DestroyLoan.php` | Unit/Domains/Contact/ManageLoans/Services/DestroyLoanTest.php | ContactModuleLoanController | port | todo |
| C158 | service | `app/Domains/Contact/ManageLoans/Services/ToggleLoan.php` | Unit/Domains/Contact/ManageLoans/Services/ToggleLoanTest.php | ContactModuleToggleLoanController | port | todo |
| C159 | service | `app/Domains/Contact/ManageLoans/Services/UpdateLoan.php` | Unit/Domains/Contact/ManageLoans/Services/UpdateLoanTest.php | ContactModuleLoanController | port | todo |
| C160 | service | `app/Domains/Contact/ManageMoodTrackingEvents/Services/CreateMoodTrackingEvent.php` | Unit/Domains/Contact/ManageMoodTrackingEvents/Services/CreateMoodTrackingEventTest.php | ContactMoodTrackingEventsController | port | todo |
| C161 | service | `app/Domains/Contact/ManageMoodTrackingEvents/Services/DestroyMoodTrackingEvent.php` | Unit/Domains/Contact/ManageMoodTrackingEvents/Services/DestroyMoodTrackingEventTest.php | — | port | todo |
| C162 | service | `app/Domains/Contact/ManageMoodTrackingEvents/Services/UpdateMoodTrackingEvent.php` | Unit/Domains/Contact/ManageMoodTrackingEvents/Services/UpdateMoodTrackingEventTest.php | — | port | todo |
| C163 | service | `app/Domains/Contact/ManageNotes/Services/CreateNote.php` | Unit/Domains/Contact/ManageNotes/Services/CreateNoteTest.php | ContactModuleNoteController, SetupDummyAccount | port | todo |
| C164 | service | `app/Domains/Contact/ManageNotes/Services/DestroyNote.php` | Unit/Domains/Contact/ManageNotes/Services/DestroyNoteTest.php | ContactModuleNoteController | port | todo |
| C165 | service | `app/Domains/Contact/ManageNotes/Services/UpdateNote.php` | Unit/Domains/Contact/ManageNotes/Services/UpdateNoteTest.php | ContactModuleNoteController | port | todo |
| C166 | service | `app/Domains/Contact/ManagePets/Services/CreatePet.php` | Unit/Domains/Contact/ManagePets/Services/CreatePetTest.php | ContactModulePetController | port | todo |
| C167 | service | `app/Domains/Contact/ManagePets/Services/DestroyPet.php` | Unit/Domains/Contact/ManagePets/Services/DestroyPetTest.php | ContactModulePetController | port | todo |
| C168 | service | `app/Domains/Contact/ManagePets/Services/UpdatePet.php` | Unit/Domains/Contact/ManagePets/Services/UpdatePetTest.php | ContactModulePetController | port | todo |
| C169 | service | `app/Domains/Contact/ManagePronouns/Services/RemovePronoun.php` | Unit/Domains/Contact/ManagePronouns/Services/RemovePronounTest.php | — | port | todo |
| C170 | service | `app/Domains/Contact/ManagePronouns/Services/SetPronoun.php` | Unit/Domains/Contact/ManagePronouns/Services/SetPronounTest.php | — | port | todo |
| C171 | service | `app/Domains/Contact/ManageQuickFacts/Services/CreateQuickFact.php` | Unit/Domains/Contact/ManageQuickFacts/Services/CreateQuickFactTest.php | ContactQuickFactController | port | todo |
| C172 | service | `app/Domains/Contact/ManageQuickFacts/Services/DestroyQuickFact.php` | Unit/Domains/Contact/ManageQuickFacts/Services/DestroyQuickFactTest.php | ContactQuickFactController | port | todo |
| C173 | service | `app/Domains/Contact/ManageQuickFacts/Services/ToggleQuickFactModule.php` | Unit/Domains/Contact/ManageQuickFacts/Services/ToggleQuickFactModuleTest.php | ContactQuickFactToggleController | port | todo |
| C174 | service | `app/Domains/Contact/ManageQuickFacts/Services/UpdateQuickFact.php` | Unit/Domains/Contact/ManageQuickFacts/Services/UpdateQuickFactTest.php | ContactQuickFactController | port | todo |
| C175 | service | `app/Domains/Contact/ManageRelationships/Services/SetRelationship.php` | Unit/Domains/Contact/ManageRelationships/Services/SetRelationshipTest.php | ContactRelationshipsController | port | todo |
| C176 | service | `app/Domains/Contact/ManageRelationships/Services/UnsetRelationship.php` | Unit/Domains/Contact/ManageRelationships/Services/UnsetRelationshipTest.php | ContactRelationshipsController | port | todo |
| C177 | service | `app/Domains/Contact/ManageReligion/Services/UpdateReligion.php` | Unit/Domains/Contact/ManageReligion/Services/UpdateReligionTest.php<br>Unit/Domains/Settings/ManageReligion/Services/UpdateReligionTest.php | ContactModuleReligionController | port | todo |
| C178 | job | `app/Domains/Contact/ManageReminders/Jobs/ProcessScheduledContactReminders.php` | Unit/Domains/Contact/ManageReminders/Jobs/ProcessScheduledContactRemindersTest.php | — | port, drop the Telegram channel | todo |
| C179 | service | `app/Domains/Contact/ManageReminders/Services/CreateContactReminder.php` | Unit/Domains/Contact/ManageReminders/Services/CreateContactReminderTest.php | ContactImportantDatesController, ContactModuleReminderController | port | todo |
| C180 | service | `app/Domains/Contact/ManageReminders/Services/DestroyReminder.php` | Unit/Domains/Contact/ManageReminders/Services/DestroyReminderTest.php | ContactModuleReminderController | port | todo |
| C181 | service | `app/Domains/Contact/ManageReminders/Services/RescheduleContactReminderForChannel.php` | Unit/Domains/Contact/ManageReminders/Services/RescheduleContactReminderForChannelTest.php | ProcessScheduledContactReminders, TestReminders | port | todo |
| C182 | service | `app/Domains/Contact/ManageReminders/Services/ScheduleContactReminderForUser.php` | Unit/Domains/Contact/ManageReminders/Services/ScheduleContactReminderForUserTest.php | CreateContactReminder, GrantVaultAccessToUser, UpdateContactReminder | port | todo |
| C183 | service | `app/Domains/Contact/ManageReminders/Services/UpdateContactReminder.php` | Unit/Domains/Contact/ManageReminders/Services/UpdateContactReminderTest.php | ContactImportantDatesController, ContactModuleReminderController | port | todo |
| C184 | dav | `app/Domains/Contact/ManageTasks/Dav/ExportContactTask.php` | — | — | port | todo |
| C185 | dav | `app/Domains/Contact/ManageTasks/Dav/ImportContactTask.php` | Unit/Domains/Contact/ManageTasks/Dav/ImportContactTaskTest.php | — | port | todo |
| C186 | service | `app/Domains/Contact/ManageTasks/Services/CreateContactTask.php` | Unit/Domains/Contact/ManageTasks/Services/CreateContactTaskTest.php | ContactModuleTaskController, ImportContactTask, SetupDummyAccount | port | todo |
| C187 | service | `app/Domains/Contact/ManageTasks/Services/DestroyContactTask.php` | Unit/Domains/Contact/ManageTasks/Services/DestroyContactTaskTest.php | CalDAVTasks, ContactModuleTaskController | port | todo |
| C188 | service | `app/Domains/Contact/ManageTasks/Services/ToggleContactTask.php` | Unit/Domains/Contact/ManageTasks/Services/ToggleContactTaskTest.php | ContactModuleTaskController | port | todo |
| C189 | service | `app/Domains/Contact/ManageTasks/Services/UpdateContactTask.php` | Unit/Domains/Contact/ManageTasks/Services/UpdateContactTaskTest.php | ContactModuleTaskController, ImportContactTask | port | todo |
| C190 | service | `app/Domains/Settings/CancelAccount/Services/CancelAccount.php` | Unit/Domains/Settings/CancelAccount/Services/CancelAccountTest.php | CancelAccountController | port | done (test: `src/domains/settings/cancel_account/tests.rs`) |
| C191 | job | `app/Domains/Settings/CreateAccount/Jobs/SetupAccount.php` | Unit/Domains/Settings/CreateAccount/Services/SetupAccountTest.php | CreateAccount | port | done (test: `src/domains/settings/create_account/tests.rs`) |
| C192 | service | `app/Domains/Settings/CreateAccount/Services/CreateAccount.php` | Unit/Domains/Settings/CreateAccount/Services/CreateAccountTest.php | CreateNewUser, SetupDummyAccount | port | done (test: `src/domains/settings/create_account/tests.rs`) |
| C193 | service | `app/Domains/Settings/ManageAddressTypes/Services/CreateAddressType.php` | Unit/Domains/Settings/ManageAddressTypes/Services/CreateAddressTypeTest.php | ImportAddress, PersonalizeAddressTypeController, SetupAccount | port | done (test: `src/domains/settings/manage_address_types/tests.rs`) |
| C194 | service | `app/Domains/Settings/ManageAddressTypes/Services/DestroyAddressType.php` | Unit/Domains/Settings/ManageAddressTypes/Services/DestroyAddressTypeTest.php | PersonalizeAddressTypeController | port | done (test: `src/domains/settings/manage_address_types/tests.rs`) |
| C195 | service | `app/Domains/Settings/ManageAddressTypes/Services/UpdateAddressType.php` | Unit/Domains/Settings/ManageAddressTypes/Services/UpdateAddressTypeTest.php | PersonalizeAddressTypeController | port | done (test: `src/domains/settings/manage_address_types/tests.rs`) |
| C196 | service | `app/Domains/Settings/ManageCallReasons/Services/CreateCallReason.php` | Unit/Domains/Settings/ManageCallReasons/Services/CreateCallReasonTest.php | PersonalizeCallReasonsController, SetupAccount | port | done (test: `src/domains/settings/manage_call_reasons/tests.rs`) |
| C197 | service | `app/Domains/Settings/ManageCallReasons/Services/CreateCallReasonType.php` | Unit/Domains/Settings/ManageCallReasons/Services/CreateCallReasonTypeTest.php | PersonalizeCallReasonTypesController, SetupAccount | port | done (test: `src/domains/settings/manage_call_reasons/tests.rs`) |
| C198 | service | `app/Domains/Settings/ManageCallReasons/Services/DestroyCallReason.php` | Unit/Domains/Settings/ManageCallReasons/Services/DestroyCallReasonTest.php | PersonalizeCallReasonsController | port | done (test: `src/domains/settings/manage_call_reasons/tests.rs`) |
| C199 | service | `app/Domains/Settings/ManageCallReasons/Services/DestroyCallReasonType.php` | Unit/Domains/Settings/ManageCallReasons/Services/DestroyCallReasonTypeTest.php | PersonalizeCallReasonTypesController | port | done (test: `src/domains/settings/manage_call_reasons/tests.rs`) |
| C200 | service | `app/Domains/Settings/ManageCallReasons/Services/UpdateCallReason.php` | Unit/Domains/Settings/ManageCallReasons/Services/UpdateCallReasonTest.php | PersonalizeCallReasonsController | port | done (test: `src/domains/settings/manage_call_reasons/tests.rs`) |
| C201 | service | `app/Domains/Settings/ManageCallReasons/Services/UpdateCallReasonType.php` | Unit/Domains/Settings/ManageCallReasons/Services/UpdateCallReasonTypeTest.php | PersonalizeCallReasonTypesController | port | done (test: `src/domains/settings/manage_call_reasons/tests.rs`) |
| C202 | service | `app/Domains/Settings/ManageContactInformationTypes/Services/CreateContactInformationType.php` | Unit/Domains/Settings/ManageContactInformationTypes/Services/CreateContactInformationTypeTest.php | ImportContactInformation, PersonalizeContatInformationTypesController, SetupAccount | port | done (test: `src/domains/settings/manage_contact_information_types/tests.rs`) |
| C203 | service | `app/Domains/Settings/ManageContactInformationTypes/Services/DestroyContactInformationType.php` | Unit/Domains/Settings/ManageContactInformationTypes/Services/DestroyContactInformationTypeTest.php | PersonalizeContatInformationTypesController | port | done (test: `src/domains/settings/manage_contact_information_types/tests.rs`) |
| C204 | service | `app/Domains/Settings/ManageContactInformationTypes/Services/UpdateContactInformationType.php` | Unit/Domains/Settings/ManageContactInformationTypes/Services/UpdateContactInformationTypeTest.php | PersonalizeContatInformationTypesController | port | done (test: `src/domains/settings/manage_contact_information_types/tests.rs`) |
| C205 | service | `app/Domains/Settings/ManageCurrencies/Services/DisableAllCurrencies.php` | Unit/Domains/Settings/ManageCurrencies/Services/DisableAllCurrenciesTest.php | PersonalizeCurrencyController | port | todo |
| C206 | service | `app/Domains/Settings/ManageCurrencies/Services/EnableAllCurrencies.php` | Unit/Domains/Settings/ManageCurrencies/Services/EnableAllCurrenciesTest.php | PersonalizeCurrencyController | port | todo |
| C207 | service | `app/Domains/Settings/ManageCurrencies/Services/ToggleCurrency.php` | Unit/Domains/Settings/ManageCurrencies/Services/ToggleCurrencyTest.php | PersonalizeCurrencyController | port | todo |
| C208 | service | `app/Domains/Settings/ManageGenders/Services/CreateGender.php` | Unit/Domains/Settings/ManageGenders/Services/CreateGenderTest.php | ManageGenderController, SetupAccount | port | done (test: `src/domains/settings/manage_genders/tests.rs`) |
| C209 | service | `app/Domains/Settings/ManageGenders/Services/DestroyGender.php` | Unit/Domains/Settings/ManageGenders/Services/DestroyGenderTest.php | ManageGenderController | port | done (test: `src/domains/settings/manage_genders/tests.rs`) |
| C210 | service | `app/Domains/Settings/ManageGenders/Services/UpdateGender.php` | Unit/Domains/Settings/ManageGenders/Services/UpdateGenderTest.php | ManageGenderController | port | done (test: `src/domains/settings/manage_genders/tests.rs`) |
| C211 | service | `app/Domains/Settings/ManageGiftOccasions/Services/CreateGiftOccasion.php` | Unit/Domains/Settings/ManageGiftOccasions/Services/CreateGiftOccasionTest.php | PersonalizeGiftOccasionController | port | todo |
| C212 | service | `app/Domains/Settings/ManageGiftOccasions/Services/DestroyGiftOccasion.php` | Unit/Domains/Settings/ManageGiftOccasions/Services/DestroyGiftOccasionTest.php | PersonalizeGiftOccasionController | port | todo |
| C213 | service | `app/Domains/Settings/ManageGiftOccasions/Services/UpdateGiftOccasion.php` | Unit/Domains/Settings/ManageGiftOccasions/Services/UpdateGiftOccasionTest.php | PersonalizeGiftOccasionController | port | todo |
| C214 | service | `app/Domains/Settings/ManageGiftOccasions/Services/UpdateGiftOccasionPosition.php` | Unit/Domains/Settings/ManageGiftOccasions/Services/UpdateGiftOccasionPositionTest.php | PersonalizeGiftOccasionsPositionController | port | todo |
| C215 | service | `app/Domains/Settings/ManageGiftStates/Services/CreateGiftState.php` | Unit/Domains/Settings/ManageGiftStates/Services/CreateGiftStateTest.php | PersonalizeGiftStateController | port | todo |
| C216 | service | `app/Domains/Settings/ManageGiftStates/Services/DestroyGiftState.php` | Unit/Domains/Settings/ManageGiftStates/Services/DestroyGiftStateTest.php | PersonalizeGiftStateController | port | todo |
| C217 | service | `app/Domains/Settings/ManageGiftStates/Services/UpdateGiftState.php` | Unit/Domains/Settings/ManageGiftStates/Services/UpdateGiftStateTest.php | PersonalizeGiftStateController | port | todo |
| C218 | service | `app/Domains/Settings/ManageGiftStates/Services/UpdateGiftStatePosition.php` | Unit/Domains/Settings/ManageGiftStates/Services/UpdateGiftStatePositionTest.php | PersonalizeGiftStatesPositionController | port | todo |
| C219 | service | `app/Domains/Settings/ManageGroupTypes/Services/CreateGroupType.php` | Unit/Domains/Settings/ManageGroupTypes/Services/CreateGroupTypeTest.php | PersonalizeGroupTypeController, SetupAccount | port | done (test: `src/domains/settings/manage_group_types/tests.rs`) |
| C220 | service | `app/Domains/Settings/ManageGroupTypes/Services/CreateGroupTypeRole.php` | Unit/Domains/Settings/ManageGroupTypes/Services/CreateGroupTypeRoleTest.php | PersonalizeGroupTypeRoleController, SetupAccount | port | done (test: `src/domains/settings/manage_group_types/tests.rs`) |
| C221 | service | `app/Domains/Settings/ManageGroupTypes/Services/DestroyGroupType.php` | Unit/Domains/Settings/ManageGroupTypes/Services/DestroyGroupTypeTest.php | PersonalizeGroupTypeController | port | done (test: `src/domains/settings/manage_group_types/tests.rs`) |
| C222 | service | `app/Domains/Settings/ManageGroupTypes/Services/DestroyGroupTypeRole.php` | Unit/Domains/Settings/ManageGroupTypes/Services/DestroyGroupTypeRoleTest.php | PersonalizeGroupTypeRoleController | port | done (test: `src/domains/settings/manage_group_types/tests.rs`) |
| C223 | service | `app/Domains/Settings/ManageGroupTypes/Services/UpdateGroupType.php` | Unit/Domains/Settings/ManageGroupTypes/Services/UpdateGroupTypeTest.php | PersonalizeGroupTypeController | port | done (test: `src/domains/settings/manage_group_types/tests.rs`) |
| C224 | service | `app/Domains/Settings/ManageGroupTypes/Services/UpdateGroupTypePosition.php` | Unit/Domains/Settings/ManageGroupTypes/Services/UpdateGroupTypePositionTest.php | PersonalizeGroupTypePositionController | port | done (test: `src/domains/settings/manage_group_types/tests.rs`) |
| C225 | service | `app/Domains/Settings/ManageGroupTypes/Services/UpdateGroupTypeRole.php` | Unit/Domains/Settings/ManageGroupTypes/Services/UpdateGroupTypeRoleTest.php | PersonalizeGroupTypeRoleController | port | done (test: `src/domains/settings/manage_group_types/tests.rs`) |
| C226 | service | `app/Domains/Settings/ManageGroupTypes/Services/UpdateGroupTypeRolePosition.php` | Unit/Domains/Settings/ManageGroupTypes/Services/UpdateGroupTypeRolePositionTest.php | PersonalizeGroupTypeRolePositionController | port | done (test: `src/domains/settings/manage_group_types/tests.rs`) |
| C227 | job | `app/Domains/Settings/ManageNotificationChannels/Jobs/SendVerificationEmailChannel.php` | Unit/Domains/Settings/ManageNotificationChannels/Jobs/SendVerificationEmailChannelTest.php | CreateUserNotificationChannel | port | done (test: `src/domains/settings/manage_notification_channels/tests.rs`) |
| C228 | service | `app/Domains/Settings/ManageNotificationChannels/Services/CreateUserNotificationChannel.php` | Unit/Domains/Settings/ManageNotificationChannels/Services/CreateUserNotificationChannelTest.php | AcceptInvitation, NotificationsController, SetupAccount, TelegramNotificationsController | port | done (test: `src/domains/settings/manage_notification_channels/tests.rs`) |
| C229 | service | `app/Domains/Settings/ManageNotificationChannels/Services/DestroyUserNotificationChannel.php` | Unit/Domains/Settings/ManageNotificationChannels/Services/DestroyUserNotificationChannelTest.php | NotificationsController | port | done (test: `src/domains/settings/manage_notification_channels/tests.rs`) |
| C230 | service | `app/Domains/Settings/ManageNotificationChannels/Services/ScheduleAllContactRemindersForNotificationChannel.php` | Unit/Domains/Settings/ManageNotificationChannels/Services/ScheduleAllContactRemindersForNotificationChannelTest.php | TelegramWebhookController, ToggleUserNotificationChannel, VerifyUserNotificationChannelEmailAddress | port | done (test: `src/domains/settings/manage_notification_channels/tests.rs`) |
| C231 | service | `app/Domains/Settings/ManageNotificationChannels/Services/SendTestEmail.php` | Unit/Domains/Settings/ManageNotificationChannels/Services/SendTestEmailTest.php | NotificationsTestController | port | done (test: `src/domains/settings/manage_notification_channels/tests.rs`) |
| C232 | service | `app/Domains/Settings/ManageNotificationChannels/Services/SendTestTelegramNotification.php` | Unit/Domains/Settings/ManageNotificationChannels/Services/SendTestTelegramNotificationTest.php | NotificationsTestController | remove: Telegram removed | todo (remove) |
| C233 | service | `app/Domains/Settings/ManageNotificationChannels/Services/ToggleUserNotificationChannel.php` | Unit/Domains/Settings/ManageNotificationChannels/Services/ToggleUserNotificationChannelTest.php | NotificationsToggleController | port | done (test: `src/domains/settings/manage_notification_channels/tests.rs`) |
| C234 | service | `app/Domains/Settings/ManageNotificationChannels/Services/VerifyUserNotificationChannelEmailAddress.php` | Unit/Domains/Settings/ManageNotificationChannels/Services/VerifyUserNotificationChannelEmailAddressTest.php | NotificationsVerificationController | port | done (test: `src/domains/settings/manage_notification_channels/tests.rs`) |
| C235 | service | `app/Domains/Settings/ManagePetCategories/Services/CreatePetCategory.php` | Unit/Domains/Settings/ManagePetCategories/Services/CreatePetCategoryTest.php | PersonalizePetCategoriesController, SetupAccount | port | done (test: `src/domains/settings/manage_pet_categories/tests.rs`) |
| C236 | service | `app/Domains/Settings/ManagePetCategories/Services/DestroyPetCategory.php` | Unit/Domains/Settings/ManagePetCategories/Services/DestroyPetCategoryTest.php | PersonalizePetCategoriesController | port | done (test: `src/domains/settings/manage_pet_categories/tests.rs`) |
| C237 | service | `app/Domains/Settings/ManagePetCategories/Services/UpdatePetCategory.php` | Unit/Domains/Settings/ManagePetCategories/Services/UpdatePetCategoryTest.php | PersonalizePetCategoriesController | port | done (test: `src/domains/settings/manage_pet_categories/tests.rs`) |
| C238 | service | `app/Domains/Settings/ManagePostTemplates/Services/CreatePostTemplate.php` | Unit/Domains/Settings/ManagePostTemplates/Services/CreatePostTemplateTest.php | PersonalizePostTemplateController, SetupAccount | port | done (test: `src/domains/settings/manage_post_templates/tests.rs`) |
| C239 | service | `app/Domains/Settings/ManagePostTemplates/Services/CreatePostTemplateSection.php` | Unit/Domains/Settings/ManagePostTemplates/Services/CreatePostTemplateSectionTest.php | PersonalizePostTemplateSectionController, SetupAccount | port | done (test: `src/domains/settings/manage_post_templates/tests.rs`) |
| C240 | service | `app/Domains/Settings/ManagePostTemplates/Services/DestroyPostTemplate.php` | Unit/Domains/Settings/ManagePostTemplates/Services/DestroyPostTemplateTest.php | PersonalizePostTemplateController | port | done (test: `src/domains/settings/manage_post_templates/tests.rs`) |
| C241 | service | `app/Domains/Settings/ManagePostTemplates/Services/DestroyPostTemplateSection.php` | Unit/Domains/Settings/ManagePostTemplates/Services/DestroyPostTemplateSectionTest.php | PersonalizePostTemplateSectionController | port | done (test: `src/domains/settings/manage_post_templates/tests.rs`) |
| C242 | service | `app/Domains/Settings/ManagePostTemplates/Services/UpdatePostTemplate.php` | Unit/Domains/Settings/ManagePostTemplates/Services/UpdatePostTemplateTest.php | PersonalizePostTemplateController | port | done (test: `src/domains/settings/manage_post_templates/tests.rs`) |
| C243 | service | `app/Domains/Settings/ManagePostTemplates/Services/UpdatePostTemplatePosition.php` | Unit/Domains/Settings/ManagePostTemplates/Services/UpdatePostTemplatePositionTest.php | PersonalizePostTemplatePositionController | port | done (test: `src/domains/settings/manage_post_templates/tests.rs`) |
| C244 | service | `app/Domains/Settings/ManagePostTemplates/Services/UpdatePostTemplateSection.php` | Unit/Domains/Settings/ManagePostTemplates/Services/UpdatePostTemplateSectionTest.php | PersonalizePostTemplateSectionController | port | done (test: `src/domains/settings/manage_post_templates/tests.rs`) |
| C245 | service | `app/Domains/Settings/ManagePostTemplates/Services/UpdatePostTemplateSectionPosition.php` | Unit/Domains/Settings/ManagePostTemplates/Services/UpdatePostTemplateSectionPositionTest.php | PersonalizePostTemplateSectionPositionController | port | done (test: `src/domains/settings/manage_post_templates/tests.rs`) |
| C246 | service | `app/Domains/Settings/ManagePronouns/Services/CreatePronoun.php` | Unit/Domains/Settings/ManagePronouns/Services/CreatePronounTest.php | PersonalizePronounController, SetupAccount | port | done (test: `src/domains/settings/manage_pronouns/tests.rs`) |
| C247 | service | `app/Domains/Settings/ManagePronouns/Services/DestroyPronoun.php` | Unit/Domains/Settings/ManagePronouns/Services/DestroyPronounTest.php | PersonalizePronounController | port | done (test: `src/domains/settings/manage_pronouns/tests.rs`) |
| C248 | service | `app/Domains/Settings/ManagePronouns/Services/UpdatePronoun.php` | Unit/Domains/Settings/ManagePronouns/Services/UpdatePronounTest.php | PersonalizePronounController | port | done (test: `src/domains/settings/manage_pronouns/tests.rs`) |
| C249 | service | `app/Domains/Settings/ManageRelationshipTypes/Services/CreateRelationshipGroupType.php` | Unit/Domains/Settings/ManageRelationshipTypes/Services/CreateRelationshipGroupTypeTest.php | PersonalizeRelationshipController, SetupAccount | port | done (test: `src/domains/settings/manage_relationship_types/tests.rs`) |
| C250 | service | `app/Domains/Settings/ManageRelationshipTypes/Services/CreateRelationshipType.php` | Unit/Domains/Settings/ManageRelationshipTypes/Services/CreateRelationshipTypeTest.php | PersonalizeRelationshipTypeController | port | done (test: `src/domains/settings/manage_relationship_types/tests.rs`) |
| C251 | service | `app/Domains/Settings/ManageRelationshipTypes/Services/DestroyRelationshipGroupType.php` | Unit/Domains/Settings/ManageRelationshipTypes/Services/DestroyRelationshipGroupTypeTest.php | PersonalizeRelationshipController | port | done (test: `src/domains/settings/manage_relationship_types/tests.rs`) |
| C252 | service | `app/Domains/Settings/ManageRelationshipTypes/Services/DestroyRelationshipType.php` | Unit/Domains/Settings/ManageRelationshipTypes/Services/DestroyRelationshipTypeTest.php | PersonalizeRelationshipTypeController | port | done (test: `src/domains/settings/manage_relationship_types/tests.rs`) |
| C253 | service | `app/Domains/Settings/ManageRelationshipTypes/Services/UpdateRelationshipGroupType.php` | Unit/Domains/Settings/ManageRelationshipTypes/Services/UpdateRelationshipGroupTypeTest.php | PersonalizeRelationshipController | port | done (test: `src/domains/settings/manage_relationship_types/tests.rs`) |
| C254 | service | `app/Domains/Settings/ManageRelationshipTypes/Services/UpdateRelationshipType.php` | Unit/Domains/Settings/ManageRelationshipTypes/Services/UpdateRelationshipTypeTest.php | PersonalizeRelationshipTypeController | port | done (test: `src/domains/settings/manage_relationship_types/tests.rs`) |
| C255 | service | `app/Domains/Settings/ManageReligion/Services/CreateReligion.php` | Unit/Domains/Settings/ManageReligion/Services/CreateReligionTest.php | PersonalizeReligionController | port | todo |
| C256 | service | `app/Domains/Settings/ManageReligion/Services/DestroyReligion.php` | Unit/Domains/Settings/ManageReligion/Services/DestroyReligionTest.php | PersonalizeReligionController | port | todo |
| C257 | service | `app/Domains/Settings/ManageReligion/Services/UpdateReligion.php` | Unit/Domains/Contact/ManageReligion/Services/UpdateReligionTest.php<br>Unit/Domains/Settings/ManageReligion/Services/UpdateReligionTest.php | PersonalizeReligionController | port | todo |
| C258 | service | `app/Domains/Settings/ManageReligion/Services/UpdateReligionPosition.php` | Unit/Domains/Settings/ManageReligion/Services/UpdateReligionPositionTest.php | PersonalizeReligionsPositionController | port | todo |
| C259 | service | `app/Domains/Settings/ManageTemplates/Services/AssociateModuleToTemplatePage.php` | Unit/Domains/Settings/ManageTemplates/Services/AssociateModuleToTemplatePageTest.php | PersonalizeTemplatePageModulesController, SetupAccount | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C260 | service | `app/Domains/Settings/ManageTemplates/Services/CreateModule.php` | Unit/Domains/Settings/ManageTemplates/Services/CreateModuleTest.php | SetupAccount | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C261 | service | `app/Domains/Settings/ManageTemplates/Services/CreateTemplate.php` | Unit/Domains/Settings/ManageTemplates/Services/CreateTemplateTest.php | PersonalizeTemplatesController, SetupAccount | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C262 | service | `app/Domains/Settings/ManageTemplates/Services/CreateTemplatePage.php` | Unit/Domains/Settings/ManageTemplates/Services/CreateTemplatePageTest.php | CreateTemplate, PersonalizeTemplatePagesController, SetupAccount | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C263 | service | `app/Domains/Settings/ManageTemplates/Services/DestroyModule.php` | Unit/Domains/Settings/ManageTemplates/Services/DestroyModuleTest.php | — | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C264 | service | `app/Domains/Settings/ManageTemplates/Services/DestroyTemplate.php` | Unit/Domains/Settings/ManageTemplates/Services/DestroyTemplateTest.php | PersonalizeTemplatesController | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C265 | service | `app/Domains/Settings/ManageTemplates/Services/DestroyTemplatePage.php` | Unit/Domains/Settings/ManageTemplates/Services/DestroyTemplatePageTest.php | PersonalizeTemplatePagesController | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C266 | service | `app/Domains/Settings/ManageTemplates/Services/RemoveModuleFromTemplatePage.php` | Unit/Domains/Settings/ManageTemplates/Services/RemoveModuleFromTemplatePageTest.php | PersonalizeTemplatePageModulesController | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C267 | service | `app/Domains/Settings/ManageTemplates/Services/UpdateModule.php` | Unit/Domains/Settings/ManageTemplates/Services/UpdateModuleTest.php | — | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C268 | service | `app/Domains/Settings/ManageTemplates/Services/UpdateModulePosition.php` | Unit/Domains/Settings/ManageTemplates/Services/UpdateModulePositionTest.php | PersonalizeTemplatePageModulesPositionController | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C269 | service | `app/Domains/Settings/ManageTemplates/Services/UpdateTemplate.php` | Unit/Domains/Settings/ManageTemplates/Services/UpdateTemplateTest.php | PersonalizeTemplatesController | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C270 | service | `app/Domains/Settings/ManageTemplates/Services/UpdateTemplatePage.php` | Unit/Domains/Settings/ManageTemplates/Services/UpdateTemplatePageTest.php | PersonalizeTemplatePagesController | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C271 | service | `app/Domains/Settings/ManageTemplates/Services/UpdateTemplatePagePosition.php` | Unit/Domains/Settings/ManageTemplates/Services/UpdateTemplatePagePositionTest.php | PersonalizeTemplatePagePositionController | port | done (test: `src/domains/settings/manage_templates/tests.rs`) |
| C272 | service | `app/Domains/Settings/ManageUserPreferences/Services/StoreDateFormatPreference.php` | Unit/Domains/Settings/ManageUserPreferences/Services/StoreDateFormatPreferenceTest.php | PreferencesDateFormatController | port | done (test: `src/domains/settings/manage_user_preferences/tests.rs`) |
| C273 | service | `app/Domains/Settings/ManageUserPreferences/Services/StoreDistanceFormatPreference.php` | Unit/Domains/Settings/ManageUserPreferences/Services/StoreDistanceFormatPreferenceTest.php | PreferencesDistanceFormatController | port | done (test: `src/domains/settings/manage_user_preferences/tests.rs`) |
| C274 | service | `app/Domains/Settings/ManageUserPreferences/Services/StoreHelpPreference.php` | Unit/Domains/Settings/ManageUserPreferences/Services/StoreHelpPreferenceTest.php | PreferencesHelpController | port | done (test: `src/domains/settings/manage_user_preferences/tests.rs`) |
| C275 | service | `app/Domains/Settings/ManageUserPreferences/Services/StoreLocale.php` | Unit/Domains/Settings/ManageUserPreferences/Services/StoreLocaleTest.php | PreferencesLocaleController | port | done (test: `src/domains/settings/manage_user_preferences/tests.rs`) |
| C276 | service | `app/Domains/Settings/ManageUserPreferences/Services/StoreMapsPreference.php` | Unit/Domains/Settings/ManageUserPreferences/Services/StoreMapsPreferenceTest.php | PreferencesMapsPreferenceController | port | done (test: `src/domains/settings/manage_user_preferences/tests.rs`) |
| C277 | service | `app/Domains/Settings/ManageUserPreferences/Services/StoreNameOrderPreference.php` | Unit/Domains/Settings/ManageUserPreferences/Services/StoreNameOrderPreferenceTest.php | PreferencesNameOrderController | port | done (test: `src/domains/settings/manage_user_preferences/tests.rs`) |
| C278 | service | `app/Domains/Settings/ManageUserPreferences/Services/StoreNumberFormatPreference.php` | Unit/Domains/Settings/ManageUserPreferences/Services/StoreNumberFormatPreferenceTest.php | PreferencesNumberFormatController | port | done (test: `src/domains/settings/manage_user_preferences/tests.rs`) |
| C279 | service | `app/Domains/Settings/ManageUserPreferences/Services/StoreTimezone.php` | Unit/Domains/Settings/ManageUserPreferences/Services/StoreTimezoneTest.php | PreferencesTimezoneController | port | done (test: `src/domains/settings/manage_user_preferences/tests.rs`) |
| C280 | service | `app/Domains/Settings/ManageUsers/Services/AcceptInvitation.php` | Unit/Domains/Settings/ManageUsers/Services/AcceptInvitationTest.php | AcceptInvitationController | port | done (test: `src/domains/settings/manage_users/tests.rs`) |
| C281 | service | `app/Domains/Settings/ManageUsers/Services/DestroyUser.php` | Unit/Domains/Settings/ManageUsers/Services/DestroyUserTest.php | DeleteUser, UserController | port | done (test: `src/domains/settings/manage_users/tests.rs`) |
| C282 | service | `app/Domains/Settings/ManageUsers/Services/GiveAdministratorPrivilege.php` | Unit/Domains/Settings/ManageUsers/Services/GiveAdministratorPrivilegeTest.php | UserController | port | done (test: `src/domains/settings/manage_users/tests.rs`) |
| C283 | service | `app/Domains/Settings/ManageUsers/Services/InviteUser.php` | Unit/Domains/Settings/ManageUsers/Services/InviteUserTest.php | UserController | port | done (test: `src/domains/settings/manage_users/tests.rs`) |
| C284 | service | `app/Domains/Settings/ManageUsers/Services/RemoveAdministratorPrivilege.php` | Unit/Domains/Settings/ManageUsers/Services/RemoveAdministratorPrivilegeTest.php | UserController | port | done (test: `src/domains/settings/manage_users/tests.rs`) |
| C285 | service | `app/Domains/Vault/ManageAddresses/Services/CreateAddress.php` | Unit/Domains/Vault/ManageAddresses/Services/CreateAddressTest.php | ContactModuleAddressController, ImportAddress | port, drop the `GetGPSCoordinate` dispatch (LocationIQ) | todo |
| C286 | service | `app/Domains/Vault/ManageAddresses/Services/DestroyAddress.php` | Unit/Domains/Vault/ManageAddresses/Services/DestroyAddressTest.php | — | port | todo |
| C287 | service | `app/Domains/Vault/ManageAddresses/Services/GetGPSCoordinate.php` | Unit/Domains/Vault/ManageAddresses/Services/GetGPSCoordinateTest.php | CreateAddress, UpdateAddress | remove: Mapbox/LocationIQ removed | todo (remove) |
| C288 | service | `app/Domains/Vault/ManageAddresses/Services/UpdateAddress.php` | Unit/Domains/Vault/ManageAddresses/Services/UpdateAddressTest.php | ContactModuleAddressController, ImportAddress | port, drop the `GetGPSCoordinate` dispatch (LocationIQ) | todo |
| C289 | service | `app/Domains/Vault/ManageCompanies/Services/CreateCompany.php` | Unit/Domains/Vault/ManageCompanies/Services/CreateCompanyTest.php | ContactModuleJobInformationController, MoveContactToAnotherVault | port | todo |
| C290 | service | `app/Domains/Vault/ManageJournals/Services/AddContactToPost.php` | Unit/Domains/Vault/ManageJournals/Services/AddContactToPostTest.php | PostController | port | todo |
| C291 | service | `app/Domains/Vault/ManageJournals/Services/AddPhotoToPost.php` | Unit/Domains/Vault/ManageJournals/Services/AddPhotoToPostTest.php | PostPhotoController | port | todo |
| C292 | service | `app/Domains/Vault/ManageJournals/Services/AddPostToSliceOfLife.php` | Unit/Domains/Vault/ManageJournals/Services/AddPostToSliceOfLifeTest.php | PostSliceOfLifeController | port | todo |
| C293 | service | `app/Domains/Vault/ManageJournals/Services/AssignTag.php` | Unit/Domains/Vault/ManageJournals/Services/AssignTagTest.php | PostTagController | port | todo |
| C294 | service | `app/Domains/Vault/ManageJournals/Services/CreateJournal.php` | Unit/Domains/Vault/ManageJournals/Services/CreateJournalTest.php | JournalController, SetupDummyAccount | port | todo |
| C295 | service | `app/Domains/Vault/ManageJournals/Services/CreateJournalMetric.php` | Unit/Domains/Vault/ManageJournals/Services/CreateJournalMetricTest.php | JournalMetricController | port | todo |
| C296 | service | `app/Domains/Vault/ManageJournals/Services/CreatePost.php` | Unit/Domains/Vault/ManageJournals/Services/CreatePostTest.php | PostController, SetupDummyAccount | port | todo |
| C297 | service | `app/Domains/Vault/ManageJournals/Services/CreatePostMetric.php` | Unit/Domains/Vault/ManageJournals/Services/CreatePostMetricTest.php | PostMetricController | port | todo |
| C298 | service | `app/Domains/Vault/ManageJournals/Services/CreateSliceOfLife.php` | Unit/Domains/Vault/ManageJournals/Services/CreateSliceOfLifeTest.php | SliceOfLifeController | port | todo |
| C299 | service | `app/Domains/Vault/ManageJournals/Services/DestroyJournal.php` | Unit/Domains/Vault/ManageJournals/Services/DestroyJournalTest.php | JournalController | port | todo |
| C300 | service | `app/Domains/Vault/ManageJournals/Services/DestroyJournalMetric.php` | Unit/Domains/Vault/ManageJournals/Services/DestroyJournalMetricTest.php | JournalMetricController | port | todo |
| C301 | service | `app/Domains/Vault/ManageJournals/Services/DestroyPost.php` | Unit/Domains/Vault/ManageJournals/Services/DestroyPostTest.php | PostController | port | todo |
| C302 | service | `app/Domains/Vault/ManageJournals/Services/DestroyPostMetric.php` | Unit/Domains/Vault/ManageJournals/Services/DestroyPostMetricTest.php | PostMetricController | port | todo |
| C303 | service | `app/Domains/Vault/ManageJournals/Services/DestroySliceOfLife.php` | Unit/Domains/Vault/ManageJournals/Services/DestroySliceOfLifeTest.php | SliceOfLifeController | port | todo |
| C304 | service | `app/Domains/Vault/ManageJournals/Services/IncrementPostReadCounter.php` | Unit/Domains/Vault/ManageJournals/Services/IncrementPostReadCounterTest.php | PostController | port | todo |
| C305 | service | `app/Domains/Vault/ManageJournals/Services/RemoveContactFromPost.php` | Unit/Domains/Vault/ManageJournals/Services/RemoveContactFromPostTest.php | — | port | todo |
| C306 | service | `app/Domains/Vault/ManageJournals/Services/RemovePostFromSliceOfLife.php` | Unit/Domains/Vault/ManageJournals/Services/RemovePostFromSliceOfLifeTest.php | PostSliceOfLifeController | port | todo |
| C307 | service | `app/Domains/Vault/ManageJournals/Services/RemoveSliceOfLifeCoverImage.php` | Unit/Domains/Vault/ManageJournals/Services/RemoveSliceOfLifeCoverImageTest.php | SliceOfLifeCoverImageController | port | todo |
| C308 | service | `app/Domains/Vault/ManageJournals/Services/RemoveTag.php` | Unit/Domains/Vault/ManageJournals/Services/RemoveTagTest.php | PostTagController | port | todo |
| C309 | service | `app/Domains/Vault/ManageJournals/Services/SetSliceOfLifeCoverImage.php` | Unit/Domains/Vault/ManageJournals/Services/SetSliceOfLifeCoverImageTest.php | SliceOfLifeCoverImageController | port | todo |
| C310 | service | `app/Domains/Vault/ManageJournals/Services/UpdateJournal.php` | Unit/Domains/Vault/ManageJournals/Services/UpdateJournalTest.php | JournalController | port | todo |
| C311 | service | `app/Domains/Vault/ManageJournals/Services/UpdateJournalMetric.php` | Unit/Domains/Vault/ManageJournals/Services/UpdateJournalMetricTest.php | JournalMetricController | port | todo |
| C312 | service | `app/Domains/Vault/ManageJournals/Services/UpdatePost.php` | Unit/Domains/Vault/ManageJournals/Services/UpdatePostTest.php | PostController | port | todo |
| C313 | service | `app/Domains/Vault/ManageJournals/Services/UpdatePostMetric.php` | Unit/Domains/Vault/ManageJournals/Services/UpdatePostMetricTest.php | — | port | todo |
| C314 | service | `app/Domains/Vault/ManageJournals/Services/UpdateSliceOfLife.php` | Unit/Domains/Vault/ManageJournals/Services/UpdateSliceOfLifeTest.php | SliceOfLifeController | port | todo |
| C315 | service | `app/Domains/Vault/ManageLifeMetrics/Services/CreateLifeMetric.php` | Unit/Domains/Vault/ManageLifeMetrics/Services/CreateLifeMetricTest.php | LifeMetricController | port | todo |
| C316 | service | `app/Domains/Vault/ManageLifeMetrics/Services/DestroyLifeMetric.php` | Unit/Domains/Vault/ManageLifeMetrics/Services/DestroyLifeMetricTest.php | LifeMetricController | port | todo |
| C317 | service | `app/Domains/Vault/ManageLifeMetrics/Services/IncrementLifeMetric.php` | Unit/Domains/Vault/ManageLifeMetrics/Services/IncrementLifeMetricTest.php | LifeMetricContactController | port | todo |
| C318 | service | `app/Domains/Vault/ManageLifeMetrics/Services/UpdateLifeMetric.php` | Unit/Domains/Vault/ManageLifeMetrics/Services/UpdateLifeMetricTest.php | LifeMetricController | port | todo |
| C319 | service | `app/Domains/Vault/ManageVault/Services/CreateVault.php` | Unit/Domains/Vault/ManageVault/Services/CreateVaultTest.php | SetupDummyAccount, VaultController, VaultController | port | done (test: `src/domains/vault/manage_vault/tests.rs`) |
| C320 | service | `app/Domains/Vault/ManageVault/Services/DestroyVault.php` | Unit/Domains/Vault/ManageVault/Services/DestroyVaultTest.php | VaultController, VaultController | port | done (test: `src/domains/vault/manage_vault/tests.rs`) |
| C321 | service | `app/Domains/Vault/ManageVault/Services/UpdateVault.php` | Unit/Domains/Vault/ManageVault/Services/UpdateVaultTest.php | VaultController, VaultController | port | done (test: `src/domains/vault/manage_vault/tests.rs`) |
| C322 | service | `app/Domains/Vault/ManageVault/Services/UpdateVaultDashboardDefaultTab.php` | Unit/Domains/Vault/ManageVault/Services/UpdateVaultDashboardDefaultTabTest.php | VaultDefaultTabOnDashboardController | port | done (test: `src/domains/vault/manage_vault/tests.rs`) |
| C323 | service | `app/Domains/Vault/ManageVault/Services/UpdateVaultTabVisibility.php` | Unit/Domains/Vault/ManageVault/Services/UpdateVaultTabVisibilityTest.php | VaultSettingsTabVisibilityController | port | done (test: `src/domains/vault/manage_vault/tests.rs`) |
| C324 | controller | `app/Domains/Vault/ManageVault/Web/Controllers/VaultLifeEventController.php` | — | — | remove: no route points at it (§11 F5) | todo (remove) |
| C325 | service | `app/Domains/Vault/ManageVaultImportantDateTypes/Services/CreateContactImportantDateType.php` | Unit/Domains/Vault/ManageVaultImportantDateTypes/Services/CreateContactImportantDateTypeTest.php | CreateVault, VaultSettingsContactImportantDateTypeController | port | done (test: `src/domains/vault/manage_vault_important_date_types/tests.rs`) |
| C326 | service | `app/Domains/Vault/ManageVaultImportantDateTypes/Services/DestroyContactImportantDateType.php` | Unit/Domains/Vault/ManageVaultImportantDateTypes/Services/DestroyContactImportantDateTypeTest.php | VaultSettingsContactImportantDateTypeController | port | done (test: `src/domains/vault/manage_vault_important_date_types/tests.rs`) |
| C327 | service | `app/Domains/Vault/ManageVaultImportantDateTypes/Services/UpdateContactImportantDateType.php` | Unit/Domains/Vault/ManageVaultImportantDateTypes/Services/UpdateContactImportantDateTypeTest.php | VaultSettingsContactImportantDateTypeController | port | done (test: `src/domains/vault/manage_vault_important_date_types/tests.rs`) |
| C328 | service | `app/Domains/Vault/ManageVaultSettings/Services/ChangeVaultAccess.php` | Unit/Domains/Vault/ManageVaultSettings/Services/ChangeVaultAccessTest.php | VaultSettingsUserController | port | done (test: `src/domains/vault/manage_vault_settings/tests.rs`) |
| C329 | service | `app/Domains/Vault/ManageVaultSettings/Services/CreateLabel.php` | Unit/Domains/Vault/ManageVaultSettings/Services/CreateLabelTest.php | ContactModuleLabelController, ImportLabels, VaultSettingsLabelController | port | todo |
| C330 | service | `app/Domains/Vault/ManageVaultSettings/Services/CreateLifeEventCategory.php` | Unit/Domains/Vault/ManageVaultSettings/Services/CreateLifeEventCategoryTest.php | VaultSettingsLifeEventCategoriesController | port | todo |
| C331 | service | `app/Domains/Vault/ManageVaultSettings/Services/CreateLifeEventType.php` | Unit/Domains/Vault/ManageVaultSettings/Services/CreateLifeEventTypeTest.php | VaultSettingsLifeEventTypesController | port | todo |
| C332 | service | `app/Domains/Vault/ManageVaultSettings/Services/CreateMoodTrackingParameter.php` | Unit/Domains/Vault/ManageVaultSettings/Services/CreateMoodTrackingParameterTest.php | VaultSettingsMoodTrackingParameterController | port | todo |
| C333 | service | `app/Domains/Vault/ManageVaultSettings/Services/CreateQuickFactTemplate.php` | Unit/Domains/Vault/ManageVaultSettings/Services/CreateQuickFactTemplateTest.php | VaultSettingsQuickFactTemplateController | port | todo |
| C334 | service | `app/Domains/Vault/ManageVaultSettings/Services/CreateTag.php` | Unit/Domains/Vault/ManageVaultSettings/Services/CreateTagTest.php | PostTagController, VaultSettingsTagController | port | todo |
| C335 | service | `app/Domains/Vault/ManageVaultSettings/Services/DestroyLabel.php` | Unit/Domains/Vault/ManageVaultSettings/Services/DestroyLabelTest.php | VaultSettingsLabelController | port | todo |
| C336 | service | `app/Domains/Vault/ManageVaultSettings/Services/DestroyLifeEventCategory.php` | Unit/Domains/Vault/ManageVaultSettings/Services/DestroyLifeEventCategoryTest.php | VaultSettingsLifeEventCategoriesController | port | todo |
| C337 | service | `app/Domains/Vault/ManageVaultSettings/Services/DestroyLifeEventType.php` | Unit/Domains/Vault/ManageVaultSettings/Services/DestroyLifeEventTypeTest.php | VaultSettingsLifeEventTypesController | port | todo |
| C338 | service | `app/Domains/Vault/ManageVaultSettings/Services/DestroyMoodTrackingParameter.php` | Unit/Domains/Vault/ManageVaultSettings/Services/DestroyMoodTrackingParameterTest.php | VaultSettingsMoodTrackingParameterController | port | todo |
| C339 | service | `app/Domains/Vault/ManageVaultSettings/Services/DestroyQuickFactTemplate.php` | Unit/Domains/Vault/ManageVaultSettings/Services/DestroyQuickFactTemplateTest.php | VaultSettingsQuickFactTemplateController | port | todo |
| C340 | service | `app/Domains/Vault/ManageVaultSettings/Services/DestroyTag.php` | Unit/Domains/Vault/ManageVaultSettings/Services/DestroyTagTest.php | VaultSettingsTagController | port | todo |
| C341 | service | `app/Domains/Vault/ManageVaultSettings/Services/GrantVaultAccessToUser.php` | Unit/Domains/Vault/ManageVaultSettings/Services/GrantVaultAccessToUserTest.php | VaultSettingsUserController | port | done (test: `src/domains/vault/manage_vault_settings/tests.rs`) |
| C342 | service | `app/Domains/Vault/ManageVaultSettings/Services/RemoveVaultAccess.php` | Unit/Domains/Vault/ManageVaultSettings/Services/RemoveVaultAccessTest.php | VaultSettingsUserController | port | done (test: `src/domains/vault/manage_vault_settings/tests.rs`) |
| C343 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateLabel.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateLabelTest.php | VaultSettingsLabelController | port | todo |
| C344 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateLifeEventCategory.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateLifeEventCategoryTest.php | VaultSettingsLifeEventCategoriesController | port | todo |
| C345 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateLifeEventCategoryPosition.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateLifeEventCategoryPositionTest.php | VaultSettingsLifeEventCategoriesPositionController | port | todo |
| C346 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateLifeEventType.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateLifeEventTypeTest.php | VaultSettingsLifeEventTypesController | port | todo |
| C347 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateLifeEventTypePosition.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateLifeEventTypePositionTest.php | VaultSettingsLifeEventTypesPositionController | port | todo |
| C348 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateMoodTrackingParameter.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateMoodTrackingParameterTest.php | VaultSettingsMoodTrackingParameterController | port | todo |
| C349 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateMoodTrackingParameterPosition.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateMoodTrackingParameterPositionTest.php | VaultSettingsMoodTrackingParameterPositionController | port | todo |
| C350 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateQuickFactTemplate.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateQuickFactTemplateTest.php | VaultSettingsQuickFactTemplateController | port | todo |
| C351 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateQuickFactTemplatePosition.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateQuickFactTemplatePositionTest.php | VaultSettingsQuickFactTemplatePositionController | port | todo |
| C352 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateTag.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateTagTest.php | VaultSettingsTagController | port | todo |
| C353 | service | `app/Domains/Vault/ManageVaultSettings/Services/UpdateVaultDefaultTemplate.php` | Unit/Domains/Vault/ManageVaultSettings/Services/UpdateVaultDefaultTemplateTest.php | VaultSettingsTemplateController | port | done (test: `src/domains/vault/manage_vault_settings/tests.rs`) |
| C354 | exception | `app/Exceptions/CantBeDeletedException.php` | — | DestroyContact, DestroyContactImportantDateType, DestroyLifeEventCategory, DestroyLifeEventType, DestroyModule | port | todo |
| C355 | exception | `app/Exceptions/EntryAlreadyExistException.php` | — | SetupDummyAccount | port | todo |
| C356 | exception | `app/Exceptions/EnvVariablesNotSetException.php` | — | DeleteFileInStorage, GetGPSCoordinate, UploadFile | port | todo |
| C357 | exception | `app/Exceptions/Handler.php` | — | — | port | todo |
| C358 | exception | `app/Exceptions/MaximumNumberOfUsersInVaultException.php` | — | — | port | todo |
| C359 | exception | `app/Exceptions/NotEnoughPermissionException.php` | — | BaseService, CardDAVBackend, CopyContactToAnotherVault, ImportAddress, ImportContactInformation, MoveContactToAnotherVault | port | todo |
| C360 | exception | `app/Exceptions/SameUserException.php` | — | ChangeVaultAccess, GrantVaultAccessToUser, RemoveVaultAccess | port | todo |
| C361 | helper | `app/Helpers/AvatarHelper.php` | Unit/Helpers/AvatarHelperTest.php | Contact | port | todo |
| C362 | helper | `app/Helpers/CollectionHelper.php` | Unit/Helpers/CollectionHelperTest.php | AppServiceProvider | port | todo |
| C363 | helper | `app/Helpers/ContactCardHelper.php` | Unit/Helpers/ContactCardHelperTest.php | ModuleLifeEventViewHelper, ModuleLoanViewHelper, PostShowViewHelper, ReportCitiesShowViewHelper, ReportCountriesShowViewHelper, ReportImportantDateSummaryInd… | port | todo |
| C364 | helper | `app/Helpers/ContactImportantDateHelper.php` | — | Contact | port | todo |
| C365 | helper | `app/Helpers/ContactReminderHelper.php` | Unit/Helpers/ContactReminderHelperTest.php | ModuleRemindersViewHelper | port | todo |
| C366 | helper | `app/Helpers/DateHelper.php` | Unit/Helpers/DateHelperTest.php | ActionFeedMoodTrackingEvent, ContactImportantDatesViewHelper, GoalShowViewHelper, JournalIndexViewHelper, JournalShowViewHelper, ModuleCallsViewHelper, Modul… | port | todo |
| C367 | helper | `app/Helpers/DistanceHelper.php` | Unit/Helpers/DistanceHelperTest.php | ModuleLifeEventViewHelper | port | todo |
| C368 | helper | `app/Helpers/FileHelper.php` | Unit/Helpers/FileHelperTest.php | ContactPhotosIndexViewHelper, ContactPhotosShowViewHelper, ModuleDocumentsViewHelper, ModulePhotosViewHelper, PostEditViewHelper, StorageIndexViewHelper, Vau… | port | todo |
| C369 | helper | `app/Helpers/GoalHelper.php` | Unit/Helpers/GoalHelperTest.php | GoalShowViewHelper, ModuleGoalsViewHelper | port | todo |
| C370 | helper | `app/Helpers/ImportantDateHelper.php` | Unit/Helpers/ImportantDateHelperTest.php | Contact, ContactImportantDatesViewHelper, CreateContactImportantDate, DestroyContactImportantDate, ModuleImportantDatesViewHelper, ReportImportantDateSummary… | port | todo |
| C371 | helper | `app/Helpers/MapHelper.php` | Unit/Helpers/MapHelperTest.php | ActionFeedAddress, AssociateAddressToContact, ContactModuleAddressImageController, GetGPSCoordinate, ModuleContactAddressesViewHelper, RemoveAddressFromConta… | port, drop `getStaticImage` (Mapbox); keep `getMapLink`, `getAddressAsString` | todo |
| C372 | helper | `app/Helpers/MonetaryNumberHelper.php` | Unit/Helpers/MonetaryNumberHelperTest.php | ContactModuleLoanController, ModuleLoanViewHelper, UserPreferencesIndexViewHelper | port | todo |
| C373 | helper | `app/Helpers/NameHelper.php` | Unit/Helpers/NameHelperTest.php | Contact, ProcessScheduledContactReminders, TestReminders, UserPreferencesIndexViewHelper | port | todo |
| C374 | helper | `app/Helpers/PaginatorHelper.php` | Unit/Helpers/PaginatorHelperTest.php | ContactController, ContactFeedController, ContactLabelController, ContactModuleTimelineEventController, ContactNotesController, ContactPhotoController, Vault… | port | todo |
| C375 | helper | `app/Helpers/PostHelper.php` | Unit/Helpers/PostHelperTest.php | PostController, PostEditViewHelper | port | todo |
| C376 | helper | `app/Helpers/SQLHelper.php` | — | JournalShowViewHelper | port | todo |
| C377 | helper | `app/Helpers/ScoutHelper.php` | — | Contact, Group, Loan, Note, SetupScout | remove: Meilisearch/Typesense removed (DB search kept) | todo (remove) |
| C378 | helper | `app/Helpers/SignupHelper.php` | Unit/Helpers/SignupHelperTest.php | AttemptToAuthenticateSocialite, EnsureSignupIsEnabled, LoginController | port | todo |
| C379 | helper | `app/Helpers/SliceOfLifeHelper.php` | Unit/Helpers/SliceOfLifeHelperTest.php | JournalShowViewHelper, PostShowViewHelper, SliceOfLifeIndexViewHelper, SliceOfLifeShowViewHelper | port | todo |
| C380 | helper | `app/Helpers/StorageHelper.php` | Unit/Helpers/StorageHelperTest.php | ContactPhotosIndexViewHelper, ContactShowViewHelper, ModuleDocumentsViewHelper, ModulePhotosViewHelper, PostEditViewHelper, SliceOfLifeShowViewHelper | port, count local file sizes | todo |
| C381 | helper | `app/Helpers/UserHelper.php` | Unit/Helpers/UserHelperTest.php | ModuleFeedViewHelper, ModuleNotesViewHelper, NotesIndexViewHelper | port | todo |
| C382 | helper | `app/Helpers/VaultHelper.php` | Unit/Helpers/VaultHelperTest.php | VaultIndexViewHelper, VaultSettingsIndexViewHelper | port | todo |
| C383 | helper | `app/Helpers/WallpaperHelper.php` | Unit/Helpers/WallpaperHelperTest.php | LoginController | port | todo |
| C384 | helper | `app/Helpers/WikipediaHelper.php` | Unit/Helpers/WikipediaHelperTest.php | ReportCitiesShowViewHelper, ReportCountriesShowViewHelper | port | todo |
| C385 | helper | `app/Helpers/helpers.php` | — | — | port | todo |
| C386 | controller | `app/Http/Controllers/ApiController.php` | Unit/Controllers/ApiControllerTest.php | UserController, VaultController | port | todo |
| C387 | controller | `app/Http/Controllers/Auth/RegisterController.php` | — | FortifyServiceProvider | port | todo |
| C388 | controller | `app/Http/Controllers/Auth/ViewHelpers/AcceptInvitationShowViewHelper.php` | Unit/Controllers/Auth/ViewHelpers/AcceptInvitationShowViewHelperTest.php | AcceptInvitationController | port | todo |
| C389 | controller | `app/Http/Controllers/Controller.php` | — | AcceptInvitationController, AccountStorageController, ApiController, CancelAccountController, ContactArchiveController, ContactController, ContactFavoriteCon… | port | todo |
| C390 | controller | `app/Http/Controllers/Profile/WebauthnDestroyResponse.php` | — | AppServiceProvider | remove: WebAuthn removed | todo (remove) |
| C391 | controller | `app/Http/Controllers/Profile/WebauthnUpdateResponse.php` | — | AppServiceProvider | remove: WebAuthn removed | todo (remove) |
| C392 | middleware | `app/Http/Middleware/Authenticate.php` | — | AuthenticateWithTokenOnBasicAuth | port | todo |
| C393 | middleware | `app/Http/Middleware/AuthenticateWithTokenOnBasicAuth.php` | — | EnsureDavRequestsAreStateful | port | todo |
| C394 | middleware | `app/Http/Middleware/EnsureDavRequestsAreStateful.php` | — | — | port | todo |
| C395 | middleware | `app/Http/Middleware/EnsureSignupIsEnabled.php` | — | — | port | todo |
| C396 | middleware | `app/Http/Middleware/HandleInertiaRequests.php` | — | — | remove: Inertia/Ziggy removed (SPA) | todo (remove) |
| C397 | middleware | `app/Http/Middleware/SanctumSetUser.php` | — | EnsureDavRequestsAreStateful | port | todo |
| C398 | request | `app/Http/Requests/Auth/LoginRequest.php` | — | — | port | todo |
| C399 | resource | `app/Http/Resources/UserResource.php` | — | UserController | port | todo |
| C400 | resource | `app/Http/Resources/VaultResource.php` | — | VaultController | port | todo |
| C401 | other | `app/Interfaces/ServiceInterface.php` | — | AcceptInvitation, AddContactToGroup, AddContactToPost, AddPhotoToPost, AddPostToSliceOfLife, AssignLabel, AssignTag, AssociateAddressToContact, AssociateModu… | port | todo |
| C402 | listener | `app/Listeners/LoginListener.php` | — | — | port | todo |
| C403 | listener | `app/Listeners/WebauthnAuthenticateListener.php` | — | — | remove: WebAuthn removed | todo (remove) |
| C404 | logging | `app/Logging/CleanLogs.php` | — | — | port | todo |
| C405 | logging | `app/Logging/Loggable.php` | — | AddressBookSubscription, LoggingHandler | port | todo |
| C406 | logging | `app/Logging/LoggingHandler.php` | — | — | port | todo |
| C407 | mail | `app/Mail/TestEmailSent.php` | — | SendTestEmail | port | todo |
| C408 | mail | `app/Mail/UserInvited.php` | — | InviteUser | port | todo |
| C409 | mail | `app/Mail/UserNotificationChannelEmailCreated.php` | — | SendVerificationEmailChannel | port | todo |
| C410 | model | `app/Models/Account.php` | Unit/Models/AccountTest.php | AddressBookSubscription, AddressType, BaseService, CallReasonType, CancelAccount, ContactInformationType, CreateAccount, Currency, CurrencyIndexViewHelper, E… | port | todo |
| C411 | model | `app/Models/Address.php` | Unit/Models/AddressTest.php | ActionFeedAddress, AssociateAddressToContact, Contact, ContactModuleAddressController, ContactModuleAddressImageController, CreateAddress, DestroyAddress, Ge… | port | todo |
| C412 | model | `app/Models/AddressBookSubscription.php` | — | CreateAddressBookSubscription, DeleteLocalVCard, DeleteMultipleVCard, DeleteVCard, GetMultipleVCard, GetVCard, HasSubscription, LoggingHandler, NewAddressBoo… | port | todo |
| C413 | model | `app/Models/AddressType.php` | Unit/Models/AddressTypeTest.php | Account, Address, CreateAddressType, ImportAddress, ModuleContactAddressesViewHelper, PersonalizeAddressTypeIndexViewHelper, UpdateAddressType | port | todo |
| C414 | model | `app/Models/Call.php` | Unit/Models/CallTest.php | CallReason, CallReasonType, Contact, CreateCall, DestroyCall, ModuleCallsViewHelper, UpdateCall | port | todo |
| C415 | model | `app/Models/CallReason.php` | Unit/Models/CallReasonTest.php | Call, CallReasonType, CreateCallReason, PersonalizeCallReasonsIndexViewHelper, UpdateCallReason | port | todo |
| C416 | model | `app/Models/CallReasonType.php` | Unit/Models/CallReasonTypeTest.php | Account, CallReason, CreateCallReasonType, PersonalizeCallReasonsIndexViewHelper, UpdateCallReasonType | port | todo |
| C417 | model | `app/Models/Company.php` | Unit/Models/CompanyTest.php | CompanyIndexViewHelper, CompanyViewHelper, Contact, ContactModuleJobInformationController, CreateCompany, ModuleCompanyViewHelper, UpdateJobInformation, Vault | port | todo |
| C418 | model | `app/Models/Contact.php` | Unit/Models/ContactTest.php | Address, AddressBookSubscription, AuthServiceProvider, AvatarHelper, BaseService, CalDAVDates, CalDAVTasks, Call, CardDAVBackend, Company, CompanyIndexViewHe… | port, drop Scout `Searchable` engine config; keep DB search columns | todo |
| C419 | model | `app/Models/ContactFeedItem.php` | Unit/Models/ContactFeedItemTest.php | ActionFeedAddress, ActionFeedContactInformation, ActionFeedGenericContactInformation, ActionFeedGoal, ActionFeedLabelAssigned, ActionFeedMoodTrackingEvent, A… | port | todo |
| C420 | model | `app/Models/ContactImportantDate.php` | Unit/Models/ContactImportantDateTest.php | CalDAVDates, Contact, ContactImportantDateType, ContactImportantDatesController, ContactImportantDatesViewHelper, CreateContactImportantDate, CreateVault, De… | port | todo |
| C421 | model | `app/Models/ContactImportantDateType.php` | Unit/Models/ContactImportantDateTypeTest.php | ContactImportantDate, ContactImportantDateHelper, CreateContactImportantDateType, ImportImportantDates, UpdateContactImportantDateType, Vault, VaultImportant… | port | todo |
| C422 | model | `app/Models/ContactInformation.php` | Unit/Models/ContactInformationTest.php | Contact, CreateContactInformation, DestroyContactInformation, ExportContactInformation, ImportContactInformation, ModuleContactInformationViewHelper, UpdateC… | port | todo |
| C423 | model | `app/Models/ContactInformationType.php` | Unit/Models/ContactInformationTypeTest.php | Account, ContactInformation, CreateContactInformation, CreateContactInformationType, ModuleContactInformationViewHelper, PersonalizeContactInformationTypeInd… | port | todo |
| C424 | model | `app/Models/ContactReminder.php` | Unit/Models/ContactReminderTest.php | Contact, ContactModuleReminderController, ContactReminderHelper, CreateContactReminder, DestroyReminder, GrantVaultAccessToUser, ModuleRemindersViewHelper, P… | port | todo |
| C425 | model | `app/Models/ContactTask.php` | Unit/Models/ContactTaskTest.php | CalDAVTasks, Contact, CreateContactTask, ExportContactTask, ExportVCalendar, ImportContactTask, ImportVCalendar, ModuleContactTasksViewHelper, ToggleContactT… | port | todo |
| C426 | model | `app/Models/Currency.php` | Unit/Models/CurrencyTest.php | Account, ContactModuleLoanController, LifeEvent, Loan, PersonalizeCurrencyIndexViewHelper, SetupAccount | port | todo |
| C427 | model | `app/Models/Emotion.php` | Unit/Models/EmotionTest.php | Account, Call, LifeEvent, Note, SetupAccount | port | todo |
| C428 | model | `app/Models/File.php` | Unit/Models/FileTest.php | AddPhotoToPost, CancelAccount, Contact, ContactModuleDocumentController, ContactModulePhotoController, ContactPhotoController, ContactPhotosIndexViewHelper, … | port | todo |
| C429 | model | `app/Models/Gender.php` | Unit/Models/GenderTest.php | Account, Contact, ContactCreateViewHelper, ContactRelationshipsCreateViewHelper, CreateGender, ExportGender, ImportContact, ManageGenderIndexViewHelper, Upda… | port | todo |
| C430 | model | `app/Models/GiftOccasion.php` | Unit/Models/GiftOccasionTest.php | Account, CreateGiftOccasion, DestroyGiftOccasion, PersonalizeGiftOccasionViewHelper, UpdateGiftOccasion, UpdateGiftOccasionPosition | port | todo |
| C431 | model | `app/Models/GiftState.php` | Unit/Models/GiftStateTest.php | Account, CreateGiftState, DestroyGiftState, PersonalizeGiftStateViewHelper, UpdateGiftState, UpdateGiftStatePosition | port | todo |
| C432 | model | `app/Models/Goal.php` | Unit/Models/GoalTest.php | Contact, ContactGoalController, ContactModuleStreakController, CreateGoal, DestroyGoal, GoalHelper, GoalShowViewHelper, ModuleGoalsViewHelper, Streak, Toggle… | port | todo |
| C433 | model | `app/Models/Group.php` | Unit/Models/GroupTest.php | AuthServiceProvider, BaseService, CardDAVBackend, Contact, ContactModuleGroupController, CreateGroup, ExportKind, ExportMembers, ExportNames, ExportTimestamp… | port, drop Scout `Searchable` engine config; keep DB search columns | todo |
| C434 | model | `app/Models/GroupType.php` | Unit/Models/GroupTypeTest.php | Account, CreateGroupType, DestroyGroupType, DestroyGroupTypeRole, Group, GroupEditViewHelper, GroupTypeRole, ModuleGroupsViewHelper, PersonalizeGroupTypeView… | port | todo |
| C435 | model | `app/Models/GroupTypeRole.php` | Unit/Models/GroupTypeRoleTest.php | AddContactToGroup, CreateGroupTypeRole, DestroyGroupTypeRole, GroupShowViewHelper, GroupType, PersonalizeGroupTypeViewHelper, UpdateGroupTypeRole, UpdateGrou… | port | todo |
| C436 | model | `app/Models/Instance/Cron.php` | — | CronEvent | port | todo |
| C437 | model | `app/Models/Journal.php` | Unit/Models/JournalTest.php | AuthServiceProvider, CreateJournal, DestroyJournal, JournalController, JournalEditViewHelper, JournalIndexViewHelper, JournalMetric, JournalMetricIndexViewHe… | port | todo |
| C438 | model | `app/Models/JournalMetric.php` | Unit/Models/JournalMetricTest.php | CreateJournalMetric, DestroyJournalMetric, Journal, JournalMetricIndexViewHelper, PostMetric, UpdateJournalMetric | port | todo |
| C439 | model | `app/Models/Label.php` | Unit/Models/LabelTest.php | AssignLabel, Contact, ContactIndexViewHelper, ContactLabelController, CreateLabel, ImportLabels, ModuleLabelViewHelper, RemoveLabel, UpdateLabel, Vault, Vaul… | port | todo |
| C440 | model | `app/Models/LifeEvent.php` | Unit/Models/LifeEventTest.php | Contact, CreateLifeEvent, DestroyLifeEvent, LifeEventType, ModuleLifeEventViewHelper, TimelineEvent, ToggleLifeEvent, UpdateLifeEvent | port | todo |
| C441 | model | `app/Models/LifeEventCategory.php` | Unit/Models/LifeEventCategoryTest.php | CreateLifeEventCategory, CreateVault, LifeEventType, ModuleLifeEventViewHelper, UpdateLifeEventCategory, UpdateLifeEventCategoryPosition, UpdateLifeEventType… | port | todo |
| C442 | model | `app/Models/LifeEventType.php` | Unit/Models/LifeEventTypeTest.php | CreateLifeEvent, CreateLifeEventType, CreateVault, LifeEvent, LifeEventCategory, ModuleLifeEventViewHelper, UpdateLifeEvent, UpdateLifeEventType, UpdateLifeE… | port | todo |
| C443 | model | `app/Models/LifeMetric.php` | Unit/Models/LifeMetricTest.php | Contact, CreateLifeMetric, IncrementLifeMetric, UpdateLifeMetric, Vault, VaultLifeMetricsViewHelper | port | todo |
| C444 | model | `app/Models/Loan.php` | Unit/Models/LoanTest.php | Contact, CreateLoan, DestroyLoan, ModuleLoanViewHelper, ToggleLoan, UpdateLoan, Vault | port, drop Scout `Searchable` engine config; keep DB search columns | todo |
| C445 | model | `app/Models/Log.php` | — | AddressBookSubscription, CleanLogs, LoggingHandler | port | todo |
| C446 | model | `app/Models/Module.php` | Unit/Models/ModuleTest.php | Account, AssociateModuleToTemplatePage, ContactShowViewHelper, CreateModule, ModuleRow, PersonalizeModuleIndexViewHelper, PersonalizeTemplatePageShowViewHelp… | port | todo |
| C447 | model | `app/Models/ModuleRow.php` | Unit/Models/ModuleRowTest.php | Module, ModuleRowField | port | todo |
| C448 | model | `app/Models/ModuleRowField.php` | Unit/Models/ModuleRowFieldTest.php | ModuleRow | port | todo |
| C449 | model | `app/Models/MoodTrackingEvent.php` | Unit/Models/MoodTrackingEventTest.php | Contact, CreateMoodTrackingEvent, DestroyMoodTrackingEvent, MoodTrackingParameter, PostShowViewHelper, UpdateMoodTrackingEvent, VaultCalendarIndexViewHelper,… | port | todo |
| C450 | model | `app/Models/MoodTrackingParameter.php` | Unit/Models/MoodTrackingParameterTest.php | CreateMoodTrackingParameter, CreateVault, MoodTrackingEvent, UpdateMoodTrackingParameter, UpdateMoodTrackingParameterPosition, Vault, VaultSettingsIndexViewH… | port | todo |
| C451 | model | `app/Models/MultiAvatar.php` | — | AvatarHelper | port | todo |
| C452 | model | `app/Models/Note.php` | Unit/Models/NoteTest.php | Contact, CreateNote, DestroyNote, ModuleNotesViewHelper, NotesIndexViewHelper, UpdateNote, User, VaultSearchIndexViewHelper | port, drop Scout `Searchable` engine config; keep DB search columns | todo |
| C453 | model | `app/Models/Pet.php` | Unit/Models/PetTest.php | Contact, CreatePet, DestroyPet, ModulePetsViewHelper, PetCategory, UpdatePet | port | todo |
| C454 | model | `app/Models/PetCategory.php` | Unit/Models/PetCategoryTest.php | Account, CreatePetCategory, PersonalizePetCategoriesIndexViewHelper, Pet, UpdatePetCategory | port | todo |
| C455 | model | `app/Models/Post.php` | Unit/Models/PostTest.php | AddContactToPost, AddPhotoToPost, AddPostToSliceOfLife, AuthServiceProvider, Contact, CreatePost, DestroyPost, IncrementPostReadCounter, Journal, JournalPhot… | port | todo |
| C456 | model | `app/Models/PostMetric.php` | Unit/Models/PostMetricTest.php | CreatePostMetric, DestroyPostMetric, JournalMetric, Post, PostEditViewHelper, PostShowViewHelper, UpdatePostMetric | port | todo |
| C457 | model | `app/Models/PostSection.php` | Unit/Models/PostSectionTest.php | CreatePost, Post, PostEditViewHelper, PostShowViewHelper | port | todo |
| C458 | model | `app/Models/PostTemplate.php` | Unit/Models/PostTemplateTest.php | Account, CreatePost, CreatePostTemplate, DestroyPostTemplate, PersonalizePostTemplateSectionController, PersonalizePostTemplateSectionPositionController, Per… | port | todo |
| C459 | model | `app/Models/PostTemplateSection.php` | Unit/Models/PostTemplateSectionTest.php | CreatePostTemplateSection, DestroyPostTemplateSection, PersonalizePostTemplateViewHelper, PostCreateViewHelper, PostTemplate, UpdatePostTemplateSection, Upda… | port | todo |
| C460 | model | `app/Models/Pronoun.php` | Unit/Models/PronounTest.php | Account, Contact, ContactCreateViewHelper, ContactRelationshipsCreateViewHelper, CreatePronoun, PersonalizePronounIndexViewHelper, SetPronoun, UpdateContact,… | port | todo |
| C461 | model | `app/Models/QuickFact.php` | Unit/Models/QuickFactTest.php | Contact, ContactModuleQuickFactViewHelper, CreateQuickFact, UpdateQuickFact | port | todo |
| C462 | model | `app/Models/RelationshipGroupType.php` | Unit/Models/RelationshipGroupTypeTest.php | Account, ContactRelationshipsCreateViewHelper, CreateRelationshipGroupType, ModuleFamilySummaryViewHelper, PersonalizeRelationshipIndexViewHelper, Relationsh… | port | todo |
| C463 | model | `app/Models/RelationshipType.php` | Unit/Models/RelationshipTypeTest.php | ContactRelationshipsCreateViewHelper, CreateRelationshipType, ModuleFamilySummaryViewHelper, PersonalizeRelationshipIndexViewHelper, RelationshipGroupType, S… | port | todo |
| C464 | model | `app/Models/Religion.php` | Unit/Models/ReligionTest.php | Account, Contact, CreateReligion, DestroyReligion, ModuleReligionViewHelper, PersonalizeReligionViewHelper, UpdateReligion, UpdateReligion, UpdateReligionPos… | port | todo |
| C465 | model | `app/Models/SliceOfLife.php` | Unit/Models/SliceOfLifeTest.php | AddPostToSliceOfLife, AuthServiceProvider, CreateSliceOfLife, DestroySliceOfLife, Journal, JournalShowViewHelper, Post, PostEditViewHelper, RemoveSliceOfLife… | port | todo |
| C466 | model | `app/Models/Streak.php` | Unit/Models/StreakTest.php | Goal, ToggleStreak | port | todo |
| C467 | model | `app/Models/SyncToken.php` | — | AddressBookSubscription, CleanSyncToken, SyncDAVBackend, TokenDeleteEvent | port | todo |
| C468 | model | `app/Models/Tag.php` | Unit/Models/TagTest.php | AssignTag, CreateTag, JournalShowViewHelper, Post, PostEditViewHelper, PostShowViewHelper, RemoveTag, UpdateTag, Vault, VaultSettingsIndexViewHelper | port | todo |
| C469 | model | `app/Models/Template.php` | Unit/Models/TemplateTest.php | Account, Contact, ContactCreateViewHelper, CreateTemplate, DestroyTemplate, PersonalizeTemplateIndexViewHelper, PersonalizeTemplatePagesController, Personali… | port | todo |
| C470 | model | `app/Models/TemplatePage.php` | Unit/Models/TemplatePageTest.php | AssociateModuleToTemplatePage, ContactPageController, ContactShowViewHelper, CreateTemplate, CreateTemplatePage, DestroyTemplatePage, Module, PersonalizeTemp… | port | todo |
| C471 | model | `app/Models/TimelineEvent.php` | Unit/Models/TimelineEventTest.php | Contact, CreateLifeEvent, CreateTimelineEvent, DestroyLifeEvent, DestroyTimelineEvent, LifeEvent, ModuleLifeEventViewHelper, ToggleTimelineEvent, UpdateLifeE… | port | todo |
| C472 | model | `app/Models/User.php` | Unit/Models/UserTest.php | AcceptInvitation, AcceptInvitationController, Account, ActionFeedAddress, ActionFeedMoodTrackingEvent, AddressBookSubscription, AppServiceProvider, AttemptTo… | port, drop `webauthnKeys`, `userTokens` relations | todo |
| C473 | model | `app/Models/UserNotificationChannel.php` | Unit/Models/UserNotificationChannelTest.php | AcceptInvitation, ContactReminder, CreateUserNotificationChannel, DestroyUserNotificationChannel, NotificationsController, NotificationsIndexViewHelper, Noti… | port, drop `TYPE_TELEGRAM` | todo |
| C474 | model | `app/Models/UserNotificationSent.php` | Unit/Models/UserNotificationSentTest.php | ProcessScheduledContactReminders, ReminderTriggered, SendTestEmail, UserNotificationChannel | port | todo |
| C475 | model | `app/Models/UserToken.php` | — | AttemptToAuthenticateSocialite, User | remove: OAuth/Socialite removed | todo (remove) |
| C476 | model | `app/Models/Vault.php` | Unit/Models/VaultTest.php | AbstractCalDAVBackend, Account, AddressBookSubscription, BaseService, CalDAVBackend, CardDAVBackend, Company, CompanyIndexViewHelper, CompanyViewHelper, Cont… | port, drop Scout-related bits | todo |
| C477 | model | `app/Models/VaultQuickFactsTemplate.php` | Unit/Models/VaultQuickFactsTemplateTest.php | ContactModuleQuickFactViewHelper, ContactShowViewHelper, CreateQuickFactTemplate, CreateVault, QuickFact, UpdateQuickFactTemplate, UpdateQuickFactTemplatePos… | port | todo |
| C478 | model | `app/Models/WebauthnKey.php` | — | UserProfile, WebauthnAuthenticateListener | remove: WebAuthn removed | todo (remove) |
| C479 | notification | `app/Notifications/ReminderTriggered.php` | — | ProcessScheduledContactReminders, SendTestTelegramNotification, TestReminders | port, drop the Telegram channel | todo |
| C480 | other | `app/Policies/VaultPolicy.php` | — | — | port | todo |
| C481 | provider | `app/Providers/AppServiceProvider.php` | — | — | port, drop Socialite, Pulse, Telescope wiring | todo |
| C482 | provider | `app/Providers/AuthServiceProvider.php` | — | — | port | todo |
| C483 | provider | `app/Providers/DAVServiceProvider.php` | — | — | port | todo |
| C484 | provider | `app/Providers/FortifyServiceProvider.php` | — | — | port | todo |
| C485 | provider | `app/Providers/JetstreamServiceProvider.php` | — | — | port | todo |
| C486 | provider | `app/Providers/TelescopeServiceProvider.php` | — | AppServiceProvider | remove: Telescope removed | todo (remove) |
| C487 | service | `app/Services/BaseService.php` | Unit/Services/BaseServiceTest.php | AcceptInvitation, AddContactToPost, AddPhotoToPost, AddPostToSliceOfLife, AssignLabel, AssignTag, AssociateAddressToContact, AssociateModuleToTemplatePage, C… | port | todo |
| C488 | service | `app/Services/QueuableService.php` | — | AddContactToGroup, CancelAccount, CleanLogs, CleanSyncToken, DestroyContact, DestroyContactImportantDate, DestroyContactTask, DestroyGroup, GetGPSCoordinate,… | port | todo |
| C489 | trait | `app/Traits/DAVFormat.php` | — | ImportVCalendar, ImportVCard | port | todo |
| C490 | trait | `app/Traits/HasUuids.php` | Unit/Traits/HasUuidsTest.php | ContactImportantDate, ContactTask, Group | port | todo |
| C491 | trait | `app/Traits/JsonRespondController.php` | — | ApiController | port | todo |
<!-- END GENERATED classes -->

## 10. Frontend changes

The Vue app keeps its pages and components. Every page in §2's `Vue page`
column changes its data layer from Inertia props to the generated API client.
The rows below are the pages and components whose change goes beyond that.

| ID | File(s) | Change | Status |
|---|---|---|---|
| P001 | `resources/js/app.js`, `ssr.js`, `bootstrap.js`, `methods.js`, `resources/views/app.blade.php`, `vite.config.js` | Standalone Vite SPA: drop Inertia, Ziggy, SSR, Sentry; add a router and the API client | todo |
| P002 | every `InertiaLink`/`Link`, `router.visit`, `useForm`, `usePage`, `$page.props` use | Replace with router links and API calls; layout data from an API call | todo |
| P003 | every `route('...')` call in Vue (`logout`, `profile.show`, `vault.index`, `settings.index`, `register`, `login`, `password.*`, `two-factor.*`, `verification.send`, …) | Replace with router paths / client calls | todo |
| P004 | `Components/Uploadcare.vue` and its users `Shared/Modules/Photos.vue`, `Shared/Modules/Documents.vue`, `Pages/Vault/Contact/Show.vue` (avatar), `Pages/Vault/Contact/Photos/Index.vue`, `Pages/Vault/Journal/Post/Edit.vue`, `Pages/Vault/Journal/Slices/Show.vue` | New local upload component (multipart to the API); image URLs from the file endpoint | todo |
| P005 | `Pages/Auth/ExternalProviders.vue`, `Pages/Profile/Partials/UpdateProviders.vue`, provider buttons in `Auth/Login.vue`, `Auth/Register.vue`, `Profile/Show.vue` | remove: OAuth | todo (remove) |
| P006 | `Pages/Webauthn/*` (7 files), WebAuthn parts of `Auth/Login.vue`, `Auth/TwoFactorChallenge.vue`, `Profile/Show.vue`, `Components/Jetstream/ConfirmsPassword.vue`, `methods.js` | remove: WebAuthn | todo (remove) |
| P007 | `Pages/Settings/Notifications/Partials/Telegram.vue`, its use in `Settings/Notifications/Index.vue` | remove: Telegram | todo (remove) |
| P008 | `Pages/Profile/Partials/DeleteUserForm.vue` line 65 | remove the customers.monicahq.com line (and its `lang/*.json` key) | todo (remove) |
| P009 | `resources/js/sentry.js`, Sentry imports in `app.js`/`ssr.js` | remove: Sentry | todo (remove) |
| P010 | `Pages/Auth/Login.vue` | Data (signup enabled, deletion notice, wallpaper, beta) from the instance info endpoint | todo |
| P011 | `Pages/API/Index.vue`, `API/Partials/ApiTokenManager.vue` | Bearer token management via the API | todo |
| P012 | `Pages/Profile/Show.vue` and partials | Profile, password, 2FA, sessions via the API | todo |
| P013 | `Pages/Vault/Reports/Address/Cities/Index.vue`, `Countries/Index.vue` | Depends on Q1 (Wikipedia) | todo |
| P014 | `lang/*.json` keys used only by removed features | remove | todo (remove) |
| P015 | `Pages/TermsOfService.vue`, `Pages/PrivacyPolicy.vue` | Markdown from a static asset or an API endpoint | todo |
| P016 | Date/name/number formatting now done client side (X015) for every component that showed server-formatted strings | todo |

## 11. Findings and open questions

Findings in the current code that affect what "parity" means:

- **F1. Undefined gate on slices of life.** `routes/web.php` protects
  `journals/{journal}/slices/{slice}/*` with `can:slice-owner,journal,slice`,
  but `AuthServiceProvider` defines `sliceOfLife-owner`. Laravel denies an
  undefined ability, so R157–R162 (show, edit, update, cover update/destroy,
  destroy) most likely answer 403 today. The port will apply the intended
  check (slice belongs to the journal). Confirm when approving the spec.
- **F2. Dead routes.** R172 (`vault.companies.show`), R174
  (`vault.settings.update`), R179/R183 (settings label/tag index), R232
  (`settings.user.show`), R312–R314 (personalize module store/update/destroy)
  point at controller methods that don't exist. Their URLs are built by
  ViewHelpers but no Vue code calls them. They are marked `remove`.
- **F3.** `ContactModuleAddressImageController` (R083) is the only consumer of
  Mapbox; "View on map" links in `Shared/Modules/Addresses.vue` use the user's
  maps preference (Google Maps / OpenStreetMap URLs built from the address)
  and are kept, as is the maps preference (R218).
- **F4.** `config/unsplash.php` is unused; the login wallpapers are local files.
- **F5.** `VaultLifeEventController` (C324) has no route. Nothing to port
  beyond what the services it calls already cover.

Bugs found while drafting the spec. The spec specifies the intended
behaviour, so the port fixes them:

- **F6.** `ToggleFavoriteContact` writes an `unfavorited` feed item on a
  contact's first favorite (R052).
- **F7.** `UpdatePet` looks the pet up by the *new* category, so changing the
  category returns 404 (R099).
- **F8.** `UpdateCall` ignores `description` (R113); the task controller always
  sends `description: null` on update (R109).
- **F9.** `CreateTimelineEvent` reads `summary` instead of `label`, so timeline
  event labels are never saved (R118).
- **F10.** `UpdateReligion` does `findOrFail(null)` when clearing the religion,
  so clearing returns 404 (R094).
- **F11.** `RemoveModuleFromTemplatePage` uses `toggle()`, so removing a module
  that isn't on the page adds it (R310).
- **F12.** Notification channel verification accepts any UUID instead of the
  channel's `verification_token` (R224).
- **F13.** `RemoveVaultAccess` deletes the user's scheduled reminders in every
  vault, not just the one they leave (R178).
- **F14.** The vault reminders page matches reminders by month number only, so
  next year's reminders show under this month (R023); the mood report skips
  the last day of each month (R030); the address report counts the contacts
  of the first matching address only (R027–R029).
- **F15.** The post view counter requires an editor, so a viewer opening a
  post gets an error (R142).
- **F16.** Every post autosave detaches and re-adds all mentioned contacts and
  writes an `added_to_post` feed item for each (R144).
- **F17.** Delete endpoints for contact documents/photos and post photos
  accept any file id in the vault (`DestroyFile`); loan, timeline and life
  event paths don't check that the contact in the path is involved. The spec
  answers 404 for anything not attached to the parent in the path.
- **F19.** `RemoveVaultAccess` (R178) soft-deletes the member's contact in the
  vault and expects the `user_vault` row to go with it, but a soft delete
  doesn't cascade: the removed user stays a member and keeps access. The port
  deletes the membership.
- **F20.** `GrantVaultAccessToUser` (R176) on an existing member inserts a
  second membership and a second contact (no unique key). The spec answers 409.
- **F21.** `ToggleUserNotificationChannel` (R226) deletes the contact reminders
  themselves (`->contactReminders->each->delete()`) when a channel is
  deactivated, instead of only that channel's schedules. The port deletes the
  schedules. `ProcessScheduledContactReminders` does the same when a channel
  reaches 10 failures; the port deletes that channel's schedules there too.
- **F22.** `CreateLoan`/`UpdateLoan` attach the loaners and loanees with two
  `syncWithoutDetaching` loops on `contact_loan` (one row per loaner and
  loanee pair); each call overwrites the other key of existing rows, so a loan
  with several loaners and several loanees keeps only one loaner. The port
  stores every pair.
- **F18.** `can_be_deleted` is enforced only by the UI for templates, template
  pages, post templates and sections, and contact information types; deleting
  the contact information page breaks the contact page. The spec answers 409.

Decisions (2026-10-19: the user asked to take the rewrite to the finish
without stopping at the ★s; the recommendations below are applied):

- Q1 → (a), changed from (b) during Phase 4: the server asks Wikipedia when
  a city/country report is requested (5 s timeout, empty fields on failure,
  `WIKIPEDIA_URL=` disables it), so the SPA makes no request outside the
  generated client (plan, Done when). The server already makes outbound
  calls for the CardDAV client.
- Q4 → self-service deletion stays, behind `ENABLE_ACCOUNT_DELETION` (off).
- Q5 → fixed: an invited administrator stays an administrator.
- Q6 → goal streak numbers fixed; un-settling a loan and un-completing a task
  clear `settled_at` / `completed_at`; the tag post count stays vault-wide.
- Q7 → deleting a journal, a slice or a cover image deletes their files.
- Q8 → forgot-password answers 204 whether or not the email has an account.
- Q9 → account-level reads are open to members, writes to administrators.
- F1, F6–F21: fixed as described.

Open questions (as asked at the phase 1 ★):

- **Q1. Wikipedia lookups (I004).** The city and country report pages call
  Wikipedia for a summary and thumbnail. The plan allows no external service
  except SMTP. Options: (a) keep it as an optional call the server makes when
  the page is requested, failing quietly offline; (b) let the browser call
  Wikipedia directly; (c) remove the panel (a feature cut, needs approval).
  Recommendation: (b), the server stays free of outbound calls and the page
  is unchanged.
- **Q2. Unused GeoIP package (I013).** `stevebauman/location` has no call
  sites, so nothing to port. Listed only so it isn't mistaken for a feature.
- **Q3. Markdown.** Notes, posts and other text fields are rendered to HTML on
  the server today. The API returns raw text and the frontend renders it with
  the same rules. (Default unless you object.)
- **Q4. Self-service account deletion (V032).** It is off in
  `config/jetstream.php`, so `DeleteUserForm` never shows. Keep it behind a
  config flag that defaults to off (default), or drop it. If kept, the spec
  adds one rule: 409 when the user is the account's last administrator.
- **Q5. Invited administrators.** `AcceptInvitation` sets
  `is_account_administrator = false`, so inviting someone as an administrator
  has no effect. The spec keeps this. Fix it (recommended)?
- **Q6. Quirks kept as today, fix or keep?** Goal streak stats (`max_streak`
  is 1 with no streaks, `current_streak` is the run length minus 1);
  un-settling a loan and un-completing a task set `settled_at` /
  `completed_at` to now; a tag's `post_count` counts posts in the whole
  vault, not the journal. Recommendation: fix the streak numbers and the
  timestamps (clear them when un-setting); keep the tag count.
- **Q7. Orphaned files.** Deleting a journal or a slice leaves the `files`
  rows (and, with local storage, the bytes) of post photos and slice covers;
  replacing a slice cover keeps the old file. Recommendation: delete them, so
  they stop counting against the storage limit.
- **Q8. Forgot password reveals accounts.** An unknown email gets a 422 today,
  which tells anyone whether an email has an account. Keep, or answer 200
  either way (recommended)?
- **Q9. Settings reads open to all account members.** The settings pages are
  admin-only today, but contact forms already need the same lists (genders,
  pronouns, …). The spec makes every account-level read `account:member` and
  every write `account:admin`, including the users list (emails of all
  account users). OK?
