<script setup>
import { trans } from 'laravel-vue-i18n';
import { useForm } from '@/lib/form';
import { route, visit } from '@/router';
import { resetPassword } from '@/api/client';
import { flashStatus } from '@/adapters/auth';
import JetButton from '@/Components/Button.vue';
import JetGuestLayout from '@/Shared/Guest.vue';
import JetInput from '@/Components/Input.vue';
import JetLabel from '@/Components/Label.vue';
import JetValidationErrors from '@/Components/ValidationErrors.vue';

const props = defineProps({
  email: String,
  token: String,
});

const form = useForm({
  token: props.token,
  email: props.email,
  password: '',
  password_confirmation: '',
});

const submit = () => {
  form.submit(
    (data) =>
      resetPassword(
        {},
        {
          token: data.token,
          email: data.email,
          password: data.password,
          password_confirmation: data.password_confirmation,
        },
      ),
    {
      onSuccess: () => {
        flashStatus(trans('passwords.reset'));
        return visit(route('login'));
      },
      onFinish: () => form.reset('password', 'password_confirmation'),
    },
  );
};
</script>

<template>
  <JetGuestLayout>
    <JetValidationErrors class="mb-4" />

    <form @submit.prevent="submit">
      <div>
        <JetLabel for="email" :value="$t('Email')" />
        <JetInput
          id="email"
          v-model="form.email"
          type="email"
          class="mt-1 block w-full"
          required
          autofocus
          autocomplete="username" />
      </div>

      <div class="mt-4">
        <JetLabel for="password" :value="$t('Password')" />
        <JetInput
          id="password"
          v-model="form.password"
          type="password"
          class="mt-1 block w-full"
          required
          autocomplete="new-password" />
      </div>

      <div class="mt-4">
        <JetLabel for="password_confirmation" :value="$t('Confirm Password')" />
        <JetInput
          id="password_confirmation"
          v-model="form.password_confirmation"
          type="password"
          class="mt-1 block w-full"
          required
          autocomplete="new-password" />
      </div>

      <div class="mt-4 flex items-center justify-end">
        <JetButton :class="{ 'opacity-25': form.processing }" :disabled="form.processing">
          {{ $t('Reset Password') }}
        </JetButton>
      </div>
    </form>
  </JetGuestLayout>
</template>
