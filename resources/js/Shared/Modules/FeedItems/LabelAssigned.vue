<template>
  <div class="rounded-lg border border-gray-300 dark:border-gray-700">
    <!-- contact information -->
    <div
      v-if="!contactViewMode"
      class="flex items-center border-b border-gray-300 px-3 py-2 text-sm dark:border-gray-700">
      <avatar
        :data="data.contact.avatar"
        :class="'relative me-2 h-5 w-5 rounded-full border border-gray-200 dark:border-gray-800'" />

      <div class="flex flex-col">
        <AppLink :href="data.contact.url" class="text-gray-800 hover:underline dark:text-gray-200">{{
          data.contact.name
        }}</AppLink>
      </div>
    </div>

    <div class="px-3 pb-0 pt-2">
      <!-- the label still exists in the system -->
      <span
        v-if="data.label.object"
        class="mb-2 me-2 inline-block rounded-xs px-2 py-1 text-xs font-semibold last:me-0"
        :class="data.label.object.bg_color + ' ' + data.label.object.text_color">
        <AppLink :href="data.label.object.url">{{ data.label.object.name }}</AppLink>
      </span>

      <!-- the label was deleted -->
      <span
        v-else
        class="mb-2 me-2 inline-block rounded-xs bg-neutral-200 px-2 py-1 text-xs font-semibold text-neutral-800 last:me-0">
        <span>{{ data.label.description }}</span>
      </span>
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
