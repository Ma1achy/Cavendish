//! The world's read held to a voxel body, to the closed form of a box, to the infinite floor's
//! null and to its own bound; the world's spec held to its caps (`docs/slices/S4.md` §3).

use super::*;
use crate::read::{self, SourceId};
use crate::{BodySpec, SceneSpec, ShapeKind, ShapeSpec};
use cav_core::forward::Reading;
use cav_core::voxel::kernel::prism_potential;

/// An open scene of the default five towers, a short timeline, and `world`.
fn scene(world: WorldSpec) -> SceneSpec {
    SceneSpec { room: None, floor: None, duration: 3.0, world: Some(world), ..SceneSpec::room() }
}

fn concrete() -> MaterialSpec {
    MaterialSpec { label: "concrete".into(), density: 2400.0, colour: "#b4b4aa".into() }
}

fn boxed(lo: [f64; 3], hi: [f64; 3]) -> PieceSpec {
    PieceSpec {
        label: "box".into(),
        material: 0,
        kind: PieceKind::Box {
            centre: [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])],
            size: [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]],
        },
    }
}

fn world_of(pitch: f64, pieces: Vec<PieceSpec>) -> WorldSpec {
    WorldSpec {
        pitch,
        supersample: 2,
        origin: [0.0; 3],
        materials: vec![concrete()],
        pieces,
        stream: StreamSpec::default(),
        read: WorldReadSpec::default(),
        lod: LodSpec::default(),
    }
}

/// Every tower's phases from one source of `spec`: `(lower, upper, gradiometer)` of each tower's
/// first shot.
fn first_shots(spec: &SceneSpec, source: SourceId) -> Vec<[f64; 3]> {
    let shots = read::shots(spec).unwrap();
    let c = read::contribution(spec, source, &shots, &|| false).unwrap();
    assert!(c.settled, "{:?}", c.warnings);
    (0..spec.array.towers.len())
        .map(|d| {
            let at = shots.starts[d];
            [c.lower[at], c.upper[at], c.gradiometer[at]]
        })
        .collect()
}

/// The worst difference of two towers' phases, each relative to the larger of the two values.
fn worst(a: &[[f64; 3]], b: &[[f64; 3]]) -> f64 {
    a.iter()
        .zip(b)
        .flat_map(|(x, y)| (0..3).map(move |i| (x[i] - y[i]).abs() / x[i].abs().max(y[i].abs())))
        .fold(0.0, f64::max)
}

/// The box of R4 and R5: concrete over four chunks (x and y cross 8 m), on the world's lattice.
const BOX: ([f64; 3], [f64; 3]) = ([6.0, 5.0, 0.25], [10.0, 9.0, 2.25]);

cav_validate::case! {
    test: validation_world_box_reads_as_a_voxel_body,
    name: "world.box_reads_as_a_voxel_body",
    subsystem: "world",
    slice: "S4",
    reference: Exact,
    against: "a box of concrete over four chunks of the world against the same box made a voxel body: the worst relative difference of any channel of any tower",
    unit: Relative,
    tolerance: 1e-12,
    measure: {
        let (lo, hi) = BOX;
        let world = world_of(0.25, vec![boxed(lo, hi)]);
        let spec = scene(world.clone());
        let as_world = first_shots(&spec, SourceId::World);
        // The same matter as a voxel body: its grid is centred on its box, which puts it on the
        // world's lattice; its mass is the box's.
        let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        let centre = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])];
        let mut body = BodySpec::point("box", 2400.0 * size[0] * size[1] * size[2], centre);
        body.shape = Some(ShapeSpec { kind: ShapeKind::Cuboid { size }, pitch: 0.25, supersample: 2 });
        let as_body_spec = SceneSpec { world: None, bodies: vec![body], ..spec.clone() };
        let as_body = first_shots(&as_body_spec, SourceId::Body(0));
        assert!(as_world.iter().all(|t| t[2] != 0.0));
        // Control: the world with one of its four chunks left out is far from it.
        let w = World::new(&world);
        let (keys, _) = read_keys(&spec, &w).unwrap();
        let mut made = make(&w, &keys, &|| false).unwrap().unwrap();
        assert_eq!(made.chunks.len(), 4);
        made.chunks.remove(1);
        let readers = read::Readers::new(&spec);
        let base = Vec3::from_array(spec.array.towers[0].position);
        let short = read_tower(&made, readers.of(0), base, 10.1, 0.0, PHASE_TOLERANCE, u64::MAX).unwrap();
        let r = short.audited.reading;
        assert!(worst(&[[r.lower, r.upper, r.lower - r.upper]], &as_world[..1]) > 1e-3);
        cav_validate::Measured::Error(worst(&as_world, &as_body))
    },
}

/// A uniform box as its closed form: `(V(upper) − V(lower))/G` from the prism.
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

/// The gradiometer of tower `n` of `spec` reading the prism `lo..hi` of concrete.
fn prism_reading(spec: &SceneSpec, n: usize, lo: [f64; 3], hi: [f64; 3]) -> Reading {
    let readers = read::Readers::new(spec);
    let source = Prism { density: 2400.0, lo: Vec3::from_array(lo), hi: Vec3::from_array(hi) };
    readers
        .of(n)
        .read_to(Vec3::from_array(spec.array.towers[n].position), &source, 0.0, PHASE_TOLERANCE)
        .reading
}

cav_validate::case! {
    test: validation_world_box_reads_as_the_prism,
    name: "world.box_reads_as_the_prism",
    subsystem: "world",
    slice: "S4",
    reference: ClosedForm,
    against: "a box of concrete on the world's cells: each tower's gradiometer against the closed form of the prism, the worst relative difference",
    unit: Relative,
    tolerance: 1e-9,
    measure: {
        let (lo, hi) = BOX;
        let spec = scene(world_of(0.25, vec![boxed(lo, hi)]));
        let got = first_shots(&spec, SourceId::World);
        let mut error = 0f64;
        for (n, tower) in got.iter().enumerate() {
            let want = prism_reading(&spec, n, lo, hi);
            error = error.max((tower[2] - (want.lower - want.upper)).abs() / tower[2].abs());
        }
        // Control: the prism a cell longer is not it.
        let want = prism_reading(&spec, 0, lo, [hi[0] + 0.25, hi[1], hi[2]]);
        assert!((got[0][2] - (want.lower - want.upper)).abs() / got[0][2].abs() > 1e-6);
        cav_validate::Measured::Error(error)
    },
}

/// The slab series of R6: a square slab 32 m thick (two cells of 16 m) whose top is 1 m under the bases, of
/// half-widths 160, 320, 640 and 1280 m about the centre tower, read alone. Returns the worst
/// relative difference from the prism's closed form, and each width's gradiometer, radians.
fn slab_series() -> &'static (f64, Vec<f64>) {
    static SERIES: std::sync::OnceLock<(f64, Vec<f64>)> = std::sync::OnceLock::new();
    SERIES.get_or_init(|| {
        let towers = SceneSpec::room().array.towers;
        let centre =
            towers.iter().position(|t| t.position == [0.0, 0.0, 0.0]).expect("a tower at the origin");
        let mut against_prism = 0f64;
        let mut readings = Vec::new();
        for half in [160.0, 320.0, 640.0, 1280.0] {
            let (lo, hi) = ([-half, -half, -33.0], [half, half, -1.0]);
            let mut world = world_of(16.0, vec![boxed(lo, hi)]);
            world.origin = [0.0, 0.0, -1.0];
            world.read.radius = 2.0 * half;
            let mut spec = scene(world);
            spec.array.towers = vec![towers[centre].clone()];
            let got = first_shots(&spec, SourceId::World)[0][2];
            let want = prism_reading(&spec, 0, lo, hi);
            against_prism = against_prism.max((got - (want.lower - want.upper)).abs() / got.abs());
            readings.push(got);
        }
        (against_prism, readings)
    })
}

cav_validate::case! {
    test: validation_world_slab_equals_the_prism,
    name: "world.slab_equals_the_prism",
    subsystem: "world",
    slice: "S4",
    reference: ClosedForm,
    against: "a square slab of the world, 320 m to 2560 m across and 32 m thick, read at the centre tower: its gradiometer against the prism's closed form, the worst relative difference",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(slab_series().0),
}

cav_validate::case! {
    test: validation_world_slab_converges_to_the_null,
    name: "world.slab_converges_to_the_null",
    subsystem: "world",
    slice: "S4",
    reference: ClosedForm,
    against: "the same slab: its gradiometer halves as its width doubles (a finite floor's edge term falls as 1/L, to the infinite floor's null); the worst ratio's relative distance from 2",
    unit: Relative,
    tolerance: 0.05,
    measure: {
        let readings = &slab_series().1;
        let off = |a: f64, b: f64| (a / b / 2.0 - 1.0).abs();
        let worst_ratio = readings.windows(2).map(|w| off(w[0], w[1])).fold(0.0, f64::max);
        assert!(readings.windows(2).all(|w| w[1].abs() < w[0].abs()), "{readings:?}");
        // Control: the same slab twice (no growth) is a ratio of 1, far from 2.
        assert!(off(readings[0], readings[0]) > 0.05);
        cav_validate::Measured::Error(worst_ratio)
    },
}

/// R7's measurement: the worst error over the bound, and the loosest bound over the error where
/// the error is above rounding, over q = a/R from 0.45 to 0.05 (the error over the bound also at
/// 0.002, where the control is).
fn moments_against_bound() -> &'static (f64, f64) {
    static FOUND: std::sync::OnceLock<(f64, f64)> = std::sync::OnceLock::new();
    FOUND.get_or_init(|| {
        // One chunk of hilly terrain and a house (two materials), read through its moments against
        // its leaves, from near (q = a/R = 0.45) to far (q = 0.05), on the centre tower.
        let mut world = world_of(0.25, Vec::new());
        world.materials.push(MaterialSpec {
            label: "brick".into(),
            density: 1900.0,
            colour: "#a0503c".into(),
        });
        world.pieces = vec![
            PieceSpec {
                label: "house".into(),
                material: 1,
                kind: PieceKind::Box { centre: [3.0, 4.0, 2.0], size: [2.5, 3.0, 3.5] },
            },
            PieceSpec {
                label: "hill".into(),
                material: 0,
                kind: PieceKind::Terrain {
                    lo: [0.0, 0.0],
                    hi: [8.0, 8.0],
                    bottom: 0.0,
                    level: 1.5,
                    relief: 1.0,
                    wavelength: 6.0,
                    seed: 4,
                },
            },
        ];
        let spec = scene(world.clone());
        let w = World::new(&world);
        let chunk = w.chunk(ChunkKey::new(0, 0, 0)).unwrap().unwrap();
        assert_eq!(chunk.palette, [0, 1]);
        let multipole = Multipole::of(&chunk, &w.grid, &w.densities);
        let leaves = WorldSource { cubes: chunk.cubes(&w.grid, &w.densities), far: Vec::new() };
        let readers = read::Readers::new(&spec);
        let reader = readers.of(0);
        let (mut worst_over, mut loosest) = (0f64, 0f64);
        let mut checked = 0;
        for q in [0.45, 0.3, 0.2, 0.1, 0.05, 0.002] {
            // The tower placed so that the chunk's centre is a/q from its atoms' path, to the side.
            let distance = multipole.radius / q;
            let c = multipole.moments.centre;
            let base = Vec3::new(c.x + distance, c.y, 0.0);
            let exact = reader.read_to(base, &leaves, 0.0, 1e-14).reading;
            let far = WorldSource { cubes: Vec::new(), far: vec![multipole] };
            let approx = reader.read_to(base, &far, 0.0, 1e-14).reading;
            let bound = reader.read(base, &BoundSource { far: &far.far }, 0.0);
            for (e, a, b) in
                [(exact.lower, approx.lower, bound.lower), (exact.upper, approx.upper, bound.upper)]
            {
                let error = (e - a).abs();
                worst_over = worst_over.max(error / b.abs());
                // The looseness is measured from 0.45 to 0.05, as the brief says; the far point is for
                // the control (there the bound is 1054 times the error: S4.md §7).
                if error > 1e-13 * e.abs() && q >= 0.05 {
                    loosest = loosest.max(b.abs() / error);
                    checked += 1;
                }
            }
            // Control: the octupole left out breaks the bound where the octupole outweighs what the
            // bound allows for the rest (the bound is a few hundred times the error, so far out).
            if q <= 0.002 {
                let mut quadrupole = multipole;
                quadrupole.moments.third = [0.0; 10];
                let cut = reader
                    .read_to(base, &WorldSource { cubes: Vec::new(), far: vec![quadrupole] }, 0.0, 1e-14)
                    .reading;
                assert!((exact.lower - cut.lower).abs() > bound.lower.abs(), "q = {q}");
            }
        }
        assert!(checked >= 6, "{checked}");
        (worst_over, loosest)
    })
}

cav_validate::case! {
    test: validation_world_moments_within_their_bound,
    name: "world.moments_within_their_bound",
    subsystem: "world",
    slice: "S4",
    reference: Exact,
    against: "a chunk of terrain and a house (two materials) read through its moments to the octupole, against its leaves: the worst error over the stated bound, q = a/R from 0.45 to 0.002 (the octupole left out breaks the bound at q = 0.002: the control)",
    unit: Relative,
    tolerance: 1.0,
    measure: cav_validate::Measured::Error(moments_against_bound().0),
}

cav_validate::case! {
    test: validation_world_moments_bound_is_not_vacuous,
    name: "world.moments_bound_is_not_vacuous",
    subsystem: "world",
    slice: "S4",
    reference: Exact,
    against: "the same: the bound over the error, the loosest where the error is above rounding",
    unit: Relative,
    tolerance: 1000.0,
    measure: cav_validate::Measured::Error(moments_against_bound().1),
}

#[test]
fn the_world_is_a_source_like_any_other() {
    let mut spec = scene(world_of(0.5, vec![boxed([4.0, 4.0, 0.0], [6.0, 6.0, 2.0])]));
    spec.bodies.push(BodySpec::point("mass", 50.0, [-3.0, 2.0, 1.0]));
    spec.floor = Some(crate::FloorSpec {
        height: -1.0,
        kind: crate::FloorKind::Slab { thickness: 0.5 },
        density: 2400.0,
    });
    // Floor, world, then bodies: the order contributions are summed in.
    assert_eq!(read::sources(&spec), [SourceId::Floor, SourceId::World, SourceId::Body(0)]);
    let shots = read::shots(&spec).unwrap();
    let c = read::contribution(&spec, SourceId::World, &shots, &|| false).unwrap();
    // Static: every shot a tower takes reads the same; and the estimate is said per tower.
    for d in 0..spec.array.towers.len() {
        let at = shots.starts[d];
        let n = shots.count(d);
        assert!(n > 1 && (at..at + n).all(|k| c.gradiometer[k] == c.gradiometer[at]));
    }
    assert!(c.estimates.iter().all(|e| e.is_finite()));
    // A world of nothing reads nothing; and a scene without a world has no such source.
    let empty = scene(world_of(0.5, Vec::new()));
    let none = read::contribution(&empty, SourceId::World, &read::shots(&empty).unwrap(), &|| false).unwrap();
    assert!(none.gradiometer.iter().all(|v| *v == 0.0));
    assert!(!read::sources(&SceneSpec::room()).contains(&SourceId::World));
    // Control: the box moved a metre reads otherwise.
    let mut moved = spec.clone();
    moved.world = Some(world_of(0.5, vec![boxed([5.0, 4.0, 0.0], [7.0, 6.0, 2.0])]));
    let other = read::contribution(&moved, SourceId::World, &shots, &|| false).unwrap();
    assert_ne!(other.gradiometer, c.gradiometer);
}

#[test]
fn the_world_is_bounded_in_the_spec() {
    let good = scene(world_of(0.5, vec![boxed([1.0, 1.0, 0.0], [2.0, 2.0, 1.0])]));
    assert_eq!(good.validate(), Ok(()));
    type Break = Box<dyn Fn(&mut WorldSpec)>;
    let cases: Vec<(&str, Break)> = vec![
        ("world.pitch", Box::new(|w| w.pitch = 1e-4)),
        ("world.pitch", Box::new(|w| w.pitch = f64::NAN)),
        ("world.supersample", Box::new(|w| w.supersample = 7)),
        ("world.materials", Box::new(|w| w.materials.clear())),
        ("world.materials", Box::new(|w| w.materials = vec![concrete(); 256])),
        ("world.materials[0].density", Box::new(|w| w.materials[0].density = -1.0)),
        ("world.pieces", Box::new(|w| w.pieces = vec![w.pieces[0].clone(); MAX_PIECES + 1])),
        ("world.pieces[0].material", Box::new(|w| w.pieces[0].material = 1)),
        (
            "world.pieces[0].kind.Box.size[1]",
            Box::new(|w| w.pieces[0].kind = PieceKind::Box { centre: [0.0; 3], size: [1.0, 0.0, 1.0] }),
        ),
        (
            "world.pieces[0].kind.Terrain",
            Box::new(|w| {
                w.pieces[0].kind = PieceKind::Terrain {
                    lo: [1.0, 0.0],
                    hi: [0.0, 1.0],
                    bottom: -1.0,
                    level: 0.0,
                    relief: 0.1,
                    wavelength: 1.0,
                    seed: 0,
                }
            }),
        ),
        (
            "world.pieces[0].kind.Terrain.bottom",
            Box::new(|w| {
                w.pieces[0].kind = PieceKind::Terrain {
                    lo: [0.0, 0.0],
                    hi: [1.0, 1.0],
                    bottom: -0.05,
                    level: 0.0,
                    relief: 0.1,
                    wavelength: 1.0,
                    seed: 0,
                }
            }),
        ),
        ("world.read.radius", Box::new(|w| w.read.radius = 2e4)),
        ("world.read.tolerance", Box::new(|w| w.read.tolerance = -1.0)),
        ("world.lod.error_px", Box::new(|w| w.lod.error_px = 0.1)),
        ("world.stream.tower_radius", Box::new(|w| w.stream.tower_radius = 1e4)),
        ("world.origin", Box::new(|w| w.origin = [2e6, 0.0, 0.0])),
    ];
    for (path, break_it) in cases {
        let mut spec = good.clone();
        break_it(spec.world.as_mut().unwrap());
        let why = spec.validate().unwrap_err().0;
        assert!(why.starts_with(path), "{path}: {why}");
    }
    // A world survives RON and JSON, and a scene without one writes no `world` at all.
    assert_eq!(SceneSpec::from_ron(&good.to_ron()).unwrap(), good);
    assert!(!SceneSpec::room().to_ron().contains("world"));
}

#[test]
fn the_world_read_is_bounded() {
    // A wide world at a fine pitch: the search for chunks within the radius is refused before it
    // starts, with the numbers.
    let mut world = world_of(0.01, vec![boxed([-400.0, -400.0, -2.0], [400.0, 400.0, -1.0])]);
    world.read.radius = 500.0;
    let spec = scene(world.clone());
    let why = read_keys(&spec, &World::new(&world)).err().unwrap();
    assert!(why.contains(&MAX_READ_SCAN.to_string()), "{why}");
    let shots = read::shots(&spec).unwrap();
    let c = read::contribution(&spec, SourceId::World, &shots, &|| false).unwrap();
    assert!(!c.settled && c.warnings.iter().any(|w| w.text.starts_with("the world is not read")));
    // Within the scan but past the chunk cap: the nearest are kept, nearest first, and the rest said.
    let mut world = world_of(0.05, vec![boxed([-60.0, -60.0, -2.0], [60.0, 60.0, -1.0])]);
    world.read.radius = 80.0;
    let spec = scene(world.clone());
    let (keys, said) = read_keys(&spec, &World::new(&world)).unwrap();
    assert_eq!(keys.len(), MAX_READ_CHUNKS);
    assert!(said.iter().any(|s| s.contains("are read and the rest are not")), "{said:?}");
    // Control: a radius that holds the whole world says nothing is left.
    let mut world = world_of(0.5, vec![boxed([1.0, 1.0, 0.0], [3.0, 3.0, 1.0])]);
    world.read.radius = 100.0;
    let spec = scene(world.clone());
    let (keys, said) = read_keys(&spec, &World::new(&world)).unwrap();
    assert!(!keys.is_empty() && said.is_empty(), "{said:?}");
    // The kernel's work: a read past its share is refused, with the numbers.
    let w = World::new(&world);
    let made = make(&w, &keys, &|| false).unwrap().unwrap();
    let readers = read::Readers::new(&spec);
    let why = read_tower(&made, readers.of(0), Vec3::ZERO, 10.1, 0.0, 0.0, 1000).err().unwrap();
    assert!(why.contains("more than the 1000"), "{why}");
}
