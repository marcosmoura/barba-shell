# Stache Feature Specs

Evergreen capability specifications for Stache. Each spec defines a product contract and its
decision boundary; the implementation is changed only through an evidence-backed update to that
contract.

## Evidence and Status

The current implementation evidence is source commit
`44358d4b06902b090ec769fa0b985a4d62a75096`. Intended behavior is the external
`stache-docs` snapshot whose index identifies that same source snapshot. Neither evidence class
overrides the other.

Every capability follows [template.md](template.md): its **Evidence Base** separates
implementation evidence, test evidence, and intended documentation. Only `Aligned` and
`Current-only` evidence is promoted into normative contract text. `Intended-only`, `Conflict`,
and `Known defect` evidence remains decision-bearing and keeps the owner draft.

| Mark         | Meaning                                                      |
| ------------ | ------------------------------------------------------------ |
| ✅ Normative | A settled capability with no open decision.                  |
| 🟡 Draft     | A capability with one or more unresolved evidence decisions. |

**Audit status:** 44 normative specs and 16 draft specs, for 60 capability specs.

## Catalog

<a id="specs-foundation"></a>

### Foundation (`foundation/`)

Shared configuration, lifecycle, platform, and application-shell contracts. The
application shell owns the allowlisted `open_app` boundary consumed by bar items, including
[CPU](bar/09-cpu-item.md).

| #   | Spec                                                              | Status       | Responsibility                                                                                    |
| --- | ----------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------------- |
| 01  | [configuration-contract](foundation/01-configuration-contract.md) | 🟡 Draft     | Config discovery, JSONC/defaults/schema, immutable snapshot, change detection                     |
| 02  | [module-lifecycle](foundation/02-module-lifecycle.md)             | 🟡 Draft     | Registered lifecycle abstraction and unresolved universal startup/tray-routing boundary           |
| 03  | [startup-orchestration](foundation/03-startup-orchestration.md)   | 🟡 Draft     | Desktop base/background initialization, tiling sequencing, and detached shortcut commands         |
| 04  | [shutdown-reload](foundation/04-shutdown-reload.md)               | ✅ Normative | Terminal arbitration, cleanup ordering, signals, reload/exit precedence                           |
| 05  | [cli-control-surface](foundation/05-cli-control-surface.md)       | ✅ Normative | Binary modes, CLI grammar/output, socket protocol, query/command transport                        |
| 06  | [frontend-events](foundation/06-frontend-events.md)               | ✅ Normative | Tauri command and event declarations, emitters, consumers, and delivery/cache contracts           |
| 07  | [cache-management](foundation/07-cache-management.md)             | 🟡 Draft     | Cache roots/namespaces and clear-cache live-socket safety boundary                                |
| 08  | [keybinding-dispatch](foundation/08-keybinding-dispatch.md)       | ✅ Normative | Shortcut normalization/registration, special keys, shell-free command sequences                   |
| 09  | [platform-capabilities](foundation/09-platform-capabilities.md)   | ✅ Normative | Accessibility snapshot, executors, safe platform-adapter/FFI rules                                |
| 10  | [application-shell](foundation/10-application-shell.md)           | 🟡 Draft     | macOS runtime package, bundle/window/plugin/resource boundary, and allowlisted application launch |

<a id="specs-wallpaper"></a>

### Wallpapers (`wallpaper/`)

| #   | Spec                                                         | Status       | Responsibility                                                             |
| --- | ------------------------------------------------------------ | ------------ | -------------------------------------------------------------------------- |
| 01  | [cycling](wallpaper/01-cycling.md)                           | 🟡 Draft     | Selection order, rotation timer, pause/resume/manual interplay             |
| 02  | [discovery-collection](wallpaper/02-discovery-collection.md) | 🟡 Draft     | Path/list discovery, filtering, ordering, immutable collection             |
| 03  | [processing-cache](wallpaper/03-processing-cache.md)         | ✅ Normative | Screen sizing, resize/effects pipeline, cache identity and bulk generation |
| 04  | [application-adapter](wallpaper/04-application-adapter.md)   | ✅ Normative | macOS all/one-screen application and error/partial-success semantics       |

<a id="specs-audio"></a>

### Audio (`audio/`)

| #   | Spec                                               | Status       | Responsibility                                                            |
| --- | -------------------------------------------------- | ------------ | ------------------------------------------------------------------------- |
| 01  | [device-inventory](audio/01-device-inventory.md)   | 🟡 Draft     | CoreAudio observation, directional capability, routing/list schemas       |
| 02  | [routing-policy](audio/02-routing-policy.md)       | ✅ Normative | Pure priority/dependency, AirPlay/HDMI, and fallback target selection     |
| 03  | [topology-reaction](audio/03-topology-reaction.md) | 🟡 Draft     | Listener generations, reconciliation, default writes, pause/resume safety |
| 04  | [queries-commands](audio/04-queries-commands.md)   | ✅ Normative | Standalone list grammar, filters, table/JSON and exit semantics           |

<a id="specs-capabilities"></a>

### Standalone Capabilities (`capabilities/`)

| #   | Spec                                              | Status       | Responsibility                                                                |
| --- | ------------------------------------------------- | ------------ | ----------------------------------------------------------------------------- |
| 01  | [hold-to-quit](capabilities/01-hold-to-quit.md)   | 🟡 Draft     | Command+Q hold state, suppression, termination, alert and tap lifecycle       |
| 02  | [menu-anywhere](capabilities/02-menu-anywhere.md) | ✅ Normative | Exact mouse trigger, AX menu projection/action, tap lifecycle                 |
| 03  | [notunes](capabilities/03-notunes.md)             | ✅ Normative | Music/iTunes launch blocking, replacement, workspace observer lifecycle       |
| 04  | [keep-awake](capabilities/04-keep-awake.md)       | 🟡 Draft     | Desired/actual assertion, lock reaction, commands/events and watcher          |
| 05  | [tray-controls](capabilities/05-tray-controls.md) | 🟡 Draft     | Base menu, deferred Modules projection, toggle threading and terminal actions |

<a id="specs-bar"></a>

### Status Bar (`bar/`)

The bar owns visible composition, not reusable component/store/hook implementation details.

| #   | Spec                                                                 | Status       | Responsibility                                                                             |
| --- | -------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------ |
| 01  | [bar-window-lifecycle](bar/01-bar-window-lifecycle.md)               | ✅ Normative | Bar lifecycle plus `Spaces → Media → Status` composition and ordered status items          |
| 02  | [geometry-menubar-visibility](bar/02-geometry-menubar-visibility.md) | 🟡 Draft     | Logical frame/display reaction and system-menu visibility observation/presentation         |
| 03  | [spaces-presentation](bar/03-spaces-presentation.md)                 | ✅ Normative | Tiling query projection, responsive workspace/window UI and event invalidation             |
| 04  | [media-item](bar/04-media-item.md)                                   | ✅ Normative | Sidecar media snapshots, artwork/cache, source presentation and launch                     |
| 05  | [battery-item](bar/05-battery-item.md)                               | ✅ Normative | Battery snapshot/polling, bar state, and detailed widget content                           |
| 06  | [wifi-item](bar/06-wifi-item.md)                                     | ✅ Normative | Interface classification, network name/signal, settings action                             |
| 07  | [weather-data](bar/07-weather-data.md)                               | ✅ Normative | Provider/location fallbacks, forecast cache, bar/widget presentation                       |
| 08  | [clock-calendar](bar/08-clock-calendar.md)                           | 🟡 Draft     | Clock format/refresh and calendar navigation/midnight freshness                            |
| 09  | [cpu-item](bar/09-cpu-item.md)                                       | ✅ Normative | CPU information query, temperature fallback, hot presentation, and Activity Monitor action |

The [bar window lifecycle](bar/01-bar-window-lifecycle.md) owns `Spaces → Media → Status`.
Within Status, the order is [Weather](bar/07-weather-data.md) →
[CPU](bar/09-cpu-item.md) → [Battery](bar/05-battery-item.md) →
[Keep Awake](capabilities/04-keep-awake.md) → [Wi-Fi](bar/06-wifi-item.md) →
[Clock](bar/08-clock-calendar.md). Application-launch authorization remains owned by the
[application shell](foundation/10-application-shell.md); no independent Apps item exists.

<a id="specs-widgets"></a>

### Widgets (`widgets/`)

| #   | Spec                                                 | Status   | Responsibility                                                                |
| --- | ---------------------------------------------------- | -------- | ----------------------------------------------------------------------------- |
| 01  | [overlay-lifecycle](widgets/01-overlay-lifecycle.md) | 🟡 Draft | Widget window toggling, positioning, animation races, click-outside dismissal |

<a id="specs-tiling"></a>

### Tiling (`tiling/`)

| #   | Spec                                                                      | Status       | Responsibility                                                         |
| --- | ------------------------------------------------------------------------- | ------------ | ---------------------------------------------------------------------- |
| 01  | [window-eligibility](tiling/01-window-eligibility.md)                     | ✅ Normative | Which windows are ever managed, laid out, or bordered                  |
| 02  | [window-enumeration](tiling/02-window-enumeration.md)                     | ✅ Normative | AX-first discovery scan producing window candidates                    |
| 03  | [app-window-identity](tiling/03-app-window-identity.md)                   | ✅ Normative | PID+launch-date identity, fail-closed capture, PID-reuse safety        |
| 04  | [rules-workspace-assignment](tiling/04-rules-workspace-assignment.md)     | ✅ Normative | Rule matching and the workspace fallback chain                         |
| 05  | [event-observation](tiling/05-event-observation.md)                       | ✅ Normative | AXObserver/NSWorkspace/display sources, observer lifecycle             |
| 06  | [event-normalization-batching](tiling/06-event-normalization-batching.md) | ✅ Normative | Immediate focus dispatch, geometry batching, dedup of no-op events     |
| 07  | [initial-reconciliation](tiling/07-initial-reconciliation.md)             | ✅ Normative | Startup scan → tracked state bootstrap                                 |
| 08  | [screen-topology](tiling/08-screen-topology.md)                           | ✅ Normative | Display connect/disconnect, screen records, workspace placement        |
| 09  | [workspace-state](tiling/09-workspace-state.md)                           | ✅ Normative | Workspace creation, defaults, focus/visibility flags                   |
| 10  | [window-state-machine](tiling/10-window-state-machine.md)                 | ✅ Normative | Tracked window lifecycle: created→tracked→destroyed, attribute updates |
| 11  | [native-tabs](tiling/11-native-tabs.md)                                   | ✅ Normative | Tab detection, group membership, active-tab treatment                  |
| 12  | [layout-selection](tiling/12-layout-selection.md)                         | 🟡 Draft     | Per-workspace layout resolution and runtime switching                  |
| 13  | [layout-algorithms](tiling/13-layout-algorithms.md)                       | ✅ Normative | dwindle/split/monocle/master/grid/floating computation contracts       |
| 14  | [gaps-usable-geometry](tiling/14-gaps-usable-geometry.md)                 | ✅ Normative | Inner/outer gap application, bar-aware top inset, per-screen gaps      |
| 15  | [minimum-size-constraints](tiling/15-minimum-size-constraints.md)         | ✅ Normative | Reported vs inferred minimums, violation prevention                    |
| 16  | [frame-application](tiling/16-frame-application.md)                       | ✅ Normative | Applying computed frames, transaction/animation interplay              |
| 17  | [focus-navigation](tiling/17-focus-navigation.md)                         | ✅ Normative | Focus tracking, directional navigation, focus-driven effects           |
| 18  | [move-reorder](tiling/18-move-reorder.md)                                 | ✅ Normative | Swapping/reordering windows within a workspace                         |
| 19  | [cross-workspace-moves](tiling/19-cross-workspace-moves.md)               | ✅ Normative | Send-to-workspace/screen semantics incl. hide/unhide                   |
| 20  | [resize-split-ratios](tiling/20-resize-split-ratios.md)                   | ✅ Normative | Interactive resize driving ratio adjustments                           |
| 21  | [floating-presets](tiling/21-floating-presets.md)                         | ✅ Normative | Floating flag, default position, named presets, preset-on-open         |
| 22  | [workspace-balance](tiling/22-workspace-balance.md)                       | ✅ Normative | Even distribution rebalancing                                          |
| 23  | [mouse-interactions](tiling/23-mouse-interactions.md)                     | ✅ Normative | Drag-to-tile, modifier interactions, drag state machine                |
| 24  | [visibility-app-hiding](tiling/24-visibility-app-hiding.md)               | ✅ Normative | Workspace switching hides/unhides whole apps; ownership tracking       |
| 25  | [hidden-app-restoration](tiling/25-hidden-app-restoration.md)             | ✅ Normative | Restoring Stache-hidden apps on shutdown/pause; seal-and-drain         |
| 26  | [focused-borders](tiling/26-focused-borders.md)                           | ✅ Normative | JankyBorders integration, per-layout overrides, animation throttling   |
| 27  | [runtime-lifecycle-quarantine](tiling/27-runtime-lifecycle-quarantine.md) | ✅ Normative | Pause/resume teardown, staged startup rollback, quarantine retry       |

## Glossary

Canonical vocabulary pinned by the audited owner. Status follows the owner spec.

| Term                      | Definition                                                                      | Pinned in                                                            |
| ------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| Active config             | Selected configuration file for the current process.                            | [foundation/01](foundation/01-configuration-contract.md) 🟡 Draft    |
| Snapshot                  | Immutable prepared configuration retained by the running process.               | [foundation/01](foundation/01-configuration-contract.md) 🟡 Draft    |
| Lifecycle registry        | The lifecycle abstraction managed during Tauri setup.                           | [foundation/02](foundation/02-module-lifecycle.md) 🟡 Draft          |
| Base modules              | Watcher, IPC socket, tray, bar, and widgets initialized before background work. | [foundation/03](foundation/03-startup-orchestration.md) 🟡 Draft     |
| Background initialization | The asynchronous work started by `lazy_load_modules`.                           | [foundation/03](foundation/03-startup-orchestration.md) 🟡 Draft     |
| Pending action            | The terminal action currently awaiting final commit.                            | [foundation/04](foundation/04-shutdown-reload.md) ✅ Normative       |
| JSON line                 | One newline-delimited JSON socket request or response.                          | [foundation/05](foundation/05-cli-control-surface.md) ✅ Normative   |
| Declared-only             | A public event declaration without a verified emitter.                          | [foundation/06](foundation/06-frontend-events.md) ✅ Normative       |
| Cache root                | The app cache directory, including the live IPC socket path today.              | [foundation/07](foundation/07-cache-management.md) 🟡 Draft          |
| Standard binding          | A registered ordinary global shortcut.                                          | [foundation/08](foundation/08-keybinding-dispatch.md) ✅ Normative   |
| Trusted                   | Accessibility permission state accepted by the platform adapter.                | [foundation/09](foundation/09-platform-capabilities.md) ✅ Normative |
| Desktop mode              | Invocation selected as desktop mode by argument/path rules.                     | [foundation/10](foundation/10-application-shell.md) 🟡 Draft         |
| Allowed app               | Canonical launch target admitted by case-insensitive `open_app` matching.       | [foundation/10](foundation/10-application-shell.md) 🟡 Draft         |
| Manual action             | Explicit wallpaper set or generate request.                                     | [wallpaper/01](wallpaper/01-cycling.md) 🟡 Draft                     |
| Supported image           | A discovery candidate with a supported image extension.                         | [wallpaper/02](wallpaper/02-discovery-collection.md) 🟡 Draft        |
| Artifact                  | Processed wallpaper image for a screen/settings target.                         | [wallpaper/03](wallpaper/03-processing-cache.md) ✅ Normative        |
| All-screen setter         | Adapter operation applying a wallpaper through the aggregate all-screen path.   | [wallpaper/04](wallpaper/04-application-adapter.md) ✅ Normative     |
| Inventory                 | CoreAudio device observation used for audio policy.                             | [audio/01](audio/01-device-inventory.md) 🟡 Draft                    |
| Priority entry            | One ordered audio routing-policy entry.                                         | [audio/02](audio/02-routing-policy.md) ✅ Normative                  |
| Generation                | Audio listener identity used to reject stale callbacks.                         | [audio/03](audio/03-topology-reaction.md) ✅ Normative               |
| All filter                | Audio CLI filter selecting all inventory records.                               | [audio/04](audio/04-queries-commands.md) ✅ Normative                |
| Early release             | Command+Q release before the configured hold interval.                          | [capabilities/01](capabilities/01-hold-to-quit.md) 🟡 Draft          |
| Chord                     | Configured mouse modifier set for Menu Anywhere.                                | [capabilities/02](capabilities/02-menu-anywhere.md) ✅ Normative     |
| Blocked app               | Music application denied launch by NoTunes.                                     | [capabilities/03](capabilities/03-notunes.md) ✅ Normative           |
| Desired awake             | Process-local intent to own a wake assertion.                                   | [capabilities/04](capabilities/04-keep-awake.md) 🟡 Draft            |
| Actual awake              | Whether Keep Awake currently owns an assertion handle.                          | [capabilities/04](capabilities/04-keep-awake.md) 🟡 Draft            |
| Projected status          | Tray representation of a feature status.                                        | [capabilities/05](capabilities/05-tray-controls.md) 🟡 Draft         |
| Bar window                | The Tauri webview labelled `bar`.                                               | [bar/01](bar/01-bar-window-lifecycle.md) ✅ Normative                |
| Status composition        | The ordered right-side Status group.                                            | [bar/01](bar/01-bar-window-lifecycle.md) ✅ Normative                |
| Menu visible              | Observed visibility of the native macOS menu bar.                               | [bar/02](bar/02-geometry-menubar-visibility.md) 🟡 Draft             |
| Workspace priority        | Hard-coded presentation order for known workspaces.                             | [bar/03](bar/03-spaces-presentation.md) ✅ Normative                 |
| Media payload             | Current sidecar media snapshot projected to the bar.                            | [bar/04](bar/04-media-item.md) ✅ Normative                          |
| Absent battery            | Successful no-battery result that hides the item.                               | [bar/05](bar/05-battery-item.md) ✅ Normative                        |
| RSSI                      | Wi-Fi received signal strength used for classification.                         | [bar/06](bar/06-wifi-item.md) ✅ Normative                           |
| Current conditions        | Resolved weather conditions used by the bar/widget.                             | [bar/07](bar/07-weather-data.md) ✅ Normative                        |
| Displayed month           | Calendar month currently being rendered.                                        | [bar/08](bar/08-clock-calendar.md) 🟡 Draft                          |
| Global usage              | Aggregate rather than per-core CPU utilization.                                 | [bar/09](bar/09-cpu-item.md) ✅ Normative                            |
| Available temperature     | CPU temperature strictly between 0°C and 150°C.                                 | [bar/09](bar/09-cpu-item.md) ✅ Normative                            |
| Hot                       | CPU temperature at or above 85°C.                                               | [bar/09](bar/09-cpu-item.md) ✅ Normative                            |
| Active widget             | Widget currently open or closing in the shared overlay.                         | [widgets/01](widgets/01-overlay-lifecycle.md) 🟡 Draft               |
| Close generation          | Counter invalidating stale widget-close continuations.                          | [widgets/01](widgets/01-overlay-lifecycle.md) 🟡 Draft               |
| Actor message             | Input crossing into the tiling actor boundary.                                  | [tiling/01](tiling/01-window-eligibility.md) ✅ Normative            |
| Bare external window ID   | Numeric window ID at an external boundary; not an exact target.                 | [tiling/03](tiling/03-app-window-identity.md) ✅ Normative           |
| Exact window target       | Application identity plus window ID for native or delayed work.                 | [tiling/03](tiling/03-app-window-identity.md) ✅ Normative           |
| LayoutType                | One of the configured tiling layout names.                                      | [tiling/12](tiling/12-layout-selection.md) 🟡 Draft                  |
| Usable frame              | Screen geometry after gaps and main-bar inset.                                  | [tiling/14](tiling/14-gaps-usable-geometry.md) ✅ Normative          |
| Effective minimum         | Combined reported and inferred minimum window size.                             | [tiling/15](tiling/15-minimum-size-constraints.md) ✅ Normative      |
| Focused target            | Exact window target currently selected by tiling.                               | [tiling/17](tiling/17-focus-navigation.md) ✅ Normative              |
| Quarantine                | Partial tiling runtime retained for later stop retry.                           | [tiling/27](tiling/27-runtime-lifecycle-quarantine.md) ✅ Normative  |

## Decision Ledger

Only current decision IDs from the audited specs and session ledger appear here. Each ID has one
primary owner; secondary consumers link to that owner instead of creating duplicate ledger rows.

| ID     | Owner                                                             | Decision                                                                                                  | Status                     |
| ------ | ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | -------------------------- |
| C3     | [foundation/01](foundation/01-configuration-contract.md) 🟡 Draft | Strict configuration-load failure versus the current default fallback.                                    | Open                       |
| L2     | [foundation/02](foundation/02-module-lifecycle.md) 🟡 Draft       | Whether lifecycle registration becomes the universal startup route.                                       | Open                       |
| L3     | [foundation/02](foundation/02-module-lifecycle.md) 🟡 Draft       | Whether lifecycle registry operations become the uniform tray/control route.                              | Open                       |
| S1     | [foundation/03](foundation/03-startup-orchestration.md) 🟡 Draft  | Whether shortcut command completion must form a startup barrier.                                          | Open                       |
| S2     | [foundation/03](foundation/03-startup-orchestration.md) 🟡 Draft  | Whether startup failures require a common observable lifecycle status.                                    | Open                       |
| CA1    | [foundation/07](foundation/07-cache-management.md) 🟡 Draft       | Whether cache clear may delete the live IPC socket.                                                       | Open                       |
| CA2    | [foundation/07](foundation/07-cache-management.md) 🟡 Draft       | Whether cache subdirectory paths require root-containment enforcement.                                    | Open                       |
| F10-D1 | [foundation/10](foundation/10-application-shell.md) 🟡 Draft      | Whether stale README macOS 10.15 documentation is corrected or qualified against the 14.0 bundle minimum. | Open                       |
| W1     | [wallpaper/01](wallpaper/01-cycling.md) 🟡 Draft                  | The evidence-backed manual-action timer-reset outcome.                                                    | Resolved (evidence-backed) |
| W01-D1 | [wallpaper/01](wallpaper/01-cycling.md) 🟡 Draft                  | Whether per-screen random wallpaper selection must guarantee different images.                            | Open                       |
| W3     | [wallpaper/02](wallpaper/02-discovery-collection.md) 🟡 Draft     | How bare relative wallpaper paths are resolved or rejected.                                               | Open                       |
| AU1    | [audio/01](audio/01-device-inventory.md) 🟡 Draft                 | Whether inventory failures retain typed partial or unavailable results instead of collapsing facts.       | Open                       |
| AU3    | [audio/03](audio/03-topology-reaction.md) 🟡 Draft                | Whether enabled audio startup failure is distinct from a deliberate pause.                                | Open                       |
| AU4    | [audio/03](audio/03-topology-reaction.md) 🟡 Draft                | Whether listener setup/removal failure requires rollback or quarantine.                                   | Open                       |
| AU2    | [audio/02](audio/02-routing-policy.md) ✅ Normative               | The evidence-backed `dependsOn` routing-policy resolution outcome.                                        | Resolved (evidence-backed) |
| AU5    | [audio/04](audio/04-queries-commands.md) ✅ Normative             | The evidence-backed read-only audio CLI outcome.                                                          | Resolved (evidence-backed) |
| CQ1    | [capabilities/01](capabilities/01-hold-to-quit.md) 🟡 Draft       | Whether the existing unconsumed Command+Q alert event needs visible feedback.                             | Open                       |
| KA1    | [capabilities/04](capabilities/04-keep-awake.md) 🟡 Draft         | How the object backend event and boolean frontend consumer defect are resolved.                           | Open                       |
| KA2    | [capabilities/04](capabilities/04-keep-awake.md) 🟡 Draft         | The evidence-backed process-lifetime lock-watcher ownership outcome.                                      | Resolved (evidence-backed) |
| KA3    | [capabilities/04](capabilities/04-keep-awake.md) 🟡 Draft         | Whether wake-acquisition failure has an observable error or retry contract.                               | Open                       |
| TR1    | [capabilities/05](capabilities/05-tray-controls.md) 🟡 Draft      | Whether failed tray toggles refresh from actual lifecycle status.                                         | Open                       |
| TR2    | [capabilities/05](capabilities/05-tray-controls.md) 🟡 Draft      | Whether tray/submenu construction failure needs an observable, retryable recovery contract.               | Open                       |
| C05-D1 | [capabilities/05](capabilities/05-tray-controls.md) 🟡 Draft      | Whether submenu installation after initializer returns establishes accurate module-ready check states.    | Open                       |
| MB1    | [bar/02](bar/02-geometry-menubar-visibility.md) 🟡 Draft          | Whether menu visibility affects only renderer content or native bar-window visibility.                    | Open                       |
| CL2    | [bar/08](bar/08-clock-calendar.md) 🟡 Draft                       | Whether month navigation clamps dates 29–31 or retains JavaScript overflow.                               | Open                       |
| WG2    | [widgets/01](widgets/01-overlay-lifecycle.md) 🟡 Draft            | Whether a newer widget open may replace an in-flight close.                                               | Open                       |
| T12-D1 | [tiling/12](tiling/12-layout-selection.md) 🟡 Draft               | Whether the declared layout-changed event gains a verified emitter and consumer contract.                 | Open                       |

## Maintenance Rules

- Update a capability, its glossary pin, catalog status, and its Decision Ledger row together.
- A resolved decision remains `Resolved (evidence-backed)` only when its owner records the outcome
  under **Resolved Decisions** and its evidence supports the normative text.
- Out-of-scope concerns link directly to their current owner or state `Not a current capability`.
- Implementation plans may reference specifications but do not redefine their contracts.
