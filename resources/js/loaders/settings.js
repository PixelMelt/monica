// Page loaders: settings. Each returns the props of its page (see router/index.js).
import { trans } from 'laravel-vue-i18n';
import { layoutData, all } from '@/loaders/common';
import { route } from '@/router';
import { session } from '@/lib/session';
import {
  listMySessions,
  listMyTokens,
  listNotificationChannels,
  verifyNotificationChannel as sendVerification,
  getNotificationChannel,
  listNotificationChannelLogs,
  listAccountUsers,
  getAccountStorage,
} from '@/api/client';
import { ApiError } from '@/api/runtime';
import * as dto from '@/adapters/settings';

/** `can:administrator`: the account settings pages are for administrators. */
function administrator() {
  if (!session.user.is_account_administrator) {
    throw new ApiError(403, { detail: trans('This action is unauthorized.'), code: 'forbidden' });
  }
}

/** Jetstream's `UserProfileController::show` + `UserProfile`: the browser sessions and the profile features. */
export async function profile() {
  const sessions = await listMySessions();
  return {
    confirmsTwoFactorAuthentication: true,
    sessions: sessions.data.map(dto.session),
    locales: dto.locales(),
    layoutData: layoutData(),
    jetstream: {
      canUpdateProfileInformation: true,
      canUpdatePassword: true,
      canManageTwoFactorAuthentication: true,
      hasAccountDeletionFeatures: session.instance.features.account_deletion,
    },
    user: session.user,
  };
}

/** Jetstream's `ApiTokenController::index` (permissions from `JetstreamServiceProvider`). */
export async function apiTokens() {
  const tokens = await listMyTokens();
  return {
    layoutData: layoutData(),
    tokens: tokens.data.map(dto.apiToken),
    availablePermissions: ['read', 'write'],
    defaultPermissions: ['read'],
  };
}

/** `SettingsController::index`. */
export const index = () => ({
  layoutData: layoutData(),
  data: dto.settingsIndex(session.user),
});

/** `PreferencesController::index`. */
export const preferences = () => ({
  layoutData: layoutData(),
  data: dto.preferences(session.user.preferences),
});

/** `NotificationsController::index`. */
export async function notifications() {
  const channels = await listNotificationChannels();
  return {
    layoutData: layoutData(),
    data: dto.notifications(channels.data),
  };
}

/** `NotificationsVerificationController::store`: verifies the channel, then shows the channels. */
export async function verifyNotificationChannel(to) {
  await sendVerification({ notification_channel_id: to.params.notification }, { token: to.params.uuid });
  return notifications();
}

/** `NotificationsLogController::index`: someone else's or an unknown channel goes to the vaults. */
export async function notificationLogs(to) {
  const id = to.params.notification;
  try {
    const [channel, logs] = await Promise.all([
      getNotificationChannel({ notification_channel_id: id }),
      all(listNotificationChannelLogs, { notification_channel_id: id }),
    ]);
    return {
      layoutData: layoutData(),
      data: dto.notificationLogs(channel, logs),
    };
  } catch (e) {
    if (e instanceof ApiError && [400, 404, 422].includes(e.status)) return { redirect: route('vault.index') };
    throw e;
  }
}

/** `UserController::index`. */
export async function users() {
  administrator();
  const list = await listAccountUsers();
  return {
    layoutData: layoutData(),
    data: dto.users(list.data, session.user),
  };
}

/** `UserController::create`. */
export function createUser() {
  administrator();
  return {
    layoutData: layoutData(),
    data: dto.userCreate(),
  };
}

/** `AccountStorageController::index`. */
export async function storage() {
  administrator();
  const usage = await getAccountStorage();
  return {
    layoutData: layoutData(),
    data: dto.storage(usage),
  };
}

/** `CancelAccountController::index`. */
export function cancel() {
  administrator();
  return {
    layoutData: layoutData(),
    data: dto.cancelAccount(),
  };
}
