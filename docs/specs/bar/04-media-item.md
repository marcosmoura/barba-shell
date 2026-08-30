# Status Bar 04 — Media Item

> Status: ✅ Normative

## Purpose

The Media region displays the last media-control payload when it has a usable label and can request an allowlisted source-player launch.

## Scope

- Sidecar-stream payload caching, artwork processing, event emission, and frontend projection.

### Out of Scope

| Excluded concern                 | Owner                                                                    | Boundary note                                    |
| -------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------ |
| Bar composition                  | [bar/01 bar window lifecycle](01-bar-window-lifecycle.md)                | Media is the centre region.                      |
| Launch authorization and targets | [foundation/10 application shell](../foundation/10-application-shell.md) | Media only consumes `open_app`.                  |
| Cache-root policy                | [foundation/07 cache management](../foundation/07-cache-management.md)   | This component has a media-artwork subdirectory. |

## Terminology

- **Media payload** — the JSON cached from the media-control stream.
- **Artwork** — base64 PNG after current processing.

## Data Contract

`get_current_media_info()` returns the last cached JSON payload or `null`. The frontend derives `title - artist` when artist exists and prefixes `Paused: ` when `playing` is false. It renders nothing without a label. Artwork is decoded from base64, resized/cropped to 128×128 PNG, cached, and published as base64; encoded input is capped at 16 MiB, decoded input at 12 MiB, dimensions at 8192, and pixels at 16 MiB.

## Configuration Contract

No media-item configuration exists.

## Inputs

- The bundled `media-control` sidecar stream.
- `stache://media/playback-changed` JSON payloads.
- A click on a payload whose bundle identifier has a mapped launch target.

## State Transitions

| From     | Input                   | To                   | Effect                                       |
| -------- | ----------------------- | -------------------- | -------------------------------------------- |
| Empty    | stream payload          | Cached               | Process artwork and publish changed payload. |
| Cached   | same state hash         | Cached               | Suppress duplicate publication.              |
| Cached   | changed payload         | Cached               | Replace cache and emit.                      |
| Rendered | mapped-player click     | Rendered             | Invoke `open_app`.                           |
| Any      | missing/invalid payload | Empty or prior cache | Render nothing or log processing failure.    |

## Outputs

The backend emits `stache://media/playback-changed` to the bar webview with media JSON; the command returns the cached JSON or null. The frontend displays optional artwork, a player icon, and the transformed label.

## Derived Effects

`components::init` starts the stream in a named background thread. Media click calls `open_app` only when `MEDIA_APPS_BY_BUNDLE_ID` supplies a mapped target.

## Failure & Recovery

Sidecar/path/stream and artwork processing failures are logged; no item is rendered until usable data appears. Initial command failure is caught by the frontend and treated as null. Artwork load failure clears artwork without hiding valid text. No restart/backoff or explicit thread teardown policy is implemented here.

## Cross-Module Contracts

[bar/01](01-bar-window-lifecycle.md) starts component initialization. [foundation/06](../foundation/06-frontend-events.md) declares the event. [foundation/10](../foundation/10-application-shell.md) owns the launch allowlist.

## Acceptance Scenarios

1. Given a playing payload with title and artist, when rendered, then it shows `title - artist`.
2. Given a paused payload, when rendered, then it prefixes `Paused: `.
3. Given no cached payload or label, when rendered, then the item is absent.
4. Given valid artwork, when processed, then the published data is 128×128 PNG base64.
5. Given oversize/unsupported artwork, when received, then it is rejected without rendering unsafe artwork.
6. Given an unmapped bundle identifier, when clicked, then no launch request is made.
7. Given sidecar failure, when no later payload arrives, then the item remains absent and no retry contract is implied.

## Testing Seam

`parse_output`, artwork decoding/resizing, hash suppression, and `parseMediaPayload` are seams. Existing coverage: `app/native/src/modules/bar/components/media.rs:430-1032` and `app/ui/renderer/bar/Media/Media.test.tsx`.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                               | Implementation evidence                                                | Test evidence                                                                                                                                                                                                                                                                 | Intended documentation | Disposition |
| -------------------------------------------------------------- | ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------- | ----------- |
| Cached command, sidecar, event, and artwork policy             | `app/native/src/modules/bar/components/media.rs:28-64,148-199,249-428` | `media.rs` — `test_get_current_media_info_none_when_unset`; `test_get_current_media_info_returns_last_state`; `test_calculate_state_hash_changes_with_data`; `test_artwork_valid_png_decodes`; `test_artwork_oversized_encoded_rejected`; `test_artwork_pixel_limit_rejected` | `status-bar.md:70-80`  | Aligned     |
| Frontend label, null rendering, mapped launch, artwork preload | `app/ui/renderer/bar/Media/Media.state.ts:15-115`; `Media.tsx:9-29`    | `Media.test.tsx` — `renders nothing when no media is playing`; `renders paused prefix when media is paused`; `renders artwork when available`; `handles click on media container`                                                                                             | `status-bar.md:75-80`  | Aligned     |
