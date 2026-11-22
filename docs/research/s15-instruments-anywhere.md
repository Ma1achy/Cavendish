# S15, instruments anywhere: research note

> Research note for slice S15 (`plan/REBUILD.md` §9.3), written 22 Nov 2026 on branch
> `research/s15-instruments-anywhere`. GPU-free. It answers the owner's four questions and checks the
> instrument rows of `plan/VALIDATION-AND-RESEARCH.md` §2.6.
>
> **How it was made.** Four researcher agents each took one question. A first pass had no network
> (arXiv and the publishers were blocked) and could only cite search snippets; those drafts were
> thrown back once access to arXiv, NASA NSSDCA and ESA opened, and every literature claim below was
> re-read in the paper itself (PDFs fetched from `arxiv.org/pdf/<id>` and read as text: tables,
> equations, captions). Each fact carries its URL and a short quotation or an equation/table number.
> What still could not be opened is in §7. Numbers marked *computed here* come from the scripts in
> `docs/research/s15-instruments-anywhere/scripts/` (the Rust one runs the engine's own `Reader`,
> `cargo run --release` in `scripts/q1bench`; nothing in the engine was changed). Spot checks by the
> author of this note against the PDFs: `v_l = 3.86 m/s` in Carlton & McCabe; MIGA's 200 m cavity and
> `2T = 500 ms`; AEDGE's `4.4×10⁷ m`, `300 s`, `10⁻⁵ rad/√Hz`.

## 1. One-page summary

**What the model is, and why it extends.** The engine's phase per interferometer,
`δφ = (m/ħ)∫₀^{2T}[V(upper arm) − V(lower arm)] dt` with the *source* potential only, is the AION
collaboration's own formula (Carlton & McCabe, [arXiv:2308.10731](https://arxiv.org/abs/2308.10731),
eq. 11); for a distant source it reduces to `ΔΦ = −k_eff Γ Δr T²` with **no extra factor** (their eq. 13;
Asenbaum, [arXiv:1610.03832](https://arxiv.org/abs/1610.03832)), and `k_eff = n·2π/λ` for single-photon
clock transitions (the engine's convention; two-photon Bragg/Raman instruments use `2nk`). The Earth's
own field adds 10⁴ to 10⁷ rad to every shot, identical shot to shot; the engine rightly omits it, and
every extension below has to keep it omitted *and* keep the geometry it changes.

| | Answer | Signal size (rad) | Task-sized recommendation | Out of reach |
|---|---|---|---|---|
| **1. Mixed towers** | Every `InstrumentSpec` field becomes per tower (partial override on a shared default); readers shared by instrument key; readout grids per tower, so the bundle is ragged: store CSR plus a padded `(T_max, D)` view with a mask and `NaN` times. Real arrays that mix: MAGIS-100 (three sources, modes A–D), AION stages (10/100/2000 m), AICE (3 → 10 sources), ELGAR (80 gradiometers per arm). | 10 kg at 10 m: **−8.5 mrad** (AION-10), **+51 mrad** (AICE); far-field ratio **164** = (Δr·T²) ratio; 1 t wraps past π on AICE | Per-tower override, `Arc<Reader>` by key, `readout_times(tower)`, weighted Lomb–Scargle; array-level bit-equality test | Bragg (`2nk`) sequences, resonant sequences, interferometer lists per tower |
| **2. Horizontal** | Reads `k̂ᵀΓb` (sensitive axis against baseline), not "the horizontal component". Real designs (MIGA, ELGAR) launch **vertically** through two **horizontal** cavities: π/2 pulses in the lower beam, π in the upper beam gT²/2 higher (30.6 cm at T = 0.25 s). Coriolis and Sagnac are **one** term, `2k_eff(v₀×Ω)T²`. | 10 kg at 10 m, 10 m baseline: **−22 mrad**; the closed-form tidal estimate is **43 % off** at R = 10 m (error ∝ 1/R²); Earth's own Γ term 7×10⁴ rad (static); 1 µrad needs the two interferometers' vertical velocities equal to 2×10⁻¹² m/s | 3-vector arms with exact unit vectors, vertical branch bit-identical (`to_bits` test with a tilted control), two beam heights in the manifest | Wavefront and cavity-mode terms, finite pulses, double-loop gradient compensation |
| **3. Gravity** | Normal gravity (Somigliana, derived constants reproduce the printed older pair) plus free-air height; the static gradient is calibration, not scene. `g` must reach the flight and the launch rule: the source phase moves **1.5 %** (25 µrad of 1.66 mrad) from equator to pole at fixed `T`. Moon and Mars presets need a different `T`/`u₀`. | Earth-gradient phase **7.4×10⁴ rad** (AION-10), **1.2×10⁷** (AICE); `du₀ = T·dg = 0.038 m/s` for equator to pole | `normal_gravity(lat, h)`, per-tower `gravity` re-deriving the launch rule, Moon/Mars presets as data | A 2190-degree EGM2008 synthesis in the engine; lunar mascons |
| **4. Orbit** | Free fall, `g = 0`, long `T`; the signal is the Earth's tidal field in the satellite frame. For an Earth-pointing satellite with atoms at rest in its frame the radial reading is **3n²**, not 2n². The engine's arm integral on the exact tidal potential is exact for a quadratic potential and leaves an `O((nT)²)` residual (0.02–0.35 rad for Rb at T = 5–10 s), so the first mode needs the Hill flow as an in-engine oracle. | **8×10² to 10⁵ rad** (fringe-counted, not mrad); the ISS (4.2×10⁵ kg, 400 km) reads **2×10⁻¹¹ rad** on AION-10 | Circular orbit, point mass then J2 (`V_zz = (2GM/r³)(1 − 6J₂(R/r)²P₂)`), Hill-flow oracle, refuse rotating attitudes until the rotation term lands | GOCE inversion, drag-free control, laser links, strain response of AEDGE/MAGIS-space |

**Section 2.6 rows (§2 below has the reasons).** `per_tower_instruments`: OK, weak at `Reader` level (bit-equality is
tautological there; move it to array level and add control arms). `vertical_case_unchanged`: OK. `horizontal_baseline_tensor`:
**weak** (43 % off in the near field; replace by a known-answer test plus a far-field bound). `local_gravity_from_field_model`:
**weak** (normal-gravity half fine; the "field model's own test values" half is unconfirmed). `orbit_gradient_tensor`: OK on
the closed forms; **GOCE cannot be the oracle**. `injection_recovered`: **weak, defer**. `two_sites_reject_local_noise`: **weak**
(N and "common mode" undefined; now specifiable from Chaibi et al.). Also wrong: the §1 table's "AICE 100 m: 70 m, 2.5 s, 5 s cycle"
mixes two documents and its 5 s cycle and 21.27 m/s are in no source; `coriolis_and_sagnac_phase` (S14) is weak.

**Things the owner should look at**
1. **AICE preset.** The 70 m / 2.5 s pair is the *Technical Proposal's* 140 m shaft ([arXiv:2608.18743](https://arxiv.org/abs/2608.18743)); the "100 m" LoI and feasibility study have Δz = 50 m, T = 1.4 s, n = 5494, repetition interval 1.5 s, so **2T = 2.8 s exceeds the cycle**. `SceneSpec::validate` refuses `2T ≥ cycle`. Decide which preset is canonical and whether overlapping shots are allowed (each shot has its own cloud, so overlap is physical). For George.
2. **Licences (owner decides).** GOCE data are free under ESA's terms with a registration, for the recipient's own EO research; redistribution needs the recipient to accept the terms, so this note recommends **not committing GOCE files**. The EGM2008 and ICGEM licences could not be read.
3. **Plan wording.** The §1 AICE row, the ULDM benchmark line (`2×10⁻¹⁶ eV … ~10⁻² µrad`: not in 2308.10731, origin unknown), and the `Chaibi` identifier (`1601.00417`, not `1601.00957`).

**Suggested tasks** (each small, each leaves the vertical case bit-identical): (a) per-tower instrument override, reader sharing, ragged readout times; (b) general-axis arms as 3-vectors with the `to_bits` test; (c) site gravity module (`normal_gravity`, Moon and Mars presets, launch rule from `g`); (d) horizontal preset with the two-beam-height manifest entry; (e) orbital frame v0 (tidal Legendre source, Hill oracle, refusal of rotating attitudes); (f) the systematics terms as functions (Coriolis/Sagnac first). Details and tolerances are in each question's (c) and (d).

**Could not verify (headline).** NGA's WGS84 PDF, ICGEM and the EGM2008 test-point question (hosts refused); GRGM1200A and GMM-3 as printed; the primary Wuhan 10 m paper; AICE's own launch speed and repetition interval (absent from the sources); the AION-10 hardware `T` (the TDR has none; `0.73 s` is Carlton & McCabe's assumption, whereas Badurina et al. Table 1 uses `1.4 s`); CARIOQA's gradiometer sensitivity (its phase 0 is a one-axis accelerometer); several GOCE papers. Full lists are in each question's (g) and in §7.

## 2. The instrument rows of §2.6, checked against sources

| Row | Verdict | Reason (details in the question's section) |
|---|---|---|
| `per_tower_instruments` | **OK, weak as written** | Bit-equality holds and was measured (four unequal towers, reverse order, threads: all bits equal; control arm: changing one tower's `T` by 1e-9 changes only that tower). At `Reader` level it is tautological; its value is at array level (shared state, parallel reductions, culling). §3 (d). |
| `vertical_case_unchanged` | **OK** | `x + 0.0`, `x·1.0` and zero products are exact in IEEE arithmetic; a floating rotation is not (`cos(π/2) = 6.1×10⁻¹⁷`). Keep exact unit vectors; add a tilted control. §4 (d). |
| `horizontal_baseline_tensor` | **Weak** | "Reads the horizontal gradient component" holds only in the far field. Replace by (i) a known-answer test, `ΔΦ = k T² k̂ᵀΓ(r₀ + v₀T + (7/12) g T²)` for a uniform-gradient source, 2×10⁻¹⁶ measured, plus (ii) a far-field bound `≤ (s_max² + L²)/R²`. Name the component by (k̂, b). §4 (d). |
| `local_gravity_from_field_model` | **Weak** | Normal-gravity half sound. The "field model's own test values" half is unconfirmed (NGA and ICGEM did not open). Replace by Somigliana, the free-air series and the Poisson trace `2ω²` (closed forms, no fixture); EGM2008 only against recorded ICGEM output. §5 (d), (f). |
| `orbit_gradient_tensor` | **OK on the closed forms; GOCE is plausibility only** | GOCE is measured by electrostatic accelerometers and its products depend on a field model; it tests frames and field, not the atom phase. Oracles: point mass, exact J2 `V_zz`, Hill flow, Carraz eq. 2, Trimeche's 1087 rad. §6 (d), (f). |
| `injection_recovered` | **Weak, defer** | The proposals' curves are design projections, plots without a data file, under the arXiv non-exclusive licence; the engine has no noise or GW response, so it would test the injector against itself. For the orbit part use `σ_Γ = σ_φ/(k L T²)` and 1/√N. §6 (f). |
| `two_sites_reject_local_noise` | **Weak, now specifiable** | Chaibi et al. fix N = number of gradiometers and `σ = √2 σ_η/√N` (eq. 4); "factor 2" and "≥ 10" come from a seismic/atmospheric model with anti-correlation that the scene lacks. Test locality (exact), estimator statistics (3σ, ρ control arm) and make no claim of real rejection. §3 (f). |
| `distant_nulls` (ISS, not in the brief's range but checked) | **OK, with a number** | 4.2×10⁵ kg at 400 km: 2.1×10⁻¹¹ rad on AION-10, below the 10⁻¹⁰ rad audit; one interferometer alone sees 8.4×10⁻⁷ rad, so the null holds only in the differential. §6 (f). |
| `coriolis_and_sagnac_phase` (S14) | **Weak** | Coriolis and Sagnac are one term for this geometry (Hogan §4.1; ELGAR §3.2): `Δφ = 2k_eff(v₀×Ω)T²` = `(2m/ħ)Ω·A`. Hold the sign to Hogan, the magnitude to Dickerson Table I (5.1 rad; Dickerson's printed sign is opposite and unexplained). §4 (d). |
| plan §1 AICE row | **Wrong as labelled** | See owner item 1. |

Not touched here (S14 rows that do not concern instruments): `globe_equals_origin`, `tidal_gradient_closed_form`, the pressure and seismic rows.

## 3. Q1. Towers of different heights and sequences in one array

**Status of sources (revision 2).** The first draft was written without access to arXiv and tagged every literature fact `[S]` or `UNVERIFIED`. This revision opened the primary PDFs (downloaded from `https://arxiv.org/pdf/<id>`, converted to text, and read in the body: tables, equations, captions). Each fact below carries the URL it was read at and a short quotation or an equation/table number. Facts I could still not confirm are in (g). Everything tagged **computed here** was run against the engine (`forward.rs` as it stands, `scripts/q1bench/`, Rust, path-depends on `cav-core`; nothing in the repo touched), or in `scripts/q1_window.py` and `scripts/q1_formula_check.py` (numpy/plain Python), and is reproducible.

Identifier corrections: Chaibi et al. is **arXiv:1601.00417** (PRD 93, 021101(R); journal reference read at <https://arxiv.org/abs/1601.00417>), not 1601.00957; ELGAR is **arXiv:1911.03701** (CQG 37, 225017; <https://arxiv.org/abs/1911.03701>), not 2007.04014. The "TP" identifier in the first draft (arXiv:2608.18743) **exists**: it is "Technical Proposal for the Atom Interferometer CERN Experiment (AICE) Facility", submitted 19 Aug 2026 [hep-ex] (<https://arxiv.org/abs/2608.18743>, <https://arxiv.org/pdf/2608.18743>). I treated it as I treated the others (read the body) but it is one month old and I have not seen any later version.

### (a) What the literature says, and what changes in the model

#### a.1 Signal scaling and the k_eff convention (now read in the sources)

The engine's phase for one interferometer is δφ = (m/ħ) ∫₀^2T [V(upper arm) − V(lower arm)] dt, V the source potential only.

- **This is the AION collaboration's own formula.** Carlton & McCabe, "From RATs to riches" (<https://arxiv.org/abs/2308.10731>, PDF <https://arxiv.org/pdf/2308.10731>), eq. (11): δφ_ℓ = (m_A/ħ) ∫_{ℓΔt}^{ℓΔt+2T} dt [V(z₂,t) − V(z₁,t)], "the indices 1, 2 denote the classical paths travelled by the atoms in each arm", with the unperturbed (Earth) phases treated separately; the gradiometer phase is eq. (9): Δφ = φ^(u) − φ^(l). So "source potential only, classical arms" is the method the AION analysis uses, not a Cavendish invention. (The paper also includes the Earth gradient γ_zz in the trajectories, eq. (10); the engine does not.)
- **Gradiometer formula, with the factor.** Same paper, eq. (13) (far-field, D, h ≫ arm excursion, stationary mass M at lateral distance D and height h): "Δφ_ℓ ≃ −n k T² G_N M Δr (D² − 2h²)/(D² + h²)^{5/2}". That is **Δφ = −k_eff · Γ · Δr · T²** with k_eff = n k and Γ = ∂²V/∂z² = GM(D² − 2h²)/(D²+h²)^{5/2} (I checked the second derivative by hand). **No factor 2 and no factor 1/2.** The paper also states Δφ ∝ M ("simply linear in M") and the inverse-cube law for D ≫ h. Computed here (`q1_formula_check.py`): eq. (13) gives −1.6008e-8 rad for 10 kg at D = 1 km, h = 1 m, T = 0.73 s, Δr = 5 m, n = 1000; the engine gave −1.6007e-8 (draft, table in (b)). Agreement to 1e-4.
- **Independent confirmations of the scaling.** Dimopoulos et al. 2008 (AGIS, <https://arxiv.org/abs/0806.2125>): "The differential phase shift caused by this effect is δk_eff ∇g L T²", the gradiometer phase is k_eff ∇g L T² per unit k_eff (scaling statement, no numerical prefactor given); the single-interferometer response is φ = k_eff a T² (their text near eq. 3). Hogan, Johnson & Kasevich, "Light-pulse atom interferometry" (<https://arxiv.org/abs/0806.3261>): "The phase shift in an atom interferometer due to a constant acceleration g is Δφ = k_eff g T²" and Table 2 lists −k_eff g T² (sign convention) as term 1, and −k_eff T_zz v_z T³ and (ħk_eff²/2m) T_zz T³ as gravity-gradient terms; gradiometer = difference of two such accelerometers "with a common laser beam" (§3, "Gravity gradiometer"). Asenbaum et al. PRL 118, 183602 (<https://arxiv.org/abs/1610.03832>): gradiometer phase "T_zz ≈ −dφ/(n ħ k L T²)" with L the baseline, i.e. dφ = −n k L T_zz T², again no extra factor; and the recoil phase of a single interferometer in a uniform gradient "Δφ_R = −(ħ/2m) n²k² T_zz T³ = −½ n k Δz T_zz T²", the same for the upper and lower interferometer so it cancels in the difference. Dickerson et al. PRL 111, 083001 (<https://arxiv.org/abs/1305.1700>) Table I lists k_eff g T² and (ħk_eff²/2m) T_zz T³.
- **k_eff convention: single-photon clock transition with n kicks is k_eff = n k, k = 2π/λ. Not 2nk.** AICE TP, eq. (2.1) (<https://arxiv.org/pdf/2608.18743>): "k_eff = n k₀, where k₀ = 2π/λ ... n counts the number of photon recoils; a two-photon interaction transfers two recoils." MAGIS-100, Table 2 caption (<https://arxiv.org/pdf/2104.02835>): "LMT atom optics of order n refers to an nħk momentum splitting between the two arms ... (corresponding to n photon recoil kicks)". Badurina et al. (<https://arxiv.org/abs/1911.11755>), §2: "Each pulse imparts ... an increase in momentum of ħk ... This enhances the differential gravitational wave signal by a factor of n." Two-photon (Raman/Bragg) instruments use **k_eff = 2 n k**: ELGAR (<https://arxiv.org/pdf/1911.03701>), §3: "the wavenumber k_eff = 2 n k_l ... n two-photon transitions"; ZAIGA (<https://arxiv.org/abs/1903.09288>): "N = 1000 LMT beam splitter (k_eff = 2N k_l = 1.6 × 10¹⁰ m⁻¹)"; Dickerson (Raman): "k_eff = 2k". The engine's k_eff = n·2π/λ = 9.0017e9 rad/m (n = 1000, 698 nm) is the right convention for the Sr presets; a Rb Bragg preset must set an effective k = 2n·2π/780 nm explicitly. The engine's `kicks` should therefore be documented as "recoils of momentum ħ·2π/λ between arms", with two-photon instruments passing 2n.

So the response goes as **n · Δr · T²** (and 1/r³ in the far field): linear in the LMT order, in the baseline, quadratic in T. It holds only for D ≫ Δr and arm excursions; at D = 10 m it is wrong by 1.7× for AION-10 and 2.0× for AICE (table in (b)).

Because the factor-2 question was open: the engine's own integral reproduces the first-order-in-Δz form to 0.1-0.2 % (table in (b)), and the sources above agree with it, so **no change to the formula is needed**.

#### a.2 What really varies between towers in the proposed arrays (read)

| Array | What differs between its interferometers / stages | Source and where in it |
|---|---|---|
| AION-10 | **Two** vertical 5 m interferometers in a 10 m vertical vacuum system, ⁸⁷Sr clock transition, 698 nm, launch lattices, one atom source ("sidearm") per pipe | TDR <https://arxiv.org/pdf/2508.03491>, §2.5 "two vertically aligned 5 m interferometers housed in a 10 m XHV system"; §3.1 "an aligned pair of 5 m baseline atom interferometers in a 10 m vertical vacuum tube ... narrow-linewidth interferometer laser at 698 nm to perform interferometry with clouds of ⁸⁷Sr atoms"; §3.8 "At the bottom of each interferometer beam pipe ... interconnect chambers"; §3.9 launch lattice, scaffold "approximately 0.5 m above and below each interconnect chamber". The TDR contains **no** T, n, cycle time or launch speed (searched). |
| AION stages | Same ⁸⁷Sr family at L = 10, 100, 2000 m with T = 1.4 s (10 m, 100 m) and 5 s (km); LMT n = 100/1000 (AION-10 initial/goal), 1000/40 000 (AION-100), 40 000 (km); GW response broadband (π/2-π-π/2) or **resonant** (π/2 - Q π-pulses - π/2) | Badurina et al. <https://arxiv.org/pdf/1911.11755>, Table 1 and §2 (resonant mode "π/2 − π − ··· − π − π/2 with Q π-pulses"). Badurina's own caption: "we assume T = 1.4 s, which corresponds to a 10 m atom interferometer setup in launch mode. The exact value for T will depend on the precise design". The first 10 m "is aimed at ultralight dark matter, and is not expected to detect GWs" (<https://arxiv.org/abs/2108.02468>, §1). |
| AICE (LoI/CFS, "100 m") | L = 100 m, T = 1.4 s, n = 5494, Δz = 50 m, N = 10⁸ atoms, Δt = 1.5 s, phase noise 10⁻⁵ /√Hz, T_Int = 10⁸ s; "up to ten connection nodes" for sources (5 to 10 ultimately); shaft PX46 is ∼143 m high | LoI <https://arxiv.org/pdf/2509.11867>, Table 1 and §2; feasibility study <https://arxiv.org/pdf/2304.00614>, Table 1 (identical row), "internal height of 143 m". |
| AICE (Technical Proposal, Aug 2026) | L = 140 m. Performance ladder: Initial n ≈ 100, 3 sources, δφ ≈ 10⁻³; **Baseline n = 10³, 5–10 sources, δφ ≈ 10⁻⁴**; Stretch n = 4×10⁴, 10–20 sources, 10⁻⁵ (rad/√Hz). T_grad = 2.5 s, T_MZ = T_ax = 5.0 s. Scalar-ULDM gradiometer "cloud separation δr = L/2 = 70 m at the 140 m Baseline", ⁸⁷Sr 698 nm; dual-species full-baseline mode (⁸⁷Sr/¹⁷¹Yb, 698/578 nm) launched from the bottom, T = 5.0 s | TP <https://arxiv.org/pdf/2608.18743>, Table 1; Fig. 6 caption "separated by δr = L/2" and "The baseline gradiometer uses an interrogation time T = 2.5 s"; §3 "gradiometer with cloud separation δr = L/2 = 70 m at the 140 m Baseline"; §2.3.2 and Fig. 7 "launched from the bottom of the shaft ... T = 5.0 s". |
| MAGIS-100 | One 100 m vertical shaft, **three Sr sources at the top, middle and bottom**, operated in **modes A–D**: A upper+middle sources dropped together, "fall 50 m", free-fall "more than 3 s"; B bottom+upper launched upward for several metres, baseline ≈ 100 m, 1 s-scale; C all three together (GGN correlation across >2 sources); D two Sr isotopes launched together from the bottom (EP/dark-matter). Table 2: Sr prototype tower (Stanford) L = 10 m, n = 10², 2 sources; MAGIS-100 L = 100 m, n = 10² (initial) to 4×10⁴ (final), 3 sources | <https://arxiv.org/pdf/2104.02835>, the instrument-description pages 19-20 (three atom sources, modes A-D), Table 2. Published as Quantum Sci. Technol. 6, 044003 (<https://arxiv.org/abs/2104.02835>). |
| MIGA | **Horizontal**: "Three atomic clouds are prepared at the same height and horizontally separated" along **two horizontal 200 m cavities**; ⁸⁷Rb at 780 nm, two-photon **Bragg**; "2T = 500 ms" (T = 250 ms, vertical rise ≃ 30.6 cm), three atomic heads launch "almost vertical parabolic flight"; heavy source masses "placed at a variable distance up to 150 m" for calibration; the AIs measure horizontal acceleration | <https://arxiv.org/pdf/1703.02490> (v3), §3.2 ("cavity ... L = 200 m ... Rb ... 2T = 500 ms"), §4 (Fig. 5 caption "Three atomic heads at positions X1,2,3", text "horizontally separated by a distance L"), §6 ("heavy source masses ... up to 150 m"). The first draft's "150 m cavity" is **not** in this paper; 150 m is the source-mass distance. I did not read the later MIGA papers (2204.12137, 1505.07137) so I cannot say whether the cavity length changed. |
| ZAIGA | Equilateral triangle, two AIs 1 km apart per arm, a 300 m vertical tunnel "with atom fountain and atom clocks", two **10 m atom fountains** at the top and bottom of the tunnel for EP tests; Rb and Sr; Raman, Bragg, lattice clocks | <https://arxiv.org/pdf/1903.09288> (abstract; ZAIGA-EP section "two 10-meter atom fountains mounted on both top and bottom of the tunnel"; Table 1 in the MAGIS paper "ZAIGA 300 m, 3 baselines, Rb, Sr"). Journal: Int. J. Mod. Phys. D 28 (2019) 1940005, read in reference [17] of Badurina et al., <https://arxiv.org/pdf/1911.11755> (the arXiv abs page lists no journal reference). |
| ELGAR | **Horizontal**, two perpendicular arms, each "N = 80 atom gradiometers of baseline L = 16.3 km, regularly spaced by a distance δ = 200 m, leading to a total arm length of L_T = (N − 1)δ + L = 32.1 km"; ⁸⁷Rb, 780 nm, Bragg (k_eff = 2 n k_l); Fig. 10: n-scale k_eff = 1000·2π/780 nm, T = 200 ms, 1 µrad/√Hz; array target 3.3×10⁻²² /√Hz at 1.7 Hz; a vertical arm could be added | <https://arxiv.org/pdf/1911.03701>, §1.3.1, Fig. 4 caption ("2N total atom interferometers"), Fig. 10 caption, §1.2 (eq. 2 with 2nk_l). |
| Chaibi et al. 2016 | The array above in its original form: N gradiometers, L = 16.3 km, δ = 200 m, T = 0.3 s (2T = 0.6 s), n = 1000 (Fig. 3), see below | <https://arxiv.org/pdf/1601.00417> |
| Stanford 10 m tower | "10-meter tall vacuum enclosure", Raman, 2T = 2.3 s; the later Bragg interferometer: "launched vertically into a 10 m tall atomic fountain"; gradiometer baselines L = 10 to 32 cm, n = 10 to 38, T = 500-900 ms, launch heights ≈ 8.25-8.45 m (Fig. 2/3) | Dickerson <https://arxiv.org/abs/1305.1700> abstract and §1; Asenbaum <https://arxiv.org/abs/1610.03832>, §2 and Fig. 2/3 captions. ZAIGA gives its launch height: "maximum launch height ... is 9 m, ... effective interference area inside the magnetic shield is 8.2 m, ... longest free evolution time ... 1.34 s" (<https://arxiv.org/pdf/1903.09288>, §6). |
| Hannover VLBAI | "10 m vertical baseline" with an octagonal permalloy shield, central 8 m = region of interest; ⁸⁷Rb (with ¹⁷¹Yb planned in ZAIGA's account: "height of 10 m and an effective interference area of 9 m") | <https://arxiv.org/pdf/2209.08886> ("With its 10 m vertical baseline", "central 8 m of the baseline, the region of interest"); <https://arxiv.org/pdf/1911.12320> ("10 m-long high-performance magnetic shield for Very Long Baseline Atom Interferometry"); ZAIGA text above for the 9 m interference area. |
| Wuhan 10 m (WIPM) | "Zhan's group at WIPM designed and developed a 10-meter AI for high-precision WEP test, they obtained the time-of-flight signal of 12-m-height atom fountain" | Only found second-hand, in ZAIGA <https://arxiv.org/pdf/1903.09288> §6; I could not find the primary Wuhan paper (searched arXiv). |
| TVLBAI roadmap | The workshop reports are cited by the TP as the "TVLBAI roadmap [15]" (not opened) | AICE TP (<https://arxiv.org/pdf/2608.18743>, §2.1.1). arXiv:2310.08183 (AVS Quantum Sci. 6, 024701) from the first draft was **not opened**. |

What this says for the engine: the differences between towers in these proposals are **(i) baseline/Δr and tower height, (ii) T, (iii) LMT order n, (iv) orientation (MIGA, ELGAR horizontal), (v) atom species and diffraction type (Rb Bragg with k_eff = 2nk vs Sr single-photon with k_eff = nk), (vi) sequence (broadband vs resonant: Badurina; MAGIS and ELGAR also), (vii) number of sources on the baseline (AION-10: 2; MAGIS-100: 3 with modes A-D; AICE: 3 initially, 5-10 later; ELGAR: 80 gradiometers per arm)**. The engine already carries (i), (ii), (iii: `kicks`, not exposed in the spec), mass (v, partly). It does **not** carry (iv), (vi), (vii) and its two-photon Bragg case.

What the sources say about **cadence** (relevant to the mixed-cycle design): AION-10 "Δt = 2 s" (Carlton & McCabe, §II.B, <https://arxiv.org/pdf/2308.10731>: "a baseline L = 10 m, a separation Δr = 5 m, and n = 1000. Assuming that the atom clouds are launched with a speed of v_l = 3.86 m s⁻¹, we find T = 0.73 s, and we assume Δt = 2 s"; here 2T = 1.46 s < Δt). AICE LoI Table 1: Δt = **1.5 s** with T = 1.4 s, i.e. **2T = 2.8 s > Δt**: the proposal's repetition interval is shorter than one interferometer's duration, so shots overlap in time (the TP gives no repetition interval; I searched it for "repetition", "cycle", "Δt"; line 3009 only says continuous sources "could unlock very high ... repetition rates").

#### a.3 What changes in the data contract (unchanged from the first draft: engineering, not literature)

1. **Per-tower instrument**: every field of `InstrumentSpec` becomes per tower (below). `Instrument::kicks`/`wavelength` are not in `InstrumentSpec` at all; add `kicks` (n) and optionally `wavelength` as spec fields (defaults 1000, 698 nm), since n is the lever in the n·Δr·T² scaling. Document `kicks` as the count of photon recoils ħ·2π/λ between arms (k_eff = n k); a two-photon preset passes 2n.
2. **Flight nodes** depend only on the instrument (`Reader::new` builds them), so they are per distinct instrument, shared (c.2).
3. **Readout times are per tower with different counts**: tower d reads out at t_k = o_d + k·cycle_d, k ≥ 0, with t_0 ≥ 2T_d; count N_d = ⌊(duration − t_0)/cycle_d⌋ + 1. AION-10 (2 s) and a 5 s cycle over 12 s give 5 and 3 (see (b) finding 3 on what the AICE cycle really is).
4. **Per-tower sensitivity** differs by orders of magnitude (b), so the bundle must carry each tower's own gain and noise level.

**Ragged vs padded.** `.npy` has no ragged type. Recommendation: the primary storage is **CSR-style**: `t_flat`, `phase_flat` (float64) and `tower_offsets (D+1)`; plus the **padded view** the plan already promises, `(T_max, D)` for `readout_time`, `phase`, `mask`, with `n_shots (D,)`. Padded cells: `mask = False`, `phase = 0`, `readout_time = NaN` (NaN, not 0, so an unmasked consumer fails loudly; never zero-fill without the mask, the point of the lost-shot scripts). Tracks stay on the nominal `time (T,)` axis (M15-R3). `.npy` export writes the padded arrays and `n_shots`; the CSR arrays go as extra files. M15-R1 earlier chose equal shot counts to avoid ragged tensors (`reference/milestones/M15-…`, "Why equal shot counts"); that holds for offsets/jitter, not for unequal cycles, so the plan's `(T, D)` becomes `(T_max, D)` + mask. **Windows for ML**: a window of 64 shots is 128 s on AION-10 and 96 s at the LoI's 1.5 s; Gradar windows should be defined in **time** (W seconds, padded to the largest count with mask), a decision for the ML slice, noted in (g).

**Nyquist and joint spectra (computed here, `q1_window.py`; unchanged).** Per-tower Nyquist is 1/(2·cycle_d): 0.25 Hz (AION-10, Δt = 2 s), 0.1 Hz for a 5 s cycle, 0.33 Hz for the LoI's 1.5 s. `spectrum::fold_frequency(f, nyquist)` is per tower and already right. For a joint periodogram the window is the shot-weighted sum of the per-tower windows. Measured |W| of the union (20 000 s of each; AION 2 s and a 5 s second tower, the first draft's example):

| ν (Hz) | AION alone | 5 s tower alone | union, integer offsets, equal weights | union, offset 0.37 s | union, jitter ±0.3 s | union, inverse-variance weights (second tower 164× more precise) |
|---|---|---|---|---|---|---|
| 0.2 | 0.000 | 1.000 | 0.286 | 0.286 | 0.279 | **1.000** |
| 0.4 | 0.000 | 1.000 | 0.286 | 0.286 | 0.259 | **1.000** |
| 0.5 | 1.000 | 0.000 | 0.714 | 0.714 | 0.715 | 0.000 |
| 1.0 | 1.000 | 1.000 | **1.000** | 0.559 | 0.857 | 1.000 |

Reading: unequal cycles break each tower's aliasing; but **if the noise weights are the right ones the precise tower's aliases come back at full height**. The 164 is the far-field n·Δr·T² gain ratio of AICE (TP: Δr = 70 m, T = 2.5 s) to AION-10 (5 m, 0.73 s), now source-backed (b). Consequences:

- `lomb_scargle(t, y, f)` in `cav-core/src/spectrum.rs` takes one series, no weights. A joint spectrum needs a **weighted** generalised Lomb–Scargle (heteroscedastic; VanderPlas 2018 and VanderPlas & Ivezić 2015: **not opened**, UNVERIFIED). A joint coherent fit must either fit a free amplitude per tower, or the designer shows the **incoherent sum of per-tower spectra, each divided by its own σ_d²**. Default recommendation: incoherent sum; coherent joint only for one source of known geometry.
- The existing `docs/research/s2-replay-spec-paths-cadence.md` §4 closed forms stay per tower; the joint window is Σ_d w_d N_d W_d / Σ_d w_d N_d (w_d = 1/σ_d², phases kept), checkable by Monte Carlo as §4.2 does.

**Gradar and tracker geometry.** The Fisher information for source parameters θ is J = Σ_d Σ_k σ_d⁻² ∂ΔΦ_d(t_k)/∂θ ∂ΔΦ_d(t_k)ᵀ; with a 164× gain ratio one AICE shot outweighs ~27 000 AION-10 shots at equal σ (gain² ratio), so a heterogeneous array's CRB is dominated by its highest-gain tower near the source. Recommendations: (1) model inputs are per-tower phase **divided by that tower's σ** or the per-tower `gain`/`sigma` is an `observable`-role input; (2) the per-tower instrument descriptor (position, orientation, Δr, T, cycle, n) is an `observable` meta-input, not a target; (3) train with randomised instrument mixes; (4) the CRB baseline in `ML-AND-API.md` §4 is computed with per-tower σ_d and the real flight integral, not the quasi-static one.

### (b) Orders of magnitude in radians (computed here, `q1bench`, engine finite-difference integral)

Engine constants: k_eff = n·2π/λ = 9.0017e9 rad/m (n = 1000, 698 nm), v_kick = 6.5779 m/s (the TP and Carlton use the same k_eff convention, a.1). AICE preset (the TP's Baseline gradiometer, a.2): Δr = 70 m, T = 2.5 s, u₀ = 21.236 m/s. Source: static point mass at lateral distance D, height h above the tower base. "Field@mid-arm" is the first-order-in-Δz form −k_eff·Σ w·Δz·a_z(z_mid)/v_kick; "kΓΔrT²" is the quasi-static formula with Γ at the baseline midpoint.

| Instrument | M, D, h | engine ΔΦ (rad) | field@mid-arm | kΓΔrT² |
|---|---|---|---|---|
| AION-10 | 10 kg, 10 m, 1 m | −8.46e-3 | −8.48e-3 (×1.002) | −1.45e-2 (×1.71) |
| AION-10 | 10 kg, 100 m, 1 m | −1.589e-5 | ×1.000 | ×1.007 |
| AION-10 | 10 kg, 1 km, 1 m | −1.6007e-8 | ×1.000 | ×1.000 |
| AICE | 10 kg, 10 m, 1 m | +5.08e-2 | +4.97e-2 (×0.978) | +1.04e-1 (×2.05) |
| AICE | 10 kg, 10 m, 35 m (mid-baseline) | −1.08e-1 | −1.31e-1 (×1.22) | −2.63 (×24) |
| AICE | 10 kg, 100 m, 1 m | −5.29e-4 | ×0.994 | ×2.9 |
| AICE | 10 kg, 1 km, 1 m | −2.582e-6 | ×1.000 | ×1.013 |

The phase is linear in mass (the engine's test asserts it to the bit; Carlton & McCabe state the same, "the phase response is simply linear in M"), so **1 t is ×100**: 1 t at 10 m (lateral, h = 1 m) reads about 0.85 rad on AION-10 and 5.1 rad on AICE, so it wraps past π on AICE, while 10 kg gives 8.5 mrad and 51 mrad. The existing anchor "10 kg passing at D = 10 m, about 7 mrad" (forward.rs) is the same order as −8.5 mrad here at h = 1 m. Cross-check against the AION paper's own numbers: the paper's per-shot atom-shot-noise scale is "O(250) µrad" (§III, "atom shot noise (O(250) µrad cf. Fig. 2)"), and its small-animal transients are "O(50) µrad ripples" (same section), so the engine's mrad-scale 10 kg signals at 10 m are one to two orders above shot noise, as expected.

**AION-10 vs AICE, same source and distance.** Far field (D = 1 km): 2.6149e-6 / 1.6007e-8 = **163.4**; the scaling n·Δr·T² gives (70/5)·(2.5/0.73)² = 14 × 11.73 = **164.2** (n equal), and eq. (13) of Carlton & McCabe evaluated for both gives the ratio 164.20 (`q1_formula_check.py`). Near field the ratio is not 164: at D = 10 m, h = 1 m it is 6.0 (opposite sign), because D is comparable to Δr = 70 m and to the AICE arm excursion. Rule: the quasi-static form is good to 1 % only when D ≳ 20·max(Δr, arm excursion); inside that, only the finite-difference integral is right. Quadrature: the audited read (`read_to`) agrees with the fixed rule to 8e-17 rad (AICE, D = 10 m) and 2.8e-11 rad at D = 3 m, one halving, so the default rule (order 6, 0.1 s panels; 300 nodes for AICE, 108 for AION-10) is far inside `PHASE_TOLERANCE` = 1e-10 at AICE's T.

**What the engine deliberately leaves out, in radians (computed here, plain arithmetic with G-gradient T_zz = 3.086e-6 s⁻²).** Single-interferometer Earth terms: k_eff g T² = 4.7e10 rad (AION-10) and 5.5e11 rad (AICE); the Earth-gradient recoil phase ½ n k Δz T_zz T² (Asenbaum eq. quoted in a.1, Δz = v_kick·T) = 3.6e4 rad (AION-10), 1.4e6 rad (AICE), common to both interferometers; the Earth-gradient gradiometer phase k_eff T_zz Δr T² = 7.4e4 rad (AION-10) and 1.2e7 rad (AICE). All are static and the same every shot, so they are removed by calibration and by the differential readout; the 10⁻⁶ rad-scale signals ride on top. What matters for S15: they are huge compared with the signal, so any launch-rule or g-error that changes them from shot to shot (Dimopoulos et al.: δk_eff ∇g L T², δT terms) matters, and a per-tower T or Δr changes them by T² or Δr.

**Findings about the presets (re-examined with the sources):**
1. AION-10 numbers verified in the AION paper itself: L = 10 m, Δr = 5 m, n = 1000, v_l = 3.86 m/s, T = 0.73 s, Δt = 2 s (Carlton & McCabe, quoted above; <https://arxiv.org/pdf/2308.10731>), shot-noise "δφ = 10⁻⁴ rad/√Hz" for the AION-10 (goal) scenario. The engine's u₀ from u₀ = gT − v_kick/2 is **3.872 m/s** (computed here, exact ħ, 698 nm, g = 9.81), which the paper rounds to 3.86 (the launch rule matches to 0.3 %; the paper does not give its g or the rule, so I cannot say which). The first draft's "0.74 s / 4.86 m" quote came from an unnamed snippet and is **contradicted** by the primary source; deleted.
2. AICE u₀: the engine gives **21.236 m/s**; `plan/VALIDATION-AND-RESEARCH.md` §1 says 21.27 = gT − v/2 with rounded constants (v = 6.505). No source states a launch speed for AICE (searched LoI, CFS, TP for "launch", "m/s"). The table should say "21.24 (21.27 with the scripts' rounded constants); not in any source".
3. **AICE geometry and cycle (corrected).** The first draft said the 70 m preset "does not fit a 100 m facility". With the sources: the **TP's Baseline gradiometer is exactly this preset**: L = 140 m, δr = L/2 = 70 m, T_grad = 2.5 s (a.2). The engine's upper-arm apex is 70 + 38.9 = **108.9 m < 140 m** (computed here, as in the draft), so it fits the TP's 140 m shaft, not the "100 m" of the LoI/CFS (where L = 100 m, T = 1.4 s, Δz = 50 m, n = 5494). The label "AICE 100 m" in sec. 1 therefore mixes two documents: **70 m / 2.5 s belong to the TP (140 m)**; **the 100 m documents say 50 m / 1.4 s / 1.5 s cycle**. One more numerical coincidence I could not resolve: the TP (Section 7.4, p. 125) says for the GW Stretch case "the upper source sits approximately 30.7 m below the top of the shaft, so that the gradiometer baseline is Δz ≈ 109 m" (<https://arxiv.org/pdf/2608.18743>, p. 125). gT²/2 = 30.66 m at T = 2.5 s (computed here), so 30.7 m is the rise of the **mean** trajectory, and 140 − 30.7 = 109.3 m ≈ the engine's 108.9 m arm apex. I do not know whether the TP's two numbers (δr = 70 m for scalar ULDM, Δz ≈ 109 m for GWs) are two configurations or one; ask the authors/George, and do not hard-code either as "the" AICE.
4. **Cycle 5 s and the `2T ≥ cycle` check.** "Cycle 5 s" for AICE is **not in any source I read**: T_MZ = 5.0 s in the TP is an *interrogation time* (a full-baseline dual-species mode; T_MZ ≃ √(2L/g)) and the plan's cycle looks like a conflation of that with a repetition interval. The only stated repetition interval is the LoI/CFS Δt = 1.5 s (with 2T = 2.8 s). So the literature's AICE has **2T > cycle** (overlapping shots), which `SceneSpec::validate` (`2T ≥ cycle` refused) forbids; AION-10 (2T = 1.46 s < 2 s) is fine. Either allow overlapping interferometers (needs the separate-source structure: each shot has its own cloud, so overlap is physically fine with multiple sources) or document that the engine does not model them. A question for George.

### (c) Recommendation: small enough for tasks

**c.1 Spec (cav-sim `spec.rs`).**
```rust
pub struct TowerSpec {
    pub label: String, pub position: [f64; 3],
    #[serde(default)] pub instrument: Option<InstrumentOverride>,   // None = the array default
    #[serde(default)] pub timing: Timing,                            // offset, jitter (M15-R1)
}
pub struct InstrumentOverride { /* every InstrumentSpec field as Option<_> */ }
impl ArraySpec { pub fn instrument_of(&self, tower: usize) -> InstrumentSpec }   // default merged with override
```
A **partial** override with a shared default (M15-R2's design), not a full copy, so editing the default propagates to unset fields and a tower can be "AION-10 with T = 0.9". The merged spec is validated per tower with the same checks, error paths `array.towers[n].instrument.half_sequence`. Additive and optional: SPEC_VERSION stays 1; old files load (the S0 test's pattern); `deny_unknown_fields` stays. Add `kicks` to `InstrumentSpec` (default 1000, documented as in a.1) with range in `limits`. Two things to fix at the same time: `bounds()` uses `array.instrument.tower_height` and `Motion::position(time, array.instrument.gravity)` uses the instrument's g for free-falling bodies. Per-tower height must be read per tower; and a body's fall must use the **world's** g, not a tower's (S15 local g): either a scene-level `gravity` or document that the array default is the world's. Add `validate` checks that arms stay inside the tower (apex ≤ tower_height + stated tolerance), which fails the 70 m preset at tower_height = 100 m and passes it at 140 m (b, finding 3).

**c.2 Readers.** Key = every field of the resolved `Instrument` by bits (`to_bits`, with −0.0 normalised, plus enums); `HashMap<InstrumentKey, Arc<Reader>>` built at array construction, towers resolve to the same `Arc` when equal. `Reader` is `Sync` (`OnceLock` levels), so threads share it. Memory is small: the finest of the 9 levels is 2⁸ × 300 nodes × 32 B ≈ 2.5 MB for AICE and built lazily. Optional refinement (not now): the `Flight` depends on (T, u₀, g, kicks, floor, quadrature) only, so instruments differing in Δr, G or sign could share nodes.

**c.3 Readout grid.** One function `readout_times(tower) -> Vec<f64>` (offset + k·cycle + keyed jitter, first readout ≥ 2T_d) used by the engine, the cadence designer and the tests.

**c.4 Cadence designer and spectra.** Per-tower rows (cycle, offset, jitter, gaps, loss); per-tower Nyquist and window; a joint view with the weighted window formula; weighted generalised Lomb–Scargle in `spectrum.rs` (`lomb_scargle_weighted(t, y, sigma, f)`), held to astropy's `LombScargle(t, y, dy)` as `docs/research/s1-…` §4 already holds the unweighted one (to 1e-9), and to the uniform case when all σ are equal. The NUFFT form (TASK-S1-14) must take the weights from the start.

### (d) Validation, tolerances written before measuring

**What the literature validates against (the cases to reuse).**
1. Carlton & McCabe eq. (13) as the far-field closed form: Δφ = −n k T² G M Δr (D² − 2h²)/(D² + h²)^{5/2}. Tolerance, written before measuring: engine within 1e-3 relative for D ≥ 20·max(Δr, arm excursion), checked on both presets (first draft table already shows 1e-4 at D = 1 km on AION-10; AICE at 1 km passes at 1.3e-2 relative for the *quasi-static midpoint* form but eq. (13) uses Γ at the lower launch point; run the test with Γ at the lower IFO and a 1 % tolerance, and state which).
2. The same paper's scenarios: a 1000 kg object at D = 10 m (lift, car), person on stairs, "O(50) µrad" animal ripples, with the AION-10 geometry (launch points at h = 0 and h = 5 m, dashed lines in Fig. 3). Their curves are plotted, not tabulated, so the check is shape (sign change near D ≈ √2 h, inverse-cube tail), not digits (they say the sign changes at "D ∼ h rather than D = √2 h" for small D, h).
3. Asenbaum et al., Fig. 4/§3: a 1 rad gradiometer phase from a lead source mass near a Stanford 10 m gradiometer (L = 32 cm, n = 30, T = 600 ms; "130 mrad per shot" noise) is the experimental precedent for the engine's source-mass phase, with the recoil term as the correction. Source mass dimensions are in their Fig. 2(a); I did not extract them, so this is a candidate case, not yet specified (g).
4. MIGA: calibration with "heavy source masses ... at a variable distance up to 150 m" is a candidate horizontal case if horizontal axes are built; the instrument is a Rb Bragg, k_eff = 2nk case, 2T = 500 ms.

**`per_tower_instruments` (verdict: OK as a claim, weak as written; unchanged from the first draft since it is an engineering statement).** "Exact" is achievable on the CPU path and I measured it: four towers (AION-10, AICE, AION-10 elsewhere, AICE elsewhere), two moving bodies, read (i) each alone with its own `Reader`, (ii) with one shared `Arc<Reader>` per distinct instrument, towers dispatched in **reverse order on separate threads**: **all four readings are bit-identical** (`to_bits` equal), and two `Reader`s of one instrument agree to the bit. Control arm: changing tower 2's T by one part in 10⁹ changes **only** tower 2's bits. The reason it is exact: `Reader::read(tower, source, readout)` is a pure function of (instrument, tower position, source, readout) with a fixed node order and no cross-tower sum. So the test as worded is nearly tautological at `Reader` level; it earns its keep only at **array/engine level**, where state is shared. What would break bit-equality there: (1) a parallel reduction that splits one tower's node sum or the body sum differently by thread count; (2) any scene-level culling/LOD by distance to the array; (3) a common time origin formed from the array; (4) FMA/`target-cpu` differences; (5) the GPU path, which is not f64: hold it to a tolerance, not to the bits; (6) a cache keyed by an instrument hash that omits a field. Wording: *"Each tower's CPU readings in an array of unequal towers equal the readings of a one-tower array with that tower's resolved instrument and position, **bit for bit**, for every shot, with the Reader shared by instrument key and towers evaluated in any order and on any number of threads. Control arms: (a) one tower's instrument changed by 1 ulp of T changes that tower's bits and no other's; (b) an override equal to the default gives bits equal to no override (and one Reader built, counted); (c) swapping two towers' instruments swaps their outputs; (d) GPU vs CPU: ≤ 1e-6 relative, not bits."* Add the ordinary accuracy check for the second preset: `a_static_mass_reads_what_the_plain_formula_gives` (≤ 1e-11 relative to the brute-force formula) is run on AION-10 only; repeat it on `Instrument::aice()` and add the test that the arms stay in the tower.

**`two_sites_reject_local_noise` (verdict: weak; the row's quantities are now identifiable from Chaibi et al.; replacement below).** What Chaibi et al. (<https://arxiv.org/pdf/1601.00417>) actually say, read in the body:
- N is the **number of gradiometer measurements in an array** (each gradiometer = two AIs on a common laser), "N = 80, δ = 200 m, L = 16.3 km" in Fig. 3. The estimate is the mean of N gradiometers, eq. (3), with residual noise **"σ_HN = √2 σ_η / √N"** (eq. 4), σ_η = √(σ_a² + σ_ε²) (Newtonian-noise and detection-noise s.d.). The √2 is from the two AIs of one gradiometer, "assumed uncorrelated between the 2 AIs ... always valid for the detection noise and applies for the NN when the gradiometer baseline L is much larger than the NN correlation length". "The AI array brings a √N rejection factor for the NN (and for the detection noise)", in **noise amplitude**, valid when the N measurements are independent, i.e. when δ ≫ L_ρ(ω) (their eq. 12).
- What is correlated: the **gravity-acceleration noise**. Saulson-type model, ground cut into cells of size equal to the half wavelength, **L_ρ(ω) = v_u/(2f)** (v_u = 2 km/s for P waves), with mass conservation giving **anti-correlation** between adjacent cells (Fig. 2: negative minimum at x_ac ≈ 1.3 in units of L_ρ). Choosing the spacing δ to hit that anti-correlation gives rejection beyond √N: maximum at f = v x_ac/(2δ) = **1.1 Hz (air-density NN) and 6.5 Hz (seismic NN)** for their δ; "With the 3-4 km baseline of current best optical detectors, our method reaches a NN rejection of about 2 ... strong NN rejection by more than a decade" for the dedicated 16 km configuration (abstract: "factor 2 ... 10-fold or more").
- What is **not** correlated in their model: detection (shot) noise between AIs. What is common: the GW signal (and laser phase, common-mode and rejected in each gradiometer).

So "1/√N and common-mode factors" in the plan row can be written as: **amplitude ∝ √2/√N for independent per-gradiometer noise (eq. 4)**; common-mode = the same signal/laser noise in both AIs of a gradiometer. The "factor 2" and "≥ 10" are properties of the **NN correlation model with L_ρ and anti-correlation**, which the engine's scene does not contain. Replacement: **(1)** engine-level, exact: a body near site A and 200 m from B (use ELGAR's δ = 200 m, <https://arxiv.org/pdf/1911.03701>, §1.3.1): the reading at B equals the `Reader` reading with B's own geometry (locality, to the bit as in the first row), and the A/B ratio follows (r_A/r_B)³ in the far field (≤ 1e-3 relative); **(2)** statistical, with the pass criterion written before measuring: with injected independent Gaussian noise from the engine's noise stack and one common injected tone, the measured variance of the N-gradiometer mean equals 2σ²/N (Chaibi eq. 4 form) within 3σ of Monte Carlo (tolerance 3·√(2/M) in variance for M realisations), the coherence γ² = S_s²/[(S_s+S_n1)(S_s+S_n2)] and the 1/√K floor of K averaged segments match, and a control arm sets the pairwise correlation ρ = 0.5 and the variance formula σ²[1 + (N−1)ρ]/N (derived here, standard) predicts the change; **(3)** a stated limitation: Chaibi's NN rejection is a function of δ/L_ρ(f) from a seismic/atmospheric field model, which the scene does not model, so the row cannot claim real NN rejection, only the estimator statistics and locality. Optional extension if a model is wanted later: implement their J(x) correlation function and reproduce their Fig. 5 rejection curves as a validation (digitise from the figure; the dashed √80 = 8.9 line is the baseline).

**Preset verification (replaces the first draft's paragraph).**

| Quantity in plan §1 | Verdict | Source |
|---|---|---|
| AION-10: 10 m baseline, δr = 5 m | verified | Carlton & McCabe (<https://arxiv.org/pdf/2308.10731>, §II.B); TDR §3.1 (two 5 m interferometers, 10 m tube) (<https://arxiv.org/pdf/2508.03491>) |
| AION-10: T = 0.73 s, u₀ = 3.86 m/s, Δt (cycle) 2 s, n = 1000 | verified (T, u₀, Δt, n) as the paper's assumed values for "AION-10 (goal)"; Badurina's Table 1 uses a different T = 1.4 s for a 10 m "launch mode" (<https://arxiv.org/pdf/1911.11755>), so 0.73 is the Carlton & McCabe/AION-analysis choice, not a hardware fact; the TDR has none of these | as above |
| Nyquist 0.25 Hz | follows from Δt = 2 s | |
| AICE: 70 m gradiometer, T = 2.5 s | verified for the **TP's** L = 140 m Baseline (not "100 m") | TP Table 1, Fig. 6, §3 (<https://arxiv.org/pdf/2608.18743>) |
| AICE: cycle 5 s, Nyquist 0.1 Hz | **not found**; LoI/CFS repetition Δt = 1.5 s (→ 0.33 Hz) at T = 1.4 s | LoI Table 1 (<https://arxiv.org/pdf/2509.11867>) |
| AICE: u₀ = 21.27 m/s | derived, not sourced (21.236 exact constants) | (b) finding 2 |
| AICE: T_obs = 3×10⁷ s | **not found**: LoI/CFS Table 1 give T_Int = 10⁸ s; TP: "t_int 1 yr" (= 3.15×10⁷ s) in Table 1 for Baseline and Stretch | LoI Table 1; TP Table 1 |
| Shot-noise floors 10⁻⁴ (baseline) and 10⁻⁵ (stretch) rad/√Hz | verified | AION-10 (goal) 10⁻⁴ (Carlton & McCabe; Badurina Table 1); TP Table 1 Baseline 10⁻⁴, Stretch 10⁻⁵ |
| ULDM benchmark "m_φ = 2×10⁻¹⁶ eV at current MICROSCOPE bounds gives ~10⁻² µrad" | **not found in the cited paper**; in 2308.10731 MICROSCOPE appears only as reference [79] for the statement that d_me = 1 "is orders of magnitude larger than current constraints on this parameter"; the paper's signal sizes are for d_me = 1, m_φ ≃ 4×10⁻¹⁶ eV (f = 0.1 Hz) (Fig. 2), and no value near 10⁻² µrad appears. The statement may be the plan author's own scaling; to be rederived, not cited | <https://arxiv.org/pdf/2308.10731>, §II and ref. [79] |
| "250 µrad per shot" (M17) | the paper's "atom shot noise (O(250) µrad cf. Fig. 2)" | <https://arxiv.org/pdf/2308.10731>, §III |

### (e) Out of reach or not worth doing now

Rb Bragg (MIGA, ELGAR, Stanford) and other two-photon sequences: not a parameter change. k_eff = 2nk, different arm geometry (symmetric about the apogee for MIGA), and for MIGA the cavity field. Resonant versus broadband **sequences** (Badurina §2: π/2-π...π-π/2 with Q π-pulses, MAGIS and ELGAR also): the engine models one Mach-Zehnder per shot; a multi-pulse response is a different kernel, and the GW/ULDM use needs the ULDM channel (not built). The AION broadband LMT sequence in Carlton & McCabe is π/2, (4n − 3) π, π/2 (their text, §II); the engine's n-kick arm geometry is its first-order form. MAGIS-100's three sources and modes A-D need an interferometer **list** per tower (N heights) instead of two; a clean generalisation (`heights: Vec<f64>`, gradiometers = pairs) but beyond S15. Real multi-site Newtonian-noise cancellation (Chaibi's L_ρ and anti-correlation, Harms) needs a seismic/atmospheric field model, not the scene's bodies. Species other than Sr: the atom-mass field exists, but the 698 nm clock transition and n = 1000 would be wrong; leave to a preset. Earth-gradient recoil and rotation terms (Asenbaum, Dickerson, HJK Table 2): static, and a systematics-slice item, not a forward-model change.

### (f) Verdicts on the assigned 2.6 rows

| Row | Verdict | Why, and the replacement |
|---|---|---|
| `per_tower_instruments` | **OK / weak** | Bit-equality is achievable and measured at `Reader` level, but at that level it is tautological. Keep "exact", move it to array level with the control arms in (d); add the AICE brute-force and arms-in-tower checks. (Engineering claim; no source applies.) |
| `two_sites_reject_local_noise` | **weak, now specifiable** | Chaibi et al. fix the meaning: N = number of gradiometers in the array, amplitude rejection √N (σ = √2 σ_η/√N, eq. 4), independent detection noise, common signal/laser; "factor 2" and "≥ 10" come from L_ρ and anti-correlation in a seismic/atmospheric NN model the scene does not have. Replace by locality (exact), estimator statistics (3σ, with ρ control arm) and an explicit "no claim of real NN rejection". |
| plan §1 AICE row (not assigned, but affected) | **wrong as labelled** | "AICE 100 m: 70 m, 2.5 s, 5 s cycle" mixes the TP (L = 140 m: 70 m, 2.5 s) with an unsourced cycle; the 100 m documents have Δz = 50 m, T = 1.4 s, Δt = 1.5 s. Relabel and add a second preset if the LoI case is wanted. |

### (g) Could not verify

- **Wuhan 10 m (WIPM) primary paper**: only ZAIGA's second-hand description; Hannover VLBAI facility height is from the Hannover VLBAI papers (10 m baseline, 8 m ROI) and ZAIGA's "height of 10 m and an effective interference area of 9 m", not from a design paper.
- **AICE**: the TP is a month-old preprint and I could not reconcile its δr = 70 m (scalar) with Δz ≈ 109 m (GW, TP p. 125) and "30.7 m below the top". No launch speed and no repetition interval in the TP. The 5 s cycle has no source.
- **AION-10**: T, n, cycle and launch speed are taken from the AION *analysis* paper (Carlton & McCabe) as assumed values; the AION-10 TDR (<https://arxiv.org/pdf/2508.03491>) contains none of them (searched), so they are not hardware specifications. The AION-10 mean-trajectory rise (gT²/2 = 2.6 m) and apex in the engine were not compared with a published trajectory.
- **Eq. (13) vs engine at small D**: Carlton & McCabe's expansion assumes D, h ≫ z(t); I verified the far-field limit only (1e-4 at 1 km), not their Fig. 3 curves.
- **Dimopoulos 2008 and Hogan et al.** give the scaling k_eff ∇g L T² and k_eff g T² but not a boxed gradiometer equation with a numerical prefactor; the prefactor 1 comes from Carlton & McCabe eq. (13), Asenbaum (T_zz ≈ −dφ/(n ħ k L T²)) and the engine's own integral.
- **MIGA after 2018** (cavity length, number of AIs in later papers 2204.12137, 1505.07137), **TVLBAI roadmap 2310.08183**, **VanderPlas weighted/multiband Lomb–Scargle**: not opened.
- **Asenbaum source-mass geometry** (Fig. 2a) not extracted; the lead-brick case is a candidate, not a specified validation.
- **ULDM benchmark** "2e-16 eV, ~1e-2 µrad": not in 2308.10731; origin unknown.
- Chaibi's rejection factors "2" and "≥ 10" were read in the abstract and §I; Fig. 5 (rejection vs f) I read only through its caption and text (1.1 Hz and 6.5 Hz maxima).

**Further questions (not chased):** time-window vs shot-count windows for Gradar; whether AICE shots may overlap (2T > cycle, LoI Δt = 1.5 s); which AICE preset is canonical (TP 140 m vs LoI 100 m); per-tower σ_d source; interferometer lists per tower (MAGIS-100 modes A-D); a scene-level `gravity`; whether to add a Bragg (k_eff = 2nk) preset (MIGA/ELGAR/Stanford) with horizontal axis.


## 4. Q2: a gradiometer on a horizontal axis in Earth gravity

**Status.** Rewritten after reading the primary papers (PDFs fetched from arxiv.org and read as text; page/section/equation numbers below are from those PDFs). Earlier "UNVERIFIED" claims are now verified, corrected or deleted. Scripts: `scripts/q2.py` (earlier numerics) and `scripts/q2_verify.py` (new, closed-form check). "Computed here" means a script of mine produced the number.

Sign and symbol conventions used below, fixed by the sources: k = k_eff k̂; Γ_ij = −∂²U/∂x_i∂x_j = ∂_i g_j (Roura Eq. A2, ELGAR Sec. 3.2, Hogan Eq. 54 with T_ij ≡ ∂_j g_i); Ω the Earth's angular velocity; T the pulse spacing; 2T total.

### (a) What the literature says

**MIGA (read: [arXiv:1703.02490v3 PDF](https://arxiv.org/pdf/1703.02490), Sci. Rep. 8:14064 per [arXiv abs page](https://arxiv.org/abs/1703.02490)).**

| Item | What the paper says | Where |
|---|---|---|
| Launch | Atoms launched **vertically** "in an almost vertical parabolic flight" at "≈4 m/s" by moving molasses; 3D-MOT loaded from 2D-MOT; N ≃ 10⁶ ⁸⁷Rb, "few µK", 1 Hz | Fig. 5 caption; Sec. 4.1 |
| Pulses | Pulses are applied "in a symmetric fashion with respect to the apogee": π/2 and recombining π/2 through the **lower** cavity, π through the **upper** cavity | Sec. 4.2 |
| Pulse type | 3-pulse Mach-Zehnder in the **Bragg regime**, orders n = 1 up to 10 considered, "two horizontal cavity fields" | Sec. 2.2; Sec. 4.2 |
| Two beams, not one | "Two horizontal cavities", axes "at a vertical distance of ≃30.6 cm to have an interrogation time of T=250 ms"; one 0.5 m diameter vacuum tube hosts both | Sec. 4.2; Sec. 4.3 |
| T and 2T | T = 250 ms, 2T = 500 ms | Sec. 3.2, 4.2 |
| Cavity length | **200 m** (finesse 100) in this version; the "150 m" in the brief is not the cavity: it is "a variable distance up to **150 m**" for the calibration source masses | Sec. 3.2; Sec. 4.2; Sec. 7 (Fig. 13 text) |
| Number, spacing | Three atom interferometers at X₁,₂,₃, "at the same height and horizontally separated by a distance L"; located at the injection end, mid-way, and far end of a gallery (so about 100 m apart); extra AIs may be added later | Fig. 1, Fig. 5 text, Fig. 13 caption |
| Beam | TEM00, mirror ROC 555 m, beam radius "7.28 mm at the waist and 8.04 mm at the mirrors", cloud radius "≃5 mm" at the last pulse | Sec. 4.2 |
| Sensitive axis | "The horizontal acceleration at the position of the three atomic clouds is then simultaneously measured using Bragg interferometry". Horizontal k, horizontal baseline along the cavity: gradiometer reads a Γ_xx-type difference | Sec. 4 (opening) |
| Initial horizontal velocity | Atoms are prepared "with a horizontal velocity that fulfills the Bragg condition for diffraction": the cloud starts at momentum −nħk and is split to ±nħk | Sec. 4.1; Sec. 2.2 |
| Gravity sag | Handled by geometry: free-fall height gT²/2 (9.81 × 0.25²/2 = 0.307 m, computed here) is the separation of the two cavity axes. Nothing else is needed because k ⟂ g | Sec. 4.2, 4.3 |
| Coriolis | "differentially canceled in the gradiometric measurement, if the launch directions … do not change between runs". Residual for a relative horizontal launch angle Δθ ⟂ k: **φ_C = −2 Ω_E sin α_lat k T² v_l Δθ ≃ 470·Δθ** (Eq. 20), latitude 43.9°; launch stability of 1 µrad needed | Sec. 4.2, Eq. (20) |
| Wavefronts | Cavity filtering "effectively reduces the sensitivity of the interferometers to laser wavefront distortions, which are a leading source of noise" | Sec. 4.2 |
| Response | Sensitivity-function formalism, Eq. (1): Δφ_AT(t) = n ∫ s(T−t) dΔϕ/dT dT | Sec. 2.2 |

Checks of mine: with k_eff = 2k (n = 1, 780 nm), T = 0.25 s, Ω = 72.9 µrad/s, λ = 43.9°, the coefficient in Eq. 20 is 101.8 rad per rad per (m/s) of v_l; 470 needs v_l = 4.6 m/s (text says ≈4 m/s, which gives ≈407). I did not reconcile this; it does not affect the recommendation (computed here).

The structure the earlier draft derived is therefore confirmed: a horizontal instrument is a vertical fountain through horizontal beams, with the π/2 beams at height z_L and the π beam at z_L + gT²/2. My earlier claim that "a horizontal long-T instrument needs a tall beam, a tracking mirror or small T" was incomplete: MIGA and ELGAR use **separate beams at different heights**, so T is set by the beam separation, not the waist.

**ELGAR (read: [arXiv:1911.03701v1 PDF](https://arxiv.org/pdf/1911.03701); CQG 37, 225017 per the earlier catalogue record, not checked on the publisher page).**
- Geometry: "atom are launched on a vertical parabolic trajectory into the interrogation region, where the interferometer is created in a symmetric way around the apogee using a set of two horizontal laser beam" (Sec. 1.1, Fig. 2). Fig. 2 caption: "lower beam … π/2 pulse while an upper beam, where the apogee is, operates as a π pulse"; recombination "when they fall through lower interrogation beam".
- Baseline design (Sec. 3.8): "double-loop interferometer with a gravity gradient compensation scheme, an effective wavenumber of keff = 1000 kl, a baseline L = 16.3 km, and a pulse separation time T = 200 ms". Each arm: N = 80 gradiometers, spacing δ = 200 m (Sec. 1.3). Pulse spacing T–2T–T for the double loop, total 4T (Sec. 3.5).
- Beam size: "Efficient Bragg pulses … about 100 µs … atoms … travel about 20 cm perpendicular to the beam … beam waist … several 10 cm" for a 1000 k_l ladder with 4 m/s transverse motion (Sec. 3.7). This is the real constraint on a horizontal beam and it is about LMT pulse duration, not about gT²/2.
- "In the horizontal configuration, the atoms travel orthogonal to the beam splitters, constraining the minimum beam size … and defining the pulse separation T" (Sec. 3.4). Horizontal is favoured (Sec. 3.8).
- Phase terms (Sec. 3.2): single loop, "rotations and gravity gradients give rise to phase terms 2 k_eff·(v₀×Ω)T² and k_effᵀ Γ T²(x₀+v₀T)"; Γ "corresponds to minus the Hessian of the gravitational potential". Double loop: Eq. (33) rotation (leading −6 k⊥·v₀ T ΩT²), Eq. (37) Γ coupling −2 k_effᵀ Γ T² v₀T, Eq. (38) coupling to g, including 2(ΩT×k_eff)·g T² and −4 k_effᵀ(ΓT²)gT² ; Earth's horizontal gradient "Γxx = Γyy ≈ 1.5 × 10⁻⁶ s⁻²" (Sec. 7.1.4), which agrees with my GM/R³ = 1.5414e-6 s⁻² (computed here).
- The follow-up [Technologies for ELGAR, arXiv:2007.04014](https://arxiv.org/pdf/2007.04014) repeats the same geometry text (Sec. 3.4, 3.7). Not read further.

**ZAIGA (read: [arXiv:1903.09288 PDF](https://arxiv.org/pdf/1903.09288)).** Journal: the arXiv page lists DOI 10.1142/S0218271819400054, a World Scientific prefix; ISSN stem 0218-2718 is the International Journal of Modern Physics D, consistent with the earlier snippet (IJMPD 29(4), 2020). I could not open the publisher or Crossref page (blocked), so the journal, volume and issue remain "inferred, not confirmed". It is not Gen. Relativ. Gravit. Content: "equilateral triangle … two 1-km-apart atom interferometers in each arm, a 300-meter vertical tunnel with atom fountain" (abstract); a 10 m AI in drop mode (free-fall 10 m, 1.4 s) and a lower AI in launch mode (Sec. 3.3); Raman light-pulse with 1000 LMT; Coriolis compensation by a tip-tilt platform (Sec. 3.4.7). Each side has "two counter-propagating light beams, which are used to control the two AIs" (text before Fig. 3). Launch scheme of the horizontal AIs is not given in the passages I read.

**Chaibi et al. 2016 ([arXiv:1601.00417](https://arxiv.org/pdf/1601.00417)).** Gradiometer = two AIs separated by baseline L interrogated by one retro-reflected laser, 3 pulses, Bragg/LMT. It is a noise-model paper (Newtonian noise, N-AI arrays). It does not discuss launch or orientation. Not a source for geometry.

**The phase with gradient and rotation terms.**
- Roura, [arXiv:1509.08098](https://arxiv.org/pdf/1509.08098), Eq. (4): δφ = k_effᵀ(g + a₀)T² + k_effᵀ Γ T²(r₀ + v₀T) + …, Γ_ij = −∂²U/∂x_i∂x_j (Eq. A2). Eq. (5): the two-species difference. Gradient compensation: middle-pulse wavenumber change Δk_eff = (ΓT²/2)k_eff (the equation printed after Eq. (7); ELGAR Sec. 3.2 quotes the same).
- Dickerson et al., [arXiv:1305.1700](https://arxiv.org/pdf/1305.1700) (PRL 111, 083001), Table I (T = 1.15 s, k_eff = 2·2π/780 nm): term 1 k_eff g T² (2.1e8 rad); term 2 **2k_eff·(Ω×v)T²** (5.1 rad); term 3 k_eff v_z δT; term 4 (ħk_eff²/2m) T_zz T³; term 5 k_eff T_zi(x_i + v_i T)T²; term 6 ½k_eff α(v_x²+v_y²)T² with wavefront curvature α. I recomputed terms 1 and 2: 2.09e8 and 5.11 rad (computed here); they match.
- Hogan, Johnson, Kasevich, [arXiv:0806.3261](https://arxiv.org/pdf/0806.3261), Table II (fountain, k_eff along z): term 1 −k_eff g T², term 3 −k_eff T_zz v_z T³, **term 4 +(7/12)k_eff g T_zz T⁴**, term 6 2k_eff v_x Ω_y T², term 2 k_eff R_e Ω_y² T². Gyroscope paragraph (Sec. 4.1): "∆φ_laser ≈ k₀ · (2v₀ × Ω)T²" and "this shift can also be written in the Sagnac form φ = (2m/ħ) Ω·A".
- 7/12 verified independently: I solved z'' = −g + Γ_zz z by first-order perturbation and phase k(z₀ − 2z_T + z_2T) with sympy; result k T²[Γ(z₀+v₀T) − (7/12) g Γ T²] (computed here, output `G*(-7*T**4*g*k/12 + T**3*k*v0 + T**2*k*z0) - T**2*g*k`, g the positive magnitude). This equals Roura Eq. (4) plus Hogan's term 4 with g_vec = −g ẑ, i.e. **k_effᵀ Γ T² [r₀ + v₀T + (7/12) g_vec T²]**.

**Sagnac is the same term as Coriolis, not a second one.** Hogan states both forms of one shift, k₀·(2v₀×Ω)T² = (2m/ħ)Ω·A (Sec. 4.1). My earlier numerical area check (A = v_k T² v_z; 2mΩA/ħ equals 2kv_zT²Ω, both 1.9188e10 per unit Ω) agrees. Counting both would double-count. ELGAR even labels the term "The Sagnac term 2 k_eff·(v₀×Ω)T²" (Sec. 3.2).

**Sign conventions disagree between papers and must be pinned down.**

| Source | Rotation term for the MZ |
|---|---|
| Hogan 0806.3261 §4.1 | k·(2v₀×Ω)T² = −2k·(Ω×v₀)T² |
| ELGAR Sec. 3.2 | 2k_eff·(v₀×Ω)T² (same as Hogan) |
| Dickerson Table I | +2k_eff·(Ω×v)T² (opposite sign) |
| my earlier draft | −2k·(Ω×v)T² (agrees with Hogan/ELGAR) |

I did not find the reason for Dickerson's opposite sign (possibly a different sign convention for Ω or the phase). Hogan's Eq. (53) Lagrangian, L = ½m(ṙ + Ω×(r+R_e))² − mφ, defines the physics unambiguously.

**Earth-tensor constants (computed here, earlier script).** GM_E/R³ = 1.5414e-6 s⁻²; for AION k, T the term kT²Γ_xx L is 3.7e4 rad at L = 5 m, 7.4e4 at 10 m, 1.1e6 at 150 m; vertical Γ_zz Δr = 5 m gives 7.4e4 rad; centrifugal gradient Ω² = 5.3e-9 s⁻² gives 2.6e2 rad at 10 m. These are site constants; the engine adds only the source's change. The existing "drops out" wording is true only for the source-induced difference by linearity. For a horizontal baseline the term is Earth's curvature (a straight beam at L is tilted L/R_E; g/R = GM/R³): derived by dimension, not in any source I read.

**Beam pointing and wavefronts.** Dimopoulos et al., [arXiv:0806.2125](https://arxiv.org/pdf/0806.2125), Sec. "Effects of Rotation": laser fixed to the Earth gives "a differential Coriolis acceleration ∼ ω_E δv" between clouds (δv ≲ 1e-8 m/s from thermal effects gives ~1e-13 g); cure is to "servo the laser's axis so that the axis remains non-rotating"; wobble δω also gives a centrifugal L(δω)²; atoms insensitive to rotations "if the phase fronts are appropriately curved" (R equal to L). Also: axis jitter gives ∼ ∇g L(ω_E T)(δωT). Wavefront effects: phase fronts "cannot be made perfectly flat", giving a transverse-displacement coupling (Sec. on 2-D analysis); numerically, Dickerson term 6 is 0.04 rad at λ/10 per cm². ELGAR Eqs. (34)-(36) give the relative-beam-misalignment term Δφ = 4k_eff Δn̂·v₀T and a pointing-jitter term; ELGAR Sec. 7.1.1 requires δθ ≲ 0.32 nrad. None of this is in the engine.

**New computed-here result: the engine's integral already is the published closed form.** For a source potential that is a uniform-gradient field V = −½ rᵀΓr, the engine's integral (m/ħ)∫[V(arm₂) − V(arm₁)]dt, on arms with triangular separation and central trajectory r₀ + v₀t + ½g t², equals −k T² k̂ᵀΓ(r₀ + v₀T + 7/12 gT²) (so ΔΦ = φ₁ − φ₂ = +kT² k̂ᵀΓ(…), Roura Eq. 4 plus Hogan term 4) to 2e-16 relative for k̂ = ẑ, x̂ and an oblique unit vector, random symmetric Γ, T = 0.73 s, k_eff = 9.0e9 m⁻¹ (script `q2_verify.py`). The 7/12 comes from ∫₀^{2T} s t² dt = (7/6) v_k T⁴ for the triangle (analytic, same check). The engine's neglect of Γ-deflected trajectories is O((ΓT²)²) per ELGAR; ΓT² ≈ 8e-7 (horizontal) to 1.6e-6 (vertical, T = 0.73 s) for Earth's own gradient.

**What the forward model still misses for a horizontal instrument:** Earth's constants (k·g tilt, Γ terms), Coriolis/Sagnac, laser and mirror noise, wavefront/cavity-mode effects, finite pulse duration, beam overlap and cloud size, velocity spread. The source-phase part is the only signal Cavendish is for.

### (b) Orders of magnitude in radians (computed here)

Arms: separation s(t) along k̂, symmetric (±v t/2), v = 6.5777 m/s, vertical fountain with apex at T (u₀ = gT, z = 2.6 m at the π pulse, atoms fall transversely). IFO₁ at x = 0, IFO₂ at x = L, sign ΔΦ = φ₁ − φ₂. 8-point Gauss, 40 panels per half. Exact = engine-form integral; tidal = −(m/ħ)∫ s(t) k̂·H(centre(t))·L dt.

| Case (T = 0.73 s, L = 10 m, k = 9.0e9 m⁻¹, source at (5, 10, 1.3) unless noted) | exact (rad) | tidal (rad) |
|---|---|---|
| 10 kg, 10 m perpendicular to baseline | −2.21e-2 | −3.15e-2 (43% off) |
| 1 t, same | −2.21 | −3.15 |
| 10 kg, on baseline axis, x = 20 | +2.44e-2 | +1.87e-2 |
| 10 kg, perpendicular, atoms without fall (g = 0) | −2.24e-2 | n/a |

Convergence of tidal to exact for 1 kg, perpendicular: R = 10 → +43%, 30 → 4.7%, 100 → 0.42%, 300 → 0.047%, 1000 m → 0.0042%: error ∝ 1/R². On axis: −48%, −6.1%, −0.56%, −0.062%, −0.0056%. For comparison the vertical-baseline anchor is ~7 mrad for 10 kg at 10 m (reference/design/instrument.md); the horizontal 10 m baseline gives ~22 mrad. Fall changes the result by about 1%.

MIGA-scale check, now with real numbers: Rb-87, k_eff = 2k = 1.61e7 m⁻¹, T = 0.25 s gives kT² = 1.0e6 rad per unit (s⁻² × m); a 1 t mass at 10 m gives Γ ~ 2GM/r³ = 1.3e-10 s⁻², so Γ L k T² ≈ 1e-2 rad for L = 100 m (computed here by hand; far-field order only, invalid for L > r; not a test). The MIGA paper's own gradient sensitivity is "of the order of 10⁻³ Eötvös at 1 s" (Sec. 7), i.e. 1e-12 s⁻².

### (c) Recommendation (a task a builder can do, vertical case untouched)

1. **Keep the existing vertical code path as is** and add a general path beside it. Do not rotate coordinates: x + 0.0 and x·1.0 are bit-identical for 10⁶ random doubles; 0.0·x = ±0; the 3-D squared distance (ax²+ay²)+dz² with exactly zero x, y arm components equals the engine's `across + below·below` (computed here). A rotation by π/2 is not exact (cos(π/2) = 6.1e-17), so never build axes from angles. Represent the baseline as (unit vector, length) with the vertical case exactly (0,0,1).
2. **Arms as 3-vectors:** offset p(t) = u t + ½ g t² (vectors) plus the kick vector after T. `Node.lower/upper` become `Vec3`; `Source::difference` takes the two IFO origins and arm offsets. Form the arms' difference before squaring so no cancellation is introduced.
3. **Gate the proof with a test:** the vertical instrument through the general path must be `to_bits`-equal to the old path on the existing anchors and the three scenes of `a_static_mass_reads_what_the_plain_formula_gives` (control arm: tilt the baseline by 1e-3, bits must differ). If bits ever differ, keep the vertical branch as a separate function chosen when the baseline is exactly (0,0,1).
4. **Horizontal geometry, as built in MIGA and ELGAR.** `Instrument.axis = { baseline, kick, gravity }`. Launch vector u = (−v_k/2) k̂ + g T ẑ (symmetric closed loop, apex at T; the cloud starts with momentum −nħk, which is what MIGA's "horizontal velocity that fulfills the Bragg condition" means). `floor = false`. Beam heights are not one number but two: lower beam (π/2 at t = 0 and 2T) at z_L, upper beam (π at T) at z_L + gT²/2: 0.31 m for T = 0.25 s (MIGA), 2.61 m at T = 0.73 s (AION), 12.3 m at T = 2.5 s (AICE). Report these in the manifest; warn (do not refuse) if gT²/2 exceeds the user's stated beam separation, and note ELGAR's waist constraint for LMT ladders (several 10 cm, Sec. 3.7).
5. **Data contract:** the horizontal tower channel is the scalar ΔΦ; add baseline and kick unit vectors to the manifest; no tensor-shape change. Earth's constant term, Coriolis and tilt are not added to the signal in S15; they go in the systematics budget as functions of the published terms listed in (d.3).
6. **Documentation line:** the horizontal baseline reads Γ_xx-type (k̂ ∥ b) in MIGA's layout; any other (k̂, b) pair reads the component k̂ᵀΓb.

### (d) Validation cases and tolerances (written before measuring)

1. **`horizontal_baseline_tensor`:** two parts.
   (i) Known-answer: uniform-gradient source potential V = −½ rᵀΓr must give ΔΦ = φ₁ − φ₂ = +k T² k̂ᵀΓ (r₀ + v₀T + (7/12) g_vec T²) for any k̂ and symmetric Γ (r₀, v₀: central path at the first pulse), tolerance 1e-11 relative (measured here: 2e-16). Control arm: flip the sign of the 7/12 term, and require failure; a second control uses a k̂ that does not match the Γ axis and requires the oblique-component result. Γ here is the source tensor; the engine's neglect of Γ-deflected arms is O((ΓT²)²) (≈1e-6 relative for Earth's tensor).
   (ii) Far field: a 1 kg point at R in a set of directions: |exact/tidal − 1| ≤ c(s_max² + L²)/R² with c = 1; assert ≤ 0.6%, 0.07%, 0.007% at R = 100, 300, 1000 m (measured constant ≈ 0.4).
   No published numeric phase for a horizontal gradiometer with a known mass exists in what I read; MIGA's calibration with source masses "up to 150 m" is a plan, not a result.
2. **`vertical_case_unchanged`:** `to_bits` equality as in (c.3). Control arm: 1e-3 tilt.
3. **`coriolis_and_sagnac_phase`:** hold to the published terms (one term, not two):
   (i) **Δφ_rot = 2 k_eff·(v₀×Ω)T²** (Hogan §4.1; ELGAR §3.2; equal to −2k·(Ω×v₀)T²), checked against the area form (2m/ħ)Ω·A with A the enclosed area computed from the arms: relative 1e-9 (done here: 7.010513 m² against 7.010513 m²).
   (ii) Against an explicit rotating-frame calculation from the Hogan Eq. (53) Lagrangian (build task): relative 1e-6.
   (iii) Published numbers: Dickerson Table I term 2 = 5.1 rad for k_eff = 2·2π/780 nm, T = 1.15 s, |Ω| = 60 µrad/s, v = 2 mm/s (I get 5.11, computed here; the test should be on magnitude, 1%); MIGA Eq. (20) coefficient 2Ω sinλ k T² v_l = 101.8 rad per (rad · m/s) at the MIGA parameters, so 470 ≃ v_l 4.6 m/s (not reconciled; use only as an order check, 15%).
   Sign: the test fixes the sign by the engine's own k̂ and Ω convention, derived from Eq. (53); Dickerson's printed sign is opposite to Hogan/ELGAR and must not be used to fix it.
   Gradient-coupled companion terms (Hogan Table II, Dickerson Table I): −k T_zz v_z T³ and +(7/12) k g T_zz T⁴ (ELGAR Eq. 37, 38 for the double loop). The 7/12 term is covered by d.1.

### (e) Out of reach / not worth doing now

Wavefront curvature and cavity-mode physics (MIGA/Dickerson term 6, Dimopoulos), finite pulses, atom-cloud temperature averaging, laser noise, ELGAR's double-loop/triple-loop geometries and gradient-compensating Δk (a second slice if wanted), Earth's constant gradient and Coriolis as signals (10¹⁰ dynamic range cannot be held to µrad in double precision without fringe fitting), any fully general tensor-valued baseline.

### (f) Verdicts on the 2.6 rows

- **`horizontal_baseline_tensor` — weak as worded; replacement stands and is now source-backed.** "Reads the horizontal gradient component" is right only in the far field (43% off at R = 10 m, computed here). Replace with the two-part test in (d.1). Name the component by (k̂, b). The tensor convention is Γ_ij = −∂²U/∂x_i∂x_j [Roura Eq. A2] and the phase is k^TΓT²(r₀+v₀T) plus the 7/12 term [Roura Eq. 4; Hogan Table II; verified here].
- **`vertical_case_unchanged` — OK, unchanged.** Bit-level argument supported by arithmetic checks; add the control arm. Nothing in the literature changes it; the engine's vertical arms are the same fountain geometry as ELGAR Fig. 2(b).
- **`coriolis_and_sagnac_phase` — rewrite.** Name the single published term Δφ = 2k_eff·(v₀×Ω)T² (equal to the Sagnac shift (2m/ħ)Ω·A), hold it to Hogan/ELGAR in sign and to Dickerson Table I (5.1 rad) in magnitude, and add the gradiometric residual Eq. (20) of MIGA as an order check. Not "the rotation terms". The row is an S14/systematics row; assign it as above.

### (g) Could not verify

- ZAIGA's journal and volume (IJMPD 29(4), 2020) from the DOI prefix only; Crossref and doi.org returned 403 through the proxy. ELGAR's CQG 37, 225017 not checked on the publisher page.
- The version question for MIGA: I read v3 (June 2018), which gives a 200 m cavity, T = 250 ms. The earlier brief's "150 m" is the source-mass distance. Earlier arXiv versions (v1/v2) were not read, and the final Sci. Rep. text was not compared with v3.
- Reason for Dickerson's opposite sign of the Coriolis term; and the MIGA Eq. (20) numerical prefactor (4.6 vs ≈4 m/s). The coefficient −6 k⊥·v₀ in ELGAR Eq. (33) and the −4 Γ g coefficient in Eq. (38) were read but not re-derived.
- The launch scheme of ZAIGA's horizontal arms; whether MIGA's two cavities are vertically exactly gT²/2 apart in the real build (the paper says ≃30.5–30.6 cm design).
- The statement "Earth's curvature = the horizontal Γ term" is derived by dimension, not found in a source.
- Hogan's Eq. 65 and Table III (counter-rotation compensation) were read but not exercised.

### Further questions (not chased)
- Tip-tilt/rotation compensation in a horizontal cavity (MIGA has none; ZAIGA does): how much of Eq. (20) survives.
- Double-loop geometry with Δk_eff gradient compensation (ELGAR baseline) as an optional second instrument type.
- Does MIGA's v1/v2 differ (150 m)? Check the published Sci. Rep. text.


## 5. Q3. Other gravity environments (Earth sites, anomalies, Moon, Mars)

**Source access (second pass).** Through the session proxy I could open, with `curl --cacert /root/.ccr/ca-bundle.crt`: arXiv (abs and pdf), the NSSDCA fact sheets, earth.esa.int and pypi.org. The WebFetch tool cannot resolve nssdc.gsfc.nasa.gov; curl can. I could **not** open: earth-info.nga.mil (the tunnel closes mid-exchange), nsgreg.nga.mil and icgem.gfz.de (HTTP 403 from the proxy), the doi.org/Wiley page of Pavlis et al. 2012, pgda.gsfc.nasa.gov and pds-geosciences.wustl.edu, unoosa.org, DTIC, Wikipedia, Cambridge, Springer, Nature. So NGA.STND.0036, EGM2008 (paper and archive), ICGEM, GRAIL and GMM-3 papers are still unread. Where a primary was closed, the text says so and gives what was done instead: an independent derivation or measurement ("computed here", script `scripts/q3.py`), a secondary arXiv paper that quotes it, or UNVERIFIED.

### (a) What the literature says

**WGS84 normal gravity.**

Defining parameters used: a = 6 378 137 m, 1/f = 298.257 223 563, GM = 3.986 004 418e14 m³/s², ω = 7.292 115e-5 rad/s. **Not read from NGA.STND.0036 (closed).** Partial independent evidence:
- a and 1/f: Mangum (arXiv:1411.1617, p. 16, Eq. B8 and its text) uses "0.00669437999013" for e² and the WGS84 ellipsoid "6378.137 (1 − 1/298.257223563)" km. [arxiv.org/abs/1411.1617](https://arxiv.org/abs/1411.1617). My e² from 1/f = 0.00669437999014 agrees to the last printed digit (rounding).
- ω = 7.292115e-5 rad/s is printed (for GRS80, same ω) in Kopeikin, [arXiv:1708.09456](https://arxiv.org/abs/1708.09456), p. 24: "ω = 7.292115 × 10−5 rad s−1". That paper gives GRS80 (1/f = 298.257222101, GM = 398600.5e9), not WGS84. The two ellipsoids differ at the 1.5e-9 level in 1/f and in GM.
- GM: not read anywhere in full precision. The NSSDCA Earth column prints 0.39860 ×10⁶ km³/s² only ([moonfact table](https://nssdc.gsfc.nasa.gov/planetary/factsheet/moonfact.html), "GM (x 10^6 km3/s2) ... 0.39860").

**Measurement that settles the Somigliana constants (computed here).** I derived γe, γp, k from (a, f, GM, ω) with the Hofmann-Wellenhof & Moritz closed forms, for two GM values:

| GM used | γe derived | k derived | printed value |
|---|---|---|---|
| 3.986 005e14 (original WGS84) | 9.780 326 7714 | 0.001 931 851 386 | γe = 9.7803267714, k = 0.00193185138639: Mangum Eq. B8 |
| 3.986 004 418e14 (2014 update) | 9.780 325 3359 | 0.001 931 852 652 41 | γe = 9.7803253359, k = 0.00193185265241: remembered, UNVERIFIED as printed by NGA |

The old pair printed in Mangum is reproduced to 1e-10 m/s² by the old GM. This confirms the derivation chain and shows the pairs are not interchangeable: they differ by 1.4e-6 m/s² (0.14 mGal) in γe. The new pair then follows from the new GM. It is consistent with my memory of NGA.STND.0036, but I have not seen the page: **the current-GM constants remain UNVERIFIED as printed**. The builder should make the constants a unit-tested `const` block with the derivation as the test, so that a later reading of the NGA PDF is a one-line check. Derived here as well: γp = 9.8321849379, e² = 0.00669437999014, m = ω²a²b/GM = 0.003449787, J2 = 1.0826298e-3 (NSSDCA prints J2 = 1082.63e-6 for Earth: [table](https://nssdc.gsfc.nasa.gov/planetary/factsheet/moonfact.html)).

**Somigliana.** Mangum, Eqs. B2-B4 ([arXiv:1411.1617](https://arxiv.org/abs/1411.1617)): g_m = (aγe cos²φ + bγp sin²φ)/√(a² cos²φ + b² sin²φ) = γe (1 + κ sin²φ)/√(1 − e² sin²φ), κ = bγp/(aγe) − 1, quoting Somigliana 1929 and Moritz 1980. Kopeikin's Eq. (214) gives the same first term in the ellipsoidal angle θ, and says (footnote 8) that the geographic latitude Φ relates to it by a tan β = b tan Φ, "[5, Eq. 2-77]", [5] = Heiskanen & Moritz. [arXiv:1708.09456](https://arxiv.org/abs/1708.09456). So the form with geographic latitude is as above; I have not seen Heiskanen & Moritz or Torge itself.

**Free-air series with height.** Mangum Eq. B8 uses the linear form: "− 0.003086 Hc m/s²" with Hc in km. That is 308.6 mGal/km, the textbook linear gradient, which agrees with my numbers below. The second-order form, γ(h) = γ0 [1 − (2/a)(1 + f + m − 2f sin²φ) h + 3h²/a²], is the one I wrote from memory as Hofmann-Wellenhof & Moritz Eq. 2-215; **the equation number and print are UNVERIFIED (book not opened)**. It was checked here against the gradient of the J2+centrifugal potential, differences in |∇W| between h and 0:

| φ | h | |∇W| change | series | series minus potential |
|---|---|---|---|---|
| 0° | 1 km | −0.003087 m/s² | −0.003087 | −0.007 mGal |
| 45° | 9 km | −0.027712 | −0.027711 | −0.077 mGal |
| 45° | 100 km | −0.301458 | −0.301323 | −13.5 mGal |
| 45° | 9 km, linear term only | | −0.027770 | 0.058 mGal worse |

So the series is good to 0.1 mGal up to 9 km (the residual is the unmodelled higher-order terms and the J2-only test model), and not meant above that. The h² term matters at the 0.06 mGal level at 9 km: it is a required part of any "0.1 mGal" test.

**Normal gradient tensor (computed here, not a table in any source).** From W = GM/r [1 − J2 (a/r)² P2(sin φ_c)] + ½ω²(x²+y²), central-difference Hessian in the geodetic north/west/up frame. T_ij = ∂i∂j W with W the geodetic potential (positive, +GM/r). E = 1e-9 s⁻².

| site | T_NN | T_WW | T_UU (= −dγ/dh) | T_NU |
|---|---|---|---|---|
| φ = 0, h = 0 | −1543.7 E | −1533.4 | +3087.8 | 0 |
| 45°, 0 | −1540.1 | −1534.9 | +3085.6 | −8.1 |
| 51.75°, 0 | −1539.2 | −1535.2 | +3085.1 | −7.9 |
| 90°, 0 | −1536.3 | −1536.3 | +3083.3 | 0 |
| 45°, 9 km | −1533.6 | −1528.4 | +3072.6 | −8.1 |

The trace is 2ω² = 10.63 E at every row (Poisson in the rotating frame; a strong exact control). **Sign convention, corrected.** My first draft said the brief's "Txx = Tyy ≈ +1.5e3, Tzz = −3.1e-6" had the signs reversed. That was wrong as a criticism: the engine uses the physics potential V = −Gm/r (phase = (m/ħ)∫ΔV dt), and ∂²V/∂z² = −2GM/r³ = −3.1e-6 s⁻², +1.5e-6 horizontally. The table above is the geodesy sign (W = −V). Whichever is used must be stated in the test. The AION literature uses the geodesy sign: in the Lagrangian of Carlton & McCabe, Eq. (10), "½ γzz z²" enters with γzz ≈ +3e-6 s⁻² and the constant gradient phase is positive, nkγzz δr T² ([arXiv:2308.10731](https://arxiv.org/abs/2308.10731), footnote 5, Eq. 10).

**Bouguer slab and buried bodies (closed forms checked here by direct numerical integration of Newton's law).** No free source opened for these textbook formulas (Telford, Hinze closed; none on arXiv found), so I state them as checked mathematics, not citations:
- Slab, ρ = 2670 kg/m³: g = 2πGρ h = 0.1120 mGal/m (= 0.0419 mGal/m per g/cm³; the brief's 0.0419 is the per-g/cm³ figure). Check: a 1 m thick disc of radius 1000 km gives 0.11197 mGal against 2πGρt = 0.11197 mGal. An infinite slab has zero gradient.
- Sphere, centre depth D: g = GM/D², T_zz = 2GM/D³ on the vertical axis. Voxel sum (60³ midpoint, radius 5 m, D = 20 m, Δρ = −2670): 2.3328e-7 m/s² against 2.3327e-7 (relative 6e-5, voxelisation). Cavity: 0.023 mGal, 23.3 E.
- Horizontal cylinder (tunnel, r = 1 m, axis depth 10 m, Δρ = −2670): g = 2Gλ/D, T_zz = 2Gλ/D² on the vertical through the axis; numerical line integral agrees to the 5th figure: 0.0112 mGal, 11.2 E.

**EGM2008.** Pavlis et al. 2012, J. Geophys. Res. 117, B04406, doi:10.1029/2011JB008916: **not opened**. What two arXiv papers that read the NGA page say:
- Shen et al., Global geoid model GGM2022, [arXiv:2306.15192](https://arxiv.org/abs/2306.15192), p. 6: EGM2008 "is complete to spherical harmonic degree and order 2159, and contains additional spherical harmonic coefficients extending to degree 2190 and order 2159", and gives the NGA address `earth-info.nga.mil/GandG/wgs84/gravitymod/egm2008/egm08_wgs84.html`. Also: "the variation range of the geoid undulation N is around the magnitude of ±100 meters", and elsewhere "geoid undulations vary globally within ±110 m"; it calls EGM2008 a "5′ × 5′ resolution (∼ 10 km)" model.
- Kopeikin [arXiv:1708.09456](https://arxiv.org/abs/1708.09456), p. 3-4: coefficients "complete up to all spherical harmonics of the degree and order 2159", full access at `earth-info.nga.mil/GandG/wgs84/gravitymod/egm2008/index.html`.

**Not confirmed, by anything I could open:** whether a test-point file ships with the NGA archive and what it contains, the `hsynth` test output, the names `EGM2008_to2190_ZeroTide` and "Interpolation/Test", NGA's terms of use, the ICGEM model page and licence, ICGEM's calculation-service functionals ("gravity_disturbance", "T_zz"), the quoted anomaly RMS and deflection-of-the-vertical figures. The ICGEM and NGA hosts must be allowed (or the files supplied) for these.

**Anomaly sizes, my own estimate (computed here, a generic scaling, not an EGM2008 number).** A sinusoidal anomaly of amplitude Δg and wavelength λ has gradient amplitude about 2πΔg/λ: 100 mGal at λ = 10 km gives about 630 E; 10 mGal at 100 km gives about 6 E. So a few E to a few hundred E is the plausible range for EGM2008 gradient anomalies at its 10 km resolution. This replaces the first draft's unsourced "10-100 E".

**GOCE-class requirement.** Carraz et al., [arXiv:1406.0765](https://arxiv.org/abs/1406.0765): future gravity missions "will require gradiometers with sensitivity in the order of 1 mE/√Hz". Useful only as a reminder that 1 E is an enormous signal for a ground instrument.

**Moon and Mars (NSSDCA, opened).** Moon: [moonfact.html](https://nssdc.gsfc.nasa.gov/planetary/factsheet/moonfact.html) (last updated 11 January 2024); Mars: [marsfact.html](https://nssdc.gsfc.nasa.gov/planetary/factsheet/marsfact.html) (19 May 2025). Table rows as printed:

| quantity | Moon | Mars |
|---|---|---|
| equatorial radius (km) | 1738.1 | 3396.2 |
| volumetric mean radius (km) | 1737.4 | 3389.5 |
| surface acceleration | 1.62 m/s² | 3.69 (eq), 3.73 (pole) |
| GM (×10⁶ km³/s²) | 0.00490 | 0.042828 |
| J2 (×10⁻⁶) | 202.7 | 1960.45 |
| sidereal rotation period | 655.720 h | 24.6229 h |
| atmosphere | night surface pressure 3 × 10⁻¹⁵ bar | 6.36 mb at mean radius, 4.0 to 8.7 mb by season |

Computed here from these rows: Moon g = GM/R² = 1.622 m/s², 2GM/R³ = 1866 E, ω = 2.6617e-6 rad/s (ω² = 0.01 E). Mars g = 3.713, 2GM/R³ = 2187 E, ω = 7.0882e-5 rad/s, ω² = 5.0 E. The earlier values (J2, periods) from memory agree. **Corrections:** Mars surface pressure is 636 Pa (0.6% of Earth's 101 325 Pa), not "about 600 Pa" (within 6%, but it is now a quoted number with a range); the Moon's atmosphere is 3e-10 Pa at night, nothing for any scene body to feel.

Higher-precision GM: GRGM1200A (GM = 4902.80011526323 km³/s², R = 1738.0 km) and GMM-3 (GM = 4.282837285418775e4 km³/s², R = 3396.0 km, k2 = 0.1697) were taken from search-result snippets of the PDS labels, which I could not open (PDS and PGDA blocked), so they stay UNVERIFIED. For a vertical tower, NSSDCA's 3 to 6 figures change g by under 0.1% and are enough for a preset. Rotation (IAU WGCCRE, Archinal et al.) was not opened; NSSDCA's sidereal periods stand in.

**k_eff convention in AION (opened).**
- Badurina et al. (AION), [arXiv:1911.11755](https://arxiv.org/abs/1911.11755), Sec. 2: single-photon clock transition, "k = 2π/λ denotes the wave-vector of the photons"; LMT: "n − 1 π-pulses can be applied from opposite directions"; each pulse gives momentum ħk, and this "enhances the differential gravitational wave signal by a factor of n". So the arm momentum split is n·ħk and the effective wave number is n·k, with k the single-photon value.
- Carlton & McCabe, [arXiv:2308.10731](https://arxiv.org/abs/2308.10731), Sec. III: "each pulse transfers a momentum ±nℏk, where k is the wave-vector of the laser"; broadband sequence "an initial π/2-pulse, (4n − 3) π-pulses ... and a final π/2-pulse, where n is the number of LMT kicks". Same paper: AION-10 goal "a baseline L = 10 m, a separation ∆r = 5 m, and n = 1000 ... launched with a speed of vl = 3.86 m s⁻¹, we find T = 0.73 s", with ∆t = 2 s. Every number matches `Instrument::aion10`, so that preset is confirmed against its source.
- Contrast: Raman interferometers use k_eff = 2k (two photons). Dickerson et al., [arXiv:1305.1700](https://arxiv.org/abs/1305.1700), Table I caption: "keff = 2k = 2 · 2π/780 nm". Do not mix: the engine's n·k, k = 2π/698 nm, is the AION convention and the "2nk" column of my first draft is a Raman-style figure that does not apply and is dropped.

**How the Earth gradient is handled (opened).**
- The Earth-gradient phase of the gradiometer is a constant: "The gravity gradient gives a constant contribution ∆ϕℓ = nkγzz δrT², which we do not include when plotting" (Carlton & McCabe, footnote 5). They also "ignore effects from Coriolis forces as we assume they are corrected with a rotation compensation system". So Cavendish's omission of the static gradient and of rotation follows the AION modelling literature.
- Per-interferometer compensation: Roura, [arXiv:1509.08098](https://arxiv.org/abs/1509.08098), Eq. (11): "∆keff = Γ T²/2 keff" applied to the second pulse; Eq. (4) shows the uncompensated phase "keff (g + a0) T² + keff Γ T² (r0 + v0 T)". This removes the dependence on initial position and velocity and the contrast loss; it is not a removal of the common g term. D'Amico et al., PRL 119, 253201 (2017), "Canceling the gravity gradient phase shift in atom interferometry", is cited by Carlton & McCabe as ref. [91]; I saw its title there and did not open it.
- Dickerson et al. Table I gives sizes for their 10 m Raman fountain: term 1, keff g T², 2.1e8 rad; term 4, ħkeff² Tzz T³/2m, 0.44 rad; term 5, keff Tzi (xi + vi T) T², 0.18 rad (Tzz = 3075 E, T = 1.15 s). Computed here: 2.088e8 and 0.444, so my formulas are the table's. The chirp in this paper is only the AOM that "chirps the other seed to correct for the atoms' Doppler shift". It is not a gradient-compensation method, so the brief's attribution of gradient compensation to Dickerson is not supported; Roura's Eq. (11) is the primary.

### (b) Orders of magnitude in radians for the engine's instruments

Earth's own field enters the full instrument phase, not the source phase. Convention: k_eff = n·k = 9.0017e9 rad/m (AION, above).

| quantity | AION-10 (Δr 5 m, T 0.73 s) | AICE (Δr 70 m, T 2.5 s) |
|---|---|---|
| k_eff g T² | 4.7e10 rad | 5.5e11 |
| k_eff Γ_zz Δr T² at 3086 E | 7.4e4 rad | 1.2e7 |
| per 1 E of gradient | 24.0 rad | 3.9e3 |
| per 1 mGal of g (one IFO) | 4.8e4 rad | 5.6e5 |
| per 1 µGal of g | 48 rad | 563 |
| launch shift for Δg = 0.052 m/s² (equator to pole), du0 = T Δg | 0.038 m/s | 0.130 m/s |
| arms' closure miss at fixed u0, 2T² Δg | 0.055 m | 0.65 m |

All computed here. These phases are 8 to 12 orders above the µrad signals. The engine's decision to leave out the static field is correct and matches Carlton & McCabe; it must only carry what the static field changes in the geometry. The chirp that cancels k_eff g T² is α/2π = 1.4e10 Hz/s at 9.81 m/s².

Variation of Γ_zz over the Earth (computed here): equator 3087.8 E, pole 3083.3 E (0.15%); 13 E lower at 9 km. A few-per-mille change in a 7.4e4 rad phase is about 100 rad, so a constant uncompensated gradient cannot be treated as a perturbation: it is calibration. With 6 E to 600 E of anomaly (order-of-magnitude above), AION-10's phase moves 150 to 15 000 rad.

**What g does reach the source phase through** is the flight geometry only. Benchmark (computed here, Python port of `Arm` and the Gauss rule, 1 kg at (5, 0, 1) m, static source, floor on):

| case | phase |
|---|---|
| g = 9.81, u0 = 3.86 | −1.6625e-3 rad |
| g = 9.81, u0 = gT − v/2 = 3.8723 | −1.6421e-3 |
| g = 9.78, u0 = 3.86 | −1.6409e-3 |
| g = 9.83, u0 = 3.86 | −1.6768e-3 |
| g = 9.78, u0 = gT − v/2 (symmetric) | −1.6567e-3 |

The source phase moves by about 1.5% (25 µrad on 1.66 mrad) between the equator and the pole at fixed T. That is above any validation tolerance, so g must be an input to flight and launch. The launch rule matters: fixed u0 against symmetric u0 differs by 1.2% at 9.81, and the floor is touched at fixed u0.

For AION-10 at g = 9.81 the lower arm's apex is at u0/g = 0.393 s and it lands at 2u0/g = 0.787 s, after T = 0.73: the kick at T arrives with the arm still airborne, and the upper arm reaches 5.006 m. The symmetric u0 is 3.872, within 0.012 m/s of the published 3.86 (Carlton & McCabe, above). Whether AION uses exactly the symmetric rule is not stated in what I read; 3.86 is quoted as the launch speed.

Moon-tide gradient, 2GM/d³ = 1.7e-4 E, would give 4e-3 rad on AION-10 (computed here); it belongs to the `tidal_gradient_closed_form` rows (not mine).

### (c) Recommendation (task-sized)

1. `gravity: f64` is already an instrument parameter (`Instrument::gravity`). Do not add Γ to the forward model for the vertical case. A per-tower spec carries `gravity`; the launch rule `u0 = gT − v/2` (already `symmetric_launch`) is re-derived unless the user pins `launch_velocity`.
2. A pure function `normal_gravity(lat_deg, height_m) -> f64`: Somigliana (Mangum Eq. B2 form) plus the second-order series above, with a, 1/f, GM, ω as the defining `const`s and γe, k, e² derived (or asserted against the derivation) in a unit test. Add `normal_gradient_enu` only if horizontal-axis instruments need the Earth tensor; the table in (a) is its test, with the sign convention stated.
3. Optional EGM2008: a small committed fixture of (lat, lon, δg, gradient) per site, with provenance (service, date, query). This depends on getting ICGEM or the NGA archive; neither could be read here. Do not implement a 2190-degree synthesis in the engine.
4. Everything that is "scene" stays scene: Bouguer slabs, terrain, a cavity, a tunnel, a buried sphere are voxels the engine already handles. Only g (and optionally Γ for a compensation term) is instrument.
5. Moon and Mars presets: GM, R, ω, J2 from the NSSDCA rows above (GRGM1200A/GMM-3 GM optional when read), g = GM/R² − ω²R cos²φ, and the launch rule.

Moon and Mars flights (computed here, tower rise limit 5 m per interferometer):

| body | symmetric launch at n = 1000 needs | u0 = 0, 5 m rise | the Earth config on it |
|---|---|---|---|
| Moon (g = 1.623) | T ≥ v/2g = 2.03 s, a 10.0 m rise (a 15 m tower) | T = 1.43 s, v = 4.65 m/s (n ≈ 707); phase −3.05e-3 rad | upper arm rises 7.2 m, lower arm lands at 4.76 s: arms do not meet at 2T |
| Mars (g = 3.714) | T ≥ 0.886 s, 4.37 m rise (fits) | T = 0.947 s, n ≈ 1070 | fits only with a different u0 |

Phases for the 1 kg test: −1.93e-3 (Moon, T = 1.18, u0 clamped to 0) and −2.0e-3 (Mars, u0 = 0, T = 0.947) against −1.66e-3 on Earth: the same order. Earth-gradient phase for Δr = 5 m: Moon T = 1.43, 1.7e5 rad; Mars T = 0.947, 8.8e4 rad; Earth AION-10, 7.4e4 rad. Nothing changes in kind. Moon and Mars are slightly easier in radians for the same mass and range.

### (d) Validation cases and tolerances (written before measuring)

1. `normal_gravity_somigliana`: derived constants reproduce γe, γp, k, e² to 1e-10 relative; the formula at 0, 45, 90°: 9.7803253359, 9.8061977694, 9.8321849379 (computed here). Control: the original-GM pair (9.7803267714, 0.00193185138639, printed in Mangum Eq. B8) differs by 1.4e-6 m/s² and must fail at 1e-9. Tolerance 1e-10 relative. If NGA.STND.0036 becomes readable, add its printed digits as a second assertion.
2. `free_air_gradient`: dγ/dh and γ(h) from the series against the potential-gradient numbers above, 0.1 mGal over 0 to 9 km (measured 0.085 mGal worst case) and 0.5 E for dγ/dh (measured 0.1 E). Control: the textbook linear 3086 E fails at the equator by 1.7 E.
3. `poisson_trace`: trace of the normal tensor = 2ω² = 10.63 E to 0.05 E (measured 10.63 everywhere).
4. `bouguer_slab_constant`: 2πGρ = 0.1120 mGal/m for 2670 kg/m³ (checked by disc integration, 0.11197); the engine's `Slab` null test already covers zero gradient.
5. `buried_sphere_and_cylinder`: engine against g = GM/D², T_zz = 2GM/D³; cylinder 2Gλ/D, 2Gλ/D²; relative 1e-6 for analytic prisms/spheres, voxel scenes to the discretisation error of the scene (6e-5 at 60³ measured).
6. `local_g_flight`: at g ≠ 9.81 the symmetric launch closes at 2T to 1e-12 m; the phase sensitivity above (1.5% per 0.05 m/s²) is reproduced by the Rust engine at the same inputs to 1e-9 relative.
7. `aion10_preset_matches_paper`: L = 10 m, Δr = 5 m, n = 1000, v_l = 3.86 m/s gives T = 0.73 s (Carlton & McCabe): check the engine's own T from its launch rule against 0.73 to 0.01 s. Uses a source I have read.
8. `earth_gradient_phase_formulas`: k_eff g T² and ħk_eff² Tzz T³/2m against Dickerson's Table I numbers, 2.1e8 and 0.44 rad (reproduced to 0.5% and 1% here).
9. EGM2008 values: only if a fixture is added; tolerance equal to the quoted precision of the service output (UNVERIFIED), recorded with date and query.

### (e) Out of reach / not worth doing

- Full EGM2008 synthesis (degree 2190) inside the engine: the benefit is hundreds of mGal that the instrument treats as calibration; a cached per-site table suffices. A degree-2 synthesis tied to the normal field is a cheap self-check (J2 = 1.0826298e-3 derived here from the ellipsoid; NSSDCA prints 1082.63e-6).
- Absolute Moon and Mars gravity at the µGal level and local variations (lunar mascons): no source read.
- Modelling the static Earth gradient: calibration, omitted by design, as in Carlton & McCabe.

### (f) Verdict on the 2.6 rows

- `local_gravity_from_field_model` — **weak, restated.** Two halves. The "normal gravity formula" half is sound: Somigliana in Mangum Eq. B2 form, the original-GM constants reproduced by derivation, the current-GM constants consistent with my memory of NGA but not read. The "field model's own test values" half is **not confirmed**: I could not open the NGA EGM2008 page or any test file (earth-info.nga.mil unreachable), so whether a test-point file ships is unknown; the two arXiv papers that cite the page describe only coefficients to degree 2190/order 2159. Replace with: (i) Somigliana at fixed latitudes, free-air series and Poisson trace as in (d)1-3 (closed forms, no external fixture); (ii) an EGM2008 comparison only against values from a named ICGEM or NGA output, with date and query recorded; (iii) a degree-2 synthesis tie to the normal field; (iv) state the sign convention of the gradient in the test.
- `globe_equals_origin` — not needed for this question.

### (g) Could not verify

- NGA.STND.0036 (earth-info.nga.mil: connection closed; nsgreg.nga.mil: 403): the defining parameters as printed, γe, γp, k, e² (new GM), the "2014" values. Evidence is the derivation and agreement with a printed older pair (Mangum), not the primary.
- Hofmann-Wellenhof & Moritz Eq. 2-215 and the second-order series' provenance; Heiskanen & Moritz Eq. 2-77 (seen only as cited by Kopeikin); Telford/Hinze closed forms (checked by numerical integration instead).
- Pavlis et al. 2012 (Wiley blocked); the EGM2008 test-point file, `hsynth` test output, "Interpolation/Test" file, terms of use; the ICGEM page, calculation-service functionals and licence (icgem.gfz.de 403); EGM2008 anomaly RMS and deflections of the vertical; GOCE or Pail/Rummel anomaly figures. Only the ±100/110 m geoid range is sourced, from a secondary arXiv paper.
- GRGM1200A (Lemoine et al. 2014) and GMM-3 (Genova et al. 2016) values (PDS/PGDA/publisher blocked); the IAU WGCCRE report. NSSDCA values stand in.
- D'Amico et al. 2017 (title seen only in a reference list).
- That AION uses the symmetric launch rule (3.86 m/s is stated; the rule is not).

Further questions: the Earth-rotation (Coriolis, ω × v) phase for the horizontal arm separation and its scale; the Moon's tidal term against the source signal; terrain correction for a real site; allow earth-info.nga.mil and icgem.gfz.de (or supply the NGA PDF and the EGM2008 archive page) so (g) can be closed.


## 6. Q4. An atom-interferometer gradiometer in orbit

**Status of the evidence.** This revision replaces the first draft, which was written with no network. This time the arXiv PDFs were downloaded and read (text, formulae, tables) for: AEDGE, MAGIS-space, STE-QUEST, SAGE, AION, Carraz 2014, Trimeche 2019, Hogan-Johnson-Kasevich 2008, CARIOQA phase 0, Becker 2018, Lachmann 2021, Elliott 2023, Williams 2024 and the China Space Station payload paper. The GOCE facts come from ESA's own pages and the Level 1b handbook, which were opened. **Not openable from this session** (hosts blocked: link.springer.com, nature.com, doi.org, hal, DLR elib, ICGEM/GFZ, esa.int, ScienceDirect): Douch 2018, Aveline 2020, Frye 2021 (BECCAL), the CARIOQA-PMP papers, Floberghagen 2011, Rummel 2011, Storey and Cohen-Tannoudji 1994, any geodesy text, the ICGEM licence. Each is listed in (g); none is cited for a number. Quotations are under 25 words. Everything marked *computed here* is reproduced by `scripts/orbit_bench.py` (numpy, scipy, sympy; `python3 -I`; sections 1-5 are the derivation checks, section 6 was added to test the published numbers).

### (a) What the literature says

**a1. The proposals, as the papers state them.** "Shot rate" is the cycle time or flux the paper gives. All values are from the PDF text unless marked otherwise.

| Proposal | Geometry and orbit | Baseline | T | Atom | Shot rate | Sensitivity as stated |
|---|---|---|---|---|---|---|
| **AEDGE** ([arXiv:1908.00802v2](https://arxiv.org/abs/1908.00802), §4.1-4.2, Table 1) | two satellites; "a pair of satellites in medium earth orbit (MEO)". The white paper gives no orbit radius | **L = 4.4 x 10^7 m = 44 000 km** (Table 1, row AEDGE) | T_int = 300 s (so 2TQ < 300 s); n_max_p = 1000 pulses; wavepacket separation limited to 90 cm | Sr, 698 nm clock transition, single-photon | not stated | phase noise 10^-5 rad/sqrt(Hz) (Table 1); strain curve from Eq. (4.1)-(4.2): "Delta phi_res ~ 2 Q k_eff h L" in the low-frequency limit; "in broadband mode extends from ~10^2 down to ~10^-4 Hz" (§3.1) |
| **MAGIS-space** ([arXiv:1711.02225](https://arxiv.org/abs/1711.02225), §3.5, Table 1) | "circular orbits with semi-major axis 2.4 x 10^7 m"; satellites at 0 and 138 degrees true anomaly, inclination 28.5 degrees; "The period of the orbits is 600 minutes." | "L = 4.4 x 10^7 meters" | broadband curve T = 7.5 s with 20 hbar-k optics, Q = 1 (§3.2); design limit 2TQ < 300 s; n_p < 10^3 | Sr | readout "Requires flux of 10^8 atoms/sec" | AI readout noise 10^-4 rad/sqrt(Hz) (Table 1); band 30 mHz to 10 Hz; strain only as the plotted curve (Fig. 1) |
| **AION space stage** ([arXiv:1911.11755](https://arxiv.org/abs/1911.11755), §1, §2) | "satellite-based (thousands of kilometres scale)" is the long-term stage 4 | none of its own | none | Sr | none | "To benchmark the sensitivity of a space-based detector, we use the results from the proposed AEDGE". No AION-specific space curve |
| **SAGE** (Tino et al., [arXiv:1907.03867](https://arxiv.org/abs/1907.03867), Eur. Phys. J. D 73, 228, id found via [INSPIRE](https://inspirehep.net/api/literature?q=arxiv:1907.03867)) | GW detector: conservative L = 2 x 10^9 m, two satellites; the lattice-clock variant is on "a heliocentric orbit" (Fig. 6); the redshift test uses a GTO "as in the STE-QUEST M4 proposal (apogee at 33600 km, perigee at 2500 km)"; the AI variant's orbit is not stated | L = 2 x 10^9 m (AI) | not stated | Sr, "7 x 10^6 atoms with 20 pK" | sampling rate 0.2 Hz | atomic shot noise 10^-3 rad/sqrt(Hz), 2 hbar-k optics (n_P = 7), peak strain about 3 x 10^-20 /sqrt(Hz) (§3.1) |
| **STE-QUEST** ([arXiv:1312.5980v2](https://arxiv.org/abs/1312.5980), §2-3) | highly elliptical, "16 h" per revolution; perigee altitude rises from "about 700 km to 2200 km"; the AI runs 0.5 h per perigee pass; inertial pointing | none (a WEP test with two species in one interferometer, not a gradiometer) | "T = 5 s, and a cycle time Tc = 20 s" | 87Rb and 85Rb, 10^6 atoms each, k = 8 pi/780 nm | 20 s | per shot sigma = sqrt(2/N)/(C k T^2) = 2.93 x 10^-12 m/s^2 for C = 0.6 (reproduced here: 2.926e-12); Eotvos ratio about 5 x 10^-14 per revolution, 2 x 10^-15 after 625 revolutions; Earth gradient at perigee 2.2 x 10^-6 s^-2. (The paper writes the shot figure as "sigma/sqrt(Tc)"; the formula gives the per-shot value.) |
| **Carraz 2014** ([arXiv:1406.0765](https://arxiv.org/abs/1406.0765)) | low-orbit gradiometer concept; no altitude stated in the text | "separated by d = 50 cm" | 2T = 10 s (T = 5 s) | 87Rb, double diffraction, k_eff = 8 pi/lambda, N = 10^6 | "BEC of few 10^6 atoms loaded in Tcycle = 1 s" | Eq. (6): Delta gamma = sqrt2/(sqrt(N) k_eff d T^2) sqrt(Tcycle) = **3.5 mE/sqrt(Hz)** (reproduced: 3.51); four-pulse variant 4.7 mE/sqrt(Hz); future missions "will require" about 1 mE/sqrt(Hz) |
| **Trimeche 2019** ([arXiv:1903.09828](https://arxiv.org/abs/1903.09828), CQG 36 via [INSPIRE](https://inspirehep.net/api/literature?q=arxiv:1903.09828)) | "dedicated satellite at low altitude (250-300 km)"; the recovery simulations use a GOCE orbit segment with "average altitude of 239 km" (March-May 2013); nadir pointing, rotation-compensated | D = 0.5 m ("a distance D = 0.5 m") | T = 5 s | 87Rb, 780 nm, k = 4 pi/780 nm = 1.61e7 1/m with double diffraction (phase 2 k z Tzz T^2), 10^6 detected atoms | production "cycle time of only 1.2 s", up to 8 interferometers at once | per shot 3.5 mE (sigma_Phi = 1/sqrt(N)), "this phase shift is as large as 1087 rad" (reproduced: 1087.5 rad and 3.51 mE); nominal white noise **5 mE/sqrt(Hz)** in the gravity-field simulation; GOCE quoted at "10-20 mE/Hz^1/2" |
| **CARIOQA phase 0** (Leveque et al., [arXiv:2211.01215](https://arxiv.org/abs/2211.01215), SPIE 12777, 127773L) | "altitude of 600 km on a sun synchronous orbit", nadir pointing, atoms at the centre of mass; single axis along the velocity vector | none (accelerometer, not a gradiometer) | 2T = 2 s (Table 2) | 87Rb BEC, Raman double diffraction, N_det = 10^5 | cycle t_c = 3.5 s | "1 x 10^-10 m s^-2 Hz^-1/2" targeted projection-noise floor; 3 x 10^-10 (conservative) or 8 x 10^-11 (enhanced) from Table 2 |

*Corrections to the first draft.* (i) AEDGE's baseline is **44 000 km** in the white paper (Table 1 gives 4.4 x 10^7 m; MAGIS-space gives the same L and a chord of 4.48 x 10^7 m for its 138-degree geometry, computed here). "40 000" appears in the AEDGE paper only as the pulse number of the terrestrial scenario. (ii) "About 24 000 km" is the MAGIS-space **semi-major axis** (altitude about 17 600 km, computed here), not an AEDGE value; the "10 h" figure is MAGIS-space's **orbital period** (600 min; the stated a = 2.4e7 m gives 617 min, so one is rounded), not an interferometer duration (300 s). (iii) The "CARIOQA gradiometry" title does not exist: the paper is "CARIOQA: Definition of a Quantum Pathfinder Mission"; the instrument is a one-axis accelerometer, so there is no CARIOQA gradient figure to validate against. The CARIOQA-PMP papers (DLR, Springer) could not be opened.

*Missing in Trimeche:* the paper has no mission table with orbit, T, D, N and sensitivity; its Table I is the phase-term table used in a3. The numbers above are from §II, §IV, §V and §VIII.

**a1b. In-orbit and near-orbit demonstrations (none is a gradiometer).**

| Experiment | What was done | Source |
|---|---|---|
| MAIUS-1 (Becker 2018, Nature 562, 391) | sounding rocket, "six-minute space flight", apogee 243 km (Fig. 1), 87Rb BEC, Bragg scattering, 110 experiments | [arXiv:1806.06679](https://arxiv.org/abs/1806.06679) |
| Lachmann 2021 (Nat. Commun. 12, 1317) | same rocket; BEC matter-wave interference by light pulses, pulse spacing 1 ms or 2 ms; "establishes matter-wave interferometry in space" | [arXiv:2101.00972](https://arxiv.org/abs/2101.00972) |
| CAL on the ISS, dual species (Elliott 2023, Nature 623, 502) | 87Rb and 41K Mach-Zehnder, Bragg, "interrogation time ... of T = 0.5 ms", visibility 0.15-0.16, differential phase 0.37 (+0.45, -1.19) rad | [arXiv:2306.15223](https://arxiv.org/abs/2306.15223) |
| CAL, ISS vibration (Williams 2024, Nat. Commun. 15, 6414) | 87Rb Mach-Zehnder, T = 0.5 ms and 10 ms, "visibility V = 0.36(2)", ISS acceleration data (SAMS) used to model the visibility loss at larger T | [arXiv:2402.14685](https://arxiv.org/abs/2402.14685) |
| China Space Station payload (He et al.) | dual-species Rb AI payload launched 12 November 2022; the paper reports ground tests and an expected WEP precision "in the order of 10^-10"; no in-orbit result in the paper | [arXiv:2306.04097](https://arxiv.org/abs/2306.04097) |
| Aveline 2020 (Nature 582, 193), BECCAL (Frye 2021) | not opened (the ISS and BECCAL papers are cited only through Elliott 2023) | UNVERIFIED |

So the longest interrogation actually flown is 10 ms (CAL), against 5 s in the proposals: the proposals' sensitivities are design projections. An in-orbit gradiometer has not flown.

**a1c. GOCE (opened).** ESA EGG page: "Distance between accelerometers: 0.5 m" and "Gravity measurement bandwidth: 5 mHz to 100 mHz" ([EGG overview](https://earth.esa.int/eogateway/instruments/egg/description)). The Level 1b handbook, Table 2 ([GOCE L1b Products User Handbook](https://earth.esa.int/eogateway/documents/20142/37627/GOCE-Level-1b-Products-User-Handbook), issue 2, 2008): baseline 0.5 m, MBW 5 x 10^-3 to 10^-1 Hz, accelerometer precision "2 x 10^-12 m/s^2/sqrt(Hz)" on the sensitive axes and 10^-10 on the less sensitive one. Per axis that is sqrt2 x 2e-12/0.5 = 5.7 mE/sqrt(Hz) at specification level (computed here). The published performance, "10-20 mE/Hz^1/2 in the measurement bandwidth", is quoted by Trimeche (abstract region). I did not open Floberghagen 2011 or Rummel 2011, so the Vxx/Vyy 10 and Vzz 20 split in the first draft is unverified. Altitude: the ESA data catalogue lists gradient grids "at 225 km and 255 km", with "GGG_225 ... lower orbit phase" and "GGG_255 ... nominal orbit phase" ([ESA GOCE grids](https://earth.esa.int/eogateway/catalog/goce-global-gravity-field-models-and-grids)); Trimeche quotes "typically 255 km" in one place and 259 km in another and 239 km for March-May 2013. The sharper date and altitude history (lowering from 255 to about 224 km) rests on ESA pages I could not open (esa.int) and is UNVERIFIED.

**a2. The tensor (derived; checked numerically).** With V = +GM/r and LVLH axes (x along track, z radial, y orbit normal), a point-mass Earth gives V_ij = n^2 diag(-1, -1, +2), n^2 = GM/r^3. Computed here (GM = 3.986004418e14, R = 6 378 137 m):

| h | n (rad/s) | GM/r^3 | Vzz = 2GM/r^3 | period |
|---|---|---|---|---|
| 224 km | 1.1769e-3 | 1385 E | 2770 E | 88.98 min |
| 255 km | 1.1687e-3 | 1366 E | 2732 E | 89.61 min |
| 400 km | 1.1314e-3 | 1280 E | 2560 E | 92.56 min |

Trimeche gives "Omega_orb = 1.2 mrad/s (for an altitude of about 250 km)" and "Tzz = -2.7 x 10^-6 s^-2": magnitude 2.7e-6 agrees with 2732 E; the sign is a convention (see a3).

**J2.** For V = (GM/r)[1 - J2 (R/r)^2 P2(sin phi)] the radial-radial component is V_zz = (2GM/r^3)[1 - 6 J2 (R/r)^2 P2(sin phi)], up to 0.6 %, not 0.1 %: +0.30 % at the equator, -0.59 % at latitude 83 degrees. Verified at four points of a 96.7-degree orbit at 255 km against the sympy Hessian and a finite-difference Hessian (agreement 0.01 E in 2740 E); V_xz appears (up to 8 E). Control: coefficient 5 in place of 6 misses by 9.7 E. **Source status:** this formula is *derived and verified numerically, not checked against a geodesy text* (Rummel, Reigber, Torge not available here). What I did check is the definition: Trimeche's Eq. (12) is the standard fully normalised series, and its degree-2 order-0 term equals the form above for Cbar20 = -J2/sqrt5 (computed here). Eq. (13) of that paper, V_ij = R (d^2V) R^T, is the frame rotation used.

**a3. What a satellite-fixed gradiometer measures; check against papers.** In the translating frame the atoms obey the exact tidal potential U_t(x) = V(x_s + x) - V(x_s) - x . grad V(x_s) (Legendre series for a point mass); the monopole and linear terms cancel in free fall. For a laser frame rotating at Omega the relative acceleration is x'' = V x - 2 Omega x v - Omega x (Omega x x) - Omega' x x, i.e. coefficient Gamma_eff = V - tilde(Omega)^2 - d tilde(Omega)/dt on x, with tilde(Omega)^2 = Omega Omega^T - Omega^2 I. That identity is textbook classical mechanics; **no paper I read states it in matrix form**, and none of them contains the dOmega/dt term (Trimeche takes a constant orbital rate). What the papers do give, and my cases reproduce:

- Single interferometer phase, atoms released at relative velocity v0: Delta phi = k[a + Gamma_eff x0 - 2 Omega x v0] T^2 + k Gamma v0 T^3 + O((nT)^2). Two cases (computed here by exact Kepler flight, 255 km, Rb, L = 0.5 m, T = 5 s): **A**, atoms at rest in the rotating satellite frame, radial coefficient 3.0000 n^2; **B**, atoms at inertial rest, laser co-rotating, radial coefficient 1.0000 n^2; inertial laser, tidal only, 2 n^2.
- **Carraz** (§II): "The gradient force is gamma = Vzz - Omega^2", Omega^2 = omega_x^2 + omega_y^2 the instrument rotation. For Omega perpendicular to the measurement axis my case B gives V_zz - Omega^2, so Carraz corresponds to **B**.
- **Trimeche Table I** (general column): z coefficient 2k z (Tzz - Omega_m^2 + Omega_y^2) T^2 with Omega_m the mirror rotation rate and Omega_y the satellite rate; for Omega_m = Omega_y it is 2 k z Tzz T^2 (tidal only, my inertial laser case); for Omega_m = 0 (mirror fixed to the satellite) it is Tzz + Omega_y^2, which equals my case A, 3 n^2, **if Tzz = +2 n^2 (acceleration = +T x)**. The paper's own value Tzz = -2.7e-6 would give 1 n^2 instead. I cannot resolve this from the text: either Table I or the quoted sign is in a different convention. Builders should use the exact-flow numbers computed here (A = 3 n^2, B = 1 n^2), not the table's sign.
- **Coriolis / Sagnac:** Hogan et al. ([arXiv:0806.3261](https://arxiv.org/abs/0806.3261), Table II, term 6): "2 k_eff v_x Omega_y T^2", equal to my -2 k . (Omega x v) T^2 for v along x and Omega along y. Trimeche Table I has "4 k v_x (-Omega_m + Omega_y) T^2", twice as large because its double diffraction doubles the effective k.
- **7/12 coefficient.** Hogan Table II term 4: "(7/12) k_eff g Tzz T^4" (the paper's size for its EP-test geometry: -9.21 x 10^2 rad). Carraz Eq. (2): phase proportional to (z0 - a_M/gamma)(gamma T^2 + (7/12) gamma^2 T^4) + v0 gamma T^3. I expanded 1 + cosh(2x) - 2 cosh(x) = x^2 + (7/12) x^4 + (31/360) x^6 (sympy, computed here), which is Carraz's closed form for equal half-times, so my (7/12)(lambda T)^2 relative correction is the published coefficient.
- **Quadratic Hamiltonians.** Trimeche Eq. (1): "Phi = 2 k1.rA - 4 k2.(rB+rC)/2 + 2 k3.(rD+rD2)/2", with the statement "This formula is valid for Hamiltonians at most quadratic in position and momentum", citing Borde 2004 and Storey and Cohen-Tannoudji 1994 (neither opened). Hogan §3 derives the same: for a second-order external Hamiltonian the centre-of-mass wavefunction "must be the same for both arms and so does not contribute to the phase difference". So the laser-phase-on-the-classical-path statement is verified in two primary sources; the Storey paper itself is not.

**a4. Exact flow (a check, not a literature claim).** For atoms at rest in the rotating frame, Hill's flow gives z(t) = (4 - 3 cos nt) z0 and a gradiometer phase k L 6 cos(nT)(1 - cos nT) = 3 n^2 k L T^2 [1 - (7/12)(nT)^2 + ...]; for a fixed axis with tidal-only dynamics the radial flow is hyperbolic (lambda = sqrt2 n) and the phase is k L 2 cosh(lambda T)(cosh lambda T - 1) = 2 n^2 k L T^2 [1 + (7/12)(lambda T)^2]. The relative correction is of order (nT)^2, 1e-5 to 1e-4 at T = 5 to 10 s.

**a5. Mapping onto the engine's potential-integral formula.** With g = 0 and straight arms the engine's dphi = (m/hbar) integral [U(upper) - U(lower)] dt applied to U_t reproduces k Gamma L T^2 exactly for a quadratic U (ratio 1.0000000000000002, computed here). It is a first-order result on unperturbed arms, so it omits the O(Gamma T^2) bending of the arms. Against the exact flow (fixed axis, tidal only): 0.022 rad at T = 5 s and 0.35 rad at T = 10 s (Rb, L = 0.5 m, 255 km), relative 4.0e-5 and 1.6e-4, equal to (7/12)(lambda T)^2 to three digits. The rotation terms are not a potential: centrifugal is -1/2 |Omega x x|^2, Coriolis is the velocity-dependent Sagnac term. Trimeche's Table II gives sensitivities to them: a mirror-rate mismatch delta_Omega = 1e-6 rad/s and an initial-velocity error delta_v = 1e-6 m/s are each tabulated as differential-phase residuals (read in the paper, not reproduced here), so the orbital mode must carry both.

### (b) Orders of magnitude in radians (computed here)

Rb two-photon k = 4 pi/lambda = 1.6106e7 1/m (single diffraction; **double diffraction, used by Carraz, Trimeche and CARIOQA, doubles the phase and halves the noise-equivalent gradient**); the engine's Sr convention k_eff = 2 pi 1000/698 nm = 9.0017e9 1/m. Leading-order radial phase with Gamma_eff = 3 n^2 (case A; the tidal-only 2 n^2 in brackets):

| Instrument | h | Delta Phi (rad) |
|---|---|---|
| Rb, T = 1 s, L = 0.5 m | 255 / 400 km | 33 (22) / 31 (21) |
| Rb, T = 5 s, L = 0.5 m (Trimeche-like, single diffraction) | 255 / 400 km | 825 (550) / 773 (515) |
| Rb, T = 10 s, L = 0.5 m | 255 / 400 km | 3300 (2200) / 3092 (2062) |
| Sr 1000 kicks, T = 0.73 s, L = 0.5 m | 255 / 400 km | 9.8e3 (6.6e3) / 9.2e3 (6.1e3) |
| Sr 1000 kicks, T = 0.73 s, L = 5 m (AION-10 geometry) | 255 / 400 km | 9.8e4 (6.6e4) / 9.2e4 (6.1e4) |

So the orbital case is thousands to 1e5 rad, fringe-counted; f64 resolves a microradian on 1e5 rad (1e-11 relative, far above eps = 1.1e-16). Trimeche's 1087 rad is the double-diffraction, 2.7e-6 s^-2, tidal-only figure (reproduced: 1087.5 rad).

**Noise-equivalent gradient, corrected.** With phase noise sigma_Phi per interferometer, the single-diffraction conversion is sigma_Gamma = sqrt2 sigma_Phi/(k L T^2); 1 mrad per interferometer difference = 4.97 mE at Rb, T = 5 s, L = 0.5 m, and **2.48 mE for double diffraction** (computed here). The draft said the 4.97 figure "reproduces" the paper's mE/shot statement; that was loose. What reproduces it is Trimeche's formula sigma_Tzz = sqrt2/(2 sqrt(N) k D T^2) with N = 10^6: 3.51 mE/shot; and Carraz Eq. (6) with k_eff = 8 pi/lambda and Tcycle = 1 s: 3.51 mE/sqrt(Hz) (both computed here). 1 mrad per interferometer is the shot-noise level of N = 10^6 atoms (sigma_Phi = 1/sqrt(N)), the assumption both papers make.

Exact-flow checks (Kepler, point mass, 255 km, Rb, L = 0.5 m): radial A = 2.9999 n^2 k L T^2, along-track 0.0000, cross-track -1.0000; B = +1.0000, -2.0000, -1.0000; same to 1e-4 at T = 10 s. The simulation agrees with the Hill closed form to 9e-9 (T = 5 s) and 1.3e-9 (T = 10 s) relative. The leading-order formula is off by 0.016 rad (T = 5 s) and 0.26 rad (T = 10 s), and for the Sr/AION geometry the same estimate gives about 0.05 rad: **an orbital mode that stops at the leading term cannot claim a microradian**. Octupole relative to quadrupole is about x/r = 7.6e-7 for x = 5 m, 0.05 to 0.07 rad on 1e5 rad (AION geometry): keep the Legendre series.

**Gravity-gradient sizes against the proposals' phase noise.** For AEDGE-like strain the phase is Delta phi ~ 2 Q k_eff h L; with k_eff = n omega_A/c (omega_A of 698 nm) and L = 4.4e7 m, h = 1e-21 gives, at Q = 1, n = 1: k = 9.0e6 1/m, so 2 x 9.0e6 x 1e-21 x 4.4e7 = 7.9e-7 rad (computed by hand here); compared with the stated noise 1e-5 rad/sqrt(Hz) that is under one second of averaging at Q = n = 1. This is arithmetic on Eq. (4.2) for orientation, not a sensitivity claim.

### (c) Recommendation (small enough for a task)

**Orbital mode, v0.** A new `OrbitalFrame` background, selected by the instrument preset, **leaving the ground case bit-identical** (default `Frame::Ground` takes the present code path; a test asserts bitwise equality of the vertical AION-10 and AICE readings).
1. Instrument preset `orbit`: `gravity = 0`, `floor = false`, arms straight (u0 = -v_kick/2 so they are symmetric), `half_sequence` up to 10 s, `ifo_separation` L, axis in {radial, along-track, cross-track}.
2. A `Source` implementing U_t for the circular-orbit point-mass Earth through the Legendre series to l = 4 (relative to the quadrupole the l = 4 term is about (x/r)^2 = 6e-13 and l = 5 about 4e-19 for x = 5 m; estimated, not run), with `difference()` formed as the existing point-mass kernel does. J2 enters as a Hessian (closed forms in a2) plus an optional cubic term.
3. A `Rotation` term: Gamma_eff correction as an extra quadratic potential -1/2 |Omega x x|^2, plus Coriolis as the area/Sagnac term shared with `coriolis_and_sagnac_phase`. The spec must name the release convention (case A or B above; Trimeche's Omega_m selects between them). Until that lands, orbital mode must refuse rotating attitudes (inertial pointing only), rather than return 2 n^2 for an Earth-pointing satellite.
4. A second, independent path: the exact linear flow (Hill matrix for circular orbits) with the laser-phase formula k.[x(0) - 2x(T) + x(2T)] (Trimeche Eq. 1 for the centre of mass). Use it as the in-engine oracle and to report the O((nT)^2) residual of the arm integral.
5. Time dependence: over 2T = 20 s the satellite moves 150 km and the LVLH axes turn by 2nT = 1.3 degrees; keep Gamma constant in LVLH for a circular point-mass orbit and sample it along the orbit for J2 or an eccentric orbit.
6. Data contract: no new tensor shapes; per-tower spec carries `frame` and the axis; outputs radians as today, unwrapped (documented).
7. Presets from the table in a1, kept as data, not code: `carraz2014` (d = 0.5 m, T = 5 s, Rb, double diffraction), `trimeche2019` (same with 239 km), `ste_quest` (T = 5 s, 700-2200 km perigee, inertial). Do not add AEDGE/MAGIS-space presets until a GW response function exists.
8. **Flag for the plan owner:** the AION paper's Table 1 gives T = 1.4 s for AION-10 ("10 m atom interferometer setup in launch mode") with 2T the interrogation time, whereas the engine uses T = 0.73 s (2T = 1.46 s, `defaults.rs` and `VALIDATION-AND-RESEARCH.md` row AION-10). Resolved in §3 (b): the 0.73 s comes from Carlton & McCabe ([arXiv:2308.10731](https://arxiv.org/abs/2308.10731) §II.B: L = 10 m, Δr = 5 m, n = 1000, v_l = 3.86 m/s gives T = 0.73 s, Δt = 2 s), which is the AION analysis paper's own assumption; Badurina et al.'s Table 1 uses T = 1.4 s for a different launch assumption. Neither is a hardware specification (the AION-10 TDR gives no T).

### (d) Validation cases and tolerances (written before the numbers were measured; measured values after)

| Case | Reference | Tolerance | Measured |
|---|---|---|---|
| point-mass tensor on a circular orbit | n^2 diag(-1, -1, 2), closed form | 1e-12 relative | not run (closed form) |
| J2 V_zz | (2GM/r^3)(1 - 6 J2 (R/r)^2 P2), against sympy Hessian | 1e-9 relative | agrees to 0.01 E / 2740 E (finite-difference limit) |
| J2 full tensor | finite-difference Hessian of the potential | 1e-5 relative | 2.7e-3 E / 2740 E = 1e-6 |
| gradiometer phase, case A | Hill closed form 6 k L cos(nT)(1 - cos nT) | 1e-7 relative | 9e-9, 1.3e-9 |
| series coefficient | 1 + cosh 2x - 2 cosh x = x^2 + (7/12) x^4, Carraz Eq. (2), Hogan Table II term 4 | exact (sympy) | holds |
| leading term | 3 n^2 k L T^2 with 7/12 (nT)^2 correction | residual within 10 % of the stated correction | matches to 3 digits |
| arm integral, g = 0, quadratic U | k Gamma L T^2 | 1e-12 relative | 2e-16 |
| arm integral against exact tidal-only flow | k L 2 cosh(lambda T)(cosh lambda T - 1) | (7/12)(lambda T)^2 of the phase, +-10 % | 3.98e-5 and 1.59e-4 against 3.98e-5 and 1.59e-4 |
| Trimeche signal and noise | 2 k D Tzz T^2 = 1087 rad; sigma = sqrt2/(2 sqrt(N) k D T^2) = 3.5 mE/shot | 1e-3 relative (paper gives 2-4 figures) | 1087.5 rad (4.6e-4), 3.51 mE (3e-3, paper rounds to 3.5) |
| Carraz noise | Eq. (6) = 3.5 mE/sqrt(Hz) | 1e-2 relative | 3.51 (3e-3) |
| STE-QUEST shot noise | sqrt(2/N)/(C k T^2) = 2.93e-12 m/s^2 | 1e-2 relative | 2.926e-12 (1e-3) |
| control arms | wrong coefficient 5 for 6; Gamma_eff = 2 n^2 instead of 3 n^2 for case A | must fail by more than 100x the tolerance | 9.7 E; 50 % |

**GOCE data, for plausibility only.** Products (all opened on ESA's catalogue): Level 1b gradiometer `EGG_NOM_1b` (latest baseline _0202, [Level 1](https://earth.esa.int/eogateway/catalog/goce-level-1)); Level 2 gravity gradients in the gradiometer frame `EGG_NOM_2_` (_0203) and in the terrestrial frame `EGG_TRF_2_` (_0101) ([Level 2](https://earth.esa.int/eogateway/catalog/goce-level-2)). **The brief's `EGG_NOM_2` is Level 2, not Level 1b.** Ready-made gradient grids at 225 and 255 km (GOCE+ GeoExplore: `GGG_225`, `GGG_255`) and the sixth-release gravity-field coefficients (direct and time-wise, ICGEM format `.IDF`) are on [the grids and models page](https://earth.esa.int/eogateway/catalog/goce-global-gravity-field-models-and-grids). **How to download:** create an EO Sign In account ("requires users to log in with an EO Sign In account ... before any downloads can proceed"), then the Dissemination Service or `ftps://goce-ds.eo.esa.int/` (implicit FTP over TLS); no PI request is needed for these free collections. **Terms** ([ESA-EOPG-PDGS-PR-1 issue 1.1](https://earth.esa.int/eogateway/documents/20142/1564626/Terms-and-Conditions-for-the-use-of-ESA-Data.pdf), read in full): free dataset, "electronic online registration with immediate access"; B.6a use "exclusively for their own EO research and applications purposes"; B.5 further distribution only after the recipient accepts these terms; B.10 publications state "Data provided by the European Space Agency (ESA)"; C.2 mark data "(c) ESA (year of reception)". Consequence: do not commit GOCE files into the Cavendish repository or a published fixture without an owner decision; fetch them in a script at validation time. This is a licensing question, so it goes to the owner (CLAUDE.md "Ask first").

Pass criterion for a plausibility check, written before measuring: the J2-only radial gradient along a GOCE track must be within 1 % (27 E) of the Level 2 Vzz after rotating GRF to LVLH. I did not run it (no download is possible without an EO Sign In account); it tests the field model and frames, not the atom-phase code. The ICGEM licence for `GO_CONS_GCF_2_TIM_R6` and `DIR_R6` is UNVERIFIED (ICGEM blocked), but the same sixth-release coefficient files are listed on ESA's page under the same ESA terms.

### (e) Out of reach / not worth doing now
- Real GOCE data inversion, gravity-field recovery, space-wise or time-wise solutions.
- Drag-free control, attitude-control and thruster noise models, accelerometer hybridisation.
- Satellite-to-satellite laser links, AEDGE/MAGIS-space/SAGE strain response (10^4 to 10^9 m baseline, light-travel-time phase, a GW response function), a CARIOQA instrument model.
- Cold-atom source physics on the ISS (CAL). Expansion and wavefront aberrations are separate systematics, not orbit-specific.
- Time-dependent field models beyond static J2.

### (f) Verdicts on the assigned 2.6 rows

| Row | Verdict | Why and replacement |
|---|---|---|
| `orbit_gradient_tensor` | **OK on the closed-form half; the data half is now concrete but only a plausibility check** | Closed forms (point mass, exact J2 V_zz, Hessian) are right and checked numerically; the J2 formula is not checked against a geodesy text. "Published GOCE gradients" can be named: Level 2 `EGG_NOM_2_`/`EGG_TRF_2_` and the 225/255 km grids above, free under ESA terms with registration. GOCE measures with electrostatic accelerometers and its published products depend on a field model, so it cannot be a tolerance-bearing oracle for the atom-phase code. Replace the oracle by: closed forms, Hill closed form and exact Kepler flight, Carraz Eq. (2) series, Trimeche's 1087 rad and 3.5 mE/shot; keep GOCE at the 1 % level of (d). |
| `injection_recovered` (sensitivity-curve part) | **Weak, defer; wording to change** | AEDGE and MAGIS-space publish design projections (Table 1 parameters plus Eq. 4.1-4.2 / the MAGIS Eq. 1-2); the curves are plots, and I found **no data file, repository or supplementary table** for them in the papers or by search. The papers carry the arXiv non-exclusive distribution licence, so the figures cannot be reused as data; the curve is reproducible from the published formulae. The engine has no noise model and no GW response, so "recovered at the published sensitivity" would test the injector against itself. Replace the orbit part by: sigma_Gamma from sigma_Phi (Trimeche, Carraz) with 1/sqrt(N) averaging, reproduced to 1 %. Move the GW and dark-matter half to a later slice, with Eq. (4.2) in the low-frequency limit as its first test. |
| `distant_nulls`, ISS | **OK, with a stated number** | 4.2e5 kg at 400 km overhead of an AION-10 tower (k_eff = 9.0e9 1/m, Delta r = 5 m, T = 0.73 s): tidal bound 2GM/d^3 = 8.76e-22 s^-2 (8.8e-13 E); quasi-static bound k Gamma Delta r T^2 = **2.1e-11 rad**; the engine's own integral formula, evaluated with a Python copy of the point-mass kernel, gives +2.101e-11 rad: 4.8e4 times below 1 microradian and below the 1e-10 rad audit tolerance. The single interferometer sees -8.4e-7 rad, so the null exists only in the differential. Jupiter and the tsunami were not computed. |

### (g) Could not verify
- **Unopenable here** (hosts blocked): Douch et al. 2018 (Adv. Space Res. 61, 1307; only its bibliographic record and Trimeche's remark that it studied "the single cross-track axis in Nadir configuration" were read), Aveline 2020, Frye 2021 BECCAL, the CARIOQA-PMP papers (DLR and Springer chapter), Floberghagen 2011, Rummel 2011, Storey and Cohen-Tannoudji 1994, Borde 2004, the ESA operations pages for GOCE's altitude history, ICGEM and its licence. The GOCE noise split (Vxx/Vyy against Vzz) and the 224 km history are therefore unconfirmed.
- AEDGE gives no orbit radius; the 17 600 km altitude is from MAGIS-space's semi-major axis. The AEDGE and MAGIS-space strain curves were read only as plots.
- SAGE's atom-interferometer orbit and T are not stated in the text I read.
- The Tzz sign convention in Trimeche (a3): the table and its quoted -2.7e-6 cannot both be taken at face value.
- The matrix form Gamma_eff = V - tilde(Omega)^2 - d tilde(Omega)/dt and its dOmega/dt term appear in none of the papers read; they are textbook mechanics, confirmed numerically here for constant Omega only.
- The J2 closed form is not checked against a geodesy text; the physical constants GM, R, J2 are the usual WGS84/GRS80 values from memory.
- Nothing about an in-orbit gradiometer measurement exists in the literature I read; all sensitivities are projections.
- No search was made of the post-2024 Chinese space-station results beyond the paper above; it reports ground tests only.

### Further questions
1. How large is the Coriolis/Sagnac term for a 1e-6 rad/s residual rotation-rate error? Trimeche's Table II tabulates it; read the numbers and reproduce them with the Hill flow (one script).
2. Where does the exact quadratic flow stop being adequate: L = 70 m, T = 2.5 s (AICE) in orbit, and a non-circular orbit?
3. Does the data contract need a `frame` field per tower, or only per array? Decide in the S15 brief.
4. Is the engine's AION-10 T = 0.73 s deliberate (see c8)?


## 7. What could not be verified (all four questions)

- **Geodesy hosts.** `earth-info.nga.mil` dropped the connection and `icgem.gfz.de` and `nsgreg.nga.mil` returned 403 through the proxy, so NGA.STND.0036 (WGS84 as printed), the EGM2008 archive page, any test-point file, the EGM2008 terms of use and the ICGEM calculation service were not read. The normal-gravity constants were *derived* from the defining parameters and checked against the older printed pair (Mangum, [arXiv:1411.1617](https://arxiv.org/abs/1411.1617) eq. B8); the current-GM pair is consistent with that derivation but unread as printed.
- **Papers behind publisher hosts**: Pavlis et al. 2012 (EGM2008), Lemoine et al. 2014 (GRGM1200A), Genova et al. 2016 (GMM-3), Douch et al. 2018, Aveline et al. 2020, Frye et al. 2021 (BECCAL), the CARIOQA-PMP papers, Floberghagen et al. 2011, Rummel et al. 2011, Storey and Cohen-Tannoudji 1994. Moon and Mars J2, rotation and surface pressure stand on the NASA NSSDCA fact sheets, not on the mission gravity papers.
- **AICE and AION numbers.** No source states AICE's launch speed or a 5 s cycle; the AICE Technical Proposal is a month-old preprint whose δr = 70 m (scalar gradiometer) and Δz ≈ 109 m (GW stretch case) were not reconciled. The AION-10 TDR contains no `T`, `n`, cycle or launch speed.
- **MIGA**: the 2018 paper's v3 was read (200 m cavities, `2T = 500 ms`); earlier versions, the published Sci. Rep. text and the later MIGA papers were not. Equation 20's `470 rad per rad` needs `v_l ≈ 4.6 m/s` against the text's ≈ 4 m/s; not reconciled.
- **Sign conventions**: Dickerson's Coriolis term has the opposite sign to Hogan and ELGAR (unexplained); Trimeche's quoted `T_zz = −2.7×10⁻⁶ s⁻²` cannot be taken together with the table that gives a positive sign for the case-A release (unresolved).
- **Not found at all**: a primary paper for the Wuhan 10 m interferometer; any in-orbit gradiometer measurement (every sensitivity in the proposals is a projection); data files for the AEDGE or MAGIS-space sensitivity curves; a published numerical phase for a horizontal gradiometer beside a known mass (MIGA's source-mass calibration is a plan, not a result).
- **Derived, not found in a source**: the matrix form `Γ_eff = V − Ω̃² − dΩ̃/dt`; the J2 radial gradient (checked against a sympy Hessian and a finite-difference Hessian, not against a geodesy text); the Earth-curvature identity for a horizontal baseline (by dimension only); Bouguer, buried-sphere and cylinder closed forms (checked by numerical integration; no free textbook source opened).
- **Not exercised**: ELGAR's double-loop and triple-loop coefficients (read, not re-derived); Hogan's Eq. 65 and Table III.

### Further questions (collected; not chased)

1. Time-window versus shot-count windows for Gradar on mixed arrays; a per-tower σ source (the AION TDR does not give one).
2. Whether shots may overlap in the engine (AICE LoI: 2T = 2.8 s against Δt = 1.5 s).
3. Interferometer lists per tower (MAGIS-100 modes A–D) instead of exactly two.
4. A Bragg preset (`k_eff = 2nk`) for MIGA, ELGAR and the Stanford tower, with the horizontal axis.
5. Coriolis/Sagnac size for a 10⁻⁶ rad/s rate error in orbit (Trimeche Table II) and where the exact quadratic flow stops being adequate at AICE geometry (L = 70 m, T = 2.5 s) and on a non-circular orbit.
6. Does the data contract need a `frame` field per tower, or only per array? A scene-level `gravity` for falling bodies, separate from any tower's.
7. Allow `earth-info.nga.mil` and `icgem.gfz.de`, or supply the NGA PDF and the EGM2008 archive page, to close §5 (g).

## 8. Scripts

All in `docs/research/s15-instruments-anywhere/scripts/`: `q1bench/` (Rust, links `cav-core` by path, `cargo run --release`: finite-difference phases, quadrature audit, bit-equality of unequal towers with a control arm), `q1_window.py` (joint spectral windows), `q1_formula_check.py` (Carlton & McCabe eq. 13), `q2.py` and `q2_verify.py` (horizontal baseline, Coriolis/Sagnac area, uniform-gradient identity), `q3.py` (normal gravity, tensors, Moon and Mars flights), `orbit_bench.py` (tensor, J2, Hill flow, exact Kepler flight). The Python ones need numpy (and scipy and sympy for the orbit one); run them with `python3 -I`.
