// Page loaders: contacts. Each returns the props of its page (see router/index.js).
import { notPorted } from '@/loaders/common';

export const index = notPorted('contacts.index');

export const create = notPorted('contacts.create');

export const show = notPorted('contacts.show');

export const edit = notPorted('contacts.edit');

export const move = notPorted('contacts.move');

export const blank = notPorted('contacts.blank');

export const importantDates = notPorted('contacts.importantDates');

export const notes = notPorted('contacts.notes');

export const goal = notPorted('contacts.goal');

export const createRelationship = notPorted('contacts.createRelationship');

export const photos = notPorted('contacts.photos');

export const photo = notPorted('contacts.photo');
