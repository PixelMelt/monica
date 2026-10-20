import './bootstrap';
import '../css/app.css';

import { createApp } from 'vue';
import { i18nVue } from 'laravel-vue-i18n';
import App from './App.vue';
import router, { route } from './router';
import methods from './methods';
import { onApiError } from './api/runtime';
import { localeIetf } from './lib/session';

const langs = import.meta.glob('../../lang/*.json');
const groups = import.meta.glob('../../lang/*/{format,pagination}.json');

/** `group.key` → text for the nested arrays of a group file. */
function flatten(prefix, value, out = {}) {
  for (const [k, v] of Object.entries(value)) {
    if (v !== null && typeof v === 'object') flatten(`${prefix}.${k}`, v, out);
    else out[`${prefix}.${k}`] = v;
  }
  return out;
}

/** The strings of `lang/<lang>.json`, with the `format` and `pagination` groups of `lang/<lang>/` as `group.key`. */
async function messages(lang) {
  const app = (await langs[`../../lang/${lang}.json`]?.())?.default ?? {};
  const merged = {};
  for (const group of ['format', 'pagination']) {
    const file = await groups[`../../lang/${lang}/${group}.json`]?.();
    if (file) flatten(group, file.default, merged);
  }
  return { default: { ...merged, ...app } };
}

onApiError((error, op) => {
  // A request made on a page after the session ended: log in again. (The
  // session probe and page loaders are handled by the router's guard.)
  const current = router.currentRoute.value;
  if (
    error.status === 401 &&
    op.path !== '/me/bootstrap' &&
    current.matched.length > 0 &&
    !['guest', 'any'].includes(current.meta.access)
  ) {
    router.push({ name: 'login' });
  }
});

const app = createApp(App);

app
  .use(router)
  .use(i18nVue, {
    resolve: messages,
  })
  .mixin({ methods: { route, localeIetf, ...methods } })
  .mount('#app');
