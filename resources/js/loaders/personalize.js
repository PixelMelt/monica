// Page loaders: personalize. Each returns the props of its page (see router/index.js).
import { notPorted, layoutData } from '@/loaders/common';
import { route } from '@/router';
import { listGenders } from '@/api/client';
import { sortByCollator } from '@/lib/format';
import * as dto from '@/adapters/personalize';

export const index = notPorted('personalize.index');

export const relationships = notPorted('personalize.relationships');

export const callReasons = notPorted('personalize.callReasons');

export const giftOccasions = notPorted('personalize.giftOccasions');

export const giftStates = notPorted('personalize.giftStates');

export const postTemplates = notPorted('personalize.postTemplates');

export const groupTypes = notPorted('personalize.groupTypes');

/** `ManageGenderIndexViewHelper::data`. */
export async function genders() {
  const list = await listGenders();
  return {
    layoutData: layoutData(),
    data: {
      genders: sortByCollator(list.data.map(dto.gender)),
      url: {
        settings: route('settings.index'),
        personalize: route('settings.personalize.index'),
      },
    },
  };
}

export const pronouns = notPorted('personalize.pronouns');

export const addressTypes = notPorted('personalize.addressTypes');

export const petCategories = notPorted('personalize.petCategories');

export const contactInformationTypes = notPorted('personalize.contactInformationTypes');

export const templates = notPorted('personalize.templates');

export const template = notPorted('personalize.template');

export const modules = notPorted('personalize.modules');

export const currencies = notPorted('personalize.currencies');

export const religions = notPorted('personalize.religions');
