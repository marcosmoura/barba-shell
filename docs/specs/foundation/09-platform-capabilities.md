# Foundation 09 — Platform Capabilities

> Status: ✅ Normative

## Purpose

Provide the current macOS Accessibility permission boundary and the thread/FFI constraints consumed by system-facing features.

## Scope

- Process-wide Accessibility check cache, prompt behavior, macOS-only build boundary, main-thread dispatch, and observed FFI error propagation.

### Out of Scope

| Excluded concern            | Owner                                                                    | Boundary note                                                         |
| --------------------------- | ------------------------------------------------------------------------ | --------------------------------------------------------------------- |
| Bundle identity/permissions | [application shell](10-application-shell.md)                             | This spec owns runtime permission checking, not package declarations. |
| Tiling capability policy    | [tiling runtime lifecycle](../tiling/27-runtime-lifecycle-quarantine.md) | It consumes Accessibility/dispatch facts.                             |
| Hotkey event-tap policy     | [keybinding dispatch](08-keybinding-dispatch.md)                         | It owns Caps Lock failures and restoration.                           |

## Terminology

- **Trusted**: `AXIsProcessTrusted` reports current Accessibility authorization.
- **Prompted check**: `AXIsProcessTrustedWithOptions` called with `AXTrustedCheckOptionPrompt`.

## Data Contract

`ACCESSIBILITY_GRANTED: OnceLock<bool>` caches the first `is_accessibility_granted()` result for the process. Its initializer calls `platform::accessibility::check_and_prompt`; the result is boolean, not a typed degraded-state object. `is_trusted()` is a nonprompting direct FFI check. The native binary has a compile-time `target_os = "macos"` guard.

## Configuration Contract

No owned configuration keys.

## Inputs

macOS Accessibility state, first application permission check, FFI return values, and callers requiring AppKit/Accessibility main-thread work.

## State Transitions

| From      | Input                            | To                                   | Effect                                       |
| --------- | -------------------------------- | ------------------------------------ | -------------------------------------------- |
| unchecked | first `is_accessibility_granted` | cached true/false                    | Prompting AX API is called once.             |
| cached    | later check                      | same cached value                    | No second prompt/check through this API.     |
| untrusted | dependent feature init           | feature-defined unavailable behavior | `run` warns; each feature owns its handling. |

## Outputs

A boolean permission fact and warning when startup finds it false. It publishes no permission event or queryable aggregate degradation snapshot.

## Derived Effects

`check_and_prompt` constructs the Core Foundation options dictionary and calls ApplicationServices FFI. `platform::thread::dispatch_on_main_sync` is the main-thread boundary used by tiling startup; it executes inline when already on main, otherwise synchronously dispatches and panics if its result channel cannot receive the main-thread closure result. Other platform adapters expose their own error results/logging; no generic typed adapter wrapper is established.

## Failure & Recovery

Authorization denial returns false and usually needs a later application restart after user action. FFI boolean calls do not carry an error object. `dispatch_on_main_sync` has no `Result` error channel: failed return-channel reception panics at its `expect`. No source proves joinability, centralized adapter quarantine, or a queryable degraded-mode API.

## Cross-Module Contracts

[Startup orchestration](03-startup-orchestration.md) obtains and logs the cached fact. [Keybinding dispatch](08-keybinding-dispatch.md), Menu Anywhere, and tiling must treat platform prerequisites as their own feature behavior. [Application shell](10-application-shell.md) owns declarations/resources, not this runtime check.

## Acceptance Scenarios

1. Given first permission lookup, when called, then the prompting check runs and its boolean is cached.
2. Given a later lookup, when called, then it returns the same cached boolean without another prompt.
3. Given denial at startup, when `run` continues, then a warning is logged and unrelated base initialization continues.
4. Given non-macOS compilation, when compiling the binary, then the compile-time guard rejects it.
5. Given tiling main-thread setup from background work, when dispatched, then `dispatch_on_main_sync` blocks until its closure result arrives; if its result channel fails, the caller panics at the dispatch seam.
6. Given a requested typed platform-degradation report, when no implementation exists, then no such current capability is claimed.

## Testing Seam

`platform/accessibility.rs::tests::test_is_trusted_returns_bool` narrowly exercises the nonprompting FFI call. Prompted/cached `is_accessibility_granted` behavior and `dispatch_on_main_sync` channel-failure behavior are source-only integration evidence.

## Open Decisions

None.

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                        | Implementation evidence                                                                                                  | Test evidence                                                    | Intended documentation           | Disposition  |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------- | -------------------------------- | ------------ |
| Cached prompted Accessibility check     | `app/native/src/lib.rs::{ACCESSIBILITY_GRANTED,is_accessibility_granted}`; `platform/accessibility.rs::check_and_prompt` | None — source-only evidence                                      | `getting-started.md#Permissions` | Aligned      |
| Nonprompting direct Accessibility probe | `platform/accessibility.rs::is_trusted`                                                                                  | `platform/accessibility.rs::tests::test_is_trusted_returns_bool` | `getting-started.md#Permissions` | Current-only |
| Typed adapter/degradation publication   | platform adapters and `lib.rs::run` expose boolean/log paths only                                                        | None — source-only evidence                                      | `architecture.md#Permissions`    | Current-only |
