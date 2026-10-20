// Page loaders: journals. Each returns the props of its page (see router/index.js).
import { layoutData } from '@/loaders/common';
import { route } from '@/router';
import {
  listJournals,
  getJournal,
  getJournalOverview,
  listJournalPhotos,
  listPostTemplates,
  createJournalPost,
  getJournalPost,
  recordJournalPostView,
  listJournalSlices,
  listVaultTags,
  getJournalSlice,
  listJournalMetrics,
} from '@/api/client';
import { ApiError } from '@/api/runtime';
import { vaultPermission } from '@/lib/session';
import * as dto from '@/adapters/journals';

/** `Gate::authorize('vault-editor', $vault)`: a 403 page for viewers. */
function authorizeEditor(vaultId) {
  const permission = vaultPermission(vaultId);
  if (permission === null || permission > 200) throw new ApiError(403, { title: 'Forbidden', code: 'forbidden' });
}

const journalParams = (to) => ({ vault_id: to.params.vault, journal_id: to.params.journal });

/** `JournalController@index`: `JournalIndexViewHelper::data`. */
export async function index(to) {
  const vaultId = to.params.vault;
  const journals = await listJournals({ vault_id: vaultId });
  return {
    layoutData: layoutData(vaultId),
    data: dto.journalIndex(journals.data, vaultId),
  };
}

/** `JournalController@create`: `JournalCreateViewHelper::data`. */
export async function create(to) {
  const vaultId = to.params.vault;
  authorizeEditor(vaultId);
  return {
    layoutData: layoutData(vaultId),
    data: dto.journalCreate(vaultId),
  };
}

/**
 * `JournalController@show` (this year) and `JournalController@year`:
 * `JournalShowViewHelper::data`.
 */
export async function show(to) {
  const vaultId = to.params.vault;
  const overview = await getJournalOverview({ ...journalParams(to), year: to.params.year });
  return {
    layoutData: layoutData(vaultId),
    data: dto.journalShow(overview),
  };
}

/** `JournalPhotoController@index`: `JournalPhotoIndexViewHelper::data`. */
export async function photos(to) {
  const vaultId = to.params.vault;
  const [journal, list] = await Promise.all([getJournal(journalParams(to)), listJournalPhotos(journalParams(to))]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.journalPhotoIndex(journal, list.data),
  };
}

/** `JournalController@edit`: `JournalEditViewHelper::data`. */
export async function edit(to) {
  const vaultId = to.params.vault;
  const journal = await getJournal(journalParams(to));
  return {
    layoutData: layoutData(vaultId),
    data: dto.journalEdit(journal),
  };
}

/** `PostController@create`: `PostCreateViewHelper::data`. */
export async function chooseTemplate(to) {
  const vaultId = to.params.vault;
  const [journal, templates] = await Promise.all([getJournal(journalParams(to)), listPostTemplates()]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.postCreate(journal, templates.data),
  };
}

/**
 * `PostController@store`: creates a draft from the template on visiting
 * the page, then goes to its edit page.
 */
export async function createPost(to) {
  const post = await createJournalPost(journalParams(to), { post_template_id: Number(to.params.template) });
  return {
    redirect: route('post.edit', { vault: to.params.vault, journal: to.params.journal, post: post.id }),
  };
}

/** `PostController@show`: counts the read, `PostShowViewHelper::data`. */
export async function post(to) {
  const vaultId = to.params.vault;
  const params = { ...journalParams(to), post_id: to.params.post };
  const [result] = await Promise.all([getJournalPost(params), recordJournalPostView(params)]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.postShow(result, vaultId),
  };
}

/** `PostController@edit`: `PostEditViewHelper::data`. */
export async function editPost(to) {
  const vaultId = to.params.vault;
  const [result, journalSlices, vaultTags] = await Promise.all([
    getJournalPost({ ...journalParams(to), post_id: to.params.post }),
    listJournalSlices(journalParams(to)),
    listVaultTags({ vault_id: vaultId }),
  ]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.postEdit(result, journalSlices.data, vaultTags.data, vaultId),
  };
}

/** `SliceOfLifeController@index`: `SliceOfLifeIndexViewHelper::data`. */
export async function slices(to) {
  const vaultId = to.params.vault;
  const [journal, list] = await Promise.all([getJournal(journalParams(to)), listJournalSlices(journalParams(to))]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.sliceOfLifeIndex(journal, list.data),
  };
}

/** `SliceOfLifeController@show`: `SliceOfLifeShowViewHelper::data`. */
export async function slice(to) {
  const vaultId = to.params.vault;
  const result = await getJournalSlice({ ...journalParams(to), slice_id: to.params.slice });
  return {
    layoutData: layoutData(vaultId),
    data: dto.sliceOfLifeShow(result, vaultId),
  };
}

/** `SliceOfLifeController@edit`: `SliceOfLifeEditViewHelper::data`. */
export async function editSlice(to) {
  const vaultId = to.params.vault;
  const result = await getJournalSlice({ ...journalParams(to), slice_id: to.params.slice });
  return {
    layoutData: layoutData(vaultId),
    data: dto.sliceOfLifeEdit(result, vaultId),
  };
}

/** `JournalMetricController@index`: `JournalMetricIndexViewHelper::data`. */
export async function metrics(to) {
  const vaultId = to.params.vault;
  const [journal, list] = await Promise.all([getJournal(journalParams(to)), listJournalMetrics(journalParams(to))]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.journalMetricIndex(journal, list.data),
  };
}
