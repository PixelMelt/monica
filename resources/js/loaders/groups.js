// Page loaders: groups. Each returns the props of its page (see router/index.js).
import { layoutData } from '@/loaders/common';
import { listVaultGroups, getVaultGroup, listGroupTypes } from '@/api/client';
import { ApiError } from '@/api/runtime';
import { vaultPermission } from '@/lib/session';
import { sortByCollator } from '@/lib/format';
import * as dto from '@/adapters/groups';

/** `Gate::authorize('vault-editor', $vault)`: a 403 page for viewers. */
function authorizeEditor(vaultId) {
  const permission = vaultPermission(vaultId);
  if (permission === null || permission > 200) throw new ApiError(403, { title: 'Forbidden', code: 'forbidden' });
}

/** `GroupController@index`: `GroupIndexViewHelper::data`. */
export async function index(to) {
  const vaultId = to.params.vault;
  const groups = await listVaultGroups({ vault_id: vaultId });
  return {
    layoutData: layoutData(vaultId),
    data: sortByCollator(groups.data).map(dto.groupIndex),
  };
}

/** `GroupController@show`: `GroupShowViewHelper::data`. */
export async function show(to) {
  const vaultId = to.params.vault;
  const group = await getVaultGroup({ vault_id: vaultId, group_id: to.params.group });
  return {
    layoutData: layoutData(vaultId),
    data: dto.groupShow(group),
  };
}

/** `GroupController@edit`: `GroupEditViewHelper::data`. */
export async function edit(to) {
  const vaultId = to.params.vault;
  authorizeEditor(vaultId);
  const [group, groupTypes] = await Promise.all([
    getVaultGroup({ vault_id: vaultId, group_id: to.params.group }),
    listGroupTypes(),
  ]);
  return {
    layoutData: layoutData(vaultId),
    data: dto.groupEdit(group, groupTypes.data),
  };
}
