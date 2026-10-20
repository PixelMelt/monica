// The props of the groups pages, built from API objects as their ViewHelpers
// built them from models (`app/Domains/Contact/ManageGroups/Web/ViewHelpers`).

import { trans } from 'laravel-vue-i18n';
import { contactName } from '@/lib/format';
import { avatar } from '@/lib/helpers';
import { route } from '@/router';

/** `$model->label` of a seeded row: the label, else its translated key. */
const labelOf = (item) => item.label || (item.label_translation_key ? trans(item.label_translation_key) : null);

/** The contact rows of `GroupIndexViewHelper::data` and `GroupShowViewHelper::data`. */
const member = (contact) => ({
  id: contact.id,
  name: contactName(contact),
  age: contact.age,
  avatar: avatar(contact),
  url: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
});

/** `GroupIndexViewHelper::data` (one group of the list). */
export const groupIndex = (group) => ({
  id: group.id,
  name: group.name,
  url: {
    show: route('group.show', { vault: group.vault_id, group: group.id }),
  },
  contacts: group.members.map(member),
});

/** `GroupShowViewHelper::data`. */
export function groupShow(group) {
  const roles = group.roles
    .filter((bucket) => bucket.role !== null)
    .map((bucket) => ({
      id: bucket.role.id,
      label: labelOf(bucket.role),
      contacts: bucket.members.map(member),
    }));

  // the contacts that are not assigned to a role, only when there are some
  const noRole = group.roles.find((bucket) => bucket.role === null);
  if (noRole && noRole.members.length > 0) {
    roles.push({
      id: -1,
      label: trans('No role'),
      contacts: noRole.members.map(member),
    });
  }

  return {
    id: group.id,
    name: group.name,
    contact_count: group.members.length,
    type: {
      label: group.group_type ? labelOf(group.group_type) : null,
    },
    roles,
    url: {
      edit: route('group.edit', { vault: group.vault_id, group: group.id }),
    },
  };
}

/** `GroupEditViewHelper::data`, with the account's group types (by position). */
export const groupEdit = (group, groupTypes) => ({
  id: group.id,
  name: group.name,
  group_type_id: group.group_type?.id ?? null,
  group_types: groupTypes.map((groupType) => ({
    id: groupType.id,
    name: labelOf(groupType),
  })),
  url: {
    back: route('group.show', { vault: group.vault_id, group: group.id }),
  },
});
