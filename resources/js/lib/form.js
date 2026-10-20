// Form state for pages that used Inertia's `useForm`: the fields plus
// `errors` (first message per field), `processing`, `recentlySuccessful`,
// `reset()`, `clearErrors()` and `data()`. `submit(send)` runs a call of
// the API client with the form data and fills `errors` from a 422.

import { reactive } from 'vue';
import { cloneDeep } from 'lodash';
import { ApiError } from '@/api/runtime';

/**
 * The errors of the last form sent on this page, as Inertia shared them in
 * `$page.props.errors` (shown by `Components/ValidationErrors.vue`).
 */
export const pageErrors = reactive({ errors: {} });

const STATE = ['errors', 'hasErrors', 'processing', 'wasSuccessful', 'recentlySuccessful'];

export function useForm(initial = {}) {
  let defaults = cloneDeep(initial);
  let transform = (data) => data;
  let timer = null;

  const form = reactive({
    ...cloneDeep(initial),
    errors: {},
    hasErrors: false,
    processing: false,
    wasSuccessful: false,
    recentlySuccessful: false,

    data() {
      return Object.fromEntries(Object.keys(defaults).map((key) => [key, form[key]]));
    },

    transform(fn) {
      transform = fn;
      return form;
    },

    defaults(values) {
      defaults = values === undefined ? cloneDeep(form.data()) : { ...defaults, ...cloneDeep(values) };
      return form;
    },

    reset(...fields) {
      const keys = fields.length ? fields : Object.keys(defaults);
      for (const key of keys) form[key] = cloneDeep(defaults[key]);
      return form;
    },

    setError(field, message) {
      Object.assign(form.errors, typeof field === 'string' ? { [field]: message } : field);
      form.hasErrors = Object.keys(form.errors).length > 0;
      return form;
    },

    clearErrors(...fields) {
      if (fields.length === 0) form.errors = {};
      for (const field of fields) delete form.errors[field];
      form.hasErrors = Object.keys(form.errors).length > 0;
      return form;
    },

    /**
     * Sends the form with `send(data)` (a call of the API client) and
     * resolves to its result, or to undefined when it failed.
     */
    async submit(send, { onSuccess, onError, onFinish } = {}) {
      form.processing = true;
      form.wasSuccessful = false;
      form.recentlySuccessful = false;
      clearTimeout(timer);
      try {
        const result = await send(transform(form.data()));
        form.clearErrors();
        pageErrors.errors = {};
        form.wasSuccessful = true;
        form.recentlySuccessful = true;
        timer = setTimeout(() => (form.recentlySuccessful = false), 2000);
        await onSuccess?.(result);
        return result;
      } catch (error) {
        if (!(error instanceof ApiError)) throw error;
        form.errors = Object.fromEntries(Object.entries(error.errors).map(([k, v]) => [k, v[0]]));
        if (Object.keys(form.errors).length === 0) form.errors = { message: error.message };
        form.hasErrors = true;
        pageErrors.errors = { ...form.errors };
        onError?.(form.errors, error);
        return undefined;
      } finally {
        form.processing = false;
        onFinish?.();
      }
    },
  });

  for (const key of STATE) {
    if (key in initial) throw new Error(`useForm: "${key}" is reserved`);
  }
  return form;
}
