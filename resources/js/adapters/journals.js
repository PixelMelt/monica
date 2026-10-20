// The props of the journals pages, built from API objects as their ViewHelpers
// built them from models (`app/Domains/Vault/ManageJournals/Web/ViewHelpers`).

import { trans } from 'laravel-vue-i18n';
import {
  contactName,
  formatDate,
  formatDayNumber,
  formatLongMonthAndYear,
  formatShortDay,
  formatUserDate,
  sortByCollator,
} from '@/lib/format';
import { avatar, contactCard, formatFileSize, sliceDateRange } from '@/lib/helpers';
import { safeMarkdown } from '@/lib/markdown';
import { urls } from '@/api/client';
import { route } from '@/router';

// --- helpers ------------------------------------------------------------

/** `$model->label` of a seeded row: the label, else its translated key. */
const labelOf = (item) => item.label || (item.label_translation_key ? trans(item.label_translation_key) : null);

/** `$post->title`: the title, else "Undefined". */
const postTitle = (post) => post.title || trans('Undefined');

/** A photo as an `<img src>`: `thumbnail` for squares up to 300 px, else `original`. */
const fileUrl = (vaultId, fileId, variant) => urls.getVaultFileContent({ vault_id: vaultId, file_id: fileId, variant });

/** Carbon's `format('M')` (always English), upper-cased. */
const MONTHS = ['JAN', 'FEB', 'MAR', 'APR', 'MAY', 'JUN', 'JUL', 'AUG', 'SEP', 'OCT', 'NOV', 'DEC'];

/** A text field as Laravel stored it (`ConvertEmptyStringsToNull`). */
export const orNull = (value) => (value === '' ? null : value);

const journalUrl = (vaultId, journalId) => route('journal.show', { vault: vaultId, journal: journalId });

const postUrl = (vaultId, journalId, postId) =>
  route('post.show', { vault: vaultId, journal: journalId, post: postId });

const sliceUrl = (vaultId, journalId, sliceId) =>
  route('slices.show', { vault: vaultId, journal: journalId, slice: sliceId });

// --- JournalIndexViewHelper ---------------------------------------------

/** `JournalIndexViewHelper::dto`. */
export const journal = (j) => ({
  id: j.id,
  name: j.name,
  description: j.description,
  last_updated: j.last_post_written_at ? formatUserDate(j.last_post_written_at) : null,
  url: {
    show: journalUrl(j.vault_id, j.id),
  },
});

/** `JournalIndexViewHelper::data`. */
export const journalIndex = (journals, vaultId) => ({
  journals: sortByCollator(journals).map(journal),
  url: {
    create: route('journal.create', { vault: vaultId }),
  },
});

// --- JournalCreateViewHelper, JournalEditViewHelper ---------------------

/** `JournalCreateViewHelper::data`. */
export const journalCreate = (vaultId) => ({
  url: {
    back: route('journal.index', { vault: vaultId }),
  },
});

/** `JournalEditViewHelper::data`. */
export const journalEdit = (j) => ({
  id: j.id,
  name: j.name,
  description: j.description,
  url: {
    back: journalUrl(j.vault_id, j.id),
  },
});

// --- JournalShowViewHelper ----------------------------------------------

/**
 * `JournalShowViewHelper::postsInYear`: the months of the year, December
 * first, each coloured by its number of posts.
 */
export function postsInYear(overview) {
  const j = overview.journal;
  const year = overview.year;

  const months = overview.months.map((m) => {
    const posts = m.posts.map((post) => ({
      id: post.id,
      title: postTitle(post),
      excerpt: post.excerpt,
      written_at_day: formatShortDay(post.written_at).toUpperCase(),
      written_at_day_number: formatDayNumber(post.written_at),
      photo: post.first_photo
        ? {
            id: post.first_photo.id,
            url: {
              show: fileUrl(j.vault_id, post.first_photo.id, 'thumbnail'),
            },
          }
        : null,
      url: {
        show: postUrl(j.vault_id, j.id, post.id),
      },
    }));

    return {
      id: m.month,
      month: MONTHS[m.month - 1],
      month_human_format: formatLongMonthAndYear(
        `${String(year).padStart(4, '0')}-${String(m.month).padStart(2, '0')}-01`,
      ),
      posts,
      count: posts.length,
      color: 'bg-gray-50 dark:bg-gray-900',
    };
  });

  // the more posts in a month, the darker the color
  const maxPostsInMonth = Math.max(0, ...months.map((m) => m.count));
  for (const month of months) {
    if (month.count > 0) {
      const percent = Math.round((month.count / maxPostsInMonth) * 100);
      const round = percent - ((percent % 100) - 100);
      const dark = 1000 - round;
      month.color = `bg-green-${round} dark:bg-green-${dark}`;
    }
  }

  return months;
}

/** `JournalShowViewHelper::yearsOfContentInJournal`. */
export const yearsOfContentInJournal = (overview) =>
  overview.years.map((y) => ({
    year: y.year,
    posts: y.post_count,
    url: {
      show: route('journal.year', { vault: overview.journal.vault_id, journal: overview.journal.id, year: y.year }),
    },
  }));

/** `JournalShowViewHelper::tags`. */
export const tags = (overview) =>
  overview.tags.map((tag) => ({
    id: tag.id,
    name: tag.name,
    count: tag.post_count,
  }));

/** `JournalShowViewHelper::slices`. */
export const slices = (overview) =>
  overview.slices.map((slice) => ({
    id: slice.id,
    name: slice.name,
    date_range: sliceDateRange(slice.oldest_post_written_at, slice.newest_post_written_at),
    cover_image: slice.file_cover_image_id
      ? fileUrl(overview.journal.vault_id, slice.file_cover_image_id, 'original')
      : null,
    url: {
      show: sliceUrl(overview.journal.vault_id, overview.journal.id, slice.id),
    },
  }));

/** `JournalShowViewHelper::data`, from `getJournalOverview`. */
export function journalShow(overview) {
  const j = overview.journal;
  const params = { vault: j.vault_id, journal: j.id };
  return {
    id: j.id,
    name: j.name,
    description: j.description,
    months: postsInYear(overview),
    years: yearsOfContentInJournal(overview),
    tags: tags(overview),
    slices: slices(overview),
    url: {
      journal_metrics: route('journal_metrics.index', params),
      photo_index: route('journal.photo.index', params),
      edit: route('journal.edit', params),
      create: route('post.create', params),
      slice_index: route('slices.index', params),
    },
  };
}

// --- JournalPhotoIndexViewHelper ----------------------------------------

/** `JournalPhotoIndexViewHelper::dto`. */
export const journalPhoto = (file, j) => ({
  id: file.id,
  name: file.name,
  url: {
    post: postUrl(j.vault_id, j.id, file.post_id),
    display: fileUrl(j.vault_id, file.id, 'thumbnail'),
  },
});

/** `JournalPhotoIndexViewHelper::data`. */
export const journalPhotoIndex = (j, photos) => ({
  id: j.id,
  name: j.name,
  description: j.description,
  photos: photos.map((file) => journalPhoto(file, j)),
  url: {
    show: journalUrl(j.vault_id, j.id),
  },
});

// --- JournalMetricIndexViewHelper ---------------------------------------

/** `JournalMetricIndexViewHelper::dto`. */
export const journalMetric = (metric) => ({
  id: metric.id,
  label: metric.label,
});

/** `JournalMetricIndexViewHelper::data`. */
export const journalMetricIndex = (j, metrics) => ({
  journal: {
    id: j.id,
    name: j.name,
    url: {
      show: journalUrl(j.vault_id, j.id),
    },
  },
  journalMetrics: sortByCollator(metrics, 'label').map(journalMetric),
});

// --- PostCreateViewHelper -----------------------------------------------

/** `PostCreateViewHelper::data`, with the account's post templates (by position). */
export const postCreate = (j, templates) => ({
  journal: {
    name: j.name,
  },
  templates: templates.map((template) => ({
    id: template.id,
    label: labelOf(template),
    sections: template.sections.map((section) => ({
      id: section.id,
      label: labelOf(section),
    })),
    url: {
      create: route('post.store', { vault: j.vault_id, journal: j.id, template: template.id }),
    },
  })),
  url: {
    back: journalUrl(j.vault_id, j.id),
  },
});

// --- PostShowViewHelper -------------------------------------------------

/** `PostShowViewHelper::getSections`: the sections with content, rendered. */
const getSections = (post) =>
  post.sections
    .filter((section) => section.content !== null)
    .map((section) => ({
      id: section.id,
      label: section.label,
      content: safeMarkdown(section.content),
    }));

/** `PostShowViewHelper::getTags`. */
const getTags = (post) => sortByCollator(post.tags).map((tag) => ({ id: tag.id, name: tag.name }));

/** `PostShowViewHelper::getPhotos`. */
const getPhotos = (post, vaultId) =>
  post.photos.map((file) => ({
    id: file.id,
    name: file.name,
    url: {
      display: fileUrl(vaultId, file.id, 'thumbnail'),
    },
  }));

/** `PostShowViewHelper::getMood`: null when the user has no contact in the vault. */
const getMood = (post) =>
  post.mood_tracking_events === null
    ? null
    : post.mood_tracking_events.map((mood) => ({
        id: mood.id,
        note: mood.note,
        number_of_hours_slept: mood.number_of_hours_slept,
        mood_tracking_parameter: {
          id: mood.mood_tracking_parameter.id,
          label: labelOf(mood.mood_tracking_parameter),
        },
      }));

/** `PostShowViewHelper::getMetrics`. */
const getMetrics = (post) =>
  post.metrics.map((metric) => ({
    id: metric.journal_metric.id,
    label: metric.journal_metric.label,
    total: metric.total,
    post_metrics: metric.post_metrics.map((postMetric) => ({
      id: postMetric.id,
      value: postMetric.value,
      label: postMetric.label,
    })),
  }));

/** The previous/next post of `PostShowViewHelper::data`. */
const neighbour = (other, post, vaultId) =>
  other
    ? {
        id: other.id,
        title: postTitle(other),
        title_exists: postTitle(other) !== trans('Undefined'),
        url: {
          show: postUrl(vaultId, post.journal.id, other.id),
        },
      }
    : null;

/** `PostShowViewHelper::data`. */
export function postShow(post, vaultId) {
  return {
    id: post.id,
    title: postTitle(post),
    title_exists: postTitle(post) !== trans('Undefined'),
    written_at: formatUserDate(post.written_at),
    published: post.published,
    sections: getSections(post),
    tags: getTags(post),
    contacts: post.contacts.map(contactCard),
    photos: getPhotos(post, vaultId),
    moodTrackingEvents: getMood(post),
    journalMetrics: getMetrics(post),
    previousPost: neighbour(post.previous_post, post, vaultId),
    nextPost: neighbour(post.next_post, post, vaultId),
    journal: {
      name: post.journal.name,
      url: {
        show: journalUrl(vaultId, post.journal.id),
      },
    },
    sliceOfLife: post.slice_of_life
      ? {
          id: post.slice_of_life.id,
          name: post.slice_of_life.name,
          date_range: sliceDateRange(
            post.slice_of_life.oldest_post_written_at,
            post.slice_of_life.newest_post_written_at,
          ),
          url: {
            show: sliceUrl(vaultId, post.journal.id, post.slice_of_life.id),
          },
        }
      : null,
    url: {
      edit: route('post.edit', { vault: vaultId, journal: post.journal.id, post: post.id }),
      back: journalUrl(vaultId, post.journal.id),
    },
  };
}

// --- PostEditViewHelper, PostHelper -------------------------------------

/** `PostHelper::statistics`. */
export const postStatistics = (post) => ({
  word_count: post.word_count,
  time_to_read_in_minute: post.time_to_read_in_minute,
  view_count: post.view_count,
});

/** `PostEditViewHelper::dtoTag`. */
export const postTag = (tag, taken = false) => ({
  id: tag.id,
  name: tag.name,
  taken,
  url: {},
});

/** `PostEditViewHelper::dtoContact`. */
export const postContact = (contact) => ({
  id: contact.id,
  name: contactName(contact),
  avatar: avatar(contact),
  url: {
    show: route('contact.show', { vault: contact.vault_id, contact: contact.id }),
  },
});

/** `PostEditViewHelper::dtoSlice`. */
export const postSlice = (slice, vaultId) => ({
  id: slice.id,
  name: slice.name,
  url: {
    show: sliceUrl(vaultId, slice.journal_id, slice.id),
  },
});

/** `PostEditViewHelper::dtoPhoto`. */
export const postPhoto = (file, vaultId) => ({
  id: file.id,
  name: file.name,
  size: formatFileSize(file.size),
  mime_type: file.mime_type,
  url: {
    show: fileUrl(vaultId, file.id, 'thumbnail'),
  },
});

/** `PostEditViewHelper::dtoPostMetric`. */
export const postMetric = (pm) => ({
  id: pm.id,
  value: pm.value,
  label: pm.label,
});

/** `PostEditViewHelper::journalMetrics`. */
export const postJournalMetrics = (post) =>
  post.metrics.map((metric) => ({
    id: metric.journal_metric.id,
    label: metric.journal_metric.label,
    post_metrics: metric.post_metrics.map(postMetric),
  }));

/**
 * `PostEditViewHelper::data`, with the journal's slices of life and the
 * vault's tags.
 */
export function postEdit(post, journalSlices, vaultTags, vaultId) {
  const taken = new Set(post.tags.map((tag) => tag.id));
  return {
    id: post.id,
    title: postTitle(post),
    date: formatUserDate(post.written_at),
    editable_date: post.written_at,
    sections: post.sections.map((section) => ({
      id: section.id,
      label: section.label,
      content: section.content,
    })),
    contacts: post.contacts.map(postContact),
    photos: post.photos.map((file) => postPhoto(file, vaultId)),
    slice: post.slice_of_life ? postSlice(post.slice_of_life, vaultId) : null,
    slices: journalSlices.map((slice) => ({
      id: slice.id,
      name: slice.name,
    })),
    statistics: postStatistics(post),
    tags_in_post: post.tags.map((tag) => postTag(tag)),
    tags_in_vault: vaultTags.map((tag) => postTag(tag, taken.has(tag.id))),
    journal_metrics: postJournalMetrics(post),
    canUploadFile: post.can_upload_file,
    journal: {
      name: post.journal.name,
    },
    url: {
      show: postUrl(vaultId, post.journal.id, post.id),
      back: journalUrl(vaultId, post.journal.id),
    },
  };
}

// --- SliceOfLifeIndexViewHelper -----------------------------------------

/** `SliceOfLifeIndexViewHelper::dtoSlice`. */
export const sliceOfLifeIndexSlice = (slice, vaultId) => ({
  id: slice.id,
  name: slice.name,
  date_range: sliceDateRange(slice.oldest_post_written_at, slice.newest_post_written_at),
  url: {
    show: sliceUrl(vaultId, slice.journal_id, slice.id),
  },
});

/** `SliceOfLifeIndexViewHelper::data`. */
export const sliceOfLifeIndex = (j, journalSlices) => ({
  journal: {
    id: j.id,
    name: j.name,
    url: {
      show: journalUrl(j.vault_id, j.id),
    },
  },
  slicesOfLife: sortByCollator(journalSlices).map((slice) => sliceOfLifeIndexSlice(slice, j.vault_id)),
});

// --- SliceOfLifeShowViewHelper ------------------------------------------

/** `SliceOfLifeShowViewHelper::dtoSlice`. */
export const sliceOfLifeShowSlice = (slice, vaultId) => ({
  id: slice.id,
  name: slice.name,
  description: slice.description,
  date_range: sliceDateRange(slice.oldest_post_written_at, slice.newest_post_written_at),
  cover_image: slice.file_cover_image_id ? fileUrl(vaultId, slice.file_cover_image_id, 'original') : null,
  url: {
    show: sliceUrl(vaultId, slice.journal_id, slice.id),
    edit: route('slices.edit', { vault: vaultId, journal: slice.journal_id, slice: slice.id }),
  },
});

/** `SliceOfLifeShowViewHelper::data`. */
export const sliceOfLifeShow = (slice, vaultId) => ({
  journal: {
    id: slice.journal.id,
    name: slice.journal.name,
    url: {
      show: journalUrl(vaultId, slice.journal.id),
    },
  },
  slice: sliceOfLifeShowSlice(slice, vaultId),
  posts: slice.posts.map((post) => ({
    id: post.id,
    title: postTitle(post),
    excerpt: post.excerpt,
    written_at_day: formatShortDay(post.written_at).toUpperCase(),
    written_at_day_number: formatDayNumber(post.written_at),
    written_at: formatDate(post.written_at),
    url: {
      show: postUrl(vaultId, slice.journal_id, post.id),
    },
  })),
  contacts: slice.contacts.map(contactCard),
  canUploadFile: slice.can_upload_file,
  url: {
    slices_index: route('slices.index', { vault: vaultId, journal: slice.journal_id }),
  },
});

// --- SliceOfLifeEditViewHelper ------------------------------------------

/** `SliceOfLifeEditViewHelper::data`. */
export const sliceOfLifeEdit = (slice, vaultId) => ({
  slice: {
    id: slice.id,
    name: slice.name,
    description: slice.description,
    url: {
      show: sliceUrl(vaultId, slice.journal.id, slice.id),
    },
  },
  journal: {
    id: slice.journal.id,
    name: slice.journal.name,
    url: {
      show: journalUrl(vaultId, slice.journal.id),
    },
  },
  url: {
    back: sliceUrl(vaultId, slice.journal_id, slice.id),
    slices_index: route('slices.index', { vault: vaultId, journal: slice.journal_id }),
  },
});
