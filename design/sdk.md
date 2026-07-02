# Cavendish — `sdk` drill-down

> Subsystem design for the `sdk` crate: the **Python/torch boundary**. A thin PyO3 layer that
> exposes `run`/`stream` and the `Scenario`/`Prior`/`FieldSet` constructors to Python, and marshals
> a `StateBundle` into torch tensors — ideally without copying. Companion to `DESIGN.md` (§6
> inventory) and `state.md` (§4 tensor layout, the payoff realised here).
>
> **Dependencies:** `sdk → generate, scenario, config, state`. It defines no physics and no
> orchestration; it is a marshalling layer over `generate`.

---

## 1. Responsibility & boundaries

**Owns:** the Python extension module (`import cavendish`) — the bindings for building a run, driving
it, and receiving the bundle as torch tensors; and the GIL discipline around the Rust work.

**Does not own:** any physics, the orchestration (`generate`), the output shape (`state`). `sdk`
translates between Rust and Python and nothing more — every heavy operation is a call downward.

**Invariant it guarantees:** the Python-visible result is faithful to the Rust `StateBundle` —
same fields, shapes, dtypes, and values (`state.md` §11), with `None` exactly where a `FieldSet`
flag was unset.

---

## 2. The surface exposed to Python

A small, Pythonic API mirroring the Rust entry points:

```python
import cavendish as cv

scenario = cv.Scenario(sources=[...], array=..., fields=cv.FieldSet(decomposition=True))
bundle   = cv.run(scenario)                      # one StateBundle
for bundle in cv.stream(prior, n=100_000):       # a reproducible batch, streamed
    ...
```

`Scenario`, `Prior`, `FieldSet`, `Schedule` are exposed as constructible objects (or built from a
config file, `config`); `run` and `stream` are the two verbs. The `Prior` (batch sugar,
`scenario.md` §6) is here so a Python training loop can sample scenarios without leaving the module.

---

## 3. The torch hand-off — zero-copy where possible

This is the crate's reason to exist, and it is where the **tensor-native layout** (`state.md` §4)
pays off. Each `StateBundle` field is already a contiguous array in the shape and dtype torch wants,
so `sdk` hands it over via **DLPack**: the Rust buffer is wrapped in a DLPack capsule and imported
with `torch.from_dlpack`, sharing memory rather than copying. The `(S,T,3)`, `(T,D)`, … arrays
become torch tensors of identical shape with no per-field reshape on the Python side.

Copying is the fallback, not the path — it happens only where a device/dtype cannot be shared (§7).
Getting this right is why `state` fixed the layout and dtypes up front: `sdk` is the beneficiary,
not the place where the layout is *fixed up*.

---

## 4. GIL release

The forward model and the GPU evaluation are pure Rust and touch no Python objects, so `sdk` runs
them **outside the GIL** (PyO3 `Python::allow_threads`). This matters most for `stream`: while Rust
computes the next batch on the GPU with the GIL released, the Python training loop can train on the
*previous* batch — compute and training overlap instead of serialising. Without this the engine
would stall every Python step; with it, generation hides behind the optimiser.

---

## 5. The streaming API in Python

`stream` surfaces as a Python iterator yielding one bundle at a time, memory-bounded end to end (it
inherits `generate`'s bounded stream, `generate.md` §6): Rust holds a batch, Python consumes it, and
neither materialises the dataset. This is meant to sit under a `DataLoader`/training loop — the
consumer decides batching-into-minibatches, shuffling, and what is input vs target (the engine
imposes none of that, spec §1). Reproducibility rides the seed, so a stream replays identically.

---

## 6. The bundle as a Python object

A `StateBundle` arrives as a typed object with named tensor attributes (`bundle.signal`,
`bundle.source_position`, `bundle.source_angular_velocity`, …) — or equivalently a dict of tensors.
**Optional fields are `None` when their `FieldSet` flag was unset** — `bundle.signal_targets is
None` unless `decomposition` was on, `bundle.field_grid is None` unless `field` was — so the
Python side reads the cost knob directly off the object. `meta` comes across as a plain Python
structure (resolved config + seed), keeping each bundle self-describing.

---

## 7. Device placement

Where the tensors land depends on the backend and the hardware:

- **CUDA (RTX 2080)** — fields the GPU backend computed on-device are handed to torch as CUDA
  tensors via DLPack, **zero-copy**, on the same device — the ideal path for the training loop.
- **CPU** — the reference backend's arrays share memory with torch CPU tensors (DLPack / buffer
  protocol), also copy-free.
- **MPS (M3)** — torch's DLPack support on MPS is limited, so this path may **copy** to hand tensors
  to torch; `sdk` does so honestly rather than pretending zero-copy. (An area to revisit as MPS
  interop matures, §10.)

`sdk` reports the device on the returned tensors; it does not silently move data between devices.

---

## 8. Errors

Rust `RunError`s (`generate.md` §10) surface as Python exceptions with the underlying message; a
`Scenario` that fails validation raises on construction. In a stream, a failing item raises at that
iteration (or is delivered per `generate`'s mid-stream policy, `generate.md` §12) without
invalidating earlier bundles. No Rust panic crosses the boundary as a panic — it is converted.

---

## 9. Exit requirements (definition of done)

| Requirement | Check | Tol |
|---|---|---|
| fidelity | Python tensors match the Rust bundle in shape/dtype/value | exact |
| zero-copy (CUDA/CPU) | a shared-memory tensor reflects the Rust buffer without a copy | structural |
| optional ⇒ `None` | an unset `FieldSet` flag yields `None` for those attributes | exact |
| GIL released | Python threads make progress while a batch computes | structural |
| streaming bounded | a long Python stream holds ≪ the whole dataset | structural |
| errors convert | Rust errors become Python exceptions; no panic crosses the boundary | structural |
| reproducible | a seeded stream replays identical tensors | exact |

`sdk` is verifiable from Python against a known `Scenario` without touching the physics — its job is
faithful marshalling, GIL discipline, and copy-free hand-off, nothing more.

---

## 10. Open sub-questions (resolve in implementation)

- **Bundle type.** A PyO3 class with tensor attributes vs a plain dict vs a NamedTuple/dataclass —
  and how `meta` is typed on the Python side.
- **MPS interop.** Whether MPS zero-copy is achievable now or the copy is unavoidable until torch's
  DLPack-on-MPS improves.
- **Packaging.** maturin build, wheels for macOS-arm64 (M3) and linux-x64+CUDA, and how the CUDA
  build is gated so the CPU/MPS wheel stays slim.
- **Prefetch depth.** Whether `stream` prefetches more than one batch ahead (deeper overlap) and how
  that trades against memory.
