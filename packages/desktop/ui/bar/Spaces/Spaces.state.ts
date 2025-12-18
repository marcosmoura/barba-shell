import { useCallback, useMemo } from 'react';

import { useQueryClient, useQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';

import { useTauriEvent } from '@/hooks';
import { CliEvents } from '@/types';

import type {
  Workspaces,
  CLICommandPayload,
  FocusedAppPayload,
  HyprspaceWorkspacePayload,
} from './Spaces.types';

const workspaceDefaultOrder = [
  'terminal',
  'coding',
  'browser',
  'music',
  'design',
  'communication',
  'misc',
  'files',
  'mail',
  'tasks',
];

const getSortedWorkspaces = (workspaces: HyprspaceWorkspacePayload[] | undefined) => {
  if (!workspaces) {
    return null;
  }

  return [...workspaces].sort(
    (a, b) =>
      workspaceDefaultOrder.indexOf(a.workspace) - workspaceDefaultOrder.indexOf(b.workspace),
  );
};

const fetchCurrentHyprspaceWorkspace = async () => {
  const { workspace } = await invoke<HyprspaceWorkspacePayload>('get_hyprspace_focused_workspace');

  return workspace;
};

const fetchHyprspaceWorkspaceList = async () => {
  const workspaces = await invoke<HyprspaceWorkspacePayload[]>('get_hyprspace_workspaces');

  return getSortedWorkspaces(workspaces)?.map(({ workspace }) => workspace);
};

const fetchFocusedApp = async () => {
  const [{ appName }] = await invoke<FocusedAppPayload>('get_hyprspace_focused_window');

  return appName;
};

export const useSpaces = () => {
  const queryClient = useQueryClient();
  const { data: workspaceData } = useQuery({
    queryKey: ['hyprspace_workspaces'],
    queryFn: fetchHyprspaceWorkspaceList,
    refetchOnMount: true,
  });
  const { data: focusedWorkspace } = useQuery({
    queryKey: ['hyprspace_current_workspace'],
    queryFn: fetchCurrentHyprspaceWorkspace,
    refetchOnMount: true,
  });
  const { data: focusedApp } = useQuery({
    queryKey: ['focused_app'],
    queryFn: fetchFocusedApp,
    refetchOnMount: true,
  });

  useTauriEvent<CLICommandPayload>(CliEvents.COMMAND_RECEIVED, ({ payload: { name } }) => {
    if (name === 'focus-changed') {
      queryClient.invalidateQueries({ queryKey: ['focused_app'] });
    }

    if (name === 'workspace-changed') {
      queryClient.invalidateQueries({ queryKey: ['hyprspace_current_workspace'] });
      queryClient.invalidateQueries({ queryKey: ['hyprspace_workspaces'] });
    }
  });

  const workspaces = useMemo<Workspaces>(() => {
    if (!workspaceData) {
      return [];
    }

    return workspaceData.map((name) => ({
      key: name,
      isFocused: name === focusedWorkspace,
      name: name.charAt(0).toUpperCase() + name.slice(1),
    }));
  }, [focusedWorkspace, workspaceData]);

  const onSpaceClick = useCallback(
    (name: string) => async () => {
      try {
        await invoke('go_to_hyprspace_workspace', { workspace: name });
      } catch (error) {
        console.error('Error switching workspace:', error);
      }
    },
    [],
  );

  return { workspaces, focusedWorkspace, focusedApp, onSpaceClick };
};
