// Contact page modules, set A: see adapters/modules.js. The exports named
// after a module type (`notes`, `reminders`, `loans`, `tasks`, `calls`,
// `pets`, `goals`, `feed`, `life_events`) are the module adapters,
// `(data, ctx) => props.data`; the other exports port the ViewHelper `dto*`
// methods the components apply to the objects the API returns. Don't
// export a name that is another module's type.
//
// Every module's data carries `vault_id` and `contact_id`, the path of the
// calls its component makes.

import dayjs from 'dayjs';
import { trans } from 'laravel-vue-i18n';
import { route } from '@/router';
import { contactName, formatShortDay, formatUserDate, getDays, getMonths, isoFormat, label, today } from '@/lib/format';
import {
  avatar,
  contactCard,
  contactInformation,
  formatDistance,
  formatMoney,
  formatMoneyValue,
  formatReminderDate,
  mapLink,
  moneyInputValue,
} from '@/lib/helpers';

// --- helpers -------------------------------------------------------------

/** `$model->label` of the rows seeded with a `label_translation_key`. */
export const seededLabel = (item) =>
  item?.label || (item?.label_translation_key ? trans(item.label_translation_key) : null);

/** `Str::limit($text, $limit)`. */
export function strLimit(text, limit) {
  const chars = [...(text ?? '')];
  if (chars.length <= limit) return text;
  return `${chars.slice(0, limit).join('').trimEnd()}...`;
}

/** A form value as an integer (`''`/`null` → null), as the API takes ids and counts. */
export const toInt = (value) => (value === '' || value === null || value === undefined ? null : Number(value));

/** An optional text field: `''` → null (Laravel's `ConvertEmptyStringsToNull`). */
export const blank = (value) => (value === '' || value === undefined ? null : value);

/**
 * `Carbon::parse($value)->format('Y-m-d')` of a date picker value
 * (`YYYY-MM-DD`); an empty value is today, as Carbon parses it.
 */
export const pickedDate = (value) => (value ? String(value).slice(0, 10) : today().format('YYYY-MM-DD'));

const ids = (vaultId, contactId) => ({ vault_id: vaultId, contact_id: contactId });

// --- notes ---------------------------------------------------------------

/** The `{id, name, type}` emotions of the note and call forms (`listEmotions`). */
export const emotion = (e) => ({ id: e.id, name: label(e), type: e.type });

/** `ModuleNotesViewHelper::dto` (and `NotesIndexViewHelper::dto`, the same fields). */
export const note = (n) => ({
  id: n.id,
  body: n.body,
  body_excerpt: [...n.body].length >= 200 ? strLimit(n.body, 200) : null,
  show_full_content: false,
  title: n.title,
  emotion: n.emotion ? { id: n.emotion.id, name: label(n.emotion) } : null,
  author: n.author?.contact ? contactInformation(n.author.contact) : null,
  written_at: formatUserDate(n.created_at),
});

/**
 * `ModuleNotesViewHelper::data`: the 3 newest notes. The emotions of the
 * form are loaded by the component (`listEmotions`).
 */
export const notes = (data, { vaultId, contactId }) => ({
  notes: data.notes.map(note),
  ...ids(vaultId, contactId),
  url: {
    index: route('contact.note.index', { vault: vaultId, contact: contactId }),
  },
});

// --- reminders -----------------------------------------------------------

/**
 * `ModuleRemindersViewHelper::dtoReminder`, plus `year`: the edit form
 * rebuilds its date from day, month and year.
 */
export const reminder = (r) => ({
  id: r.id,
  label: r.label,
  date: formatReminderDate(r),
  type: r.type,
  frequency_number: r.frequency_number,
  day: r.day,
  month: r.month,
  year: r.year,
  choice: r.year ? 'full_date' : 'month_day',
  reminder_choice: r.type === 'one_time' ? 'one_time' : 'recurring',
});

/** `ModuleRemindersViewHelper::data`. */
export const reminders = (data, { vaultId, contactId }) => ({
  reminders: data.reminders.map(reminder),
  months: getMonths(),
  days: getDays(),
  ...ids(vaultId, contactId),
});

// --- loans ---------------------------------------------------------------

/** `CurrencyIndexViewHelper::data` (the loan form's currencies, `listAccountCurrencies?active=true`). */
export const currencyOption = (c) => ({ id: c.id, name: c.code, selected: null });

const uniqueById = (list) => list.filter((x, i) => list.findIndex((y) => y.id === x.id) === i);

/** `ModuleLoanViewHelper::dtoLoan`. */
export function loan(l) {
  const currency = l.currency?.code ?? null;

  return {
    id: l.id,
    type: l.type,
    name: l.name,
    description: l.description,
    amount_full: l.amount_lent ? formatMoney(l.amount_lent, currency) : null,
    amount_lent: l.amount_lent ? formatMoneyValue(l.amount_lent, currency) : null,
    amount_lent_input: l.amount_lent ? moneyInputValue(l.amount_lent, currency) : null,
    currency_id: l.currency?.id ?? null,
    currency_name: currency,
    loaned_at: l.loaned_at,
    loaned_at_human_format: l.loaned_at ? formatUserDate(l.loaned_at) : null,
    loaners: uniqueById(l.loaners).map(contactCard),
    loanees: uniqueById(l.loanees).map(contactCard),
    settled: l.settled,
    settled_at_human_format: l.settled_at ? formatUserDate(l.settled_at) : null,
  };
}

/** `ModuleLoanViewHelper::data`: the unsettled loans. */
export const loans = (data, { vaultId, contactId }) => ({
  loans: data.loans.map(loan),
  current_date: dayjs.utc().format('YYYY-MM-DD'),
  ...ids(vaultId, contactId),
});

// --- tasks ---------------------------------------------------------------

/** `ModuleContactTasksViewHelper::dtoTask`, plus `contact_id` (the path of its calls). */
export const task = (t) => ({
  id: t.id,
  contact_id: t.contact_id,
  label: t.label,
  description: t.description,
  completed: t.completed,
  completed_at: t.completed_at ? formatUserDate(t.completed_at) : null,
  due_at: t.due_at
    ? {
        formatted: formatUserDate(t.due_at),
        value: t.due_at,
        is_late: t.is_late,
      }
    : null,
});

/** `ModuleContactTasksViewHelper::data`: the open tasks; completed ones load on demand. */
export const tasks = (data, { vaultId, contactId }) => ({
  tasks: data.tasks.map(task),
  completed_tasks_count: data.completed_tasks_count,
  ...ids(vaultId, contactId),
});

// --- calls ---------------------------------------------------------------

/** The `call_reason_types` of `ModuleCallsViewHelper::data` (`listCallReasonTypes`). */
export const callReasonType = (t) => ({
  id: t.id,
  label: seededLabel(t),
  reasons: t.reasons.map((r) => ({ id: r.id, label: seededLabel(r) })),
});

/**
 * `ModuleCallsViewHelper::dto`, plus `called_at_value` (`YYYY-MM-DD`, the
 * edit form's date; `called_at` is formatted).
 */
export const call = (c) => ({
  id: c.id,
  called_at: formatUserDate(c.called_at),
  called_at_value: c.called_at,
  duration: c.duration,
  description: c.description,
  who_initiated: c.who_initiated,
  type: c.type,
  answered: c.answered,
  emotion: c.emotion ? emotion(c.emotion) : null,
  reason: c.call_reason ? { id: c.call_reason.id, label: seededLabel(c.call_reason) } : null,
});

/**
 * `ModuleCallsViewHelper::data`. The emotions and call reasons of the form
 * are loaded by the component.
 */
export const calls = (data, { page, vaultId, contactId }) => ({
  contact_name: contactName(page.contact),
  calls: data.calls.map(call),
  ...ids(vaultId, contactId),
});

// --- pets ----------------------------------------------------------------

/** The `pet_categories` of `ModulePetsViewHelper::data` (`listPetCategories`). */
export const petCategory = (c) => ({ id: c.id, name: label(c) });

/** `ModulePetsViewHelper::dto`. */
export const pet = (p) => ({
  id: p.id,
  name: p.name,
  pet_category: petCategory(p.pet_category),
});

/** `ModulePetsViewHelper::data`. The categories are loaded by the component. */
export const pets = (data, { vaultId, contactId }) => ({
  pets: data.pets.map(pet),
  ...ids(vaultId, contactId),
});

// --- goals ---------------------------------------------------------------

/** `ModuleGoalsViewHelper::dto` (`last_7_days`: `getLast7Days`, oldest first). */
export const goal = (g, vaultId) => ({
  id: g.id,
  name: g.name,
  active: g.active,
  streaks_statistics: { max_streak: g.max_streak, current_streak: g.current_streak },
  last_7_days: g.last_7_days.map((day, i) => ({
    id: g.last_7_days.length - 1 - i,
    day: formatShortDay(day.date),
    day_number: isoFormat(day.date, 'DD'),
    happened_at: day.date,
    active: day.has_streak,
  })),
  url: {
    show: route('contact.goal.show', { vault: vaultId, contact: g.contact_id, goal: g.id }),
  },
});

/** `ModuleGoalsViewHelper::data`: the active goals. */
export const goals = (data, { vaultId, contactId }) => ({
  active_goals: data.goals.map((g) => goal(g, vaultId)),
  inactive_goals_count: data.inactive_goals_count,
  ...ids(vaultId, contactId),
});

/**
 * `GoalShowViewHelper::data` from a `GoalDetail` and its contact: the
 * weeks of the streak calendar from `streaks_from` to `streaks_to`.
 */
export function goalDetail(g, contact) {
  const from = dayjs(g.streaks_from);
  const numberOfWeeks = Math.floor(dayjs(g.streaks_to).diff(from, 'day') / 7) + 1;
  const streakDays = new Set(g.streaks.map((s) => s.happened_at));

  const weeks = [];
  let current = from;
  for (let week = 1; week <= numberOfWeeks; week++) {
    const streaks = [];
    for (let day = 1; day <= 7; day++) {
      const date = current.format('YYYY-MM-DD');
      streaks.push({
        id: day,
        date: formatUserDate(date),
        streak: streakDays.has(date),
        not_yet_happened: date > g.streaks_to,
      });
      current = current.add(1, 'day');
    }
    weeks.push({ id: week, streaks });
  }

  return {
    id: g.id,
    vault_id: contact.vault_id,
    contact_id: contact.id,
    name: g.name,
    active: g.active,
    streaks_statistics: { max_streak: g.max_streak, current_streak: g.current_streak },
    weeks,
    count: g.streaks.length,
    contact: {
      name: contactName(contact),
    },
    url: {
      contact: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
    },
  };
}

// --- feed ----------------------------------------------------------------

const FEED_SENTENCES = {
  contact_created: 'created the contact',
  author_deleted: 'Deleted author',
  information_updated: 'updated the contact information',
  important_date_created: 'added an important date',
  important_date_updated: 'updated an important date',
  important_date_destroyed: 'deleted an important date',
  address_created: 'added an address',
  address_updated: 'updated an address',
  address_destroyed: 'deleted an address',
  pet_created: 'added a pet',
  pet_updated: 'updated a pet',
  pet_destroyed: 'deleted a pet',
  contact_information_created: 'added a contact information',
  contact_information_updated: 'updated a contact information',
  contact_information_destroyed: 'deleted a contact information',
  label_assigned: 'assigned a label',
  label_removed: 'removed a label',
  note_created: 'wrote a note',
  note_updated: 'edited a note',
  note_destroyed: 'deleted a note',
  job_information_updated: 'updated the job information',
  religion_updated: 'updated the religion',
  goal_created: 'created a goal',
  goal_updated: 'updated a goal',
  goal_destroyed: 'deleted a goal',
  added_to_group: 'added the contact to a group',
  removed_from_group: 'removed the contact from a group',
  added_to_post: 'added the contact to a post',
  removed_from_post: 'removed the contact from a post',
  archived: 'archived the contact',
  unarchived: 'unarchived the contact',
  favorited: 'added the contact to the favorites',
  unfavorited: 'removed the contact from the favorites',
  changed_avatar: 'updated the avatar of the contact',
  mood_tracking_event_added: 'logged the mood',
};

/** The avatar `ModuleFeedViewHelper::getAuthor` shows for a deleted author. */
const DELETED_AUTHOR_AVATAR = `<svg viewBox="0 0 390 353" fill="none" xmlns="http://www.w3.org/2000/svg">
                <path fill-rule="evenodd" clip-rule="evenodd" d="M198.147 353C289.425 353 390.705 294.334 377.899 181.5C365.093 68.6657 289.425 10 198.147 10C106.869 10 31.794 61.4285 12.2144 181.5C-7.36527 301.571 106.869 353 198.147 353Z" fill="#2C2B29"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M196.926 320C270.146 320 351.389 272.965 341.117 182.5C330.844 92.0352 270.146 45 196.926 45C123.705 45 63.4825 86.2328 47.7763 182.5C32.0701 278.767 123.705 320 196.926 320Z" fill="white"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M52.4154 132C62.3371 132.033 66.6473 96.5559 84.3659 80.4033C100.632 65.5752 138 60.4908 138 43.3473C138 7.52904 99.1419 0 64.8295 0C30.517 0 0 36.3305 0 72.1487C0 107.967 33.3855 131.937 52.4154 132Z" fill="#2C2B29"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M337.585 132C327.663 132.033 323.353 96.5559 305.634 80.4033C289.368 65.5752 252 60.4908 252 43.3473C252 7.52904 290.858 0 325.171 0C359.483 0 390 36.3305 390 72.1487C390 107.967 356.615 131.937 337.585 132Z" fill="#2C2B29"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M275.651 252.546C301.619 246.357 312.447 235.443 312.447 200.175C312.447 164.907 295.905 129.098 267.423 129.098C238.941 129.098 220.028 154.564 220.028 189.832C220.028 225.1 249.682 258.734 275.651 252.546Z" fill="#2B2A28"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M266.143 207.804C273.209 207.804 275.311 206.002 278.505 201.954C282.087 197.416 284.758 192.151 283.885 181.278C282.426 163.109 274.764 154.752 259.773 154.752C244.783 154.752 241.859 166.27 241.859 181.278C241.859 196.286 251.152 207.804 266.143 207.804Z" fill="white"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M115.349 252.546C89.3806 246.357 78.5526 235.443 78.5526 200.175C78.5526 164.907 95.0948 129.098 123.577 129.098C152.059 129.098 170.972 154.564 170.972 189.832C170.972 225.1 141.318 258.734 115.349 252.546Z" fill="#2B2A28"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M124.857 207.804C117.791 207.804 115.689 206.002 112.495 201.954C108.913 197.416 106.242 192.151 107.115 181.278C108.574 163.109 116.236 154.752 131.227 154.752C146.217 154.752 149.141 166.27 149.141 181.278C149.141 196.286 139.848 207.804 124.857 207.804Z" fill="white"/>
                <path fill-rule="evenodd" clip-rule="evenodd" d="M196.204 276C210.316 276 224 255.244 224 246.342C224 237.441 210.112 236 196 236C181.888 236 168 237.441 168 246.342C168 255.244 182.092 276 196.204 276Z" fill="#2C2B29"/>
            </svg>`;

/** `ModuleFeedViewHelper::getAuthor`. */
function feedAuthor(item) {
  if (!item.author) {
    return {
      name: trans('Deleted author'),
      avatar: { type: 'svg', content: DELETED_AUTHOR_AVATAR },
      url: null,
    };
  }

  return item.author.contact ? contactInformation(item.author.contact) : null;
}

/** The `contact` of the `ActionFeed*::data` methods (and `ActionFeedGenericContactInformation::data`). */
const feedContact = (c) => ({
  id: c.id,
  name: contactName(c),
  age: c.age,
  avatar: avatar(c),
  url: route('contact.show', { vault: c.vault_id, contact: c.id }),
});

/** `ModuleFeedViewHelper::getData`: the `ActionFeed*::data` of the item's action. */
function feedData(item) {
  const contact = feedContact(item.contact);
  const description = item.description;

  switch (item.action) {
    case 'label_assigned':
    case 'label_removed': {
      const l = item.label;
      return {
        label: {
          object: l
            ? {
                id: l.id,
                name: l.name,
                bg_color: l.bg_color,
                text_color: l.text_color,
                url: route('contact.label.index', { vault: item.contact.vault_id, label: l.id }),
              }
            : null,
          description,
        },
        contact,
      };
    }

    case 'address_created':
    case 'address_updated':
    case 'address_destroyed': {
      const a = item.address;
      return {
        address: {
          object: a
            ? {
                id: a.id,
                line_1: a.line_1,
                line_2: a.line_2,
                city: a.city,
                province: a.province,
                postal_code: a.postal_code,
                country: a.country,
                type: a.address_type ? { id: a.address_type.id, name: label(a.address_type) } : null,
                url: {
                  show: mapLink(a),
                },
              }
            : null,
          description,
        },
        contact,
      };
    }

    case 'contact_information_created':
    case 'contact_information_updated':
    case 'contact_information_destroyed': {
      const info = item.contact_information;
      const type = info?.type;
      const typeName = type ? type.name || trans(type.name_translation_key ?? '') : null;
      return {
        information: {
          object: info
            ? {
                id: info.id,
                label: type.can_be_deleted ? typeName : info.data,
                data: type.protocol ? type.protocol + info.data : info.data,
                contact_information_type: { id: type.id, name: typeName },
              }
            : null,
          description,
        },
        contact,
      };
    }

    case 'pet_created':
    case 'pet_updated':
    case 'pet_destroyed':
      return {
        pet: {
          object: item.pet ? pet(item.pet) : null,
          description,
        },
        contact,
      };

    case 'note_created':
    case 'note_updated':
    case 'note_destroyed':
      return {
        note: {
          object: item.note ? { id: item.note.id, title: item.note.title, body: strLimit(item.note.body, 30) } : null,
          description,
        },
        contact,
      };

    case 'goal_created':
    case 'goal_updated':
    case 'goal_destroyed':
      return {
        goal: {
          object: item.goal ? { id: item.goal.id, name: item.goal.name } : null,
          description,
        },
        contact,
      };

    case 'mood_tracking_event_added':
    case 'mood_tracking_event_updated':
    case 'mood_tracking_event_deleted': {
      const e = item.mood_tracking_event;
      return {
        mood_tracking_event: {
          object: e
            ? {
                id: e.id,
                rated_at: formatUserDate(e.rated_at),
                note: e.note,
                number_of_hours_slept: e.number_of_hours_slept,
              }
            : null,
          description,
        },
        contact,
      };
    }

    default:
      return contact;
  }
}

/** One item of `ModuleFeedViewHelper::data` (a `FeedItem` of the contact or vault feed). */
export const feedItem = (item) => ({
  id: item.id,
  action: item.action,
  author: feedAuthor(item),
  sentence: trans(FEED_SENTENCES[item.action] ?? 'unknown action'),
  data: feedData(item),
  created_at: formatUserDate(item.created_at),
});

/**
 * The feed module: what `Shared/Modules/Feed.vue` needs to load the
 * contact's feed, with its first page (`FeedModule`).
 */
export const feed = (data, { vaultId, contactId }) => ({
  ...ids(vaultId, contactId),
  items: data.feed_items.map(feedItem),
  meta: data.meta,
});

// --- life events ---------------------------------------------------------

/** `ModuleLifeEventViewHelper::dtoLifeEventCategory` (`listVaultLifeEventCategories`). */
export const lifeEventCategory = (c) => ({
  id: c.id,
  label: seededLabel(c),
  life_event_types: c.life_event_types.map((t) => ({ id: t.id, label: seededLabel(t) })),
});

/**
 * `ModuleLifeEventViewHelper::dtoLifeEvent`, plus `distance_value` (the
 * stored number, for the edit form; `distance` is formatted).
 */
export const lifeEvent = (e) => ({
  id: e.id,
  emotion_id: e.emotion_id,
  collapsed: e.collapsed,
  summary: e.summary,
  description: e.description,
  happened_at: formatUserDate(e.happened_at),
  started_at: e.happened_at,
  costs: e.costs,
  currency_id: e.currency_id,
  paid_by_contact_id: e.paid_by_contact_id,
  duration_in_minutes: e.duration_in_minutes,
  distance: e.distance && e.distance_unit ? formatDistance(e.distance, e.distance_unit) : null,
  distance_value: e.distance,
  distance_unit: e.distance_unit,
  from_place: e.from_place,
  to_place: e.to_place,
  place: e.place,
  participants: e.participants.map(contactCard),
  timeline_event: {
    id: e.timeline_event_id,
  },
  life_event_type: {
    id: e.life_event_type.id,
    label: seededLabel(e.life_event_type),
    category: {
      id: e.life_event_category.id,
      label: seededLabel(e.life_event_category),
    },
  },
});

/**
 * `TimelineEvent::range`: the date of its first life event, or "first —
 * last" when the earliest and the latest are different life events.
 */
function timelineRange(lifeEvents) {
  if (lifeEvents.length === 0) return '';

  let first = lifeEvents[0];
  let last = lifeEvents[0];
  for (const e of lifeEvents) {
    if (e.happened_at < first.happened_at) first = e;
    if (e.happened_at > last.happened_at) last = e;
  }

  if (first.id === last.id) return formatUserDate(first.happened_at);

  return `${formatUserDate(first.happened_at)} — ${formatUserDate(last.happened_at)}`;
}

/** `ModuleLifeEventViewHelper::dtoTimelineEvent`. */
export const timelineEvent = (t) => ({
  id: t.id,
  label: t.label,
  collapsed: t.collapsed,
  happened_at: timelineRange(t.life_events),
  life_events: t.life_events.map(lifeEvent),
});

/**
 * `ModuleLifeEventViewHelper::data` for `contact` (a `ContactSummary`),
 * with the first page of its timeline events (`LifeEventsModule`). The
 * categories of the form are loaded by the component when
 * `life_event_categories` is null.
 */
export function lifeEventsModule(data, vaultId, contact) {
  const now = today().format('YYYY-MM-DD');

  return {
    ...ids(vaultId, contact.id),
    contact: contactCard(contact),
    current_date: now,
    current_date_human_format: formatUserDate(now),
    life_event_categories: null,
    timeline_events: data.timeline_events.map(timelineEvent),
    meta: data.meta,
  };
}

/** The life events module of the contact page. */
export const life_events = (data, { page, vaultId }) => lifeEventsModule(data, vaultId, page.contact);
