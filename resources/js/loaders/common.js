// Helpers shared by the page loaders.

import { session, userName } from '@/lib/session';
import { route } from '@/router';

/** Placeholder for a page whose loader is not written yet. */
export const notPorted = (name) => () => {
  throw new Error(`The loader ${name} is not ported yet.`);
};

/**
 * `VaultIndexViewHelper::layoutData($vault)`: what `Layouts/Layout.vue`
 * shows, from the session of this visit.
 */
export function layoutData(vaultId = null) {
  const user = session.user;
  const vault = vaultId ? session.vaults.find((v) => v.id === vaultId) : null;
  return {
    user: { id: user.id, name: userName(user) },
    vault: vault
      ? {
          id: vault.id,
          name: vault.name,
          permission: {
            at_least_editor: vault.permission <= 200,
            at_least_manager: vault.permission <= 100,
          },
          visibility: {
            show_group_tab: vault.show_group_tab,
            show_tasks_tab: vault.show_tasks_tab,
            show_files_tab: vault.show_files_tab,
            show_journal_tab: vault.show_journal_tab,
            show_companies_tab: vault.show_companies_tab,
            show_reports_tab: vault.show_reports_tab,
            show_calendar_tab: vault.show_calendar_tab,
          },
          url: {
            dashboard: route('vault.show', { vault: vault.id }),
            contacts: route('contact.index', { vault: vault.id }),
            calendar: route('vault.calendar.index', { vault: vault.id }),
            journals: route('journal.index', { vault: vault.id }),
            groups: route('group.index', { vault: vault.id }),
            companies: route('vault.companies.index', { vault: vault.id }),
            tasks: route('vault.tasks.index', { vault: vault.id }),
            files: route('vault.files.index', { vault: vault.id }),
            reports: route('vault.reports.index', { vault: vault.id }),
            settings: route('vault.settings.index', { vault: vault.id }),
            search: route('vault.search.index', { vault: vault.id }),
          },
        }
      : null,
    url: {
      vaults: route('vault.index'),
      settings: route('settings.index'),
    },
  };
}

/** Fetches every page of a paginated list operation. */
export async function all(operation, params = {}) {
  const items = [];
  for (let page = 1; ; page++) {
    const res = await operation({ ...params, page, per_page: 100 });
    items.push(...res.data);
    if (!res.meta || page >= res.meta.last_page) return items;
  }
}
