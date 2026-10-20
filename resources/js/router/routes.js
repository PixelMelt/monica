// The SPA's pages: the web routes of the Laravel app, with their URLs and
// names (so `route('contact.show', …)` keeps working), the page component,
// and the loader that fetches its props from the API (`loaders/*.js`).
//
// `access`: `guest` (logged-out only), `any`, `user` (logged in, email
// not verified yet is fine), or `verified` (the default).

import * as auth from '@/loaders/auth';
import * as settings from '@/loaders/settings';
import * as personalize from '@/loaders/personalize';
import * as vaults from '@/loaders/vaults';
import * as contacts from '@/loaders/contacts';
import * as groups from '@/loaders/groups';
import * as journals from '@/loaders/journals';
import * as dashboard from '@/loaders/dashboard';
import * as modulesA from '@/loaders/modules-a';
import * as modulesB from '@/loaders/modules-b';

const page = (name) => () => import(`../Pages/${name}.vue`);

const r = (path, name, component, load, extra = {}) => ({
  path,
  name,
  component: page(component),
  meta: { load, component, ...extra },
});

export default [
  { path: '/', name: 'home', component: page('Vault/Index'), meta: { load: auth.home, component: 'Vault/Index' } },

  // Authentication (Fortify, Jetstream)
  r('/login', 'login', 'Auth/Login', auth.login, { access: 'guest' }),
  r('/register', 'register', 'Auth/Register', auth.register, { access: 'guest' }),
  r('/forgot-password', 'password.request', 'Auth/ForgotPassword', auth.forgotPassword, { access: 'guest' }),
  r('/reset-password/:token', 'password.reset', 'Auth/ResetPassword', auth.resetPassword, { access: 'guest' }),
  r('/two-factor-challenge', 'two-factor.login', 'Auth/TwoFactorChallenge', auth.twoFactorChallenge, {
    access: 'guest',
  }),
  r('/email/verify', 'verification.notice', 'Auth/VerifyEmail', auth.verifyEmail, { access: 'user' }),
  r('/email/verify/:id/:hash', 'verification.verify', 'Auth/VerifyEmail', auth.verifyEmailLink, { access: 'user' }),
  r('/user/confirm-password', 'password.confirm', 'Auth/ConfirmPassword', auth.confirmPassword, { access: 'user' }),
  r('/invitation/:code', 'invitation.show', 'Auth/AcceptInvitation', auth.acceptInvitation, { access: 'guest' }),
  r('/terms-of-service', 'terms.show', 'TermsOfService', auth.terms, { access: 'any' }),
  r('/privacy-policy', 'policy.show', 'PrivacyPolicy', auth.policy, { access: 'any' }),

  // User and account settings
  r('/user/profile', 'profile.show', 'Profile/Show', settings.profile, { access: 'user' }),
  r('/user/api-tokens', 'api-tokens.index', 'API/Index', settings.apiTokens),
  r('/settings', 'settings.index', 'Settings/Index', settings.index),
  r('/settings/preferences', 'settings.preferences.index', 'Settings/Preferences/Index', settings.preferences),
  r('/settings/notifications', 'settings.notifications.index', 'Settings/Notifications/Index', settings.notifications),
  r(
    '/settings/notifications/:notification/verify/:uuid',
    'settings.notifications.verification.store',
    'Settings/Notifications/Index',
    settings.verifyNotificationChannel,
  ),
  r(
    '/settings/notifications/:notification/logs',
    'settings.notifications.log.index',
    'Settings/Notifications/Logs/Index',
    settings.notificationLogs,
  ),
  r('/settings/users', 'settings.user.index', 'Settings/Users/Index', settings.users),
  r('/settings/users/create', 'settings.user.create', 'Settings/Users/Create', settings.createUser),
  r('/settings/storage', 'settings.storage.index', 'Settings/Storage/Index', settings.storage),
  r('/settings/cancel', 'settings.cancel.index', 'Settings/CancelAccount/Index', settings.cancel),

  // Personalize the account
  r('/settings/personalize', 'settings.personalize.index', 'Settings/Personalize/Index', personalize.index),
  r(
    '/settings/personalize/relationships',
    'settings.personalize.relationship.index',
    'Settings/Personalize/Relationships/Index',
    personalize.relationships,
  ),
  r(
    '/settings/personalize/callReasonTypes',
    'settings.personalize.call_reasons.index',
    'Settings/Personalize/CallReasons/Index',
    personalize.callReasons,
  ),
  r(
    '/settings/personalize/giftOccasions',
    'settings.personalize.gift_occasions.index',
    'Settings/Personalize/GiftOccasions/Index',
    personalize.giftOccasions,
  ),
  r(
    '/settings/personalize/giftStates',
    'settings.personalize.gift_states.index',
    'Settings/Personalize/GiftStates/Index',
    personalize.giftStates,
  ),
  r(
    '/settings/personalize/postTemplates',
    'settings.personalize.post_templates.index',
    'Settings/Personalize/PostTemplates/Index',
    personalize.postTemplates,
  ),
  r(
    '/settings/personalize/groupTypes',
    'settings.personalize.group_types.index',
    'Settings/Personalize/GroupTypes/Index',
    personalize.groupTypes,
  ),
  r(
    '/settings/personalize/genders',
    'settings.personalize.gender.index',
    'Settings/Personalize/Genders/Index',
    personalize.genders,
  ),
  r(
    '/settings/personalize/pronouns',
    'settings.personalize.pronoun.index',
    'Settings/Personalize/Pronouns/Index',
    personalize.pronouns,
  ),
  r(
    '/settings/personalize/addressTypes',
    'settings.personalize.address_type.index',
    'Settings/Personalize/AddressTypes/Index',
    personalize.addressTypes,
  ),
  r(
    '/settings/personalize/petCategories',
    'settings.personalize.pet_category.index',
    'Settings/Personalize/PetCategories/Index',
    personalize.petCategories,
  ),
  r(
    '/settings/personalize/contactInformationType',
    'settings.personalize.contact_information_type.index',
    'Settings/Personalize/ContactInformationTypes/Index',
    personalize.contactInformationTypes,
  ),
  r(
    '/settings/personalize/templates',
    'settings.personalize.template.index',
    'Settings/Personalize/Templates/Index',
    personalize.templates,
  ),
  r(
    '/settings/personalize/templates/:template',
    'settings.personalize.template.show',
    'Settings/Personalize/Templates/Show',
    personalize.template,
  ),
  r(
    '/settings/personalize/modules',
    'settings.personalize.module.index',
    'Settings/Personalize/Modules/Index',
    personalize.modules,
  ),
  r(
    '/settings/personalize/currencies',
    'settings.personalize.currency.index',
    'Settings/Personalize/Currencies/Index',
    personalize.currencies,
  ),
  r(
    '/settings/personalize/religions',
    'settings.personalize.religions.index',
    'Settings/Personalize/Religions/Index',
    personalize.religions,
  ),

  // Vaults
  r('/vaults', 'vault.index', 'Vault/Index', vaults.index),
  r('/vaults/create', 'vault.create', 'Vault/Create', vaults.create),
  r('/vaults/:vault', 'vault.show', 'Vault/Dashboard/Index', dashboard.dashboard),
  r('/vaults/:vault/edit', 'vault.edit', 'Vault/Edit', vaults.edit),
  r('/vaults/:vault/calendar', 'vault.calendar.index', 'Vault/Calendar/Index', vaults.calendar),
  r(
    '/vaults/:vault/calendar/years/:year/months/:month',
    'vault.calendar.month',
    'Vault/Calendar/Index',
    vaults.calendar,
  ),
  r('/vaults/:vault/reminders', 'vault.reminder.index', 'Vault/Dashboard/Reminder/Index', dashboard.reminders),
  r('/vaults/:vault/tasks', 'vault.tasks.index', 'Vault/Dashboard/Task/Index', dashboard.tasks),
  r('/vaults/:vault/reports', 'vault.reports.index', 'Vault/Reports/Index', vaults.reports),
  r(
    '/vaults/:vault/reports/addresses',
    'vault.reports.addresses.index',
    'Vault/Reports/Address/Index',
    vaults.addressReport,
  ),
  r(
    '/vaults/:vault/reports/addresses/city/:city',
    'vault.reports.addresses.cities.show',
    'Vault/Reports/Address/Cities/Index',
    vaults.cityReport,
  ),
  r(
    '/vaults/:vault/reports/addresses/country/:country',
    'vault.reports.addresses.countries.show',
    'Vault/Reports/Address/Countries/Index',
    vaults.countryReport,
  ),
  r(
    '/vaults/:vault/reports/moodTrackingEvents',
    'vault.reports.mood_tracking_events.index',
    'Vault/Reports/MoodTrackingEvents/Index',
    vaults.moodReport,
  ),
  r(
    '/vaults/:vault/reports/importantDates',
    'vault.reports.important_dates.index',
    'Vault/Reports/ImportantDate/Index',
    vaults.importantDateReport,
  ),
  r('/vaults/:vault/files', 'vault.files.index', 'Vault/Files/Index', vaults.files),
  r('/vaults/:vault/files/photos', 'vault.files.photos', 'Vault/Files/Index', vaults.files, { fileType: 'photo' }),
  r('/vaults/:vault/files/documents', 'vault.files.documents', 'Vault/Files/Index', vaults.files, {
    fileType: 'document',
  }),
  r('/vaults/:vault/files/avatars', 'vault.files.avatars', 'Vault/Files/Index', vaults.files, { fileType: 'avatar' }),
  r('/vaults/:vault/companies', 'vault.companies.index', 'Vault/Companies/Index', vaults.companies),
  r('/vaults/:vault/settings', 'vault.settings.index', 'Vault/Settings/Index', vaults.settings),
  r('/vaults/:vault/search', 'vault.search.index', 'Vault/Search/Index', vaults.search),

  // Contacts
  r('/vaults/:vault/contacts', 'contact.index', 'Vault/Contact/Index', contacts.index),
  r('/vaults/:vault/contacts/labels/:label', 'contact.label.index', 'Vault/Contact/Index', contacts.index),
  r('/vaults/:vault/contacts/create', 'contact.create', 'Vault/Contact/Create', contacts.create),
  r('/vaults/:vault/contacts/:contact', 'contact.show', 'Vault/Contact/Show', contacts.show),
  r('/vaults/:vault/contacts/:contact/tabs/:slug', 'contact.page.show', 'Vault/Contact/Show', contacts.show),
  r('/vaults/:vault/contacts/:contact/edit', 'contact.edit', 'Vault/Contact/Names/Edit', contacts.edit),
  r('/vaults/:vault/contacts/:contact/move', 'contact.move.show', 'Vault/Contact/Move', contacts.move),
  r('/vaults/:vault/contacts/:contact/update-template', 'contact.blank', 'Vault/Contact/Blank', contacts.blank),
  r(
    '/vaults/:vault/contacts/:contact/dates',
    'contact.date.index',
    'Vault/Contact/ImportantDates/Index',
    contacts.importantDates,
  ),
  r('/vaults/:vault/contacts/:contact/notes', 'contact.note.index', 'Vault/Contact/Notes/Index', modulesA.notes),
  r('/vaults/:vault/contacts/:contact/goals/:goal', 'contact.goal.show', 'Vault/Contact/Goals/Show', modulesA.goal),
  r(
    '/vaults/:vault/contacts/:contact/relationships/create',
    'contact.relationships.create',
    'Vault/Contact/Relationships/Create',
    modulesB.createRelationship,
  ),
  r('/vaults/:vault/contacts/:contact/photos', 'contact.photo.index', 'Vault/Contact/Photos/Index', modulesB.photos),
  r(
    '/vaults/:vault/contacts/:contact/photos/:photo',
    'contact.photo.show',
    'Vault/Contact/Photos/Show',
    modulesB.photo,
  ),

  // Groups
  r('/vaults/:vault/groups', 'group.index', 'Vault/Group/Index', groups.index),
  r('/vaults/:vault/groups/:group', 'group.show', 'Vault/Group/Show', groups.show),
  r('/vaults/:vault/groups/:group/edit', 'group.edit', 'Vault/Group/Edit', groups.edit),

  // Journals
  r('/vaults/:vault/journals', 'journal.index', 'Vault/Journal/Index', journals.index),
  r('/vaults/:vault/journals/create', 'journal.create', 'Vault/Journal/Create', journals.create),
  r('/vaults/:vault/journals/:journal', 'journal.show', 'Vault/Journal/Show', journals.show),
  r('/vaults/:vault/journals/:journal/years/:year', 'journal.year', 'Vault/Journal/Show', journals.show),
  r('/vaults/:vault/journals/:journal/photos', 'journal.photo.index', 'Vault/Journal/Photo/Index', journals.photos),
  r('/vaults/:vault/journals/:journal/edit', 'journal.edit', 'Vault/Journal/Edit', journals.edit),
  r(
    '/vaults/:vault/journals/:journal/posts/create',
    'post.create',
    'Vault/Journal/Post/Template',
    journals.chooseTemplate,
  ),
  r(
    '/vaults/:vault/journals/:journal/posts/template/:template',
    'post.store',
    'Vault/Journal/Post/Template',
    journals.createPost,
  ),
  r('/vaults/:vault/journals/:journal/posts/:post', 'post.show', 'Vault/Journal/Post/Show', journals.post),
  r('/vaults/:vault/journals/:journal/posts/:post/edit', 'post.edit', 'Vault/Journal/Post/Edit', journals.editPost),
  r('/vaults/:vault/journals/:journal/slices', 'slices.index', 'Vault/Journal/Slices/Index', journals.slices),
  r('/vaults/:vault/journals/:journal/slices/:slice', 'slices.show', 'Vault/Journal/Slices/Show', journals.slice),
  r(
    '/vaults/:vault/journals/:journal/slices/:slice/edit',
    'slices.edit',
    'Vault/Journal/Slices/Edit',
    journals.editSlice,
  ),
  r(
    '/vaults/:vault/journals/:journal/metrics',
    'journal_metrics.index',
    'Vault/Journal/Metrics/Index',
    journals.metrics,
  ),

  { path: '/:pathMatch(.*)*', name: 'not-found', component: page('Error'), meta: { access: 'any', status: 404 } },
];
