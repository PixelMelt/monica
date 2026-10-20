// What every page knows about the visitor: the instance info (public) and,
// once logged in, `GET /me/bootstrap` (the user, preferences and vaults).
// The router fetches it again on every page visit, as the server used to
// send the layout data with every page.

import { reactive } from 'vue';
import { loadLanguageAsync } from 'laravel-vue-i18n';
import { getBootstrap, getInstance } from '@/api/client';
import { setLocale } from '@/lib/format';

export const session = reactive({
  /** `Instance` (signup enabled, locales, version…). */
  instance: null,
  /** `CurrentUser`, or null when logged out. */
  user: null,
  /** `BootstrapVault[]` of the user. */
  vaults: [],
});

let current = null;

const RTL = ['ar', 'he', 'ur'];

async function applyLocale(locale) {
  const html = document.documentElement;
  html.lang = locale.replace('_', '-');
  html.dir = RTL.includes(locale.split('_')[0]) ? 'rtl' : 'ltr';
  setLocale(locale);
  await loadLanguageAsync(locale);
}

/** Fetches the session again; resolves to `session`. */
export function refreshSession({ authenticated = true } = {}) {
  current = (async () => {
    if (authenticated) {
      try {
        const boot = await getBootstrap();
        session.instance = boot.instance;
        session.user = boot.user;
        session.vaults = boot.vaults;
      } catch (e) {
        if (e.status !== 401 && e.status !== 403) throw e;
        session.user = null;
        session.vaults = [];
        if (e.status === 403) throw e;
      }
    }
    if (!session.user) {
      session.instance = await getInstance();
    }
    await applyLocale(session.user?.preferences.locale ?? session.instance.locale);
    return session;
  })();
  return current;
}

/** The session of the current page visit. */
export function currentSession() {
  return current ?? refreshSession();
}

/** `$user->name`. */
export const userName = (user) => `${user.first_name ?? ''} ${user.last_name ?? ''}`;

/** The user's permission in a vault (100 manage, 200 edit, 300 view), or null. */
export function vaultPermission(vaultId) {
  return session.vaults.find((v) => v.id === vaultId)?.permission ?? null;
}

/** `$user->locale_ietf`: the user's locale as a language tag (`pt-BR`). */
export const localeIetf = () =>
  (session.user?.preferences.locale ?? session.instance?.locale ?? 'en').replace('_', '-');
