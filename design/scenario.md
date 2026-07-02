# Cavendish — `scenario` drill-down

> Subsystem design for the `scenario` crate. The primary contract is dead simple: a **`Scenario`**
> is a fully-specified runnable unit, and `generate` turns it into a `StateBundle`. **Direct
> construction is the main path.** A **`Prior`** is *optional sugar* — a sampler that produces
> `Scenario`s for the "generate a big varied batch reproducibly" case; it is not the contract and
> nothing requires it. Companion to `DESIGN.md` (§2 nouns, §6 inventory) and the spec (§schedule
> `sec:schedule`, §array `sec:array`, §Gradar `sec:radar`).
>
> **Dependencies:** `scenario -> source, instrument, uldm, noise, config`. Does **not** depend on
> `compute`/`generate`; those consume `Scenario`s.

---

## 1. Responsibility & boundaries

**Owns:** the **`Scenario`** (the runnable unit — the contract); the **`Schedule`** (measurement
cadence + contamination); and an **optional `Prior`** (a convenience sampler, §6).

**Does not own:** the domain pieces (`source`, `instrument`, `uldm`, `noise` define them;
`scenario` composes them), execution (`compute`/`generate`), the static config schema (`config`).

**The one irreducible operation is `Scenario -> StateBundle`.** Everything else here — composition,
the schedule, the `Prior` — is in service of building or batching `Scenario`s; none of it is
required to run one. A hand-built `Scenario` handed to `generate` is the whole story.

---

## 2. The `Scenario` — the runnable unit (the contract)

```rust
pub struct Scenario {
    sources:  Vec<Source>,        // the scene — one or many targets (§4)
    array:    DetectorArray,      // from instrument
    uldm:     Option<UldmConfig>, // the common-mode channel, present or not
    noise:    NoiseStack,         // additive stack + optional atmospheric field (from noise)
    schedule: Schedule,           // measurement times + contamination (§3)
    fields:   FieldSet,           // which bundle outputs to compute
    seed:     u64,                // RNG key root (noise/field realisation, §5)
}
```

Everything the forward model needs, in one value built **directly**: pick the sources, the array,
the optional ULDM line, the noise, the schedule, and what to record. `generate.run(scenario)` ->
`StateBundle`. A `Scenario` carries no live state — it is a *specification*; running it is the
mandatory path, and **constructing one by hand is the primary way to use the simulator**.

---

## 3. The `Schedule` — cadence, gaps, contamination (the detection front-end)

```rust
pub struct Schedule { times: Vec<f64>, mask: Vec<bool> }   // {t_l} and the contamination flag
```

The realised measurement times and a per-cycle **contamination mask** — a field of the `Scenario`.
The schedule **is the Gradar front-end** (detection-before-tracking, spec §schedule): a downstream
tracker must first decide *when* a signal is present — across gaps and corrupted cycles — before it
can track, and the schedule is what makes that problem realistic.

- **Uniform** (default) — fixed cadence `dt` (spec `tab:params`); deterministic, no sampling.
- **Realistic** — irregular spacing/gaps (the instrument is not always measuring) and **jitter**,
  set in the schedule spec directly (or drawn by a `Prior` if one is used, §6).
- **Contamination** — a fraction of cycles flagged (transient-corrupted); the `mask` rides into the
  `StateBundle` so a consumer sees which measurements to distrust.

Non-uniform sampling is also what makes the spectral analysis (Lomb-Scargle, spec `sec:freq`)
non-trivial — a uniform FFT would not do.

---

## 4. Composition — multi-target scenes and the body dictionary

Because gravity is linear, a scene is just **`Vec<Source>`** (`DESIGN.md` §2): put one source or
several into the `Scenario` and the forward model sums them. **Multi-source** composition is
`scenario`'s; **single-source** time-sequencing (a piecewise `Trajectory`) is `source`'s
(`source.md` §8). The **body dictionary** is the catalogue you draw bodies from (by hand, or via a
`Prior`) — the primitives and imported meshes (`source.md` §6) with their mass ranges; it is also
what the shape-as-model-selection story (spec §5.4) selects *over*, so the dictionary is shared
between generation and any eventual inverse model.

---

## 5. Determinism of a run

A `StateBundle` is a pure function of `(Scenario, backend)`: the `seed` keys the counter-based RNG
(`compute.md` §8) for the noise and atmospheric-field realisations, so the same `Scenario` reruns
identically across CPU/GPU and ordering. This is the property that matters, and it holds for
**hand-built** scenarios — no `Prior` involved.

---

## 6. The `Prior` — optional sugar for batched generation

When you want not one scenario but a *large varied batch*, writing each `Scenario` by hand does not
scale. A **`Prior`** is the convenience: a `Scenario` whose fields are **distributions** instead of
fixed values, plus a sampler.

```rust
pub struct Prior { /* a distribution per scenario field */ }
impl Prior { pub fn sample(&self, seed: u64) -> Scenario; }

let batch: Vec<Scenario> = (0..n).map(|i| prior.sample(root ^ i)).collect();   // run as usual
```

What it buys, and *only* this:
- **scale** — write the ranges once and draw a million scenarios, instead of a million configs;
- **coverage** — span a parameter space deliberately (mass/distance ranges, target-count mix,
  motion families, the degenerate-pair families a downstream tracker would need to resolve);
- **reproducibility from a seed** — `(Prior, seed)` reproduces an entire batch from ~10 lines, via
  the same key tree the realisations already use (§5);
- **a batch source** — the GPU path wants a `Vec<Scenario>`; a sampler is the natural supply.

**It is not the contract, and it is bypassable.** The irreducible operation is `Scenario ->
StateBundle`; a `Prior` only *produces* `Scenario`s, and you can always build them directly or write
the ten-line sampling loop yourself. It earns its place for the common batch case and the
reproducibility-from-a-seed property (genuinely fiddly to get right by hand) — nothing more. Built
from `config`'s distribution primitives; validated at construction so every `sample` is runnable.

---

## 7. The array as a knob

The array is a field of the `Scenario`, fixed for a run. Varying it across runs — by hand, or by
letting a `Prior` range the ground positions — is how you study geometry: the placement is the free
parameter the spec optimises against the Cramer-Rao floor (spec §array, §Gradar), baseline vs source
distance setting localisation precision. v1 arrays are horizontally separated vertical gradiometers
(`instrument.md` §8).

---

## 8. Connection to `generate`

`Scenario -> StateBundle` is the contract; `generate.run(scenario)` runs one, the batched form runs
many (handing `compute` the `EvalBatch`). A `Prior`, *if used*, just supplies the batch
(`{sample(seed ^ i)}`); `generate` neither knows nor cares whether a `Scenario` was hand-built or
sampled.

---

## 9. Errors & API

A `Scenario` is validated at construction (`Result<_, ScenarioError>`: an empty scene with no ULDM,
a `dt <= 0` schedule, a malformed array). A `Prior`, if built, is validated so `sample` is **total**
(no inverted ranges, non-empty body dictionary). Public surface: `Scenario` (+ its builder),
`Schedule`, `NoiseStack`, `FieldSet`, and the optional `Prior` (+ builder). `generate` consumes
`Scenario`s; `config` supplies the distribution primitives a `Prior` is built from.

---

## 10. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| **the contract** | a hand-built `Scenario` runs to a `StateBundle` | total |
| run determinism | same `(Scenario, backend)` -> identical bundle, across CPU/GPU and order | exact |
| schedule | uniform default exact; realistic gaps/jitter realisable; mask carried to the bundle | model |
| multi-target | a scene of `N` independent sources composes and runs | exact |
| `Prior` samples (if built) | every `sample` yields a runnable `Scenario`; `(Prior, seed)` reproduces a batch | total/exact |
| coverage (if built) | a `Prior` can span the identifiability-ladder cases | review |

The headline is the first row: **a hand-built `Scenario` runs to a bundle.** The `Prior` rows are
conditional — the sugar is correct when present, and simply absent when not.

---

## 11. Open sub-questions (resolve in implementation)

- **`Prior` specification surface (if kept).** A declarative config schema (read from a file) vs a
  builder API vs both — how a batch experiment is written down. Ties to `config`'s schema design.
- **Contamination model.** What fraction of cycles, and what transient process flags them — a
  realism parameter set against how the front-end is meant to be stressed.
- **Array variation.** When the array is swept, whether from a distribution or actively optimised
  against the CRB (the latter couples generation to the analysis in `state`).
