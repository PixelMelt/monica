<script setup>
import { useForm } from '@/lib/form';
import { updateMe } from '@/api/client';
import JetButton from '@/Components/Button.vue';
import JetFormSection from '@/Components/Jetstream/FormSection.vue';
import JetInput from '@/Components/Input.vue';
import JetInputError from '@/Components/InputError.vue';
import JetLabel from '@/Components/Label.vue';
import JetActionMessage from '@/Components/Jetstream/ActionMessage.vue';

const props = defineProps({
  user: Object,
});

const form = useForm({
  first_name: props.user.first_name,
  last_name: props.user.last_name,
  email: props.user.email,
});

const updateProfileInformation = () => {
  form.submit((data) => updateMe({}, { first_name: data.first_name, last_name: data.last_name, email: data.email }));
};
</script>

<template>
  <JetFormSection @submitted="updateProfileInformation">
    <template #title>
      {{ $t('Profile Information') }}
    </template>

    <template #description>
      {{ $t("Update your account's profile information and email address.") }}
    </template>

    <template #form>
      <!-- First Name -->
      <div class="col-span-6 mb-4 sm:col-span-4">
        <JetLabel for="first_name" :value="$t('First name')" />
        <JetInput
          id="first_name"
          v-model="form.first_name"
          type="text"
          class="mt-1 block w-full"
          autocomplete="firstname" />
        <JetInputError :message="form.errors.first_name" class="mt-2" />
      </div>

      <!-- Last Name -->
      <div class="col-span-6 mb-4 sm:col-span-4">
        <JetLabel for="last_name" :value="$t('Last name')" />
        <JetInput
          id="last_name"
          v-model="form.last_name"
          type="text"
          class="mt-1 block w-full"
          autocomplete="lastname" />
        <JetInputError :message="form.errors.last_name" class="mt-2" />
      </div>

      <!-- Email -->
      <div class="col-span-6 mb-4 sm:col-span-4">
        <JetLabel for="email" :value="$t('Email')" />
        <JetInput id="email" v-model="form.email" type="email" class="mt-1 block w-full" autocomplete="email" />
        <JetInputError :message="form.errors.email" class="mt-2" />
      </div>
    </template>

    <template #actions>
      <JetActionMessage :on="form.recentlySuccessful" class="me-3">
        {{ $t('Saved.') }}
      </JetActionMessage>

      <JetButton :class="{ 'opacity-25': form.processing }" :disabled="form.processing">
        {{ $t('Save') }}
      </JetButton>
    </template>
  </JetFormSection>
</template>
