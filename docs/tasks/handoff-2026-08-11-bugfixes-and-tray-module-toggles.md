# Handoff: Bugfixes and Tray Module Toggles Session

**Date:** 2026-08-11
**Branch:** `main`, 38 commits ahead of `origin/main`
**Status:** Design approved; plan revised through `ba076a1`; Phase 6 not started in source.

## TL;DR

This session fixes Stache's menubar (target ≤200 ms), Ghostty and
floating-window bugs, adds tray toggles for six modules, and restores
Stache-hidden apps on supported shutdown paths. The design is approved and the
implementation plan is revised through `ba076a1`, but no Phase 6 source
migration has started. The menubar detector merged in Tasks 1-2 was disproven
by manual testing, so Task 3, Phase 3, and the shutdown-verification path stay
blocked pending user decisions. Next: run and record an architecture gate on
the plan, commit plan and handoff while preserving the protected `device.rs`
edit, then implement Tasks 19A-19G with 19D+19E as one atomic cutover.

## Objective

1. Fix Stache's menubar (target ≤200 ms), Ghostty, floating-window, and
   shutdown-restoration issues; add tray toggles for six modules.
2. Later: add literal active-layout `§` keybinding support. Unscoped: no spec,
   plan task, acceptance criteria, or owner exists yet; treat as deferred until
   a task is written.

## Critical Constraints

- **Preserve the unstaged user edit exactly:**
  `app/native/src/modules/audio/device.rs`, blob
  `50451982cc8ec2064079a2acec1170dfb49dec38`. Do not stage, commit, or modify
  it. It was recovered from unreachable Git objects after an earlier
  accidental loss; verify with `git hash-object` before any commit. Forbidden:
  `git add -A`, `git add .`, `git reset --hard`, `git checkout .`,
  `git stash`, `git commit -am`. Commit only via explicit `git add <paths>`.
  If the blob is lost, recover it via `git fsck --full --no-reflogs
--unreachable` and re-verify the hash; that is how it was recovered
  previously.
- Uncommitted state today: the protected `device.rs` edit (1 line), the plan
  (1693 insertions, 390 deletions), and this handoff (untracked). Nothing is
  staged.
- Menubar: `NSMenu.menuBarVisible()` (Tasks 1-2, commits `d03e811`..`7db3e82`)
  was disproven empirically for transient auto-hide. Apple's API reports
  application menu-bar policy ("visible and selectable"), the value stayed
  constant during reveal/hide animation, and the successful result suppressed
  the `CGWindowList` fallback, so Stache never reacted. Do not re-implement
  it. A replacement detector is an open design decision (owner: user).
- Ghostty and floating-window bugs were not reproduced during the manual
  diagnostic session; diagnostics exist, no speculative fixes. Phase 3 stays
  blocked until the user provides a repro log.
- Shutdown policy: restore only Stache-hidden applications; preserve
  independently hidden/minimized windows. `SIGKILL` cannot be handled.
- Task 19D+19E is one atomic cutover. During 19D, do not commit, run
  `pnpm tauri:dev`, quit, restart, or hand off; the 19D tree is intentionally
  unshippable (plan lines 3343-3351). Task 19F only adds tests for production
  restoration. Task 20 CAS shutdown arbiter is unchanged.
- Commits `f2e20c8` through `95fa2f5` (generation/FIFO/PID-set ownership) are
  superseded by Tasks 19A-19G. Root cause of the `attempted=0 restored=0`
  manual failure: AppShown callbacks and actor workspace operations mutated PID
  ownership in different ordering domains.

## Approved architecture (Phase 6)

- Actor single-writer ownership.
- `AppIdentity = (pid, launchDate bits)`.
- `WindowTarget = (AppIdentity, window_id)`.
- Exact observer/tab/effect/cache/animation validation.
- Sealed `VisibilityRegistry` (shared `Arc` created in `StateActor::spawn`,
  shared with the handle via `new_with_registry`).
- `RuntimeSlot::{Empty, Running(TilingRuntime), Quarantined(PartialRuntime)}`
  restartable runtime with completion latches and rollback.
- Conservative policy: if an unobserved user show→rehide leaves the app
  hidden, keep Stache ownership.

## Work state

### Completed

- Approved design/spec:
  `docs/tasks/specs/2026-07-16-bugfixes-and-tray-module-toggles-design.md`
  (redesign commit `c8efdb6`).
- Plan revisions through `ba076a1` (11 advisor-review findings), `2aa6b06`,
  `3d6112c`.
- Phase 2 tracing Tasks 4-7 merged and reviewed (`de63053`..`76c960a`).
- Task 20 CAS arbiter (`fb3fe1b`). Manual quit test exposed
  `attempted=0 restored=0`; root cause being addressed by the Phase 6
  actor/identity redesign.
- Protected audio edit recovered and verified intact.

### Task status

| Task                                    | Status                                                                |
| --------------------------------------- | --------------------------------------------------------------------- |
| 1-2 menubar detector, 100 ms poll       | Merged (`d03e811`..`7db3e82`) but invalid; failed manual verification |
| 3 menubar ≤200 ms verification          | Pending a corrected detector design (owner: user)                     |
| 4-7 Phase 2 tracing                     | Merged and reviewed (`de63053`..`76c960a`)                            |
| 8 manual repro and evidence             | Attempted; neither bug reproduced; no logs saved                      |
| 9-18 tray lifecycle                     | Not started                                                           |
| 19A-19G Phase 6 actor/identity redesign | Not started; no source migration                                      |
| 20 centralized orderly shutdown         | Merged (`fb3fe1b`); unchanged                                         |
| 21 manual shutdown verification         | Blocked pending actor/identity redesign                               |
| Phase 3 (Ghostty, floating fixes)       | Blocked; no repro evidence                                            |

### Active (uncommitted)

`docs/tasks/plans/2026-07-16-bugfixes-and-tray-module-toggles.md` (4425 lines;
1693 insertions, 390 deletions; plus the 1-line `device.rs` edit):

- Actor-identity (Task 19B), exact-effects, restartable-tiling-runtime (19A)
  corrections.
- `RuntimeSlot::Quarantined(PartialRuntime)`, completion latches, main-thread
  setup/teardown, fatal/optional startup stages, pause restoration, exact
  `RefreshActiveBorder`, `SetExpectedFrames`, drag targets, cache/effect
  identity.
- Advisor-review findings reconciled and verified in-file (`ba076a1`):
  `PartialRuntime` quarantine, `pub(crate) CompletionLatch`, Task 19C
  `spawn() -> (StateActorHandle, CompletionLatch)` contract, teardown-test
  order, Task 19B staged file lists (actor/handle.rs, rules/mod.rs,
  state/tiling_state.rs).
- **No Phase 6 source migration started.** None of `AppIdentity`,
  `VisibilityRegistry`, `RuntimeSlot`, `PartialRuntime`, `CompletionLatch`,
  `LaunchDateBits` exist under `app/native/src/` yet.

### Blocked

- Architecture approval of the latest plan: the gate is undefined (see
  "Next move").
- Manual shutdown verification: fails pending actor/identity redesign.
- Menubar root-cause fix: needs a design decision.
- Tray lifecycle Tasks 9-18 and the `§` keybinding.
- Phase 3 fixes: no repro evidence.

## Plan structure (for navigation)

- Tasks 1-8: Phase 1/2 menubar latency + diagnostics. Tasks 1-2 are merged but
  invalid; Task 3, Phase 3 blocked (see constraints).
- Tasks 9-15: `LifecycleModule` trait + tray toggles for six modules:
  wallpapers, commandQuit tap, notunes observer, proxyAudio, menuAnywhere tap,
  tiling `reset()`.
- Tasks 16-18: lifecycle registry, tray `CheckMenuItems`, manual verification.
- Tasks 19A-19G: Phase 6 actor/identity/visibility redesign.
- Tasks 20-21: centralized orderly shutdown + manual verification.

Module-name to directory mapping: `wallpapers` -> `modules/wallpaper/`,
`commandQuit` -> `modules/cmd_q/`, `proxyAudio` -> feature/config inside
`modules/audio/`, `menuAnywhere` -> `modules/menu_anywhere/`. Task 9 creates a
new `modules/services/` directory; do not confuse it with the existing
top-level `services/` (dead `Module`/`BackgroundService` traits, not reused).

## Architecture gate verdict (2026-08-12)

Gate re-run on `docs/tasks/plans/2026-08-12-restartable-tiling-runtime.md`,
`docs/tasks/plans/2026-08-12-app-identity-restoration.md`, and
`docs/tasks/plans/2026-08-12-module-lifecycle-tray.md` per the master plan's
criteria (actor sole writer; fail-closed `AppIdentity`/`WindowTarget`
propagation; atomic seal/drain + seal-aware relinquish; 19D+19E one atomic
cutover; no locks across main-thread dispatch/completion waits; workspace
cycling excluded; every task compiles against current source).

**Verdict: APPROVED — zero open actionable findings** after two review rounds:

Round 1 (fresh reviewer, 6 findings — all corrected in-plan):

1. HIGH: 19D pause-restore ordering was prose-only; the 15D-1 code took the
   runtime (`mem::replace` → `Empty`) before restoring, so `get_handle()`
   returns `None` and Stache-hidden apps would never be restored on tray-pause.
   Fixed: concrete `restore_visibility_if_pending()` helper in
   `app-identity-restoration.md` 19D Step 5 (restore before the take; mark the
   stage flag after), plus the new 19F test
   `pause_restores_via_published_handle_before_teardown` that publishes a
   `Running` slot with a populated registry and asserts the registry is sealed
   and drained.
2. MEDIUM: the 19D dispatch rewrite orphans the private wrappers
   `on_switch_workspace`/`on_send_workspace_to_screen` in `actor/mod.rs`,
   which would fail 19G's `clippy -- -D warnings` on `dead_code`. Fixed:
   explicit deletion instruction in 19D Step 5.
3. MEDIUM: four compile-blocking gaps (missing `Instant` import in
   `processor.rs` `stop_and_wait`; missing `AtomicU64` in
   `wallpaper/manager.rs`; missing `AudioObjectRemovePropertyListener` in
   `audio/watcher.rs`; `clear_interrupted_positions()` called with no args
   instead of `get_interrupted_positions().clear()`). Fixed with explicit
   import/code instructions in each plan.
4. LOW: global-state test race — 15B's global-reading tests now hold
   `TEST_LIFECYCLE_LOCK` (defined in 15B Step 1; 15D-3/19F reference it).
5. LOW: borders pause test asserted `LAST_COMMAND == ""` after `pause()`, which
   fails when JankyBorders is installed; assertion removed, flags only.
6. LOW: "adjust to actual code" prose removed (`windows_identity_iter` is final
   against `ObservableVector<Window>`; 19G's retained-helper list made
   concrete).

Round 2 (fresh reviewer): all ten fixes verified; one remaining LOW — a
test-fixture `TeardownProgress` literal missing `transient_services_paused:
true` (would mutate borders/mouse-monitor statics outside the lifecycle lock).
Fixed in `restartable-tiling-runtime.md` 15D-3. Final verdict: zero open
actionable findings.

Also recorded: the uncommitted spec clarification is confirmed as
`modules/services/traits.rs` → top-level `services/traits.rs` (spec lines 94,
290). Tasks 1-2 (`NSMenu.menuBarVisible()`) were disproven empirically (see
Critical Constraints).

## Next move

1. Run a fresh architecture gate on the latest plan and record the result.
   No gate artifact exists; define one now. Proposed: re-review the plan's
   Phase 6 invariants with a fresh reviewer, with actor single-writer,
   `AppIdentity` fail-closed, seal/drain, atomic 19D+19E, and the deadlock
   rules (plan lines 3881-3905) as pass criteria; exit when zero open findings;
   record approval in this handoff or a review comment. Owner: user.
2. Commit the approved plan and this handoff together (verify the protected
   `device.rs` hash first; leave `device.rs` unstaged).
3. Get user approval, then implement Tasks 19A-19G. During the 19D+19E
   cutover, do not run the app, quit, restart, or commit until 19E tests pass.

## Recent commits (selected)

```text
ba076a1 docs(plan): fix 11 findings from advisor review
2aa6b06 docs: make actor visibility plan executable
3d6112c docs: correct actor visibility migration plan
c8efdb6 docs: redesign shutdown visibility ownership
95fa2f5 fix: move OS hidden-state query inside tracker mutex (TOCTOU race)
dbe0d54 fix: ownership-race corrections with structured logs and EventProcessor seam
f2e20c8 fix: per-PID generation/FIFO state machine for hidden-app ownership race
fb3fe1b fix: CAS state machine eliminates check/action and concurrent-override races
12d913e fix: atomic terminal-request arbiter prevents exit/restart race
177f756 feat: restore Stache-hidden apps before shutdown
```

Selected list: two interleaved style-only commits are omitted (`95a4383`
between `95fa2f5` and `dbe0d54`; `acc656c` between `12d913e` and `177f756`).
The branch is 38 commits ahead of `origin/main`; see `git log origin/main..HEAD`
for the full list.

## Relevant files

- `docs/tasks/specs/2026-07-16-bugfixes-and-tray-module-toggles-design.md`:
  approved combined design.
- `docs/tasks/plans/2026-07-16-bugfixes-and-tray-module-toggles.md`: active
  uncommitted implementation plan (4425 lines; ~1694 insertions uncommitted
  across it and `device.rs`).
- `docs/tasks/handoff-2026-08-11-bugfixes-and-tray-module-toggles.md`: this
  handoff (untracked; stage/commit it alongside the plan when approved).
- `.slim/deepwork/bugfixes-tray-toggles.md`: ignored local session record
  (per-task review results, manual-session notes, `pnpm test` baseline of 1038
  passed). Migrate anything a future session needs into tracked docs; it is
  not committed and may be deleted.
- `app/native/src/modules/bar/menubar.rs`: unresolved menubar detector.
- `app/native/src/modules/tiling/`: window identity, visibility, effects,
  lifecycle, diagnostics.
- `app/native/src/app_shutdown.rs`: approved shutdown coordinator/CAS arbiter.
- `app/native/src/modules/audio/device.rs`: **protected user edit; must remain
  unstaged** (blob `50451982cc8ec2064079a2acec1170dfb49dec38`).

## Verification

**Baseline recorded 2026-08-12** (before Task 9; non-mutating checks):

- `git status --short` — exactly the docs/plan/spec/handoff changes and the
  protected `device.rs`; index empty.
- `git diff --check` / `git diff --cached --check` — clean.
- `git hash-object app/native/src/modules/audio/device.rs` —
  `50451982cc8ec2064079a2acec1170dfb49dec38` (verified before every commit).
- `pnpm test` — **1094 passed, 0 failed, 0 skipped** (historical baseline was
  1038 with two flaky: `test_batched_geometry_updates`, a border animation
  test; both passed this run).
- `cargo check -p stache` — clean.
- `cargo clippy --workspace --all -- -D warnings` (pedantic + nursery) —
  clean.
- `cargo fmt --all -- --check` — **1 pre-existing diff**: the protected user
  edit in `device.rs:204` (the `MatchStrategy::Regex` arm is not collapsed to
  one line by rustfmt). Must not be modified; treat as pre-existing, not a
  regression. All other files fmt-clean.
- UI: `pnpm tsgo --noEmit` clean; `pnpm oxlint` 0 warnings/errors (installed
  oxlint rejects the `--allow-empty-input` flag from the plan — use plain
  `pnpm oxlint`); `pnpm stylelint` 0 problems.

Do NOT run `pnpm lint` or `pnpm format` in this tree: `lint:native` (clippy
`--fix` + `cargo sort`) and `format:native` (`cargo fmt --all`) would rewrite
`device.rs` / the working tree.

```bash
git status --short            # expect: M device.rs, M plan.md, ?? handoff-*.md
git diff --check              # no whitespace errors (tracked changes only)
git diff --cached --check     # index must be empty
git diff --cached --name-only # must NOT list device.rs
git hash-object app/native/src/modules/audio/device.rs
                              # expect 50451982cc8ec2064079a2acec1170dfb49dec38
```

Manual verification prerequisites: tray toggles (Tasks 15, 18) and shutdown
paths (Task 21) need Accessibility permission and a release build
(`pnpm tauri:build`); the tray "Reload Stache" item exists only in release.

## Open questions

| Question                     | Owner | Needed decision                                          |
| ---------------------------- | ----- | -------------------------------------------------------- |
| Menubar detector replacement | user  | approve a new detection approach; Task 3 is pending this |
| Phase 3 go/no-go             | user  | supply a repro log, or abandon Phase 3                   |
| `§` keybinding               | user  | write a spec and plan task, or drop it                   |
| Architecture gate            | user  | run and record the gate defined under "Next move"        |

## Definition of done (session)

Gate approved and recorded; plan + handoff committed with `device.rs` hash
verified and unstaged; Tasks 19A-19G merged with all unit tests green (plan
Verification Summary, lines 4432-4441); Task 21 manual verification passed;
verification baseline re-recorded.
