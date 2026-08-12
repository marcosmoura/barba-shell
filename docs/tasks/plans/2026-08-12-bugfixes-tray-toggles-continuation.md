# Bugfixes + Tray Module Toggles — Master Continuation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resume the approved Stache work (menubar latency, Ghostty/floating diagnostics, tray toggles for six modules, shutdown restoration of Stache-hidden apps) by executing the corrected, dependency-ordered Tasks 9 → 15 → 19A-19G → 10-14 → 16-18 → 21, with the protected `device.rs` edit untouched.

**Architecture:** Four documents. This master plan defines order, gates, baseline, and protected-file rules. Three executable subplans carry the actual TDD tasks: `2026-08-12-restartable-tiling-runtime.md` (Tasks 9, 15), `2026-08-12-app-identity-restoration.md` (Tasks 19A-19G), and `2026-08-12-module-lifecycle-tray.md` (Tasks 10-14, 16-18). The approved architecture (actor-single-writer `VisibilityRegistry`, `AppIdentity = (pid, launchDate bits)`, `WindowTarget = (AppIdentity, window_id)`, sealed `RuntimeSlot::{Empty, Running, Quarantined}`, atomic 19D+19E cutover) is unchanged; the dependency order is corrected and the tray/module tasks were rewritten against current source after audit.

**Tech Stack:** Rust (Tauri 2.11.2), `objc` v0.2.7, `core-foundation` 0.10, `objc2-core-audio` 0.3.2, `parking_lot`, `dashmap`, `eyeball`, strict Clippy (`pedantic` + `nursery`).

**Spec:** `docs/tasks/specs/2026-07-16-bugfixes-and-tray-module-toggles-design.md` (approved; uncommitted 2-line path clarification pending).
**Prior plan (superseded by these documents):** `docs/tasks/plans/2026-07-16-bugfixes-and-tray-module-toggles.md` — keep as reference for Phase 1/2 and Tasks 20-21 details; its Phase 4-6 task text is replaced by the three subplans.

---

## Current git state (verified 2026-08-12)

- Branch `main`, HEAD `ba076a1`, **38 commits ahead of `origin/main`**.
- `git status --short` shows exactly:
  - ` M app/native/src/modules/audio/device.rs` (protected)
  - ` M docs/tasks/plans/2026-07-16-bugfixes-and-tray-module-toggles.md`
  - ` M docs/tasks/specs/2026-07-16-bugfixes-and-tray-module-toggles-design.md`
  - `?? docs/tasks/handoff-2026-08-11-bugfixes-and-tray-module-toggles.md`
- Index is empty (`git diff --cached --name-only` → nothing). `git diff --check` clean.
- No Phase 6 migration exists in source: `rg "AppIdentity|VisibilityRegistry|RuntimeSlot|PartialRuntime|CompletionLatch|LaunchDateBits|WindowTarget" app/native/src` → no matches.

## Protected file — absolute rules

`app/native/src/modules/audio/device.rs` is an unstaged 1-line user edit. Blob **must** equal `50451982cc8ec2064079a2acec1170dfb49dec38`. Verify with `git hash-object` before any commit. If lost, recover via `git fsck --full --no-reflogs --unreachable` and re-verify.

Forbidden in every session: `git add -A`, `git add .`, `git stash`, `git reset --hard`, `git checkout .`, `git commit -am`. Commit only explicit `git add <paths>`; never include `device.rs`.

## Dependency / execution order

| #   | Work                                             | Plan                                                 | Gate before proceeding                            |
| --- | ------------------------------------------------ | ---------------------------------------------------- | ------------------------------------------------- |
| 0   | Documentation reconciliation + architecture gate | this doc, §Documentation gate                        | zero open findings; docs committed; user go-ahead |
| 1   | Record baseline (non-mutating)                   | §Baseline                                            | baseline recorded                                 |
| 2   | Task 9: `LifecycleModule` + `ModuleStatus`       | restartable-tiling-runtime.md                        | tests + clippy green                              |
| 3   | Task 15: restartable tiling runtime (15A-15D)    | restartable-tiling-runtime.md                        | every 15A-15D checkpoint green                    |
| 4   | Tasks 19A-19G: identity + actor visibility       | app-identity-restoration.md                          | 19D+19E single atomic commit; 19G searches empty  |
| 5   | Tasks 10-14: module lifecycles                   | module-lifecycle-tray.md                             | per-task tests + clippy green                     |
| 6   | Tasks 16-17: registry + tray                     | module-lifecycle-tray.md                             | full `cargo test -p stache --lib` green           |
| 7   | Task 18 + Task 21: release manual verification   | module-lifecycle-tray.md Task 18; prior plan Task 21 | manual results recorded                           |

Task 20 (CAS orderly shutdown, commit `fb3fe1b`) is **merged and unchanged** — `app/native/src/app_shutdown.rs` must NOT be edited by any task below. It keeps calling `tiling::restore_stache_hidden_apps()` (public signature preserved through Phase 6).

## Documentation gate

Re-run a fresh architecture gate on the three subplans with a reviewer who has not seen them. Pass criteria:

1. Actor is the sole runtime writer of ownership state; handle exposes only terminal `seal_and_drain_visibility`.
2. `AppIdentity`/`WindowTarget` are fail-closed and propagated through every delayed/tab/effect/cache/animation/drag/routing path; no bare-PID fallback remains after 19G.
3. `VisibilityRegistry` seal/drain is atomic; relinquishment is seal-aware (`relinquish_if_open`); late writes impossible.
4. 19D+19E is one atomic cutover: no intermediate commit, no app run, no restart, no handoff during 19D.
5. No lifecycle/runtime/visibility lock is held across main-thread dispatch or completion waits; teardown order cannot deadlock (prior plan lines 3881-3905).
6. Workspace cycling (`on_cycle_workspace`) is excluded from the visibility cutover and unchanged.
7. Every task in the three subplans compiles against current source with no TODO/TBD/"adapt to actual code" instructions.

**Exit condition: zero open actionable findings.** Record approval in the handoff or a review comment.

**Docs commit:** only when (a) the gate records zero findings, (b) `git hash-object app/native/src/modules/audio/device.rs` returns `50451982cc8ec2064079a2acec1170dfb49dec38`, (c) `device.rs` is left unstaged. Then:

```bash
git add docs/tasks/plans/2026-07-16-bugfixes-and-tray-module-toggles.md \
  docs/tasks/plans/2026-08-12-bugfixes-tray-toggles-continuation.md \
  docs/tasks/plans/2026-08-12-restartable-tiling-runtime.md \
  docs/tasks/plans/2026-08-12-app-identity-restoration.md \
  docs/tasks/plans/2026-08-12-module-lifecycle-tray.md \
  docs/tasks/specs/2026-07-16-bugfixes-and-tray-module-toggles-design.md \
  docs/tasks/handoff-2026-08-11-bugfixes-and-tray-module-toggles.md
git commit -m "docs(plan): corrected continuation plans after architecture gate"
```

Also record in the handoff: the uncommitted spec clarification (`modules/services/traits.rs` → top-level `services/traits.rs`), the fresh gate verdict, and that Tasks 1-2 (`NSMenu.menuBarVisible()`) were disproven empirically.

**Begin source (Task 9):** only after the docs commit lands **and** the user gives explicit go-ahead.

## Baseline (non-mutating — run before Task 9)

```bash
git status --short            # expect: M device.rs, M plan.md, M design.md, ?? handoff-*.md
git diff --check              # no whitespace errors
git diff --cached --check     # index must be empty
git diff --cached --name-only # must NOT list device.rs
git hash-object app/native/src/modules/audio/device.rs
                              # expect 50451982cc8ec2064079a2acec1170dfb49dec38
pnpm test                     # historical baseline: 1038 passed (2 unrelated flaky:
                              #   test_batched_geometry_updates, a border animation test)
cargo check -p stache
cargo clippy --workspace --all -- -D warnings   # pedantic + nursery
cargo fmt --all -- --check
```

UI checks (no `--fix`, no writes):

```bash
pnpm tsgo --noEmit
oxlint --allow-empty-input
stylelint --formatter verbose --allow-empty-input './app/ui/**/*.(styles.ts|css)'
```

**Do NOT run `pnpm lint` or `pnpm format` in this tree**: `lint:native` (`cargo clippy --fix` + `cargo sort`) and `format:native` (`cargo fmt --all`) would rewrite `device.rs` / the working tree. Record any pre-existing failures in the handoff so regressions are distinguishable.

## Manual verification prerequisites

Tray toggles (Tasks 15D/18) and shutdown paths (Task 21) need Accessibility permission and a release build (`pnpm tauri:build`). The tray "Reload Stache" item exists only in release builds.

## Deferred / blocked (owner: user)

- **Menubar:** Tasks 1-2 merged (`d03e811`..`7db3e82`) but invalid — `NSMenu.menuBarVisible()` stayed constant during reveal/hide and suppressed the `CGWindowList` fallback. Do not re-implement `query_menu_bar_visible_via_nsmenu`. Task 3 (≤200 ms verification) is pending a user-approved replacement detector.
- **Phase 3 (Ghostty, floating-window):** no repro evidence; no speculative fixes. Blocked until the user supplies a repro log.
- **Literal `§` keybinding:** unscoped; no spec, plan, or owner. Deferred.
- **Workspace cycling:** `on_cycle_workspace` has no production caller and currently performs no OS hide/show. Phase 6 leaves it unchanged (user decision).

## Definition of done

Gate approved and recorded; plan + spec + handoff committed with `device.rs` hash verified and unstaged; Tasks 9, 15, 19A-19G, 10-14, 16-17 merged with all unit tests green; Task 18 and Task 21 manual verification passed; baseline re-recorded.
