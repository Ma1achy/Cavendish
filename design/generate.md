# Cavendish — `generate` drill-down

> Subsystem design for the `generate` crate: the **orchestration** layer. It takes a `Scenario`,
> drives the forward model, and fills a `StateBundle`; and it streams batches for dataset generation.
> It is the *conductor* — it defines no new physics, it wires the seams together. Companion to
> `DESIGN.md` (§4 control flow, §6 inventory) and the spec (§bundle `tab:bundle`, §atmo `sec:atmo`).
>
> **Dependencies:** `generate → scenario, state, compute, source, instrument, uldm, noise, gravity,
> config` — everything below it. Consumed by `sdk`; sibling to `analysis`.

---

## 1. Responsibility & boundaries

**Owns:** the two entry points — `run(scenario) → StateBundle` (one) and `stream(...) → impl
Iterator<StateBundle>` (a batch) — and the order in which the forward model is assembled and the
bundle filled.

**Does not own:** any physics. The field and phase live in `gravity`/`instrument`, the motions in
`source`, the common-mode line in `uldm`, the additive stack in `noise`, the execution in `compute`,
the output shape in `state`, the experiment in `scenario`. `generate` holds none of these — it
*calls* them.

**Invariant it guarantees:** a `StateBundle` is a pure, reproducible function of `(Scenario,
backend)` (§8) — `generate` adds no hidden state between the seams.

---

## 2. The single `run` — the conductor sequence

For one `Scenario`, in order:

1. **Resolve.** Build the `DetectorArray` and its four ballistic arms *once* (`instrument.md` §4);
   build each `Source`'s dynamics (`source.md` — closed-form motions become closures, the two ODE
   motions are readied for on-device integration).
2. **Poses (compute Pass 1).** `compute` generates every source's pose at every measurement time,
   integrating ODE motions on-device with `fine_dt` substeps (`compute.md` §5). Parameters up, poses
   back — not uploaded.
3. **Realise the atmospheric field** (if present). Seeded `δρ` field, a `FieldContribution`
   (`noise.md`; spec `sec:atmo`) — a stochastic *source*, part of the forward pass, not post-hoc.
4. **Field + phase (compute Pass 2).** For each measurement × detector, the chosen `PhaseModel`
   evaluates `ΔΦ`, summing the `FieldContribution`s (target bodies **+** atmospheric field). If the
   decomposition is requested, this runs per field-source group (§4).
5. **ULDM.** Add the closed-form common-mode phase (`uldm`), broadcast identically across detectors.
6. **Post-hoc noise.** Apply the `NoiseSource` stack (shot, vibration) in order to the clean signal
   (§5).
7. **Derived** (iff requested). Shape descriptors via the `gravity` reduction; the Lomb–Scargle
   periodogram via `state` (`state.md` §5, §7).
8. **Assemble.** Fill the `StateBundle`, computing *only* the requested fields (§7).

`generate` is exactly this sequence and nothing more — the physics is behind each call.

---

## 3. Forward-model wiring — the seams compose

The forward model is an assembly of the four seam traits plus ULDM, and `generate` is where they
meet: `SourceDynamics` supplies poses; the world potential is a **sum of `FieldContribution`s**
(rigid bodies + atmospheric, `noise.md`); the `PhaseModel` turns that field into `ΔΦ` over the
`DetectorArray`; the `NoiseSource` stack and the ULDM line are added on top; and `ComputeBackend`
runs all of it (`compute.md`). `generate` holds the composition, not the pieces — swap a
`PhaseModel` or add a `FieldContribution` and the conductor is unchanged.

---

## 4. The signal decomposition — superposition, exactly

The redesigned bundle (`state.md`) offers the ground-truth channels `signal_targets`,
`signal_atmospheric`, `signal_uldm`, `signal_noise`, which **sum to** `signal`. This works because
**the phase is linear in the source potential** (the propagation integral is linear in `V`, and the
quasi-static path is linear in `Γ`; both PhaseModels, spec `eq:singlephi`/`eq:doublediff`). Since
`V = V_targets + V_atmospheric`, the gravitational phase superposes:
`ΔΦ_targets + ΔΦ_atmospheric = ΔΦ_gravitational`. So `generate`:

- **decomposition off** (default) — runs Pass 2 *once* with the combined field (cheapest);
- **decomposition on** — runs the phase evaluation *per field-source group* (targets-only, then
  atmospheric-only), records each channel, and sums. ULDM and noise are already separate channels.

This is the ≈2× gravitational cost flagged in `state.md` §3 — a genuine compute choice, hence a
`FieldSet` flag. The decomposition is **exact**, not an approximation: linearity makes the channels
add back to `signal` to numerical tolerance (an exit check, §11).

---

## 5. Post-hoc noise

Shot and vibration are additive and applied *after* the clean forward signal, by walking the
`NoiseSource` stack in its defined order (order is significant, spec `NoiseSource` contract). Each
draws from the seeded counter-based RNG (§8). `signal_noise` (if requested) records the realisation
actually added, so `signal − signal_noise` recovers the clean signal exactly. Atmospheric GGN is
**not** here — it entered as a field source in step 4 (the one subtlety `noise.md` settled).

---

## 6. Batch & stream dispatch

`stream` is the dataset path. Its source is a `Vec<Scenario>` or a `Prior` (sampled, `scenario.md`
§6); it hands `compute` an `EvalBatch` (parallel over scenario × source × measurement,
`compute.md`) and yields `StateBundle`s. Two properties matter:

- **Memory-bounded.** It computes a batch, yields those bundles, and moves on — it never holds the
  whole dataset. Batch size is a throughput/memory knob (§12), not a semantic one.
- **Batch-invariant.** A bundle from a batch is identical to the same `Scenario` run alone (§8) —
  batching is an execution detail, checked at the exit (§11).

This is the "generate ML datasets at scale" path (spec §1): reproducible from a seed, tensor-native,
and streamed rather than materialised.

---

## 7. `FieldSet`-driven work

`generate` reads the `Scenario`'s `FieldSet` (`state.md` §3) and does the minimum: the mandatory
motion + `signal` + `mask` always; the `shape` reduction, the `decomposition` extra passes (§4), the
heavy `field` samples, and the `periodogram` **only** when their flag is set. The knob is realised
here as *work not done* — an unset flag means those kernels never launch and those tensors are never
allocated. Selection changes cost, never meaning.

---

## 8. Determinism & seeding

Every stochastic step keys the **counter-based RNG** (`compute.md` §8) from the `Scenario`'s `seed`
through a fixed sub-tree — atmospheric realisation, then each `NoiseSource` — so a run is a pure
function of `(Scenario, backend)`, reproducible across CPU/GPU, ordering, and batching. `generate`
introduces no sequential RNG and no shared mutable state between scenarios, which is what makes the
stream order-independent and the batch invariant.

---

## 9. Backend selection

`generate` holds the `ComputeBackend` and picks it from a small `RunConfig` (device, batch size),
defaulting to GPU where available and CPU otherwise (`compute.md`: CPU is the bit-exact reference,
GPU the differential-first fast path). It uses the **forward** backend only; the `Dual` autodiff
path is `analysis`'s concern, not `generate`'s — producing the bundle never needs derivatives.

---

## 10. Errors & API

Public surface: `run(&Scenario) → Result<StateBundle, RunError>`, `stream(...) → impl
Iterator<Item = Result<StateBundle, RunError>>`, and `RunConfig`. A single `run` fails only on
backend/resource errors (the `Scenario` was validated at construction, `scenario.md` §9). In a
stream, a failing `Scenario` yields an `Err` for that item **without** killing the stream (§12), so
one bad draw does not lose a batch. `sdk` wraps these for Python; `analysis` consumes the same
forward model separately.

---

## 11. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| end-to-end forward | a `Scenario` runs to a `StateBundle` reproducing the published anchors (spec) through the full wiring | model |
| decomposition superposes | `signal_targets + signal_atmospheric + signal_uldm + signal_noise` equals `signal` | tol |
| noise recoverable | `signal − signal_noise` recovers the clean signal; stack applied in order | exact |
| atmospheric in-pass | atmospheric contribution appears in `signal_targets`-free channel, not in post-hoc noise | structural |
| `FieldSet` minimal | an unset flag ⇒ that work never runs and that tensor is absent | structural |
| batch-invariant | a batched bundle equals the same `Scenario` run alone | exact |
| memory-bounded stream | streaming a large batch holds ≪ the whole dataset | structural |
| determinism | same `(Scenario, backend)` → identical bundle; stream order-independent | exact |

`generate` needs the whole stack beneath it; its correctness is *compositional* — given the seams'
own exits, `generate`'s job is that the wiring, superposition, seeding, and batching are faithful.

---

## 12. Open sub-questions (resolve in implementation)

- **Stream shape.** A synchronous `Iterator`, an async `Stream`, or a bounded channel with a worker
  pool — and the backpressure model when the consumer is slower than the GPU.
- **Batch sizing.** How the batch size is chosen against GPU memory and the `FieldSet` (the
  volumetric `field_grid` shrinks a feasible batch dramatically); auto-tuning vs a config knob.
- **Decomposition fusion.** Whether the targets-only and atmospheric-only passes can be fused into
  one kernel launch with two accumulators, recovering most of the 2× (§4) — an optimisation, not a
  correctness matter.
- **Mid-stream failure policy.** Whether an `Err` item logs-and-continues, retries with a fresh
  sub-seed, or is configurable; how partial batches are reported.
