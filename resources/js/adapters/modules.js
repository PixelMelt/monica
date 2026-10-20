// The `data` prop of each module of the contact page (Vault/Contact/Show),
// built from a `ContactPageModule` of `GET …/contacts/{id}/page`. Each
// adapters file below exports one function per module type, named after
// the type (`notes`, `contact_names`, …): `(data, ctx) => props.data`, with
// `ctx = { page, vaultId, contactId }` (`page` is the `ContactPage`).

import * as header from '@/adapters/contact-header';
import * as modulesA from '@/adapters/modules-a';
import * as modulesB from '@/adapters/modules-b';

const REGISTRY = { ...header, ...modulesA, ...modulesB };

export function moduleData(module, ctx) {
  const adapter = REGISTRY[module.type];
  if (!adapter) throw new Error(`no adapter for the ${module.type} module`);
  return adapter(module.data, { ...ctx, module });
}
