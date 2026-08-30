# Foundation 02 — Module Lifecycle

> Status: 🟡 Draft · Open: L2, L3

## Purpose

Describe the registered lifecycle abstraction without claiming that it currently governs feature startup or tray control.

## Scope

- Registry construction, registration, lookup, snapshots, and lifecycle operation interface.

### Out of Scope

| Excluded concern            | Owner                                                       | Boundary note                                                          |
| --------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------- |
| Actual startup order        | [startup orchestration](03-startup-orchestration.md)        | `lib.rs` currently calls feature `init` functions directly.            |
| Tray menu controls          | [tray controls](../capabilities/05-tray-controls.md)        | Its feature-specific handlers are not established as registry routing. |
| Resource state for a module | [capability specifications](../index.md#specs-capabilities) | Registry status is not proof of adapter state.                         |

## Terminology

- **Lifecycle registry**: the `LifecycleRegistry` managed during Tauri setup.
- **Lifecycle module**: an object registered with that registry.

## Data Contract

`lib.rs::run` registers Wallpaper, Cmd-Q, NoTunes, Proxy Audio, Menu Anywhere, and Tiling lifecycle objects, then manages the registry. `modules/lifecycle_registry.rs` defines its identity/status and operation surface. L2 and L3 decide whether that surface becomes the universal startup, status, and tray-control contract.

## Configuration Contract

Features own their enabled/configured semantics. L2/L3 decide whether registration becomes a uniform configuration-derived lifecycle policy.

## Inputs

Tauri setup registration and any direct registry lookup/operation callers.

## State Transitions

The registry can be constructed, receive registrations, and be managed by the app. L2 decides startup transition ownership; L3 decides tray transition ownership and aggregate-status publication.

## Outputs

Registry-held module metadata/operations for its callers. L3 decides any public aggregate lifecycle publication.

## Derived Effects

Registration stores lifecycle objects. L2 and L3 decide whether later feature/tray actions must route through them.

## Failure & Recovery

Registry construction/registration occurs in setup. Feature initialization failure/panic handling and any common status/retry/quarantine policy are decision boundaries under L2/L3.

## Cross-Module Contracts

[Startup orchestration](03-startup-orchestration.md) creates the registry before base/background initialization. [Tray controls](../capabilities/05-tray-controls.md) owns its routing contract; L3 decides whether a registry contract supersedes it.

## Acceptance Scenarios

1. Given desktop setup, when it completes registration, then the six lifecycle objects are managed by Tauri.
2. Given a rewrite that needs a universal startup route, when it selects a lifecycle owner, then L2 must be resolved before the route is specified.
3. Given a rewrite that needs an aggregate status after feature failure, when it selects publication semantics, then L2/L3 must be resolved before the status is specified.
4. Given a rewrite that needs a registry toggle, when it selects tray routing, then L3 must be resolved before the action is specified.

## Testing Seam

`modules/lifecycle_registry.rs` is the stable seam for behavior it actually implements; no cited test establishes a process-wide startup or tray contract.

## Open Decisions

| ID  | Current behavior                                                                                             | Documented intent                                                             | Rewrite consequence                                                               | Evidence                                                                                                                                            |
| --- | ------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- | --------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| L2  | The registry is registered, but startup calls feature `init` functions directly.                             | Architecture documentation presents modules as a lifecycle-managed subsystem. | Either migrate startup to the registry or document it as registration-only.       | `app/native/src/lib.rs:217-239`; `app/native/src/modules/lifecycle_registry.rs`; `/Users/marcosmoura/Documents/stache-docs/architecture.md#Modules` |
| L3  | No audited path proves tray toggles route uniformly through registry operations or publish aggregate status. | Lifecycle/tray documentation describes uniform module control.                | Keep tray behavior feature-owned until a concrete routing/status contract exists. | `app/native/src/modules/lifecycle_registry.rs`; `app/native/src/modules/tray`; `/Users/marcosmoura/Documents/stache-docs/tray-and-lifecycle.md`     |

## Resolved Decisions

None.

## Evidence Base

<!-- | Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition | -->

| Contract surface                         | Implementation evidence                                                            | Test evidence               | Intended documentation                             | Disposition |
| ---------------------------------------- | ---------------------------------------------------------------------------------- | --------------------------- | -------------------------------------------------- | ----------- |
| Registry registration                    | `lib.rs::run:217-239`; `modules/lifecycle_registry.rs`                             | None — source-only evidence | `architecture.md#Modules`                          | Conflict    |
| Registry-driven startup and tray control | Direct feature startup in `lib.rs::lazy_load_modules`; feature-specific tray paths | None — source-only evidence | `architecture.md#Modules`; `tray-and-lifecycle.md` | Conflict    |
