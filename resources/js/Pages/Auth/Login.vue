<script setup>
import { useForm } from '@/lib/form';
import { route, visit, intended } from '@/router';
import { logIn } from '@/api/client';
import { setPendingLogin } from '@/adapters/auth';
import JetCheckbox from '@/Components/Checkbox.vue';
import JetValidationErrors from '@/Components/ValidationErrors.vue';
import TextInput from '@/Shared/Form/TextInput.vue';
import PrimaryButton from '@/Shared/Form/PrimaryButton.vue';
import AuthenticationCardLogo from '@/Components/AuthenticationCardLogo.vue';

defineProps({
  wallpaperUrl: String,
});

const form = useForm({
  username: '',
  password: '',
  remember: true,
});

const submit = () => {
  form.submit((data) => logIn({}, { username: data.username, password: data.password, remember: data.remember }), {
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

    <h1 class="mt-4 text-center text-xl text-gray-800 dark:text-gray-200">
      <span class="me-2"> 👋 </span>
      {{ $t('Sign in to your account') }}
    </h1>

    <div
      :class="[
        'mt-6 mb-4 flex w-full flex-col overflow-hidden bg-white shadow-md/20 dark:shadow-md/80 dark:bg-gray-800 sm:max-w-4xl',
        'rounded-lg md:flex-row border border-gray-100 dark:border-gray-600',
      ]">
      <img :src="wallpaperUrl" class="w-full hidden sm:w-10/12 md:block object-cover" :alt="$t('Wallpaper')" />
      <div class="w-full">
        <div class="px-6 pb-6 pt-8">
          <JetValidationErrors class="mb-2" />

          <form @submit.prevent="submit" class="dark:text-gray-800">
            <div class="mb-3">
              <TextInput
                v-model="form.username"
                :label="$t('Username')"
                :type="'text'"
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
      </div>
    </div>
  </div>
</template>
