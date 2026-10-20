import './bootstrap';
import '../css/app.css';

import { createApp } from 'vue';
import { i18nVue } from 'laravel-vue-i18n';
import App from './App.vue';
import router, { route } from './router';
import methods from './methods';
import { onApiError } from './api/runtime';
import { helpLinks } from './lib/help';
import { localeIetf } from './lib/session';

const langs = import.meta.glob('../../lang/*.json');

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
app.config.globalProperties.helpLinks = helpLinks;

app
  .use(router)
  .use(i18nVue, {
    resolve: (lang) => langs[`../../lang/${lang}.json`]?.() ?? Promise.resolve({ default: {} }),
  })
  .mixin({ methods: { route, localeIetf, ...methods } })
  .mount('#app');
