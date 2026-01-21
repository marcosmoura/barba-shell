import type { BatteryInfo } from '@/stores/BatteryStore/BatteryStore.types';

import type { TilingWorkspace, TilingWindow } from './Spaces/Spaces.types';

/**
 * CPU information from the backend.
 */
export interface CpuInfo {
  /** CPU usage percentage (0-100). */
  usage: number;
  /** CPU temperature in Celsius (null if unavailable). */
  temperature: number | null;
}

/**
 * Weather configuration from the backend.
 */
export interface WeatherConfigInfo {
  /** API key for Visual Crossing Weather API. */
  visualCrossingApiKey: string;
  /** Default location for weather data when geolocation fails. */
  defaultLocation: string;
}

/**
 * Initial tiling state from the backend.
 */
export interface TilingInitialState {
  /** All workspaces. */
  workspaces: TilingWorkspace[];
  /** Currently focused workspace name. */
  focusedWorkspace: string | null;
  /** Windows in the current workspace. */
  currentWorkspaceWindows: TilingWindow[];
  /** Currently focused window. */
  focusedWindow: TilingWindow | null;
}

/**
 * Batched initial state returned by `get_initial_state` command.
 * Contains all data needed for the bar's initial render in a single IPC call.
 */
export interface InitialState {
  /** Battery information. */
  battery: BatteryInfo | null;
  /** CPU information. */
  cpu: CpuInfo | null;
  /** Current media playback info. */
  media: unknown | null;
  /** Weather configuration. */
  weatherConfig: WeatherConfigInfo | null;
  /** Tiling state (if enabled and initialized). */
  tiling: TilingInitialState | null;
}
