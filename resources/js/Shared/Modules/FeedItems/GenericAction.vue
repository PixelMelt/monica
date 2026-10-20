<template>
  <div
    v-if="!contactViewMode"
    class="flex items-center rounded-lg border border-gray-300 px-3 py-2 dark:border-gray-700">
    <avatar
      :data="data.avatar"
      :class="'relative me-2 h-8 w-8 rounded-full border border-gray-200 dark:border-gray-800'" />

    <div class="flex flex-col">
      <AppLink :href="data.url" class="text-gray-800 hover:underline dark:text-gray-200">{{ data.name }}</AppLink>
      <span v-if="data.age || data.age === 0" class="text-gray-500">{{ data.age }}</span>
      <span v-else class="text-xs text-gray-500">{{ $t('Unknown age') }}</span>
    </div>
  </div>
</template>

<script>
import Link from '@/router/Link.vue';
import Avatar from '@/Shared/Avatar.vue';

export default {
  components: {
    AppLink: Link,
    Avatar,
  },

  props: {
    data: {
      type: Object,
      default: null,
    },
    contactViewMode: {
      type: Boolean,
      default: true,
    },
  },
};
</script>
