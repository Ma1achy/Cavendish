# Cavendish rebuild — the ML side and the Python API

> Companion to `plan/REBUILD.md`. Modelled on Four's ML stream (`Ma1achy/Four`, `docs/ML_STREAM.md`,
> `crates/four-ml`, `python/four`): a Rust core that simulates while you train, a strictly typed
> Python package over it, typed errors with stable codes, a manifest and a ledger for every run, and a
> read-through cache that is bit-identical to a fresh run. Built in slice S11, with the API shape fixed
> early (S2) so the GUI, CLI, MCP and Python agree from the start.

---

## 1. The idea: simulate while you train

```
scene specs + distributions (or recorded runs)
  → cav-engine: build each scene, run its stages, read every tower      (Rust, threads, GPU)
  → typed tables: shots, tracks, sources, events, fields                (columns with dtype, shape, unit, role)
  → cavendish (Python): filter, split, windows, batches [B, T, D, …] + mask + meta, manifest + ledger
```

No dataset has to be written first. A training run names a source (a scene family with parameter
distributions, or a directory of recorded runs), picks fields, and iterates tensor-ready arrays. Writing
a dataset to disk is the same pipeline with a sink, for when a fixed set is wanted (benchmarks, sharing).

## 2. Tables and fields

| Table | One row per | Holds |
|---|---|---|
| `shots` | tower × shot | readout time, launch time, total phase, mask (lost, excised, gated), the channel split (targets, ambient, ULDM, noise) |
| `tracks` | source × playback frame | pose, velocity, mass, bounding box, contact state |
| `sources` | source | kind, mass, density, shape, channel, population and event id |
| `events` | event | contact, sleep, lift transient, regrid, MOT failure, gap start/end — with time and source |
| `fields` | field frame (optional, large) | voxel or air snapshots at a chosen level of detail, for models that see the world |

Every field has a **role**, and batching respects it:

| Role | Meaning | May be a model input? |
|---|---|---|
| `observable` | what a real instrument records: phases, readout times, the shot mask | yes |
| `target` | ground truth to predict: positions, masses, counts, f₀ and orbit geometry of oscillators | only if asked for explicitly as a target |
| `nuisance` | the channel split and noise realisations: what a denoiser learns to remove | as a target for denoising, never as an input |
| `internal` | numerics, error estimates, debug | no |

Putting a `target` or `nuisance` field among the inputs is refused (`DatasetError.LeakingField`) unless
the call says why. That is the single most likely way to fool yourself with simulated data.

`cavendish.fields(table, names = …)` resolves names and returns `UnknownField(name, nearest, others)`
when a name is wrong. The field reference (dtype, shape, unit, role, description) is generated from the
Rust definitions, so the docs, the Python stubs and the engine cannot drift.

## 3. Using it

```python
import cavendish as cav
from cavendish import Err

src = cav.Source.scenes("scenes/room/people.ron",
                        vary      = {"population.walkers.count": cav.Uniform(1, 4),
                                "population.walkers.mass": cav.Normal(75, 12),
                                "array.preset": cav.Choice(["aion10", "aice100"])},
                        count = 200_000, root_seed = 7)
if isinstance(src, Err):
    raise SystemExit(f"cannot build the source: {src.err_value!r}")
split = cav.split(src.ok_value, by = "scene_seed", salt = "run-1", val = 0.05, test = 0.05)
batches = cav.batches(src.ok_value, table = "shots", window = 64, batch_size = 32,
                      inputs = ["phase", "readout_time", "mask"],
                      targets = ["tracks.position@shot"],      # aligned to each shot's readout time
                      indices = split.ok_value["train"], seed = 0, threads = 8)
for b in batches.ok_value:
    b["phase"]   # [B, T, D] float32   (T shots, D towers)
    b["mask"]    # [B, T, D] bool
    b["target"]  # [B, T, S, 3] with b["target_mask"] for absent sources
    b["meta"]    # per-window: scene hash, preset, seeds, numerics
cav.write_manifest("run-1.json", cav.manifest(src.ok_value, split.ok_value["train"],
                                              ledger = batches.ok_value.ledger))
```

- **Results, not exceptions**, exactly as Four: `cav.Result[T, cav.Error]` with `Ok`/`Err`, `match`-able
  frozen dataclasses for every error, each with a stable `code`, mirroring the Rust enums field for field.
  An `error_catalog()` from the extension and a test that fails when the Python mirror differs.
- **A scene that fails is not an `Err`**: it is counted in the ledger with a typed `detail` (which stage,
  which source, the engine's code) and the run continues.
- **Splits never leak:** they hash `salt:key` and move whole scene families, populations and shared
  seeds together; the manifest counts anything found on two sides (it must be 0).
- **Per-tower time:** with staggered firing, `shots` windows are aligned per tower and carry their own
  times; `cav.align(…)` resamples onto a common grid when a model wants one, and says it did.
- **Cache:** `cache = dir` reads through a `.cav` run cache keyed by spec hash and engine SHA; later passes
  skip simulation. Bit-identical to a fresh run, verified by `cav verify`.
- **Typing:** pyright strict; the extension built with maturin under a `python` Cargo profile that
  unwinds on panic (a Rust panic becomes a Python exception) and is not stripped (macOS rejects it).
- **Framework adapters** in examples, not in the core: a torch `IterableDataset`, a JAX iterator.

### 3.1 Style

All Python follows `.claude/skills/python-style/SKILL.md`, which is Four's style: pyright strict,
spaces around keyword `=`, aligned signature and field columns, section banners, Google docstrings with
typed `Args:`, `Result` values and typed error families instead of exceptions, a ledger for per-item
failures, double-underscore private methods. The example above is written in it.

## 4. The ML tasks Cavendish should serve, and how each is judged

| Task | Inputs | Target | Baselines it must beat |
|---|---|---|---|
| **tracking** (unknown number of bodies) | phases, times, mask | positions over time, count | the matched-filter and inversion trackers (M17); the Cramér–Rao bound as the floor |
| **mass and geometry** | as above | mass, size class | least squares on the response maps |
| **denoising** | phases | the `targets` channel (signal without ambient and noise) | notch + excision pipeline from the oscillatory-noise paper |
| **oscillator fingerprinting** | phases (any cadence) | f₀, harmonics, folded or not, orbit tilt | the comb-fold search of `VALIDATION-AND-RESEARCH.md` §3 |
| **transient detection** | phases | event intervals (lifts, people, cars) | the rolling-σ rule (mean + 5σ) from RATs to riches |
| **ULDM sensitivity** | long cadenced series | injected coupling | Lomb–Scargle d_min |

An `eval` module scores each task on fixed benchmark sets (written once, with manifests), reports per
preset and per scene family, and keeps a table in `docs/ml/benchmarks.md`. Each benchmark has a
**null set** too — scenes with nothing detectable (an airliner overhead) — so a model that invents
targets is caught.

## 5. Sim-to-real

Train for the instrument that will exist, not the simulator: randomise what is uncertain (instrument
parameters within tolerance, tower placement and tilt errors, noise spectra, shot loss, cadence
jitter, the air model's parameters), report the error ledger per batch, and keep the real-data path
open — the same `shots` table read from instrument files when AION data exists, so a model trained
here runs unchanged on it.

## 6. The interfaces agree

The Python API, the CLI (`cav batch`, `cav verify`, `cav fields`), the MCP tools (`dataset_build`,
`fields`, `signal`) and the GUI's dataset builder are the same engine calls. As in Four's collector MCP:
responses are size-bounded and paged, errors are the same typed codes, and every tool has a test.

## 7. Measured

A table like Four's, filled from runs on the M3 and the 2080: scenes per second by family, shots per
second into Python, cache hit speed-up, memory per worker. Targets to beat are set after S5.
