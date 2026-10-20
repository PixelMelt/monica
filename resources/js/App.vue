<script setup>
import ErrorPage from '@/Pages/Error.vue';
</script>

<template>
  <RouterView v-slot="{ Component, route }">
    <ErrorPage v-if="route.meta.error" :status="route.meta.error.status" />
    <component :is="Component" v-else-if="Component" :key="route.fullPath" v-bind="route.meta.props" />
  </RouterView>
</template>
