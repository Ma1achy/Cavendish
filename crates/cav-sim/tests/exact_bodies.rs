//! Bodies read exactly, with no voxels (TASK-S5-06; `docs/slices/S5.md` §3 rows A1–A3 and X1,
//! the numbers in §7.5).

use cav_core::Vec3;
use cav_core::forward::{Instrument, Mass, Reader};
use cav_sim::exact::{ExactBody, MAX_EXACT_FACES};
use cav_sim::read::{SourceId, contribution, only, reading, shots};
use cav_sim::{
    BodySpec, Motion, PlatonicSolid, Representation, SceneSpec, ShapeKind, ShapeSpec, TowerSpec, dev,
};

/// One AION-10 tower at the origin, a short timeline, and `bodies`.
fn scene_with(bodies: Vec<BodySpec>) -> SceneSpec {
    let mut scene = dev::void();
    scene.array.towers = vec![TowerSpec::at("tower 1", [0.0, 0.0, 0.0])];
    scene.duration = 6.0;
    scene.bodies = bodies;
    assert_eq!(scene.validate(), Ok(()));
    scene
}

fn exact(kind: ShapeKind, pitch: f64) -> ShapeSpec {
    let mut shape = ShapeSpec::new(kind, pitch, 1);
    shape.representation = Representation::Exact;
    shape
}

/// The worst relative difference over the three channels of the first shot, between an `Exact`
/// cube and the same cube as a voxel body of one cell (which the cube kernel reads exactly).
fn exact_cube_against_one_cell() -> f64 {
    let side = 0.8;
    let at = [1.7, 0.43, 2.31];
    let cube = ShapeKind::Cuboid { size: [side; 3] };
    let one = ShapeSpec::new(cube.clone(), side, 1);
    let tree = one.tree(250.0, &[]).unwrap();
    assert_eq!(tree.leaves.len(), 1, "a cube on a grid of its own side is one cell");
    let voxels = scene_with(vec![BodySpec::shaped("cube", 250.0, one, Motion::Static { position: at })]);
    let solid =
        scene_with(vec![BodySpec::shaped("cube", 250.0, exact(cube, side), Motion::Static { position: at })]);
    let read = |scene: &SceneSpec| {
        let shots = shots(scene).unwrap();
        let part = contribution(scene, SourceId::Body(0), &shots, &|| false).unwrap();
        assert!(part.settled, "{:?}", part.warnings);
        [part.lower[0], part.upper[0], part.gradiometer[0]]
    };
    let (a, b) = (read(&voxels), read(&solid));
    let worst = |b: &[f64; 3]| a.iter().zip(b).map(|(x, y)| (x - y).abs() / x.abs()).fold(0.0, f64::max);
    // Control: the same cube a millimetre off is seen, far past the tolerance.
    let mut moved = solid.clone();
    moved.bodies[0].motion = Motion::Static { position: [at[0] + 1e-3, at[1], at[2]] };
    let off = worst(&read(&moved));
    assert!(off > 1e-6, "a millimetre is not seen: {off:e}");
    worst(&b)
}

cav_validate::case! {
    test: validation_an_exact_cube_is_the_cube_kernel,
    name: "exact.cube_is_the_cube_kernel",
    subsystem: "voxel",
    slice: "S5",
    reference: ClosedForm,
    against: "an 0.8 m cube 1.7 m off a tower's axis read Exact (the polyhedral closed form) against the same cube as one voxel (the cube kernel, exact); one AION-10 shot, each channel relative",
    unit: Relative,
    tolerance: 1e-12,
    measure: cav_validate::Measured::Error(exact_cube_against_one_cell()),
}

/// The gradiometer's departure from a point mass of the same mass at the centroid, radians, for
/// `kind` (2,000 kg/m³) with its centroid `distance` metres from the tower's line and `height` metres
/// up, both by one fixed Gauss rule (so that the quadrature's own error cancels between them).
fn departure_at(kind: &ShapeKind, distance: f64, height: f64) -> f64 {
    let instrument = Instrument::aion10();
    let reader = Reader::new(instrument);
    let solid = kind.faceted().unwrap();
    let centroid = solid.moments(2000.0).centre;
    let separation = instrument.kick_velocity() * instrument.half_sequence;
    let angle: f64 = 0.3;
    let place = Vec3::new(distance * angle.cos(), distance * angle.sin(), height) - centroid;
    let body = ExactBody::new(solid, 2000.0, separation, move |_| place);
    let point = Mass::point(body.mass(), move |_| place + centroid);
    let (a, b) = (reader.read(Vec3::ZERO, &body, 1.46), reader.read(Vec3::ZERO, &point, 1.46));
    ((a.lower - a.upper) - (b.lower - b.upper)).abs()
}

/// The least-squares slope of `ln` departure on `ln R` over R = 30, 60, 120, 240 m.
fn departure_slope(kind: &ShapeKind) -> f64 {
    departure_slope_at(kind, 5.0)
}

/// [`departure_slope`] with the centroid `height` metres up.
fn departure_slope_at(kind: &ShapeKind, height: f64) -> f64 {
    let points: Vec<(f64, f64)> = [30.0, 60.0, 120.0, 240.0]
        .iter()
        .map(|&r: &f64| (r.ln(), departure_at(kind, r, height).ln()))
        .collect();
    let n = points.len() as f64;
    let (mx, my) = (points.iter().map(|p| p.0).sum::<f64>() / n, points.iter().map(|p| p.1).sum::<f64>() / n);
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = points.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    sxy / sxx
}

/// Each solid with the order `l` of its first multipole past the monopole.
///
/// (The fall-off A2 asks for is `R^−(l+3)`, except the tetrahedron's: see [`expected_slope`].)
fn platonic_orders() -> [(PlatonicSolid, f64); 5] {
    [
        (PlatonicSolid::Tetrahedron, 3.0),
        (PlatonicSolid::Cube, 4.0),
        (PlatonicSolid::Octahedron, 4.0),
        (PlatonicSolid::Dodecahedron, 6.0),
        (PlatonicSolid::Icosahedron, 6.0),
    ]
}

/// The slope A2 expects for `solid`, whose first multipole past the monopole is of order `l`, with
/// its centroid level with the arms' middle (5 m up): `−(l + 3)`, except the tetrahedron's `−(l + 4)`.
///
/// Why the tetrahedron differs (the owner's ruling, 28 Nov 2026, `docs/slices/S5.md` §7.5): its
/// octupole is a pure `xyz` term about its centroid, odd in height, so a vertical gradiometer level
/// with the centroid cannot see it, and what it sees first is its `l = 4` part. The criterion was
/// first written as `−(l + 3)` for all five; that read (−6.961 against −6) is kept in the record.
/// [`the_tetrahedron_s_octupole_is_unseen_level_with_it`] checks that the octupole is there and seen
/// off that level, so this exception cannot hide a missing one.
fn expected_slope(solid: PlatonicSolid, l: f64) -> f64 {
    if solid == PlatonicSolid::Tetrahedron { -(l + 4.0) } else { -(l + 3.0) }
}

/// The worst `|slope − expected|` over the five solids (10 m an edge).
fn platonic_slope_error() -> f64 {
    platonic_orders()
        .into_iter()
        .map(|(solid, l)| {
            let slope = departure_slope(&ShapeKind::Platonic { solid, edge: 10.0 });
            let expected = expected_slope(solid, l);
            eprintln!("{solid:?}: slope {slope:.4} against {expected}");
            (slope - expected).abs()
        })
        .fold(0.0, f64::max)
}

/// The slope of the departure with the centroid at a fixed elevation of 30° above the arms' middle
/// (at `5 m + R tan 30°`), over the same `R`.
fn departure_slope_at_30_degrees(kind: &ShapeKind) -> f64 {
    let points: Vec<(f64, f64)> = [30.0, 60.0, 120.0, 240.0]
        .iter()
        .map(|&r: &f64| (r.ln(), departure_at(kind, r, 5.0 + r * 30f64.to_radians().tan()).ln()))
        .collect();
    let n = points.len() as f64;
    let (mx, my) = (points.iter().map(|p| p.0).sum::<f64>() / n, points.iter().map(|p| p.1).sum::<f64>() / n);
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = points.iter().map(|p| (p.0 - mx) * (p.0 - mx)).sum();
    sxy / sxx
}

#[test]
fn the_tetrahedron_s_octupole_is_unseen_level_with_it() {
    // The owner's ruling on A2: level with the centroid the tetrahedron falls as R^−7, its
    // octupole unseen. Control: at a fixed 30° elevation the same octupole is seen, R^−6, so the
    // solid does have it and the level read is not a solid missing its octupole.
    let tetrahedron = ShapeKind::Platonic { solid: PlatonicSolid::Tetrahedron, edge: 10.0 };
    let level = departure_slope(&tetrahedron);
    let raised = departure_slope_at_30_degrees(&tetrahedron);
    eprintln!("tetrahedron: slope {level:.4} level with it, {raised:.4} at 30°");
    assert!((level + 7.0).abs() <= 0.2, "level: {level}");
    assert!((raised + 6.0).abs() <= 0.2, "at 30°: {raised}");
}

cav_validate::case! {
    test: validation_platonic_solids_miss_their_low_multipoles,
    name: "exact.platonic_solids_miss_their_low_multipoles",
    subsystem: "voxel",
    slice: "S5",
    reference: ClosedForm,
    against: "the AION-10 gradiometer's departure from a point mass of the same mass, Platonic solids of 10 m an edge read Exact, R = 30 to 240 m: its log-log slope against −(l+3), l = 3 (tetrahedron), 4 (cube, octahedron), 6 (dodecahedron, icosahedron); the worst |slope + l + 3|, the tetrahedron at |slope + l + 4| (restated after measuring with the owner's approval, S5 §7.5: its xyz octupole is unseen level with its centroid)",
    unit: Slope,
    tolerance: 0.2,
    measure: cav_validate::Measured::Error(platonic_slope_error()),
}

#[test]
fn a_squashed_cube_has_a_quadrupole() {
    // Control for A2: a cube squashed by 1 % has a quadrupole, so it departs as R^−5.
    let slope = departure_slope(&ShapeKind::Cuboid { size: [10.0, 10.0, 9.9] });
    assert!((slope + 5.0).abs() <= 0.2, "{slope}");
}

#[test]
fn an_exact_body_is_a_source_like_any_other() {
    let ball = ShapeSpec::new(ShapeKind::Sphere { radius: 0.3 }, 0.3 / 8.0, 2);
    let scene = scene_with(vec![
        BodySpec::shaped(
            "icosahedron",
            100.0,
            exact(ShapeKind::Platonic { solid: PlatonicSolid::Icosahedron, edge: 0.6 }, 0.05),
            Motion::Static { position: [1.5, 0.6, 2.5] },
        ),
        BodySpec::shaped("ball", 80.0, ball, Motion::Static { position: [-1.2, 0.9, 3.0] }),
        BodySpec::point("point", 50.0, [0.7, -2.1, 4.0]),
    ]);
    let shots = shots(&scene).unwrap();
    let time = shots.readout(0, 0);
    let never = || false;
    let whole = reading(&scene, time, &never).unwrap();
    assert!(whole.settled, "{:?}", whole.warnings);
    let parts: Vec<_> = (0..3).map(|n| reading(&only(&scene, &[n]), time, &never).unwrap()).collect();
    let (w, size) = (&whole.towers[0], whole.towers[0].lower.abs().max(whole.towers[0].upper.abs()));
    for (got, pick) in [
        (w.lower, (|p: &cav_sim::read::Phases| p.lower) as fn(&cav_sim::read::Phases) -> f64),
        (w.upper, |p| p.upper),
        (w.gradiometer, |p| p.gradiometer),
    ] {
        let sum: f64 = parts.iter().map(|r| pick(&r.towers[0])).sum();
        assert!((got - sum).abs() <= 1e-15 * size, "{got} against {sum}");
    }
    // Control: leaving the exact body out is seen.
    let without: f64 = parts[1..].iter().map(|r| r.towers[0].gradiometer).sum();
    assert!((w.gradiometer - without).abs() > 1e-6 * size);
    // The field is written for Exact and not for Sampled, and the scene round-trips.
    let text = scene.to_ron();
    assert_eq!(text.matches("representation").count(), 1, "{text}");
    assert_eq!(SceneSpec::from_ron(&text).unwrap(), scene);
    assert_eq!(SceneSpec::from_json(&scene.to_json()).unwrap(), scene);
}

#[test]
fn an_exact_body_is_bounded() {
    use cav_core::voxel::mesh::{to_stl, torus_mesh};
    // A torus of 100 × 51 × 2 = 10,200 triangles, held, is refused with its count before its solid
    // is made; one of 100 × 50 × 2 = 10,000 is read.
    let held = |across: u32| {
        let (v, t) = torus_mesh(1.0, 0.3, 100, across);
        let (hash, _) = cav_sim::assets::hold(&format!("torus-{across}.stl"), &to_stl(&v, &t)).unwrap();
        (t.len(), ShapeKind::Mesh { file: format!("torus-{across}.stl"), scale: 1.0, hash: Some(hash) })
    };
    let (count, big) = held(51);
    assert!(count > MAX_EXACT_FACES);
    let why = big.faceted().unwrap_err();
    assert!(why.contains(&count.to_string()) && why.contains("10000"), "{why}");
    let (count, fits) = held(50);
    assert_eq!(count, MAX_EXACT_FACES);
    assert_eq!(fits.faceted().unwrap().faces().len(), MAX_EXACT_FACES);
    // Through a scene: the big one is not read, and says why.
    let scene = scene_with(vec![BodySpec::shaped(
        "ring",
        10.0,
        exact(big, 0.05),
        Motion::Static { position: [3.0, 0.0, 2.0] },
    )]);
    let part = contribution(&scene, SourceId::Body(0), &shots(&scene).unwrap(), &|| false).unwrap();
    assert!(!part.settled && part.warnings.iter().any(|w| w.contains("10200")), "{:?}", part.warnings);
    // A curved solid and a body with parts are refused, naming the field.
    let mut sphere = scene_with(vec![]);
    sphere.bodies.push(BodySpec::shaped(
        "ball",
        1.0,
        exact(ShapeKind::Sphere { radius: 0.3 }, 0.05),
        Motion::Static { position: [2.0, 0.0, 1.0] },
    ));
    let why = sphere.validate().unwrap_err().0;
    assert!(why.starts_with("bodies[0].shape.representation") && why.contains("Sphere"), "{why}");
    let mut parted = exact(ShapeKind::Cuboid { size: [0.5; 3] }, 0.05);
    parted.material = Some("steel".into());
    parted.parts.push(cav_sim::shape::PartSpec {
        mode: cav_sim::shape::PartMode::Add,
        material: Some("lead".into()),
        at: [0.4, 0.0, 0.0],
        kind: cav_sim::shape::PartKind::Solid { shape: ShapeKind::Cuboid { size: [0.2; 3] } },
    });
    let why = parted.validate("shape").unwrap_err();
    assert!(why.starts_with("shape.representation") && why.contains("parts"), "{why}");
    // Control: the same body Sampled is valid.
    parted.representation = Representation::Sampled;
    assert_eq!(parted.validate("shape"), Ok(()));
}

#[test]
#[ignore = "a diagnostic: A2's tetrahedron by the height of its centroid (S5 §7.5)"]
fn tetrahedron_by_height() {
    let tetrahedron = ShapeKind::Platonic { solid: PlatonicSolid::Tetrahedron, edge: 10.0 };
    let cube = ShapeKind::Platonic { solid: PlatonicSolid::Cube, edge: 10.0 };
    for height in [-30.0, -10.0, 0.0, 2.5, 5.0, 7.5, 20.0, 40.0] {
        println!(
            "height {height} m: tetrahedron {:.4}, cube {:.4}; tetrahedron's departure at 30 m {:.3e} rad, at 240 m {:.3e} rad",
            departure_slope_at(&tetrahedron, height),
            departure_slope_at(&cube, height),
            departure_at(&tetrahedron, 30.0, height),
            departure_at(&tetrahedron, 240.0, height)
        );
    }
}

#[test]
#[ignore = "a diagnostic: A2 at a fixed elevation of 30° (the centroid at 5 m + R tan 30°) (S5 §7.5)"]
fn platonic_slopes_at_a_fixed_elevation() {
    for (solid, l) in platonic_orders() {
        let slope = departure_slope_at_30_degrees(&ShapeKind::Platonic { solid, edge: 10.0 });
        println!("{solid:?}: slope {slope:.4} against {}", -(l + 3.0));
    }
}

#[test]
fn the_exact_solids_read_as_their_voxels_do() {
    // `exact-solids` is `platonic-row` read Exact: each body's first shot within the voxels' own
    // error of the voxel reading (0.6 m solids at 5 cm), and settled.
    let (exact, voxels) = (dev::exact_solids(), dev::platonic_row());
    let shots = shots(&exact).unwrap();
    let mut worst = 0f64;
    for n in 0..exact.bodies.len() {
        let a = contribution(&exact, SourceId::Body(n), &shots, &|| false).unwrap();
        let b = contribution(&voxels, SourceId::Body(n), &shots, &|| false).unwrap();
        assert!(a.settled, "{:?}", a.warnings);
        worst = worst.max((a.gradiometer[0] - b.gradiometer[0]).abs() / b.gradiometer[0].abs());
    }
    eprintln!("exact-solids against platonic-row: worst {worst:.3e} of the gradiometer");
    assert!(worst < 1e-4 && worst > 0.0, "{worst}");
}

#[test]
#[ignore = "a diagnostic: the time of exact-solids' read against platonic-row's (release; S5 §7.5)"]
fn exact_solids_read_time() {
    for scene in [dev::platonic_row(), dev::exact_solids()] {
        let shots = shots(&scene).unwrap();
        let started = std::time::Instant::now();
        for n in 0..scene.bodies.len() {
            let _ = contribution(&scene, SourceId::Body(n), &shots, &|| false).unwrap();
        }
        println!("{}: {:.4} s for its five bodies", scene.name, started.elapsed().as_secs_f64());
    }
}
