<script setup>
// Laravel's minimal error page ("404 | Not Found").
import { computed } from 'vue';
import { trans } from 'laravel-vue-i18n';
import Head from '@/router/Head.vue';

const props = defineProps({ status: { type: Number, default: 404 } });

const MESSAGES = {
  401: 'Unauthorized',
  403: 'Forbidden',
  404: 'Not Found',
  419: 'Page Expired',
  429: 'Too Many Requests',
  500: 'Server Error',
  503: 'Service Unavailable',
};

const message = computed(() => trans(MESSAGES[props.status] ?? MESSAGES[500]));
</script>

<template>
  <Head :title="message" />
  <div class="relative flex min-h-screen items-top justify-center bg-gray-100 dark:bg-gray-900 sm:items-center sm:pt-0">
    <div class="mx-auto max-w-xl sm:px-6 lg:px-8">
      <div class="flex items-center pt-8 sm:justify-start sm:pt-0">
        <div class="border-r border-gray-400 px-4 text-lg tracking-wider text-gray-500">{{ status }}</div>
        <div class="ml-4 text-lg uppercase tracking-wider text-gray-500">{{ message }}</div>
      </div>
    </div>
  </div>
</template>
