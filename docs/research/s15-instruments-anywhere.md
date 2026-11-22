# S15 research: instruments anywhere — unequal towers, a horizontal axis, other gravity, orbit

*Research note for slice S15. GPU-free. Written 22 Nov 2026. Questions from the owner: (1) towers of different
heights and sequences in one array; (2) a gradiometer on a horizontal axis; (3) other gravity environments, the
Moon and Mars; (4) a gradiometer in orbit. Plus a check of the instrument rows of
`plan/VALIDATION-AND-RESEARCH.md` §2.6.*

## 0. Read this first: what this note is and is not

**I could not read any primary source.** In this session the fetch tool fails with DNS errors for arXiv,
the journals, ESA, NASA, ICGEM and Wikipedia, and the shell proxy answers 403; only GitHub and PyPI were
reachable. What I had was a web search that returns a few lines about each paper. So the note keeps three kinds of
statement apart, and tags each one:

- **[M]** *measured here*: a number from a script in [`s15-instruments-anywhere/`](s15-instruments-anywhere/)
  (the engine's own `cav-core` forward model, or a short numpy script), output in
  [`results/`](s15-instruments-anywhere/results/). These I stand behind.
- **[D]** *derived here*: a formula I worked out in this note and, where the tag says so, checked with an [M] number.
- **[S]** *search summary*: a paper exists and says roughly this, according to a search result's few lines. The link is the
  paper's address; **I did not open it**. Treat the number as a pointer to read, not as verified.
- **[R]** *recalled*: from memory, no source seen. Where it is used it is a number to check, and §7 lists them.

The brief asked for every claim linked to its primary source. That is true only of the [S] items' addresses, and
those are unread. What the note can honestly offer is the physics derived and **checked against the engine**, the
signal sizes in radians, the validation cases, and a reading list for the builder. Before S15's brief is written,
someone with network access should read the papers in §7's list: it will take an afternoon and the derived
formulas below give them something definite to check.

## 1. One-page summary

| | Answer | Size in radians | Task-sized recommendation |
|---|---|---|---|
| **1 Unequal towers** | `Reader` already takes one `Instrument`; only `ArraySpec` shares one. Give each tower its own instrument (a table plus an index), make the bundle say which, and write a signal in radians *and* in s⁻² (÷ `n k Δr T²`), because radians of different towers are not commensurable. | AION-10 reads 24 rad per Eötvös (1 E = 10⁻⁹ s⁻²); AICE 3 940 rad/E: 164× more [D]. | One task: per-tower instrument in the spec, `Reader` per distinct instrument, tower-alone test **with a control arm**, and a check that every arm stays inside its tower. **The AICE preset fails that check**: its highest arm reaches 108.9 m in a 100 m tower [M]. |
| **2 Horizontal axis** | Phase `ΔΦ = n k_L T² · n̂·Γ·L` (beam axis n̂, pair separation L); a horizontal pair along the beam reads Γ_xx = −g/R, half the vertical gradient and opposite in sign. Checked on far masses to 5×10⁻⁵ (broadside) [M]. The atoms fall 10.5 m during a 1.46 s shot: *that*, not the maths, is what limits a horizontal beam. First-order Coriolis from the fountain motion is largest for an east–west beam and zero for a north–south one [D]. | A near mass reads 10% differently if the sag is ignored (−4.12 vs −4.58 mrad) [M]. Earth's own Γ_xx gives 3.7×10⁴ rad (static, not modelled). | One task: a general-axis path beside the untouched vertical one (dispatch), three-point-mass tests and a rotation-covariance test, a `beam_clearance` number reported instead of silently assumed. |
| **3 Gravity** | Normal gravity (Somigliana, WGS84) and the J2 tensor give g and Γ to a few ×10⁻³ of Γ; a global model adds anomalies ([S] EGM2008 via ICGEM, unread). Local g moves a nearby mass's phase by ±7×10⁻⁴ of itself between equator and pole [M]. **The AION launch rule has no solution on the Moon (needs T > 2.03 s) and on Mars needs T > 0.887 s** [M]. | g from 9.7803 to 9.8322 m/s²: ≤ 6 µrad on an 8 mrad signal [M]. Γ_zz: Earth 3 083 E, Moon 1 865 E, Mars 2 189 E [D]. | One task: `Site { latitude, height }` → g, Γ (normal gravity + J2) with the formula's own test values; presets for the Moon and Mars that choose a feasible T. Field-model anomalies are a second, later task. |
| **4 Orbit** | In a free-falling, rotating frame the atoms feel `M = Γ + ω²(I − ŷŷᵀ)`; nadir pointing in a circular orbit gives `M = ω² diag(0, −1, 3)` [D, checked against a numeric J2 tensor, §5]. Phase `n k_eff L T² M_nn`. The hard part is rotation: 1 µm/s of velocity mismatch at the orbital rate gives ~1 rad at T = 5 s [M]. | 550 rad static (L 0.5 m, T 5 s); 0.20 rad per Eötvös of anomaly; the J2 swing along the orbit is 4.9 rad [M]. | First orbital mode = a *circular orbit with J2*, a nadir or inertial attitude, `n k L T² M_nn(t)` integrated over the shot; validated against the closed point-mass tensor, the J2 tensor and Laplace's equation. GOCE data is an audit, not a unit test. |

The sections below give the working. §6 audits the §2.6 rows: **five are weak or vague, one is under-specified in a way that
would hide a bug, and one (`local_gravity_from_field_model`) is partly wrong** (Somigliana gives normal gravity, not the
gradient or the anomaly). Two numbers in the plan need correcting: the AICE geometry and the AICE launch speed.

---

## 2. Question 1 — towers of different heights and sequences in one array

### 2.1 What the engine does today

- `cav_core::forward::Instrument` holds everything the phase depends on (`Δr`, `T`, `u₀`, `g`, `m`, `ħ`, `λ`,
  kicks, floor, sign, quadrature) and `Reader::new(instrument)` is built from *one*
  (`crates/cav-core/src/forward.rs:67-110, 548-600`).
- `Source::difference(tower, heights, lower, upper, time)` takes the two interferometers' launch heights and the
  arms' rise, so nothing in a source assumes one instrument (`forward.rs:452-463`).
- `ArraySpec` has *one* `InstrumentSpec` for every tower (`crates/cav-sim/src/spec.rs:90-93`).
  `tower_height` is used by the renderer and by range checks only: **the forward model never reads it** (the spec
  says only "`ifo_separation` less than `tower_height`", `spec.rs:100-109`).

So per-tower instruments need no change to the physics. They need the spec, the bundle and a safety check.

### 2.2 What changes

**Model.** One `Reader` per *distinct* instrument (keyed by the instrument's value, so equal towers share a reader
and its Gauss rule). Each tower's shot is launched at `readout − 2T_tower`, so the towers' shots overlap in
different ways; `Source::breaks` already works per shot. Cost per read scales with the node count
(`Instrument::node_count`), which is longer for a long `T`: AICE's 5 s flight against AION-10's 1.46 s.

**Spec.** `towers: Vec<TowerSpec>` gains an instrument: either an `Option<InstrumentSpec>` override of the array's
default (smallest change, existing `.ron` files load unchanged) or an `instruments` table with an index per tower
(better if many towers share a few designs). The `Set` path, undo and schema machinery of S2 need only the new path.

**Geometry check, which the model lacks.** An arm's highest point must be inside the tower. From the engine's own
arm formulas (`Arm::rise`) and u₀ for each preset [M]:

| Preset | u₀ | Δr | Tower | Highest arm point |
|---|---|---|---|---|
| AION-10 | 3.86 m/s | 5 m | 10 m | **10.006 m** |
| AICE | 21.236 m/s | 70 m | 100 m | **108.88 m** |

The AION-10 overshoot is already in `plan/PROGRESS.md` as a question for George. The AICE one is new and larger: with
`Δr = 70 m` the upper interferometer needs 108.9 m of shaft, so either the 100 m is the height of the *launch
points* and not the flight, or Δr is smaller, or the preset is not what the scripts imply. I do not know which; it is
a question for George, and the checker should be added now so that no array can pass a spec in which atoms leave
their tower.
(*How:* the upper arm leaves at `u₀ + v_kick`, rises to T, loses the kick and rises on to its apex, plus Δr:
[`arm_tops.py`](s15-instruments-anywhere/arm_tops.py), output in `results/arm_tops.txt`. It reproduces `plan/PROGRESS.md`'s 10.006 m for AION-10, which is the check.)

**Data contract (`REBUILD.md` §3.3).** The bundle already carries per-tower readout times `(T, D)`. Add:

1. an **instrument table**: one row per distinct instrument with every field above plus `n`, `k`, the sensing axis,
   and the instrument's hash; one `tower → instrument` column. The numerics block records each tower's quadrature.
2. each tower's signal in **radians as read** and in a **common currency**, the gradient estimate
   `Γ̂ = ΔΦ / (n k_L L T²)` in s⁻² (or E). Radians from different towers are not the same unit of anything: for a source at
   200 m the same 1 kg gives −2.0×10⁻⁷ rad on AION-10 and −2.9×10⁻⁵ rad on AICE (floor off) [M], a factor of 146, while
   the gradients differ by far less. Gradar's training set needs the instrument descriptors as inputs or it learns
   the instrument and not the source.
3. the shot times per tower. They differ in cycle as well as in length (`T`, 2 s against 5 s), which is the S2 cadence
   designer's job; nothing new is needed except that the Nyquist and the periodogram window are per tower.

**The scale between instruments** [D, from the quasi-static phase of §3.1]: `S = n k L T²` rad per s⁻².
AION-10: 1000 × 9.0×10⁶ m⁻¹ × 5 m × 0.533 s² = 2.4×10¹⁰ rad/s⁻² = **24 rad/E**; AICE: 3.9×10¹² = **3 940 rad/E** [M].

### 2.3 Real and proposed arrays that mix baselines

All [S], unread:

- **The AION programme is staged and meant to run as a network of unequal instruments**: 10 m → 100 m → km, with
  the 10 m tower giving "early network data in conjunction with the MAGIS prototype tower and MAGIS-100"
  ([AION talk, CERN](https://indico.cern.ch/event/1084435/contributions/4559381/attachments/2334624/3979156/aion100-cern-workshop.pdf);
  MAGIS-100 is a 100 m vertical strontium clock-atom gradiometer in the Fermilab shaft,
  [arXiv:2104.02835](https://arxiv.org/abs/2104.02835)). The AION white paper is cited as arXiv:1911.11755; I did not find it.
- **ZAIGA** plans a 300 m vertical tunnel *and* a horizontal tunnel in stage one, then a 1 km triangle with two
  interferometers 1 km apart in each arm
  ([arXiv:1903.09288](https://arxiv.org/abs/1903.09288); stage dates from the later reviews
  [arXiv:2110.11564](https://arxiv.org/pdf/2110.11564), [arXiv:2111.09715](https://arxiv.org/pdf/2111.09715)).
- **MIGA**: three atom interferometers along a 150 m horizontal cavity (a 300 m figure appears in a 2016 review, so
  the design moved) ([arXiv:2204.12137](https://arxiv.org/pdf/2204.12137), [arXiv:1703.02490](https://arxiv.org/pdf/1703.02490)).
- **ELGAR**: horizontal arms of 16.3 km per gradiometer, 80 gradiometers per arm and a 32.1 km total baseline in
  one design table
  ([arXiv:1911.03701](https://arxiv.org/pdf/1911.03701)).

I know of no published array that *mixes* a vertical short tower and a long horizontal one in a single analysis; the
networks above are proposals in which the stages are unequal. That is exactly the use for per-tower instruments.

### 2.4 Validation (task-sized)

1. `per_tower_instruments` as written, with a **control arm**: swap two towers' instruments and the readings must
   change; set both equal and they must equal the shared-instrument array to the bit. Without the control arm the test
   passes if the array code ignores the per-tower instrument and every tower happens to be the default.
2. `arms_inside_tower` for every preset (fails today on both; fix the preset or the check, say which).
3. A mixed array's tower-1 reading equals a single-tower scene's reading, bit for bit, including when tower 2 has a
   different `T` (the shots overlap in time; a shared cache keyed only on the readout time would break it: the control).

**Out of reach for this slice:** a common analysis across unequal instruments (a joint fit); that is S16.

---

## 3. Question 2 — a gradiometer on a horizontal axis

### 3.1 The phase formula, and which component it reads

For two interferometers whose beam axis is the unit vector n̂ and whose launch points differ by the vector **L**, in a
gravity-gradient tensor Γ_ij = ∂ᵢ∂ⱼ(GM/r) (the gradient of the field's acceleration):

```text
ΔΦ = δφ_lower − δφ_upper = n k_L T² · n̂·Γ·L            (quasi-static, arms separate by v_kick·t)    [D]
```

with `n k_L = n·2π/λ` (the engine's one-photon clock transition with n = 1000 kicks, so `m v_kick/ħ = n k_L`).
Derivation: the single interferometer's phase is `(m/ħ)∫[V(upper arm) − V(lower arm)]dt` (the engine's
`forward.rs:1-10`); to first order in the arm separation `s(t)n̂` that is `(m/ħ)∫ s(t) n̂·∇V dt`; differencing the two
interferometers displaced by **L** gives `(m/ħ) ∫ s(t) n̂·Γ·L dt`; and with `s = v_kick t` rising to T then falling,
`∫s dt = v_kick T²`, so the factor is `m v_kick T²/ħ = n k_L T²`.

- Vertical gradiometer: n̂ = L̂ = ẑ, so Γ_zz.
- Horizontal gradiometer with the pair along the beam: n̂ = L̂ = x̂, so Γ_xx.
- Horizontal beam with a *vertical* pair separation: n̂·Γ·L = Γ_xz L, a cross-gradient that is ≈ 0 for a spherical Earth
  (§4: −5.5 E from J2 at 45°): a useful null.

**Checked against the engine and a general-axis model.** I wrote the propagation integral for an arbitrary axis, gravity
vector and launch points ([`general_axis_lab.py`](s15-instruments-anywhere/general_axis_lab.py)) and checked its
vertical case against the *engine* with the floor switched off (the floor is a vertical-launch idea; §3.4):

| Case (1 kg point mass, AION-10, floor off) | Engine `Reader` | general-axis model | rel. diff |
|---|---|---|---|
| d = 5 m, z = 5 m | −8.062868267517×10⁻³ | −8.062868268392×10⁻³ | 1×10⁻¹⁰ |
| d = 50 m, z = 5 m | −1.272837317887×10⁻⁵ | −1.272837317890×10⁻⁵ | 2×10⁻¹² |
| d = 200 m, z = 5 m | −2.000271234258×10⁻⁷ | −2.000271234258×10⁻⁷ | < 10⁻¹² |
| d = 3 m, z = 12 m | 5.640124988244×10⁻³ | 5.640124975711×10⁻³ | 2×10⁻⁹ (quadrature order 12 vs 6) |

Then the horizontal case, same model, beam along x, atoms dropped from rest, a 1 kg mass 1 000 m away
(`results/general_axis.txt`) [M]:

| Geometry | model | `n k_L Γ_xx L T²` | ratio |
|---|---|---|---|
| mass broadside (on y), Γ_xx = −GM/r³ | −1.600751×10⁻⁹ | 1.600833×10⁻⁹ | 0.99995 |
| mass end-on (on x), Γ_xx = +2GM/r³ | 3.249235×10⁻⁹ | 3.225798×10⁻⁹ | 1.00727 |
| *vertical*, mass at d = 1000 m, Γ_zz | −1.600808×10⁻⁹ | 1.600652×10⁻⁹ | 1.00010 |

The end-on error is the finite baseline: L/D = 0.5%, error 0.73%; broadside it is second order. So the formula is
right and its **region of validity is D ≫ L and D ≫ the fall**, and the closed-form test must say so.
(An earlier line in `tensor_lab.py`'s output, "quasi-static −1.28×10⁻² vs engine −8.06×10⁻³", is *not* a check: the mass is 5 m
from a 5 m baseline. It is the quasi-static formula failing where it should fail, and is kept as the picture of why
the integral exists.)

### 3.2 How real horizontal instruments work — what I could and could not establish

[S] only, unread:

- **MIGA** interrogates three atom interferometers, aligned horizontally along a 150 m vacuum vessel, with the
  resonant field of an *optical cavity*, so the same beam addresses all of them and the cavity phase is imprinted on the matter
  waves in each pulse; it measures horizontal accelerations and strain, and a 6.4 m gradiometer was built as a
  test bench ([arXiv:2204.12137](https://arxiv.org/pdf/2204.12137), [arXiv:1703.02490](https://arxiv.org/pdf/1703.02490),
  [hal-01874243](https://hal.archives-ouvertes.fr/hal-01874243)). The atoms are Rb; its design strain sensitivity is quoted as
  2×10⁻¹³/√Hz at 2 Hz.
- **ELGAR**: underground horizontal arms, atom gradiometers; single-gradiometer shot-noise strain 4.1×10⁻²¹/√Hz at 1.7 Hz;
  the technology companion is [arXiv:2007.04014](https://arxiv.org/pdf/2007.04014).
- **ZAIGA**: a horizontal tunnel in stage one ([arXiv:1903.09288](https://arxiv.org/abs/1903.09288)).

**What I could not establish, and the note does not guess:** where each instrument launches its atoms, how the atoms
cross the beam, what pulse sequence and interrogation time they run, and how they deal with the fall. The sources
that answer it are MIGA's and ELGAR's technology papers above, and Hogan–Johnson–Kasevich's review
([arXiv:0806.3261](https://arxiv.org/pdf/0806.3261)) for the formalism. What follows is therefore the *constraint*, derived, and not a description of what they do.

**The constraint [D, numbers M].** A horizontal beam is at one height; gravity is perpendicular to it; atoms in free
fall drop ½gt². Over one 2T = 1.46 s shot that is **10.5 m**, over AICE's 2T = 5 s it is **122.6 m**
(`results/tensors_and_phases.txt`). Between the pulses of one interferometer (spacing T) it is ½gT² = 2.6 m at T = 0.73 s even if
the shot is centred on the apex. A beam a few centimetres across (a cavity mode's waist; [R]) cannot do that. So
either the instrument uses **short T** (beam radius w ≥ ½gT² gives T ≲ 45 ms for w = 1 cm), or the beam geometry
follows the atoms (a wide beam, a tilted beam, or gravity compensated by an optical lattice), or the atoms are
supported rather than dropped. A forward model should therefore **report the beam clearance** — the largest distance
any arm gets from the beam axis during the shot — and let the owner decide whether a long-T horizontal instrument is
sensible, not assume it is.

### 3.3 What differs from the vertical case: rotation, pointing, sag [D, sizes M]

*Rotation and Coriolis.* A rotating frame adds a phase `2 n k_L (Ω × v)·n̂ T²` for an atom of velocity **v** [S: this is
the standard form in the review above and in the space-gradiometer concept
[arXiv:1406.0765](https://arxiv.org/pdf/1406.0765)]. For a **vertical** instrument v ∥ n̂ so the term vanishes to first order
(it survives through tilt and horizontal velocity). For a **horizontal** beam the fountain velocity (≈ gT = 7.2 m/s) is
perpendicular to n̂ and the term is as large as it gets. In local east–north–up Ω = Ω(0, cos λ, sin λ), so
Ω × ẑ = (Ω cos λ, 0, 0): **the Coriolis push on a vertically moving atom is east–west**. An east–west beam sees
`n k_L 2Ω cos λ v T²` = 5.0×10⁶ rad at the equator (common to both interferometers, so it cancels in ΔΦ); a
**north–south beam sees none** at first order. What does *not* cancel is a velocity mismatch δv between the
interferometers' atom clouds: **7.0×10⁵ rad per m/s**, so 1 µrad needs δv < 1.4×10⁻¹² m/s [M]. Launch jitter and temperature
set this; it is the most demanding item and it sets the orientation: **choose north–south for a horizontal instrument**.
(The sensitivity is cos λ times that, so latitude helps only at the poles.)

*Beam pointing.* A tilt θ of the beam from horizontal makes a common-mode phase `n k_L g sin θ T²` = 4.7×10⁴ rad for 1 µrad
[M]; it cancels in the difference if both interferometers see the same beam. The difference term is
`n̂·Γ·L = L(Γ_xx + (θ+φ)Γ_xz + θφ Γ_zz)` for beam tilt θ and pair tilt φ: with Γ_zz = 3×10⁻⁶ s⁻² and θφ = 10⁻¹²
that is 3×10⁻⁹ E = 7×10⁻⁸ rad on AION-10 [M-derived arithmetic], negligible; the cross term uses Γ_xz (≈ 5 E, §4) and is smaller still.

*Gravity sag.* In the model the sag does not move the arms' *separation* (both arms fall alike) so `b − a = s n̂` stays
exact and the engine's cancellation-free form generalises (§3.5); the sag enters only through where the pair is, relative to the
source. For a 1 kg mass 5 m beside the beam centre the phase is −4.121×10⁻³ rad with the fall and −4.579×10⁻³ rad without,
a **10% effect** [M], so a horizontal tower modelled "as if the atoms stayed put" is wrong at the first digit near sources.

*The Earth's own gradient* contributes `n k_L Γ_xx L T²` = 3.7×10⁴ rad on a 5 m horizontal baseline, 7.4×10⁴ rad on
the vertical one [M]. The engine reads the *source's* potential only and not Earth's static field, so neither appears. Earth's
gradient also bends the arms: a relative change of ΓT² ≈ 1.6×10⁻⁶ in their separation, so a fractional 10⁻⁶ in the source phase
(10⁻⁸ rad on 8 mrad) [D]. That is below every tolerance in use today, and above the 10⁻¹² of the bit tie; record it as an approximation.

### 3.4 Generalising the forward model so the vertical case is unchanged

The plan's "unchanged to the bit" is achievable only by **dispatch**, and it is a test of the dispatch, not of the physics.

1. Keep `Instrument`, `Arm::rise`, `Flight`, `Source::difference` as they are for the vertical axis: same code, same bits.
2. Add `axis: Axis` (a unit vector in the local frame; the default `Axis::Up`), the gravity *vector* (default `(0, 0, −g)`)
   and the **launch plane**. The "floor" (an arm stops at its launch height) is meaningful only for a vertical launch from
   a surface; for any other axis it must be off, or defined as a plane the arms cannot cross.
3. For `axis != Up` call a new path: arms as 3-D positions `p(t) = p₀ + v t + ½ g t²` (+ recoil along n̂), and
   a source method taking 3-D positions.
4. **The cancellation-free form survives**: the engine writes `1/a − 1/b = (b² − a²)/(ab(a+b))` and gets `b² − a²` from the
   arms' rise. For vectors, `|b|² − |a|² = (b − a)·(b + a)`. The two arms differ only by the recoil, so `b − a = s(t) n̂` exactly,
   with no gravity in it, and `b + a` is twice the midpoint. The vertical code is the 1-D case of this identity.

**The bit test and a physics test, together.** (a) `vertical_case_unchanged`: the *default* path bit-equal to the S1 model.
(b) `generic_path_on_vertical`: force the generic path on a vertical baseline and require ≤ 10⁻¹² relative against the
vertical one, with floor off (the measured agreement of my own independent implementation is 10⁻¹⁰ to 2×10⁻⁹ because it
uses a different quadrature; the generic path in Rust shares the rule, so it should do far better). Control arm: tilt
the axis by 10⁻⁶ rad and the two must differ. Without (b) the bit test says nothing about whether the generalisation is right.

### 3.5 Validation cases (with references) and what is out of reach

| Test | Pass criterion (to be written before measuring) | Reference |
|---|---|---|
| `horizontal_baseline_tensor` far mass | relative error ≤ 3 L/D + 4 (fall/D), run at 3 values of D, error falls with D; control: the *vertical* component fails by a factor 2 and the sign | closed-form Γ_xx, §3.1, measured ratios above |
| `rotation_covariance`, g off | ΔΦ(n̂, source) = ΔΦ(Rn̂, R·source) to 10⁻¹² for random rotations R; control: rotate the scene and not the axis | exact symmetry |
| `laplace_three_axes` | Γ_xx + Γ_yy + Γ_zz = 0 outside the mass: three gradiometers at one site sum to ≤ 10⁻⁶ of the largest | Laplace's equation |
| `sag_matters` | near mass, g on vs off differs by the measured 10% | §3.3 |
| `beam_clearance` | reported value equals ½g t² at the arms' extreme | closed form |

**Out of reach:** MIGA's cavity-mode wavefront physics, the beam's finite size, how a real horizontal instrument keeps its atoms in the
beam, magnetic gradients along a long tube.

### 3.6 Recommended task

*TASK-S15-02, horizontal axis:* `Axis` and gravity vector in the spec; the generic path of §3.4; tests above; the clearance report.
One builder session if the signal is read for a point mass and a slab only (the voxel path is S5's).

---

## 4. Question 3 — different gravity environments

### 4.1 Normal gravity and its gradient

[R, reproduced by [`tensor_lab.py`](s15-instruments-anywhere/tensor_lab.py)] The WGS84 normal gravity on the ellipsoid
is Somigliana's closed form,

```text
γ(φ) = γₑ (1 + k sin²φ) / √(1 − e² sin²φ),   γₑ = 9.7803253359 m/s²,  k = 0.00193185265241,  e² = 0.00669437999014
```

and with height h above the ellipsoid, `γ(φ, h) ≈ γ(φ)[1 − (2/a)(1 + f + m − 2f sin²φ)h + 3h²/a²]`
(a = 6 378 137 m, f = 1/298.257223563, m = 0.00344978650684). The sources are the NGA/NIMA WGS84 definition (TR8350.2) and
Moritz's *Geodetic Reference System 1980*; I did not open either, so the six constants and two formulas are [R]. The numbers
this gives [M]:

| Site | g (m/s²) | change with +100 m |
|---|---|---|
| Equator | 9.780325 | −3.1×10⁻⁴ |
| Fermilab, 41.83° | 9.803335 | −3.1×10⁻⁴ |
| Oxford, 51.76° | 9.812263 | −3.1×10⁻⁴ |
| Pole | 9.832185 | −3.1×10⁻⁴ |

The equator and pole values match the WGS84 defining numbers as I remember them, which shows only that my recall of the constants
is self-consistent. **The engine's 9.81 is within 3×10⁻³ of every inhabited latitude**, but the spread between the equator and the pole
(0.052 m/s²) matters to the phase (§4.3).

The gradient. A point-mass Earth gives Γ_zz = 2g/R = 3 080 E (R = 6 371 km), Γ_xx = Γ_yy = −g/R = −1 540 E. Adding J2
(the potential `GM/r [1 − J₂(a/r)²(3z²/r² − 1)/2]`, J₂ = 1.08262668×10⁻³ [R]) and differentiating it numerically in an
east–north–up frame gives [M, `results/tensors_and_phases.txt`]:

| Latitude | Γ_EE | Γ_NN | Γ_UU | Γ_NU |
|---|---|---|---|---|
| 0° | −1 538.7 | −1 543.7 | +3 082.4 | 0 |
| 45° | −1 540.2 | −1 542.8 | +3 083.0 | −5.47 |
| 51.76° | −1 540.6 | −1 542.5 | +3 083.1 | −5.3 |
| 90° | −1 541.6 | −1 541.6 | +3 083.3 | 0 |

(E = 10⁻⁹ s⁻²; the trace is 0 to the printed digits. These are gravitation, not including the centrifugal term:
**the rotating Earth adds ω_E² = 5.3 E to the two horizontal-plane components**, the same size as the off-diagonal term
above, so a model that omits it is wrong by about 5 E in Γ_EE and Γ_NN.) The standard "free-air gradient" of
−0.3086 mGal/m is Γ_UU = 3 086 E: compare 3 083 E from J2 and 3 080 E from the point mass.

**A global field model** adds what the closed forms cannot: [S] EGM2008 is distributed through the
[ICGEM service](https://icgem.gfz-potsdam.de/) and compared with GOCE-based models in the geodesy literature
([GOCE Level 2 handbook](https://earth.esa.int/eogateway/documents/d/earth-online/goce-level-2-product-data-handbook) is the ESA
page I found). I could not open ICGEM or confirm the model's resolution or any test values; my recollection is that
EGM2008 goes to degree 2159 (about 9 km half-wavelength), **which cannot supply the gradient at the scale of a
10 m tower or a building**. Anomalies beyond the ellipsoid are tens to hundreds of mGal in g and tens to hundreds of E in Γ
near topography [R]; a site on a mountain or in a quarry needs the local mass model (S14's terrain, or the voxel scene itself) and
not the global model. A recommendation that does not depend on any of it: **the instrument takes `g` and `Γ` as inputs**,
with a "site" helper filling them from the normal-gravity and J2 closed forms now, and a field-model reader later.

### 4.2 What local g changes in the instrument

1. **The launch rule.** `u₀ = gT − v_kick/2` closes both arms at the launch height at 2T (`Instrument::symmetric_launch`).
   With the engine's constants v_kick = 6.578 m/s, so on the Moon (g = 1.62) it gives **u₀ = −2.11 m/s** with AION-10's
   T = 0.73 s (negative: no fountain), and on Mars (g = 3.71) **−0.58 m/s** [M]. A closed launch needs
   `T > v_kick/(2g)`: **2.03 s on the Moon, 0.887 s on Mars** [M]. At T = 1.2× that, the Moon's u₀ is 0.658 m/s and
   the signal of a 1 kg mass at (5, 0, 5) is −21.1 mrad against −8.06 mrad on Earth; on Mars at T = 1.064 s, −7.85 mrad [M].
   The presets must therefore **choose T**, not copy AION-10's.
2. **The flight.** Arm heights scale as g: the Moon's flight reaches higher at the same u₀ and a given tower holds a
   longer T. The check of §2.2 applies.
3. **The fringe position.** A common-mode g error shifts each interferometer's phase by `n k_L δg T²` =
   4.8×10⁴ rad per mGal (10⁻⁵ m/s²) for AION-10 [M-derived]; the laser chirp is set to cancel it, and only the *difference* between
   the two interferometers matters, which is the gradient: `Γ_zz Δr` = 15 µm/s² = 1.5 mGal across 5 m. So the instrument
   needs the local `g` to set the chirp and does not need it to be exact for the difference; it needs it at the 10⁻⁷ level for the mid-fringe position.
4. **The signal.** Holding u₀ = 3.86 m/s and sweeping g for a 1 kg mass at (5, 0, 5) [M, `results/engine_gravity_sweep.txt`]:

| g (m/s²) | phase (rad) | relative to g = 9.81 |
|---|---|---|
| 9.7803 (equator) | −8.0688×10⁻³ | +7.4×10⁻⁴ |
| 9.8066 (standard) | −8.0635×10⁻³ | +8.6×10⁻⁵ |
| 9.8100 | −8.0628×10⁻³ | 0 |
| 9.8322 (pole) | −8.0581×10⁻³ | −5.8×10⁻⁴ |
| 9.70 | −8.0840×10⁻³ | +2.6×10⁻³ |
| 9.90 | −8.0424×10⁻³ | −2.5×10⁻³ |

So over the Earth's range of g the signal moves by ±7×10⁻⁴, **≤ 6 µrad on an 8 mrad signal**; with the launch closed (u₀ = gT − v_kick/2) it moves by
−3.1×10⁻⁴ (equator) to +7.8×10⁻⁴ (pole) [M]. It is small but **above the µrad budget**, so `g` must be a site input and not a constant.

### 4.3 Moon and Mars

| | g (m/s²) | R | 2g/R point-mass Γ | AION-10-like phase for it |
|---|---|---|---|---|
| Earth | 9.81 | 6 371 km | 3.080×10⁻⁶ s⁻² | 7.4×10⁴ rad |
| Moon | 1.62 | 1 737.4 km | 1.865×10⁻⁶ | 4.5×10⁴ |
| Mars | 3.71 | 3 389.5 km | 2.189×10⁻⁶ | 5.3×10⁴ |

[R] for g and R (GM_Moon = 4.9028×10¹² m³/s², GM_Mars = 4.2828×10¹³ [R]); [D] for the rest. The Moon's rotation rate, 2.7×10⁻⁶ rad/s, is 27 times
slower than the Earth's, so Coriolis and Sagnac terms shrink by that factor there; Mars rotates as fast as the Earth. The local
field models I would use (GRAIL for the Moon, MRO120D for Mars) are from memory and unread. A first Moon or Mars mode needs only g, GM, R, Ω and J₂ as
site inputs: all of the above follows from the same closed forms.

### 4.4 Test values and validation

| Test | Reference |
|---|---|
| `normal_gravity` at the equator, the pole and 45° | γₑ = 9.7803253359, γₚ = 9.8321849378 m/s² (WGS84) [R]: the test fails on a wrong constant, with the control "use 9.81 everywhere" |
| `j2_tensor` against a numerical differentiation of the potential, trace zero, symmetric, Γ_UU(0°) = 3 082.4 E [M] | closed form; the check itself |
| `point_mass_limit`: set J₂ = 0 and get 2g/R, −g/R | closed form |
| `launch_rule_has_no_solution`: the Moon at T = 0.73 s returns an error, not a negative speed | [M] |

Field-model test values: I could not confirm that EGM2008 ships any, nor which; a task cannot rely on them.
Out of reach: a real site's anomalies without the local mass model; tide-system conventions (tide-free, zero-tide) which change g at the 10⁻⁷ level and must be stated when a field model is used.

---

## 5. Question 4 — a gradiometer in orbit

### 5.1 The research and the proposals

All [S] unless stated; **none of these papers was opened**, so every "value" below is a pointer, and the empty cells are things the search did not show.

| Concept | What it is | Geometry, T, sensitivity | Source |
|---|---|---|---|
| **AEDGE** | Space atom interferometer for ultralight dark matter and gravitational waves between LISA and ground detectors; **two satellites on one line of sight**, a long baseline, cold atoms on each | baseline, T, strain curve: *not seen* | [arXiv:1908.00802](https://www.arxiv.org/pdf/1908.00802), [Exp. Astron. 2021](https://link.springer.com/article/10.1007/s10686-021-09701-3) |
| **STE-QUEST** | Test of the equivalence principle, ⁸⁵Rb/⁸⁷Rb dual species (a later version ⁴¹K/⁸⁷Rb) plus clocks; **not selected** in 2014 (budget, technology), resubmitted for M4 in 2015 | not seen | [ESA/SRE(2013)6](https://sci.esa.int/documents/33940/36014/1567259888264-ESA_SRE_2013_6_STE-QUEST.pdf), [arXiv:1312.5980](https://www.arxiv.org/pdf/1312.5980), [arXiv:2211.15412](https://arxiv.org/pdf/2211.15412v1) |
| **SAGE** | Space atomic gravity explorer: Sr optical clocks and Sr atom interferometers, multi-satellite, proposed to ESA in 2016 | not seen | [arXiv:1907.03867](https://ar5iv.labs.arxiv.org/html/1907.03867) |
| **MAGIS-space** | The space version of MAGIS-100's concept | I did not find it; my recollection that it is in Graham et al., "Mid-band gravitational wave detection with precision atomic sensors" (arXiv:1711.02225) is [R] | — |
| **CARIOQA** | Pathfinder for a quantum space accelerometer; **single axis, along the velocity**, ⁸⁷Rb, target 1×10⁻¹⁰ m s⁻² Hz⁻¹ᐟ² (quantum-projection floor), Phase A from Jan 2024 for 18 months | one-axis accelerometer, not a gradiometer | [arXiv:2211.01215](https://arxiv.org/pdf/2211.01215), [DLR](https://www.dlr.de/en/si/research-transfer/projects/carioqa), [SPIE](https://journals.spiedigitallibrary.org/conference-proceedings-of-spie/12777/127773L/CARIOQA-definition-of-a-Quantum-Pathfinder-Mission/10.1117/12.2690536.full) |
| **Rotation in CARIOQA** | Systematics of the active compensation of the rotation of a *nadir-pointing* satellite on a ⁸⁷Rb interferometer | — | [arXiv:2212.06686](https://arxiv.org/pdf/2212.06686) |
| **Cold-atom gradiometer for geodesy** | A spaceborne gradiometer from cold-atom interferometers measuring the diagonal tensor elements, to beat GOCE over more of the spectrum | the paper's noise curves: not seen | [Carraz et al., arXiv:1406.0765](https://arxiv.org/pdf/1406.0765) |
| | Two satellites at 373 km, each with a cold-atom accelerometer, laser-ranged | — | [arXiv:2011.03382](https://arxiv.org/pdf/2011.03382) |
| | Closed-loop simulations: a cold-atom gradiometer, especially in quasi-inertial mode, outperforms GOCE *if* strict technical requirements are met | — | [DLR, Adv. Space Res. 2024](https://elib.dlr.de/210767/2/1-s2.0-S0273117724012031-main.pdf) |
| | Atom *clocks* instead: a static field two to five times better than GOCE above degree and order 50 | — | [Müller & Wu 2020, summary](https://e-docs.geo-leo.de/entities/publication/3d155a15-5751-4a1b-accd-933b26a3b328) |
| **Cold Atom Laboratory (ISS)** | ⁸⁷Rb Bose–Einstein condensates in an Earth-orbiting lab (Aveline et al., *Nature*, June 2020); interferometry results: *not seen* | — | I found no page for the Aveline paper itself (the search returned the MAIUS paper's, [Nature](https://www.nature.com/articles/s41586-018-0605-1)) |
| **MAIUS-1 rocket** | ⁸⁷Rb BEC on an atom chip; 243 km apogee, about six minutes of microgravity, 110 experiments including interferometry, a BEC flux 64% higher than on the ground | six minutes: so T ≲ a few s | [Becker et al. 2018, arXiv:1806.06679](https://arxiv.org/pdf/1806.06679) |

What the survey shows and does not: **nothing flown is a gradiometer**. The flown results are BEC production and short
interferometry in microgravity (MAIUS, CAL). The gradiometer cases are proposals and simulations, which is why §5.5's validation can
be against closed forms and published *model* sensitivity curves, and not against measured atom-gradiometer data in orbit.

### 5.2 The phase model for a free-falling instrument [D]

Take the satellite's frame, origin at its centre of mass (free-falling, with the satellite's non-gravitational acceleration **a**_ng
acting on the *satellite* but not on the freely released atoms), rotating with angular velocity **Ω**(t) relative to inertial space. An atom at
ρ relative to the origin obeys

```text
ρ̈ = Γ_g ρ  −  2 Ω × ρ̇  −  Ω × (Ω × ρ)  −  Ω̇ × ρ  −  a_ng          (to first order in ρ)
   = M ρ − 2 Ω × ρ̇ − Ω̇ × ρ − a_ng,         M = Γ_g + Ω² I − Ω Ωᵀ       (using −Ω×(Ω×ρ) = Ω²ρ − (Ω·ρ)Ω)
```

so the gradient the atoms respond to is **M, not Γ_g**. For a circular orbit of angular rate ω with the satellite's x along
the track, y along the orbit normal, z radial (a nadir-pointing attitude: Ω = ω ŷ):

```text
Γ_g(point mass) = ω² diag(−1, −1, +2)      M = Γ_g + ω²(I − ŷŷᵀ) = ω² diag(0, −1, +3)     (ω² = GM/r³)
```

The point-mass part I checked numerically at the 255 km orbit: Γ_g = diag(−1 365.8, −1 365.8, +2 731.6) E, matching
ω² diag(−1, −1, 2) with ω = 1.1687×10⁻³ rad/s (period 5 376 s) [M, `results/orbit.txt`]. The trace of M is 2ω², the centrifugal part,
the same reduction the GOCE processing applies [S: the GOCE products are corrected]; I did not read it.

The phase of the pair, with arms separating by `s(t)` along the sensing axis ê and the two atom clouds a distance L apart:

```text
ΔΦ = (m/ħ) L ∫₀^2T s(t) ê·M(t)·ê dt  +  Coriolis + inertial terms        quasi-static:  n k_eff L T² M_êê
```

with **no launch**: the clouds are released (cooled, or a BEC), the arm separation is the recoil `s = v_kick t`, and nothing
falls, so the engine's `Arm` (launch, fall, floor) becomes `s(t)` only, and T may be long: T = 5 to 10 s is claimed for space designs
[S, not read]. For Rb at 780 nm with a two-photon k_eff = 1.61×10⁷ m⁻¹ (n = 1, one pair): `k_eff L T² Γ` is 88 rad for L = 0.5 m,
T = 2 s at 255 km, and **550 rad at T = 5 s**; **1 mE/√Hz = 10⁻¹² s⁻² of gradient noise is 2.0×10⁻⁴ rad/√Hz** at T = 5 s; and **one Eötvös
of anomaly is 0.20 rad** [M, `results/tensors_and_phases.txt`, `orbit.txt`]. The signal of geodesy interest is the *anomalous* part, a few E.

**The terms that decide whether it works:**

| Term | Size | Note |
|---|---|---|
| Coriolis `2 k_eff (Ω × δv)·ê T²` | **0.94 rad per µm/s** of velocity mismatch at ω = 1.17×10⁻³, T = 5 s [M] | for 1 µrad, δv < 1 pm/s: the orbital rotation has to be compensated (a rotating mirror, or an inertial attitude). The CARIOQA rotation paper above is exactly this problem for a nadir satellite [S] |
| Rotation of the mirror during the shot | ωT = 5.8 mrad at T = 5 s [M] | the beam pointing moves; it couples to cloud position and velocity |
| Non-gravitational acceleration (drag, radiation pressure) | `k_eff a T²` = 400 rad per AI for a = 10⁻⁶ m/s² at T = 5 s [M-derived] | common to both interferometers on one satellite, so it cancels in ΔΦ except through the scale-factor mismatch and the platform's own gradient; a drag-free or accelerometer-aided platform shrinks it. Typical a at 255 km is ~10⁻⁶ m/s² [R: GOCE needed drag-free control against it] |
| Euler term `Ω̇ × ρ` | zero for a constant rate; attitude jitter | |
| J2 swing in M_zz over one orbit | **−16.1 to +8.2 E**, i.e. **4.9 rad** at L = 0.5 m, T = 5 s [M] | the periodic signal that a first orbital mode should reproduce |
| |Γ_xz| from J2 | up to 8.1 E [M] | the cross-term |
| Satellite self-gravity | a 100 kg lump 1 m away: `GM/r³` = 6.7×10⁻⁹ s⁻² = 6 670 E | **larger than every geodesy signal**: calibrate and model the satellite |

### 5.3 What a first orbital mode in Cavendish should compute

*TASK-S15-04, orbit, the smallest useful version:*

1. **A site in space**: a circular orbit (altitude, inclination), propagated analytically with the J2 secular rates of the node and perigee
   (`dΩ/dt` and the perigee drift are closed forms [R]; one test: a sun-synchronous orbit's node drifts 360° a year at about 96.7° and 255 km, which my script
   assumed as the inclination only to choose a representative orbit). Time = the shot's start.
2. **An attitude**: nadir-pointing (Ω = ω ŷ) or inertial (Ω = 0), a parameter.
3. **The tensor**: Γ_g(r) from a point mass plus J2 (the closed form, differentiated analytically: I differentiated it numerically,
   which is the test), rotated into the satellite frame, plus the rotation term of §5.2 → M(t).
4. **The read**: `ΔΦ = n k_eff L ∫ s(t) M_êê(t₀ + t) dt`, which is the engine's propagation integral with the arm's *rise* replaced by the
   separation `s(t)` and the potential replaced by the gradient field. At ωT ≈ 6×10⁻³ it differs from the quasi-static `k_eff L T² M` by (ωT)² = 4×10⁻⁵ of itself [D];
   the integral costs almost nothing and removes the approximation.
5. **Out of the model, with a stated bound**: drag and radiation pressure (common mode; set to zero and bounded by the scale-factor mismatch), the Coriolis
   term (reported as a budget line `2 k_eff Ω δv T²` and not simulated), self-gravity, attitude jitter.

### 5.4 Validation, with references

| Test | Reference | Criterion (written first) |
|---|---|---|
| `orbit_point_mass_tensor` | closed form: Γ_g = ω² diag(−1, −1, 2); M = ω² diag(0, −1, 3) | ≤ 10⁻¹² relative (a differentiation-free formula), control: use the radius of the Earth's surface |
| `orbit_j2_tensor` | numerical differentiation of the J2 potential (what I did: the J2 − point-mass tensor spans −16.1 to +8.2 E in zz at 255 km), **plus Laplace's equation** (trace 0: 0.05 E in my differentiation, from the step, so the analytic form must give 10⁻¹² of Γ_zz) | exact |
| `orbit_phase_vs_quasi_static` | `k_eff L T² M` | agreement to (ωT)² |
| `orbit_gradient_audit_goce` | GOCE gradients (below) | residual statistic stated before looking; an *audit*, not a unit test |
| `orbit_sensitivity_curve` | the proposal's own noise curve | the engine's shot noise converted by `k_eff L T²` lands on the published curve at the published parameters |

**GOCE data.** [S] The Level 1b and Level 2 products are freely available from ESA's EO Gateway (an EO sign-in account is needed to download);
`EGG_NOM_2` holds the calibrated gradients in the gradiometer frame, the diagonal components being the most accurate;
`EGG_TRF_2` is the local north-oriented version, which **mixes the measurements with model information**, so it
is not an independent check
([GOCE Level 2](https://earth.esa.int/eogateway/catalog/goce-level-2), [the product handbook](https://earth.esa.int/eogateway/documents/d/earth-online/goce-level-2-product-data-handbook),
[the gradiometer page](https://dragon3.esa.int/eogateway/instruments/egg)). For the audit use `EGG_NOM_2`, the precise orbit and attitude
products, and a degree-≥ 200 gravity model, remove the rotation terms exactly as §5.2, and compare per component. I could not check the products' licence terms,
their exact format or GOCE's published noise (my recollection of 10–20 mE/√Hz is [R]). Per `REBUILD.md` §9.3 the data is cached outside the repo and its licence is recorded.

### 5.5 What is out of reach

Gravity-field recovery from the data (that is a geodesy pipeline, not an instrument model); drag-free control and orbit determination; two-satellite baselines
(AEDGE's, SAGE's) as a *gravity-wave* instrument, which needs the laser link and clock noise; the atom cloud's physics at T = 10 s (expansion, wavefront aberration);
the 1 pm/s velocity control.

---

## 6. Audit of `VALIDATION-AND-RESEARCH.md` §2.6 (the instrument rows, `per_tower_instruments` onwards)

Each row is checked against what the measurements here support; I could not check their cited sources, which the §2.6
preamble already says were "named from memory".

| Row | Verdict | Why, and what to change |
|---|---|---|
| `per_tower_instruments` | **Weak**: passes for the wrong reason | "an array of unequal towers equals each tower alone, exact" is satisfied by code that ignores the per-tower instrument if the towers are alike, and by a code path in which towers do not interact at all. Add the control arm of §2.4 (swap instruments: readings must change) and a test with overlapping shots of different length. |
| `vertical_case_unchanged` | **Right for dispatch, silent on physics** | "to the bit" can hold only if the vertical axis keeps the old code, in which case the test proves the `if` works. Add `generic_path_on_vertical` (≤ 10⁻¹² against the vertical one, floor off, §3.4); I measured an independent implementation agreeing to 10⁻¹⁰–2×10⁻⁹ with a different quadrature. |
| `horizontal_baseline_tensor` | **Under-specified, would hide a bug** | "reads the horizontal gradient component; closed form" omits the regime. The tensor formula is accurate to 5×10⁻⁵ broadside and 0.7% end-on at D = 1 000 m, L = 5 m [M], and **fails by 10% for a source at 5 m** because of the sag [M]. State D ≫ L and D ≫ fall, the error law, and that the near-source test is against the *propagation integral*, not the tensor. |
| `local_gravity_from_field_model` | **Weak and partly wrong** | (a) Somigliana gives **normal** gravity on the ellipsoid, not "g at a site"; the site's value needs the anomaly, and the vertical *gradient* needs the J2 tensor or the field model, not Somigliana. (b) "the field model's own test values": I could not confirm that EGM2008 provides any. (c) EGM2008's resolution (my recollection: degree 2159) is far coarser than a tower. (d) tide system, gravity vs gravitation (centrifugal included or not) and ellipsoidal vs orthometric height are unstated: 5 E and 3×10⁻⁷ g respectively. Split into `normal_gravity`, `j2_tensor` and (later) `field_model_point`. |
| `orbit_gradient_tensor` | **Weak as a published-data test** | "published GOCE gradients" are in the gradiometer frame with the rotation terms in them and the `EGG_TRF_2` gradients mix model information [S]; a match is not a pure test of the forward model, and the engine cannot match the data better than the gravity model plus the noise allow. Keep the closed forms (point mass, J2, Laplace) as the tests, make GOCE an *audit* with a pre-stated statistic. The row also lacks the centrifugal reduction `M = Γ + ω²(I − ŷŷᵀ)`. |
| `injection_recovered` | **Weak, and out of S15's scope** | "recovered at the published sensitivity" with "the proposals' sensitivity curves": the proposals' curves (AION-km, MAGIS-km, AEDGE, ELGAR) assume km-scale baselines and noise models the engine does not have, so a 10 m instrument cannot reach them. The testable statement is self-consistency: the injected amplitude is recovered with the error the engine's own noise gives (a Fisher/Cramér–Rao bound) and the instrument's transfer function has the published shape. The GW response formula needs reading from the AION or MAGIS papers: I did not. |
| `two_sites_reject_local_noise` | **Vague** | "the predicted 1/√N and common-mode factors": the cross-spectrum of two independent noises has relative scatter 1/√M for M averaged segments; "common-mode factor" is undefined. Say what is averaged and what the common signal is. Belongs to S16. |

Two corrections to the plan's numbers that this note found: **the AICE preset (`Δr = 70 m`, 100 m tower, `u₀ = 21.24 m/s`) sends the upper arm
to 108.9 m** (§2.2), and `VALIDATION-AND-RESEARCH.md` §1 gives the AICE launch as 21.27 m/s where the engine's constants give 21.236 m/s
(`gT − v_kick/2` with ħ, m and k of `constants.rs`; the 0.17% difference is the scripts' rounded constants).

---

## 7. What I could not verify

1. **Every [S] and [R] item above.** No paper was opened. The reading list, in order of what the builder needs first:
   - Hogan, Johnson & Kasevich, *Light-pulse atom interferometry* [arXiv:0806.3261](https://arxiv.org/pdf/0806.3261): the phase-shift formulae, the gradiometer section, the rotation and gradient terms (§3.1, §3.3, §5.2).
   - MIGA: [arXiv:2204.12137](https://arxiv.org/pdf/2204.12137), [arXiv:1703.02490](https://arxiv.org/pdf/1703.02490); ELGAR: [arXiv:1911.03701](https://arxiv.org/pdf/1911.03701), [arXiv:2007.04014](https://arxiv.org/pdf/2007.04014); ZAIGA: [arXiv:1903.09288](https://arxiv.org/abs/1903.09288): launch, beam crossing, pulse sequence, T, and the Coriolis treatment of the horizontal case (§3.2).
   - Carraz et al. [arXiv:1406.0765](https://arxiv.org/pdf/1406.0765): a spaceborne gradiometer's geometry, T and sensitivity, the non-gravitational terms with their bounds (§5.1, §5.2).
   - CARIOQA [arXiv:2211.01215](https://arxiv.org/pdf/2211.01215) and the rotation paper [arXiv:2212.06686](https://arxiv.org/pdf/2212.06686).
   - AEDGE [arXiv:1908.00802](https://www.arxiv.org/pdf/1908.00802), SAGE [arXiv:1907.03867](https://ar5iv.labs.arxiv.org/html/1907.03867), STE-QUEST [ESA/SRE(2013)6](https://sci.esa.int/documents/33940/36014/1567259888264-ESA_SRE_2013_6_STE-QUEST.pdf): baselines, T, sensitivities, which the survey table left blank.
   - The Cold Atom Laboratory paper (Aveline et al., *Nature* 2020): I did not find its address; the interferometry results.
   - The GOCE Level 2 handbook and the licence of the products; the WGS84 definition (NGA TR8350.2), EGM2008's documentation and any test points it ships; the Moon's and Mars's constants (GM, R, J₂) and models.
2. **The WGS84 constants and the J₂, GM, R values** of §4 are recalled; the scripts reproduce their internal consistency (equator and pole values), not their truth.
3. **How a real horizontal instrument deals with the fall** (§3.2): only the constraint is derived.
4. **The Coriolis formula's sign and the `M` formula's sign conventions** [D]: derived and checked for the point-mass tensor and the trace, but the *phase* sign of the Coriolis term was not simulated; the engine has no rotating frame. A rotation test would add one to the general-axis model (Coriolis acceleration on the arms) and check the 7.0×10⁵ rad/(m/s) figure.
5. **The phase of an orbiting gradiometer against a simulation**: §5.2 is a derivation with a numerical tensor check; no propagation integral with a time-varying M was run.
6. **The floor and the horizontal case**: the engine's floor is part of the S1 default; I tested only with it off (it is a vertical-launch device) and did not decide what the horizontal analogue should be.
7. **Reproducibility.** The scripts run with the standard library and numpy; the Rust lab needs `cargo` and the repository (`cd docs/research/s15-instruments-anywhere/engine_lab && cargo run --release --bin floor_off`). Nothing in `crates/` was changed.

### Questions for George and the owner

- The AICE preset: is `Δr = 70 m` in a 100 m shaft right, given that the arms then reach 108.9 m? (And AION-10's 10.006 m, already asked.)
- Which horizontal instrument is meant (MIGA-like cavity, ELGAR-like long arm, a table-top)? The answer fixes whether a horizontal AION-like tower with T = 0.73 s is wanted at all, given a 10 m fall.
- Orbit: gradiometer or accelerometer pair, which atom, T, and which satellite attitude.

## 8. How the numbers were made

| Script | What it does | Output |
|---|---|---|
| [`engine_lab/src/bin/floor_off.rs`](s15-instruments-anywhere/engine_lab/src/bin/floor_off.rs) | the engine's `Reader` on point masses with the floor off | [`results/engine_floor_off.txt`](s15-instruments-anywhere/results/engine_floor_off.txt) |
| [`engine_lab/src/bin/gravity_sweep.rs`](s15-instruments-anywhere/engine_lab/src/bin/gravity_sweep.rs) | signal sizes, the sweep over local g, Moon and Mars | [`results/engine_gravity_sweep.txt`](s15-instruments-anywhere/results/engine_gravity_sweep.txt) |
| [`general_axis_lab.py`](s15-instruments-anywhere/general_axis_lab.py) | the propagation integral on any axis; reproduces the engine; the horizontal checks | [`results/general_axis.txt`](s15-instruments-anywhere/results/general_axis.txt) |
| [`tensor_lab.py`](s15-instruments-anywhere/tensor_lab.py) | normal gravity, the J2 tensor, phase scales, Coriolis, Moon/Mars | [`results/tensors_and_phases.txt`](s15-instruments-anywhere/results/tensors_and_phases.txt) |
| [`orbit_lab.py`](s15-instruments-anywhere/orbit_lab.py) | the tensor along a 255 km, 96.7° orbit, J2 swing | [`results/orbit.txt`](s15-instruments-anywhere/results/orbit.txt) |

[`arm_tops.py`](s15-instruments-anywhere/arm_tops.py) gives the arm tops of §2.2; the builder should add the check as a test in the engine.

## 9. Further questions (not chased)

- The Lense–Thirring and Sagnac terms for an orbiting instrument, and the relativistic corrections at T = 10 s.
- Blackbody shift on a warm tower (S13's temperature field feeding S15's systematics budget).
- A joint analysis of unequal instruments (S16).
- Gravity-gradient noise from the atmosphere at a mountain site (S14).
