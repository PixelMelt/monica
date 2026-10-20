// Page loaders: vaults. Each returns the props of its page (see router/index.js).
import { reactive } from 'vue';
import { layoutData } from '@/loaders/common';
import { route } from '@/router';
import {
  getVault,
  getVaultAddressReport,
  getVaultCalendarMonth,
  getVaultCityAddressReport,
  getVaultCountryAddressReport,
  getVaultImportantDateReport,
  getVaultMoodTrackingReport,
  listTemplates,
  listVaultCompanies,
  listVaultContactImportantDateTypes,
  listVaultFiles,
  listVaultLabels,
  listVaultLifeEventCategories,
  listVaultMemberCandidates,
  listVaultMembers,
  listVaultMoodTrackingParameters,
  listVaultQuickFactTemplates,
  listVaults,
  listVaultTags,
} from '@/api/client';
import { sortByCollator, today } from '@/lib/format';
import { paginator } from '@/adapters/paginator';
import * as dto from '@/adapters/vaults';

/** `VaultController@index`: `VaultIndexViewHelper::data`. */
export async function index() {
  const list = await listVaults();
  return {
    layoutData: layoutData(),
    data: {
      vaults: list.data.map(dto.vault),
      url: {
        vault: {
          create: route('vault.create'),
        },
      },
    },
  };
}

/** `VaultController@create`: `VaultCreateViewHelper::data`. */
export async function create() {
  return {
    layoutData: layoutData(),
    data: {
      url: {
        back: route('vault.index'),
      },
    },
  };
}

/** `VaultController@edit`: `VaultEditViewHelper::data`. */
export async function edit(to) {
  const vault = await getVault({ vault_id: to.params.vault });
  return {
    layoutData: layoutData(),
    data: dto.vaultEdit(vault),
  };
}

/**
 * `VaultCalendarController@index` (the current month) and `@month`:
 * `VaultCalendarIndexViewHelper::data`.
 */
export async function calendar(to) {
  const vaultId = to.params.vault;
  const now = today();
  const year = to.params.year ? Number(to.params.year) : now.year();
  const month = to.params.month ? Number(to.params.month) : now.month() + 1;
  const grid = await getVaultCalendarMonth({ vault_id: vaultId, year, month });
  return {
    layoutData: layoutData(vaultId),
    data: dto.calendarMonth(grid, vaultId),
  };
}

/** `ReportIndexController@index`: `ReportIndexViewHelper::data`. */
export async function reports(to) {
  const vaultId = to.params.vault;
  await getVault({ vault_id: vaultId });
  return {
    layoutData: layoutData(vaultId),
    data: dto.reportIndex(vaultId),
  };
}

/** `ReportAddressesController@index`: `ReportAddressIndexViewHelper::data`. */
export async function addressReport(to) {
  const vaultId = to.params.vault;
  const report = await getVaultAddressReport({ vault_id: vaultId });
  return {
    layoutData: layoutData(vaultId),
    data: dto.addressReport(report, vaultId),
  };
}

/** `ReportAddressesCitiesController@show`: `ReportCitiesShowViewHelper::data`. */
export async function cityReport(to) {
  const vaultId = to.params.vault;
  const city = to.params.city;
  const report = await getVaultCityAddressReport({ vault_id: vaultId, city });
  return {
    layoutData: layoutData(vaultId),
    data: dto.cityReport(report, city, report.wikipedia, vaultId),
  };
}

/** `ReportAddressesCountriesController@show`: `ReportCountriesShowViewHelper::data`. */
export async function countryReport(to) {
  const vaultId = to.params.vault;
  const country = to.params.country;
  const report = await getVaultCountryAddressReport({ vault_id: vaultId, country });
  return {
    layoutData: layoutData(vaultId),
    data: dto.countryReport(report, country, report.wikipedia, vaultId),
  };
}

/** `ReportMoodTrackingEventController@index`: the current year. */
export async function moodReport(to) {
  const vaultId = to.params.vault;
  const report = await getVaultMoodTrackingReport({ vault_id: vaultId });
  return {
    layoutData: layoutData(vaultId),
    data: dto.moodTrackingReport(report, vaultId),
  };
}

/** `ReportImportantDateSummaryController@index`. */
export async function importantDateReport(to) {
  const vaultId = to.params.vault;
  const report = await getVaultImportantDateReport({ vault_id: vaultId });
  return {
    layoutData: layoutData(vaultId),
    data: dto.importantDateReport(report, vaultId),
  };
}

const FILE_TABS = { photo: 'photos', document: 'documents', avatar: 'avatars' };

/**
 * `VaultFileController@index`, `@photos`, `@documents`, `@avatars`
 * (`meta.fileType`): `VaultFileIndexViewHelper::data`, 25 per page. The
 * props are reactive, as Inertia's were: the page removes deleted files
 * from `data.files` in place.
 */
export async function files(to) {
  const vaultId = to.params.vault;
  const type = to.meta.fileType ?? null;
  const list = await listVaultFiles({ vault_id: vaultId, type, page: to.query.page ?? 1, per_page: 25 });
  return reactive({
    layoutData: layoutData(vaultId),
    data: {
      files: list.data.map((f) => dto.file(f, vaultId)),
      statistics: dto.fileStatistics(list.counts, vaultId),
    },
    paginator: paginator(list.meta, list.data.length, to),
    tab: FILE_TABS[type] ?? 'index',
  });
}

/** `VaultCompanyController@index`: `CompanyIndexViewHelper::data`. */
export async function companies(to) {
  const vaultId = to.params.vault;
  const list = await listVaultCompanies({ vault_id: vaultId });
  return {
    layoutData: layoutData(vaultId),
    data: {
      companies: list.data.map(dto.company),
    },
  };
}

/** `VaultSettingsController@index`: `VaultSettingsIndexViewHelper::data`. */
export async function settings(to) {
  const vaultId = to.params.vault;
  const params = { vault_id: vaultId };
  const [
    vault,
    templates,
    members,
    candidates,
    labels,
    tags,
    dateTypes,
    moodTrackingParameters,
    lifeEventCategories,
    quickFactTemplates,
  ] = await Promise.all([
    getVault(params),
    listTemplates(),
    listVaultMembers(params),
    listVaultMemberCandidates(params),
    listVaultLabels(params),
    listVaultTags(params),
    listVaultContactImportantDateTypes(params),
    listVaultMoodTrackingParameters(params),
    listVaultLifeEventCategories(params),
    listVaultQuickFactTemplates(params),
  ]);
  return {
    layoutData: layoutData(vaultId),
    data: {
      templates: sortByCollator(templates.data.map((t) => dto.template(t, vault))),
      users_in_vault: members.data.map(dto.member),
      users_in_account: candidates.data.map((u) => dto.user(u)),
      labels: labels.data.map(dto.label),
      label_colors: dto.labelColors(),
      tags: tags.data.map(dto.tag),
      contact_important_date_types: dateTypes.data.map(dto.importantDateType),
      mood_tracking_parameters: moodTrackingParameters.data.map(dto.moodTrackingParameter),
      mood_tracking_parameter_colors: dto.moodTrackingParameterColors(),
      life_event_categories: lifeEventCategories.data.map(dto.lifeEventCategory),
      quick_fact_templates: quickFactTemplates.data.map(dto.quickFactTemplateEntry),
      visibility: dto.visibility(vault),
    },
  };
}

/**
 * `VaultSearchController@index`: the search term of the URL
 * (`?searchTerm=`). The page runs the search itself when it mounts, so the
 * results of `VaultSearchIndexViewHelper::data` are not fetched here.
 */
export async function search(to) {
  const vaultId = to.params.vault;
  await getVault({ vault_id: vaultId });
  return {
    layoutData: layoutData(vaultId),
    data: {
      query: to.query.searchTerm ?? null,
    },
  };
}
