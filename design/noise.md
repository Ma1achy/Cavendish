# Cavendish — `noise` drill-down

> Subsystem design for the `noise` crate: the realism stack — additive measurement noise plus the
> atmospheric gravity-gradient noise (GGN) that is the realistic floor the Gradar front-end must
> detect through. **This drill-down settles the deferred source-vs-noise question** (`DESIGN.md`
> §7). Companion to `DESIGN.md` (§3.3 seam, §6 inventory) and the spec (§noise, §atmo `sec:atmo`,
> `tab:atmo`, `carlton2025`).
>
> **Dependencies:** `noise → gravity` (the field types and the field-contribution interface, §7),
> `math`. The atmospheric model is wired into the scene by `scenario`; `compute` evaluates it.

---

## 1. The decision — two kinds of noise

Noise enters Cavendish in **two structurally different** ways, and conflating them was the open
question. Settled:

- **Additive measurement noise** (atom shot noise, vibration residual) perturbs the *measured
  phase* — it has nothing to do with the gravitational field. It is applied **post-hoc** to the
  computed signal: `NoiseSource::add` (§3).
- **Atmospheric GGN** is a *real gravitational signal* — a stochastic density field `δρ` that pulls
  on the atoms. It must go through the **full forward model** (the arm integral, the gradiometer
  double-difference) to get the right phase, and its finite **spatial correlation length** produces
  placement-dependent partial common-mode across the array. So it is realised as a **stochastic
  field source evaluated in the forward pass** (§6), **not** a post-hoc addition.

**Why a field source, not a `NoiseSource::add`.** A post-hoc scalar add could not capture the
double-difference or the per-detector correlation without re-running the forward model on the GGN
field anyway — so evaluating it *in* the pass is both more correct and cheaper (one pass over bodies
+ field). It is "noise" only at the **configuration** level: a thin adaptor lets it be declared in
the noise stack, and `scenario` wires it into the scene as a field contributor (§7). This realises
the `DESIGN.md` §7 lean ("stochastic source with a thin noise adaptor").

---

## 2. Responsibility & boundaries

**Owns:** the `NoiseSource` trait and stack for additive noise (`ShotNoise`, `VibrationResidual`);
the **atmospheric GGN field model** (the Carlton parameterisation, its seeded realisation, and its
field evaluation).

**Does not own:** the forward model (`instrument`), the field kernel (`gravity`), execution
(`compute`), scene assembly (`scenario`). For ULDM see `uldm` (a different channel — common-mode,
not a floor).

**Invariants:** every component is **seeded and deterministic** given the RNG key
(`compute.md` §8); additive components are applied **after** the forward model is validated; the
stack **may run empty** (spec) — atmospheric GGN is the principal growth target, not a v1
requirement.

---

## 3. The additive stack — `NoiseSource`

```rust
pub trait NoiseSource {
    fn add(&self, t: &[f64], scene: &DetectorArray, signal: &mut SignalBatch, rng: &mut CbrngKey);
}
// impls: ShotNoise, VibrationResidual.  (Atmospheric GGN is NOT here — §6.)
```

- **`ShotNoise{n_atoms, C}`** — the atom-counting limit `σ_φ ≈ 1/(C√N_atoms)`; an independent
  Gaussian per (detector, measurement), seeded. The fundamental measurement floor.
- **`VibrationResidual{psd, rejection}`** — platform vibration as a coloured time-series from a
  PSD, **reduced by the gradiometer common-mode rejection ratio** (the two IFOs share the platform,
  so most vibration cancels; a residual leaks), then added. The `scene` argument lets it respect
  per-detector structure if needed; shot noise ignores it.

Applied post-forward-model (§8); both deterministic given the key.

---

## 4. (reserved — additive components extend here)

New additive measurement-noise terms (detection-efficiency, laser-frequency residual, …) slot in as
further `NoiseSource` impls without touching the forward pass.

---

## 5. Atmospheric GGN — the model

The Carlton 2025 model (spec `sec:atmo`, `tab:atmo`), two channels, each mapping a fluctuation to
`δρ/ρ₀` and thence to a potential the atoms feel:

- **Infrasound (pressure).** Adiabatic `δρ/ρ₀ = δp/(γp₀)`; plane waves reflecting off the half-space
  surface. A realised set of plane waves (amplitudes from the global high/​low models, seeded
  directions/​phases) gives a **closed-form** potential and gradient at any `(z,t)` — cheap.
- **Temperature (turbulence).** `δρ/ρ₀ = δT/T₀`; thermal eddies advected at wind `U` (Taylor's
  frozen turbulence). A realised sum of Fourier modes drawn from the Greenwood–Tarazano spectrum
  `Φ(k) ∝ (k²+k/Λ)^{−11/6}` (outer scale `Λ` at the Obukhov length) gives a **numerical** field —
  heavier than infrasound.

The realisation is **seeded** per scenario (counter-based RNG, `compute.md` §8), so the field is
reproducible.

---

## 6. Atmospheric GGN — as a field source in the forward pass

The realised `δρ` field is a **field contributor** to `V(z,t)`: it provides potential/​gradient at a
point and time, exactly the quantity a rigid body's posed cloud provides (§7), and the forward model
sums it with the bodies. Consequences that fall out *for free* by doing it this way (and would not
from a post-hoc add):

- it passes through the arm integral and the **double-difference** (spec `eq:singlephi`/​
  `eq:doublediff`), so its phase is physically correct;
- its **finite correlation length** (infrasound wavelength ≈343 m @ 1 Hz; eddy scale ≈170 m) makes
  it **partially correlated** across horizontally separated detectors — between fully common-mode
  ULDM and a fully local mass (spec §array), so the array separates all three regimes;
- **depth** `z₀` mitigates it (temperature falls off sharply; vertical infrasound penetrates
  furthest) — automatic, since `z₀` enters the field evaluation.

The **thin `NoiseSource` adaptor** is config-only: declaring `atmospheric{…}` in the noise stack
produces an `AtmosphericField` that `scenario` adds to the scene's contributors; it does **not**
implement `add`.

---

## 7. The field-contributor generalisation

Atmospheric GGN motivates one clean refinement to the forward model: `V(z,t)` is a **sum over
heterogeneous field contributors**, not only rigid bodies.

```rust
pub trait FieldContribution {            // lives in `gravity` (owns the field types)
    fn potential<S: Scalar>(&self, p: Vec3<S>, t: f64) -> S;
    fn gradient_tensor<S: Scalar>(&self, p: Vec3<S>, t: f64) -> Mat3<S>;
}
```

- A **rigid body** satisfies it via `source`'s pose + `gravity`'s kernel on the posed cloud (the v1
  path, unchanged).
- **`AtmosphericField`** (this crate) satisfies it via the closed-form/​spectral model (§5).

`instrument`'s forward model sums `&dyn FieldContribution`; the rigid-body path is exactly as
designed (`instrument.md` §4), with atmospheric GGN as a second contributor kind. (A body remains a
`SourceDynamics` for the bundle's track labels; `FieldContribution` is just its field view.)

---

## 8. Connection to `compute`

- **Additive noise** (shot, vibration) is applied **after** the forward pass — in `generate`'s
  post-step (`DESIGN.md` §4), or as a final `NoiseSource` sweep — never inside the field kernel.
- **Atmospheric GGN** is evaluated **inside** the forward pass (Pass 2, `compute.md` §6) as another
  `FieldContribution`. Infrasound's closed form is cheap on the GPU; the **temperature spectral
  field** is heavier (a mode sum per arm point per time) — realised mode amplitudes are precomputed
  on the CPU and uploaded, evaluated on the GPU, or atmospheric runs are kept to the CPU path where
  throughput is not the constraint (an implementation call, §12).
- Both draw from the **counter-based RNG** keyed by `(global_seed, scenario_id, stream_id)`, so
  noise and field realisations are reproducible across CPU/​GPU and evaluation order.

---

## 9. Determinism

Every component is a pure function of `(batch, RNG key)`. The additive stack is bit-reproducible on
the CPU; the atmospheric field inherits the backend's reproducibility class (f32/​reordered on GPU).
Seeds compose through the key tree, never through sequential state.

---

## 10. Errors & API

Construction validates each component (`σ_φ ≥ 0`, a well-formed PSD, physical `tab:atmo`
parameters) → `Result<_, NoiseError>`; `add` and the field evaluation are infallible. Public
surface: `NoiseSource`, `ShotNoise`, `VibrationResidual`, the stack type, `AtmosphericField` (+ its
config and the adaptor), and `FieldContribution` is re-exported from `gravity`. `scenario` assembles
the stack and wires the atmospheric field; `compute`/​`generate` apply them.

---

## 11. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| shot noise | per-measurement Gaussian with `σ_φ = 1/(C√N_atoms)`; white | statistics |
| vibration rejection | residual scales with the rejection ratio; mostly common-mode | tol |
| stack empties | an empty stack returns the clean signal unchanged | exact |
| seeded reproducibility | identical realisation given the key, across CPU/​GPU and order | exact / class |
| atmospheric structure | the field passes through the double-difference; partial cross-detector correlation with the right correlation length; `z₀` mitigates | model |
| atmospheric spectrum | infrasound + temperature PSDs match the `sec:atmo` forms in band | shape |

**Scope note:** the v1 exit is the model *structure* — the two channels, the field-contributor
wiring, the correlation/​depth behaviour. The **global amplitude models** (Bowman/​Kristoffersen for
infrasound, ERA5 for temperature) are the implementation-time addition when realistic *levels* are
wanted (spec `sec:atmo`); not required for the structure to be correct.

---

## 12. Open sub-questions (resolve in implementation)

- **Temperature field on the GPU.** Precompute-and-upload mode amplitudes vs an on-device mode sum
  vs CPU-only atmospheric runs — a throughput/​footprint call (the mode count can be large).
- **Shot/​vibration placement.** As a final `NoiseSource` sweep in `compute` vs in `generate`'s
  post-step — either is deterministic; pick for where the signal buffer is most naturally owned.
- **Global amplitude models.** Which empirical model and season to pull when realistic levels are
  added — a data-ingest task, deliberately out of the structural v1.
- **`FieldContribution` ergonomics.** Final shape of the trait and how `compute` dispatches over a
  heterogeneous contributor list on the GPU (bodies vs field) without dynamic dispatch in the kernel.
