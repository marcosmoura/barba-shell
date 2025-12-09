export type LayoutMode =
  | 'tiling'
  | 'monocle'
  | 'master'
  | 'split'
  | 'split-vertical'
  | 'split-horizontal'
  | 'floating'
  | 'scrolling';

export interface FocusedAppInfo {
  /// Application name (e.g., "Visual Studio Code").
  name: string;
  /// Application bundle identifier (e.g., "com.microsoft.VSCode").
  appId: string;
  /// Number of windows from this app in the workspace.
  windowCount: number;
}

export interface WorkspaceInfo {
  /// Workspace name/identifier.
  name: string;
  /// Current layout mode.
  layout: LayoutMode;
  /// Screen this workspace is on.
  screen: string;
  /// Whether this workspace is currently focused.
  isFocused: boolean;
  /// Number of windows in this workspace.
  windowCount: number;
  /// Information about the focused app in this workspace (if any).
  focusedApp?: FocusedAppInfo;
}

export type Workspaces = WorkspaceInfo[];
