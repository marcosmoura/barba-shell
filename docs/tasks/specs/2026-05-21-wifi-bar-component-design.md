# WiFi Bar Component Design

## Summary

A new WiFi status bar component that displays the current Wi-Fi connection status and network name. Clicking opens System Settings → Wi-Fi pane.

## Native: `wifi.rs`

File: `app/native/src/modules/bar/components/wifi.rs`

- `WifiInfo` struct with `status: WifiStatus` and `network_name: Option<String>`
- `WifiStatus` enum: `Unknown`, `Off`, `Disconnected`, `Connected`
- `get_wifi_info` Tauri command — calls macOS `networksetup -getairportnetwork en0` and `-getairportpower en0` via Tauri shell plugin, returns parsed result
- `open_wifi_settings` Tauri command — opens `x-apple.systempreferences:com.apple.wifi` via `open` shell command
- Pure parsing helpers for testability (no shell dependency)

## Registration

- Add `pub mod wifi;` to `bar/components/mod.rs`
- Register `get_wifi_info` in `lib.rs` `generate_handler![]`
- Add URL entry `"Wi-Fi"` → `x-apple.systempreferences:com.apple.wifi` to `apps.rs` `ALLOWED_APPS`, matching the existing Battery URL pattern. Frontend calls `invoke('open_app', { name: 'Wi-Fi' })` on click.

## React: `Status/Wifi/`

Files under `app/ui/renderer/bar/Status/Wifi/`:

- `Wifi.types.ts` — TypeScript interfaces matching Rust payload
- `Wifi.state.ts` — `useWifi` hook with React Query polling (every 5s) for `get_wifi_info`
- `Wifi.tsx` — Component showing icon + network name label (or status text)
- `Wifi.test.tsx` — Test component rendering with connected/disconnected/off states
- `index.ts` — Re-export
- Add `<Wifi />` to `Status.tsx` between `<KeepAwake />` and `<Cpu />`

## Behavior

- **Connected**: Show WiFi icon + network name (SSID)
- **Off**: Show WiFi icon + "Wi-Fi Off" label, icon grayed
- **Disconnected**: Show WiFi icon + "Disconnected" label, icon grayed
- **Unknown**: Render null (graceful degrade)
- Click always opens System Settings Wi-Fi pane
- Polling interval: 5 seconds (matches CPU refresh pattern)
- Icon: WifiIcon from @hugeicons/core-free-icons

## Test Plan

- Rust: Unit tests for `networksetup` output parsing (all states, malformed input)
- React: QueryClient-seeded rendering tests for connected/disconnected/off states
- Update `app/ui/tests/setup.ts` with default mocks for `get_wifi_info` and `open_wifi_settings`
