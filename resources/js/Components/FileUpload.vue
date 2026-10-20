<script setup>
// The upload widget (P004, local storage): a click on the slot opens the
// browser's file picker; each chosen file is sent as multipart `file` with
// `upload(formData)` (a call of the API client), then `success` is emitted
// with the API's `File`, or `error` with the ApiError.
import { ref } from 'vue';
import { trans } from 'laravel-vue-i18n';
import { flash } from '@/methods';

const props = defineProps({
  upload: { type: Function, required: true },
  multiple: { type: Boolean, default: false },
  imagesOnly: { type: Boolean, default: false },
  accept: { type: String, default: null },
});

const emit = defineEmits(['success', 'error']);

const input = ref(null);
const uploading = ref(false);

const IMAGE_TYPES = 'image/jpeg,image/png,image/gif,image/webp';

const open = () => {
  if (!uploading.value) input.value.click();
};

const send = async (event) => {
  const files = [...event.target.files];
  event.target.value = '';
  uploading.value = true;
  try {
    for (const file of files) {
      const body = new FormData();
      body.append('file', file, file.name);
      try {
        emit('success', await props.upload(body));
      } catch (error) {
        flash(error.message ?? trans('Oops! Something went wrong.'), 'error');
        emit('error', error);
      }
    }
  } finally {
    uploading.value = false;
  }
};
</script>

<template>
  <div :class="{ 'cursor-wait opacity-50': uploading }" :aria-busy="uploading" @click.prevent="open">
    <slot :uploading="uploading" />
    <input
      ref="input"
      type="file"
      class="hidden"
      :multiple="multiple"
      :accept="accept ?? (imagesOnly ? IMAGE_TYPES : null)"
      @change="send" />
  </div>
</template>
