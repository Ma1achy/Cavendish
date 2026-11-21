# M18 — The scene library (plan / implementation brief)

> **Status: planned — nothing here is built.** Depends on: M15 (scenes are `SceneSpec` files), M14
> (trees, level of detail, fluids) for the scenes that need them, M17 (populations). The standard
> shapes and the simple outdoor scenes need only what exists today plus the spec.
>
> **Goal.** A catalogue of ready scenes, from a single point to a building full of people and a river
> outside — each one a file, each one runnable from the GUI, the console and Python, each one there
> because it exercises a named capability. A general simulator is only as convincing as the range of
> things it has demonstrably simulated.

---

## 1. Rules for a library scene

1. It is a `scenes/<family>/<name>.ron` spec with documented parameters and sensible distributions
   for the dataset builder — never Rust code.
2. It has a test: it builds, runs, conserves mass, decomposes exactly, and its peak signal falls in
   a stated range.
3. It states what is physical and what is a stand-in (a procedural gait, a placeholder spectrum).
4. It appears in the viewer's library with a thumbnail rendered headlessly from the scene itself.

## 2. How big is the signal? — a rough guide

Far from a tower a gradiometer's phase falls roughly as mass over distance cubed. Anchoring on the
recorded walker (70 kg, 0.6 rad at 3.4 m) gives about 0.34 rad·m³/kg. This is an order-of-magnitude
guide from one scene — direction, height and sign matter, and it fails within a couple of metres —
but it reproduces the collaboration's own small-animal figure to a factor of two (0.25 kg at 15 m:
25 µrad here, ~50 µrad published). Shot noise is 250 µrad–1 mrad per shot.

| Source | Mass, distance | Rough peak phase | Against shot noise |
|---|---|---|---|
| person in the room | 70 kg, 2 m | ~3 rad | enormous |
| person in the next room | 70 kg, 5 m | ~0.2 rad | large |
| car on a road | 1.5 t, 20 m | ~60 mrad | large |
| lorry | 20 t, 30 m | ~250 mrad | large |
| drone | 5 kg, 10 m | ~2 mrad | visible |
| bird | 1 kg, 5 m | ~3 mrad | visible close in |
| light aircraft, low pass | 1 t, 150 m | ~0.1 mrad | under |
| airliner overhead | 70 t, 1 km | ~25 µrad | well under |
| boat (see §4) | 5 t, 30 m | ~1 mrad, not 60 | visible only close |

So "planes over a field" is mostly a *null* scene — worth having precisely because a tracker must
learn that some things are invisible — and its test asserts the smallness.

## 3. The catalogue

### 3.1 Standard bodies — the unit tests of the library

One scene per shape, at rest, on a straight pass, and tumbling freely. All are convex or simple, so
each gets an exact or conservative `Solid::cover` and voxelises top-down.

| Shape | Status | Note |
|---|---|---|
| point; sphere; spherical shell; cuboid; cylinder | exist | the v1 anchors |
| **tube** (hollow cylinder); **cone**; **capsule**; **torus**; **ellipsoid** | to add | closed-form occupancy; analytic mass and inertia as the oracle |
| **convex polyhedron** — one type, from a list of half-spaces | to add | covers the **pyramid**, **prism**, and all five **Platonic solids** (tetrahedron, cube, octahedron, dodecahedron, icosahedron) as constructors; inertia from the standard polyhedron formulae |
| union, difference, translated, rotated, scaled | union and translated exist | constructive solid geometry over any of the above |
| mesh (STL / OBJ / glTF) | exists (M10) | anything else |

Why the Platonic solids earn a place: by symmetry the tetrahedron and upward have no quadrupole, and
the dodecahedron and icosahedron none below `l = 6`, so they are sharp tests of exactly the multipole
orders the tree walk truncates at.

### 3.2 Indoors

| Scene | What moves | Needs |
|---|---|---|
| `room/pass` | one body (any standard shape, or the walker) crossing the array | exists |
| `room/people` | several walkers with different gaits, masses and paths, crossing and stopping | populations (M17), multi-source bundles |
| `room/dropped-object`, `room/stack` | boxes falling, bouncing, coming to rest, stacks | voxel contact (M14 E) |
| `room/radiator`, `room/door-opens` | convection from a heat source; a door swinging and the cold air it lets in | fluid (exists); open boundary |
| `room/machinery` | a rotating unbalanced mass; a vibrating plate | exist |
| `building/stairwell` | the Beecroft scene: stairs, foyer, lift | M17 |
| `building/floors` | people on several floors around a tower that spans them; a lift with its counterweight | populations; static structure as trees |
| `building/water` | a cistern filling and draining; water moving in pipes | liquids (M14 H); a pipe is a prescribed-flow source (§4) |

### 3.3 Outdoors

An outdoor array of towers on open ground, with terrain as a static tree body from a height field.

| Scene | What moves | Needs |
|---|---|---|
| `field/aircraft` | an airliner at altitude, a light aircraft on a low pass | rigid bodies on long straight paths; expected: near-null |
| `road/traffic` | cars, vans and lorries as a population on a road at 10–100 m | populations; `Distance` level of detail |
| `air/birds` | a flock: point or small-ellipsoid bodies on smooth random paths, flapping ignored | populations; curved paths |
| `air/drones` | hover, transit and orbit; payload pick-up changes the mass mid-flight | a path with dwell; a mass that steps (two sources handing over) |
| `water/boats` | hulls moving on still water | the displaced-water treatment of §4; later, liquids with two-way coupling |
| `water/river` | steady flow, then a flood wave or a changing level | prescribed-flow source; the level change is the signal (§4) |
| `water/waterfall` | a falling, aerated sheet and a turbulent plunge pool | liquids (M14 H); or a stochastic density field as a stand-in, labelled |
| `ground/seismic` | no bodies; Rayleigh waves | M17 |
| `weather/wind`, `weather/front` | advected temperature structure; a pressure front | the analytic atmospheric field (exists) and the fluid with open boundaries |

### 3.4 Mixed and adversarial — for training

Scenes built to break a tracker: two bodies whose signals cancel at one tower; a heavy far source and
a light near one with the same peak; a body that stops; a body that crosses directly over a tower
(sign flips); the same pass with and without a nearby lorry; daytime clutter around a quiet target.

## 4. Physics that the water scenes need stated

- **A floating boat is almost invisible.** It displaces its own weight of water, so the net mass
  change is zero; what remains is the offset between the boat's mass and the water it replaced — a
  dipole and higher, falling one power of distance faster and smaller by roughly (hull depth) /
  (range). A boat modelled as its mass alone would be wrong by one to two orders of magnitude. Until
  liquids exist, a boat is a body plus a **negative-density copy of its submerged volume**
  (the same device as `Parcels::in_fluid` for air), which is exact for still water.
- **Steady flow gravitates statically.** A river running at constant level is the same mass in the
  same place at every instant: a constant offset, no signal. What the instrument sees is *change* —
  level, a flood wave, surface waves, aeration. `water/river` is therefore a scene about level and
  waves, with the steady case as a null test.
- **A waterfall is a density fluctuation problem.** Aerated falling water and a turbulent pool have
  fluctuating bulk density; the honest version needs the liquid solver. The stand-in is a stochastic
  density field with a stated spectrum, flagged as a placeholder in the manifest.
- **Scale.** Outdoor scenes span hundreds of metres to kilometres while bodies near a tower may be
  voxelised at centimetres. Positions are `f64` in the world; every body has its own tree in its own
  frame; terrain and water are trees refined by the response (fine near towers). No single global
  grid ever has to span the scene.

## 5. Requirements

| ID | Requirement |
|---|---|
| M18-R1 | The shapes of §3.1 as `Solid`s with `cover`, analytic mass and inertia, and a spec constructor each. |
| M18-R2 | Paths the catalogue needs beyond v1's: polyline with rounded corners (M17), smooth random (band-limited) paths, hover/dwell, orbit about a point, and a path sampled from data. |
| M18-R3 | Terrain from a height field as a static tree; a flat-water body with a level that may vary in time. |
| M18-R4 | The displaced-volume device for floating and immersed bodies, for air and water alike. |
| M18-R5 | Every scene of §3.2–3.4 as a spec file meeting the four rules of §1; scenes whose engine support is not built yet are listed with the phase that unblocks them, not faked. |
| M18-R6 | A library index (`scenes/index.ron`): name, family, description, parameters, required capabilities, thumbnail. The viewer's library, the console's `ls scenes` and Python read the same index. |

## 6. Tests

| Level | Test | Asserts | Tol |
|---|---|---|---|
| unit | `shape_mass_and_inertia_match_analytic` | every new shape, voxelised at three pitches, converges to the closed form | rate ≥1.8; ≤1e-3 at finest |
| unit | `cover_is_conservative_for_every_shape` | random boxes | exact |
| unit | `platonic_solids_have_no_low_multipoles` | quadrupole of all five; `l` = 4 of the dodecahedron and icosahedron | ≤1e-10 of `M R^l` (analytic); voxelised: falls with pitch |
| unit | `floating_body_has_zero_net_mass` | hull + displaced copy sums to zero; far field falls one power faster than a monopole | ≤1e-12 / exponent −4 ± 0.15 |
| unit | `steady_river_is_silent` | constant level and flow: signal constant in time | ≤1e-12 |
| unit | `river_level_change_is_the_signal` | a level step produces the phase of the added slab | ≤1e-9 |
| unit | `terrain_tree_conserves_volume` | height field → tree volume | ≤1e-12 |
| integration | `every_library_scene_runs` | builds, runs, conserves mass, decomposes exactly | ≤1e-10 |
| integration | `signal_guide_holds` | each row of §2's table within a factor of ten, sign aside; the airliner under shot noise | factor 10 |
| integration | `adversarial_pairs_are_what_they_claim` | the cancelling pair cancels at the named tower; the matched-peak pair matches | 5% |
| e2e | `library_thumbnails_render` | one non-empty headless frame per scene | structural |

## 7. Exit requirements

| Requirement | Check |
|---|---|
| the standard shapes are complete and correct (R1) | `shape_mass_and_inertia_match_analytic`, `platonic_solids_have_no_low_multipoles` |
| water is modelled as water (R3, R4) | `floating_body_has_zero_net_mass`, `steady_river_is_silent`, `river_level_change_is_the_signal` |
| the catalogue exists and runs (R5, R6) | `every_library_scene_runs`; the index lists every scene with its status |
| the guide is honest | `signal_guide_holds` |

## 8. Build order

Shapes and the simple outdoor scenes first (aircraft, traffic, birds, drones: rigid bodies on paths —
they need only M15's spec). Boats with the displaced-volume device next. Rooms with contact after
M14 E. The river's level and waves, the cistern and the waterfall after M14 H. The stairwell with M17.

## 9. Open questions

- Whether flapping, rotor wash and wakes ever matter (a bird's wing mass is grams at metres: no; a
  boat's wake is a surface-wave problem and might, close in).
- An outdoor array's layout — nothing in the sources describes one; it is a design choice for Gradar.
- Human bodies beyond the procedural walker: skinned mesh import with real gaits is M12's open item.
