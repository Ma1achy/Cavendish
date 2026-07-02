# Cavendish — `instrument` drill-down

> Subsystem design for the `instrument` crate: how a gradiometer turns a gravitational field into
> a differential phase. This closes the forward-model loop — it is the crate `compute` executes.
> Companion to `DESIGN.md` (§3.2 seam, §6 inventory) and the spec (§forward `sec:forward`,
> `sec:traj`, the gradiometer geometry, `sec:array`). Numbers live in the spec; this is the
> structure and the contracts.
>
> **Dependencies:** `instrument → gravity` (the potential/​gradient kernel), `source` (the
> `SourceDynamics` it evaluates), `math`. It does **not** depend on `compute`; `compute` depends on
> it (it calls/​re-expresses the phase kernel, §9).

---

## 1. Responsibility & boundaries

**Owns:** the `PhaseModel` seam and its two implementations; the detector geometry (`Detector`,
`DetectorArray`); the four **arm trajectories** and their construction; the gradiometer
double-difference.

**Does not own:** the field math (`gravity`), source kinematics (`source`), execution/​batching
(`compute`), the ULDM channel (`uldm` — added downstream), scene assembly (`scenario`).
`instrument` *defines* the measurement; it neither moves the sources nor schedules the run.

**Invariants it guarantees:** the **double-difference is internal** to a detector (one detector →
one scalar `ΔΦ`; the array's cross-detector structure is downstream); **`N=1` is a single
gradiometer** on the same code path; the response is **linear in source mass** (spec
`eq:doublediff`); common-mode (laser phase, uniform `g`) **cancels** in the difference.

---

## 2. The `PhaseModel` seam

```rust
pub trait PhaseModel {
    /// One gradiometer's differential phase at measurement time t, given the scene of sources.
    fn delta_phi(&self, sources: &[&dyn SourceDynamics], det: &Detector, t: f64) -> f64;
}
// impls: PropagationIntegral (the reference, spec v1) and QuasiStaticGradient (the fast path).
```

`delta_phi` returns **one detector's** `ΔΦ_ℓ`. The array is iterated outside (in `compute`/​
`generate`) to produce the per-detector signal vector; the localisation/​tensor reconstruction over
that vector is the Gradar tracker's job (spec §Gradar), not `instrument`'s.

---

## 3. Geometry — the detector, the array, the four arms

A **`Detector`** is one gradiometer: two Mach–Zehnder ⁸⁷Sr interferometers stacked vertically,
interrogated by **common lasers**, launched `Δr` apart in the tower and sensitive to the
*difference* in local gravity between them (spec §gradiometer). The atom/​sequence parameters are
one apparatus (shared across the array); only the **placement** varies per detector — it is the
free parameter and the analysis knob (spec `sec:array`).

```rust
pub struct DetectorArray { detectors: Vec<Detector>, config: InstrumentConfig }
pub struct Detector      { placement: Isometry3 }          // ground position (v1: vertical axis, §8)
pub struct InstrumentConfig {
    m_a: f64, k_eff: f64, n_kick: u32,    // ⁸⁷Sr mass; 2π/λ (λ=698 nm); LMT kicks (1000)
    half_seq: f64, cadence: f64,          // T (2T = interrogation); Δt (measurement spacing)
    ifo_sep: f64, tower: f64,             // Δr (5 m); tower height (10 m)
    g: f64, gamma_zz: f64, fine_dt: f64,  // local gravity; static Earth gradient; integration step
    phase_model: PhaseModelKind,          // §6/§7, default PropagationIntegral
}
```

**The four arm trajectories depend only on the instrument and are built once** (spec `sec:traj`):
IFO1 upper, IFO1 lower, IFO2 upper, IFO2 lower. Each is **ballistic free-fall under `−g`** from a
launch with `±v_rec` (`v_rec = n_kick·ℏk/m_A ≈ 6.5 m/s` — the arm-separation velocity that sets the
*absolute* signal scale), with the `π`-pulse swapping the recoil at `t=T`. Closed-form parabolas;
no per-scenario state. `DetectorArray` resolves the `DESIGN.md` §7 name clash (`DetectorArray` for
the instrument side; `Vec<Source>` for sources).

---

## 4. The forward model — two levels of differencing

Along the vertical flight an atom follows the semiclassical Lagrangian
`L = m_A(½ż² − gz + ½γ_zz z² − V(z,t))`, with `V` the source potential `gravity` evaluates
(`eq:cloudpot`). The phase is built from two nested differences:

1. **Arm difference (within one interferometer)** — the propagation phase is the time integral of
   the potential *difference between the two arms* (spec `eq:singlephi`):
   `δφ_ℓ = (m_A/ℏ) ∫[V(z_u,t) − V(z_l,t)] dt` over `[ℓΔt, ℓΔt+2T]`. The arms separate by up to
   `~v_rec·T`, so this is a genuine finite difference across the wavepacket — the single-IFO signal.
2. **Gradiometer double-difference (across the two interferometers)** — `ΔΦ_ℓ = δφ_{ℓ,2} − δφ_{ℓ,1}`
   (spec `eq:doublediff`). The two IFOs see nearly the same source (differing by `Δr/D`), so the
   common-mode cancels and the residual carries the GGN.

The signal is the series `{ΔΦ_ℓ}` over measurements; `ΔΦ ∝ m` (linear in source mass). The static
Earth gradient `γ_zz` is a constant background that drops out of the differential signal — carried
as a config constant, not re-derived per shot.

---

## 5. Arm construction

Built once from `InstrumentConfig`, reused for every scenario and measurement. Per arm: launch
height (the IFO position), launch velocity (`±v_rec`), free-fall under `−g`, the `π`-pulse velocity
swap at `T`, a reflecting floor at launch height. The result is four closed-form `z(t)` on
`[0, 2T]`, sampled by the phase models. Because the arms are instrument-only, they are **not**
recomputed when sources move — only `V` along them changes. (`compute`'s Pass 2 evaluates the arm
points inline, `compute.md` §6.)

---

## 6. `PropagationIntegral` — the reference (spec v1)

The faithful model. For each IFO, quadrature of `∫[V(z_u,t) − V(z_l,t)] dt` over `2T` at `fine_dt`
steps (`V` from `gravity` at the arm points, body-frame trick over the source clouds), then the
double-difference. Captures source time-variation over the flight and finite arm/​IFO separation —
everything the spec's `eq:singlephi`/​`eq:doublediff` contain, with no small-separation assumption.
This is the oracle the fast path and the GPU are checked against.

---

## 7. `QuasiStaticGradient` — the fast path

When the source varies slowly over `2T` and `Δr ≪ D`, both differences linearise: the arm
difference reduces to the local source field and the double-difference to the gradient, giving
`ΔΦ ≈ C · Γ_zz · Δr` with `C` a kinematic constant (`m_A/ℏ` × the trajectory-area factor
`∫(z_u−z_l)dt`). One `gradient_tensor` evaluation per detector per measurement (at the tower
midpoint) instead of four arm quadratures — the cheap path for large batches and the leading-order
sanity check on `PropagationIntegral`. **Selection** is a per-run `InstrumentConfig` flag (resolving
the `DESIGN.md` §7 question): default `PropagationIntegral`, `QuasiStaticGradient` opt-in for speed.

---

## 8. The array — per-detector signal, `N=1`, orientation

`delta_phi` is evaluated per detector to yield the signal vector `ΔΦ[detector, measurement]`; the
array combination that breaks the mass–distance degeneracy is downstream (spec `sec:array`,
Gradar). **`N=1`** is the same code path with one detector — no special case. **Orientation:** v1
detectors are vertical gradiometers separated **horizontally** (the placement varies, the axis is
vertical); the arbitrary-orientation interferometer is a kept-but-unbuilt seam (spec §deferred), so
`Detector.placement` carries a full `Isometry3` but v1 populates only the translation + vertical
axis.

---

## 9. Connection to `compute` — the phase kernel and conditioning

`instrument` owns the **canonical** phase math (Rust, generic over `Scalar`): a
`fn delta_phi_kernel<S: Scalar>(…)` the `PhaseModel` impls wrap, `compute`'s `CpuBackend` calls
directly, and `compute`'s `WgpuBackend` re-expresses in WGSL — the same one-Rust-truth-one-GPU-
re-expression pattern as the gravity kernel and the source integrator.

**Differential-first (spec `nfr:cond`) is an `instrument` obligation, not just `compute`'s.** The
gradiometer subtracts two nearly-equal single-IFO phases, so forming large absolute `δφ`'s and
subtracting would lose the mrad signal in f32. The kernel instead accumulates the **differenced
integrand** `[V(z_u,2)−V(z_l,2)] − [V(z_u,1)−V(z_l,1)]` as one conditioned quantity (via the field
gradient along the trajectories) — `QuasiStaticGradient` is inherently conditioned (`∝ Γ_zz·Δr`,
no large-absolute subtraction). This is what keeps `WgpuBackend`'s f32 path above the floor
(`compute.md` §6).

---

## 10. Errors

Construction validates `InstrumentConfig` and returns `Result<_, InstrumentError>`: non-positive
`T`/​`Δr`/​`tower`, `Δr > tower`, zero `n_kick`, a placement with a degenerate axis. After
construction `delta_phi` is infallible. (Source/​field errors surface from those crates at their
own construction.)

---

## 11. Public API surface

`PhaseModel`, `PropagationIntegral`, `QuasiStaticGradient`, `Detector`, `DetectorArray`,
`InstrumentConfig`, `PhaseModelKind`, the arm-trajectory builder, and `delta_phi_kernel` (exposed
for `compute`). `scenario` assembles the `DetectorArray`; `compute` consumes the kernel + the arm
trajectories; `generate` iterates the array.

---

## 12. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| **published anchors** | with `source`+`gravity`: 10 kg @ D=10 m, 2.5 m/s → ~7 mrad; 1 kg @ 5 m osc → ~2 mrad; concrete wall → ~50 µrad; scaffold → ~10 µrad | order/​≈ |
| common-mode rejection | uniform `g` / shared laser phase / a far uniform field → `ΔΦ ≈ 0` | floor |
| model agreement | `QuasiStaticGradient` ≈ `PropagationIntegral` for slow source, `Δr ≪ D`; diverge correctly otherwise | tol |
| mass linearity | `ΔΦ ∝ m_source` | exact (`eq:doublediff`) |
| `N=1` | single-detector array on the same path | structural |
| arms instrument-only | four arms built once; unchanged as sources move | structural |
| conditioning | f32 `ΔΦ` stays above the floor; no large-absolute cancellation (§9) | floor |
| autodiff | `delta_phi` differentiable w.r.t. source params via `Dual` | ~1e-6 |

The **published anchors** are the headline: reproducing them (an integration test over `instrument`
+ `source` + `gravity`) is what certifies the forward model is physically right — the foundation the
whole data engine rests on.

---

## 13. Open sub-questions (resolve in implementation)

- **Arm trajectory fidelity.** How precisely to model the reflecting floor, finite pulse duration,
  and any transverse motion — vs the vertical ballistic idealisation. Set by matching the anchors.
- **`Γ_zz` evaluation point** for `QuasiStaticGradient` — tower midpoint vs a per-IFO pair; the
  midpoint is the leading order, a pair captures the next term. Pick by agreement with the integral.
- **Orientation activation.** When the arbitrary-orientation seam is eventually built, whether a
  per-detector axis needs new arm construction or just a rotated placement frame.
- **`γ_zz` handling.** Whether the static Earth gradient is ever needed beyond a constant (e.g. for
  realism in the absolute phase) or stays a dropped-in-the-difference background.
