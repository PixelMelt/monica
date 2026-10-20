<template>
  <div class="mb-10">
    <!-- title + cta -->
    <div class="mb-3 items-center justify-between border-b border-gray-200 pb-2 dark:border-gray-700 sm:flex">
      <div class="mb-2 sm:mb-0 flex items-center gap-2">
        <CloudUpload class="h-4 w-4 text-gray-600" />

        <span class="font-semibold">
          {{ $t('Documents') }}
        </span>
      </div>
      <file-upload v-if="data.canUploadFile" :upload="upload" @success="onSuccess">
        <pretty-button :text="$t('Add a document')" :icon="'plus'" :class="'w-full sm:w-fit'" />
      </file-upload>
    </div>

    <!-- not enough space in storage -->
    <div
      v-if="!data.canUploadFile"
      class="mb-6 rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
      <p class="bg-gray-100 p-3 text-center">
        <span class="me-1">⚠️</span>
        {{ $t('You don’t have enough space left in your account. Please upgrade.') }}
      </p>
    </div>

    <!-- documents -->
    <div v-if="localDocuments.length > 0">
      <ul class="mb-4 rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
        <li
          v-for="document in localDocuments"
          :key="document.id"
          class="item-list border-b border-gray-200 hover:bg-slate-50 dark:border-gray-700 dark:bg-slate-900 dark:hover:bg-slate-800">
          <!-- document -->
          <div class="flex items-center justify-between px-3 py-2">
            <span class="flex items-center">
              <span class="max-w-sm truncate">{{ document.name }}</span>

              <span class="ms-2 rounded-xs border bg-blue-50 px-1 py-0 font-mono text-xs text-blue-500">
                {{ document.size }}
              </span>
            </span>

            <!-- actions -->
            <ul class="text-sm">
              <li class="me-4 inline">
                <a :href="document.url.download" class="text-blue-500 hover:underline">{{ $t('Download') }}</a>
              </li>
              <li class="inline cursor-pointer text-red-500 hover:text-red-900" @click="destroy(document)">
                {{ $t('Delete') }}
              </li>
            </ul>
          </div>
        </li>
      </ul>
    </div>

    <!-- blank state -->
    <div
      v-if="localDocuments.length === 0"
      class="mb-6 rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
      <img src="/img/contact_blank_document.svg" :alt="$t('Documents')" class="mx-auto mt-4 h-16 w-16" />
      <p class="px-5 pb-5 pt-2 text-center">
        {{ $t('There are no documents yet.') }}
      </p>
    </div>
  </div>
</template>

<script>
import PrettyButton from '@/Shared/Form/PrettyButton.vue';
import FileUpload from '@/Components/FileUpload.vue';
import { CloudUpload } from 'lucide-vue-next';
import { deleteContactDocument, uploadContactDocument } from '@/api/client';
import * as dto from '@/adapters/modules-b';

export default {
  components: {
    PrettyButton,
    FileUpload,
    CloudUpload,
  },

  props: {
    data: {
      type: Object,
      default: null,
    },
  },

  data() {
    return {
      localDocuments: [],
      form: {
        errors: [],
      },
    };
  },

  created() {
    this.localDocuments = this.data.documents;
  },

  methods: {
    upload(body) {
      return uploadContactDocument(
        { vault_id: this.$route.params.vault, contact_id: this.$route.params.contact },
        body,
      );
    },

    onSuccess(file) {
      this.localDocuments.unshift(dto.document(file, this.$route.params.vault));
      this.flash(this.$t('The document has been added'), 'success');
    },

    destroy(document) {
      if (confirm(this.$t('Are you sure? This will delete the document permanently.'))) {
        deleteContactDocument({
          vault_id: this.$route.params.vault,
          contact_id: this.$route.params.contact,
          document_id: document.id,
        })
          .then(() => {
            this.flash(this.$t('Are you sure? This will delete the document permanently.'), 'success');
            var id = this.localDocuments.findIndex((x) => x.id === document.id);
            this.localDocuments.splice(id, 1);
          })
          .catch((error) => {
            this.form.errors = error.response.data;
          });
      }
    },
  },
};
</script>

<style lang="scss" scoped>
.icon-sidebar {
  color: #737e8d;
  top: -2px;
}

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
