# Validation report

Written by `cargo xtask validate --report`; do not edit by hand. Every case is declared with
`cav_validate::case!` beside the code it checks, and all of them are listed here: one that did not run
is shown as **missing**, never left out.

**116 of 116 cases pass; worst margin: materials.shared_cells_are_within_their_dipole_bound at 99% of its tolerance.**

<picture><source media="(prefers-color-scheme: dark)" srcset="error-vs-tolerance-dark.svg"><img src="error-vs-tolerance-light.svg" alt="Each case's measured error against its tolerance, on a log scale." width="100%"></picture>


## array

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `array.geometry_vs_v2_recording` (S0) | reference recording: tower x, y and base z, the room and 2T of docs/viewer-mockup/replay.json (the v2 recording); largest difference | 0 | 0 | metres | 0% |  | pass |
| `array.unequal_towers_read_as_each_alone` (S3) | exact self-consistency: four towers of four instruments (AION-10; AICE 0.3 s late; T = 0.9 s; Δr = 4 m and 800 kicks) and two moving bodies: each tower's every shot, every channel and readout time, against a one-tower scene of that tower's resolved instrument, position and offset, at 1 and 6 threads; values that differ in any bit | 0 | 0 | count | 0% |  | pass |

## bundle

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `bundle.npy_round_trip` (S2) | exact self-consistency: 10^6 random f64 bit patterns and the awkward values written as .npy and read back; values whose bits differ | 0 | 0 | count | 0% |  | pass |
| `bundle.round_trip` (S2) | exact self-consistency: a bundle of the staggered scene (five towers on their own clocks, a fifth of the shots lost, two bodies) written and read back; values of any array that differ in any bit from what the engine holds, and shapes that are not the stated ones | 0 | 0 | count | 0% |  | pass |

## cadence

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `cadence.window_loss_floor` (S2) | closed form: shots lost independently with probability p = 0.01 and 0.05 from 4000 shots every 5 s: the mean over 400 keyed seeds and 64 frequencies between the first two lines of /W/^2 - /W0/^2 (W normalised by the kept count, W0 the Dirichlet kernel of the plan) against p/((1-p)N); the worse /measured/predicted - 1/ | 0.0289 | 0.05 | relative | 58% |  | pass |
| `cadence.window_cycle_jitter` (S2) | closed form: cycle jitter as a random walk, positive U(T0, T0+d) and symmetric U(T0-d, T0+d), d = 0.1 and 0.3 s, T0 = 5 s, 500 shots: the mean of /W/^2 over 2000 keyed seeds at the alias 1/Tbar, 1.5 line-widths above it and at 1.37 times it, against (1/N^2)[N + 2 Re sum (N-k) phi^k]; the worst /measured/predicted - 1/ of the twelve (each is also held to 3 standard errors of its mean) | 0.032 | 0.08 | relative | 40% |  | pass |
| `cadence.window_uniform` (S2) | closed form: the spectral window /W/ of 200 shots every 5 s by direct sum against the Dirichlet kernel /sin(N pi nu T0)/(N sin(pi nu T0))/ at 257 frequencies from 0 to 0.8 Hz, the five lines among them; largest absolute difference of /W/ (the peak is 1) | 4.121e-14 | 1.000e-12 | relative | 4% |  | pass |
| `cadence.window_grid_jitter` (S2) | closed form: jitter about a grid, shot times n T0 + U(-0.3, 0.3) s, T0 = 5 s, 500 shots: the mean of /W/^2 over 2000 keyed seeds at the first alias 1/T0 against /psi/^2 /D_N/^2 + (1 - /psi/^2)/N; /measured/predicted - 1/ (also held to 3 standard errors of the mean, and the line's height at N and 10 N to 3%) | 6.658e-5 | 0.08 | relative | 0% |  | pass |
| `cadence.window_gapped` (S2) | closed form: planned gaps, L of every P slots of 5 s taken, L/P = 0.4 as 4 of 10 and as 40 of 100, fifty whole periods: the window's height at k/(P T0), k = 1..4, by direct sum against /sin(pi k L/P)/(L sin(pi k/P))/; largest absolute difference of heights (the peak is 1) | 6.078e-15 | 1.000e-9 | relative | 0% |  | pass |
| `cadence.periodogram_on_uneven_shots` (S2) | exact self-consistency: the cadence-lab scene (each cycle up to 0.3 s late, 2% of shots lost): the engine's periodogram of tower 2, by non-uniform FFT, against the exact Lomb–Scargle of the same shots; largest difference as a fraction of the peak | 2.493e-15 | 1.000e-9 | relative | 0% |  | pass |
| `cadence.window_alternating` (S2) | closed form: cycles of 5.5 s and 6.0 s in turn: the window's height at k/T_s, k = 1..4, by direct sum at 2000 shots against /cos(pi k T_A/T_s)/, and against the same at 20 000 shots; the larger absolute difference of heights (the peak is 1) | 1.089e-12 | 0.001 | relative | 0% |  | pass |

## engine

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `engine.cached_sum_equals_cold_run` (S1) | exact self-consistency: superposition after a drag of one body and back, summed from kept contributions, against a cold run of the same scene; values of any channel that differ, to the bit | 0 | 0 | count | 0% |  | pass |
| `engine.replay` (S0) | exact self-consistency: a session replayed from its journal, and from its journal saved as JSON; replays whose state digest differs | 0 | 0 | count | 0% |  | pass |
| `engine.replay_bundles` (S2) | exact self-consistency: a session (every kind of edit, a drag, staggered towers, a comparison switched on and off, a jittered and thinned cadence, undo and redo) that exports two bundles and a scene, against the same session replayed four ways: from its journal in memory, from its journal saved to disk and read back, with the workers settled after every command, and with them never waited for; files of any replay that differ in any byte (each manifest's provenance, the build's revision and dirty flag, among them) | 0 | 0 | count | 0% |  | pass |
| `engine.thread_count` (S2) | exact self-consistency: the signals of three scenes (staggered towers on a jittered, thinned cadence; a sphere through the towers, with its warnings; two bodies) read on 1, 2, 3 and 6 threads; values, estimates, warnings and flags that differ in any way from the one-thread read | 0 | 0 | count | 0% |  | pass |

## figure

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `figure.png_regenerates` (S2) | exact self-consistency: the comb's periodogram and signal as PNG figures (paper-column at 300 pixels per inch, slide), painted by two engines on the local adapter, each with a device of its own: the .png, .csv and .json files that differ in any byte | 0 | 0 | count | 0% |  | pass |
| `figure.regenerates` (S2) | exact self-consistency: six figures of the comb (periodogram, signal, one tower's lower channel over a span, cadence window, flight diagram; SVG and PDF; four presets) written by two engines from the same commands: each figure, its .csv and its .json; files that differ in any byte (each .json's provenance, the build's revision and dirty flag, among them) | 0 | 0 | count | 0% |  | pass |
| `figure.view_regenerates` (S3) | exact self-consistency: the unit cube's view coloured by height, as vector SVG and PDF with its record, written by two engines from the same commands: files that differ in any byte | 0 | 0 | count | 0% |  | pass |

## forward

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `forward.precision_far` (S1) | exact self-consistency: the oscillation anchor and a tower 1.36 m from it, both moved by (10000.3, -7000.7, 300.1) m, read the same series as at the origin; largest difference as a fraction of the peak | 1.038e-12 | 1.000e-11 | relative | 10% |  | pass |
| `forward.quadrature_order` (S1) | closed form: Gauss-Legendre with 3 nodes a panel converges as h^6: the oscillation anchor's lower-interferometer phase at panels of T/2, T/4, T/8, T/16 against 12 nodes on T/64 (floor off, so the pieces are the two halves). The case is about the order; the 1e-9 on the finest step is a ceiling with no prediction behind it | 3.781e-11 | 1.000e-9 | relative | 4% | 6.15 (expected 6 ± 0.5) | pass |
| `forward.quadrature_on_the_anchors` (S1) | exact self-consistency: the engine's rule (Gauss, 6 nodes, 0.1 s panels) against 12 nodes on 0.025 s panels, on a static mass and the two published anchors; largest difference as a fraction of each series' peak | 2.893e-15 | 1.000e-11 | relative | 0% |  | pass |
| `forward.audited_read_close_pass` (S1) | exact self-consistency: 10 kg passing 0.5 m from a tower at 2.5 m/s, read until two successive rules agree to 1e-10 rad, against 12 nodes on 1.6 ms panels; largest error over 25 shots and both interferometers | 9.770e-15 | 1.000e-10 | radians | 0% |  | pass |
| `forward.v1_anchor_fixture` (S1) | v1 oracle: v1's outputs on the three anchors and on the oscillation read by the five towers, frozen in oracle/fixtures/v1 (the oracle tier checks them against v1 itself); engine set to v1's conventions; largest difference as a fraction of each series' peak | 1.575e-15 | 1.000e-10 | relative | 0% |  | pass |

## materials

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `materials.shared_cells_are_within_their_dipole_bound` (S19) | closed form: a steel cuboid and a lead one (0.175 × 0.2 × 0.2 m each, side by side along x) as one body at 0.05 m pitch, k = 2, the interface halfway through a layer of 16 cells: tower 1's gradiometer phase of one shot against the two closed-form prisms, as a fraction of the shared cells' first-order dipole bound Σ /Δρ/·v·h/8 · /∂ΔΦ/∂x/ (1 % over it allowed for the higher orders, (h/r)² ≈ 1e-3 here) | 0.9999 | 1.01 | relative | 99% |  | pass |
| `materials.two_material_body_is_the_sum_of_its_parts` (S19) | exact self-consistency: a steel box and a lead box (0.4 m, sharing a face) as one body of two materials, against the two read as separate uniform bodies of density × volume: tower 1's gradiometer phase over every shot, beside it on a 6 s timeline | 7.105e-15 | 1.000e-9 | radians | 0% |  | pass |
| `materials.imported_box_mesh_is_the_cuboid` (S19) | exact self-consistency: an STL box (1 m × 0.5 m × 0.25 m) imported as a steel body, against the Cuboid body of the same size, material and pitch (0.05 m): the same leaves, and tower 1's gradiometer phase over every shot of a 6 s timeline, relative | 0 | 1.000e-12 | relative | 0% |  | pass |

## maths

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `maths.perspective_divide` (S0) | closed form: the CPU matrix Mat4::perspective only, not the renderer: a point ten times further away projects ten times nearer the axis | 0 | 1.000e-12 | relative | 0% |  | pass |

## nulls

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `nulls.finite_floor_edge` (S1) | closed form: a concrete disc 1 m thick under a tower on its axis, radius 400 m to 6.4 km: the gradiometer's reading as a fraction of what one interferometer reads from the infinite slab; it falls as 1/R, the edge's uniform gradient, towards the slab's zero | 7.812e-4 | 0.001 | relative | 78% | 1.00 (expected 1 ± 0.05) | pass |
| `nulls.infinite_floor` (S1) | closed form: an infinite uniform slab (and the half-space) gives uniform g: the forward model's reading of the slab at each of five towers at different heights, lower minus upper; values over five towers and six shots that are not exactly zero | 0 | 0 | count | 0% |  | pass |
| `nulls.void` (S1) | exact self-consistency: the void: the array and nothing else; values of any channel of any tower's any shot that are not exactly zero | 0 | 0 | count | 0% |  | pass |

## oracle

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `oracle.published_anchor_oscillation_amplitude` (S2) | published paper: the published size of the oscillation anchor, about 2 mrad (1 kg at D = 5 m, 0.1 Hz; reference/design/instrument.md §12, Fig 15), read as an amplitude, half the peak to peak (George, 23 Nov 2026): the engine's half peak to peak with its defaults over one period, 41 shots; difference as a fraction of 2 mrad | 0.0766 | 0.1 | relative | 77% |  | pass |
| `oracle.script_arm_tables` (S1) | George's script: the four arm tables of 1D_oscillator, fig2_10m, 100m_oscillators and fig2_uldm_folding against the closed-form arms with the floor, at (j + 1)·dt; largest difference | 6.357e-13 | 1.000e-12 | metres | 64% |  | pass |
| `oracle.script_rule_converges_100m` (S1) | George's script: the scripts' trapezoid on the 100m_oscillators scene, offset gone, on the scripts' own window (which ends one step short of 2T), at steps of 0.01, 0.005, 0.0025, 0.00125 s (the first is the fixture's rule, held to it to 1e-9) against the engine's own model: both the missing step and the trapezoid shrink as the step does; largest difference as a fraction of the peak | 1.688e-5 | 3.000e-5 | relative | 56% | 1.99 (expected 2 ± 0.2) | pass |
| `oracle.script_limit_fig2_10m_d` (S1) | George's script: fig2_10m panel d (a circle at 0.35625 Hz): the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by a step of dt/128, against the engine's own model; largest difference as a fraction of the peak | 5.041e-9 | 1.000e-8 | relative | 50% |  | pass |
| `oracle.script_limit_fig2_10m_c` (S1) | George's script: fig2_10m panel c (0.3275 Hz): the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by a step of dt/128, against the engine's own model; largest difference as a fraction of the peak | 4.295e-9 | 1.000e-8 | relative | 43% |  | pass |
| `oracle.script_limit_100m_oscillators` (S1) | George's script: 100m_oscillators: the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by two Richardson steps (dt, dt/2, dt/4), against the engine's own model; largest difference as a fraction of the peak | 3.298e-10 | 1.000e-9 | relative | 33% |  | pass |
| `oracle.published_anchor_pass_half_largest` (S3) | published paper: the published size of the moving anchor, about 7 mrad (10 kg at D = 10 m rising at 2.5 m/s; reference/design/instrument.md §12, Fig 12): George (24 Nov 2026) says it is the same pass; read, by the coordinator, as half the largest /ΔΦ/ (to be confirmed by George); the engine with its defaults, as a fraction of 7 mrad; tolerance the published sizes' 10% (S2) | 0.0248 | 0.1 | relative | 25% |  | pass |
| `oracle.script_limit_fig2_10m_b` (S1) | George's script: fig2_10m panel b: the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by a step of dt/128, against the engine's own model; largest difference as a fraction of the peak | 2.435e-9 | 1.000e-8 | relative | 24% |  | pass |
| `oracle.script_cuboid_copy` (S3) | George's script: cuboid: the script's rule copied (arms a step ahead of the wall, linear between steps, its window, each step integrated exactly, the closed-form prism for its spline) against its own table and rule with its quad replaced by exact integration (scripts/script_pipeline.py; its quad is 2.5e-5 of the phase off that); AC series, 20 shots; tolerance three times the script's own AC change when its table is doubled (3.57e-8 of the peak) | 2.029e-8 | 1.100e-7 | relative | 18% |  | pass |
| `oracle.script_cylinder_ratio` (S3) | George's script: cylinder: the ratio of the cylinder's largest AC /ΔΦ/ to the point mass's, 4 heights h0 × 9 heights H (R = 0.5 m, 100 kg, x0 = 3 + 0.1 cos 2π·0.1t), engine (voxel cylinder) against script: the ratios' largest difference | 7.303e-4 | 0.005 | relative | 15% |  | pass |
| `oracle.script_limit_1d_oscillator` (S1) | George's script: 1D_oscillator: the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by a step of dt/128, against the engine's own model; largest difference as a fraction of the peak | 9.993e-10 | 1.000e-8 | relative | 10% |  | pass |
| `oracle.scaffold_single_mode_paths` (S3) | exact self-consistency: scaffold_single_mode: the displaced-body read (every leaf of every member at α(τ)) against the table method (V(α) per node, Chebyshev in α), the same nodes, three shots, over the largest AC phase | 8.222e-8 | 1.000e-6 | relative | 8% |  | pass |
| `oracle.script_scaffold_series` (S3) | George's script: scaffold: the engine's AC series by the script's sampling (arms a step ahead, its window; 48 voxel members at 1 cm, Chebyshev in α), 50 shots, against the script's pipeline on its integrand integrated tightly (scripts/script_pipeline.py: its spline at 24 α × 480 heights, its interp1d arms, quad replaced by exact integration); tolerance three times the pipeline's last doubling (7.39e-4) plus 1e-3 for the members. Its published series is its nquad noise (3600 times the signal) and is not compared | 2.055e-4 | 0.0032 | relative | 6% |  | pass |
| `oracle.script_scaffold_multimode_series` (S3) | George's script: scaffold_multimode: the engine's AC series by the script's sampling, 100 shots, against the script's pipeline on its integrand integrated tightly (24 α × 480 heights, quad replaced by exact integration); tolerance three times the pipeline's last doubling (8.44e-6) plus 1e-3 for the members. Its published series is its nquad noise and is not compared | 2.955e-5 | 0.001 | relative | 3% |  | pass |
| `oracle.engine_rule_converged_100m` (S1) | exact self-consistency: the engine's audited read on the 100m_oscillators scene (a source 1 m from the atoms) against 12 nodes on 3 ms panels, 4000 shots; largest difference as a fraction of the peak | 2.742e-11 | 1.000e-9 | relative | 3% |  | pass |
| `oracle.script_limit_fig2_uldm_folding_d` (S1) | George's script: fig2_uldm_folding panel d (a circle at 0.1425 Hz): the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by two Richardson steps (dt, dt/2, dt/4), against the engine's own model; largest difference as a fraction of the peak | 7.346e-12 | 1.000e-9 | relative | 1% |  | pass |
| `oracle.script_limit_fig2_uldm_folding_c` (S1) | George's script: fig2_uldm_folding panel c (0.131 Hz): the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by two Richardson steps (dt, dt/2, dt/4), against the engine's own model; largest difference as a fraction of the peak | 4.827e-12 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.script_limit_fig2_uldm_folding_b` (S1) | George's script: fig2_uldm_folding panel b: the scripts' trapezoid with the offset gone, on the whole flight, taken to its limit by two Richardson steps (dt, dt/2, dt/4), against the engine's own model; largest difference as a fraction of the peak | 4.781e-12 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.script_same_time_1d_oscillator` (S1) | George's script: 1D_oscillator with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt) | 6.597e-5 | 0.02 | relative | 0% |  | pass |
| `oracle.script_scaffold_potential` (S3) | George's script: scaffold: V(α, z) − V(α₀, z) at 12 of the script's points, its own integrand integrated tightly (its table's nquad is far coarser than the change), against the engine's 48 voxel members (1 cm), over the largest change | 2.304e-5 | 0.01 | relative | 0% |  | pass |
| `oracle.script_scaffold_multimode_potential` (S3) | George's script: scaffold_multimode: V(α, z) − V(α₀, z) at 12 of the script's points (α to ±8), its integrand integrated tightly, against the engine's voxel members (1 cm), over the largest change | 2.301e-5 | 0.01 | relative | 0% |  | pass |
| `oracle.script_published_1d_oscillator` (S1) | George's script: 1D_oscillator as published (AION-10, 1 kg, D = 5 m, 0.1 Hz, 1000 shots), by the rule that copies its sampling; largest difference as a fraction of the peak | 1.599e-12 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.script_published_fig2_10m_c` (S1) | George's script: fig2_10m panel c as published (AION-10, 0.3275 Hz, above Nyquist), by the rule that copies its sampling; largest difference as a fraction of the peak | 1.037e-12 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.scaffold_multimode_paths` (S3) | exact self-consistency: scaffold_multimode_scalar_trick: the displaced-body read against the table method, the script's eight modes sharing one α, three shots, over the largest AC phase | 1.027e-9 | 1.000e-6 | relative | 0% |  | pass |
| `oracle.script_same_time_fig2_10m_d` (S1) | George's script: fig2_10m panel d (a circle at 0.35625 Hz) with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt) | 1.998e-5 | 0.02 | relative | 0% |  | pass |
| `oracle.script_cuboid_potential` (S3) | George's script: cuboid: the script's potential (its nquad) on its 50 × 50 grid, against the closed-form prism and the wall's voxel tree (7.5 cm, a grid that fits its faces); the worse of the two | 8.947e-14 | 1.000e-10 | relative | 0% |  | pass |
| `oracle.script_same_time_fig2_10m_c` (S1) | George's script: fig2_10m panel c (0.3275 Hz) with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt) | 1.549e-5 | 0.02 | relative | 0% |  | pass |
| `oracle.script_cuboid_limit` (S3) | George's script: cuboid: the script's rule with the offset gone, on the whole flight at dt/128, against the engine's read of the voxel wall; AC series, 20 shots; tolerance from the read's own audit (2.3e-10 rad of 2.86e-5) | 7.666e-9 | 1.000e-5 | relative | 0% |  | pass |
| `oracle.script_same_time_fig2_10m_b` (S1) | George's script: fig2_10m panel b with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt) | 9.934e-6 | 0.02 | relative | 0% |  | pass |
| `oracle.script_published_fig2_10m_d` (S1) | George's script: fig2_10m panel d as published (AION-10, a circle in the D-h plane at 0.35625 Hz), by the rule that copies its sampling; largest difference as a fraction of the peak | 2.918e-13 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.script_published_fig2_10m_b` (S1) | George's script: fig2_10m panel b as published (AION-10, 0.05 Hz, 10000 shots), by the rule that copies its sampling; largest difference as a fraction of the peak | 1.494e-13 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.script_cuboid_series` (S3) | George's script: cuboid: the engine's AC series minus the script's against what the script's rule predicts for it (its limit minus its copy, plus its quad's own error), shot by shot, over the largest prediction, two-sided; the limit rescaled, given the copy | 2.925e-6 | 0.02 | relative | 0% |  | pass |
| `oracle.script_published_100m_oscillators` (S1) | George's script: 100m_oscillators as published (AICE, 0.05 kg, D = 1 m, 0.08 Hz, 4000 shots), by the rule that copies its sampling; largest difference as a fraction of the peak | 7.818e-14 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.script_published_fig2_uldm_folding_d` (S1) | George's script: fig2_uldm_folding panel d as published (AICE, a circle in the D-h plane at 0.1425 Hz), by the rule that copies its sampling; largest difference as a fraction of the peak | 4.713e-14 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.script_published_fig2_uldm_folding_c` (S1) | George's script: fig2_uldm_folding panel c as published (AICE, 0.131 Hz, above Nyquist), by the rule that copies its sampling; largest difference as a fraction of the peak | 4.088e-14 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.script_published_fig2_uldm_folding_b` (S1) | George's script: fig2_uldm_folding panel b as published (AICE, 0.02 Hz, 4000 shots), by the rule that copies its sampling; largest difference as a fraction of the peak | 3.896e-14 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.v1_five_towers` (S1) | v1 oracle: v1's PropagationIntegral on the oscillation anchor read by each of the default array's five towers, engine set to v1's conventions; largest difference as a fraction of that tower's peak | 1.575e-15 | 1.000e-10 | relative | 0% |  | pass |
| `oracle.script_same_time_100m_oscillators` (S1) | George's script: 100m_oscillators with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt) | 3.075e-7 | 0.02 | relative | 0% |  | pass |
| `oracle.v1_oscillation` (S1) | v1 oracle: v1's PropagationIntegral on the published oscillation anchor (1 kg, D = 5 m, 0.1 Hz), 41 shots, engine set to v1's conventions; largest difference as a fraction of the peak | 1.476e-15 | 1.000e-10 | relative | 0% |  | pass |
| `oracle.v1_moving_pass` (S1) | v1 oracle: v1's PropagationIntegral on the published moving anchor (10 kg, D = 10 m, 2.5 m/s), 21 shots, engine set to v1's conventions; largest difference as a fraction of the peak | 1.016e-15 | 1.000e-10 | relative | 0% |  | pass |
| `oracle.periodogram_vs_scripts` (S1) | George's script: the engine's classical periodogram of each script's own series against the script's own spectrum: 1D_oscillator (no mean taken off, zero bin kept), and panels a to d of fig2_10m and fig2_uldm_folding, clean and with the script's noise; largest difference as a fraction of each spectrum's peak | 4.011e-15 | 1.000e-9 | relative | 0% |  | pass |
| `oracle.v1_static_point` (S1) | v1 oracle: v1's PropagationIntegral at v1-final on 100 kg at rest 2 m from the tower, engine set to v1's conventions; largest difference as a fraction of the peak | 2.200e-16 | 1.000e-10 | relative | 0% |  | pass |
| `oracle.script_same_time_fig2_uldm_folding_d` (S1) | George's script: fig2_uldm_folding panel d (a circle at 0.1425 Hz) with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt) | 3.815e-8 | 0.02 | relative | 0% |  | pass |
| `oracle.script_same_time_fig2_uldm_folding_c` (S1) | George's script: fig2_uldm_folding panel c (0.131 Hz) with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt) | 3.158e-8 | 0.02 | relative | 0% |  | pass |
| `oracle.script_same_time_fig2_uldm_folding_b` (S1) | George's script: fig2_uldm_folding panel b with only the one-step offset corrected: the engine minus the fixture against what the scripts' rule predicts (its limit minus the rule at the scripts' step and window), shot by shot; largest difference as a fraction of the largest predicted gap. Two-sided: it replaced a bound on the difference itself (acknowledged.txt) | 3.096e-8 | 0.02 | relative | 0% |  | pass |
| `oracle.fold_vs_script` (S1) | George's script: fold_frequency against fig2_folding_map.py's own, at 2001 frequencies from 0 to five times its Nyquist frequency of 0.1 Hz; largest difference | 0 | 1.000e-12 | relative | 0% |  | pass |
| `oracle.script_launch_rule` (S2) | George's script: the kick and the launch speed the AICE scripts computed as they ran (v_kick and u0 in the meta.json of 100m_oscillators and fig2_uldm_folding; fig2_final.py computes them by the same two lines) against the engine's kick n·ħk/m and its rule g·T − v_kick/2 with the scripts' constants; largest relative difference | 0 | 1.000e-12 | relative | 0% |  | pass |

## presets

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `presets.compare_equals_other_preset` (S2) | exact self-consistency: the comb read on AION-10 and, by `compare aice`, on AICE 140 m as well: the comparison's signal against the same scene loaded with the AICE instrument; values of any channel that differ in any bit | 0 | 0 | count | 0% |  | pass |

## render

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `render.gpu_projection` (S0) | closed form: the f64 camera's projection of a point on each tower; distance to the centre of the nearest pixel of that tower's colour, worst over five towers and four views. The measure resolves no finer than half a pixel's diagonal, 0.71 px | 0.7071 | 2 | pixels | 35% |  | pass |
| `render.gpu_perspective_ratio` (S0) | closed form: the ratio of the on-screen heights of a near and a far tower, as the f64 camera predicts it | 0.0023 | 0.03 | relative | 8% |  | pass |
| `render.gpu_determinism` (S0) | exact self-consistency: the same frame drawn twice, and by a second renderer on the same device; bytes that differ | 0 | 0 | count | 0% |  | pass |
| `render.gpu_local_frame_at_10_km` (S0) | exact self-consistency: end to end on the GPU: the infinite-floor scene moved by (10000, -7000, 300) m, camera and all, against the same scene at the origin; bytes of the picture that differ | 0 | 0 | count | 0% |  | pass |
| `render.local_frame_at_10_km` (S0) | exact self-consistency: CPU geometry only: the vertices of the same scene built at the world origin, each relative to its own target | 0 | 1.000e-5 | metres | 0% |  | pass |

## scenes

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `scenes.flash` (S1) | closed form: flash: a mass that exists for 1 ms reads, in the one shot in flight, the integrand at the middle of that millisecond times its length; largest difference over five towers as a fraction of that | 2.121e-6 | 1.000e-5 | relative | 21% |  | pass |
| `scenes.precision_far` (S1) | exact self-consistency: precision-far: the scene 10 km from the origin against the same scene at the origin, all five towers; largest difference as a fraction of each tower's peak | 1.024e-12 | 1.000e-11 | relative | 10% |  | pass |
| `scenes.switch_on` (S1) | exact self-consistency: switch-on: the shot in flight when the mass appears reads the always-there mass's integral from the switch to the readout (12 nodes on 2 ms panels); shots before read nothing, shots after read all of it; largest difference as a fraction of the full reading | 1.107e-13 | 1.000e-10 | relative | 0% |  | pass |
| `scenes.time_reversal` (S1) | closed form: time-reversal: with the launch that closes the arms at 2T (u0 = gT - v_kick/2), a pass played backwards reads the forward pass's series in reverse; largest difference over five towers as a fraction of the peak | 8.712e-16 | 1.000e-12 | relative | 0% |  | pass |
| `scenes.ring_orbit` (S1) | closed form: ring-orbit: a mass circling the centre tower at fixed radius and height reads the same every shot; largest departure from the mean as a fraction of it | 7.603e-16 | 1.000e-12 | relative | 0% |  | pass |
| `scenes.mirror_pair` (S1) | exact self-consistency: mirror-pair: towers 2 and 3 swap their readings of the two bodies, and towers 1, 4 and 5 read the same from each; values that differ, to the bit | 0 | 0 | count | 0% |  | pass |
| `scenes.superposition` (S1) | exact self-consistency: superposition: A alone, B alone, A + B; values of any channel, tower and shot where the reading of both is not the sum of the two, to the bit | 0 | 0 | count | 0% |  | pass |

## spec

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `spec.json_f64_exact` (S2) | exact self-consistency: 10^6 random finite f64 bit patterns and the awkward values (negative zero, subnormals, the largest), written as JSON and read back, as text and through serde_json::Value; values whose bits differ | 0 | 0 | count | 0% |  | pass |
| `spec.json_round_trip` (S0) | exact self-consistency: a spec of awkward numbers written to JSON and read back; numbers whose bits differ | 0 | 0 | count | 0% |  | pass |
| `spec.ron_f64_exact` (S2) | exact self-consistency: 10^6 random finite f64 bit patterns and the awkward values, written as RON and read back; values whose bits differ | 0 | 0 | count | 0% |  | pass |
| `spec.ron_round_trip` (S0) | exact self-consistency: a spec of awkward numbers written to RON and read back; numbers whose bits differ | 0 | 0 | count | 0% |  | pass |

## spectrum

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `spectrum.lomb_scargle_equals_fft` (S1) | closed form: on a uniform, gap-free grid Lomb-Scargle (Scargle's tau, mean taken off, times dt_eff) is the classical periodogram: 2000 shots of a comb, a tone and noise, every bin below Nyquist; largest relative difference | 3.900e-10 | 1.000e-9 | relative | 39% |  | pass |
| `spectrum.nufft_kernel_converges` (S1) | exact self-consistency: the NUFFT Lomb-Scargle's error against the exact sum at kernel widths 4, 6, 8, 10, 12, each as a fraction of its predicted ceiling 10^(2-w) of the peak (a decade a grid point, one decade in hand); the largest of the five | 0.0014 | 1 | relative | 0% |  | pass |
| `spectrum.nufft_lomb_scargle_long` (S1) | exact self-consistency: the NUFFT Lomb-Scargle on 100 000 jittered shots, 50 000 frequencies to 0.25 Hz, against the exact sum at 48 of them (the peak's bin, its neighbours, the rest spread evenly); largest difference as a fraction of the peak; the rounding floor is predicted near N*eps = 2e-11 | 4.717e-15 | 1.000e-9 | relative | 0% |  | pass |
| `spectrum.nufft_lomb_scargle_equals_exact` (S1) | exact self-consistency: the NUFFT Lomb-Scargle (two type-1 transforms, exponential-of-semicircle kernel 14 points wide, upsampling 2) against the exact N*M sum: 2000 shots at 2 s with up to 0.3 s of jitter, 1001 frequencies from 0 to 0.25 Hz; largest difference as a fraction of the peak | 3.111e-15 | 1.000e-9 | relative | 0% |  | pass |

## timing

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `timing.offset_shifts_one_tower` (S2) | exact self-consistency: the comb with tower 3 fired 0.292 s late: that tower reads what the synchronous scene reads at t + 0.292 s; largest difference as a fraction of the peak (the other towers must be unchanged to the bit) | 0 | 1.000e-12 | relative | 0% |  | pass |
| `timing.zero_offsets` (S2) | exact self-consistency: the comb's signal against what the engine at the tag s1-point-mass printed (a fixture: 5000 readout times and phases); and the comb and the pass with every tower's offset zero, against each shot read as S1 read it (the audited read at k·cycle + 2T, sources summed in the scene's order); values of any channel that differ in any bit | 0 | 0 | count | 0% |  | pass |

## validate

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `validate.fitted_order` (S0) | closed form: the report's own order fit, not the engine: the slope of log(3h²) against log h is 2 | 0 | 1.000e-12 | relative | 0% |  | pass |

## voxel

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `voxel.cube_to_point_order` (S3) | closed form: /V_cube − V_point///V_point/ on the axis at s/R from 0.1 to 0.01: order 4 ± 0.02, and at 0.01 within 1% above the leading term 7/480·(s/R)⁴ | 1.458e-10 | 1.473e-10 | relative | 99% | 4.00 (expected 4 ± 0.02) | pass |
| `voxel.cube_series_within_bound` (S3) | closed form: Gauss–Legendre 24³ quadrature of the cube; s/R from 0.02 to 0.3, 64 directions; orders 0, 4, 8, 12: the worst error as a fraction of the series' own bound | 0.4163 | 1 | relative | 42% |  | pass |
| `voxel.cube_kernel_exact` (S3) | closed form: Gauss–Legendre quadrature of the cube (24³, 48³ close) from s/R = 1e-4 to 0.8 in 64 directions: series or prism, as the kernel chooses | 1.399e-14 | 1.000e-13 | relative | 14% |  | pass |
| `voxel.mass_is_the_spec` (S3) | exact self-consistency: the spec's mass, against the Morton-order sum of the leaves' masses, 17 shapes at 32 cells across | 3.259e-14 | 1.000e-12 | relative | 3% |  | pass |
| `voxel.volume_and_inertia` (S3) | closed form: each primitive's and Platonic solid's closed-form volume and inertia tensor at 64 cells across, k = 4: the worst error as a fraction of its derived envelope (volume √3·A·δ/V, A its surface, δ = h/k; inertia 2·M·r²max times that over the largest principal moment) | 0.0188 | 1 | relative | 2% |  | pass |
| `voxel.merged_equals_unmerged` (S3) | exact self-consistency: the same body voxelised unmerged (the raster's finest cells, made apart from the merged tree): potentials at 24 points from 0.6 to 12 m and one AION-10 shot 2 m away, each primitive and Platonic solid at 32 cells across | 1.666e-14 | 1.000e-12 | relative | 2% |  | pass |
| `voxel.planar_mass_is_the_prism` (S3) | closed form: planar_mass (no script): a 2 × 2 × 0.25 m plate voxelised on a grid that fits its faces, against the closed-form prism; one AION-10 shot, three channels over the larger single interferometer's phase | 5.341e-15 | 1.000e-12 | relative | 1% |  | pass |
| `voxel.sphere_reads_as_the_sphere` (S3) | closed form: a uniform sphere (a point outside itself), r = 0.5 m, 100 kg, 1.6 m off a tower's axis: one AION-10 shot, three channels, voxelised at r/16 and k = 4 | 1.919e-7 | 0.002 | relative | 0% |  | pass |
| `voxel.tree_equals_raster` (S3) | exact self-consistency: the raster voxeliser over every cell: 17 shapes (primitives, Platonic solids, a random hull, three meshes) at 8, 13 and 21 cells across with k = 1, 2, 4, placed at random | 0 | 0 | count | 0% |  | pass |

## world

| Case | Checked against | Measured | Tolerance | Unit | Of tolerance | Order | Result |
|---|---|---|---|---|---|---|---|
| `world.moments_bound_is_not_vacuous` (S4) | exact self-consistency: the same: the bound over the error, the loosest over every q tried (0.45 to 0.001) where the error is above rounding | 457.0544 | 1000 | relative | 46% |  | pass |
| `world.slab_converges_to_the_null` (S4) | closed form: the same slab: its gradiometer halves as its width doubles (a finite floor's edge term falls as 1/L, to the infinite floor's null); the worst ratio's relative distance from 2 | 0.0197 | 0.05 | relative | 39% |  | pass |
| `world.slab_equals_the_prism` (S4) | closed form: a square slab of the world, 320 m to 2560 m across and 32 m thick, read at the centre tower: its gradiometer against the prism's closed form, the worst relative difference | 2.835e-11 | 1.000e-9 | relative | 3% |  | pass |
| `world.moments_within_their_bound` (S4) | exact self-consistency: a chunk of terrain and a house (two materials) read through its moments to the octupole, against its leaves: the worst error over the stated bound, q = a/R from 0.45 to 0.001 (controls: the quadrupole left out breaks the bound from q = 0.1 in, the octupole flipped from 0.05, the octupole left out from 0.02) | 0.0228 | 1 | relative | 2% |  | pass |
| `world.box_reads_as_a_voxel_body` (S4) | exact self-consistency: a box of concrete over four chunks of the world against the same box made a voxel body: the worst relative difference of any channel of any tower | 4.565e-16 | 1.000e-12 | relative | 0% |  | pass |
| `world.box_reads_as_the_prism` (S4) | closed form: a box of concrete on the world's cells: each tower's gradiometer against the closed form of the prism, the worst relative difference | 1.739e-13 | 1.000e-9 | relative | 0% |  | pass |

## Convergence

<picture><source media="(prefers-color-scheme: dark)" srcset="convergence-dark.svg"><img src="convergence-light.svg" alt="Error against step size for each convergence case, with the expected rate drawn in." width="100%"></picture>

## Over time

<picture><source media="(prefers-color-scheme: dark)" srcset="history-dark.svg"><img src="history-light.svg" alt="Each case's error as a fraction of its tolerance at every commit that changed this report." width="100%"></picture>

The history is read from the earlier versions of `report.json` in git.
