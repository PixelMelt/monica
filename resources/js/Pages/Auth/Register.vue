<script setup>
import { useForm } from '@/lib/form';
import { visit, intended } from '@/router';
import { registerUser } from '@/api/client';
import JetGuestLayout from '@/Shared/Guest.vue';
import JetInput from '@/Components/Input.vue';
import JetLabel from '@/Components/Label.vue';
import JetValidationErrors from '@/Components/ValidationErrors.vue';
import PrimaryButton from '@/Shared/Form/PrimaryButton.vue';

const form = useForm({
  first_name: '',
  last_name: '',
  username: '',
  password: '',
  password_confirmation: '',
});

const submit = () => {
  form.submit(
    (data) =>
      registerUser(
        {},
        {
          first_name: data.first_name,
          last_name: data.last_name,
          username: data.username,
          password: data.password,
          password_confirmation: data.password_confirmation,
        },
      ),
    {
      onSuccess: () => visit(intended()),
      onFinish: () => form.reset('password', 'password_confirmation'),
    },
  );
};
</script>

<template>
  <JetGuestLayout>
    <JetValidationErrors class="mb-4" />

    <p class="mb-2 text-lg font-bold">{{ $t('Set up Monica') }}</p>
    <p class="mb-8 text-sm text-gray-500">{{ $t('Your name here will be used to add yourself as a contact.') }}</p>

    <form @submit.prevent="submit">
      <div>
        <JetLabel for="first_name" :value="$t('First name')" />
        <JetInput
          id="first_name"
          v-model="form.first_name"
          type="text"
          class="mt-1 block w-full"
          required
          autofocus
          :autocomplete="'first_name'" />
      </div>

      <div class="mt-4">
        <JetLabel for="last_name" :value="$t('Last name')" />
        <JetInput
          id="last_name"
          v-model="form.last_name"
          type="text"
          class="mt-1 block w-full"
          required
          :autocomplete="'last_name'" />
      </div>

      <div class="mt-4">
        <JetLabel for="username" :value="$t('Username')" />
        <JetInput
          id="username"
          v-model="form.username"
          type="text"
          class="mt-1 block w-full"
          required
          autocapitalize="none"
          autocomplete="username" />
      </div>

      <div class="mt-4">
        <JetLabel for="password" :value="$t('Password')" />
        <JetInput
          id="password"
          v-model="form.password"
          type="password"
          class="mt-1 mb-1 block w-full"
          required
          autocomplete="new-password" />
        <p class="text-xs text-gray-600">{{ $t('The password should be at least 8 characters long.') }}</p>
      </div>

      <div class="mb-8 mt-4">
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
        <PrimaryButton :text="$t('Create Account')" :class="'save'" />
      </div>
    </form>
  </JetGuestLayout>
</template>
