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
    ShapeSpec { kind: ShapeKind::Sphere { radius: 0.5 }, pitch, supersample: 4 }
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
    test: validation_a_voxel_sphere_reads_as_the_sphere,
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
        ShapeSpec { kind: ShapeKind::Cuboid { size: [0.4, 0.3, 0.2] }, pitch: 0.05, supersample: 2 },
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
    let missing = ShapeSpec {
        kind: ShapeKind::Mesh { file: "no/such/mesh.obj".into(), scale: 1.0 },
        pitch: 0.05,
        supersample: 2,
    };
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
    let shape =
        ShapeSpec { kind: ShapeKind::Cuboid { size: [size, size, thickness] }, pitch: 0.125, supersample: 2 };
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
