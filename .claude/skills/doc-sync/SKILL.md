---
name: doc-sync
description: Keeping Cavendish's living documents consistent when a design decision changes — the spec, DESIGN.md, the design/*.md drill-downs, and the milestones/*.md briefs must agree. Use whenever a change touches the data model (StateBundle/FieldSet), a seam, the crate layering, a physics convention, or a constant.
---

# Doc-sync

Cavendish's documents are a layered contract, and a design change is not done until they agree. A
change that lands in one document and not the others is a latent contradiction the next session will
trip over.

## The propagation rule

When a decision changes, update **together, in the same change**:

- `cavendish-spec/cavendish.tex` — the authoritative spec (physics, requirements, the `StateBundle`
  table `tab:bundle`).
- `DESIGN.md` — if the layering, a seam, or the data-contract framing moved.
- the relevant `design/*.md` — the affected subsystem drill-down(s).
- the relevant `milestones/*.md` — any brief whose requirements, equations, tests, or exit table are affected.

Precedence when they conflict: **spec wins**, then `DESIGN.md`, then `design/*.md`, then the briefs.
`docs/notes/` is historical and never authoritative.

## Cross-checks before finishing

- **The spec compiles clean**: `latexmk -pdf cavendish.tex` → exit 0, 0 unresolved refs, 0 undefined
  cites, no overfull boxes > 25 pt. A design change that breaks the build is not finished.
- **Field inventories match**: when the data model changes, diff the `StateBundle` field list in the
  spec's `tab:bundle` against `design/state.md` field-for-field. They must be identical — this is how a
  monkey-patched schema gets caught.
- **British English** everywhere, and the existing terse house tone.

## Recurring traps (from this project's history)

- Do **not** re-introduce ML-pipeline framing — no `(signal, label)` pairs, no "the dataset", no
  train/eval split baked into the engine. The engine dumps the full state; the task is downstream.
- Do **not** re-elevate `Prior` to a central abstraction; it is optional batch sugar.
- Do **not** describe `FieldSet` as a task selector; it is a cost/storage knob.
- Do **not** reopen the ⁸⁷Sr isotope question — the spec is correct (AION/Carlton Table I); George's
  `# Sr-88` code comment is incidental.
