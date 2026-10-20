// Page loaders: modules-a. Each returns the props of its page (see router/index.js).
import { layoutData } from '@/loaders/common';
import { route } from '@/router';
import { getContact, getContactGoal, listContactNotes, listEmotions } from '@/api/client';
import { contactName } from '@/lib/format';
import { paginator } from '@/adapters/paginator';
import * as dto from '@/adapters/modules-a';

/** `ContactNotesController@index`: `NotesIndexViewHelper::data`, 10 notes per page. */
export async function notes(to) {
  const vaultId = to.params.vault;
  const contactId = to.params.contact;
  const [list, contact, emotions] = await Promise.all([
    listContactNotes({ vault_id: vaultId, contact_id: contactId, page: to.query.page ?? 1, per_page: 10 }),
    getContact({ vault_id: vaultId, contact_id: contactId }),
    listEmotions(),
  ]);

  return {
    layoutData: layoutData(vaultId),
    data: {
      contact: {
        name: contactName(contact),
      },
      notes: list.data.map(dto.note),
      emotions: emotions.data.map(dto.emotion),
      vault_id: vaultId,
      contact_id: contactId,
      url: {
        contact: route('contact.show', { vault: vaultId, contact: contactId }),
      },
    },
    paginator: paginator(list.meta, list.data.length, to),
  };
}

/** `ContactGoalController@show`: `GoalShowViewHelper::data`. */
export async function goal(to) {
  const vaultId = to.params.vault;
  const contactId = to.params.contact;
  const [detail, contact] = await Promise.all([
    getContactGoal({ vault_id: vaultId, contact_id: contactId, goal_id: to.params.goal }),
    getContact({ vault_id: vaultId, contact_id: contactId }),
  ]);

  return {
    layoutData: layoutData(vaultId),
    data: dto.goalDetail(detail, contact),
  };
}
