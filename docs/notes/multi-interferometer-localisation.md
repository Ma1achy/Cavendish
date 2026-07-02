# Cavendish — multi-interferometer array & source localisation (working note)

Extends the instrument from a single gradiometer to a spatially-separated array
of them, and adds source localisation as a target. Destined for the instrument
and scenario sections and §9 of `cavendish.tex`. Park behind scope-Q1 (what the
JEPA predicts), since it and the schedule layer both hang off that answer.

---

## 0. Why — spatial separation is a ULDM/GGN discriminant first, a localiser second

The instinct (recover position from the signal differences) is right, but the
strongest reason to build the array isn't localisation — it's that spatial
separation is a **discriminant between ULDM and GGN**, which lands directly on the
reframing in the test-case note (ULDM = signal, moving masses = GGN noise).

A single gradiometer cannot separate a coherent global oscillation from a local
gravitational transient at the same frequency. An array can, because they have
opposite spatial signatures.

---

## 1. The physics

### Common-mode vs differential across the array

- **ULDM is spatially coherent over the whole array.** A 4e-16 eV scalar has a de
  Broglie coherence length of order λ_C/(v/c) with v ~ 10⁻³c, i.e. ~10¹¹ m
  (astronomical-unit scale) — larger than any terrestrial array by many orders of
  magnitude, even against Earth's diameter (~10⁷ m). So every detector sees the
  *same* 0.1 Hz ULDM phase, in step. (It is also temporally coherent over ~10⁶
  cycles, which is why the peak is narrow and long coherent integration works.)
- **Local GGN is the opposite — a near-field effect.** A moving mass's gradient
  signal falls off with distance and looks completely different two metres over.
- **The cut:** **common-mode across the array → ULDM** (coherent everywhere);
  **differential across the array → local GGN** (different at each point). This is
  a handle on the *primary* goal — digging DM out of the gradient noise — not just
  the secondary one.

### Localisation (what was asked for — and it works)

Three independent handles on source position, all of which only exist with ≥2
spatially-separated detectors:

- **Differential amplitude.** The closer detector reads bigger; the ratios across
  detectors triangulate distance. Crucially, **mass is common-mode (scales every
  detector equally) while position is differential (changes the ratios)** — so the
  array *separates mass from distance*, breaking the single-gradiometer
  mass–distance degeneracy that has been the limiting ambiguity throughout.
- **Relative timing.** Closest approach to each detector happens at a different
  time as the source passes; the timing differences pin the trajectory geometry.
- **Tensor orientation.** Detectors at different positions see the gradient tensor
  at different bearings to the source, constraining direction.

This is **gravitational triangulation** — the same principle a gravitational-wave
or seismic network uses to localise by amplitude and time differences across
stations.

---

## 2. Honest caveats (so we don't oversell it)

- **Precision scales like (array baseline / source distance) × SNR.** A network
  localises well only when its baseline is comparable to the source distance:
  building-scale arrays nail nearby sources and degrade on distant or weak ones.
  This is exactly what the Fisher/CRB analysis will quantify, with **array
  placement geometry as a design knob** to optimise.
- **Do not literally difference the gradiometers for localisation.** Subtracting
  two of them common-mode-rejects and throws away the absolute amplitude that
  carries distance; each level of differencing also costs signal and worsens the
  conditioning already flagged (NFR-9). For localisation you want the **joint
  multi-channel signal**, with the inference forming whatever combination is
  optimal — common-mode for the coherent ULDM, differential for the local GGN. So
  "recover position from the difference" is better stated as **"from the joint
  structure across channels."**

---

## 3. Geometry — the physical near-term array

Atoms free-fall vertically, so the natural near-term array is **several vertical
gradiometers at different (x, y) ground positions** (different tower bases) — the
AION-network-like case. Arbitrary-orientation interferometers are a longer-term
abstraction; horizontally-separated vertical gradiometers are the physical case to
build first.

---

## 4. Architecture — mostly free

The key fact: **the gravity kernel does not care where the field points are.** It
already evaluates over arbitrary (x, y, z) points; they merely all lie on one
vertical line today. Adding detectors is just more field points at offset
placements.

- **Instrument** generalises from "one baseline" to a `Scene` holding a set of
  `Detector`s, each a gradiometer with a `Placement` (ground position + tower
  base; baseline vertical by default). N = 1 is today's case. This reuses the
  rigid-body "define in a local frame, transform per placement" pattern exactly,
  applied to detectors instead of bodies.
- **Source, trajectory, phase model: unchanged.** The double-difference stays
  *within* a detector; cross-detector relations are **derived**, not a new physics
  primitive.
- **StateBundle grows a detector axis:** signal becomes ΔΦ[detector, measurement];
  the label gains the source's full **3D position** (now identifiable); placements
  go in the scenario metadata.
- **Identifiability (§9) extends to localisation:** the CRB returns a **position
  covariance** given the array geometry — where "how well can we recover position"
  gets its quantitative answer.
- **Compute unchanged in kind:** the outer product gains a detector dimension
  (scenario × detector × timestep × field point) — the same parallel GPU kernel,
  more field points. If anything it strengthens the GPU-primary call.

---

## 5. Connection to scope-Q1 / the JEPA

- **Source localisation is a clean, well-posed JEPA target that only exists with
  the array** — a single gradiometer cannot pose it. This sharpens scope-Q1: the
  array makes "where is the source" a first-class, answerable output.
- The **multi-detector signal is a natural multi-channel input** to the temporal
  JEPA, where the **cross-channel structure is exactly what the latent should
  encode** (common-mode → ULDM, differential → GGN, ratios/timings → position).

---

## 6. Where this lands in the doc (next actions)

- **Instrument section** — generalise to `Scene` / `Detector` / `Placement`;
  N = 1 recovers today's single gradiometer.
- **StateBundle** — add the detector axis (ΔΦ[detector, measurement]); add the
  source's 3D position to the label.
- **§9 identifiability** — add the array-localisation CRB (position covariance
  given geometry); array placement is a design knob for the Fisher analysis.
- **Scenario metadata** — carry the array placement geometry.
- **Compute note** — add the detector dimension to the outer product.
- **Park behind scope-Q1**, alongside the schedule layer — all three depend on
  what the JEPA is actually predicting.
