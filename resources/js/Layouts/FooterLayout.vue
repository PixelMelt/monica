<script setup>
import { computed } from 'vue';
import { trans } from 'laravel-vue-i18n';
import { session } from '@/lib/session';

const REPOSITORY = 'https://github.com/monicahq/monica/';
const LINK_CLASS = 'underline text-xs dark:text-gray-100 hover:text-gray-900 hover:dark:text-gray-200';

const escape = (text) =>
  text.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

// `HandleInertiaRequests::footer()`: the version, and the commit linked to
// the repository (rendered as `Str::markdownExternalLink` did).
const footer = computed(() => {
  const instance = session.instance;
  if (!instance) return '';
  if (!instance.commit) return escape(trans('Version :version.', { version: instance.version }));
  const url = `${REPOSITORY}commit/${instance.commit}`;
  const message = escape(
    trans('Version :version — commit [:short](:url).', { version: instance.version, short: 'SHORT', url: 'URL' }),
  );
  const link = `<a rel="noopener noreferrer" target="_blank" class="${LINK_CLASS}" href="${escape(url)}">${escape(instance.commit.slice(0, 7))}</a>`;
  return message.replace('[SHORT](URL)', link);
});
</script>

<template>
  <footer class="mb-8 mt-4 text-center text-xs dark:text-gray-100">
    <div class="mb-2">
      {{ $t('Monica. All rights reserved. 2017 — :date.', { date: new Date().getFullYear().toString() }) }}
      {{ $t('Made from all over the world. We ❤️ you.') }}
    </div>
    <div>
      {{ $t('This site is open source.') }}
      <span v-html="footer"></span>
    </div>
  </footer>
</template>
