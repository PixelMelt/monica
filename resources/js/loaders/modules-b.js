// Page loaders: modules-b (the pages opened from the contact page's
// relationships and photos modules). Each returns the props of its page
// (see router/index.js).
import { layoutData } from '@/loaders/common';
import {
  getContact,
  getContactPhoto,
  listContactPhotos,
  listGenders,
  listPronouns,
  listRelationshipGroupTypes,
} from '@/api/client';
import { paginator } from '@/adapters/paginator';
import * as dto from '@/adapters/modules-b';

/** `ContactRelationshipsController@create`: `ContactRelationshipsCreateViewHelper::data`. */
export async function createRelationship(to) {
  const { vault, contact } = to.params;
  const [c, genders, pronouns, groupTypes] = await Promise.all([
    getContact({ vault_id: vault, contact_id: contact }),
    listGenders(),
    listPronouns(),
    listRelationshipGroupTypes(),
  ]);
  return {
    layoutData: layoutData(vault),
    data: dto.contactRelationshipsCreate(c, genders.data, pronouns.data, groupTypes.data),
  };
}

/** `ContactPhotoController@index`: `ContactPhotosIndexViewHelper::data`, 30 photos per page. */
export async function photos(to) {
  const { vault, contact } = to.params;
  const [c, list] = await Promise.all([
    getContact({ vault_id: vault, contact_id: contact }),
    listContactPhotos({ vault_id: vault, contact_id: contact, page: to.query.page, per_page: 30 }),
  ]);
  return {
    layoutData: layoutData(vault),
    data: dto.contactPhotosIndex(c, list),
    paginator: paginator(list.meta, list.data.length, to),
  };
}

/** `ContactPhotoController@show`: `ContactPhotosShowViewHelper::data`. */
export async function photo(to) {
  const { vault, contact, photo: photoId } = to.params;
  const [c, file] = await Promise.all([
    getContact({ vault_id: vault, contact_id: contact }),
    getContactPhoto({ vault_id: vault, contact_id: contact, photo_id: photoId }),
  ]);
  return {
    layoutData: layoutData(vault),
    data: dto.contactPhotosShow(c, file),
  };
}
