# S14 research note — the wider world: what lies beyond the voxel scene, and the rotating Earth

> Research note for a possible "wider world" layer (working number S14). Method note, not a slice brief.
> Written 22 Nov 2026 on branch `research/s14-wider-world`. GPU-free; nothing in the engine was changed.
> The numbers are produced by [`s14-wider-world/estimates.py`](s14-wider-world/estimates.py) (standard library
> only; output in [`estimates.out.txt`](s14-wider-world/estimates.out.txt)); every figure below that is not a
> published value is that script's output, with the formula shown here.
>
> **Read this first about sources.** The sandbox this was written in had no outbound network except package
> registries (arXiv and every publisher host returned 403; the owner's request to allow them was not answered).
> So **no paper was opened in full**. Each reference in §10 carries a status: **S** = the title, abstract or a
> quoted passage came back from a web search this session; **M** = cited from memory and *not* checked;
> **R** = taken from this repository's own documents; **D** = derived in this note. Section 11 lists what to
> check first. Treat every **M** citation as a pointer to read, not as a verified fact.

---

## 0. One-page summary

**Question.** Can the world beyond the voxel scene be simulated approximately, outside the voxel engine,
to see what distant things do to the instrument? **Yes, and for a few sources it is not optional.** The
instrument reads a vertical gradient: phase ΔΦ = k_eff T² Δr · Γ_zz (D, §1), a constant of 2.4×10¹⁰ rad per s⁻²
for AION-10 and 3.9×10¹² for AICE (assuming AICE's separation is 70 m and its kick number is AION's: **to confirm
with George**). One shot's noise is 5×10⁻⁵ rad (AION-10) and 3×10⁻⁵ rad (AICE) from the stated 10⁻⁴ rad/√Hz. So
AION-10 resolves a gradient of 2×10⁻¹⁵ s⁻² per shot and AICE 8×10⁻¹⁸ s⁻². Far-field sources reach the
gradiometer in four ways: direct Newtonian attraction (2Gm/r³, or 2πGδρ-type forms for extended fields), a wave
that moves or compresses local mass, displacement of the tower through the ambient gradient (only
6GM/R⁴ = 1.45×10⁻¹² s⁻² per metre, which is small), and loading (a Green's-function sum of the first two).

| Source | Γ_zz (s⁻²) | AION-10 (rad) | AICE (rad) | Band (Hz) | Verdict |
|---|---|---|---|---|---|
| Lunar + solar body tide (with Love factor 0.97) | 1.5×10⁻¹³ | 3.6×10⁻³ | 0.59 | 10⁻⁵ | **Seen after averaging at AION-10; seen shot to shot (step 4×10⁻⁴ rad) at AICE.** Fully predictable. |
| Ocean-tide attraction (1 m, coast 100 km, upper bound) | 2×10⁻¹² | 5×10⁻² | 8 (wraps) | 10⁻⁵ | **Dominates the body tide near any sea.** A Green's-function job; open data. |
| Atmospheric pressure systems (λ = 1000 km, 10 hPa) | 2.5×10⁻¹³ | 6×10⁻³ | 1.0 | 10⁻⁵ | **As large as the tide. Seen after averaging.** Closed form from a pressure map. |
| Air density change within a few km (per 1 %) | 5×10⁻¹² | 0.12 | 20 | 10⁻⁵–10⁻³ | **Near field: belongs in the scene** (S8), driven by weather as a boundary condition. |
| Cloud / rain cell overhead | 2–8×10⁻¹⁴ | 0.5–2×10⁻³ | 0.08–0.3 | 10⁻³ | Above shot noise; minutes. Upper bound, cancellation unquantified. |
| Soil-moisture / water contrast (10 mm over 10 km) | 4×10⁻¹³ | 10⁻² | 1.6 | 10⁻⁵ | Real; **1/a, so near contrasts dominate** (roofs, lawns, a river) and belong in the scene. |
| Infrasound, microbarom (0.1–1 Pa at 0.2 Hz) | 4×10⁻¹⁶–4×10⁻¹⁵ | 0.8–8×10⁻⁵ | 6×10⁻⁴–6×10⁻³ | 0.1–0.3 | At/below shot noise at AION-10; above it at AICE. |
| Microseism Newtonian noise (placeholder spectrum) | — | 0.015–0.15 of shot ASD | 1–10 of shot ASD | 0.1–0.3 | Below shot noise at AION-10; **at or above it at AICE.** Spectrum is a placeholder. |
| Teleseismic Rayleigh waves, Newtonian part | 5×10⁻¹⁵ (Ms 7) | 1×10⁻⁴ | 2×10⁻² | 0.03–0.1 | Visible above **Ms ≈ 6.2 (AION-10) and ≈ 4.0 (AICE)** at 80°. Instrument shaking, not modelled, is far larger. |
| Prompt elasto-gravity signal | 4×10⁻¹⁵ (Mw 9, 1000 km) | 1×10⁻⁴ | 1.6×10⁻² | 0.01–0.1 | AION-10: only a great quake. AICE: Mw 7 within ~300 km. Tens of seconds. |
| Tsunami (0.5 m, 10⁴ km², 150–500 km) | 5×10⁻¹⁵–2×10⁻¹³ | 1.3×10⁻⁴–5×10⁻³ | 0.02–0.8 | 10⁻³ | Above shot noise within a few hundred km of a coast; nothing inland. |
| Hunga Tonga Lamb wave (100 Pa, 1 h) | 2×10⁻¹⁴ | 5×10⁻⁴ | 8×10⁻² | 3×10⁻⁴ | Above shot noise in principle; a one-off parametric wave. |
| Free oscillations (1 mm ringing) | 1.5×10⁻¹⁵ | 3×10⁻⁵ | 6×10⁻³ | 3×10⁻⁴ | After a day's averaging in white noise only; **never in practice** at AION-10. |
| Pole tide (10 mm) | 1.5×10⁻¹⁴ | 3×10⁻⁴ | 0.06 | 3×10⁻⁸ | Indistinguishable from drift. Negligible as a signal. |
| Lorry beyond the scene (30 t at 500 m / 1 km) | 3×10⁻¹⁴ / 4×10⁻¹⁵ | 8×10⁻⁴ / 1×10⁻⁴ | 0.12 / 0.015 | 0.05 | **Sets the scene radius**: 1.2 km (AION-10) or 6.8 km (AICE) at 5×10⁻⁵ rad. |
| Train (400 t at 500 m) | 4×10⁻¹³ | 10⁻² | 1.6 | 0.05 | Real. 2 km at AION-10 for 5×10⁻⁵ rad. |
| Wind sway of buildings (10⁷ kg, 1 mm) at 300 m / 1 km | 5×10⁻¹⁶ / 4×10⁻¹⁸ | 10⁻⁵ / 6×10⁻⁸ | 2×10⁻³ / 2×10⁻⁵ | 0.5 | **Nothing would be seen beyond ~1 km.** Inside it, the scene's job. |
| Events on the far side of the planet (static, 1 Gt) | 8×10⁻²⁰ | nothing | nothing | — | **Nothing**, except through waves: seismic, Lamb, tide. |

**What the rotating Earth does** (§5): the Coriolis acceleration is common to both interferometers and cancels in
the gradiometer difference except through a launch-velocity mismatch (phase 2k_eff Ω cos φ T² Δv: **0.43 rad per
µm/s at AION-10, 5 rad per µm/s at AICE**; one shot's noise needs Δv below 1×10⁻¹⁰ m/s and 6×10⁻¹² m/s), which
real instruments null with a tip-tilt mirror rather than suppress. The Sagnac phase of a light-pulse
interferometer is *the same term*, not a separate one. The atoms are carried sideways by 0.09 mm (AION-10) and
5.6 mm (AICE) over the flight, which changes the phase of a source 1 m away by 3×10⁻⁴ and 2×10⁻² relative: far
above the engine's 10⁻⁹ oracle level, so the arm trajectories need an optional Coriolis correction. Length of day
and polar motion are negligible as signals (≤ 5×10⁻⁶ rad). The Rossby number is 175 for a room or a building,
9 for a town, 0.09 for a weather system: Coriolis on the air matters only at town scale and above.

**Recommendation (§8).** One slice, no network and no data files: an **ambient far-field layer** carrying
(1) the Moon and Sun as exact far point masses from a short analytic ephemeris, times the Love-number factor;
(2) plane-wave and pressure-field atmosphere from the closed form Γ(k) = 2πG k ∫δρ(z) e^{−k|z−h|} dz (covers
pressure systems, infrasound and a Lamb wave with one formula); (3) the **scene-radius rule** r = (2GmC/ε)^{1/3}
and a Campbell-theorem noise for roads and rails beyond it; (4) an optional Coriolis correction to the arms and
a latitude/azimuth frame. Leave out seismic synthesis, tsunami, ocean loading, hydrology and ERA5 import.

---

## 1. From a source to a phase: the translation used throughout

**Instrument (R).** `crates/cav-core/src/defaults.rs` and `constants.rs`: tower 10 m, IFO separation Δr = 5 m,
half-sequence T = 0.73 s, cycle 2 s, λ = 698 nm, 1000 kicks, so k_eff = 2π·1000/λ = 9.00×10⁹ rad/m.
AICE (`plan/VALIDATION-AND-RESEARCH.md` §1): 70 m gradiometer, T = 2.5 s, cycle 5 s, launch 21.27 m/s. **Assumed
here, to confirm with George:** AICE's IFO separation is the 70 m and its kick number is AION's.

**(E1) Phase per differential acceleration.** For a source slowly varying over the 2T flight, the arm difference
and the double difference reduce to (`reference/design/instrument.md` §7, R; Peters et al. [R15] for the standard
single-interferometer result Φ = k_eff a T²):

    ΔΦ = k_eff T² · (a(z+Δr) − a(z)) = k_eff T² Δr · Γ_zz = C · Γ_zz,   C = k_eff T² Δr.

    AION-10:  C = 9.00e9 × 0.5329 × 5  = 2.40e10 rad per s⁻²  (24 rad per eotvos, 1 E = 1e-9 s⁻²)
    AICE:     C = 9.00e9 × 6.25   × 70 = 3.94e12 rad per s⁻²

A uniform field δg cancels exactly in the double difference. A real instrument has a finite common-mode
rejection, which the forward model does not carry (flagged in §9). The signal is unwrapped phase; the readout's
2π ambiguity (AICE's ocean-tide phases reach several radians) is a downstream matter.

**(E2) Frequency response.** For a differential acceleration at angular frequency ω the three-pulse sequence
weights the signal by the triangle of width 2T, so (D)

    |H(f)| / T² = 4 sin²(ωT/2) / (ωT)²,    first null at f = 1/T = 1.37 Hz (AION-10), 0.40 Hz (AICE).

Below 0.1 Hz H ≈ 1. **Sampling:** the cycle sets Nyquist at 0.25 Hz (AION-10) and 0.10 Hz (AICE); anything above
folds in unless H has suppressed it. Infrasound at 0.2 Hz is inside the AION-10 band; anything above ~0.4 Hz
(wind noise, machinery) is attenuated ∝ 1/f² only beyond ωT ≈ 1 and otherwise aliases.

**(E3) Noise per shot.** One-sided 10⁻⁴ rad/√Hz at cycle Δt gives σ_shot = 10⁻⁴ / √(2Δt) = 5.0×10⁻⁵ rad
(AION-10) and 3.2×10⁻⁵ rad (AICE). This is the white shot-noise floor of the plan; technical noise (laser
phase, vibration, tilt, magnetic) is *not* in it and is worse at the slow frequencies most of this note lives at.

**(E4) Detectability, three tiers.** "One shot": amplitude A_phase against σ_shot. "Averaged": a tone of known
frequency and phase over N = t_obs/Δt shots has amplitude uncertainty σ_shot √(2/N), so SNR = A √(N/2) / σ_shot.
"Never": below the averaged floor for any plausible observation time, or below technical noise by construction.
SNR(avg) values are in `estimates.out.txt`; they assume white noise and **overstate** what slow signals can
achieve.

**(E5) Four ways a far source gets in.**
1. *Direct attraction.* A point mass m at distance r on the axis gives Γ_zz = 2Gm/r³. A horizontally layered
   density anomaly δρ(z) with horizontal wavenumber k gives, at height h above flat ground (D, by summing
   Poisson sheets, each sheet of surface density σ cos kx giving Γ = 2πGσ k e^{−k|z−z′|}):

       Γ_zz(k, h) = 2πG k ∫ δρ(z′) e^{−k|z′−h|} dz′.                                         (E5a)

   At the ground, with δρ = δρ₀ e^{−z/H}: Γ = 2πGδρ₀ kH/(1+kH): zero for k → 0 (an infinite uniform layer has
   no gradient below it), 2πGδρ₀ for kH ≫ 1. Inside the layer, k → ∞: 4πGδρ, which is the "4πGρ ≈ 1 E" of air.
2. *A wave moving local mass.* Rayleigh waves displace the surface; the Newtonian gradient is
   Γ ≈ γ 2πGρ k ξ_z with γ of order 0.5–1 (Hughes & Thorne [R4]; Harms [R3]). **The factor must be taken from
   the primary source before use** (M17 §3 deliberately did not restate it).
3. *Displacement of the tower through the ambient gradient.* ∂²/∂z² of Earth's gravity potential changes by
   6GM_E/R⁴ = 1.45×10⁻¹² s⁻² for each metre the tower rises (D).
4. *Loading:* the sum of 1 and 3 plus the induced potential, by Green's function (Farrell [R24]).

---

## 2. Source by source

Each subsection: mechanism, formula, numbers (all from `estimates.out.txt`), band, verdict.

### 2.1 Lunar and solar body tides, and the solid-Earth response

**Mechanism.** The Moon's tidal potential is W = (GM_m r²/d³)(3cos²ψ − 1)/2 [R42, R22], so the direct vertical
gradient is Γ = (GM_m/d³)(3cos²ψ − 1): +2× at the zenith, −1× at the horizon. GM_m/d³ = 8.63×10⁻¹⁴ s⁻² (mean
distance 3.844×10⁸ m), the Sun's 3.96×10⁻¹⁴ s⁻². Sampled over a year with circular orbits at latitude 51.8°, the
sum swings from −1.26×10⁻¹³ to +1.85×10⁻¹³ s⁻² (amplitude 1.55×10⁻¹³). Perigee/apogee scale the Moon's part by
×1.18 / ×0.85.

**The solid-Earth factor (D, derived here; check against a published gradient tide before use).** The tower
rides the crust and is surrounded by the Earth's own field. With degree-2 Love numbers h₂, k₂ the three pieces
of ∂²/∂r² are: direct GM P/d³; the induced potential k₂W(R/r)³, giving 6k₂ times the direct; and the vertical
displacement u = h₂W/g carried through the ambient ∂³U/∂r³ = −6GM_E/R⁴, giving −3h₂ times the direct. So

    Γ_tide = (GM/d³)(3cos²ψ − 1) · (1 + 6k₂ − 3h₂),   with k₂ ≈ 0.30, h₂ ≈ 0.61 (IERS 2010 ch. 7 [R22], M):
    factor = 1 + 1.80 − 1.83 = 0.97.

The same three-piece derivation applied to *gravity* gives 1 + h₂ − 3k₂/2 = 1.16, the textbook gravimetric
factor, which is the check on the signs. The near-cancellation (0.97 ≈ 1) is why the gradient tide is close
to the plain Newtonian tide of a rigid Earth. A literature value for the gradient factor was not found (11).

**Numbers.** Amplitude after the factor 1.5×10⁻¹³ s⁻²: AION-10 3.6×10⁻³ rad, AICE 0.59 rad. The phase changes
by ≤ 1×10⁻⁶ rad per shot at AION-10 (so only averaging sees it: SNR(avg) 10⁴ for a day) and by 4×10⁻⁴ rad per
shot at AICE (13 σ per shot, visible shot to shot). Band: M2 at 2.24×10⁻⁵ Hz, S2, K1, O1 and long-period lines
(Mf, Mm, Sa), all far below the instrument's band edge and in the slow-drift regime.

**Horizontal gradients and tilt.** Tidal tilt (~10⁻⁷–10⁻⁶ rad) couples to the static field only at second order
(3×10⁻⁶ s⁻² × θ² ≈ 3×10⁻²⁰ s⁻²): nothing, to first order Γ_zx of a sphere is zero.

**Verdict.** Predictable to better than 1 % from an ephemeris, therefore a **subtraction target, not a
nuisance**: the Moon and Sun are the best-understood far-field source there is.

### 2.2 Ocean tides, attraction and loading

**Mechanism.** The ocean tide is a moving water load of order 1 m amplitude (open ocean) to several metres
(shelf seas). It attracts directly and it loads the crust, displacing the tower and the induced potential.

**Formulae (D).** For a sea of amplitude a beyond a straight coast d away, with the tower at the coast level
(Γ = −G dm/r³, ∫ over a half plane = π/d):

    Γ_zz ≈ π G ρ_w a / d.     1 m, d = 100 km: 2.2×10⁻¹² s⁻²;  d = 1000 km: 2.2×10⁻¹³;  d = 10 km: 2.2×10⁻¹¹.

This is an **upper bound**: real tides change sign across amphidromic systems, and loading deformation partly
cancels the attraction. The correct sum is the load Green's function of Farrell [R24], evaluated by convolving
a tide model over the globe. The displacement term alone is 1.45×10⁻¹² s⁻² per metre: 1 cm of loading
displacement is 1.5×10⁻¹⁴ s⁻² (AION-10 3.5×10⁻⁴ rad).

**Verdict.** At a site within ~1000 km of a tidal sea the ocean tide is **at least as large as the body tide and
usually larger** (AION-10: 5×10⁻³–5×10⁻² rad; AICE: ≥ 1 rad, beyond one fringe). Oxford is ~100 km from the nearest
tidal water (to be checked with George's site list), so this is the dominant tidal term there. It is modelled
not analytically but by Green's-function convolution of a tide model (FES2014, TPXO: §4), the same method
that gives the site-specific ocean-loading tables of Bos & Scherneck [R25].

### 2.3 Atmosphere: pressure systems, fronts, clouds, rain, soil moisture, infrasound, atmospheric Newtonian noise

**Pressure systems (D).** With a barotropic anomaly δρ = ρ(z) δp/p₀ and (E5a), at the ground:

    Γ_zz = 2πG (ρ₀/p₀) δp · kH/(1 + kH),   H = 8.4 km.

A 10 hPa anomaly with λ = 1000 km gives 2.5×10⁻¹³ s⁻² (AION-10 6×10⁻³ rad; AICE 1.0 rad) at f ~ 10⁻⁵ Hz (a system
moving at 10 m/s). λ = 300 km, 5 hPa gives 3.7×10⁻¹³. Weather is therefore **as large as the lunar and solar
tide**, and varies on days; it is the most important ambient gradient after the tides. The only data needed
is a surface pressure map. The ground-level formula is exact for flat ground and horizontal layers; the real
density anomaly depends on the temperature structure, which a surface map does not give: a 3 % boundary-layer
density change across a front (λ = 100 km, H = 1 km) gives 8.9×10⁻¹³ s⁻² (**the temperature term can exceed the
pressure term**).

**Near-field air: a consequence the scene must honour (D).** For kH ≫ 1 the ground-level formula tends to
2πGδρ and inside the layer to 4πGδρ. A 1 % change in air density within a few kilometres is therefore
5×10⁻¹² s⁻² at the ground (AION-10 0.12 rad, AICE 20 rad), 1.0×10⁻¹¹ aloft. **Below ground level the layered
contribution is exactly zero** (the gradient jumps from −4πGδρ above to 0 below), which is a real reason
underground laboratories are quiet. `reference/design/physics.md` (line 192, R) treats uniform ambient air
beyond the room as contributing nothing; that is correct for matter that surrounds the instrument on all sides
(a cavity in a uniform medium), but **not for air above a ground plane that changes density with the weather**.
The scene's air needs the ambient pressure and temperature as boundary conditions (§6).

**Clouds and rain overhead (D).** A finite cylinder of density anomaly δρ, radius a, between heights z₁ and z₂,
on the axis:

    Γ_zz = 2πG δρ [ z₂/√(a²+z₂²) − z₁/√(a²+z₁²) ]       (and 0 for an infinite slab: only edges count).

A cumulus 1 km in radius, 1.5–2.5 km up: with the water's own mass (LWC 0.5 g/m³) 2.0×10⁻¹⁴ s⁻²; with the
*thermal* anomaly of latent heating (0.5 K, δρ = 2×10⁻³ kg/m³) 8.1×10⁻¹⁴ s⁻². Condensation conserves mass, so
the water-mass figure is an upper bound and **the temperature anomaly dominates the density change**. A rain
cell 5 km in radius, 0–2 km, 4.6×10⁻⁴ kg/m³ of falling water: 7.2×10⁻¹⁴ s⁻² (AION-10 1.7×10⁻³ rad; AICE 0.28 rad),
again an upper bound (the cloud above lost the mass that arrived below). Band 10⁻³–5×10⁻⁴ Hz. Verdict: above
shot noise for tens of minutes at both instruments if the cell is overhead; not quantified beyond that.

**Soil moisture, groundwater, rain on a catchment (D).** A disc of surface density σ and radius a, tower a
height d above it:

    Γ_zz = 2πGσ a²/(a² + d²)^{3/2}  →  2πGσ/a  (d ≪ a).     An infinite sheet gives 0: only contrasts count.

10 mm of water (σ = 10 kg/m²) over 10 km: 4.2×10⁻¹³ s⁻² (AION-10 10⁻² rad, AICE 1.6 rad); 1 mm over 100 km:
4×10⁻¹⁵ (AION-10 10⁻⁴ rad). **The effect scales as 1/a, so the nearest contrast dominates**: 20 mm of rain on a
roof or lawn 100 m across, against dry ground, is 8×10⁻¹¹ s⁻² (AION-10 2 rad). Hydrology is therefore mostly
a *scene* problem (roofs, drains, soil, a river), with a far-field remainder from mapped soil moisture
(§4: ERA5-Land, GLDAS). Superconducting gravimeters see this as gravity (Creutzfeldt et al. [R39], M); a
gradiometer version was not found.

**Infrasound (D).** A plane acoustic wave of pressure amplitude δp at 0.2 Hz has k = 3.7×10⁻³ m⁻¹ ≫ 1/H, so
(E5a) gives

    Γ_zz = 2πG δp/c²   (δρ = δp/c² adiabatic):   3.6×10⁻¹⁵ s⁻² per pascal.

Phase per pascal at 0.2 Hz: 8×10⁻⁵ rad (AION-10), 5.8×10⁻³ rad (AICE). Microbaroms are of order 0.1–1 Pa near
0.2 Hz (M; the primary source is not checked): at AION-10 that is 0.16–1.6 shot noise per shot, at AICE 18–180.
This agrees in form with Creighton [R5] and Fiorucci et al. [R6], whose treatment also shows that a building
and burial suppress it and that advected temperature perturbations decay with depth (S, Harms review [R3]). The
displacement of the tower by the pressure load (≈ 0.4 mm/hPa, M, unverified) adds 6×10⁻¹⁶ s⁻² per hPa: nothing.

**Verdict.** Pressure systems and fronts: seen after averaging; closed form from a pressure map. Clouds and rain:
visible overhead, estimates one-sided. Hydrology: real, mostly near. Infrasound: marginal at AION-10, above shot
noise at AICE. These are the cheapest sources to model: **one formula, E5a, covers pressure maps, infrasound and
a global Lamb wave.**

### 2.4 Ocean microseisms and their Newtonian noise

**Mechanism.** Wave-wave interaction (Longuet-Higgins [R40], M) makes the secondary microseism near 0.1–0.3 Hz
(period 4–10 s), a Rayleigh-wave field with horizontal wavelength ~ c/f ≈ 1.5 km/s ÷ 0.2 Hz ≈ 7.5 km, much larger
than the 5–70 m instrument separation. The Newtonian gradient of a surface wave field is E5-2.

**Estimate (D, placeholder spectrum).** With Γ = γ 2πGρ k ξ_z, γ = 0.7, ρ = 2670, c = 1500 m/s, f = 0.2 Hz:
ξ_z = 10⁻⁷ m/√Hz (quiet) gives 6.6×10⁻¹⁷ s⁻²/√Hz and a phase ASD of 1.5×10⁻⁶ rad/√Hz at AION-10 (1.5 % of the
shot-noise ASD) but 1.05×10⁻⁴ at AICE (**1.05× the shot-noise ASD**). A storm-level 10⁻⁶ m/√Hz is ten times
that: 15 % at AION-10, **10.5× at AICE**. The free-air displacement term is 450× smaller (1.45×10⁻¹⁹ s⁻²/√Hz).
**The spectrum is a placeholder**: no Oxford site spectrum is in hand (M17 §3, R); replace the 10⁻⁷–10⁻⁶ m/√Hz
bracket with the Peterson noise models [R35] and then a measurement. The site's own machinery lines (30–100 Hz in
the TDR [R1]) are far above the band.

**Not modelled by this note:** ground vibration of the instrument itself, which shakes both interferometers
nearly in common and is the instrument's own isolation problem (the TDR's "nanometre-level vibrational
stability" [R1, S]). It is orders of magnitude larger than the Newtonian effect.

**Verdict.** AION-10: below shot noise (never, at the stated floor). **AICE: at or above it; the microseism
Newtonian noise is the limit to plan around**, exactly as it is for third-generation gravitational-wave
detectors [R3].

### 2.5 Teleseismic body and surface waves and their Newtonian noise

**Amplitudes.** The surface-wave magnitude relation (IASPEI standard [R36], M): Ms = log₁₀(A/T) + 1.66 log₁₀ Δ + 3.3
with A the ground amplitude in µm, T the period in s, Δ the distance in degrees. At T = 20 s and Δ = 80°: Ms 6 gives
A = 6.9 µm, Ms 7 69 µm, Ms 7.5 220 µm, Ms 8.5 2.2 mm.

**Newtonian gradient** (E5-2): for Ms 7 at 80°, Γ = 4.9×10⁻¹⁵ s⁻² (0.05 Hz, c = 3.5 km/s, γ = 0.7): AION-10
1.2×10⁻⁴ rad (2.3 shot, averaged SNR 29 over a 10-min train), AICE 1.8×10⁻² rad (SNR 4500). **Smallest Ms visible at
SNR 5 over 10 minutes at 80°: ≈ 6.2 (AION-10), ≈ 4.0 (AICE).** The 20 s Rayleigh train is sampled (cycle 2 s and
5 s) with 10 and 4 points per period.

**Body waves.** P and S arrive earlier and are smaller in displacement; their Newtonian effect in a half-space
is the bulk term (4π/3)Gρ ξ_P for an infinite medium, reduced near a free surface [R3, M]. They are not the
limit: the surface waves are bigger.

**Verdict.** A moderate teleseism is a **seen** Newtonian transient at AICE and for great quakes at AION-10.
The instrument's own shaking by the same wave (ground acceleration ξω² ≈ 2×10⁻⁵ m/s² for the Ms 7.5 amplitude of 220 µm at 20 s, against a
Newtonian differential of 1.6×10⁻¹⁴ s⁻² × 5 m = 8×10⁻¹⁴ m/s²) is **about 8 orders larger** and decides whether anything is measured: the
forward model gives the Newtonian part only.

### 2.6 Prompt elasto-gravity signals

**Mechanism.** A large rupture changes the density field everywhere at once, so gravity changes arrive at the
speed of light, before the P wave (Harms et al. [R7]; Vallée et al. [R8]; Montagner et al. [R9]; Juhel et al. [R10]).
Prompt perturbations of order 1 µGal were observed at seismometers for Tohoku (Vallée et al. [R8], S for the
claim of detection; amplitude M).

**Scaling (D, order of magnitude only).** Dimensional analysis of a point double couple of moment M₀ gives
δg ~ C G M₀/(α² r²) with α the P speed and C a dimensionless time function, **calibrated here to ~1 µGal for
Mw 9 at 1000 km** (C ≈ 0.1; this is not a derivation). Gradient ~ δg / r.

| Event | δg | Γ | AION-10 | AICE | Window |
|---|---|---|---|---|---|
| Mw 9, 1000 km | 4×10⁻⁹ m/s² | 4×10⁻¹⁵ s⁻² | 1.0×10⁻⁴ rad | 1.6×10⁻² rad | 125 s |
| Mw 7, 100 km | 4×10⁻¹⁰ | 4×10⁻¹⁵ | 1.0×10⁻⁴ | 1.6×10⁻² | 12 s |
| Mw 7, 300 km | 5×10⁻¹¹ | 1.5×10⁻¹⁶ | 3.7×10⁻⁶ | 6×10⁻⁴ | 38 s |
| Mw 8, 1000 km | 1×10⁻¹⁰ | 1.3×10⁻¹⁶ | 3×10⁻⁶ | 5×10⁻⁴ | 125 s |

**Verdict.** AION-10: only a great earthquake within ~1000 km (a 2-shot signal over ~60 shots). AICE: Mw 7 within
~300 km at SNR ~40. **The prompt window is tens of seconds; inertia of the ground (not modelled) acts on the instrument
in the same window.** Validate against the published normal-mode simulation (Juhel [R10]) before trusting any C.

### 2.7 Tsunamis

**Mechanism and formula (D).** A tsunami moves water mass; treating a patch as a mass m = ρ_w a A at distance d,
Γ = 2Gm/d³, plus loading (small at these amplitudes, ~mm). 0.5 m over 10⁴ km² at 500 km: 5.5×10⁻¹⁵ s⁻² (AION-10
1.3×10⁻⁴ rad, 2.6 shot; AICE 2×10⁻² rad); at 150 km 2×10⁻¹³ (AION-10 4.9×10⁻³, AICE 0.8); 2 m over 2500 km² at 100 km
6.8×10⁻¹³. Period 10–60 min, f ~ 10⁻³ Hz.

**Verdict.** Above shot noise within a few hundred km of a coast during a tsunami; **nothing inland and nothing
at the middle of an ocean basin for a site like Oxford**, where the realistic tsunami source is remote and rare.
Honest answer for the stated instruments: a curiosity unless the AICE site is coastal (not in hand; ask George).

### 2.8 A global atmospheric Lamb wave (Hunga Tonga, 2022)

**Source facts (S, snippets only).** The Matoza et al. supplement [R11] and a GRL study report a Lamb wave of
"several hundred pascals" near the volcano, circling the globe five to six times over 15–19 January 2022; Wright et
al. [R12, M] is the primary Lamb-wave paper. The global amplitude was of order 100 Pa at far stations (M, **not
checked**).

**Formula (D).** A Lamb wave is a horizontally propagating pressure pulse, λ ≈ c × period = 310 m/s × 1 h ≈ 1100 km,
k = 5.7×10⁻⁶ m⁻¹, kH = 0.048, so (E5a) gives Γ = 2πG(δp/c²) kH/(1+kH): 100 Pa gives 2.0×10⁻¹⁴ s⁻² (AION-10
4.8×10⁻⁴ rad, 9.6 shot; AICE 7.9×10⁻² rad); loading adds 6×10⁻¹⁶ s⁻² (3 % of the Newtonian term). At f ≈ 3×10⁻⁴ Hz.

**Verdict.** A Hunga-like wave is above shot noise in principle at both instruments and trivially simulated
(one plane wave, E5a). Whether it is above the instrument's *technical* noise at 3×10⁻⁴ Hz is unknown to me. It is
**the cleanest published event to validate the atmosphere path against**, because the pressure record is public.

### 2.9 Earth's free oscillations

**Mechanism.** After a great earthquake the Earth rings in modes whose lowest members have periods of 54 min (₀S₂),
20 min (₀S₀), 3×10⁻⁴ Hz band (Dahlen & Tromp, Masters & Widmer, M). Only the free-air term is estimated here:
Γ = 1.45×10⁻¹² s⁻² per metre of vertical surface displacement. 1 mm ringing: 1.45×10⁻¹⁵ s⁻² (AION-10 3.5×10⁻⁵ rad,
0.7 shot; AICE 5.7×10⁻³); 0.1 mm: 3.5×10⁻⁶. The mode's own potential adds order-one factors not computed.

**Verdict.** AION-10: after averaging for about a day in white noise, **never in practice** (below technical
noise at 3×10⁻⁴ Hz and decaying). AICE: above shot noise for hours after a great quake, again before technical noise
is considered. Not worth a model beyond a damped sinusoid.

### 2.10 Trains, traffic and wind beyond the scene

**Point masses (D).** Γ = 2Gm/r³ on axis (worst case). A 30 t lorry: 500 m, 3.2×10⁻¹⁴ s⁻² (AION-10 7.7×10⁻⁴ rad,
15 shot; AICE 0.12 rad); 1 km 4×10⁻¹⁵ (1×10⁻⁴ rad, 1.9 shot; AICE 0.015); 5 km 3×10⁻¹⁷ (AION-10 nothing; AICE 1.2×10⁻⁴).
A 400 t train: 500 m 4.3×10⁻¹³ (AION-10 1.0×10⁻² rad; AICE 1.6 rad), 2 km 6.7×10⁻¹⁵ (1.6×10⁻⁴, 2.5×10⁻²).

**The scene-radius rule (D).** The distance at which a source of mass m falls below a phase tolerance ε:

    r_s = (2 G m C / ε)^{1/3},     C = k_eff T² Δr.

    lorry 30 t:   ε = 1e-6 rad: AION-10 4.6 km, AICE 25 km;   ε = 5e-5 rad: 1.2 km, 6.8 km
    train 400 t:  ε = 1e-6 rad: AION-10 10.9 km, AICE 59 km;  ε = 5e-5 rad: 2.9 km, 16 km
    car 1.5 t:    ε = 1e-6 rad: AION-10 1.7 km, AICE 9.2 km;   ε = 5e-5 rad: 0.46 km, 2.5 km

At the plan's µrad-level budget a lorry has to be a real body to **~5 km (AION-10) and ~25 km (AICE)**, a town
and then some. Above 5×10⁻⁵ rad it is 1.2 and 6.8 km.

**Many vehicles beyond the radius (D, Campbell's theorem).** A Poisson stream of identical masses along a
straight road at perpendicular distance d, line density ν (vehicles/m) gives, with Γ = 2Gm/r³,

    var Γ_zz = ν m² · 4G² · (3π / 8 d⁵),          (∫ dx/(d²+x²)³ = 3π/8d⁵).

ν = 0.01/m (10 per km), m = 1.5 t: d = 200 m rms 3.8×10⁻¹⁴ s⁻² (AION-10 9×10⁻⁴ rad, 18 shot), d = 1 km 6.9×10⁻¹⁶
(1.6×10⁻⁵ rad, 0.33 shot; AICE 2.6×10⁻³). The variance falls as d⁻⁵: **the far traffic is a stochastic,
statistically exact, cheap source**, to be drawn per seed and per road, not voxelised.

**Wind on buildings and trees.** A building of mass M swaying a distance x changes the gradient by ~6GMx/d⁴ (D):
1×10⁷ kg and 1 mm at 100 m: 4×10⁻¹⁴ s⁻² (AION-10 6×10⁻⁴ rad, 12 shot); at 1000 m 4×10⁻¹⁸ (6×10⁻⁸ rad).
At 300 m the same building gives 5×10⁻¹⁶ s⁻² (AION-10 1×10⁻⁵ rad, 0.24 shot; AICE 2×10⁻³ rad). **Nothing would be seen beyond about 1 km** (AICE 2×10⁻⁵ rad at 1 km); inside it is the scene's modal-synthesis job (S7).

### 2.11 The far side of the planet

A static mass change of 10¹² kg (a gigatonne) at 1.2×10⁷ m produces 2Gm/d³ = 8×10⁻²⁰ s⁻² (AION-10 2×10⁻⁹ rad): nothing, at any
conceivable mass. **Only things that travel reach the instrument from there**: seismic and acoustic waves
(§2.5, §2.8, §2.9), tides (§2.1–2.2) and the ocean's response. An event "on the other side" is simulated by the
wave that arrives, never by its static field.

---

## 3. Models cheap enough to live outside the voxels

Common form: a **far source is an analytic or tabulated function of time that supplies a Taylor (regular solid
harmonic) expansion of its external potential about the tower**, which enters the forward model as an ambient
potential on the same arms the scene uses (the `Channel::Ambient` of `reference/design/world.md`, R). The propagation
integral then handles the finite arm separation exactly and the time variation over the 2T flight.

| Source | Cheap model | Inputs |
|---|---|---|
| Moon, Sun | exact far point mass (not a quadratic expansion): Γ = (GM/d³)(3 n nᵀ − I), times the Love factor 0.97; includes the l = 3 term (R/d = 1.7 %) | analytic ephemeris (Meeus ch. 47 and VSOP87 truncated [R45], M; DE440s as oracle [R34]); Earth rotation angle |
| Solid-Earth response | frequency-independent h₂, k₂ (IERS nominal) at first; the diurnal resonance (FCN) correction later | IERS 2010 ch. 7 [R22] |
| Ocean tide and loading | Green's-function convolution of a tide model, tabulated per site (≤ kB) or computed once | FES/TPXO + Farrell Green's functions [R24, R30]; Bos–Scherneck site tables [R25] |
| Pressure systems and fronts | (E5a) per Fourier mode, or direct integral over a map; parametric Gaussian highs/lows for scenes | a surface pressure map; boundary-layer temperature anomaly optional |
| Clouds and rain | finite cylinders (§2.3), per scene as moving bodies with density anomaly; parametric | position, radius, δρ |
| Soil moisture and hydrology | disc/sheet formula; far part from a mapped field, near part voxelised | ERA5-Land / GLDAS; scene |
| Infrasound, Lamb wave | plane acoustic wave, Γ = 2πG δp/c² (k ≫ 1/H) or E5a | amplitude, frequency, direction, speed |
| Microseisms (S12, M17-R5) | Rayleigh plane-wave superposition with seeded phases; Γ = γ 2πGρ k ξ_z | a displacement spectrum (placeholder → Peterson [R35] → site) |
| Teleseismic arrivals | a few named events: surface-wave trains from a dispersion model (Ms relation for amplitude; PREM/ak135 group velocities [R26, R27]); Newtonian part via the Rayleigh formula | Global CMT source [R33]; travel-time tables |
| Prompt elasto-gravity | closed-form homogeneous-medium solution of Harms et al. [R7] as a time series; normal modes (QSSP [R37]) as oracle | source time function |
| Tsunami | moving sheet, Γ = 2Gm/d³ summed over a shallow-water patch; loading by Green's function | a tsunami scenario |
| Free oscillations | sum of damped sinusoids from a mode table | mode periods, Q, amplitudes |
| Far traffic and rail | Campbell noise for roads; rails as a rare Poisson process of heavy bodies | line densities, masses |

**How they share the forward model and the error budget.** (1) *One channel.* Each far layer returns
ΔΦ_far(t) on the ambient channel, so the decomposition `signal = targets + ambient` holds unchanged
and a Gradar dataset can switch layers on and off per scene. (2) *One budget, in radians.* Each layer reports
(a) a **truncation bound** (the omitted multipole order: relative error ~ (L/d)^{ℓmax−1} with L the arm excursion
≤ 10 m (AION-10) or ≤ 100 m (AICE), so a quadratic expansion is good to far below 10⁻⁹ for the Moon, ~10⁻⁸ (cubic order) for a 1000-km
weather field at AICE, and only ~1 % (cubic order) for a source 1 km away at AICE), (b) a **model bound** (barotropic assumption, γ in the Rayleigh formula,
spectrum placeholder), (c) a **data bound**, converted to rad by C. The bundle records them beside the tree-walk
error. (3) *Honest labelling*: any layer whose model bound exceeds its signal is flagged "placeholder" in the
manifest, as M17 requires for the seismic spectrum.

**Hand-over to the voxel scene.** The principle is that a source is voxel scene or far layer, never both, and
the boundary is set by the **phase tolerance, not by distance alone**.

1. *Scene radius per class* by r_s = (2GmC/ε)^{1/3} (§2.10): masses with a well-defined body (vehicles, trains,
   buildings) are bodies inside r_s and Campbell noise or nothing outside it.
2. *Extended fields* (air, ground water): the scene owns a cylinder of radius R_s about the instrument; the
   far layer supplies the field **outside** it. For a layered field the far contribution is the full formula minus the
   cylinder formula of §2.3, evaluated analytically, so nothing is counted twice and nothing dropped.
3. *Boundary conditions both ways.* The far layer gives the scene its ambient pressure p(t), temperature T(t)
   and, if it has them, moisture and wind (the S8 air's boundary values). The scene gives nothing back (the
   instrument is a passive reader of the far field).
4. *Test the seam.* A **seam test**: the same field computed fully voxelised to R_far (slow) and as scene + far layer
   must agree to a stated tolerance, with a control arm that drops the cylinder subtraction and must fail by the
   expected amount.

---

## 4. Real planet data (owner's addition (a))

**What serves a simulation of the 100 m tower in the environment of the whole planet plus Moon and Sun.** The
table gives, for each dataset: what it is for, resolution, download size, licence and redistribution terms
(**reported only; the owner decides**), whether a first version needs it, and caching. Sizes are the
smallest practical product, not the largest; those marked ◇ are my arithmetic or memory, not checked.
Licences marked ✔ were read in a search result this session (summary only); the rest are from memory.

| Dataset | Role | Resolution / content | Size | Licence and redistribution | First version? |
|---|---|---|---|---|---|
| **PREM** [R26] | radial density, elastic moduli, gravity; Love numbers, Green's functions, normal modes, free oscillations | 1-D, ~ 10² layers | < 1 MB table ◇ (IRIS EMC hosts it) | published model; EMC terms to be read | **No** (Love numbers nominal); v2 loading/seismic |
| **ak135 / ak135-f** [R27] | body-wave travel times, group-velocity base | 1-D | < 1 MB ◇ | free for research (to confirm) | No; v2 seismic arrival times |
| **CRUST1.0** [R28] | crustal density and velocity: local Rayleigh dispersion, near-site structure | 1°×1°, 9 layers | ~10 MB ◇ | free; terms to confirm | No; optional for site structure |
| **ETOPO 2022** [R32] | topography/bathymetry: coast distance, ocean Green's-function masks | 15″–60″ | 60″ ≈ 0.5 GB, 15″ tens of GB ◇ | NOAA public domain (M) | No |
| **GEBCO 2024** [R32] | same, bathymetry-led | 15″ (43200×86400) | 7.5 GB as int16 raw ◇ (arithmetic) | **✔ "public domain, free; users must acknowledge the source"** (GEBCO terms page); a BODC notice also states copyright/database rights, which I could not reconcile | No |
| **EGM2008** [R29] (or EIGEN-6C4/XGM2019e) | static geoid/gravity, local anomalies; the static gradient is a constant in the engine | degree and order 2190 | ~2.4M coefficient lines ◇ (≈100–200 MB text, arithmetic) | free via ICGEM (M) | **No**: static field |
| **FES2014 / FES2022** [R30] | global ocean tide (34 constituents) → loading, attraction | 1/16° | several GB ◇ | **✔ AVISO "for all purposes (scientific, commercial…) — see licence agreement"**; registration required | No; v2 ocean tide |
| **TPXO9-atlas** [R30] | alternative global tide | 1/30° | GB ◇ | **✔ copyright Oregon State University; use under a user agreement accepted by use**; the agreement's terms not read | No; optional cross-check |
| **ERA5 surface pressure, 2 m temperature** [R31] | pressure-system Newtonian gradient and scene boundary conditions | 0.25°, hourly, 1440×721 | ≈2 MB per field in 16-bit GRIB ◇; one month hourly ≈1.5 GB | Copernicus licence; **CC-BY 4.0 (M, unverified)** | No; v2: a week around a storm |
| **ERA5-Land, GLDAS-2.1, GRACE-FO** [R31, M] | soil moisture / total water: far hydrology | 0.1°, 0.25°, 0.5° | GB per year ◇ | CC-BY (ERA5-Land, M); NASA open (M) | No; optional |
| **Global CMT** [R33] | moment tensors, hypocentres, 1976– | ~60k events | ~100 MB ◇ (ndk text) | free with citation (M) | No; a handful of named events in v2 |
| **USGS ComCat** | catalogue, finite faults | events | small | US Government, public domain (M) | No |
| **JPL DE440 / DE440s** [R34] | ephemerides | Chebyshev, 1550–2650 | **DE440 ≈ 114 MB; DE440s ≈ 17 or 33 MB: sources conflict (✔)** | NASA/NAIF kernels, free (terms not read) | **No** as a dependency; **yes as an oracle download** |
| **Analytic series (Meeus / VSOP87 / ELP truncated)** | ephemeris without data | ~100 terms | embedded, ~kB | published; free | **Yes** (§8) |
| **IERS EOP C04, leap seconds** [R22] | UT1−UTC, polar motion | daily | ~MB | IERS public (M) | UT1 optional; AICE needs it (§5.5) |
| **Tidal catalogues** (HW95, Tamura) [R23] | tide-generating potential | 10³–10⁴ waves | kB–MB | published tables | No (ephemeris instead) |
| **IGETS superconducting gravimeter records** [R46] | validation of tides, pressure, loading in gravity | 1 s–1 min | small per station | open with citation (M) | oracle only |
| **EOST loading service / Bos–Scherneck BLQ** [R25] | precomputed loading and tide series per site | per site | kB | free (M) | validation only |

**What a first version needs.** Nothing downloaded, for the recommendation of §8. The ephemeris is a short embedded
analytic series; Love numbers are IERS nominal values; the pressure field is parametric. **One data download
only, as an oracle:** `de440s.bsp` (and optionally one IGETS record) to bound the series' error.
**What is optional (v2).** ERA5 surface fields for a chosen week (the largest realistic first import: ~340 MB for a
week of hourly pressure and temperature); FES2014 for an ocean-tide scene; Global CMT events by name.

**Caching outside the repository (proposal).** A single data root, `$CAVENDISH_DATA`, defaulting to
`$XDG_CACHE_HOME/cavendish` and set by the user for large disks. Every dataset is fetched by a named command (`cav data
fetch <name>`), stored under `<root>/<name>/<version>/`, with a sidecar `manifest.json` recording the source URL,
licence text, retrieval date, byte size and SHA-256. Derived products (regridded pressure series, tabulated Green's
functions, site tide tables) are cached by the hash of their inputs and parameters, so a bundle's manifest
(M16) can name them and `verify` can re-derive them. Nothing under the data root is committed; only the small
manifests and the licence notes (`docs/data/licences.md`) are. A dataset whose terms forbid redistribution is
fetched on the user's machine from the publisher and never mirrored into a bundle. The sandbox this note was
written in could not reach any of these hosts, so no size or licence above was confirmed on the publisher's page
except where marked ✔.

---

## 5. The rotating frame (owner's addition (b))

Earth's rotation Ω = 7.292115×10⁻⁵ rad/s. In a frame fixed to the tower the atom obeys

    r̈ = g(r) − 2 Ω × ṙ − Ω × (Ω × r) + (tide, sources),

with the centrifugal term absorbed into the measured g and the static gradient (Ω² = 5.3×10⁻⁹ s⁻², about 5 E
against Earth's 3086 E: a constant, in the engine's config constant γ_zz). At latitude 51.76° (Oxford; the AICE
site is unknown): Ω_north = 4.51×10⁻⁵, Ω_up = 5.73×10⁻⁵ rad/s, Coriolis parameter f = 2Ω sin φ = 1.145×10⁻⁴ s⁻¹.

### 5.1 The Coriolis phase and the Sagnac phase are one term

For a light-pulse interferometer with wave vector k_eff and initial velocity v₀ the rotation phase is
Φ_rot = 2 k_eff · (Ω × v₀) T² [R15, R16, R17, M]: the Coriolis acceleration −2Ω × v₀ projected on k_eff. Equivalently
it is the Sagnac phase 2mΩ·A/ħ of the interferometer's loop area A = (ħk/m) T × v_⊥ T. **These are the same
quantity, not two terms.** (The statement that the "Sagnac" and Coriolis expressions coincide is the standard
treatment in the Varenna lectures of Hogan et al. [R17], M; read before quoting.) For a vertical k_eff and a horizontal
launch velocity v_E (east–west) the vertical component of Ω × v is Ω_north v_E:

    Φ = 2 k_eff Ω cos φ v_E T²   →   4.33×10⁵ rad per m/s (AION-10),  5.08×10⁶ rad per m/s (AICE).

The two interferometers of a gradiometer share lasers and launch, so the common v_E cancels. **The residue is the
mismatch Δv_E**: one shot's noise needs Δv_E < 1.15×10⁻¹⁰ m/s (AION-10) or 6.2×10⁻¹² m/s (AICE). **Real
instruments therefore do not rely on mismatch control: they compensate.** In the Berkeley fountain Lan et al.
[R13, S] tilt the retroreflecting mirror on piezo actuators so that the momentum transfer k₁ − k₂ keeps a fixed
direction in the inertial frame, which recovers fringe contrast (the paper's wave packets were separated by up to
8.8 mm; the contrast gain is quoted at up to 350 %). The optimal rotation rate is minus the horizontal
projection of Earth's rotation (S, from the gradiometer sensitivity paper [R14]). Terrestrial very-long-baseline
interferometers use the same tip-tilt compensation (Workshop summary [R20], S). Launch-velocity correction by
imaging is an alternative (Dickerson et al. [R18], M).

### 5.2 Coriolis moves the atoms, and the near field notices

Free fall for 2T from u₀ (AION-10 3.86 m/s, 1.46 s; AICE 21.27 m/s, 5 s) in the east–west direction accelerates at
−2Ω_north v_z, so the displacement is x = −2Ω cos φ ∫ (z − z₀) dt (D): **+0.088 mm (AION-10) and −5.55 mm (AICE)**,
with a final east speed of 0.44 mm/s and 1.5 mm/s. A source at distance r changes its phase by ~3 δx/r, i.e.
**2.6×10⁻⁴ relative at 1 m (AION-10) and 1.7×10⁻² (AICE)**: far above the engine's 10⁻⁹ tie to v1 (which has
no Coriolis). So: **the arm trajectories get an optional first-order Coriolis (and gradient-of-centrifugal)
correction** (closed-form polynomials in t); the oracle ties are run with it off; with it on, the check is the
analytic first-order displacement above. For sources beyond ~100 m it is negligible (3δx/r ≈ 3×10⁻⁵ at AION-10).

### 5.3 Gradient–Coriolis cross terms

The Coriolis displacement δx interacts with horizontal gradients of the vertical acceleration: Φ ≈ k_eff T² Γ_zx δx.
Earth's own Γ_zx is zero for a sphere, so this matters near buildings and slabs (the scene's job, already handled
by 5.2). Asenbaum et al. [R19, M] and Dimopoulos et al. [R16, M] give the published expressions for the phase
shifts of gradients and rotation (including the familiar k_eff Γ_zz T²(z₀ + v₀T + 7/12 gT²) of Peters et al. [R15,
M], whose gradiometer difference is the ΔΦ of (E1)). **Not verified here.**

### 5.4 Earth-rotation compensation, as an instrument parameter

Model it, do not try to suppress it: a parameter "residual rotation-compensation error ΔΩ_⊥" (rad/s) so that the
residual phase is 2 k_eff ΔΩ_⊥ × (v₀ + tilt-induced displacement) T², zero by default and swept in studies. The
mirror's tilt is an ambient additive term; this keeps the rotating frame out of every voxel computation.

### 5.5 The reference-frame chain, and what each link costs

ICRS (ephemeris) → GCRS → CIRS (precession-nutation, IAU 2006/2000A) → TIRS (Earth rotation angle, UT1) → ITRS
(polar motion) → local east-north-up at the tower (latitude, longitude, height) → the tower frame (azimuth, plumb tilt).
For tide direction the cost of skipping a link, as a relative error of the tide (D):

| Link | Size | Effect |
|---|---|---|
| Precession (50″/yr; 26 years from J2000 ≈ 1300″ ≈ 6×10⁻³ rad) | needed | 1 % of tide if omitted |
| Nutation (≤ 17″ ≈ 8×10⁻⁵ rad) | optional | ≤ 2×10⁻⁴ of tide: below AION-10 shot noise, **~4 σ at AICE** (0.59 rad × 2×10⁻⁴ = 1.2×10⁻⁴ rad) |
| UT1 − UTC (≤ 0.9 s → Ω × 0.9 s = 6.6×10⁻⁵ rad) | optional | 8×10⁻⁵ rad at AICE (2.5 σ); 5×10⁻⁷ at AION-10 |
| Polar motion (0.3″ = 1.5×10⁻⁶ rad) | skip | 10⁻⁶ of tide |
| Local geodetic vs geocentric latitude (up to 0.19° = 3×10⁻³ rad) | needed | |

### 5.6 Length of day, polar motion and the pole tide as signals

LOD ±1 ms/day: ΔΩ = 8.4×10⁻¹³ rad/s (1.2×10⁻⁸ relative); through the Coriolis term with a 1 mm/s east-velocity
mismatch it is 5×10⁻⁶ rad (AION-10): **negligible**. Polar motion of 0.3″ changes the centrifugal gravity by
~10⁻⁷ m/s² (a common-mode shift the gradiometer difference removes). The **pole tide** (IERS, vertical
displacement up to ~10 mm [R22, M]) works through the free-air term: 1.45×10⁻¹⁴ s⁻², AION-10 3.5×10⁻⁴ rad, AICE
0.06 rad at the Chandler period of 433 days, **indistinguishable from drift**. Neither is worth modelling.

### 5.7 Coriolis on the air and water in the scene

Rossby number Ro = U/(f L), f = 1.145×10⁻⁴ s⁻¹ (D; Coriolis/inertia = 1/Ro):

| Flow | U, L | Ro | Coriolis/inertia | Matters? |
|---|---|---|---|---|
| room (radiator plume) | 0.1 m/s, 5 m | 175 | 0.6 % | no |
| building | 1 m/s, 50 m | 175 | 0.6 % | no |
| town wind | 5 m/s, 5 km | 8.7 | 11 % | yes, weakly |
| sea breeze | 5 m/s, 50 km | 0.87 | 115 % | yes |
| river, harbour | 1 m/s, 1 km | 8.7 | 11 % | weakly |
| synoptic system | 10 m/s, 1000 km | 0.087 | 11× | geostrophic |

The inertial period is 15.2 h, 2.4 h to turn a 1 m/s flow by 1 rad. Against buoyancy a room plume's Coriolis force
is 2Ωu/(gΔT/T) ≈ 3×10⁻⁵ for u = 0.3 m/s, ΔT = 30 K. **So: no Coriolis in the S8 room and building
solvers; an optional −2Ω × u body force for a town-scale or water-body scene; and for weather the far layer
takes the pressure map as given** (it already is a geostrophic product).

### 5.8 What matters, at a glance

| Effect | AION-10 | AICE |
|---|---|---|
| Coriolis/Sagnac residual via Δv_E | **matters**: 0.43 rad per µm/s: must be compensated | **matters**: 5 rad per µm/s |
| Coriolis displacement of the atoms | 0.09 mm: matters only for sources < ~10 m | 5.6 mm: matters for sources < ~100 m; build the option |
| Earth orientation for the tide (UT1, nutation) | no | marginally (2–3 σ), include UT1 |
| LOD, polar motion | no | no |
| Pole tide | no | drift |
| Coriolis on scene air and water | no (rooms, buildings) | town scale only |

---

## 6. Validation

Each case follows the repository's rule: a pass criterion written before measuring, a control arm that fails when
the property is broken, and no moving of a tolerance to pass.

| Case | Asserts | Against | Control arm |
|---|---|---|---|
| `tide_direct_closed_form` | far point-mass gradient = GM/d³(3 n nᵀ − I); phase = C Γ_zz for a uniform tensor, through the existing forward model | closed form; ≤ 1e-9 relative | a Moon at 3.8e6 m (1 % of true distance) must violate the quadratic approximation by the predicted (L/d)² |
| `tide_ephemeris_vs_de440s` | the analytic Moon/Sun direction and distance vs DE440s over 30 years; the phase difference in rad | DE440s [R34]; the tolerance written from the budget (≤ 1e-6 rad AION-10) | truncating the lunar series to 5 terms must fail |
| `tide_love_factor` | 1 + 6k₂ − 3h₂ reproduces 1 + h₂ − 3k₂/2 when the same derivation is run on gravity (self-check), and matches a published gradient tide if one is found | closed form + literature (§11) | h₂ → 0 changes the answer by 1.8 |
| `tide_vs_standard_software` | the Newtonian tidal *potential/gravity* from the engine's ephemeris vs ETERNA/PyGTide and the IERS Conventions routine for dates and sites | ETERNA [R23]; ≤ 1e-9 m/s² and the l = 2 gravity at fixed (h₂, k₂) | dropping the Sun fails |
| `tide_vs_superconducting_gravimeter` | gravity tide plus the standard pressure admittance and ocean loading vs an IGETS station record over a month | IGETS [R46]; residual stated | wrong latitude fails |
| `pressure_planewave_closed_form` | E5a for λ = 10 m… 10⁴ km vs direct numerical integration of the density field in the voxel/point engine at a stated truncation | brute-force sum; ≤ 1e-4 | k → ∞ must return 2πGδρ; k → 0 must return 0 |
| `pressure_admittance` | the same machinery's *gravity* (not gradient) per hPa plus loading vs the standard ≈ −0.3 µGal/hPa [R38, M] | Merriam, Boy et al., IGETS records | omitting loading changes the sign of the admittance |
| `infrasound_closed_form` | 2πGδp/c² vs the engine on a voxelised plane wave | closed form | rigid-wall boundary changes it by the stated factor |
| `lamb_hunga_replay` | the published Hunga pressure record (public barometers) through E5a; the phase amplitude within a stated factor; the surface-to-space note | Matoza [R11], Wright [R12] | a wavelength 10× off fails |
| `cylinder_cloud_closed_form`, `disc_soil_moisture_closed_form` | §2.3 formulas vs the engine's cylinder/disc kernels (S3) | closed form ≤ 1e-9 | an infinite slab must give 0 |
| `rayleigh_plane_wave_reference` | M17's test, on the primary formula [R3, R4] | the reference's closed form | γ perturbed fails |
| `seismic_noise_vs_gw_models` | the Newtonian-noise PSD for a stated site against published Newtonian-noise budgets for gravitational-wave detectors | Harms [R3]; Fiorucci [R6] | spectrum × 10 fails |
| `pegs_replay_tohoku` | prompt-signal time series for Tohoku at stated stations vs published observations and the normal-mode simulation | Vallée [R8], Juhel [R10], QSSP [R37] | C changed by 3× fails |
| `coriolis_displacement_closed_form` | the engine's Coriolis-corrected arm = x(t) = −2Ω cos φ ∫(z − z₀) dt, to first order | closed form ≤ 1e-6 relative (O(ΩT)) | rotation off, or sign flipped, fails |
| `rotation_phase_2k_omega_v_t2` | with a horizontal launch-velocity difference, ΔΦ = 2k_eff Ω_⊥ Δv T² | Peters [R15], Dimopoulos [R16], Hogan [R17] | compensation on makes it fall to the stated residual |
| `lan_compensation_replay` | contrast vs mirror rotation rate for the published parameters (2n = 10, T = 250 ms, latitude 37.87°) | Lan et al. [R13] | a wrong latitude fails |
| `scene_radius_rule` | the r_s formula against brute-force summation for a lorry on a road, within 10 % | engine | a doubled mass fails |
| `seam_voxel_vs_far` | §3 item 4: a field voxelised to R_far vs scene + far layer, within stated tolerance | the engine's own two paths | removing the cylinder subtraction fails |
| `unified_budget_reports_far_terms` | every far layer's bound is in the bundle in radians | structural | a missing bound fails |

---

## 7. Open method questions this leaves

1. The primary formula for Rayleigh-wave Newtonian gradient (γ in E5-2) and its dependence on depth of the
   instrument (the TDR's 15 m-deep stairwell [R1, R2]); take it from Harms [R3].
2. A literature value for the **gradient** tidal factor (§2.1).
3. Coherence of ocean tides near the site: whether the half-plane bound of §2.2 is a factor of 2 or 20 too high.
4. The size of the *cancellation* in rain and cloud effects (§2.3).
5. AICE's site, depth, separation and kick number (§1).
6. The instrument's common-mode rejection for uniform fields, the technical noise at 10⁻⁵–10⁻³ Hz and the
   vibration-isolation transfer function: without them, "above shot noise" says less about what is measured.
7. The measured Oxford microseism spectrum (M17 §3's open item).

---

## 8. Recommendation: a first version small enough for one slice

**"S14: the ambient far field — tides, weather, far traffic, rotation."** No data files, no network, no GPU.

**Build (about a thousand lines and their tests):**
1. **An ambient far-field layer** with one interface: given a time, return a regular-solid-harmonic expansion about
   the tower (order ≤ 3) in the tower frame, so it plugs into the forward model as an ambient potential. The existing
   `Channel::Ambient` and the propagation-integral socket are the hooks (`reference/design/world.md`, R).
2. **Moon and Sun** as exact far point masses from an embedded truncated analytic ephemeris, the factor 1 + 6k₂ − 3h₂,
   precession and Earth rotation angle (nutation and polar motion off; UT1 optional). Latitude, longitude and azimuth
   in the scene spec. *Demo:* a day of the AICE signal with the tide visible, a month with the spring–neap beat.
3. **Atmosphere** by E5a: parametric pressure systems (Gaussians, fronts, plane waves) and a plane acoustic wave;
   covers pressure, infrasound and a Hunga-like Lamb wave in one formula. *Demo:* a front passing, a Lamb wave.
4. **The scene-radius rule** r_s = (2GmC/ε)^{1/3} shown in the viewer as a ring per instrument and per source
   class, and **Campbell noise for far roads and rails** (§2.10). *Demo:* shrink the ring, watch the phase error
   bound grow.
5. **Rotation**: latitude → Ω; an optional first-order Coriolis correction to the arm trajectories with its error
   budget; a tilt-compensation parameter; the Δv_E sensitivity shown in the error ledger.
6. The §6 cases that need no download: the tide closed form, the plane-wave and cylinder closed forms, Coriolis,
   the scene-radius rule, the seam test. The DE440s and IGETS comparisons are **oracle-tier** (one-off downloads
   cached outside the repo).

**Leave out, in order of when they earn a slice:** (i) ocean tide and loading from FES/TPXO with Farrell Green's
functions (the largest missing tide piece near a sea); (ii) ERA5 import and soil moisture; (iii) the microseism
field (already S12's R5) and the Newtonian-noise budget; (iv) a teleseismic and prompt-gravity layer with
normal modes (a research slice on its own); (v) tsunami and free oscillations (parametric only, on request);
(vi) the voxel scene's weather boundary conditions (after S8–S10 exist). Put the rest in `plan/IDEAS.md`.

**Calls worth a second look at the checkpoint.** (a) That the far layer is a regular-harmonic *potential* and not a
list of point masses; (b) the Love-number factor and its derivation; (c) including the optional Coriolis arm
correction when the plan's oracle (v1) has none; (d) the treatment of uniform ambient air above ground against
`reference/design/physics.md` line 192.

---

## 9. What this note does not model

Technical noise of any kind (laser phase, vibration, tilt, magnetic, thermal); the instrument's common-mode
rejection of uniform fields; phase wrapping; the shaking of the instrument by seismic waves; and the 3-D structure
of the real atmosphere beyond a layered, barotropic model.

---

## 10. References and their status

**S** = abstract/snippet read in a web search this session; **M** = from memory, unchecked; **R** = from this repository.

- **R1** AION Collaboration, *AION-10: Technical Design Report for a 10 m Atom Interferometer in Oxford*, [arXiv:2508.03491](https://arxiv.org/abs/2508.03491). **S** (abstract; no numbers read).
- **R2** Carlton, McCabe et al., *From RATs to riches*, [arXiv:2308.10731](https://arxiv.org/abs/2308.10731). **R** (via `reference/milestones/M17-site-and-baseline.md`).
- **R3** Harms, *Terrestrial gravity fluctuations*, Living Rev. Relativ. 18, 3 (2015), [arXiv:1507.05850](https://arxiv.org/abs/1507.05850), [doi:10.1007/lrr-2015-3](https://doi.org/10.1007/lrr-2015-3). **S** (summary: strain-noise forms for infrasound and temperature; temperature NN falls with depth; atmospheric NN insignificant for advanced LIGO/Virgo).
- **R4** Hughes & Thorne, Phys. Rev. D 58, 122002 (1998), [doi:10.1103/PhysRevD.58.122002](https://doi.org/10.1103/PhysRevD.58.122002). **M**.
- **R5** Creighton, atmospheric Newtonian noise (title not verified), Class. Quantum Grav. 25, 125011 (2008), [doi:10.1088/0264-9381/25/12/125011](https://doi.org/10.1088/0264-9381/25/12/125011). **S** (citation and its use only; title from memory).
- **R6** Fiorucci, Harms, Barsuglia, Fiori, Paoletti, *Impact of infrasound atmospheric noise on gravity detectors…*, [arXiv:1801.04564](https://arxiv.org/abs/1801.04564). **S**.
- **R7** Harms, Ampuero et al., *Transient gravity perturbations induced by earthquake rupture*, Geophys. J. Int. (2015), [Caltech record](https://authors.library.caltech.edu/records/hs6t9-kcm59), [IPGP PDF](https://www.ipgp.fr/~jpm/PUBLICATIONS/15Harms-GJI-grav_Theory.pdf). **S** (abstract: homogeneous infinite non-self-gravitating double couple, adequate for half-space finite faults).
- **R8** Vallée et al., *Observations and modeling of the elastogravity signals preceding direct seismic waves*, Science 358, 1164 (2017), [IPGP PDF](https://www.ipgp.fr/~vallee/PUBLICATIONS/2017_Vallee_et_al_PromptElastoGravity_Science_AcceptedVersion_and_Suppmat.pdf). **S**.
- **R9** Montagner et al., *Prompt gravity signal induced by the 2011 Tohoku-Oki earthquake*, Nat. Commun. 7, 13349 (2016), [doi:10.1038/ncomms13349](https://doi.org/10.1038/ncomms13349). **M**.
- **R10** Juhel et al., *Normal mode simulation of prompt elastogravity signals induced by an earthquake rupture*, Geophys. J. Int. 216, 935 (2019), [IPGP PDF](https://www.ipgp.fr/~jpm/PUBLICATIONS/19Juhel-GJI.pdf). **S**.
- **R11** Matoza et al., *Atmospheric waves and global seismoacoustic observations of the January 2022 Hunga eruption, Tonga*, Science 377 (2022), [supplement](https://ntrs.nasa.gov/api/citations/20230003439/downloads/science.abo7063_sm.pdf). **S** (contents list; amplitudes not read).
- **R12** Wright et al., *Surface-to-space atmospheric waves from Hunga Tonga–Hunga Ha'apai eruption*, Nature 609, 741 (2022), [doi:10.1038/s41586-022-05012-5](https://doi.org/10.1038/s41586-022-05012-5). **M**.
- **R13** Lan, Kuan, Estey, Haslinger, Müller, *Influence of the Coriolis force in atom interferometry*, Phys. Rev. Lett. 108, 090402 (2012), [arXiv:1110.6910](https://arxiv.org/abs/1110.6910). **S**.
- **R14** *Sensitivity limits of a Raman atom interferometer as a gravity gradiometer*, [arXiv:1312.3741](https://arxiv.org/abs/1312.3741). **S** (the Coriolis passage quoted in §5.1).
- **R15** Peters, Chung, Chu, *High-precision gravity measurements using atom interferometry*, Metrologia 38, 25 (2001), [doi:10.1088/0026-1394/38/1/4](https://doi.org/10.1088/0026-1394/38/1/4). **M**.
- **R16** Dimopoulos, Graham, Hogan, Kasevich, *General relativistic effects in atom interferometry*, Phys. Rev. D 78, 042003 (2008), [doi:10.1103/PhysRevD.78.042003](https://doi.org/10.1103/PhysRevD.78.042003). **M**.
- **R17** Hogan, Johnson, Kasevich, *Light-pulse atom interferometry*, Varenna lectures (2008), [arXiv:0806.3261](https://arxiv.org/abs/0806.3261). **M**.
- **R18** Dickerson et al., *Multiaxis inertial sensing with long-time point source atom interferometry*, Phys. Rev. Lett. 111, 083001 (2013), [doi:10.1103/PhysRevLett.111.083001](https://doi.org/10.1103/PhysRevLett.111.083001). **M**.
- **R19** Asenbaum, Overstreet, Kim, Curti, Kasevich, *Phase shift in an atom interferometer due to spacetime curvature across its wave function*, Phys. Rev. Lett. 118, 183602 (2017), [doi:10.1103/PhysRevLett.118.183602](https://doi.org/10.1103/PhysRevLett.118.183602). **M**.
- **R20** *Terrestrial Very-Long-Baseline Atom Interferometry: Workshop Summary*, [arXiv:2310.08183](https://arxiv.org/abs/2310.08183). **S**.
- **R21** Chaibi et al., Phys. Rev. D 93, 021101 (2016); Canuel et al. (MIGA), Sci. Rep. 8, 14064 (2018). **M**, cited only for context.
- **R22** *IERS Conventions (2010)*, IERS Technical Note 36, [iers-conventions.obspm.fr](https://iers-conventions.obspm.fr/). **M** (ch. 7 for Love numbers, solid-Earth and pole tides).
- **R23** Hartmann & Wenzel, Geophys. Res. Lett. 22, 3553 (1995); Wenzel, *ETERNA 3.4*, Bull. Inf. Marées Terrestres 124 (1996). **M**.
- **R24** Farrell, *Deformation of the Earth by surface loads*, Rev. Geophys. 10, 761 (1972), [doi:10.1029/RG010i003p00761](https://doi.org/10.1029/RG010i003p00761). **M**.
- **R25** Agnew, *NLOADF*, J. Geophys. Res. 102, 5109 (1997); Bos & Scherneck ocean loading service. **M**.
- **R26** Dziewonski & Anderson, *Preliminary reference Earth model*, Phys. Earth Planet. Inter. 25, 297 (1981), [doi:10.1016/0031-9201(81)90046-7](https://doi.org/10.1016/0031-9201(81)90046-7). **M**.
- **R27** Kennett, Engdahl, Buland, Geophys. J. Int. 122, 108 (1995). **M**.
- **R28** Laske, Masters, Frey, Eakins, *Update on CRUST1.0*, EGU 2013, [igppweb.ucsd.edu/~gabi/crust1.html](https://igppweb.ucsd.edu/~gabi/crust1.html). **M**.
- **R29** Pavlis et al., *The development and evaluation of the EGM2008*, J. Geophys. Res. 117, B04406 (2012), [doi:10.1029/2011JB008916](https://doi.org/10.1029/2011JB008916). **M**.
- **R30** Lyard et al., *FES2014 global ocean tide atlas*, Ocean Sci. 17, 615 (2021), [PDF](https://os.copernicus.org/articles/17/615/2021/os-17-615-2021.pdf) **S**; [AVISO FES page](https://www.aviso.altimetry.fr/en/data/products/auxiliary-products/global-tide-fes.html) **S** (licence wording); [TPXO README](https://ftp.tugraz.at/pub/ITSG/oceanAndAtmosphericTides/models/TPXO10_OCN/00README.txt) **S** (Oregon State copyright and user agreement); Egbert & Erofeeva, J. Atmos. Ocean. Technol. 19, 183 (2002) **M**.
- **R31** Hersbach et al., *The ERA5 global reanalysis*, Q. J. R. Meteorol. Soc. 146, 1999 (2020), [doi:10.1002/qj.3803](https://doi.org/10.1002/qj.3803). **M**.
- **R32** [GEBCO terms of use](https://www.gebco.net/data-products/gridded-bathymetry/terms-of-use) **S**; NOAA NCEI, *ETOPO 2022*, [doi:10.25921/fd45-gt74](https://doi.org/10.25921/fd45-gt74) **M**.
- **R33** Ekström, Nettles, Dziewoński, *The global CMT project 2004–2010*, Phys. Earth Planet. Inter. 200–201, 1 (2012), [doi:10.1016/j.pepi.2012.04.002](https://doi.org/10.1016/j.pepi.2012.04.002); [globalcmt.org](https://www.globalcmt.org/). **M**.
- **R34** Park et al., *The JPL planetary and lunar ephemerides DE440 and DE441*, Astron. J. 161, 105 (2021), [doi:10.3847/1538-3881/abd414](https://doi.org/10.3847/1538-3881/abd414) **M**; [NAIF generic kernels](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/aareadme.txt) **S** (file sizes reported by third-party pages conflict).
- **R35** Peterson, *Observations and modeling of seismic background noise*, USGS Open-File Report 93-322 (1993). **M**.
- **R36** Bormann & Dewey, *New Manual of Seismological Observatory Practice* (surface-wave magnitude), [doi:10.2312/GFZ.NMSOP-2](https://doi.org/10.2312/GFZ.NMSOP-2). **M**.
- **R37** Wang, Heimann, Zhang, Wang, Dahm, *Complete synthetic seismograms based on a spherical self-gravitating Earth model with an atmosphere–ocean–mantle–core structure*, Geophys. J. Int. 210, 1739 (2017). **M**.
- **R38** Merriam (1992) and Boy et al. (2002) on the atmospheric gravity admittance; Hinderer, Crossley, Warburton, *Gravimetric methods — superconducting gravity meters*, Treatise on Geophysics. **M**.
- **R39** Creutzfeldt et al., *Gravity effect of soil moisture / rain*, Water Resour. Res. (2010). **M** (exact title not checked).
- **R40** Longuet-Higgins, *A theory of the origin of microseisms*, Phil. Trans. R. Soc. A 243, 1 (1950). **M**.
- **R42** Torge & Müller, *Geodesy*, 4th ed.; Melchior, *The Tides of the Planet Earth*. **M**.
- **R45** Meeus, *Astronomical Algorithms*, ch. 47; Bretagnon & Francou, *VSOP87* (1988). **M**.
- **R46** pyTMD, PyGTide; IGETS data base, [isdc.gfz-potsdam.de](https://isdc.gfz-potsdam.de/igets-data-base/). **M**.
- This repository: `crates/cav-core/src/defaults.rs`, `constants.rs`; `reference/design/instrument.md` §4–§7; `reference/design/world.md`; `reference/design/physics.md` (line 192); `reference/milestones/M17-site-and-baseline.md`; `plan/REBUILD.md` §2, §3.4, §5.2, §9, §9.2; `plan/VALIDATION-AND-RESEARCH.md` §1, §2.5, §4.

---

## 11. What I could not verify

1. **No source was opened in full.** All of §10's **M** entries are citations I believe correct but did not check;
   DOIs, volume numbers and some titles may be wrong. The **S** entries were read as search snippets only.
   The network was closed to arXiv and publishers; the request to open it was not answered. **First action for
   the builder: read R3, R4, R13, R15–R17 and R22 in full.**
2. **The AION-10 TDR's numbers** (separation, T, kicks, shot-noise sensitivity, site vibration, any Newtonian-noise
   estimate): not read; the repo's constants are used instead, and AICE's separation and kick number are assumed.
3. **The Rayleigh-wave Newtonian-gradient factor γ (≈0.7)** and the formula's form Γ = γ2πGρkξ_z: a reconstruction
   by analogy with the Bouguer sheet, not Harms's or Hughes & Thorne's expression. Every seismic number (§2.4,
   §2.5) is only as good as it.
4. **The gradient tidal factor 1 + 6k₂ − 3h₂**: derived here and cross-checked by reproducing the gravimetric factor
   1.16 with the same method; not checked against any published gradient tide.
5. **Love numbers** (h₂ ≈ 0.61, k₂ ≈ 0.30): from memory of IERS 2010 Table 7.2.
6. **The PEGS constant C ≈ 0.1**: calibrated to a remembered ~1 µGal for Tohoku, not derived; the signal's
   time function and the inertial response are not modelled.
7. **Hunga Tonga amplitude (100 Pa) and period (1 h)**: Matoza's supplement contents list and one GRL snippet
   ("several hundred pascals" near the source) were all I read; the 100 Pa global figure and 1 h period are from memory.
8. **Microbarom amplitudes (0.1–1 Pa)**, **the atmospheric pressure displacement coefficient (0.4 mm/hPa)**,
   **atmospheric admittance (−0.3 µGal/hPa)**, **ground displacement spectra (10⁻⁷–10⁻⁶ m/√Hz)** and **tidal-sea
   amplitudes**: remembered orders of magnitude, no source read.
9. **Dataset sizes and licences**, except the four ✔ ones: from memory or my own arithmetic (GEBCO int16,
   EGM2008 line count, ERA5 field size). GEBCO's two statements (public domain; copyright and database rights) are
   unreconciled. DE440s is quoted as ≈17 MB by one third-party page and ≈33 MB by another. **The owner decides licences;
   none was read in its full legal text.**
10. **Whether weather fields at the ground behave as the barotropic, flat-ground, horizontally layered model** assumes:
    no comparison against a 3-D atmospheric Newtonian calculation was made. Cloud and rain estimates are one-sided.
11. **The rotation phase expressions** (2k_eff(Ω × v₀)T², the Sagnac equivalence, the 7/12 gT² gradient term): from
    memory of the standard literature; the Coriolis displacement and its numbers are my own integration of the equation of
    motion for a centre-of-mass free fall (not the arm structure of the 1000-kick sequence).
12. **The instrument's technical noise**: nothing here says what is *measured*, only what is above the white shot-noise
    floor of the plan.

## 12. Further questions (not chased)

- Which Oxford-area data exist for a site microseism spectrum (a seismometer in the Beecroft stairwell)?
- What are AICE's site, orientation, depth and tower construction?
- Does the instrument track fringes through several radians of slow tide, or run in a fixed-phase servo?
- Which tidal constituents does the AION collaboration's own analysis remove?
- Is a gradient-aware tide code published (a wrapper around ETERNA for gradients)?
- How large is the true cancellation between ocean attraction and crustal loading at 100 km from a shelf sea?
