<script setup>
import { session } from '@/lib/session';
import { helpUrl } from '@/lib/help';
import { Tooltip } from 'ant-design-vue';
import { computed } from 'vue';
import { MessageCircleQuestion } from 'lucide-vue-next';

const props = defineProps({
  url: String,
  top: {
    type: String,
    default: '3px',
  },
});

const finalURL = computed(() => {
  return helpUrl + props.url;
});
</script>

<template>
  <a
    v-if="session.user?.preferences.help_shown"
    class="relative inline cursor-pointer"
    :href="finalURL"
    :style="'top:' + top"
    target="_blank"
    lang="en"
    rel="noopener noreferrer">
    <Tooltip placement="topLeft" :title="$t('This link will open in a new tab')" arrow-point-at-center>
      <MessageCircleQuestion
        class="h-4 w-4 text-gray-400 hover:text-gray-600 dark:text-gray-400 dark:hover:text-gray-600" />
    </Tooltip>
  </a>
</template>
