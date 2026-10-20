<script setup>
import { ref } from 'vue';
import { trans } from 'laravel-vue-i18n';
import { useForm } from '@/lib/form';
import { requestPasswordReset } from '@/api/client';
import JetButton from '@/Components/Button.vue';
import JetGuestLayout from '@/Shared/Guest.vue';
import JetInput from '@/Components/Input.vue';
import JetLabel from '@/Components/Label.vue';
import JetValidationErrors from '@/Components/ValidationErrors.vue';

// `session('status')`: the answer of the last request (Q8: the same whether
// or not the email has an account).
const status = ref(null);

const form = useForm({
  email: '',
});

const submit = () => {
  form.submit((data) => requestPasswordReset({}, { email: data.email }), {
    onSuccess: () => (status.value = trans('passwords.sent')),
  });
};
</script>

<template>
  <JetGuestLayout>
    <div class="mb-4 text-sm text-gray-600 dark:text-gray-400">
      {{
        $t(
          'Forgot your password? No problem. Just let us know your email address and we will email you a password reset link that will allow you to choose a new one.',
        )
      }}
    </div>

    <div v-if="status" class="mb-4 text-sm font-medium text-green-600">
      {{ status }}
    </div>

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

      <div class="mt-4 flex items-center justify-end">
        <JetButton :class="{ 'opacity-25': form.processing }" :disabled="form.processing">
          {{ $t('Email Password Reset Link') }}
        </JetButton>
      </div>
    </form>
  </JetGuestLayout>
</template>
