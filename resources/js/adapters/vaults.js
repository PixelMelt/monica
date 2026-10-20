// The props of the vaults pages, built from API objects as their ViewHelpers
// built them from models (`app/Domains/Vault/*/Web/ViewHelpers`).

import dayjs from 'dayjs';
import { trans } from 'laravel-vue-i18n';
import {
  contactName,
  formatDate,
  formatFullDate,
  formatLongMonthAndYear,
  formatMonthAndYear,
  formatMonthNumber,
  formatUserDate,
  label as translated,
} from '@/lib/format';
import { addressAsString, avatar, contactCard, formatFileSize, formatImportantDate } from '@/lib/helpers';
import { userName } from '@/lib/session';
import { urls } from '@/api/client';
import { route } from '@/router';

// --- helpers (PHP functions that have no port in lib/) ------------------

/** A seeded row's `label` accessor: `label`, else the translated `label_translation_key`. */
const seededLabel = (row) => row.label || (row.label_translation_key ? trans(row.label_translation_key) : null);

/** `Str::ucfirst`. */
export function ucfirst(text) {
  const [first = '', ...rest] = Array.from(text ?? '');
  return first.toUpperCase() + rest.join('');
}

/** `Str::limit($text, $limit)`: cut to `limit` characters, with `...` appended when cut. */
export function strLimit(text, limit = 100) {
  const chars = Array.from(text ?? '');
  if (chars.length <= limit) return text;
  return `${chars.slice(0, limit).join('').trimEnd()}...`;
}

const pad = (n) => String(n).padStart(2, '0');
const firstOfMonth = (year, month) => `${year}-${pad(month)}-01`;

// --- VaultIndexViewHelper, VaultEditViewHelper ---------------------------

/** `VaultIndexViewHelper::getContacts`: one of the random contacts of a vault card. */
export const vaultContact = (contact) => ({
  id: contact.id,
  name: contactName(contact),
  avatar: avatar(contact),
});

/** `VaultIndexViewHelper::data`: one vault of the list (a `VaultListItem`). */
export function vault(v) {
  const contacts = v.contacts_sample.map(vaultContact);
  return {
    id: v.id,
    name: v.name,
    description: v.description,
    contacts,
    remaining_contacts: v.contacts_count - contacts.length,
    url: {
      show: route('vault.show', { vault: v.id }),
      edit: route('vault.edit', { vault: v.id }),
      settings: route('vault.settings.index', { vault: v.id }),
    },
  };
}

/** `VaultEditViewHelper::data`. */
export const vaultEdit = (v) => ({
  id: v.id,
  name: v.name,
  description: v.description,
  url: {
    back: route('vault.index'),
  },
});

// --- VaultSettingsIndexViewHelper, VaultImportantDateTypesViewHelper -----

/** `VaultSettingsIndexViewHelper::data`: one of the account's templates. */
export const template = (t, v) => ({
  id: t.id,
  name: translated(t),
  is_default: v.default_template_id === t.id,
});

/**
 * `VaultSettingsIndexViewHelper::dtoUser` from a `UserSummary` and the
 * user's permission in the vault (null when not a member).
 */
export const user = (u, permission = null) => ({
  id: u.id,
  name: userName(u),
  permission,
});

/** `VaultSettingsIndexViewHelper::dtoUser` from a `VaultMember`. */
export const member = (m) => user(m.user, m.permission);

/** `VaultSettingsIndexViewHelper::dtoLabel`. */
export const label = (l) => ({
  id: l.id,
  name: l.name,
  count: l.contacts_count ?? null,
  bg_color: l.bg_color,
  text_color: l.text_color,
});

/** `VaultSettingsIndexViewHelper::data`: the colors a label can take. */
export const labelColors = () => [
  { bg_color: 'bg-neutral-200', text_color: 'text-neutral-800' },
  { bg_color: 'bg-red-200', text_color: 'text-red-600' },
  { bg_color: 'bg-amber-200', text_color: 'text-amber-600' },
  { bg_color: 'bg-emerald-200', text_color: 'text-emerald-600' },
  { bg_color: 'bg-slate-200', text_color: 'text-slate-600' },
  { bg_color: 'bg-sky-200', text_color: 'text-sky-600' },
];

/** `VaultSettingsIndexViewHelper::dtoTag`. */
export const tag = (t) => ({
  id: t.id,
  name: t.name,
  count: t.posts_count ?? null,
});

/** `VaultImportantDateTypesViewHelper::dto`. */
export const importantDateType = (t) => ({
  id: t.id,
  label: t.label,
  internal_type: t.internal_type,
  can_be_deleted: t.can_be_deleted,
});

/** `VaultSettingsIndexViewHelper::dtoMoodTrackingParameter`. */
export const moodTrackingParameter = (p) => ({
  id: p.id,
  label: seededLabel(p),
  hex_color: p.hex_color,
  position: p.position,
});

/** `VaultSettingsIndexViewHelper::data`: the colors a mood tracking parameter can take. */
export const moodTrackingParameterColors = () =>
  [
    'bg-lime-500',
    'bg-lime-300',
    'bg-cyan-600',
    'bg-cyan-300',
    'bg-orange-600',
    'bg-orange-300',
    'bg-red-400',
    'bg-red-700',
    'bg-stone-400',
    'bg-stone-700',
  ].map((hex_color) => ({ hex_color }));

/** `VaultSettingsIndexViewHelper::dtoType`. */
export const lifeEventType = (categoryId, t) => ({
  id: t.id,
  label: seededLabel(t),
  can_be_deleted: t.can_be_deleted,
  life_event_category_id: categoryId,
  position: t.position,
});

/** `VaultSettingsIndexViewHelper::dtoLifeEventCategory`. */
export const lifeEventCategory = (c) => ({
  id: c.id,
  label: seededLabel(c),
  position: c.position,
  can_be_deleted: c.can_be_deleted,
  life_event_types: (c.life_event_types ?? []).map((t) => lifeEventType(c.id, t)),
});

/** `VaultSettingsIndexViewHelper::dtoQuickFactTemplateEntry`. */
export const quickFactTemplateEntry = (t) => ({
  id: t.id,
  label: seededLabel(t),
  position: t.position,
});

/** `VaultSettingsIndexViewHelper::data`: the tabs shown in the vault. */
export const visibility = (v) => ({
  show_group_tab: v.show_group_tab,
  show_tasks_tab: v.show_tasks_tab,
  show_files_tab: v.show_files_tab,
  show_journal_tab: v.show_journal_tab,
  show_companies_tab: v.show_companies_tab,
  show_reports_tab: v.show_reports_tab,
  show_calendar_tab: v.show_calendar_tab,
});

// --- VaultCalendarIndexViewHelper ----------------------------------------

/** `VaultCalendarIndexViewHelper::getImportantDates`: one date of a day. */
export const calendarImportantDate = (d) => ({
  id: d.id,
  label: d.label,
  type: {
    id: d.contact_important_date_type?.id,
    label: d.contact_important_date_type?.label,
  },
  contact: contactCard(d.contact),
});

/** `VaultCalendarIndexViewHelper::getMood`: one mood tracking event of a day. */
export const calendarMood = (e) => ({
  id: e.id,
  note: e.note,
  number_of_hours_slept: e.number_of_hours_slept,
  mood_tracking_parameter: {
    id: e.mood_tracking_parameter.id,
    label: seededLabel(e.mood_tracking_parameter),
    hex_color: e.mood_tracking_parameter.hex_color,
  },
});

/** `VaultCalendarIndexViewHelper::getJournalEntries`: one post of a day. */
export const calendarPost = (p, vaultId) => ({
  id: p.id,
  title: p.title || trans('Undefined'),
  url: {
    show: route('post.show', { vault: vaultId, journal: p.journal.id, post: p.id }),
  },
});

/**
 * `VaultCalendarIndexViewHelper::buildMonth`: one day of the grid.
 * `year`, `month` and `day` replace `url.show` (getVaultCalendarDay).
 */
export function calendarDay(d, vaultId) {
  const [year, month, day] = d.date.split('-').map(Number);
  return {
    id: day,
    date: d.date.slice(8, 10),
    is_today: d.is_today,
    is_in_month: d.is_in_month,
    important_dates: d.important_dates.map(calendarImportantDate),
    mood_events: d.mood_tracking_events.map(calendarMood),
    posts: d.posts.map((p) => calendarPost(p, vaultId)),
    year,
    month,
    day,
  };
}

/** `VaultCalendarIndexViewHelper::data`, from a `CalendarMonth`. */
export function calendarMonth(m, vaultId) {
  const date = dayjs(firstOfMonth(m.year, m.month));
  const previous = date.subtract(1, 'month');
  const next = date.add(1, 'month');
  return {
    weeks: m.weeks.map((week) => week.map((d) => calendarDay(d, vaultId))),
    current_month: formatLongMonthAndYear(date.format('YYYY-MM-DD')),
    previous_month: formatLongMonthAndYear(previous.format('YYYY-MM-DD')),
    next_month: formatLongMonthAndYear(next.format('YYYY-MM-DD')),
    url: {
      previous: route('vault.calendar.month', { vault: vaultId, year: previous.year(), month: previous.month() + 1 }),
      next: route('vault.calendar.month', { vault: vaultId, year: next.year(), month: next.month() + 1 }),
    },
  };
}

/** `VaultCalendarIndexViewHelper::getDayInformation`, from a `CalendarDay`. */
export const dayInformation = (d, vaultId) => ({
  day: formatFullDate(d.date),
  important_dates: d.important_dates.map(calendarImportantDate),
  mood_events: d.mood_tracking_events.map(calendarMood),
  posts: d.posts.map((p) => calendarPost(p, vaultId)),
});

// --- Reports ---------------------------------------------------------------

/** `ReportIndexViewHelper::data`. */
export const reportIndex = (vaultId) => ({
  url: {
    addresses: route('vault.reports.addresses.index', { vault: vaultId }),
    mood_tracking_events: route('vault.reports.mood_tracking_events.index', { vault: vaultId }),
    important_date_summary: route('vault.reports.important_dates.index', { vault: vaultId }),
  },
});

/** `ReportAddressIndexViewHelper::data`, from an `AddressReport`. */
export const addressReport = (r, vaultId) => ({
  cities: r.cities.map((c, i) => ({
    id: i,
    name: ucfirst(c.city),
    contacts: c.contacts_count,
    url: {
      index: route('vault.reports.addresses.cities.show', { vault: vaultId, city: c.city }),
    },
  })),
  countries: r.countries.map((c, i) => ({
    id: i,
    name: ucfirst(c.country),
    contacts: c.contacts_count,
    url: {
      index: route('vault.reports.addresses.countries.show', { vault: vaultId, country: c.country }),
    },
  })),
});

/** `ReportCitiesShowViewHelper::data` / `ReportCountriesShowViewHelper::data`: one address. */
export const reportAddress = (a, field) => ({
  id: a.id,
  name: ucfirst(a[field]),
  address: addressAsString(a),
  contacts: a.contacts.map(contactCard),
});

/** `ReportCitiesShowViewHelper::data`, from an `AddressPlaceReport`. */
export const cityReport = (r, city, info, vaultId) => ({
  city: ucfirst(city),
  addresses: r.addresses.map((a) => reportAddress(a, 'city')),
  wikipedia: info,
  url: {
    addresses: route('vault.reports.addresses.index', { vault: vaultId }),
  },
});

/** `ReportCountriesShowViewHelper::data`, from an `AddressPlaceReport`. */
export const countryReport = (r, country, info, vaultId) => ({
  country: ucfirst(country),
  addresses: r.addresses.map((a) => reportAddress(a, 'country')),
  wikipedia: info,
  url: {
    addresses: route('vault.reports.addresses.index', { vault: vaultId }),
  },
});

/**
 * `ReportMoodTrackingEventIndexViewHelper::data`, from a
 * `MoodTrackingReport`. The PHP loop stops before the last day of each
 * month (`$day < daysInMonth`); kept as is.
 */
export const moodTrackingReport = (r, vaultId) => ({
  months: r.months.map((m) => ({
    id: m.month,
    month_word: formatMonthNumber(firstOfMonth(r.year, m.month)),
    days: m.days
      .filter((d) => d.day < m.days.length)
      .map((d) => ({
        id: d.day,
        event: d.mood_tracking_event
          ? { id: d.mood_tracking_event.id, hex_color: d.mood_tracking_event.hex_color }
          : null,
      })),
  })),
  url: {
    reports: route('vault.reports.index', { vault: vaultId }),
  },
});

/** `ReportImportantDateSummaryIndexViewHelper::data`, from an `ImportantDateReport`. */
export const importantDateReport = (r, vaultId) => ({
  months: r.months.map((m, i) => ({
    id: i,
    month: formatMonthAndYear(firstOfMonth(m.year, m.month)),
    important_dates: m.important_dates.map((d) => ({
      id: d.id,
      label: d.label,
      happened_at: formatImportantDate(d),
      happened_at_age: d.age,
      contact: contactCard(d.contact),
    })),
  })),
  url: {
    reports: route('vault.reports.index', { vault: vaultId }),
  },
});

// --- VaultFileIndexViewHelper ----------------------------------------------

/** `VaultFileIndexViewHelper::getObjectDetails`: the contact the file belongs to, else `[]`. */
export const fileObject = (contact) =>
  contact
    ? {
        type: 'contact',
        id: contact.id,
        name: contactName(contact),
        avatar: avatar(contact),
        url: {
          show: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
        },
      }
    : [];

/** `VaultFileIndexViewHelper::data`: one file of the list (a `VaultFile`). */
export const file = (f, vaultId) => ({
  id: f.id,
  name: f.name,
  mime_type: f.mime_type,
  size: formatFileSize(f.size),
  created_at: formatUserDate(f.created_at),
  object: fileObject(f.contact),
  url: {
    download: urls.getVaultFileContent({ vault_id: vaultId, file_id: f.id, variant: 'original' }),
  },
});

/** `VaultFileIndexViewHelper::statistics`, from the list's `counts`. */
export const fileStatistics = (counts, vaultId) => ({
  statistics: {
    all: counts.photo + counts.document + counts.avatar,
    photos: counts.photo,
    documents: counts.document,
    avatars: counts.avatar,
  },
  url: {
    index: route('vault.files.index', { vault: vaultId }),
    photos: route('vault.files.photos', { vault: vaultId }),
    documents: route('vault.files.documents', { vault: vaultId }),
    avatars: route('vault.files.avatars', { vault: vaultId }),
  },
});

// --- CompanyIndexViewHelper ------------------------------------------------

/** `CompanyIndexViewHelper::dto`. */
export const company = (c) => ({
  id: c.id,
  name: c.name,
  contacts: c.contacts.map((contact) => ({
    id: contact.id,
    avatar: avatar(contact),
    url: {
      show: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
    },
  })),
});

// --- VaultSearchIndexViewHelper --------------------------------------------

/** The search results' contact name: first, last, nickname, maiden and middle names. */
const searchName = (c) =>
  `${c.first_name ?? ''} ${c.last_name ?? ''} ${c.nickname ?? ''} ${c.maiden_name ?? ''} ${c.middle_name ?? ''}`;

/** `VaultSearchIndexViewHelper::contacts`: one contact. */
export const searchContact = (c) => ({
  id: c.id,
  name: searchName(c),
  url: route('contact.show', { vault: c.vault_id, contact: c.id }),
});

/** `VaultSearchIndexViewHelper::notes`: one note. */
export const searchNote = (n, vaultId) => ({
  id: n.id,
  title: n.title,
  body: n.body,
  body_excerpt: strLimit(n.body, 100),
  show_full_content: false,
  written_at: formatDate(n.created_at),
  contact: {
    id: n.contact_id,
    name: searchName(n.contact),
    url: route('contact.show', { vault: vaultId, contact: n.contact_id }),
  },
});

/** `VaultSearchIndexViewHelper::groups`: one group. */
export const searchGroup = (g, vaultId) => ({
  id: g.id,
  name: g.name,
  url: route('group.show', { vault: vaultId, group: g.id }),
});

/** `VaultSearchIndexViewHelper::data`, from `SearchResults`. */
export const searchResults = (r, query, vaultId) => ({
  query,
  contacts: r.contacts.map(searchContact),
  notes: r.notes.map((n) => searchNote(n, vaultId)),
  groups: r.groups.map((g) => searchGroup(g, vaultId)),
});
