# Status Bar 03 — Spaces Presentation

> Status: ✅ Normative

## Purpose

Spaces projects tiling workspaces and focused-workspace windows into the bar and forwards focus requests without owning tiling state.

## Scope

- Query/view-model mapping, ordering, responsive app labels, and highlights.
- Tiling-event invalidation, focus commands, and failure fallback.

### Out of Scope

| Excluded concern                 | Owner                                                                | Boundary note                           |
| -------------------------------- | -------------------------------------------------------------------- | --------------------------------------- |
| Workspace/window state           | [tiling/09 workspace state](../tiling/09-workspace-state.md)         | Spaces only consumes query snapshots.   |
| Focus effect and target validity | [tiling/17 focus navigation](../tiling/17-focus-navigation.md)       | Spaces invokes its Tauri commands.      |
| Bar composition                  | [bar/01 bar window lifecycle](01-bar-window-lifecycle.md)            | Spaces is only the left region.         |
| Tiling event declarations        | [foundation/06 frontend events](../foundation/06-frontend-events.md) | This spec owns cache responses to them. |

## Terminology

- **Workspace priority** — the frontend name ordering before its alphabetical fallback.
- **Laptop presentation** — only the focused app when a laptop media query matches.

## Data Contract

Workspace data is obtained from `get_tiling_workspaces` plus `get_tiling_focused_workspace`; only each workspace `name` is retained. Apps come from `get_tiling_current_workspace_windows` plus `get_tiling_focused_window` and retain `{ appName, windowId: id, windowTitle: title }`.

The current frontend ordering is exactly `terminal`, `coding`, `browser`, `music`, `design`, `communication`, `guitar`, `misc`, `files`, `tasks`; unknown names follow alphabetically. Workspace labels capitalize the first character. Desktop shows all queried apps; laptop mode shows the focused app when present. Duplicate displayed app names use a nonempty title truncated to 40 characters (25 in laptop mode); other apps use the app name.

## Configuration Contract

No Spaces-specific configuration exists. Tiling availability and the frontend laptop media query determine visibility.

## Inputs

- Tauri workspace/window/focus/readiness queries.
- `tiling/initialized`, workspace, tracked/untracked, title, and focus events.
- Workspace and app clicks.

## State Transitions

| From       | Input                                      | To         | Effect                                           |
| ---------- | ------------------------------------------ | ---------- | ------------------------------------------------ |
| Mount      | tiling readiness true or initialized event | Enabled    | Invalidate both query caches.                    |
| Enabled    | workspace-changed                          | Enabled    | Immediately invalidate workspace and app caches. |
| Enabled    | tracked/untracked/title/focus event        | Debouncing | Reset the 100 ms app-cache timer.                |
| Debouncing | timer expires                              | Enabled    | Invalidate the app cache once.                   |
| Enabled    | workspace click                            | Enabled    | Invoke `focus_tiling_workspace({ name })`.       |
| Enabled    | app click                                  | Enabled    | Invoke `focus_tiling_window({ windowId })`.      |
| Any        | tiling query error                         | Empty      | Return empty/default data.                       |

The pending debounce timer is cleared on unmount.

## Outputs

- Ordered workspace and responsive app view models.
- `focus_tiling_workspace` and `focus_tiling_window` requests using bare numeric `windowId`.
- Query invalidation; it does not publish a Spaces event.

## Derived Effects

The hook makes two parallel reads for each snapshot. Focus-command failures are logged by `invokeWithErrorHandling` and do not optimistically mutate cached state.

## Failure & Recovery

Tiling query failures are converted to empty/undefined presentation; an initialization or later event can recover it. A rejected mount-time `is_tiling_enabled` invocation is not caught in the effect. Missed events can leave cache state stale because no polling exists.

## Cross-Module Contracts

[tiling/09](../tiling/09-workspace-state.md) provides query state; [tiling/17](../tiling/17-focus-navigation.md) handles focus. [bar/01](01-bar-window-lifecycle.md) provides rendering. The bar consumes Tiling events through [foundation/06](../foundation/06-frontend-events.md).

## Acceptance Scenarios

1. Given unavailable tiling queries, when Spaces loads, then it returns empty workspace/app fallbacks.
2. Given priority and unknown workspace names, when mapped, then priority names precede alphabetical unknown names.
3. Given initialized tiling, when the event arrives, then both caches invalidate.
4. Given repeated focus-related events within 100 ms, when the timer settles, then the app cache invalidates once.
5. Given workspace change, when received, then both caches invalidate immediately.
6. Given laptop mode and focused app, when rendering, then only that app is shown.
7. Given duplicate displayed app names, when their titles exist, then titles use the documented truncation boundary.
8. Given a workspace or app click, when its command rejects, then state is not changed optimistically.
9. Given unmount during debounce, when cleanup runs, then the timer is cleared.

## Testing Seam

`getSortedWorkspaces`, data fetchers, and `useSpaces` are stable seams. Existing coverage includes
`fetches workspaces and the focused workspace in parallel`, `returns empty defaults when tiling is
unavailable`, `focuses the selected workspace`, `focuses the selected app window`, `invalidates the
apps query after the focus-change debounce`, and `does not fire the focus-change debounce after
unmount` in `app/ui/renderer/bar/Spaces/Spaces.state.test.tsx`.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                             | Implementation evidence                                     | Test evidence                                                                                                                                                                                                                         | Intended documentation | Disposition  |
| -------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------- | ------------ |
| Queries, focus commands, and empty fallbacks | `app/ui/renderer/bar/Spaces/Spaces.state.ts:45-110,188-202` | `Spaces.state.test.tsx` — `fetches workspaces and the focused workspace in parallel`; `returns empty defaults when tiling is unavailable`; `focuses the selected workspace`; `focuses the selected app window`                        | `status-bar.md:58-68`  | Aligned      |
| Frontend priority and alphabetical fallback  | `app/ui/renderer/bar/Spaces/Spaces.state.ts:12-42`          | None — source-only evidence                                                                                                                                                                                                           | `status-bar.md:58-68`  | Current-only |
| Responsive labels and event debounce         | `app/ui/renderer/bar/Spaces/Spaces.state.ts:145-243`        | `Spaces.state.test.tsx` — `invalidates the apps query after the focus-change debounce`; `does not fire the focus-change debounce after unmount`; `Spaces.test.tsx` — `renders focused app name`; `renders multiple apps in workspace` | `status-bar.md:66-68`  | Current-only |
