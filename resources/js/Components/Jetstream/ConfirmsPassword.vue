<script setup>
import { ref, reactive, nextTick } from 'vue';
import { trans } from 'laravel-vue-i18n';
import { confirmPassword as sendPassword, getPasswordConfirmationStatus } from '@/api/client';
import Button from '@/Components/Button.vue';
import DialogModal from './DialogModal.vue';
import Input from '@/Components/Input.vue';
import InputError from '@/Components/InputError.vue';
import SecondaryButton from './SecondaryButton.vue';

const emit = defineEmits(['confirmed']);

defineProps({
  title: {
    type: String,
    default: trans('Confirm access'),
  },
  content: {
    type: String,
    default: trans('For your security, please confirm the access to continue.'),
  },
  button: {
    type: String,
    default: trans('Confirm'),
  },
});

const confirmingPassword = ref(false);

const form = reactive({
  password: '',
  error: '',
  processing: false,
});

const passwordInput = ref(null);

const startConfirmingPassword = () => {
  getPasswordConfirmationStatus().then((status) => {
    if (status.confirmed) {
      emit('confirmed');
    } else {
      confirmingPassword.value = true;

      nextTick().then(() => passwordInput.value.focus());
    }
  });
};

const confirmPassword = () => {
  form.processing = true;

  sendPassword({}, { password: form.password })
    .then(() => {
      form.processing = false;

      confirm();
    })
    .catch((error) => {
      form.processing = false;
      form.error = error.errors?.password?.[0] ?? error.message;
      nextTick().then(() => passwordInput.value.focus());
    });
};

const confirm = () => {
  closeModal();
  nextTick().then(() => emit('confirmed'));
};

const closeModal = () => {
  confirmingPassword.value = false;
  form.password = '';
  form.error = '';
};
</script>

<template>
  <span>
    <span @click="startConfirmingPassword">
      <slot />
    </span>

    <DialogModal :show="confirmingPassword" @close="closeModal">
      <template #title>
        {{ title }}
      </template>

      <template #content>
        {{ content }}

        <div class="mt-4">
          {{ $t('Authenticate using your password:') }}
          <Input
            ref="passwordInput"
            v-model="form.password"
            type="password"
            class="mt-1 block w-3/4"
            :placeholder="$t('Password')"
            :autocomplete="'current-password'"
            @keyup.enter="confirmPassword" />

          <InputError :message="form.error" class="mt-2" />
        </div>
      </template>

      <template #footer>
        <SecondaryButton @click="closeModal">
          {{ $t('Cancel') }}
        </SecondaryButton>

        <Button
          class="ms-3"
          :class="{ 'opacity-25': form.processing }"
          :disabled="form.processing"
          @click="confirmPassword">
          {{ button }}
        </Button>
      </template>
    </DialogModal>
  </span>
</template>
