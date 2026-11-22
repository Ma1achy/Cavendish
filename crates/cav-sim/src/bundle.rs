//! The bundle: what a run hands on, as a directory of `.npy` arrays and a manifest.
//!
//! Names and shapes are v1's `StateBundle` (`design/state.md` at tag `v1-final`): measurement
//! first, `(T, D)` for a value of each of `T` shots at each of `D` towers. Everything is `f64`
//! (`<f8`), the engine's own precision until the GPU path of S5; `valid` is `bool` (`|b1`).
//!
//! | File | Shape | What |
//! |---|---|---|
//! | `time.npy` | `(T,)` | each shot's readout on the array's clock, seconds |
//! | `detector_time.npy` | `(T, D)` | each tower's own readout time, seconds |
//! | `valid.npy` | `(T, D)` | whether the tower took the shot |
//! | `signal.npy` | `(T, D)` | the gradiometer's ΔΦ, radians: the sum over sources of each source's own difference, so it is `signal_per_ifo`'s difference to rounding, not to the bit |
//! | `signal_per_ifo.npy` | `(T, D, 2)` | δφ₁ (the lower interferometer), δφ₂ (the upper), radians |
//! | `detector_placement.npy` | `(D, 7)` | each tower's base x, y, z (metres), then the quaternion w, x, y, z of its axis (upright: 1, 0, 0, 0) |
//! | `source_position.npy` | `(S, T, 3)` | each body's position at `time`, metres |
//! | `source_mass.npy` | `(S,)` | kilograms |
//! | `error_estimate.npy` | `(D,)` | the largest quadrature estimate of any shot of the tower, radians |
//! | `scene.ron` | | the scene, as `load` reads it |
//! | `manifest.json` | | the arrays' names, shapes, dtypes and units; the numerics; the scene; the sign |
//!
//! **Sign.** ΔΦ is lower minus upper (δφ₁ − δφ₂) unless the scene's instrument says otherwise;
//! v1's bundles were upper minus lower. The manifest says which this one is.
//!
//! **What a bundle never holds.** A NaN: its bits are not the same from run to run, so a bundle
//! with one could not be compared with its own regeneration; [`write`] refuses, naming the
//! place. A value for a shot that was not taken: those hold 0 and `valid` says so. An infinity
//! is allowed and is exact in `.npy`: over a half-space floor each interferometer's own phase is
//! infinite (the gradiometer's is not), and the manifest carries the warning.
//!
//! Arrays are written through a buffer, value by value: writing a bundle holds no second copy of
//! the signal.
//!
//! Frame: metres, x east, y north, z up; seconds on the scene's clock.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use cav_core::forward::{
    DEFAULT_QUADRATURE, MAX_HALVINGS, MAX_NODES, PHASE_TOLERANCE, PHASE_TOLERANCE_RELATIVE,
};
use cav_core::npy;

use crate::read::Signal;
use crate::{SceneSpec, SignSpec};

/// The version of the bundle's layout this build writes.
pub const BUNDLE_VERSION: u32 = 1;

/// Who made a bundle: the engine's version and the revision of its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    /// The engine's version, as `cav doctor` reports it.
    pub version: String,
    /// The git revision of the source the engine was built from, or `unknown`.
    pub revision: String,
    /// Whether that source had changes that were not committed; `None` when it is not known.
    pub dirty: Option<bool>,
}

/// One array of a bundle, as the manifest lists it.
struct Listed {
    name: &'static str,
    shape: Vec<usize>,
    dtype: &'static str,
    unit: &'static str,
    about: &'static str,
}

fn create(dir: &Path, name: &str) -> Result<std::io::BufWriter<std::fs::File>, String> {
    let path = dir.join(name);
    std::fs::File::create(&path).map(std::io::BufWriter::new).map_err(|e| format!("{}: {e}", path.display()))
}

fn finish(
    dir: &Path,
    name: &str,
    written: std::io::Result<()>,
    mut file: std::io::BufWriter<std::fs::File>,
) -> Result<(), String> {
    written.and_then(|()| file.flush()).map_err(|e| format!("{}: {e}", dir.join(name).display()))
}

/// A channel held tower by tower, read out shots first: the order of a `(T, D)` array.
fn by_shot(values: &[f64], shots: usize, towers: usize) -> impl Iterator<Item = f64> + '_ {
    (0..shots).flat_map(move |k| (0..towers).map(move |d| values[d * shots + k]))
}

/// The first value of the signal that is not a number, as a place a person can find.
fn first_nan(signal: &Signal) -> Option<String> {
    let shots = signal.times.len();
    for (name, values) in [("ΔΦ", &signal.gradiometer), ("δφ₁", &signal.lower), ("δφ₂", &signal.upper)]
    {
        if let Some(at) = values.iter().position(|value| value.is_nan()) {
            return Some(format!(
                "{name} of tower {} at shot {} (read out at {} s) is not a number",
                at / shots.max(1) + 1,
                at % shots.max(1) + 1,
                signal.readout(at / shots.max(1), at % shots.max(1))
            ));
        }
    }
    None
}

/// Writes the bundle of `spec` and its `signal` into `dir` (made if it is not there), and gives
/// the files written. The signal must be the scene's own, whole: the caller (the engine) writes
/// only from a refined one.
///
/// # Errors
/// When the signal is not this scene's (another number of towers), holds a NaN, or a file cannot
/// be written. Files already written stay; the manifest is written last, so a directory with a
/// manifest is a whole bundle.
pub fn write(dir: &Path, spec: &SceneSpec, signal: &Signal, build: &Build) -> Result<Vec<PathBuf>, String> {
    let (towers, shots, bodies) = (signal.towers, signal.times.len(), spec.bodies.len());
    if towers != spec.array.towers.len() {
        return Err(format!(
            "the signal is of {towers} tower(s) and the scene has {}: it is not this scene's signal",
            spec.array.towers.len()
        ));
    }
    if let Some(place) = first_nan(signal) {
        return Err(format!(
            "{place}; a bundle is never written with one (its bits would differ from run to run)"
        ));
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut listed: Vec<Listed> = Vec::new();
    let mut list = |name, shape: &[usize], dtype, unit, about| {
        listed.push(Listed { name, shape: shape.to_vec(), dtype, unit, about });
    };
    // Shots first, then towers: (T, D). The signal itself is held tower by tower.

    let mut file = create(dir, "time.npy")?;
    let written = npy::write_f64(&mut file, &[shots], signal.times.iter().copied());
    finish(dir, "time.npy", written, file)?;
    list("time", &[shots], npy::F64, "s", "each shot's readout on the array's clock");

    let mut file = create(dir, "detector_time.npy")?;
    let times = (0..shots).flat_map(|k| (0..towers).map(move |d| signal.readout(d, k)));
    let written = npy::write_f64(&mut file, &[shots, towers], times);
    finish(dir, "detector_time.npy", written, file)?;
    list(
        "detector_time",
        &[shots, towers],
        npy::F64,
        "s",
        "each tower's own readout time: the array's clock and the tower's offset",
    );

    let mut file = create(dir, "valid.npy")?;
    let taken = (0..shots).flat_map(|k| (0..towers).map(move |d| signal.valid[d * shots + k]));
    let written = npy::write_bool(&mut file, &[shots, towers], taken);
    finish(dir, "valid.npy", written, file)?;
    list(
        "valid",
        &[shots, towers],
        npy::BOOL,
        "",
        "whether the tower took the shot; where false the signal holds 0, which is not a measurement",
    );

    let mut file = create(dir, "signal.npy")?;
    let written = npy::write_f64(&mut file, &[shots, towers], by_shot(&signal.gradiometer, shots, towers));
    finish(dir, "signal.npy", written, file)?;
    list(
        "signal",
        &[shots, towers],
        npy::F64,
        "rad",
        "the gradiometer's differential phase, with the sign the manifest states",
    );

    let mut file = create(dir, "signal_per_ifo.npy")?;
    let both = (0..shots).flat_map(|k| {
        (0..towers).flat_map(move |d| [signal.lower[d * shots + k], signal.upper[d * shots + k]])
    });
    let written = npy::write_f64(&mut file, &[shots, towers, 2], both);
    finish(dir, "signal_per_ifo.npy", written, file)?;
    list(
        "signal_per_ifo",
        &[shots, towers, 2],
        npy::F64,
        "rad",
        "the two interferometers' own phases: [..., 0] the lower (δφ₁), [..., 1] the upper (δφ₂)",
    );

    let mut file = create(dir, "detector_placement.npy")?;
    let placed = spec.array.towers.iter().flat_map(|tower| {
        let [x, y, z] = tower.position;
        [x, y, z, 1.0, 0.0, 0.0, 0.0]
    });
    let written = npy::write_f64(&mut file, &[towers, 7], placed);
    finish(dir, "detector_placement.npy", written, file)?;
    list(
        "detector_placement",
        &[towers, 7],
        npy::F64,
        "m, then 1",
        "each tower's base x, y, z, then the quaternion w, x, y, z of its axis (upright)",
    );

    let gravity = spec.array.instrument.gravity;
    let mut file = create(dir, "source_position.npy")?;
    let positions = spec
        .bodies
        .iter()
        .flat_map(|body| signal.times.iter().flat_map(move |time| body.motion.position(*time, gravity)));
    let written = npy::write_f64(&mut file, &[bodies, shots, 3], positions);
    finish(dir, "source_position.npy", written, file)?;
    list(
        "source_position",
        &[bodies, shots, 3],
        npy::F64,
        "m",
        "each body's position at `time` (the array's clock), whether or not it exists then",
    );

    let mut file = create(dir, "source_mass.npy")?;
    let written = npy::write_f64(&mut file, &[bodies], spec.bodies.iter().map(|body| body.mass));
    finish(dir, "source_mass.npy", written, file)?;
    list("source_mass", &[bodies], npy::F64, "kg", "each body's mass, in the scene's order");

    let mut file = create(dir, "error_estimate.npy")?;
    let written = npy::write_f64(&mut file, &[towers], signal.estimates.iter().copied());
    finish(dir, "error_estimate.npy", written, file)?;
    list(
        "error_estimate",
        &[towers],
        npy::F64,
        "rad",
        "the largest quadrature estimate of any shot of any source at the tower: measured, by reading until two rules agree",
    );

    let scene = dir.join("scene.ron");
    std::fs::write(&scene, spec.to_ron()).map_err(|e| format!("{}: {e}", scene.display()))?;

    let manifest = dir.join("manifest.json");
    let text =
        serde_json::to_string_pretty(&manifest_of(spec, signal, build, &listed)).expect("a manifest is JSON");
    std::fs::write(&manifest, text + "\n").map_err(|e| format!("{}: {e}", manifest.display()))?;

    let mut files: Vec<PathBuf> =
        listed.iter().map(|array| dir.join(format!("{}.npy", array.name))).collect();
    files.extend([scene, manifest]);
    Ok(files)
}

/// The numerics block: what made the numbers, so that they can be made again or doubted.
#[must_use]
pub fn numerics(spec: &SceneSpec, build: &Build) -> serde_json::Value {
    let instrument = spec.array.instrument.instrument();
    serde_json::json!({
        "engine": { "version": build.version, "revision": build.revision, "dirty": build.dirty },
        "platform": format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
        "precision": "f64 on the CPU",
        "quadrature": format!("{DEFAULT_QUADRATURE:?}, panels halved until two rules agree"),
        "phase_tolerance_rad": PHASE_TOLERANCE,
        "phase_tolerance_relative": PHASE_TOLERANCE_RELATIVE,
        "max_halvings": MAX_HALVINGS,
        "max_nodes_per_read": MAX_NODES,
        "constants": {
            "big_g": instrument.big_g,
            "hbar": instrument.hbar,
            "atom_mass_kg": instrument.atom_mass,
            "kick_velocity_m_per_s": instrument.kick_velocity(),
            "gravity_m_per_s2": instrument.gravity,
        },
        "replay": "bit-identical with the same binary on the same platform (sin and cos are the platform's)",
    })
}

fn manifest_of(spec: &SceneSpec, signal: &Signal, build: &Build, listed: &[Listed]) -> serde_json::Value {
    let arrays: Vec<serde_json::Value> = listed
        .iter()
        .map(|array| {
            serde_json::json!({
                "name": array.name,
                "file": format!("{}.npy", array.name),
                "shape": array.shape,
                "dtype": array.dtype,
                "unit": array.unit,
                "about": array.about,
            })
        })
        .collect();
    let sign = match spec.array.instrument.sign {
        SignSpec::LowerMinusUpper => {
            "lower minus upper (δφ₁ − δφ₂): George's convention. v1's bundles are upper minus lower: the opposite sign"
        }
        SignSpec::UpperMinusLower => {
            "upper minus lower (δφ₂ − δφ₁): v1's convention, set by this scene's instrument"
        }
    };
    serde_json::json!({
        "cavendish_bundle": BUNDLE_VERSION,
        "scene": spec.name,
        "shots": signal.times.len(),
        "towers": signal.towers,
        "sources": spec.bodies.len(),
        "sign": sign,
        "settled": signal.settled,
        "warnings": signal.warnings.iter().map(|warning| warning.text.clone()).collect::<Vec<_>>(),
        "numerics": numerics(spec, build),
        "arrays": arrays,
        "spec": spec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dev, read};
    use cav_validate::Measured;

    fn build() -> Build {
        Build { version: "0.0.0".into(), revision: "0123abc".into(), dirty: Some(false) }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/bundles").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// The staggered scene, short, with a schedule that leaves shots out: every kind of array
    /// has something in it.
    fn scene() -> SceneSpec {
        let mut scene = dev::staggered();
        scene.duration = 60.0;
        scene.array.schedule.loss = 0.2;
        scene.bodies.push(crate::BodySpec::point("still", 3.0, [1.0, -2.0, 0.5]));
        assert_eq!(scene.validate(), Ok(()));
        scene
    }

    fn array(dir: &Path, name: &str) -> npy::Array {
        npy::parse(&std::fs::read(dir.join(format!("{name}.npy"))).unwrap())
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    }

    cav_validate::case! {
        test: validation_bundle_round_trips,
        name: "bundle.round_trip",
        subsystem: "bundle",
        slice: "S2",
        reference: Exact,
        against: "a bundle of the staggered scene (five towers on their own clocks, a fifth of the shots lost, two bodies) written and read back; values of any array that differ in any bit from what the engine holds, and shapes that are not the stated ones",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let scene = scene();
            let signal = read::signal(&scene).unwrap();
            let dir = scratch("round trip");
            let files = write(&dir, &scene, &signal, &build()).unwrap();
            assert_eq!(files.len(), 11);
            let (shots, towers) = (signal.times.len(), 5);
            let mut wrong = 0usize;
            let mut check = |name: &str, shape: &[usize], expected: Vec<f64>| {
                let (found, values) = array(&dir, name).into_f64().unwrap();
                wrong += usize::from(found != shape);
                wrong += usize::from(values.len() != expected.len());
                wrong += values.iter().zip(&expected).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
            };
            let by_shot = |values: &[f64]| -> Vec<f64> {
                (0..shots).flat_map(|k| (0..towers).map(move |d| (d, k))).map(|(d, k)| values[d * shots + k]).collect()
            };
            check("time", &[shots], signal.times.clone());
            check("detector_time", &[shots, towers], (0..shots).flat_map(|k| (0..towers).map(move |d| (d, k))).map(|(d, k)| signal.readout(d, k)).collect());
            check("signal", &[shots, towers], by_shot(&signal.gradiometer));
            let per_ifo: Vec<f64> = (0..shots)
                .flat_map(|k| (0..towers).map(move |d| (d, k)))
                .flat_map(|(d, k)| [signal.lower[d * shots + k], signal.upper[d * shots + k]])
                .collect();
            check("signal_per_ifo", &[shots, towers, 2], per_ifo);
            check("error_estimate", &[towers], signal.estimates.clone());
            check("source_mass", &[2], vec![scene.bodies[0].mass, 3.0]);
            let placement: Vec<f64> = scene.array.towers.iter().flat_map(|t| [t.position[0], t.position[1], t.position[2], 1.0, 0.0, 0.0, 0.0]).collect();
            check("detector_placement", &[towers, 7], placement);
            let positions: Vec<f64> = scene.bodies.iter().flat_map(|body| signal.times.iter().flat_map(|t| body.motion.position(*t, 9.81))).collect();
            check("source_position", &[2, shots, 3], positions);
            let (shape, taken) = array(&dir, "valid").into_bool().unwrap();
            let expected: Vec<bool> = (0..shots).flat_map(|k| (0..towers).map(move |d| (d, k))).map(|(d, k)| signal.valid[d * shots + k]).collect();
            wrong += usize::from(shape != [shots, towers]) + taken.iter().zip(&expected).filter(|(a, b)| a != b).count();
            // The scene in the bundle is the scene.
            assert_eq!(crate::load(dir.join("scene.ron").to_str().unwrap()).unwrap(), scene);
            // What the arrays say is what was read: the gradiometer is lower minus upper, a shot
            // not taken is exactly zero, and some shots were not taken.
            let (_, signal_values) = array(&dir, "signal").into_f64().unwrap();
            let (_, pair) = array(&dir, "signal_per_ifo").into_f64().unwrap();
            // (To rounding and not to the bit: ΔΦ is the sum over sources of each source's own
            // difference, which is how the difference is kept free of cancellation.)
            let peak = signal_values.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            assert!((0..shots * towers).all(|n| (signal_values[n] - (pair[2 * n] - pair[2 * n + 1])).abs() <= 1e-12 * peak));
            let untaken = taken.iter().filter(|t| !**t).count();
            assert!(untaken >= 15 && taken.iter().zip(&signal_values).all(|(t, v)| *t || v.to_bits() == 0), "{untaken} not taken");
            // Control: one bit of one value changed in a file is one value that differs.
            let path = dir.join("signal.npy");
            let mut bytes = std::fs::read(&path).unwrap();
            let last = bytes.len() - 8;
            bytes[last] ^= 1;
            std::fs::write(&path, &bytes).unwrap();
            let (_, nudged) = array(&dir, "signal").into_f64().unwrap();
            assert_eq!(nudged.iter().zip(&signal_values).filter(|(a, b)| a.to_bits() != b.to_bits()).count(), 1);
            Measured::Error(wrong as f64)
        },
    }

    #[test]
    fn a_bundle_with_a_nan_is_refused() {
        let scene = scene();
        let mut signal = read::signal(&scene).unwrap();
        let shots = signal.times.len();
        let dir = scratch("not a number");
        // One value that is not a number, in the upper interferometer of tower 3 at shot 5.
        signal.upper[2 * shots + 4] = f64::NAN;
        let why = write(&dir, &scene, &signal, &build()).unwrap_err();
        assert!(why.contains("δφ₂ of tower 3 at shot 5") && why.contains("is not a number"), "{why}");
        assert!(!dir.exists(), "nothing was written, not even the directory");
        // An infinity is exact and is allowed: the half-space's own channels.
        let floor = dev::infinite_floor();
        let infinite = read::signal(&floor).unwrap();
        let dir = scratch("half space");
        write(&dir, &floor, &infinite, &build()).unwrap();
        let (_, pair) = array(&dir, "signal_per_ifo").into_f64().unwrap();
        assert!(pair.iter().all(|value| *value == f64::INFINITY));
        let (_, null) = array(&dir, "signal").into_f64().unwrap();
        assert!(null.iter().all(|value| value.to_bits() == 0));
        let manifest = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
        assert!(manifest.contains("the floor is a half-space"), "the manifest carries the warning");
        // A signal that is another scene's is refused too. Control: the whole one is written.
        let other = read::signal(&dev::point()).unwrap();
        let mut fewer = scene.clone();
        fewer.array.towers.truncate(3);
        assert!(write(&dir, &fewer, &other, &build()).unwrap_err().contains("not this scene's signal"));
        signal.upper[2 * shots + 4] = 0.0;
        assert!(write(&scratch("whole"), &scene, &signal, &build()).is_ok());
    }

    #[test]
    fn the_manifest_says_what_made_it() {
        let scene = scene();
        let signal = read::signal(&scene).unwrap();
        let dir = scratch("manifest");
        write(&dir, &scene, &signal, &build()).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["cavendish_bundle"], 1);
        assert_eq!(
            (&manifest["scene"], &manifest["towers"], &manifest["sources"]),
            (&"staggered".into(), &5.into(), &2.into())
        );
        // The sign, in words, and that it is not v1's.
        let sign = manifest["sign"].as_str().unwrap();
        assert!(
            sign.starts_with("lower minus upper") && sign.contains("v1's bundles are upper minus lower"),
            "{sign}"
        );
        // The numerics: precision, quadrature, tolerances, the engine's revision, the platform.
        let numerics = &manifest["numerics"];
        assert_eq!(
            numerics["engine"],
            serde_json::json!({"version": "0.0.0", "revision": "0123abc", "dirty": false})
        );
        assert_eq!(numerics["precision"], "f64 on the CPU");
        assert_eq!(numerics["phase_tolerance_rad"], 1e-10);
        assert!(numerics["quadrature"].as_str().unwrap().contains("Gauss"));
        assert_eq!(numerics["constants"]["atom_mass_kg"], cav_core::constants::SR87_MASS);
        assert!(numerics["replay"].as_str().unwrap().contains("same platform"));
        // Every array is listed with its shape, dtype and unit, and the file is there.
        let arrays = manifest["arrays"].as_array().unwrap();
        assert_eq!(arrays.len(), 9);
        for listed in arrays {
            let found = array(&dir, listed["name"].as_str().unwrap());
            let shape: Vec<usize> =
                listed["shape"].as_array().unwrap().iter().map(|n| n.as_u64().unwrap() as usize).collect();
            assert_eq!(found.shape, shape, "{}", listed["name"]);
            assert_eq!(found.descr(), listed["dtype"].as_str().unwrap());
        }
        let signal_entry = arrays.iter().find(|a| a["name"] == "signal").unwrap();
        assert_eq!((&signal_entry["unit"], &signal_entry["dtype"]), (&"rad".into(), &"<f8".into()));
        // The scene, verbatim: the manifest alone regenerates the bundle.
        assert_eq!(SceneSpec::from_json(&manifest["spec"].to_string()).unwrap(), scene);
        // Control: v1's sign is said as v1's when the scene asks for it.
        let mut flipped = scene.clone();
        flipped.array.instrument.sign = SignSpec::UpperMinusLower;
        let signal = read::signal(&flipped).unwrap();
        write(&dir, &flipped, &signal, &build()).unwrap();
        let again = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
        assert!(again.contains("upper minus lower (δφ₂ − δφ₁): v1's convention"));
    }
}
