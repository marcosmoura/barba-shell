import { useEffect } from 'react';

import { useQueryClient, useSuspenseQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';

import { useDisableRightClick } from '@/hooks';
import { useTauri } from '@/hooks/useTauri';
import { MenubarEvents } from '@/types';

import type { InitialState } from './Bar.types';

// ============================================================================
// Initial State Hook (internal)
// ============================================================================

/**
 * Fetches all initial state in a single IPC call and distributes to query caches.
 * This reduces startup IPC calls from ~6 to 1.
 */
const useInitialState = () => {
  const queryClient = useQueryClient();

  const { data } = useSuspenseQuery({
    queryKey: ['initial-state'],
    queryFn: () => invoke<InitialState>('get_initial_state'),
    staleTime: Infinity,
    gcTime: 0,
  });

  useEffect(() => {
    if (!data) return;

    if (data.battery !== null) {
      queryClient.setQueryData(['battery'], data.battery);
    }

    if (data.cpu !== null) {
      queryClient.setQueryData(['cpu'], data.cpu);
    }

    if (data.media !== null) {
      queryClient.setQueryData(['media'], data.media);
    }

    if (data.weatherConfig !== null) {
      queryClient.setQueryData(['weather_config'], data.weatherConfig);
    }

    if (data.tiling) {
      const { workspaces, focusedWorkspace, currentWorkspaceWindows, focusedWindow } = data.tiling;

      queryClient.setQueryData(['tiling_workspace_data'], {
        workspacesData: workspaces.map((w) => w.name),
        focusedWorkspace,
      });

      queryClient.setQueryData(['tiling_workspace_apps'], {
        appsList: currentWorkspaceWindows.map((w) => ({
          appName: w.appName,
          windowId: w.id,
          windowTitle: w.title,
        })),
        focusedApp: focusedWindow
          ? {
              appName: focusedWindow.appName,
              windowId: focusedWindow.id,
              windowTitle: focusedWindow.title,
            }
          : null,
      });
    }
  }, [data, queryClient]);
};

// ============================================================================
// Public Hook
// ============================================================================

export const useBar = () => {
  useInitialState();

  const { data: menuHidden } = useTauri<boolean>({
    queryKey: ['menubar-visibility'],
    queryFn: async () => false,
    eventName: MenubarEvents.VISIBILITY_CHANGED,
    staleTime: Infinity,
  });

  useDisableRightClick();

  return { menuHidden };
};
