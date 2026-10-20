// The props of the Settings/Personalize pages, built from API objects as
// their ViewHelpers built them from models (`app/Domains/Settings/*`).

import { label } from '@/lib/format';

/** `ManageGenderIndexViewHelper::dtoGender`. */
export const gender = (g) => ({ id: g.id, name: label(g) });
