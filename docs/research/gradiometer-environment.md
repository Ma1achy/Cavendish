# What else an AION-10 gradiometer sees: environment and instrument systematics, and what Cavendish should simulate

> **Updated 27 Nov 2026 from Saulson 1984** (PRD 30, 732, whole paper, equations from the page images). No
> row or recommendation moves: no number in the table came from Saulson. Two things change in §2's text: the
> ZAIGA line said ZAIGA used "Saulson's coherence-cell model … and 20 m/s wind"; Saulson's cells are sized by
> the *acoustic* wavelength and contain no wind, and ZAIGA's pressure spectrum is a tenth of Saulson's
> low-frequency one (§2, ELGAR and ZAIGA). And the infrasound bullet now says where Saulson's model applies to
> AION-10, which is nowhere directly: it is a short-wavelength (λ ≪ arm) model, and AION-10 is deep in the
> long-wavelength regime (§2, Infrasound).

> **Updated 28 Nov 2026 from five more primary sources** (owner's PDFs, not committed): Fiorucci et al. 2018
> (whole, was abstract only), Junca et al. 2019 (arXiv v1 re-read), Boy, Gegout & Hinderer 2002, Boy & Chao
> 2005, and Canuel et al. 2020 *Technologies for the ELGAR large scale atom interferometer array*
> (arXiv:2007.04014), which **is** ELGAR's companion ref. [56] (the ELGAR CQG paper's reference list names it).
> No row moves. What changes: (i) Fiorucci's sub-Hz case (TOBA, a 10 m gradient sensor) is the one that maps
> onto AION-10: a building or cavity adds nothing below 1 Hz, and depth suppresses infrasound noise *less* at
> low frequency (§2, Infrasound); (ii) ELGAR's blackbody acceleration requirement is derived from Haslinger's
> potential, the same form as row 8a, which closes §8's question (§2, ELGAR); (iii) Boy 2002 gives a measured
> admittance (−2.71 nm s⁻²/hPa, Strasbourg) and the Bouguer value −4.27, both consistent with §2.1, and both
> Boy papers work at periods ≥ 12 h, far below AION-10's band (§2.1). Junca 2019 is a Newtonian-noise paper, not
> the MIGA systematics paper the note wanted; that remains unidentified (§7 item 9).

> **Updated 28 Nov 2026 (third batch) from three more primary sources** (owner-supplied PDFs, not
> committed): Neumeyer, Hagedoorn, Leitloff & Schmidt 2004 (*J. Geodyn.* 38, 437–450, whole paper), Abe,
> Kroner, Neumeyer & Chen, *Assessment of atmospheric reductions for terrestrial gravity observations*
> (Bull. Inf. Marées Terrestres, whole paper), and Canuel et al. 2018, *Exploring gravity with the MIGA
> large scale atom interferometer* (*Sci. Rep.* 8, 14064, read as arXiv:1703.02490v3, whole paper). What
> changes: (i) **new row 13a**, the *surface-pressure-independent* (SPI) air-mass signal: air moving aloft
> at constant surface pressure changes surface gravity by ~2 µGal (Neumeyer §6, Fig. 7–8) to 4.5 µGal peak
> to peak from within 0.5° (Abe §3-1, Fig. 6a), which no barometer-driven model (row 13) can see. Carried to
> the gradiometer by [`spi.py`](gradiometer-environment/spi.py) it is 3×10⁻³–3×10⁻² rad (D) depending on its
> unpublished horizontal scale, up to ~3× row 13; the pass criterion written first (≤ row 13) was **not
> met**, so it is a row, not a remark (§2.1). (ii) The zone that matters: 3-D air data out to 5° and 2-D
> beyond (Abe §3-1, §4); the attraction Green's function changes sign near 1.1° (Neumeyer §3). (iii) The
> MIGA paper is an instrument and site paper, **not** a systematics budget: it gives a Coriolis requirement
> (1 µrad launch-direction stability for 1 mrad, Eq. (20)), detection noise (1 mrad/√Hz at 10⁶ atoms/s,
> §3.2) and LSBB's site numbers (§5.1), but no environmental error budget; the MIGA line in §7 item 9 is
> closed as "read, not what was wanted" (§2.4). No recommendation changes.

> **Changed after reading the primary sources (25 Nov 2026).** The owner fetched the sixteen papers this note
> could not read, and ZAIGA was read from arXiv. Every [snippet], second-hand or from-memory claim below
> has been checked against them and now cites page, section, table or equation. What moved:
> - **New row 9a, wind-advected outdoor temperature** (Creighton 2008 §3, Eq. (8), carried to the
>   gradiometer by `sizes.py`): 4×10⁻³–6×10⁻² rad/√Hz at 0.1 Hz for wind of 5–10 m/s and a tower 5–20 m
>   inside a building, 40–600× the goal (1×10⁻⁴–7×10⁻³ at 0.25 Hz). Creighton and Harms both warn the model is unvalidated below a few Hz, so this is
>   an *unvalidated order of magnitude*, not a prediction; it is the largest in-band environmental term in
>   the table and needs a site measurement (§7 item 10). Recommendation 5 changes accordingly.
> - **New row 8a, the blackbody *force*** (Haslinger et al. 2018): the ground-state Stark potential acts
>   on both arms whatever their state, so unlike row 8 it is not reduced by keeping the arms in the
>   ground state: 0.20 rad static for the same 1 K parabolic profile. ELGAR budgets blackbody
>   radiation as an *acceleration* noise for the same reason (Canuel 2020, Table 2 line 9).
>   Recommendation 3 gains one line: S13's temperature field also feeds the ordinary potential.
> - **Row 3, wavefront × temperature**: 0.05–5 → **0.02–2 rad**, now from Dickerson's printed Table I
>   term 6 (½ k_eff α (v_x²+v_y²) T²) instead of my own form, which was twice it.
> - **Blackbody coefficient**: −2.3 → −2.354(32) Hz (T/300 K)⁴ (Falke 2011 Eq. (1)) → **−2.279 Hz**
>   (T/300 K)⁴ from the polarizabilities of Safronova et al. 2013 (third pass, §3); dν/dT 0.0307 →
>   0.0314 → **0.0304 Hz/K**. Row 8: 0.14 → **0.13 rad** (0.1388 → 0.1344); row 22 unchanged at 3×10⁻⁶ rad
>   (2.92 → 2.83×10⁻⁶).
> - **Blackbody force, row 8a, no longer from memory.** α_g(Sr) = 197.14(20) a.u. and the 300 K field
>   are now from Safronova et al. 2013 and Porsev & Derevianko 2006 (§3): the static value is unchanged
>   (197.2 → 197.14 a.u.), the ground-state dynamic correction (−0.0028 Hz) is now included, and the row
>   stays at **0.20 rad** (0.1995 → 0.1997); dν_g/dT 0.0226 → 0.0227 Hz/K.
> - **Merriam's −0.356 µGal/hPa** is his *local-zone admittance* (0–0.5°), not a Table Mountain value
>   as the search summary said; Merriam's effective range is −0.27 to −0.43 µGal/hPa.
> - **DOIs**: the ZAIGA DOI from memory was wrong (it resolves to Quiros et al., an unrelated
>   scalar-tensor paper, IJMPD 29, 2050047); ZAIGA is IJMPD 29, 1940005,
>   [doi:10.1142/S0218271819400054](https://doi.org/10.1142/S0218271819400054). Every other DOI was
>   confirmed on the paper itself.
> - New validation cases from the papers (§5.1): D'Amico's zero crossing, Lan's compensating tilt rate,
>   Van Camp's 45 kg boy, Creutzfeldt's 1 m water slab, Farrell's Table A3.

> Research note, 25 Nov 2026. Scope: everything an atom-interferometer gravity gradiometer (AION-10, ⁸⁷Sr,
> 698 nm, n = 1000, T = 0.73 s, Δr = 5 m) reads that is *not* seismic ground motion or seismic Newtonian
> noise (another note covers those). It builds on `docs/research/s14-wider-world.md` (far field, tides,
> rotation), `docs/research/s15-instruments-anywhere.md` (rotation and gradient phase terms, verified there)
> and `docs/research/s13-heat.md` (air temperature), and does not repeat their derivations.
>
> **Sources.** Unlike the S14 note, this one was written with network access: the papers marked **[read]**
> below were downloaded from arXiv and read in the relevant sections (the text extraction garbles some
> printed prefactors; where I relied on one I re-read it in a second extraction or the page image and say
> so). Papers marked **[read, primary]** were read from the published PDFs fetched by the owner on
> 25 Nov 2026. **[derived]** means computed here. **[snippet]** (abstract or summary only) remains only on
> Fiorucci et al. and Stray et al., which were not in the fetched set (Fiorucci was read in full on 28 Nov;
> only Stray remains a snippet).
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
   [Mitchell et al. 2022 §6.1](https://arxiv.org/abs/2202.04763) **[read]**). The same thermal field also
   exerts a *force* on both arms through the ground state's Stark shift
   ([Haslinger et al. 2018, Eq. (2)](https://doi.org/10.1038/s41567-017-0004-9) **[read, primary]**),
   0.20 rad for the same profile **[derived]**, independent of which arm is excited: it is an ordinary
   potential, so the engine's existing integral carries it. The linear Zeeman shift of the
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
S13's temperature field, and S16's scalar dark matter), with an occupancy-pattern parameter, plus the
ground-state blackbody potential h ν_g(T(x)) added to the ordinary potential on both arms;
(4) optional **Earth's gradient, Coriolis and the Roura kick** in the arms (S14/S15 already ask for the
first two); (5) a **finite kick-duration** parameter, because an LMT train of 10 ms changes every signal
by ~1.4 % **[derived]**. Environment sources (weather, rain, ground water, tides) stay where S14 put them;
this note confirms their sizes and adds the rain-on-a-roof case, which at 2–3 rad for 10 mm of water
**[derived]** belongs in the scene, not the far field. **Changed 25 Nov:** wind-advected outdoor
temperature (Creighton's model, row 9a) may be the largest in-band environmental noise, 10⁻²-ish rad/√Hz
at 0.1 Hz, but the model is unvalidated in AION's band; carry it as a labelled noise channel with its
parameters exposed (c_T², wind speed, building size), not as physics, and ask for a site measurement.

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
| 1 | Earth's static vertical gradient | 7.4×10⁴ | S | no (by design) | C·Γ_E, Γ_E = 3.086×10⁻⁶ s⁻² | [derived]; removed in practice by [Roura Eq. 11](https://arxiv.org/abs/1509.08098), demonstrated in [D'Amico 2017, Eq. (1)–(3), Figs. 2–3](https://doi.org/10.1103/PhysRevLett.119.253201) [read, primary] |
| 2 | Person on the stairs beside the tower | ~1–6 | T | **yes (core)** | Carlton Eq. (11)–(12) numerically; Eq. (13) far field | [Carlton & McCabe Fig. 3](https://arxiv.org/abs/2308.10731) |
| 3 | Wavefront curvature × cloud temperature (1–10 nK, R = 1–10 km) | 0.02–2 (static); fluctuates with δT/T | S, N | no | ½ k_eff α (v_x²+v_y²) T², α = 1/(2R): k_eff σ_v² T²/(2R) | [Dickerson 2013, Table I term 6](https://doi.org/10.1103/PhysRevLett.111.083001) [read, primary]; [derived] |
| 4 | Linear Zeeman (489 Hz/G), 5 mG field inhomogeneity read as curvature over 5 m | ~5 | S | no | 2π A Δr c₁ B″, A = 1.76 m s | [derived]; [TDR §2.3](https://arxiv.org/abs/2508.03491) |
| 5 | Rain, 10 mm on a roof of radius 20–50 m, 10 m above the upper IFO | 1.8–3.1 | D, T | no (needs scene water) | disc: a_z = 2πGσ(1 − d/√(a²+d²)) | [derived]; near-field dominance: [Creutzfeldt et al. 2008, p. WA100, Fig. 5](https://doi.org/10.1190/1.2992508), [2010 ¶9, ¶41](https://doi.org/10.1029/2009WR008359) [read, primary] |
| 6 | Lift, 1000 kg at 10 m, moving the tower's height | ~1.5 | T | **yes (core)** | as #2 | [Carlton Fig. 3](https://arxiv.org/abs/2308.10731) |
| 7 | Coriolis, per µm/s of transverse velocity jitter, uncompensated | 0.43 (4.3×10⁻⁴ per nm/s) | N | no | 2 k_eff Ω cos φ v_⊥ T² | s14 §5.1, s15 (`docs/research/s14-wider-world.md`); [Lan 2012, p. 090402-4: Δφ = 2Ω·(v₀×k)T²](https://doi.org/10.1103/PhysRevLett.108.090402); [Dubetsky & Kasevich Table I row 2](https://doi.org/10.1103/PhysRevA.74.023615) [read, primary] |
| 8 | Blackbody shift, 1 K parabolic temperature along the tower | 0.13 | S | no | 2π A Δr (dν/dT) T″, dν/dT = 0.0304 Hz/K | [derived]; coefficient −2.279 Hz (T/300 K)⁴ = static −2.130 (Eq. (5), Δα = 247.379 a.u.) + dynamic −0.1492, [Safronova et al. 2013](https://doi.org/10.1103/PhysRevA.87.012509) Eq. (5), Tables IV, VI [read, primary]; Falke 2011 Eq. (1) and [Porsev & Derevianko 2006](https://doi.org/10.1103/PhysRevA.74.020502) Table I give −2.354(32), from the older Δα = 261(4) a.u. (§3) |
| 8a | Blackbody *force* (ground-state Stark potential on both arms), same 1 K parabolic profile | 0.20 | S | no (needs S13's field in the potential) | C (h/m) (dν_g/dT) T″, dν_g/dT = 0.0227 Hz/K | [derived]; force measured by [Haslinger 2018, Eq. (2), Fig. 2](https://doi.org/10.1038/s41567-017-0004-9) [read, primary]; α_g(Sr) = 197.14(20) a.u., [Safronova et al. 2013](https://doi.org/10.1103/PhysRevA.87.012509) Table IV [read, primary] |
| 9 | People in the foyer (100 kg, 5 m, 12 m up); cars on the road (1–5 t, 20 m) | 0.03–0.2 | T | **yes (core)** | Carlton Eq. (13): −k_eff T² G M Δr (D²−2h²)/(D²+h²)^{5/2} | [Carlton Fig. 3](https://arxiv.org/abs/2308.10731); Eq. (13) gives 0.09 and −0.03 rad [derived] |
| 9a | Wind-advected outdoor temperature, 0.1 Hz, wind 5–10 m/s, tower 5–20 m inside the building (c_T² = 0.2 K² m^−2/3) | 4×10⁻³–6×10⁻² rad/√Hz (**unvalidated model**) | N | no | Creighton Eq. (8) with the gradiometer kernel (`sizes.py`) | [Creighton 2008 §3, Eq. (8), (13)](https://doi.org/10.1088/0264-9381/25/12/125011) [read, primary]; validity warning: [Harms §5.6](https://arxiv.org/abs/1507.05850) |
| 10 | Ocean-tide attraction near a coast (upper bound) | ≤5×10⁻² | D | no | πGρ_w a/d; Farrell Green's functions | s14 §2.2 (`docs/research/s14-wider-world.md`); [Farrell 1972 Table A3](https://doi.org/10.1029/RG010i003p00761) [read, primary] |
| 11 | Air in a 5 m × 6 m hall, per K of mean temperature | 2.2×10⁻² | D | planned (S8, S13) | slab: 4πG δρ Δz | s13 §5.2 (`docs/research/s13-heat.md`) |
| 12 | Gravity gradient × launch jitter, per IFO: per µm height / per µm/s velocity | 1.5×10⁻² / 1.1×10⁻² | N | no | k_eff Γ T²(δz + δv T) | [Roura Eq. 4](https://arxiv.org/abs/1509.08098); 7/12 term verified in s15 (`docs/research/s15-instruments-anywhere.md`) |
| 13 | Pressure systems (10 hPa at 1000 km; 5 hPa at 300 km) | 6×10⁻³–9×10⁻³ | D | planned (S14) | 2πG(ρ₀/p₀)δp·kH/(1+kH) | s14 §2.3 (`docs/research/s14-wider-world.md`), recomputed [derived]; gravimeter admittance [Merriam 1992 p. 495](https://doi.org/10.1111/j.1365-246X.1992.tb00112.x) [read, primary] |
| 13a | Air moving aloft at constant surface pressure (SPI part), 2–2.25 µGal amplitude, horizontal scale 100–1000 km | 3×10⁻³–3×10⁻² | D | no (needs a 3-D air density field) | Γ_zz = k·δg for a lateral harmonic of wavenumber k (exact below any source above the sensor) | [derived] in `spi.py`; amplitude from [Neumeyer et al. 2004 §6, Fig. 7–8](https://doi.org/10.1016/j.jog.2004.07.006) and Abe et al. (BIM, §3-1, Fig. 6a) [read, primary]; the horizontal scale is **not** in either paper |
| 14 | AC Stark, wavefront × kinematics, pointing × wavefront, at MAGIS's reference parameters scaled to n = 1000 | 5–6×10⁻³ rad/√Hz each | N | no | MAGIS scalings (§4) | [MAGIS §5.2, 5.4, 5.5](https://arxiv.org/abs/2104.02835) |
| 15 | Solid-Earth tide (Moon + Sun, Love factor 0.97) | 3.6×10⁻³ | D | planned (S14) | (GM/d³)(3cos²ψ − 1)(1 + 6k₂ − 3h₂) | s14 §2.1 (`docs/research/s14-wider-world.md`) |
| 16 | Linear Zeeman noise, 1 µG/√Hz differential at 0.1 Hz, simple sequence | 1.0×10⁻³ rad/√Hz | N | no | 2π c₁ δB (4/ω) sin²(ωT/2) | [derived]; requirement from [TDR §2.3](https://arxiv.org/abs/2508.03491) |
| 17 | Mean-field shift (δN/N = 0.01), n = 1000 | 5×10⁻⁴ rad/√Hz | N | no | MAGIS scaling | [MAGIS §5.7](https://arxiv.org/abs/2104.02835); same order as the measured 10⁻¹⁵ shift at 10¹¹ cm⁻³ in [Swallows 2012 Fig. 5](https://doi.org/10.1109/TUFFC.2012.2210) [read, primary] |
| 18 | Timing jitter 1 ns/√Hz, with the clouds' 11 µm/s velocity difference from Γ | 1.1×10⁻⁴ rad/√Hz | N | no | MAGIS §5.10 scaling | [MAGIS §5.10](https://arxiv.org/abs/2104.02835); Δv [derived] |
| 19 | Infrasound, per pascal (adiabatic plane wave, k ≫ 1/H) | 8.5×10⁻⁵ per Pa | N | planned (S14) | 2πG δp/c²; underground: Junca Eq. (23)–(25) | [Harms §5.1](https://arxiv.org/abs/1507.05850); [Junca 2019 §IV](https://arxiv.org/abs/1902.05337) |
| 20 | Small animals near the base (5 × 0.25 kg) | ~5×10⁻⁵ | T | **yes (core)** | as #2 | [Carlton Fig. 3](https://arxiv.org/abs/2308.10731) |
| 21 | Quadratic Zeeman after co-magnetometry (−0.23 Hz/G², B₀ = 10 G, 1 µG/√Hz) | 1×10⁻⁵ rad/√Hz | N | no | 2π·2βB₀δB·(4/ω)sin²(ωT/2) | [MAGIS §5.8](https://arxiv.org/abs/2104.02835); coefficient −23.0(3) Hz/mT² = −0.230 Hz/G², [Falke 2011 §3.2, Fig. 4](https://doi.org/10.1088/0026-1394/48/5/022) [read, primary]; [derived] |
| 22 | Blackbody drift, 0.1 K/h *difference* between the IFOs; 1 K, 1 h oscillation | 3×10⁻⁶; 1×10⁻⁶ rad/√Hz | D, N | no | −2π ν̇ T²; MAGIS §5.9 | [derived]; [Mitchell 2022 §6.1](https://arxiv.org/abs/2202.04763) |
| 23 | Mirror vibration (10⁻⁴ m/s²/√Hz) with Δv = 11 µm/s | 8×10⁻⁹ rad/√Hz | N | no | MAGIS §5.3 | [MAGIS §5.3](https://arxiv.org/abs/2104.02835) |
| 24 | Laser frequency noise (single-photon gradiometer) | ~10⁻¹³ rad/√Hz | N | no | N Δv δk Δτ: MAGIS §5.1; [Graham 2013 Table I term 3](https://doi.org/10.1103/PhysRevLett.110.171102) [read, primary] | [MAGIS §5.1](https://arxiv.org/abs/2104.02835) |
| — | Finite kick (LMT train) duration τ: relative change of *every* signal | 1.4×10⁻³ (τ = 1 ms) … 6.7×10⁻² (50 ms) | — | no | separation integral with a ramp | [derived]; "a substantial fraction of a second" for large n: [MAGIS §5.2](https://arxiv.org/abs/2104.02835) |

Rows 8a and 9a were added on 25 Nov 2026 and row 13a on 28 Nov (third batch); the other numbers are kept so earlier references stay valid.
Read the table with three cautions. (a) Rows 3, 4, 8 and 16 use my single-kick model of which arm is excited
when; real LMT sequences keep both arms in the ground state for most of the flight (MAGIS §5.8), which
shrinks them by roughly the excited-state dwell fraction. (b) Rows 14 and 17 are *requirements in disguise*:
they are what MAGIS's reference parameters would give at n = 1000, not predictions for AION-10, whose
parameters are not published. (c) Static rows matter for a twin and for contrast, not for a search for
time-varying signals.

---

## 2. The environment (other than seismic)

### 2.1 Atmospheric mass

- **Pressure systems and the gravity admittance.** Gravimeters see ≈ −0.3 µGal/hPa: "about −0.3 µgal mbar⁻¹"
  in [Merriam 1992, Introduction, p. 488](https://doi.org/10.1111/j.1365-246X.1992.tb00112.x)
  **[read, primary]**, who reviews earlier values (Merriam 1981's fit −0.34 at Alice Springs; Warburton &
  Goodkind −0.43 for large cells to −0.35 for cells tens of km across; p. 489). His own Green's functions
  give a **local-zone admittance of −0.356 µGal/mbar** (0–0.5°, p. 495); in that zone loading is only
  ~1 % of the Newtonian attraction, about half the local signal comes from 1–10 km, and 90 % of a
  1000 km weather system's signal comes from within 50 km (p. 490). The effective (single-regression)
  admittance can lie anywhere in −0.27 to −0.43 µGal/mbar depending on the regional pressure field
  (p. 495). Van Camp et al. split it as −4 nm s⁻²/hPa attraction and +1 loading, net −3, worth ~30 nm s⁻²
  for a 10 hPa standard deviation, with a single admittance removing ~90 %
  ([Van Camp 2017, Appendix A1.3](https://doi.org/10.1002/2017RG000566) **[read, primary]**), and the
  correction falls to zero by 1–2 mHz (their discussion of station noise against Peterson's low-noise model). The Bouguer value
  2πGδp/g = 0.43 µGal/hPa [derived] is the attraction alone. **For a gradiometer the admittance is the wrong quantity**: a laterally
  uniform air layer has zero vertical gradient at the ground, and what is read is the lateral structure
  through 2πG(ρ₀/p₀)δp·kH/(1+kH) (S14's E5a). Recomputed here: 6.1×10⁻³ rad (10 hPa, λ = 1000 km) and
  9.1×10⁻³ rad (5 hPa, λ = 300 km). Gravimeters see atmospheric effects below ~1 mHz, modelled as pressure
  plus loading ([Harms §5](https://arxiv.org/abs/1507.05850) **[read]**).
  Boy, Gegout, Hinderer 2002 (GJI 149, 534–545; no DOI printed on the PDF) **[read, primary, 28 Nov]**
  agree: the direct attraction comes from regional pressure (~1000 km, about 90 % of the loading signal) and
  the elastic part from wavelengths > 4000 km (about 10 %) (§5.1, Fig. 7 discussion); the Bouguer plate term
  is −4.27 nm s⁻² hPa⁻¹ (§3.1, thin-layer Green's function); the fitted local admittance at Strasbourg is −2.71 ± 0.03 nm s⁻² hPa⁻¹
  (Fig. 7 caption), inside Merriam's range; and modelling the air within 0.25–0.5° (~50 km) with the local
  barometer is optimal (§5.1, Fig. 6). A thin-layer model does *not* beat the local admittance; a
  pseudo-stratified (hydrostatic, isothermal) atmosphere cuts residual scatter by ~20 % from 12 h to 2 yr (§5.1).
  All of it assumes hydrostatic equilibrium, "verified for periods larger than 12 hr" (§2), so none of it reaches AION-10's band.
  [Boy & Chao 2005](https://doi.org/10.1029/2002JB002333) (JGR 110, B08412) **[read, primary, 28 Nov]** show the
  3-D atmosphere differs from the thin layer above GRACE's sensitivity only to degree 15–20 (wavelengths
  > 2000–2500 km, ¶[1], ¶[29]): a satellite-geodesy correction, irrelevant to a 10 m baseline.
- **3-D air density, and the part a barometer cannot see** (added 28 Nov, third batch).
  [Neumeyer, Hagedoorn, Leitloff & Schmidt 2004](https://doi.org/10.1016/j.jog.2004.07.006) (*J. Geodyn.* 38,
  437–450) **[read, primary]** split the effect into attraction and elastic deformation (§1) and show the
  attraction cannot be modelled from surface pressure with a standard density profile: at one ground pressure
  (911.5 ± 0.5 hPa) the density profile differs by up to ~0.1 kg/m³ at a given height (§2, Fig. 1–2), and a
  test density change up to 16 km over 5 h moves gravity by ~3 µGal (§4.1, Fig. 5). Their attraction is a sum
  over spherical air segments from ECMWF 3-D fields (0.5° grid, 60 levels to 60 km, 6 h), as point masses
  (Eq. (3)) or by the exact potential of a spherical segment (Eq. (4)–(6)); the two agree within ±0.2 µGal on a
  ~1 km interpolated grid (§6). Density is the moist ideal gas ρ = p/(R_L T (1 − q + q/ε)) (Eq. (1)). The
  troposphere (~11 km) holds ~90 % of the air, 30 km ~99 % (§2). Merriam's attraction Green's function
  changes most within 0–0.5° (the local zone, where attraction dominates), attraction and deformation cancel
  at 1.1°, and beyond that deformation dominates (§3, Fig. 3); for deformation a 2-D grid to ~10° suffices
  (§3). At Vienna the 3-D attraction against a single admittance of −0.43 µGal/hPa leaves a **surface-pressure-
  independent (SPI) part of ~2 µGal maximum**, seasonal (air rises in summer, so attraction falls) with shorter
  excursions (§6, Fig. 7–8); the fitted local admittance there is −0.29 µGal/hPa and the deformation term was
  taken as +0.14 µGal/hPa (§7). Abe, Kroner, Neumeyer & Chen (*Bull. Inf. Marées Terrestres*, pp. 11825–11845;
  no DOI found) **[read, primary]** extend this at Moxa over 4 years: the innermost zone (0–0.5°) gives ~20 µGal
  maximum attraction, RMS 3.19 µGal; 0.5–1.5° about 1/7 of it, 1.5–3° about 1/30, 3–5° 0.3 µGal p-p, and beyond
  5° the sign reverses because Earth's curvature puts the air below the sensor (§3-1, Fig. 4). The 3-D minus
  2-D (with temperature) difference is 4.5, 3, 0.8 and 0.26 µGal p-p for 0–0.5°, 0.5–1.5°, 1.5–3°, 3–5°, and
  < 0.2 µGal beyond 5° (§3-1, Fig. 6; §4), so their recommendation is 3-D data to 5° and 2-D beyond. The whole
  3-D+2-D reduction differs from 2-D by 0.5 µGal p-p (RMS 0.1, seasonal 0.15) and from a single admittance
  (−0.3649 µGal/hPa) by ~3 µGal p-p (RMS 0.4) (§3-2, Fig. 8); the admittance over-reduces long periods by
  11–12 % because larger air masses load the Earth more and so have a smaller admittance (§3-3). Air in
  ECMWF levels above 64 km changed gravity by 8.4×10⁻⁴ µGal (§2-1). **All of this is at ≥ 6 h sampling**, below
  AION-10's band, like Boy's. **For the gradiometer** the SPI part matters as lateral structure: below any
  source above the sensor, a horizontal harmonic of wavenumber k gives Γ_zz = k δg exactly, and a laterally
  uniform part gives none. Neither paper states the SPI field's horizontal scale; at 100 km (about Abe's
  innermost zone), 300 km and 1000 km, the 2–2.25 µGal amplitude gives 3.0–3.4×10⁻², 1.0–1.1×10⁻² and
  3.0–3.4×10⁻³ rad ([`spi.py`](gradiometer-environment/spi.py), output
  [`spi.out.txt`](gradiometer-environment/spi.out.txt); its control recovers row 13's formula to the
  factor 1/(1+kH)). That is row 13a: a slow drift up to ~3× the barometer-driven row 13, which S14's E5a
  (surface pressure only) will miss. It is an upper bound for a given amplitude (the measured δg includes a
  uniform part that has no gradient), and ≪ 1 rad in any case, so it is a drift a twin needs, not a search
  limit. A 3-D density field (ECMWF-like, 0.5°, 6 h) driving the far-field air source would carry it.
- **Infrasound.** With δρ/ρ₀ = δp/(γp₀) and a wave fully reflected at the ground, the gravity perturbation is
  [Harms Eq. (143), (171)](https://arxiv.org/abs/1507.05850) **[read]**; for a detector at depth h,
  [Junca et al. 2019 Eq. (23)–(25)](https://arxiv.org/abs/1902.05337) **[read]** give
  a ∝ (4πGρ₀/(γp₀k)) δp e^{−hk sinθ}, so burial suppresses the near-horizontal waves exponentially.
  In gradient form at the surface: 3.6×10⁻¹⁵ s⁻² per Pa, 8.5×10⁻⁵ rad per Pa [derived, agrees with S14].
  MIGA measured the pressure spectrum on site with a microbarometer (1.75 mPa rms resolution, 0.02–4 Hz),
  compared it with Bowman's low/mid/high noise models, and found infrasound Newtonian noise rising as
  f^−2.2 below 0.1 Hz and detectable by the improved MIGA in windy weather
  ([Junca §IV, Figs. 7–9](https://arxiv.org/abs/1902.05337)). Re-read 28 Nov: the 1.75 mPa / 0.02–4 Hz sensor,
  the 5 m/s "light wind" period and the f^−2.2 rise below 100 mHz (Fig. 9 text) are as stated; below 0.02 Hz the
  spectrum is an extrapolation `S_p ∝ ω^−2.2` from the IMS-station model (§IV, Fig. 8), not a measurement, and
  Junca warn that a single-station pressure record "potentially includes a strong contribution that is
  non-coherent on the scale of a large detector, and thus cannot be modelled by plane waves" (§IV).
  [Fiorucci et al. 2018](https://doi.org/10.1103/PhysRevD.97.062003) **[read, primary, 28 Nov]** add the building
  and depth. Their model is Harms's plane wave reflected at the ground (Eq. (1)–(3)); the building is a 6 m
  hemisphere with a 1 m vacuum chamber 1.5 m up, and its interior air matters only near c_s/D ≈ 30 Hz (§VII A,
  Fig. 9). The case that maps onto AION-10 is their *TOBA* (a 10 m gradient sensor below 1 Hz, §IV): there the
  noise is "correlated over the extent of the detector", so they take the gravity-gradient tensor of the plane
  wave rather than summing test masses (§IV; spectra Eq. (10), (12)); the building is neglected "since the cavity or building
  hosting the detector is much smaller than the length of infrasound waves" (§V); even 300 m down the cavity air
  is "well below" the outside air's contribution (Fig. 13); and "the capability of attenuating the infrasound NN
  by placing the detector underground decreases significantly at low frequencies" (§VII C, Fig. 12), with
  infrasound noise "a few orders of magnitude above 10⁻²⁰ /√Hz at 0.1 Hz" (§VIII). So for AION-10 the hall is
  irrelevant to infrasound and row 19's free-field plane-wave gradient is the right model. They also find sound
  coherence lengths typically *shorter* than the wavelength (refs. [36, 37], §IV), and that wind on outdoor
  microphones destroys inter-microphone coherence below 10 Hz (§VI, Fig. 8), which makes Wiener cancellation
  "very challenging" (§VIII). On advected temperature they agree with Harms: the homogeneous model holds above
  ~100 mHz, and below a few tens of mHz new models including the ground temperature are needed (§III B).
  Harms warns that sound transmitted into the ground may *raise* the gravity perturbation (transmissivity
  2α₁/α₂ can exceed 1 at soft sites) and that the model is unproven below a few Hz
  ([Harms §5.1, §5.6](https://arxiv.org/abs/1507.05850)).
  The original estimate, [Saulson 1984](https://doi.org/10.1103/PhysRevD.30.732) **[read, primary]**, fills
  the half-space with independent cells of size λ/2, λ = v_s/f the *acoustic* wavelength (p. 733), sums
  cos²θ/r⁴ over them with an inner cut-off r_min = λ/4 (Eq. (5)), so the force is "dominated strongly by the
  nearest few" cells, and takes |ΔM|² = ½(πv_s)⁶(ρ_a²/p_a²)|Δp|²/ω⁶, the ½ approximating 1/γ² (Eq. (8)–(9)).
  It is a *short-wavelength* model (λ ≪ L, valid above ~10 Hz for km arms, p. 733); the long-wavelength case
  (λ ≫ L, a gradient across the arms) is the Appendix, Eq. (25), where the noise grows linearly with baseline
  in amplitude (p. 736). Saulson checks the independent-cell assumption against one correlated case, a
  "half-plane wave" by the parallel-plate analogy, ẍ = 2√2 GλΔρ (Eq. (17)) against √(4/3) GλΔρ for
  independent cells (Eq. (18)), and calls "the difference … small" (p. 734); times k, Eq. (17) is 2√2 × the
  2πG δp/c² of row 19 [derived], within the crudeness of the slab picture, so row 19 stands. His pressure
  spectrum (Posmentier; Balachandran) is 3×10³ nbar²/Hz (1 Hz/f)² for ½–3 Hz and 3×10² nbar²/Hz above
  (p. 734), i.e. 3×10⁻⁵ Pa²/Hz (f/Hz)⁻² and 3×10⁻⁶ Pa²/Hz [derived, 1 nbar = 10⁻⁴ Pa]: nothing below ½ Hz.
  For AION-10 the sound wavelength at 0.01–1 Hz is 0.34–34 km [derived] against a 10 m tower, so only the
  long-wavelength form could apply, and Saulson's Appendix is written for the horizontal L-shaped
  interferometer's (F₀ₓ−F₁ₓ)−(F₀ᵧ−F₂ᵧ), not a vertical gradiometer; the plane-wave forms above (Harms, Junca)
  remain the ones to use.
- **Advected temperature in the air.** Creighton's model of quasi-static temperature fields carried past a
  test mass by wind (δρ = −ρ₀δT/T₀, Kolmogorov structure function, Taylor's hypothesis) is reviewed with its
  spectrum in [Harms §5.2, Eq. (145)–(155), (173)](https://arxiv.org/abs/1507.05850) **[read]** and is
  given in [Creighton 2008 §3](https://doi.org/10.1088/0264-9381/25/12/125011) **[read, primary]**: pockets
  of size l ≈ v/(2πf) with ⟨δT²(l)⟩ = c_T² l^p, p ≈ 2/3, arriving independently along each streamline
  (order-of-magnitude Eq. (8), rigorous Eq. (10)); c_T² ≈ 0.2 K² m^−2/3 is "a typical daytime peak" and
  v ≈ 20 m/s "fairly windy" (p. 9). The spectrum is cut off exponentially above v/(2π r_min), r_min the
  distance to the nearest streamline (p. 8). **That cut-off is what makes it negligible for LIGO at
  10 Hz and what fails for AION-10**: at 0.1 Hz, v = 10 m/s and r_min ≈ 10 m, 2πf r_min/v ≈ 0.6, so no
  suppression. Carried to the gradiometer's vertical baseline (`sizes.py`, Eq. (8) with the kernel
  a_z(z₂) − a_z(z₁), streamlines excluded from a building of half-width b and height H), it gives
  2×10⁻² rad/√Hz at 0.1 Hz for v = 10 m/s, b = 10 m, H = 25 m; 6×10⁻² for b = 5 m; 7×10⁻³ for b = 20 m;
  4×10⁻³ for v = 5 m/s; and 10⁻⁴–7×10⁻³ at 0.25 Hz **[derived]**. Two caveats bound how much weight it
  bears. Creighton's power law holds only for l up to ~50 times the height above ground, i.e. above
  ≈ 0.2 Hz (v/10 m s⁻¹) for streamlines a metre up (p. 8); and Harms states the advected-temperature model
  "likely does not hold at low frequencies" because the field is no longer quasi-static and ground
  temperature matters (§5.6). So this is an **unvalidated order of magnitude**, and still the largest
  in-band environmental term here; it is row 9a. Creighton also finds sonic-boom shocks and wind-borne
  objects hitting buildings give large transients (abstract; §4–5): fences "within about 30 m".
- **Temperature in the room or shaft.** S13 gives 2.2×10⁻² rad per K for a 5 m × 6 m hall and 2.2×10⁻³ for a
  1 m shaft. MAGIS's site survey found the underground shaft stable and the surface building following the
  weather daily, and set targets of ±0.3 °C for the laser room and ±1 °C for enclosures
  ([Mitchell et al. 2022 §2, §6.1](https://arxiv.org/abs/2202.04763) **[read]**). At 2.2×10⁻² rad/K, ±0.3 °C in
  a hall is ~7 mrad of slow drift: a real, slow signal.

### 2.2 Hydrology and loading

- **Near water dominates.** The disc formula (S14 §2.3) makes a rain contrast 1/a: 10 mm on a roof of radius
  20–50 m, 10 m above the upper IFO, gives 1.8–3.1 rad across the 5 m baseline [derived; geometry
  illustrative, the Beecroft roof is not in hand]. At the Wettzell superconducting gravimeter, a 1 m water
  layer over the whole model gives 42 µGal on flat terrain and 52 µGal with topography (the Bouguer slab
  is 41.9 µGal [derived]); with real topography 80 % (1 m layers) to 52 % (20 m layers) of the signal
  comes from within 50 m, 91–84 % within 500 m; on flat terrain 98–66 % within 50 m
  ([Creutzfeldt et al. 2008, abstract and p. WA100](https://doi.org/10.1190/1.2992508)
  **[read, primary]**; restated in [Creutzfeldt et al. 2010 ¶9](https://doi.org/10.1029/2009WR008359)
  **[read, primary]**: "the higher percentages apply to … WSC closer to the terrain surface"). Both
  papers stress the **building**: the gravimeter house's foundation removes 8.5 µGal of the 1 m response,
  its roof's "umbrella effect" stops infiltration beneath it, and snow on the roof *lowers* gravity because
  it sits above the sensor (2008, p. WA100–WA101; 2010 ¶41); Van Camp et al. advise small enclosures to
  limit the umbrella effect ([2017, appendix "Limiting the umbrella effect"](https://doi.org/10.1002/2017RG000566)).
  For a gradiometer, whose kernel falls as 1/r³ rather than 1/r², the near-field share is larger still. So
  a roof, gutters, a lawn and a drain are scene objects, and rain is a source with mass, which S10 (water)
  can carry; the umbrella (dry soil under the building) is part of the same scene.
- **Loading.** Ocean and atmospheric loading move the tower through the ambient gradient, 1.45×10⁻¹² s⁻² per
  metre (S14): 1 cm is 3.5×10⁻⁴ rad. The point-load Green's functions are tabulated per kilogram for the
  Gutenberg–Bullen earth in [Farrell 1972, Table A3](https://doi.org/10.1029/RG010i003p00761)
  **[read, primary]** (radial displacement −33.6×10⁻¹²/(aθ) m and elastic gravity −77.9×10⁻¹⁸/(aθ) m s⁻²
  per kg near the load, aθ in metres; normalisation §6, Figs. 2–3), and for PREM in
  [Guo et al. 2004, Table 2](https://doi.org/10.1111/j.1365-246X.2004.02410.x) **[read, primary]**, who
  also show that treating the atmosphere as a thin layer rather than a thick one makes a negligible
  difference (summary, Table 7). Either table is a ready validation set for a far-field loading module.

### 2.3 Tides and rotation

S14 sized these and S15 verified the phase terms; nothing here overturns them. Two additions from the
papers read: the rotation-compensating mirror for MAGIS has 1.3 mrad of range on three in-vacuum piezos,
measured from below by a position-sensitive detector ([MAGIS-100 Fig. 17](https://arxiv.org/abs/2104.02835)),
and AION-10's design realises compensation with a tunable effective pivot from the steering mirrors and the
bottom retroreflector ([TDR §3.6](https://arxiv.org/abs/2508.03491)). At Oxford the mirror must turn at
Ω cos φ = 4.5×10⁻⁵ rad/s, 66 µrad over 2T [derived]. Lan et al. found the contrast-maximising tilt rate
at 58.5 ± 1.0 µrad/s against Ω cos φ = 57.4 µrad/s at Berkeley, with the Coriolis phase
Δφ = 2Ω·(v₀ × k)T² ([Lan 2012, Table I and p. 090402-4](https://doi.org/10.1103/PhysRevLett.108.090402)
**[read, primary]**); Dickerson et al. compensated Ω_E = 57.9 µrad/s with a tip-tilt stage of 1 nrad
precision and confirmed the rotation term's slope to <10 % ([Dickerson 2013, Fig. 3, Table I term 2](https://doi.org/10.1103/PhysRevLett.111.083001)
**[read, primary]**). D'Amico et al. still saw shifts of up to 4 mrad for a 0.1 mm/s change of transverse
velocity with the mirror rotating, so a "compensated" coupling need not be small
([D'Amico 2017, p. 253201-4](https://doi.org/10.1103/PhysRevLett.119.253201) **[read, primary]**). With compensation, MAGIS's leading residual for a
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
- **Superconducting gravimeters see people.** A 45 kg boy sitting for 6 minutes with his navel ~1 m above
  an iGrav lowered gravity by 2.8 nm s⁻² ([Van Camp 2017, Fig. A7](https://doi.org/10.1002/2017RG000566)
  **[read, primary]**); a point mass gives 3.0 [derived]. Van Camp et al. say such instruments "detect the
  presence of operators and nearby vehicles" and resolve "10 kg at a distance of one meter" (§4).
  A second known-mass case (§5.1).
- **ELGAR and ZAIGA.** ELGAR's requirements table ([Canuel 2020, Table 2 and §2.1](https://doi.org/10.1088/1361-6382/aba80e)
  **[read, primary]**) sets each spurious term below 0.1 µrad/√Hz per interferometer at 1.7 Hz and lists,
  among others, velocity noise against Earth's rotation (50 nm s⁻¹ Hz^−1/2), gravity-gradient compensation
  to 1 %, pointing jitter, a 25 km wavefront radius, magnetic field 0.1 nT Hz^−1/2 for a 1 nT/m gradient
  and **blackbody radiation 2 mK Hz^−1/2 for a 0.1 K/m temperature gradient, as an acceleration noise**
  (lines 8–10); the derivations are in its companion paper [56], **read 28 Nov**: Canuel et al.,
  *Technologies for the ELGAR large scale atom interferometer array*, [arXiv:2007.04014](https://arxiv.org/abs/2007.04014)
  (63 pp.; §6 and §7.2 read). Its blackbody line is Haslinger's force: `a_x = ∂/∂x [2α_at σ T⁴(x,t)/(m_at c ε₀)]`
  (§7.2.2, Eq. (32), p. 47, citing Haslinger), linearised for `T = T₀ + δT(t) + τx` to
  `S_a = (24 α_at σ T₀² τ/(m_at c ε₀))² S_δT` (Eq. (33)), giving ≤ 2×10⁻³ K/√Hz at 1.7 Hz for Rb, 300 K and
  0.1 K/m. That is U = −½α⟨E²⟩ with ⟨E²⟩ = 4σT⁴/(cε₀) [derived check], the same mechanism and form as row 8a, so
  row 8a carries to Sr by α_g(Sr); it is ELGAR's *horizontal* gradient, where row 8a is vertical. Its
  Newtonian-noise section (§6) models seismic and infrasound NN after Junca (its ref. [38]) with Eq. (8)–(9)
  (infrasound: isotropic adiabatic plane waves with the depth factor e^{−2hk sinθ}, Bowman's mid model), and
  finds both "well above" 10⁻²¹/√Hz in 0.1–1 Hz for one 16.3 km gradiometer (Fig. 13); the 1/√N averaging
  rests on the NN correlation being positive for d < λ/2, negative for λ/2 < d < λ and ~0 beyond, with
  natural seismic anti-correlation limiting the gain factor to K ≳ 0.5 and inhomogeneous ground possibly giving
  K > 1 (§6.2, p. 39); a single-sensor Wiener filter with correlation 0.5 removes only 13 % (§6.4). It also
  names advected temperature (citing Fiorucci) as a further NN source (§6.1). It also argues that averaging
  80 gradiometers per arm reduces gravity-gradient noise by ≥ 1/√N (§1.3.2). ZAIGA ([Zhan et al. 2020,
  §2.2.2](https://doi.org/10.1142/S0218271819400054), read from [arXiv:1903.09288](https://arxiv.org/abs/1903.09288))
  is a Newtonian-noise estimate rather than a systematics budget: Saulson's coherence-cell model with
  an assumed pressure spectrum p̃² ≈ 3×10⁻⁶/(f/Hz)² Pa²/Hz and 20 m/s wind, concluding that Newtonian noise
  "overtakes the shot noise below 1 Hz"; no site measurement yet (Eq. (4)–(9)). Checked against
  [Saulson 1984](https://doi.org/10.1103/PhysRevD.30.732) on 27 Nov (corrected): ZAIGA's Eq. (8) is Saulson's
  Eq. (9) with the sound speed v_s replaced by a wind speed v_w ≈ 20 m/s, while ZAIGA still defines its
  coherence length as the acoustic wavelength λ_c = v_s/f (ZAIGA p. 5); Saulson's model has no wind in it.
  |ΔM|² ∝ v⁶, so 20 m/s for ≈ 340 m/s lowers it by ≈ 2×10⁷ [derived]; how far that carries into ZAIGA's Fig. 7
  depends on which speed its λ_c used, which the text does not say. And the spectrum ZAIGA calls "the same
  estimation as [38]" is Saulson's *above-3 Hz* level, 3×10⁻⁶ Pa²/Hz, given the f⁻² slope of his ½–3 Hz branch,
  which is 3×10⁻⁵ Pa²/Hz (f/Hz)⁻² (Saulson p. 734, converted [derived]): a tenth of Saulson's, and extrapolated
  below his ½ Hz limit. Treat ZAIGA's infrasound curve as unsourced in level.
- **MIGA** (added 28 Nov, third batch): [Canuel et al. 2018](https://doi.org/10.1038/s41598-018-32165-z),
  *Sci. Rep.* 8, 14064, read whole as arXiv:1703.02490v3 **[read, primary]**. This is the instrument and site
  paper, not an environmental systematics budget, so the note's long-standing request (§7 item 9) is answered
  "read, and there is no such budget in it". What it gives: three ⁸⁷Rb Bragg interferometers in a 200 m
  cavity (finesse 100, 780 nm, 2T = 500 ms), 500 m under LSBB (§3.2, §1); detection noise 1 mrad/√Hz at
  10⁶ atoms/s (§3.2, p. 12), dominant at n = 1 everywhere in band; Newtonian noise from seismic and air below
  10⁻¹⁶/√Hz in strain above 0.1 Hz, quoted from its ref. [47] (§3.2, p. 10). **Coriolis**: the gradiometric
  phase cancels if the launch directions do not change between runs; a change δθ in relative launch direction
  gives φ_C = −2Ω_E sin φ_lat k T² v_l δθ ≈ 470 δθ for n = 1 at 43.9°, so 1 mrad needs 1 µrad launch stability
  (Eq. (20), §4.2, p. 17; v_l is not stated in the text read, so 470 was not recomputed). The AION-10 form
  is row 7 with v_⊥ = u₀ δθ: 0.43 rad per µm/s × 3.86 µm/s per µrad = **1.7 rad per µrad** of relative
  launch-direction change, uncompensated [derived]. Vacuum below 10⁻⁹ mbar in the interferometer regions,
  magnetic shielding > 10⁴ (§4.3, p. 17–18). **Site** (§5–5.1): passive temperature stability better than
  0.1 °C in the galleries, air pressure and circulation controlled by airlocks (p. 18); seismic noise near
  Peterson's low-noise model on quiet days (Fig. 9); an iOSG superconducting gravimeter at 1.8 nm s⁻²/√Hz at
  1 mHz (Fig. 10, p. 21); residual magnetic noise ~2 fT/√Hz above 40 Hz in the shielded capsule, and no
  magnetic signal above 0.2 nT correlated with groundwater flow (p. 22–23); the sloping main gallery (5 %) was
  rejected because it projects g into the measurement (§5.2). Its geophysics plan (§5.3): a gradient
  sensitivity of ~10⁻³ E at 1 s, calibration on heavy source masses at up to 150 m, and karst-aquifer
  (hydrology) monitoring. For scale, AION-10's goal 10⁻⁴ rad/√Hz over C = 2.4×10¹⁰ rad/s⁻² is 4×10⁻⁶ E/√Hz
  [derived], ~250× below MIGA's stated figure. The known-mass calibration at up to 150 m is the same kind of
  case as §5.1's `vancamp_boy`.

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
co-magnetometers with opposite-m_F interferometers, leaving the quadratic shift
([MAGIS §5.8](https://arxiv.org/abs/2104.02835)), measured by PTB as a = −23.0(3) Hz mT⁻² (= −0.230 Hz/G²)
over 23–630 µT ([Falke et al. 2011 §3.2, Fig. 4](https://doi.org/10.1088/0026-1394/48/5/022)
**[read, primary]**). Rows 4, 16, 21 of the table. Blackbody: Δν_BBR = −2.354(32) Hz (T/300 K)⁴
([Falke 2011 Eq. (1)](https://doi.org/10.1088/0026-1394/48/5/022), which takes Porsev & Derevianko's value;
MAGIS §5.9 and Graham 2013 p. 4 round it to −2.3) was the number used in the second pass; it is
superseded by **−2.279 Hz (T/300 K)⁴**, 0.0304 Hz/K, from the polarizabilities below; rows 8 and 22.

**The blackbody coefficient from the polarizabilities (third pass, 25 Nov 2026).**
- *Porsev & Derevianko 2006* ([Porsev & Derevianko 2006](https://doi.org/10.1103/PhysRevA.74.020502), Rapid Communication; read from the PDF): the E1 shift of a level is
  −(2/15)(απ)³(k_BT)⁴ α₀(1 + η) in atomic units, with η a "dynamic" fractional correction (their Eq. (7);
  the same form as Safronova's Eq. (5)). Static polarizabilities, Table II: Sr 5s² ¹S₀ **197.2(2)** a.u.,
  5s5p ³P₀ **458.3(3.6)** a.u. (so Δα = 261.1); clock BBR shift at 300 K, Table I: **−2.35432 Hz**, fractional
  −5.5×10⁻¹⁵, uncertainty 7×10⁻¹⁷; the M1 and E2 terms are 2.4×10⁻⁵ Hz and 2.5×10⁻⁸ Hz, negligible (p. 2).
  This is where Falke's −2.354 comes from.
- *Safronova et al. 2013* ([Safronova et al. 2013](https://doi.org/10.1103/PhysRevA.87.012509); read from the PDF): ground state 5s² ¹S₀ α₀ = **197.14(20)** a.u.
  (Step 2 before §VI, p. 6, and Table IV, last entry of the ¹S₀ column; fixed from the measured 5s5p ¹P₁ lifetime);
  clock state 5s5p ³P₀ α₀ = **444.5(2)** a.u. (Table V, "Total"); the clock Stark shift is therefore
  Δα = **247.379(7)** a.u. (the PTB measurement of Middelmann et al. 2012, taken as input; their own
  theory gives 247.5) against Porsev & Derevianko's 261(4), whose ³P₀ value (458.3) was too high
  (444.5). Unit conversion (§II): α/h = 2.48832×10⁻⁸ Hz/(V/m)² per a.u. Dynamic correction at 300 K,
  Table VI and the abstract: 5s² ¹S₀ **−0.0028 Hz**, 5s5p ³P₀ −0.1520 Hz, clock **−0.1492(16) Hz** (7 % of
  the static shift, 98.2 % from the 5s4d ³D₁–5s5p ³P₀ transition; −0.1477(23) Hz in Middelmann).
  The paper does not print a total. Summing its pieces, **my arithmetic** (`sizes.py`): Eq. (5) with
  247.379 a.u. gives −2.130 Hz and the total is **−2.279 Hz**, 3.2 % smaller in magnitude than Falke's
  −2.354. The 2006 theory had a different ³P₀ polarizability, so −2.354 is not "static plus Safronova's
  dynamic term". Eq. (5)'s static term equals −½α₀⟨E²⟩ with ⟨E²⟩ = (831.9 V/m)² at 300 K (my check in
  `sizes.py`); that is the field the forward model needs, and neither paper prints it.
- *Which to use.* Falke's PTB paper (2011) predates Middelmann's measurement, so its −2.354(32) is the
  superseded theory value; rows 8 and 22 now use −2.279 Hz. The T⁴ scaling used for the derivative
  ignores the weaker temperature dependence of the dynamic term (Safronova Eq. (7) has T⁴, T², … terms);
  that term is 7 % of the clock shift and 0.16 % of the ground-state shift, so row 8 could be off by a
  few per cent of 7 % and row 8a by less than 0.2 %. I did not compute the variation. PTB's own vacuum system showed
daily variations < 100 mK at each sensor but 4.2 K peak-to-peak *gradients* across it (§3.2), so a
1 K profile along a 10 m tube is not extreme. The 10⁻¹¹ Hz cm³ mean-field scale (row 17) is the
same order as JILA's measured 10⁻¹⁵ fractional shift at 10¹¹ cm⁻³ (≈ 4×10⁻¹² Hz cm³ [derived]), which
they suppress to (0.6 ± 1.7)×10⁻¹⁷ in a strongly interacting lattice
([Swallows 2012, Figs. 5, 8](https://doi.org/10.1109/TUFFC.2012.2210) **[read, primary]**).

**The blackbody force (added 25 Nov 2026).** Haslinger et al. measured the attraction of Cs atoms to a
heated cylinder, a(r) ∝ −α (T_s⁴ − T₀⁴) R²/r³, in agreement with the a.c. Stark-shift gradient of the
*ground state* and "orders of magnitude stronger than the outward-directed radiation pressure"; they
note that a spatially constant shift is common to both arms and cancels
([Haslinger 2018, abstract, Eq. (2), Fig. 2](https://doi.org/10.1038/s41567-017-0004-9)
**[read, primary]**). For Sr the ground-state shift is a potential h ν_g(T(x)) felt by *both* arms
whatever their internal state, with |ν_g| ≈ 1.70 Hz at 300 K (static 1.698 Hz from α_g = 197.14 a.u.,
Safronova 2013 Table IV, plus the ground-state dynamic correction 0.0028 Hz, Table VI) and
dν_g/dT = 0.0227 Hz/K **[derived]**. The first version used 197.2 a.u. from memory; that is Porsev &
Derevianko's value (Table II, 197.2(2)), so the number barely moved. Through the gradiometer it acts like gravity:
C (h/m) ν_g″, 110 rad per Hz/m², so the same 1 K parabolic profile gives **0.20 rad** (row 8a), and a
linear profile is a uniform force, 1.0×10⁻¹⁰ m s⁻² per K/m, which cancels between the IFOs. Unlike
row 8 it is not reduced by keeping the arms in the ground state. In Cavendish it needs no new channel:
add h ν_g(T) from S13's temperature field to the potential the arms already integrate; the excited arm's
extra shift stays in the state channel. ELGAR's Table 2 line 9 is this effect, budgeted as acceleration
noise ([Canuel 2020](https://doi.org/10.1088/1361-6382/aba80e)).

**The S13 estimate, corrected.** S13 §6 item 9 guessed ~0.1 rad per kelvin of temperature *difference*
between the IFOs. A static difference that is linear in height cancels in the gradiometer; a static
difference with curvature (1 K parabolic over 5 m) gives 0.13 rad, static; a difference in *drift* of
0.1 K/h gives 3×10⁻⁶ rad. So S13's temperature field matters to the blackbody channel only through its
curvature along the tower and its in-band fluctuation; the clock-shift part is small in the real sequence,
the force part (0.20 rad per 1 K of parabolic profile) is not. The gravity of the air (2.2×10⁻² rad/K,
S13) remains the larger heat effect for time-varying work indoors; outdoors, wind-advected temperature
(row 9a) may be larger still.

---

## 4. Instrument systematics, with their published scalings

All from [MAGIS-100 §5](https://arxiv.org/abs/2104.02835) **[read; prefactors re-read in a raw text
extraction]** unless stated; each scales linearly with n, quoted at their n = 100 reference and moved here
to n = 1000.

| Term | MAGIS scaling (n = 100) | At n = 1000, MAGIS reference parameters | What sets it |
|---|---|---|---|
| Laser frequency noise | ~1×10⁻¹³ rad/√Hz (Δv/100 µm/s)(δf/10 Hz/√Hz)(τ/100 µs) | ~10⁻¹³ (Δv = 11 µm/s) | single-photon common mode: N Δv δk Δτ, with the requirement cδk/2π ≲ 10² kHz/√Hz for their satellite case ([Graham et al. 2013, Table I term 3](https://doi.org/10.1103/PhysRevLett.110.171102) **[read, primary]**) |
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
three orders"; ELGAR requires compensation to 1 %, Canuel 2020 Table 2 line 4). The compensation also
cancels Earth's 7.4×10⁴ rad constant (row 1), which is why Carlton & McCabe and Cavendish can leave it out.
**Demonstrated** by D'Amico et al. with three ⁸⁷Rb clouds 31 cm apart, 2T = 320 ms: Eq. (1) is the
uncompensated phase k_eff g T² − k_eff Γ_zz(z₀ + v_z0 T)T², Eq. (2) the condition Δk_eff = (Γ_zz T²/2)k_eff,
and Eq. (3) makes the gradiometer phase linear in the π-pulse detuning with a zero crossing that gives Γ
without knowing the baseline. They measured Γ₂₃ = −3.32(2), Γ₁₂ = −3.48(1), Γ₁₃ = −3.40(1) ×10⁻⁶ s⁻² (source
masses far) and, with them close, Γ₁₃ = (Γ₁₂ + Γ₂₃)/2 as expected; the slope gave d₂₃ = 0.3072(3) m. The
residual instability was 10 mrad, dominated by the AC Stark shift of the Raman beams
([D'Amico 2017, Eq. (1)–(3), Figs. 2–5, p. 253201-3–4](https://doi.org/10.1103/PhysRevLett.119.253201)
**[read, primary]**). The full term list to T⁴, including the (7/12) k g Γ T⁴ term S15 verified and
Roura's k Γ v T³, is [Dubetsky & Kasevich 2006, Table I](https://doi.org/10.1103/PhysRevA.74.023615)
**[read, primary; page image]** (column 3 with T₃ = 2T: (1/8) k v_z T₃³ T_zz = k v Γ T³,
(7/192) k g T₃⁴ T_zz = (7/12) k g Γ T⁴, ħk²T₃³T_zz/(16m) = (ħk²/2m)ΓT³); Dickerson et al.'s Table I
lists the same terms with sizes for a 10 m Rb fountain (terms 4–6: 0.44, 0.18, 0.04 rad). The finite-pulse and gradient-induced Doppler detuning
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
| Air redistributed aloft at constant surface pressure (row 13a) | no | the same far-field air source fed a **3-D density field** (moist ideal gas, Neumeyer Eq. (1)) to 5°, surface pressure beyond (Abe §4) | E5a from surface pressure alone misses up to ~3×10⁻² rad of slow drift (added 28 Nov) |
| Air temperature in the room | planned S8/S13 | scene source | S13 |
| Rain, roof water, soil moisture near the tower | not planned as such | **scene source** (S10 water: a sheet with a fill level) | 1/a: near water dominates (2–3 rad) |
| Tides, ocean loading, rotation | planned S14 | far-field source; Coriolis optional in the arms | S14 |
| Earth's gradient in the flight; Roura Δk at T | not planned explicitly | **instrument options** in `Instrument` (`earth_gradient`, `gradient_compensation`) | changes signals by ~10⁻⁶ relative; validates against Roura |
| Finite kick duration | no | **instrument parameter** `kick_duration` (ramp) | relative error τ/T on every signal |
| Clock shifts: Zeeman, blackbody, AC Stark, scalar ULDM | no | **state-dependent channel**: a shift field δν(x, t) read on the excited arm, with an occupancy pattern | one integral, four physical sources; ULDM is S16's |
| Blackbody force (ground-state Stark potential) | no | **a term in the ordinary potential**, h ν_g(T(x)) from S13's temperature field, on both arms | 0.2 rad per K of profile curvature; needs no new channel (added 25 Nov) |
| Wind-advected outdoor temperature | no | **noise channel** with exposed c_T², wind speed and building size (Creighton Eq. (8) spectrum), labelled unvalidated | model unproven below a few Hz; 10⁻²-ish rad/√Hz at 0.1 Hz if it holds (added 25 Nov) |
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
| `damico_zero_crossing` | with source masses giving Γ₁₂, Γ₂₃, the π-pulse detuning that nulls each gradiometer phase gives Γ independent of d, and Γ₁₃ = (Γ₁₂+Γ₂₃)/2 | [D'Amico 2017 Eq. (3)](https://doi.org/10.1103/PhysRevLett.119.253201) and its measured values, ≤ 1 % | an error in d must not move the zero crossing; it must move the ellipse-fit value by Δd/d |
| `coriolis_tilt_rate` | the tip-tilt rate that zeroes the transverse-velocity phase equals Ω cos φ | Lan 2012 (58.5 ± 1.0 vs 57.4 µrad/s); Dickerson Table I term 2 | the wrong sign of rate doubles the phase |
| `vancamp_boy` | a 45 kg body ~1 m above a gravimeter point | 2.8 nm s⁻² measured ([Van Camp 2017 Fig. A7](https://doi.org/10.1002/2017RG000566)); point mass 3.0 | moving the mass to the sensor's height gives ~0 vertical |
| `water_slab_bouguer` | a 1 m water layer of large radius under a gravimeter | 2πGρh = 41.9 µGal; Creutzfeldt 2008's flat-terrain model 42 | a finite disc of radius a converges as 1 − d/√(a²+d²) |
| `farrell_point_load` (when loading is built) | radial displacement and elastic gravity of a 1 kg surface load | [Farrell 1972 Table A3](https://doi.org/10.1029/RG010i003p00761) (or Guo 2004 Table 2 for PREM), ≤ 1 % | |

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
   temperature field, and S16's scalar dark matter, all with closed forms (§5.1). *Added 25 Nov:* when
   S13's field reaches the tower, also add the ground-state blackbody potential h ν_g(T) to the ordinary
   potential on both arms (row 8a); it is as large as the clock-shift part and survives any sequence.
4. **Give `Instrument` three options**: Earth's gradient in the flight, the Roura Δk at T, and a kick
   duration. Each defaults off, so the v1 tie and every existing fixture stay bit-identical. D'Amico's
   zero-crossing measurement is the published check on the second (§5.1).
5. **Leave seismic to the other note; keep S14's far field as it is; put rain and roof water (and the
   building's umbrella) in the scene.** *Changed 25 Nov:* add wind-advected outdoor temperature to the
   noise-model layer as a labelled, *unvalidated* channel (Creighton's spectrum with c_T², wind speed and
   building size as parameters, default off), and put the question to George: a weather mast with a fast
   thermometer and anemometer at the Beecroft site would settle whether it is 10⁻² or 10⁻⁴ rad/√Hz. Do not
   simulate it as a physical air field until a measurement exists; the model's authors say it is unproven
   in this band.

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
3. **Wavefront curvature × temperature** (row 3) now uses Dickerson's printed term 6 (read from the page
   image). Recomputing Dickerson's own row from his stated parameters gives 0.066 rad against the printed
   0.04, so either the table uses a different velocity convention or I misread one parameter: the form is
   right to within a factor of 2. For single-photon LMT with pulses from alternating directions the
   coefficient may differ again. Order of magnitude only.
4. **Rain on a roof**: the Beecroft roof's size and height above the tower are not in hand; the 2–3 rad is
   illustrative. (Creutzfeldt's near-field shares are now read in both papers.)
5. **Admittance**: now read in Merriam and Van Camp, and (28 Nov) Boy, Gegout & Hinderer 2002 and Boy & Chao
   2005 (§2.1): both are for periods ≥ 12 h and change nothing here. (28 Nov, third batch) Neumeyer et al.
   2004 and Abe et al. read whole: they add row 13a. **Not verified:** the SPI field's horizontal scale,
   which sets row 13a within a factor of 10 (100–1000 km swept); the amplitudes are read from the papers'
   text and figure captions, not digitised from the plots. Resolve with a 3-D reanalysis field (ECMWF or
   ERA5) at the site: compute Γ_zz directly from the density cells and compare with row 13's surface-pressure
   value. Abe et al.'s PDF prints neither volume nor year; "BIM 146" is from the file name only, the page
   numbers (11825–11845) are printed, and Crossref has no record of it.
6. **Carlton Fig. 3 magnitudes** are read off a plot (person on stairs: −6 to +3 rad on the axis; lift
   ~ ±0.75–1.5 rad; foyer ~0.2; car ~ −0.05 to −0.1). Eq. (13) at far-field positions gives 0.09 and −0.03 rad,
   the same order; the near-field values need the integral.
7. **Blackbody force (row 8a)**: the polarizability (197.14 a.u.) and the dynamic correction are now from
   Safronova et al. 2013; the 300 K field ⟨E²⟩ = (831.9 V/m)² is my arithmetic from their Eq. (5), not a
   printed number, and the T⁴ scaling of the small dynamic correction is assumed.
   Haslinger measured Cs, not Sr, and the force on Sr atoms in a gradiometer has not been measured; the
   order of magnitude is safe
   because the ground-state shift is a large fraction of the measured clock shift.
8. **Thorne & Winstein** read in abstract and introduction only; the PMC summary of Stray et al. is a
   snippet. Fiorucci et al. read in full on 28 Nov (§2.1).
9. **ELGAR's companion paper** (ref. [56], arXiv:2007.04014) read on 28 Nov, §6 and §7.2 (§2.4); its §7.1,
   §7.3–7.5 (rotations, wavefronts, scattered light, interactions) were not read. **MIGA's systematics paper**:
   closed 28 Nov (third batch). The candidate, Canuel et al., *Exploring gravity with the MIGA large scale atom
   interferometer*, Sci. Rep. 8, 14064 (2018), doi:10.1038/s41598-018-32165-z (Crossref-checked), was read whole
   as arXiv:1703.02490v3 (§2.4): it has a Coriolis requirement and site numbers but no environmental
   systematics budget. No other MIGA paper is known to me that has one; I did not search further. The arXiv
   v3 was read, not the Sci. Rep. typeset version. Junca's journal reference (PRD 99, 104026) is not printed
   on the arXiv v1 read.
10. **Wind-advected temperature (row 9a)**: an extrapolation of Creighton's order-of-magnitude Eq. (8)
    below its validity (Kolmogorov scaling only for l ≲ 50 × height, i.e. f ≳ 0.2 Hz (v/10 m s⁻¹); Harms
    §5.6 says the model "likely does not hold at low frequencies"), with c_T² at its daytime peak, a
    uniform wind, a box building and the lower IFO 2 m above ground. Its spread over plausible geometries
    is a factor of ~15 at 0.1 Hz. Resolve with a site measurement of c_T² and wind (or a measured gravimeter
    or gradiometer record against wind speed); the slice can carry it as a parameterised channel meanwhile.
11. **ZAIGA** read from arXiv:1903.09288 (the accepted manuscript), not the journal version.

---

## 8. Further questions (not chased)

- What does the AION collaboration's own systematics budget (if any beyond the TDR) assign to magnetic,
  wavefront and blackbody terms at n = 1000?
- Does AION-10 plan gravity-gradient compensation (Roura/D'Amico) or multi-loop sequences?
- Are the Beecroft stairs to be closed during runs, as Carlton & McCabe assume at night?
- How large is the sound-to-ground transmission term Harms flags, at a building on clay (Oxford)?
- Can S13's temperature field and S8's air give a *joint* blackbody-plus-air-gravity budget for the tower
  (the same temperature drives both, with opposite structure: air reads the room, blackbody reads the tube)?
- Does the building damp advected-temperature noise inside (the stairwell air is still, but the outdoor
  field is within 10–20 m), and does the noise fall with height as c_T² ∝ z^(−4/3) suggests?
- ~~What does ELGAR's companion paper derive for the blackbody acceleration noise, and does it match row 8a's
  form when moved to Sr?~~ Answered 28 Nov: Haslinger's force, same form (§2.4, ELGAR and ZAIGA).

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
- Fiorucci, Harms, Barsuglia, Fiori, Paoletti, PRD 97, 062003 (2018), [arXiv:1801.04564](https://arxiv.org/abs/1801.04564) — in part; the published version read in full on 28 Nov (below).
- Thorne & Winstein, PRD 60, 082001 (1999), [arXiv:gr-qc/9810016](https://arxiv.org/abs/gr-qc/9810016) — abstract and §I.
- Stray et al., Nature 602, 590 (2022), [doi:10.1038/s41586-021-04315-3](https://doi.org/10.1038/s41586-021-04315-3) — via PMC, summary of full text.
- Read from the published PDFs on 25 Nov 2026 (fetched by the owner; not committed):
  - D'Amico, Rosi, Zhan, Cacciapuoti, Fattori, Tino, *Canceling the gravity gradient phase shift in atom interferometry*, PRL 119, 253201 (2017), [doi:10.1103/PhysRevLett.119.253201](https://doi.org/10.1103/PhysRevLett.119.253201) — whole paper.
  - Canuel et al. (ELGAR), *ELGAR — a European Laboratory for Gravitation and Atom-interferometric Research*, CQG 37, 225017 (2020), [doi:10.1088/1361-6382/aba80e](https://doi.org/10.1088/1361-6382/aba80e) — §1.3–2.1, Tables 1–2.
  - Creighton, *Tumbleweeds and airborne gravitational noise sources for LIGO*, CQG 25, 125011 (2008), [doi:10.1088/0264-9381/25/12/125011](https://doi.org/10.1088/0264-9381/25/12/125011) — abstract, §3 (Eq. (8)–(18)).
  - Creutzfeldt, Güntner, Klügel, Wziontek, *Simulating the influence of water storage changes on the superconducting gravimeter of the Geodetic Observatory Wettzell, Germany*, Geophysics 73, WA95 (2008), [doi:10.1190/1.2992508](https://doi.org/10.1190/1.2992508) — abstract, results (p. WA99–WA101), Table 3.
  - Creutzfeldt, Güntner, Thoss, Merz, Wziontek, *Measuring the effect of local water storage changes on in situ gravity observations*, WRR 46, W08531 (2010), [doi:10.1029/2009WR008359](https://doi.org/10.1029/2009WR008359) — §2, ¶41.
  - Dickerson, Hogan, Sugarbaker, Johnson, Kasevich, *Multiaxis inertial sensing with long-time point source atom interferometry*, PRL 111, 083001 (2013), [doi:10.1103/PhysRevLett.111.083001](https://doi.org/10.1103/PhysRevLett.111.083001) — Table I (page image), Figs. 1–3.
  - Dubetsky & Kasevich, *Atom interferometer as a selective sensor of rotation or gravity*, PRA 74, 023615 (2006), [doi:10.1103/PhysRevA.74.023615](https://doi.org/10.1103/PhysRevA.74.023615) — §I–II, Table I (page images).
  - Falke et al., *The ⁸⁷Sr optical frequency standard at PTB*, Metrologia 48, 399 (2011), [doi:10.1088/0026-1394/48/5/022](https://doi.org/10.1088/0026-1394/48/5/022) — §3.2–3.3, Eq. (1).
  - Farrell, *Deformation of the Earth by surface loads*, Rev. Geophys. Space Phys. 10, 761 (1972), [doi:10.1029/RG010i003p00761](https://doi.org/10.1029/RG010i003p00761) — §6, Figs. 2–4, Table A3.
  - Graham, Hogan, Kasevich, Rajendran, *New method for gravitational wave detection with atomic sensors*, PRL 110, 171102 (2013), [doi:10.1103/PhysRevLett.110.171102](https://doi.org/10.1103/PhysRevLett.110.171102) — Eq. (1), Table I (page images).
  - Guo et al., *Green's function of the deformation of the Earth as a result of atmospheric loading*, GJI 159, 53 (2004), [doi:10.1111/j.1365-246X.2004.02410.x](https://doi.org/10.1111/j.1365-246X.2004.02410.x) — summary, §1, table captions.
  - Haslinger et al., *Attractive force on atoms due to blackbody radiation*, Nat. Phys. 14, 257 (2018), [doi:10.1038/s41567-017-0004-9](https://doi.org/10.1038/s41567-017-0004-9) — main text to Eq. (2).
  - Lan, Kuan, Estey, Haslinger, Müller, *Influence of the Coriolis force in atom interferometry*, PRL 108, 090402 (2012), [doi:10.1103/PhysRevLett.108.090402](https://doi.org/10.1103/PhysRevLett.108.090402) — whole paper.
  - Merriam, *Atmospheric pressure and gravity*, GJI 109, 488 (1992), [doi:10.1111/j.1365-246X.1992.tb00112.x](https://doi.org/10.1111/j.1365-246X.1992.tb00112.x) — summary, introduction, p. 490, p. 495.
  - Swallows et al., *Operating a ⁸⁷Sr optical lattice clock with high precision and at high density*, IEEE TUFFC 59, 416 (2012), [doi:10.1109/TUFFC.2012.2210](https://doi.org/10.1109/TUFFC.2012.2210) — abstract, density-shift section, Figs. 5, 8.
  - Van Camp et al., *Geophysics from terrestrial time-variable gravity measurements*, Rev. Geophys. 55 (2017), [doi:10.1002/2017RG000566](https://doi.org/10.1002/2017RG000566) — §3.4, §4, Appendix A1.3 and A-figures.
  - Zhan et al. (ZAIGA), *ZAIGA: Zhaoshan long-baseline atom interferometer gravitation antenna*, IJMPD 29, 1940005 (2020), [doi:10.1142/S0218271819400054](https://doi.org/10.1142/S0218271819400054), read as [arXiv:1903.09288](https://arxiv.org/abs/1903.09288) — §2.2.
  - Not ZAIGA: the file fetched from the from-memory DOI 10.1142/S0218271820500479 is Quiros, De Arcia, García-Salcedo, Gonzalez, Horta-Rangel, *An issue with the classification of the scalar-tensor theories of gravity*, IJMPD 29, 2050047 (2020), unrelated to this note.
  - Harms §5.6 re-read from [arXiv:1507.05850](https://arxiv.org/abs/1507.05850) for the advected-temperature validity warning.
- Read from the published PDF on 27 Nov 2026 (fetched by the owner; not committed):
  - Saulson, *Terrestrial gravitational noise on a gravitational wave antenna*, PRD 30, 732 (1984), [doi:10.1103/PhysRevD.30.732](https://doi.org/10.1103/PhysRevD.30.732) — whole paper (pp. 732–736), Eq. (1)–(25) from the page images.
- Read from the published PDFs on 28 Nov 2026 (fetched by the owner; not committed):
  - Fiorucci, Harms, Barsuglia, Fiori, Paoletti, *Impact of infrasound atmospheric noise on gravity detectors used for astrophysical and geophysical applications*, PRD 97, 062003 (2018), [doi:10.1103/PhysRevD.97.062003](https://doi.org/10.1103/PhysRevD.97.062003) — whole paper (equations only as far as the text extraction allows).
  - Boy, Gegout, Hinderer, *Reduction of surface gravity data from global atmospheric pressure loading*, GJI 149, 534–545 (2002) — §1–2, §3.1, §5.1, §6; no DOI printed.
  - Boy & Chao, *Precise evaluation of atmospheric loading effects on Earth's time-variable gravity field*, JGR 110, B08412 (2005), [doi:10.1029/2002JB002333](https://doi.org/10.1029/2002JB002333) — whole paper.
  - Canuel et al., *Technologies for the ELGAR large scale atom interferometer array*, [arXiv:2007.04014](https://arxiv.org/abs/2007.04014) (2020) — §6, §7.2 (p. 47 from the page image); ref. [56] of Canuel 2020 CQG.
  - Junca et al., arXiv:1902.05337v1 — re-read §IV–V and the conclusion.
- Read from owner-supplied PDFs on 28 Nov 2026, third batch (not committed):
  - Neumeyer, Hagedoorn, Leitloff, Schmidt, *Gravity reduction with three-dimensional atmospheric pressure data for precise ground gravity measurements*, J. Geodyn. 38, 437–450 (2004), [doi:10.1016/j.jog.2004.07.006](https://doi.org/10.1016/j.jog.2004.07.006) (Crossref-checked) — whole paper.
  - Abe, Kroner, Neumeyer, Chen, *Assessment of atmospheric reductions for terrestrial gravity observations*, Bulletin d'Information des Marées Terrestres, pp. 11825–11845 (volume 146 by the file name; year not printed, after 2009 by its references; no DOI found in Crossref) — whole paper.
  - Canuel et al. (MIGA), *Exploring gravity with the MIGA large scale atom interferometer*, Sci. Rep. 8, 14064 (2018), [doi:10.1038/s41598-018-32165-z](https://doi.org/10.1038/s41598-018-32165-z) (Crossref-checked), read as [arXiv:1703.02490v3](https://arxiv.org/abs/1703.02490) — whole paper (appendices A–B skimmed).
- This repository: `crates/cav-core/src/forward.rs`; `plan/REBUILD.md` §9, §9.3; `plan/PROGRESS.md` (For George, rulings);
  `docs/research/s13-heat.md` §5–6, `s14-wider-world.md`, `s15-instruments-anywhere.md`.

## Papers still unread

Every paper in the earlier "could not read" list has now been read (above). Still unread, and worth
fetching if a slice leans on them: the rest of ELGAR's companion paper (§7.1, §7.3–7.5); Neumeyer, Schmidt &
Stoeber 2006 (IAG Symposia 130, 541–548, the improved 3-D attraction both atmospheric papers build on; not
supplied, no DOI checked). MIGA's paper (Canuel et al. 2018) was read on 28 Nov (third batch) and is not a
systematics budget (§7 item 9). Saulson 1984 was read on 27 Nov; the ELGAR companion (§6, §7.2), Fiorucci et al. and both
Boy papers on 28 Nov (above). Safronova et al. 2013 and Porsev & Derevianko 2006 (Sr polarizabilities, rows 8 and 8a) were
read in the third pass.
