# Progress

The builder updates this after every slice. The owner reads it at checkpoints.

| Slice | State | Tag | Notes |
|---|---|---|---|
| S0 the shell | building, task by task (`plan/tasks.yaml`) | | research: `docs/research/s0-app-stack-and-build-speed.md` |

## Questions for the owner
- **21 Nov 2026 — a Windows Defender exclusion for the repo's `target/` directory?** Cold builds of
  the S0 stack vary from 60 to 102 s for one configuration; real-time scanning is the likely cause.
  It is a security setting outside the repo, so it waits for a yes. Nothing depends on it.

- **21 Nov 2026 — fixtures from the reference scripts.** `github.com/Thranduil02/atom-interferometry`
  is public but has no licence file, so nothing grants permission to copy or redistribute. May
  fixtures made by running those scripts be committed here, and should the author be asked to add a
  licence? Until answered, S1 keeps such fixtures untracked and regenerates them locally.
- **21 Nov 2026 — two changes to the plan that S1's research asks for** (detail:
  `docs/research/s1-periodograms-and-script-fixtures.md`). Defaults are in use until you say otherwise:
  1. `point_oscillator_phase_matches_script ≤1e-9` holds only with a test-only quadrature that copies
     the scripts' sampling (their arm is one sample ahead of the source). The engine's own forward
     model differs from the scripts by 2×10⁻⁴ to 4×10⁻³ of peak. Default: keep ≤1e-9 for the
     script-copying quadrature, and state a separate measured tolerance for the engine's model,
     which stays tied to v1 at ≤1e-10.
  2. No GPU periodogram in S1: a single-precision NUFFT was wrong by 4% of the peak at 10⁵ shots.
     Default: f64 on the CPU (classical, exact Lomb–Scargle, NUFFT Lomb–Scargle); the GPU version
     waits for float-float in S5.

## For George
Defaults in use until answered: v1's conventions (δφ₂ − δφ₁, no floor, u₀ = 3.86 m/s, m = 1.46×10⁻²⁵ kg).
- The reference scripts pair the arm at (j+1)·dt with the source at j·dt, and the `fig2_*` window
  ends at 2T − dt. Intended?
- The scripts compute lower-minus-upper (δφ₁ − δφ₂); v1 and `instrument.md` use δφ₂ − δφ₁. Which is
  the paper's sign convention?
- ⁸⁷Sr or ⁸⁸Sr? The mass used everywhere, 1.46×10⁻²⁵ kg, is ⁸⁸Sr; ⁸⁷Sr is 1.2% lighter.
- AION-10 launch: u₀ = 3.86 m/s, or gT − v_kick/2 = 3.909 m/s so the arms close at launch height?
- Should the arms stop at the launch height (the scripts clamp; v1 has no floor; `instrument.md`
  §5 says "reflecting")?

## Owner's rulings (21 Nov 2026)
- **Tools on the build PC:** anything the build needs may be installed. Done so far: rustup with
  stable `x86_64-pc-windows-msvc` (rustc 1.100.0), `cargo-nextest`.
- **GitHub:** `gh` for all GitHub work (git pushes use its login via `gh auth setup-git`); plain git
  for local commits; never force-push.
- **This PC is for building and testing correctness only** (Windows 10 Pro, GTX 1660 SUPER, 6 GB).
  The design and the targets are not shrunk to fit it. Every slice works here at whatever scale the
  card manages; the measured numbers go in the slice notes; the performance targets (60 fps, 10¹⁰
  cells, latency budgets) are marked "to be measured on a faster GPU later" and do not fail a slice.
  Correctness, oracle and GPU-parity tests must pass here.
- `origin/build-machine` was already in `main` (merge `176bc81`); nothing further to merge.

## Decisions worth a second look
- **S0's "builds in under a minute cold" is recorded, not gated.** Measured on this PC: 60–102 s
  cold (median about 75 s) for an egui + wgpu + rmcp skeleton, front-end bound; warm rebuild 4.5 s.
  Following the owner's ruling on this PC's role, the slice records cold and warm times and gates
  only on `quick` under 10 s.
- **rust-lld is left off for now**, against REBUILD §8: measured gain 0.3 s per link, lost in the
  noise. Kept as a commented option to re-measure when the binary grows.
- **Tasks merge as they pass `quick`; the review is per slice.** Each task has its own PR
  (`task/<TASK-id>`) merged with a merge commit so later tasks build on `main`; a fresh reviewer
  reads the whole slice at the end, its findings are fixed in further task PRs (new ids), and the
  slice is tagged only when no important finding remains.
- **Cloud sessions** are started by the loop in a minimised console window (`claude --cloud` needs a
  console); their work comes back as a pushed branch. The first one: `research/s3-sparse-voxel-structures`.
