# Cavendish

A voxel world engine around an array of atom-interferometer gravity gradiometers (AION-10): anything
with mass moves in a sparse voxel world, and a forward model turns it into what each tower reads, for
Gradar's training data and for interactive exploration.

## Which documents govern
1. `plan/REBUILD.md` — the plan. On the `rebuild` branch it overrides everything older.
2. `plan/VALIDATION-AND-RESEARCH.md`, `plan/ML-AND-API.md` — its companions.
3. `plan/PROGRESS.md` — where the work stands; read it first after any restart.
4. `design/*.md`, `milestones/M*.md`, `DESIGN.md`, the spec — the v1/v2 record: read for physics and
   design detail, never as instructions about crates or layout. Where they disagree with the plan, the
   plan wins; leave one line in the slice notes saying so.
The code is the source of truth for what exists. Nothing is set in stone: if the plan is wrong or
unclear, do the sensible thing and record one line on what you chose and why.

## How work runs
- `plan/KICKOFF.md`: the loop. Agents in `.claude/agents/`: `builder`, `reviewer`, `researcher`.
- One slice at a time (§9 of the plan); each ends with a working app that does something new, a demo
  and screenshots. Main always builds and runs.
- Research before physics: open method questions go to `researcher` agents first; their notes in
  `docs/research/` are cited by the slice brief.
- Spend part of every slice on making it better than asked (accuracy, speed, polish, UX); put ideas you
  do not do now in `plan/IDEAS.md`.

## Build and test (local first, no CI dependence)
- `cargo xtask quick` before every commit (under 30 s warm); `cargo xtask gpu` and `cargo xtask oracle`
  before a slice is done; `cargo xtask demo <slice>` for screenshots.
- `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings` clean.
- Python follows `.claude/skills/python-style` (Four's style), pyright strict.

## Quality bar
- A test that guards a property has a control arm that fails when the property is broken.
- Never weaken an oracle, move a tolerance or regenerate a fixture to pass. Explain the mismatch first.
- A suspicious number gets a measurement, not a story. Write the pass criterion before measuring.
- Every approximation is bounded or audited against the phase tolerance (radians) and recorded.
- British English. Simplicity over cleverness. Dependencies only where they earn their place.

## Ask the owner first only for
Money; contacting anyone outside; licences and publishing; deleting history or force-pushing; anything
outside the repo on the owner's machines; decisions that cannot be undone. Write the question into
`plan/PROGRESS.md` and carry on with work that does not depend on it. Everything else: decide, and list
the calls worth a second look at the next checkpoint.

## Physics questions for George
Site geometry, array layout, firing rules and anything about the real instrument go in
`plan/PROGRESS.md` under "For George"; use the stated defaults until answered.
