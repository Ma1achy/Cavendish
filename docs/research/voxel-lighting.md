# Making the voxel world look good: ambient occlusion, shading, shadows, sky, colour

Research note, 25 November 2026. No code was changed and nothing was built or run: the numbers below are
the literature's, scaled by reasoning where I say so, and the first slice that builds any of this must
measure its own (section 9). Links are to the primary source where I could read it; where I could not, the
claim is marked **(not read)** or **(unverified)**.

**Changed after reading the primary sources (25 Nov 2026).** The owner supplied the papers; the web items
were fetched directly. Papers are cited by title, authors, year and the open links in the source list;
none is copied into the repo.
- **Hillaire 2020, Table 2 read from the page image:** LUT sizes and costs are now exact (transmittance
  256x64, sky-view 200x100, aerial perspective 32^3 over 32 km, multiple scattering 32^2; 0.17 ms for all
  four LUTs on a GTX 1080), and the **total is 0.31 ms at 1280x720** (section 7), of which 0.14 ms is the
  on-screen sky and aerial perspective. The earlier "garbled table" caveat is gone. The paper gives **no
  1080p figure**; the 1080p number below is my scaling. The paper also says 16-bit float LUTs are enough
  for its model, which removes the `float32-filterable` concern on the web.
- **GTAO's `b` sign settled:** the fit is `G(A, rho) = a A^3 - b A^2 + c A` with `b = 4.7951 rho - 0.6417`
  (Appendix B, eq. 33; it was "eq. 10" in my earlier note, wrongly). Checked: `G(1, rho) = 1` for every
  `rho` only with the minus sign. The paper's main text (section 4.2, eq. 12) gives an analytic
  alternative, `rho A / (1 - rho (1 - A))`. The AO integral is cosine-weighted (eq. 5), and the 0.5 ms
  PS4 figure is half resolution with a 4x4 spatial and 6-frame temporal reuse (section 4.3).
- **Bruneton & Neyret 2008 read:** table sizes (8 MB 4-D table at 16-bit), a 5 s precompute on an
  8800 GTS, and light shafts by geometry-shader shadow volumes, which do not fit Cavendish (no geometry
  shaders in WebGPU; the far field is not geometry). Recommendation unchanged.
- **Hosek-Wilkie 2012 read:** no run-time timing in the paper, only "slightly more costly to evaluate
  than the Preetham model" (section 8); fitted for a ground observer, turbidity 1 to 10, sun at or above
  the horizon. Confirms "ground views only".
- **Teardown's own blog read** (Gustafsson 2018): replaces the secondhand PC Games Hardware line with the
  author's numbers (1-bit volume, 4 mip levels, 5 AO rays per pixel, about 9 ms for all lighting at
  1080p on a GTX 1080) and adds blue-noise sampling advice for any jittered shadow. Shadow recommendation
  unchanged; its cost estimate is now anchored to a primary figure.
- **Cube World:** Wollay's 24 Oct 2026 patch notes read; they name tweaks (bloom, lighting, sand and snow
  colour, a fix for coloured bands near shaded edges) but no method. The Reddit colour thread could not be
  fetched. Section 7 still stands on its own, as before.

## 1. One-page summary

**What the renderer has today.** Near chunks are greedy meshes (`ChunkVertex { cell, attributes: [face,
material, 0, 0] }`, two spare bytes per vertex); far bricks are ray-marched in the fragment shader with
`frag_depth`; the only lighting is one directional term, `base * (0.38 + 0.62 * max(n.l, 0))`, applied in
linear space, with `params.z` switching it off (`crates/cav-render/src/world_render.wgsl`, `shade`).
There is no AO, no shadow, no sky, no fog, no tone mapping. The frame already costs 19 to 33 ms on the
GTX 1660 SUPER (`docs/slices/S4.md` section 7.4), so every layer below must be nearly free or switchable.

**Ranked recommendation** (cost = GPU time on the 1660 SUPER at 1280x720, my estimate unless stated):

| Rank | Layer | Why first | Cost |
|---|---|---|---|
| 1 | **Per-vertex baked AO** in the greedy mesh (0fps), merge only when AO matches | Near-free at run time (a byte per vertex), view independent, exact on a corner and a step so it is testable | about 0 ms; mesh time up a little |
| 2 | **The same AO formula evaluated at the ray-march hit** for far bricks (3 neighbour bit lookups per face corner, bilinear on the face) | Near and far match; no extra ray steps; no noise | under 0.3 ms (estimate; needs a measurement) |
| 3 | **Sun plus hemisphere ambient, sRGB-correct, one tone curve, distance fog** to the sky colour | Fog also hides the LOD seam; pure arithmetic | under 0.1 ms |
| 4 | **Hard sun shadows by a ray through a camera-centred coarse occupancy volume** (Teardown's way), not shadow maps | Shadow maps would need the ray-marched far field drawn a second time; the grid is already the data | 0.5 to 2 ms (estimate; the main unknown; Teardown's author gives about 9 ms at 1080p on a GTX 1080 for *all* its traced lighting, AO, fog and reflections with ten lights) |
| 5 | **Analytic sky now, Hillaire LUT atmosphere with the planet** | A sky gradient is a few ALU; Hillaire's method is 0.31 ms in total at 1280x720 on a GTX 1080 (0.17 ms LUTs + 0.14 ms on screen; paper, section 7 and Table 2) | 0.3 to 0.5 ms later |
| 6 | **GTAO** only if baked AO is not enough (thin features, bodies) | Needs depth plus normals, a denoise, and care with MSAA | 0.5 ms at 1080p, half resolution, on a PS4 (paper); 0.6 ms at 1080p full resolution on an RTX 2060 (XeGTAO), so 1 to 2 ms here (estimate) |

**Beauty versus data.** Every layer is a field in a `LookSpec` (data, principle 11), and a *data* look is
the identity: base colour from the colour map, no AO, no shadow, no fog, no tone curve (section 8). The
beauty layers multiply a colour that data never passes through, so a colour map is never distorted, and the
data look is the default whenever a field colour is on.

**Cube World's look** (section 7): per-voxel colour, no textures, flat faces, soft per-vertex occlusion. No
primary source describes its internals (the developer's 2026 patch notes name tweaks, not methods), so
section 7 describes the general technique and labels the Cavendish choices as mine.

**Not verified.** Any millisecond figure for this card or for WebGPU; Cube World's actual method; the
Hosek-Wilkie run-time cost (the paper gives none; see section 6 here); Hillaire at 1080p (the paper gives 720p only).

## 2. What the code is today (so the layers fit it)

- The mesh vertex has `attributes: [u8; 4]` = `[face, material-1, 0, 0]` (`world.rs:153`, `:266`), uploaded
  as `Uint8x4` (`:581`). Four 2-bit AO values are exactly one byte: the spare bytes hold them, no format
  change.
- Greedy merging is per slice and per material (`world.rs:172-178`): the merge key gains the AO byte.
- The march returns the hit cell and face (`world_common.wgsl`, `march`); `cell_set(base, c)` reads one
  occupancy bit of the 32^3 brick. So AO at the hit is three `cell_set` calls per face corner.
- The pipeline takes `samples` (MSAA) and a depth format (`world.rs:506`); bricks write `frag_depth`.
- Web: WebGPU only, one thread, compute shaders available, no WebGL fallback
  (`docs/research/web-build-and-pages.md` sections 2 and 3). WebGPU's default limits give 8 storage buffers
  per shader stage and 3-D textures up to 2048 per side ([WebGPU spec, limits](https://www.w3.org/TR/webgpu/#limits)),
  and `timestamp-query` is an optional feature, so a GPU-time badge on the web must tolerate its absence.

## 3. Ambient occlusion

### 3.1 Baked per-vertex AO in the greedy mesh (recommended, layer 1)

The classic method ([0fps, "Ambient occlusion for Minecraft-like worlds"](https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/))
gives each face vertex a value from the three cells touching that corner on the face's open side:

```
vertexAO(side1, side2, corner) = 0                          if side1 and side2
                               = 3 - (side1 + side2 + corner) otherwise      (0 darkest, 3 open)
```

```
 open side of a face (looking at the face):      a step edge:
   s2 | c        c, s1, s2 = the three cells        ______
   ---+---        touching vertex v on the           |    |   vertices under the lip
   v  | s1        open side                    _______|    |   get AO 1 or 2, the open
                                                      ^ lip     floor far away 3
```

- **Anisotropy.** A quad is two triangles, so non-planar AO values interpolate differently along each
  diagonal. 0fps flips the diagonal when `a00 + a11 > a01 + a10`. Needed, one comparison at mesh time.
- **Greedy merging.** 0fps: facets merge only when their AO matches at every vertex, and "along greedy
  edges longer than one voxel the AO values stay constant" (my reading of the post, which I read through a
  summariser, not line by line). My own derivation, which the builder should test rather than trust: two
  cells may merge along an axis only when all four AO values are equal **and** the cell's AO does not
  change along that axis (`a(u0,v) == a(u1,v)` for both `v`). Otherwise a merged strip of N cells would
  stretch one cell's gradient over N cells. Flat open floors (all 3) and uniform walls still merge
  fully; cells at a corner gradient stay one cell wide. The cost is more quads next to features; measure
  vertices against `MAX_WORLD_VERTICES` (12 M) with a stress scene.
- **Neighbour reach.** Cells at a chunk edge need the six neighbour chunks' cells (and edge/corner
  neighbours): the mesher already looks up face neighbours (`neighbours: [Option<&[u8]>; 6]`); AO needs the
  twelve edge and eight corner chunks too, or a conservative "missing = air" which leaves a visible seam
  until the neighbour arrives and the chunk is re-meshed. Say which; I would take the seam and re-mesh,
  since the streaming already re-meshes when a neighbour arrives (S4 J-items).
- **Cost.** 0fps: "a modest increase in mesh construction time"; at run time it is one vertex byte and a
  varying. Mesh-thread cost is not measured.
- **Limits.** The method assumes full cubes ([comment on the same post](https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/));
  Cavendish's world voxels are cubes. Voxel *bodies* drawn as trees (S3) are not greedy-meshed and would
  need their own path or none.

### 3.2 Far bricks: the same formula at the hit (recommended, layer 2)

Do not march extra rays. At the hit cell, for the hit face, take the four corners of the face on its open
side, evaluate `vertexAO` from the neighbouring occupancy bits (`cell_set`, three per corner, 12 reads
and fewer after sharing), and bilinearly interpolate by the hit point's position on the face (with the
same diagonal-flip rule so near and far agree). Properties: deterministic, no noise, no temporal history,
identical to the mesh's AO where the two levels meet, and in a coarse level the AO is of coarse cells, so
its reach grows with the level, which is what the eye expects at distance.

- Cells at a brick edge need the neighbouring brick: either treat outside as empty (a faint seam on
  brick boundaries, visible only at level seams) or look up the neighbour's bits (a brick table the
  renderer does not have today; bricks are drawn as boxes). I recommend outside = empty first and a test
  that measures the seam.
- Cost: a handful of dependent bit reads on top of a march that already does hundreds per pixel. Not
  measured.
- **Alternatives for the far field, rejected for now.** Ray or cone AO in the march: Teardown does
  large-scale AO with 2 cosine-weighted rays per pixel, up to 24 units, through its volumes, denoised with
  12 spatial samples and temporal blending ([Wittens, "Teardown Frame Teardown"](https://acko.net/blog/teardown-frame-teardown/),
  [Montoya's breakdown](https://juandiegomontoya.github.io/teardown_breakdown.html)). The developer's
  own earlier account of the engine ([Gustafsson, "From screen space to voxel space", 2018](https://blog.voxagon.se/2018/10/17/from-screen-space-to-voxel-space.html))
  gives five unsophisticated rays per pixel in semi-random hemisphere directions, traced with fixed steps (not a
  watertight walk, which he keeps for where exactness matters), each ray starting in screen space for the
  first short stretch and switching to the voxel volume if nothing is hit, because the voxel-space origin
  must be pushed off the surface to avoid self-intersection and loses fine detail. Denoising is "spatially
  blurring and temporally accumulating", fed by 2-D blue noise advanced each frame by the golden ratio
  ([Gustafsson, "The importance of good noise", 2018](https://blog.voxagon.se/2018/12/07/the-importance-of-good-noise.html)).
  That needs a history buffer, motion vectors and a denoiser; on one web thread and a 1660 it would be
  the single largest cost for a smoother effect than the data justifies.

### 3.3 Screen-space AO (GTAO), as an optional layer

Jimenez, Wu, Pesce and Jarabo, "Practical Real-Time Strategies for Accurate Indirect Occlusion" (2016,
Activision technical report, [PDF](https://www.activision.com/cdn/research/Practical_Real_Time_Strategies_for_Accurate_Indirect_Occlusion_NEW%20VERSION_COLOR.pdf);
read in full, equations checked against the page images):

- **Cosine weighting.** The AO integral is written "on its radiometrically-correct form", with the
  foreshortening term: `A(x) = (1/pi) int_0^pi int_{-pi/2}^{pi/2} V(phi, theta) cos(theta - gamma)^+ |sin theta| dtheta dphi`
  (eq. 5; `gamma` the angle between normal and view vector), unlike HBAO's uniform form. With binary
  visibility above two horizon angles per slice the inner integral is closed-form (eq. 7) and needs the
  normal projected into the slice plane (eqs. 8, 9). This is the same cosine-weighted AO that baked
  per-vertex AO approximates, so a GTAO layer and the baked layer measure the same quantity and can be
  compared against a ray-traced reference (their Figure 7).
- **Multi-bounce.** Two forms. Main text, section 4.2, eq. 12, from a Neumann series under uniform
  lighting and constant albedo: `L_r = L_i (rho / pi) A / (1 - rho (1 - A))`. Appendix B, eq. 33, a fit
  to simulations with up to three bounces over seven albedos:
  `G(A, rho) = a A^3 - b A^2 + c A`, `a = 2.0404 rho - 0.3324`, **`b = 4.7951 rho - 0.6417`**,
  `c = 2.7552 rho + 0.6903`. The minus sign is on the `A^2` term; written as Horner's rule,
  `((a A - b) A + c) A`. Check: `G(1, rho) = 0.0005 rho + 0.9996`, i.e. 1 at every albedo, whereas with
  `+b` it would be 0.68 at `rho = 0.1` and 8.3 at `rho = 0.9` (my arithmetic; a cheap unit test with a
  control arm). Figure 4: eq. 12 versus the fit, error below 10 % even at high albedo and low `A`.
- **Cost and sampling.** "0.5 ms on a PS4 at 1080p (for a standard halfres occlusion buffer)" (abstract,
  section 1). Section 4.3: half resolution, bilaterally upsampled; one direction per pixel with 12 steps
  in total, a 4x4 bilateral reconstruction with a 4x4 tiled noise, and 6 rotations over frames with
  reprojection into an exponential accumulation buffer, "4 x 4 x 6 = 96 effective sampled directions per
  pixel". So the published cost assumes temporal history; without it the noise is 16 directions.
  Sampling radius is scaled with camera distance to make AO view-independent and clamped near the camera
  to protect the cache (section 4.1); a thickness heuristic (eq. 13) limits thin-occluder over-darkening.

Intel's port, [XeGTAO](https://github.com/GameTechDev/XeGTAO) (archived; not one of the papers re-read):
High preset at full resolution, 3 slices x 6 samples, about 0.56 ms at 1080p on an RTX 2060, 1.4 ms at
2160p on an RTX 3070, 2.39 ms at 1080p on an integrated Intel GPU; Medium about two-thirds of High; bent
normals add about 25 %; it takes depth plus (advisably) normals, and a 5x5 depth-aware denoise. The RTX
2060 and the 1660 SUPER have similar memory bandwidth (336 GB/s, my recollection, unverified), so I
expect 0.6 to 1 ms at 1080p and 1.5 to 2.5 ms at 1440p here: an **estimate, not a measurement**.

Why it is not first: it needs a resolved depth and a normal source, so the renderer's MSAA path and the
`frag_depth` bricks both need checking (**unverified** how the current pass resolves depth); it ignores
anything outside the screen; it darkens by view, so a data view would show moving halos; and baked AO
already gives the voxel look. It earns a place for voxel *bodies* and the interiors of buildings if baked
AO does not reach them.

## 4. Shading

- **Model.** `colour = albedo * (sun_colour * max(n.l, 0) * shadow + ambient(n) * ao)`. Hemisphere ambient
  (sky colour above, ground bounce below, blended by `n.z` in Cavendish's z-up frame) is the usual cheap
  stand-in for sky irradiance; the existing constant `0.38 + 0.62 key` is a degenerate case of it.
  Normals are the six axis normals, flat faces, which is the voxel look; smooth normals would hide the
  voxel structure that a gravity-gradiometer scene should show honestly.
- **Linear and sRGB.** Do the sums in linear light and let the `*Srgb` target encode, as the renderer
  already does (`web-build-and-pages.md` section 2 lists `Rgba8UnormSrgb`; the palette uniform is linear
  RGB already). MSAA resolve of an sRGB target is correct in hardware; a later HDR target needs the
  tone mapper to do the encode.
- **Tone mapping.** Only needed once the sun exceeds 1.0 or an atmosphere makes HDR values. Options:
  [Narkowicz's ACES fit](https://64.github.io/tonemapping/) (cheap; "slightly oversaturates", per
  that page), a per-channel curve that shifts hue, or the
  [Khronos PBR Neutral](https://github.com/KhronosGroup/ToneMapping/blob/main/PBR_Neutral/README.md)
  mapper, designed so base colours are reproduced exactly in the 0.08 to 0.8 range and that "causes no hue
  shifts". For Cavendish the neutral one is the right default: it is the one that keeps a palette
  colour a palette colour. The data look uses no tone curve at all (identity).
- **Baked versus dynamic face lighting.** Sun direction is data and moves (time of day, later); do not
  bake sun into vertices. Bake only AO, which is static in the geometry. Everything else is per-fragment
  arithmetic on a uniform.
- **Cost.** Arithmetic only; far below noise on this card. Fog is section 6.

## 5. Shadows

### 5.1 Why not cascaded shadow maps first

CSMs split the frustum and render a depth map per split, then filter with PCF or variance maps
([Microsoft, Cascaded Shadow Maps](https://learn.microsoft.com/en-us/windows/win32/dxtecharts/cascaded-shadow-maps)):
four steps per frame, static split intervals to stop shimmering, a blend band between cascades, a depth
bias per kernel, and the "dueling frusta" case when the sun faces the camera. For Cavendish two problems
are specific. First, **the far field is not geometry**: bricks are boxes whose surface exists only inside
the fragment shader's march, so a shadow pass would have to run the march per cascade (the S4 numbers say
the march is where the frame time goes). Second, at 10^9 to 10^11 cells a depth map's resolution
(4096^2 in the article's words can be "insufficient" up close) does not cover the span between a tower and
a ridge a kilometre away without many cascades. CSM works on the web (a depth texture array and a
comparison sampler are core WebGPU) and is the fallback for voxel *bodies* if tracing them is too costly.

### 5.2 Ray-marched sun shadows through the grid (recommended, layer 4)

A voxel world has its own acceleration structure. Teardown casts "one ray per pixel" toward a random point
of the light, through a **volumetric shadow map of the whole play area**, 1-bit, a 2x2x2 block per 8-bit
texel, two extra mip levels so a ray skips empty 2x2x2 and 4x4x4 blocks, and a "super sparse" trace with
steps that double as the ray travels ([Wittens](https://acko.net/blog/teardown-frame-teardown/),
[Montoya](https://juandiegomontoya.github.io/teardown_breakdown.html); Wittens gives 262 MB for a
1752x100x1500 level). The developer's own description of the precursor engine
([Gustafsson, "From screen space to voxel space", 2018](https://blog.voxagon.se/2018/10/17/from-screen-space-to-voxel-space.html))
agrees and adds numbers: a 5 cm shadow volume over 100x100x25 m (about two billion voxels) at "one bit per
voxel", eight neighbours to a byte as octants of a 10 cm cell, four mip levels in all (the octant bits,
10, 20 and 40 cm), about 292 MB; a ray starts "in the lowest mip level" and "if it hits, switch[es] to
the larger ones progressively", i.e. coarse first, refining only where something is there; one ray per
pixel toward each light with the light jittered for soft shadows; moving objects re-voxelised on the CPU
and uploaded as sub-volumes; and "all ambient occlusion, lighting, fog and reflections in this scene in
about 9 ms, including denoising" at full HD with about ten lights, "Timings taken on a GTX 1080". He names memory and the CPU update
of large moving objects as the drawbacks, and notes that sharp shadows reveal the voxel grid. The 2020
design notes ([Gustafsson, "Teardown design notes"](https://blog.voxagon.se/2020/11/05/teardown-design-notes.html))
add one constraint: a level was cut to 400 m because of "a 3D texture size limitation on AMD graphics
cards". The earlier line here, a PC Games Hardware report on ray cost and hardware, was secondhand and
is dropped: none of the author's posts I read says it.

What that means for Cavendish: a whole-world volume is out of the question at 10^11 cells, but the
clipmap already organises the world around the camera. A **shadow volume per clipmap level** (say 3
levels of 128^3 bits = 256 KiB each, a 3-D texture or a storage buffer) centred and snapped as the levels
are, filled from the same bricks the far field uploads, lets a shadow ray walk fine cells near the pixel
and coarser ones as it goes. A 1-bit volume is also what the bricks are (32^3 bits), so this is a
re-pack, not new data. Per-frame cost is one DDA per pixel of up to about 64 to 128 hierarchical steps
(my guess; the sun leaves the volume in a bounded distance); run it as a compute pass over the depth
buffer, or in the fragment shader at the hit. **I have no measurement of this on any GPU.** It is the
main unknown, so the slice should measure it first at 1280x720 and 2560x1440 and cap it like the far
field's overdraw (S4's `MAX_BRICK_OVERDRAW` is the model).

- **Hard first, soft later.** The sun's angular diameter is about 0.53 degrees, so the true penumbra is
  about 1 cm per metre of occluder distance (my arithmetic): hard shadows are right at human scale.
  Teardown jitters the ray over the light and then blurs and temporally blends to hide the noise; that
  needs history and is the part to skip. A cheaper soft option later: 4 fixed rays in a rotated
  pattern per 2x2 quad, no history. If jitter is ever used, take it from a 2-D blue-noise texture
  mapped one texel per pixel with no filtering, advanced per frame by `fract(noise + 0.618034 * (frame % N))`
  (and the R2 sequence's irrationals per channel for vectors), as Gustafsson recommends; he also records
  a reader's warning that wrapping the frame count adds low-frequency error
  ([Gustafsson, "The importance of good noise"](https://blog.voxagon.se/2018/12/07/the-importance-of-good-noise.html)).
- **Cost anchor.** Gustafsson's 9 ms on a GTX 1080 at 1080p covers far more than one hard sun ray (AO,
  ten lights, fog, reflections, a denoiser), so it is an upper bound on the kind of pass, not a forecast
  for this one. The 0.5 to 2 ms estimate stands until measured.
- **Self-shadowing and terminators.** Start the ray one half cell out along the face normal and skip
  faces with `n.l <= 0` (they are dark already). Without this the first cell shadows itself.
- **Web.** Compute and storage buffers are core WebGPU; one thread is irrelevant, the CPU only uploads.
  8 storage buffers per stage is the constraint: the shadow volume must share an existing binding or be
  a 3-D texture (the current bind group already has palette, origins and bricks).
- **Bodies and moving things.** A body is its own volume in its own frame (REBUILD 5.2); shadows of
  bodies need either their own small volumes marched after the world's or a per-frame stamp into the
  near level. Unresolved; list under further questions.

## 6. Sky, atmosphere and distance

| Method | What it gives | Cost | Fit for Cavendish |
|---|---|---|---|
| Gradient + sun disc (analytic) | a plausible sky for the data/figure looks | a few ALU | now; also gives the fog colour |
| Preetham 1999, Hosek-Wilkie 2012 ([page](https://cgg.mff.cuni.cz/projects/SkylightModelling/), [paper](https://cgg.mff.cuni.cz/projects/SkylightModelling/HosekWilkie_SkylightModel_SIGGRAPH2012_Preprint_lowres.pdf)) | an analytic sky-dome radiance: a modified Perez function with nine parameters A to I (eq. 8), each Bezier-interpolated in solar elevation from fitted tables, per spectral band or per CIE XYZ channel; turbidity 1 to 10, ground albedo 0 to 1, sun from 0 to 90 degrees elevation (section 5); fitted to a path tracer with the observer at sea level (section 3) | **no timing in the paper**: "slightly more costly to evaluate than the Preetham model ... could be alleviated e.g. by pre-computation of sky maps" (section 8); per pixel it is a few `exp`/`cos`/`pow` per channel | ground views only, confirmed: no aerial perspective, no view from altitude or space, and "it cannot recreate the earth casting a shadow onto the atmosphere" after sunset (section 8) |
| Bruneton and Neyret 2008 ([paper](https://core.ac.uk/download/pdf/50255743.pdf), [implementation](https://ebruneton.github.io/precomputed_atmospheric_scattering/)) | precomputed single and multiple scattering, ground to space, Rayleigh and Mie, ground reflection, terrain shadows and light shafts (abstract); transmittance 64x256 and irradiance 16x64 2-D tables, a 32x128x32x8 4-D scattering table packed as one 32x128x256 RGBA 3-D texture, 8 MB at 16-bit (section 6) | precompute "5 scattering orders ... in 5 seconds on a NVidia 8800 GTS" (section 6); run time "a few texture fetches per pixel (< 10)", 125 fps at 1024x768 on that card without light shafts, 25 fps with them, the shafts' two passes costing about 32 ms (section 6). Hillaire measures the 2017 code: 0.22 ms to render at 720p but 250 ms to update all LUTs (Hillaire, section 7) | not chosen: one 4-D table per atmosphere, a slow update when the atmosphere changes, and its light shafts need shadow volumes extruded by a geometry shader (section 6), which WebGPU lacks and Cavendish's ray-marched far field cannot supply |
| Hillaire 2020 ([paper](https://sebh.github.io/publications/egsr2020.pdf), [slides](https://blog.selfshadow.com/publications/s2020-shading-course/hillaire/s2020_pbs_hillaire_slides.pdf), [code](https://github.com/sebh/UnrealEngineSkyAtmosphere)) | four small LUTs, all re-rendered every frame: transmittance (Bruneton's 2-D parameterisation, section 5.2), sky-view lat-long LUT with a quadratic squeeze toward the horizon (section 5.3, the sun disc composited after), aerial-perspective froxel volume, in-scatter RGB plus mean transmittance A, "32 x 32 over the screen and 32 depth slices over a depth range of 32 kilometers" (section 5.4), and a 2-D multiple-scattering LUT giving all orders at once by `1/(1 - f_ms)` (section 5.5, eq. 9); ground and space; 16-bit floats are enough (section 6) | **Table 2, GTX 1080:** transmittance 256x64, 40 steps, 0.01 ms; sky-view 200x100, 30 steps, 0.05 ms; aerial perspective 32^3, 30 steps, 0.04 ms; multiple scattering 32^2, 20 steps, 0.07 ms. "The final on-screen rendering of the sky and atmospheric perspective is 0.14 milliseconds" and "the total render time is 0.31 ms for a resolution of 1280 x 720" (section 7). From space it switches to per-pixel ray marching: 0.33 ms, 0.5 ms total. iPhone 6s rows: 0.53, 0.27, 0.11, 0.12 ms. Volumetric shadows (mountains on the air) need a separate ray march with shadow maps: 32 samples raise the sky to 1.0 ms (section 7). **No 1080p figure in the paper** | **the choice** once the planet exists |

[A WebGPU port exists](https://github.com/JolifantoBambla/webgpu-sky-atmosphere) (a port of Hillaire's MIT-licensed
demo; the aerial-perspective LUT is a frustum volume, example size 64x64x32; it needs `float32-filterable`
for an `rgba32float` transmittance LUT and `timestamp-query` only for its timings; Rayleigh, Mie and
ozone layers; the planet centre is placeable in world space; distance scale 1 unit = 1 km by default).
Third-party timings for a GTX 1060 DirectX 11 port: sky-view 64x64 at 0.036 ms and the aerial LUT 64x64x32
at 0.03 ms (a search summary of that repository's README, **not read directly**). Hillaire's paper says a
16-bit float representation "is enough" for his model (section 6, against 32-bit needed for Bruneton's
iterative one), so Cavendish can store the LUTs as `rgba16float`, which core WebGPU filters, and drop the
`float32-filterable` requirement.

**Scaling Table 2 to Cavendish (my arithmetic, not a measurement).** The four LUTs (0.17 ms on a GTX
1080) do not depend on screen size. The on-screen 0.14 ms at 720p is per pixel; if it scaled with pixel
count it would be about 0.32 ms at 1080p and 0.56 ms at 1440p, so the whole sky about 0.5 ms at 1080p on a
1080. The 1660 SUPER is a slower card than the 1080; expect somewhat more. The aerial-perspective default
(32 km deep) covers a ground view of the AION site; from altitude or space Hillaire switches to per-pixel
marching (0.33 ms on screen).

**What an atmosphere needs for a planet world.** Only these (all from Hillaire's and the port's
parameters): planet radius and atmosphere top height; Rayleigh, Mie and absorption coefficients and scale
heights (data: an `Atmosphere` record, so Mars or an airless Moon are rows, not code); the planet centre in the
camera-relative frame (the renderer already works in offsets from a target, REBUILD 3.4, which is what an
atmosphere wants: the camera's altitude is `|eye - centre| - R`); a ground albedo for the multiple-scattering
LUT; the sun direction and illuminance. A flat world with no planet can use the same model with a very
large `R`. Float precision: at Earth radius, altitudes are differences of 6.4e6 m values, so the LUT
parameterisation uses altitude and zenith angle, not world positions: do the subtraction in f64 on the
CPU (as chunk origins already are) and pass the camera's altitude and up vector.

**Fog and aerial perspective.** The cheap version, `colour = mix(sky_horizon, colour, exp(-d * density))`
with density as data, is layer 3 and needs only the hit distance (the march knows `t`; meshes know depth).
It hides the LOD transition at the outer ring, and Teardown's fog "depends only on view direction", which
the same page calls "a fair amount cheaper" ([Wittens](https://acko.net/blog/teardown-frame-teardown/)).
Hillaire's aerial-perspective LUT replaces it with the physically correct in-scattering and
transmittance per distance, tinted blue-ward with distance, from the same data; it is then the same
code path with different numbers, so build the fog's interface (a distance in, a `(inscatter,
transmittance)` out) as that LUT's interface.

## 7. Voxel colouring (Cube World-style)

**What I could and could not establish.** I found no primary source for Cube World's internals: its
developer's posts on the [TIGSource thread](https://forums.tigsource.com/index.php?topic=19952.0)
returned HTTP 403 to my fetcher **(not read)**, and a search summary of it says only that each voxel carries
its own colour so the landscape "works like a low-resolution texture", that the style is untextured,
and that the scene is chunks of polygons drawn with OpenGL **(secondhand)**. The developer's current
patch notes ([Wollay, "0.1.12: Lighting, Spirit Cubes and Editor Controls", 24 October 2026](https://wollay.com/patch-notes/2026-10-24-0-1-12-lighting-spirit-cubes-and-editor-controls/),
read directly) name only adjustments: bloom and lighting refined so glow and reflections read more
strongly while keeping lit surfaces from washing out, sand and snow colours tuned, and a fix for
"colored bands appearing across sand near shaded edges". They describe no method. The Reddit thread on
colour tweaking listed in the index could not be fetched (reddit.com refuses automated readers) **(not
read)**. So what follows is the general technique voxel games of that kind use, derived from the sources
above and ordinary practice, and stated as Cavendish's own choices, not as a description of that game.
One general lesson from that fix, mine: smooth shading gradients across a flat-coloured face band visibly
in 8-bit output, so the golden-image test (section 10, test 6) should include a long, gently shaded
face and check for banding.

1. **Colour is per voxel data, not texture.** The voxel's palette entry gives the albedo; the mesh carries
   the colour index (`material - 1`, today) or the final colour.
2. **Soft per-vertex AO over flat faces** (section 3.1): the dark creases that make blocks read as shapes
   without any texture. This is most of the look.
3. **Subtle per-voxel variation.** A deterministic hash of the voxel's integer coordinates (and level, so
   it is stable under LOD) perturbs the palette colour by a few percent in lightness and less in hue:
   `albedo * (1 + j * (hash(x, y, z) - 0.5))`, `j` per material (data). It must be a pure function of the
   integer cell and a seed so it is identical in the mesh and the march, and identical after a re-mesh.
   At coarse levels the hash of the coarse cell is a fixed higher-contrast pattern: attenuate `j` by
   the level so distant terrain does not sparkle (my recommendation; to be judged by eye).
4. **Smooth gradients across terrain.** Colour by height and slope blends two palette entries:
   `t = smoothstep(h0, h1, height) * slope_term`, evaluated at the cell, not per fragment, so the
   gradient is voxel-stepped but broad. Grass on top faces, rock on steep, snow high. With per-vertex
   evaluation the steps soften across a face; with per-cell evaluation they stay crisp. Choose per
   material (data).
5. **How this fits Cavendish's palette.** The planned S19 per-voxel material has density, colour, elastic
   and thermal properties; the **colour is a property of the material, the look rules (jitter amount,
   height/slope blend, two-colour ramps) are fields beside it**, in the same schema, none feeding the
   physics (principle 3 and 4: the gravity read uses density only). The stored world chunk keeps
   `material` as one byte per cell; the colour variation is **computed at draw time from the cell
   coordinates**, never stored, so the 10^11-cell world pays no memory for it.
6. **The data caveat (REBUILD section 6).** When a field colours the voxels, none of 3 or 4 applies; the
   field's colour map is the albedo (see section 8).

## 8. Beauty and data are separate looks, defined as data

A `LookSpec` (one record, loaded and saved like a layout, principle 11), fields in bold are the switches:

```
look "beauty"                      look "data"  (default whenever a field colour is on)
  albedo      : material palette     albedo      : the colour map's colour, exactly
  jitter      : per material         jitter      : 0
  ao          : baked 0..1 strength  ao          : 0   (identity)
  sun         : direction, colour    sun         : none (flat), or the existing 0.38+0.62 key
  shadow      : off | hard           shadow      : off
  ambient     : hemisphere           ambient     : 1.0
  fog/sky     : on, density, colour  fog/sky     : off, background as today
  tone        : neutral | none       tone        : none (identity)
```

Rules that keep data honest:
- Beauty terms are **multiplicative on albedo and never on the field's value**; a data look has every
  multiplier at 1 and no tone curve, so the on-screen colour equals the legend's colour exactly (checkable
  by reading back pixels, section 10).
- The legend (`cav-plot`) is generated from the field, not from the lit result; the beauty look must not be
  active while a legend is shown unless the user asks, and then a badge says "lit: colours shifted"
  (the renderer already shows a badge for each cap it reaches; same mechanism).
- **A mild AO may remain in a data view as a separate, opt-in `shape` layer** (AO of strength 0.25 does
  not change hue, only lightness), because without any cue a field-coloured voxel world reads as flat
  paint. That choice is the owner's; the default is none.
- **Detail tiers.** `preview`: baked AO and a single sun, no shadows, no fog LUT; `standard`: + hard
  shadows and the fog; `reference`/screenshot: + GTAO if it exists, soft shadows, atmosphere LUTs. Defined
  in the tier data, as S19's tiers are, by naming a `look`.

## 9. Budget on this card and on the web

Not measurable here (no builds, the owner is using the PC). The frame is 19 to 33 ms already (S4 section
7.4), the 99th percentile past 50 ms, so the target is **beauty total at most 1.5 ms at 720p** to be on
by default on the 1660 SUPER, and everything beyond is tiered. Estimates, all to be replaced by the
slice's own numbers via `cav bench`:

| Layer | 1660 SUPER, 720p | 1440p | Web (WebGPU) |
|---|---|---|---|
| Baked AO, shading, fog | under 0.1 ms; mesh-thread time up | the same | the same; no extra bindings |
| Hit-point AO in the march | under 0.3 ms | under 1 ms | the same shader; reads more of the brick buffer |
| Ray-marched sun shadow | 0.5 to 2 ms (unmeasured) | 2 to 6 | compute is core; watch the 8 storage-buffer limit; Chrome on Windows is the likely device |
| Hillaire sky, 0.31 ms total on a GTX 1080 at 720p (Table 2) | 0.3 to 0.5 | 0.6 to 0.9 (LUTs 0.17 ms fixed; on-screen part scales with pixels) | 16-bit LUTs, which the paper says suffice; no `float32-filterable` needed |
| GTAO | 0.6 to 1 (at 1080p) | 1.5 to 2.5 | needs depth plus normal textures; a resolve per MSAA sample; the paper's 0.5 ms assumes half resolution and temporal history |

The first rows are the only ones I would build without measuring; the shadow row decides the slice that
builds it, and the rule from `plan/` and the owner's desktop freeze applies to it as to the far field:
**a cap per frame, said as a badge when reached**, and the bench's `--max-frame-ms` alarm.

## 10. Tests (control arms included)

Each looks like S4's R-rows. A property test with a control that fails when the property breaks:

1. **AO values on known configurations (CPU, exact).** A single cube on a plane; a straight wall's
   concave corner; a one-cell step; a flat floor. Expected `vertexAO`: floor far from walls all 3; the
   floor vertex at a wall foot 2; at a concave corner (both sides plus the corner) 0; a convex edge 3
   on both faces; a step's lip as the 0fps table gives. Control: swap `side1 && side2` for `side1 || side2`
   (or drop the corner term) and the corner case fails.
2. **Merging preserves AO.** For random worlds the per-cell AO reconstructed from the merged quads equals
   the unmerged per-cell AO within interpolation at the cell centres; control: merging on material alone
   (the old key) fails on a corner. This is the property section 3.1's derivation claims, so it needs
   the test most.
3. **Diagonal flip.** A quad with AO `(3,3,3,0)` and one with `(0,3,3,3)` must flip; control: no flip gives
   a different pixel row sum along the other diagonal.
4. **Near and far agree.** The march's AO at the centre of a face's cell equals the mesh's AO bilinear at
   the same point, to a quantisation step, for 1,000 random faces; control: without the diagonal rule
   the far field differs on a skewed corner.
5. **GPU/CPU twin.** The hit AO is in the shared WGSL function with a CPU twin (as `march` has,
   R11), compared on the same rays.
6. **Golden images with tolerance.** `town` and a small `corner-and-step` scene, rendered at 480x270
   with a stated per-pixel and mean tolerance (so a driver change does not break it), with the beauty look
   on and **the same view with each layer turned off**, which must differ from the golden by more than
   the tolerance (the control arm) and must equal the old unlit render for the data look to the bit.
   Include one long, gently shaded face and bound the largest run of equal 8-bit values along it, so
   gradient banding is caught (section 7).
7. **Data stays honest.** In the data look, a pixel read back from a face coloured by a field equals the
   colour map's output at that value, within one 8-bit step; control: with beauty on it must not.
8. **Shadow sanity.** A pillar casts a shadow of the geometrically expected length on a plane
   (`h / tan(elevation)`), a point in the shadow reads `shadow = 0`, one outside reads 1; control: a
   shadow volume shifted by one cell moves the edge one cell. Terminator: faces away from the sun have no
   self-shadow acne (count shadowed lit-facing fragments on a flat floor: 0).
9. **Budget tests.** The shadow pass's work per frame obeys its cap; the badge appears when it binds
   (the S4 pattern, `gpu_meshes_past_the_vertex_cap_are_not_drawn`).

## 11. Build order

1. **Slice A (about a day):** `LookSpec` as data with only `beauty`/`data` and the existing sun; AO byte
   in the vertex and merge key; hit AO in the march; hemisphere ambient; tests 1 to 5, 7. Measure.
2. **Slice B:** fog and analytic sky background, the neutral tone curve; golden images; the badge for
   "lit view".
3. **Slice C:** shadow volume per clipmap level and the shadow pass, with its cap; test 8; measurement
   decides on or off by default per tier.
4. **Slice D (with the planet, S14's far side):** the `Atmosphere` record and Hillaire's LUTs, aerial
   perspective replacing the fog; time of day.
5. **Later:** GTAO for bodies, soft shadows, per-material colour rules in the S19 material editor.

## 12. What I could not verify, and how to resolve it

- **Every millisecond on the 1660 SUPER and on WebGPU.** Resolve: a `bench` row per layer in the slice
  (the S4 bench already records mean, 99th percentile and worst).
- **The ray-marched shadow's cost and whether the 128^3-per-level volume is enough.** Resolve first in
  slice C, with a throwaway compute kernel before the integration.
- **Whether merged AO is exact** (my derivation in 3.1, from the 0fps statement): test 2.
- **Cube World's actual technique:** no primary source describes it (the 2026 patch notes name tweaks
  only; the Reddit thread could not be fetched). Do not cite it as a method; the section 7 recipe stands
  on its own.
- **Hillaire at 1080p and on this card:** the paper gives 720p on a GTX 1080 only (Table 2); the 1080p and
  1440p figures in section 9 are my pixel-count scaling. Resolve with the slice's bench.
- **Hosek-Wilkie's run-time cost:** the paper gives no timing. Irrelevant unless Hillaire proves too slow.
- **The WebGPU port's GTX 1060 timings:** still from a search summary of its README.
- **GTAO's cost on this card** at full and half resolution, and without temporal history. (The `b` sign is
  settled: section 3.3.)
- **Whether the current MSAA path leaves a depth a screen-space pass can read** (read `renderer.rs`).

## 13. Further questions (not chased)

- Shadows and AO for moving voxel bodies against the world volume (one volume per body, REBUILD 5.2).
- Water and fluids in the lighting (S8 to S10): absorption, caustics are out of scope here.
- An interior-light model (point lights, emissive voxels) and the Minecraft-style propagated light
  REBUILD 5.3 mentions: not needed for the sun-lit look.
- Screenshots and the paper style: the paper draw style's lighting (none) and the beauty look's relation
  to the figure pipeline.
- Temporal accumulation (TAA) as a shared cost for soft shadows and GTAO, and its interaction with the
  data look.

## Sources

- 0fps, [Ambient occlusion for Minecraft-like worlds](https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/) (read through a summariser).
- Jimenez, Wu, Pesce and Jarabo, "Practical Real-Time Strategies for Accurate Indirect Occlusion", Activision technical report, 2016, [PDF](https://www.activision.com/cdn/research/Practical_Real_Time_Strategies_for_Accurate_Indirect_Occlusion_NEW%20VERSION_COLOR.pdf) (read: eqs. 5 to 13, section 4.3, Appendix B eq. 33; also at [iryoku.com](https://www.iryoku.com/downloads/Practical-Realtime-Strategies-for-Accurate-Indirect-Occlusion.pdf)); Intel [XeGTAO](https://github.com/GameTechDev/XeGTAO).
- Wittens, [Teardown Frame Teardown](https://acko.net/blog/teardown-frame-teardown/); Montoya, [Teardown breakdown](https://juandiegomontoya.github.io/teardown_breakdown.html).
- Gustafsson, [From screen space to voxel space](https://blog.voxagon.se/2018/10/17/from-screen-space-to-voxel-space.html) (2018), [The importance of good noise](https://blog.voxagon.se/2018/12/07/the-importance-of-good-noise.html) (2018), [Teardown design notes](https://blog.voxagon.se/2020/11/05/teardown-design-notes.html) (2020): read directly.
- Wollay, [0.1.12: Lighting, Spirit Cubes and Editor Controls](https://wollay.com/patch-notes/2026-10-24-0-1-12-lighting-spirit-cubes-and-editor-controls/) (24 October 2026): read directly. [Reddit colour thread](https://www.reddit.com/r/CubeWorld/comments/1c7wgow/wollay_on_twitter_more_progress_tweaking_color/): not fetchable.
- Microsoft, [Cascaded Shadow Maps](https://learn.microsoft.com/en-us/windows/win32/dxtecharts/cascaded-shadow-maps).
- Hillaire, "A Scalable and Production Ready Sky and Atmosphere Rendering Technique", Computer Graphics Forum 39(4), EGSR 2020, [author PDF](https://sebh.github.io/publications/egsr2020.pdf), [EG page](https://diglib.eg.org/handle/10.1111/cgf14050) (read: sections 5 and 7, Table 2 from the page image); [slides](https://blog.selfshadow.com/publications/s2020-shading-course/hillaire/s2020_pbs_hillaire_slides.pdf); [code](https://github.com/sebh/UnrealEngineSkyAtmosphere); [WebGPU port](https://github.com/JolifantoBambla/webgpu-sky-atmosphere).
- Bruneton and Neyret, "Precomputed Atmospheric Scattering", Computer Graphics Forum 27(4), EGSR 2008, [open PDF](https://core.ac.uk/download/pdf/50255743.pdf), [publisher](https://onlinelibrary.wiley.com/doi/abs/10.1111/j.1467-8659.2008.01245.x) (read: sections 4 to 6); Bruneton, [reference implementation](https://ebruneton.github.io/precomputed_atmospheric_scattering/).
- Hosek and Wilkie, "An Analytic Model for Full Spectral Sky-Dome Radiance", ACM TOG 31(4), SIGGRAPH 2012, [project page](https://cgg.mff.cuni.cz/projects/SkylightModelling/), [low-resolution preprint](https://cgg.mff.cuni.cz/projects/SkylightModelling/HosekWilkie_SkylightModel_SIGGRAPH2012_Preprint_lowres.pdf) (read: sections 3, 5, 7, 8).
- Narkowicz and Hill, via [64's tone mapping notes](https://64.github.io/tonemapping/); [Khronos PBR Neutral](https://github.com/KhronosGroup/ToneMapping/blob/main/PBR_Neutral/README.md).
- [WebGPU specification, limits](https://www.w3.org/TR/webgpu/#limits).
- In-repo: `docs/slices/S4.md`, `docs/slices/S3.md`, `docs/research/web-build-and-pages.md`, `plan/REBUILD.md` sections 2, 3.4, 5.2, 6, `crates/cav-render/src/world.rs`, `world_render.wgsl`, `world_common.wgsl`.
