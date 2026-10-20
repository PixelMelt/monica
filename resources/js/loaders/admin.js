// Page loaders: instance administration. Each returns the props of its page (see router/index.js).
import { trans } from 'laravel-vue-i18n';
import { layoutData } from '@/loaders/common';
import { session } from '@/lib/session';
import { listAdminAccounts } from '@/api/client';
import { ApiError } from '@/api/runtime';
import * as dto from '@/adapters/admin';

/** The instance administrator's dashboard: the accounts of the install. */
export async function accounts() {
  if (!session.user.is_instance_administrator) {
    throw new ApiError(403, { detail: trans('This action is unauthorized.'), code: 'forbidden' });
  }
  const list = await listAdminAccounts();
  return {
    layoutData: layoutData(),
    data: dto.accounts(list.data),
  };
}
