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

---

## 1. One-page summary

**The problem is unusually kind, and S11 should exploit that before it trains anything.**
Cavendish's forward model is deterministic, its phase is **linear in mass** and smooth in position
and velocity (`reference/milestones/M17-site-and-baseline.md` §3), and the instrument noise the plan
models is additive (shot noise, 10⁻⁴ rad/√Hz baseline, `plan/VALIDATION-AND-RESEARCH.md` l. 20). For
a scene of a few point-like bodies in shot noise, the likelihood is therefore *known in closed
form*: a Gaussian around the forward model. That gives three things learning problems rarely have:
an exact Fisher matrix, a maximum-likelihood estimator that provably reaches the Cramér–Rao bound at
high SNR ([Vallisneri 2008](https://arxiv.org/abs/gr-qc/0703086), §II D), and an exact posterior
against which a learned posterior can be checked. Learning earns its place only where the
likelihood stops being tractable: unknown numbers of bodies, ambient populations, seismic Newtonian
noise, masks and gaps, instrument nuisances. That is the territory simulation-based inference was
built for ([Cranmer, Brehmer & Louppe 2020](https://arxiv.org/abs/1911.01429)).

| Question | Answer |
|---|---|
| 1. Inversion and localisation | **Matched-filter bank + Gauss–Newton (profiled maximum likelihood)** is the baseline; it is what GW searches do ([Owen & Sathyaprakash 1998](https://arxiv.org/abs/gr-qc/9808076); [Allen et al. FINDCHIRP](https://arxiv.org/abs/gr-qc/0509116)). Regularised grid inversion (Li & Oldenburg) is non-unique by construction and was measured here at 47–62× the CRB in range at high SNR; keep it as a second, labelled-weak baseline. Euler deconvolution does not fit five towers with a 5 m baseline. |
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
- **For Cavendish:** the analogue of depth is distance from the towers, and gradiometer kernels decay
  faster (r⁻³, so β nearer 3 would be the analogue; Li's 2001 gradiometer paper treats this but was
  not read, see the end). The array sees a moving body, so the grid must be per time window and the
  window short enough that the body barely moves — which leaves 5 towers × 5 shots = 25 numbers for
  hundreds of cells. *Measured here* (§8): a positivity-constrained Tikhonov inversion with a
  discrepancy-principle λ recovers the along-track position but the range and the mass badly
  (range RMSE 47–62× the CRB at high SNR, mass errors comparable with the mass). That is exactly the non-uniqueness Li & Oldenburg describe: mass and
  distance trade off. It is the plan's baseline (b) (M17-R6) and worth keeping as a *labelled-weak*
  yardstick, because it is the only baseline that needs no track model and so extends to extended
  and many bodies.

### 2.3 Euler deconvolution

Euler's homogeneity relation turns field values and their spatial gradients at many stations into
source positions with an assumed structural index (Reid et al. 1990, Thompson 1982; not read, see the
end; [Zhang et al. 2000](https://doi.org/10.1190/1.1444745) extends it to tensor gradients). It needs
dense gridded gradient data around the source. Cavendish has five stations, measures one differenced
component over a 5 m baseline that is not small against source distances of 4–60 m, and its sources
move. **Not recommended** (my reasoning from the method's inputs, not a published comparison).

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
  (the "mining gold" approach of Brehmer et al., [doi](https://doi.org/10.1073/pnas.1915980117), not
  read) (§3 D). Cavendish can supply the exact joint score for the target parameters (it has the
  noise-free signal), which is a route to better sample efficiency later; not needed first.
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
- Euler deconvolution's unsuitability is reasoned from its inputs, not tested.
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

## Papers I could not read

- Reid et al. 1990, Euler deconvolution: https://doi.org/10.1190/1.1442774
- Thompson 1982, EULDPH: https://doi.org/10.1190/1.1441278
- Zhang et al. 2000, Euler deconvolution of gravity tensor gradient data: https://doi.org/10.1190/1.1444745
- Li 2001, 3-D inversion of gravity gradiometer data: https://doi.org/10.1190/1.1816383
- Brehmer et al. 2020, Mining gold from implicit models: https://doi.org/10.1073/pnas.1915980117
- Hughes & Thorne 1998, Seismic gravity-gradient noise: https://doi.org/10.1103/PhysRevD.58.122002
- Stray et al. 2022, Quantum sensing for gravity cartography (a field validation of an atom
  gradiometer against a known tunnel): https://doi.org/10.1038/s41586-021-04315-3
- Kay, *Fundamentals of Statistical Signal Processing: Estimation Theory* (1993), the textbook CRB
  reference: no DOI; not read.
