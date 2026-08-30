# Wallpaper 03 — Processing Cache

> Status: ✅ Normative

## Purpose

Resize configured images, apply effects, and retain processed PNG artifacts for wallpaper application.

## Scope

- Screen-size fallback, effects, cache naming/reuse, and `generate-all` processing.

### Out of Scope

| Excluded concern              | Owner                                      | Boundary note                         |
| ----------------------------- | ------------------------------------------ | ------------------------------------- |
| Source collection             | [Wallpaper 02](02-discovery-collection.md) | Supplies source paths.                |
| Selection/timer               | [Wallpaper 01](01-cycling.md)              | Decides when processing is requested. |
| Applying an artifact to macOS | [Wallpaper 04](04-application-adapter.md)  | Owns desktop setting.                 |

## Terminology

- **Artifact** — cached processed PNG for a source/settings/screen combination.
- **Fallback screen** — `2560×1440` for size lookup failure; a single screen for count failure.

## Data Contract

Cache paths are under the `wallpapers` cache subdirectory. Cache filenames derive from source path, blur, radius, screen dimensions, and for explicit screens the screen index. Processing uses cover resize, optional blur, and optional black-filled rounded corners. Existing cache paths are returned before source decoding.

## Configuration Contract

Consumes `wallpapers.blur` and `wallpapers.radius` as unsigned image-effect values. Collection and timer config are owned by [Wallpaper 01](01-cycling.md).

## Inputs

`process_image` uses primary-screen dimensions; `process_image_for_screen` uses a zero-based index. `wallpaper generate-all` has no flags and processes each collection/screen pair.

## State Transitions

No owned service state. For each request: determine screen size, calculate path, return a present cache path or create cache directory, decode/process, and save PNG.

## Outputs

`generate-all` streams progress to stdout, using a spinner in a terminal, and prints cache/time summary. It returns manager/processing failure through the CLI error path.

## Derived Effects

The cache directory is created as needed. Size lookup uses macOS screen APIs; failure falls back to `2560×1440`, and screen-count failure falls back to one screen.

## Failure & Recovery

Directory creation, image decode, image save, and processing failures are `ProcessingError`. A cache hit is trusted by path existence; there is no decode/metadata validation, single-flight lock, atomic temporary publication, or aggregate nonzero result for a mixed bulk run.

## Cross-Module Contracts

[Wallpaper 01](01-cycling.md) uses this component before each setter. [Wallpaper 04](04-application-adapter.md) receives the returned artifact. Cache-root deletion policy belongs to foundation cache ownership.

## Acceptance Scenarios

1. **Normal cache miss.** Given a supported source and screen, when processed, then a resized/effected PNG is saved under the cache subdirectory.
2. **Boundary fallback.** Given screen lookup failure, when processing runs, then it uses `2560×1440`; count failure yields one generation screen.
3. **Failure.** Given an undecodable source, when processed, then it returns `ProcessingError`.
4. **Lifecycle/cache hit.** Given an existing cache path, when requested again, then it is reused without validation or regeneration.

## Testing Seam

Pure size/cache-key/effect helpers in `processing.rs` are stable unit seams. Filesystem-backed processing tests cover image behavior without desktop-setting APIs.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                            | Implementation evidence                                   | Test evidence                                                         | Intended documentation           | Disposition  |
| ----------------------------------------------------------- | --------------------------------------------------------- | --------------------------------------------------------------------- | -------------------------------- | ------------ |
| Format, resize/effects, cache key and directory             | `processing.rs:20-24,181-262,303-474`                     | `processing.rs::tests`                                                | `wallpapers.md#Image Processing` | Aligned      |
| CLI generate-all grammar and streamed output                | `cli/commands/wallpaper.rs:151-160`; `manager.rs:488-754` | `cli/commands/wallpaper.rs::tests::test_wallpaper_generate_all_parse` | `wallpapers.md#CLI`              | Aligned      |
| Primary-screen fallback                                     | `processing.rs:85-116`                                    | None — source-only evidence                                           | `wallpapers.md#Image Processing` | Aligned      |
| Per-screen count/size fallback to one screen or `2560×1440` | `processing.rs:118-179`                                   | None — source-only evidence                                           | None — source-only evidence      | Current-only |
| Cache trust, publication, and bulk partial-result behavior  | `processing.rs:303-345`; `manager.rs:640-754`             | None — source-only evidence                                           | None — source-only evidence      | Current-only |
