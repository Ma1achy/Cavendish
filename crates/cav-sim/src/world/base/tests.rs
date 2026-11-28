//! The infinite bases held to their closed forms (`docs/slices/S5.md` §3, B1–B3): the empty plane
//! reads S1's slab and no gradient, departures on a plane read as the prisms they are, a planet
//! reads as its layers and a box sunk into it differs from the plane's as `1/R`; the merge of the
//! two voxelisations is the cells added; the walk stays within its ledger on signed masses.

use super::*;
use crate::SceneSpec;
use crate::read::Readers;
use crate::world::tests::{boxed, scene};
use crate::world::{LodSpec, MaterialSpec, PieceSpec, StreamSpec, World, WorldReadSpec, WorldSpec};
use cav_core::forward::{PHASE_TOLERANCE, Reader, Reading};
use cav_core::voxel::kernel::prism_potential;
use cav_core::voxel::world::{ChunkKey, Piece, raster_chunk};

/// Concrete for buildings, air for ditches.
fn materials() -> Vec<MaterialSpec> {
    vec![MaterialSpec::plain("concrete", 2400.0, "#b2b0a8"), MaterialSpec::plain("air", 0.0, "#202838")]
}

/// The plane of B1 and B2: rock 8 m thick, its top at `surface`.
const PLANE: (f64, f64) = (2000.0, 8.0);

fn plane(surface: f64) -> BaseSpec {
    BaseSpec::Plane { surface, density: PLANE.0, thickness: PLANE.1 }
}

/// A world on `base` of `pieces` (concrete is material 0, air 1) at `pitch`, sampled `k`.
fn world(base: BaseSpec, pitch: f64, k: u8, pieces: Vec<PieceSpec>) -> WorldSpec {
    WorldSpec {
        pitch,
        supersample: k,
        origin: [0.0; 3],
        materials: materials(),
        pieces,
        stream: StreamSpec::default(),
        read: WorldReadSpec::default(),
        lod: LodSpec::default(),
        base,
    }
}

/// A box of `material` from its corners.
fn piece(label: &str, material: usize, lo: [f64; 3], hi: [f64; 3]) -> PieceSpec {
    PieceSpec { label: label.into(), material, ..boxed(lo, hi) }
}

/// Every tower of `spec` reading its world at readout 0.
fn reads(spec: &SceneSpec) -> Vec<crate::world::TowerRead> {
    assert_eq!(spec.validate(), Ok(()));
    let readers = Readers::new(spec);
    let readouts = vec![0.0; spec.array.towers.len()];
    crate::world::read_world(spec, &readers, &readouts, &|| false).unwrap().unwrap().towers
}

/// The gradiometer, lower minus upper.
fn gradiometer(r: Reading) -> f64 {
    r.lower - r.upper
}

/// A uniform box of `density` as its closed form.
struct Prism {
    density: f64,
    lo: Vec3,
    hi: Vec3,
}

impl Source for Prism {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _: f64) -> [f64; 2] {
        heights.map(|h| {
            let at = |rise: f64| {
                let p = tower + Vec3::new(0.0, 0.0, h + rise);
                prism_potential(self.density, self.lo - p, self.hi - p)
            };
            at(upper) - at(lower)
        })
    }
}

/// The sum of prisms `(density, lo, hi)` read by tower `n` of `spec`, audited.
fn prisms(spec: &SceneSpec, n: usize, boxes: &[(f64, [f64; 3], [f64; 3])]) -> Reading {
    let readers = Readers::new(spec);
    let base = Vec3::from_array(spec.array.towers[n].position);
    boxes.iter().fold(Reading::default(), |sum, &(density, lo, hi)| {
        let source = Prism { density, lo: Vec3::from_array(lo), hi: Vec3::from_array(hi) };
        sum + readers.of(n).read_to(base, &source, 0.0, PHASE_TOLERANCE).reading
    })
}

/// The worst relative difference of the towers' departure gradiometers from the prisms'.
fn against_prisms(spec: &SceneSpec, boxes: &[(f64, [f64; 3], [f64; 3])]) -> f64 {
    reads(spec)
        .iter()
        .enumerate()
        .map(|(n, read)| {
            let (got, want) = (gradiometer(read.departures.reading), gradiometer(prisms(spec, n, boxes)));
            assert!(want != 0.0);
            (got - want).abs() / want.abs()
        })
        .fold(0.0, f64::max)
}

#[test]
fn the_merge_is_the_cells_added() {
    // Every finest cell of a chunk holds, in the merged leaves, its pieces' samples at their
    // material's density less its overlap's samples at the base's: the raster oracle of both, on
    // a box and a ditch cut off the lattice by the plane's surface and the planet's curve.
    let pieces = vec![
        piece("building", 0, [1.3, 1.1, -1.7], [5.9, 4.4, 3.2]),
        piece("ditch", 1, [-6.2, -3.3, -1.9], [-0.4, -0.6, 0.2]),
    ];
    for base in [
        plane(-0.37),
        BaseSpec::Sphere { radius: 60.0, layers: vec![LayerSpec { thickness: 30.0, density: 2700.0 }] },
    ] {
        let spec = world(base, 0.25, 3, pieces.clone());
        let w = World::new(&spec);
        let b = w.base.clone().unwrap();
        let solid = b.solid();
        let overlap = Overlap { pieces: w.solids.iter().map(|(s, _)| s.as_ref()).collect(), base: solid };
        let sample = w.grid.sample_volume();
        let mut checked = 0;
        for key in [
            ChunkKey::new(0, 0, -1),
            ChunkKey::new(-1, -1, -1),
            ChunkKey::new(0, 0, 0),
            ChunkKey::new(-1, -1, 0),
        ] {
            let (counts, materials) = raster_chunk(&w.pieces(), &w.grid, key);
            let (under, _) = raster_chunk(&[Piece { solid: &overlap, material: 0 }], &w.grid, key);
            let want: Vec<f64> = (0..counts.len())
                .map(|i| {
                    f64::from(counts[i]) * sample * w.densities[usize::from(materials[i])]
                        - f64::from(under[i]) * sample * b.density()
                })
                .collect();
            let mut got = vec![0.0; counts.len()];
            if let Some(read) = w.read_leaves(key).unwrap() {
                assert!(read.leaves.windows(2).all(|p| p[0].0 < p[1].0), "Morton order");
                for (&(k, level), &(_, mass, _)) in read.leaves.iter().zip(&read.cubes) {
                    let [x0, y0, z0] = morton::decode(k);
                    let side = 1u32 << level;
                    let share = mass / f64::from(side * side * side);
                    for z in z0..z0 + side {
                        for y in y0..y0 + side {
                            for x in x0..x0 + side {
                                let i = cav_core::voxel::world::cell_index([x, y, z]);
                                assert_eq!(got[i], 0.0, "a cell under two leaves");
                                got[i] = share;
                            }
                        }
                    }
                }
            }
            for (g, w) in got.iter().zip(&want) {
                assert!((g - w).abs() <= 1e-12 * w.abs().max(1.0), "{g} {w}");
            }
            checked += want.iter().filter(|m| **m < 0.0).count();
        }
        assert!(checked > 100, "the ditch and the foundation were in the chunks: {checked}");
    }
    // Control: the overlap left out (a patch of the same pieces) is not the departures.
    let spec = world(plane(-0.37), 0.25, 3, pieces.clone());
    let patch = World::new(&WorldSpec { base: BaseSpec::Patch, ..spec.clone() });
    let key = ChunkKey::new(-1, -1, -1);
    let mass =
        |w: &World| w.read_leaves(key).unwrap().map_or(0.0, |r| r.cubes.iter().map(|c| c.1).sum::<f64>());
    assert!(mass(&World::new(&spec)) < 0.0 && mass(&patch) == 0.0);
}

#[test]
fn a_base_is_refused_where_its_closed_form_does_not_hold() {
    let mut spec = scene(world(plane(0.5), 0.25, 2, Vec::new()));
    let said = spec.validate().unwrap_err().0;
    assert!(said.contains("stands below the plane's surface at z = 0.5"), "{said}");
    spec.world.as_mut().unwrap().base = plane(0.0);
    assert_eq!(spec.validate(), Ok(()), "towers on the surface stand on it");
    // A planet: a tower 1 cm into it is refused, one on the pole is not.
    let planet =
        BaseSpec::Sphere { radius: 1000.0, layers: vec![LayerSpec { thickness: 10.0, density: 2700.0 }] };
    spec.world.as_mut().unwrap().base = planet.clone();
    assert_eq!(spec.validate(), Ok(()));
    spec.array.towers[0].position[2] = -0.01;
    assert!(spec.validate().unwrap_err().0.contains("planet's surface"));
    spec.array.towers[0].position[2] = 0.0;
    // A piece that reaches below the top layer (10 m thick) is refused, in words.
    spec.world.as_mut().unwrap().pieces = vec![piece("mine", 1, [-1.0, -1.0, -12.0], [1.0, 1.0, 0.0])];
    let said = spec.validate().unwrap_err().0;
    assert!(said.contains("`mine`") && said.contains("top layer"), "{said}");
    spec.world.as_mut().unwrap().pieces = vec![piece("cellar", 1, [-1.0, -1.0, -9.0], [1.0, 1.0, 0.0])];
    assert_eq!(spec.validate(), Ok(()));
    // Layers thicker than the planet, and numbers out of range.
    let thick = BaseSpec::Sphere { radius: 10.0, layers: vec![LayerSpec { thickness: 11.0, density: 1.0 }] };
    let bad = [thick, BaseSpec::Sphere { radius: 10.0, layers: Vec::new() }, plane(f64::NAN)];
    for base in bad {
        let mut s = spec.clone();
        s.world.as_mut().unwrap().base = base.clone();
        assert!(s.validate().is_err(), "{base:?}");
    }
}

#[test]
fn a_patch_is_not_written_and_a_base_is_read_back() {
    let patch = scene(world(BaseSpec::Patch, 0.25, 2, Vec::new()));
    assert!(!patch.to_ron().contains("base"), "the default is not written");
    let mut planet = patch.clone();
    planet.world.as_mut().unwrap().base =
        BaseSpec::Sphere { radius: 6.4e6, layers: vec![LayerSpec { thickness: 3.5e4, density: 2900.0 }] };
    let text = planet.to_ron();
    assert!(text.contains("Sphere"));
    assert_eq!(SceneSpec::from_ron(&text).unwrap(), planet);
}

/// B1's measurement: the empty plane's worst gradiometer, radians, and its interferometers' worst
/// relative difference from S1's slab read alone.
fn empty_plane() -> (f64, f64) {
    let spec = scene(world(plane(-0.5), 0.5, 2, Vec::new()));
    let readers = Readers::new(&spec);
    let slab = Slab { density: PLANE.0, thickness: PLANE.1 };
    let (mut gradient, mut against) = (0f64, 0f64);
    for (n, read) in reads(&spec).iter().enumerate() {
        let r = read.audited.reading;
        assert!(read.audited.settled && read.base.is_some());
        gradient = gradient.max(gradiometer(r).abs());
        let base = Vec3::from_array(spec.array.towers[n].position);
        let want = readers.of(n).read_to(base, &slab, 0.0, PHASE_TOLERANCE).reading;
        let fixed = readers.of(n).read(base, &slab, 0.0);
        for (g, w) in [(r.lower, want.lower), (r.upper, want.upper), (r.lower, fixed.lower)] {
            against = against.max((g - w).abs() / w.abs());
        }
    }
    (gradient, against)
}

cav_validate::case! {
    test: validation_an_empty_plane_reads_no_gradient,
    name: "world.empty_plane_reads_no_gradient",
    subsystem: "world",
    slice: "S5",
    reference: ClosedForm,
    against: "an infinite plane (2000 kg/m3, 8 m thick) with no pieces, at the five towers: the worst gradiometer, radians (control: the same slab as a 200 m patch reads at least 1 rad)",
    unit: Radians,
    tolerance: 1e-12,
    measure: {
        let (gradient, against) = empty_plane();
        // Each interferometer is S1's slab read alone (audited, and the floor's fixed rule).
        assert!(against <= 1e-12, "{against}");
        // Control: the same slab, 200 m square, as pieces of a patch, read at the centre tower.
        let patch = world(BaseSpec::Patch, 1.0, 2, vec![piece("slab", 0, [-100.0, -100.0, -8.5], [100.0, 100.0, -0.5])]);
        let mut patch = WorldSpec { origin: [0.0, 0.0, 0.5], materials: vec![MaterialSpec::plain("rock", PLANE.0, "#808080")], ..patch };
        patch.read.tolerance = 1e-6;
        let mut spec = scene(patch);
        spec.array.towers.truncate(1);
        let finite = gradiometer(reads(&spec)[0].audited.reading).abs();
        assert!(finite >= 1.0, "{finite}");
        println!("B1: empty plane {gradient:.1e} rad, interferometers against the slab {against:.1e}; 200 m patch {finite:.3} rad");
        cav_validate::Measured::Error(gradient)
    },
}

/// The building of B2 and the soundness test: concrete from 1 m under the surface to 4 m above.
const BUILDING: ([f64; 3], [f64; 3]) = ([6.0, 5.0, -1.0], [10.0, 9.0, 4.0]);
/// The ditch: 2 m of air under the surface.
const DITCH: ([f64; 3], [f64; 3]) = ([-10.0, 3.0, -2.0], [-4.0, 7.0, 0.0]);

cav_validate::case! {
    test: validation_departures_on_a_plane,
    name: "world.departures_on_a_plane",
    subsystem: "world",
    slice: "S5",
    reference: ClosedForm,
    against: "a plane (2000 kg/m3) under a box standing on it, a building with a 1 m foundation, a ditch of air, and the building and the ditch together: each tower's departure gradiometer against the prisms (the box's; above and below the surface at its density and its density less the plane's; minus the ditch's at the plane's), the worst relative difference (control: the ditch of a patch reads nothing)",
    unit: Relative,
    tolerance: 1e-9,
    measure: {
        let (rho, slab) = (2400.0, PLANE.0);
        let (lo, hi) = BUILDING;
        let (dlo, dhi) = DITCH;
        let above = (rho, [lo[0], lo[1], 0.0], hi);
        let below = (rho - slab, lo, [hi[0], hi[1], 0.0]);
        let ditch = (-slab, dlo, dhi);
        let on = |pieces| scene(world(plane(0.0), 0.25, 2, pieces));
        let cases = [
            (on(vec![piece("box", 0, [lo[0], lo[1], 0.0], hi)]), vec![above]),
            (on(vec![piece("building", 0, lo, hi)]), vec![above, below]),
            (on(vec![piece("ditch", 1, dlo, dhi)]), vec![ditch]),
            (on(vec![piece("building", 0, lo, hi), piece("ditch", 1, dlo, dhi)]), vec![above, below, ditch]),
        ];
        let mut error = 0f64;
        for (spec, boxes) in &cases {
            let e = against_prisms(spec, boxes);
            println!("B2: {} pieces: {e:.2e}", spec.world.as_ref().unwrap().pieces.len());
            error = error.max(e);
        }
        // Control: the ditch of a patch weighs nothing, so it reads nothing, not the ditch.
        let mut patch = on(vec![piece("ditch", 1, dlo, dhi)]);
        patch.world.as_mut().unwrap().base = BaseSpec::Patch;
        assert!(reads(&patch).iter().all(|r| gradiometer(r.departures.reading) == 0.0));
        cav_validate::Measured::Error(error)
    },
}

/// A planet of radius `radius`: `layers` of `(thickness, density)` from the surface down.
fn planet(radius: f64, layers: &[(f64, f64)]) -> BaseSpec {
    BaseSpec::Sphere {
        radius,
        layers: layers.iter().map(|&(thickness, density)| LayerSpec { thickness, density }).collect(),
    }
}

/// The towers' readings of the layers as point masses at the centre, one by one, added.
fn layers_read(spec: &SceneSpec, radius: f64, layers: &[(f64, f64)]) -> Vec<Reading> {
    let readers = Readers::new(spec);
    let centre = Vec3::new(0.0, 0.0, -radius);
    spec.array
        .towers
        .iter()
        .enumerate()
        .map(|(n, tower)| {
            let mut outer = radius;
            let mut sum = Reading::default();
            for &(thickness, density) in layers {
                let inner = outer - thickness;
                let mass = 4.0 / 3.0 * std::f64::consts::PI * density * (outer.powi(3) - inner.powi(3));
                let point = Mass::point(mass, move |_| centre);
                sum = sum
                    + readers
                        .of(n)
                        .read_to(Vec3::from_array(tower.position), &point, 0.0, PHASE_TOLERANCE)
                        .reading;
                outer = inner;
            }
            sum
        })
        .collect()
}

/// The departures' gradiometer at the centre tower of a box sunk into `base` (concrete, 32 m by
/// 32 m, from 3 m under the pole to 1 m above it, 10 m east of the tower), within 1e-6 rad.
fn sunk(base: BaseSpec) -> f64 {
    let mut spec =
        scene(world(base, 0.25, 4, vec![piece("hill", 0, [10.0, -16.0, -3.0], [42.0, 16.0, 1.0])]));
    spec.array.towers.truncate(1);
    // Within 1e-6 rad: the differences measured are 20 rad and more.
    spec.world.as_mut().unwrap().read.tolerance = 1e-6;
    gradiometer(reads(&spec)[0].departures.reading)
}

cav_validate::case! {
    test: validation_a_planet_reads_as_its_layers,
    name: "world.planet_reads_as_its_layers",
    subsystem: "world",
    slice: "S5",
    reference: ClosedForm,
    against: "a planet of 10 km in two layers, at the five towers: each interferometer against its layers read as point masses at its centre, the worst relative difference; a box above its surface against the prism (≤ 1e-9); the sunk box is world.sunk_box_falls_as_one_over_r",
    unit: Relative,
    tolerance: 1e-12,
    measure: {
        let layers = [(2000.0, 2700.0), (8000.0, 5500.0)];
        let spec = scene(world(planet(1.0e4, &layers), 0.25, 2, Vec::new()));
        let want = layers_read(&spec, 1.0e4, &layers);
        // Each interferometer. The gradiometer, a thousandth of either and their difference, is
        // held to this relative to itself by `validation_a_planet_s_gradiometer_reads_as_its_layers`
        // (TASK-S5-28, `docs/slices/S5.md` §7.16: two reads of the same 5.5e7 rad agree to some
        // ulps of it, 1.3e-12 of their difference); here it is printed.
        let (mut error, mut own) = (0f64, 0f64);
        for (read, want) in reads(&spec).iter().zip(&want) {
            let got = read.audited.reading;
            for (g, w) in [(got.lower, want.lower), (got.upper, want.upper)] {
                error = error.max((g - w).abs() / w.abs());
            }
            own = own.max((gradiometer(got) - gradiometer(*want)).abs() / gradiometer(*want).abs());
        }
        println!("B3: gradiometer against the layers {own:.2e} of itself");
        // Control: the core left out is far from it.
        let shallow = layers_read(&spec, 1.0e4, &layers[..1]);
        assert!((gradiometer(shallow[0]) / gradiometer(want[0]) - 1.0).abs() > 1e-3);
        // A box above the surface reads as its prism, the planet under it or not.
        let (lo, hi) = ([6.0, 5.0, 0.25], [10.0, 9.0, 2.25]);
        let above = scene(world(planet(1.0e4, &layers), 0.25, 2, vec![piece("box", 0, lo, hi)]));
        let prism = against_prisms(&above, &[(2400.0, lo, hi)]);
        assert!(prism <= 1e-9, "{prism}");
        println!("B3: layers {error:.2e}; box above {prism:.2e}");
        cav_validate::Measured::Error(error)
    },
}

cav_validate::case! {
    test: validation_slow_a_sunk_box_falls_as_one_over_r,
    name: "world.sunk_box_falls_as_one_over_r",
    subsystem: "world",
    slice: "S5",
    reference: ClosedForm,
    against: "B3's third part: a box of concrete sunk 3 m into uniform planets of 400 m to 3.2 km, its departures' gradiometer at the pole against the same box on a plane: the difference's ratio per doubling of the radius, its worst relative distance from 2 (control: the box read as a patch, with no base, differs from the plane's by the same at every radius)",
    unit: Relative,
    tolerance: 0.1,
    measure: {
        // The planet's surface falls away under the box by (x² + y²)/2R; the base's matter there
        // is not replaced, so the departures differ from the plane's by a lens of it: as 1/R.
        let flat = sunk(BaseSpec::Plane { surface: 0.0, density: 2700.0, thickness: 100.0 });
        let radii = [400.0, 800.0, 1600.0, 3200.0];
        let differences: Vec<f64> = radii.iter().map(|&r| sunk(planet(r, &[(r, 2700.0)])) - flat).collect();
        let worst = |d: &[f64]| d.windows(2).map(|w| (w[0] / w[1] / 2.0 - 1.0).abs()).fold(0.0, f64::max);
        println!("B3, sunk: flat {flat:.4e} rad, differences {differences:?}, worst {:.4}", worst(&differences));
        // Control: departures not taken against the base (a patch) do not change with R at all.
        let patch = sunk(BaseSpec::Patch) - flat;
        assert!(worst(&[patch, patch]) > 0.1);
        cav_validate::Measured::Error(worst(&differences))
    },
}

/// The worst error over ledger and ledger over tolerance of the walk on a plane world with a
/// building (its foundation below the surface) and a ditch of air: signed masses. As W2
/// (`world::walk::tests`): the accepted nodes by their expansion against their own leaves,
/// through one coarse Gauss rule.
fn signed_ledgers() -> (f64, f64, usize) {
    use crate::world::octree::Octree;
    use crate::world::walk::tests::chunks_under;
    use crate::world::walk::{Aim, LedgerSource, MAX_CUT_NODES, Rule, far_field, walk_with};
    use cav_core::forward::Quadrature;
    let (lo, hi) = BUILDING;
    let (dlo, dhi) = DITCH;
    let spec =
        scene(world(plane(0.0), 0.25, 2, vec![piece("building", 0, lo, hi), piece("ditch", 1, dlo, dhi)]));
    let w = World::new(spec.world.as_ref().unwrap());
    let tree = Octree::build(&w, &|| false).unwrap().unwrap();
    let readers = Readers::new(&spec);
    let reader = Reader::new(cav_core::forward::Instrument {
        quadrature: Quadrature::Gauss { order: 3, panel: 0.5 },
        ..*readers.of(0).instrument()
    });
    let keys: Vec<ChunkKey> = tree.level(0).map(|id| tree.chunk(id).unwrap()).collect();
    let made = crate::world::open(&w, &keys, &|| false).unwrap().unwrap();
    let height = spec.array.instrument_of(0).tower_height;
    let (mut over, mut used, mut accepted) = (0f64, 0f64, 0usize);
    let bases = [
        [0.0, 0.0, 0.0],
        [-7.0, 8.0, 0.0],
        [8.0, 10.0, 4.0],
        [30.0, 0.0, 0.0],
        [-40.0, 20.0, 1.0],
        [80.0, 40.0, 0.0],
        [0.0, 150.0, 0.0],
        [-200.0, -100.0, 0.0],
    ];
    for base in bases.map(Vec3::from_array) {
        for tolerance in [1e-2, 1e-4, 1e-6] {
            let probe = Aim {
                base,
                height,
                per: crate::world::radians_per_kg_m(&reader, base, 0.0),
                tolerance,
                bubble: 0.0,
                max_work: u64::MAX,
                max_nodes: MAX_CUT_NODES,
                rule: Rule::Octupole,
            };
            let cut = walk_with(&tree, &probe, &made);
            assert!(cut.settled);
            let far = far_field(&tree, &made, &cut, Rule::Octupole);
            let tree_of = |k: &ChunkKey| &made[made.binary_search_by_key(k, |t| t.key).unwrap()];
            let mut leaves: Vec<crate::world::Cube> = Vec::new();
            for key in &cut.accepted {
                let mut under = Vec::new();
                chunks_under(&tree, tree.find(*key).unwrap(), &mut under);
                for chunk in under {
                    leaves.extend_from_slice(&tree_of(&chunk).cubes);
                }
            }
            for key in &cut.inner {
                let t = tree_of(&key.chunk);
                let r = &t.node(t.find(*key).unwrap()).leaves;
                leaves.extend_from_slice(&t.cubes[r.start as usize..r.end as usize]);
            }
            accepted += cut.accepted.len() + cut.inner.len();
            let exact = crate::world::tests::leaf_exact(&reader, base, &leaves);
            let got = reader.read(base, &far, 0.0);
            let bound = reader.read(base, &LedgerSource { nodes: &far.nodes, rule: Rule::Octupole }, 0.0);
            let ledger = [bound.lower.abs(), bound.upper.abs()];
            let arm = |e: f64, g: f64, l: f64| (g - e).abs() / (l + 1e-13 * e.abs());
            let g = ((got.lower - got.upper) - (exact.lower - exact.upper)).abs()
                / (ledger[0] + ledger[1] + 1e-13 * (exact.lower.abs() + exact.upper.abs()));
            over = over
                .max(arm(exact.lower, got.lower, ledger[0]))
                .max(arm(exact.upper, got.upper, ledger[1]))
                .max(g);
            used = used.max((ledger[0] + ledger[1]) / tolerance);
        }
    }
    (over, used, accepted)
}

#[test]
fn the_walk_is_within_its_ledger_on_signed_masses() {
    // W2 on departures: a building that adds and a ditch that carves, so nodes hold masses of both
    // signs. Error ≤ ledger ≤ τ at every base and tolerance.
    let (over, used, accepted) = signed_ledgers();
    println!("signed: error/ledger {over:.3}, ledger/tolerance {used:.3}, {accepted} nodes accepted");
    assert!(accepted > 0, "the walk accepted nodes, so the ledger had work to do");
    assert!(over <= 1.0 && used <= 1.0, "{over} {used}");
    // Control: a building and a ditch of equal and opposite mass side by side. The bound made of
    // |m| covers what the octupole leaves out; one made of the signed masses (Σ m s^k, nearly 0)
    // does not.
    let cubes = [(Vec3::new(0.5, 0.0, 0.0), 1000.0, 1.0), (Vec3::new(-0.5, 0.0, 0.0), -1000.0, 1.0)];
    let summary = cav_core::voxel::summary::Summary::of_cubes(Vec3::ZERO, cubes);
    let mut signed = summary;
    for (k, a) in signed.absolute.iter_mut().enumerate() {
        *a = cubes.iter().map(|&(at, m, _)| m * at.length().powi(k as i32)).sum::<f64>().max(0.0);
    }
    let at = |r: f64, rise: f64| {
        let p = Vec3::new(r, 0.3, rise);
        let exact: f64 = cubes
            .iter()
            .map(|&(c, m, h)| {
                prism_potential(
                    m / (h * h * h),
                    c - Vec3::new(h / 2.0, h / 2.0, h / 2.0) - p,
                    c + Vec3::new(h / 2.0, h / 2.0, h / 2.0) - p,
                )
            })
            .sum();
        exact
    };
    let (r, gap) = (4.0, 0.5);
    let exact = at(r, gap) - at(r, 0.0);
    let expanded = summary.difference([r, 0.3], 0.0, gap, -gap);
    let error = (expanded - exact).abs();
    let sound = summary.bound(Vec3::new(r, 0.3, 0.0).length(), gap);
    let broken = signed.bound(Vec3::new(r, 0.3, 0.0).length(), gap);
    assert!(error > 0.0 && error <= sound, "{error} {sound}");
    assert!(error > broken, "{error} {broken}");
}
