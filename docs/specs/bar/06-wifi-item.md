# Status Bar 06 — Wi-Fi Item

> Status: ✅ Normative

## Purpose

The Wi-Fi item classifies the current wireless condition, displays actionable state when known, and requests the allowlisted Wi-Fi settings target on click.

## Scope

- `get_wifi_info` discovery/result, polling projection, classification, and click.

### Out of Scope

| Excluded concern                  | Owner                                                                    | Boundary note                                   |
| --------------------------------- | ------------------------------------------------------------------------ | ----------------------------------------------- |
| App/settings launch authorization | [foundation/10 application shell](../foundation/10-application-shell.md) | This item is only an `open_app` consumer.       |
| Bar composition                   | [bar/01 bar window lifecycle](01-bar-window-lifecycle.md)                | Status owns order.                              |
| Network configuration             | Not a current capability                                                 | The item observes; it does not configure Wi-Fi. |

## Terminology

- **Unknown** — no useful wireless interface state could be determined.
- **RSSI** — optional signal strength in the returned payload.

## Data Contract

`get_wifi_info` accepts no frontend argument and returns `{ status: Unknown | Off | Disconnected | Connected, networkName: string | null, signalStrength: integer | null }`. It runs blocking discovery through `spawn_blocking`; a join failure returns `Unknown`, not a command error.

Connected labels use `networkName`; Off is `Wi-Fi Off`, Disconnected is `Disconnected`, and Unknown has no label and is not rendered. Connected is sky, Disconnected yellow, Off red. Connected signal bands are full at `>= -60`, medium at `>= -70`, low at `>= -80`, otherwise generic; Off and Disconnected use their distinct icons.

## Configuration Contract

None. The 5-second frontend refetch interval is fixed.

## Inputs

- `networksetup`, `ifconfig`, `ipconfig`, and CoreWLAN discovery output.
- Wi-Fi item click.

## State Transitions

| From              | Input                               | To           | Effect                                |
| ----------------- | ----------------------------------- | ------------ | ------------------------------------- |
| Querying          | connected interface                 | Connected    | Display SSID and signal icon.         |
| Querying          | powered-on unconnected interface    | Disconnected | Display disconnected state.           |
| Querying          | powered-off interface               | Off          | Display off state.                    |
| Querying          | no usable interface or task failure | Unknown      | Hide item.                            |
| Any visible state | click                               | Same         | Invoke `open_app({ name: 'Wi-Fi' })`. |

## Outputs

The Tauri result is a snapshot; the renderer polls it every 5 seconds and does not publish an event.

## Derived Effects

The backend resolves command paths once, probes candidate interfaces, and makes CoreWLAN RSSI access on the main thread. The frontend call is an application-launch request governed by [foundation/10](../foundation/10-application-shell.md).

## Failure & Recovery

Shell, parsing, CoreWLAN, and worker join failures degrade to a classified `Unknown`/fallback snapshot. The frontend may hide Unknown rather than expose an error. A later 5-second poll is the current recovery mechanism.

## Cross-Module Contracts

[bar/01](01-bar-window-lifecycle.md) owns status placement. [foundation/10](../foundation/10-application-shell.md) owns `open_app` target validation.

## Acceptance Scenarios

1. Given a connected interface with SSID, when queried, then Connected returns the SSID and optional RSSI.
2. Given a powered-on unconnected interface, when queried, then Disconnected is displayed.
3. Given powered-off Wi-Fi, when queried, then `Wi-Fi Off` is displayed in red.
4. Given no discoverable interface or a blocking-task join failure, when queried, then the item is hidden as Unknown.
5. Given RSSI -60, -70, and -80 boundaries, when rendered, then each uses its documented inclusive icon band.
6. Given a visible item click, when processed, then it requests the Wi-Fi launch target.
7. Given a transient discovery failure, when the next 5-second query succeeds, then the new snapshot replaces Unknown.

## Testing Seam

Command-output parsers, classification, signal bands, and the frontend state hook are stable seams. Existing tests: `app/native/src/modules/bar/components/wifi.rs:386-577` and `app/ui/renderer/bar/Status/Wifi/Wifi.test.tsx`.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                | Implementation evidence                                                  | Test evidence                                                                                                                                                                                                   | Intended documentation  | Disposition |
| ----------------------------------------------- | ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- | ----------- |
| Snapshot discovery, status, and fallback        | `app/native/src/modules/bar/components/wifi.rs:22-43,311-385`            | `wifi.rs` — `test_wifi_info_off`; `test_wifi_info_connected`; `test_wifi_info_disconnected`; `test_wifi_info_unknown`; `test_wifi_info_serialization`                                                           | `status-bar.md:126-130` | Aligned     |
| Poll, classification, hidden Unknown, and click | `app/ui/renderer/bar/Status/Wifi/Wifi.state.ts:19-107`; `Wifi.tsx:11-23` | `Wifi.test.tsx` — `maps excellent macOS RSSI values to full signal icon`; `maps RSSI values to descending signal icons`; `renders wifi off label`; `renders disconnected label`; `renders nothing when unknown` | `status-bar.md:126-130` | Aligned     |
