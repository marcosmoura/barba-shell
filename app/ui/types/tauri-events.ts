/**
 * Tauri Event Definitions
 *
 * All events emitted from the Rust backend follow a consistent naming convention:
 * `stache://<module>/<event-name>`
 *
 * This file defines TypeScript constants that mirror the Rust events module
 * at `app/native/src/events.rs`. Keep these in sync!
 */

/**
 * Menubar-related events
 */
export const MenubarEvents = {
  /** Emitted when the system menu bar visibility changes. Payload: boolean */
  VISIBILITY_CHANGED: 'stache://menubar/visibility-changed',
} as const;

/**
 * Keep-awake (caffeinate) related events
 */
export const KeepAwakeEvents = {
  /** Emitted when keep-awake state changes. Payload: { locked: boolean, desired_awake: boolean } */
  STATE_CHANGED: 'stache://keepawake/state-changed',
} as const;

/**
 * Media playback related events
 */
export const MediaEvents = {
  /** Emitted when media playback state changes. Payload: MediaInfo object */
  PLAYBACK_CHANGED: 'stache://media/playback-changed',
} as const;

/**
 * Spaces/workspace related events
 *
 * These events are triggered by CLI commands (`stache event ...`) and are used
 * by the Spaces component to refresh workspace and window data.
 */
export const SpacesEvents = {
  /** Emitted when window focus changes. Triggered by: `stache event window-focus-changed`. Payload: void */
  WINDOW_FOCUS_CHANGED: 'stache://spaces/window-focus-changed',
  /** Emitted when workspace changes. Triggered by: `stache event workspace-changed <name>`. Payload: string */
  WORKSPACE_CHANGED: 'stache://spaces/workspace-changed',
} as const;

/**
 * Widget-related events
 */
export const WidgetsEvents = {
  /** Emitted to toggle a widget's visibility. Payload: WidgetConfig */
  TOGGLE: 'stache://widgets/toggle',
  /** Emitted when user clicks outside the widgets window. Payload: void */
  CLICK_OUTSIDE: 'stache://widgets/click-outside',
} as const;

/**
 * Cmd+Q hold-to-quit related events
 */
export const CmdQEvents = {
  /** Emitted when user presses Cmd+Q to show hold-to-quit alert. Payload: string (message) */
  ALERT: 'stache://cmd-q/alert',
} as const;

/**
 * Reload app events
 */
export const AppEvents = {
  /** Emitted to signal that the app should reload. Payload: void */
  RELOAD: 'stache://app/reload',
} as const;
