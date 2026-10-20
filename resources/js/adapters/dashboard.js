// The props of the vault dashboard, reminders and tasks pages, built from
// the API objects as `VaultShowViewHelper`, `VaultReminderIndexViewHelper`,
// `VaultTasksIndexViewHelper` and `VaultLifeMetricsViewHelper` built them.

import { route } from '@/router';
import { contactName, formatMonthAndYear, formatMonthNumber, formatUserDate, today } from '@/lib/format';
import { avatar } from '@/lib/helpers';
import { seededLabel, task } from '@/adapters/modules-a';

const pad = (n) => String(n).padStart(2, '0');

/** The `{id, name, avatar, url: {show}}` contact of the dashboard lists. */
const contactLink = (c) => ({
  id: c.id,
  name: contactName(c),
  avatar: avatar(c),
  url: {
    show: route('contact.show', { vault: c.vault_id, contact: c.id }),
  },
});

/** `VaultShowViewHelper::lastUpdatedContacts`. */
export const lastUpdatedContacts = (contacts) => contacts.map(contactLink);

/** `VaultShowViewHelper::favorites`. */
export const favorites = (contacts) => contacts.map(contactLink);

/** A reminder of `VaultShowViewHelper::upcomingReminders` and `VaultReminderIndexViewHelper::data`. */
const upcomingReminder = (r) => ({
  id: r.contact_reminder_id,
  label: r.label,
  scheduled_at: formatUserDate(r.scheduled_at),
  contact: contactLink(r.contact),
});

/** `VaultShowViewHelper::upcomingReminders`. */
export const upcomingReminders = (reminders, vaultId) => ({
  reminders: reminders.map(upcomingReminder),
  url: {
    index: route('vault.reminder.index', { vault: vaultId }),
  },
});

/** `VaultShowViewHelper::dueTasks`. */
export const dueTasks = (tasks, vaultId) => ({
  tasks: tasks.map((t) => ({ ...task(t), contact: contactLink(t.contact) })),
  url: {
    index: route('vault.tasks.index', { vault: vaultId }),
  },
});

/**
 * `VaultShowViewHelper::moodTrackingEvents`. The form records the mood of
 * the user's own contact (`contact_id`).
 */
export const moodTrackingEvents = (dashboard, vaultId) => ({
  mood_tracking_parameters: dashboard.mood_tracking_parameters.map((p) => ({
    id: p.id,
    label: seededLabel(p),
    hex_color: p.hex_color,
  })),
  current_date: dashboard.current_date,
  vault_id: vaultId,
  contact_id: dashboard.user_contact.id,
  url: {
    history: route('vault.reports.mood_tracking_events.index', { vault: vaultId }),
  },
});

/** `VaultLifeMetricsViewHelper::dto`. */
export const lifeMetric = (m) => ({
  id: m.id,
  incremented: false,
  show_graph: false,
  label: m.label,
  stats: m.stats,
  years: m.years.map((year) => ({ year })),
  months: m.months.map((month) => ({
    id: month.month,
    friendly_name: formatMonthNumber(`${today().year()}-${pad(month.month)}-01`),
    events: month.events,
  })),
  max_number_of_events: m.max_number_of_events,
});

/** `VaultLifeMetricsViewHelper::data`, plus the vault of its calls. */
export const lifeMetrics = (metrics, vaultId) => ({
  data: metrics.map(lifeMetric),
  vault_id: vaultId,
});

/** `VaultReminderIndexViewHelper::data`: the next 12 months. */
export const reminderMonths = (months) =>
  months.map((m, i) => ({
    id: i,
    month: formatMonthAndYear(`${m.year}-${pad(m.month)}-01`),
    reminders: m.reminders.map(upcomingReminder),
  }));

/** `VaultTasksIndexViewHelper::data`: the open tasks, by contact. */
export const vaultTasks = (groups) =>
  groups.map((g) => ({
    tasks: g.tasks.map((t) => ({
      id: t.id,
      contact_id: t.contact_id,
      label: t.label,
      due_at: t.due_at
        ? {
            formatted: formatUserDate(t.due_at),
            value: t.due_at,
            is_late: t.is_late,
          }
        : null,
    })),
    contact: contactLink(g.contact),
  }));
