// The props of the contacts pages (Vault/Contact/*) and of the contact
// page's header blocks, built from API objects as their ViewHelpers built
// them from models (`app/Domains/Contact/Manage*/Web/ViewHelpers`). The
// header modules' `data` adapters are in adapters/contact-header.js.

import { trans } from 'laravel-vue-i18n';
import { contactName, getDays, getMonths, label as translated, sortByCollator } from '@/lib/format';
import { avatar, formatImportantDate, groupsSummary, importantDateType } from '@/lib/helpers';
import { urls } from '@/api/client';
import { session } from '@/lib/session';
import { route } from '@/router';

/**
 * A form value as Laravel's `ConvertEmptyStringsToNull` passed it on:
 * `''` (and undefined) become null.
 */
export const nullable = (value) => (value === '' || value === undefined ? null : value);

/** An id picked in a `Dropdown` (a string, '' or -1/0 for none) as the API's integer or null. */
export const toId = (value) => {
  const id = Number(nullable(value) ?? 0);
  return id > 0 ? id : null;
};

/** `$religion->name`: the name, else the translated key. */
const religionName = (r) => r.name ?? (r.translation_key ? trans(r.translation_key) : null);

/** `$template->label` of a quick fact template: the label, else the translated key. */
const quickFactTemplateLabel = (t) => t.label || (t.label_translation_key ? trans(t.label_translation_key) : null);

/** SQL `ORDER BY name ASC` on the stored column (nulls first, then by id as stored). */
const byStoredName = (items) =>
  [...items].sort((a, b) => {
    if (a.name === b.name) return 0;
    if (a.name === null) return -1;
    if (b.name === null) return 1;
    return a.name < b.name ? -1 : 1;
  });

// --- ContactIndexViewHelper --------------------------------------------

/**
 * `ContactIndexViewHelper::data`: the contacts of the page, the vault's
 * labels that have contacts, and the sort options.
 */
export function index(contacts, labels, vaultId, labelId) {
  return {
    contacts: contacts.map((contact) => ({
      id: contact.id,
      name: contactName(contact),
      avatar: avatar(contact),
      url: {
        show: route('contact.show', { vault: vaultId, contact: contact.id }),
      },
    })),
    labels: sortByCollator(labels)
      .filter((label) => label.contacts_count > 0)
      .map((label) => ({
        id: label.id,
        name: label.name,
        count: label.contacts_count,
        url: {
          show: route('contact.label.index', { vault: label.vault_id, label: label.id }),
        },
      })),
    current_label: labelId,
    user_contact_sort_order: session.user.preferences.contact_sort_order,
    contact_sort_orders: [
      { id: 'last_updated', name: trans('By last updated') },
      { id: 'asc', name: trans('From A to Z') },
      { id: 'desc', name: trans('From Z to A') },
    ],
    url: {
      contact: {
        index: route('contact.index', { vault: vaultId }),
        create: route('contact.create', { vault: vaultId }),
      },
    },
  };
}

// --- ContactCreateViewHelper, ContactEditViewHelper --------------------

/** `ContactCreateViewHelper::data`. */
export function create(genders, pronouns, templates, vault) {
  return {
    genders: sortByCollator(genders.map((g) => ({ id: g.id, name: translated(g) }))),
    pronouns: sortByCollator(pronouns.map((p) => ({ id: p.id, name: translated(p) }))),
    templates: sortByCollator(
      templates.map((t) => ({ id: t.id, name: translated(t), selected: t.id === vault.default_template_id })),
    ),
    url: {
      back: route('contact.index', { vault: vault.id }),
    },
  };
}

/** `ContactEditViewHelper::data`. */
export function edit(contact, genders, pronouns) {
  return {
    contact: {
      id: contact.id,
      name: contactName(contact),
      first_name: contact.first_name,
      last_name: contact.last_name,
      middle_name: contact.middle_name,
      nickname: contact.nickname,
      maiden_name: contact.maiden_name,
      gender_id: contact.gender_id,
      pronoun_id: contact.pronoun_id,
      prefix: contact.prefix,
      suffix: contact.suffix,
    },
    genders: byStoredName(genders).map((g) => ({
      id: g.id,
      name: translated(g),
      selected: g.id === contact.gender_id,
    })),
    pronouns: byStoredName(pronouns).map((p) => ({
      id: p.id,
      name: translated(p),
      selected: p.id === contact.pronoun_id,
    })),
    url: {
      show: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
    },
  };
}

// --- ContactShowMoveViewHelper, ContactShowBlankViewHelper -------------

/** `ContactShowMoveViewHelper::data`: the user's other vaults. */
export function move(contact, vaults) {
  return {
    vaults: sortByCollator(vaults)
      .filter((vault) => vault.id !== contact.vault_id)
      .map((vault) => ({ id: vault.id, name: vault.name, count: vault.contacts_count })),
    contact: {
      name: contactName(contact),
    },
  };
}

/** `ContactShowBlankViewHelper::data`: the account's templates. */
export function blank(contact, templates) {
  return {
    templates: templates.map((t) => ({ id: t.id, name: translated(t) })),
    contact: {
      name: contactName(contact),
    },
  };
}

// --- ContactShowViewHelper ---------------------------------------------

/**
 * `ContactShowViewHelper::data` / `::dataForTemplatePage` from a
 * `ContactPage`; `contactInformation` and `modules` are the header and page
 * modules with their `data` already built (`getContactInformation`,
 * `modules`).
 */
export function show(page, contactInformation, modules) {
  const contact = page.contact;
  return {
    contact_name: contactNames(contact),
    listed: contact.listed,
    template_pages: page.template_pages.map((p) => ({
      id: p.id,
      name: translated(p),
      selected: page.active_template_page_id !== null ? p.id === page.active_template_page_id : null,
      url: {
        show: route('contact.page.show', { vault: contact.vault_id, contact: contact.id, slug: p.slug }),
      },
    })),
    contact_information: contactInformation,
    group_summary_information: groupsSummary(page.groups, contact.vault_id),
    quick_fact_template_entries: quickFacts(page),
    modules,
    avatar: {
      hasFile: contact.avatar_file_id !== null,
    },
    options: {
      can_be_archived: !page.is_current_user_contact,
      can_be_deleted: !page.is_current_user_contact,
    },
    url: {
      update_template: route('contact.blank', { vault: contact.vault_id, contact: contact.id }),
      move_contact: route('contact.move.show', { vault: contact.vault_id, contact: contact.id }),
      download_vcard: urls.getContactVCard({ vault_id: contact.vault_id, contact_id: contact.id }),
    },
  };
}

/** `ContactShowViewHelper::quickFacts`: the quick facts block. */
export function quickFacts(page) {
  const first = page.quick_fact_templates[0];
  return {
    show_quick_facts: page.contact.show_quick_facts,
    templates: page.quick_fact_templates.map(quickFactTemplate),
    quick_facts: first
      ? {
          template: quickFactTemplate(first),
          quick_facts: page.quick_facts.map(quickFact),
        }
      : null,
  };
}

/** The `template` of `ContactModuleQuickFactViewHelper::data`. */
export const quickFactTemplate = (template) => ({
  id: template.id,
  label: quickFactTemplateLabel(template),
});

/** `ContactModuleQuickFactViewHelper::dto`. */
export const quickFact = (q) => ({ id: q.id, content: q.content });

// --- Header modules (ManageContactName, ManageLabels, ManageJobInformation, ManageReligion)

/**
 * `ModuleContactNameViewHelper::data` from a `ContactNamesModule` or a
 * `Contact` (both carry the names and the user's `is_favorite`).
 */
export const contactNames = (contact) => ({
  name: contactName(contact),
  is_favorite: contact.is_favorite,
  url: {
    edit: route('contact.edit', { vault: contact.vault_id, contact: contact.id }),
  },
});

/** `ModuleLabelViewHelper::dtoLabel`. */
export const label = (l, taken = false) => ({
  id: l.id,
  name: l.name,
  bg_color: l.bg_color,
  text_color: l.text_color,
  taken,
  url: {
    show: route('contact.label.index', { vault: l.vault_id, label: l.id }),
  },
});

/**
 * `ModuleLabelViewHelper::data`: the contact's labels. The vault's labels
 * (`labels_in_vault`) are loaded by the block when it opens.
 */
export const labelsModule = (labels) => ({
  labels_in_contact: labels.map((l) => label(l)),
  labels_in_vault: null,
});

/** `ModuleCompanyViewHelper::dto`. */
export const company = (c, contactCompanyId) => ({
  id: c.id,
  name: c.name,
  type: c.type,
  selected: c.id === contactCompanyId,
});

/** `ModuleCompanyViewHelper::data` from a `JobInformation` (`CompanyModule`). */
export const jobInformation = (info) => ({
  job_position: info.job_position,
  company: info.company ? company(info.company, info.company.id) : null,
});

/** `ModuleReligionViewHelper::dto`. */
export const religion = (r, contactReligionId) => ({
  id: r.id,
  name: religionName(r),
  selected: r.id === contactReligionId,
});

/**
 * `ModuleReligionViewHelper::data` from a `ContactReligion`
 * (`ReligionsModule`). The account's religions (`religions`) are loaded by
 * the block when it opens.
 */
export const religionModule = (data) => ({
  religion: data.religion ? { id: data.religion.id, name: religionName(data.religion) } : null,
  religions: null,
});

// --- ContactImportantDatesViewHelper -----------------------------------

/** `ContactImportantDatesViewHelper::dto`. */
export function importantDate(date) {
  const type = date.contact_important_date_type;
  return {
    id: date.id,
    label: date.label,
    date: formatImportantDate(date),
    type: type ? { id: type.id, label: type.label } : null,
    age: date.age,
    choice: importantDateType(date),
    completeDate: importantDateType(date) === 'full_date' ? `${date.year}-${date.month}-${date.day}` : '',
    month: date.month,
    day: date.day,
  };
}

/** `ContactImportantDatesViewHelper::data`. */
export function importantDates(contact, dates, types) {
  return {
    contact: {
      name: contactName(contact),
    },
    dates: dates.map(importantDate),
    months: getMonths(),
    days: getDays(),
    // an empty entry so the dropdown can be nullified
    date_types: [{ id: -1, name: '' }, ...types.map((type) => ({ id: type.id, name: type.label }))],
    url: {
      contact: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
    },
  };
}
