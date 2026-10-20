// The props of the settings pages, built from API objects as their ViewHelpers
// built them from models.

import dayjs from 'dayjs';
import { trans } from 'laravel-vue-i18n';
import { route } from '@/router';
import { session as current, userName, localeIetf } from '@/lib/session';
import { contactName, formatDate, formatUserDate, isoFormat, sortByCollator } from '@/lib/format';
import { formatFileSize } from '@/lib/helpers';

const UNITS = ['year', 'month', 'week', 'day', 'hour', 'minute', 'second'];

/**
 * Carbon's `diffForHumans()` of a past timestamp ("3 minutes ago"): the
 * largest whole unit, in the user's language.
 */
export function diffForHumans(value) {
  const then = dayjs(value);
  const now = dayjs();
  const format = new Intl.RelativeTimeFormat(localeIetf(), { numeric: 'always' });
  for (const unit of UNITS) {
    const count = now.diff(then, unit);
    if (count >= 1 || unit === 'second') return format.format(-count, unit);
  }
  return null;
}

// --- SettingsIndexViewHelper -------------------------------------------

/** `SettingsIndexViewHelper::data`. */
export const settingsIndex = (user) => ({
  is_account_administrator: user.is_account_administrator,
  url: {
    preferences: { index: route('settings.preferences.index') },
    notifications: { index: route('settings.notifications.index') },
    users: { index: route('settings.user.index') },
    personalize: { index: route('settings.personalize.index') },
    storage: { index: route('settings.storage.index') },
    cancel: { index: route('settings.cancel.index') },
  },
});

// --- Jetstream (profile, API tokens) -----------------------------------

/** Jetstream's `UserProfileController::sessions` (one session). */
export const session = (s) => ({
  agent: {
    is_desktop: s.is_desktop,
    platform: s.platform,
    browser: s.browser,
  },
  ip_address: s.ip_address,
  is_current_device: s.is_current_device,
  last_active: diffForHumans(s.last_activity),
});

/** `UserProfile`: `config('localizer.supported_locales')` with their names, by name. */
export const locales = () => sortByCollator(current.instance.locales.map((l) => ({ id: l.id, name: l.name })));

/** Jetstream's `ApiTokenController::index` (one token). */
export const apiToken = (t) => ({
  ...t,
  last_used_ago: t.last_used_at ? diffForHumans(t.last_used_at) : null,
});

// --- UserPreferencesIndexViewHelper ------------------------------------

/** `UserPreferencesIndexViewHelper::data`. */
export const preferences = (prefs) => ({
  help: help(prefs),
  name_order: nameOrder(prefs),
  date_format: dateFormat(prefs),
  timezone: timezone(prefs),
  number_format: numberFormat(prefs),
  distance_format: distanceFormat(prefs),
  maps: maps(prefs),
  locale: locale(prefs),
  url: {
    settings: route('settings.index'),
    back: route('settings.index'),
  },
});

/** `UserPreferencesIndexViewHelper::dtoHelp`. */
export const help = (prefs) => ({ help_shown: prefs.help_shown });

/** `UserPreferencesIndexViewHelper::dtoNameOrder`. */
export const nameOrder = (prefs) => ({
  name_example: contactName(
    { first_name: 'James', last_name: 'Bond', nickname: '007', middle_name: 'W.', maiden_name: 'Muller' },
    prefs.name_order,
  ),
  name_order: prefs.name_order,
});

const DATE_FORMATS = ['MMM DD, YYYY', 'DD MMM YYYY', 'YYYY/MM/DD', 'DD/MM/YYYY'];

/** `UserPreferencesIndexViewHelper::dtoDateFormat` (`Carbon::now()` in the app timezone, UTC). */
export function dateFormat(prefs) {
  const now = new Date().toISOString();
  return {
    dates: DATE_FORMATS.map((format, i) => ({ id: i + 1, format, value: isoFormat(now, format, null) })),
    date_format: prefs.date_format,
    human_date_format: isoFormat(now, prefs.date_format, null),
  };
}

/** `UserPreferencesIndexViewHelper::dtoTimezone`. */
export const timezone = (prefs) => ({ timezone: prefs.timezone });

/**
 * `MonetaryNumberHelper::formatValue` of 123456 for a user with the
 * locale's default number format.
 */
const localeDefaultNumber = () =>
  new Intl.NumberFormat(localeIetf(), { maximumFractionDigits: 3 }).format(123456 / 100).trim();

/** `UserPreferencesIndexViewHelper::dtoNumberFormat`. */
export const numberFormat = (prefs) => ({
  numbers: [
    { id: 0, format: trans('Locale default: :value', { value: localeDefaultNumber() }), value: 'locale' },
    { id: 1, format: '1,234.56', value: '1,234.56' },
    { id: 2, format: '1 234,56', value: '1 234,56' },
    { id: 3, format: '1.234,56', value: '1.234,56' },
    { id: 4, format: '1234.56', value: '1234.56' },
  ],
  number_format: prefs.number_format,
});

/** `UserPreferencesIndexViewHelper::dtoDistanceFormat`. */
export const distanceFormat = (prefs) => ({ number_format: prefs.distance_format });

/** `UserPreferencesIndexViewHelper::dtoMapsPreferences`. */
export const maps = (prefs) => ({
  types: [
    {
      id: 1,
      type: trans('Google Maps'),
      description: trans(
        'Google Maps offers the best accuracy and details, but it is not ideal from a privacy standpoint.',
      ),
      value: 'google_maps',
    },
    {
      id: 2,
      type: trans('Open Street Maps'),
      description: trans('Open Street Maps is a great privacy alternative, but offers less details.'),
      value: 'open_street_maps',
    },
  ],
  default_map_site: prefs.default_map_site,
  default_map_site_i18n:
    prefs.default_map_site === 'open_street_maps' ? trans('Open Street Maps') : trans('Google Maps'),
});

/** `UserPreferencesIndexViewHelper::language`: the language's own name. */
export const language = (code) => (code ? (current.instance.locales.find((l) => l.id === code)?.name ?? code) : '');

/** `UserPreferencesIndexViewHelper::dtoLocale`. */
export const locale = (prefs) => ({
  id: prefs.locale,
  name: language(prefs.locale),
  dir: current.instance.locales.find((l) => l.id === prefs.locale)?.dir ?? 'ltr',
  locales: locales(),
});

// --- NotificationsIndexViewHelper --------------------------------------

/** `NotificationsIndexViewHelper::data`: the email channels. */
export const notifications = (channels) => ({
  emails: channels.filter((c) => c.type === 'email').map(email),
  url: {
    settings: route('settings.index'),
    back: route('settings.index'),
  },
});

/** `NotificationsIndexViewHelper::dtoEmail`. */
export const email = (c) => ({
  id: c.id,
  type: c.type,
  label: c.label,
  content: c.content,
  active: c.active,
  verified_at: c.verified_at ? isoFormat(c.verified_at, 'YYYY-MM-DD HH:mm:ss', null) : null,
  preferred_time: c.preferred_time,
  url: {
    logs: route('settings.notifications.log.index', { notification: c.id }),
  },
});

/** `NotificationsLogIndexViewHelper::data`. */
export const notificationLogs = (channel, logs) => ({
  channel: {
    id: channel.id,
    type: channel.type === 'email' ? trans('Email address') : trans('Email'),
    label: channel.label,
  },
  notifications: logs.map((n) => ({
    id: n.id,
    sent_at: formatUserDate(n.sent_at),
    subject_line: n.subject_line,
    error: n.error,
  })),
  url: {
    settings: route('settings.index'),
    channels: route('settings.notifications.index'),
    back: route('settings.index'),
  },
});

// --- UserIndexViewHelper, UserCreateViewHelper -------------------------

/** `UserIndexViewHelper::data`. */
export const users = (list, loggedUser) => ({
  users: list.map((u) => user(u, loggedUser)),
  url: {
    settings: { index: route('settings.index') },
    users: { create: route('settings.user.create') },
  },
});

/** `UserIndexViewHelper::dtoUser`. */
export const user = (u, loggedUser) => ({
  id: u.id,
  email: u.email,
  name: userName(u),
  is_account_administrator: u.is_account_administrator,
  invitation_accepted_at: u.invitation_accepted_at ? formatDate(u.invitation_accepted_at) : null,
  is_logged_user: u.id === loggedUser.id,
});

/** `UserCreateViewHelper::data`. */
export const userCreate = () => ({
  url: {
    settings: route('settings.index'),
    back: route('settings.user.index'),
  },
});

// --- StorageIndexViewHelper, CancelAccountViewHelper -------------------

const percent = (part, total) => (total > 0 ? Math.round((part * 100) / total) : 0);

/** `StorageIndexViewHelper::data`. */
export function storage(usage) {
  const total = usage.photo.size + usage.document.size + usage.avatar.size;
  const limit = usage.storage_limit_in_mb * 1024 * 1024;
  const type = (t) => ({ total: t.count, total_percent: percent(t.size, total), size: formatFileSize(t.size) });
  return {
    statistics: {
      total: formatFileSize(total),
      total_percent: percent(total, limit),
      photo: type(usage.photo),
      document: type(usage.document),
      avatar: type(usage.avatar),
    },
    account_limit: formatFileSize(limit),
    url: {
      settings: { index: route('settings.index') },
    },
  };
}

/** `CancelAccountViewHelper::data`. */
export const cancelAccount = () => ({
  url: {
    settings: route('settings.index'),
    back: route('settings.index'),
  },
});
