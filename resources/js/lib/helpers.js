// Client-side ports of the formatting parts of `app/Helpers/*` (X015).
// Each function names the PHP method it ports.

import { trans } from 'laravel-vue-i18n';
import { session } from '@/lib/session';
import { contactName, formatDate, isoFormat } from '@/lib/format';
import { urls } from '@/api/client';
import { route } from '@/router';

const prefs = () => session.user?.preferences ?? {};

// --- MonetaryNumberHelper ----------------------------------------------

/** Digits after the decimal point of an ISO 4217 currency. */
export function currencyDigits(currency) {
  try {
    return new Intl.NumberFormat('en', { style: 'currency', currency }).resolvedOptions().maximumFractionDigits;
  } catch {
    return 2;
  }
}

/** The user's separators (`User::NUMBER_FORMAT_TYPE_*`), or null for the locale's. */
function separators() {
  switch (prefs().number_format) {
    case '1,234.56':
      return { decimal: '.', thousands: ',' };
    case '1 234,56':
      return { decimal: ',', thousands: ' ' };
    case '1.234,56':
      return { decimal: ',', thousands: '.' };
    case '1234.56':
      return { decimal: '.', thousands: '' };
    default:
      return null;
  }
}

const intlLocale = () => (session.user?.preferences.locale ?? 'en').replace('_', '-');

/**
 * `MonetaryNumberHelper::formatValue`: the amount without currency, as
 * ICU's decimal style prints it ("1,234.5").
 */
export function formatMoneyValue(amount, currency = null) {
  const digits = currency ? currencyDigits(currency) : 2;
  const value = amount / 10 ** digits;
  const parts = new Intl.NumberFormat(intlLocale(), { maximumFractionDigits: 3 }).formatToParts(value);
  const sep = separators();
  return parts
    .map((p) => {
      if (sep && p.type === 'decimal') return sep.decimal;
      if (sep && p.type === 'group') return sep.thousands;
      return p.value;
    })
    .join('')
    .trim();
}

/** `MonetaryNumberHelper::format`: the amount with the currency ("$1,234.50"). */
export function formatMoney(amount, currency = null) {
  const value = formatMoneyValue(amount, currency);
  if (currency === null) return value;
  const digits = currencyDigits(currency);
  const formatter = new Intl.NumberFormat(intlLocale(), { style: 'currency', currency });
  const parts = formatter.formatToParts(amount / 10 ** digits);
  let replaced = false;
  return parts
    .map((p) => {
      if (['integer', 'group', 'decimal', 'fraction'].includes(p.type)) {
        if (replaced) return '';
        replaced = true;
        return value;
      }
      return p.value;
    })
    .join('');
}

/** `MonetaryNumberHelper::inputValue`: the amount as typed in a form ("1234.50"). */
export function moneyInputValue(amount, currency = null) {
  const digits = currency ? currencyDigits(currency) : 2;
  const negative = amount < 0;
  const abs = String(Math.abs(amount)).padStart(digits + 1, '0');
  const value = digits ? `${abs.slice(0, -digits)}.${abs.slice(-digits)}` : abs;
  return negative ? `-${value}` : value;
}

/** `MonetaryNumberHelper::parseInput`: "145.00" → 14500. */
export function parseMoneyInput(value, currency = null) {
  const digits = currency ? currencyDigits(currency) : 2;
  const [int, frac = ''] = String(value).trim().split('.');
  return parseInt(`${int}${frac.padEnd(digits, '0').slice(0, digits)}`, 10);
}

// --- DistanceHelper, FileHelper, MapHelper -----------------------------

/** `DistanceHelper::format`: a distance in the user's unit. */
export function formatDistance(distance, unit) {
  const format = prefs().distance_format;
  if (format === 'km' && unit === 'mi') distance = Math.round(distance * 1.609344 * 100) / 100;
  else if (format === 'mi' && unit === 'km') distance = Math.round((distance / 1.609344) * 100) / 100;
  return format === 'km' ? trans(':distance km', { distance }) : trans(':distance miles', { distance });
}

/** `FileHelper::formatFileSize`: "1.5MB". */
export function formatFileSize(bytes) {
  const units = ['B', 'kB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'];
  let i = 0;
  while (bytes / 1024 > 0.9) {
    bytes /= 1024;
    i++;
  }
  return `${Math.round(bytes * 100) / 100}${units[i]}`;
}

/** PHP's `urlencode`. */
export const urlencode = (text) =>
  encodeURIComponent(text)
    .replace(/[!'()*~]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`)
    .replace(/%20/g, '+');

/** `MapHelper::getAddressAsString`. */
export function addressAsString(address) {
  const parts = [address.line_1, address.line_2, address.city, address.province, address.postal_code, address.country];
  let sentence = address.line_1 ?? '';
  for (const part of parts.slice(1)) if (part !== null && part !== undefined) sentence += ` ${part}`;
  return sentence.replace(/\s+/g, ' ');
}

/** `MapHelper::getMapLink`: the address on the user's map site. */
export function mapLink(address) {
  const text = addressAsString(address);
  if (!text) return '';
  const encoded = urlencode(text);
  return prefs().default_map_site === 'open_street_maps'
    ? `https://www.openstreetmap.org/search?query=${encoded}`
    : `https://www.google.com/maps/place/${encoded}`;
}

// --- ImportantDateHelper, ContactReminderHelper, SliceOfLifeHelper -----

/** `ImportantDateHelper::determineType`: `full_date`, `month_day`, `year`, '' or null. */
export function importantDateType(date) {
  if (!date.day && !date.month && !date.year) return null;
  if (date.day && date.month && date.year) return 'full_date';
  if (!date.day && !date.month && date.year) return 'year';
  if (date.day && date.month && !date.year) return 'month_day';
  return '';
}

const pad = (n) => String(n).padStart(2, '0');

function monthDay(month, day) {
  const d = `1900-${pad(month)}-${pad(day)}`;
  switch (prefs().date_format) {
    case 'MMM DD, YYYY':
      return isoFormat(d, 'MMM DD');
    case 'DD MMM YYYY':
      return isoFormat(d, 'DD MMM');
    case 'YYYY/MM/DD':
      return isoFormat(d, 'MM/DD');
    default:
      return isoFormat(d, 'DD/MM');
  }
}

/** `ImportantDateHelper::formatDate`: a partial date in the user's format. */
export function formatImportantDate(date) {
  if (!date.day && !date.month && !date.year) return null;
  switch (importantDateType(date)) {
    case 'full_date':
      return isoFormat(`${date.year}-${pad(date.month)}-${pad(date.day)}`, prefs().date_format);
    case 'year':
      return String(date.year);
    case 'month_day':
      return monthDay(date.month, date.day);
    default:
      return '';
  }
}

/** `ContactReminderHelper::formatDate`. */
export function formatReminderDate(reminder) {
  if (!reminder.day && !reminder.month) return null;
  if (reminder.day && reminder.month && !reminder.year) return monthDay(reminder.month, reminder.day);
  if (reminder.day && reminder.month && reminder.year) {
    return isoFormat(`${reminder.year}-${pad(reminder.month)}-${pad(reminder.day)}`, prefs().date_format);
  }
  return '';
}

/**
 * `SliceOfLifeHelper::getDateRange`, from the oldest and newest
 * `written_at` of the slice's posts.
 */
export function sliceDateRange(oldest, newest) {
  const range = `${oldest ? formatDate(oldest) : ''} - ${newest ? formatDate(newest) : ''}`;
  return range.trim() === '-' ? null : range;
}

// --- Contacts ----------------------------------------------------------

/** `$contact->avatar`: `{type: 'url', content}` from the avatar endpoint. */
export const avatar = (contact) => ({
  type: 'url',
  content: urls.getContactAvatar({ vault_id: contact.vault_id, contact_id: contact.id }),
});

/** `UserHelper::getInformationAboutContact` from a `ContactSummary`. */
export const contactInformation = (contact) =>
  contact && {
    id: contact.id,
    name: contactName(contact),
    avatar: avatar(contact),
    url: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
  };

/** `GroupsViewHelper::summary`: a contact's groups in its vault. */
export const groupsSummary = (groups, vaultId) =>
  (groups ?? []).map((g) => ({
    id: g.id,
    name: g.name,
    url: { show: route('group.show', { vault: vaultId, group: g.id }) },
  }));

/** `ContactCardHelper::data` from a `ContactCard` (summary + age + groups). */
export const contactCard = (card) => ({
  id: card.id,
  name: contactName(card),
  age: card.age ?? null,
  avatar: avatar(card),
  groups: groupsSummary(card.groups, card.vault_id),
  url: route('contact.show', { vault: card.vault_id, contact: card.id }),
});
