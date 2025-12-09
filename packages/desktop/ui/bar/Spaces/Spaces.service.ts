import type { QueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';

import type { WorkspaceInfo, Workspaces } from './Spaces.types';

export const fetchWorkspaceList = async () => invoke<WorkspaceInfo[]>('get_workspaces');

export const onWorkspaceChange = (workspaces: Workspaces, queryClient: QueryClient) => {
  if (workspaces) {
    queryClient.setQueryData<Workspaces>(['workspaces'], workspaces);
  }
};

export const onWorkspaceClick = async (name: string) => {
  try {
    await invoke('switch_workspace', { name });
  } catch (error) {
    console.error('Error switching workspace:', error);
  }
};
