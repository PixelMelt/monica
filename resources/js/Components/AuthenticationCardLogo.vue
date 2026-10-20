<script setup>
import Link from '@/router/Link.vue';
import ApplicationLogo from '@/Components/ApplicationLogo.vue';
</script>

<template>
  <Link :href="'/'">
    <div class="flex items-center">
      <ApplicationLogo />
    </div>
  </Link>
</template>
