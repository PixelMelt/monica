// The props of the instance administration pages, built from API objects.

import { route } from '@/router';
import { session, userName } from '@/lib/session';
import { formatDate } from '@/lib/format';
import { formatFileSize } from '@/lib/helpers';

/** An account of the install, with its users and what it holds. */
export const account = (a) => ({
  id: a.id,
  users: a.users.map((u) => ({
    id: u.id,
    username: u.username,
    name: userName(u).trim() || u.username,
    is_account_administrator: u.is_account_administrator,
    is_instance_administrator: u.is_instance_administrator,
  })),
  vault_count: a.vault_count,
  contact_count: a.contact_count,
  file_size: formatFileSize(a.file_size),
  created_at: a.created_at ? formatDate(a.created_at) : null,
  is_own: a.id === session.user.account_id,
});

/** The accounts page. */
export const accounts = (list) => ({
  accounts: list.map(account),
  url: {
    settings: route('settings.index'),
  },
});
