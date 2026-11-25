# What else an AION-10 gradiometer sees: environment and instrument systematics, and what Cavendish should simulate

> Research note, 25 Nov 2026. Scope: everything an atom-interferometer gravity gradiometer (AION-10, ⁸⁷Sr,
> 698 nm, n = 1000, T = 0.73 s, Δr = 5 m) reads that is *not* seismic ground motion or seismic Newtonian
> noise (another note covers those). It builds on `docs/research/s14-wider-world.md` (far field, tides,
> rotation), `docs/research/s15-instruments-anywhere.md` (rotation and gradient phase terms, verified there)
> and `docs/research/s13-heat.md` (air temperature), and does not repeat their derivations.
>
> **Sources.** Unlike the S14 note, this one was written with network access: the papers marked **[read]**
> below were downloaded from arXiv and read in the relevant sections (the text extraction garbles some
> printed prefactors; where I relied on one I re-read it in a second extraction and say so). Everything
> else is marked **[snippet]** (abstract or search result only) or **[derived]** (computed here).
> **Numbers.** Every size that is not a quotation comes from
> [`gradiometer-environment/sizes.py`](gradiometer-environment/sizes.py) (standard library only; output in
> [`sizes.out.txt`](gradiometer-environment/sizes.out.txt)), which uses the engine's own arm rules
> (`crates/cav-core/src/forward.rs`: one n-photon kick at 0, the π pulse at T, a floor at the launch height).

---

## 0. One-page summary

**The instrument's ruler.** A slow, uniform vertical gradient Γ gives ΔΦ = C·Γ with C = k_eff T² Δr =
2.40×10¹⁰ rad per s⁻² (24 rad per eötvös); one shot's noise at the goal 10⁻⁴ rad/√Hz
([AION, Table 1](https://arxiv.org/abs/1911.11755)) and a 2 s cycle is 5×10⁻⁵ rad. Rad/√Hz figures below
divide by 2 to give rad per shot.

**Three findings that change what Cavendish should build.**

1. **The moving-mass signals Cavendish targets are large, and published for this very site.** Carlton &
   McCabe simulate the Beecroft stairwell with the same semiclassical phase Cavendish uses: a person on the
   stairs beside the tower gives several radians, a 1000 kg lift at 10 m about 1.5 rad, people in the foyer
   and cars on the road 0.05–0.2 rad, five rodents under the base ~50 µrad
   ([Carlton & McCabe 2023, §III, Fig. 3](https://arxiv.org/abs/2308.10731) **[read]**). That paper is the
   best validation target in the literature for Cavendish's core and should become an oracle case.
2. **Instrument systematics are as large as the environment, and none is in the forward model.** Kinematic
   jitter through Earth's own gradient (1.5×10⁻⁵ rad per nm of launch height, per IFO) and through Earth's
   rotation (4.3×10⁻⁴ rad per nm/s of transverse velocity) exceed shot noise at nanometre and sub-nm/s
   jitter, so real instruments compensate: the gradient by changing the middle pulse's wave number by
   Δk/k = ΓT²/2 ([Roura 2017, Eq. 11](https://arxiv.org/abs/1509.08098) **[read]**), rotation with a tip-tilt
   mirror ([MAGIS-100 §4, Fig. 17](https://arxiv.org/abs/2104.02835) **[read]**;
   [AION-10 TDR §3.6](https://arxiv.org/abs/2508.03491) **[read]**). What survives is a *residual*, and that is
   what Cavendish should carry. Scaled to n = 1000 from MAGIS-100's published budget, wavefront × kinematic
   jitter, pointing jitter and AC Stark shifts are each ~5×10⁻³ rad/√Hz at MAGIS's reference parameters,
   fifty times the AION-10 goal: AION-10's requirements on those parameters must be ~50× tighter than
   MAGIS's ([MAGIS-100 §5.2, 5.4, 5.5](https://arxiv.org/abs/2104.02835)).
3. **Clock-transition shifts (magnetic, blackbody, AC Stark, and scalar dark matter) are one mechanism**: an
   energy shift felt only by the arm in the excited state. In the engine's single-kick sequence the upper
   arm is excited on [0, T] and the lower on [T, 2T], so a uniform, constant shift cancels exactly; a
   linear-in-height static shift is read by each IFO (11 rad per Hz/m) and cancels in the gradiometer;
   only its curvature survives (55 rad per Hz/m²) **[derived]**. This corrects the S13 note's first guess
   of ~0.1 rad per kelvin for the blackbody shift: a 1 K parabolic temperature profile along the tower gives
   0.14 rad, *static*; a 0.1 K/h difference in drift between the two IFOs gives 3×10⁻⁶ rad; MAGIS's estimate
   for a 1 K, 1 h oscillation is 10⁻⁶ rad/√Hz ([MAGIS-100 §5.9](https://arxiv.org/abs/2104.02835);
   [Mitchell et al. 2022 §6.1](https://arxiv.org/abs/2202.04763) **[read]**). The linear Zeeman shift of the
   stretched line is 489 Hz/G ([TDR §2.3](https://arxiv.org/abs/2508.03491)); with the TDR's 1 µG/√Hz field
   noise requirement, the simple sequence gives 1×10⁻³ rad/√Hz differential, ten times the goal, so the real
   sequence must keep the arms mostly in the ground state (as MAGIS states it does) or co-magnetometer
   ([MAGIS-100 §5.8](https://arxiv.org/abs/2104.02835)). Which arm is excited when is therefore an instrument
   parameter, not a detail.

**Recommendation (§7).** In order: (1) an oracle case reproducing Carlton & McCabe's Fig. 3 and their
Eq. (13); (2) a **noise-model layer** on the readout (seeded, labelled per component, in rad) carrying shot
noise plus the residual-coupling terms with closed forms (gradient × launch jitter, rotation × transverse
jitter, wavefront, AC Stark, magnetic, timing), each parameterised by the published coupling;
(3) a **state-dependent-shift channel** in the forward model (one integral serving Zeeman, blackbody from
S13's temperature field, and S16's scalar dark matter), with an occupancy-pattern parameter;
(4) optional **Earth's gradient, Coriolis and the Roura kick** in the arms (S14/S15 already ask for the
first two); (5) a **finite kick-duration** parameter, because an LMT train of 10 ms changes every signal
by ~1.4 % **[derived]**. Environment sources (weather, rain, ground water, tides) stay where S14 put them;
this note confirms their sizes and adds the rain-on-a-roof case, which at 2–3 rad for 10 mm of water
**[derived]** belongs in the scene, not the far field.

---

## 1. Effects ranked by size (AION-10, radians)

"Size" is the typical amplitude of the phase it puts on the gradiometer reading; for noise, rad/√Hz with
rad per shot = ASD/2. "Kind": **S** static (a constant offset, invisible to a time-varying search but a
real number in a twin), **D** slow drift (hours to days), **T** transient (seconds to minutes), **N** noise
in band (0.01–0.25 Hz). "Now": what Cavendish models today (from `forward.rs`: source potential on point
arms in uniform g; no Earth gradient, rotation, internal state or noise on the readout; shot noise only in
spectrum tests).

| # | Effect | Size (rad) | Kind | Now | Closed form to validate against | Source |
|---|---|---|---|---|---|---|
| 1 | Earth's static vertical gradient | 7.4×10⁴ | S | no (by design) | C·Γ_E, Γ_E = 3.086×10⁻⁶ s⁻² | [derived]; removed in practice by [Roura Eq. 11](https://arxiv.org/abs/1509.08098), shown in [D'Amico 2017](https://doi.org/10.1103/PhysRevLett.119.253201) [snippet] |
| 2 | Person on the stairs beside the tower | ~1–6 | T | **yes (core)** | Carlton Eq. (11)–(12) numerically; Eq. (13) far field | [Carlton & McCabe Fig. 3](https://arxiv.org/abs/2308.10731) |
| 3 | Wavefront curvature × cloud temperature (1–10 nK, R = 1–10 km) | 0.05–5 (static); fluctuates with δT/T | S, N | no | k_eff T² σ_v²/R (Dickerson term 6, averaged) | [derived] from s15 note (`docs/research/s15-instruments-anywhere.md`) (Dickerson Table I) |
| 4 | Linear Zeeman (489 Hz/G), 5 mG field inhomogeneity read as curvature over 5 m | ~5 | S | no | 2π A Δr c₁ B″, A = 1.76 m s | [derived]; [TDR §2.3](https://arxiv.org/abs/2508.03491) |
| 5 | Rain, 10 mm on a roof of radius 20–50 m, 10 m above the upper IFO | 1.8–3.1 | D, T | no (needs scene water) | disc: a_z = 2πGσ(1 − d/√(a²+d²)) | [derived]; near-field dominance: [Creutzfeldt et al. 2010](https://doi.org/10.1029/2009WR008359) [snippet] |
| 6 | Lift, 1000 kg at 10 m, moving the tower's height | ~1.5 | T | **yes (core)** | as #2 | [Carlton Fig. 3](https://arxiv.org/abs/2308.10731) |
| 7 | Coriolis, per µm/s of transverse velocity jitter, uncompensated | 0.43 (4.3×10⁻⁴ per nm/s) | N | no | 2 k_eff Ω cos φ v_⊥ T² | s14 §5.1, s15 (`docs/research/s14-wider-world.md`); [Lan 2012](https://arxiv.org/abs/1110.6910) [snippet] |
| 8 | Blackbody shift, 1 K parabolic temperature along the tower | 0.14 | S | no | 2π A Δr (dν/dT) T″, dν/dT = 0.031 Hz/K | [derived]; coefficient −2.3 Hz (T/300 K)⁴ from [MAGIS §5.9](https://arxiv.org/abs/2104.02835) |
| 9 | People in the foyer (100 kg, 5 m, 12 m up); cars on the road (1–5 t, 20 m) | 0.03–0.2 | T | **yes (core)** | Carlton Eq. (13): −k_eff T² G M Δr (D²−2h²)/(D²+h²)^{5/2} | [Carlton Fig. 3](https://arxiv.org/abs/2308.10731); Eq. (13) gives 0.09 and −0.03 rad [derived] |
| 10 | Ocean-tide attraction near a coast (upper bound) | ≤5×10⁻² | D | no | πGρ_w a/d; Farrell Green's functions | s14 §2.2 (`docs/research/s14-wider-world.md`) |
| 11 | Air in a 5 m × 6 m hall, per K of mean temperature | 2.2×10⁻² | D | planned (S8, S13) | slab: 4πG δρ Δz | s13 §5.2 (`docs/research/s13-heat.md`) |
| 12 | Gravity gradient × launch jitter, per IFO: per µm height / per µm/s velocity | 1.5×10⁻² / 1.1×10⁻² | N | no | k_eff Γ T²(δz + δv T) | [Roura Eq. 4](https://arxiv.org/abs/1509.08098); 7/12 term verified in s15 (`docs/research/s15-instruments-anywhere.md`) |
| 13 | Pressure systems (10 hPa at 1000 km; 5 hPa at 300 km) | 6×10⁻³–9×10⁻³ | D | planned (S14) | 2πG(ρ₀/p₀)δp·kH/(1+kH) | s14 §2.3 (`docs/research/s14-wider-world.md`), recomputed [derived] |
| 14 | AC Stark, wavefront × kinematics, pointing × wavefront, at MAGIS's reference parameters scaled to n = 1000 | 5–6×10⁻³ rad/√Hz each | N | no | MAGIS scalings (§4) | [MAGIS §5.2, 5.4, 5.5](https://arxiv.org/abs/2104.02835) |
| 15 | Solid-Earth tide (Moon + Sun, Love factor 0.97) | 3.6×10⁻³ | D | planned (S14) | (GM/d³)(3cos²ψ − 1)(1 + 6k₂ − 3h₂) | s14 §2.1 (`docs/research/s14-wider-world.md`) |
| 16 | Linear Zeeman noise, 1 µG/√Hz differential at 0.1 Hz, simple sequence | 1.0×10⁻³ rad/√Hz | N | no | 2π c₁ δB (4/ω) sin²(ωT/2) | [derived]; requirement from [TDR §2.3](https://arxiv.org/abs/2508.03491) |
| 17 | Mean-field shift (δN/N = 0.01), n = 1000 | 5×10⁻⁴ rad/√Hz | N | no | MAGIS scaling | [MAGIS §5.7](https://arxiv.org/abs/2104.02835) |
| 18 | Timing jitter 1 ns/√Hz, with the clouds' 11 µm/s velocity difference from Γ | 1.1×10⁻⁴ rad/√Hz | N | no | MAGIS §5.10 scaling | [MAGIS §5.10](https://arxiv.org/abs/2104.02835); Δv [derived] |
| 19 | Infrasound, per pascal (adiabatic plane wave, k ≫ 1/H) | 8.5×10⁻⁵ per Pa | N | planned (S14) | 2πG δp/c²; underground: Junca Eq. (23)–(25) | [Harms §5.1](https://arxiv.org/abs/1507.05850); [Junca 2019 §IV](https://arxiv.org/abs/1902.05337) |
| 20 | Small animals near the base (5 × 0.25 kg) | ~5×10⁻⁵ | T | **yes (core)** | as #2 | [Carlton Fig. 3](https://arxiv.org/abs/2308.10731) |
| 21 | Quadratic Zeeman after co-magnetometry (−0.23 Hz/G², B₀ = 10 G, 1 µG/√Hz) | 1×10⁻⁵ rad/√Hz | N | no | 2π·2βB₀δB·(4/ω)sin²(ωT/2) | [MAGIS §5.8](https://arxiv.org/abs/2104.02835) (citing Falke 2011); [derived] |
| 22 | Blackbody drift, 0.1 K/h *difference* between the IFOs; 1 K, 1 h oscillation | 3×10⁻⁶; 1×10⁻⁶ rad/√Hz | D, N | no | −2π ν̇ T²; MAGIS §5.9 | [derived]; [Mitchell 2022 §6.1](https://arxiv.org/abs/2202.04763) |
| 23 | Mirror vibration (10⁻⁴ m/s²/√Hz) with Δv = 11 µm/s | 8×10⁻⁹ rad/√Hz | N | no | MAGIS §5.3 | [MAGIS §5.3](https://arxiv.org/abs/2104.02835) |
| 24 | Laser frequency noise (single-photon gradiometer) | ~10⁻¹³ rad/√Hz | N | no | MAGIS §5.1, term 3 of Graham Table 1 | [MAGIS §5.1](https://arxiv.org/abs/2104.02835) |
| — | Finite kick (LMT train) duration τ: relative change of *every* signal | 1.4×10⁻³ (τ = 1 ms) … 6.7×10⁻² (50 ms) | — | no | separation integral with a ramp | [derived]; "a substantial fraction of a second" for large n: [MAGIS §5.2](https://arxiv.org/abs/2104.02835) |

Read the table with three cautions. (a) Rows 3, 4, 8 and 16 use my single-kick model of which arm is excited
when; real LMT sequences keep both arms in the ground state for most of the flight (MAGIS §5.8), which
shrinks them by roughly the excited-state dwell fraction. (b) Rows 14 and 17 are *requirements in disguise*:
they are what MAGIS's reference parameters would give at n = 1000, not predictions for AION-10, whose
parameters are not published. (c) Static rows matter for a twin and for contrast, not for a search for
time-varying signals.

---

## 2. The environment (other than seismic)

### 2.1 Atmospheric mass

- **Pressure systems and the gravity admittance.** Gravimeters see ≈ −0.3 µGal/hPa: the attraction of the
  air column above (the Bouguer value 2πGδp/g = 0.43 µGal/hPa [derived]) partly offset by elastic loading.
  The figure −0.34 µGal/mbar is attributed to Merriam (1981) and −0.356 µGal/mbar was found at Table Mountain
  ([search summary of Merriam 1992](https://typeset.io/pdf/atmospheric-pressure-and-gravity-1nns7xhg19.pdf)
  [snippet]); I did not read Merriam. **For a gradiometer the admittance is the wrong quantity**: a laterally
  uniform air layer has zero vertical gradient at the ground, and what is read is the lateral structure
  through 2πG(ρ₀/p₀)δp·kH/(1+kH) (S14's E5a). Recomputed here: 6.1×10⁻³ rad (10 hPa, λ = 1000 km) and
  9.1×10⁻³ rad (5 hPa, λ = 300 km). Gravimeters see atmospheric effects below ~1 mHz, modelled as pressure
  plus loading ([Harms §5](https://arxiv.org/abs/1507.05850) **[read]**).
- **Infrasound.** With δρ/ρ₀ = δp/(γp₀) and a wave fully reflected at the ground, the gravity perturbation is
  [Harms Eq. (143), (171)](https://arxiv.org/abs/1507.05850) **[read]**; for a detector at depth h,
  [Junca et al. 2019 Eq. (23)–(25)](https://arxiv.org/abs/1902.05337) **[read]** give
  a ∝ (4πGρ₀/(γp₀k)) δp e^{−hk sinθ}, so burial suppresses the near-horizontal waves exponentially.
  In gradient form at the surface: 3.6×10⁻¹⁵ s⁻² per Pa, 8.5×10⁻⁵ rad per Pa [derived, agrees with S14].
  MIGA measured the pressure spectrum on site with a microbarometer (1.75 mPa rms resolution, 0.02–4 Hz),
  compared it with Bowman's low/mid/high noise models, and found infrasound Newtonian noise rising as
  f^−2.2 below 0.1 Hz and detectable by the improved MIGA in windy weather
  ([Junca §IV, Figs. 7–9](https://arxiv.org/abs/1902.05337)). Fiorucci et al. add the effect of the hosting
  building and of depth ([abstract](https://arxiv.org/abs/1801.04564) **[snippet]**; the text says the field
  outside and inside the building differ and models a 6 m hemispherical building **[read, in part]**).
  Harms warns that sound transmitted into the ground may *raise* the gravity perturbation (transmissivity
  2α₁/α₂ can exceed 1 at soft sites) and that the model is unproven below a few Hz
  ([Harms §5.1, §5.6](https://arxiv.org/abs/1507.05850)).
- **Advected temperature in the air.** Creighton's model of quasi-static temperature fields carried past a
  test mass by wind (δρ = −ρ₀δT/T₀, Kolmogorov structure function, Taylor's hypothesis) is reviewed with its
  spectrum in [Harms §5.2, Eq. (145)–(155), (173)](https://arxiv.org/abs/1507.05850) **[read]**; it falls
  exponentially with the distance to the nearest streamline, which a building sets. Creighton's own paper
  ("Tumbleweeds and airborne gravitational noise sources for LIGO", CQG 25, 125011) I did not read; its title
  is confirmed by Harms's reference [49].
- **Temperature in the room or shaft.** S13 gives 2.2×10⁻² rad per K for a 5 m × 6 m hall and 2.2×10⁻³ for a
  1 m shaft. MAGIS's site survey found the underground shaft stable and the surface building following the
  weather daily, and set targets of ±0.3 °C for the laser room and ±1 °C for enclosures
  ([Mitchell et al. 2022 §2, §6.1](https://arxiv.org/abs/2202.04763) **[read]**). At 2.2×10⁻² rad/K, ±0.3 °C in
  a hall is ~7 mrad of slow drift: a real, slow signal.

### 2.2 Hydrology and loading

- **Near water dominates.** The disc formula (S14 §2.3) makes a rain contrast 1/a: 10 mm on a roof of radius
  20–50 m, 10 m above the upper IFO, gives 1.8–3.1 rad across the 5 m baseline [derived; geometry
  illustrative, the Beecroft roof is not in hand]. Superconducting-gravimeter work at Wettzell attributes
  52–80 % of the local hydrological gravity signal to water within 50 m of the instrument
  ([Creutzfeldt et al. 2010, search summary](https://doi.org/10.1029/2009WR008359) [snippet]). So a roof,
  gutters, a lawn and a drain are scene objects, and rain is a source with mass, which S10 (water) can carry.
- **Loading.** Ocean and atmospheric loading move the tower through the ambient gradient, 1.45×10⁻¹² s⁻² per
  metre (S14): 1 cm is 3.5×10⁻⁴ rad. The load Green's functions are Farrell's and Guo et al.'s
  ([Farrell 1972](https://doi.org/10.1029/RG010i003p00761), [Guo et al. 2004](https://doi.org/10.1111/j.1365-246X.2004.02410.x), both cited by Harms; not read).

### 2.3 Tides and rotation

S14 sized these and S15 verified the phase terms; nothing here overturns them. Two additions from the
papers read: the rotation-compensating mirror for MAGIS has 1.3 mrad of range on three in-vacuum piezos,
measured from below by a position-sensitive detector ([MAGIS-100 Fig. 17](https://arxiv.org/abs/2104.02835)),
and AION-10's design realises compensation with a tunable effective pivot from the steering mirrors and the
bottom retroreflector ([TDR §3.6](https://arxiv.org/abs/2508.03491)). At Oxford the mirror must turn at
Ω cos φ = 4.5×10⁻⁵ rad/s, 66 µrad over 2T [derived]. With compensation, MAGIS's leading residual for a
three-loop sequence is (17/3 + 4√2) n k δv_x Ω_y T_zz T⁴; for AION-10's numbers that is 4×10⁻⁹ rad per nm/s
[derived from MAGIS §5.6]: negligible.

### 2.4 Anthropogenic

- **AION-10's own site.** Carlton & McCabe model a passenger lift (1000–2000 kg, 2.5 m/s, 20 moves an hour at
  peak), people on the stairs (5 an hour, 50–100 kg, 0.5 m/s up the stairs), 100 people an hour in the foyer,
  60 vehicles an hour (1–5 t, 10 m/s, 20 m away) and rodents, through the phase
  δφ = (m_A/ħ)∫[V(z₂,t) − V(z₁,t)]dt with V = −GM/√(D² + (z − h)²): the same form as `forward.rs`
  ([§III, Eq. (11)–(12)](https://arxiv.org/abs/2308.10731)). They show the semiclassical treatment is
  adequate (leading correction ~10⁻¹⁶ g for 80 kg at 2 m and a 1 mm cloud), that the daytime series is
  dominated by these transients, and that masking them restores ULDM sensitivity to within 10–40 % of
  shot-noise-limited; night running alone costs ~30 % ([abstract, §IV](https://arxiv.org/abs/2308.10731)).
  They ignore Coriolis (assumed compensated) and the Earth-gradient constant (noting sequences can remove it).
- **Gravitational-wave detectors.** Thorne & Winstein: walking people produce gravity-gradient noise through
  their centre-of-mass jerks; doors slamming and fists striking walls (5 kg m/s) are comparable; vehicles
  decelerating to rest dominate the vehicle term; people must be kept ≳10 m from LIGO test masses
  ([Thorne & Winstein 1999](https://arxiv.org/abs/gr-qc/9810016) **[read, abstract and §I]**). Those are
  10 Hz effects; at AION-10's 0.01–0.25 Hz the quasi-static mass term dominates. Harms gives the Fourier
  transform of a mass in uniform motion, a(ω) ∝ (2Gm/v²)[K₁(ωr/v) … + iK₀(ωr/v) …], exponentially cut off above
  v/r, and recommends a speed limit below 10 m/s for 0.1 Hz sensitivity
  ([Harms §6.2, Eq. (176)–(181)](https://arxiv.org/abs/1507.05850) **[read]**); and a rule of thumb that an
  oscillating object matters only if V/r³ is not small ([§6.1](https://arxiv.org/abs/1507.05850)).
- **MAGIS-100 site.** Higher-frequency seismic content from air handlers, compressors and a lift; human noise
  falls at night ([Mitchell et al. 2022 §4](https://arxiv.org/abs/2202.04763) **[read]**). No gravitational
  estimate of people or vehicles there.
- **A field atom gradiometer.** Birmingham's 1 m Rb gradiometer measured a lab mass change of 205 ± 13.1 E
  against 202 E modelled, and found a tunnel of 150 E modelled peak (17.5 mrad); it did not quantify people or
  vehicles ([Stray et al. 2022](https://doi.org/10.1038/s41586-021-04315-3), via
  [PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC8866129/) [snippet of full text]). A ready
  "known mass, measured phase" case for a Raman instrument.
- **ELGAR and ZAIGA.** Listed by MAGIS as having done similar budgets
  ([MAGIS §5 intro](https://arxiv.org/abs/2104.02835)); I did not read them (see the DOI list).

---

## 3. The state-dependent shift channel (Zeeman, blackbody, AC Stark, scalar dark matter)

**Mechanism.** On the 698 nm clock line each arm alternates between ¹S₀ and ³P₀. A shift δν(x, t) of the
transition adds phase 2π∫δν dt to whichever arm is excited. In the engine's sequence the upper (kicked)
arm is excited on [0, T] and the lower on [T, 2T], so per IFO

    Φ = 2π [ ∫₀^T δν(z_u, t) dt − ∫_T^{2T} δν(z_l, t) dt ].

Consequences [derived, `sizes.py`]:
- uniform and constant: Φ = 0 exactly (an echo);
- uniform drift ν̇: Φ = −2π ν̇ T² (3.35 rad per Hz/s), common to both IFOs if the drift is common;
- uniform noise at frequency f: amplitude 2π δν (4/ω) sin²(ωT/2) (2.07 rad per Hz at 0.1 Hz);
- static linear in height ν′: Φ = 2π ν′ A with A = ∫₀^T z_u − ∫_T^{2T} z_l = 1.76 m s, the same for both IFOs,
  so it **cancels in the gradiometer**; the gradiometer reads the curvature, 2π ν″ Δr A = 55 rad per Hz/m².

So each IFO reads the gradient of the shift and the gradiometer its curvature, exactly as for gravity. The
scalar-ULDM phase of Carlton & McCabe's Eq. (7) is this channel with δν ∝ cos(mt), which gives a ready
closed form for testing it.

**Numbers.** Magnetic: the stretched 9/2 → 9/2 line shifts 489 Hz/G, with a bias up to 10 G, 5 mG
homogeneity and a 1 µG/√Hz noise requirement ([TDR §2.3](https://arxiv.org/abs/2508.03491)); MAGIS
co-magnetometers with opposite-m_F interferometers, leaving −0.23 Hz/G²
([MAGIS §5.8](https://arxiv.org/abs/2104.02835), citing [Falke et al. 2011](https://doi.org/10.1088/0026-1394/48/5/022)).
Rows 4, 16, 21 of the table. Blackbody: −2.3 Hz (T/300 K)⁴ ([MAGIS §5.9](https://arxiv.org/abs/2104.02835)),
0.031 Hz/K; rows 8 and 22. A blackbody *force* on atoms also exists
([Haslinger et al. 2018](https://doi.org/10.1038/s41567-017-0004-9), cited by MAGIS; not read).

**The S13 estimate, corrected.** S13 §6 item 9 guessed ~0.1 rad per kelvin of temperature *difference*
between the IFOs. A static difference that is linear in height cancels in the gradiometer; a static
difference with curvature (1 K parabolic over 5 m) gives 0.14 rad, static; a difference in *drift* of
0.1 K/h gives 3×10⁻⁶ rad. So S13's temperature field matters to the blackbody channel only through its
curvature along the tower and its in-band fluctuation, both small in the real sequence. The gravity of the
air (2.2×10⁻² rad/K, S13) remains the larger heat effect for time-varying work.

---

## 4. Instrument systematics, with their published scalings

All from [MAGIS-100 §5](https://arxiv.org/abs/2104.02835) **[read; prefactors re-read in a raw text
extraction]** unless stated; each scales linearly with n, quoted at their n = 100 reference and moved here
to n = 1000.

| Term | MAGIS scaling (n = 100) | At n = 1000, MAGIS reference parameters | What sets it |
|---|---|---|---|
| Laser frequency noise | ~1×10⁻¹³ rad/√Hz (Δv/100 µm/s)(δf/10 Hz/√Hz)(τ/100 µs) | ~10⁻¹³ (Δv = 11 µm/s) | single-photon common mode ([Graham et al. 2013](https://doi.org/10.1103/PhysRevLett.110.171102), Table 1 term 3) |
| Wavefront × kinematic jitter, 1st order | 5×10⁻⁴ rad/√Hz (ε/5 mrad)(δx/10 µm/√Hz)(sin…/0.3) | 5×10⁻³ | aberration amplitude ε at 1/k_t ≈ 3 mm; per-shot position knowledge |
| Wavefront, 2nd order | 3×10⁻⁸ rad/√Hz, ∝ n² | 3×10⁻⁶ | transverse kicks |
| Mirror vibration | 1×10⁻⁸ rad/√Hz (Δv)(T)(δa/10⁻⁴ m s⁻² /√Hz) | 8×10⁻⁹ | velocity mismatch of the clouds |
| Pointing × wavefront | 6×10⁻⁴ rad/√Hz (δθ/30 nrad/√Hz) | 6×10⁻³ | AION-10 holds pointing to < 35 nrad ([TDR §3.5](https://arxiv.org/abs/2508.03491)) |
| AC Stark | 6×10⁻⁴ rad/√Hz (δI/I / 10⁻⁵/√Hz) | 6×10⁻³ | differential intensity between the IFOs |
| Rotation × gradient × velocity (3-loop) | 2×10⁻⁵ rad/√Hz (δv_x/10 µm/s/√Hz) | — | 4×10⁻⁹ per nm/s at AION's T [derived] |
| Mean field (⁸⁷Sr, ~10⁻¹¹ Hz cm³) | 5×10⁻⁵ rad/√Hz (τ/200 µs)(δN/N / 0.01) | 5×10⁻⁴ | population asymmetry |
| Quadratic Zeeman | 1×10⁻³ rad/√Hz (B₀/1 G)(δB/1 mG/√Hz)(T/1 s) | not ∝ n | bias current stability |
| Blackbody | 1×10⁻⁶ rad/√Hz (1 K, 1 h, T = 1 s, 3-loop) | not ∝ n | tube temperature |
| Timing jitter | 1×10⁻⁴ rad/√Hz (Δv/100 µm/s)(δT/1 ns/√Hz) | 1.1×10⁻⁴ per ns/√Hz | pulse generator |
| Gas index of refraction | strain-equivalent 10⁻²⁶–10⁻²¹/√Hz at 10⁻¹¹ Torr | negligible | vacuum |
| Cloud's own index of refraction | ~10⁻⁷ rad/√Hz | ~10⁻⁶ | atom number and size jitter |

**Gravity gradient and its compensation.** Per IFO, δφ = k_effᵀ(g + a)T² + k_effᵀΓT²(r₀ + v₀T) + …
([Roura Eq. 4](https://arxiv.org/abs/1509.08098)); the two clouds' launch jitters are independent, so the
gradiometer sees k_eff Γ T² times their *difference*: 1.48×10⁻⁵ rad per nm and 1.08×10⁻⁵ rad per nm/s
[derived]. Changing the middle pulse's effective wave number by Δk_eff = (ΓT²/2)k_eff closes the
interferometer, removes the dependence on r₀ and v₀, and moves it onto the mirror position at the middle
pulse, −2k_effᵀΔ·r_mirror·T² (Eq. 11–13), which cancels between the two IFOs of a single-laser gradiometer.
For AION-10, Δk/k = 8.2×10⁻⁷, a 353 MHz laser frequency step, and the kick at T changes by 5.4 µm/s
[derived]. Residual: the error in the assumed Γ (Roura's "relative accuracy δΓ/Γ ~ 10⁻³ relaxes co-location by
three orders"). The compensation also cancels Earth's 7.4×10⁴ rad constant (row 1), which is why
Carlton & McCabe and Cavendish can leave it out. The finite-pulse and gradient-induced Doppler detuning
(2π × 200 Hz for MAGIS's 100 m, ~0.5 % pulse inefficiency) is in [MAGIS §5.6](https://arxiv.org/abs/2104.02835).

**Finite kick duration.** The engine kicks instantly. A real LMT train of N pulses lasts N·τ_π; with a ramp of
duration τ at the start of each half the arm-separation integral falls by ≈ τ/T: 1.4×10⁻³ at 1 ms,
1.4×10⁻² at 10 ms, 6.7×10⁻² at 50 ms [derived]. That is a relative error on *every* moving-mass signal, far
above the engine's 10⁻⁹ oracle tie. MAGIS says the interaction time "can become a substantial fraction of a
second" ([§5.2](https://arxiv.org/abs/2104.02835)). AION-10's pulse durations are not published (§8).

---

## 5. What Cavendish models, and what to add

| Effect | Today | Add as | Why this form |
|---|---|---|---|
| Moving masses (people, lift, cars, animals) | **source, core** | an oracle case on Carlton & McCabe | it is the same integral; the paper is the published anchor for this site |
| Weather, infrasound, Lamb wave | planned S14 | far-field source (E5a) | closed form; data-driven later |
| Air temperature in the room | planned S8/S13 | scene source | S13 |
| Rain, roof water, soil moisture near the tower | not planned as such | **scene source** (S10 water: a sheet with a fill level) | 1/a: near water dominates (2–3 rad) |
| Tides, ocean loading, rotation | planned S14 | far-field source; Coriolis optional in the arms | S14 |
| Earth's gradient in the flight; Roura Δk at T | not planned explicitly | **instrument options** in `Instrument` (`earth_gradient`, `gradient_compensation`) | changes signals by ~10⁻⁶ relative; validates against Roura |
| Finite kick duration | no | **instrument parameter** `kick_duration` (ramp) | relative error τ/T on every signal |
| Clock shifts: Zeeman, blackbody, AC Stark, scalar ULDM | no | **state-dependent channel**: a shift field δν(x, t) read on the excited arm, with an occupancy pattern | one integral, four physical sources; ULDM is S16's |
| Kinematic jitter × residual Γ, residual Ω; wavefront; pointing; AC Stark intensity noise; mean field; timing | no | **noise models** on the readout, each with its coupling and a residual parameter | statistically exact from closed forms; no physics to simulate |
| Laser frequency noise, mirror vibration, gas index | no | **null tests** with the number recorded | ≤ 10⁻⁸ rad/√Hz |

**The noise-model layer, concretely.** For each tower and shot, add a seeded draw from each enabled
component: white shot noise σ = ASD/√(2Δt); k_eff ΔΓ T²(δz₁ − δz₂ + (δv₁ − δv₂)T) with ΔΓ the compensation
residual and δz, δv drawn per cloud; 2k_eff ΔΩ_⊥ (δv_x,1 − δv_x,2) T² with ΔΩ_⊥ the tip-tilt residual; and the
MAGIS-scaled ASD terms as coloured noise with the instrument's transfer function. Each component is a named
channel in the bundle (as `signal = targets + ambient` already is), so Gradar can switch them and a reader
sees what each costs.

### 5.1 Validation cases (pass criterion first; each has a control that must fail)

| Case | Asserts | Against | Control |
|---|---|---|---|
| `carlton_fig3_replay` | the engine's phase for the lift, stairs, foyer, car and RAT trajectories of Carlton Fig. 3 | their figure's extremes, within 10 % (read off the plot) | swapping the IFO sign flips every curve |
| `carlton_eq13_far_field` | stationary point mass, D, h ≫ arm excursion | Eq. (13): −k_eff T² G M Δr (D²−2h²)/(D²+h²)^{5/2}, ≤ 10⁻³ relative at D = 200 m | the sign change at D ≈ √2 h must appear |
| `clock_shift_uniform_is_silent` | uniform constant δν gives 0 per IFO | exact (≤ 10⁻¹² rad) | the shift applied to both arms (wrong occupancy) gives 2π δν T |
| `clock_shift_gradient_cancels` | linear static δν: per IFO 2π ν′A, gradiometer 0 | A from the closed-form arms | quadratic δν gives 2π ν″ Δr A |
| `clock_shift_drift` | uniform ν̇: −2π ν̇ T² | closed form | |
| `scalar_uldm_closed_form` | the channel with δν ∝ cos(ωt) | Carlton Eq. (7) for their broadband sequence (once the occupancy pattern is that sequence's) | wrong ω fails |
| `roura_compensation` | with Δk = (ΓT²/2)k at T, the phase no longer depends on z₀, v₀ (to O(Γ²)); without it, k Γ T²(δz + δvT) | [Roura Eq. 4, 11–13](https://arxiv.org/abs/1509.08098); s15's 7/12 check | Δk = 0 restores the dependence |
| `kick_ramp_limit` | the separation integral with a ramp of τ; τ → 0 bit-identical to today | closed form of the ramp | |
| `noise_channels_periodogram` | each noise channel's periodogram equals its ASD × transfer function | the engine's own `spectrum.rs` estimators | doubling an ASD fails |
| `infrasound_depth` | surface and buried response of a plane acoustic wave | Harms Eq. (143); Junca Eq. (23)–(25) | k → 0 gives 0 |
| `stray_lab_mass` (optional) | a Raman 1 m gradiometer with Stray et al.'s lab mass change | 202 E modelled / 205 ± 13 E measured | |

---

## 6. Recommendation, with reasons

1. **Validate the core against Carlton & McCabe first.** It is the only published simulation of AION-10's
   own site with the same physics, it covers every anthropogenic class asked about, and it costs one
   oracle case. (Prefer the case that checks a known answer.)
2. **Add the noise-model layer before adding more physics.** Most instrumental terms are linear couplings of
   a jitter to a known coefficient; drawing them per shot is statistically exact, cheap, and validated by
   periodogram. This is what a training set for Gradar needs to be realistic; simulating wavefronts or
   magnetic fields from first principles would add cost without a published answer to check against.
3. **Build the state-dependent channel once.** It serves the magnetic and blackbody budgets (S15), S13's
   temperature field, and S16's scalar dark matter, all with closed forms (§5.1).
4. **Give `Instrument` three options**: Earth's gradient in the flight, the Roura Δk at T, and a kick
   duration. Each defaults off, so the v1 tie and every existing fixture stay bit-identical.
5. **Leave seismic to the other note; keep S14's far field as it is; put rain and roof water in the scene.**

---

## 7. What I could not verify

1. **AION-10's actual sequence**: which arm is excited when during the n = 1000 LMT train, the π-pulse
   duration and total kick time, the cloud temperature and size, the bias field in use, and whether a
   co-magnetometer is planned. The TDR states requirements, not these. My clock-shift and kick-duration
   numbers use the engine's single-kick sequence and are upper bounds; the 1 µG/√Hz requirement giving
   10× the phase goal in my model suggests the real sequence suppresses excited-state dwell. **Ask the AION
   team (via George).**
2. **MAGIS scaling prefactors**: the PDF's equations extract badly. I re-read each prefactor in a second
   extraction (5×10⁻⁴, 3×10⁻⁸, 1×10⁻⁸, 6×10⁻⁴, 6×10⁻⁴, 2×10⁻⁵, 5×10⁻⁵, 1×10⁻³, 1×10⁻⁶, 1×10⁻⁴, 10⁻¹³), but the
   full parameter dependence of the wavefront terms (the k_t and sin factors) I could not read cleanly.
   Check against the published version (Quantum Sci. Technol. 6, 044003).
3. **Wavefront curvature × temperature** (row 3) uses the two-photon Raman form (Dickerson term 6) with k_eff;
   for single-photon LMT with pulses from alternating directions the coefficient may differ. Order of
   magnitude only.
4. **Rain on a roof**: the Beecroft roof's size and height above the tower are not in hand; the 2–3 rad is
   illustrative. Creutzfeldt's 52–80 %-within-50 m is from a search summary, not the paper.
5. **The −0.3 µGal/hPa admittance** is from a search summary of Merriam; I did not read Merriam or Boy et al.
6. **Carlton Fig. 3 magnitudes** are read off a plot (person on stairs: −6 to +3 rad on the axis; lift
   ~ ±0.75–1.5 rad; foyer ~0.2; car ~ −0.05 to −0.1). Eq. (13) at far-field positions gives 0.09 and −0.03 rad,
   the same order; the near-field values need the integral.
7. **Blackbody coefficient**: −2.3 Hz at 300 K is MAGIS's quoted figure (citing Falke et al.); I did not read
   the Sr clock papers.
8. **Thorne & Winstein** read in abstract and introduction only; Fiorucci et al. read in part.
9. **ELGAR, ZAIGA, MIGA's systematics paper, Creighton, D'Amico, Lan, Graham**: not read (DOIs below).

---

## 8. Further questions (not chased)

- What does the AION collaboration's own systematics budget (if any beyond the TDR) assign to magnetic,
  wavefront and blackbody terms at n = 1000?
- Does AION-10 plan gravity-gradient compensation (Roura/D'Amico) or multi-loop sequences?
- Are the Beecroft stairs to be closed during runs, as Carlton & McCabe assume at night?
- How large is the sound-to-ground transmission term Harms flags, at a building on clay (Oxford)?
- Can S13's temperature field and S8's air give a *joint* blackbody-plus-air-gravity budget for the tower
  (the same temperature drives both, with opposite structure: air reads the room, blackbody reads the tube)?

---

## References read

- AION Collaboration, *AION-10: Technical Design Report*, [arXiv:2508.03491](https://arxiv.org/abs/2508.03491) — §2.2, 2.3, 3.5, 3.6, 4.3.
- Badurina et al., *AION: An Atom Interferometer Observatory and Network*, [arXiv:1911.11755](https://arxiv.org/abs/1911.11755) — Table 1.
- Abe et al. (MAGIS-100), *Matter-wave Atomic Gradiometer Interferometric Sensor*, [arXiv:2104.02835](https://arxiv.org/abs/2104.02835) — §4, §5.
- Mitchell, Kovachy, Hahn, Adamson, Chattopadhyay, *MAGIS-100 environmental characterization and noise analysis*, JINST 17 P01007 (2022), [arXiv:2202.04763](https://arxiv.org/abs/2202.04763).
- Roura, *Circumventing Heisenberg's uncertainty principle in atom interferometry tests of the equivalence principle*, PRL 118, 160401 (2017), [arXiv:1509.08098](https://arxiv.org/abs/1509.08098) — Eq. 4, 11–14.
- Carlton & McCabe, *From RATs to riches: mitigating anthropogenic and synanthropic noise in atom interferometer searches for ultra-light dark matter*, [arXiv:2308.10731](https://arxiv.org/abs/2308.10731).
- Junca et al., *Characterizing Earth gravity field fluctuations with the MIGA antenna*, PRD 99, 104026 (2019), [arXiv:1902.05337](https://arxiv.org/abs/1902.05337) — §IV, V.
- Harms, *Terrestrial gravity fluctuations*, Living Rev. Relativ. 18, 3 (2015), [arXiv:1507.05850](https://arxiv.org/abs/1507.05850) — §5, §6.
- Fiorucci, Harms, Barsuglia, Fiori, Paoletti, PRD 97, 062003 (2018), [arXiv:1801.04564](https://arxiv.org/abs/1801.04564) — in part.
- Thorne & Winstein, PRD 60, 082001 (1999), [arXiv:gr-qc/9810016](https://arxiv.org/abs/gr-qc/9810016) — abstract and §I.
- Stray et al., Nature 602, 590 (2022), [doi:10.1038/s41586-021-04315-3](https://doi.org/10.1038/s41586-021-04315-3) — via PMC, summary of full text.
- This repository: `crates/cav-core/src/forward.rs`; `plan/REBUILD.md` §9, §9.3; `plan/PROGRESS.md` (For George, rulings);
  `docs/research/s13-heat.md` §5–6, `s14-wider-world.md`, `s15-instruments-anywhere.md`.

## Papers I could not read

DOIs are from the reference lists of the papers read (MAGIS-100, Harms) or from search results, except
where marked "from memory".

- Creighton, *Tumbleweeds and airborne gravitational noise sources for LIGO*: https://doi.org/10.1088/0264-9381/25/12/125011
- Merriam, *Atmospheric pressure and gravity*, GJI 109, 488 (1992; DOI from a search summary): https://doi.org/10.1111/j.1365-246X.1992.tb00112.x
- Creutzfeldt et al., local water storage and gravity at Wettzell: https://doi.org/10.1029/2009WR008359
- Creutzfeldt et al., hydrological gravity modelling (2008): https://doi.org/10.1190/1.2992508
- Van Camp et al., *Geophysics from terrestrial time-variable gravity measurements*: https://doi.org/10.1002/2017RG000566
- Farrell, *Deformation of the Earth by surface loads*: https://doi.org/10.1029/RG010i003p00761
- D'Amico et al., *Canceling the gravity gradient phase shift in atom interferometry*: https://doi.org/10.1103/PhysRevLett.119.253201
- Lan et al., *Influence of the Coriolis force in atom interferometry*, PRL 108, 090402 (DOI from memory; arXiv:1110.6910): https://doi.org/10.1103/PhysRevLett.108.090402
- Graham, Hogan, Kasevich, Rajendran, *New method for gravitational wave detection with atomic sensors*: https://doi.org/10.1103/PhysRevLett.110.171102
- Falke et al., *The ⁸⁷Sr optical frequency standard at PTB* (Zeeman and blackbody coefficients): https://doi.org/10.1088/0026-1394/48/5/022
- Haslinger et al., *Attractive force on atoms due to blackbody radiation*: https://doi.org/10.1038/s41567-017-0004-9
- Dubetsky & Kasevich, *Atom interferometer as a selective sensor of rotation or gravity* (multi-loop): https://doi.org/10.1103/PhysRevA.74.023615
- Dickerson et al., *Multiaxis inertial sensing with long-time point source atom interferometry* (DOI from memory; the S15 note read arXiv:1305.1700): https://doi.org/10.1103/PhysRevLett.111.083001
- Swallows et al., ⁸⁷Sr mean-field shift: https://doi.org/10.1109/TUFFC.2012.2210
- Guo et al., GJI 159, 53 (2004; DOI from a citing paper): https://doi.org/10.1111/j.1365-246X.2004.02410.x
- ELGAR (Canuel et al., CQG 37, 225017, 2020; also arXiv:1911.03701; DOI from memory, volume and article number confirmed by search): https://doi.org/10.1088/1361-6382/aba80e
- ZAIGA (Zhan et al., IJMPD 29, 2050047, 2020; DOI from memory, not confirmed): https://doi.org/10.1142/S0218271820500479
