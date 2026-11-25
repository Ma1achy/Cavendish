# Gradar: what to learn, and how to tell whether it learned it

> Research note for slice S11 ("Datasets and Gradar", `plan/REBUILD.md` §9; tasks TASK-S11-05 to
> -07), written 25 Nov 2026. It answers: what Gradar should learn from Cavendish's phase series, which
> non-learned baseline and which bound it is judged against, and what S11 should build first.
>
> **How it was made.** Every paper cited by arXiv number was downloaded as a PDF from
> `arxiv.org/pdf/<id>` and read as text (`pdftotext`); section, equation or figure numbers are given
> where a claim rests on one. Li & Oldenburg (1998) was read from the copy on the UBC Geophysical
> Inversion Facility's site. What could not be opened is listed at the end. The numbers marked
> *measured here* come from a small NumPy benchmark (`bench/crb_bench.py`, beside this note, with its
> full output in `bench/run300.txt`; §8), run on the build PC with `python crb_bench.py 300`. Nothing in the repository was changed.
>
> **Second pass (25 Nov 2026).** Eight papers the first pass could not open were then supplied as PDFs
> by the owner and read as text (`pdftotext`; the two Extended Data tables of Stray et al., which are
> images, were read from rendered pages). They are cited below with page, section or equation numbers
> and marked **[read, 2nd pass]**. How much of each was read is in §9.

## Changed after reading the primary sources (25 Nov 2026)

No recommendation for the S11 build order moved. Five claims in the first pass were recalled or
secondhand and are corrected, one is sharpened, and one source turned out to be unusable.

| Topic | First pass said | The source says | Effect |
|---|---|---|---|
| **Euler deconvolution** (§2.3) | "Needs dense gridded gradient data"; unsuitable by my reasoning from inputs | Not so: Zhang et al. run it on profiles and ungridded data too (p. 513, 516). The real obstacle is the **inputs per station**: conventional Euler needs the field T_z **and** its three gradients (Zhang eq. 1–2); tensor Euler needs the gravity vector and all six tensor components (eq. 3–4, "3n equations containing six unknowns"). Cavendish's tower measures **one** differenced component and no field. Reid et al. also say the method "should be directly applicable to gravity gradiometer measurements" (Conclusion, p. 88) and want profile spacing no larger than the source depth (Discussion, p. 87). | Verdict stands (still not recommended), now on checkable reasons, and the "dense grid" reason is dropped. §2.3. |
| **Depth weighting for gradients** (§2.2) | "β nearer 3 would be the analogue"; Li 2001 "not read" | Li 2001 uses `w(z) = (z + z₀)^(−3/2)` (the paper's second eq. numbered (5)), i.e. β = 3 in Li & Oldenburg's `(z + z₀)^(−β/2)`; `z₀` is fitted so `w²` matches the decay of the sensitivities ("inverse distance cubed"). | Guess confirmed. Li 2001 offers little else beyond the five-component data vector, cell-wise bounds and wavelet compression: it is a four-page abstract (§2.2). |
| **CRB conditions, efficiency** (§3.1) | Cited via Vallisneri only; Kay "not read" | **The supplied Kay PDF is front matter only** (5 pages: cover, title, contents). It cannot support a theorem citation. The contents locate the relevant sections (§3.4 p. 30, §3.9 p. 47 "CRLB for the General Gaussian Case", §7.5 p. 164, §7.9 p. 190). | Nothing changed; the conditions are given in §3.1 as *from memory, unverified*, with the pages to fetch. |
| **Joint score ("mining gold")** (§4) | "A route to better sample efficiency later" for NPE | Brehmer et al.'s losses (eq. 7, 9, 12–14) train **likelihood-ratio and likelihood** surrogates (ROLR, RASCAL, CASCAL, SCANDAL) and a locally sufficient statistic (SALLY, eq. 15); none trains a posterior. Gains are for **small parameter differences**; "less powerful for deciding between model parameter points that are far apart". | The idea does not help the first model (amortised NPE over a wide prior). Moved to "later, and only for a likelihood or ratio surrogate" (§4). |
| **Seismic noise** (§6) | Harms 2015 only | Hughes & Thorne is about the *force* on a test mass (their footnote 1: "not gradients of the local gravitational field ... but simply the direct fluctuation of the gravitational force"); Rayleigh modes only, Love modes give none; reduced transfer function β′ 0.35–0.6 quiet, 0.15–1.4 noisy (abstract, eq. 1.28, 1.30). | Adds a magnitude scale and a mode picture; the Fisher matrix still needs the full Σ (§6). |
| **Noise model** (§1, §3) | "Instrument noise is additive (shot noise)" | True that it is additive and Gaussian-like, but Stray et al.'s field budget is **not shot-noise-limited**: detection (background light 344 E/√Hz, photodiode 144 E/√Hz) exceeds atom shot noise (184 E/√Hz) (Extended Data Table 1). | σ per tower is an input to `fisher()`, not a constant; test with and without a detection floor (§2.6, §7.3). |

New in this pass: §2.6 (what Stray et al. report, with numbers a Cavendish validation scene can
use), a field-anchored test row in §7.3, and §9's list of what was and was not read.

---

## 1. One-page summary

**The problem is unusually kind, and S11 should exploit that before it trains anything.**
Cavendish's forward model is deterministic, its phase is **linear in mass** and smooth in position
and velocity (`reference/milestones/M17-site-and-baseline.md` §3), and the instrument noise the plan
models is additive (shot noise, 10⁻⁴ rad/√Hz baseline, `plan/VALIDATION-AND-RESEARCH.md` l. 20). For
a scene of a few point-like bodies in shot noise, the likelihood is therefore *known in closed
form*: a Gaussian around the forward model. That gives three things learning problems rarely have:
an exact Fisher matrix, a maximum-likelihood estimator that reaches the Cramér–Rao bound at
high SNR ([Vallisneri 2008](https://arxiv.org/abs/gr-qc/0703086), §II D; the textbook theorems are
Kay's, which could not be cited from the supplied file, §3.1), and an exact posterior
against which a learned posterior can be checked. Learning earns its place only where the
likelihood stops being tractable: unknown numbers of bodies, ambient populations, seismic Newtonian
noise, masks and gaps, instrument nuisances. That is the territory simulation-based inference was
built for ([Cranmer, Brehmer & Louppe 2020](https://arxiv.org/abs/1911.01429)).

| Question | Answer |
|---|---|
| 1. Inversion and localisation | **Matched-filter bank + Gauss–Newton (profiled maximum likelihood)** is the baseline; it is what GW searches do ([Owen & Sathyaprakash 1998](https://arxiv.org/abs/gr-qc/9808076); [Allen et al. FINDCHIRP](https://arxiv.org/abs/gr-qc/0509116)). Regularised grid inversion (Li & Oldenburg) is non-unique by construction and was measured here at 47–62× the CRB in range at high SNR; keep it as a second, labelled-weak baseline, with Li's gradiometer depth weighting `(r + r₀)^(−3/2)` ([Li 2001](https://doi.org/10.1190/1.1816383), eq. (5)) in distance from the array. Euler deconvolution ([Reid et al. 1990](https://doi.org/10.1190/1.1442774); [Zhang et al. 2000](https://doi.org/10.1190/1.1444745)) needs the field and its gradients, or the full tensor, at every station; five single-component towers supply neither, so it is not used (§2.3). The field precedent for atom-gradiometer inversion is a parametric-model Bayesian fit sampled with NUTS ([Stray et al. 2022](https://doi.org/10.1038/s41586-021-04315-3), Methods), the "exact posterior" reference this note already proposes (§2.6). |
| 2. Fisher and CRB | `F = JᵀΣ⁻¹J` from **central finite differences of the f64 CPU forward model**: mass derivative is exact (linearity), position steps of 10⁻³–10⁻⁴ m give 4×10⁻⁸–4×10⁻¹⁰ relative error; **f32 phases make it useless** (2×10⁻³ error at the same step), so never from the GPU path (*measured here*). Parameters: mass, position and velocity at a reference time (5 per body for straight tracks). Check each test scene with Vallisneri's \|log r\| ≤ 0.1 criterion before quoting a bound. |
| 3. SBI | Neural posterior estimation (NPE) with a normalising flow and an embedding network, **amortised**, trained on simulations with noise drawn at batch time, exactly as DINGO does for LIGO ([Dax et al. 2021](https://arxiv.org/abs/2106.12594)). Validate with simulation-based calibration ([Talts et al. 2018](https://arxiv.org/abs/1804.06788)) and expected coverage ([Hermans et al. 2022](https://arxiv.org/abs/2110.06581)), and on the tractable scenes against the exact posterior. Ensembles for conservativeness. |
| 4. Architecture | A **set network over towers** (Deep Sets / Set Transformer) with a **per-shot time embedding** (mTAN, SeFT) and the mask as input: tower order must not matter, cadence is ragged. No imputation. Domain randomisation for sim-to-real. |
| 5. Detection in noise | Wiener filtering with witness sensors ([Coughlin et al. 2016](https://arxiv.org/abs/1606.01716); [Harms 2015](https://arxiv.org/abs/1507.05850) §7.1) and array averaging ([Chaibi et al. 2016](https://arxiv.org/abs/1601.00417)) for stationary ambient fields; forward-model subtraction for finite moving sources; the rolling-σ mask ([Carlton & McCabe 2023](https://arxiv.org/abs/2308.10731)) as the transient baseline. A gravity witness also cancels signal (Harms §7.1.7): test for it. |
| 6. Datasets | Store **noise-free unit-mass signals** and draw noise, mass scale and masks at batch time; split by scene family and seed hash (the plan's rule); every window carries tier, truncation and the Fisher matrix of its targets. 10⁵–10⁶ windows for NPE is the literature's scale (DINGO 5×10⁶). |

**Recommendation for S11 (detail in §7).** Build, in order: (1) the exact-likelihood toolkit —
forward model as a function of θ, finite-difference Jacobian, Fisher matrix, CRB, and the \|log r\|
check; (2) the matched-filter + Gauss–Newton baseline, validated by **attaining the CRB** (RMSE/√CRB in
[0.8, 1.25]) on high-SNR scenes, with a control in which a wrong Fisher matrix fails; (3) the grid
inversion as the second baseline; (4) only then the first model: amortised NPE for one body on a
straight track, judged by SBC, coverage and RMSE against the CRB and against the baseline.

**The exit criterion "CRB ≤ baseline error" should be sharpened.** As written it cannot fail for any
unbiased estimator and any correctly computed bound — it only catches a wrong Fisher matrix. Worse, a
learned model with a prior can legitimately beat the CRB at low SNR, because the CRB binds unbiased
estimators only (Vallisneri §II C, "biased estimators can actually outperform unbiased estimators").
Measured here: the bounded baseline itself "beats" the CRB on position at SNR ≈ 6 (§8). Proposed
instead: *on scenes where \|log r\| ≤ 0.1, baseline RMSE/√CRB ∈ [0.8, 1.25] for every parameter
(measured 0.96–1.07 at SNR 226–814, and a wrong-σ control fails at 1.9–2.1); elsewhere, report the
ratio and the coverage, and compare the model with the exact posterior.*

---

## 2. Inversion and source localisation (question 1)

### 2.1 What Gradar's signal is

One point mass `M` on a track `x(t)` gives, at tower `d` and shot `s`, `φ_ds = M · w_d(x(t_s + ·))`,
with `w` the tower's response to a unit mass integrated over the 2T = 1.46 s flight (M17 §3; the arm
integral is the AION collaboration's eq. 11 in [Carlton & McCabe 2023](https://arxiv.org/abs/2308.10731),
as `docs/research/s15-instruments-anywhere.md` §1 records). Linear in `M`, non-linear in the track,
falling as r⁻³ in the far field (M17 test `far_field_scales_as_inverse_cube`). The default array is
five towers, a centre and four at 4 m on a cross, firing every 2 s (`crates/cav-core/src/defaults.rs`).

### 2.2 Potential-field inversion (Li & Oldenburg 1998)

- **Non-uniqueness is fundamental, not numerical.** "There are infinitely many equivalent density
  distributions beneath the surface that will reproduce the known field. One such distribution is an
  infinitesimally thin layer of mass just beneath the surface", so gravity data have "no inherent
  depth resolution" ([Li & Oldenburg 1998](https://gif.eos.ubc.ca/sites/default/files/Li_1998a.pdf),
  introduction; [doi](https://doi.org/10.1190/1.1444302)).
- **Remedy: a model objective with depth weighting**, minimised subject to a target misfit. Because a
  minimum-norm model "will generate a density distribution that is concentrated near the surface",
  they weight by `w(z) = (z + z₀)^(−β/2)` to counter the kernels' decay; β between 1.5 and 2.0 gave
  satisfactory results for gravity (same paper, "Depth weighting"). Positivity is imposed as a bound.
- **What Li 2001 adds for gradiometer data** **[read, 2nd pass]**
  ([Li 2001, SEG expanded abstract, 4 pp.](https://doi.org/10.1190/1.1816383)). (1) Data are the five
  independent components of the symmetric, traceless tensor (eq. 2, 3), stacked: "if five independent
  components are measured at p locations, then N = 5p". (2) The same Tikhonov objective (eq. 5, 8) with
  a depth weight matched to the gradient kernel: "the sensitivity of the gradiometer data decays as
  inverse distance cubed", so `w(z) = (z + z₀)^(−3/2)` (the second equation numbered (5)), with `z₀`
  chosen "to allow the best match between the decay of the sensitivity and the square of the weighting
  function". In Li & Oldenburg's `(z + z₀)^(−β/2)` that is β = 3, against the β ≈ 1.5–2 quoted above
  for gravity. (3) Cell-wise lower and upper bounds enforced by an interior-point log barrier (eq. 8),
  which also lets known cells be frozen. (4) Wavelet compression of the sensitivity matrix, from
  Li & Oldenburg 2000, *Geophys. J. Int.* (cited there as "accepted"; not read), for
  large grids. (5) The observation that gradiometer data are mathematically a magnetic problem with a
  known magnetisation direction, so "much of the insight ... from magnetic data is applicable". Its one
  example is a synthetic salt body, 200 m stations, 1 E Gaussian noise, five components, "the bottom
  of the salt is well imaged"; there is no resolution analysis, no single-component case and no
  comparison with other methods. So it supports the weight, not a claim about accuracy.
- **For Cavendish:** the analogue of depth is distance from the towers, and the kernel decays as r⁻³,
  so the weight is `(r + r₀)^(−3/2)` in distance from the array (Li weights in z only because his data
  lie on one surface; that is an adaptation, not something Li 2001 does). The array sees a moving body, so the grid must be per time window and the
  window short enough that the body barely moves — which leaves 5 towers × 5 shots = 25 numbers for
  hundreds of cells. *Measured here* (§8): a positivity-constrained Tikhonov inversion with a
  discrepancy-principle λ recovers the along-track position but the range and the mass badly
  (range RMSE 47–62× the CRB at high SNR, mass errors comparable with the mass). That is exactly the non-uniqueness Li & Oldenburg describe: mass and
  distance trade off. It is the plan's baseline (b) (M17-R6) and worth keeping as a *labelled-weak*
  yardstick, because it is the only baseline that needs no track model and so extends to extended
  and many bodies.

### 2.3 Euler deconvolution

**[read, 2nd pass: Thompson 1982 §"Euler's equation", "Implementation"; Reid et al. 1990 Theory, Solution
strategy, Discussion, Conclusion; Zhang et al. 2000 in full text]**

- **The method.** A field homogeneous of degree −N satisfies `(x−x₀)∂T/∂x + (y−y₀)∂T/∂y + (z−z₀)∂T/∂z
  = −N(T−B)`, with `B` a regional value ([Thompson 1982](https://doi.org/10.1190/1.1441278), eq. 5;
  [Reid et al. 1990](https://doi.org/10.1190/1.1442774), eq. 1). Within a moving window of stations it
  is solved by least squares for `(x₀, y₀, z₀, B)` at a chosen structural index N; solutions are kept
  only if the fitted depth's standard deviation is small (Reid: "less than, say, 15 %", step 3a;
  Thompson: an acceptance tolerance, eq. 9) and are judged by how they cluster over several indices.
  Thompson uses a seven-point window and five indices; Reid a 10×10 window (100 equations, four
  unknowns), 3×3 at the very least, with minimum returned depth "about the same as the grid interval"
  and maximum "about twice the window size" (p. 88). Reid p. 81: a wrong index gives "scattered
  solutions and biased depths".
- **Zhang's tensor form** ([Zhang et al. 2000](https://doi.org/10.1190/1.1444745), p. 513). Conventional
  Euler on gravity takes the vertical field `T_z` and its three gradients (eq. 1: `(x−x₀)T_zx +
  (y−y₀)T_zy + (z−z₀)T_zz = N(B_z − T_z)`, four unknowns). Tensor Euler adds the two horizontal
  components (eq. 3, 4): "n data points give rise to 3n equations containing six unknowns", solvable
  for n > 2. A point mass satisfies it exactly with structural index 2 for the field (eq. 15–17) and
  the tensor falls as degree −3 (eq. 9–14). They find it clusters better than conventional Euler on
  Bell Geospace survey data (0.5 E resolution at 2 km wavelength, 250 m grid, p. 515–516), on profiles,
  grids and ungridded data alike. Their prism and cylinder tests need an empirical index of 0.4–0.5,
  not the point-source value, because finite bodies are not homogeneous (p. 514–515).
- **Why it still does not fit Cavendish.** (1) **Inputs.** Every Euler form needs, at each station, the
  field *and* its spatial derivatives (conventional) or the gravity vector and six tensor components
  (tensor). A tower gives one differenced vertical component, integrated over a 1.46 s flight. Applying
  Euler to that single component alone would need its gradients in x, y and z; x and y could be
  differenced between towers 4 m apart, z cannot. (2) **Window.** Reid's smallest window is nine
  stations and real data want 36–100; the array has five, on a cross, with a baseline that is not small
  against source distances of 4–60 m, and Reid asks that spacing "should not exceed the depth to
  sources of interest" (p. 87). (3) **Static field.** The relation holds for a fixed source; Cavendish's
  sources move. (4) **Sources.** The index must be guessed per feature; for a lone moving body the
  forward model already supplies the exact one. The first-pass phrase "needs dense gridded data" was
  wrong: Zhang runs it on profiles and ungridded data, and Reid's conclusion says the method "should be
  directly applicable to gravity gradiometer measurements" (p. 88), for surveys that measure those
  components on a grid. **Verdict: still not recommended**; this is reasoning from the papers' stated
  inputs, not a test. If a future array measured the full tensor and field at many stations, it is the
  cheap seeding step to revisit.

### 2.4 Matched filters and template banks (gravitational-wave practice)

- For a known waveform family in Gaussian noise, the matched filter is the likelihood: maximise the
  noise-weighted correlation over a bank of templates, with amplitude and phase maximised
  analytically ([Allen et al., FINDCHIRP](https://arxiv.org/abs/gr-qc/0509116)).
- **Template spacing** follows a metric derived from the match, "in the same way the information
  matrix … is derived from the ambiguity function" ([Owen & Sathyaprakash 1998](https://arxiv.org/abs/gr-qc/9808076),
  eq. 2.8); a minimal match of 0.97 is conventional because "the fraction of event rate retained is
  approximately FF³" for sources uniform in space with amplitude ∝ 1/distance (§I).
  *Consequence for Cavendish, derived here:* gradiometer amplitude falls as r⁻³, so range ∝ SNR^(1/3)
  and detection volume ∝ SNR; a 0.97 match costs only ~3 % of events, and the bank can be coarser
  than a GW bank.
- **Glitches**: FINDCHIRP adds a χ² discriminant over frequency bands to reject non-Gaussian
  transients (§IX). The Cavendish analogue is a per-tower χ² of the best-fit residual: a lift or a
  slammed door fits one tower and not the others.
- **For Cavendish**: mass is linear, so it is profiled out exactly (M̂ = ⟨u, d⟩/⟨u, u⟩ for the
  unit-mass template u); the bank spans position, velocity and reference time; Gauss–Newton with the
  finite-difference Jacobian refines the best template. This is M17-R6 (a). *Measured here*: a 2,925
  template bank for one straight-line walker builds in 0.7 s in NumPy and never missed the global
  optimum at SNR ≥ 40.

### 2.5 Bayesian inference of trajectories

With the likelihood known (Gaussian noise around the forward model), the posterior over a track is
`p(θ|d) ∝ exp(−½‖d − φ(θ)‖²_Σ) p(θ)`; for one body with 5 parameters it can be sampled directly (MCMC
or a dense grid around the matched-filter peak). This is what the GW community calls "standard
inference" and what DINGO is checked against ([Dax et al. 2021](https://arxiv.org/abs/2106.12594),
Fig. 2 and text). It is too slow for every window of a year-long series, which is the case for
amortisation (§4), but it is the **reference posterior** on the tractable benchmark scenes. A
sequential version (a particle filter over bodies) is the standard tool for tracking with
gradiometers on moving platforms ([Phillips et al. 2022](https://arxiv.org/abs/2204.05246)); for
unknown numbers of bodies, nothing in the gradiometry literature I found does better than
detect-then-fit, which is why the count belongs to the learned model.

### 2.6 A field atom gradiometer: Stray et al. 2022 **[read, 2nd pass, incl. Extended Data Tables 1–2]**

[Stray et al., *Nature* 602, 590 (2022)](https://doi.org/10.1038/s41586-021-04315-3): a University of
Birmingham hourglass cold-atom gradiometer, ⁸⁷Rb, vertical baseline 1 m, one component (G_zz), used
outdoors to find a known multi-utility tunnel.

| Quantity | Value | Where |
|---|---|---|
| Interrogation | π/2–π–π/2, T = 85 ms; ~0.7 Hz; 2.7×10⁵ and 1.7×10⁵ atoms in the two interferometers | Methods |
| Lab check | moving a test mass: measured ΔG_zz (205.0 ± 13.1) E against modelled 202 E | p. 591, Fig. 1c |
| Short-term sensitivity | (466 ± 8) E/√Hz; statistical uncertainty 20 E within 10 min (Allan deviation, 8.6 h outdoors) | p. 591, Fig. 2 |
| Noise budget | Johnson 144, background light 344, atom shot 184 E/√Hz; the quadrature sum, 416 E/√Hz, is close to the measured 466; quantum projection noise by atom number alone ≈ 44 E/√Hz | Ext. Data Table 1; Methods |
| Systematics | Coriolis < 26 E (5.9 E/degree of yaw), tilt < 1 E, baseline change < 0.1 E, magnetic < 43 E | Ext. Data Table 1 |
| Phase scale | 150 E ↔ 17.5 mrad; 100 E ↔ 11.6 mrad (≈ 0.116 mrad per E) | p. 592; Methods |
| Data yield | 98.4 % of ellipse fits usable (fits with ε > 0.05 discarded) | Methods |
| Survey | 8.5 m line, 0.5 m spacing, 17 stations; 600 runs per station (twelve 25-point ellipses per direction), typically 3 visits; base-station line subtracted for drift | Methods; Ext. Data Fig. 3 |
| Target | tunnel of 2 m internal cross-section, 0.2 m concrete wall, under a road between two multistorey buildings; modelled peak 150 E | p. 592 |
| Result | detection SNR 8 (150 E / mean 17.9 E error); centre (0.19 ± 0.19) m along the line; centre depth (1.89 −0.59/+2.3) m; top about 0.89 m deep; soil density anomaly −1.80 ± 0.15 g/cm³ | p. 592; Methods |
| Projection | 10-point line scan at SNR 3 in 15 min of measurement | p. 593 |

- **Cross-check, my arithmetic:** `φ = k_eff T² Γ L` with k_eff = 4π/780 nm = 1.61×10⁷ m⁻¹ gives
  0.116 mrad per E at T = 85 ms, L = 1 m, matching the paper's 11.6 mrad per 100 E. Their ~466 E/√Hz
  is therefore about 54 mrad/√Hz, some 500 times the plan's baseline of 10⁻⁴ rad/√Hz
  (`plan/VALIDATION-AND-RESEARCH.md`): the prototype is a different instrument from AION-10, so these
  are field numbers for realism, not Cavendish's.
- **How they inferred the tunnel**: not an unconstrained inversion. A **parametric Bayesian model** of a
  buried cuboid with the analytic polyhedron gravity formulas of Okabe (1979), likelihood a normal
  around the model, **informative priors** (Ext. Data Table 2: x centroid N(0, 4 m); depth and sizes
  Gamma(2, 0.5); density contrast N(−1.8, 0.1) g/cm³; site gradient and offset N(0, 10) with units E/m
  and E; a log-normal multiplier on the uncertainty for model misfit), sampled with the No-U-Turn
  sampler in PyMC3, and summarised as a probability-of-excavation map. The posterior in depth and size
  is non-Gaussian and skewed, "due to the known depth ambiguity for gravity sensing" (Ext. Data Fig. 4),
  and "ambiguity exists between model parameters specifying depth, area and density". Total
  per-point uncertainty is the Pythagorean sum of the standard error and a model-error term.
- **Back-of-envelope, not a validation** (script in the session scratchpad, not kept): an infinite
  2 m × 2 m void of density contrast −1.8 g/cm³ gives |G_zz| at the point above it of about 245 E,
  162 E and 113 E for a centre 1.89, 2.39 and 2.89 m below the observation point. The paper's
  modelled 150 E (which also includes buildings and drain, and a 1 m-baseline measurement) is inside
  that range; it cannot tell the geometries apart.
- **Use for Cavendish.** (1) A **field-anchored validation scene**: build the site (cuboid void,
  soil density, tower at the reported heights) and require the predicted profile to match the paper's
  modelled peak within a stated band (§7.3). (2) The **inference pattern** of §2.5 and §7.2 (parametric
  model, informative priors, NUTS or a grid for a few parameters, posterior summarised spatially) is
  what a field group did and published; it is the reference posterior, not a substitute for it.
  (3) Because the posterior in depth/size/density is skewed even at SNR 8, report a coverage and a
  posterior width, not a single estimate. (4) σ per tower must allow a **detection floor** above atom
  shot noise (detection supplied 373 of 416 E/√Hz in quadrature here). (5) Drift removal by a base
  station is part of their practice; Cavendish's equivalent is a witness or a rolling baseline.
  What the paper does not give: a comparison of inversion methods, any moving source, any multi-tower
  array, or a noise spectrum beyond the Allan deviation.

## 3. Fisher information and the Cramér–Rao bound (question 2)

### 3.1 Definition and what it does and does not mean

For data `d = φ(θ) + n` with Gaussian noise of covariance Σ, `F_ij = ∂_iφᵀ Σ⁻¹ ∂_jφ`.
[Vallisneri 2008](https://arxiv.org/abs/gr-qc/0703086) (introduction, items 1–3) separates three
readings of F⁻¹: (1) a lower bound on the covariance of **any unbiased** estimator (the CRB);
(2) the covariance of the maximum-likelihood estimator in the high-SNR limit, equivalently the
linearised-signal approximation (LSA); (3) the posterior covariance at high SNR with flat priors.
He concludes that the CRB is "seldom useful … as a proper bound" and that "where it is useful, it
reduces to the high-SNR/LSA result for the ML estimator" (§II C). Two consequences for S11:

- **Biased estimators can beat it** (§II C, discussion of eq. 12). A neural network trained with a
  prior is biased towards the prior at low SNR, so "model error below the CRB" is not a bug there.
- **Consistency check** (§VI, eq. 47): on the Fisher 1σ surface, compute
  `|log r| = ½‖Δφ − JΔθ‖²_Σ` (the non-linear part of the signal change); if it exceeds ~0.1, the
  Fisher result is not self-consistent. One sinusoid needs SNR > 1.09 by this test (Fig. 1). It
  costs 2·(number of parameters) forward runs: cheap enough to run on every benchmark scene.
- **Kay's conditions: not verified from the source.** The Kay PDF supplied for this pass
  (*Fundamentals of Statistical Signal Processing: Estimation Theory*, 1993) is five pages: cover,
  title page and the table of contents. So no theorem can be cited from it. What the contents do
  establish is where to look: Ch. 3 "Cramer-Rao Lower Bound" (§3.4 CRLB p. 30; §3.5 signals in white
  Gaussian noise p. 35; §3.7 vector parameter p. 39; **§3.9 "CRLB for the General Gaussian Case"
  p. 47**, the case here: mean `φ(θ)`, covariance Σ), and Ch. 7 (§7.5 "Properties of the MLE" p. 164;
  §7.8 vector MLE p. 182; §7.9 asymptotic MLE p. 190). From memory only, and to be checked against
  those pages: the bound needs the regularity condition `E[∂ln p(x;θ)/∂θ] = 0` for all θ; an
  unbiased estimator attains it iff `∂ln p/∂θ = I(θ)(g(x) − θ)` for some functions g and I, and then
  it is the MVU estimator; for a Gaussian with θ-dependent mean and covariance the information has the
  extra trace term `½ tr[Σ⁻¹ ∂Σ/∂θ Σ⁻¹ ∂Σ/∂θ]`, which vanishes when Σ does not depend on θ (as
  assumed here); the MLE is asymptotically efficient, *as the number of independent data grows*, again
  under regularity. The practical reading is Vallisneri's (above), which was read from source: for
  `d = φ(θ) + n` with θ-independent Σ the estimator is efficient only in the linear-signal limit,
  which the |log r| test checks. Nothing in the S11 plan depends on the exact theorem number.
- **Singular Fisher matrices** signal a parameter combination the data do not constrain (§IV):
  "a singular Fisher matrix is almost always a symptom" of a degeneracy, handled by priors or by
  reparametrising. *Measured here*: a **single tower** gives an exactly singular Fisher matrix
  (normalised condition > 10¹⁵) whose null vector is a rotation of the track about the tower axis,
  as axial symmetry demands; five towers break it (condition ~10³).

### 3.2 How to compute it from Cavendish's forward model

The forward model is not written to be differentiable, and need not be:

| Derivative | Method | Error (*measured here*, toy model §8) |
|---|---|---|
| ∂φ/∂M | exact: φ/M (linearity) — also the control for the finite differences | — |
| ∂φ/∂M by central difference | step 10⁻⁴·M | 3×10⁻¹² relative |
| ∂φ/∂x by central difference, **f64** phases | step 10⁻³ m / 10⁻⁴ m | 4×10⁻⁸ / 4×10⁻¹⁰ relative |
| the same, phases rounded to **f32** | best step 10⁻² m; 10⁻⁴ m | 7.8×10⁻⁶ / 2.1×10⁻³ relative |

So: **central differences on the f64 CPU path** (the oracle twin every GPU kernel already has,
`plan/REBUILD.md` §2.6), 2P + 1 forward runs per scene for P parameters. Automatic differentiation
would be more elegant and is not needed. Complex-step differentiation would avoid the step choice
but requires complex arithmetic through the forward model; not worth it at these errors.

**Parameters** for one body on a straight track: (M, x₀, y₀, vₓ, v_y) at a reference time inside the
window, height known (people and vehicles) or a sixth parameter (lifts, animals underground). For
oscillators: (M, f₀, amplitude, orbit tilt, phase). For a population the "parameters" are per body
and the Fisher matrix is block-structured; overlapping bodies couple through the off-diagonal blocks.
The noise covariance Σ is diagonal for shot noise; with seismic Newtonian noise it is not (it is
correlated across towers, [Harms 2015](https://arxiv.org/abs/1507.05850) eq. 234), and the Fisher
matrix must use the full Σ — this is where the CRB stops being trivially cheap and the Wiener
picture (§5) and the bound meet.

### 3.3 How it sets the bar

| Regime (by \|log r\| and SNR) | What the CRB means | What a model must show |
|---|---|---|
| \|log r\| ≤ 0.1 (high SNR) | ML attains it; it is the answer | baseline RMSE/√CRB ≈ 1 (a *test*); model within a stated factor of it |
| \|log r\| > 0.1 (low SNR) | a loose, possibly wrong bound for unbiased estimators | posterior calibrated (SBC, coverage) and RMSE vs the exact posterior's |
| singular / ill-conditioned F | a degeneracy | report the constrained combinations only |

## 4. Simulation-based inference (question 3)

- **What it is.** Cranmer, Brehmer & Louppe ([arXiv:1911.01429](https://arxiv.org/abs/1911.01429);
  [PNAS](https://doi.org/10.1073/pnas.1912789117)) review inference when a simulator can be run but
  its likelihood is intractable. Neural surrogates of the likelihood, posterior or likelihood ratio
  "are all amortized: after an upfront simulation and training phase, the surrogates can be evaluated
  efficiently for arbitrary data and parameter points", and "the network then implicitly marginalizes
  over all other (latent) variables in the simulator" (§3). That second sentence is the reason SBI
  suits Cavendish: ambient walkers, lifts, seismic waves and instrument nuisances are latent variables
  Cavendish can sample but no one can integrate out by hand.
- **Use what the simulator offers.** "If any of the quantities discussed in Sec. 2 are available, they
  should be leveraged" — differentiable simulators, the joint score and joint likelihood ratio
  (the "mining gold" approach of Brehmer et al.) (§3 D).
- **What "mining gold" is, from the source** **[read, 2nd pass]**
  ([Brehmer et al. 2020, PNAS](https://doi.org/10.1073/pnas.1915980117)). If the simulator exposes its
  latent variables z, it can return, per run, the **joint score** `t(x,z|θ₀) = ∇θ log p(x,z|θ)` (eq. 3)
  and **joint likelihood ratio** `r(x,z|θ₀,θ₁)` (eq. 4). Regressing on them recovers the intractable
  marginals, because the minimiser of the squared error is the conditional expectation (eq. 6–10:
  `r*(x) = E[r(x,z)|x,θ₁] = r(x|θ₀,θ₁)`). The losses train a **likelihood ratio** (ROLR eq. 11, RASCAL
  eq. 12, CASCAL eq. 13) or a **likelihood** (SCANDAL eq. 14), or give a locally sufficient statistic:
  near θ_ref the model is exponential-family with the score as sufficient statistic (eq. 15; SALLY,
  SALLINO). Reported effect: RASCAL needed two orders of magnitude fewer samples than CARL on a Higgs
  simulator (Fig. 4); SCANDAL and RASCAL were "dramatically more sample efficient" on a toy (Fig. 2).
  **Limits stated by the authors:** the augmented data are "less powerful for deciding between model
  parameter points that are far apart" because the joint distributions barely overlap; the local
  methods "plateau" away from θ_ref; and in the Lotka–Volterra test they trade density-estimation
  quality for ratio quality (p. 5–6). **None of them trains a posterior.**
- **For Cavendish.** (a) This does *not* help the first model (amortised NPE over a wide prior), so
  the first-pass remark that it was a route to better NPE sample efficiency is withdrawn. (b) It would
  matter for a likelihood or ratio surrogate in a narrow parameter region, e.g. refining around a
  matched-filter peak. (c) For additive Gaussian noise at fixed latents the joint score is
  `∂φᵀΣ⁻¹(x − φ)` (derived here from eq. 3, not taken from the paper), the matched-filter output
  weighted by the Jacobian: the baseline's Gauss–Newton step is already SALLY's statistic, so the
  baseline needs no learning to have it. (d) It needs the Jacobian at fixed latents, which §3.2's
  central differences supply. Not needed first; revisit only if a likelihood surrogate is built.
- **Cautions.** None of the diagnostics "address the issues encountered if the model is misspecified"
  (§3 C) — the sim-to-real gap (§5.3 below) is outside what SBI can check.
- **Which method.** The benchmark of [Lueckmann et al. 2021](https://arxiv.org/abs/2101.04653) (REJ-ABC,
  SMC-ABC, NLE, NPE, NRE and sequential variants; classifier two-sample test C2ST against reference
  posteriors; 10³–10⁵ simulations) finds no uniformly best algorithm, that sequential methods are more
  sample-efficient, and advises: to infer "separately for different data points … methods that allow
  'amortization' (NPE) are likely preferable", and NPE "can directly reduce high-dimensional data as
  part of their network architectures" (Box 1). Gradar infers for every window of every run, so:
  **amortised NPE**. NPE itself is from [Papamakarios & Murray 2016](https://arxiv.org/abs/1605.06376);
  flows are reviewed in [Papamakarios et al. 2021](https://arxiv.org/abs/1912.02762).
- **The closest precedent: DINGO** ([Dax et al. 2021](https://arxiv.org/abs/2106.12594)). NPE for
  LIGO–Virgo with a normalising flow and an embedding network whose first layer is seeded with the
  principal components of clean waveforms; the network is conditioned on the noise PSD so one network
  covers non-stationary noise; extrinsic parameters and **noise realisations are drawn during
  training** from 5×10⁶ stored waveforms; "no indication of overfitting"; about 10 days on one A100;
  P–P plot over 1000 injections with combined p-value 0.46 (Fig. 2); 20 s per event against O(day).
  Every design choice transfers: store clean signals, add noise at batch time, seed the embedding
  with principal components of the unit-mass templates, condition on the noise level and the mask.
- **Validation.** (a) **SBC**: draw θ from the prior, simulate, draw L posterior samples; the rank of
  the true θ among them is uniform on {0…L} for a correct posterior (Theorem 1, Algorithm 1 of
  [Talts et al. 2018](https://arxiv.org/abs/1804.06788)). (b) **Expected coverage** of the 1−α highest
  posterior density regions (Definition 2.1 of [Hermans et al. 2022](https://arxiv.org/abs/2110.06581)),
  where all benchmarked SBI methods, NPE included, "can yield overconfident posteriors" and ensembles
  are more reliable. Their caveat is the control arm S11 needs: an estimator that returns the prior
  has perfect coverage and no information (§2), so coverage must be paired with an information or
  error measure. (c) On the tractable scenes, **compare with the exact posterior** (C2ST, Lueckmann).

## 5. Architectures, sim-to-real, calibration (question 4)

### 5.1 Shape of the input

`phase [T, D]`, `readout_time [T, D]`, `mask [T, D]` per window (`plan/ML-AND-API.md` §3), with D
towers that may fire staggered and lose shots. Three properties should be built in, not learned:

| Property | Architecture | Source |
|---|---|---|
| tower order does not matter; towers can be added | sum-decomposition `ρ(Σφ(x))` over towers (Deep Sets, Theorem 2), or attention over towers with inducing points, O(nm) not O(n²) | [Zaheer et al. 2017](https://arxiv.org/abs/1703.06114); [Lee et al. 2019](https://arxiv.org/abs/1810.00825) §3.1 |
| shots are irregular and unaligned across towers | a learned embedding of continuous time and attention to a fixed set of reference times (mTAN); or each observation as a set element (time, tower, value) with no imputation (SeFT) | [Shukla & Marlin 2021](https://arxiv.org/abs/2101.10318); [Horn et al. 2020](https://arxiv.org/abs/1909.12064) |
| tower geometry matters | feed each tower's position as a feature of its set element (a graph over towers is the same idea with edges) | follows from the two above |

SeFT reports that decoupled impute-then-classify schemes do worse than end-to-end ones and that
imputation discards "missingness patterns" ([Horn et al.](https://arxiv.org/abs/1909.12064) §1): feed
the mask, do not fill gaps. Neural controlled differential equations
([Kidger et al. 2020](https://arxiv.org/abs/2005.08926)) handle irregular series too but are heavier;
not first. **Physics-informed networks** in the PINN sense are not the right fit: the physics is
already exact in the simulator; the physics belongs in the *input representation* (whitening by the
noise level, unit-mass templates as the embedding's first layer, as DINGO does).

**First model:** a SeFT-style set encoder over (t, tower position, phase, mask) elements → a
flow (neural spline or MAF) over θ. It handles stagger, gaps and any number of towers with one
mechanism, and it is small.

### 5.2 Detection head and the count

Unknown body count is a set-prediction problem. Before attempting it: a detection head
(present/absent per window) judged by ROC against the matched filter, which is the GW community's
first test of a learned detector — a CNN on whitened strain "can reproduce the sensitivity of a
matched-filtering search" ([Gabbard et al. 2018](https://arxiv.org/abs/1712.06041), abstract and
Fig. 2). The benchmark null set (`plan/ML-AND-API.md` §4) fixes the false-alarm axis.

### 5.3 Sim-to-real and calibration

- **Domain randomisation**: "with enough variability in the simulator, the real world may appear to
  the model as just another variation" ([Tobin et al. 2017](https://arxiv.org/abs/1703.06907),
  abstract). The plan's list (instrument parameters, tower placement and tilt, noise spectra, loss,
  jitter, air model; `plan/ML-AND-API.md` §5) is that recipe. Each randomised quantity is a nuisance
  parameter that the NPE network marginalises (§4).
- **Calibration**: modern networks are overconfident; temperature scaling is "surprisingly
  effective" for classifiers ([Guo et al. 2017](https://arxiv.org/abs/1706.04599)); deep ensembles
  trained with a proper scoring rule give calibrated regression uncertainty and higher uncertainty
  out of distribution ([Lakshminarayanan et al. 2017](https://arxiv.org/abs/1612.01474)). For NPE the
  flow's density is the uncertainty, checked by SBC and coverage; ensembles of five flows are the cheap
  remedy for overconfidence (Hermans et al.).

## 6. Detection in noise (question 5)

- **Wiener filtering with witnesses.** LIGO's planned Newtonian-noise cancellation estimates the
  gravity noise from a seismometer array and subtracts it; the residual is
  `R = 1 − C_STᵀ C_SS⁻¹ C_ST / C_TT` (eq. 2 of [Coughlin et al. 2016](https://arxiv.org/abs/1606.01716)),
  bounded below by `1/(N·SNR²)` for N seismometers. The Wiener filter is the optimal linear filter if
  the fields are wide-sense stationary and the noise additive ([Harms 2015](https://arxiv.org/abs/1507.05850)
  §7.1, §7.1.1); broadband mitigation factors near 10 were simulated ([Driggers, Harms & Adhikari 2012](https://arxiv.org/abs/1207.0275), abstract).
- **Array averaging for atom interferometers.** An array of atom gradiometers "yields several
  estimations of the NN, whose effect can thus be reduced via statistical averaging": a factor 2 with
  existing geometries and 10 or more with a dedicated layout exploiting the noise correlation length
  ([Chaibi et al. 2016](https://arxiv.org/abs/1601.00417), abstract).
- **What Hughes & Thorne 1998 give** **[read, 2nd pass: abstract, §I A–D, §II start, §V]**
  ([Hughes & Thorne 1998](https://doi.org/10.1103/PhysRevD.58.122002)). They model ambient seismic
  waves as Love and Rayleigh modes in layered ground and compute the Newtonian force on a test mass.
  (1) It is a *force* noise, not a gradient: footnote 1, "it is not gradients of the local
  gravitational field which cause this noise, but simply the direct fluctuation of the gravitational
  force"; an atom gradiometer's response needs the same density field differenced over its baseline,
  which this paper does not do. (2) **Love modes contribute nothing** (eq. 1.11, "no compression and no
  density variations"); Rayleigh modes do. (3) Transfer function `x̃/W̃ = 4πGρ β(f)/(2πf)²` (eq. 1.8),
  ρ ≈ 1.8 g/cm³, β = γ·G·β′; Saulson's crude β = 1/√3 = 0.58 (eq. 1.9) was "remarkably good". β′
  = 0.35–0.6 at quiet times and 0.15–1.4 at noisy times (eq. 1.28, 1.30; Table I: fundamental mode
  0.4–0.9, overtones 0–1.4, P-like overtones 0–0.15 because surface and subsurface sources cancel).
  Arithmetic here from eq. 1.6, 1.8, 1.27: the acceleration spectrum is `4πGρ β W̃`, about
  7×10⁻¹⁶ m s⁻²/√Hz at 10 Hz for β = 0.5 and the "standard LIGO seismic spectrum"
  (10⁻⁹ m/√Hz) at the LIGO site; not a gradient, and not Cavendish's site. (4) The gravity comes from
  shallow ground: the e-folding depth is `c_H/2πf ≈ 16 m (c_H/1000 m s⁻¹)(10 Hz/f)` (eq. 1.25), and
  for the fundamental mode in a half space c_H = 0.93 c_S = 205 m/s, reduced wavelength 3.3 m at 10 Hz
  (eq. 2.2, 2.3). (5) **Correlation across a few metres:** the factor γ that accounts for the correlation
  between the two LIGO corner masses 5 m apart is "within about 10 per cent of unity for all
  frequencies" (Fig. 1, eq. 1.19; the abstract calls it "a weak correlation" effect). I could not
  turn that into a statement about tower-to-tower correlation without the appendix. At
  Cavendish's shot rate (one per 2 s, so signal band below 0.25 Hz) the reduced wavelength is of order
  100 m against a 4 m tower spacing, so the ambient field should be strongly correlated across the
  towers; this is my extrapolation of eq. 2.3, not a result of the paper. (6) Mitigation they
  propose is the one §6 already has: monitor the density field and subtract it (3-D arrays of
  surface seismometers plus borehole dilatometers), or moats against the fundamental mode (§V D);
  they also note jerky human activity can exceed seismic noise unless kept away (§I, citing Spero).
  (7) The authors say their mode mixture is a rough estimate "plagued by a large number of
  uncertainties" and ask for measured sound speeds and Poisson ratios versus depth (§V C): the numbers
  above are LIGO-site numbers.
- **The trap.** "A device able to subtract gravity noise can also cancel GW signals"
  ([Harms](https://arxiv.org/abs/1507.05850) §7.1.7). For Gradar: if other towers are used as witnesses
  of the ambient field, they also see the target; a denoiser can remove the target. Every denoising
  benchmark needs an injected-target recovery check.
- **Finite moving sources** are, in Harms's words, "mostly a practical problem, and trivial from the
  theory perspective" (§7.1): if the source is modelled, subtract it. That is the forward-model
  residual Gradar's tracking head produces.
- **Transients.** The AION collaboration's own cleaning is a rolling standard deviation over N = 101
  shots with a threshold of "mean-plus-five-standard-deviations of an ASN-limited experiment", masking
  what crosses it ([Carlton & McCabe 2023](https://arxiv.org/abs/2308.10731), eqs. 14–15); it restores
  ULDM sensitivity "to within between 10% and 40%" of shot-noise-limited (abstract). This is already the
  plan's transient baseline (`plan/ML-AND-API.md` §4).
- **Mapping to Cavendish.** Shot noise is white and independent per tower; seismic Newtonian noise and
  distant populations are correlated across towers; nearby bodies are deterministic signals. The
  baseline stack is therefore: Wiener / array combination for the correlated stationary part (using
  the ambient channel's statistics from simulation), forward-model fit for the deterministic part,
  rolling-σ mask for the rest. The learned denoiser must beat each on the channel it targets.

## 7. Datasets, and what S11 should build (questions 6 and 7)

### 7.1 Dataset design

- **Labels.** Per window: body count; per body (M, x, y, z, vₓ, v_y at the window's reference time,
  plus path family); for oscillators f₀ and orbit; for denoising the `targets` channel. Position
  labels at a fixed reference time, not per shot, so a track is a few numbers and the CRB is defined
  on the same parameters. Roles as in `plan/ML-AND-API.md` §2: the Fisher matrix and the SNR are
  `internal` (for evaluation), never inputs.
- **Storage that amortises the simulator.** Because phase is linear in mass and noise is additive,
  store the **noise-free, unit-mass** signal per body and draw mass scale, noise, loss and jitter
  masks at batch time — DINGO's practice of drawing extrinsics and noise during training. One
  simulated window then serves many SNRs. (Jitter changes readout times and so the signal; it must be
  simulated, not drawn later.)
- **Splits** by `salt:scene_seed` with whole families together (`plan/ML-AND-API.md` §3) are what the
  leakage literature asks for: [Kapoor & Narayanan 2023](https://arxiv.org/abs/2207.07048) list
  pre-processing on train and test together (L1.2), duplicates (L1.4), temporal leakage (L3.1) and
  non-independence between train and test (L3.2) among eight leakage types. For Cavendish, windows cut
  from one long run are not independent: split by run, never by window. Normalisation statistics
  (whitening) from the training split only.
- **Provenance.** Each window's `meta` carries the tier and truncation from the bundle (as the plan
  says) **and** its Fisher matrix and \|log r\| for the targets, so evaluation can stratify by
  "CRB meaningful" without recomputation.
- **Size.** One window of the default array over 60 s is 5 × 31 float32 = 620 B; 10⁶ windows = 0.6 GB
  plus labels. DINGO trained on 5×10⁶ waveforms; Lueckmann's budgets were 10³–10⁵ for low-dimensional
  tasks. Simulation cost: the read in parallel made a 10,000-shot series in 0.6 s (`plan/IDEAS.md`,
  periodic-source table), about 0.06 ms a shot, so 10⁶ point-mass windows are of order half an hour of
  CPU; voxel bodies and air cost more (not measured).

### 7.2 Build order for S11 (TASK-S11-05 and the first model)

1. **`cav.likelihood`** (Python over the engine, f64 CPU): `phase(θ)`, `jacobian(θ, steps)`,
   `fisher(θ, σ)`, `crb(θ, σ)`, `log_r_max(θ, σ)`.
2. **Baseline (a)**: matched-filter bank over straight tracks with mass profiled, Gauss–Newton refine,
   damped and bounded to the prior box, per-tower χ² residual.
3. **Baseline (b)**: positivity-constrained Tikhonov grid inversion per window (Li & Oldenburg form,
   distance weighting, discrepancy principle).
4. **The CRB table** (`baseline_vs_bound_vs_model`): per scene family and SNR bin, CRB, baseline
   RMSE and bias, the ratio, \|log r\|, coverage of the Fisher ellipsoid.
5. **First model**: amortised NPE for one body, straight track, the default array with loss and
   jitter, noise drawn at batch time; SeFT-style encoder + neural spline flow; an ensemble of five.
6. **Only then** count and multi-body, denoising, fingerprinting.

### 7.3 Tests, each with its control arm

| Test | Asserts | Control arm (must fail) |
|---|---|---|
| `fisher_mass_derivative_is_exact` | FD ∂φ/∂M equals φ/M to 10⁻⁹ relative | a step of 10⁻⁸·M on f32 phases exceeds the tolerance |
| `fisher_fd_converges` | FD ∂φ/∂x at steps h and h/2 agree to 4× the h² rate on f64 | the same on f32 phases does not |
| `single_tower_fisher_is_singular_along_rotation` | null vector of the one-tower Fisher matrix is the rotation about the tower | five towers: condition < 10⁵ |
| `ml_baseline_attains_crb` | on scenes with \|log r\| ≤ 0.1, 500 noise draws, RMSE/√CRB ∈ [0.8, 1.25] per parameter and Fisher-ellipsoid coverage within 0.05 of 0.68 | CRB computed with σ/2 gives ratio ≈ 2 and fails (*measured*: 1.92–2.13) |
| `bank_finds_global_optimum` | Gauss–Newton from the bank's best is never worse than from the truth at SNR ≥ 40 | a bank with 4× the spacing misses |
| `inversion_conserves_mass` (M17) | noiseless total mass within 1 % | without positivity, it does not |
| `npe_sbc_uniform` | SBC rank histograms uniform (χ² p > 0.01) for every parameter | a network whose posterior width is halved fails |
| `npe_coverage_and_information` | coverage ≥ nominal − 0.02 at 50/90 % **and** RMSE ≤ 1.5× the exact-posterior RMSE on tractable scenes | a model that returns the prior passes coverage and fails the RMSE arm |
| `npe_matches_exact_posterior` | C2ST accuracy ≤ 0.6 against MCMC on 20 tractable scenes | a deliberately shifted posterior gives > 0.6 |
| `null_set_has_no_detections` | false-alarm rate on null scenes at the chosen threshold | a model trained without null scenes exceeds it |
| `stray_tunnel_profile_matches_paper` (forward model, **not** inference) | a 2 m × 2 m void of −1.8 g/cm³ with its centre ~2.4–2.9 m below a 1 m-baseline gradiometer, as an infinite prism, gives a peak \|G_zz\| in 100–250 E, bracketing the paper's modelled 150 E (§2.6); with the buildings of the site added the band should tighten, and the band is to be set from the paper's Fig. 3b geometry before running | the same void 6 m deep falls outside the band |
| `fisher_with_detection_floor` | CRB scales with the per-tower σ; adding a 373 E/√Hz-equivalent detection term to a 184 E/√Hz shot term raises σ by 2.3× and the CRB by the same | σ taken as shot noise only fails the ratio |
| `denoiser_keeps_injected_target` | injected target recovered within the CRB after denoising | a denoiser that uses the target tower as witness of itself removes it |

## 8. Measured here: the toy CRB benchmark

`bench/crb_bench.py` (NumPy only; ≈ 250 lines). It is **not** Cavendish's forward model: it
re-implements the arm integral of the reference scripts (constants from
`docs/research/s1-periodograms-and-script-fixtures.md` §2; 64-node Gauss–Legendre over the flight;
the source moves during the flight; upper minus lower interferometer) for a point mass at height 1 m
walking past the default five-tower array, shots every 2 s, all towers in step, 31 shots each
(60 s window), white noise σ = 1 mrad per shot. Its sanity check against the S15 note's "10 kg at 10 m:
−8.5 mrad" gives 7.7–12.7 mrad depending on the source height (the S15 geometry and sign convention
were not checked; order of magnitude only). 64 vs 128 quadrature nodes differ by 4×10⁻⁵ rad at a
0.3 rad peak. Estimators: (a) bank (13 × 9 × 5 × 5 = 2,925 templates) + damped Gauss–Newton bounded to
M ∈ [0, 5000] kg, |x|,|y| ≤ 150 m, |v| ≤ 5 m/s; (b) Tikhonov + positivity on a 1 m grid, 10 s window,
centroid.

300 noise draws per row (60 for the grid inversion); walker at 1 m/s along x, closest approach y₀
from the centre tower, x₀ = 0 at t = 0. Run time 225 s for everything on the build PC (two BLAS
threads).

| y₀ (m) | M (kg) | optimal SNR | max \|log r\| (1σ) | CRB σ: M, x₀, y₀ | baseline (a) RMSE/√CRB: M, x₀, y₀, vₓ, v_y | Fisher-ellipsoid 68 % coverage | bank misses | grid inversion RMSE: M, x, y |
|---|---|---|---|---|---|---|---|---|
| 8 | 75 | 814 | 0.004 | 0.89 kg, 1.9 cm, 5.5 cm | 1.07, 0.97, 1.06, 1.03, 0.96 | 0.65 | 0/300 | 46 kg, 2.8 cm, 3.4 m |
| 15 | 75 | 226 | 0.012 | 3.2 kg, 17 cm, 22 cm | 1.01, 1.03, 1.01, 0.96, 1.03 | 0.68 | 0/300 | 66 kg, 1.2 cm, 10.5 m |
| 30 | 75 | 40 | 2.5 | 26 kg, 3.4 m, 3.5 m | 1.16, 0.97, 1.00, 1.00, 0.97 | 0.45 | 0/300 | 74 kg, 5.6 cm, 25.6 m |
| 60 | 750 | 62 | 12 | 302 kg, 8.1 m, 8.1 m | 1.23, 1.07, 1.07, 1.04, 1.06 | 0.24 | 0/300 | 748 kg, 13 cm, 55.7 m |
| 45 | 75 | 14 | 1.3×10³ | 106 kg, 21 m, 21 m | **6.9**, 1.42, 1.25, 1.41, 1.49 | 0.12 | 6/300 | 75 kg, 0.97 m, 40.6 m |
| 60 | 75 | 6.2 | 1.1×10⁵ | 302 kg, 81 m, 81 m | 3.3, **0.63, 0.71**, 0.89, 0.98 | 0.02 | 48/300 | 75 kg, 1.5 m, 56.7 m |

Other measurements from the same run:

- **Control arm.** RMSE divided by a CRB computed with σ/2 (a deliberately wrong Fisher matrix) at
  y₀ = 8 m: 2.13, 1.94, 2.12, 2.07, 1.92 — outside [0.8, 1.25] for every parameter, as it must be.
- **Finite differences**: the table in §3.2 (f32 at 10⁻³ m step: 1.6×10⁻⁴; at 10⁻⁴ m: 2.1×10⁻³).
- **One tower**: normalised condition number > 5×10¹⁵ at y₀ = 8 and 15 m; null vector
  (0, −0.71, 0, 0, +0.71) in normalised (M, x₀, y₀, vₓ, v_y): a rotation of the track about the tower.
- Bias of baseline (a) is below 0.1 CRB for every parameter at SNR ≥ 226.

**Reading.**

1. **Where \|log r\| ≤ 0.1 (SNR ≥ ~200 here), the bank + Gauss–Newton estimator attains the CRB**
   within Monte-Carlo error (ratios 0.96–1.07, standard error ~4 %), its errors fill the Fisher 1σ
   ellipsoid at the nominal 68 %, and the wrong-σ control fails by the expected factor of 2. This is
   the known-answer validation case S11 can adopt as is.
2. **Vallisneri's criterion is conservative here**: at SNR 40–62 (\|log r\| 2.5–12) the RMSE ratios
   are still 1.0–1.23, but the Fisher ellipsoid's coverage has already collapsed (0.45, 0.24): the error
   distribution is no longer Gaussian (a mass–range banana). RMSE alone would hide it; coverage shows it.
   Gate the strict test on \|log r\| ≤ 0.1, report the rest.
3. **At low SNR the bounded estimator "beats" the CRB on position (0.63–0.71 at SNR 6)** while missing
   the global optimum in 16 % of draws and erring on mass by 3×: the prior box is doing the work, as
   Vallisneri §II C predicts for biased estimators. A learned model will do the same, legitimately.
4. **The grid inversion recovers the along-track coordinate and nothing else**: range errors 47–62×
   the CRB at high SNR and mass errors comparable with the mass itself (part of the gap is that it sees
   10 s against the CRB's 60 s). Its along-track errors *below* the CRB at y₀ = 15 m are not information
   either: the truth sits on the array's symmetry axis at the centre of the grid, where a blurred
   symmetric solution's centroid lands regardless — a reminder that a number under the bound needs an
   explanation before it is believed (CLAUDE.md: "A suspicious number gets a measurement").

## 9. What I could not verify

- Cavendish's real forward model was not run; the benchmark's toy model agrees in order of magnitude
  only. The finite-difference step sizes and the f32 failure should be re-measured on the engine's
  f64 reader in the slice (one test, §7.3).
- The pass band [0.8, 1.25] for RMSE/√CRB is from Monte-Carlo error at 300–500 draws (standard error
  of an RMSE ≈ 1/√(2n) ≈ 4 %) plus margin; not from a source.
- Euler deconvolution's unsuitability is reasoned from the inputs the papers require (§2.3), not tested.
- Kay's CRB and efficiency conditions (§3.1) are from memory; the supplied file has no theorem text.
- Hughes & Thorne's numbers are for a LIGO test mass's force, not a gradiometer's gradient; nothing
  here converts them to Cavendish's response or site (§6).
- Stray et al.'s tunnel check (§2.6) is a one-line infinite-prism estimate; the paper's own site model
  (buildings, drain, finite length) was not reproduced.
- Whether the seismic Newtonian-noise field makes the likelihood intractable in practice depends on
  the site spectrum, which does not exist yet (M17 §3).
- No paper was found on tracking moving masses with arrays of atom gradiometers; RATs to riches
  treats them as noise. Gradar's tracking task appears to be new.

## 10. Further questions (not chased)

1. Template-bank metric for straight tracks (Owen's construction applied to φ(θ)): bank size per
   scene family and its 0.97-match spacing.
2. Whether the joint score from the simulator (Brehmer et al.) cuts NPE's simulation budget here.
3. Tower layout chosen to maximise det F for the stairwell populations — an experimental-design
   question for George.
4. A CRB with correlated seismic noise (full Σ) and how much array averaging buys (Chaibi) for
   tracking rather than ULDM.

## Source status after the second pass

Read from the supplied PDFs (copies are not in the repository):

| Paper | How much was read |
|---|---|
| Thompson 1982 | Euler's equation, structural index, implementation, acceptance test; the model and field examples only skimmed |
| Reid et al. 1990 | Abstract, theory, solution strategy, discussion, conclusion; the field example and appendix derivations not read |
| Zhang et al. 2000 | All text; the figures (solution maps) were not examined |
| Li 2001 | All of it (four pages) |
| Brehmer et al. 2020 | Main text through the conclusions; the supporting information is not in the PDF |
| Stray et al. 2022 | Main text, Methods, Extended Data Tables 1 and 2 (as images); Extended Data Figs 3–4 not examined beyond captions |
| Hughes & Thorne 1998 | Abstract, §I (A–D), start of §II, §V; the layered-medium derivations (§II–IV) and appendices not read |
| Kay 1993 | **Front matter and contents only (5 pages); no theorem text** |

Still to fetch: **Kay, Ch. 3 (pp. 27–47, with the proofs in Appendices 3A–3C, pp. 67–77) and Ch. 7
(pp. 164–173, 190)**, to replace the from-memory conditions in §3.1; and Li & Oldenburg 2000
(*Geophys. J. Int.*) only if the wavelet compression is ever wanted.
