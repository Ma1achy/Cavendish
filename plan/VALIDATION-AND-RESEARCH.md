# Cavendish rebuild — validation from the AION oscillatory-noise work, features, and research

> Companion to `plan/REBUILD.md`. Sources: Parish, Bason & McCabe, *Oscillatory Gravity Gradient Noise in
> Long-Baseline Atom Interferometer Gradiometers: Identification and Mitigation Techniques* (preprint);
> Carlton & McCabe et al., *From RATs to riches* (arXiv:2308.10731); G. Parish, *PhD upgrade report*
> (ch. 2, "Oscillatory Noise Characterisation"); the reference scripts at
> `github.com/Thranduil02/atom-interferometry` (38 Python scripts, the code behind the figures).
> The upgrade report was read by section structure and its chapter-2 headings, not cover to cover;
> the builder should read ch. 2 and 3 in full before slice S1's brief.

---

## 1. The reference instruments

| Preset | Baseline / IFO separation | T (half) | Launch u₀ | Cycle | Nyquist | Notes |
|---|---|---|---|---|---|---|
| **AION-10** | 10 m / δr = 5 m | 0.73 s (2T = 1.46 s) | 3.86 m/s | 2 s | 0.25 Hz | the scripts' `1D_oscillator`, `fig2_10m` |
| **AICE 100 m** | 70 m gradiometer | 2.5 s | g·T − v_kick/2 = 21.27 m/s | 5 s | 0.1 Hz | `fig2_final.py` and the cadence studies; T_obs = 3×10⁷ s |

Shot-noise floors used throughout: 10⁻⁴ rad/√Hz (baseline) and 10⁻⁵ rad/√Hz (stretch). ULDM benchmark:
m_φ = 2×10⁻¹⁶ eV at current MICROSCOPE bounds gives ~10⁻² µrad. Both presets ship in the scene library
and the array editor (S2), and every validation below runs on both.

## 2. Validation suite — results the engine must reproduce

Each item is a named test in the oracle tier unless marked *quick*. "Script" means: run the reference
script headless (matplotlib Agg, reduced N, fixed seed), save its arrays as a fixture once, and compare.
The scripts are the oracle for the analysis layer as v1 is for the forward model.

### 2.1 Single oscillators — the harmonic comb (paper §II.B; report §2.2–2.3)

| Test | Asserts | Against |
|---|---|---|
| `point_oscillator_phase_matches_script` | phase series of a 1-D vertical point-mass oscillator (AION-10 and AICE) | `1D_oscillator`, `100m_oscillators`; ≤1e-9 relative |
| `comb_at_integer_harmonics` | periodogram peaks at n·f₀, n = 1…4, and nowhere else above the noise | script; peak positions to one bin |
| `harmonic_heights_fall_as_amplitude_ratio` | the n-th harmonic falls by ~(A/√X)ⁿ relative to the fundamental (X the squared distance scale of the paper) | the paper's suppression law; log-slope within 10% over a sweep of A |
| `uldm_has_no_comb` *quick* | an injected ULDM tone produces one peak, no harmonics | analytic |
| `nyquist_reflection` *quick* | a harmonic with f_N < n·f₀ < 2f_N appears at 2f_N − n·f₀; the general fold is the triangle wave of `fold_frequency` | analytic + `fig2_folding_map.py` |
| `folded_comb_fingerprint` | several folded harmonics land on observed peaks, each where the fold predicts | `fig2_uldm_folding`, `fig2_10m` |

### 2.2 Motion geometry — direction from harmonic ratios (paper §II.C; report §2.3.5–2.5)

| Test | Asserts | Against |
|---|---|---|
| `circle_ratio_vs_tilt` | for a circular orbit tilted by θ in the (D, h) plane, the second-to-first harmonic height ratio is maximal at θ = 0°, 180° (horizontal) and minimal at 90°, 270° (vertical) | `fig2_final.py`, `fig2_circle_summary.py` |
| `ratio_symmetries` *quick* | the ratio is unchanged by θ → 180° − θ (reflection) and by reversing the sense of circulation (the sequence is symmetric in time) | the paper's symmetry argument |
| `horizontal_orbit_drives_fundamental_through_D_only` | for a horizontal orbit the w-motion appears only from 2f₀ upward | paper §II.C |
| `linear_direction_sweep` | 1-D oscillation at angle β: ratio vs β | `fig2_directions_1d.py`, `fig2_multi_directional.py` |
| `pendulum_and_sphere` | simple pendulum (L = 14.5 m, a = 0.5, b = 0.25 in the script) and forced motion on a sphere | `fig2_multi_directional.py`; report §2.4.2, §2.5.1 |

### 2.3 Extended bodies — where the point mass breaks (report §2.6; paper §II.C)

| Test | Asserts | Against |
|---|---|---|
| `cylinder_vs_point_mass` | max ‖ΔΦ‖ over radius R and height H, at several heights h₀, against a point mass; the error passes 10% when the body's vertical extent passes ~10% of the baseline | `cylinder` |
| `cuboid_potential_matches_script` | the script's cuboid ("shaky wall") potential vs the engine's prism kernel and its voxel tree | `cuboid`; ≤1e-10 |
| `planar_mass` | an oscillating plate | report §2.6.1 |
| `scaffold_single_mode` | aluminium-cylinder scaffold, amplitude rising linearly from 1 µm at z = 0 to 1 mm at z = 10 m, x-displacement A_i·cos(2πft) | `scaffold` |
| `scaffold_multimode_scalar_trick` | several modes sharing one amplitude profile: displacement is A_i·α(t) with scalar α, so one V(α, z) table serves all modes — the engine's displaced-body path must agree with the table method | `scaffold_multimode` |

These are also the natural first users of the voxel tree: the scaffold is exactly a displaced body
(M12's `Parcels::displaced`), and it is the first validated case where the point approximation fails.

### 2.4 Sampling and analysis (paper §III–IV)

| Test | Asserts | Against |
|---|---|---|
| `lomb_scargle_equals_fft_on_uniform_grid` *quick* | LS reduces to the classical periodogram for uniform, gap-free sampling | analytic |
| `ls_is_least_squares_sinusoid` *quick* | S_LS ∝ variance explained by the best-fit a·cos + b·sin at each trial frequency | definition |
| `random_jitter_floor_recedes` | T_c ~ U(T_nom − δ, T_nom + δ): diffuse floor around the peak whose height *relative* to the peak falls with N | `cycle_jitter_sweep`, `cadence_bragg_comparison` |
| `alias_ratio_vs_n` | positive jitter T_c ~ U(T_nom, T_nom + δ), T_nom = 5 s: alias-to-peak ratio vs N for δ ∈ {0, 0.1, 0.2, 0.3} s, mean and spread over seeds | `cadence_alias_ratio`, `alias_ratio_N`, `jitter_alias_ratio_full`, `alias_suppression_factor` |
| `alternating_cadence_sidebands_persist` | T = 5.5, 6.0, 5.5 … s: sidebands at f_sig ± k/T_super (T_super = 11.5 s) whose relative height does **not** fall with N; spectral window as the paper's Eq. (24) | `alternating_cadence_aice` |
| `lost_shots_floor_and_dmin` | uniform cadence, each shot lost with probability p ∈ [0, 0.05]: LS noise floor and minimum detectable ULDM coupling d_min(p) | `lost_shots_aice(_parallel)`, `gapped_fft_vs_ls(_averaged)` |
| `zero_filled_fft_fails_ls_does_not` | on gapped data | `gapped_fft_vs_ls` |
| `lift_excision_recovers_tone` | Poisson lift transients (~1 per 15 min; 1000 kg at 10 m, −10 → 20 m at 1 m/s), excised with padding, LS recovers f_sig | `lift_excision_demo`, `lift_uldm_osc_pipeline` |
| `notch_filter_removes_known_tone` *quick* | least-squares â cos + b̂ sin at f_c subtracted; neighbouring tones untouched | paper §IV |
| `week_and_night_schedules` | a 7-day night-only run (18:00–08:00 ± 30 min), lift excision, MOT failures, thinning | `week_pipeline`, `nighttime_gapped_series`, `thinning_pipeline` |

## 3. Features these sources suggest

1. **An analysis layer in the engine, not only in Python:** Lomb–Scargle (exact and NUFFT-accelerated),
   the classical periodogram, notch filters, transient detection and excision, folding maps, ULDM
   injection and d_min estimation. Exposed in the GUI, the CLI and MCP.
2. **Source fingerprinting:** given a periodogram, find harmonic combs including folded ones, propose
   (f₀, harmonics, folded-or-not), and estimate orbit geometry from harmonic height ratios. A natural
   feature for Gradar too: classify an oscillator's motion from its comb.
3. **A cadence designer:** set cycle time, jitter (symmetric, positive-only), alternation, planned gaps,
   loss probability and schedules; see the spectral window, predicted alias ratios and d_min live, and
   export the schedule into the scene spec. This is where per-tower timing (M15) earns its keep.
4. **Instrument presets** (AION-10, AICE 100 m, user-defined), including the AICE launch rule
   u₀ = gT − v_kick/2.
5. **Oscillator and structure library:** point, linear, circular, pendulum, forced-sphere oscillators;
   plates, "shaky walls", cylinders; the AION scaffold as a displaced multimode body.
6. **Periodogram views in the app:** live, per tower, with the comb and fold overlay, the noise floors,
   the ULDM tone, and selecting a peak highlights the source that makes it (via the contribution field).
7. **A "what would this look like at AICE" toggle:** the same scene re-run on the other preset.
8. **Paper-ready figures:** the paper's house style (log-y periodograms, grey floors, stems for the
   comb, a diamond for ULDM) as a figure preset, exportable as SVG/PDF from the paper draw style.

## 4. Research for the solids, vibration and deformation — and the fluids

The owner's instinct is right: vibrating and deforming structure is a finite-element problem, and the
voxel world is a natural mesh for it. What the literature offers, to be researched properly by the
builder's research agents before S7–S10:

- **Voxel finite elements.** Each uniform leaf is a trilinear hexahedral element; micro-CT bone
  mechanics solves 10⁸–10⁹-element voxel models matrix-free with multigrid, and GPU voxel FE codes do
  explicit elastic-wave propagation at scale. For Cavendish: modal analysis of structures (walls, floors,
  the scaffold) gives mode shapes and frequencies; the displaced-body path then plays modes back.
  Octree grading needs hanging-node constraints, which is the same 2:1 balance as the adaptive fluid.
- **Material point method (MPM, MLS-MPM)** on a sparse grid unifies elastic, plastic, granular and fluid
  matter with collisions — the "one solver for everything that deforms" option, at the cost of
  accuracy control. Worth a spike for soft and granular matter; not the first choice for small-amplitude
  structural vibration, where modal FE is exact and cheap.
- **Fluids:** the plan's MAC/Boussinesq projection solver with an adaptive octree stays the default
  (it conserves mass and has a validated benchmark). Lattice Boltzmann on GPU adaptive grids is the
  serious alternative for room airflow and natural convection and maps well to voxels; a research
  spike should compare both on the heated cavity and a room plume, against cost.
- **Small-amplitude vibration and the instrument:** displacements are µm–mm and the gravity read is
  linear in displacement to first order, so a structure's signal is (mode shape × response map) ×
  modal amplitude: compute one weight per mode, then a time series costs one dot product per mode.
  The scaffold script's scalar-α trick is the single-mode version of exactly this.

**Research programme for the builder.** Before each physics slice, spawn research agents (cloud
sessions are fine) that each produce a short, sourced note in `docs/research/`: the methods, their
accuracy and cost at Cavendish's scales, the validation cases the literature uses, and a
recommendation. A reviewer agent checks the note's sources. The slice brief cites the notes.
Starting topics: voxel and octree FE with hanging nodes; matrix-free multigrid on GPUs; modal
synthesis for gravity read-out; MPM vs FE for deformables; LBM vs projection for indoor air; GPU
sparse-grid data structures (NanoVDB, SPGrid, 64-trees); NUFFT-based Lomb–Scargle; seismic Newtonian
noise models; the Beecroft site measurements.

## 5. How this changes the slices

- **S1** (a point mass) also delivers the oscillator library, the Lomb–Scargle and classical
  periodograms, and the §2.1 tests: a point oscillator's comb and its folding are the first thing the
  app shows that v1 never did.
- **S2** adds the AICE preset and the cadence designer (§3.3) alongside per-tower timing.
- **S3** adds cylinders, cuboids and the scaffold, and the §2.3 tests: the first place the voxel tree
  beats the point mass on a published case.
- **S6** adds the §2.2 motion-geometry tests and source fingerprinting.
- **S7** gains a structural-dynamics track (modal voxel FE) after its research note.
- **S11** reproduces the paper's sampling results (§2.4) through the dataset path, on both presets.
