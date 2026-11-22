# Review of `s3-sparse-voxel-structures.md`

*Reviewer who did not write the note. 21 Nov 2026. GPU-free. Most serious first.*

**Limit of this review.** The session's proxy refused every primary host (arxiv.org, gpuweb.github.io, w3.org, docs.rs, openvdb.org, chalmers.se, voxagon.se, wisc.edu; a `web-fetch` agent got DNS failures for the same hosts). Only the GitHub page for wgpu PR 6616 opened. Everything else was checked through web-search result text, which quotes the primary pages but is not the page. Those verdicts are marked *(search)*. I did not retry blocked hosts. Each *not verifiable* below needs one re-run with the hosts allowed.

## 1. Findings that change what S3 or S5 would do

1. **The terrain walk query is inside the ground, not "0.12 above the terrain" (§4.2).** `storage_model.py` queries terrain at z = 0.12 of the box. The height field is 0.25 + three sines, so the surface lies between 0.166 and 0.334 (computed on a 2001² grid). The query is 0.05–0.21 box-widths below the surface, in solid matter. The terrain walk rows therefore measure a buried point, not a tower above a road, and the "tests" and "evals" columns for terrain are not the case the note says they are. The sphere query (z = 0.98, surface at 0.90, so 0.08 outside) is correct. The qualitative result probably survives, since the terrain ratios track the sphere's, but the numbers need re-running with the query above the surface (say z ≈ 0.40).
2. **The "within about 3.5×" claim mixes value widths (§4.2, "Reading it").** The rows that carry 4 B per cell are the octree (57.8 MB), the 64-tree (30.9), the 4 B brickmap (102) and VDB (110): a 3.6× spread, as claimed. The 32³ chunk row (97.9 MB) is at 1 B per cell. At 4 B per cell it would be about 390 MB, 6.7× the octree and 12.6× the 64-tree. The brickmap 1 B row (29.0) is likewise not like-for-like. The conclusion that layout is worth "≤ 3×" in memory (§1, §8) holds only against the 64-tree and brick rows. Against the world-volume chunk layout the plan itself proposes (§5), it is several times worse. State the comparison at one value width.
3. **The 64-tree topology row is probably understated (§4.2).** The model charges 12 B per node (u64 child mask + u32 pointer). A 4³ node that can hold both child nodes and uniform leaves needs a second mask (leaf or empty versus child), so about 20 B. The note's own caveat covers the slot sidecar, not this. The 2.3–37 MB topology figures and the "mostly an accounting effect" remark should carry this uncertainty. The row is only a lower bound.
4. **The binding count in §6 has no headroom against the default limit.** The note lists seven bindings "under 8 if that limit is right". The limit is right: `maxStorageBuffersPerShaderStage` defaults to 8 in wgpu-types *(search of the latest docs.rs page, "Defaults to 8")*. But `params` is a uniform, so the seven bindings are six storage buffers, and the S5 kernel still needs the query list, the output buffer, and likely per-body transforms. That is eight or more. Either the layout is merged (moments_a/b into one array) or the builder must request a higher limit. The note's own advice (query `adapter.limits()`) covers it, but the "7 under 8" statement is misleading.
5. **The f32 moment bound (§5) is a single-node bound.** The arithmetic reproduces (6×10⁻⁸ × θ² × 4.7 ≈ 1×10⁻¹⁴; 6×10⁻⁸ × 4.7 × 2×10⁻⁴ ≈ 6×10⁻¹⁴; f32 mass ≈ 3×10⁻⁷ rad, ≈3000× over 10⁻¹⁰). Two omissions: the plan's bound has a factor 4 for four arm evaluations (`voxel.md` §2), which the note drops; and the errors of N accepted nodes add (linearly if the rounding is biased, as √N if random). With 10⁴–10⁶ accepted nodes the margin of 10⁴ in the COM and quadrupole terms is not obviously enough. The note does say "a bound, not a measurement"; add the N-fold sum to what S5 must test.
6. **The wrong PR is cited for the 64-bit integer claim (§6).** wgpu PR 6616 is "Allow 64-bit hex literals and const-evaluate unary ops" (merged 26 Nov 2024). It changes WGSL literal parsing. Its thread does say the validator rejects `u64`/`i64` unless the `INT64` feature is enabled, so the claim is true, but the PR is not the citation for it. Cite the wgpu `Features` documentation for `SHADER_INT64`. *Confirmed (claim), wrong (citation).*

## 2. Do the recommendation and evidence agree?

Mostly yes, with the caveats above.

- **Follows from the evidence:** keeping the plan's octree; integer leaf values (a design choice that makes the bit-exact oracle apply to every re-layout, and a sound one); rejecting SPGrid for matter; deferring the 64-tree to a profile. The walk-cost table reproduces and supports "do not choose the packing by the ray marcher" (see §3).
- **Follows only partly:** "reject NanoVDB". The reasons are fixed topology *(search: confirmed)* and per-node statistics that are not multipoles (reasonable, not sourced). The note's third reason, "3-level fixed depth", is imprecise: the default tree is a root plus three node levels (5-4-3), with 8³ leaves *(search: confirmed)*. The VDB storage row is the note's own estimate, labelled as such.
- **Does not follow:** "layout is worth ≤ 3× in memory" (finding 2) and "no layout changes the exponent". The latter is true for the walk cost shown, but the sphere is a smooth closed body, and the tolerance-versus-leaves argument is only supported on two friendly scenes.
- **Not tested against the plan's target.** The terrain scene at depth 12 is a 4096³ box. At the S4 target (1 km² at 5 cm) a side is about 2×10⁴ cells, depth ≈ 14.3. Nothing here is within a factor of 20–30 per side of that, so "storage is not the constraint" is an extrapolation by the surface-scaling argument (leaves ×16 per two depths, which does reproduce for the sphere: 4.21 M → 67.5 M, ×16.0).
- **The 10⁹-cell body** used for the 80 MB figure is outside the plan's 10⁴–10⁷ cells per body (`voxel.md` §1); it is a stress case, which is fine, but say so.

## 3. Reproduction

All computed in a scratch directory, `python3 -I`, numpy 2.5.3, Python 3.13.

| Check | Result |
|---|---|
| `storage_model.py` re-run for sphere and terrain at depths 10, 11, 12 (with `--dag --walk` as the note's file list says) | All six outputs identical to `results/*.txt`, line for line, apart from the timing in the first comment line (terrain depth 12 took 749 s here, so budget 12+ minutes). The model is deterministic. |
| `make_tables.py sphere-10 sphere-12 terrain-10 terrain-12` | Every cell of the storage table in §4.2 reproduces exactly, including bytes per cell and the occupied / leaves / internal row. |
| Walk table ratios (`t64 / oct`) | Depth 10: evals 1.24–2.98×, tests 2.30–4.76×, matching "1.2–3× and 2.3–4.8×". Depth 12: sphere evals 2.39–2.98×, terrain 1.98–2.51×, matching "2.4–3.0× / 2.0–2.5×". |
| DAG table | 5.1 M / 89.7 k = 57×, 14.0 M / 128 k = 109×, matching "57–110×"; fractional 7.3–8.9×, matching "7–9×". |
| Leaf scaling | 4.21 M → 67.5 M = ×16.03. |
| §6 arithmetic | 128 MiB / 8 B = 16.8 M; /88 B = 1.53 M; /96 B = 1.40 M; 14.5 M × 88 B = 1.28 GB. All correct. |

Reproduction is not validation. Points the model itself leaves open:

- The note says moment records are 88 B. `storage_model.py` charges 80 B (`moments_f32`) in its own `octree_moments_f32_B`; only `make_tables.py` uses 88. The table uses 88, so the table is consistent with the note, but the two scripts disagree with each other.
- The terrain scene is solid to the bottom of the box, so the interior merges to a few huge uniform leaves. That makes terrain friendlier than a scene with floors, interiors and air. The note says "friendly"; it is more so than "one material, no interiors" suggests.
- The brickmap and chunk rows store one hash entry per uniform solid brick or chunk, which charges them for interior the octree gets free; this is a modelling choice, not a measurement, and it favours the octree.
- The merge only fuses eight *Inside* children. Eight equal fractional children never merge. That is the plan's rule, but it means the merge yield on real bodies with many partial boundary cells is untested.

## 4. Claims with links

*Confirmed* = the primary source (or, marked *(search)*, its quoted text) says it. *Not verifiable* = host blocked and search did not quote it.

| Claim | Verdict |
|---|---|
| WebGPU defaults `maxStorageBufferBindingSize` 128 MiB (134217728) and `maxBufferSize` 256 MiB (268435456) | **Confirmed** *(search: MDN GPUSupportedLimits lists both)*. The WebGPU spec itself was not opened. |
| wgpu defaults match | **Not verifiable** for the exact numbers (docs.rs blocked; search showed an older wgpu-types page, "Defaults to 128 MB", and "Defaults to 8" for storage buffers per stage). The latest source page is wgpu-types 29.x. |
| `maxStorageBuffersPerShaderStage` = 8 (the note says it could not confirm) | **Confirmed** *(search: wgpu-types docs)*. |
| `maxBindGroups` = 4 (the note could not confirm) | **Not verifiable** here (search found it only in wgpu 0.7–0.11 docs; I recall 4 in the spec, but did not read it). |
| "Higher values must be requested at device creation and are capped by the adapter" | **Confirmed** *(search: wgpu docs wording, "limits better than the default must be requested")*. |
| 64-bit integers gated behind a wgpu feature, PR 6616 | **Claim confirmed, citation wrong** (finding 6). Metal status not checked. |
| Kämpe et al. 2013: Epic Citadel, 128K³, 19 billion voxels, 945 MB vs 5.1 GB for an SVO without pointers | **Confirmed** *(search: quotes the PDF and the Chalmers record)*. |
| Cornerstone: linear octrees for Barnes–Hut on GPU "up to 8 trillion particles" (arXiv 2307.06345) | **Confirmed** *(search: abstract)*. The 8 trillion run is on LUMI-G, many GPUs, so it says nothing about single-device build times. |
| `voxel.md` §12: Cornerstone "0.5 ms sort, 0.3 ms build" for 10⁶ keys on an A100 | **Not verifiable.** The note passes this on as [D]; I found nothing in search to check it. |
| Teardown: one byte per voxel, up to 255 materials per volume (Voxagon) | **Confirmed** *(search: quotes the blog)*. |
| Aokana (arXiv 2505.02017): chunks as SVDAGs, LOD and streaming, about 5% of data in VRAM | **Confirmed** *(search: abstract and review summary; the 5% figure is described as from the paper, exact wording not seen)*. |
| Hybrid Voxel Formats (arXiv 2410.14128): hybrids give a better storage/ray-speed Pareto frontier | **Confirmed** *(search: abstract)*. |
| NanoVDB: topology fixed after build; 32-byte alignment; breadth-first; default 5-4-3 | **Confirmed** *(search: FAQ and NanoVDB header text)*. "3-level" is imprecise (root plus three levels). The note's recollection of 5-4-3 was right. |
| fVDB (arXiv 2407.01781): one index grid with feature arrays beside it | **Not verifiable** (not searched; host blocked). |
| Karras HPG 2012, DOI 10.2312/EGGH/HPG12/033-037; Careil et al. 2020 (CGF); van Wingerden 2015 (Utrecht) | **Not verifiable.** The citations match what I remember of these papers, but nothing was opened. The note's "colour handled by that paper" for Careil is unchecked. |
| GigaVoxels / Laine & Karras report: bricks of 16³–32³ waste memory except for volumetric data | **Not verifiable**, and the attribution mixes two documents; check the wording in each. |
| SPGrid "relies on x86 virtual memory"; Sifakis et al. 2026 dynamic grids; the 64-tree guide (dubiousconst282) | **Not verifiable** (hosts blocked). |
| `plan/REBUILD.md` and `reference/design/voxel.md` references (§2.7 scale, §5.1 3,000 evaluation points, `voxel.md` §2 θ table and 4.7 rad, §3 30-bit keys and WGSL without 64-bit integers, §13 4³ re-pack, bodies of 10⁴–10⁷) | **Confirmed** against the repository. "§2.7" means principle 7 of §2. The plan's "~3,000 points" is 147 × 2 × 2 × 5 = 2,940, as the note says. |
| GTX 1660 SUPER, 6 GB build PC | **Confirmed** (`plan/PROGRESS.md`). |

Claims 1 to 5 in the note's §1 that rest on the note's own measurements or derivations are tagged [M] and [mine]; none is presented as sourced when it is not.

## 5. What a builder of S3 would still have to measure

1. **CPU build and rebuild time** of a real body: 10⁷ cells, 10⁶ leaves, rebuilt after every edit and every component split. The note has none (the A100 figure is GPU, unverified). Write the pass criterion first (the plan's frame budget for a stamp or a destruction edit).
2. **The real voxeliser's leaf count** on meshes with materials, shells and interiors, not the sphere and a sine field. This is what decides the merge yield and every storage figure.
3. **Merge yield with integer counts.** Counts at the boundary make equal-valued siblings rare; measure how many internal nodes merge at k³ ≤ 255 on a mesh.
4. **Integer-count semantics:** `count × unit` against the plan's "Σm = M exactly". The note says this changes one line of the S3 brief; the oracle that mass and inertia match the analytic values to a stated tolerance needs its tolerance written down before measuring.
5. **Re-run the walk with the terrain query above the surface** (finding 1) and on a body with a hollow interior.
6. **Node acceptance counts at tol = 10⁻⁶, 10⁻⁸, 10⁻¹⁰** (the note's own question 1), because the moment record count, not the topology, sets the S5 memory.
7. **Control arms** from the note's §7 table (change one count; swap two children; merge across a material boundary; zero a moment term) each written as a failing test before the property test passes.
8. For S5, not S3: adapter limits on the build PC (`cav doctor`), binding count (finding 4), float-float parity of the f32 moment bound (finding 5), and the 4³ re-pack against a frame profile.

## 6. Smaller points

- The note's §2 says only GitHub and PyPI resolved. The same applies to this review, which is why most links are "search-confirmed" only. Run the check again once the hosts are allowed; the table in §4 shows which rows need it.
- `storage_model.py` is not in Four's style (aligned `=` spacing is used, but the `lambda` assignment and `# noqa` lines, and no pyright run, are visible). I did not run pyright or the style check; not part of this task.
- Odd depths (11) are excluded from the tables for good reason (64-tree pairs levels), but the DAG table uses depth 11 and so mixes depths 10 and 11 under the labels 1024³ and 2048³; that is correctly labelled.
