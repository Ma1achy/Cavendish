//! The dev and debug scenes of REBUILD §9.1: scenes whose answer is known.
//!
//! Each is a file in `scenes/dev/` and a builder here; a test holds the two together, so the file a
//! person reads and the scene a test builds cannot drift apart. S0 has the first two, both empty of
//! moving matter; S1 adds the scenes with a point mass. Their known answers are validation cases
//! in `read.rs`.

use crate::{BodySpec, FloorKind, FloorSpec, Motion, SceneSpec};

/// `void`: the array and nothing else.
#[must_use]
pub fn void() -> SceneSpec {
    SceneSpec {
        name: "void".into(),
        description: "The array and nothing else: no room, no air, no ground. Every channel must read \
                      exactly zero; the renderer must cope with an empty world."
            .into(),
        room: None,
        floor: None,
        ..SceneSpec::room()
    }
}

/// `infinite-floor`: the array over a uniform half-space of concrete.
#[must_use]
pub fn infinite_floor() -> SceneSpec {
    SceneSpec {
        name: "infinite-floor".into(),
        description: "A uniform half-space below the array, drawn as a grid to the horizon. An infinite \
                      uniform slab gives uniform g and no gradient, so its differenced phase is zero."
            .into(),
        room: None,
        floor: Some(FloorSpec { height: 0.0, kind: FloorKind::HalfSpace, density: 2400.0 }),
        ..SceneSpec::room()
    }
}

// ── S1: scenes with a point mass ────────────────────────────────────────────────────────────────

/// An open scene (no room, no floor) with the default array, a name, what it is for, how long it
/// runs and its bodies.
fn open(name: &str, description: &str, duration: f64, bodies: Vec<BodySpec>) -> SceneSpec {
    SceneSpec {
        name: name.into(),
        description: description.into(),
        duration,
        room: None,
        floor: None,
        bodies,
        ..SceneSpec::room()
    }
}

/// `point`: one mass at rest in the room, to drag about.
#[must_use]
pub fn point() -> SceneSpec {
    SceneSpec {
        name: "point".into(),
        description: "One mass at rest in the room: 100 kg, 2 m from the centre tower, the static anchor \
                      that v1 is run on. Select it and drag it; every tower's phase follows."
            .into(),
        bodies: vec![BodySpec::point("mass", 100.0, [2.0, 0.0, 2.5])],
        ..SceneSpec::room()
    }
}

/// `pass`: the published moving anchor, a straight vertical pass.
#[must_use]
pub fn pass() -> SceneSpec {
    let motion = Motion::Pass { from: [10.0, 0.0, -10.0], to: [10.0, 0.0, 15.0], start: 0.0, speed: 2.5 };
    open(
        "pass",
        "A straight pass: 10 kg, 10 m from the centre tower, rising at 2.5 m/s from 10 m below the base. \
         The published moving anchor. Press play.",
        12.0,
        vec![BodySpec::moving("lift", 10.0, motion)],
    )
}

/// `oscillator-zoo`: the oscillator library at the reference scripts' parameters, one of each kind,
/// each 3 m from the centre tower.
#[must_use]
pub fn oscillator_zoo() -> SceneSpec {
    let half = 0.5;
    let bodies = vec![
        // fig2_10m panel b: up and down at 0.05 Hz, below Nyquist: a plain comb.
        BodySpec::moving(
            "linear",
            1.0,
            Motion::Linear {
                centre: [3.0, 0.0, 1.0],
                amplitude: [0.0, 0.0, half],
                frequency: 0.05,
                phase: 0.0,
            },
        ),
        // fig2_10m panel d: a circle in the vertical plane through the tower's axis, at 0.35625 Hz,
        // above Nyquist: every harmonic folds.
        BodySpec::moving(
            "circular",
            1.0,
            Motion::Circular {
                centre: [0.0, 3.0, 1.0],
                u: [0.0, half, 0.0],
                v: [0.0, 0.0, half],
                frequency: 0.356_25,
                phase: 0.0,
            },
        ),
        // fig2_multi_directional.py panel f: the spherical pendulum, L = 14.5 m, 0.5 m by 0.25 m.
        BodySpec::moving(
            "pendulum",
            1.0,
            Motion::Pendulum {
                pivot: [-3.0, 0.0, 15.5],
                length: 14.5,
                major: half,
                minor: 0.25,
                azimuth: 0.0,
            },
        ),
        // Forced motion on a sphere: this slice's own parametrisation (see `motion.rs`).
        BodySpec::moving(
            "sphere",
            1.0,
            Motion::Sphere {
                centre: [0.0, -3.0, 3.0],
                radius: 2.0,
                tilt: 20.0,
                tilt_frequency: 0.02,
                spin_frequency: 0.01,
            },
        ),
    ];
    open(
        "oscillator-zoo",
        "The oscillator library at the reference scripts' parameters: a linear oscillator at 0.05 Hz, a \
         circular one at 0.35625 Hz (above Nyquist, so its comb folds), a spherical pendulum of 14.5 m \
         and forced motion on a sphere, each 1 kg and 3 m from the centre tower. For the comb and the folds.",
        2000.0,
        bodies,
    )
}

/// The reference scripts' oscillator: 1 kg, 3 m from the centre tower, moving 0.5 m either side
/// of 1 m up, at `frequency` hertz.
fn script_oscillator(frequency: f64) -> BodySpec {
    let motion =
        Motion::Linear { centre: [3.0, 0.0, 1.0], amplitude: [0.0, 0.0, 0.5], frequency, phase: 0.0 };
    BodySpec::moving("oscillator", 1.0, motion)
}

/// `comb`: one oscillator below Nyquist, whose harmonics stand where they are.
#[must_use]
pub fn comb() -> SceneSpec {
    open(
        "comb",
        "One oscillator at 0.05 Hz (panel b of the scripts' fig2_10m): its periodogram is a comb at 0.05, 0.10, 0.15 and 0.20 Hz, each harmonic lower than the last, all below the Nyquist frequency of 0.25 Hz.",
        2000.0,
        vec![script_oscillator(0.05)],
    )
}

/// `folded-comb`: the same oscillator above Nyquist, whose harmonics all fold.
#[must_use]
pub fn folded_comb() -> SceneSpec {
    open(
        "folded-comb",
        "One oscillator at 0.3275 Hz (panel c of the scripts' fig2_10m), above the Nyquist frequency of 0.25 Hz: every harmonic folds, to 0.1725, 0.155, 0.0175 and 0.19 Hz.",
        2000.0,
        vec![script_oscillator(0.3275)],
    )
}

/// `superposition`: two bodies, to be read alone and together.
#[must_use]
pub fn superposition() -> SceneSpec {
    let swing =
        Motion::Linear { centre: [5.0, 0.0, 1.0], amplitude: [0.0, 0.0, 1.0], frequency: 0.1, phase: 0.0 };
    open(
        "superposition",
        "A, then B, then A + B: an oscillating mass and a mass at rest. Each tower's reading with both \
         present is the sum of its readings with each alone, exactly, channel by channel.",
        40.0,
        vec![BodySpec::moving("a", 1.0, swing), BodySpec::point("b", 25.0, [-2.0, 3.0, 4.0])],
    )
}

/// `mirror-pair`: two equal masses, mirror images in the vertical plane x = 0 through the array.
#[must_use]
pub fn mirror_pair() -> SceneSpec {
    let side = |x: f64| Motion::Linear {
        centre: [x, 1.0, 1.5],
        amplitude: [0.2 * x.signum(), 0.0, 0.5],
        frequency: 0.07,
        phase: 0.0,
    };
    open(
        "mirror-pair",
        "Two equal masses, mirror images in the vertical plane x = 0. The towers on the plane (1, 4 and 5) \
         read the same from each; the mirrored towers (2 and 3) swap their readings of the two.",
        40.0,
        vec![BodySpec::moving("east", 5.0, side(2.5)), BodySpec::moving("west", 5.0, side(-2.5))],
    )
}

/// `switch-on`: a mass that appears in the middle of a flight and stays.
#[must_use]
pub fn switch_on() -> SceneSpec {
    let mut body = BodySpec::point("mass", 50.0, [3.0, 0.0, 2.0]);
    body.appears = Some(5.0);
    open(
        "switch-on",
        "A mass that appears at 5 s, inside the flight launched at 4 s, and stays: the step response. \
         The shot in flight reads the part of its integral after 5 s; earlier shots read nothing.",
        12.0,
        vec![body],
    )
}

/// `flash`: a mass that exists for one millisecond inside a flight.
#[must_use]
pub fn flash() -> SceneSpec {
    let mut body = BodySpec::point("mass", 1000.0, [3.0, 0.0, 2.0]);
    (body.appears, body.vanishes) = (Some(4.5), Some(4.501));
    open(
        "flash",
        "A mass that exists for one millisecond, at 4.5 s, inside the flight launched at 4 s: the impulse \
         response of the flight window. That one shot reads the integrand there times a millisecond.",
        12.0,
        vec![body],
    )
}

/// `time-reversal`: a pass and the same pass backwards, with the launch that makes the sequence
/// symmetric in time.
#[must_use]
pub fn time_reversal() -> SceneSpec {
    // Six shots, launched at 0, 2, … 10 s and read out 1.46 s later: the middle of them all is 5.73 s.
    // A pass of 12 m at 2 m/s centred on that moment starts at 2.73 s.
    let (low, high) = ([6.0, 0.0, -4.0], [6.0, 0.0, 8.0]);
    let there = Motion::Pass { from: low, to: high, start: 2.73, speed: 2.0 };
    let back = Motion::Pass { from: high, to: low, start: 2.73, speed: 2.0 };
    let mut scene = open(
        "time-reversal",
        "A pass and the same pass played backwards. With the launch that closes the arms at the launch \
         height at 2T (u₀ = gT − v_kick/2) each arm is its own mirror image about T, and the backward \
         pass reads the forward pass's series in reverse.",
        12.0,
        vec![BodySpec::moving("forward", 20.0, there), BodySpec::moving("backward", 20.0, back)],
    );
    scene.array.instrument.launch_velocity = scene.array.instrument.instrument().symmetric_launch();
    scene
}

/// `ring-orbit`: a mass circling the centre tower at fixed height and radius.
#[must_use]
pub fn ring_orbit() -> SceneSpec {
    let orbit = Motion::Circular {
        centre: [0.0, 0.0, 2.0],
        u: [3.0, 0.0, 0.0],
        v: [0.0, 3.0, 0.0],
        frequency: 0.05,
        phase: 0.0,
    };
    open(
        "ring-orbit",
        "A mass circling the centre tower, 3 m out and 2 m up. The centre tower's reading is constant; \
         any ripple in it is quadrature or rounding. The outer towers see it come and go.",
        60.0,
        vec![BodySpec::moving("satellite", 10.0, orbit)],
    )
}

/// `through-the-tower`: a sphere that passes through the atoms' path.
#[must_use]
pub fn through_the_tower() -> SceneSpec {
    let mut body = BodySpec::moving(
        "ball",
        70.0,
        Motion::Pass { from: [-6.0, 0.0, 2.0], to: [6.0, 0.0, 2.0], start: 0.0, speed: 1.0 },
    );
    body.radius = 0.3;
    open(
        "through-the-tower",
        "A 70 kg sphere of radius 0.3 m crossing the array along y = 0 at 2 m up, straight through the \
         atoms' paths of towers 3, 1 and 2. A sphere's potential is finite inside it, so every reading \
         is finite; the engine warns that a body is inside a flight.",
        14.0,
        vec![body],
    )
}

/// How far `precision-far` is from the origin: `[x, y, z]` (east, north, up), metres, in the
/// world's frame. Not whole numbers: those are held exactly and would test nothing.
pub const FAR: [f64; 3] = [10_000.3, -7_000.7, 300.1];

/// `precision-far`: the oscillation anchor and the array, moved by `shift` (`[x, y, z]`, metres,
/// the world's frame; [`FAR`] puts it 10 km from the origin); with `shift` of zero, the same
/// scene at the origin to compare it with.
#[must_use]
pub fn precision_far_at(shift: [f64; 3]) -> SceneSpec {
    let moved = |p: [f64; 3]| [p[0] + shift[0], p[1] + shift[1], p[2] + shift[2]];
    let swing = Motion::Linear {
        centre: moved([5.3, 0.4, 1.0]),
        amplitude: [0.0, 0.0, 1.0],
        frequency: 0.1,
        phase: 0.0,
    };
    let mut scene = open(
        "precision-far",
        "The oscillation anchor and the array 10 km from the origin. Positions are turned into offsets \
         from each tower before anything is computed, so every tower reads what it reads at the origin.",
        40.0,
        vec![BodySpec::moving("oscillator", 1.0, swing)],
    );
    for tower in &mut scene.array.towers {
        tower.position = moved(tower.position);
    }
    scene
}

/// `precision-far` as the library has it: 10 km out.
#[must_use]
pub fn precision_far() -> SceneSpec {
    precision_far_at(FAR)
}

/// `staggered`: the comb's oscillator read by five towers on their own clocks, each a fifth of a
/// flight (2T/5 = 0.292 s) after the one before. The towers sample the same motion at different
/// instants: each has its own readout times, and each one's periodogram has the comb.
#[must_use]
pub fn staggered() -> SceneSpec {
    let mut scene = comb();
    scene.name = "staggered".into();
    scene.description = "The comb's oscillator read by five towers on their own clocks: tower n fires (n − 1)·2T/5 \
                         after tower 1. Each tower has its own readout times; drag a lane with Shift to move one."
        .into();
    let step = 2.0 * scene.array.instrument.half_sequence / 5.0;
    for (n, tower) in scene.array.towers.iter_mut().enumerate() {
        tower.timing.offset = n as f64 * step;
    }
    scene
}

/// `cadence-lab`: the comb's oscillator sampled on a schedule that is not even: each cycle up to
/// 0.3 s late (a random walk, as the reference scripts draw it) and two shots in a hundred lost.
/// The tone is known (0.05 Hz and its harmonics); what the cadence does to it is the subject.
#[must_use]
pub fn cadence_lab() -> SceneSpec {
    let mut scene = comb();
    scene.name = "cadence-lab".into();
    scene.description = "A known tone on an uneven cadence: the comb's oscillator, each cycle up to 0.3 s late and 2% \
                         of shots lost. `show cadence` draws the spectral window; edit array.schedule and watch it."
        .into();
    scene.array.schedule = crate::ScheduleSpec {
        jitter: Some(crate::JitterSpec {
            model: crate::JitterModel::Walk,
            kind: crate::JitterKind::Positive,
            delta: 0.3,
        }),
        loss: 0.02,
        seed: 1,
        ..crate::ScheduleSpec::default()
    };
    scene
}

/// `mixed-array`: towers that do not share an instrument. Tower 1 is AION-10 (the array's own),
/// tower 2 is the AICE preset (70 m apart in a 100 m tower, T = 2.5 s, a shot every 5 s, its
/// launch derived), tower 3 is AION-10 with T = 0.9 s. Each has its own shots and its own flight;
/// an oscillating mass between them is read by all three.
#[must_use]
pub fn mixed_array() -> SceneSpec {
    let swing =
        Motion::Linear { centre: [3.0, 2.0, 4.0], amplitude: [0.0, 0.0, 1.0], frequency: 0.05, phase: 0.0 };
    let mut scene = open(
        "mixed-array",
        "Three towers that do not share an instrument: AION-10, AICE (a shot every 5 s, 70 m apart) and AION-10 \
         with T = 0.9 s. Each tower has its own shots and its own flight; an oscillating mass is read by all three.",
        60.0,
        vec![BodySpec::moving("oscillator", 10.0, swing)],
    );
    let aice = crate::presets::aice();
    let base = scene.array.instrument.clone();
    let slower = crate::InstrumentSpec { half_sequence: 0.9, ..base.clone() };
    scene.array.towers = vec![
        crate::TowerSpec::at("aion-10", [0.0, 0.0, 0.0]),
        crate::TowerSpec {
            instrument: Some(crate::InstrumentOverride::between(&base, &aice)),
            ..crate::TowerSpec::at("aice", [6.0, 0.0, 0.0])
        },
        crate::TowerSpec {
            instrument: Some(crate::InstrumentOverride::between(&base, &slower)),
            ..crate::TowerSpec::at("aion-10, T = 0.9 s", [0.0, 6.0, 0.0])
        },
    ];
    scene
}

/// `no-towers`: a body in a room and no array at all. Nothing is read; everything else works.
#[must_use]
pub fn no_towers() -> SceneSpec {
    let mut scene = open(
        "no-towers",
        "A room with a mass passing through it and no towers: a scene need not have an array. Nothing is \
         read; the timeline, the panes and the bundle still work, empty.",
        20.0,
        vec![BodySpec::moving(
            "mass",
            5.0,
            Motion::Pass { from: [-4.0, 0.0, 1.0], to: [4.0, 0.0, 1.0], start: 0.0, speed: 1.0 },
        )],
    );
    scene.array.towers.clear();
    scene.room = SceneSpec::room().room;
    scene
}

/// The S3 scenes' towers: tower 1 alone at the origin when `alone`, else the default five.
fn s3_base(name: &str, description: &str, alone: bool) -> SceneSpec {
    let mut scene = void();
    scene.name = name.into();
    scene.description = description.into();
    scene.duration = 12.0;
    if alone {
        scene.array.towers.truncate(1);
    }
    scene
}

/// `unit-cube`: a cube of one metre and a tonne, 2 m east of tower 1, voxelised on a grid that
/// fits its faces: its tree is a handful of merged cubes, and it reads exactly as the prism.
#[must_use]
pub fn unit_cube() -> SceneSpec {
    let mut scene = s3_base(
        "unit-cube",
        "A cube of one metre and a tonne beside tower 1, made of voxels on a grid that fits its faces: a few \
         merged leaves that read as the closed-form prism.",
        false,
    );
    let cube = crate::ShapeSpec::new(crate::ShapeKind::Cuboid { size: [1.0; 3] }, 0.125, 4);
    scene.bodies.push(BodySpec::shaped("cube", 1000.0, cube, Motion::Static { position: [2.0, 0.0, 2.5] }));
    scene
}

/// `sphere`: a uniform sphere of half a metre made of voxels (r/16), and the analytic sphere of the
/// same mass mirrored across tower 1: outside itself each is a point, so the two read alike to the
/// voxelisation's error.
#[must_use]
pub fn sphere() -> SceneSpec {
    let mut scene = s3_base(
        "sphere",
        "Two spheres of 500 kg either side of tower 1: one made of voxels (r/16), one the analytic sphere. Outside \
         themselves both are a point mass; tower 1 reads them as mirror images.",
        false,
    );
    let ball = crate::ShapeSpec::new(crate::ShapeKind::Sphere { radius: 0.5 }, 0.5 / 16.0, 4);
    scene.bodies.push(BodySpec::shaped(
        "voxel sphere",
        500.0,
        ball,
        Motion::Static { position: [2.0, 0.0, 2.5] },
    ));
    let mut analytic = BodySpec::point("analytic sphere", 500.0, [-2.0, 0.0, 2.5]);
    analytic.radius = 0.5;
    scene.bodies.push(analytic);
    scene
}

/// `hollow-sphere`: a spherical shell of 500 kg beside tower 1, 0.5 m outside and 0.35 m inside,
/// made of voxels at r/16: a body whose section is a ring, for looking inside a body (`section`).
/// Outside itself it is a point mass, as the solid sphere is.
#[must_use]
pub fn hollow_sphere() -> SceneSpec {
    let mut scene = s3_base(
        "hollow-sphere",
        "A hollow sphere of 500 kg beside tower 1, 0.5 m outside and 0.35 m inside, made of voxels (r/16). \
         Its sections are rings: step a section plane through it to see the cavity.",
        false,
    );
    let shell = crate::ShapeSpec::new(crate::ShapeKind::Shell { outer: 0.5, inner: 0.35 }, 0.5 / 16.0, 4);
    scene.bodies.push(BodySpec::shaped(
        "hollow sphere",
        500.0,
        shell,
        Motion::Static { position: [2.0, 0.0, 2.5] },
    ));
    scene
}

/// `platonic-row`: the five Platonic solids, 100 kg each and 0.6 m an edge, on a circle of 2.2 m
/// about tower 1 at 2.5 m up: their missing low multipoles (`reference/milestones/M18`).
#[must_use]
pub fn platonic_row() -> SceneSpec {
    let mut scene = s3_base(
        "platonic-row",
        "The five Platonic solids, 100 kg and 0.6 m an edge, on a circle of 2.2 m about tower 1: each is a point mass \
         to its fourth moment, the cube and the octahedron to their sixth.",
        false,
    );
    let solids = [
        crate::PlatonicSolid::Tetrahedron,
        crate::PlatonicSolid::Cube,
        crate::PlatonicSolid::Octahedron,
        crate::PlatonicSolid::Dodecahedron,
        crate::PlatonicSolid::Icosahedron,
    ];
    for (k, solid) in solids.into_iter().enumerate() {
        let angle = (18.0 + 72.0 * k as f64).to_radians();
        let shape = crate::ShapeSpec::new(crate::ShapeKind::Platonic { solid, edge: 0.6 }, 0.05, 4);
        let at = [2.2 * angle.cos(), 2.2 * angle.sin(), 2.5];
        scene.bodies.push(BodySpec::shaped(
            &format!("{solid:?}").to_lowercase(),
            100.0,
            shape,
            Motion::Static { position: at },
        ));
    }
    scene
}

/// George's steel scaffold (`scaffold` and `scaffold_multimode`): four columns of 10 m and
/// eleven square rings of 1 m sides, tubes of 33.8 mm radius, 7850 kg/m³, centred on the tower's
/// axis; each member moves along x as `A_i·α(t)`, with `A_i` rising linearly in height from
/// `bottom` (at z = 0) to `top` (at z = 10 m), taken at the member's centre. Members are voxelised
/// at `pitch`.
#[must_use]
pub fn scaffold_members(bottom: f64, top: f64, modes: &[crate::ModeSpec], pitch: f64) -> Vec<BodySpec> {
    const RADIUS: f64 = 0.0338;
    const DENSITY: f64 = 7850.0;
    let rod = |length: f64, axis: crate::Axis| {
        crate::ShapeSpec::new(crate::ShapeKind::Cylinder { radius: RADIUS, length, axis }, pitch, 2)
    };
    let mass = |length: f64| DENSITY * std::f64::consts::PI * RADIUS * RADIUS * length;
    let mut members = Vec::new();
    let mut add = |label: String, length: f64, axis, at: [f64; 3]| {
        let amplitude = bottom + (top - bottom) * at[2] / 10.0;
        let motion = Motion::Modes { centre: at, amplitude: [amplitude, 0.0, 0.0], modes: modes.to_vec() };
        members.push(BodySpec::shaped(&label, mass(length), rod(length, axis), motion));
    };
    for (n, (sx, sy)) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].into_iter().enumerate() {
        add(format!("column {}", n + 1), 10.0, crate::Axis::Z, [0.5 * sx, 0.5 * sy, 5.0]);
    }
    for k in 0..=10 {
        let z = f64::from(k);
        for sy in [-1.0, 1.0] {
            add(
                format!("ring {k} x{}", if sy < 0.0 { "s" } else { "n" }),
                1.0,
                crate::Axis::X,
                [0.0, 0.5 * sy, z],
            );
        }
        for sx in [-1.0, 1.0] {
            add(
                format!("ring {k} y{}", if sx < 0.0 { "w" } else { "e" }),
                1.0,
                crate::Axis::Y,
                [0.5 * sx, 0.0, z],
            );
        }
    }
    members
}

/// `scaffold`: George's scaffold around tower 1, vibrating in its one mode at 22.4 Hz, 1 µm at the
/// foot to 1 mm at the top (his `scaffold` script). Coarse (2 cm voxels) and short (five shots):
/// a scene to look at; the validation reads it finer (`oracle/tests/bodies.rs`).
#[must_use]
pub fn scaffold() -> SceneSpec {
    let mut scene = s3_base(
        "scaffold",
        "George's steel scaffold around tower 1: 48 tubes, each moving along x by its own amplitude (1 µm at the foot to \
         1 mm at the top) times one mode at 22.4 Hz, folded far below Nyquist. Made of 2 cm voxels; five shots.",
        true,
    );
    scene.duration = 10.0;
    let mode = [crate::ModeSpec { frequency: 22.4, scale: 1.0, phase: 0.0 }];
    scene.bodies = scaffold_members(1e-6, 1e-3, &mode, 0.02);
    scene
}

/// Every dev scene S3 adds, by builder.
pub const S3_SCENES: [fn() -> SceneSpec; 7] =
    [unit_cube, sphere, platonic_row, scaffold, mixed_array, no_towers, hollow_sphere];

/// Every dev scene S2 adds, by builder.
pub const S2_SCENES: [fn() -> SceneSpec; 2] = [staggered, cadence_lab];

/// Every dev scene S1 adds, by builder.
pub const S1_SCENES: [fn() -> SceneSpec; 13] = [
    point,
    pass,
    comb,
    folded_comb,
    oscillator_zoo,
    superposition,
    mirror_pair,
    switch_on,
    flash,
    time_reversal,
    ring_orbit,
    through_the_tower,
    precision_far,
];

// ── S4: big scenes ─────────────────────────────────────────────────────────────────────────────

/// A material of the S4 scenes.
fn material(label: &str, density: f64, colour: &str) -> crate::world::MaterialSpec {
    crate::world::MaterialSpec::plain(label, density, colour)
}

/// A box piece of the S4 scenes, from its corners.
fn block(label: &str, material: usize, lo: [f64; 3], hi: [f64; 3]) -> crate::world::PieceSpec {
    crate::world::PieceSpec {
        label: label.into(),
        material,
        kind: crate::world::PieceKind::Box {
            centre: [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])],
            size: [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]],
        },
    }
}

/// `world-box`: a concrete box in the world volume beside the array, on the world's cells and
/// over four of its chunks. It reads as the same box made a voxel body, and as the prism's
/// closed form (`world.box_reads_as_a_voxel_body`, `world.box_reads_as_the_prism`).
#[must_use]
pub fn world_box() -> SceneSpec {
    SceneSpec {
        name: "world-box".into(),
        description:
            "A box of concrete, 4 × 4 × 2 m, in the world volume beside the array: chunks of 8 m at a \
                      25 cm pitch. It reads as the same box made a voxel body and as the prism's closed form."
                .into(),
        room: None,
        floor: None,
        duration: 10.0,
        world: Some(crate::world::WorldSpec {
            pitch: 0.25,
            supersample: 2,
            origin: [0.0; 3],
            materials: vec![material("concrete", 2400.0, "#b2b0a8")],
            pieces: vec![block("box", 0, [6.0, 5.0, 0.25], [10.0, 9.0, 2.25])],
            stream: crate::world::StreamSpec::default(),
            read: crate::world::WorldReadSpec::default(),
            lod: crate::world::LodSpec::default(),
        }),
        ..SceneSpec::room()
    }
}

/// The buildings of `stress-1e10`: forty blocks on a jittered grid, none within 30 m of the
/// array, of heights from 4 to 22 m, alternately concrete and brick; deterministic.
fn town_buildings() -> Vec<crate::world::PieceSpec> {
    let mut out = Vec::new();
    let mut n = 0u32;
    for row in 0..8 {
        for column in 0..5 {
            // A cheap, fixed scatter: no random numbers, so the scene is the same everywhere.
            let jitter = |k: u32| f64::from((k * 7919 + 104_729) % 97) / 97.0;
            let x = -420.0 + 210.0 * f64::from(column) + 40.0 * jitter(n);
            let y = -400.0 + 110.0 * f64::from(row) + 30.0 * jitter(n + 13);
            if x.abs() < 40.0 && y.abs() < 40.0 {
                continue;
            }
            let (w, d) = (10.0 + 14.0 * jitter(n + 5), 8.0 + 10.0 * jitter(n + 9));
            let h = 4.0 + 18.0 * jitter(n + 21);
            out.push(block(
                &format!("building {}", n + 1),
                1 + (n as usize % 2) * 2,
                [x, y, -1.0],
                [x + w, y + d, h],
            ));
            n += 1;
        }
    }
    out
}

/// `stress-1e10`: a kilometre square of rolling ground at 5 cm, a town of forty buildings and a
/// road through it, around the array: about 10¹⁰ finest cells (REBUILD §9.1). For the big-scene
/// budgets of §5, measured (`docs/slices/S4.md` §7). Its 4×10⁵ chunks are past the whole-world
/// octree's cap (`docs/slices/S5.md` §7.2), so the instrument does not read it, and says so; its
/// `read` is a 2 m bubble at 1e-3 rad.
#[must_use]
pub fn stress_1e10() -> SceneSpec {
    let mut pieces = vec![block("road", 2, [-500.0, 52.0, -0.9], [500.0, 59.0, -0.05])];
    pieces.extend(town_buildings());
    pieces.push(crate::world::PieceSpec {
        label: "ground".into(),
        material: 0,
        kind: crate::world::PieceKind::Terrain {
            lo: [-500.0, -500.0],
            hi: [500.0, 500.0],
            bottom: -1.55,
            level: -0.3,
            relief: 0.25,
            wavelength: 160.0,
            seed: 1,
        },
    });
    SceneSpec {
        name: "stress-1e10".into(),
        description:
            "A square kilometre of rolling ground at a 5 cm pitch, forty buildings and a road, around \
                      the array: about 10^10 finest cells. Fly over it. The instrument does not read it: its \
                      chunks are past the whole-world read's cap, and its readings say so."
                .into(),
        room: None,
        floor: None,
        duration: 10.0,
        world: Some(crate::world::WorldSpec {
            pitch: 0.05,
            supersample: 1,
            origin: [0.0; 3],
            materials: vec![
                material("ground", 1800.0, "#708054"),
                material("concrete", 2400.0, "#b2b0a8"),
                material("asphalt", 2300.0, "#46484c"),
                material("brick", 1900.0, "#9c5440"),
            ],
            pieces,
            stream: crate::world::StreamSpec::default(),
            read: crate::world::WorldReadSpec { radius: 2.0, tolerance: 1e-3 },
            lod: crate::world::LodSpec::default(),
        }),
        ..SceneSpec::room()
    }
}

/// `town`: `stress-1e10` scaled down for pictures on a modest GPU: 200 m square at 10 cm, eight
/// buildings and the road, about 5×10⁷ finest cells. The screenshots are of this; the full scene
/// is measured only in short, bounded runs (`cav bench flythrough`).
#[must_use]
pub fn town() -> SceneSpec {
    let mut scene = stress_1e10();
    let world = scene.world.as_mut().expect("stress-1e10 has a world");
    world.pitch = 0.1;
    // 1e-2 rad, not stress-1e10's 1e-3: the walk opens chunks of 3.2 m from about 90 m in at
    // 1e-2 and nearly all of them at 1e-3, whose audited read is then past the kernel's share
    // and does not settle (`docs/slices/S5.md` §7.3).
    world.read.tolerance = 1e-2;
    world.pieces.retain(|p| match &p.kind {
        crate::world::PieceKind::Box { .. } => p.label == "road",
        crate::world::PieceKind::Terrain { .. } => true,
    });
    for piece in &mut world.pieces {
        match &mut piece.kind {
            crate::world::PieceKind::Terrain { lo, hi, wavelength, .. } => {
                *lo = [-100.0, -100.0];
                *hi = [100.0, 100.0];
                *wavelength = 60.0;
            }
            crate::world::PieceKind::Box { centre, size } if piece.label == "road" => {
                size[0] = 200.0;
                centre[0] = 0.0;
            }
            crate::world::PieceKind::Box { .. } => {}
        }
    }
    // Eight buildings round the array, none within 25 m of it, alternately concrete and brick.
    let spots = [
        (-70.0, -60.0),
        (-30.0, -75.0),
        (35.0, -55.0),
        (70.0, -20.0),
        (60.0, 30.0),
        (20.0, 70.0),
        (-45.0, 75.0),
        (-80.0, 20.0),
    ];
    for (n, (x, y)) in spots.into_iter().enumerate() {
        let (w, d, h) = (10.0 + 2.0 * n as f64, 9.0 + (n % 3) as f64 * 3.0, 5.0 + 2.5 * n as f64);
        let piece = block(&format!("building {}", n + 1), 1 + (n % 2) * 2, [x, y, -1.0], [x + w, y + d, h]);
        world.pieces.insert(0, piece);
    }
    let n = world.pieces.len();
    scene.name = "town".into();
    scene.description = format!(
        "stress-1e10 scaled down: 200 m of rolling ground at 10 cm, {} buildings and the road, around the array: about 5x10^7 \
         finest cells. For pictures on a modest GPU. The instrument reads the whole world: what is within 2 m of each tower \
         leaf by leaf, the rest through the octree within 1e-2 rad.",
        n - 2
    );
    scene
}

/// The scenes S4 added, in the order the library lists them.
pub const S4_SCENES: [fn() -> SceneSpec; 3] = [world_box, stress_1e10, town];

/// `exact-solids`: `platonic-row`'s five solids, each read `Exact` (TASK-S5-06): the polyhedral
/// closed form near, the exact moments far, no voxels in the read (they are still drawn). Its
/// readings are `platonic-row`'s to the voxels' error (`docs/slices/S5.md` §7.5).
#[must_use]
pub fn exact_solids() -> SceneSpec {
    let mut scene = platonic_row();
    scene.name = "exact-solids".into();
    scene.description =
        "platonic-row's five solids, 100 kg and 0.6 m an edge, read exactly (Exact): the polyhedral \
                         closed form near and their exact moments far, with no voxels in the read."
            .into();
    for body in &mut scene.bodies {
        if let Some(shape) = body.shape.as_mut() {
            shape.representation = crate::Representation::Exact;
        }
    }
    scene
}

/// The scenes S5 added, in the order the library lists them.
pub const S5_SCENES: [fn() -> SceneSpec; 1] = [exact_solids];

/// The text of a dev scene's file: a comment that says where it comes from, then the builder's RON.
#[must_use]
pub fn file_text(scene: &SceneSpec) -> String {
    format!(
        "// Dev scene `{}` (REBUILD §9.1), written from its builder in crates/cav-sim/src/dev.rs; a test\n\
         // holds the two together. Frame: metres, x east, y north, z up; seconds; angles in degrees.\n\
         // `cav schema` describes every field.\n{}\n",
        scene.name,
        // ron ends its lines as the platform does; a scene file has one form everywhere.
        scene.to_ron().replace("\r\n", "\n")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;

    fn scene_path(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenes/dev").join(format!("{name}.ron"))
    }

    #[test]
    fn the_s1_scene_files_are_their_builders() {
        // With CAV_WRITE_SCENES set, a scene with no file yet gets one. A file that exists is never
        // rewritten here: a builder that has drifted from its file fails below, and is looked at.
        // CAV_REWRITE_SCENES is for the day the file format itself gains a field (S2: each tower's
        // timing): every file is written again from its builder, and the diff is read before it
        // is committed.
        let write = std::env::var_os("CAV_WRITE_SCENES").is_some();
        let rewrite = std::env::var_os("CAV_REWRITE_SCENES").is_some();
        for build in S1_SCENES.into_iter().chain(S2_SCENES).chain(S3_SCENES).chain(S4_SCENES).chain(S5_SCENES)
        {
            let scene = build();
            assert_eq!(scene.validate(), Ok(()), "{}", scene.name);
            let path = scene_path(&scene.name);
            if rewrite || (write && !path.exists()) {
                std::fs::write(&path, file_text(&scene)).unwrap();
            }
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(
                text.replace("\r\n", "\n"),
                file_text(&scene),
                "{} is not what its builder writes",
                scene.name
            );
            assert_eq!(SceneSpec::from_ron(&text).unwrap(), scene, "{}", scene.name);
        }
        // Control: the comparison sees one changed digit.
        let mut nudged = point();
        nudged.bodies[0].mass = 100.5;
        assert_ne!(file_text(&nudged), file_text(&point()));
    }

    fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    /// Where body `n` of a scene is at a time.
    fn at(scene: &SceneSpec, n: usize, t: f64) -> [f64; 3] {
        scene.bodies[n].motion.position(t, scene.array.instrument.gravity)
    }

    #[test]
    fn the_s1_dev_scenes_are_what_they_say() {
        // point: one mass at rest, inside the room.
        let scene = point();
        assert_eq!(scene.bodies.len(), 1);
        assert_eq!(
            (scene.bodies[0].mass, at(&scene, 0, 0.0), at(&scene, 0, 9.0)),
            (100.0, [2.0, 0.0, 2.5], [2.0, 0.0, 2.5])
        );
        let room = scene.room.as_ref().expect("the point is in the room");
        assert!(
            (0..3).all(|k| (room.origin[k]..=room.origin[k] + room.size[k]).contains(&at(&scene, 0, 0.0)[k]))
        );

        // pass: the published moving anchor.
        let scene = pass();
        assert_eq!(scene.bodies[0].mass, 10.0);
        for t in [0.0, 4.0, 10.0] {
            let p = at(&scene, 0, t);
            assert!(distance(p, [10.0, 0.0, -10.0 + 2.5 * t]) < 1e-12, "{t}: {p:?}");
        }

        // oscillator-zoo: one of each kind, each 3 m from the centre tower's axis.
        let scene = oscillator_zoo();
        let kinds: Vec<&str> = scene.bodies.iter().map(|b| b.motion.kind()).collect();
        assert_eq!(kinds, ["linear", "circular", "pendulum", "sphere"]);
        for body in &scene.bodies {
            let (anchor, _) = body.motion.anchor();
            assert!((anchor[0].hypot(anchor[1]) - 3.0).abs() < 1e-12, "{}", body.label);
            assert_eq!(body.mass, 1.0);
        }
        let nyquist = 0.5 / scene.array.instrument.cycle;
        let frequency = |n: usize| scene.bodies[n].motion.frequency(scene.array.instrument.gravity).unwrap();
        assert!(frequency(0) < nyquist && 4.0 * frequency(0) < nyquist, "the linear comb is below Nyquist");
        assert!(frequency(1) > nyquist, "the circular one folds");
        assert!(scene.duration / scene.array.instrument.cycle >= 1000.0, "enough shots for a periodogram");

        // superposition: two bodies, one moving.
        let scene = superposition();
        assert_eq!(scene.bodies.len(), 2);
        assert_ne!(at(&scene, 0, 0.0), at(&scene, 0, 2.5));
        assert_eq!(at(&scene, 1, 0.0), at(&scene, 1, 2.5));

        // mirror-pair: mirror images in the plane x = 0, at every time.
        let scene = mirror_pair();
        assert_eq!(scene.bodies[0].mass, scene.bodies[1].mass);
        for t in [0.0, 1.7, 13.0, 39.9] {
            let (east, west) = (at(&scene, 0, t), at(&scene, 1, t));
            assert_eq!([-east[0], east[1], east[2]], west, "t = {t}");
        }
        assert!(at(&scene, 0, 0.0)[0] > 0.0);

        // switch-on and flash: the times are inside the flight launched at 4 s.
        let flight = |scene: &SceneSpec| (4.0, 4.0 + 2.0 * scene.array.instrument.half_sequence);
        let scene = switch_on();
        let (appears, vanishes) = scene.bodies[0].exists();
        assert!(appears > flight(&scene).0 && appears < flight(&scene).1 && vanishes == f64::INFINITY);
        let scene = flash();
        let (appears, vanishes) = scene.bodies[0].exists();
        assert!(appears > flight(&scene).0 && vanishes < flight(&scene).1);
        assert!((vanishes - appears - 1e-3).abs() < 1e-12, "one millisecond");

        // time-reversal: the backward pass is the forward one mirrored about the middle of the
        // shots, and the launch is the symmetric one.
        let scene = time_reversal();
        let instrument = scene.array.instrument.instrument();
        assert_eq!(scene.array.instrument.launch_velocity, instrument.symmetric_launch());
        let shots = ((scene.duration - instrument.flight_time()) / scene.array.instrument.cycle).floor();
        let middle = 0.5 * (shots * scene.array.instrument.cycle + instrument.flight_time());
        assert!((middle - 5.73).abs() < 1e-12, "{middle}");
        for t in [0.0, 2.73, 4.0, 5.73, 7.1, 8.73, 12.0] {
            assert!(distance(at(&scene, 1, t), at(&scene, 0, 2.0 * middle - t)) < 1e-12, "t = {t}");
        }
        // Control: the default launch is not the symmetric one.
        assert!(
            (SceneSpec::room().array.instrument.launch_velocity - instrument.symmetric_launch()).abs() > 0.01
        );

        // ring-orbit: always 3 m from the centre tower's axis and 2 m up.
        let scene = ring_orbit();
        for step in 0..50 {
            let p = at(&scene, 0, 1.3 * f64::from(step));
            assert!((p[0].hypot(p[1]) - 3.0).abs() < 1e-12 && p[2] == 2.0);
        }

        // through-the-tower: a sphere whose centre crosses three towers' axes.
        let scene = through_the_tower();
        assert!(scene.bodies[0].radius > 0.0);
        for (tower, t) in [(2, 2.0), (0, 6.0), (1, 10.0)] {
            let [x, y, _] = scene.array.towers[tower].position;
            let p = at(&scene, 0, t);
            assert!((p[0] - x).hypot(p[1] - y) < 1e-12, "tower {}: {p:?}", tower + 1);
        }

        // precision-far: 10 km out, and otherwise the scene at the origin.
        let (far, near) = (precision_far(), precision_far_at([0.0; 3]));
        assert!(far.array.towers[0].position[0] > 9_999.0);
        for (a, b) in far.array.towers.iter().zip(&near.array.towers) {
            assert!(
                distance(
                    a.position,
                    [b.position[0] + FAR[0], b.position[1] + FAR[1], b.position[2] + FAR[2]]
                ) < 1e-9
            );
        }
        let offset = |scene: &SceneSpec, t: f64| {
            let (p, tower) = (at(scene, 0, t), scene.array.towers[1].position);
            [p[0] - tower[0], p[1] - tower[1], p[2] - tower[2]]
        };
        assert!(distance(offset(&far, 3.3), offset(&near, 3.3)) < 1e-9);
        assert_eq!(S1_SCENES.len(), 13);

        // comb and folded-comb: the scripts' oscillator, below and above Nyquist.
        let nyquist = 0.25;
        let (below, above) = (comb(), folded_comb());
        assert_eq!(below.bodies[0].motion.frequency(9.81), Some(0.05));
        assert_eq!(above.bodies[0].motion.frequency(9.81), Some(0.3275));
        assert!(4.0 * 0.05 < nyquist && 0.3275 > nyquist);
        assert_eq!(at(&below, 0, 0.0), [3.0, 0.0, 1.5]);
        assert_eq!(below.bodies[0].motion.anchor().0, above.bodies[0].motion.anchor().0);
    }

    #[test]
    fn void_is_the_array_and_nothing_else() {
        let void = load("void").unwrap();
        assert_eq!(void, super::void(), "scenes/dev/void.ron is the builder's scene");
        assert_eq!(void.array, SceneSpec::room().array, "the same array as the default scene");
        assert_eq!(void.array.towers.len(), 5);
        assert!(void.room.is_none() && void.floor.is_none());
        // Control: the default scene is not a void, so the comparison can tell them apart.
        assert_ne!(
            SceneSpec { name: "void".into(), description: void.description.clone(), ..SceneSpec::room() },
            void
        );
    }

    #[test]
    fn infinite_floor_adds_only_a_floor() {
        let floor = load("infinite-floor").unwrap();
        assert_eq!(floor, infinite_floor(), "scenes/dev/infinite-floor.ron is the builder's scene");
        let spec = floor.floor.clone().expect("the scene has a floor");
        assert_eq!(spec.kind, FloorKind::HalfSpace);
        assert_eq!(spec.height, 0.0, "the towers stand on it");
        // Apart from the floor and the words, it is the void.
        let stripped =
            SceneSpec { floor: None, name: "void".into(), description: void().description, ..floor };
        assert_eq!(stripped, void());
        // Control: the floor is what differs.
        assert_ne!(infinite_floor().floor, void().floor);
    }
}
