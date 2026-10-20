<script setup>
import { computed, ref } from 'vue';
import { useForm } from '@/lib/form';
import { route, visit } from '@/router';
import { sendEmailVerification, logOut } from '@/api/client';
import JetButton from '@/Components/Button.vue';
import JetGuestLayout from '@/Shared/Guest.vue';

// `session('status')` after the link was sent again.
const status = ref(null);

const form = useForm({});

const verificationLinkSent = computed(() => status.value === 'verification-link-sent');

const submit = () => {
  form.submit(() => sendEmailVerification(), {
    onSuccess: () => (status.value = 'verification-link-sent'),
  });
};

const logout = async () => {
  await logOut();
  visit(route('login'));
};
</script>

<template>
  <JetGuestLayout>
    <div class="mb-4 text-sm text-gray-600 dark:text-gray-400">
      {{
        $t(
          'Thanks for signing up! Before getting started, could you verify your email address by clicking on the link we just emailed to you? If you didn’t receive the email, we will gladly send you another.',
        )
      }}
    </div>

    <div v-if="verificationLinkSent" class="mb-4 text-sm font-medium text-green-600">
      {{ $t('A new verification link has been sent to the email address you provided during registration.') }}
    </div>

    <form @submit.prevent="submit">
      <div class="mt-4 flex items-center justify-between">
        <JetButton :class="{ 'opacity-25': form.processing }" :disabled="form.processing">
          {{ $t('Resend Verification Email') }}
        </JetButton>

        <button
          type="button"
          class="text-sm text-gray-600 underline hover:text-gray-900 dark:text-gray-400 dark:hover:text-gray-100"
          @click="logout">
          {{ $t('Log Out') }}
        </button>
      </div>
    </form>
  </JetGuestLayout>
</template>
