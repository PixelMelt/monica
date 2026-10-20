<script setup>
// A link to a page of the SPA (Inertia's <Link>): a plain <a href> whose
// clicks navigate in the app. Modified clicks, other targets and external
// URLs behave as normal links.
import { useRouter } from 'vue-router';

const props = defineProps({
  href: { type: String, default: '' },
  as: { type: String, default: 'a' },
  replace: { type: Boolean, default: false },
  data: { type: Object, default: null },
});

const router = useRouter();

const target = () => {
  if (!props.data) return props.href;
  const url = new URL(props.href, window.location.origin);
  for (const [k, v] of Object.entries(props.data)) url.searchParams.set(k, v);
  return url.pathname + url.search + url.hash;
};

const external = (href) => /^[a-z][a-z0-9+.-]*:/i.test(href) && !href.startsWith(window.location.origin);

const onClick = (event) => {
  if (event.defaultPrevented || event.button !== 0) return;
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  if (event.currentTarget.getAttribute?.('target') === '_blank') return;
  if (!props.href || external(props.href)) return;
  event.preventDefault();
  const to = target();
  props.replace ? router.replace(to) : router.push(to);
};
</script>

<template>
  <component :is="as" :href="as === 'a' ? href : undefined" @click="onClick">
    <slot />
  </component>
</template>
