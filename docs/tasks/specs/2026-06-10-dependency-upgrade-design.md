# Dependency Upgrade Plan

## Scope

Upgrade all project dependencies (Rust toolchain, Node.js, pnpm, JS/UI packages, Rust crates) to latest compatible versions while maintaining existing functionality.

## Current State

| Layer                      | Current            | Target             |
| -------------------------- | ------------------ | ------------------ |
| Rust toolchain             | nightly-2026-05-21 | nightly-2026-06-09 |
| Node.js                    | v26.0.0            | v26.3.0            |
| pnpm                       | 10.30.1            | 11.5.3             |
| Tauri (CLI + API + native) | 2.11.2             | 2.11.2 (unchanged) |
| Stylelint config order     | 8.0.2              | 10.0.0             |

## Strategy: Sequential per-layer with verification

1. **Rust toolchain** — Update `rust-toolchain.toml` channel, install new nightly
2. **Node.js** — Upgrade to v26.3.0
3. **pnpm** — Upgrade to 11.5.3, regenerate lockfile
4. **JS/UI deps** — `pnpm update --latest` for all packages
5. **Rust deps** — `cargo update` for transitive; manual direct dep bumps
6. **Verify** — `pnpm lint && pnpm test` after each layer

## Risk Mitigation

- Per-layer verification allows bisecting failures
- No Tauri major version changes (stays at 2.x)
- Rust deps are patch/minor within existing Cargo.toml semver ranges
- JS deps: major bumps evaluated individually (stylelint-config-clean-order 8→10)
