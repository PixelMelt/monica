// Page loaders: dashboard. Each returns the props of its page (see router/index.js).
import { layoutData } from '@/loaders/common';
import { getVaultDashboard, listVaultLifeEventCategories, listVaultReminders, listVaultTasks } from '@/api/client';
import { lifeEventCategory, lifeEventsModule } from '@/adapters/modules-a';
import * as dto from '@/adapters/dashboard';

/** `VaultController@show`: the dashboard (`VaultShowViewHelper`, life events and life metrics of the user's contact). */
export async function dashboard(to) {
  const vaultId = to.params.vault;
  const [data, categories] = await Promise.all([
    getVaultDashboard({ vault_id: vaultId }),
    listVaultLifeEventCategories({ vault_id: vaultId }),
  ]);

  return {
    layoutData: layoutData(vaultId),
    lastUpdatedContacts: dto.lastUpdatedContacts(data.last_updated_contacts),
    upcomingReminders: dto.upcomingReminders(data.upcoming_reminders, vaultId),
    favorites: dto.favorites(data.favorites),
    dueTasks: dto.dueTasks(data.due_tasks, vaultId),
    moodTrackingEvents: dto.moodTrackingEvents(data, vaultId),
    defaultTab: data.default_activity_tab,
    lifeEvents: {
      ...lifeEventsModule(data.life_events, vaultId, data.user_contact),
      life_event_categories: categories.data.map(lifeEventCategory),
    },
    lifeMetrics: dto.lifeMetrics(data.life_metrics, vaultId),
    feed: { vault_id: vaultId, contact_id: null },
  };
}

/** `VaultReminderController@index`: `VaultReminderIndexViewHelper::data`. */
export async function reminders(to) {
  const vaultId = to.params.vault;
  const list = await listVaultReminders({ vault_id: vaultId });

  return {
    layoutData: layoutData(vaultId),
    data: dto.reminderMonths(list.data),
  };
}

/** `VaultTaskController@index`: `VaultTasksIndexViewHelper::data`. */
export async function tasks(to) {
  const vaultId = to.params.vault;
  const list = await listVaultTasks({ vault_id: vaultId });

  return {
    layoutData: layoutData(vaultId),
    data: dto.vaultTasks(list.data),
  };
}
