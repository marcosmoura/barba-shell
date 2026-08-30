# Wallpaper 02 — Discovery Collection

> Status: 🟡 Draft · Open: W3

## Purpose

Build the wallpaper manager's usable collection from a directory or explicit list.

## Scope

- `wallpapers.path` and `wallpapers.list` discovery and list CLI output.

### Out of Scope

| Excluded concern     | Owner                                     | Boundary note                       |
| -------------------- | ----------------------------------------- | ----------------------------------- |
| Rotation             | [Wallpaper 01](01-cycling.md)             | Consumes the discovered collection. |
| Processing artifacts | [Wallpaper 03](03-processing-cache.md)    | Runs after discovery.               |
| macOS screen setting | [Wallpaper 04](04-application-adapter.md) | Receives a selected path.           |

## Terminology

- **Supported image** — a path with case-insensitive `jpg`, `jpeg`, `png`, or `webp` extension.
- **Bare relative path** — a non-tilde relative path passed to `platform::path::expand`.

## Data Contract

A nonempty `path` takes precedence over `list`. Directory discovery returns supported regular entries from `processing::list_images_in_directory`; explicit list entries that do not exist or are unsupported are omitted. An empty final collection makes manager creation return `NoWallpapers`.

## Configuration Contract

`wallpapers.path` defaults to `""` and must name an existing directory when nonempty. `wallpapers.list` defaults to `[]`; entries are expanded individually. The config parser owns JSONC syntax and file precedence.

## Inputs

The global manager calls discovery at construction. `stache wallpaper list` obtains the initialized manager's paths; no input changes the collection after construction.

## State Transitions

No owned mutable state. Construction yields a collection, `InvalidPath`, or an empty collection subsequently rejected as `NoWallpapers`.

## Outputs

`wallpaper list` emits a pretty JSON array when nonempty and exactly `No wallpapers found.` when the collection is empty. Discovery or serialization errors become `StacheError` and a nonzero CLI process result.

## Derived Effects

Directory enumeration reads the configured directory. It does not create files or validate image decodability.

## Failure & Recovery

A missing path or non-directory produces `InvalidPath`. `list_images_in_directory` collapses directory-entry/read errors to the returned vector; there is no distinct unreadable-directory error. Subsequent construction reports `NoWallpapers` if no usable item was returned.

## Cross-Module Contracts

[Wallpaper 01](01-cycling.md) treats its collected paths as immutable. [Wallpaper 04](04-application-adapter.md) maps CLI screen inputs but does not rediscover paths.

## Acceptance Scenarios

1. **Normal directory.** Given an existing directory with PNG and text files, when constructed, then only the PNG is collected.
2. **Boundary precedence.** Given nonempty `path` and `list`, when constructed, then only directory discovery is used.
3. **Failure.** Given a missing configured directory, when constructed, then `InvalidPath` is returned.
4. **Lifecycle.** Given the initialized collection is empty at CLI list time, when list runs, then it prints the exact empty message without changing module state.

## Testing Seam

`WallpaperManager::new` and `processing::list_images_in_directory` make collection outcomes observable without invoking macOS. CLI parser tests cover list grammar, not filesystem outcomes.

## Open Decisions

| ID  | Current behavior                                                                              | Documented intent                                          | Rewrite consequence                                                                                                                       | Evidence                                                                         |
| --- | --------------------------------------------------------------------------------------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| W3  | `expand` expands `~`; a bare relative path remains relative to the process working directory. | List entries are described as full or home-relative paths. | Decide whether bare relatives are rejected, documented as CWD-relative, or resolved another way; do not claim config-relative resolution. | `platform/path.rs::expand`; `manager.rs:120-148`; `wallpapers.md#Configuration`. |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                                            | Implementation evidence                           | Test evidence                                                 | Intended documentation        | Disposition  |
| ----------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------- | ----------------------------- | ------------ |
| Path precedence, supported extensions, skipped list entries | `manager.rs:120-148`; `processing.rs:20,274-301`  | `processing.rs::tests`                                        | `wallpapers.md#Configuration` | Aligned      |
| CLI list shape and empty text                               | `cli/commands/wallpaper.rs:135-148::execute_list` | `cli/commands/wallpaper.rs::tests::test_wallpaper_list_parse` | `cli.md`; `wallpapers.md#CLI` | Aligned      |
| Bare-relative path base                                     | `platform/path.rs::expand`                        | None — source-only evidence                                   | `wallpapers.md#Configuration` | Conflict     |
| Directory read/error collapse                               | `processing.rs:281-301`; `manager.rs:103-148`     | None — source-only evidence                                   | None — source-only evidence   | Current-only |
