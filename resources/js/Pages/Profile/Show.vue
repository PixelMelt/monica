<script setup>
import Layout from '@/Layouts/Layout.vue';
import LogoutOtherBrowserSessionsForm from '@/Pages/Profile/Partials/LogoutOtherBrowserSessionsForm.vue';
import TwoFactorAuthenticationForm from '@/Pages/Profile/Partials/TwoFactorAuthenticationForm.vue';
import UpdatePasswordForm from '@/Pages/Profile/Partials/UpdatePasswordForm.vue';
import UpdateProfileInformationForm from '@/Pages/Profile/Partials/UpdateProfileInformationForm.vue';
import Breadcrumb from '@/Components/Breadcrumb.vue';

defineProps({
  confirmsTwoFactorAuthentication: Boolean,
  sessions: Array,
  locales: Array,
  layoutData: Object,
  jetstream: Object,
  user: Object,
});
</script>

<template>
  <Layout :layout-data="layoutData">
    <template #header>
      <Breadcrumb
        :items="[
          {
            name: $t('Settings'),
            url: route('settings.index'),
          },
          {
            name: $t('Account and security'),
          },
        ]" />
    </template>

    <div class="relative">
      <div class="mx-auto max-w-3xl px-2 py-2 sm:px-6 sm:py-6 lg:px-8">
        <div class="mb-16" v-if="jetstream.canUpdateProfileInformation">
          <UpdateProfileInformationForm :user="user" />
        </div>

        <div class="mb-16" v-if="jetstream.canUpdatePassword">
          <UpdatePasswordForm class="mt-10 sm:mt-0" />
        </div>

        <div class="mb-16" v-if="jetstream.canManageTwoFactorAuthentication">
          <TwoFactorAuthenticationForm :requires-confirmation="confirmsTwoFactorAuthentication" class="mt-10 sm:mt-0" />
        </div>

        <div class="mb-16">
          <LogoutOtherBrowserSessionsForm :sessions="sessions" class="mt-10 sm:mt-0" />
        </div>
      </div>
    </div>
  </Layout>
</template>
