//! The editable scenario spec the App holds. A `Scenario` cannot be stored — its `source` is a
//! `Box<dyn SourceDynamics>`, not `Clone` — so the App keeps this plain-data spec and `build_scenario`
//! realises a fresh `Scenario` each run. The spec is `Clone + Send` (plain enums/numbers), which is what
//! lets the background Run thread take a copy. Presets (`crate::presets`) are values of this type.

use std::path::PathBuf;

use generate::{
    AtmoConfig, Detector, DetectorArray, FieldSet, Isometry3, NoiseSource, NoiseStack, Orient,
    Path, PhaseModelKind, Quat, Scenario, Schedule, ShotNoise, Source, Timing, Trajectory,
    UldmConfig, Vec3, VibrationResidual,
};
use gravity::Cloud;
use shape::{
    load_solid, principal_frame, voxelise, voxelise_mesh, Cuboid, Cylinder, MassSpec, MeshImport,
    MeshSolid, Shell, Solid, Sphere, Translated, TriSoup, Union, VoxelParams,
};

/// A world axis — the direction a detector line marches, or a motion/oscillation axis in the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn unit(self) -> [f64; 3] {
        match self {
            Axis::X => [1.0, 0.0, 0.0],
            Axis::Y => [0.0, 1.0, 0.0],
            Axis::Z => [0.0, 0.0, 1.0],
        }
    }
}

/// The body's geometry. `mass`/`pitch` are shared (on [`ScenarioParams`]); this carries only the shape.
/// `Scaffold`/`MeshDemo` have fixed composition (in code); `MeshFile` imports an STL/OBJ/glTF by path.
#[derive(Clone, Debug, PartialEq)]
pub enum BodySpec {
    Sphere { r: f64 },
    Cuboid { half: [f64; 3] },
    Cylinder { r: f64, half_l: f64 },
    Shell { r: f64, thickness: f64 },
    Scaffold,
    MeshDemo,
    MeshFile { path: String, scale: f64 },
}

/// The centre-of-mass path (body-frame `u ∈ [0,1]` reparameterised by [`TimingSpec`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathSpec {
    Static,
    LinearPass {
        a: [f64; 3],
        b: [f64; 3],
    },
    Oscillation {
        axis: [f64; 3],
        amp: f64,
        freq: f64,
        phase: f64,
    },
    Circular {
        radius: f64,
        freq: f64,
    },
}

/// How `u` advances with time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimingSpec {
    Uniform { rate: f64 },
    Eased { rate: f64, accel: f64 },
}

/// The body's orientation over time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OrientSpec {
    Fixed,
    FreeRotation {
        omega0: [f64; 3],
    },
    Libration {
        axis: [f64; 3],
        pivot_distance: f64,
        theta0: f64,
        thetadot0: f64,
    },
}

/// A line of detectors: `count` of them, `spacing` apart, along `axis`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetectorSpec {
    pub count: usize,
    pub spacing: f64,
    pub axis: Axis,
}

/// The measurement schedule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScheduleSpec {
    Uniform {
        cadence: f64,
        n: usize,
    },
    Gappy {
        cadence: f64,
        n: usize,
        p_drop: f64,
        seed: u64,
    },
    Jittered {
        cadence: f64,
        n: usize,
        jitter: f64,
        seed: u64,
    },
}

/// The post-hoc additive noise stack (each entry optional).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct NoiseSpec {
    pub shot: Option<f64>,
    pub vibration: Option<VibrationSpec>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VibrationSpec {
    pub sigma: f64,
    pub rho: f64,
    pub rejection: f64,
}

/// The full editable scenario. `Clone + Debug` (not `PartialEq`/`Copy` — `UldmConfig`/`AtmoConfig` are
/// not `PartialEq`, and the mesh path is a `String`).
#[derive(Clone, Debug)]
pub struct ScenarioParams {
    pub body: BodySpec,
    pub mass: f64,
    pub pitch: f64,
    pub placement: [f64; 3],
    pub path: PathSpec,
    pub timing: TimingSpec,
    pub orient: OrientSpec,
    pub fine_dt: f64,
    pub detectors: DetectorSpec,
    pub schedule: ScheduleSpec,
    pub contamination: Option<(f64, u64)>,
    pub fields: FieldSet,
    pub uldm: Option<UldmConfig>,
    pub atmo: Option<AtmoConfig>,
    pub noise: NoiseSpec,
    pub phase_model: PhaseModelKind,
    pub seed: u64,
}

impl Default for ScenarioParams {
    fn default() -> Self {
        ScenarioParams {
            body: BodySpec::Cuboid {
                half: [0.35, 0.2, 0.12],
            },
            mass: 500.0,
            pitch: 0.08,
            placement: [3.0, 0.0, 3.0],
            path: PathSpec::Static,
            timing: TimingSpec::Uniform { rate: 0.0 },
            orient: OrientSpec::Fixed,
            fine_dt: 0.01,
            detectors: DetectorSpec {
                count: 2,
                spacing: 1.0,
                axis: Axis::Z,
            },
            schedule: ScheduleSpec::Uniform {
                cadence: 2.0,
                n: 64,
            },
            contamination: None,
            fields: FieldSet {
                periodogram: true,
                ..FieldSet::default()
            },
            uldm: None,
            atmo: None,
            noise: NoiseSpec::default(),
            phase_model: PhaseModelKind::PropagationIntegral,
            seed: 7,
        }
    }
}

fn v(a: [f64; 3]) -> Vec3<f64> {
    Vec3::new(a[0], a[1], a[2])
}

/// Realise a fresh `Scenario` from the spec. Fallible: a mesh that will not load, or a body that will not
/// voxelise, returns an `Err(String)` the App shows as a toast — never a crash. Signal amplitude is
/// linear in `mass`, so tweak-and-rerun on `mass` is visible (and testable).
pub fn build_scenario(p: &ScenarioParams) -> Result<Scenario, String> {
    let cloud = build_body(p)?;
    let placement = Isometry3::new(Quat::identity(), v(p.placement));
    let traj = Trajectory::new(placement, build_path(&p.path), build_timing(&p.timing))
        .with_orient(build_orient(&p.orient));
    let source = Source::new(cloud, traj).with_fine_dt(p.fine_dt);

    let mut schedule = build_schedule(&p.schedule);
    if let Some((frac, seed)) = p.contamination {
        schedule = schedule.with_contamination(frac, seed);
    }

    let mut scenario = Scenario::new(
        Box::new(source),
        build_array(&p.detectors),
        schedule,
        p.seed,
    )
    .with_field_set(p.fields)
    .with_phase_model(p.phase_model);
    if let Some(u) = p.uldm {
        scenario = scenario.with_uldm(u);
    }
    if let Some(a) = p.atmo {
        scenario = scenario.with_atmo(a);
    }
    let noise = build_noise(&p.noise);
    if !noise.0.is_empty() {
        scenario = scenario.with_noise(noise);
    }
    Ok(scenario)
}

/// Voxelise the body to a unit-frame `Cloud`. A mesh (or a compound body that is not axis-aligned) that
/// will spin is re-expressed in its principal frame first — `Orient::FreeRotation`'s precondition (M10-R7).
fn build_body(p: &ScenarioParams) -> Result<Cloud, String> {
    let vp = VoxelParams::pitch(p.pitch);
    let mass = MassSpec::Total(p.mass);
    let cloud = match &p.body {
        BodySpec::Sphere { r } => voxelise(&Sphere { r: *r }, &vp, mass),
        BodySpec::Cuboid { half } => voxelise(&Cuboid { half: *half }, &vp, mass),
        BodySpec::Cylinder { r, half_l } => voxelise(
            &Cylinder {
                r: *r,
                half_l: *half_l,
            },
            &vp,
            mass,
        ),
        BodySpec::Shell { r, thickness } => voxelise(
            &Shell {
                r: *r,
                thickness: *thickness,
            },
            &vp,
            mass,
        ),
        BodySpec::Scaffold => voxelise(&scaffold(), &vp, mass),
        BodySpec::MeshDemo => {
            let (mesh, _) =
                MeshSolid::from_soup(tilted_box_soup()).map_err(|e| format!("mesh: {e:?}"))?;
            voxelise_mesh(&mesh, &vp, mass)
        }
        BodySpec::MeshFile { path, scale } => {
            let import = MeshImport {
                path: PathBuf::from(path),
                scale: Some(*scale),
                voxel: vp,
                mass,
            };
            let (mesh, _) = load_solid(&import).map_err(|e| format!("mesh load: {e:?}"))?;
            voxelise_mesh(&mesh, &vp, mass)
        }
    }
    .map_err(|e| format!("voxelise: {e:?}"))?;

    let spins = matches!(p.orient, OrientSpec::FreeRotation { .. });
    let off_axis = matches!(
        p.body,
        BodySpec::MeshDemo | BodySpec::MeshFile { .. } | BodySpec::Scaffold
    );
    Ok(if spins && off_axis {
        principal_frame(&cloud).0
    } else {
        cloud
    })
}

/// A small scaffold: four vertical cylindrical posts and two top cross-beams — a compound `Union` body.
fn scaffold() -> Union {
    let post = |x: f64, y: f64| -> Box<dyn Solid> {
        Box::new(Translated {
            inner: Box::new(Cylinder {
                r: 0.05,
                half_l: 0.6,
            }),
            offset: [x, y, 0.0],
        })
    };
    let beam = |half: [f64; 3]| -> Box<dyn Solid> {
        Box::new(Translated {
            inner: Box::new(Cuboid { half }),
            offset: [0.0, 0.0, 0.55],
        })
    };
    Union(vec![
        post(-0.4, -0.4),
        post(0.4, -0.4),
        post(-0.4, 0.4),
        post(0.4, 0.4),
        beam([0.45, 0.05, 0.05]),
        beam([0.05, 0.45, 0.05]),
    ])
}

/// A watertight asymmetric box mesh (distinct half-extents), tilted out of its principal frame — the M10
/// demo body: an imported mesh that tumbles once re-expressed in its principal frame.
fn tilted_box_soup() -> TriSoup {
    let mut verts = vec![
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ];
    let h = [0.35, 0.2, 0.12];
    let (a, b) = (0.5f64, 0.7f64); // yaw, then pitch
    let (ca, sa, cb, sb) = (a.cos(), a.sin(), b.cos(), b.sin());
    for vt in &mut verts {
        let p = [vt[0] * 2.0 * h[0], vt[1] * 2.0 * h[1], vt[2] * 2.0 * h[2]];
        let z = [ca * p[0] - sa * p[1], sa * p[0] + ca * p[1], p[2]];
        *vt = [z[0], cb * z[1] - sb * z[2], sb * z[1] + cb * z[2]];
    }
    let tris = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [2, 3, 7],
        [2, 7, 6],
        [0, 4, 7],
        [0, 7, 3],
        [1, 2, 6],
        [1, 6, 5],
    ];
    TriSoup { verts, tris }
}

fn build_path(p: &PathSpec) -> Path {
    match *p {
        PathSpec::Static => Path::Static,
        PathSpec::LinearPass { a, b } => Path::LinearPass { a: v(a), b: v(b) },
        PathSpec::Oscillation {
            axis,
            amp,
            freq,
            phase,
        } => Path::Oscillation {
            axis: v(axis),
            amp,
            freq,
            phase,
        },
        PathSpec::Circular { radius, freq } => Path::Circular { radius, freq },
    }
}

fn build_timing(t: &TimingSpec) -> Timing {
    match *t {
        TimingSpec::Uniform { rate } => Timing::Uniform { rate },
        TimingSpec::Eased { rate, accel } => Timing::Eased { rate, accel },
    }
}

fn build_orient(o: &OrientSpec) -> Orient {
    match *o {
        OrientSpec::Fixed => Orient::Fixed(Quat::identity()),
        OrientSpec::FreeRotation { omega0 } => Orient::FreeRotation { omega0: v(omega0) },
        OrientSpec::Libration {
            axis,
            pivot_distance,
            theta0,
            thetadot0,
        } => Orient::Libration {
            axis: v(axis),
            pivot_distance,
            theta0,
            thetadot0,
        },
    }
}

fn build_array(d: &DetectorSpec) -> DetectorArray {
    let unit = d.axis.unit();
    let dets = (0..d.count.max(1))
        .map(|i| {
            let off = i as f64 * d.spacing;
            match d.axis {
                // A vertical (z) line stacks the IFO pairs by base height; other axes offset the placement.
                Axis::Z => Detector::new(off),
                _ => Detector::placed(Isometry3::new(
                    Quat::identity(),
                    Vec3::new(unit[0] * off, unit[1] * off, unit[2] * off),
                )),
            }
        })
        .collect();
    DetectorArray::new(dets)
}

fn build_schedule(s: &ScheduleSpec) -> Schedule {
    match *s {
        ScheduleSpec::Uniform { cadence, n } => Schedule::uniform(cadence, n.max(1)),
        ScheduleSpec::Gappy {
            cadence,
            n,
            p_drop,
            seed,
        } => Schedule::gappy(cadence, n.max(1), p_drop, seed),
        ScheduleSpec::Jittered {
            cadence,
            n,
            jitter,
            seed,
        } => Schedule::jittered(cadence, n.max(1), jitter, seed),
    }
}

fn build_noise(n: &NoiseSpec) -> NoiseStack {
    let mut stack: Vec<Box<dyn NoiseSource>> = Vec::new();
    if let Some(sigma) = n.shot {
        stack.push(Box::new(ShotNoise { sigma }));
    }
    if let Some(vib) = n.vibration {
        stack.push(Box::new(VibrationResidual {
            sigma: vib.sigma,
            rho: vib.rho,
            rejection: vib.rejection,
        }));
    }
    NoiseStack(stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::StateBundle;

    /// The clean (noise-subtracted) peak amplitude of detector 0.
    fn clean_amplitude(b: &StateBundle) -> f64 {
        b.signal
            .iter()
            .zip(&b.signal_noise)
            .map(|(s, n)| (s[0] - n[0]).abs())
            .fold(0.0_f64, f64::max)
    }

    #[test]
    fn tweak_rerun() {
        // Doubling the source mass doubles the clean signal amplitude — the potential is linear in mass,
        // so the clean channel scales exactly. "The view updates" is judged live (Run after moving mass).
        let base = ScenarioParams::default();
        let heavier = ScenarioParams {
            mass: 2.0 * base.mass,
            ..base.clone()
        };
        let a1 = clean_amplitude(&generate::run(&build_scenario(&base).unwrap()));
        let a2 = clean_amplitude(&generate::run(&build_scenario(&heavier).unwrap()));
        assert!(a1 > 0.0, "no signal to scale");
        let ratio = a2 / a1;
        assert!(
            (ratio - 2.0).abs() < 1e-9,
            "amplitude ratio {ratio}, expected 2 (signal ∝ mass)"
        );
    }

    #[test]
    fn mesh_file_body_loads() {
        // A `MeshFile` body parses an on-disk STL and voxelises — the full import path the GUI drives.
        let stl = "solid t
facet normal 0 0 -1
outer loop
vertex 0 0 0
vertex 0 1 0
vertex 1 0 0
endloop
endfacet
facet normal 0 -1 0
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 0 1
endloop
endfacet
facet normal -1 0 0
outer loop
vertex 0 0 0
vertex 0 0 1
vertex 0 1 0
endloop
endfacet
facet normal 1 1 1
outer loop
vertex 1 0 0
vertex 0 1 0
vertex 0 0 1
endloop
endfacet
endsolid t
";
        let path = std::env::temp_dir().join("cavendish_viewer_tetra.stl");
        std::fs::write(&path, stl).expect("write stl");
        let p = ScenarioParams {
            body: BodySpec::MeshFile {
                path: path.to_string_lossy().into_owned(),
                scale: 1.0,
            },
            mass: 1.0,
            pitch: 0.15,
            schedule: ScheduleSpec::Uniform { cadence: 1.0, n: 4 },
            ..ScenarioParams::default()
        };
        let scenario = build_scenario(&p).expect("mesh-file body builds");
        let bundle = generate::run(&scenario);
        std::fs::remove_file(&path).ok();
        assert_eq!(bundle.time.len(), 4, "imported mesh ran");
    }

    #[test]
    #[ignore = "review-grade: run `cargo run -p viewer`, load a rotation preset, and scrub to watch the flip"]
    fn tumble_visible() {
        // An asymmetric top spun about its intermediate axis (y) undergoes the Dzhanibekov flip. Here we
        // only assert the tumble is PRESENT — ω throughout, and the orientation genuinely evolves across ℓ.
        let p = ScenarioParams {
            orient: OrientSpec::FreeRotation {
                omega0: [0.0, 3.0, 0.02],
            },
            schedule: ScheduleSpec::Uniform {
                cadence: 0.05,
                n: 60,
            },
            ..ScenarioParams::default()
        };
        let b = generate::run(&build_scenario(&p).unwrap());
        let angvel = &b.source_angular_velocity[0];
        assert!(
            angvel
                .iter()
                .all(|w| w[0] * w[0] + w[1] * w[1] + w[2] * w[2] > 0.0),
            "ω vanished — nothing spinning"
        );
        let first = b.source_orientation[0][0];
        let evolved = b.source_orientation[0]
            .iter()
            .any(|q| (0..4).map(|k| (q[k] - first[k]).abs()).sum::<f64>() > 0.1);
        assert!(evolved, "orientation did not evolve — no tumble to see");
    }
}
