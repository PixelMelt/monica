<script setup>
// Sets the document title as Inertia's <Head :title> did: "<title> - Monica".
import { watchEffect } from 'vue';

const props = defineProps({ title: { type: String, default: null } });

watchEffect(() => {
  if (props.title) document.title = `${props.title} - Monica`;
});
</script>

<template>
  <slot />
</template>
