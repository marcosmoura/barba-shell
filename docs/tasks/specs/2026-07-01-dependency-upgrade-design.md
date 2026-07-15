# Dependency Upgrade Plan

## Scope

Upgrade all project dependencies and developer tooling to their latest compatible versions — Rust toolchain, pnpm, JS/UI packages, Rust crates, and external build tools (cargo-sort, sccache, media-control) — while maintaining existing functionality. Direct commits to `main`, no worktree, no PR (matching the prior upgrade cycle's precedent).

## Current State

| Layer                                            | Current                                                           | Target                                                                                                                               |
| ------------------------------------------------ | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Rust toolchain                                   | `nightly-2026-06-09`                                              | `nightly-2026-06-30`                                                                                                                 |
| Node.js                                          | `v26.4.0`                                                         | `v26.4.0` (already latest — no action)                                                                                               |
| pnpm                                             | `11.5.3`                                                          | `11.9.0`                                                                                                                             |
| Rust crates (via `cargo update`)                 | 42 packages behind (patch/minor only)                             | Up to date (no Cargo.toml edits needed — full manual crates.io audit of all ~47 direct/build deps found zero additional major bumps) |
| `@linaria/core` + `@wyw-in-js/vite`              | `7.0.0` + `1.1.0`                                                 | `8.0.0` + `2.1.3` (coordinated major bump, requires `vite.config.ts` edit)                                                           |
| Other JS/UI deps (via `pnpm update --latest`)    | ~22 packages behind (patch/minor)                                 | Up to date                                                                                                                           |
| Dependabot-flagged transitive deps               | 5 open PRs (brace-expansion, yaml, happy-dom, picomatch, flatted) | Resolved via new `pnpm-workspace.yaml` overrides                                                                                     |
| `pnpm-workspace.yaml` `minimumReleaseAgeExclude` | Pins reference old exact versions                                 | Reconciled to match newly-bumped versions                                                                                            |
| cargo-sort (external tool)                       | `2.1.3`                                                           | `2.1.4`                                                                                                                              |
| sccache (external tool)                          | `0.16.0`                                                          | `0.16.0` (already latest — no action)                                                                                                |
| media-control (Homebrew, external tool)          | `HEAD-815bcb5` (dev/HEAD track)                                   | No action — HEAD track has no comparable numbered version; switching to `0.7.6` stable would be a track change, out of scope         |

## Strategy: Sequential per-layer with verification

1. **Rust toolchain** — bump `rust-toolchain.toml` channel to `nightly-2026-06-30`, install via rustup, verify `cargo --version`.
2. **pnpm** — bump global pnpm install to `11.9.0`, verify `pnpm --version`.
3. **External dev tools** — `cargo install cargo-sort` (2.1.3→2.1.4). Skip sccache (already latest) and media-control (out of scope, different track).
4. **JS/UI dependencies:**
   - Run `pnpm update --latest` for all non-Linaria packages.
   - Handle `@linaria/core`/`@wyw-in-js/vite` as a coordinated pair: bump both to `8.0.0`/`2.1.3`, remove the now-invalid `babelOptions.plugins` module-resolver block from `vite.config.ts` (lines 31-43), rely on the existing top-level `resolve.alias` + wyw-in-js v2's default `eval.resolver: 'bundler'` mode to resolve `@/...` imports in `.styles.ts` files. Verify via full build that design-system values (colors, motion, etc.) still statically resolve; visually check extracted CSS for cascade-order regressions.
   - Add 5 new `overrides:` entries to `pnpm-workspace.yaml` for the open Dependabot PRs: `brace-expansion: 2.0.3`, `yaml: 2.8.3`, `happy-dom: 20.8.9`, `picomatch: 2.3.2`, `flatted: 3.4.2`.
   - Reconcile all `minimumReleaseAgeExclude` version pins in `pnpm-workspace.yaml` to match the newly resolved versions (oxfmt, oxlint, their platform bindings, `@types/node`, `vite`, `@typescript/native-preview` + its platform binaries) by cross-checking `pnpm-lock.yaml` after the update.
   - Verify via `pnpm lint && pnpm test`.
5. **Rust crates** — run bare `cargo update` (NOT `cargo update --workspace`, which is a documented no-op here since the single workspace member `stache` isn't itself a registry dependency of anything). Verify via `cargo check --workspace` and `cargo clippy --workspace`.
6. **Full verification** — `pnpm lint && pnpm test` (JS+Rust combined lint/test scripts), `cargo test --workspace` if applicable.
7. **Commit** — single commit `chore: upgrade all dependencies to latest versions` plus a version bump commit, matching prior cycle's two-commit pattern (`c4e18a6` + `0c03de6`).

## Risk Mitigation

- Per-layer verification allows bisecting failures to a specific layer.
- No Tauri major version change (core stays 2.x; full crates.io audit confirmed no 3.x exists yet).
- Rust crate updates are patch/minor only within existing semver ranges — confirmed via exhaustive manual crates.io check of all ~47 direct/build dependencies, not just `cargo update`'s dry-run.
- `@linaria/core`/`@wyw-in-js/vite` is the one confirmed-risky change in this cycle: both packages are coupled (Linaria 8 pins an exact `@wyw-in-js/processor-utils` version) and must be bumped together. The specific breaking change affecting this repo (`babelOptions` removal) has a known, testable mitigation path (rely on existing `resolve.alias`, fall back to `eval.resolver: 'native'` if needed). Build + visual CSS diff required before considering this step done.
- `@styled/typescript-styled-plugin` and `postcss-styled-syntax` are confirmed unaffected by the Linaria bump (lexical-only tools, no runtime coupling).
- Dependabot PR resolution via `pnpm-workspace.yaml` overrides follows the repo's existing override pattern (used previously for `flatted` etc.) and should cause Dependabot to auto-close the 5 open PRs once merged to `main`.
- media-control and sccache require no changes; flagged in this spec purely for completeness of the "external tools" scope requested by the user.
