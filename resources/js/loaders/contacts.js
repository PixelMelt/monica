// Page loaders: contacts. Each returns the props of its page (see router/index.js).
import { layoutData } from '@/loaders/common';
import { route } from '@/router';
import {
  getContact,
  getContactPage,
  getVault,
  listContactImportantDates,
  listContacts,
  listGenders,
  listPronouns,
  listTemplates,
  listVaultContactImportantDateTypes,
  listVaultLabels,
  listVaults,
  recordContactView,
} from '@/api/client';
import { ApiError } from '@/api/runtime';
import { vaultPermission } from '@/lib/session';
import { paginator } from '@/adapters/paginator';
import { moduleData } from '@/adapters/modules';
import * as dto from '@/adapters/contacts';

/**
 * `ContactController@index` (25 listed contacts, in the user's sort order)
 * and `ContactLabelController@index` (10 listed contacts with the label,
 * oldest first).
 */
export async function index(to) {
  const vaultId = to.params.vault;
  const labelId = to.params.label ? Number(to.params.label) : null;
  const page = Number(to.query.page ?? 1);
  const query = labelId ? { label_id: labelId, sort: 'created_at', per_page: 10 } : { per_page: 25 };
  const [contacts, labels] = await Promise.all([
    listContacts({ vault_id: vaultId, listed: true, page, ...query }),
    listVaultLabels({ vault_id: vaultId }),
  ]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.index(contacts.data, labels.data, vaultId, labelId),
    paginator: paginator(contacts.meta, contacts.data.length, to),
  };
}

/**
 * `ContactController@create`: `ContactCreateViewHelper::data`, after
 * `Gate::authorize('vault-editor', $vault)` (a 403 page for viewers).
 */
export async function create(to) {
  const vaultId = to.params.vault;
  const permission = vaultPermission(vaultId);
  if (permission === null || permission > 200) throw new ApiError(403, { title: 'Forbidden', code: 'forbidden' });
  const [genders, pronouns, templates, vault] = await Promise.all([
    listGenders(),
    listPronouns(),
    listTemplates(),
    getVault({ vault_id: vaultId }),
  ]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.create(genders.data, pronouns.data, templates.data, vault),
  };
}

/**
 * `ContactController@show` and `ContactPageController@show`: the contact
 * page (the requested tab, else the first one), counting a view. A contact
 * without a template goes to the template picker.
 */
export async function show(to) {
  const vaultId = to.params.vault;
  const contactId = to.params.contact;
  const page = await getContactPage({ vault_id: vaultId, contact_id: contactId, template_page: to.params.slug });
  if (page.contact.template_id === null) {
    return { redirect: route('contact.blank', { vault: vaultId, contact: contactId }) };
  }
  await recordContactView({ vault_id: vaultId, contact_id: contactId });

  // ContactShowViewHelper::getContactInformation and ::modules
  const ctx = { page, vaultId, contactId };
  const withData = (modules) => modules.map((m) => ({ id: m.id, type: m.type, data: moduleData(m, ctx) }));
  return {
    layoutData: layoutData(vaultId),
    data: dto.show(page, withData(page.header_modules), withData(page.modules)),
  };
}

/** `ContactController@edit`: `ContactEditViewHelper::data`. */
export async function edit(to) {
  const vaultId = to.params.vault;
  const [contact, genders, pronouns] = await Promise.all([
    getContact({ vault_id: vaultId, contact_id: to.params.contact }),
    listGenders(),
    listPronouns(),
  ]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.edit(contact, genders.data, pronouns.data),
  };
}

/** `ContactMoveController@show`: `ContactShowMoveViewHelper::data`. */
export async function move(to) {
  const vaultId = to.params.vault;
  const [contact, vaults] = await Promise.all([
    getContact({ vault_id: vaultId, contact_id: to.params.contact }),
    listVaults(),
  ]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.move(contact, vaults.data),
  };
}

/** `ContactNoTemplateController@show`: `ContactShowBlankViewHelper::data`. */
export async function blank(to) {
  const vaultId = to.params.vault;
  const [contact, templates] = await Promise.all([
    getContact({ vault_id: vaultId, contact_id: to.params.contact }),
    listTemplates(),
  ]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.blank(contact, templates.data),
  };
}

/** `ContactImportantDatesController@index`: `ContactImportantDatesViewHelper::data`. */
export async function importantDates(to) {
  const vaultId = to.params.vault;
  const contactId = to.params.contact;
  const [contact, dates, types] = await Promise.all([
    getContact({ vault_id: vaultId, contact_id: contactId }),
    listContactImportantDates({ vault_id: vaultId, contact_id: contactId }),
    listVaultContactImportantDateTypes({ vault_id: vaultId }),
  ]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.importantDates(contact, dates.data, types.data),
  };
}
