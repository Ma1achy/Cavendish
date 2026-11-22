# S2 research: bit-identical replay, the scene spec as a tree of paths, and the cadence designer

*Research note for slice S2 (scenes as files, the API everywhere). GPU-free. Written 22 Nov 2026.
Questions: (1) what breaks `f64` bit-exactness through RON, JSON and `.npy`, and what else makes a replay
differ; (2) how to address any field of the scene spec by a path, once, for the GUI, CLI and MCP; (3) the
closed forms behind the cadence designer and what it can show without simulating.*

Measured in this container (Linux, glibc 2.39, 4 cores, rustc 1.100.0; `serde_json` 1.0.154, `ron` 0.12.3,
`rayon` 1.12.0, `libm` 0.2, numpy 2.5.3), with the repository at `e973936`. The throwaway programs are in
[`s2-replay-spec-paths-cadence/`](s2-replay-spec-paths-cadence/) (`float_lab`, `engine_lab`, `reflect_lab`,
`window_lab.py`), their output in `results/`. Nothing in `crates/` was changed. "Measured" below means one of
those files; everything else is linked to its source.

## 1. Short answer

1. **The file formats are fine; two switches and one rule keep them so.** With `serde_json`'s
   `float_roundtrip` (already on in this workspace) 0 of 10 million `f64` values changed through JSON; without
   it 6–30% did. RON 0.12.3 changed 0 of 10 million with no switch. `-0.0`, subnormals and infinities survive
   both; NaN does not survive JSON (written as `null`, then refused on reading) and RON keeps a NaN's sign but not its payload.
   The one rule: **a bundle must never contain a NaN**, since a NaN's bits are not deterministic (§2.4).
2. **`.npy`: the in-repo reader is right; extend the writer to n dimensions.** A 30-line writer matched
   `numpy.save` byte for byte on 0-, 1- and 2-D arrays and the empty array, and numpy read every bit back (NaN payloads and
   `-0.0` included). No crate is needed.
3. **Bit-identical replay holds for the CPU path on one platform, and the S2 exit test should say so.**
   The determinism hazards that are real: **`rayon` reductions** (`par_iter().sum()` gave 16–18 different
   results in 20 runs at 3 or more threads), **HashMap iteration order** (the sum differed in 1 of 4 runs), **the platform's `libm`**
   (Rust documents `sin`, `cos`, `powf`… as non-deterministic across platforms; 3–13% of results differed
   from a pure-Rust libm by one ulp), **non-keyed random numbers**, and **GPU `fma`/flush-to-zero**. Fused
   multiply-add is *not* a hazard on the CPU: Rust never contracts `a*b+c`, and `mul_add` gave the same bits in
   software and in hardware.
4. **Paths: keep what S0 built (a `serde_json::Value` walk) and finish it, rather than add a reflection crate.**
   Move the walker into `cav-sim`, drive `ls`, `schema <path>` and the units, ranges and invalidation stage from
   the types' own `schemars` attributes (the crate is already there), and make undo an inverse command for `Set`
   (snapshots only for `Load`). A reflection crate would add 4–11 s to a cold build (and up to 37 crates) for nothing the schema does not already give (§3).
5. **Cadence designer: five closed forms are enough to draw the window live, and they match the numbers
   to 4 decimals (§4).** What *cannot* be assumed is which jitter the paper's scripts mean: they draw **cycle
   times** (shot times random-walk, the alias line broadens and its height falls as ~1/√N), not shot times about
   a grid (a Debye–Waller factor that does not fall with N). The designer must offer both and say which; the S2 brief should ask George which one the hardware does.

## 2. Replay and round trips

### 2.1 JSON

`serde_json` writes an `f64` with `zmij` ([`ser.rs`][sj-ser]; the shortest text that reads back, the property Rust's own `{:?}` has: [core fmt][core-float]), and
reads them in one of two ways. With `float_roundtrip` it calls a `lexical` parser; without it, it forms
`significand as f64` and multiplies or divides by a power of ten in `f64`, which rounds twice ([`de.rs`][sj-de]).
Rust's `str::parse::<f64>` agrees with CPython's correctly rounded `float()` on 1 000 000 strings
([`results/std_parse.txt`](s2-replay-spec-paths-cadence/results/std_parse.txt)), so I used it as the reference.

| 2 million values of | differ without `float_roundtrip` | with it |
|---|---|---|
| random bit patterns (every exponent) | 593 505 (29.7%) | 0 |
| uniform in [0, 1) | 209 606 (10.5%) | 0 |
| what a person types (6 digits, e−30…e+30) | 168 945 (8.4%) | 0 |
| scene scales (1e−27…1e6, either sign) | 446 716 (22.3%) | 0 |
| subnormals | 115 720 (5.8%) | 0 |
| 17-digit text from another tool, against `str::parse` | 396 419 (19.8%; the first found was one ulp out) | 0 |

The same counts hold through `serde_json::Value`, which is what `Command::Set` carries
([`json_nofr.txt`](s2-replay-spec-paths-cadence/results/json_nofr.txt), [`json_fr.txt`](s2-replay-spec-paths-cadence/results/json_fr.txt)).
Other facts, each measured there:

- `-0.0`, `5e-324` and `f64::MAX` round-trip. NaN and ±∞ are written as `null` ([`ser.rs`][sj-ser]) and reading
  `null` into an `f64` is an error, so a journal holding a NaN could be written but not read back. The engine
  already refuses a NaN `Seek`, `Zoom` and `Set` before it reaches the journal ([`set_cost.txt`](s2-replay-spec-paths-cadence/results/set_cost.txt)).
- Text that overflows (`1e999`) is an error on reading, not ∞ (`set_cost.txt`).
- An integer sent for a float field is accepted (`1` → `1.0`); a `u64` above 2⁵³ sent to an `f64` field is
  rounded (2⁵³+1 → 2⁵³). A `u64` field (the M15 plan's `seed`) keeps it exactly in a `Value`, but a client that
  holds JSON numbers in a double, as JavaScript does, cannot send one above 2⁵³ intact (the ECMAScript number type is an IEEE double; not re-checked here).
- `serde_json::Value` objects are sorted maps in this build (`preserve_order` is not enabled anywhere in the
  workspace: `cargo tree -e features -i serde_json`). If a future dependency turns it on, the order of
  `ls` output changes; `Engine::digest` and the contribution keys serialise typed structs, so they would not.

**Guard.** Feature unification makes `float_roundtrip` global to a build, but it is one dependency edit from
being lost. Add `json_round_trip_is_exact` (10⁶ random finite bit patterns, struct and `Value` routes) with a
control arm that sends the same values through `format!("{x:.15e}")` and must find differences, so the test is seen able to fail.

### 2.2 RON

RON 0.12.3 writes an `f64` with `{:?}` and reads it with `f64::from_str` ([`ser/mod.rs`][ron-ser], [`parse.rs`][ron-parse]); it
therefore inherits the shortest-round-trip and correct-rounding properties. Measured on the same ten million
values: **0 differ** ([`ron.txt`](s2-replay-spec-paths-cadence/results/ron.txt)). Details that matter for files people edit:

| Input | Result |
|---|---|
| `5`, `5.`, `.5`, `1e3`, `1E3`, `1_000.0`, `+1.5` | accepted as `f64` |
| `1e400`, `-1e400`, `1e-400` | silently `inf`, `-inf`, `0` (JSON refuses) |
| `inf`, `NaN`, `-NaN` | accepted; `-NaN` keeps its sign |
| a NaN with a payload, or a signalling NaN | comes back as the default quiet NaN, bits changed |
| `-0.0` | kept, and `{:?}` writes `-0.0` |
| `0x10` or `0.1f32` for an `f64` | error |
| text → `SceneSpec` → text | identical (`room`; [`ron_edges.txt`](s2-replay-spec-paths-cadence/results/ron_edges.txt)) |

`SceneSpec::validate` refuses `1e400`, `inf`, `NaN` and `-0.0` in `duration` with the reason (same file), so a file cannot smuggle a non-finite
number in through RON. Two consequences worth a line each in the slice notes: `-0.0` and `0.0` are different
bits and so different digests even though most of the physics cannot tell them apart; and `ron::to_string` output
is stable, so a saved scene can be diffed.

### 2.3 `.npy`

The format ([NEP 1][npy-nep], [numpy's writer][npy-impl]): magic, version, a header length, an ASCII dict
`{'descr': '<f8', 'fortran_order': False, 'shape': (5, 7), }` padded with spaces and a final newline so that
the data starts on a 64-byte boundary (`ARRAY_ALIGN = 64`), then raw little-endian bytes. Version 1.0 has a
`u16` header length (header ≤ 65 535 bytes), 2.0 a `u32`, 3.0 UTF-8 field names.

I wrote a generic encoder (`npy_enc` in [`float_lab/src/main.rs`](s2-replay-spec-paths-cadence/float_lab/src/main.rs)) and compared with numpy
2.5.3 ([`npy.txt`](s2-replay-spec-paths-cadence/results/npy.txt)): shapes `(5,7)`, `(35,)`, `()` and `(0,3)` came back with every bit equal (a NaN with payload
`7ff8deadbeef0001`, a negative signalling NaN, `-0.0`, `5e-324`, ∞ included) and **byte-identical to `numpy.save`**. `np.save` writes version 1.0 with a 118-byte header for every shape tried, and the data starts at byte 128; it writes `'>f8'` and `'<f4'` for those dtypes and `fortran_order: True`
for a Fortran array, all of which `cav_validate::npy::parse` already refuses by name.

- **Endianness.** Write `'<f8'` always, on every platform, with `to_le_bytes`; refuse `'>f8'` on reading (done). No `bytemuck` cast of the slice, which would be big-endian on a big-endian host.
- **Crates.** [`npyz` 0.9.1][npyz] and [`ndarray-npy` 0.11.0][ndarray-npy] exist; `ndarray-npy` needs `ndarray`. Neither earns its place for
  one dtype: extend `cav-validate/src/npy.rs` (1-D write today) to n dimensions and move it where the
  bundle writer will live. It is about 30 lines; the reader already handles n-D.
- **Test.** Commit one file numpy wrote (the existing test builds its header by hand; a real `np.save` file
  of a (5, 7) array with `-0.0`, a NaN payload and a subnormal is better) and assert the Rust writer reproduces its bytes; control arm:
  flip one header byte and one data byte and see each caught.

### 2.4 What else makes a replay differ

| Hazard | Measured | In Cavendish today | Rule |
|---|---|---|---|
| **`rayon` reduction order.** The docs say the order of a parallel `sum` "is not specified, so … the results are not fully deterministic" ([rayon][rayon-sum]) | 10⁷ terms: `par_iter().sum()` gave 1, 2, 16, 18, 18 distinct results in 20 runs at 1, 2, 3, 4, 8 threads. Sums over fixed 4096-term chunks, combined in order, gave one result at every thread count (56 ulp from the sequential sum); so did a fixed pairwise tree ([`rayon.txt`](s2-replay-spec-paths-cadence/results/rayon.txt)) | no `rayon` yet; REBUILD §3.4 plans it for voxelising and for fixed-order float-float sums | never `par_iter().sum()` or `reduce` on `f64` where the answer reaches a bundle. Fixed chunk size (a constant in the numerics block), per-chunk partials in a `Vec`, combined by a fixed tree |
| **HashMap iteration order.** `RandomState` is randomly seeded ([std][std-hashmap]) | iterating one map and summing its values gave a different sum in 1 of 4 runs ([`hashmap_order.txt`](s2-replay-spec-paths-cadence/results/hashmap_order.txt)) | `Forward::kept` is only looked up, `retain`ed and summed in `wanted` order ([`forward.rs`](../../crates/cav-engine/src/forward.rs)): safe | no iteration of a `HashMap`/`HashSet` into a sum or an output. `BTreeMap` or a `Vec`; consider clippy's `disallowed_types` to enforce it in the crates that feed bundles |
| **Fused multiply-add** | `a*b+c` and `mul_add` differ in 199 590 of 10⁶ draws; but a million `a*b+c` hashed to the same value built with default flags, with `-C target-cpu=native` (this CPU has `fma`) and with `-C target-feature=+fma`, so the compiler did not fuse them; `mul_add` hashed the same in all three builds too, i.e. software and hardware agree ([`fma.txt`](s2-replay-spec-paths-cadence/results/fma.txt)) | none; the code never calls `mul_add` | Rust has no fast-math, and `mul_add` is correctly rounded ([std docs][std-f64]; [RFC 3514][rfc3514]). Not a CPU hazard. On the **GPU** it is: WGSL lets `fma(x,y,z)` be a multiply then an add ([spec source][wgsl-fp]), and allows flushing subnormals to zero |
| **`libm`.** `sin`, `cos`, `sin_cos`, `tan`, `exp`, `ln`, `powf`, `powi`, `atan2` and `hypot` each carry an "Unspecified precision" section: "non-deterministic … varies by platform, Rust version, and can even differ within the same execution" ([std][std-f64-sin]; `sqrt` and the four operations have none). glibc "does not aim for correctly rounded results" ([manual][glibc]) | std (glibc 2.39) against the pure-Rust `libm` crate, 2×10⁶ arguments: `sin` 3.1%, `cos` 3.1%, `tan` 3.6%, `exp` 9.8%, `ln` 4.7%, `powf` 9.8%, `atan2` 13.1% differ by an ulp. Against a 200-bit `mpmath` reference on 20 000 `sin` arguments, glibc was not correctly rounded in 35 (0.18%), the `libm` crate in 615 (3.1%). `powi(3)` and `powf(3.0)` differ in 26%. `sin_cos` agreed with `sin` and `cos` ([`libm.txt`](s2-replay-spec-paths-cadence/results/libm.txt)) | `cav-core` and `cav-sim` use `sin`, `cos`, `sin_cos`, `tan`, `ln`, `atan2`, `powi`, `hypot` and `sqrt` | the contract is **same binary, same platform**. Golden bundle hashes are per platform. Using the `libm` crate for the forward model would make all three platforms agree, at 12–17 ns against 12–14 ns per `sin`+`cos` pair here (about 20% slower) ([`libm_speed.txt`](s2-replay-spec-paths-cadence/results/libm_speed.txt)); it would change every golden once and cannot be made correctly rounded. A call for the owner, not S2 (§5) |
| **Random numbers** | `rand::StdRng` "is non-portable: any future library version may replace the algorithm" ([rand][rand-std]) | the plan's `jitter_is_keyed_not_ordered` already says so | a counter-based generator: a 64-bit mix (SplitMix64 or Philox) of `(seed, stream, tower, shot)`, in-house; no stream state, so batch order and thread count cannot matter |
| **NaN** | Rust: NaN sign and payload are not deterministic, even run to run ([RFC 3514][rfc3514]); `.npy` preserves them; JSON and RON do not (§2.1, §2.2) | `validate` refuses non-finite inputs | `Export` refuses to write a bundle containing a NaN or ∞, saying where. Then a hash of the bytes cannot flake on a payload |
| **Timing** | `Query::Signal` says `refined` or `stale` depending on when the worker finishes | `Engine::digest` excludes the signal; replay compares state, not signal | `Export` and `cav run` wait for `refined` ([`settle`](../../crates/cav-engine/src/engine.rs)) and read only that; a bundle is never built from a `stale` signal |
| **Wall clock** | `LogEntry.at` is `Instant`-based | not in `digest` | keep out of bundles and manifests |

**What the S2 replay tests should assert.** (a) the existing `session_replays_from_its_journal`, widened to
`Set`, `Undo`, `Redo`, `Run` and `Export` (the digest then covers the new `Export` commands' effects through the files' hashes); (b) `cav run scene.ron --out a/` twice and
at a different thread count (if any `rayon` appears) gives byte-identical `.npy` files; (c) the same session replayed from its journal *as JSON text*
(done for S0) and *as written to disk by `cav`*; (d) a control arm for each: drop one command, change one bit of
one number, or one chunk size, and the digest or the bytes must differ. State the platform in the test's doc comment:
the claim is "same platform".

The digest is 64-bit FNV-1a over the state's JSON text ([`engine.rs`](../../crates/cav-engine/src/engine.rs)): fine to compare states in a test, not a basis for
the M16 manifest, which will want a cryptographic hash. Not S2's decision.

## 3. The spec as an editable tree

### 3.1 What exists

S0 built most of it. `Command::Set { path, value }` and `Query::Spec(path)` take paths such as
`array.towers[2].position[0]`; the engine turns the spec into a `serde_json::Value`, walks it ([`at_path`](../../crates/cav-engine/src/engine.rs)),
writes, converts back with `from_value` and runs `validate`. `Query::Schema` returns the whole JSON Schema from
`schemars`, with doc strings, ranges (`#[schemars(range(..), extend("exclusiveMinimum" = 0.0))]`) and
`deny_unknown_fields`. Undo is a stack of whole `SceneSpec` clones; a run of `Set`s of one path (a drag) is one entry. The console has
`get` and `set`; MCP has `scene_get` and `command`. Missing against REBUILD §3.2 and M15 R7/R8: `ls`, a schema *at a path*, insert and remove for lists
(`towers`, `bodies`), creating an `Option` (`room: null` → a room), a unit and an invalidation stage per path,
and a walker the CLI can use without an engine.

### 3.2 Costs of the present approach, measured

`Engine::apply(Set)` on a scene with *n* point bodies, release build ([`set_cost.txt`](s2-replay-spec-paths-cadence/results/set_cost.txt)):

| bodies | JSON | `to_value`+`from_value` | `validate` | `clone` | `apply(Set)` |
|---|---|---|---|---|---|
| 1 | 765 B | 4 µs | 0.6 µs | 0.1 µs | 43 µs |
| 100 | 12.8 kB | 109 µs | 27 µs | 5.5 µs | 618 µs |
| 1000 | 124 kB | 1.7 ms | 1.1 ms | 52 µs | **8.1 ms** |

Linear in the scene, and `check` and `perform` each call `edited`, so the round trip and `validate` are paid twice:
by arithmetic about 5.6 of the 8.1 ms (I did not profile the rest, which includes scheduling the forward worker). At 1000 bodies that is half a 60 Hz frame spent
on one drag step. It is not a problem today (a drag is one body), and S3's voxel bodies will make a scene's
serialised size far larger, so it is a problem the slice should *measure* rather than guess. The cure is cheap if needed: call `edited` once
and keep its result, then, only if still over budget, round-trip the smallest enclosing list element (`bodies[500]`, 125 bytes, ~4 µs as the 1-body row shows) and splice it into the typed `Vec`.
`clone` is not the cost: 52 µs for 1000 bodies.

### 3.3 Options

Compile cost: a synthetic spec of 60 structs and 60 enums (8 fields each, `Vec`, `Option`, `[f64; 3]`), dev profile with the workspace's settings, 4 cores
([`reflect_compile.txt`](s2-replay-spec-paths-cadence/results/reflect_compile.txt)):

| Approach | Adds to a cold build | Adds per rebuild of the 120 types | Crates added | Gives |
|---|---|---|---|---|
| **A. `serde` + `Value` walk + `schemars`** (today) | schemars: +1.7 s | +0.3 s | 12, already in tree | get/set/ls by path; schema with docs, ranges, any `x-` keyword |
| B. hand-written visitor (M15's plan) | none | none | 0 | whatever is written; ~56 fields in 14 types now (the plan said ~30), each new slice adds more |
| C. [`bevy_reflect`][bevy-reflect] 0.20.1 | **+11.3 s** | +1.3 s | 37 | typed `GetPath` with `.field` and `[i]` syntax ([source][bevy-path]); no units, ranges or schema |
| D. [`facet`][facet] 0.46.5 | +4.0 s | +1.1 s | 14 | typed shapes; pre-1.0; enums need `#[repr(u8)]` (a derive error, seen) |
| E. JSON Pointer / Patch: `Value::pointer_mut` ([RFC 6901][rfc6901]), [`json-patch`][json-patch] 5.0.0 ([RFC 6902][rfc6902]) | 0 / small | 0 | 0 / 1 | a standard path syntax and edit vocabulary (add, remove, replace, test); no schema |

Failure modes:

- **A** drifts if a field's metadata is forgotten (a unit, a stage); numbers pass through `Value`, which is exact (§2.1) but loses the type
  (an integer for a float field is accepted); a field renamed in Rust breaks saved journals and paths in scripts (guard: `SPEC_VERSION` and a migration, plus the `deny_unknown_fields` error).
- **B** drifts silently, and the guard against drift (a test comparing its paths with the `Value`'s leaves) makes A the reference anyway.
- **C** needs `Reflect` derived on every type next to `Serialize`, `Deserialize` and `JsonSchema` (rmcp needs `schemars` regardless: S0 note), and a type registry for anything beyond get/set.
  Units and ranges would need custom reflect attributes, then a second route to the schema. Cost with no gain over A.
- **D** may be the right idea later; today `facet` is 0.46.5 and its companion `facet-reflect`'s newest release is `0.50.0-rc.7`, so expect churn.
- **E** is a vocabulary to borrow, not a dependency to add.

### 3.4 Recommendation

**Keep A, and finish it in one module.** Everything below fits the existing `Command`/`Query` types.

1. **One walker in `cav-sim`** (`paths.rs`), used by the engine, `cav get|set|ls` (no engine needed for a file) and MCP. Parse a path once into `Vec<Seg>`
   (`Field(String) | Index(usize)`), with the current `a.b[2].c` syntax and its good error messages ("`bodies` has 3 items; there is no [5]"). Functions: `get`, `set`, `insert(list, index, value)`,
   `remove(list, index)`, `ls`, `schema_at`. Every edit is `serde_json::Value` in, `Value` out, then `from_value` and `validate`, as now.
2. **`ls` and `schema <path>` read the schema.** Resolve `$ref`, `anyOf` (for `Option`) and `oneOf` (externally tagged enums) on the way down, and list per child: name, kind
   (number, vector, text, flag, list, choice, optional), the doc string, range and unit, the current value.
   The GUI panels, `cav ls` and MCP `scene_ls` are three views of the same `Vec<Entry>`.
3. **Units and invalidation stages are `schemars` attributes on the fields**: `#[schemars(extend("x-unit" = "m", "x-stage" = "Forward"))]`, the mechanism the spec already uses for `exclusiveMinimum`.
   A path's stage is the nearest ancestor's tag; untagged means "re-run everything", which is safe. `SceneSpec::diff(a, b)` (M15 R8) is then: walk both `Value`s,
   collect the changed paths, take the earliest stage, with no hand-written table to forget. Test `every_leaf_has_a_unit_or_is_dimensionless_and_a_stage`
   over the room and a scene with every variant; control arm: remove one tag and see it named.
4. **Undo as inverse commands, snapshots only for `Load`.** A `Set` records `(path, old value)` (a few bytes), the inverse is `Set { path, old }`; `Insert` and `Remove`
   are each other's inverse; a drag keeps the first old value (the coalescing rule already exists). `Load` keeps an `Arc<SceneSpec>` of the scene it replaced. Reason: snapshots are cheap now (52 µs per 1000 bodies) but
   S3's voxel bodies make each one megabytes, and the stack is unbounded. Test: a random sequence of edits, undone to the start, equals the start
   to the digest; redone, equals the end; control arm: corrupt one stored inverse. Replay needs no change, since `Undo` is already a journalled command.
5. **Seen failure modes to design for.** `room: null` is a path that exists but cannot be descended: `set room.size` must say "the room is absent; `set room {…}` adds one", not "no field". An enum is addressed
   *through its variant*, `bodies[0].motion.Static.position`; switching the variant is a `Set` on `bodies[0].motion` with the whole `{"Pass": {…}}`, after which the old paths vanish, so panels must re-resolve and `ls` must show the active variant. Indices are
   positional, so an agent that removes `bodies[2]` and reuses `bodies[3]` is wrong; tell it so in the `scene_set` description, and consider addressing by unique label (`bodies[label=tank]`) as an idea (IDEAS.md), not S2.

## 4. The cadence designer

### 4.1 What the scripts do

The scripts draw **cycle times** and sum them: `T_cycles = rng.uniform(…)`, then `np.concatenate([[0], np.cumsum(T_cycles[:-1])]) + T_fly`
([`cycle_jitter_sweep` L76–79][s-jitter], [`cadence_alias_ratio` L134–137][s-alias]; symmetric in the first, `U(T_nom, T_nom+δ)` in the second; the symmetric draw is also in [`cadence_bragg_comparison` L113–114][s-bragg]).
Shot times are therefore a **random walk**. The alternating script uses a deterministic pattern with period `T_super = T_A + T_B` ([`alternating_cadence_aice` L6–8][s-alt]); its docstring says
5.5 s and 6.0 s (the plan's row) but the line that runs says `T_A, T_B = 5, 5.5` ([L28][s-alt28]): the plan's test values and the script's defaults differ, so pin the test's own.
The lost-shot script drops each cycle independently ([`lost_shots_aice` L1–12][s-lost]). All use Lomb–Scargle ([Scargle 1982][scargle], [VanderPlas 2018][vanderplas]) on the surviving times.

Everything below is for a unit tone sampled at times `tₙ`. Its classical periodogram is `|W(ν − f)|²` with the **spectral window**
`W(ν) = (1/N) Σₙ e^{−2πiνtₙ}` ([Deeming 1975][deeming]; for additive random sampling [Shapiro & Silverman 1960][shapiro]).
Lomb–Scargle of the same tone matched the window to within 5×10⁻⁴ (absolute, on a ratio near 0.5) in every case I tried
(max in the same band; last block of [`window_lab.txt`](s2-replay-spec-paths-cadence/results/window_lab.txt)), so the designer can use `|W|` and the tests can use Lomb–Scargle.
I derived the closed forms myself; they are the standard characteristic-function argument, not a transcription of the paper's Eq. (24), which I could not read (§6).

### 4.2 Closed forms, each checked by Monte Carlo ([`window_lab.py`](s2-replay-spec-paths-cadence/window_lab.py))

| Cadence | Spectral window | Check |
|---|---|---|
| **Uniform**, period `T₀` | Dirichlet: `\|W\| = \|sin(Nπν T₀) / (N sin(πν T₀))\|`; lines of height **1** at every `k/T₀` (the alias is as tall as the peak), width `1/(N T₀)` | numerical = formula to 1×10⁻¹⁴ |
| **Cycle jitter**, iid cycle times with `φ(ν) = E e^{−2πiνT}` (random walk) | `E\|W\|² = (1/N²)[N + 2 Re Σₖ₌₁ᴺ⁻¹ (N−k) φᵏ]`, closed form `= (1/N²)[N + 2 Re( φ(N(1−φ) − (1−φᴺ)) / (1−φ)² )]`. Positive `U(T₀, T₀+δ)`: `φ = e^{−2πiν(T₀+δ/2)} sinc(νδ)`. Symmetric `U(T₀−δ, T₀+δ)`: `φ = e^{−2πiνT₀} sinc(2νδ)` (`sinc x = sin πx / πx`). At the mean-rate alias `φ` is real and `1−φ ≈ (π²/6)(δ/T̄)²`, so `E\|W\|² ≈ 2 / (N(1−φ))`: a line that **broadens** and whose height falls as **1/√N** in amplitude. Off the lines the floor is `(1−\|φ\|²) / (N \|1−φ\|²)` in power relative to the peak | 4 000 seeds, N = 500: measured/formula 0.98–1.03, within about 2σ, for both kinds, δ = 0.1 and 0.3 |
| **Timing jitter about a grid**, `tₙ = nT₀ + εₙ`, iid, characteristic function `ψ` | `E\|W\|² = \|ψ\|² \|D_N\|² + (1−\|ψ\|²)/N` (`D_N` the Dirichlet kernel): the lines keep height `\|ψ(k/T₀)\|` **for any N** (Debye–Waller); only a floor `(1−\|ψ\|²)/N` falls | 2 000 seeds: 0.995–1.015; `\|ψ(1/T₀)\|` = 0.9765 for ε ~ U(−0.3, 0.3) |
| **Alternating** `T_A, T_B`, `T_s = T_A+T_B` | two samples per period: sidebands at `k/T_s` of relative height `\|cos(π k T_A / T_s)\|`; for `T_A ≈ T_B` the odd `k` are small and the even ones near 1; independent of N | 5.5/6.0: 0.0682, 0.9907, 0.2035, 0.9629 for k=1–4, equal at N = 2×10³ and 2×10⁴ to 4 decimals; also 5/5.5 and 5/8 |
| **Planned gaps**, `L` shots on of every `P` | lines at `k/(P T₀)` of relative height `\|sin(πkL/P) / (L sin(πk/P))\|` → `\|sinc(kL/P)\|` as `P` grows; independent of N | k = 1–4 for L/P = 0.4: 0.757, 0.234, 0.156, 0.190: exact to 4 decimals |
| **Lost shots**, independent, probability `p`, on any deterministic base `W₀` | `E\|W\|² = \|W₀\|² + p/((1−p)N)`: a flat floor, normalised by the kept count, **falls only as 1/N** (power) | p = 0.01, 0.05 at N = 4 000: 2.68×10⁻⁶ and 1.34×10⁻⁵ against 2.53×10⁻⁶ and 1.32×10⁻⁵ (6% and 1.4% high; 300 seeds, so I cannot say whether the 6% is noise) |

Consequences for the paper-derived tests of VALIDATION-AND-RESEARCH §2.4:

- `alias_ratio_vs_n`: compare to `√E|W|²` at `ν = 1/T̄`, not to a fitted slope: positive jitter δ = 0.1, 0.2, 0.3 gives 0.660, 0.382, 0.263 at N = 5×10³ and 0.175, 0.089, 0.060 at 10⁵. The local slope is −0.44 for δ = 0.1 and −0.49 for δ = 0.3 over that range
  (approaching −½ as `N(1−φ)` grows), which is why the script fits `a + b/√N`. The script takes the *maximum* of `√S` in a window; the maximum of a noisy line sits above its mean, so for a seed-mean comparison use the maximum in the window for both prediction and measurement, or the pointwise mean.
- `random_jitter_floor_recedes`: the floor is `(1−|φ|²)/(N|1−φ|²)`, so it falls as 1/N in power, 1/√N in amplitude.
- `alternating_cadence_sidebands_persist`: heights are `|cos(πkT_A/T_s)|` and equal at N and 10N to ~1×10⁻³; control arm: the same assertion on a jittered cadence must fail.
- `lost_shots_floor_and_dmin`: the floor above is derived, and `d_min ∝ √(p/(1−p))` follows by linearity if the floor is leakage from the oscillator's own lines (as the script defines it). **I did not simulate `d_min`**; the test should compare with a
  Monte Carlo before the formula is trusted.
- `zero_filled_fft_fails_ls_does_not`: the script zero-fills lost shots for the FFT and gives LS only the survivors ([`gapped_fft_vs_ls` L1–8][s-gap]). For a zero-mean pure tone both see the same window (LS matched `|W|` above), so what the case shows depends on the mean and the noise; I did not analyse it, and the test should say which failure it asserts.

### 4.3 Live in the GUI from the schedule alone, and what must be simulated

Cost reference: the engine's exact Lomb–Scargle takes 324 ms for N = 10⁴ and 1000 frequencies and **2.6 s for N = 10⁵**, one thread
([`ls_cost.txt`](s2-replay-spec-paths-cadence/results/ls_cost.txt)): an exact periodogram over a real campaign (10⁵–10⁷ shots) is not live.

| Shown | Cost | Needs a simulation? |
|---|---|---|
| shot lanes per tower, mean rate `1/T̄`, Nyquist `1/(2T̄)`, the alias and folded-frequency map (`fold_frequency` exists) | instant | no |
| the **expected** window `E|W|²` for uniform, cycle-jitter (both kinds), alternating, gapped, and lost shots, composed: independent loss multiplies the line by `(1−p)` of power and adds `p/((1−p)N)`; independent time jitter multiplies by `|ψ|²` and adds `(1−|ψ|²)/N` | O(frequencies): microseconds to milliseconds | no |
| predicted alias ratio against N, line heights, floors, number of kept shots, span | O(1) per point | no |
| one **realisation** of the window for a seed | exact: O(N·M), 1–3 s at 10⁵; preview with a shortened span (N ≈ 2 000) in tens of ms, labelled `preview`, then refine; NUFFT for long spans (the S1 note: f64 on the CPU) | no |
| the periodogram of the **actual** signal under any cadence | one simulated period per oscillator (below), then one table lookup per shot | once |
| noise floors and `d_min(p)` | closed-form scaling needs the noise level and a baseline; the *scatter* over seeds needs Monte Carlo | the baseline and the Monte Carlo |
| non-periodic sources (a lift, walkers, rigid dynamics, air) | the signal is a function of readout time but must be computed along the span | yes, per source |

The key fact for the "no simulation" row: the engine's read for tower *d* at readout time *t* is a function of *t* only (the shot is launched at `t − 2T` and
integrated over its flight: [`Reader::read`](../../crates/cav-core/src/forward.rs)), and `cav_sim::read::signal_at(spec, times)` already takes arbitrary times. So a **periodic source's phase is a periodic function of
readout time with the source's period**. Simulate one period of each oscillator once (its per-source contribution is already cached, REBUILD §5.1), tabulate or Fourier-fit it, and any cadence's periodogram is
`|Σₙ φ(tₙ) e^{…}|²` without touching the forward model again. Only the interpolation error has to be bounded against the phase tolerance (record it; not measured here).
The designer can therefore be live for the physics of periodic sources and for the window of any schedule, and must simulate for anything not periodic.

A caution for the staggered clocks of M15: every tower has its own `tₙ`, so each tower has its own window; the common-mode cancellation between towers
is only as good as the two windows' overlap. The designer should show that per pair.

## 5. Recommendations for the S2 builder

1. Keep `serde_json` with `float_roundtrip` and add `json_round_trip_is_exact` with a control arm (§2.1). Keep RON for files; add the `1e400`, `NaN`, `-0.0` file cases of §2.2 (they pass today).
2. Generalise `cav-validate`'s `.npy` writer to n dimensions, commit a numpy-written fixture and assert byte equality (§2.3). No crate.
3. State the replay contract as **same binary, same platform, CPU path**, in `docs/slices/S2.md` and the test docs. Golden bundle hashes per platform.
4. Make `Export` refuse NaN/∞ and wait for a `refined` signal. Bundles are never built from `stale`.
5. Adopt the three rules now, before the code exists: fixed-chunk fixed-tree `f64` reductions (`rayon` or not); no `HashMap` iteration into a number or a file; randomness as a keyed hash of `(seed, stream, tower, shot)`.
6. Finish the path module as in §3.4 (one walker in `cav-sim`, `ls`/`schema <path>`/`insert`/`remove`, `x-unit` and `x-stage` attributes, `diff`, inverse-command undo); measure `apply(Set)` at 1000 bodies in the slice notes and call `edited` once.
7. Cadence designer, in order: the window of a `ScheduleSpec` enum (`Uniform`, `CycleJitter{kind, δ}`, `Alternating`, `Gapped`, `Loss{p}`) from the closed forms of §4.2; the five paper tests compared with those forms, with control arms; one realisation by exact sum with a preview mode; then
   the periodic-source table. **Ask George** (below).
8. For the owner, later: whether to route the forward model's transcendental functions through a pure-Rust `libm` to make bundles identical across Windows, Linux and macOS (the cost is one regenerated set of goldens; the per-call time is about 20% over glibc's here).

**For George** (add to `plan/PROGRESS.md`; defaults in use): does a late shot delay all later ones (the cycle time jitters, so shot times random-walk, as the scripts model it) or is each shot taken at a clock tick and merely
late (jitter about a grid)? The first broadens the aliases and lets them recede with N; the second leaves them at `|ψ|` for ever. Default: the scripts' model.

## 6. What I could not verify

- **The paper's Eq. (24)** and the preprint: arXiv was not reachable from this container. The closed forms in §4.2 are mine, confirmed numerically, and agree with the scripts' docstrings (`f_sig ± k/T_super`); compare them with Eq. (24) when the preprint is to hand. I read the scripts at the commit pinned in the S1 note ([`d7d360a`][s-pin]), not the report.
- **The classical references** (Scargle, VanderPlas, Deeming, Shapiro & Silverman): linked from memory of their citations; the pages were not opened from here.
- **Other platforms' `libm`**: only glibc 2.39 on Linux was available. The MSVC runtime and macOS were not measured; the claim that they differ rests on Rust's documentation, not on a measurement.
- **GPU reproducibility**: the WGSL text is read; no shader was run; whether one device is bit-reproducible with itself (atomics, scheduling) is untested.
- **Compile-time figures** are for a synthetic spec on this container's four cores, one run of three rebuilds each; the ranking is firm, the seconds are not. `facet`/`facet-reflect` were not used beyond deriving.
- **`json-patch` and `jsonptr`** were looked up (versions, descriptions), not run; the RFCs were not opened.
- **`zero_filled_fft_fails_ls_does_not`** and **`lift_excision`**, **`week_and_night_schedules`**: not analysed (§4.2).
- **The interpolation error** of the one-period table (§4.3) and `d_min(p)` (§4.2) were not measured; both are for the slice.
- **`LTO` and `-C target-cpu`** were varied only for the FMA loop; the forward model's own bits were not compared across flags.
- **The `Set` cost** was measured through `Engine::apply` with the forward worker running; the split between the round trip and the rest is arithmetic, not a profile.

## 7. Further questions

- Per-pair common-mode cancellation for staggered clocks (§4.3): the window of the *difference* of two schedules. Worth its own small note when M15's timing lands.
- Whether `bundle` hashes should use a cryptographic digest (M16 manifest) and which crate, if any.
- Addressing list items by label or id instead of position, for agents.
- A property test over random specs: every leaf path of `Value` gets, sets back to itself, and undoes (M15's `every_path_gets_and_sets`), written against the new walker.

## Sources

[core-float]: https://github.com/rust-lang/rust/blob/master/library/core/src/fmt/float.rs#L62-L83
[sj-de]: https://docs.rs/crate/serde_json/1.0.154/source/src/de.rs
[sj-ser]: https://docs.rs/crate/serde_json/1.0.154/source/src/ser.rs
[ron-ser]: https://github.com/ron-rs/ron/blob/v0.12.3/src/ser/mod.rs#L805-L819
[ron-parse]: https://github.com/ron-rs/ron/blob/v0.12.3/src/parse.rs
[npy-nep]: https://github.com/numpy/numpy/blob/main/doc/neps/nep-0001-npy-format.rst
[npy-impl]: https://github.com/numpy/numpy/blob/main/numpy/lib/_format_impl.py#L181
[npyz]: https://crates.io/crates/npyz/0.9.1
[ndarray-npy]: https://crates.io/crates/ndarray-npy/0.11.0
[rayon-sum]: https://github.com/rayon-rs/rayon/blob/main/src/iter/mod.rs#L1352-L1358
[std-hashmap]: https://github.com/rust-lang/rust/blob/master/library/std/src/collections/hash/map.rs#L18-L22
[std-f64]: https://github.com/rust-lang/rust/blob/master/library/std/src/num/f64.rs#L188-L195
[std-f64-sin]: https://github.com/rust-lang/rust/blob/master/library/std/src/num/f64.rs#L687-L690
[rfc3514]: https://github.com/rust-lang/rfcs/blob/master/text/3514-float-semantics.md
[glibc]: https://github.com/bminor/glibc/blob/master/manual/math.texi#L1546-L1556
[wgsl-fp]: https://github.com/gpuweb/gpuweb/blob/main/wgsl/index.bs#L14385-L14400
[rand-std]: https://github.com/rust-random/rand/blob/master/src/rngs/std.rs#L15-L26
[bevy-reflect]: https://crates.io/crates/bevy_reflect/0.20.1
[bevy-path]: https://github.com/bevyengine/bevy/blob/main/crates/bevy_reflect/src/path/mod.rs#L110-L115
[facet]: https://crates.io/crates/facet/0.46.5
[json-patch]: https://crates.io/crates/json-patch/5.0.0
[rfc6901]: https://www.rfc-editor.org/rfc/rfc6901
[rfc6902]: https://www.rfc-editor.org/rfc/rfc6902
[s-pin]: https://github.com/Thranduil02/atom-interferometry/tree/d7d360ad92837bec929b85091b2bdd9af81d285c
[s-jitter]: https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/cycle_jitter_sweep#L76-L79
[s-alias]: https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/cadence_alias_ratio#L134-L137
[s-bragg]: https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/cadence_bragg_comparison#L113-L114
[s-alt]: https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/alternating_cadence_aice#L6-L8
[s-alt28]: https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/alternating_cadence_aice#L28
[s-lost]: https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/lost_shots_aice#L1-L12
[s-gap]: https://github.com/Thranduil02/atom-interferometry/blob/d7d360ad92837bec929b85091b2bdd9af81d285c/gapped_fft_vs_ls#L1-L8
[scargle]: https://doi.org/10.1086/160554
[vanderplas]: https://arxiv.org/abs/1703.09824
[deeming]: https://doi.org/10.1007/BF00682104
[shapiro]: https://doi.org/10.1137/0108018
