# Cavendish — identifiability: what the signal recovers, and shape as model selection

Sharpens what "recover the source" means for the Gradar target. The recoverable content is
stratified by multipole order, free-form density is impossible in principle, and restricting
to a shape family is not a fallback but the regulariser that makes the inverse well-posed.
Destined for §5 (the inference target) and the §9 / M5 Cram\'er--Rao output of
`cavendish.tex`. Builds on `multi-interferometer-localisation.md` and
`gradar.md`.

---

## 0. The organising principle: the signal stratifies by multipole order

The exterior field is a multipole series — monopole, (zero) dipole, quadrupole, … — and each
order carries a different power of (source size / distance). Recoverability falls off with
order, so **the multipole expansion already in the spec *is* the identifiability ladder**:
"what can be recovered" is answered moment by moment, not all at once. Everything below is
that ladder.

---

## 1. Trajectory and total mass — the robust rung (monopole)

- **Centre-of-mass motion lives entirely in the monopole**, and the monopole is the strongest
  term (it falls slowest, ~1/r). So position-over-time — hence velocity — is the part the
  array recovers well. This is the easy, robust content.
- **Total mass is the monopole strength.** Recoverable, but mass--distance degenerate for a
  single gradiometer (a heavy-far and a light-near source share a monopole). This is exactly
  the degeneracy the array breaks (mass is common-mode, position differential — `§4.2`).

---

## 2. Shape — the hard rung, and exactly how hard (quadrupole)

- **The dipole vanishes about the centre of mass**, so there is no 1/r² shape term: shape
  *onsets* at the quadrupole.
- **The quadrupole/monopole signal ratio scales as (a/r)²** (source size a, distance r), and
  this is invariant across which field quantity is measured — Φ, **g**, or Γ — because the
  extra 1/r powers cancel in the ratio (monopole and quadrupole both pick up the same factor
  when you differentiate). **That single number, (a/r)², is the entire shape-SNR budget.**
- **Numbers.** AION scaffold: members ~metres, distances ~metres → (a/r)² is order-unity-ish,
  so shape is *marginally* accessible. A distant lift → (a/r)² tiny → you get trajectory and
  mass and nothing about shape.
- **This is already a test.** T2.4 (cylinder-vs-point-mass sweep) is literally the forward map
  of this budget: the departure-from-monopole curve over (R, H, distance) *is* (a/r)² plotted.
  The diagnostic is in the doc.

---

## 3. The hard truth: free-form density inversion is impossible

The exterior gravitational inverse problem is genuinely **non-unique, in principle, not merely
ill-conditioned**:

- A uniform spherical shell is gravitationally identical to a point mass of the same total
  mass. More generally, interior mass can be **redistributed across a whole equivalence class
  without changing a single external moment**.
- So free-form (voxel) density reconstruction from the exterior signal is **impossible** — no
  amount of SNR, array baseline, or cleverness recovers interior structure the field cannot
  encode. This kills naive "invert the signal for ρ(x)" as a target.

---

## 4. Shape as model selection (the correct formulation)

This is what turns the impossibility into a well-posed problem, and it is why the instinct
"infer it's a cylinder with these params" is the *right* formulation, not a weaker one:

- **Restricting to a shape family is the regulariser** that makes the inverse well-posed — it
  picks one representative of the equivalence class.
- So the task is **model selection over a dictionary** {sphere, cylinder, cuboid, sheet, …}
  **+ pose + scale**, not field inversion. Finite-dimensional, well-posed, and it only ever
  asks for the low-order moments — which is all that is recoverable anyway (§2). **The
  restriction and the physics agree exactly**: you give up only the structure the field never
  carried.
- Maps straight onto the JEPA latent as a **shape-class posterior + continuous moment
  regression**, and is consistent with the engine's rigid source already being a parameterised
  primitive (the dictionary is the same body library the simulator samples from).

---

## 5. The bonus: with the class, the low moments close the geometry

The shape prior does more than classify — it lets the recoverable moments yield the geometry:

- **The quadrupole alone gives elongation, not R and H separately.** For a uniform cylinder
  about its axis, Q_zz ∝ (H² − 3R²): one number, the prolate/oblate combination.
- **Add the class + a uniform-density assumption and the system closes.** Monopole gives
  M = ρπR²H; quadrupole gives (H² − 3R²); two equations, two unknowns → back out **(R, H)**.
  The density assumption (ρ known for the material) is exactly what converts recoverable mass
  into geometry.
- **A degeneracy worth flagging:** H = √3·R gives Q_zz = 0 — a cylinder with that aspect ratio
  is indistinguishable from a sphere/point mass at quadrupole order. The model-selection power
  is genuinely zero there, not just small.

---

## 6. What the array adds: full tensor → orientation

- A **single vertical gradiometer reads ≈ one projection of Γ**, hence one projection of Q —
  so it sees the *magnitude* of elongation along one line and can only guess orientation.
- The **array's spatial diversity samples Q from multiple bearings**, reconstructing the
  **full quadrupole tensor**, whose eigenvectors are the principal axes. **Orientation becomes
  observable**, not just magnitude.
- The upgrade is from "it's elongated" to "it's a cylinder pointing *that way*." It does **not**
  repeal (a/r)² — nothing does; it makes better use of whatever quadrupole signal that budget
  allows.

---

## 7. Velocity caveat

Velocity is the derivative of the recovered track, so its limits are temporal, not multipolar:
the **within-flight smear** (each ΔΦ_ℓ is a 1.46 s integral, so a source moving appreciably
*during* a flight blurs) and the **2 s cadence**. Fine for 0.1 Hz GGN; a hard wall for
anything fast.

---

## 8. The recoverability ladder (summary)

| Content | Source | Status |
|---|---|---|
| Position over time, velocity | monopole (CoM motion) | robust (array-aided) |
| Total mass | monopole strength | recoverable; mass–distance degenerate, broken by array |
| Elongation magnitude | quadrupole, one projection | marginal — budget is (a/r)² |
| Orientation, full shape moments | full quadrupole tensor | array only; still (a/r)²-limited |
| Geometry (R, H) for a known class | monopole + quadrupole + ρ | closes *given* the shape prior |
| Interior density ρ(x), free-form | — | **impossible in principle** |

---

## 9. Where this lands in the doc (next actions)

- **§5 (inference target):** state the identifiability ladder and that the shape target is
  **class + pose + scale + low moments**, not a voxel field — with the non-uniqueness as the
  reason. This makes "recover the source" precise.
- **§9 / M5 CRB:** the Fisher output should report not just **position covariance** but
  **quadrupole detectability** and the **model-selection power between shape classes** at a
  given (a/r, SNR). That is the rigorous, quantitative answer to "is shape recoverable here."
- **Validation tie-in:** T2.4's departure-from-monopole sweep is the empirical (a/r)² budget —
  reference it as the forward-map check behind the CRB's shape claims.
- **Label / StateBundle:** the shape label is the dictionary entry + parameters, consistent
  with the parameterised body library the engine already samples; no new representation needed.
