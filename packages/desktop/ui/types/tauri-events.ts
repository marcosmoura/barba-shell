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

/**
 * CLI module events
 */
export const CliEvents = {
  COMMAND_RECEIVED: 'cli:command-received',
} as const;
