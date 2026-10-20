<script setup>
import Link from '@/router/Link.vue';
import { useForm } from '@/lib/form';
import { route, visit, intended } from '@/router';
import { logIn } from '@/api/client';
import { setPendingLogin } from '@/adapters/auth';
import JetCheckbox from '@/Components/Checkbox.vue';
import JetValidationErrors from '@/Components/ValidationErrors.vue';
import TextInput from '@/Shared/Form/TextInput.vue';
import PrimaryButton from '@/Shared/Form/PrimaryButton.vue';
import AuthenticationCardLogo from '@/Components/AuthenticationCardLogo.vue';
import Beta from './Beta.vue';
import { TriangleAlert } from 'lucide-vue-next';

defineProps({
  isSignupEnabled: Boolean,
  canResetPassword: Boolean,
  status: String,
  wallpaperUrl: String,
  beta: Boolean,
  showDeletionNotice: Boolean,
});

const form = useForm({
  email: '',
  password: '',
  remember: true,
});

const submit = () => {
  form.submit((data) => logIn({}, { email: data.email, password: data.password, remember: data.remember }), {
    onSuccess: (result) => {
      setPendingLogin(result.two_factor);
      return visit(result.two_factor ? route('two-factor.login') : intended());
    },
    onFinish: () => form.reset('password'),
  });
};
</script>

<template>
  <div class="flex min-h-screen flex-col items-center bg-gray-100 pt-6 dark:bg-gray-900 sm:justify-center sm:pt-0">
    <div class="mb-2">
      <AuthenticationCardLogo />
    </div>

    <Beta :beta="beta" />

    <h1 class="mt-4 text-center text-xl text-gray-800 dark:text-gray-200">
      <span class="me-2"> 👋 </span>
      {{ $t('Sign in to your account') }}
    </h1>

    <div
      v-if="showDeletionNotice"
      role="alert"
      class="mt-6 flex w-full items-start gap-3 border border-amber-200 bg-amber-50 px-6 py-4 text-sm text-amber-900 dark:border-amber-800 dark:bg-amber-950 dark:text-amber-100 rounded-lg sm:max-w-4xl">
      <TriangleAlert class="h-5 w-5 shrink-0 text-amber-600 dark:text-amber-400" aria-hidden="true" />
      <p>
        {{
          $t(
            'As we are about to deploy a brand new version of Monica, this instance and all its data will be deleted at the end of December 2026',
          )
        }}
      </p>
    </div>

    <div
      :class="[
        'mt-6 mb-4 flex w-full flex-col overflow-hidden bg-white shadow-md/20 dark:shadow-md/80 dark:bg-gray-800 sm:max-w-4xl',
        'rounded-lg md:flex-row border border-gray-100 dark:border-gray-600',
      ]">
      <img :src="wallpaperUrl" class="w-full hidden sm:w-10/12 md:block object-cover" :alt="$t('Wallpaper')" />
      <div class="w-full">
        <div class="px-6 pb-6 pt-8">
          <JetValidationErrors class="mb-2" />

          <div v-if="status" class="mb-2 text-sm font-medium text-green-600 dark:text-green-400">
            {{ status }}
          </div>

          <form @submit.prevent="submit" class="dark:text-gray-800">
            <div class="mb-3">
              <TextInput
                v-model="form.email"
                :label="$t('Email')"
                :type="'email'"
                :input-class="'block w-full'"
                :required="true"
                :autofocus="true"
                :autocomplete="'username'"
                :maxlength="255" />
            </div>

            <div class="mb-3 relative">
              <label class="mb-2 text-sm dark:text-gray-100" :for="'password'">
                {{ $t('Password') }}
              </label>

              <Link
                v-if="canResetPassword"
                :href="route('password.request')"
                class="text-sm text-blue-500 hover:underline absolute right-0">
                {{ $t('Forgot your password?') }}
              </Link>

              <TextInput
                :id="'password'"
                v-model="form.password"
                :type="'password'"
                :input-class="'block w-full'"
                :required="true"
                :autocomplete="'current-password'"
                :maxlength="255" />
            </div>

            <div class="mb-3 flex justify-between">
              <div>
                <JetCheckbox id="remember" v-model:checked="form.remember" name="remember" />
                <label for="remember" class="ms-2 text-sm text-gray-600 dark:text-gray-400">
                  {{ $t('Remember me') }}
                </label>
              </div>

              <PrimaryButton :text="$t('Log in')" :class="'save ms-4'" />
            </div>
          </form>
        </div>

        <div v-if="isSignupEnabled" class="px-6 py-6 text-l dark:text-gray-50">
          {{ $t('New to Monica?') }}
          <Link :href="route('register')" class="text-blue-500 text-l hover:underline">
            {{ $t('Create an account') }}
          </Link>
        </div>
      </div>
    </div>
  </div>
</template>
