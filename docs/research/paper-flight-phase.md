# The paper's moving bodies with the real interferometer phase

Research note for the polyhedral-gravity paper (draft 3) and its referee report
(`docs/papers/polyhedral-gravity/REVIEW-draft-3.md`, S2 and S13). Question: what do the paper's
moving-body results become when the leading-order instantaneous k_eff T² Δg_z is replaced by the
phase AION-10 actually reads, and where does the instantaneous read hold? Scripts, data and figures
are in [`paper-flight-phase/`](paper-flight-phase/). The pass criteria were written and committed
before anything was measured ([`CRITERIA.md`](paper-flight-phase/CRITERIA.md), commit `8dcc13d`).
Every number below comes from one of those scripts and its JSON.

## Summary

- **The real phase is Cavendish's flight integral, and the script that computes it here is Cavendish
  to 2×10⁻¹⁵ rad** (C1, below). It is first order in the source, and the source moves during the
  2T = 1.46 s shot.
- **The paper's moving-body numbers are wrong mostly in space, not in time.** The arms of each
  interferometer separate by v_kick T = 4.8 m. That is the same size as the baseline, and larger than
  the 1.5–2 m distances the paper studies. So k_eff T² Δg_z, taken at the two launch points, is not
  the leading term of anything at those distances. For Spot walking past at 2 m:
  - the instrument reads **+6.5 rad**, where the paper's read gives **−4.0 rad**;
  - the time-averaging effect the referee asked about is a **−5 %** correction on top of that.
- **The pose difference shrinks.** The "4–18 % between poses" (Table 7) becomes **3.1–4.4 %** for
  Spot, the CAD part and the bunny. The 18 % was an artefact of reading g_z at the launch point,
  level with the body. The pose effect itself is real, at 0.14–0.29 rad, far above the noise.
- **The cut-cell error on the moving torus is 9–49 times the paper's (C7 fails).** In the real read
  the body-frame errors at 1.5 m are:

  | technique | real read | paper's read | ratio |
  |---|---|---|---|
  | (c) fill at the centroid | 4.6×10⁻⁴ rad | 4.9×10⁻⁵ rad | 9 |
  | (d) cut-cell moments | 1.3×10⁻⁵ rad | 2.6×10⁻⁷ rad | 49 |

  The signal is about the same size (70 rad against −69 rad). The paper's read put the lower point
  on the torus's mirror plane, where every error term's g_z nearly vanishes. Time-averaging changes
  these errors by only 2–12 %.
- **Transfer function.** For a slow, uniform gradient the transfer function is the textbook
  sinc²(πfT) (Cheinet et al. 2008):
  - down 1 % at 0.076 Hz, 10 % at 0.24 Hz and half at 0.61 Hz;
  - first null at 1/T = 1.37 Hz;
  - 1.2×10⁻³ at 10 Hz.

  Near a source the same thresholds hold to within a factor of about 1.5 in frequency.
- **Where the instantaneous read holds (time only).** For a point source passing at closest
  approach d with speed v, the error is within **1 % for vT/d ≲ 0.2** and within **10 % for
  vT/d ≲ 0.8**. The measured thresholds by distance are in §4. For a walker at 2 m the error is
  3–5 %; for a car at 10 m/s, 10 m away, it is 8–10 %.
- **The torus lines become much smaller, and the separable estimate misses by 3–4 times.** With
  world-fixed voxels at 1 m/s, the (c) line at 1/h (10 Hz) is **1.9×10⁻⁹ rad** in the real read,
  against 5×10⁻⁷ rad in the paper's read (envelope peaks). That is 3.7×10⁻³ of the paper's value,
  not the 1.2×10⁻³ the separable sinc² estimate gives (C6 fails as written). At 0.01 m/s (0.1 Hz)
  the real line is **9 times larger** than the paper's.
- **Recommendation.** For every moving-source result in the paper, read the phase with the flight
  integral, which is `cav run` or `flight.py`. Only that read can be validated against a known answer
  (C1). Do not apply a transfer function to the paper's instantaneous numbers: at the paper's
  distances the result would still be wrong in sign.

**The criteria, against what was measured** (written before measuring, `CRITERIA.md`):

| criterion | result | where |
|---|---|---|
| C1, the Python is `cav` to ≤ 1e-10 rad | **pass** (≤ 1.8×10⁻¹⁵) | §1 |
| C2, the arms' transfer is sinc² to 1e-9 | **pass** (8.9×10⁻¹⁶) | §2 |
| C3, far-field response is sinc² to 1 % for f ≤ 2 Hz | **fail as written**: relative difference at the 1.37 Hz null; absolute ≤ 5.8×10⁻³ | §2 |
| C4, −φ''T²/12 predicts the time error to a factor 1.5 | **fail**: 0.49–1.73 | §3.2 |
| C5, pose difference within a factor 2 of the paper's | **fail**: 3.2–4.3 times smaller | §3.1 |
| C6, world-frame line = leading × \|H\| to a factor 2 | **fail**: 2.9 times larger | §3.3 |
| C7, (d)'s relative error within a factor 2 of the paper's | **fail**: 48 times larger | §3.4 |

None was moved. The failures are the findings: the paper's read is not a good proxy for the real one
at its distances.

## 1. What the instrument reads, and what Cavendish computes

**Cavendish's sequence** (`crates/cav-core/src/forward.rs`, defaults in
`crates/cav-core/src/defaults.rs`):

| quantity | value |
|---|---|
| atom | ⁸⁷Sr |
| kicks | n = 1000 photon momenta at λ = 698 nm |
| v_kick | n ħk/m = 6.578 m/s |
| u₀ | 3.86 m/s |
| T | 0.73 s |
| Δr | 5 m |
| cycle | 2 s |

The kicks are instantaneous:
- the beam splitter at launch;
- the reversal at T;
- recombination at 2T.

An arm that falls back to its launch height stays there (the floor). One shot reads

δφ_k = (m_A/ħ) ∫₀^{2T} [V(upper arm of k, t) − V(lower arm of k, t)] dt, ΔΦ = δφ₁ − δφ₂,

with the source evaluated at the same instant as the atoms. These are AION-10's published numbers
as Carlton & McCabe use them (T = 0.73 s, v_l = 3.86 m/s, Δr = 5 m, n = 1000, Δt = 2 s;
[arXiv:2308.10731](https://arxiv.org/abs/2308.10731) §II, read).

**This is the right phase at first order in the source.** It is the action of the perturbing
potential along the unperturbed arms, the perturbative result Hogan, Johnson & Kasevich use for
gravity inhomogeneities ([arXiv:0806.3261](https://arxiv.org/abs/0806.3261) §5.2.1, eqs. 61–62, read).
They credit it to Storey & Cohen-Tannoudji (1994; not read, in `papers-to-fetch.md`). Carlton &
McCabe use exactly this integral for moving masses at the AION site, with V(z, t) of a moving point
mass (their eqs. 11–12; read). They estimate the non-classical correction at ~10⁻¹⁶ g (§III, read),
which is negligible here.

**Effects that are not in it, and so not in this note:**
- finite pulse durations, which low-pass the response above ~1/τ_pulse (Cheinet eq. 8);
- the laser-phase and separation-phase terms at second order in the source;
- Earth's gradient during the flight.

The engine's own note sizes the first as a 0.14–7 % change of every signal for 1–50 ms trains
(`gradiometer-environment.md` §4).

**C1: the Python is Cavendish (pass).** [`flight.py`](paper-flight-phase/flight.py) ports the
engine's arms, floor and panel cutting. [`crosscheck.py`](paper-flight-phase/crosscheck.py) compares
it with `cav run` (release build of `2767d95`, not dirty) on two moving point-mass scenes in
[`scenes/`](paper-flight-phase/scenes/), 30 readouts each:

| scene | max \|py − cav\| | control: frozen vs cav |
|---|---|---|
| 80 kg walking at 1.4 m/s, 2 m off, 1 m up | 8.3×10⁻¹⁶ rad | 2.9×10⁻² rad |
| 1 t at 10 m/s, 5 m off, 7 m up | 1.8×10⁻¹⁵ rad | 0.56 rad |

Data: [`crosscheck.json`](paper-flight-phase/crosscheck.json).

**Three reads of one shot are compared below:**
- **flight**: the integral above;
- **frozen**: the same integral with the source frozen at the π pulse;
- **leading**: the paper's k_eff T² [g_z(Δr) − g_z(0)] at the launch points.

The errors split as follows:
- *temporal* error = flight − frozen, which is what the referee's S2 asks about;
- *spatial* error = frozen − leading, which the referee did not raise;
- the paper's total error is the sum of the two.

## 2. The transfer function

For a uniform gradient Γ(t) the arms' potential difference is −Δz(t)·[a + Γ z̄(t)]. The
gradiometer phase is therefore

ΔΦ = (m_A/ħ) Δr ∫ Δz(t) Γ(t) dt.

For instantaneous kicks and no floor, Δz is the triangle v_kick·(T − |t − T|), so

|H(f)| = k_eff T² Δr · sinc²(πfT).

This is Cheinet et al.'s eq. 9, |G(ω)| = (4/ω) sin²(ωT/2), turned into an acceleration response by
H_a = k_eff |ωG|/ω² (their eqs. 6, 9 and 20; [arXiv:physics/0510197](https://arxiv.org/abs/physics/0510197),
read; published as [doi:10.1109/TIM.2007.915148](https://doi.org/10.1109/TIM.2007.915148), not read).
Results ([`transfer.py`](paper-flight-phase/transfer.py), [`transfer.json`](paper-flight-phase/transfer.json),
figure [`transfer.pdf`](paper-flight-phase/transfer.pdf)):

- **C2 (pass).** The numerical |H(f)|/H(0) from the arms equals sinc²(πfT) to 8.9×10⁻¹⁶ for f ≤ 10 Hz.
  The control, with T 1 % off, misses by 8.3×10⁻³.
- **The engine's own arms** (u₀ = 3.86 m/s, with the floor) differ from sinc² by at most 1.1×10⁻⁵.
  Their separation area is vT² × (1 − 7.5×10⁻⁶), so the floor does not matter.
- **Where it falls:**

  | point | frequency | value of sinc² |
  |---|---|---|
  | 1 % down | 0.076 Hz | |
  | 10 % down | 0.24 Hz | |
  | half | 0.61 Hz | |
  | first null | 1/T = 1.37 Hz | 0 |
  | 10 Hz | | 1.24×10⁻³ (envelope 1/(πfT)² = 1.9×10⁻³) |

  Above about 0.5 Hz it falls as f⁻² in amplitude (f⁻⁴ in power, Cheinet §V). The referee's S2
  formula is this one.
- **The cycle aliases it.** One shot per 2 s puts the Nyquist frequency at 0.25 Hz. Anything above
  that folds down, attenuated by H, into the shot series (Cheinet eq. 17).
- **The near field.** A point mass whose mass is modulated at f, at distance d, has the response
  R(f) of the flight read relative to the frozen read ([`transfer.json`](paper-flight-phase/transfer.json),
  `near`). |R − 1| reaches 1 % and 10 % at these frequencies:

  | position | 1 % | 10 % |
  |---|---|---|
  | level with the lower launch, d = 1–50 m | 0.051–0.085 Hz | 0.17–0.27 Hz |
  | mid-tower | 0.075–0.077 Hz | 0.24–0.25 Hz |
  | sinc² | 0.076 Hz | 0.24 Hz |

  The near-field response is sinc² to within a factor of about 1.5 in frequency, but not in shape.
  At d = 1.5 m, |R| departs from sinc² by up to 0.05 below 0.5 Hz and 0.11 below 2 Hz.
- **C3 fails as written.** The criterion was "within 1 % relative for f ≤ 2 Hz at 50 m". That range
  contains the null at 1.37 Hz, where any relative difference diverges. The absolute difference at
  50 m is ≤ 3.4×10⁻³ below 0.5 Hz and ≤ 5.8×10⁻³ below 2 Hz. The criterion was badly posed, not
  the physics; it is left as failed.

## 3. The paper's moving-body results, redone

### 3.1 Walking past (paper Table 7, Fig. 13c–d)

The geometry is the paper's (`exact.py passby`):
- each mesh 1 m across, ρ = 2700 kg/m³;
- centre of mass level with the lower launch, 2 m from the axis;
- walking at 1.4 m/s;
- three poses.

Each body is summed whole with the polyhedral closed form. The flight read is integrated along the
arms with the engine's rule (Gauss order 6, panels ≤ 0.1 s, converged to 3×10⁻¹³ relative).
Sources: [`walkpast.py`](paper-flight-phase/walkpast.py), `walkpast-<body>.json`,
[`walkpast.md`](paper-flight-phase/walkpast.md), [`walkpast.pdf`](paper-flight-phase/walkpast.pdf).

Peaks (rad), upright / turned 90° about z / on its side, and the largest difference between poses
at any one time over the peak:

| body | leading (paper) | frozen | **flight (real)** | pose difference: leading → flight |
|---|---|---|---|---|
| Spot | −3.98 / −4.68 / −4.00 | 6.81 / 7.14 / 6.81 | **6.48 / 6.75 / 6.47** | 18.0 % → **4.2 %** |
| CAD part (mk7) | −1.94 / −1.90 / −2.31 | 3.47 / 3.37 / 3.49 | **3.28 / 3.20 / 3.30** | 17.9 % → **4.4 %** |
| bunny | −5.00 / −5.51 / −5.53 | 9.94 / 9.67 / 10.00 | **9.41 / 9.19 / 9.47** | 9.9 % → **3.1 %** |

(The paper prints the leading peaks as magnitudes. Its sign is negative: Fig. 13c–d.)

The bunny was read every 0.2 s over ±5 s, the others every 0.1 s over ±6 s.

- **Sign and size.** The real read is positive and 1.6–1.9 times the paper's magnitude. With the body
  level with the lower launch, g_z(0) ≈ 0. The lower interferometer's arms, however, span 0–5 m:
  - its lower arm stays near the body;
  - its upper arm climbs away from it;
  - the potential difference between the arms is large and positive.

  At this distance the arms do not sample a gradient at all.
- **Time (the referee's S2).** Flight against frozen is −5 % at the peak (0.944–0.951 of the frozen
  peak on every pose and the point mass, for all three bodies). The largest temporal error over the
  pass is 4.9 % (Spot), 5.5 % (CAD part) and 5.3 % (bunny) of the peak. The real curve is broader:
  for Spot and the CAD part its full width at half maximum is 2.6 s, against 2.4 s frozen, at 0.1 s
  sampling. The leading curve's width (4.8–5.8 s) belongs to a
  different function, g_z at 5 m.
- **C5 fails.** The criterion was "pose difference within a factor 2 of the paper's"; it is a factor
  3.2–4.3. The "4–18 %" becomes 3.1–4.4 % on these three bodies; the bunny goes from 9.9 % to 3.1 %.
  - The paper's largest differences, 18 % for Spot and the CAD part, came from where the leading
    read is taken: g_z at the launch point, level with the body, is close to its zero crossing, so
    it is very sensitive to pose.
  - What survives: a walker-sized body at 2 m still reads its pose at 0.28 rad (Spot), 0.29 rad
    (bunny) and 0.14 rad (CAD part). That is 10³–10⁴ times the 5×10⁻⁵ rad per-shot noise.
- **A better instantaneous read exists but is not good enough here.** For a potential quadratic in z
  the flight integral equals a gradient read at the arms' Δz-weighted mean height. That height is
  z_eff = 2.17 m above each launch point (`flight.effective_height`).
  - At z_eff the instant read gives peaks of 8.11 rad (Spot), 4.02 rad (CAD part) and 11.56 rad
    (bunny): still 22–25 % high, because the field is not quadratic over 5 m at 2 m distance.
  - For a static point source ([`spatial.py`](paper-flight-phase/spatial.py),
    [`spatial.json`](paper-flight-phase/spatial.json)), compared with the frozen read:

    | read | within 1 % beyond | within 10 % beyond |
    |---|---|---|
    | at the effective height | 11 m (level with the launch) / 24 m (mid-tower) | 8.4 / 6.3 m |
    | at the launch points (the paper) | 94 / 48 m | 29 / 15 m |

### 3.2 Where the instantaneous read holds

Point-mass passes give the temporal error only (flight against frozen), as the largest error over
the pass divided by the peak ([`walkpast.py map`](paper-flight-phase/walkpast.py),
[`map.json`](paper-flight-phase/map.json), [`map.pdf`](paper-flight-phase/map.pdf)). The speed at
which it reaches 1 % and 10 %, with the source level with the lower launch / at mid-tower:

| closest approach d | 1 % at | 10 % at |
|---|---|---|
| 1 m | 0.34 / 0.45 m/s | 1.2 / 1.7 m/s |
| 1.5 m | 0.47 / 0.62 | 1.7 / 2.3 |
| 2 m | 0.59 / 0.79 | 2.1 / 2.8 |
| 3 m | 0.79 / 1.1 | 2.7 / 3.8 |
| 5 m | 1.0 / 1.6 | 3.4 / 5.6 |
| 7 m | 2.8 / 2.1 | 10 / 7.4 |
| 10 m | 3.4 / 2.9 | 12 / 10 |
| 20 m | 5.8 / 5.6 | 20 / 20 |

- **Rule of thumb:** within 1 % for vT/d ≲ 0.2, and within 10 % for vT/d ≲ 0.8.
  - At small v the error grows as v² (×2.13 per ×1.46 step in v).
  - Measured in units of the source's own time scale, vT/d = 0.2 is f ≈ v/(2πd) ≈ 0.04 Hz.
- **Cases:**

  | source (horizontal pass) | error |
  |---|---|
  | walker, 1.4 m/s at 2 m | 3.0–5.1 % |
  | 2.5 m/s at 10 m | 0.56–0.74 % |
  | car, 10 m/s at 10 m | 7.6–9.7 % |
  | 20 m/s at 5 m | 46–67 % |
  | 30 m/s at 20 m | 18–19 % |
  | 30 m/s at 1 m | 83–89 % |

  The two values in each row are the source at mid-tower and level with the lower launch.
- **C4 fails.** The law −φ''T²/12, from sinc² ≈ 1 − (ωT)²/12, predicts the measured error at the
  peak only to within 0.49–1.73 times, even where the error is 10⁻³–10⁻². The 1.5 bound is
  exceeded at d = 1 m (×1.5–1.7) and at d = 5 m level with the launch (×0.5–0.65). At d = 5 m the
  static read is near a zero crossing.
  - So the near-field time weighting is not the triangle.
  - The thresholds above are measured; the law is not quoted.
- **Spatial error, for scale** (launch-point leading read against frozen, largest over the pass over
  the peak, `spatial_max_over_peak`):

  | d (m) | 1 | 1.5 | 2 | 3 | 5 | 7 | 10 | 20 |
  |---|---|---|---|---|---|---|---|---|
  | level with the launch | 1.2 | 1.4 | 1.6 | 2.3 | 6.9 | 2.7 | 0.92 | 0.20 |
  | mid-tower | 0.89 | 0.83 | 0.77 | 0.65 | 0.44 | 0.30 | 0.18 | 0.05 |

  Below about 10 m the spatial error is larger than the temporal error at any walking or driving
  speed.

### 3.3 The moving torus: lines from world-fixed voxels (paper Fig. 12)

The paper's torus passes at 1.5 m (closest surface), pitch h = 0.1 m. The world-fixed error is
periodic in the lattice phase δ = x mod h, sampled at 16 phases as the paper did, and interpolated
trigonometrically. Harmonic k is a line at f = kv/h, integrated with e^{2πikx(τ)/h} inside the
flight (Gauss order 10, panels ≤ 0.01 s at 1 m/s).

What was computed, and what was not:
- Only the (d) − (c) part, the cells' quadrupoles, was computed: it is cheap, and (c)'s line is that
  to about 3 %, since the paper's (d) line is 1/36 of its (c) line.
- The (d) line itself was not computed with the flight read.

Sources: [`torus.py world`](paper-flight-phase/torus.py), [`torus-world.json`](paper-flight-phase/torus-world.json).
Largest envelope over the track, in rad:

| v (m/s) | k | f (Hz) | leading | frozen | **flight** | flight/frozen | sinc²(πfT) |
|---|---|---|---|---|---|---|---|
| 0.01 | 1 | 0.1 | 6.7×10⁻⁷ | 5.9×10⁻⁶ | **5.8×10⁻⁶** | 0.978 | 0.983 |
| 0.01 | 2 | 0.2 | 1.8×10⁻⁷ | 7.0×10⁻⁷ | **6.5×10⁻⁷** | 0.924 | 0.932 |
| 0.01 | 3 | 0.3 | 3.5×10⁻⁸ | 3.7×10⁻⁷ | **3.0×10⁻⁷** | 0.80 | 0.85 |
| 0.1 | 1 | 1 | 6.7×10⁻⁷ | 5.9×10⁻⁶ | **4.3×10⁻⁷** | 0.073 | 0.107 |
| 0.1 | 2 | 2 | 1.8×10⁻⁷ | 7.0×10⁻⁷ | **1.1×10⁻⁷** | 0.154 | 0.047 |
| 0.1 | 3 | 3 | 3.5×10⁻⁸ | 3.7×10⁻⁷ | **7.1×10⁻⁹** | 0.019 | 0.0067 |
| 1 | 1 | 10 | 5.1×10⁻⁷ † | 5.9×10⁻⁶ | **1.9×10⁻⁹** | 3.2×10⁻⁴ | 1.2×10⁻³ |

† At 1 m/s only the five positions within ±1 m of closest approach were read.

- **Below 0.3 Hz,** flight/frozen follows sinc² to 0.5–5 %.
- **Above it, the near-field response departs from sinc² by factors of 0.3 to 3.**
- **The static read of a world-fixed line is 9 times the paper's** (frozen against leading), for the
  reason of §3.1.
- **C6 fails as written.** The criterion was "flight = leading × |H(10 Hz)| within a factor 2": the
  measured 1.9×10⁻⁹ is 2.9 times 5.1×10⁻⁷ × 1.24×10⁻³. The opposite estimate, frozen × sinc², is
  3.9 times too big. Neither separable estimate is good to a factor 2.
- **What to quote:** at 1 m/s the (c) line at 10 Hz is 1.9×10⁻⁹ rad, against the paper's 3.6×10⁻⁷
  (its spectral amplitude). Scaling by the paper's own (d)/(c) ratio puts (d)'s line near 5×10⁻¹¹ rad.
  That scaling is an estimate, not measured.
- **The body-frame "no lines" result holds structurally.** Body-frame voxels have no δ dependence, so
  there is no line to attenuate. That needs no measurement; C6's body-frame half is met by
  construction.

### 3.4 Cut-cell moments on a moving body (C7)

The paper's torus pass ([`torus.py body`](paper-flight-phase/torus.py),
[`torus-body.json`](paper-flight-phase/torus-body.json)):
- h = 0.1 m, body-frame voxels;
- 1 m/s, positions every 0.25 m over ±4 m;
- 2204 cut cells, whose exact pieces have 85 904 triangles.

The errors are against the exact clipped pieces. Every read is linear, so each part (mesh, pieces,
points, quadrupoles) is read once and the techniques are sums of parts. Largest over the pass:

| read | signal | (c) error | (d) error | (c)/signal | (d)/signal |
|---|---|---|---|---|---|
| leading (paper) | −68.9 rad | 4.9×10⁻⁵ | 2.6×10⁻⁷ | 7.0×10⁻⁷ | 3.8×10⁻⁹ |
| frozen | 71.5 | 4.7×10⁻⁴ | 1.5×10⁻⁵ | 6.5×10⁻⁶ | 2.0×10⁻⁷ |
| **flight (real)** | **70.3** | **4.6×10⁻⁴** | **1.3×10⁻⁵** | 6.5×10⁻⁶ | 1.8×10⁻⁷ |

- **The leading row reproduces the paper.** Its (d) maximum is 2.6×10⁻⁷ rad, against the paper's
  2.8×10⁻⁷ "in either frame".
- **The quadrature is converged.** At the peak (x = 0) the frozen (d) error is unchanged to 1×10⁻⁸
  relative from the engine's rule (order 6, panels of 0.1 s) to order 12 with panels of 0.025 s
  ([`torus_qcheck.py`](paper-flight-phase/torus_qcheck.py), [`torus-qcheck.json`](paper-flight-phase/torus-qcheck.json)).
- **C7 fails.** The criterion was "relative error within a factor 2 of the paper's". The real read's
  relative error is 9 times the paper's for (c) and 48 times for (d).
- **Why.** The torus is symmetric about its mid-plane, z = 0, which is the lower launch height. On
  that plane g_z vanishes for the body and nearly for every cell's error. The paper's read therefore
  sees only the upper point, 5 m up. The lower interferometer's arms instead sweep 0–5 m, 1.5 m from
  the surface, where the cut cells' residual higher moments are strongest.
- **Time is the small part.** Flight against frozen changes the errors by 2 % for (c) and 12 % for
  (d) at the peak.
- **What carries over.** The ranking (d) ≪ (c), by 36 times in the real read against 190 times in the
  paper's read, and the smoothness of body-frame errors. The figure of a few 10⁻⁷ rad at 1.5 m for
  (d) does not carry over: on this body it is 1.3×10⁻⁵ rad.
- **Not checked.** Whether the measured *orders* in h survive was not tested: one pitch only.

## 4. Where the instantaneous approximation holds, in one place

| regime | temporal (time-averaging) error | spatial error of the launch-point gradient read |
|---|---|---|
| within 1 % | vT/d ≲ 0.2 (e.g. 0.6 m/s at 2 m, 3.4 m/s at 10 m); f ≲ 0.076 Hz for a uniform gradient | d ≳ 94 m level with the lower launch, ≳ 48 m mid-tower |
| within 10 % | vT/d ≲ 0.8 (2.1 m/s at 2 m, 12 m/s at 10 m); f ≲ 0.24 Hz | d ≳ 29 m level with the launch, ≳ 15 m mid-tower |
| fails (> 50 % or wrong sign) | vT/d ≳ 3 (e.g. 20 m/s at 5 m: 46–67 %); lines above 1/T = 1.37 Hz, read at ≲ 10⁻² of their instantaneous size | d ≲ 10 m: error 0.4–7 times the signal, and the sign is wrong at 2 m beside the lower launch (§3.1) |

The time column is from point-mass passes (§3.2); the frequency figures are sinc² (§2). The space
column is from `spatial.json`, for a static point source. Read at the arms' effective height
(z_eff = 2.17 m), the instant gradient is within 1 % beyond 11 m (level with the launch) or 24 m
(mid-tower), and within 10 % beyond 8.4 m or 6.3 m.

## 5. What the paper should say

1. **§1, eq. 1 and the leading-order sentence.** Say that k_eff T² Δg_z is the limit for a potential
   quadratic over the arms (0–10 m here), and that the arms separate by v_kick T = 4.8 m. Within
   ~10 m of the tower, and certainly at 1.5–2 m, the gradiometer does not read a gradient. The
   paper's error *ranking* carries over. Its error *sizes* at 1.5 m do not, and neither do its phase
   values:
   - the torus's (d) error is 1.3×10⁻⁵ rad, not 2.8×10⁻⁷ rad (§3.4);
   - the walk-past signals are 1.6–1.9 times larger and of opposite sign.

   §9's "the orders do not depend on this, the prefactors slightly do" is wrong on the prefactors and
   untested on the orders.
2. **Orientation and motion, Fig. 12.** Replace "10 Hz per m s⁻¹" amplitudes with the flight read:
   - at 1 m/s the (c) line at 1/h is 1.9×10⁻⁹ rad, not 3.6×10⁻⁷ rad;
   - (d)'s line is estimated at ~5×10⁻¹¹ rad;
   - lines above 1/T are read at ≤ 10⁻² of the instantaneous amplitude and alias into the 0.25 Hz
     band of a 2 s cycle;
   - for slow motion (0.1 Hz lines) the real line is about 9 times larger than the paper's.

   Keep the conclusion: body-frame storage has no lines.
3. **Walking past, Table 7, Fig. 13c–d.** Replace the peaks with the flight read and the pose
   difference with 3.1–4.4 %. Keep "a gradiometer that sees a walker-sized body
   at 2 m sees its shape", now as a 0.14–0.29 rad pose effect. Add that time-averaging over the shot
   lowers the peak by 5 % and widens it from 2.4 s to 2.6 s (FWHM).
4. **A sentence on when the instant read holds:** vT/d ≲ 0.2 for 1 %, ≲ 0.8 for 10 %, and
   |H(f)| = k_eff T² Δr sinc²(πfT) for a uniform gradient (Cheinet et al. 2008).
5. **S13.** Give per-shot noise: 10⁻⁴ rad/√Hz at a 2 s cycle is 5×10⁻⁵ rad per shot
   (`gradiometer-environment.md` §0). The 0.1-rad error then compares with it as 2000 times, not
   "three orders above a spectral density".
6. **§5 and Table 8: rerun at 1.5 m with the flight read before quoting sizes.** The (d) row
   "a few 10⁻⁷ rad" is a launch-point number. On the torus the real (d) error is 50 times larger
   (§3.4).
7. **Table 8's caption.** Say that its measured rows are leading-order and static, as the referee
   asks.

**Proposed replacement figure:** [`flight-phase.pdf`](paper-flight-phase/flight-phase.pdf)
(paper style, Okabe–Ito):
- (a) where the instant read is within 1 % / 10 % in speed × distance (solid: source level with the
  lower launch; dashed: mid-tower), with a walker, a 2.5 m/s pass at 10 m and a car;
- (b) Spot walking past in three poses, flight read (solid) against the paper's (dotted);
- (c) the torus's body-frame (c) and (d) errors over the pass, flight against leading;
- (d) the torus's world-fixed lines against frequency, leading against flight, with sinc².

It replaces Fig. 12 and Fig. 13c–d.

**Proposed replacement table:** the §3.1 table (leading / frozen / flight peaks and pose
difference), with a column for the arms' effective-height read if the paper wants to show why the
launch-point read fails.

## 6. Validation cases the literature uses

- **Response to a phase or acceleration modulation against sinc²** (Cheinet et al. 2008, measured
  transfer function, Figs. 4–5). Here this is C2, exact to 10⁻¹⁵.
- **Moving point masses near AION-10** (Carlton & McCabe 2023, Fig. 3: lift at 2.5 m/s, 10 m; person
  on stairs; car at 10 m/s). Here this is C1 against the engine; the engine's own oracle tier already
  carries Carlton & McCabe's "pass" anchor (`scenes/dev/pass.ron`).
- **The perturbative phase against closed forms for polynomial potentials** (Hogan et al. 2008 §5,
  Table 1; Dubetsky & Kasevich 2006, Table I). That is the engine's existing Earth-gradient work, not
  repeated here.

## 7. What I could not verify

- **AION-10's real pulse sequence and durations.** The finite-pulse low-pass (Cheinet eq. 8) is not
  modelled; at 1–50 ms trains it changes every signal by 0.14–7 % (`gradiometer-environment.md`).
- **Storey & Cohen-Tannoudji 1994 and the published Cheinet** were not read (cited through Hogan et
  al. and the arXiv preprint). Dimopoulos et al. 2008 (Hogan's ref. [34] on short-wavelength
  suppression) was not read. All are in `papers-to-fetch.md`.
- **The (d) world-frame line** was not computed with the flight read (only (c)'s quadrupole part):
  the quoted ~5×10⁻¹¹ rad is a scaling.
- **The bunny** was read at 0.2 s spacing over ±5 s (the others at 0.1 s over ±6 s), for cost.
- **Second-order effects.** Everything here is first order in the source. Laser-phase and
  separation-phase terms at second order, and the arm trajectories' own perturbation by the source,
  are not included (Lean's `phase_sum` assumes the same: referee S10).

## 8. Further questions (not chased)

- Rerun the paper's static §4–§5 benchmark with the flight read. The error orders in h should hold,
  but every absolute phase at 1.5 m changes by order one (§3.2's spatial row). It is cheap with
  `flight.py`'s `frozen_phase` and the study's per-cell sums.
- Whether the engine should offer the effective-height gradient read as a fast far-field read: it is
  within 1 % beyond 11–24 m and exact for quadratic potentials.
- The aliased spectrum of a shot series at a 2 s cycle for the torus lines (here only the envelope
  per harmonic was computed).
