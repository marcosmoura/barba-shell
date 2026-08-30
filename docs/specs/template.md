# Spec Template

Copy this structure for every capability spec. Normative sections state the contract without
implementation citations; the Evidence Base separately records current implementation, tests,
and intended documentation. Use index.md glossary terms. Every decision ID is registered in
the Decision Ledger in `docs/specs/index.md`.

````markdown
# <Domain> NN — <Capability Name>

> Status: 🟡 Draft · Open: <IDs>

Use exactly one status form:

- Settled: `> Status: ✅ Normative · Decides: <IDs>`; omit ` · Decides: <IDs>` when there
  are no decision IDs.
- Conflicted: `> Status: 🟡 Draft · Open: <IDs>`.

Promote only when there are no open decisions, no `Unowned` current capability, no internal
contradiction, exact public contracts, normal/boundary/failure/lifecycle acceptance scenarios,
and at least one exact implementation or intended-documentation anchor for every contract
surface. A `🟡 Draft` status is mandatory when any `Intended-only`, `Conflict`, or `Known defect`
row remains.

## Purpose

The user-visible outcome and why it exists. One short paragraph.

## Scope

What this spec decides. Bullet list.

### Out of Scope

Adjacent behavior this spec does not decide. Written as a table so every exclusion is a
navigable edge in the spec graph:

```markdown
| Excluded concern | Owner | Boundary note |
| ---------------- | ----- | ------------- |
```

Linking rules:

- Owner cells link to the owning spec's catalog anchor in `docs/specs/index.md`
  (`../index.md#specs-<domain>`) while the target file does not exist yet, prefixed with
  its catalog state mark (⬜ / 🟡 / ✅). When the target lands, promote the cell to a
  direct link to the spec file and drop the mark.
- One concern per row; split compound concerns into separate rows.
- A confirmed absence is `Not a current capability`, not `Unowned`. Do not use that label to
  hide an observable current behavior: every current capability has one owner, and an
  `Unowned` current capability prevents promotion to `✅ Normative`.
- Boundaries follow the catalog decomposition rule (separate state owner, trigger,
  side-effect policy, or independently testable outcome ⇒ separate spec). Reviewers may
  move any boundary; a moved boundary updates the Scope sections of both specs and the
  catalog row in the same PR.

## Terminology

Terms this spec pins or relies on, matching docs/specs/index.md. Add new canonical
terms to the index glossary in the same PR.

## Data Contract

Owned data with types-in-prose and every invariant that must always hold.
Contractual-only modules: declare inputs/outputs/errors/side effects instead of state.

## Configuration Contract

Config keys consumed: name, type, default, semantics, validation, precedence rules.

## Inputs

Commands accepted (with argument contracts) and observed facts reacted to.
State which inputs are ignored in which states.

## State Transitions

States and legal transitions as a list or table. Include entry conditions and
forbidden transitions. For contractual modules: "No owned state" plus the decision table.

## Outputs

Published events/snapshots: name, payload, when emitted, delivery guarantee.
Explicitly note "publishes nothing today" where true.

## Derived Effects

Side effects requested and which adapter executes them. Effect failure policy.

## Failure & Recovery

Error taxonomy, per-failure behavior, recovery/retry semantics, degraded modes.

## Cross-Module Contracts

Dependencies expressed only as contracts on other specs — never implementations.
Include status/lifecycle mapping if the module is tray-toggleable.

## Acceptance Scenarios

Given/When/Then scenarios covering normal paths, boundaries, conflicts between
inputs, failure injection, and lifecycle edges. Number them for traceability.

## Testing Seam

The highest stable seam acceptance tests target, and why it is stable.
Prefer existing seams; justify any new one.

## Open Decisions

Use this section only for unresolved `Intended-only`, `Conflict`, and `Known defect` claims.
Each row gets a Decision Ledger ID and keeps the spec `🟡 Draft`. Use this exact table header:

| ID  | Current behavior | Documented intent | Rewrite consequence | Evidence |
| --- | ---------------- | ----------------- | ------------------- | -------- |

Reuse the existing ledger ID for the same conflict. For a newly discovered conflict, allocate
`<domain><two-digit spec>-D<sequence>` using `F`, `W`, `A`, `C`, `B`, `WG`, or `T` as the domain
code and the smallest unused sequence in that spec; for example, `B09-D1`. A generated
2026-08-24 resolution has no authority by itself.

## Resolved Decisions

After a decision is evidence-backed and no longer open, remove it from the Open Decisions
table and record it with this exact note syntax:

- **<ID> — <title>.** Outcome: … Basis: … Evidence: …

## Evidence Base

This table is non-normative evidence, refreshed on each audit. It uses exactly this header:

| Contract surface | Implementation evidence | Test evidence | Intended documentation | Disposition |
| ---------------- | ----------------------- | ------------- | ---------------------- | ----------- |

Use only these final dispositions:

- `Aligned` — current behavior and intended documentation agree.
- `Current-only` — observable current behavior with intended documentation silent.
- `Intended-only` — intended documentation claims behavior with no current implementation.
- `Conflict` — evidence classes disagree.
- `Known defect` — intended documentation explicitly identifies broken current behavior.

Move every `Aligned` and `Current-only` claim into the normative sections above. Keep every
`Intended-only`, `Conflict`, and `Known defect` claim outside normative sections, assign it a
decision ID, and leave the spec `🟡 Draft`. Cite exact implementation files and symbols and
exact intended-documentation anchors. Name exact tests when they exist; otherwise write
`None — source-only evidence` in Test evidence rather than inventing proof.

`Unsupported` is only a session-local ledger classification for an invented generated claim
with neither source nor intended-documentation basis. Delete that claim and omit its Evidence
Base row from the corrected spec; it is never a final disposition.
````
