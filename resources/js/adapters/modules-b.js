// Contact page modules, set B (see adapters/modules.js): relationships,
// addresses, groups, contact information, documents, photos and posts, and
// the props of the pages they open (Vault/Contact/Relationships/Create,
// Vault/Contact/Photos/*), built from API objects as their ViewHelpers
// built them from models. Each function names the PHP method it ports.
//
// The module functions are named after their module type; the other
// exports must not take a module type's name (they share the registry).

import { trans } from 'laravel-vue-i18n';
import { urls } from '@/api/client';
import { route } from '@/router';
import { contactName, formatUserDate, label, sortByCollator } from '@/lib/format';
import { addressAsString, avatar, formatFileSize, mapLink } from '@/lib/helpers';

/** A seeded value: the stored text, else the translated key (the models' `! $value` accessors). */
const translated = (value, key) => value || (key ? trans(key) : null);

/** `/vaults/{vault}/files/{file}/content` of a file. */
const fileUrl = (vaultId, file, variant) => urls.getVaultFileContent({ vault_id: vaultId, file_id: file.id, variant });

// --- relationships -------------------------------------------------------

/** `ModuleRelationshipViewHelper::getContact`. */
const relatedContact = (contact) => ({
  id: contact.id,
  name: contactName(contact),
  avatar: avatar(contact),
  age: contact.age,
  url: {
    show: contact.listed ? route('contact.show', { vault: contact.vault_id, contact: contact.id }) : null,
  },
});

/**
 * One relationship of `ModuleRelationshipViewHelper::data`, seen from the
 * contact `contactId` (`id` is the relationship, for its removal).
 */
export const relationship = (r, contactId) => ({
  id: r.id,
  contact: relatedContact(r.other_contact),
  relationship_type: {
    id: r.relationship_type.id,
    name:
      r.contact_id === contactId
        ? translated(
            r.relationship_type.name_reverse_relationship,
            r.relationship_type.name_reverse_relationship_translation_key,
          )
        : translated(r.relationship_type.name, r.relationship_type.name_translation_key),
  },
});

/** `ModuleRelationshipViewHelper::data`. */
export const relationships = (data, { vaultId, contactId }) => ({
  relationship_group_types: data.relationship_group_types.map((group) => ({
    id: group.relationship_group_type.id,
    name: translated(group.relationship_group_type.name, group.relationship_group_type.name_translation_key),
    relationship_types: group.relationships.map((r) => relationship(r, contactId)),
  })),
  number_of_defined_relations: data.number_of_defined_relations,
  url: {
    create: route('contact.relationships.create', { vault: vaultId, contact: contactId }),
  },
});

/**
 * `ContactRelationshipsCreateViewHelper::data`, from getContact,
 * listGenders, listPronouns and listRelationshipGroupTypes.
 */
export const contactRelationshipsCreate = (contact, genders, pronouns, groupTypes) => ({
  contact: {
    id: contact.id,
    name: contactName(contact),
    avatar: avatar(contact),
  },
  genders: sortByCollator(genders.map((g) => ({ id: g.id, name: label(g) }))),
  pronouns: sortByCollator(pronouns.map((p) => ({ id: p.id, name: label(p) }))),
  relationship_group_types: groupTypes.map((group) => ({
    id: group.id,
    name: translated(group.name, group.name_translation_key),
    types: group.types.map((type) => ({
      id: type.id,
      name: `${translated(type.name, type.name_translation_key)} ↔ ${translated(type.name_reverse_relationship, type.name_reverse_relationship_translation_key)}`,
    })),
  })),
  relationship_types: groupTypes
    .flatMap((group) => group.types)
    .sort((a, b) => a.id - b.id)
    .map((type) => ({
      id: type.id,
      name: translated(type.name, type.name_translation_key),
      name_reverse_relationship: translated(
        type.name_reverse_relationship,
        type.name_reverse_relationship_translation_key,
      ),
    })),
  url: {
    back: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
  },
});

// --- addresses -----------------------------------------------------------

/** `ModuleContactAddressesViewHelper::dto`. */
export const address = (a) => ({
  id: a.id,
  is_past_address: a.is_past_address ?? false,
  line_1: a.line_1,
  line_2: a.line_2,
  city: a.city,
  province: a.province,
  postal_code: a.postal_code,
  country: a.country,
  type: a.address_type
    ? { id: a.address_type.id, name: translated(a.address_type.name, a.address_type.name_translation_key) }
    : null,
  url: {
    show: mapLink(a),
  },
});

/**
 * `ModuleContactAddressesViewHelper::data`. Its `address_types` come from
 * listAddressTypes (`addressTypes`), loaded by the module.
 */
export const addresses = (data) => ({
  active_addresses: data.active_addresses.map(address),
  inactive_addresses: data.inactive_addresses.map(address),
  addresses_in_vault: data.vault_addresses.map((a) => ({ id: a.id, address: addressAsString(a) })),
});

/** The `address_types` of `ModuleContactAddressesViewHelper::data`, from listAddressTypes. */
export const addressTypes = (types) =>
  types.map((t) => ({ id: t.id, name: translated(t.name, t.name_translation_key), selected: false }));

// --- groups --------------------------------------------------------------

/** `ModuleGroupsViewHelper::dto`, from a `Group` with its members. */
export const group = (g, vaultId, taken = false) => ({
  id: g.id,
  name: g.name,
  type: {
    id: g.group_type?.id ?? null,
  },
  contacts: g.members.map((member) => ({
    id: member.id,
    name: contactName(member),
    avatar: avatar(member),
    url: {
      show: route('contact.show', { vault: member.vault_id, contact: member.id }),
    },
  })),
  roles: (g.group_type?.roles ?? []).map((role) => ({
    id: role.id,
    label: translated(role.label, role.label_translation_key),
  })),
  selected: taken,
  url: {
    show: route('group.show', { vault: vaultId, group: g.id }),
  },
});

/**
 * `ModuleGroupsViewHelper::data`. Its `group_types` come from
 * listGroupTypes (`groupType`), loaded by the module.
 */
export const groups = (data, { vaultId }) => ({
  groups: data.groups.map((g) => group(g, vaultId)),
  available_groups: [
    { id: 0, name: trans('+ create a group'), selected: false },
    ...data.available_groups.map((g) => ({
      id: g.id,
      name: g.name,
      type: { id: g.group_type_id },
      selected: false,
      url: {
        show: route('group.show', { vault: vaultId, group: g.id }),
      },
    })),
  ],
});

/** `ModuleGroupsViewHelper::dtoGroupType`, from listGroupTypes. */
export const groupType = (t) => ({
  id: t.id,
  name: translated(t.label, t.label_translation_key),
  roles: t.roles.map((role) => ({
    id: role.id,
    name: translated(role.label, role.label_translation_key),
  })),
});

// --- contact information -------------------------------------------------

/** `ModuleContactInformationViewHelper::infoKinds`. */
const infoKinds = (kinds) =>
  Object.fromEntries(
    Object.entries(kinds).map(([type, list]) => [
      type,
      [{ id: '', name: '' }, ...sortByCollator(list.map((k) => ({ id: k.id, name: trans(k.name_translation_key) })))],
    ]),
  );

/**
 * `ModuleContactInformationViewHelper::dto`. `module` is the module's data
 * (as `contact_information` returns it), for the kinds and protocols.
 */
export function contactInformationItem(info, module) {
  const type = info.type;
  const typeName = translated(type.name, type.name_translation_key);

  let kind = null;
  if (info.kind !== null) {
    const known = module.contact_information_kinds[type.type]?.find((k) => k.id === info.kind.toLowerCase());
    kind = known ? { id: known.id, name: known.name ?? '' } : { id: '-1', name: info.kind };
  }

  // `ContactInformation::dataWithProtocol`
  const social = type.name_translation_key ? module.protocols[type.name_translation_key] : undefined;
  let dataWithProtocol = null;
  if (type.protocol !== null) dataWithProtocol = type.protocol + info.data;
  else if (social) dataWithProtocol = social.url + info.data;

  return {
    id: info.id,
    label: type.can_be_deleted ? typeName : info.data,
    protocol: type.protocol,
    data: info.data,
    data_with_protocol: dataWithProtocol,
    contact_information_type: {
      id: type.id,
      name: typeName,
      type: type.type,
    },
    contact_information_kind: kind,
  };
}

/**
 * `ModuleContactInformationViewHelper::data`. Its
 * `contact_information_types` come from listContactInformationTypes
 * (`contactInformationTypes`), loaded by the module.
 */
export function contact_information(data) {
  const module = {
    contact_information_kinds: infoKinds(data.kinds),
    contact_information_groups: Object.fromEntries(data.groups.map((g) => [g.type, trans(g.name_translation_key)])),
    protocols: Object.fromEntries(data.social_protocols.map((p) => [p.name_translation_key, p])),
  };
  const grouped = Object.entries(data.contact_information);
  // an empty collection was serialized as `[]`
  module.contact_information =
    grouped.length === 0
      ? []
      : Object.fromEntries(
          grouped.map(([type, list]) => [type, list.map((info) => contactInformationItem(info, module))]),
        );
  return module;
}

/**
 * The `contact_information_types` of `ModuleContactInformationViewHelper::data`:
 * the account's types grouped by `type`, each group sorted by name.
 */
export function contactInformationTypes(types, module) {
  const grouped = {};
  for (const t of types) {
    grouped[t.type] ??= { optgroup: module.contact_information_groups[t.type] ?? t.type, options: [] };
    grouped[t.type].options.push({
      id: t.id,
      name: translated(t.name, t.name_translation_key),
      type: t.type,
      name_translation_key: t.name_translation_key,
    });
  }
  for (const g of Object.values(grouped)) g.options = sortByCollator(g.options);
  return grouped;
}

// --- documents -----------------------------------------------------------

/** `ModuleDocumentsViewHelper::dto`. */
export const document = (file, vaultId) => ({
  id: file.id,
  name: file.name,
  mime_type: file.mime_type,
  size: formatFileSize(file.size),
  url: {
    download: fileUrl(vaultId, file, 'original'),
  },
});

/** `ModuleDocumentsViewHelper::data`. */
export const documents = (data, { page, vaultId }) => ({
  documents: data.documents.map((file) => document(file, vaultId)),
  canUploadFile: page.can_upload_file,
});

// --- photos --------------------------------------------------------------

/** `ModulePhotosViewHelper::dto` (300 px squares: the thumbnail). */
export const photo = (file, vaultId, contactId) => ({
  id: file.id,
  name: file.name,
  mime_type: file.mime_type,
  size: formatFileSize(file.size),
  url: {
    display: fileUrl(vaultId, file, 'thumbnail'),
    download: fileUrl(vaultId, file, 'original'),
    show: route('contact.photo.show', { vault: vaultId, contact: contactId, photo: file.id }),
  },
});

/** `ModulePhotosViewHelper::data`. */
export const photos = (data, { page, vaultId, contactId }) => ({
  photos: data.photos.map((file) => photo(file, vaultId, contactId)),
  canUploadFile: page.can_upload_file,
  url: {
    index: route('contact.photo.index', { vault: vaultId, contact: contactId }),
  },
});

/** `ContactPhotosIndexViewHelper::dto` (400 px squares: the original, cropped by CSS). */
export const contactPhotosIndexPhoto = (file, vaultId, contactId) => ({
  id: file.id,
  name: file.name,
  mime_type: file.mime_type,
  size: formatFileSize(file.size),
  url: {
    display: fileUrl(vaultId, file, 'original'),
    download: fileUrl(vaultId, file, 'original'),
    show: route('contact.photo.show', { vault: vaultId, contact: contactId, photo: file.id }),
  },
});

/** `ContactPhotosIndexViewHelper::data`, from getContact and listContactPhotos. */
export const contactPhotosIndex = (contact, list) => ({
  contact: {
    name: contactName(contact),
  },
  photos: list.data.map((file) => contactPhotosIndexPhoto(file, contact.vault_id, contact.id)),
  canUploadFile: list.can_upload_file,
  url: {
    show: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
  },
});

/** `ContactPhotosShowViewHelper::data`, from getContact and getContactPhoto. */
export const contactPhotosShow = (contact, file) => ({
  contact: {
    name: contactName(contact),
  },
  id: file.id,
  name: file.name,
  mime_type: file.mime_type,
  size: formatFileSize(file.size),
  url: {
    display: fileUrl(contact.vault_id, file, 'original'),
    download: fileUrl(contact.vault_id, file, 'original'),
    show: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
    index: route('contact.photo.index', { vault: contact.vault_id, contact: contact.id }),
  },
});

// --- posts ---------------------------------------------------------------

/** `ModulePostsViewHelper::dto`. */
export const post = (p, vaultId) => ({
  id: p.id,
  title: p.title || trans('Undefined'),
  journal: {
    id: p.journal.id,
    name: p.journal.name,
    url: {
      show: route('journal.show', { vault: vaultId, journal: p.journal.id }),
    },
  },
  written_at: formatUserDate(p.written_at),
  url: {
    show: route('post.show', { vault: vaultId, journal: p.journal.id, post: p.id }),
  },
});

/** `ModulePostsViewHelper::data`. */
export const posts = (data, { vaultId }) => data.posts.map((p) => post(p, vaultId));
