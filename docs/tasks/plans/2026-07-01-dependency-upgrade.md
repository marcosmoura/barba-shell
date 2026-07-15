# Dependency Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade all Rust and Node dependencies plus developer tooling (rustup toolchain, pnpm, cargo-sort) to their latest compatible versions, resolve 5 open Dependabot PRs via `pnpm-workspace.yaml` overrides, and land it all directly on `main` in two commits (matching the prior upgrade cycle's convention), fully verified via lint/build/test at each layer.

**Architecture:** Sequential per-layer upgrade — toolchain → pnpm → external tools → JS/UI deps → Rust crates — each bumped and verified before the next, so a failure can be bisected to one layer. The only layer with real risk is the coordinated `@linaria/core`/`@wyw-in-js/vite` major-version bump, which requires removing the now-invalid `babelOptions` block from `vite.config.ts` and a build-level check that `@/...` aliased imports still statically resolve inside `.styles.ts` files.

**Tech Stack:** pnpm 11.9.0, Rust `nightly-2026-06-30`, Tauri 2.11.x, Vite 8.1.x, Linaria 8.x / wyw-in-js 2.x, oxlint 1.72.0 / oxfmt 0.57.0, Cargo workspace (single member `app/native`).

---

### Task 1: Update Rust toolchain

**Files:**

- Modify: `/Users/marcosmoura/Projects/stache/rust-toolchain.toml`

- [ ] **Step 1: Edit the channel**

Current content:

```toml
[toolchain]
channel = "nightly-2026-06-09"
components = ["rustfmt", "clippy", "rust-analyzer"]
profile = "minimal"
```

Change `channel = "nightly-2026-06-09"` to `channel = "nightly-2026-06-30"`.

- [ ] **Step 2: Install the new toolchain**

Run: `rustup toolchain install nightly-2026-06-30 --component rustfmt --component clippy --component rust-analyzer --profile minimal`

Expected: rustup downloads and installs the toolchain (or reports it's already installed).

- [ ] **Step 3: Verify the override picks up the new toolchain**

Run (from repo root): `rustup show active-toolchain`
Expected: `nightly-2026-06-30-aarch64-apple-darwin (overridden by '/Users/marcosmoura/Projects/stache/rust-toolchain.toml')`

- [ ] **Step 4: Verify the workspace still compiles on the new toolchain**

Run: `cargo check --workspace`
Expected: compiles with no errors (warnings from pedantic/nursery clippy lints are not run by `cargo check`, only `cargo clippy` — this step just confirms the new nightly doesn't break compilation before moving on).

---

### Task 2: Update pnpm (global tool)

- [ ] **Step 1: Install pnpm 11.9.0 globally**

Run: `npm install -g pnpm@11.9.0`

- [ ] **Step 2: Verify**

Run: `pnpm --version`
Expected: `11.9.0`

---

### Task 3: Update cargo-sort

- [ ] **Step 1: Install the latest cargo-sort**

Run: `cargo install cargo-sort`

- [ ] **Step 2: Verify**

Run: `cargo sort --version`
Expected: `cargo-sort 2.1.4`

---

### Task 4: Update JS/UI dependencies

**Files:**

- Modify: `/Users/marcosmoura/Projects/stache/package.json` (dependency/devDependency versions only — not the top-level `"version"` field, that's Task 12)
- Modify: `/Users/marcosmoura/Projects/stache/pnpm-lock.yaml` (regenerated)

- [ ] **Step 1: Run the update**

Run (from repo root): `pnpm update --latest`

This single command updates every dependency to its true latest version regardless of current range syntax (caret, tilde, or exact pin), rewriting `package.json` in place. Expected version changes to confirm afterward:

- `@linaria/core`: `^7.0.0` → `^8.0.0` (major)
- `@wyw-in-js/vite`: `1.1.0` → `2.1.3` (major, exact pin preserved)
- `vite`: `~8.0.16` → `~8.1.2`
- `oxlint`: `^1.69.0` → `^1.72.0`
- `oxfmt`: `^0.54.0` → `^0.57.0`
- `@types/node`: `^25.9.2` → `^26.0.1`
- `@typescript/native-preview`: `7.0.0-dev.20260610.1` → `7.0.0-dev.20260630.1` (exact pin preserved)
- Plus patch/minor bumps: `@hugeicons/core-free-icons`, `@hugeicons/react`, `@tanstack/eslint-plugin-query`, `@tanstack/react-query`, `@tauri-apps/api`, `@tauri-apps/cli`, `@vitejs/plugin-react`, `@vitest/browser`, `@vitest/browser-playwright`, `@vitest/coverage-istanbul`, `lint-staged`, `postcss` (`8.5.15`→`8.5.16`), `vitest`, `motion`, `playwright`, `stylelint`, `eslint-plugin-react-refresh`, `postcss-styled-syntax`

- [ ] **Step 2: Reinstall to ensure a fully consistent lockfile**

Run: `pnpm install`

- [ ] **Step 3: Confirm nothing unexpected is left outdated**

Run: `pnpm outdated`
Expected: empty output (all packages now at latest). If anything besides the above list appears, stop and investigate before continuing — do not proceed to Task 5 with unexplained stragglers.

---

### Task 5: Fix vite.config.ts for wyw-in-js v2 compatibility

**Why:** `@wyw-in-js/vite` v2 removed the `babelOptions` key from `StrictOptions` entirely. The current config passes a `babelOptions.plugins` module-resolver block to resolve the `@` import alias inside `.styles.ts` files — this key is now silently ignored. The project already has a top-level Vite `resolve.alias: { '@': ... }` (see unchanged block below), which wyw-in-js v2's default `eval.resolver: 'bundler'` mode should pick up automatically. Task 6 verifies this assumption with a real build.

**Files:**

- Modify: `/Users/marcosmoura/Projects/stache/vite.config.ts`

- [ ] **Step 1: Remove the `babelOptions` block from the `wyw()` plugin call**

Current content (lines 29-51):

```ts
    wyw({
      include: [`${UI_DIR}/**/*.styles.ts`],
      babelOptions: {
        plugins: [
          [
            'module-resolver',
            {
              alias: {
                '@': path.resolve(__dirname, UI_DIR),
              },
              extensions: ['.ts', '.tsx'],
            },
          ],
        ],
      },
      importOverrides: {
        './app/ui/design-system/index.ts': { unknown: 'allow' },
        './app/ui/design-system/colors.ts': { unknown: 'allow' },
        './app/ui/design-system/motion.ts': { unknown: 'allow' },
        './app/ui/utils/media-query.ts': { unknown: 'allow' },
        './app/ui/renderer/widgets/components/Calendar/Calendar.constants.ts': { unknown: 'allow' },
      },
    }),
```

New content:

```ts
    wyw({
      include: [`${UI_DIR}/**/*.styles.ts`],
      importOverrides: {
        './app/ui/design-system/index.ts': { unknown: 'allow' },
        './app/ui/design-system/colors.ts': { unknown: 'allow' },
        './app/ui/design-system/motion.ts': { unknown: 'allow' },
        './app/ui/utils/media-query.ts': { unknown: 'allow' },
        './app/ui/renderer/widgets/components/Calendar/Calendar.constants.ts': { unknown: 'allow' },
      },
    }),
```

Do not touch the `resolve.alias` block elsewhere in the file (already correct, unchanged):

```ts
  resolve: {
    alias: {
      '@': path.resolve(__dirname, UI_DIR),
    },
    conditions: ['module', 'production'],
  },
```

---

### Task 6: Verify JS/UI layer

- [ ] **Step 1: Type-check + lint**

Run: `pnpm lint:ui`
Expected: passes (`tsgo --noEmit`, `oxlint --fix`, `stylelint --fix` all succeed with no errors).

- [ ] **Step 2: Build (validates the Linaria/wyw-in-js pipeline end-to-end)**

Run: `pnpm build`
Expected: `tsc && vite build` completes successfully. This is the critical check for Task 5 — if `@/design-system` imports inside `.styles.ts` files fail to statically resolve, this build step will fail or produce CSS missing the expected static values.

- [ ] **Step 3: If Step 2 fails with unresolved `@/...` imports inside `.styles.ts` files**

Add an explicit resolver to the `wyw()` config in `vite.config.ts` (fallback only — try this only if Step 2 actually fails):

```ts
    wyw({
      include: [`${UI_DIR}/**/*.styles.ts`],
      eval: { resolver: 'native' },
      importOverrides: {
        './app/ui/design-system/index.ts': { unknown: 'allow' },
        './app/ui/design-system/colors.ts': { unknown: 'allow' },
        './app/ui/design-system/motion.ts': { unknown: 'allow' },
        './app/ui/utils/media-query.ts': { unknown: 'allow' },
        './app/ui/renderer/widgets/components/Calendar/Calendar.constants.ts': { unknown: 'allow' },
      },
    }),
```

Then re-run `pnpm build` and confirm it passes.

- [ ] **Step 4: Run UI tests**

Run: `pnpm test:ui`
Expected: all tests pass, coverage thresholds met (80% lines/functions/statements, 65% branches).

---

### Task 7: Resolve Dependabot PRs + reconcile minimumReleaseAgeExclude

**Files:**

- Modify: `/Users/marcosmoura/Projects/stache/pnpm-workspace.yaml`

- [ ] **Step 1: Replace the file with the updated content**

Write the complete new content:

```yaml
autoInstallPeers: true

publicHoistPattern:
  - '*types*'

allowBuilds:
  esbuild: true
  unrs-resolver: true

overrides:
  brace-expansion: 2.0.3
  braces: ^3.0.3
  flatted: 3.4.2
  happy-dom: 20.8.9
  micromatch: ^4.0.8
  picomatch: 2.3.2
  postcss: ^8.5.15
  trim: ^1.0.1
  trim-newlines: ^5.0.0
  yaml: 2.8.3
  yargs-parser: ^22.0.0

minimumReleaseAgeExclude:
  - '@oxfmt/binding-android-arm-eabi@0.57.0'
  - '@oxfmt/binding-android-arm64@0.57.0'
  - '@oxfmt/binding-darwin-arm64@0.57.0'
  - '@oxfmt/binding-darwin-x64@0.57.0'
  - '@oxfmt/binding-freebsd-x64@0.57.0'
  - '@oxfmt/binding-linux-arm-gnueabihf@0.57.0'
  - '@oxfmt/binding-linux-arm-musleabihf@0.57.0'
  - '@oxfmt/binding-linux-arm64-gnu@0.57.0'
  - '@oxfmt/binding-linux-arm64-musl@0.57.0'
  - '@oxfmt/binding-linux-ppc64-gnu@0.57.0'
  - '@oxfmt/binding-linux-riscv64-gnu@0.57.0'
  - '@oxfmt/binding-linux-riscv64-musl@0.57.0'
  - '@oxfmt/binding-linux-s390x-gnu@0.57.0'
  - '@oxfmt/binding-linux-x64-gnu@0.57.0'
  - '@oxfmt/binding-linux-x64-musl@0.57.0'
  - '@oxfmt/binding-openharmony-arm64@0.57.0'
  - '@oxfmt/binding-win32-arm64-msvc@0.57.0'
  - '@oxfmt/binding-win32-ia32-msvc@0.57.0'
  - '@oxfmt/binding-win32-x64-msvc@0.57.0'
  - '@oxlint/binding-android-arm-eabi@1.72.0'
  - '@oxlint/binding-android-arm64@1.72.0'
  - '@oxlint/binding-darwin-arm64@1.72.0'
  - '@oxlint/binding-darwin-x64@1.72.0'
  - '@oxlint/binding-freebsd-x64@1.72.0'
  - '@oxlint/binding-linux-arm-gnueabihf@1.72.0'
  - '@oxlint/binding-linux-arm-musleabihf@1.72.0'
  - '@oxlint/binding-linux-arm64-gnu@1.72.0'
  - '@oxlint/binding-linux-arm64-musl@1.72.0'
  - '@oxlint/binding-linux-ppc64-gnu@1.72.0'
  - '@oxlint/binding-linux-riscv64-gnu@1.72.0'
  - '@oxlint/binding-linux-riscv64-musl@1.72.0'
  - '@oxlint/binding-linux-s390x-gnu@1.72.0'
  - '@oxlint/binding-linux-x64-gnu@1.72.0'
  - '@oxlint/binding-linux-x64-musl@1.72.0'
  - '@oxlint/binding-openharmony-arm64@1.72.0'
  - '@oxlint/binding-win32-arm64-msvc@1.72.0'
  - '@oxlint/binding-win32-ia32-msvc@1.72.0'
  - '@oxlint/binding-win32-x64-msvc@1.72.0'
  - '@types/node@26.0.1'
  - '@types/react@19.2.17'
  - oxfmt@0.57.0
  - oxlint@1.72.0
  - postcss@8.5.16
  - stylelint-config-clean-order@10.0.0
  - vite@8.1.2
  - '@typescript/native-preview-darwin-arm64@7.0.0-dev.20260630.1'
  - '@typescript/native-preview-darwin-x64@7.0.0-dev.20260630.1'
  - '@typescript/native-preview-linux-arm64@7.0.0-dev.20260630.1'
  - '@typescript/native-preview-linux-arm@7.0.0-dev.20260630.1'
  - '@typescript/native-preview-linux-x64@7.0.0-dev.20260630.1'
  - '@typescript/native-preview-win32-arm64@7.0.0-dev.20260630.1'
  - '@typescript/native-preview-win32-x64@7.0.0-dev.20260630.1'
  - '@typescript/native-preview@7.0.0-dev.20260630.1'
```

(The 5 new `overrides` entries — `brace-expansion`, `flatted`, `happy-dom`, `picomatch`, `yaml` — resolve open Dependabot PRs #12, #8, #10, #9, #11 respectively. All `minimumReleaseAgeExclude` version pins are updated to match the versions now resolved after Task 4: oxfmt/oxlint main packages and their platform bindings, `@types/node`, `vite`, and all 7 `@typescript/native-preview` platform entries. `@types/react@19.2.17` and `stylelint-config-clean-order@10.0.0` are unchanged — already latest.)

- [ ] **Step 2: Reinstall to apply the new overrides**

Run: `pnpm install`
Expected: lockfile updates to reflect `brace-expansion@2.0.3`, `yaml@2.8.3`, `happy-dom@20.8.9`, `picomatch@2.3.2`, `flatted@3.4.2` wherever they appear transitively.

---

### Task 8: Re-verify JS/UI after workspace overrides

- [ ] **Step 1: Lint**

Run: `pnpm lint:ui`
Expected: passes.

- [ ] **Step 2: Test**

Run: `pnpm test:ui`
Expected: all tests pass.

---

### Task 9: Update Rust crates

**Files:**

- Modify: `/Users/marcosmoura/Projects/stache/Cargo.lock` (regenerated — no `Cargo.toml` specifier edits needed; a full manual crates.io audit of all ~47 direct/build dependencies confirmed every one is already within its existing semver range at the latest available version)

- [ ] **Step 1: Update the lockfile**

Run (from repo root): `cargo update`

**Important:** do NOT use `cargo update --workspace` — that flag restricts updates to packages that are themselves workspace members. Since this workspace has exactly one member (`stache`, which nothing else in the registry depends on), `--workspace` is a documented no-op here (reports "Locking 0 packages"). Bare `cargo update` is required to get real transitive updates.

Expected: resolves ~42 packages to newer compatible versions, including `tauri` `2.11.2`→`2.11.5`, `tauri-build`/`tauri-codegen`/`tauri-macros`/`tauri-plugin` `2.6.2`→`2.6.3`, `tauri-runtime` `2.11.2`→`2.11.3`, `tauri-runtime-wry` `2.11.2`→`2.11.4`, `tauri-utils` `2.9.2`→`2.9.3`, `tray-icon` `0.23.1`→`0.24.1`, plus `anyhow`, `bytes`, `cc`, `log`, `smallvec`, `syn`, `time`, `uuid`, `wasm-bindgen` family, `web-sys`, and others. It should also remove several now-unused transitive packages (dependency-tree simplification from the newer `tauri-build`): `foldhash`, `hashbrown`, `id-arena`, `leb128fmt`, `pathdiff`, `prettyplease`, `unicode-xid`, `wasip3`, the `wit-bindgen`/`wit-component`/`wit-parser`/`wasm-encoder`/`wasm-metadata`/`wasmparser` family.

- [ ] **Step 2: Review the diff**

Run: `git diff Cargo.lock | head -100`
Expected: version bumps and removals matching the list above; no unexpected major-version jumps.

---

### Task 10: Verify Rust layer

- [ ] **Step 1: Lint (includes cargo-sort check + clippy pedantic/nursery)**

Run: `pnpm lint:native`
Expected: `cargo sort --workspace` passes with no reordering needed; `cargo clippy --workspace --all --fix --allow-dirty --allow-staged -- -D warnings` completes with zero warnings/errors (all `pedantic`/`nursery`/`cargo`/`all` lint levels satisfied per workspace `Cargo.toml`).

- [ ] **Step 2: Test**

Run: `pnpm test:native`
Expected: `cargo nextest run --workspace` passes all tests.

---

### Task 11: Full combined verification

- [ ] **Step 1: Full lint**

Run: `pnpm lint`
Expected: passes (`lint:ui && lint:native`).

- [ ] **Step 2: Full test**

Run: `pnpm test`
Expected: passes (`test:ui && test:native`).

- [ ] **Step 3: Regenerate the JSON schema (safety check — confirms the release-build path still works)**

Run: `./scripts/generate-schema.sh`
Expected: completes successfully, `stache.schema.json` is valid JSON (script self-validates via `jq` if available).

---

### Task 12: Bump version and commit

**Files:**

- Modify: `/Users/marcosmoura/Projects/stache/Cargo.toml` (line 8)
- Modify: `/Users/marcosmoura/Projects/stache/package.json` (line 3)
- Modify: `/Users/marcosmoura/Projects/stache/app/native/tauri.conf.json` (line 4)

- [ ] **Step 1: Commit the dependency upgrade**

Run:

```bash
git add -A
git commit -m "chore: upgrade all dependencies to latest versions"
```

- [ ] **Step 2: Bump the version in all three manifests**

`Cargo.toml` line 8 — change:

```toml
version = "0.24.0"
```

to:

```toml
version = "0.25.0"
```

`package.json` line 3 — change:

```json
  "version": "0.24.0",
```

to:

```json
  "version": "0.25.0",
```

`app/native/tauri.conf.json` line 4 — change:

```json
  "version": "0.24.0",
```

to:

```json
  "version": "0.25.0",
```

- [ ] **Step 3: Regenerate Cargo.lock's `stache` version entry**

Run: `cargo check --workspace`
Expected: updates the `stache` package's `version` field inside `Cargo.lock` to `0.25.0` with no other changes.

- [ ] **Step 4: Commit the version bump**

Run:

```bash
git add -A
git commit -m "bump version to 0.25.0"
```

- [ ] **Step 5: Final sanity check**

Run: `git log --oneline -5`
Expected: top two commits are `bump version to 0.25.0` and `chore: upgrade all dependencies to latest versions`, in that order.
