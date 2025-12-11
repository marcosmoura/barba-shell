/**
 * Tauri Custom Events
 *
 * This file provides type definitions for all custom events emitted from Rust to React.
 * All event names follow the pattern: `module:event-name`
 *
 * This centralized definition improves visibility and type safety for backend events.
 */

// ============================================================================
// Event Name Constants
// ============================================================================

/**
 * Tiling module events
 */
export const TilingEvents = {
  WORKSPACES_CHANGED: 'tiling:workspaces-changed',
  WINDOW_CREATED: 'tiling:window-created',
  WINDOW_DESTROYED: 'tiling:window-destroyed',
  WINDOW_FOCUSED: 'tiling:window-focused',
  WINDOW_MOVED: 'tiling:window-moved',
  WINDOW_RESIZED: 'tiling:window-resized',
  APP_ACTIVATED: 'tiling:app-activated',
  APP_DEACTIVATED: 'tiling:app-deactivated',
  SCREEN_FOCUSED: 'tiling:screen-focused',
} as const;

/**
 * Menubar module events
 */
export const MenubarEvents = {
  VISIBILITY_CHANGED: 'menubar:visibility-changed',
} as const;

/**
 * KeepAwake module events
 */
export const KeepAwakeEvents = {
  STATE_CHANGED: 'keepawake:state-changed',
} as const;

/**
 * Media module events
 */
export const MediaEvents = {
  PLAYBACK_CHANGED: 'media:playback-changed',
} as const;
