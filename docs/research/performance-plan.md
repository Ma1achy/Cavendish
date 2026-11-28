# Where the time and memory go, and the plan to make the engine core fast (S25 "Speed")

The owner asked for the voxel renderer, the octree and the rest to be made as fast as they can be, and
whether physics casts rays through the voxels. This note measures first (release builds on the build
PC: Ryzen 5 3600, 6 cores / 12 threads, 24 GB, GTX 1660 SUPER, Vulkan), then ranks what to do and
where each item goes. By the owner's ruling of 28 Nov 2026 (relayed by the coordinator), after
S5 comes **S25 Speed, "the engine core works well and fast"**, centred on **the octree** (an engine
cache, incremental updates, a cache-friendly layout), **LOD** (render and read LOD consistent and
quick to switch) and **chunks** (streaming p99, building, meshing, memory, saving), with the ray query,
the read's speed and the town's rendering in scope as far as they sit on those three. Then S6, S11,
S12. Everything else here is placed in S21, S23 or a later speed slice (§7).

Measured [M], read in the code [C], from an earlier slice's record [R], from a source [S], my
estimate [E]. This PC's rule (the owner's): it is for correctness; absolute targets wait for a faster
machine, so every exit below is a **relative speed-up measured on this PC** plus a budget for the
web build.

## 0. Answer in brief

1. **The gravity read is the kernel.** On one thread, `cav run town` spends **72 % of its busy CPU
   in `cube_difference`, 62 % in one function, the cube series' `q(l, n)`** [M, §2.1]. The walk,
   the summaries and the local expansion are under 6 % together. `q` sums 10 to 165 monomials per
   order; the cube's symmetry needs 2 to 10 (cubic invariants, §3.2): the series written that way
   is **3 to 12× faster** and within 5×10⁻¹⁸ of the monopole [M]. A leaf 3 to 6.7 cells from an arm
   costs 17 µs a call and one under 3 cells 68 µs (the error-bounded split's recursion, which ends
   in the series), about 100 times a far leaf [M].
2. **The world is read twice for one `cav run`**, and the octree built twice: the contribution and
   the playhead's reading are each 38–39 % of the samples [M]. The engine's octree and world-read
   cache (`plan/IDEAS.md`) halves `cav run town` and makes scrubbing a static town free.
3. **Threads scale poorly**: `cav run town` took 64 to 73 s on one read thread, 24 s on 4 and 26 to
   41 s on the default 8 [M, all under load, §2.1, §2.4]: at best 2.8× on 8. The leaf sum spawns a fresh `thread::scope` per flight node
   (`WorldSource::difference`), 2–3 % of samples in `NtCreateThreadEx` alone, and the towers and
   walks run one after another.
4. **Each opened chunk is voxelised twice per read** (once for its summary in the octree build, once
   when the walk opens it): `voxelise_chunk` is 16 % of the busy CPU [M].
5. **Rendering the town is CPU-bound, not GPU-bound.** With timestamp queries (a profiling hook,
   PR #272) the GPU's frame over the 600-frame town flythrough is **about 3 ms at the median** at
   720p (chunk meshes 1.5 ms, p99 3.8; far bricks 1.2 ms, p99 2.6) and 3.3 ms at 1440p, while the
   frame is 12–23 ms at the median and 33–74 ms at p99 (load) [M, §2.2]. The frame thread spends a
   third of its time joining the streaming's chunk-making threads, a quarter in `prepare_list`
   (the per-chunk draw list, stand-ins, fresh buffers and bind groups each frame) and a fifth
   waiting for the GPU [M]. And **the gravity read runs at the same time on 8 threads**: in the
   flythrough 40 % of all samples are the town's world read, which competes with the frame [M].
6. **Ray casts.** Gravity is a volume sum, not a ray query: no ray belongs in the read. Rays (3-D DDA,
   Amanatides & Woo) are the right tool for picking, the walk camera's sweeps, shadows and AO, the
   far-brick march and later rigid contact, and Cavendish has four separate ad-hoc versions of them
   today. One shared structure (per-brick occupancy bits with a 4³ summary mask, the octree's levels
   above, a hierarchical DDA on the CPU and the same layout on the GPU) serves all (§5).

The ranked plan is §4; the slice is §6; the placement of everything is §7.

## 1. How it was measured

- **Builds**: `cargo build --release -p cav -p cav-app`, `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`
  (symbols, same code), own target directory, below normal priority.
- **Sampling profiler**: neither `cargo flamegraph` (needs DTrace or an elevated ETW session),
  `samply` (elevated on Windows) nor `wpr` kernel sampling (administrator) runs without
  administrator rights, and this session has none; Superluminal is not installed. So a small
  out-of-process sampler was written (`docs/research/performance-plan/sampler/`, 230 lines, no
  dependency but `windows-sys`): it suspends each thread of the child that used CPU since its last
  sample (`QueryThreadCycleTime`), walks the stack with DbgHelp's `StackWalk64`, resumes it, and
  writes folded stacks. It samples at Windows' timer resolution (about 64 Hz a thread here, 4,004
  samples over the one-thread town run): coarse but unbiased for anything over a few per cent. It
  misses threads that live under 20 ms (the leaf sum's), which is why the clean profile is taken on
  one thread (`CAV_THREADS=1`).
- **GPU timestamps**: a hook gated by `CAV_GPU_TIMING` (the device asks for
  `TIMESTAMP_QUERY`, `…_INSIDE_ENCODERS`, `…_INSIDE_PASSES`, which this adapter offers) writes six
  timestamps a frame: around the cull pass, the ground and bodies, the chunks, the bricks and the
  lines; the flythrough bench prints them as columns, with the CPU's encode time apart from the wait.
  It is a separate small PR (#272, merged).
- **The kernel**: a stand-alone benchmark against `cav-core`'s own functions
  (`docs/research/performance-plan/kbench/`): it copies the series' table, checks the copy is the
  kernel's bit for bit (`cube_series`), and times three forms of the series per band of distance.
- **Load.** Other agents' builds and validation runs (S5, a paper draft) held the CPU at 100 % for
  most of this session. Profiles give shares, which survive load; wall times under load are marked
  "(load)". The quiet re-measurements are in §2.4.

## 2. Measured breakdowns

### 2.1 The gravity read of `town` (5 towers, τ = 1e-2 rad, 25 shots)

`cav run town --out …`, release. Wall: **63.9 s on one read thread** (`CAV_THREADS=1`, the engine's
two forward workers still run the contribution and the playhead reading side by side), **26.1 to
29.9 s on the default 8** (load). S5's own record for the whole world contribution is 14.4 s on a
quiet PC [R: S5 §7.9]: twice that is the contribution and the reading.

One-thread profile, share of the busy samples (3,088 of 4,004; the rest is the main thread waiting):

| Where | Busy share | What |
|---|---|---|
| `read_tower` (the leaves and the far field through the flight) | **80 %** | of which the kernel 90 % |
| — `cube_difference` → `difference_within` | **72 %** | the leaf sum, 288 flight nodes × 2 interferometers × each leaf |
| — `q(l, n)` (the series' Legendre terms), self | **62 %** | one function |
| — `Local::gather` (the far nodes' series) | 2 % | |
| `World::chunk` → `voxelise_chunk` | 16 % | chunks made for the octree and again when opened |
| `Octree::build` (summaries, merge) | 9 % | includes its voxelising |
| `walk_into` (the walk, `open`, the chunks' trees) | 10 % | includes its voxelising; `Summary::bound` 3 % |
| `read::contribution` / `read::reading` | 49 % / 51 % | **the same world read twice** |

On 8 threads (load): 34 % of samples waiting (`NtWaitForAlertByThreadId`, threads joined in
`thread::scope`), 3.3 % creating threads (`NtCreateThreadEx`), `q` 26 %.

Why the series dominates [C]: the read reads 31,155 leaves one by one at tower 1 (the walk's cut,
S5 §7.9), at 288 flight nodes, at two interferometers: about 1.8×10⁷ `cube_difference` calls a
tower. A leaf within 6.7 to 400 cells of the arm takes the series to order 6–16, chosen so that the
truncation is below `f64::EPSILON/2` of **that leaf's monopole**, whatever the read's tolerance;
each order calls `q`, which rebuilds three 17-entry power tables and sums 10, 20, 35, 56, 84, 120,
165 monomials for l = 4 … 16 [M]. Nearer than 6.7 cells the cube is split into eight (recursively),
and within 3 it is the prism's 8 corners × (3 logs + 3 arctangents).

### 2.2 The renderer over `town`

`cav bench flythrough --scene town --frames 600 --size 1280x720` (and 300 frames at 2560×1440),
warm region cache, release, below normal priority, GPU waited for each frame; load 35 to 100 % from
other agents (`data/q-fly-*.csv`; `flystats.py`). GPU by timestamps (`CAV_GPU_TIMING`), ms:

| | 720p median | 720p p99 | 1440p median | 1440p p99 |
|---|---|---|---|---|
| cull (compute) | 0.03 | 0.03 | 0.01 | 0.02 |
| ground, bodies | 0.12 | 0.23 | 0.08 | 0.13 |
| chunk meshes (about 1,140 drawn, 100,000 vertices) | **1.52** | 3.78 | 0.35 | 2.63 |
| far bricks (65 to 160 drawn, 1.1 to 2.5× the pane marched) | **1.23** | 2.57 | 2.87 | 3.67 |
| the frame (CPU and GPU) | 11.9–22.9 | 33–74 | 12.1 | 25.3 |
| the engine's poll (streaming) | 0.09–7.9 | 12–59 | 0.10 | 9.5 |
| meshing | 0.01–1.9 | 11.5 | 0.01 | 11.0 |
| CPU up to submit (no timing) | 7.4 | 17.5 | | |

(The bench's first frame, cold, 64 meshes and 3.5× the pane marched, took 11.7 ms of bricks: the
bricks bind only when the near field is not yet resident.) So the GPU is a quarter of the frame or
less; the rest is the CPU. Where the frame thread's time goes (sampled, `data/flythrough.folded.gz`,
the frame thread only): `Streaming::step` 33 % (almost all of it joining the chunk-making threads),
`WorldGpu::prepare_list` 27 %, waiting for the GPU's fence 20 %. Over the same run **40 % of the
process's samples are the town's gravity read** (the engine's forward workers start on load), on 8
threads beside the frame's: the frame competes with the read for cores. On one thread (the page's
shape, `CAV_THREADS=1`) the second frame took 1.4 s (0.9 s of streaming) and stopped the bench's
alarm (load).

S4's record [R: S4 §7.4, §7.10]: over `stress-1e10` the frame was 30 ms median, 59–64 ms p99 after
the frustum fix, the draw 23–27 ms; over town natively median 4.2–4.5 ms, p99 14.5–15 ms after
TASK-S4-27, the meshing p99 4–5 ms, the streaming's step p99 6.8–7.2 ms, the draw 5–7 ms; on one
thread (the page's shape) p99 18 ms. The first view of town resident in about 2 s quiet (49 s and
21.6 s in this session under load).

What the code does each frame [C, `cav-render/src/world.rs`]: per drawn chunk a `set_vertex_buffer`,
a `set_index_buffer` and a `draw_indexed_indirect` (each chunk its own two buffers, 72 B a quad:
four 12-byte vertices and six 4-byte indices); a new 128 KiB origins buffer, a palette, a uniform,
the draws, the indirect buffer and two bind groups made with `create_buffer_init` every frame;
chunk meshes drawn with no back-face culling; the bricks drawn as boxes whose fragment shader
marches and writes `frag_depth`, which turns early depth off for every brick pixel, at 4× MSAA.

### 2.3 The rest

- **Editor frame**: six viewports, `town` resident, 1.26–1.30 ms of the 4 ms budget; a walker's
  step 2.1–2.4 µs [R: S20 §7.6]. Not a target.
- **Voxeliser and cut cells**: 5–6 µs a cut cell for convex solids, 46 µs for a ball, 59 µs for a
  16,384-triangle mesh [R: S5 §7.6]; a 3.2 m town chunk is made in about 0.3 ms by its pieces'
  raster (16 % of the read above). Mesh import is bounded by the clip; the ball's inner integral in
  closed form is in `plan/IDEAS.md` (24× fewer chords).
- **The quadrature** itself (`Reader::sum`, the node visits) is under 1 % of samples: its cost is the
  source it calls, i.e. the kernel.
- **Memory** [R, C]: the octree 360 B a node, 2.5 MB for town; at `MAX_CUT_NODES` (2^19) 172 MB of
  summaries at τ = 1e-6; streaming resident about 2,700 chunks and 3,600 bricks, about 40 MB
  [S4 §7.4]; a brick on the GPU 1,042 words (4.2 kB); a leaf as a read sees it (`Cube`, a `Vec3` and two
  `f64`) 40 B, so town's 3.29 M leaves would be 132 MB if all were kept.

### 2.4 Quiet re-measurement

The machine did not go quiet in the session (an hour's watch for three readings under 35 % never
fired). The runs below were taken as the load fell to 35–80 %, each with the load beside it:

| Run | Wall | Load |
|---|---|---|
| `cav run town`, 1 read thread | 72.9 s | 37 % |
| `cav run town`, 4 | 24.4 s | 80 % |
| `cav run town`, default (8) | 40.5 s | 73 % |
| the flythrough, 720p, timed | median 11.9 ms, p99 33.3 ms | 35 % at its end |

The series benchmark (`kbench`, two runs back to back at 87–88 % load; ns a point, per band of
distance in cells; `data/q-kbench*.txt`):

| Band (cells) | mean order | kernel `correction` | powers once (bit for bit) | invariants | `cube_difference` |
|---|---|---|---|---|---|
| 6.7–10 | 15.2 | 587–595 | 472–785 | 49–81 (7–12×) | 1,309–1,327 |
| 10–30 | 11.1 | 271–479 | 209–353 | 55–57 (5–8×) | 540–947 |
| 30–100 | 7.7 | 115–197 | 151–173 | 35–43 (3–6×) | 300–491 |
| 100–400 | 6.0 | 79–174 | 70–107 | 18–29 (4–6×) | 167–277 |
| 3–6.7 (split) | | | | | 17,300–19,800 |
| under 3 (prism and split) | | | | | 67,100–68,300 |

The bit-for-bit form gains little (0.7–1.6×, within the noise): the power tables are not the cost,
the monomials are. The invariant form gains 3–12× on the series wherever it is used, including
inside the split's halves.

## 3. Two measurements that settle the first moves

### 3.1 Bit-for-bit form of the series

The kernel's own table in compact form (powers as bytes, coefficients as one array per order) and the
power tables made once a point instead of once an order: **bit-for-bit equal to `cav-core`'s kernel
over 200,000 points × 7 orders** [M]. It gains 0.7–1.6× (§2.4): not worth a task of its own.

### 3.2 The cubic-invariant form

`Q_l` is invariant under the cube's symmetry group, so on the unit sphere it is a polynomial in
`σ₂ = n_x²n_y² + n_y²n_z² + n_z²n_x²` and `σ₃ = n_x²n_y²n_z²` (the invariants of the hyperoctahedral
group are the elementary symmetric polynomials of the squares, and `σ₁ = 1` there) [S: the "kubic
harmonics" of Von der Lage & Bethe, Phys. Rev. 71, 612 (1947), doi:10.1103/PhysRev.71.612, not
re-read; the invariant-ring statement is standard]. The monomials `σ₂^i σ₃^j` with `4i + 6j ≤ l`
number **2, 3, 4, 5, 7, 8, 10** for l = 4 … 16, against 10 … 165 [M]. Fitted by least squares on
2,000 directions, the series built this way differs from the kernel's by at most **5.2×10⁻¹⁸ of the
monopole** over 200,000 points and every order [M]: below the series' own `ε/2` stop. It changes
the last bits, so it needs the oracle's tolerances checked, not fixtures regenerated, and exact
rational coefficients (derivable from `q_terms`) rather than a fit before it ships.

## 4. The ranked opportunities

Gain against effort and risk. "Ledger unchanged" means: the read's numbers within the read's own
ledger and audit of the previous build's (the W2/W3 cases pass unchanged, no tolerance moved), and
where an item claims bits, the same bits.

| # | Item | Code | Technique (source) | Expected gain [E unless M] | Effort, risk | Pass criterion (write before measuring) |
|---|---|---|---|---|---|---|
| 1 | **The engine keeps the octree and each tower's world read** | `cav-engine` forward store; `cav_sim::read::reading`; `world::read_world` | cache by the world's hash (IDEAS, TASK-S5-12); a playhead reading of a static world composed from the kept contribution; the local series kept too | `cav run town` 2× [M: two reads of 38–39 %]; a seek or a second run of an unchanged town ≈ 0 | days, low | `cav run town` ≤ 0.55× before; second `reading` of an unchanged world ≤ 5 % of the first; bits identical; control: an edited piece misses the cache |
| 2 | **The series in cubic invariants** | `cav-core/src/voxel/kernel.rs` `q`, `correction` | Q_l as a polynomial in σ₂, σ₃ (§3.2) with exact rational coefficients from `q_terms`, Horner in each | series 3–12× [M]; the series is 62 % of the read, so the read about 2× [E, Amdahl] | days, low | ≤ 1e-15 of the monopole at every order and direction against `q_terms`' form; `kernel_error_by_distance` and the oracle within their tolerances (none moved); town tower 1's read time ratio recorded |
| 3 | **The leaf's series order from the read's budget** | `order_needed`, `WorldSource` | choose each leaf's order so its truncation bound, through the flight, fits a share of τ (the walk's ledger rule applied below the node); the bound joins the ledger | most leaves at order 0–4 at τ = 1e-2 [E]; the series' 62 % mostly gone | days, medium (a physics change: a new ledger term) | W2 (error ≤ ledger ≤ τ) and W3 pass unchanged; ledger reported; control: the order forced to 0 past its bound breaks W2 |
| 4 | **One pool of threads, work split by tower and by flight node; the read leaves the frame its cores** | `world.rs` `WorldSource::difference`, `open`, `read_world`; `octree.rs`; `cav-engine` `forward/threads.rs`, `world.rs` (streaming) | a persistent pool (a scoped work queue) instead of a `thread::scope` per flight node, per streaming batch and per meshing batch; towers' walks and reads in parallel; the read's threads below the frame's priority and at most cores − 2 while a window draws; fixed block order kept so bits stay | 8-thread efficiency from about 0.35 to ≥ 0.65 [M baseline]; the frame's third spent joining the streaming's threads [M] | days, low (determinism held by order) | same bits on 1, 2, 8 threads (O3-style); `cav run town` 8 threads ≥ 5× 1 thread; flythrough frame p99 with the read running ≤ 1.2× without it |
| 5 | **Each chunk voxelised once per read** | `octree::chunk_cubes`, `world::open`, `inner::ChunkTree::of` | keep the opened chunks' trees with the cached octree (item 1); or build the inner tree during the octree's summary pass and keep only what the cut opens | the 16 % voxelising halved; with item 1 a re-read voxelises nothing | days, low | chunks voxelised per read counted: each at most once; memory of the kept trees reported against `MAX_RESIDENT`'s 512 MiB |
| 6 | **Leaves as SoA and sorted by regime** | `WorldSource::cubes`, `leaves_at` | structure of arrays (x, y, z, m; side shared per level); leaves sorted into series, split and prism groups so each group is a tight loop; AVX2 4-wide `f64` for the monopole and invariant series (Rust `std::arch` or auto-vectorised) | 2–4× on the series group [E] | days, medium | same sums in a fixed order (bits) or ≤ 1e-15 relative with the order stated; town tower 1 ratio |
| 7 | **The world's draw in one buffer: vertex pulling and one indirect draw** | `cav-render/src/world.rs` `prepare_list`, `draw`; `world_render.wgsl` | 8-byte quads (position, size, face, material, AO byte) in one arena storage buffer, pulled in the vertex shader; the cull compute writes one compacted instance list and one `draw_indirect` (WebGPU core) (voxel-engines.md §3.2, §4.1; Aaltonen & Haar 2015) | mesh memory and upload 9× [S]; CPU draw loop 3 commands × chunks → 1; no per-mesh buffers | a week, medium | the same pixels (golden image, tolerance 0) on town and a cap test; CPU encode time per frame recorded before and after; web build draws the same |
| 8 | **Per-frame buffer and bind-group churn** | `world.rs` `prepare_list`, `renderer.rs` `draw_things` | persistent origins, palette, uniform and indirect buffers written with `queue.write_buffer`; bind groups made when a buffer is replaced, not each frame; a staging belt for uploads part of `prepare_list`'s 27 % of the frame thread [M]; 0.5–2 ms a frame [E] | hours, low | CPU encode time p50 and p99 over the flythrough ≤ before; same pixels |
| 9 | **Bricks: early depth back** | `world_render.wgsl` `fs_brick`, the brick pipeline | (a) march the bricks in a depth pre-pass at half resolution and draw them with `LessEqual` against it; or (b) a previous-frame Hi-Z of brick boxes (voxel-engines.md §4.2, Aokana §3.5 [S]); (c) the march from the box's front face only, sorted front to back | over town the bricks are 1.2 ms median (2.9 at 1440p) of a 12–23 ms frame [M]: worth it for stress-1e10's far field (S4: the draw 23–27 ms), not for town | a week, medium | GPU brick time p50/p99 over the flythrough (timestamps) ≤ 0.5× before; same pixels within the golden-image rule; web budget below |
| 10 | **Streaming and meshing in the background** | `cav-engine/src/world.rs` `Streaming::step`, `WorldGpu::prepare` | chunk making and meshing on worker threads with a budget per frame for the hand-in only (natively); one thread in a page as now | the town's p99 (the step's batch 6.8–7.2 ms, meshing 4–5 ms) to the draw alone [R] | days, medium (the handover's stand-ins must still hold) | `gpu_a_moving_camera_never_loses_what_it_drew` unchanged; frame p99 over the town flythrough ≤ 0.6× before natively; one-thread p99 ≤ before |
| 11 | **Incremental octree on edit** | `octree.rs`, the engine's cache | an edited piece dirties the chunks it `reaches`; their summaries and their ancestors' are made again (O(dirty × levels)), the rest kept | an edit re-reads in proportion to what it touched | days, low (summaries are additive and made in key order) | after any edit the octree equals a fresh build bit for bit (O3's check); control: a stale ancestor is caught |
| 12 | **The shared voxel ray query** | new in `cav-core::voxel` (occupancy), used by `cav-app` `tools::pick`, `walk`/`occupancy`, later S21 and S7 | per-brick 32³ occupancy bits + 4³ mask (the GPU brick's own words), the octree levels above; Amanatides & Woo DDA per level (§5) | picking the world at all (today bodies only, brute force over every leaf) [C]; one structure for five consumers | days, low | picks and sweeps equal the brute-force answer on random rays (exact), including axis rays; control: one bit cleared changes a hit |
| 13 | **The cut's nodes in Morton order, summaries SoA** | `walk.rs`, `summary.rs` | key order is already deterministic; a Morton-ordered node array and the bound's inputs (`A_l`, centre, reach) in their own arrays (Morton 1966; Warren & Salmon 1993) | the walk is ≤ 10 % of the read today: worth it only after items 2–4 [M] | days, low | walk time per tower ratio; same cut bit for bit |
| 14 | **Region files and saving** | `world/cache_native.rs`, `region.rs` | write behind on a thread; the bricks and chunk cells palette-packed as now; a manifest of hashes so an unchanged region is not rewritten | saving off the frame [E] | days, low | no frame over the flythrough's p99 while saving; round-trip bit for bit |
| 15 | **The GPU walk** (S23) | — | the cut's far nodes and the leaves in float-float or native f64 (`SHADER_F64` is offered here; WebGPU has no f64), the guarded error-free transforms (surface-representation.md §6) | the near leaves 5–30× one core [S] | weeks, medium | GPU = CPU f64 ≤ 1e-9 with float-float (S23's exit) |

Not on the list, with why: a GPU mesher or mesh shaders (meshing is 0.03 ms a chunk since S4,
voxel-engines.md §1.4); DAGs or SVO-DAGs for the store (the leaves carry counts and materials and the
world is edited; Kämpe et al. 2013 and HashDAG measure binary occupancy of static scenes:
voxel-engines.md §2.5); NanoVDB or GVDB as the store (no WebGPU path that keeps exact integer
leaves; GVDB is CUDA: Hoetzlein 2016); ESVO (Laine & Karras 2010) for the far field (the clipmap's
bricks already give the march its LOD; its beam optimisation is item 9's idea); lossy coders on
anything that weighs.

## 5. The owner's question: ray casts through the voxels

**The gravity read is not a ray query and should never be one.** The phase is
`∫ flight ( V(z_upper) − V(z_lower) ) dt`, and `V` at a point is the sum over every piece of matter:
`−G Σ m_i / |r − r_i|` and its higher terms. Every voxel contributes, whether or not a line of sight
reaches it; there is nothing for a ray to find. What makes the sum fast is the octree with bounded
multipole acceptance (Barnes & Hut, Nature 324, 446 (1986), doi:10.1038/324446a0) and a local
expansion along the tower's line (Greengard & Rokhlin, J. Comput. Phys. 73, 325 (1987),
doi:10.1016/0021-9991(87)90140-9): S5's walk and `local.rs`. Items 1–6 make that sum faster; a DDA
would make it wrong.

**Where rays are the right tool, and what Cavendish does today** [C]:

| Query | Today | With the shared structure |
|---|---|---|
| Picking (a click on the world or a body) | `tools::pick`: a slab test against **every leaf of every body**, the world not picked at all | DDA through the world's bricks and the bodies' trees in their frames; nearest hit |
| The walk and fly camera | `occupancy.rs`: the world's pieces as analytic boxes, made each frame, swept box tests (2.1–2.4 µs a step) | swept AABB against the occupancy bits (a "fat" DDA over the cells the box's face crosses); the same answer for streamed and analytic worlds |
| Far-brick march (GPU) | `world_common.wgsl` `march`: 4³ sub-block mask, then cells, per brick | the same layout; a level of the octree's masks above the bricks to skip empty boxes |
| Shadows and AO (S21) | planned: a per-clipmap-level occupancy volume (voxel-lighting.md §5.2) | the same bits, re-packed per level |
| Rigid contact (S7) | planned | the bodies' occupancy against the world's: broad phase by brick masks, narrow by the cells, an SDF per brick only where contact needs a normal (rigid-bodies.md) |

The structure: **occupancy as bits, per brick, with a 4³ mask over 8³-cell blocks (the GPU brick's
words already: 32,768 bits + 64 + materials), and the octree's levels as masks above.** A ray
walks it with Amanatides & Woo's DDA at the coarsest level that is non-empty along the ray, descends
where a mask bit is set, and climbs back when it leaves the block, which is a hierarchical DDA
(Amanatides & Woo, Eurographics 1987 [S: first pass only, voxel-engines.md §11]; the two-level
version in the brick march; Laine & Karras 2010's ESVO for the octree form). One CPU module in
`cav-core` (`voxel::ray`) with the exact integer stepping and its f64 entry, and its GPU twin in
WGSL the march already is, held to each other by the `gpu_brick_march_matches_the_cpu` pattern.
The world's occupancy comes from the chunks' cells (resident) or, where not resident, from the
pieces' cover (conservative), so a query never waits for streaming. For S25 only picking and the walk
camera need it; S21 and S7 reuse it.

## 6. The slice: S25 "Speed: the engine core works well and fast"

**What the owner can newly do**: open `town`, scrub and re-run it without waiting for the world
again; edit a building and see only its chunks re-read; fly the town with a frame tail no worse than
the draw; pick the world; switch render LOD and read LOD views at once; save a session's world
without a hitch.

**Exits** (each measured on this PC against the S5 tag, quiet, below normal priority, written
before measuring; web budgets for the page):

| Exit | Criterion |
|---|---|
| E1 the octree is cached | a second read of an unchanged world ≤ 5 % of the first; `cav run town` ≤ 0.55× S5's; bits identical; control: an edit misses |
| E2 incremental | after any edit the octree and the cut equal a fresh build bit for bit; re-reading after a one-building edit ≤ 0.2× a fresh read |
| E3 the read per tower | town's tower 1 at τ = 1e-2 ≤ 0.33× S5's 2.4 s, at 1e-4 and 1e-6 ≤ 0.5×; W2/W3 unchanged, no tolerance moved (ledger unchanged) |
| E4 threads | 8 threads ≥ 5× 1 thread on `cav run town`; same bits on 1, 2, 8 |
| E5 chunks | each chunk voxelised at most once per read; town flythrough frame p99 ≤ 0.6× S5's natively and ≤ 1.0× on one thread; meshing and streaming p99 recorded |
| E6 LOD | render LOD and read LOD drawn from one cut description; switching `lod-view` (camera, tower, towers) within one frame of the data being there (no rebuild); the S5 overlays unchanged |
| E7 render | over the town flythrough at 720p, the CPU up to submit p50 and p99 ≤ 0.5× S5's and the frame p99 ≤ 0.6×; GPU time not worse (timestamps); same pixels |
| E8 ray query | picks and camera sweeps equal brute force on 10⁵ random rays, axis rays included; control: one bit cleared |
| E9 memory | the cache's bytes counted and capped, with a memtest row (S2 §3.1); resident world ≤ S5's at the same view |
| E10 web | the page's frame p99 over the town flythrough on one thread ≤ S5's; no new binding beyond WebGPU's 8 storage buffers a stage; wasm size within +5 % |

**Planet-ready foundation choices, compressed chunks and saved worlds** are the Worlds note's
tasks in this same milestone (`docs/research/worlds.md`; TASK-S24-02 64-bit keys, -03 `f64`
positions with local frames, -04 cube-sphere chunk addressing, -05 the recipe format, -06
compressed chunks, -07 recipe plus edit delta); this plan does not duplicate them. What it asks of
them for speed: the cache (TASK-S25-03), the Morton order (-09) and the region files (-14) keyed by
the 64-bit key from the start, so there is no second migration; and every GPU and kernel offset
formed in `f64` in a local frame, which the read already does (REBUILD §3.4). For the record,
today's `ChunkKey` is three `i32` (12 B; ±6.9×10⁹ m at 0.1 m, but the Worlds note measures it
overflowing at 6.4×10⁶ m and 1 cm), and a cube-sphere key of 3 face bits, 22 bits each of u and v
(4 M chunks × 3.2 m = 13,000 km a face side) and 17 radial bits (420 km) fits one word at 0.1 m [E].
S25 runs those tasks first where its own depend on the key (03, 09, 14).

**Tasks** (`plan/tasks.yaml`, TASK-S25-01 to 15), in this order, each measured before the next:
01 the brief; 02 profiling kept (timestamps, the sampler as `cargo xtask profile`, the baseline);
03 the octree, chunk-tree and world-read cache (items 1, 5); 04 the incremental octree (11); 05 the
series in invariants (2); 06 the near leaves' cost, the split regime's 17–68 µs a call measured
by depth and bounded by the budget (3); 07 the leaf's order from the budget (3); 08 the pool and parallel towers (4); 09 leaves by regime and Morton nodes (6, 13);
10 streaming and meshing off the frame (10); 11 the world drawn from one buffer, persistent buffers
(7, 8); 12 render and read LOD from one cut (E6); 13 the shared ray query for picking and the walk
camera (12); 14 saving off the frame, in TASK-S24-06's compressed format (14); 15 the record. 03 and 05 first: the first halves
every run, the second halves every read.

## 7. Where everything goes

| Item | S25 | Later |
|---|---|---|
| 1 cache, 5 voxelise once, 11 incremental, 13 Morton/SoA nodes | S25 (the octree) | — |
| 2 series compact and invariants, 3 order from budget, 4 pool, 6 SoA leaves | S25 (the read per tower) | — |
| 10 streaming and meshing off the frame, 14 saving | S25 (chunks) | — |
| 7 one-buffer draw, 8 buffer churn | S25 (town rendering) | — |
| E6 render and read LOD from one cut | S25 (LOD) | — |
| 12 ray query | S25 for picking and the walk camera | S21 shadows and AO; S7 contact |
| 9 bricks' early depth, Hi-Z, tile march | — | S21 (the look owns the brick shader's cost) |
| 15 the GPU walk, f64 or float-float | — | S23 |
| GPU meshing, DAGs, NanoVDB | rejected (§4) | — |

## 8. What I could not verify

- **Quiet wall times** for the render and the read, beyond §2.4: the PC was at 100 % from other
  agents for most of the session; every "(load)" number is an upper bound, and the profile shares
  are what the ranking rests on. Resolve: each S25 task measures its own before and after on a
  quiet PC, as its exit says.
- **The flythrough without the gravity read beside it**: every flythrough here ran while the
  engine's workers read the town (40 % of the samples). The bench should be able to hold the read
  off (a flag), so that the frame is measured alone and with the read; S25's TASK-S25-02 adds it.
- **The page's flythrough on one thread** stopped at its alarm on frame 1 under load (1.4 s); S4's
  quiet record is p99 18 ms. Re-measure quiet.
- **Item 3's gain**: how many of town's 31,155 leaves fall to order 0–4 under a ledger-shared budget
  is an estimate; the leaf order histogram at τ is a one-hour measurement in the slice.
- **The invariant form's exact coefficients**: fitted in f64 here; derive them from `q_terms` before
  shipping.
- Von der Lage & Bethe and Amanatides & Woo were not re-read for this note.

## 9. Further questions (not chased)

- Whether the 288 flight nodes could be fewer for a static world: every shot of a static world reads
  the same, and a 1-D interpolant of the near leaves' potential along the tower's line (piecewise
  Chebyshev, as `local.rs` does for the far nodes) would read each leaf a fixed number of times.
- Whether the 4× MSAA is worth its cost on the bricks once item 9 lands.
- Whether `MAX_CUT_NODES`' 172 MB at τ = 1e-6 should be streamed rather than held.

## Reproduce

`docs/research/performance-plan/`: `sampler/` (the profiler: `sampler <out.folded> <ms> -- <program>
[args]`), `fold.py` (self and inclusive shares from folded stacks, `.gz` or not; a third argument
keeps only stacks through a function), `flystats.py` (median and p99 of a trace's columns),
`kbench/` (the series; `cargo run --release` from its folder), `measure.ps1` (the batch as run,
with this session's scratch paths), and `data/` (the folded stacks and traces of this session).
The flythrough needs a release `cav` built after #272 for the timing columns.
