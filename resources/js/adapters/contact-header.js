// Contact page modules: see adapters/modules.js. The header modules (the
// template's `contact_information` page, left column of Vault/Contact/Show):
// one function per module type, `(data, ctx) => props.data`. Only module
// adapters are exported here (the registry takes every export); the shared
// pieces are in adapters/contacts.js.

import { route } from '@/router';
import { contactName, label as translated } from '@/lib/format';
import { avatar as contactAvatar } from '@/lib/helpers';
import { urls } from '@/api/client';
import * as dto from '@/adapters/contacts';

/**
 * `ModuleAvatarViewHelper::data`. An uploaded avatar is shown from its file
 * (its URL changes with each upload), the generated one from the avatar
 * endpoint.
 */
export function avatar(data, ctx) {
  return {
    avatar:
      data.avatar_file_id !== null
        ? {
            type: 'url',
            content: urls.getVaultFileContent({
              vault_id: ctx.vaultId,
              file_id: data.avatar_file_id,
              variant: 'thumbnail',
            }),
          }
        : contactAvatar({ vault_id: ctx.vaultId, id: ctx.contactId }),
  };
}

/** `ModuleContactNameViewHelper::data`. */
export function contact_names(data) {
  return dto.contactNames(data);
}

/**
 * `ModuleFamilySummaryViewHelper::data`. `id` is the index of the
 * relationship type among the types that have relations, as `getRelations`
 * numbered them.
 */
export function family_summary(data) {
  const relations = (members) => {
    const types = [];
    return members.map((member) => {
      if (!types.includes(member.relationship_type_id)) types.push(member.relationship_type_id);
      const contact = member.contact;
      return {
        id: types.indexOf(member.relationship_type_id),
        contact: {
          id: contact.id,
          name: contactName(contact),
          avatar: contactAvatar(contact),
          age: contact.age,
          url: {
            show: contact.listed ? route('contact.show', { vault: contact.vault_id, contact: contact.id }) : null,
          },
        },
      };
    });
  };
  return {
    family_relationships: relations(data.family_relationships),
    love_relationships: relations(data.love_relationships),
  };
}

/** `ModuleGenderPronounViewHelper::data`. */
export function gender_pronoun(data, ctx) {
  return {
    gender: data.gender ? translated(data.gender) : null,
    pronoun: data.pronoun ? translated(data.pronoun) : null,
    url: {
      edit: route('contact.edit', { vault: ctx.vaultId, contact: ctx.contactId }),
    },
  };
}

/** `ModuleImportantDatesViewHelper::data`. */
export function important_dates(data, ctx) {
  return {
    dates: data.important_dates.map((date) => {
      const full = dto.importantDate(date);
      return {
        id: full.id,
        label: full.label,
        date: full.date,
        type: full.type ? full.type.label : null,
        age: full.age,
      };
    }),
    url: {
      edit: route('contact.date.index', { vault: ctx.vaultId, contact: ctx.contactId }),
    },
  };
}

/** `ModuleLabelViewHelper::data`. */
export function labels(data) {
  return dto.labelsModule(data.labels);
}

/** `ModuleCompanyViewHelper::data`. */
export function company(data) {
  return dto.jobInformation(data);
}

/** `ModuleReligionViewHelper::data`. */
export function religions(data) {
  return dto.religionModule(data);
}
