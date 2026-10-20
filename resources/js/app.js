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

onApiError((error) => {
  // A request after the session ended: log in again, then come back.
  if (error.status === 401 && router.currentRoute.value.meta.access !== 'guest') {
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
