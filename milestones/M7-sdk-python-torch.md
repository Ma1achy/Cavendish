# M7 — SDK: Python / torch (implementation brief)

> The intended workflow: drive the engine from Python, receive torch tensors — zero-copy where the
> device allows, with the GIL released so generation overlaps training. Read with `design/sdk.md`
> and `design/state.md` §4 (the layout this milestone cashes in).
>
> **Prereq:** M6. **Delivers to:** the user-facing workflow; M8/M9 are independent.
> **Crates touched:** `sdk` (new); `python/` (package + pytest).

---

## 1. Requirements

| ID | Requirement |
|---|---|
| M7-R1 | `import cavendish`: constructors (`Scenario`, `Prior`, `FieldSet`, `Schedule`, primitives/mesh body specs) and the two verbs `run(scenario)` / `stream(prior|scenarios, n, seed)`. |
| M7-R2 | The bundle arrives as a typed Python object with named tensor attributes; **optional fields are `None`** when their `FieldSet` flag was unset. |
| M7-R3 | Torch hand-off via DLPack: **zero-copy** on CUDA and CPU; MPS copies honestly (no pretence). |
| M7-R4 | The Rust work runs with the **GIL released**; Python threads progress while a batch computes. |
| M7-R5 | `stream` is a memory-bounded Python iterator; a seeded stream replays identical tensors. |
| M7-R6 | Errors convert: `RunError`/validation → Python exceptions; no Rust panic crosses the boundary. |
| M7-R7 | Packaging: maturin; wheels for macOS-arm64 (MPS/CPU) and linux-x64 (+CUDA feature-gated). |

---

## 2. Design

### 2.1 The surface

```python
import cavendish as cv, torch
scn = cv.Scenario(sources=[cv.Source.primitive("cuboid", dims=…, mass=1000, path=cv.Path.linear(…))],
                  array=cv.Array.line(n=4, spacing=25.0),
                  fields=cv.FieldSet(decomposition=True))
b = cv.run(scn)                       # StateBundle
x = b.signal                          # torch.Tensor (T,D) — shared memory where possible
for b in cv.stream(prior, n=100_000, seed=7):
    loss = model(b.signal, b.detector_placement); …
```

### 2.2 Zero-copy and the GIL (the two things the crate exists for)

```
DLPack:  Rust buffer (contiguous, dtype fixed by state) ──capsule──► torch.from_dlpack ──► tensor
         CUDA-resident fields stay on-device; CPU shares host memory; MPS: explicit copy + note.

overlap timeline (stream):
   Rust (no GIL):   [ compute batch k+1 ······ ]        [ compute k+2 ····· ]
   Python (GIL):              [ train on batch k ]                [ train on k+1 ]
   PyO3: Python::allow_threads(|| backend.eval(…)) around every heavy call.
```

### 2.3 Device placement

| Backend produced on | handed to torch as | copy? |
|---|---|---|
| CUDA (RTX 2080) | CUDA tensor, same device | no |
| CPU (reference) | CPU tensor (shared buffer) | no |
| MPS (M3) | CPU→MPS or CPU tensor | yes (until torch DLPack-on-MPS matures) |

The bundle records the producing device; `sdk` never silently migrates data.

---

## 3. Pseudocode

```
#[pyfunction]
fn run(py: Python, scn: PyScenario) -> PyResult<PyBundle>:
    let bundle = py.allow_threads(|| generate::run(&scn.inner))
                   .map_err(to_py_err)?;                 # panic hook → PyRuntimeError
    PyBundle::wrap(bundle)                                # fields → DLPack capsules lazily

#[pyclass] PyBundle:
    #[getter] fn signal(&self, py) -> PyObject:           # torch.from_dlpack(capsule)
    #[getter] fn signal_targets(&self, py) -> Option<…>   # None when flag unset

fn stream(...) -> PyStreamIter:                            # __next__ releases the GIL for compute,
    prefetch depth 1 (one batch ahead)                     # yields wrapped bundles
```

---

## 4. Tests (pytest unless marked)

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `optional_none` | unset flags ⇒ attributes are `None`; set ⇒ tensors | exact |
| unit | `errors_convert` | invalid `Scenario` raises `ValueError`; induced backend failure raises `RuntimeError`; no abort | structural |
| unit (rust) | `no_panic_across` | a panicking mock backend surfaces as a Python exception | structural |
| integration | `shapes_dtypes` | every field: shape and dtype exactly per `tab:bundle` | exact |
| integration | `stream_iterates` | 100 bundles under a `DataLoader`-style loop; RSS bounded (≤ 2 batches) | structural |
| e2e | `fidelity` | Python tensors equal the Rust bundle value-for-value (CPU path) | exact |
| e2e | `zero_copy_cpu` | mutating the torch CPU tensor is visible through the Rust view (shared buffer probe) | structural |
| e2e | `zero_copy_cuda` (gpu-marked) | CUDA tensor device pointer equals the Rust allocation; no D2D copy in the profile | structural |
| e2e | `gil_released` | a Python heartbeat thread ticks ≥ N times during one large `run` | structural |
| e2e | `seeded_replay` | two `stream(prior, n, seed)` passes produce identical tensors | exact |

---

## 5. Exit requirements

| Requirement | Check | Tol |
|---|---|---|
| faithful marshalling | `fidelity`, `shapes_dtypes`, `optional_none` | exact |
| zero-copy where possible | `zero_copy_cpu`, `zero_copy_cuda` | structural |
| overlap | `gil_released`, `stream_iterates` | structural |
| reproducible | `seeded_replay` | exact |
| robust boundary | `errors_convert`, `no_panic_across` | structural |
| packaged | maturin wheels build in CI (macOS-arm64, linux-x64) | green |

## 6. Traceability

M7-R1 → shapes_dtypes, stream_iterates · M7-R2 → optional_none · M7-R3 → zero_copy_* · M7-R4 → gil_released · M7-R5 → stream_iterates, seeded_replay · M7-R6 → errors_convert, no_panic_across · M7-R7 → CI wheels job.
