# Dependency Upgrade Implementation Plan

> **For agentic workers:** Sequential dependency upgrades with per-layer verification checkpoints.

**Goal:** Update all project dependencies to latest compatible versions, verify nothing breaks

**Architecture:** Layer-by-layer upgrades (Rust toolchain → Node → pnpm → JS deps → Rust deps) with `pnpm lint && pnpm test` after each layer

**Tech Stack:** Rust nightly, Tauri 2.x, React 19, TypeScript 6, Vite 8, pnpm 11

---

## Task 1: Update Rust toolchain

**Files:**

- Modify: `rust-toolchain.toml:2`

- [ ] **Step 1: Install and set latest nightly**

Run: `rustup toolchain install nightly-2026-06-09`
Expected: Installs successfully

- [ ] **Step 2: Update rust-toolchain.toml**

Change channel from `nightly-2026-05-21` to `nightly-2026-06-09`

- [ ] **Step 3: Verify Rust version**

Run: `cargo --version`
Expected: Shows nightly-2026-06-09 or compatible

- [ ] **Step 4: Quick compile check**

Run: `cargo check --workspace 2>&1 | tail -20`
Expected: No errors (warnings allowed)

## Task 2: Update Node.js

- [ ] **Step 1: Install Node.js v26.3.0**

Run: `fnm install 26.3.0 && fnm use 26.3.0` OR `nvm install 26.3.0 && nvm use 26.3.0`
Expected: Installs successfully

- [ ] **Step 2: Verify Node version**

Run: `node --version`
Expected: `v26.3.0`

## Task 3: Update pnpm

- [ ] **Step 1: Install pnpm 11.5.3**

Run: `npm install -g pnpm@11.5.3`
Expected: Installs successfully

- [ ] **Step 2: Verify pnpm version**

Run: `pnpm --version`
Expected: `11.5.3`

- [ ] **Step 3: Regenerate lockfile for pnpm 11**

Run: `pnpm install`
Expected: Lockfile regenerated without errors

- [ ] **Step 4: Verify install**

Run: `pnpm ls --depth=0`
Expected: All deps resolved, no missing peer deps

## Task 4: Update JS/UI dependencies

**Files:**

- Modify: `package.json`

- [ ] **Step 1: Update all packages to latest**

Run: `pnpm update --latest`
Expected: Updates package.json and pnpm-lock.yaml

- [ ] **Step 2: Manually bump stylelint-config-clean-order (8.0.2 → 10.0.0)**

Check if `pnpm update --latest` already handles this; if not, edit `package.json` line 73

- [ ] **Step 3: Reinstall with updated lockfile**

Run: `pnpm install`
Expected: Clean install

- [ ] **Step 4: Verify no TypeScript errors**

Run: `pnpm lint:ui` or `pnpm tsgo --noEmit`
Expected: No type errors

## Task 5: Update Rust dependencies

- [ ] **Step 1: Update transitive dependencies**

Run: `cargo update --workspace`
Expected: Updates Cargo.lock with latest compatible versions

- [ ] **Step 2: Verify Rust build**

Run: `cargo check --workspace 2>&1 | tail -20`
Expected: No errors

## Task 6: Full verification

- [ ] **Step 1: Run all linters**

Run: `pnpm lint`
Expected: Clean output, no errors

- [ ] **Step 2: Run all tests**

Run: `pnpm test`
Expected: All tests pass

## Task 7: Commit

- [ ] **Step 1: Stage and commit changes**

Run: `git add -A && git commit -m "chore: upgrade all dependencies to latest versions"`
Expected: Clean commit
