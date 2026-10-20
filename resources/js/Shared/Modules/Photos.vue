<template>
  <div class="mb-10">
    <!-- title + cta -->
    <div class="mb-3 items-center justify-between border-b border-gray-200 pb-2 dark:border-gray-700 sm:flex">
      <div class="mb-2 sm:mb-0 flex items-center gap-2">
        <FileImage class="h-4 w-4 text-gray-600" />

        <span class="font-semibold">
          {{ $t('Photos') }}
        </span>
      </div>
      <file-upload :upload="upload" :images-only="true" @success="onSuccess">
        <pretty-button :text="$t('Add a photo')" :icon="'plus'" :class="'w-full sm:w-fit'" />
      </file-upload>
    </div>

    <!-- photos -->
    <div v-if="localPhotos.length > 0">
      <div class="mb-4 grid grid-cols-3 gap-4">
        <div
          v-for="photo in localPhotos"
          :key="photo.id"
          class="rounded-md border border-gray-200 p-2 shadow-xs hover:bg-slate-50 hover:shadow-lg dark:border-gray-700 dark:bg-slate-900 dark:hover:bg-slate-800">
          <AppLink :href="photo.url.show">
            <img :src="photo.url.display" :alt="photo.name" />
          </AppLink>
        </div>
      </div>

      <!-- view all button -->
      <div class="text-center">
        <AppLink
          :href="data.url.index"
          class="rounded-xs border border-gray-200 px-3 py-1 text-sm text-blue-500 hover:border-gray-500 dark:border-gray-700">
          {{ $t('View all') }}
        </AppLink>
      </div>
    </div>

    <!-- blank state -->
    <div
      v-if="localPhotos.length === 0"
      class="mb-6 rounded-lg border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900">
      <img src="/img/contact_blank_photo.svg" :alt="$t('Photos')" class="mx-auto mt-4 h-16 w-16" />
      <p class="px-5 pb-5 pt-2 text-center">
        {{ $t('There are no photos yet.') }}
      </p>
    </div>
  </div>
</template>

<script>
import Link from '@/router/Link.vue';
import PrettyButton from '@/Shared/Form/PrettyButton.vue';
import FileUpload from '@/Components/FileUpload.vue';
import { FileImage } from 'lucide-vue-next';
import { deleteContactPhoto, uploadContactPhoto } from '@/api/client';
import * as dto from '@/adapters/modules-b';

export default {
  components: {
    AppLink: Link,
    PrettyButton,
    FileUpload,
    FileImage,
  },

  props: {
    data: {
      type: Object,
      default: null,
    },
  },

  data() {
    return {
      localPhotos: [],
      form: {
        searchTerm: null,
        errors: [],
      },
    };
  },

  created() {
    this.localPhotos = this.data.photos;
  },

  methods: {
    upload(body) {
      return uploadContactPhoto({ vault_id: this.$route.params.vault, contact_id: this.$route.params.contact }, body);
    },

    onSuccess(file) {
      this.localPhotos.unshift(dto.photo(file, this.$route.params.vault, this.$route.params.contact));
      this.flash(this.$t('The photo has been added'), 'success');
    },

    destroy(photo) {
      if (confirm(this.$t('Are you sure? This action cannot be undone.'))) {
        deleteContactPhoto({
          vault_id: this.$route.params.vault,
          contact_id: this.$route.params.contact,
          photo_id: photo.id,
        })
          .then(() => {
            this.flash(this.$t('The photo has been deleted'), 'success');
            var id = this.localPhotos.findIndex((x) => x.id === photo.id);
            this.localPhotos.splice(id, 1);
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
</style>
