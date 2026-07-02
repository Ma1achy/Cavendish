---
name: milestone-workflow
description: How to execute a Cavendish milestone — the loop that turns a milestones/M*.md brief into working, tested, CI-green code while keeping the vertical slice alive. Use at the start of any coding session that implements or extends a milestone (M0–M10 or the reference-port thread).
---

# Milestone workflow

Cavendish is built in **vertical slices**: from M1 there is always a runnable `Scenario → StateBundle`
path, and each milestone *enriches* it. Never finish one crate in isolation and integrate at the end.

## The loop

1. **Read the brief in full** — `milestones/M<n>-*.md`. Then read the `design/*.md` for each crate it
   touches, and the spec sections it cites. The brief is self-contained by design; trust it over memory,
   and verify claims against the actual files rather than recollection.
2. **Implement to the exit table**, not to your own idea of done. Every requirement `M<n>-R<k>` maps to
   named tests in the brief's traceability line — make those the definition of done.
3. **Write all three test levels** the brief specifies — unit, integration, e2e — at the **exact
   tolerances given** (e.g. trace-free ≤1e-12, CPU≡GPU ≤1e-4, decomposition sum ≤1e-10). Do not invent
   looser tolerances to make a test pass; if a tolerance is genuinely wrong, raise it as a doc-sync change.
4. **Land the reference anchor.** M1/M2/M5/M6 each have a `reference::*` agreement test (see the
   physics-validation skill). Porting George's case and the Cavendish path are the *same* session — the
   anchor test needs both sides. Never skip it or replace it with a remembered figure.
5. **Keep the slice working and CI green** — `cargo build --workspace`, `cargo test --workspace`,
   `cargo fmt --check`, `cargo clippy --all-targets --all-features` (CI runs `-D warnings`). A milestone
   is not done if an earlier slice regressed.

## Discipline

- Do not pull scope forward from later milestones, and do not leave an earlier milestone's exit
  requirement unmet to start the next.
- If the brief and the spec disagree, stop and reconcile via doc-sync before coding — do not silently
  pick one.
- Prefer the smallest change that satisfies the exit table; the briefs are deliberately incremental.
