# Barba Shell - AI Coding Instructions

## Project Overview

Barba Shell is a **macOS-only** Tauri 2.x desktop application providing a status bar with tiling window manager integration. It uses a dual-language architecture:

- **Frontend**: React 19 + TypeScript (WebKit/Safari target) with Linaria for zero-runtime CSS
- **Backend**: Rust with Tauri for native macOS APIs and system integration

## Architecture

### Data Flow Pattern

```
Rust Backend (src-tauri/) → Tauri Events/Commands → React Query (frontend) → UI Components
```

1. **Rust services** in `src-tauri/src/bar/components/` expose `#[tauri::command]` functions
2. **Frontend services** in `src/bar/*/` use `invoke()` from `@tauri-apps/api/core` to call Rust commands
3. **React components** use `useTauriEventQuery` hook to subscribe to real-time events and cache data via TanStack Query

### Key Integration Pattern: `useTauriEventQuery`

Located in `src/hooks/useTauriEventQuery.ts` - bridges Tauri events with React Query:

```typescript
const { data } = useTauriEventQuery<PayloadType>({
  eventName: 'tauri_event_name',
  initialFetch: () => invoke('rust_command_name'),
  transformFn: (payload) => transformedData,
});
```

### Component Structure Convention

Each bar feature follows this structure:

```
ComponentName/
├── index.ts           # Re-exports
├── ComponentName.tsx  # React component
├── ComponentName.styles.ts  # Linaria CSS (css`` tagged templates)
├── ComponentName.service.ts # Tauri invoke calls & business logic
└── ComponentName.types.ts   # TypeScript interfaces
```

See `src/bar/Status/Battery/` or `src/bar/Spaces/Hyprspace/` as reference implementations.

## Styling Conventions

- Use **Linaria** (`@linaria/core`) for CSS - exports named CSS class constants:
  ```typescript
  export const button = css`...`;
  export const buttonActive = css`...`;
  ```
- Style files named `*.styles.ts` - automatically processed by `@wyw-in-js/vite`
- Use design tokens from `src/design-system/` (Catppuccin Mocha color palette in `colors.ts`)
- Combine classes with `cx()` from `@linaria/core`

## Rust Backend Patterns

- Commands in `src-tauri/src/bar/components/*.rs` - must be registered in `lib.rs` via `tauri::generate_handler![]`
- Use `#[tauri::command]` attribute for frontend-callable functions
- Events emitted via `window.emit("event_name", payload)` or `app_handle.emit()`
- Strict Clippy lints enabled: `pedantic`, `nursery`, `cargo` warnings

## Development Commands

```bash
pnpm dev              # Start Vite dev server (frontend only)
pnpm tauri dev        # Full app with hot reload
pnpm test:ui          # Vitest browser tests
pnpm test:tauri       # Rust tests via cargo-nextest
pnpm lint             # Both frontend (ESLint/Stylelint) and Rust (Clippy)
pnpm format           # Prettier + cargo fmt
```

## Testing Conventions

- Frontend tests use Vitest with `vitest-browser-react` for component testing
- Test files co-located: `ComponentName.test.tsx` alongside source
- Rust tests inline with `#[cfg(test)]` modules in the same file

## Path Aliases

- `@/` maps to `./src/` (configured in `vite.config.ts` and `tsconfig.app.json`)

## Critical Files

- `src-tauri/src/lib.rs` - Tauri app entry, command registration, plugin setup
- `src-tauri/src/bar/mod.rs` - Window positioning, menubar visibility, component init
- `src/hooks/useTauriEventQuery.ts` - Core pattern for Tauri-React integration
- `src/main.tsx` - React app entry with QueryClient setup
