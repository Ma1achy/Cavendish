//! Bodies made of voxels in a scene: as data, edited by path, and read (the S3 brief, R9).

use cav_core::Vec3;
use cav_core::forward::Mass;
use cav_sim::read::{SourceId, contribution, shots};
use cav_sim::{BodySpec, Motion, SceneSpec, ShapeKind, ShapeSpec, TowerSpec, dev, paths};

/// One AION-10 tower at the origin, a short timeline, and `body`.
fn scene_with(body: BodySpec) -> SceneSpec {
    let mut scene = dev::void();
    scene.array.towers = vec![TowerSpec::at("tower 1", [0.0, 0.0, 0.0])];
    scene.duration = 6.0;
    scene.bodies = vec![body];
    assert_eq!(scene.validate(), Ok(()));
    scene
}

fn sphere(pitch: f64) -> ShapeSpec {
    ShapeSpec::new(ShapeKind::Sphere { radius: 0.5 }, pitch, 4)
}

#[test]
fn a_voxel_body_round_trips_and_edits_by_path() {
    let scene = scene_with(BodySpec::shaped(
        "ball",
        80.0,
        sphere(0.05),
        Motion::Static { position: [2.0, 0.0, 3.0] },
    ));
    assert_eq!(SceneSpec::from_ron(&scene.to_ron()).unwrap(), scene);
    assert_eq!(SceneSpec::from_json(&scene.to_json()).unwrap(), scene);
    // Every field has a path, a unit and a range.
    let mut value = scene.to_value();
    assert_eq!(paths::get(&value, "bodies[0].shape.pitch").unwrap(), 0.05);
    paths::set(&mut value, "bodies[0].shape.kind.Sphere.radius", 0.25.into()).unwrap();
    let edited: SceneSpec = serde_json::from_value(value).unwrap();
    assert_eq!(edited.bodies[0].shape.as_ref().unwrap().kind, ShapeKind::Sphere { radius: 0.25 });
    assert_eq!(paths::schema_at("bodies[0].shape.pitch").unwrap()["x-unit"], "m");
    assert!(paths::fields_without_a_unit(&scene.to_value()).is_empty());
    // A scene file written before shapes existed still loads: the field is optional.
    let mut old = scene.clone();
    old.bodies[0].shape = None;
    let text = old.to_ron().replace("shape: None,", "");
    assert!(!text.contains("shape"));
    assert_eq!(SceneSpec::from_ron(&text).unwrap(), old);
    // Refusals name the field: a shape with a radius, a pitch too fine for one tree.
    let mut both = scene.clone();
    both.bodies[0].radius = 0.5;
    assert!(both.validate().unwrap_err().0.starts_with("bodies[0].radius"));
    let mut fine = scene;
    fine.bodies[0].shape.as_mut().unwrap().pitch = 1e-4;
    assert!(fine.validate().unwrap_err().0.starts_with("bodies[0].shape.pitch"));
}

/// The worst relative difference over the three channels of one shot between a voxel sphere at
/// `pitch` and the analytic sphere (a point outside itself).
fn sphere_against_point(pitch: f64) -> f64 {
    let at = [1.4, 0.6, 2.2];
    let scene = scene_with(BodySpec::shaped("ball", 100.0, sphere(pitch), Motion::Static { position: at }));
    let shots = shots(&scene).unwrap();
    let voxels = contribution(&scene, SourceId::Body(0), &shots, &|| false).unwrap();
    assert!(voxels.settled, "{:?}", voxels.warnings);
    let reader = cav_core::forward::Reader::new(scene.array.instrument.instrument());
    let point = Mass::point(100.0, |_| Vec3::from_array(at));
    let exact =
        reader.read_to(Vec3::ZERO, &point, shots.times(0)[0], cav_core::forward::PHASE_TOLERANCE).reading;
    let relative = |a: f64, b: f64| (a - b).abs() / b.abs();
    relative(voxels.lower[0], exact.lower)
        .max(relative(voxels.upper[0], exact.upper))
        .max(relative(voxels.gradiometer[0], exact.lower - exact.upper))
}

cav_validate::case! {
    test: validation_slow_a_voxel_sphere_reads_as_the_sphere,
    name: "voxel.sphere_reads_as_the_sphere",
    subsystem: "voxel",
    slice: "S3",
    reference: ClosedForm,
    against: "a uniform sphere (a point outside itself), r = 0.5 m, 100 kg, 1.6 m off a tower's axis: one AION-10 shot, three channels, voxelised at r/16 and k = 4",
    unit: Relative,
    tolerance: 2e-3,
    measure: cav_validate::Measured::Error(sphere_against_point(0.5 / 16.0)),
}

#[test]
fn a_static_body_reads_once_to_the_same_bits() {
    let still = scene_with(BodySpec::shaped(
        "cube",
        50.0,
        ShapeSpec::new(ShapeKind::Cuboid { size: [0.4, 0.3, 0.2] }, 0.05, 2),
        Motion::Static { position: [1.5, -0.5, 4.0] },
    ));
    // The same body, there "from" long before the timeline: not still in the read's eyes, so
    // read shot by shot.
    let mut shot_by_shot = still.clone();
    shot_by_shot.bodies[0].appears = Some(-1.0e5);
    let shots = shots(&still).unwrap();
    assert!(shots.count(0) >= 2);
    let a = contribution(&still, SourceId::Body(0), &shots, &|| false).unwrap();
    let b = contribution(&shot_by_shot, SourceId::Body(0), &shots, &|| false).unwrap();
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&a.gradiometer), bits(&b.gradiometer));
    assert_eq!(bits(&a.lower), bits(&b.lower));
    assert!(a.gradiometer.iter().all(|v| v.to_bits() == a.gradiometer[0].to_bits()));
    // Control: a body that moves reads differently from shot to shot.
    let mut moving = still;
    moving.bodies[0].motion =
        Motion::Linear { centre: [1.5, -0.5, 4.0], amplitude: [0.3, 0.0, 0.0], frequency: 0.1, phase: 0.0 };
    let c = contribution(&moving, SourceId::Body(0), &shots, &|| false).unwrap();
    assert_ne!(c.gradiometer[0].to_bits(), c.gradiometer[1].to_bits());
}

#[test]
fn a_body_that_cannot_be_voxelised_is_said_and_read_as_nothing() {
    let missing =
        ShapeSpec::new(ShapeKind::Mesh { file: "no/such/mesh.obj".into(), scale: 1.0, hash: None }, 0.05, 2);
    let scene =
        scene_with(BodySpec::shaped("ghost", 5.0, missing, Motion::Static { position: [2.0, 0.0, 1.0] }));
    let part = contribution(&scene, SourceId::Body(0), &shots(&scene).unwrap(), &|| false).unwrap();
    assert!(!part.settled);
    assert!(
        part.warnings.iter().any(|w| w.contains("ghost") && w.contains("no/such/mesh.obj")),
        "{:?}",
        part.warnings
    );
    assert!(part.gradiometer.iter().all(|v| *v == 0.0));
}

/// A uniform rectangular prism of `density` from `lo` to `hi` (metres, the world's frame), read in
/// closed form: the planar mass's reference.
struct Prism {
    density: f64,
    lo: Vec3,
    hi: Vec3,
}

impl cav_core::forward::Source for Prism {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _: f64) -> [f64; 2] {
        use cav_core::voxel::kernel::prism_potential;
        heights.map(|h| {
            let at = |rise: f64| tower + Vec3::new(0.0, 0.0, h + rise);
            prism_potential(self.density, self.lo - at(upper), self.hi - at(upper))
                - prism_potential(self.density, self.lo - at(lower), self.hi - at(lower))
        })
    }
}

/// The planar mass: a 2 m × 2 m × 0.25 m plate of 2400 kg/m³ centred 1 m below a tower's base and
/// 0.4 m to its side, voxelised on a grid that fits its faces (0.125 m), against the closed-form
/// prism: one shot's δφ₁, δφ₂, ΔΦ, as a fraction of the larger of |δφ₁|, |δφ₂|.
fn plate_against_prism() -> f64 {
    let (size, thickness, density) = (2.0, 0.25, 2400.0);
    let centre = [0.4, 0.0, -1.0];
    let shape = ShapeSpec::new(ShapeKind::Cuboid { size: [size, size, thickness] }, 0.125, 2);
    let mass = density * size * size * thickness;
    let scene = scene_with(BodySpec::shaped("plate", mass, shape, Motion::Static { position: centre }));
    let shots = shots(&scene).unwrap();
    let voxels = contribution(&scene, SourceId::Body(0), &shots, &|| false).unwrap();
    let reader = cav_core::forward::Reader::new(scene.array.instrument.instrument());
    let half = Vec3::new(0.5 * size, 0.5 * size, 0.5 * thickness);
    let c = Vec3::from_array(centre);
    let prism = Prism { density, lo: c - half, hi: c + half };
    let exact =
        reader.read_to(Vec3::ZERO, &prism, shots.times(0)[0], cav_core::forward::PHASE_TOLERANCE).reading;
    let scale = exact.lower.abs().max(exact.upper.abs());
    (voxels.lower[0] - exact.lower)
        .abs()
        .max((voxels.upper[0] - exact.upper).abs())
        .max((voxels.gradiometer[0] - (exact.lower - exact.upper)).abs())
        / scale
}

cav_validate::case! {
    test: validation_the_plate_is_the_prism,
    name: "voxel.planar_mass_is_the_prism",
    subsystem: "voxel",
    slice: "S3",
    reference: ClosedForm,
    against: "planar_mass (no script): a 2 × 2 × 0.25 m plate voxelised on a grid that fits its faces, against the closed-form prism; one AION-10 shot, three channels over the larger single interferometer's phase",
    unit: Relative,
    tolerance: 1e-12,
    measure: cav_validate::Measured::Error(plate_against_prism()),
}

/// A box of `size` metres a side made of `material` (a library label), or uniform of `mass`.
fn box_of(side: f64, material: Option<&str>) -> ShapeSpec {
    ShapeSpec {
        material: material.map(str::to_owned),
        ..ShapeSpec::new(ShapeKind::Cuboid { size: [side; 3] }, 0.05, 2)
    }
}

/// The largest difference over every shot, radians, between the gradiometer reading of a body of
/// two materials (a steel box with a lead box added beside it, sharing a face, as one body) and
/// the sum of the two boxes read as two uniform bodies of ρV each; and the two masses' relative
/// difference. `second` is the material of the box added (lead; steel for the control).
fn two_materials_against_parts(second: &str) -> (f64, f64, f64) {
    use cav_sim::shape::{PartKind, PartMode, PartSpec};
    let at = [1.5, 0.0, 3.0];
    let side = 0.4;
    let mut one = box_of(side, Some("steel"));
    one.parts.push(PartSpec {
        mode: PartMode::Add,
        material: Some(second.into()),
        at: [side, 0.0, 0.0],
        kind: PartKind::Solid { shape: ShapeKind::Cuboid { size: [side; 3] } },
    });
    let whole = scene_with(BodySpec::shaped("two", 1.0, one.clone(), Motion::Static { position: at }));
    let density = |label: &str| cav_sim::materials::find(&[], label).unwrap().density;
    let volume = side * side * side;
    let mut apart = whole.clone();
    apart.bodies = vec![
        BodySpec::shaped(
            "steel",
            density("steel") * volume,
            box_of(side, None),
            Motion::Static { position: at },
        ),
        BodySpec::shaped(
            "lead",
            density("lead") * volume,
            box_of(side, None),
            Motion::Static { position: [at[0] + side, at[1], at[2]] },
        ),
    ];
    assert_eq!(apart.validate(), Ok(()));
    let shots = shots(&whole).unwrap();
    let read = |scene: &SceneSpec, n| {
        let c = contribution(scene, SourceId::Body(n), &shots, &|| false).unwrap();
        assert!(c.settled, "{:?}", c.warnings);
        c.gradiometer
    };
    let joined = read(&whole, 0);
    let (a, b) = (read(&apart, 0), read(&apart, 1));
    let worst =
        joined.iter().zip(a.iter().zip(&b)).map(|(j, (a, b))| (j - (a + b)).abs()).fold(0.0, f64::max);
    let size = joined.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let tree = one.tree(1.0, &[]).unwrap();
    let mass = (density("steel") + density("lead")) * volume;
    (worst, size, (tree.body().mass - mass).abs() / mass)
}

cav_validate::case! {
    test: validation_two_material_body_is_the_sum_of_its_parts,
    name: "materials.two_material_body_is_the_sum_of_its_parts",
    subsystem: "materials",
    slice: "S19",
    reference: Exact,
    against: "a steel box and a lead box (0.4 m, sharing a face) as one body of two materials, against the two read as separate uniform bodies of density × volume: tower 1's gradiometer phase over every shot, beside it on a 6 s timeline",
    unit: Radians,
    tolerance: 1e-9,
    measure: {
        let (worst, size, mass) = two_materials_against_parts("lead");
        assert!(size > 1e-3, "the body is heard: {size:e} rad");
        assert!(mass < 1e-12, "and weighs ρV + ρV: {mass:e}");
        // Control: the box added as steel is another body, by far more than the tolerance.
        let (broken, _, broken_mass) = two_materials_against_parts("steel");
        assert!(broken > 1e-3 * size && broken_mass > 0.1, "{broken:e} {broken_mass:e}");
        cav_validate::Measured::Error(worst)
    },
}

/// Tower 1's gradiometer phase of one shot of a point of 1 kg at `p`, radians.
fn one_kilogram(scene: &SceneSpec, p: Vec3) -> f64 {
    let reader = cav_core::forward::Reader::new(scene.array.instrument.instrument());
    let time = shots(scene).unwrap().times(0)[0];
    let point = Mass::point(1.0, move |_| p);
    let r = reader.read_to(Vec3::ZERO, &point, time, cav_core::forward::PHASE_TOLERANCE).reading;
    r.lower - r.upper
}

/// A body of two materials whose interface lies in the middle of a layer of cells (S19 review,
/// minor 1): a steel cuboid 0.175 × 0.2 × 0.2 m with a lead one of the same size added beside it
/// along x, at a pitch of 0.05 m and k = 2. The outer faces lie on cell faces and the interface
/// halfway through the cells between (one sample each side), so 16 cells are shared and every
/// sample is exact: the only approximation is that a shared cell is read as one uniform cube of
/// both masses. Returns (|ΔΦ − ΔΦ of the two prisms|, the shared cells' first-order dipole bound,
/// both radians, one shot of tower 1, and how many cells are shared). `first`/`second` are the
/// two materials (steel, lead; swapped for the control).
fn shared_cells_against_prisms(first: &str, second: &str) -> (f64, f64, usize) {
    use cav_sim::shape::{PartKind, PartMode, PartSpec};
    let (a, b, pitch) = (0.175, 0.2, 0.05);
    let at = [1.5, 0.0, 3.0];
    let mut shape = ShapeSpec::new(ShapeKind::Cuboid { size: [a, b, b] }, pitch, 2);
    shape.material = Some(first.into());
    shape.parts.push(PartSpec {
        mode: PartMode::Add,
        material: Some(second.into()),
        at: [a, 0.0, 0.0],
        kind: PartKind::Solid { shape: ShapeKind::Cuboid { size: [a, b, b] } },
    });
    let scene = scene_with(BodySpec::shaped("two", 1.0, shape.clone(), Motion::Static { position: at }));
    let tree = shape.tree(1.0, &[]).unwrap();
    let shots = shots(&scene).unwrap();
    let voxels = contribution(&scene, SourceId::Body(0), &shots, &|| false).unwrap();
    assert!(voxels.settled, "{:?}", voxels.warnings);
    // The reference: steel and lead as the two prisms they are (always steel, then lead).
    let density = |label: &str| cav_sim::materials::find(&[], label).unwrap().density;
    let c = Vec3::from_array(at);
    let half = Vec3::new(0.5 * a, 0.5 * b, 0.5 * b);
    let reader = cav_core::forward::Reader::new(scene.array.instrument.instrument());
    let time = shots.times(0)[0];
    let read = |density: f64, centre: Vec3| {
        let prism = Prism { density, lo: centre - half, hi: centre + half };
        let r = reader.read_to(Vec3::ZERO, &prism, time, cav_core::forward::PHASE_TOLERANCE).reading;
        r.lower - r.upper
    };
    let exact = read(density("steel"), c) + read(density("lead"), c + Vec3::new(a, 0.0, 0.0));
    // The bound: a shared cell read as a uniform cube has the right mass but not the right
    // centre. Its dipole error, about its centre, is at most |Δρ|·v·h/8 (the excess density over
    // the lighter material filling half the cell, its centroid h/4 off: the worst case, which this
    // body attains), and to first order it moves the phase by that times ∂ΔΦ/∂x of a kilogram
    // there.
    let (v, h) = (pitch.powi(3), pitch);
    let dipole = (density(first) - density(second)).abs() * v * h / 8.0;
    let step = 1e-3;
    let bound: f64 = tree
        .mixed
        .iter()
        .map(|cell| {
            let centre = c + tree.block_centre(cell.key, 0);
            let dx = Vec3::new(step, 0.0, 0.0);
            let slope =
                (one_kilogram(&scene, centre + dx) - one_kilogram(&scene, centre - dx)) / (2.0 * step);
            dipole * slope.abs()
        })
        .sum();
    ((voxels.gradiometer[0] - exact).abs(), bound, tree.mixed.len())
}

cav_validate::case! {
    test: validation_a_shared_cell_is_within_its_dipole_bound,
    name: "materials.shared_cells_are_within_their_dipole_bound",
    subsystem: "materials",
    slice: "S19",
    reference: ClosedForm,
    against: "a steel cuboid and a lead one (0.175 × 0.2 × 0.2 m each, side by side along x) as one body at 0.05 m pitch, k = 2, the interface halfway through a layer of 16 cells: tower 1's gradiometer phase of one shot against the two closed-form prisms, as a fraction of the shared cells' first-order dipole bound Σ |Δρ|·v·h/8 · |∂ΔΦ/∂x| (1 % over it allowed for the higher orders, (h/r)² ≈ 1e-3 here)",
    unit: Relative,
    tolerance: 1.01,
    measure: {
        let (error, bound, shared) = shared_cells_against_prisms("steel", "lead");
        assert_eq!(shared, 16, "the interface's layer of cells is shared");
        // The case is not vacuous: the shared cells' error is most of the bound (it is attained
        // here) and far above the read's rounding.
        assert!(error > 0.5 * bound && bound > 1e-9, "{error:e} rad against a bound of {bound:e} rad");
        // Control: the materials the wrong way round are not the two prisms, by far more than the
        // bound.
        let (swapped, swapped_bound, _) = shared_cells_against_prisms("lead", "steel");
        assert!(swapped > 10.0 * swapped_bound, "{swapped:e} against {swapped_bound:e}");
        cav_validate::Measured::Error(error / bound)
    },
}

/// An STL box imported (1 m × 0.5 m × 0.25 m, sides an STL's single precision holds exactly;
/// steel, pitch 0.05 m) against the `Cuboid` body of
/// the same size, material and pitch, beside tower 1: (whether the leaves are the same, bit for
/// bit; the largest relative difference of the gradiometer phase over every shot; the imported
/// tree's mass against ρV, relative). `grow` scales the imported box (1 for the case, 1.1 for its
/// control).
fn imported_box_against_cuboid(grow: f64) -> (bool, f64, f64) {
    use cav_core::voxel::mesh::{box_mesh, to_stl};
    let half = Vec3::new(0.5, 0.25, 0.125);
    let (v, t) = box_mesh(-half, half);
    let bytes = to_stl(&v, &t);
    let at = [1.5, 0.0, 3.0];
    let base = scene_with(BodySpec::point("placeholder", 1.0, [9.0, 9.0, 9.0]));
    let options = cav_sim::import::Options {
        size: cav_sim::import::Size::Scale(grow),
        material: "steel".into(),
        pitch: Some(0.05),
        at,
    };
    let imported = cav_sim::import::body(&base, "box.stl", &bytes, None, &options).unwrap();
    let mut cuboid = box_of(1.0, Some("steel"));
    cuboid.kind = ShapeKind::Cuboid { size: [1.0, 0.5, 0.25] };
    let analytic = BodySpec::shaped("cuboid", 1.0, cuboid, Motion::Static { position: at });
    let read = |body: BodySpec| {
        let scene = scene_with(body.clone());
        let tree = body.shape.as_ref().unwrap().tree(body.mass, &scene.materials).unwrap();
        let part = contribution(&scene, SourceId::Body(0), &shots(&scene).unwrap(), &|| false).unwrap();
        assert!(part.settled, "{:?}", part.warnings);
        (tree, part.gradiometer)
    };
    let (mine, phase) = read(imported);
    let (theirs, expected) = read(analytic);
    let same = mine.leaves == theirs.leaves && mine.grid == theirs.grid;

    let worst = phase.iter().zip(&expected).map(|(a, b)| ((a - b) / b).abs()).fold(0.0, f64::max);
    let mass = 7850.0 * 0.125 * grow.powi(3);
    (same, worst, (mine.body().mass - mass).abs() / mass)
}

cav_validate::case! {
    test: validation_slow_imported_box_mesh_is_the_cuboid,
    name: "materials.imported_box_mesh_is_the_cuboid",
    subsystem: "materials",
    slice: "S19",
    reference: Exact,
    against: "an STL box (1 m × 0.5 m × 0.25 m) imported as a steel body, against the Cuboid body of the same size, material and pitch (0.05 m): the same leaves, and tower 1's gradiometer phase over every shot of a 6 s timeline, relative",
    unit: Relative,
    tolerance: 1e-12,
    measure: {
        let (same, worst, mass) = imported_box_against_cuboid(1.0);
        assert!(same, "the same leaves on the same grid");
        assert!(mass < 1e-12, "and it weighs ρV: {mass:e}");
        // Control: the box imported 1 % larger (as first written) is another body: another grid,
        // and a phase beyond the tolerance (measured 1.2e-7: the grid re-centred, though each
        // face moves less than the quarter cell to the nearest sample point).
        let (same, broken, _) = imported_box_against_cuboid(1.01);
        assert!(!same && broken > 1e-12, "{broken:e}");
        cav_validate::Measured::Error(worst)
    },
}

/// A uniform cube read whole by the cube kernel (exact to rounding, its difference bounded: S5
/// §7.4): the reference for a voxel cube.
struct Cube {
    mass: f64,
    side: f64,
    at: Vec3,
}

impl cav_core::forward::Source for Cube {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, _time: f64) -> [f64; 2] {
        let d = self.at - tower;
        heights.map(|h| {
            cav_core::voxel::kernel::cube_difference(
                self.mass,
                self.side,
                [-d.x, -d.y],
                h + lower - d.z,
                h + upper - d.z,
                lower - upper,
            )
        })
    }
}

/// S5's representations through a scene (TASK-S5-09): a body's `representation` is data (written
/// only when not `Sampled`), each one reads through the forward model, the cut ones nearer the
/// true cube than the sampled count (a 0.8 m cube at 6.37 cells across, so its faces are off the
/// lattice's planes, where the sampled count is first order: C2), and `Sampled` the same bits as a
/// scene without the field. (First written against a sphere at r/6.3, which `Fill` did not beat:
/// a sphere on a centred lattice is where the samples do well; `docs/slices/S5.md` §7.6.)
#[test]
fn each_representation_reads_through_the_scene() {
    use cav_sim::Representation;
    let at = [1.4, 0.6, 2.2];
    let read = |representation: Representation| {
        let shape = ShapeSpec {
            representation,
            ..ShapeSpec::new(ShapeKind::Cuboid { size: [0.8; 3] }, 0.8 / 6.37, 4)
        };
        let scene = scene_with(BodySpec::shaped("cube", 100.0, shape, Motion::Static { position: at }));
        let shots = shots(&scene).unwrap();
        let voxels = contribution(&scene, SourceId::Body(0), &shots, &|| false).unwrap();
        assert!(voxels.settled, "{representation:?}: {:?}", voxels.warnings);
        assert!((scene.bodies[0].mass_in(&scene).unwrap() - 100.0).abs() < 1e-12 * 100.0);
        (scene, shots, voxels)
    };
    let (scene, shots, sampled) = read(Representation::Sampled);
    let text = scene.to_ron();
    assert!(!text.contains("representation"), "Sampled is not written");
    let reader = cav_core::forward::Reader::new(scene.array.instrument.instrument());
    let cube = Cube { mass: 100.0, side: 0.8, at: Vec3::from_array(at) };
    let exact =
        reader.read_to(Vec3::ZERO, &cube, shots.times(0)[0], cav_core::forward::PHASE_TOLERANCE).reading;
    let error = |v: &cav_sim::read::Contribution| {
        (v.lower[0] - exact.lower).abs().max((v.upper[0] - exact.upper).abs())
    };
    let sampled_error = error(&sampled);
    for representation in [Representation::Fill, Representation::Centroid, Representation::Second] {
        let (scene, _, cut) = read(representation);
        let e = error(&cut);
        eprintln!("{representation:?}: {e:.3e} rad against Sampled's {sampled_error:.3e}");
        assert!(e < sampled_error, "{representation:?}: {e:e} against {sampled_error:e}");
        // Data: written, and read back.
        assert!(scene.to_ron().contains(&format!("representation: {representation:?}")));
        assert_eq!(SceneSpec::from_ron(&scene.to_ron()).unwrap(), scene);
    }
    // Sampled is the same bits as a scene whose file has no such field.
    let (_, _, again) = read(Representation::Sampled);
    assert_eq!(again.lower[0].to_bits(), sampled.lower[0].to_bits());
    assert_eq!(SceneSpec::from_ron(&text).unwrap(), scene);
    // A body with parts and a cut representation is refused by the field.
    let mut parts = scene.clone();
    let shape = parts.bodies[0].shape.as_mut().unwrap();
    shape.material = Some("steel".into());
    shape.representation = Representation::Fill;
    shape.parts = vec![cav_sim::shape::PartSpec {
        mode: cav_sim::shape::PartMode::Erase,
        material: None,
        at: [0.0; 3],
        kind: cav_sim::shape::PartKind::Solid { shape: ShapeKind::Sphere { radius: 0.1 } },
    }];
    assert!(parts.validate().unwrap_err().0.starts_with("bodies[0].shape.representation"));
}
