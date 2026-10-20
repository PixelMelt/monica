<template>
  <layout :layout-data="layoutData">
    <!-- breadcrumb -->
    <nav class="bg-white dark:bg-gray-900 sm:border-b border-gray-200 dark:border-gray-700">
      <div class="max-w-8xl mx-auto hidden px-4 py-2 sm:px-6 md:block">
        <div class="flex items-baseline justify-between space-x-6">
          <ul class="text-sm">
            <li class="me-2 inline text-gray-600 dark:text-gray-400">
              {{ $t('You are here:') }}
            </li>
            <li class="me-2 inline">
              <AppLink :href="data.url.settings" class="text-blue-500 hover:underline">
                {{ $t('Settings') }}
              </AppLink>
            </li>
            <li class="relative me-2 inline">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                class="icon-breadcrumb relative inline h-3 w-3"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
              </svg>
            </li>
            <li class="inline">
              {{ $t('Accounts') }}
            </li>
          </ul>
        </div>
      </div>
    </nav>

    <main class="relative sm:mt-24">
      <div class="mx-auto max-w-3xl px-2 py-2 sm:px-6 sm:py-6 lg:px-8">
        <!-- title + cta -->
        <div class="mb-6 flex items-center justify-between">
          <h3>
            <span class="me-1"> 🏛️ </span>
            {{ $t('All the accounts of this Monica') }}
          </h3>
          <pretty-button v-if="!addFormShown" :text="$t('Add an account')" :icon="'plus'" @click="showAddForm" />
        </div>

        <!-- add form -->
        <form
          v-if="addFormShown"
          class="mb-6 rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900"
          @submit.prevent="store()">
          <div class="border-b border-gray-200 p-5 dark:border-gray-700">
            <p class="mb-4 text-sm text-gray-600 dark:text-gray-400">
              {{
                $t('Each account is separate, with its own users, vaults and settings. Its first user administers it.')
              }}
            </p>

            <errors :errors="form.errors" />

            <text-input
              ref="firstName"
              v-model="form.first_name"
              :label="$t('First name')"
              :type="'text'"
              :input-class="'block w-full'"
              :class="'mb-4'"
              :required="true"
              :autocomplete="false"
              :maxlength="255"
              @esc-key-pressed="addFormShown = false" />

            <text-input
              v-model="form.last_name"
              :label="$t('Last name')"
              :type="'text'"
              :input-class="'block w-full'"
              :class="'mb-4'"
              :required="true"
              :autocomplete="false"
              :maxlength="255"
              @esc-key-pressed="addFormShown = false" />

            <text-input
              v-model="form.username"
              :label="$t('Username')"
              :type="'text'"
              :input-class="'block w-full'"
              :class="'mb-4'"
              :required="true"
              :autocomplete="false"
              :maxlength="255"
              @esc-key-pressed="addFormShown = false" />

            <text-input
              v-model="form.password"
              :label="$t('Password')"
              :type="'password'"
              :input-class="'block w-full'"
              :class="'mb-4'"
              :required="true"
              :autocomplete="'new-password'"
              :maxlength="255"
              @esc-key-pressed="addFormShown = false" />

            <text-input
              v-model="form.password_confirmation"
              :label="$t('Confirm Password')"
              :type="'password'"
              :input-class="'block w-full'"
              :class="'mb-2'"
              :required="true"
              :autocomplete="'new-password'"
              :maxlength="255"
              @esc-key-pressed="addFormShown = false" />
            <p class="text-xs text-gray-600">{{ $t('They can change their password in their profile.') }}</p>
          </div>

          <div class="flex justify-between p-5">
            <pretty-span :text="$t('Cancel')" :class="'me-3'" @click.prevent="addFormShown = false" />
            <pretty-button :text="$t('Add')" :state="loadingState" :icon="'plus'" :class="'save'" />
          </div>
        </form>

        <!-- list of accounts -->
        <ul class="account-list mb-6 rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
          <li
            v-for="account in localAccounts"
            :key="account.id"
            class="flex items-start justify-between border-b border-gray-200 px-5 py-3 hover:bg-slate-50 dark:border-gray-700 dark:bg-slate-900 dark:hover:bg-slate-800">
            <div>
              <!-- users -->
              <div v-for="user in account.users" :key="user.id" class="mb-1">
                <span>{{ user.name }}</span>
                <span class="ms-2 text-sm text-gray-400">{{ user.username }}</span>
                <span
                  v-if="user.is_account_administrator"
                  class="ms-2 rounded-xs bg-neutral-200 px-2 py-1 text-xs text-neutral-500"
                  >{{ $t('Administrator') }}</span
                >
                <span
                  v-if="user.is_instance_administrator"
                  class="ms-2 rounded-xs bg-sky-100 px-2 py-1 text-xs text-sky-700"
                  >{{ $t('Instance administrator') }}</span
                >
              </div>

              <!-- what it holds -->
              <ul class="bulleted-list mt-1 text-sm text-gray-500">
                <li v-if="account.is_own" class="me-1 inline font-semibold">{{ $t('Your account') }}</li>
                <li class="me-1 inline">{{ $t('Vaults: :count', { count: account.vault_count }) }}</li>
                <li class="me-1 inline">{{ $t('Contacts: :count', { count: account.contact_count }) }}</li>
                <li class="me-1 inline">{{ $t('Files: :size', { size: account.file_size }) }}</li>
                <li v-if="account.created_at" class="inline">
                  {{ $t('Created on :date', { date: account.created_at }) }}
                </li>
              </ul>
            </div>

            <!-- actions -->
            <ul v-if="!account.is_own" class="text-sm">
              <li class="inline cursor-pointer text-red-500 hover:text-red-900">
                <confirms-password @confirmed="destroy(account)">
                  {{ $t('Delete') }}
                </confirms-password>
              </li>
            </ul>
          </li>
        </ul>
      </div>
    </main>
  </layout>
</template>

<script>
import Link from '@/router/Link.vue';
import Layout from '@/Layouts/Layout.vue';
import PrettyButton from '@/Shared/Form/PrettyButton.vue';
import PrettySpan from '@/Shared/Form/PrettySpan.vue';
import TextInput from '@/Shared/Form/TextInput.vue';
import Errors from '@/Shared/Form/Errors.vue';
import ConfirmsPassword from '@/Components/Jetstream/ConfirmsPassword.vue';
import { createAdminAccount, deleteAdminAccount } from '@/api/client';
import * as dto from '@/adapters/admin';

export default {
  components: {
    AppLink: Link,
    Layout,
    PrettyButton,
    PrettySpan,
    TextInput,
    Errors,
    ConfirmsPassword,
  },

  props: {
    layoutData: {
      type: Object,
      default: null,
    },
    data: {
      type: Object,
      default: null,
    },
  },

  data() {
    return {
      loadingState: '',
      addFormShown: false,
      localAccounts: [],
      form: {
        first_name: '',
        last_name: '',
        username: '',
        password: '',
        password_confirmation: '',
        errors: [],
      },
    };
  },

  mounted() {
    this.localAccounts = this.data.accounts;
  },

  methods: {
    showAddForm() {
      this.form.first_name = '';
      this.form.last_name = '';
      this.form.username = '';
      this.form.password = '';
      this.form.password_confirmation = '';
      this.form.errors = [];
      this.addFormShown = true;

      this.$nextTick().then(() => {
        this.$refs.firstName.focus();
      });
    },

    store() {
      this.loadingState = 'loading';

      createAdminAccount(
        {},
        {
          first_name: this.form.first_name,
          last_name: this.form.last_name,
          username: this.form.username,
          password: this.form.password,
          password_confirmation: this.form.password_confirmation,
        },
      )
        .then((account) => {
          this.flash(this.$t('The account has been created'), 'success');
          this.localAccounts.push(dto.account(account));
          this.loadingState = null;
          this.addFormShown = false;
        })
        .catch((error) => {
          this.loadingState = null;
          this.form.errors = error.response.data;
        });
    },

    destroy(account) {
      if (confirm(this.$t('This deletes the account with its users, vaults, contacts and files.'))) {
        deleteAdminAccount({ account_id: account.id }, {}).then(() => {
          this.flash(this.$t('The account has been deleted'), 'success');
          this.localAccounts.splice(
            this.localAccounts.findIndex((x) => x.id === account.id),
            1,
          );
        });
      }
    },
  },
};
</script>

<style lang="scss" scoped>
.account-list {
  li:hover:first-child {
    border-top-left-radius: 8px;
    border-top-right-radius: 8px;
  }

  li:last-child {
    border-bottom: 0;
  }

  li:hover:last-child {
    border-bottom-left-radius: 8px;
    border-bottom-right-radius: 8px;
  }
}
</style>
