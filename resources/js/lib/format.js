// Client-side formatting of the raw values the API returns (X015), as the
// PHP helpers did it: dates with Carbon's `isoFormat` tokens and the
// translated `format.*` patterns (DateHelper), contact names with the
// user's name order (NameHelper).

import dayjs from 'dayjs';
import utc from 'dayjs/plugin/utc';
import timezone from 'dayjs/plugin/timezone';
import advancedFormat from 'dayjs/plugin/advancedFormat';
import { trans } from 'laravel-vue-i18n';
import { session } from '@/lib/session';

dayjs.extend(utc);
dayjs.extend(timezone);
dayjs.extend(advancedFormat);

const LOCALES = {
  ar: () => import('dayjs/locale/ar'),
  bn: () => import('dayjs/locale/bn'),
  ca: () => import('dayjs/locale/ca'),
  da: () => import('dayjs/locale/da'),
  de: () => import('dayjs/locale/de'),
  el: () => import('dayjs/locale/el'),
  es: () => import('dayjs/locale/es'),
  fr: () => import('dayjs/locale/fr'),
  he: () => import('dayjs/locale/he'),
  hi: () => import('dayjs/locale/hi'),
  it: () => import('dayjs/locale/it'),
  ja: () => import('dayjs/locale/ja'),
  ml: () => import('dayjs/locale/ml'),
  nl: () => import('dayjs/locale/nl'),
  nn: () => import('dayjs/locale/nn'),
  pa: () => import('dayjs/locale/pa-in'),
  pl: () => import('dayjs/locale/pl'),
  pt: () => import('dayjs/locale/pt'),
  pt_BR: () => import('dayjs/locale/pt-br'),
  ro: () => import('dayjs/locale/ro'),
  ru: () => import('dayjs/locale/ru'),
  sv: () => import('dayjs/locale/sv'),
  te: () => import('dayjs/locale/te'),
  tr: () => import('dayjs/locale/tr'),
  ur: () => import('dayjs/locale/ur'),
  vi: () => import('dayjs/locale/vi'),
  zh_CN: () => import('dayjs/locale/zh-cn'),
  zh_TW: () => import('dayjs/locale/zh-tw'),
};

let dayjsLocale = 'en';

/** Switches date formatting to `locale` (an app locale code like `pt_BR`). */
export async function setLocale(locale) {
  const load = LOCALES[locale] ?? LOCALES[locale.split('_')[0]];
  if (!load) {
    dayjsLocale = 'en';
    return;
  }
  const mod = await load();
  dayjsLocale = mod.default.name;
}

/** Carbon's `Y` (the year, unpadded) is not a dayjs token. */
const pattern = (format) => format.replace(/(^|[^Y])Y(?!Y)/g, '$1YYYY');

/**
 * A date as dayjs, in the user's timezone for timestamps
 * (`2025-08-09T22:32:20Z`) and as-is for dates (`2025-08-09`).
 */
export function toDate(value, tz = session.user?.preferences.timezone) {
  if (value === null || value === undefined) return null;
  if (typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value)) return dayjs(value).locale(dayjsLocale);
  const d = dayjs.utc(value);
  return (tz ? d.tz(tz) : d).locale(dayjsLocale);
}

/** `$date->isoFormat($format)`. */
export function isoFormat(value, format, tz) {
  const d = toDate(value, tz);
  return d ? d.format(pattern(format)) : null;
}

/** `DateHelper::format`: the user's chosen date format. */
export const formatUserDate = (value) => isoFormat(value, session.user?.preferences.date_format ?? 'MMM DD, YYYY');

/** `DateHelper::formatDate`: `format.date` ("Oct 01, 1981"). */
export const formatDate = (value, tz) => isoFormat(value, trans('format.date'), tz);

/** `DateHelper::formatShortDateWithTime`: "Oct 29, 1981 07:32 PM". */
export const formatShortDateWithTime = (value, tz) => isoFormat(value, trans('format.short_date_year_time'), tz);

/** `DateHelper::formatMonthAndDay`: "July 29th". */
export const formatMonthAndDay = (value) => isoFormat(value, trans('format.long_month_day'));

/** `DateHelper::formatMonthAndYear`: "Jul 2020". */
export const formatMonthAndYear = (value) => isoFormat(value, trans('format.short_month_year'));

/** `DateHelper::formatLongMonthAndYear`: "July 2020". */
export const formatLongMonthAndYear = (value) => isoFormat(value, trans('format.long_month_year'));

/** `DateHelper::formatShortMonthAndDay`: "Jul 29". */
export const formatShortMonthAndDay = (value) => isoFormat(value, trans('format.short_date'));

/** `DateHelper::formatShortDay`: "Wed". */
export const formatShortDay = (value) => isoFormat(value, trans('format.short_day'));

/** `DateHelper::formatDayAndMonthInParenthesis`: "Monday (Jul 29th)". */
export const formatDayAndMonthInParenthesis = (value, tz) =>
  isoFormat(value, trans('format.day_month_parenthesis'), tz);

/** `DateHelper::formatFullDate`: "Monday, Jul 29th 2020". */
export const formatFullDate = (value) => isoFormat(value, trans('format.full_date'));

/** `DateHelper::formatDayNumber`: "03". */
export const formatDayNumber = (value) => isoFormat(value, trans('format.day_number'));

/** `DateHelper::formatMonthNumber`: "Jul". */
export const formatMonthNumber = (value) => isoFormat(value, trans('format.short_month'));

/** `DateHelper::getMonths`: `[{id: 1, name: 'January'}, …]`. */
export function getMonths() {
  return Array.from({ length: 12 }, (_, i) => ({
    id: i + 1,
    name: dayjs().locale(dayjsLocale).month(i).date(1).format('MMMM'),
  }));
}

/** `DateHelper::getDays`: `[{id: 1, name: 1}, …, {id: 31, name: 31}]`. */
export function getDays() {
  return Array.from({ length: 31 }, (_, i) => ({ id: i + 1, name: i + 1 }));
}

/** Today in the user's timezone, as dayjs. */
export const today = () =>
  (session.user?.preferences.timezone ? dayjs().tz(session.user.preferences.timezone) : dayjs()).locale(dayjsLocale);

/**
 * `NameHelper::formatContactName`: the contact's name built from the user's
 * `name_order` (`%first_name% %last_name%`, …), with the prefix and suffix.
 */
export function contactName(contact, nameOrder = session.user?.preferences.name_order ?? '%first_name% %last_name%') {
  let name = contact.prefix ? `${contact.prefix} ` : '';
  name += nameOrder.replace(/%([a-z_]+)%/g, (_, field) => contact[field] ?? '');
  name = name.replaceAll('()', '').trimEnd();
  if (contact.suffix) name = `${name} ${contact.suffix}`;
  return name.trim() === '' ? trans('Unknown name') : name;
}

/** A seeded label: the user's `name`, else the translated key. */
export const label = (item) => item?.name ?? (item?.name_translation_key ? trans(item.name_translation_key) : null);

/** `sortByCollator`: sorts by a string field with the locale's collation. */
export function sortByCollator(items, field = 'name') {
  const collator = new Intl.Collator(dayjsLocale);
  return [...items].sort((a, b) => collator.compare(a[field] ?? '', b[field] ?? ''));
}
