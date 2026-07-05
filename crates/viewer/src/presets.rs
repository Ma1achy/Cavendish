//! The scenario preset library: a curated, milestone-spanning collection the panel offers for one-click
//! loading. Each preset is a fully-specified [`ScenarioParams`] the user can then edit freely. Kept
//! deliberately modest in voxel count and schedule length so a preset runs in a second or two.

use generate::{AtmoConfig, FieldSet, UldmConfig};

use crate::params::{
    Axis, BodySpec, DetectorSpec, OrientSpec, PathSpec, ScenarioParams, ScheduleSpec, TimingSpec,
};

/// One named preset, grouped for the picker.
pub struct Preset {
    pub name: &'static str,
    pub group: &'static str,
    pub params: ScenarioParams,
}

fn uniform(cadence: f64, n: usize) -> ScheduleSpec {
    ScheduleSpec::Uniform { cadence, n }
}

/// The preset collection, in display order (grouped by `group`).
pub fn presets() -> Vec<Preset> {
    let base = ScenarioParams::default();
    let mut out = Vec::new();
    let mut push = |name, group, params| {
        out.push(Preset {
            name,
            group,
            params,
        })
    };

    // ── Anchors ──
    push(
        "Concrete slab (DC)",
        "Anchors",
        ScenarioParams {
            body: BodySpec::Cuboid {
                half: [0.2, 1.0, 1.5],
            },
            mass: 10_000.0,
            pitch: 0.2,
            placement: [1.4, 0.0, 0.0],
            schedule: uniform(2.0, 8),
            ..base.clone()
        },
    );
    push(
        "Flyby (~7 mrad)",
        "Anchors",
        ScenarioParams {
            body: BodySpec::Sphere { r: 0.1 },
            mass: 10.0,
            pitch: 0.03,
            placement: [10.0, 0.0, 0.0],
            path: PathSpec::LinearPass {
                a: [0.0, 0.0, -10.0],
                b: [0.0, 0.0, 10.0],
            },
            timing: TimingSpec::Uniform { rate: 0.125 }, // 2.5 m/s over a length-20 path
            detectors: DetectorSpec {
                count: 1,
                spacing: 1.0,
                axis: Axis::Z,
            },
            schedule: uniform(0.25, 40),
            ..base.clone()
        },
    );
    push(
        "Oscillating mass (~2 mrad)",
        "Anchors",
        ScenarioParams {
            body: BodySpec::Sphere { r: 0.1 },
            mass: 1.0,
            pitch: 0.03,
            placement: [5.0, 0.0, 1.0],
            path: PathSpec::Oscillation {
                axis: [0.0, 0.0, 1.0],
                amp: 1.0,
                freq: 0.1,
                phase: std::f64::consts::FRAC_PI_2,
            },
            timing: TimingSpec::Uniform { rate: 1.0 },
            detectors: DetectorSpec {
                count: 1,
                spacing: 1.0,
                axis: Axis::Z,
            },
            schedule: uniform(0.5, 40),
            ..base.clone()
        },
    );

    // ── Motion ──
    push(
        "Two-detector flyby",
        "Motion",
        ScenarioParams {
            body: BodySpec::Sphere { r: 0.1 },
            mass: 10.0,
            pitch: 0.03,
            placement: [10.0, 0.0, 0.0],
            path: PathSpec::LinearPass {
                a: [0.0, 0.0, -10.0],
                b: [0.0, 0.0, 10.0],
            },
            timing: TimingSpec::Uniform { rate: 0.125 },
            detectors: DetectorSpec {
                count: 2,
                spacing: 4.0,
                axis: Axis::X,
            },
            schedule: uniform(0.25, 40),
            ..base.clone()
        },
    );
    push(
        "Orbiting mass",
        "Motion",
        ScenarioParams {
            body: BodySpec::Sphere { r: 0.15 },
            mass: 100.0,
            pitch: 0.04,
            placement: [0.0, 0.0, 2.0],
            path: PathSpec::Circular {
                radius: 3.0,
                freq: 0.05,
            },
            timing: TimingSpec::Uniform { rate: 1.0 },
            detectors: DetectorSpec {
                count: 1,
                spacing: 1.0,
                axis: Axis::Z,
            },
            schedule: uniform(0.5, 40),
            ..base.clone()
        },
    );
    push(
        "Lift transit + excision",
        "Motion",
        ScenarioParams {
            body: BodySpec::Sphere { r: 0.2 },
            mass: 1000.0,
            pitch: 0.06,
            placement: [2.0, 0.0, 0.0],
            path: PathSpec::LinearPass {
                a: [0.0, 0.0, -40.0],
                b: [0.0, 0.0, 40.0],
            },
            timing: TimingSpec::Uniform { rate: 0.16 }, // 12.8 m/s
            detectors: DetectorSpec {
                count: 1,
                spacing: 1.0,
                axis: Axis::Z,
            },
            schedule: uniform(0.5, 40),
            contamination: Some((0.15, 3)),
            ..base.clone()
        },
    );

    // ── Rotation ──
    push(
        "Dzhanibekov top",
        "Rotation",
        ScenarioParams {
            orient: OrientSpec::FreeRotation {
                omega0: [0.02, 3.0, 0.01],
            },
            schedule: uniform(0.05, 120),
            ..base.clone()
        },
    );
    push(
        "Pendulum (libration)",
        "Rotation",
        ScenarioParams {
            body: BodySpec::Cuboid {
                half: [0.2, 0.2, 0.5],
            },
            mass: 500.0,
            pitch: 0.1,
            placement: [3.0, 0.0, 2.0],
            orient: OrientSpec::Libration {
                axis: [0.0, 1.0, 0.0],
                pivot_distance: 2.0,
                theta0: 0.6,
                thetadot0: 0.0,
            },
            schedule: uniform(0.1, 100),
            ..base.clone()
        },
    );

    // ── Spectral ──
    push(
        "ULDM line",
        "Spectral",
        ScenarioParams {
            body: BodySpec::Cuboid {
                half: [0.2, 0.2, 0.2],
            },
            mass: 1000.0,
            pitch: 0.1,
            placement: [3.0, 0.0, 0.0],
            uldm: Some(UldmConfig {
                amplitude: 1e-3,
                frequency: 0.1,
                phase: 0.0,
            }),
            schedule: uniform(1.0, 256),
            ..base.clone()
        },
    );
    push(
        "Drive + ULDM (gappy)",
        "Spectral",
        ScenarioParams {
            body: BodySpec::Sphere { r: 0.15 },
            mass: 1000.0,
            pitch: 0.05,
            placement: [3.0, 0.0, 0.0],
            path: PathSpec::Oscillation {
                axis: [0.0, 0.0, 1.0],
                amp: 1.0,
                freq: 0.05,
                phase: 0.0,
            },
            timing: TimingSpec::Uniform { rate: 1.0 },
            uldm: Some(UldmConfig {
                amplitude: 5e-3,
                frequency: 0.1,
                phase: 0.0,
            }),
            schedule: ScheduleSpec::Gappy {
                cadence: 1.0,
                n: 256,
                p_drop: 0.2,
                seed: 11,
            },
            ..base.clone()
        },
    );
    push(
        "Jittered cadence",
        "Spectral",
        ScenarioParams {
            body: BodySpec::Cuboid {
                half: [0.2, 0.2, 0.2],
            },
            mass: 1000.0,
            pitch: 0.1,
            placement: [3.0, 0.0, 0.0],
            uldm: Some(UldmConfig {
                amplitude: 1e-3,
                frequency: 0.1,
                phase: 0.0,
            }),
            schedule: ScheduleSpec::Jittered {
                cadence: 1.0,
                n: 256,
                jitter: 0.3,
                seed: 5,
            },
            ..base.clone()
        },
    );

    // ── Channels ──
    push(
        "Full decomposition",
        "Channels",
        ScenarioParams {
            body: BodySpec::Cuboid {
                half: [0.2, 0.2, 0.2],
            },
            mass: 500.0,
            pitch: 0.1,
            placement: [3.0, 0.0, 2.5],
            uldm: Some(UldmConfig {
                amplitude: 1e-3,
                frequency: 0.1,
                phase: 0.3,
            }),
            atmo: Some(AtmoConfig {
                n_modes: 16,
                correlation_length: 50.0,
                amplitude: 1.0,
                sound_speed: 343.0,
            }),
            noise: crate::params::NoiseSpec {
                shot: Some(1e-4),
                vibration: None,
            },
            fields: FieldSet {
                shape: true,
                decomposition: true,
                periodogram: true,
            },
            schedule: uniform(2.0, 32),
            ..base.clone()
        },
    );

    // ── Bodies ──
    push(
        "Sphere",
        "Bodies",
        ScenarioParams {
            body: BodySpec::Sphere { r: 0.3 },
            mass: 1000.0,
            pitch: 0.08,
            placement: [2.5, 0.0, 2.5],
            ..base.clone()
        },
    );
    push(
        "Tumbling cylinder",
        "Bodies",
        ScenarioParams {
            body: BodySpec::Cylinder {
                r: 0.3,
                half_l: 0.6,
            },
            mass: 1000.0,
            pitch: 0.1,
            placement: [2.5, 0.0, 2.5],
            orient: OrientSpec::FreeRotation {
                omega0: [2.0, 0.0, 0.05],
            },
            schedule: uniform(0.1, 80),
            ..base.clone()
        },
    );
    push(
        "Spherical shell",
        "Bodies",
        ScenarioParams {
            body: BodySpec::Shell {
                r: 0.4,
                thickness: 0.1,
            },
            mass: 1000.0,
            pitch: 0.1,
            placement: [2.5, 0.0, 2.5],
            ..base.clone()
        },
    );
    push(
        "Scaffold (union)",
        "Bodies",
        ScenarioParams {
            body: BodySpec::Scaffold,
            mass: 500.0,
            pitch: 0.08,
            placement: [3.0, 0.0, 2.0],
            orient: OrientSpec::FreeRotation {
                omega0: [0.3, 1.5, 0.1],
            },
            schedule: uniform(0.1, 80),
            ..base.clone()
        },
    );
    push(
        "Imported mesh (tumbling)",
        "Bodies",
        ScenarioParams {
            body: BodySpec::MeshDemo,
            mass: 1.0,
            pitch: 0.05,
            placement: [3.0, 0.0, 3.0],
            orient: OrientSpec::FreeRotation {
                omega0: [0.02, 3.0, 0.01],
            },
            schedule: uniform(0.05, 120),
            ..base.clone()
        },
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::build_scenario;

    #[test]
    fn preset_names_unique() {
        let ps = presets();
        let mut names: Vec<&str> = ps.iter().map(|p| p.name).collect();
        names.sort_unstable();
        let n = names.len();
        names.dedup();
        assert_eq!(names.len(), n, "preset names must be unique");
        assert!(n >= 15, "expected a substantial library, got {n}");
    }

    #[test]
    fn every_preset_runs() {
        // Every preset builds a valid Scenario (voxelises its body, wires its motion/schedule), and a
        // short run of each executes end to end — construction coverage across every Path/Orient/body.
        for preset in presets() {
            assert!(
                build_scenario(&preset.params).is_ok(),
                "preset '{}' failed to build",
                preset.name
            );
            // A tiny schedule keeps the test fast while still exercising the forward model per preset.
            let mut quick = preset.params.clone();
            quick.schedule = ScheduleSpec::Uniform { cadence: 1.0, n: 4 };
            let scenario = build_scenario(&quick).expect("quick build");
            let bundle = generate::run(&scenario);
            assert_eq!(bundle.time.len(), 4, "preset '{}' time length", preset.name);
            assert_eq!(
                bundle.signal.len(),
                4,
                "preset '{}' signal rows",
                preset.name
            );
        }
    }
}
