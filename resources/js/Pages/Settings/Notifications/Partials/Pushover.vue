<template>
  <div class="mb-8">
    <div class="mb-3 flex items-center justify-between">
      <span class="dark:text-gray-200">Pushover</span>

      <pretty-button
        v-if="!addChannelModalShown"
        :text="$t('Add a Pushover channel')"
        :icon="'plus'"
        @click="showAddChannelModal" />
    </div>

    <!-- add modal -->
    <form
      v-if="addChannelModalShown"
      class="item-list mb-6 rounded-lg border border-b border-gray-200 bg-gray-50 hover:bg-slate-50 dark:border-gray-700 dark:bg-gray-900 dark:bg-slate-900 dark:hover:bg-slate-800"
      @submit.prevent="store()">
      <div class="border-b border-gray-200 p-5 dark:border-gray-700">
        <errors :errors="form.errors" />

        <!-- label -->
        <text-input
          ref="label"
          v-model="form.label"
          :label="$t('Give this channel a name')"
          :type="'text'"
          :autofocus="true"
          :input-class="'block w-full'"
          :class="'mb-4'"
          :required="true"
          :autocomplete="false"
          :maxlength="255"
          @esc-key-pressed="addChannelModalShown = false" />

        <!-- user key -->
        <text-input
          v-model="form.user_key"
          :label="$t('Your Pushover user key')"
          :type="'text'"
          :input-class="'block w-full'"
          :class="'mb-4'"
          :required="true"
          :autocomplete="false"
          :maxlength="255"
          @esc-key-pressed="addChannelModalShown = false" />

        <!-- application token -->
        <text-input
          v-model="form.app_token"
          :label="$t('The token of your Pushover application')"
          :type="'password'"
          :input-class="'block w-full'"
          :class="'mb-4'"
          :required="true"
          :autocomplete="false"
          :maxlength="255"
          @esc-key-pressed="addChannelModalShown = false" />

        <!-- preferred time -->
        <p class="mb-2 block text-sm">
          {{ $t('At which time should we send the notification, when the reminder occurs?') }}
        </p>
        <div class="flex items-center text-sm font-medium text-gray-700 dark:text-gray-300">
          <span class="me-2">
            {{ $t('At') }}
          </span>

          <Dropdown v-model="form.hours" dropdown-class="me-1" :required="required" :data="hours" />

          <span class="me-2">:</span>

          <Dropdown v-model="form.minutes" dropdown-class="me-1" :required="required" :data="minutes" />
        </div>
      </div>

      <div class="border-b border-gray-200 p-5 dark:border-gray-700">
        <p class="flex">
          <span class="me-2">⚠️</span>
          {{
            $t(
              'Your user key is on your Pushover dashboard. Create an application there for Monica to get its token. We’ll check both with Pushover.',
            )
          }}
        </p>
      </div>

      <div class="flex justify-between p-5">
        <pretty-span :text="$t('Cancel')" :class="'me-3'" @click.prevent="addChannelModalShown = false" />
        <pretty-button :text="$t('Add')" :state="loadingState" :icon="'plus'" :class="'save'" />
      </div>
    </form>

    <!-- list of channels -->
    <ul
      v-if="localChannels.length > 0"
      class="mb-6 rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
      <li
        v-for="channel in localChannels"
        :key="channel.id"
        class="item-list flex items-center justify-between border-b border-gray-200 px-5 py-2 hover:bg-slate-50 dark:border-gray-700 dark:bg-slate-900 dark:hover:bg-slate-800">
        <div class="flex items-center">
          <a-tooltip v-if="channel.verified_at" placement="topLeft" :title="$t('Verified')" arrow-point-at-center>
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="me-2 inline h-4 w-4 text-green-600"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M9 12l2 2 4-4M7.835 4.697a3.42 3.42 0 001.946-.806 3.42 3.42 0 014.438 0 3.42 3.42 0 001.946.806 3.42 3.42 0 013.138 3.138 3.42 3.42 0 00.806 1.946 3.42 3.42 0 010 4.438 3.42 3.42 0 00-.806 1.946 3.42 3.42 0 01-3.138 3.138 3.42 3.42 0 00-1.946.806 3.42 3.42 0 01-4.438 0 3.42 3.42 0 00-1.946-.806 3.42 3.42 0 01-3.138-3.138 3.42 3.42 0 00-.806-1.946 3.42 3.42 0 010-4.438 3.42 3.42 0 00.806-1.946 3.42 3.42 0 013.138-3.138z" />
            </svg>
          </a-tooltip>

          <!-- label + user key -->
          <div>
            <span class="mb-0 block">{{ channel.label }}</span>
            <ul class="bulleted-list me-2 text-sm text-gray-500">
              <li class="me-1 inline">
                {{ channel.user_key }}
              </li>
              <li class="inline">
                {{ $t('Sent at :time', { time: channel.preferred_time }) }}
              </li>
            </ul>
          </div>
        </div>

        <ul class="text-sm">
          <!-- activate/deactivate -->
          <li
            v-if="channel.active"
            class="me-4 inline cursor-pointer text-blue-500 hover:underline"
            @click="toggle(channel)">
            {{ $t('Deactivate') }}
          </li>
          <li
            v-if="!channel.active"
            class="me-4 inline cursor-pointer text-blue-500 hover:underline"
            @click="toggle(channel)">
            {{ $t('Activate') }}
          </li>

          <!-- link to send a test notification, if not already sent -->
          <li
            v-if="testSentId !== channel.id"
            class="me-4 inline cursor-pointer text-blue-500 hover:underline"
            @click="sendTest(channel)">
            {{ $t('Send test') }}
          </li>

          <!-- text saying that the test has been sent -->
          <li v-if="testSentId === channel.id" class="me-4 inline">
            {{ $t('Test notification sent!') }}
          </li>

          <!-- view log -->
          <li class="me-4 inline cursor-pointer text-blue-500 hover:underline">
            <AppLink :href="channel.url.logs" class="text-blue-500 hover:underline">
              {{ $t('View log') }}
            </AppLink>
          </li>

          <!-- delete channel -->
          <li class="inline cursor-pointer text-red-500 hover:text-red-900" @click="destroy(channel)">
            {{ $t('Delete') }}
          </li>
        </ul>
      </li>
    </ul>

    <!-- blank state -->
    <div v-else class="mb-6 rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
      <p class="p-5 text-center">
        {{ $t('Add a Pushover channel to be notified when a reminder occurs.') }}
      </p>
    </div>
  </div>
</template>

<script>
import Link from '@/router/Link.vue';
import { Tooltip as ATooltip } from 'ant-design-vue';
import PrettyButton from '@/Shared/Form/PrettyButton.vue';
import PrettySpan from '@/Shared/Form/PrettySpan.vue';
import TextInput from '@/Shared/Form/TextInput.vue';
import Errors from '@/Shared/Form/Errors.vue';
import Dropdown from '@/Shared/Form/Dropdown.vue';
import {
  createNotificationChannel,
  updateNotificationChannel,
  deleteNotificationChannel,
  sendNotificationChannelTest,
} from '@/api/client';
import * as dto from '@/adapters/settings';

export default {
  components: {
    AppLink: Link,
    ATooltip,
    PrettyButton,
    PrettySpan,
    TextInput,
    Errors,
    Dropdown,
  },

  props: {
    data: {
      type: Object,
      default: null,
    },
  },

  data() {
    return {
      loadingState: '',
      localChannels: [],
      addChannelModalShown: false,
      testSentId: 0,
      form: {
        label: '',
        user_key: '',
        app_token: '',
        minutes: '',
        hours: '',
        errors: [],
      },
    };
  },

  computed: {
    hours() {
      let result = [];
      for (let i = 0; i < 24; i++) {
        let name = i.toString().padStart(2, '0');
        result.push({ id: i, name: name });
      }
      return result;
    },
    minutes() {
      let result = [];
      for (let i = 0; i < 60; i += 5) {
        let name = i.toString().padStart(2, '0');
        result.push({ id: i, name: name });
      }
      return result;
    },
  },

  mounted() {
    this.localChannels = this.data.channels;
    this.form.hours = '09';
    this.form.minutes = '00';
  },

  methods: {
    showAddChannelModal() {
      this.form.label = '';
      this.form.user_key = '';
      this.form.app_token = '';
      this.form.errors = [];
      this.addChannelModalShown = true;

      this.$nextTick().then(() => {
        this.$refs.label.focus();
      });
    },

    sendTest(channel) {
      sendNotificationChannelTest({ notification_channel_id: channel.id })
        .then(() => {
          this.flash(this.$t('The test notification has been sent'), 'success');
          this.testSentId = channel.id;
        })
        .catch((error) => {
          this.flash(error.message, 'error');
        });
    },

    toggle(channel) {
      updateNotificationChannel({ notification_channel_id: channel.id }, { active: !channel.active })
        .then((updated) => {
          this.flash(this.$t('The channel has been updated'), 'success');
          this.localChannels[this.localChannels.findIndex((x) => x.id === channel.id)] = dto.channel(updated);
        })
        .catch((error) => {
          this.form.errors = error.response.data;
        });
    },

    store() {
      this.loadingState = 'loading';

      // `$request->input('hours').':'.$request->input('minutes')`, as `HH:MM`.
      const time = [this.form.hours, this.form.minutes].map((v) => String(v).padStart(2, '0')).join(':');

      createNotificationChannel(
        {},
        { label: this.form.label, user_key: this.form.user_key, app_token: this.form.app_token, preferred_time: time },
      )
        .then((channel) => {
          this.flash(this.$t('The channel has been added'), 'success');
          this.localChannels.unshift(dto.channel(channel));
          this.loadingState = null;
          this.addChannelModalShown = false;
        })
        .catch((error) => {
          this.loadingState = null;
          this.form.errors = error.response.data;
        });
    },

    destroy(channel) {
      if (confirm(this.$t('Are you sure? This action cannot be undone.'))) {
        deleteNotificationChannel({ notification_channel_id: channel.id })
          .then(() => {
            this.flash(this.$t('The channel has been deleted'), 'success');
            var id = this.localChannels.findIndex((x) => x.id === channel.id);
            this.localChannels.splice(id, 1);
          })
          .catch((error) => {
            this.loadingState = null;
            this.form.errors = error.response.data;
          });
      }
    },
  },
};
</script>

<style lang="scss" scoped>
.item-list {
  &:hover:first-child {
    border-top-left-radius: 8px;
    border-top-right-radius: 8px;
  }

  &:last-child {
    border-bottom: 0;
  }

  &:hover:last-child {
    border-bottom-left-radius: 8px;
    border-bottom-right-radius: 8px;
  }
}
</style>
