// Page loaders: personalize. Each returns the props of its page (see router/index.js).
import { layoutData } from '@/loaders/common';
import { route } from '@/router';
import {
  getTemplate,
  listAccountCurrencies,
  listAccountModules,
  listAddressTypes,
  listCallReasonTypes,
  listContactInformationTypes,
  listGenders,
  listGiftOccasions,
  listGiftStates,
  listGroupTypes,
  listPetCategories,
  listPostTemplates,
  listPronouns,
  listRelationshipGroupTypes,
  listReligions,
  listTemplates,
} from '@/api/client';
import { ApiError } from '@/api/runtime';
import { label, sortByCollator } from '@/lib/format';
import { session } from '@/lib/session';
import * as dto from '@/adapters/personalize';

/** `can:administrator` on every personalize route: a 403 page for the other users. */
function administrator() {
  if (!session.user?.is_account_administrator) throw new ApiError(403, { title: 'Forbidden', code: 'forbidden' });
}

/** The breadcrumb links every personalize page has. */
const breadcrumb = () => ({
  settings: route('settings.index'),
  personalize: route('settings.personalize.index'),
});

/** `PersonalizeIndexViewHelper::data`. */
export async function index() {
  administrator();
  return {
    layoutData: layoutData(),
    data: {
      url: {
        settings: route('settings.index'),
        back: route('settings.index'),
        manage_relationships: route('settings.personalize.relationship.index'),
        manage_genders: route('settings.personalize.gender.index'),
        manage_pronouns: route('settings.personalize.pronoun.index'),
        manage_address_types: route('settings.personalize.address_type.index'),
        manage_pet_categories: route('settings.personalize.pet_category.index'),
        manage_contact_information_types: route('settings.personalize.contact_information_type.index'),
        manage_templates: route('settings.personalize.template.index'),
        manage_modules: route('settings.personalize.module.index'),
        manage_currencies: route('settings.personalize.currency.index'),
        manage_call_reasons: route('settings.personalize.call_reasons.index'),
        manage_gift_occasions: route('settings.personalize.gift_occasions.index'),
        manage_gift_states: route('settings.personalize.gift_states.index'),
        manage_group_types: route('settings.personalize.group_types.index'),
        manage_post_templates: route('settings.personalize.post_templates.index'),
        manage_religions: route('settings.personalize.religions.index'),
      },
    },
  };
}

/** `PersonalizeRelationshipIndexViewHelper::data`. */
export async function relationships() {
  administrator();
  const list = await listRelationshipGroupTypes();
  return {
    layoutData: layoutData(),
    data: {
      group_types: sortByCollator(list.data.map(dto.relationshipGroupType)),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizeCallReasonsIndexViewHelper::data`. */
export async function callReasons() {
  administrator();
  const list = await listCallReasonTypes();
  return {
    layoutData: layoutData(),
    data: {
      call_reason_types: sortByCollator(list.data.map(dto.callReasonType), 'label'),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizeGiftOccasionViewHelper::data`. */
export async function giftOccasions() {
  administrator();
  const list = await listGiftOccasions();
  return {
    layoutData: layoutData(),
    data: {
      gift_occasions: list.data.map(dto.giftOccasion),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizeGiftStateViewHelper::data`. */
export async function giftStates() {
  administrator();
  const list = await listGiftStates();
  return {
    layoutData: layoutData(),
    data: {
      gift_states: list.data.map(dto.giftState),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizePostTemplateViewHelper::data`. */
export async function postTemplates() {
  administrator();
  const list = await listPostTemplates();
  return {
    layoutData: layoutData(),
    data: {
      post_templates: list.data.map(dto.postTemplate),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizeGroupTypeViewHelper::data`. */
export async function groupTypes() {
  administrator();
  const list = await listGroupTypes();
  return {
    layoutData: layoutData(),
    data: {
      group_types: list.data.map(dto.groupType),
      url: breadcrumb(),
    },
  };
}

/** `ManageGenderIndexViewHelper::data`. */
export async function genders() {
  administrator();
  const list = await listGenders();
  return {
    layoutData: layoutData(),
    data: {
      genders: sortByCollator(list.data.map(dto.gender)),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizePronounIndexViewHelper::data`. */
export async function pronouns() {
  administrator();
  const list = await listPronouns();
  return {
    layoutData: layoutData(),
    data: {
      pronouns: sortByCollator(list.data.map(dto.pronoun)),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizeAddressTypeIndexViewHelper::data`. */
export async function addressTypes() {
  administrator();
  const list = await listAddressTypes();
  return {
    layoutData: layoutData(),
    data: {
      address_types: sortByCollator(list.data.map(dto.addressType)),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizePetCategoriesIndexViewHelper::data`. */
export async function petCategories() {
  administrator();
  const list = await listPetCategories();
  return {
    layoutData: layoutData(),
    data: {
      pet_categories: sortByCollator(list.data.map(dto.petCategory)),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizeContactInformationTypeIndexViewHelper::data`. */
export async function contactInformationTypes() {
  administrator();
  const list = await listContactInformationTypes();
  return {
    layoutData: layoutData(),
    data: {
      contact_information_types: dto.contactInformationTypes(list.data),
      contact_information_groups: dto.infoGroups(),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizeTemplateIndexViewHelper::data`. */
export async function templates() {
  administrator();
  const list = await listTemplates();
  return {
    layoutData: layoutData(),
    data: {
      templates: sortByCollator(list.data.map(dto.template)),
      url: breadcrumb(),
    },
  };
}

/**
 * `PersonalizeTemplatesController@show` / `PersonalizeTemplateShowViewHelper::data`:
 * an unknown template sends the user to the vaults, as `redirect('vaults')` did.
 */
export async function template(to) {
  administrator();
  let detail;
  try {
    detail = await getTemplate({ template_id: to.params.template });
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) return { redirect: route('vault.index') };
    throw e;
  }
  const contactInformationPage = detail.pages.find((p) => p.type === 'contact_information');
  return {
    layoutData: layoutData(),
    data: {
      template: {
        id: detail.id,
        name: label(detail),
      },
      template_page_contact_information: contactInformationPage ? dto.templatePage(contactInformationPage) : null,
      template_pages: detail.pages.filter((p) => p.type === null).map(dto.templatePage),
      url: {
        ...breadcrumb(),
        templates: route('settings.personalize.template.index'),
      },
    },
  };
}

/** `PersonalizeModuleIndexViewHelper::data`. */
export async function modules() {
  administrator();
  const list = await listAccountModules();
  return {
    layoutData: layoutData(),
    data: {
      modules: sortByCollator(list.data.map(dto.module)),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizeCurrencyIndexViewHelper::data`. */
export async function currencies() {
  administrator();
  const list = await listAccountCurrencies();
  return {
    layoutData: layoutData(),
    data: {
      currencies: list.data.map(dto.currency),
      url: breadcrumb(),
    },
  };
}

/** `PersonalizeReligionViewHelper::data`. */
export async function religions() {
  administrator();
  const list = await listReligions();
  return {
    layoutData: layoutData(),
    data: {
      religions: list.data.map(dto.religion),
      url: breadcrumb(),
    },
  };
}
