// The props of the Settings/Personalize pages, built from API objects as
// their ViewHelpers built them from models (`app/Domains/Settings/*`).

import { trans } from 'laravel-vue-i18n';
import { route } from '@/router';
import { label, sortByCollator } from '@/lib/format';

/** A seeded `label` / `label_translation_key` pair, shown as `label()` shows a name. */
const seededLabel = (item) => label({ name: item.label, name_translation_key: item.label_translation_key });

/** `ManageGenderIndexViewHelper::dtoGender`. */
export const gender = (g) => ({ id: g.id, name: label(g) });

/** `PersonalizePronounIndexViewHelper::dtoPronoun`. */
export const pronoun = (p) => ({ id: p.id, name: label(p) });

/** `PersonalizeAddressTypeIndexViewHelper::dtoAddressType`. */
export const addressType = (a) => ({ id: a.id, name: label(a) });

/** `PersonalizePetCategoriesIndexViewHelper::dtoPetCategory`. */
export const petCategory = (c) => ({ id: c.id, name: label(c) });

/** `PersonalizeContactInformationTypeIndexViewHelper::dtoContactInformationType`. */
export const contactInformationType = (t) => ({
  id: t.id,
  name: label(t),
  protocol: t.protocol,
  type: t.type,
  can_be_deleted: t.can_be_deleted ?? true,
});

/**
 * `PersonalizeContactInformationTypeIndexViewHelper::data`'s
 * `contact_information_types`: the types grouped by `type`, each group
 * sorted by name.
 */
export function contactInformationTypes(types) {
  const groups = {};
  for (const t of types) {
    (groups[t.type ?? ''] ??= []).push(t);
  }
  for (const key of Object.keys(groups)) {
    groups[key] = sortByCollator(groups[key].map(contactInformationType));
  }
  return groups;
}

/** `ModuleContactInformationViewHelper::infoGroups` (config `app.contact_information_groups`). */
export const infoGroups = () => ({
  email: trans('Email address'),
  phone: trans('Phone number'),
  IMPP: trans('Instant messaging'),
  'X-SOCIAL-PROFILE': trans('Social profile'),
});

/** `PersonalizeRelationshipIndexViewHelper::dtoGroupType`. */
export const relationshipGroupType = (g) => ({
  id: g.id,
  name: label(g),
  can_be_deleted: g.can_be_deleted,
  types: g.types.map(relationshipType),
});

/** `PersonalizeRelationshipIndexViewHelper::dtoRelationshipType`. */
export const relationshipType = (t) => ({
  id: t.id,
  name: label(t),
  name_reverse_relationship: label({
    name: t.name_reverse_relationship,
    name_translation_key: t.name_reverse_relationship_translation_key,
  }),
  can_be_deleted: t.can_be_deleted,
});

/** `PersonalizeCallReasonsIndexViewHelper::dtoReasonType`. */
export const callReasonType = (t) => ({
  id: t.id,
  label: seededLabel(t),
  reasons: t.reasons.map(callReason),
});

/** `PersonalizeCallReasonsIndexViewHelper::dtoReason`. */
export const callReason = (r) => ({ id: r.id, label: seededLabel(r) });

/** `PersonalizeGiftOccasionViewHelper::dto`. */
export const giftOccasion = (o) => ({ id: o.id, label: seededLabel(o), position: o.position });

/** `PersonalizeGiftStateViewHelper::dto`. */
export const giftState = (s) => ({ id: s.id, label: seededLabel(s), position: s.position });

/** `PersonalizePostTemplateViewHelper::dto`. */
export const postTemplate = (t) => ({
  id: t.id,
  label: seededLabel(t),
  position: t.position,
  can_be_deleted: t.can_be_deleted,
  post_template_sections: t.sections.map((s) => postTemplateSection(t.id, s)),
});

/** `PersonalizePostTemplateViewHelper::dtoPostTemplateSection`. */
export const postTemplateSection = (postTemplateId, s) => ({
  id: s.id,
  label: seededLabel(s),
  position: s.position,
  can_be_deleted: s.can_be_deleted,
  post_template_id: postTemplateId,
});

/** `PersonalizeGroupTypeViewHelper::dto`. */
export const groupType = (t) => ({
  id: t.id,
  label: seededLabel(t),
  position: t.position,
  group_type_roles: t.roles.map((r) => groupTypeRole(t.id, r)),
});

/** `PersonalizeGroupTypeViewHelper::dtoGroupTypeRole`. */
export const groupTypeRole = (groupTypeId, r) => ({
  id: r.id,
  label: seededLabel(r),
  position: r.position,
  group_type_id: groupTypeId,
});

/** `PersonalizeTemplateIndexViewHelper::dtoTemplate`. */
export const template = (t) => ({
  id: t.id,
  name: label(t),
  can_be_deleted: t.can_be_deleted,
  url: {
    show: route('settings.personalize.template.show', { template: t.id }),
  },
});

/** `PersonalizeTemplateShowViewHelper::dtoTemplatePage`. */
export const templatePage = (p) => ({
  id: p.id,
  name: label(p),
  position: p.position,
  can_be_deleted: p.can_be_deleted,
});

/** `PersonalizeTemplatePageShowViewHelper::data`, from `getTemplatePage`. */
export const templatePageShow = (p) => ({
  page: {
    id: p.id,
    name: label(p),
  },
  modules: p.modules.map(templatePageModule),
  modules_in_account: sortByCollator(
    p.available_modules.map((m) => ({ id: m.id, name: label(m), already_used: m.already_used })),
  ),
});

/** `PersonalizeTemplatePageShowViewHelper::dtoModule`. */
export const templatePageModule = (m) => ({ id: m.id, name: label(m), position: m.position });

/** `PersonalizeModuleIndexViewHelper::dtoModule`. */
export const module = (m) => ({
  id: m.id,
  name: label(m),
  type: m.type,
  reserved_to_contact_information: m.reserved_to_contact_information,
  can_be_deleted: m.can_be_deleted,
});

/** `PersonalizeCurrencyIndexViewHelper::dtoCurrency` (`name` comes translated from the API). */
export const currency = (c) => ({ id: c.id, code: c.code, name: c.name, active: c.active });

/** `PersonalizeReligionViewHelper::dto`. */
export const religion = (r) => ({
  id: r.id,
  name: label({ name: r.name, name_translation_key: r.translation_key }),
  position: r.position,
});
