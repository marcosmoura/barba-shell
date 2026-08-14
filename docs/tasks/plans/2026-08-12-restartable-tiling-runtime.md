# Restartable Tiling Runtime — Implementation Plan (Tasks 9 + 15)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the `LifecycleModule` contract, then make tiling shutdown/restart safe: repeatable `CompletionLatch` signals on the state actor and effect subscriber, a restartable `RuntimeSlot::{Empty, Running, Quarantined}` with staged startup and rollback, reversible/generation-gated OS resources, and ordered idempotent `pause_runtime` with quarantine retry plus `TilingLifecycle`.

**Architecture:** Task 9 adds the crate-private lifecycle trait used later by the tray. Task 15A gives the actor and subscriber reusable completion latches. Task 15B replaces the single-shot `OnceLock` globals (`HANDLE`/`PROCESSOR`/`SUBSCRIBER_HANDLE`/`INITIALIZED`) with a serialized `LifecycleState` plus a `RuntimeSlot` that publishes `Running` only after every fatal `InitStage` succeeds through a testable `RuntimeFactory`, routing all AppKit/AX work through `crate::platform::thread::dispatch_on_main_sync`. Task 15C makes each OS resource individually reversible. Task 15D composes ordered teardown (restore → main-thread unregister → processor quiescence → transient pause → subscriber/actor waits → cache clearing), quarantine-on-timeout with idempotent retry, fresh-resume, and the `TilingLifecycle: LifecycleModule` impl.

**Tech Stack:** Rust (Tauri 2.11.2), `parking_lot` (already a dependency), `tokio::sync::mpsc`, GCD main-thread dispatch (`platform/thread.rs`), CoreGraphics/ApplicationServices FFI, `objc` v0.2.7.

**Supersedes:** the Task 9 and Task 15 text in `docs/tasks/plans/2026-07-16-bugfixes-and-tray-module-toggles.md`. Does NOT introduce `AppIdentity`/`VisibilityRegistry` (that is `2026-08-12-app-identity-restoration.md`).

**Preflight constraint (mandatory):** `app/native/src/modules/audio/device.rs` is a protected unstaged user edit (blob `50451982cc8ec2064079a2acec1170dfb49dec38`). Never `git add -A`/`git add .`/`git stash`/`git reset --hard`/`git checkout .`/`git commit -am`. Before every commit run `git hash-object app/native/src/modules/audio/device.rs` and confirm the hash; stage only the explicit paths listed. `app/native/src/app_shutdown.rs` must NOT be modified (Task 20 CAS arbiter, already merged).

**File structure**

| File                                                                       | Responsibility                                                                                                                                                                                                                     |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `app/native/src/modules/services/lifecycle.rs`                             | Create (Task 9): `LifecycleModule` trait + `ModuleStatus`                                                                                                                                                                          |
| `app/native/src/modules/services/mod.rs`                                   | Create (Task 9): `pub mod lifecycle;`                                                                                                                                                                                              |
| `app/native/src/modules/mod.rs`                                            | Modify (Task 9): `pub mod services;`                                                                                                                                                                                               |
| `app/native/src/modules/tiling/init.rs`                                    | Modify (15A-15D): `CompletionLatch`, `LifecycleState`, `InitStage`, `PartialRuntime`, `TilingRuntime`, `TeardownProgress`, `RuntimeSlot`, `RuntimeFactory`, `start_runtime`/`pause_runtime`/`resume`/`shutdown`, `TilingLifecycle` |
| `app/native/src/modules/tiling/actor/mod.rs`                               | Modify (15A): `spawn() -> (StateActorHandle, CompletionLatch)`, mark latch on loop exit                                                                                                                                            |
| `app/native/src/modules/tiling/effects/subscriber.rs`                      | Modify (15A): completion latch on `run()` exit; `new()` returns triple                                                                                                                                                             |
| `app/native/src/modules/tiling/events/processor.rs`                        | Modify (15C-1): `stop_and_wait`, timer completion, routing-map clear                                                                                                                                                               |
| `app/native/src/modules/tiling/events/app_monitor.rs`                      | Modify (15C-2): generation gate, observer retention, `shutdown`, `uninstall_adapter`                                                                                                                                               |
| `app/native/src/modules/tiling/events/screen_monitor.rs`                   | Modify (15C-2): generation gate, `CGDisplayRemoveReconfigurationCallback`, main-thread re-dispatch, `shutdown`                                                                                                                     |
| `app/native/src/modules/tiling/events/ax_observer.rs`                      | Modify (15C-2): generation gate, `shutdown`                                                                                                                                                                                        |
| `app/native/src/modules/tiling/events/observer.rs`                         | Modify (15C-2): standalone `shutdown()` draining observers, reset init guard                                                                                                                                                       |
| `app/native/src/modules/tiling/events/mouse_monitor.rs`                    | Modify (15C-3): `ACTIVE` gate, `set_active`, callback gating                                                                                                                                                                       |
| `app/native/src/modules/tiling/borders.rs`                                 | Modify (15C-3): `PAUSED` gate, `pause`/`resume`, drop queued commands                                                                                                                                                              |
| `app/native/src/modules/tiling/effects/animation/state.rs`                 | Modify (15C-3): `reset_transient_state`                                                                                                                                                                                            |
| `app/native/src/modules/tiling/effects/animation/mod.rs`, `effects/mod.rs` | Modify (15C-3): re-exports                                                                                                                                                                                                         |
| `app/native/src/lib.rs`                                                    | Modify (15B): bridge tiling startup through `dispatch_on_main_sync`                                                                                                                                                                |

---

## Task 9: Define the `LifecycleModule` trait

**Files:**

- Create: `app/native/src/modules/services/lifecycle.rs`
- Create: `app/native/src/modules/services/mod.rs`
- Modify: `app/native/src/modules/mod.rs` (add `pub mod services;`)

- [ ] **Step 1: Create the trait file**

`app/native/src/modules/services/lifecycle.rs` (new):

```rust
use std::fmt::Debug;

/// Runtime lifecycle state for a Stache module, surfaced in the tray menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleStatus {
    /// Disabled in persisted config. Shown in tray but locked (cannot be toggled at runtime).
    ConfiguredOff,
    /// Configured on and currently active.
    Running,
    /// Configured on but paused by the user via the tray. Resumes on restart/reload.
    Paused,
    /// Configured on but failed to start. `reason` explains why (e.g. missing permission).
    Unavailable(String),
}

/// Uniform contract so every module can be paused/resumed from the tray without
/// per-module special-casing. Implemented by each of the 6 toggleable modules.
pub trait LifecycleModule: Send + Sync {
    /// Human-readable module name (also used as the tray item label).
    fn name(&self) -> &'static str;
    /// Stable menu item id used to route `on_menu_event`.
    fn id(&self) -> &'static str;
    /// Start the module (called once at startup if configured on).
    fn start(&self) -> Result<(), String>;
    /// Pause the module, releasing/disabling its OS resources.
    fn pause(&self) -> Result<(), String>;
    /// Resume the module, re-acquiring OS resources.
    fn resume(&self) -> Result<(), String>;
    /// Current lifecycle status.
    fn status(&self) -> ModuleStatus;
}
```

- [ ] **Step 2: Expose the module**

`app/native/src/modules/services/mod.rs` (new):

```rust
pub mod lifecycle;
```

Then append one line to `app/native/src/modules/mod.rs` (next to the existing `pub mod` declarations):

```rust
pub mod services;
```

Without this parent declaration, `crate::modules::services::lifecycle` will not compile in later tasks.

- [ ] **Step 3: Build to confirm it compiles**

Run: `cargo build -p stache 2>&1 | tail -20`
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/services/lifecycle.rs app/native/src/modules/services/mod.rs app/native/src/modules/mod.rs
git commit -m "feat(lifecycle): add LifecycleModule trait and ModuleStatus"
```

---

## Task 15A: Repeatable completion contracts

**Files:**

- Modify: `app/native/src/modules/tiling/init.rs` — `CompletionLatch` + tests
- Modify: `app/native/src/modules/tiling/actor/mod.rs` — `spawn()` returns `(StateActorHandle, CompletionLatch)`, mark latch on loop exit
- Modify: `app/native/src/modules/tiling/effects/subscriber.rs` — completion latch on `run()` exit; `new()` returns triple
- Modify (all `StateActor::spawn()` callsites — see Step 4 table)

> **Visibility note:** Rust emits E0446 ("private type in public interface") when a `pub fn` returns a `pub(crate)` type. `CompletionLatch` stays `pub(crate)` per the approved contract, so `StateActor::spawn` and `EffectSubscriber::new` — which now return tuples containing it — become `pub(crate)`. Both are only called inside `modules/tiling` (production: `init.rs:202`, `init.rs:243`), so no public API is lost.

- [ ] **Step 1: Write the failing `CompletionLatch` tests**

In `app/native/src/modules/tiling/init.rs`, ensure `use std::time::Duration;` is imported (line 34 area), and append these tests to the existing `mod tests`:

```rust
    #[test]
    fn completion_latch_unmarked_times_out() {
        let latch = CompletionLatch::new();
        assert!(!latch.wait_timeout(Duration::from_millis(20)));
    }

    #[test]
    fn completion_latch_mark_complete_is_observable() {
        let latch = CompletionLatch::new();
        latch.mark_complete();
        assert!(latch.wait_timeout(Duration::from_millis(20)));
    }

    #[test]
    fn completion_latch_completion_persists_after_wait() {
        let latch = CompletionLatch::new();
        latch.mark_complete();
        assert!(latch.wait_timeout(Duration::from_millis(20)));
        assert!(latch.wait_timeout(Duration::from_millis(20)));
    }

    #[test]
    fn completion_latch_mark_complete_wakes_waiters() {
        let latch = CompletionLatch::new();
        let latch_for_thread = latch.clone();
        let thread = std::thread::spawn(move || {
            latch_for_thread.wait_timeout(Duration::from_secs(1));
        });
        std::thread::sleep(Duration::from_millis(10));
        latch.mark_complete();
        thread.join().unwrap();
    }

    #[test]
    fn completion_latch_repeatable_across_clones() {
        let latch = CompletionLatch::new();
        let clone = latch.clone();
        latch.mark_complete();
        assert!(clone.wait_timeout(Duration::from_millis(20)));
    }
```

- [ ] **Step 2: Run tests, verify RED**

Run: `cargo test -p stache --lib modules::tiling::init::tests 2>&1 | tail -20`
Expected: compile error — `cannot find type \`CompletionLatch\` in this scope`.

- [ ] **Step 3: Implement `CompletionLatch`**

In `app/native/src/modules/tiling/init.rs`, insert a new section directly after the `// Global State` block. It uses fully-qualified `std::sync` types so the 15B `parking_lot` import switch does not touch it:

```rust
// ============================================================================
// Repeatable Completion
// ============================================================================

/// Repeatable completion state: unlike a consumed one-shot receiver, retries
/// can observe that a resource has already stopped. All clones share the same
/// flag; completion stays observable after a timeout or a successful wait.
#[derive(Clone)]
pub(crate) struct CompletionLatch(Arc<(std::sync::Mutex<bool>, std::sync::Condvar)>);

impl CompletionLatch {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self(Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new())))
    }

    /// Marks the resource as complete and wakes all waiters.
    pub(crate) fn mark_complete(&self) {
        let (lock, cvar) = &*self.0;
        let mut done = lock.lock().unwrap_or_else(|e| e.into_inner());
        *done = true;
        cvar.notify_all();
    }

    /// Waits up to `timeout` for completion. Returns `true` when complete;
    /// completion remains observable on later calls after a timeout.
    #[must_use]
    pub(crate) fn wait_timeout(&self, timeout: Duration) -> bool {
        let (lock, cvar) = &*self.0;
        let mut done = lock.lock().unwrap_or_else(|e| e.into_inner());
        if !*done {
            let _ = cvar.wait_timeout(&mut done, timeout);
        }
        *done
    }
}
```

- [ ] **Step 4: Make `StateActor::spawn()` return a tuple and migrate every callsite**

In `app/native/src/modules/tiling/actor/mod.rs`, replace `spawn` (currently lines 55-75):

```rust
    /// Spawn a new state actor and return a handle for communication.
    ///
    /// The actor will run in the background and process messages. The returned
    /// latch completes only after the actor's message loop has fully exited.
    ///
    /// `pub(crate)` because the tuple exposes the crate-private
    /// `crate::modules::tiling::init::CompletionLatch`.
    #[must_use]
    pub(crate) fn spawn() -> (StateActorHandle, crate::modules::tiling::init::CompletionLatch) {
        tracing::debug!("tiling: spawning state actor");
        let (sender, receiver) = mpsc::channel(CHANNEL_BUFFER_SIZE);
        let stopped = crate::modules::tiling::init::CompletionLatch::new();
        let stopped_for_task = stopped.clone();

        let actor = Self {
            state: TilingState::new(),
            receiver,
        };

        // Spawn the actor task using Tauri's async runtime
        // This works during app setup when tokio runtime isn't directly available
        tauri::async_runtime::spawn(async move {
            actor.run().await;
            stopped_for_task.mark_complete();
        });

        (StateActorHandle::new(sender), stopped)
    }
```

Migrate **every** callsite. All use the identical line `let handle = StateActor::spawn();`; replace each with `let (handle, _stopped) = StateActor::spawn();`:

| File                                                     | Line                                        |
| -------------------------------------------------------- | ------------------------------------------- |
| `app/native/src/modules/tiling/init.rs`                  | 202 (production)                            |
| `app/native/src/modules/tiling/actor/mod.rs`             | 698, 710, 726, 742                          |
| `app/native/src/modules/tiling/events/processor.rs`      | 736, 748, 778, 802, 821, 838, 858, 872, 948 |
| `app/native/src/modules/tiling/events/app_monitor.rs`    | 332, 343                                    |
| `app/native/src/modules/tiling/events/ax_observer.rs`    | 770, 785                                    |
| `app/native/src/modules/tiling/events/screen_monitor.rs` | 341, 353, 368                               |

The `events/mod.rs:30` mention is inside a `rust,ignore` doc block — not a callsite; leave it.

- [ ] **Step 5: Add the `EffectSubscriber` completion API**

In `app/native/src/modules/tiling/effects/subscriber.rs`:

Add a `stopped` field to the struct (after `state: SubscriberState,`):

```rust
    /// Marked complete when the event loop exits (after `Shutdown` or channel close).
    stopped: crate::modules::tiling::init::CompletionLatch,
```

Replace `new` (currently lines 276-305):

```rust
    /// Creates a new effect subscriber.
    ///
    /// # Returns
    ///
    /// A tuple of (subscriber, handle, completion latch). The subscriber should
    /// be spawned as a background task; the handle used to send notifications;
    /// the latch completes only after the subscriber's loop exits.
    ///
    /// `pub(crate)` because the triple exposes the crate-private
    /// `crate::modules::tiling::init::CompletionLatch`.
    #[must_use]
    pub(crate) fn new(
        actor_handle: StateActorHandle,
        executor: EffectExecutor,
    ) -> (Self, EffectSubscriberHandle, crate::modules::tiling::init::CompletionLatch) {
        let (notification_tx, notification_rx) = mpsc::channel(256);
        let stopped = crate::modules::tiling::init::CompletionLatch::new();

        let subscriber = Self {
            actor_handle,
            executor,
            notification_rx,
            state: SubscriberState::new(),
            stopped: stopped.clone(),
        };

        let handle = EffectSubscriberHandle { notification_tx };

        (subscriber, handle, stopped)
    }
```

At the end of `run` (replace `tracing::debug!("Effect subscriber stopped");`):

```rust
        tracing::debug!("Effect subscriber stopped");
        self.stopped.mark_complete();
```

Update the production callsite `app/native/src/modules/tiling/init.rs:243`:

```rust
    let (subscriber, subscriber_handle, _subscriber_stopped) =
        EffectSubscriber::new(handle.clone(), executor);
```

- [ ] **Step 6: Build and run focused tests, verify GREEN**

```bash
cargo test -p stache --lib modules::tiling::init::tests
cargo test -p stache --lib modules::tiling::actor::tests
cargo test -p stache --lib modules::tiling::events::processor::tests
cargo test -p stache --lib modules::tiling::events::app_monitor::tests
cargo test -p stache --lib modules::tiling::events::ax_observer::tests
cargo test -p stache --lib modules::tiling::events::screen_monitor::tests
cargo test -p stache --lib modules::tiling::effects::subscriber::tests
cargo fmt --all -- --check
cargo check -p stache
cargo clippy -p stache --lib -- -D warnings
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/init.rs \
  app/native/src/modules/tiling/actor/mod.rs \
  app/native/src/modules/tiling/effects/subscriber.rs \
  app/native/src/modules/tiling/events/processor.rs \
  app/native/src/modules/tiling/events/app_monitor.rs \
  app/native/src/modules/tiling/events/ax_observer.rs \
  app/native/src/modules/tiling/events/screen_monitor.rs
git commit -m "feat(tiling): repeatable actor and subscriber completion latches"
```

---

## Task 15B: Restartable runtime slot, owned getters, staged factory, main-thread dispatch

**Files:**

- Modify: `app/native/src/modules/tiling/init.rs`
- Modify: `app/native/src/lib.rs`

> **Atomicity:** Steps 3-6 are one working-tree batch. Do not commit until Step 7 passes — an intermediate tree with the old globals removed but `init` not rewired does not compile.

- [ ] **Step 1: Write the failing runtime tests**

Append to `init.rs`'s `mod tests` (keep the 15A tests). Tests in this module observe/mutate the process-global `LIFECYCLE`/`RUNTIME`/`STOP_TIMEOUT_OVERRIDE` statics, which later tasks' tests (15D-3, 19F) also write — `cargo test` runs module tests in parallel. Introduce the shared serialization lock now and hold it in **every** test that touches the globals:

```rust
    /// Serializes tests that mutate the process-global lifecycle statics.
    static TEST_LIFECYCLE_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
```

```rust
    #[test]
    fn runtime_slot_is_empty_by_default() {
        let _guard = TEST_LIFECYCLE_LOCK.lock();
        assert!(matches!(&*RUNTIME.lock(), RuntimeSlot::Empty));
    }

    #[test]
    fn lifecycle_starts_stopped() {
        let _guard = TEST_LIFECYCLE_LOCK.lock();
        assert_eq!(*LIFECYCLE.lock(), LifecycleState::Stopped);
    }

    #[test]
    fn getters_return_none_before_start() {
        let _guard = TEST_LIFECYCLE_LOCK.lock();
        assert!(get_handle().is_none());
        assert!(get_processor().is_none());
        assert!(get_subscriber_handle().is_none());
        assert!(!is_initialized());
    }

    #[test]
    fn tiling_runtime_from_partial_requires_mandatory_fields() {
        let partial = PartialRuntime {
            generation: 1,
            actor: None,
            actor_stopped: None,
            processor: None,
            subscriber: None,
            subscriber_stopped: None,
            app_monitor: None,
            screen_monitor: None,
            ax_adapter: None,
            teardown: TeardownProgress::default(),
        };
        assert!(TilingRuntime::try_from(partial).is_err());
    }

    #[tokio::test]
    async fn runtime_factory_fails_named_stage_and_actor_exits_on_drop() {
        let failing = RuntimeFactory::new(Some(InitStage::Actor));
        assert!(failing.create_actor().is_err());

        let passing = RuntimeFactory::new(None);
        let (actor, stopped) = passing.create_actor().expect("actor stage must not fail");
        drop(actor);
        assert!(
            stopped.wait_timeout(Duration::from_secs(2)),
            "actor must stop after the last handle is dropped"
        );
    }
```

- [ ] **Step 2: Run tests, verify RED**

Run: `cargo test -p stache --lib modules::tiling::init::tests 2>&1 | tail -20`
Expected: compile errors — `RUNTIME`, `LIFECYCLE`, `RuntimeSlot`, `PartialRuntime`, `TeardownProgress`, `TilingRuntime`, `InitStage`, `RuntimeFactory` do not exist.

- [ ] **Step 3: Replace the globals with runtime-slot types and owned getters**

Change the import block (line 34) to:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
```

Replace the `// Global State` block (lines 46-63) with:

```rust
// ============================================================================
// Global State
// ============================================================================

/// Serialized lifecycle state guarding concurrent start/pause requests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleState {
    Stopped,
    Starting,
    Running,
    Stopping,
}

/// Named startup stages so a failure can be attributed and injected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InitStage {
    Actor,
    Processor,
    Subscriber,
    AppMonitor,
    ScreenMonitor,
    AxAdapter,
    StandaloneObservers,
    InitialState,
    MouseMonitor,
    Borders,
}

/// Owns every resource created so far during startup. Optional fields permit
/// rollback/quarantine after failure at any fatal `InitStage`.
#[allow(dead_code)] // adapter fields are consumed by the pause task (15D)
struct PartialRuntime {
    generation: u64,
    actor: Option<StateActorHandle>,
    actor_stopped: Option<CompletionLatch>,
    processor: Option<Arc<EventProcessor>>,
    subscriber: Option<EffectSubscriberHandle>,
    subscriber_stopped: Option<CompletionLatch>,
    app_monitor: Option<Arc<AppMonitorAdapter>>,
    screen_monitor: Option<Arc<ScreenMonitorAdapter>>,
    ax_adapter: Option<Arc<AXObserverAdapter>>,
    teardown: TeardownProgress,
}

/// Fully populated runtime required for a clean `Running` publish.
#[allow(dead_code)] // adapter fields are consumed by the pause task (15D)
struct TilingRuntime {
    generation: u64,
    actor: StateActorHandle,
    actor_stopped: CompletionLatch,
    processor: Arc<EventProcessor>,
    subscriber: EffectSubscriberHandle,
    subscriber_stopped: CompletionLatch,
    app_monitor: Arc<AppMonitorAdapter>,
    screen_monitor: Arc<ScreenMonitorAdapter>,
    ax_adapter: Arc<AXObserverAdapter>,
    teardown: TeardownProgress,
}

/// Per-stage teardown flags advanced by the runtime pause task.
#[allow(dead_code)] // advanced by the pause/retry task (15D)
#[derive(Default)]
struct TeardownProgress {
    visibility_restored: bool,
    main_thread_sources_removed: bool,
    processor_stopped: bool,
    transient_services_paused: bool,
    subscriber_stopped: bool,
    actor_stopped: bool,
    caches_cleared: bool,
}

/// Restartable runtime holder. `Quarantined` is produced by the pause/retry
/// task; a running runtime is the only state published here.
enum RuntimeSlot {
    Empty,
    Running(TilingRuntime),
    Quarantined(PartialRuntime),
}

impl TryFrom<PartialRuntime> for TilingRuntime {
    type Error = String;

    fn try_from(partial: PartialRuntime) -> Result<Self, String> {
        Ok(Self {
            generation: partial.generation,
            actor: partial.actor.ok_or("tiling: actor missing from runtime")?,
            actor_stopped: partial.actor_stopped.ok_or("tiling: actor completion latch missing")?,
            processor: partial.processor.ok_or("tiling: processor missing from runtime")?,
            subscriber: partial.subscriber.ok_or("tiling: subscriber missing from runtime")?,
            subscriber_stopped: partial
                .subscriber_stopped
                .ok_or("tiling: subscriber completion latch missing")?,
            app_monitor: partial.app_monitor.ok_or("tiling: app monitor missing from runtime")?,
            screen_monitor: partial
                .screen_monitor
                .ok_or("tiling: screen monitor missing from runtime")?,
            ax_adapter: partial.ax_adapter.ok_or("tiling: ax adapter missing from runtime")?,
            teardown: partial.teardown,
        })
    }
}

static LIFECYCLE: Mutex<LifecycleState> = Mutex::new(LifecycleState::Stopped);
static RUNTIME: Mutex<RuntimeSlot> = Mutex::new(RuntimeSlot::Empty);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
const RUNTIME_STOP_TIMEOUT: Duration = Duration::from_secs(2);
```

`APP_HANDLE` (line 60) now uses `parking_lot::Mutex`:

```rust
static APP_HANDLE: Mutex<Option<tauri::AppHandle>> = Mutex::new(None);
```

Replace the `Public API` getters (lines 69-97) — these return **owned** clones, never a `'static` reference into the mutex:

```rust
/// Gets the state actor handle for the current running runtime.
///
/// Returns `None` if the tiling runtime is not currently `Running`.
#[must_use]
pub fn get_handle() -> Option<StateActorHandle> {
    match &*RUNTIME.lock() {
        RuntimeSlot::Running(rt) => Some(rt.actor.clone()),
        RuntimeSlot::Empty | RuntimeSlot::Quarantined(_) => None,
    }
}

/// Gets the current event processor.
///
/// Returns `None` if the tiling runtime is not currently `Running`.
#[must_use]
pub fn get_processor() -> Option<Arc<EventProcessor>> {
    match &*RUNTIME.lock() {
        RuntimeSlot::Running(rt) => Some(Arc::clone(&rt.processor)),
        RuntimeSlot::Empty | RuntimeSlot::Quarantined(_) => None,
    }
}

/// Gets the current effect subscriber handle.
///
/// Returns `None` if the tiling runtime is not currently `Running`.
#[must_use]
pub fn get_subscriber_handle() -> Option<EffectSubscriberHandle> {
    match &*RUNTIME.lock() {
        RuntimeSlot::Running(rt) => Some(rt.subscriber.clone()),
        RuntimeSlot::Empty | RuntimeSlot::Quarantined(_) => None,
    }
}

/// Returns whether the tiling runtime is currently `Running`.
#[must_use]
pub fn is_initialized() -> bool { *LIFECYCLE.lock() == LifecycleState::Running }
```

Replace `store_app_handle`/`get_app_handle` (parking_lot guards return directly, no `Result`):

```rust
/// Stores the Tauri app handle for later use in event emission.
pub fn store_app_handle(handle: tauri::AppHandle) {
    *APP_HANDLE.lock() = Some(handle);
}

/// Gets the stored app handle.
#[must_use]
pub fn get_app_handle() -> Option<tauri::AppHandle> { APP_HANDLE.lock().clone() }
```

All other `get_handle()` callers (`bar/ipc_listener.rs`, `bar/components/tiling.rs`, internal `init.rs` IPC handlers) use the handle inline via `send`/`query`/`notify_*` with `&self` receivers, so the owned return type is source-compatible. The one exception is `init.rs::on_mouse_up` (lines 1179-1187): pass `&handle`:

```rust
    // Process the completed operation
    match info.operation {
        DragOperation::Move => handle_move_finished(&info, &handle),
        DragOperation::Resize => handle_resize_finished(&info, &handle),
    }
```

- [ ] **Step 4: Add the staged startup factory, `start_runtime`, `pause_runtime`; rewrite `init`/`shutdown`**

Insert a new section after the `Public API` getters:

```rust
// ============================================================================
// Staged Startup Factory
// ============================================================================

/// Staged startup factory: each method maps to one `InitStage`, lets tests fail
/// a named stage, and routes AppKit/AX work through the synchronous
/// main-thread helper.
struct RuntimeFactory {
    fail_stage: Option<InitStage>,
}

impl RuntimeFactory {
    fn new(fail_stage: Option<InitStage>) -> Self { Self { fail_stage } }

    fn fail(&self, stage: InitStage) -> Result<(), String> {
        if self.fail_stage == Some(stage) {
            return Err(format!("tiling: injected startup failure at {stage:?}"));
        }
        Ok(())
    }

    fn create_actor(&self) -> Result<(StateActorHandle, CompletionLatch), String> {
        self.fail(InitStage::Actor)?;
        Ok(StateActor::spawn())
    }

    fn create_processor(&self, actor: StateActorHandle) -> Result<Arc<EventProcessor>, String> {
        self.fail(InitStage::Processor)?;
        let processor = Arc::new(EventProcessor::new(actor));
        processor.start();
        Ok(processor)
    }

    fn create_subscriber(
        &self,
        actor: StateActorHandle,
    ) -> Result<(EffectSubscriberHandle, CompletionLatch), String> {
        self.fail(InitStage::Subscriber)?;
        let mut executor =
            get_app_handle().map_or_else(EffectExecutor::new, EffectExecutor::with_app_handle);
        if get_config().tiling.borders.is_enabled() {
            executor.set_borders_enabled(true);
        }
        let (subscriber, subscriber_handle, stopped) = EffectSubscriber::new(actor, executor);
        tauri::async_runtime::spawn(subscriber.run());
        Ok((subscriber_handle, stopped))
    }

    fn create_app_monitor(
        &self,
        processor: Arc<EventProcessor>,
    ) -> Result<Arc<AppMonitorAdapter>, String> {
        self.fail(InitStage::AppMonitor)?;
        Ok(crate::platform::thread::dispatch_on_main_sync(move || {
            let app_monitor = Arc::new(AppMonitorAdapter::new(processor));
            if !app_monitor.init() {
                tracing::warn!("tiling: app monitor initialization failed");
            }
            super::events::app_monitor::install_adapter(Arc::clone(&app_monitor));
            app_monitor
        }))
    }

    fn create_screen_monitor(
        &self,
        processor: Arc<EventProcessor>,
    ) -> Result<Arc<ScreenMonitorAdapter>, String> {
        self.fail(InitStage::ScreenMonitor)?;
        Ok(crate::platform::thread::dispatch_on_main_sync(move || {
            let screen_monitor = Arc::new(ScreenMonitorAdapter::new(processor));
            if !screen_monitor.init() {
                tracing::warn!("tiling: screen monitor initialization failed");
            }
            super::events::screen_monitor::install_adapter(Arc::clone(&screen_monitor));
            screen_monitor
        }))
    }

    fn create_ax_adapter(
        &self,
        processor: Arc<EventProcessor>,
    ) -> Result<Arc<AXObserverAdapter>, String> {
        self.fail(InitStage::AxAdapter)?;
        Ok(crate::platform::thread::dispatch_on_main_sync(move || {
            let ax_adapter = Arc::new(super::events::AXObserverAdapter::new(processor));
            super::events::ax_observer::install_adapter(Arc::clone(&ax_adapter));
            ax_adapter.activate();
            ax_adapter
        }))
    }

    fn setup_standalone_observers(&self) -> Result<(), String> {
        self.fail(InitStage::StandaloneObservers)?;
        crate::platform::thread::dispatch_on_main_sync(|| {
            if super::events::observer::init() {
                tracing::debug!("tiling: AXObserver initialized");
                Ok(())
            } else {
                Err("tiling: AXObserver initialization failed".to_string())
            }
        })
    }

    fn setup_mouse_monitor(&self) -> Result<(), String> {
        self.fail(InitStage::MouseMonitor)?;
        if super::events::mouse_monitor::init() {
            super::events::mouse_monitor::set_mouse_up_callback(on_mouse_up);
            Ok(())
        } else {
            Err("tiling: mouse monitor initialization failed".to_string())
        }
    }

    fn setup_borders(&self) -> Result<(), String> {
        self.fail(InitStage::Borders)?;
        if borders::init() {
            Ok(())
        } else {
            Err("tiling: borders initialization failed".to_string())
        }
    }

    fn enumerate_initial_state(
        &self,
        actor: StateActorHandle,
        processor: Arc<EventProcessor>,
    ) -> Result<(), String> {
        self.fail(InitStage::InitialState)?;
        crate::platform::thread::dispatch_on_main_sync(move || {
            initialize_state(&actor, &processor);
        });
        Ok(())
    }
}
```

Build orchestration and lifecycle entry points (replace `init_internal`, `init`, `shutdown`; lines 122-317):

```rust
/// Builds a full runtime for one generation, assigning every fatal stage to
/// `PartialRuntime` as it succeeds. A failure at any fatal stage drops the
/// partial (rollback by drop: closed channels stop the actor and subscriber,
/// and `EventProcessor::drop` stops its timers) and returns the error.
fn build_runtime(factory: &RuntimeFactory, generation: u64) -> Result<TilingRuntime, String> {
    let mut partial = PartialRuntime {
        generation,
        actor: None,
        actor_stopped: None,
        processor: None,
        subscriber: None,
        subscriber_stopped: None,
        app_monitor: None,
        screen_monitor: None,
        ax_adapter: None,
        teardown: TeardownProgress::default(),
    };

    let (actor, actor_stopped) = factory.create_actor()?;
    partial.actor = Some(actor.clone());
    partial.actor_stopped = Some(actor_stopped);

    let processor = factory.create_processor(actor.clone())?;
    partial.processor = Some(Arc::clone(&processor));

    let (subscriber_handle, subscriber_stopped) = factory.create_subscriber(actor.clone())?;
    partial.subscriber = Some(subscriber_handle);
    partial.subscriber_stopped = Some(subscriber_stopped);

    let app_monitor = factory.create_app_monitor(Arc::clone(&processor))?;
    partial.app_monitor = Some(app_monitor);

    let screen_monitor = factory.create_screen_monitor(Arc::clone(&processor))?;
    partial.screen_monitor = Some(screen_monitor);

    let ax_adapter = factory.create_ax_adapter(Arc::clone(&processor))?;
    partial.ax_adapter = Some(ax_adapter);

    factory.setup_standalone_observers()?;

    // Optional stages: failure logs degraded mode but still permits Running.
    if let Err(e) = factory.setup_mouse_monitor() {
        tracing::warn!("{e}");
    }
    if let Err(e) = factory.setup_borders() {
        tracing::warn!("{e}");
    }

    factory.enumerate_initial_state(actor, processor)?;

    TilingRuntime::try_from(partial)
}

/// Starts a fresh runtime generation and publishes `RuntimeSlot::Running`.
///
/// The handle is stored for event emission (idempotent with `init`). No
/// lifecycle or runtime lock is held across the staged factory calls, which
/// dispatch to the main thread.
pub fn start_runtime(app_handle: tauri::AppHandle) -> Result<(), String> {
    store_app_handle(app_handle);

    {
        let mut lifecycle = LIFECYCLE.lock();
        if *lifecycle != LifecycleState::Stopped {
            return Err(format!("tiling: cannot start while {lifecycle:?}"));
        }
        *lifecycle = LifecycleState::Starting;
    }

    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    let factory = RuntimeFactory::new(None);

    match build_runtime(&factory, generation) {
        Ok(runtime) => {
            *RUNTIME.lock() = RuntimeSlot::Running(runtime);
            *LIFECYCLE.lock() = LifecycleState::Running;
            tracing::info!("tiling: runtime {generation} started");
            Ok(())
        }
        Err(e) => {
            *LIFECYCLE.lock() = LifecycleState::Stopped;
            Err(e)
        }
    }
}

/// Stops the published runtime and publishes `Empty`/`Stopped`.
///
/// Ordered teardown, quarantine/retry, and main-thread observer unregister are
/// owned by the pause task (15D); this step waits on the subscriber/actor
/// latches so a 15B-only tree is still shippable.
fn pause_runtime() -> Result<(), String> {
    {
        let mut lifecycle = LIFECYCLE.lock();
        if *lifecycle != LifecycleState::Running {
            return Ok(());
        }
        *lifecycle = LifecycleState::Stopping;
    }

    let runtime = {
        let mut slot = RUNTIME.lock();
        match std::mem::replace(&mut *slot, RuntimeSlot::Empty) {
            RuntimeSlot::Running(rt) => rt,
            RuntimeSlot::Empty => {
                *LIFECYCLE.lock() = LifecycleState::Stopped;
                return Ok(());
            }
            RuntimeSlot::Quarantined(partial) => {
                *slot = RuntimeSlot::Quarantined(partial);
                *LIFECYCLE.lock() = LifecycleState::Stopped;
                return Ok(());
            }
        }
    };

    runtime.processor.stop();

    runtime.subscriber.shutdown();
    if !runtime.subscriber_stopped.wait_timeout(RUNTIME_STOP_TIMEOUT) {
        tracing::warn!("tiling: subscriber did not stop within {RUNTIME_STOP_TIMEOUT:?}");
    }

    let _ = runtime.actor.shutdown();
    if !runtime.actor_stopped.wait_timeout(RUNTIME_STOP_TIMEOUT) {
        tracing::warn!("tiling: actor did not stop within {RUNTIME_STOP_TIMEOUT:?}");
    }

    let generation = runtime.generation;
    drop(runtime);
    *LIFECYCLE.lock() = LifecycleState::Stopped;
    tracing::info!("tiling: runtime {generation} stopped");
    Ok(())
}

/// Initializes the `tiling` window manager.
///
/// Gates on config and accessibility, stores the app handle, then starts a
/// fresh runtime generation.
#[allow(clippy::needless_pass_by_value)] // AppHandle is intentionally passed by value for storage
pub fn init(app_handle: tauri::AppHandle) -> bool {
    if is_initialized() {
        tracing::warn!("tiling: already initialized");
        return false;
    }

    let config = get_config();
    tracing::debug!("tiling: enabled = {}", config.tiling.is_enabled());

    if !config.tiling.is_enabled() {
        tracing::info!("tiling: disabled in config (set enabled=true to enable)");
        return false;
    }

    tracing::debug!("tiling: accessibility_granted = {}", is_accessibility_granted());
    if !is_accessibility_granted() {
        tracing::warn!("tiling: accessibility permissions not granted");
        return false;
    }

    match start_runtime(app_handle.clone()) {
        Ok(()) => {
            tracing::info!("tiling: initialized successfully");
            if let Err(e) = app_handle.emit(
                events::tiling::INITIALIZED,
                serde_json::json!({ "enabled": true, "version": "v2" }),
            ) {
                tracing::warn!("tiling: failed to emit initialized event: {e}");
            }
            true
        }
        Err(e) => {
            tracing::error!("tiling: initialization failed: {e}");
            false
        }
    }
}

/// Shuts down the tiling system.
pub fn shutdown() {
    if let Err(e) = pause_runtime() {
        tracing::error!("tiling: shutdown failed: {e}");
    }
}
```

Delete the old `init_internal` function (former lines 199-317).

- [ ] **Step 5: Thread the processor through initial state enumeration**

Change `initialize_state` (line 323) and `track_existing_windows` (line 360):

```rust
fn initialize_state(handle: &StateActorHandle, processor: &EventProcessor) {
    // Detect screens on the main thread (this is called during Tauri setup)
    // NSScreen APIs must be called from the main thread
    tracing::debug!("tiling: detecting screens on main thread...");
    let screens = super::actor::handlers::get_screens_from_macos();

    if screens.is_empty() {
        tracing::warn!("tiling: no screens detected during initialization");
        return;
    }

    tracing::debug!(
        "tiling: detected {} screen(s): {:?}",
        screens.len(),
        screens.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Send pre-detected screens to the actor
    // This avoids calling macOS APIs from the async actor task
    if let Err(e) = handle.send(StateMessage::SetScreens { screens }) {
        tracing::error!("tiling: failed to send SetScreens message: {e}");
    }

    // Track existing windows
    track_existing_windows(handle, processor);
}
```

```rust
fn track_existing_windows(handle: &StateActorHandle, processor: &EventProcessor) {
```

and replace the `get_processor()` block inside `track_existing_windows` (former lines 442-446) with:

```rust
    // Also track these windows in the event processor for destroy detection
    // This is necessary because BatchWindowsCreated bypasses the processor
    let window_pids: Vec<(u32, i32)> =
        window_infos.iter().map(|w| (w.window_id, w.pid)).collect();
    processor.track_windows_for_destroy_detection(&window_pids);
```

- [ ] **Step 6: Bridge tiling startup through the main thread in `lib.rs`**

Replace `app/native/src/lib.rs` lines 130-135:

```rust
        // Initialize tiling window manager if enabled (after other modules)
        if tiling_config.is_enabled() {
            tracing::info!("tiling window manager enabled, initializing");
            let h = handle.clone();
            // AppKit/AX stages inside start_runtime must run on the main thread;
            // dispatch_on_main_sync executes inline when already on it.
            crate::platform::thread::dispatch_on_main_sync(move || tiling::init(h));
            tracing::debug!("tiling initialization complete");
        }
```

- [ ] **Step 7: Build and run focused tests, verify GREEN**

```bash
cargo test -p stache --lib modules::tiling::init::tests
cargo test -p stache --lib modules::tiling::actor::tests
cargo test -p stache --lib modules::tiling::events::processor::tests
cargo test -p stache --lib modules::tiling::effects::subscriber::tests
cargo fmt --all -- --check
cargo check -p stache
cargo clippy -p stache --lib -- -D warnings
```

Expected: all pass, including the five new tests. Note `is_initialized()` now returns false while `Starting`/`Stopping` — a new transient window for IPC listeners (`bar/ipc_listener.rs`, `commands.rs`) that report "not initialized"; acceptable and intended, call it out in the commit message.

- [ ] **Step 8: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/init.rs app/native/src/lib.rs
git commit -m "feat(tiling): restartable runtime slot with staged startup factory"
```

---

## Task 15C-1: `EventProcessor::stop_and_wait` with timer completion and map clearing

**Files:**

- Modify: `app/native/src/modules/tiling/events/processor.rs`

- [ ] **Step 1: Add the timer-completion primitive and the test-count helper**

After the refresh-rate constants, before `ScreenBatch`:

```rust
/// Repeatable, resettable completion signal used to acknowledge that a screen
/// batch timer task has fully exited its loop.
#[derive(Default)]
struct TimerCompletion {
    done: std::sync::Mutex<bool>,
    cv: std::sync::Condvar,
}

impl TimerCompletion {
    fn reset(&self) {
        *self.done.lock().unwrap_or_else(|e| e.into_inner()) = false;
    }

    fn mark(&self) {
        *self.done.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.cv.notify_all();
    }

    fn wait(&self, timeout: Duration) -> bool {
        let mut done = self.done.lock().unwrap_or_else(|e| e.into_inner());
        if !*done {
            let _ = self.cv.wait_timeout(&mut done, timeout);
        }
        *done
    }
}
```

Add a test-only helper on `EventProcessor` (used by the new tests; mark `#[cfg(test)]`):

```rust
    /// Sum of pending geometry updates across all screen batches.
    #[cfg(test)]
    fn pending_geometry_count(&self) -> usize {
        self.screen_batches.lock().values().map(|b| b.updates.len()).sum()
    }
```

- [ ] **Step 2: Add `timer_running` + `timer_done` to `ScreenBatch`**

In `struct ScreenBatch`, add fields; initialize in its constructor:

```rust
    /// Whether the timer for this screen is running.
    timer_running: AtomicBool,

    /// Completion acknowledgment for the currently running timer task.
    timer_done: Arc<TimerCompletion>,
```

```rust
    fn new(screen_id: u32, refresh_rate: f64) -> Self {
        Self {
            screen_id,
            refresh_rate: refresh_rate.clamp(MIN_REFRESH_RATE, MAX_REFRESH_RATE),
            updates: HashMap::new(),
            timer_running: AtomicBool::new(false),
            timer_done: Arc::new(TimerCompletion::default()),
        }
    }
```

- [ ] **Step 3: Mark completion at the end of the timer task**

In `start_screen_timer`, claim the timer and reset the completion before spawning:

```rust
        let interval = {
            let batches = batches.lock();
            match batches.get(&screen_id) {
                Some(batch) => {
                    if batch
                        .timer_running
                        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                        .is_err()
                    {
                        return; // Timer already running
                    }
                    batch.timer_done.reset();
                    batch.batch_interval()
                }
                None => return,
            }
        };
```

At the end of the spawned task, after the timer is marked not-running, mark completion:

```rust
            if let Some(batch) = batches.lock().get(&screen_id) {
                batch.timer_running.store(false, Ordering::SeqCst);
            }
            let timer_done = batches.lock().get(&screen_id).map(|b| Arc::clone(&b.timer_done));
            if let Some(timer_done) = timer_done {
                timer_done.mark();
            }

            tracing::trace!("Batch timer stopped for screen {screen_id}");
```

- [ ] **Step 4: Add `stop_and_wait` and routing-map clearing; update `start`**

`stop_and_wait` measures deadlines with `Instant`, so extend the existing import (processor.rs:19) to `use std::time::{Duration, Instant};`.

```rust
    /// Stop the batch flush timers.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);

        {
            let batches = self.screen_batches.lock();
            for batch in batches.values() {
                batch.timer_running.store(false, Ordering::SeqCst);
            }
        }

        tracing::debug!("EventProcessor stopped");
    }

    /// Atomically reject new events, stop all timers, wait for each timer
    /// task's completion acknowledgment, then clear all pending batches and
    /// routing maps.
    ///
    /// Returns `false` if any timer task failed to acknowledge within
    /// `timeout`; routing maps are still cleared in that case.
    pub fn stop_and_wait(&self, timeout: Duration) -> bool {
        self.running.store(false, Ordering::SeqCst);

        let completions: Vec<Arc<TimerCompletion>> = {
            let mut batches = self.screen_batches.lock();
            batches
                .values_mut()
                .filter(|batch| batch.timer_running.swap(false, Ordering::SeqCst))
                .map(|batch| Arc::clone(&batch.timer_done))
                .collect()
        };

        let deadline = Instant::now() + timeout;
        let all_done = completions.iter().all(|completion| {
            let remaining = deadline.saturating_duration_since(Instant::now());
            completion.wait(remaining)
        });

        if !all_done {
            tracing::warn!("tiling: EventProcessor timers did not stop within {timeout:?}");
        }

        self.clear_routing_maps();
        all_done
    }

    /// Discard pending geometry updates and all routing/tracking maps.
    ///
    /// Idempotent: safe to call when already stopped or on retry.
    fn clear_routing_maps(&self) {
        {
            let mut batches = self.screen_batches.lock();
            for batch in batches.values_mut() {
                batch.updates.clear();
            }
        }
        self.window_screen_map.clear();
        self.pid_windows.lock().clear();
    }

    /// Start the batch flush timers for all registered screens.
    pub fn start(&self) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            tracing::warn!("EventProcessor already running");
            return;
        }

        // Discard any pending updates and routes from a prior generation.
        self.clear_routing_maps();

        let screen_ids: Vec<u32> = self.screen_batches.lock().keys().copied().collect();
        for screen_id in screen_ids {
            self.start_screen_timer(screen_id);
        }

        tracing::debug!("EventProcessor started");
    }
```

- [ ] **Step 5: Write tests for `stop_and_wait`**

Append to the tests module in `processor.rs`:

```rust
    #[tokio::test]
    async fn test_stop_and_wait_clears_routes_and_batches() {
        let (handle, _stopped) = StateActor::spawn();
        let processor = EventProcessor::new(handle.clone());

        processor.register_screen(1, 60.0);
        processor.set_window_screen(100, 1);
        processor.on_window_moved(100, Rect::new(0.0, 0.0, 100.0, 100.0));
        processor.start();
        assert!(processor.is_running());
        assert_eq!(processor.pending_geometry_count(), 1);

        let stopped = processor.stop_and_wait(Duration::from_secs(1));
        assert!(stopped, "timer should acknowledge completion");
        assert!(!processor.is_running());
        assert_eq!(processor.pending_geometry_count(), 0);
        assert_eq!(processor.window_screen_map.len(), 0);
        assert_eq!(processor.pid_windows.lock().len(), 0);

        handle.shutdown().unwrap();
    }

    #[tokio::test]
    async fn test_stop_and_wait_when_not_running_is_idempotent() {
        let (handle, _stopped) = StateActor::spawn();
        let processor = EventProcessor::new(handle.clone());

        processor.register_screen(1, 60.0);
        processor.set_window_screen(100, 1);
        processor.on_window_moved(100, Rect::new(0.0, 0.0, 100.0, 100.0));

        // Never started: no timer tasks exist, so nothing to wait on.
        let stopped = processor.stop_and_wait(Duration::from_millis(100));
        assert!(stopped);
        assert_eq!(processor.pending_geometry_count(), 0);
        assert_eq!(processor.window_screen_map.len(), 0);

        handle.shutdown().unwrap();
    }

    #[tokio::test]
    async fn test_start_discards_stale_routes() {
        let (handle, _stopped) = StateActor::spawn();
        let processor = EventProcessor::new(handle.clone());

        processor.register_screen(1, 60.0);
        processor.set_window_screen(100, 1);
        processor.on_window_moved(100, Rect::new(0.0, 0.0, 100.0, 100.0));

        let _ = processor.stop_and_wait(Duration::from_secs(1));
        assert_eq!(processor.pending_geometry_count(), 0);

        // Restart must not resurrect the stale move.
        processor.start();
        assert_eq!(processor.pending_geometry_count(), 0);
        assert_eq!(processor.window_screen_map.len(), 0);

        handle.shutdown().unwrap();
    }
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p stache --lib modules::tiling::events::processor::tests
```

Expected: all pass, including the three new tests and the existing suite.

- [ ] **Step 7: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/events/processor.rs
git commit -m "feat(tiling): EventProcessor stop_and_wait with timer completion"
```

---

## Task 15C-2: App/screen/AX/standalone observer generation gating and removal

**Files:**

- Modify: `app/native/src/modules/tiling/events/app_monitor.rs`
- Modify: `app/native/src/modules/tiling/events/screen_monitor.rs`
- Modify: `app/native/src/modules/tiling/events/ax_observer.rs`
- Modify: `app/native/src/modules/tiling/events/observer.rs`

- [ ] **Step 1: Gate `AppMonitorAdapter`, retain its observer, add `shutdown`**

Change the struct and constructor:

```rust
pub struct AppMonitorAdapter {
    /// Reference to the event processor.
    processor: Arc<EventProcessor>,

    /// Generation this adapter belongs to (from `init::current_generation()`).
    generation: u64,

    /// Whether the adapter is initialized (observer registered).
    initialized: AtomicBool,

    /// Whether callbacks should be forwarded.
    active: AtomicBool,

    /// Retained `NSWorkspace` observer object (`*mut Object` as `usize`).
    /// Only touched on the main thread.
    observer: std::sync::Mutex<Option<usize>>,

    /// Retained workspace notification center (`*mut Object` as `usize`).
    notification_center: std::sync::Mutex<Option<usize>>,
}
```

Add `fn current_generation()` in `init.rs` (next to the getters) so adapters can read it:

```rust
/// Returns the current runtime generation counter value.
#[must_use]
pub fn current_generation() -> u64 { NEXT_GENERATION.load(Ordering::Relaxed) }
```

Constructor:

```rust
impl AppMonitorAdapter {
    /// Creates a new adapter with the given event processor.
    #[must_use]
    pub fn new(processor: Arc<EventProcessor>) -> Self {
        Self {
            processor,
            generation: crate::modules::tiling::init::current_generation(),
            initialized: AtomicBool::new(false),
            active: AtomicBool::new(false),
            observer: std::sync::Mutex::new(None),
            notification_center: std::sync::Mutex::new(None),
        }
    }

    /// Whether this adapter's callbacks may currently be forwarded.
    fn gate_open(&self) -> bool {
        self.active.load(Ordering::SeqCst)
            && self.generation == crate::modules::tiling::init::current_generation()
    }
}
```

In `init()`: after the observer object is created and non-null, retain the pointers; at the end set `active`:

```rust
            *self.observer.lock().unwrap() = Some(observer as usize);
            *self.notification_center.lock().unwrap() = Some(notification_center as usize);
```

```rust
        self.active.store(true, Ordering::SeqCst);
        tracing::debug!("AppMonitorAdapter initialized");
        true
```

Add `shutdown` (must run on the main thread; idempotent) and `uninstall_adapter` beside the existing `install_adapter` (clears the same global slot; add a `#[cfg(test)] fn get_installed_adapter() -> Option<Arc<AppMonitorAdapter>>` if tests need it):

```rust
    /// Removes the NSWorkspace observer and resets the adapter.
    ///
    /// Must be called on the main thread. Idempotent.
    pub fn shutdown(&self) {
        self.active.store(false, Ordering::SeqCst);
        if !self.initialized.swap(false, Ordering::SeqCst) {
            return;
        }

        let observer = *self.observer.lock().unwrap();
        let notification_center = *self.notification_center.lock().unwrap();

        if let (Some(observer), Some(notification_center)) = (observer, notification_center) {
            unsafe {
                let observer: *mut Object = observer as *mut Object;
                let notification_center: *mut Object = notification_center as *mut Object;

                let launch = nsstring("NSWorkspaceDidLaunchApplicationNotification");
                let _: () = msg_send![
                    notification_center,
                    removeObserver: observer
                    name: launch
                    object: std::ptr::null::<Object>()
                ];

                let terminate = nsstring("NSWorkspaceDidTerminateApplicationNotification");
                let _: () = msg_send![
                    notification_center,
                    removeObserver: observer
                    name: terminate
                    object: std::ptr::null::<Object>()
                ];

                let _: () = msg_send![observer, release];
            }
        }

        *self.observer.lock().unwrap() = None;
        *self.notification_center.lock().unwrap() = None;

        super::app_monitor::uninstall_adapter();
        tracing::debug!("AppMonitorAdapter shut down");
    }
```

Gate the handlers — first line of `on_app_launched` and `on_app_terminated`:

```rust
        if !self.gate_open() {
            return;
        }
```

- [ ] **Step 2: Gate `ScreenMonitorAdapter`, add `shutdown` + main-thread re-dispatch**

Change the struct/constructor (add `generation`, `active`), add `gate_open`, and add the removal FFI:

```rust
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayRegisterReconfigurationCallback(
        callback: unsafe extern "C" fn(u32, u32, *mut c_void),
        user_info: *mut c_void,
    ) -> i32;
    fn CGDisplayRemoveReconfigurationCallback(
        callback: unsafe extern "C" fn(u32, u32, *mut c_void),
        user_info: *mut c_void,
    ) -> i32;
}
```

```rust
    /// Removes the CoreGraphics reconfiguration callback and invalidates any
    /// in-flight delayed worker by bumping the gate. Idempotent.
    pub fn shutdown(&self) {
        self.active.store(false, Ordering::SeqCst);
        if !self.initialized.swap(false, Ordering::SeqCst) {
            return;
        }
        unsafe {
            CGDisplayRemoveReconfigurationCallback(
                display_reconfiguration_callback,
                std::ptr::null_mut(),
            );
        }
        self.processing.store(false, Ordering::SeqCst);
        super::screen_monitor::uninstall_adapter();
        tracing::debug!("ScreenMonitorAdapter shut down");
    }
```

In `init()`, set `active` after successful registration:

```rust
        self.active.store(true, Ordering::SeqCst);
        // Register current screens with the processor
        self.register_all_screens();
```

Rewrite `on_screens_changed` so no AppKit call runs on a worker thread and the generation is re-checked after the main-thread hop:

```rust
    fn on_screens_changed(&self) {
        self.set_processing(true);

        if !self.gate_open() {
            self.set_processing(false);
            return;
        }

        let screens = crate::platform::thread::dispatch_on_main_sync(|| {
            crate::modules::tiling::actor::handlers::get_screens_from_macos()
        });

        // Re-check after the main-thread hop: a pause may have torn us down.
        if !self.gate_open() {
            self.set_processing(false);
            return;
        }

        for display_id in get_all_display_ids() {
            let refresh_rate = get_display_refresh_rate(display_id);
            self.processor.register_screen(display_id, refresh_rate);
        }
        self.processor.on_set_screens(screens);
        self.set_processing(false);
    }
```

- [ ] **Step 3: Gate `AXObserverAdapter` and add `shutdown`**

Add `generation` + `active` fields, `gate_open`, and:

```rust
    /// Deactivates the adapter and uninstalls it from the global callback
    /// dispatch slot. Idempotent.
    pub fn shutdown(&self) {
        self.deactivate();
        super::ax_observer::uninstall_adapter();
        tracing::debug!("AXObserverAdapter shut down");
    }
```

In `handle_event`, replace the active check:

```rust
    pub fn handle_event(&self, event: WindowEvent) {
        if !self.gate_open() {
            tracing::trace!("Ignoring event {:?} - adapter not active", event.event_type);
            return;
        }
```

- [ ] **Step 4: Add standalone observer `shutdown`**

In `observer.rs`, add after `init()`:

```rust
/// Removes and releases every registered `AXObserver` and resets the system
/// so it can be re-initialized on a fresh resume.
///
/// # Safety
///
/// This function must be called from the main thread.
pub fn shutdown() {
    if !INITIALIZED.swap(false, Ordering::SeqCst) {
        return;
    }

    let mut state_guard = OBSERVER_STATE.lock();
    if let Some(mut state) = state_guard.take() {
        for (pid, observer) in state.observers.drain() {
            unsafe { CFRelease(observer.0.cast()) };
            tracing::trace!("Released standalone observer for pid {pid}");
        }
    }

    tracing::debug!("tiling: standalone AX observer system shut down");
}
```

> Note: Task 19B-2 (`app-identity-restoration.md`) later rewrites `ObserverState` to be address-keyed; keep `shutdown()` draining whatever map shape is current at that point.

- [ ] **Step 5: Write gating/shutdown tests**

Append to `app_monitor.rs` tests:

```rust
    #[tokio::test]
    async fn test_gate_requires_active() {
        let (handle, _stopped) = StateActor::spawn();
        let processor = Arc::new(EventProcessor::new(handle.clone()));
        let adapter = AppMonitorAdapter::new(processor);

        assert!(!adapter.gate_open(), "inactive adapter must not forward");
        adapter.active.store(true, Ordering::SeqCst);
        assert!(adapter.gate_open(), "active+current generation forwards");
        adapter.active.store(false, Ordering::SeqCst);
        assert!(!adapter.gate_open());

        handle.shutdown().unwrap();
    }

    #[tokio::test]
    async fn test_shutdown_is_idempotent_and_deactivates() {
        let (handle, _stopped) = StateActor::spawn();
        let processor = Arc::new(EventProcessor::new(handle.clone()));
        let adapter = Arc::new(AppMonitorAdapter::new(processor));

        install_adapter(Arc::clone(&adapter));
        adapter.active.store(true, Ordering::SeqCst);

        adapter.shutdown();
        assert!(!adapter.is_initialized());
        assert!(!adapter.gate_open());
        assert!(get_installed_adapter().is_none(), "shutdown uninstalls adapter");

        adapter.shutdown(); // second call is a no-op
        assert!(!adapter.is_initialized());

        handle.shutdown().unwrap();
    }
```

Append to `screen_monitor.rs` tests:

```rust
    #[tokio::test]
    async fn test_shutdown_clears_initialized_and_uninstalls() {
        let (handle, _stopped) = StateActor::spawn();
        let processor = Arc::new(EventProcessor::new(handle.clone()));
        let adapter = Arc::new(ScreenMonitorAdapter::new(processor));

        install_adapter(Arc::clone(&adapter));

        adapter.shutdown();
        assert!(!adapter.is_initialized());
        assert!(!adapter.is_processing());
        assert!(!adapter.gate_open());
        assert!(get_installed_adapter().is_none());

        handle.shutdown().unwrap();
    }
```

Append to `ax_observer.rs` tests:

```rust
    #[tokio::test]
    async fn test_shutdown_deactivates_and_uninstalls() {
        let (handle, _stopped) = StateActor::spawn();
        let processor = Arc::new(EventProcessor::new(handle.clone()));
        let adapter = Arc::new(AXObserverAdapter::new(processor));

        install_adapter(Arc::clone(&adapter));
        adapter.activate();

        adapter.shutdown();
        assert!(!adapter.is_active());
        assert!(!adapter.gate_open());
        assert!(get_installed_adapter().is_none());

        handle.shutdown().unwrap();
    }
```

Append to `observer.rs` tests:

```rust
    #[test]
    fn test_shutdown_is_idempotent() {
        // System is not initialized in unit tests; both calls must be no-ops.
        shutdown();
        assert!(!INITIALIZED.load(Ordering::SeqCst));
        shutdown();
        assert!(!INITIALIZED.load(Ordering::SeqCst));
    }
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p stache --lib modules::tiling::events
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/init.rs \
  app/native/src/modules/tiling/events/app_monitor.rs \
  app/native/src/modules/tiling/events/screen_monitor.rs \
  app/native/src/modules/tiling/events/ax_observer.rs \
  app/native/src/modules/tiling/events/observer.rs
git commit -m "feat(tiling): gate and remove app/screen/AX observers"
```

---

## Task 15C-3: Mouse/border process-lifetime gating and transient-state clearing

**Files:**

- Modify: `app/native/src/modules/tiling/events/mouse_monitor.rs`
- Modify: `app/native/src/modules/tiling/borders.rs`
- Modify: `app/native/src/modules/tiling/effects/animation/state.rs`
- Modify: `app/native/src/modules/tiling/effects/animation/mod.rs` (re-export)
- Modify: `app/native/src/modules/tiling/effects/mod.rs` (re-export)

Note: `drag_state::cancel_operation`, `tabs::clear_all_tabs`, and `effects::get_window_cache().clear()` already exist.

- [ ] **Step 1: Make the mouse tap explicitly active/inactive**

In `mouse_monitor.rs`, add:

```rust
/// Whether event processing is currently enabled.
///
/// The tap and run loop live for the process lifetime; this flag gates
/// callback work so a paused tiling runtime receives no drag state.
static ACTIVE: AtomicBool = AtomicBool::new(true);
```

Add after `is_initialized()`:

```rust
/// Enables or disables event processing.
///
/// Disabling also resets the tracked mouse-down state and clears the
/// mouse-up callback so no stale drag finishes during a pause.
pub fn set_active(active: bool) {
    ACTIVE.store(active, Ordering::SeqCst);
    if !active {
        MOUSE_DOWN.store(false, Ordering::SeqCst);
        clear_mouse_up_callback();
    }
}

/// Returns whether event processing is enabled.
#[must_use]
pub fn is_active() -> bool { ACTIVE.load(Ordering::SeqCst) }
```

(`clear_mouse_up_callback` already exists at `mouse_monitor.rs:110`.) Gate the callback — first line of `mouse_event_callback`:

```rust
    if !ACTIVE.load(Ordering::SeqCst) {
        return event;
    }
```

Add a test:

```rust
    #[test]
    fn test_set_active_gates_processing() {
        set_active(true);
        assert!(is_active());

        set_active(false);
        assert!(!is_active());
        assert!(!is_mouse_down());

        set_active(true);
        assert!(is_active());
    }
```

- [ ] **Step 2: Add `pause`/`resume` to the border runner**

In `borders.rs`, add:

```rust
/// Whether the border system is paused.
///
/// The animation runner thread lives for the process lifetime; this flag
/// makes it inert during a tiling pause.
static PAUSED: AtomicBool = AtomicBool::new(false);
```

Add after `init()`:

```rust
/// Pauses the border system: marks the runner inactive, clears the
/// last-command cache, and sends a zero-width/hidden border command so no
/// stale borders linger on screen.
pub fn pause() {
    PAUSED.store(true, Ordering::SeqCst);
    *get_last_command().lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
        String::new();

    let args = vec![
        "width=0".to_string(),
        "active_color=0x00000000".to_string(),
        "inactive_color=0x00000000".to_string(),
    ];
    if !send_command(&args) {
        tracing::debug!("tiling: borders pause hide command not sent (borders unavailable)");
    }

    tracing::debug!("tiling: borders paused");
}

/// Resumes the border system and refreshes it against fresh tiling state.
pub fn resume() {
    PAUSED.store(false, Ordering::SeqCst);
    refresh();
    tracing::debug!("tiling: borders resumed");
}

/// Returns whether the border system is paused.
#[must_use]
pub fn is_paused() -> bool { PAUSED.load(Ordering::SeqCst) }
```

Gate `on_focus_changed` (first line) and `process_update` (drop queued commands while paused):

```rust
pub fn on_focus_changed(layout: LayoutType, is_window_floating: bool) {
    if PAUSED.load(Ordering::SeqCst) {
        tracing::trace!("tiling: borders paused, ignoring focus change");
        return;
    }
```

```rust
fn process_update(
    rx: &mpsc::Receiver<AnimationCommand>,
    cmd: AnimationCommand,
) -> Option<AnimationCommand> {
    if PAUSED.load(Ordering::SeqCst) {
        tracing::trace!("tiling: borders paused, dropping queued animation command");
        return None;
    }
```

Add a test:

```rust
    #[test]
    fn test_pause_and_resume_flags() {
        resume(); // ensure clean initial state
        assert!(!is_paused());

        pause();
        assert!(is_paused());

        resume();
        assert!(!is_paused());
    }
```

> `pause()` clears the last-command cache and then attempts a real `send_command`. Only the flag transitions are asserted: with JankyBorders installed the hide command overwrites `LAST_COMMAND` on success (borders.rs:515), without it `send_command` logs and returns `false` — either way `is_paused()` holds.

- [ ] **Step 3: Add a transient animation-state reset**

In `effects/animation/state.rs`, add:

```rust
/// Resets every transient animation counter and position map.
///
/// The display-link/sync singletons are retained process-wide; only the
/// mutable animation bookkeeping is cleared so a paused runtime publishes
/// no further effects.
pub fn reset_transient_state() {
    ANIMATION_ACTIVE.store(false, Ordering::Relaxed);
    WAITING_COMMANDS.store(0, Ordering::Relaxed);
    clear_animation_end_time();
    get_interrupted_positions().clear();
}
```

> `clear_interrupted_positions(window_ids: &[u32])` (state.rs:151) removes specific windows only; the pause reset must clear the whole map, so it calls `get_interrupted_positions().clear()` (the map is a `DashMap`, state.rs:63, whose `clear()` drops every entry).

Add a test:

```rust
    #[test]
    fn test_reset_transient_state_clears_counters() {
        cancel_animation();
        cancel_animation();
        set_animation_active(true);
        store_interrupted_positions(&[(1, Rect::new(0.0, 0.0, 10.0, 10.0))]);

        reset_transient_state();

        assert!(!is_animation_active());
        assert!(!should_cancel());
        assert!(!is_animation_settling());
        assert!(get_interrupted_position(1).is_none());
    }
```

Re-export from `animation/mod.rs` (add to the existing `pub use state::{...}` list):

```rust
    reset_transient_state,
```

and from `effects/mod.rs` (add to the existing `pub use animation::{...}` list):

```rust
    reset_transient_state,
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p stache --lib modules::tiling::events::mouse_monitor
cargo test -p stache --lib modules::tiling::borders
cargo test -p stache --lib modules::tiling::effects::animation
cargo check -p stache
cargo clippy -p stache --lib -- -D warnings
cargo fmt --all -- --check
```

Expected: all pass, clippy clean, fmt clean.

- [ ] **Step 5: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/events/mouse_monitor.rs \
  app/native/src/modules/tiling/borders.rs \
  app/native/src/modules/tiling/effects/animation/state.rs \
  app/native/src/modules/tiling/effects/animation/mod.rs \
  app/native/src/modules/tiling/effects/mod.rs
git commit -m "feat(tiling): gate mouse/border runners and reset transient state"
```

---

## Task 15D-1: Ordered idempotent `pause_runtime` with quarantine retry

**Files:**

- Modify: `app/native/src/modules/tiling/init.rs`

- [ ] **Step 1: Add the testable timeout seam**

```rust
/// Test seam: overrides `RUNTIME_STOP_TIMEOUT` so timeout paths do not block
/// real seconds in unit tests.
static STOP_TIMEOUT_OVERRIDE: Mutex<Option<Duration>> = Mutex::new(None);

fn stop_timeout() -> Duration {
    STOP_TIMEOUT_OVERRIDE.lock().copied().unwrap_or(RUNTIME_STOP_TIMEOUT)
}

#[cfg(test)]
fn set_stop_timeout(override_value: Option<Duration>) {
    *STOP_TIMEOUT_OVERRIDE.lock() = override_value;
}
```

- [ ] **Step 2: Add `ensure_can_start` and wire it into `start_runtime`**

```rust
/// Guards fresh starts against a quarantined runtime or an in-progress
/// transition. `resume` and `start` must reject these states.
fn ensure_can_start() -> Result<(), String> {
    let lifecycle = *LIFECYCLE.lock();
    if lifecycle != LifecycleState::Stopped {
        return Err(format!("tiling: cannot start while lifecycle is {lifecycle:?}"));
    }
    if matches!(&*RUNTIME.lock(), RuntimeSlot::Quarantined(_)) {
        return Err(
            "tiling: previous runtime is quarantined; only pause/shutdown may retry teardown"
                .to_string(),
        );
    }
    Ok(())
}
```

In `start_runtime` (15B), replace the lifecycle gate with:

```rust
    ensure_can_start()?;

    {
        let mut lifecycle = LIFECYCLE.lock();
        *lifecycle = LifecycleState::Starting;
    }
```

- [ ] **Step 3: Wire mouse/border reactivation into fresh start**

In `build_runtime`, replace the optional stages:

```rust
    // Optional stages: failure logs degraded mode but still permits Running.
    if let Err(e) = factory.setup_mouse_monitor() {
        tracing::warn!("{e}");
    } else {
        super::events::mouse_monitor::set_active(true);
    }
    if let Err(e) = factory.setup_borders() {
        tracing::warn!("{e}");
    } else {
        super::borders::resume();
    }
```

- [ ] **Step 4: Implement `pause_runtime`**

Replace the 15B `pause_runtime` with the ordered version:

```rust
/// Tears down the running tiling runtime in strict, idempotent order.
///
/// Stages:
/// 1. `restore_stache_hidden_apps()` before any actor teardown (idempotent;
///    returns an empty summary when no running handle exists).
/// 2. On the main thread, shutdown/gate/unregister app, screen, AX, and
///    standalone observers. All main-thread work completes before any wait.
/// 3. `EventProcessor::stop_and_wait` and routing-map clear.
/// 4. Cancel drag/animations; pause borders and the mouse tap.
/// 5. Subscriber shutdown + completion-latch wait.
/// 6. Actor shutdown + completion-latch wait.
/// 7. Drop all taken handles/Arcs.
/// 8. Clear tabs, AX caches, and transient animation/border state.
/// 9. Publish `RuntimeSlot::Empty` + `LifecycleState::Stopped`.
///
/// On timeout/failure the entire runtime (with per-stage progress) is
/// reinserted as `RuntimeSlot::Quarantined` and lifecycle stays `Stopping`;
/// a later call retries only the unfinished stages.
pub fn pause_runtime() -> Result<(), String> {
    let retry = {
        let mut lifecycle = LIFECYCLE.lock();
        match *lifecycle {
            LifecycleState::Stopping => true,
            LifecycleState::Running => {
                *lifecycle = LifecycleState::Stopping;
                false
            }
            LifecycleState::Stopped | LifecycleState::Starting => {
                return Err("tiling: pause called while not running".to_string());
            }
        }
    };

    let mut runtime = {
        let mut slot = RUNTIME.lock();
        match std::mem::replace(&mut *slot, RuntimeSlot::Empty) {
            RuntimeSlot::Empty if retry => {
                *LIFECYCLE.lock() = LifecycleState::Stopped;
                return Ok(());
            }
            RuntimeSlot::Empty => {
                return Err("tiling: no runtime to pause".to_string());
            }
            RuntimeSlot::Running(r) if !retry => PartialRuntime::from(r),
            RuntimeSlot::Quarantined(p) if retry => p,
            RuntimeSlot::Running(_) => {
                return Err("tiling: runtime is running during a teardown retry".to_string());
            }
            RuntimeSlot::Quarantined(_) => {
                return Err("tiling: runtime already quarantined".to_string());
            }
        }
    };

    if !runtime.teardown.visibility_restored {
        let summary = super::visibility::restore_stache_hidden_apps();
        tracing::info!(
            "tiling: restored {} of {} hidden apps",
            summary.restored,
            summary.attempted
        );
        runtime.teardown.visibility_restored = true;
    }

    if !runtime.teardown.main_thread_sources_removed {
        let app_monitor = runtime.app_monitor.clone();
        let screen_monitor = runtime.screen_monitor.clone();
        let ax_adapter = runtime.ax_adapter.clone();
        crate::platform::thread::dispatch_on_main_sync(move || {
            if let Some(m) = app_monitor {
                m.shutdown();
            }
            if let Some(s) = screen_monitor {
                s.shutdown();
            }
            if let Some(a) = ax_adapter {
                a.shutdown();
            }
            super::events::observer::shutdown();
        });
        runtime.teardown.main_thread_sources_removed = true;
    }

    if !runtime.teardown.processor_stopped {
        if let Some(processor) = &runtime.processor
            && !processor.stop_and_wait(stop_timeout())
        {
            *RUNTIME.lock() = RuntimeSlot::Quarantined(runtime);
            return Err("tiling: processor failed to stop within timeout".to_string());
        }
        runtime.teardown.processor_stopped = true;
    }

    if !runtime.teardown.transient_services_paused {
        super::events::drag_state::cancel_operation();
        super::events::mouse_monitor::set_active(false);
        super::effects::animation::cancel_animation();
        super::effects::animation::reset_transient_state();
        super::borders::pause();
        runtime.teardown.transient_services_paused = true;
    }

    if !runtime.teardown.subscriber_stopped {
        if let Some(subscriber) = &runtime.subscriber {
            subscriber.shutdown();
        }
        if let Some(latch) = &runtime.subscriber_stopped
            && !latch.wait_timeout(stop_timeout())
        {
            *RUNTIME.lock() = RuntimeSlot::Quarantined(runtime);
            return Err("tiling: effect subscriber did not stop within timeout".to_string());
        }
        runtime.teardown.subscriber_stopped = true;
    }

    if !runtime.teardown.actor_stopped {
        if let Some(actor) = &runtime.actor {
            let _ = actor.send(StateMessage::Shutdown);
        }
        if let Some(latch) = &runtime.actor_stopped
            && !latch.wait_timeout(stop_timeout())
        {
            *RUNTIME.lock() = RuntimeSlot::Quarantined(runtime);
            return Err("tiling: state actor did not stop within timeout".to_string());
        }
        runtime.teardown.actor_stopped = true;
    }

    runtime.actor = None;
    runtime.actor_stopped = None;
    runtime.processor = None;
    runtime.subscriber = None;
    runtime.subscriber_stopped = None;
    runtime.app_monitor = None;
    runtime.screen_monitor = None;
    runtime.ax_adapter = None;

    if !runtime.teardown.caches_cleared {
        super::tabs::clear_all_tabs();
        super::effects::get_window_cache().clear();
        runtime.teardown.caches_cleared = true;
    }

    *LIFECYCLE.lock() = LifecycleState::Stopped;
    tracing::info!("tiling: runtime paused (generation {})", runtime.generation);
    Ok(())
}
```

Add the `From<TilingRuntime> for PartialRuntime` conversion (used above):

```rust
impl From<TilingRuntime> for PartialRuntime {
    fn from(runtime: TilingRuntime) -> Self {
        Self {
            generation: runtime.generation,
            actor: Some(runtime.actor),
            actor_stopped: Some(runtime.actor_stopped),
            processor: Some(runtime.processor),
            subscriber: Some(runtime.subscriber),
            subscriber_stopped: Some(runtime.subscriber_stopped),
            app_monitor: Some(runtime.app_monitor),
            screen_monitor: Some(runtime.screen_monitor),
            ax_adapter: Some(runtime.ax_adapter),
            teardown: runtime.teardown,
        }
    }
}
```

- [ ] **Step 5: Update `shutdown` and add `resume`**

```rust
/// Shuts down the tiling system, delegating to `pause_runtime`.
pub fn shutdown() {
    if let Err(e) = pause_runtime() {
        tracing::error!("tiling: shutdown teardown failed: {e}");
    }
}
```

```rust
/// Resumes a paused tiling runtime via the same fresh-start path as `start`.
///
/// Never reuses retained resources from a prior generation.
pub fn resume(app_handle: tauri::AppHandle) -> Result<(), String> {
    start_runtime(app_handle)
}
```

- [ ] **Step 6: Run build/lint**

```bash
cargo check -p stache
cargo clippy -p stache --lib -- -D warnings
cargo fmt --all -- --check
```

Expected: compiles, clippy clean, fmt clean.

- [ ] **Step 7: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/init.rs
git commit -m "feat(tiling): ordered idempotent pause with quarantine retry"
```

---

## Task 15D-2: `TilingLifecycle` and re-exports

**Files:**

- Modify: `app/native/src/modules/tiling/init.rs`
- Modify: `app/native/src/modules/tiling/mod.rs`

- [ ] **Step 1: Implement the trait**

At the bottom of `init.rs`:

```rust
use crate::modules::services::lifecycle::{LifecycleModule, ModuleStatus};

/// `LifecycleModule` adapter for the tiling window manager.
pub struct TilingLifecycle {
    app_handle: tauri::AppHandle,
}

impl TilingLifecycle {
    #[must_use]
    pub fn new(app_handle: tauri::AppHandle) -> Self { Self { app_handle } }
}

impl LifecycleModule for TilingLifecycle {
    fn name(&self) -> &'static str { "Tiling Window Manager" }

    fn id(&self) -> &'static str { "tiling" }

    fn start(&self) -> Result<(), String> { start_runtime(self.app_handle.clone()) }

    fn pause(&self) -> Result<(), String> { pause_runtime() }

    fn resume(&self) -> Result<(), String> { start_runtime(self.app_handle.clone()) }

    fn status(&self) -> ModuleStatus {
        if !is_enabled() {
            return ModuleStatus::ConfiguredOff;
        }
        match *LIFECYCLE.lock() {
            LifecycleState::Running | LifecycleState::Starting => ModuleStatus::Running,
            LifecycleState::Stopped | LifecycleState::Stopping => ModuleStatus::Paused,
        }
    }
}
```

- [ ] **Step 2: Re-export from the tiling module**

In `mod.rs`, extend the existing `pub use init::{...}` list:

```rust
    TilingLifecycle, pause_runtime, resume,
```

- [ ] **Step 3: Build and lint**

```bash
cargo check -p stache
cargo clippy -p stache --lib -- -D warnings
cargo fmt --all -- --check
```

Expected: compiles clean.

- [ ] **Step 4: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/init.rs app/native/src/modules/tiling/mod.rs
git commit -m "feat(tiling): TilingLifecycle module"
```

---

## Task 15D-3: Lifecycle orchestration tests

**Files:**

- Test: `app/native/src/modules/tiling/init.rs` (`mod tests`)

> Tests in this module mutate the process-global `LIFECYCLE`/`RUNTIME`/`STOP_TIMEOUT_OVERRIDE` statics, so they must not run concurrently with each other. Hold `TEST_LIFECYCLE_LOCK` (introduced in Task 15B Step 1) in every test that touches the globals — the 19F tests in `app-identity-restoration.md` run in the same test binary and also hold it.

- [ ] **Step 1: Write the tests**

```rust
    #[test]
    fn ensure_can_start_rejects_running_and_quarantined() {
        let _guard = TEST_LIFECYCLE_LOCK.lock();
        assert!(ensure_can_start().is_ok());

        *LIFECYCLE.lock() = LifecycleState::Starting;
        assert!(ensure_can_start().is_err());
        *LIFECYCLE.lock() = LifecycleState::Stopped;

        *RUNTIME.lock() = RuntimeSlot::Quarantined(PartialRuntime {
            generation: 1,
            actor: None,
            actor_stopped: None,
            processor: None,
            subscriber: None,
            subscriber_stopped: None,
            app_monitor: None,
            screen_monitor: None,
            ax_adapter: None,
            teardown: TeardownProgress::default(),
        });
        assert!(ensure_can_start().is_err());
        *RUNTIME.lock() = RuntimeSlot::Empty;
    }

    #[test]
    fn pause_runtime_when_stopped_errors() {
        let _guard = TEST_LIFECYCLE_LOCK.lock();
        *LIFECYCLE.lock() = LifecycleState::Stopped;
        *RUNTIME.lock() = RuntimeSlot::Empty;
        assert!(pause_runtime().is_err(), "pause with no runtime must error");
    }

    #[test]
    fn pause_from_quarantined_retries_unfinished_stages_then_empties() {
        let _guard = TEST_LIFECYCLE_LOCK.lock();
        set_stop_timeout(Some(Duration::from_millis(30)));

        // Quarantined partial: only the subscriber latch is unfinished and it
        // never completes on the first attempt -> stays quarantined.
        let unfinished = CompletionLatch::new();
        let partial = PartialRuntime {
            generation: 7,
            actor: None,
            actor_stopped: None,
            processor: None,
            subscriber: None,
            subscriber_stopped: Some(unfinished.clone()),
            app_monitor: None,
            screen_monitor: None,
            ax_adapter: None,
            teardown: TeardownProgress {
                visibility_restored: true,
                main_thread_sources_removed: true,
                processor_stopped: true,
                transient_services_paused: true,
                ..TeardownProgress::default()
            },
        };
        *RUNTIME.lock() = RuntimeSlot::Quarantined(partial);
        *LIFECYCLE.lock() = LifecycleState::Stopping;

        let err = pause_runtime().unwrap_err();
        assert!(
            err.contains("subscriber"),
            "first attempt must time out on the subscriber latch: {err}"
        );
        assert!(
            matches!(&*RUNTIME.lock(), RuntimeSlot::Quarantined(_)),
            "unfinished runtime must stay quarantined"
        );
        assert_eq!(*LIFECYCLE.lock(), LifecycleState::Stopping);

        // Complete the latch and retry: teardown finishes and the slot empties.
        unfinished.mark_complete();
        pause_runtime().unwrap();
        assert!(matches!(&*RUNTIME.lock(), RuntimeSlot::Empty));
        assert_eq!(*LIFECYCLE.lock(), LifecycleState::Stopped);

        set_stop_timeout(None);
    }

    #[test]
    fn pause_from_quarantined_with_restore_pending_runs_restore_first() {
        let _guard = TEST_LIFECYCLE_LOCK.lock();
        set_stop_timeout(Some(Duration::from_millis(30)));

        // All stages except visibility_restored are marked done; the restore
        // call runs with no running handle and is a harmless empty no-op that
        // still advances the stage flag (idempotency with terminal cleanup).
        let partial = PartialRuntime {
            generation: 8,
            actor: None,
            actor_stopped: None,
            processor: None,
            subscriber: None,
            subscriber_stopped: Some(CompletionLatch::new()),
            app_monitor: None,
            screen_monitor: None,
            ax_adapter: None,
            teardown: TeardownProgress {
                main_thread_sources_removed: true,
                processor_stopped: true,
                transient_services_paused: true,
                ..TeardownProgress::default()
            },
        };
        *RUNTIME.lock() = RuntimeSlot::Quarantined(partial);
        *LIFECYCLE.lock() = LifecycleState::Stopping;

        // The subscriber latch below never completes; the error proves we got
        // past the restore + transient stages, and the quarantine is retained.
        let err = pause_runtime().unwrap_err();
        assert!(err.contains("subscriber"), "{err}");
        assert!(matches!(&*RUNTIME.lock(), RuntimeSlot::Quarantined(_)));
        assert_eq!(*LIFECYCLE.lock(), LifecycleState::Stopping);

        set_stop_timeout(None);
    }
```

> The restore-before-actor-teardown ordering with a _populated_ registry is proven deterministically in Task 19F (`app-identity-restoration.md`) with injected restore hooks; these tests prove the same stage ordering on the runtime side.

- [ ] **Step 2: Run tests**

```bash
cargo test -p stache --lib modules::tiling::init::tests
cargo test -p stache --lib modules::tiling
cargo check -p stache
cargo clippy -p stache --lib -- -D warnings
cargo fmt --all -- --check
```

Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git hash-object app/native/src/modules/audio/device.rs   # must print 50451982cc8ec2064079a2acec1170dfb49dec38
git add app/native/src/modules/tiling/init.rs
git commit -m "test(tiling): lifecycle orchestration and quarantine retry"
```

---

## Verification summary (Tasks 9, 15A-15D)

```bash
cargo test -p stache --lib modules::tiling
cargo test -p stache --lib
cargo clippy -p stache --lib -- -D warnings
cargo fmt --all -- --check
cargo check -p stache
git status --short            # device.rs must remain  M and unstaged
git diff --cached --name-only # must never list device.rs
```

On macOS with Accessibility enabled, manually start, pause, and resume twice: one actor/subscriber/processor generation active, stale callbacks do nothing, Stache-owned hidden apps restored before each pause, windows freshly enumerated, borders/drag handling resume once. `app_shutdown.rs` is untouched and `tiling::shutdown` keeps its `fn()` signature.
