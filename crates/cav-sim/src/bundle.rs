//! The bundle: what a run hands on, as a directory of `.npy` arrays and a manifest.
//!
//! Names and shapes are v1's `StateBundle` (`design/state.md` at tag `v1-final`): measurement
//! first, `(T_max, D)` for a value of each of up to `T_max` shots at each of `D` towers. Each tower
//! has its own shots (its own cycle and 2T, S3), so the signal is **ragged**: the `(T_max, D)`
//! arrays are a padded view, `mask` saying where a tower has a shot at all; the `_flat` arrays are
//! the same values stored tower after tower with `tower_offsets` (CSR), which is how the engine
//! holds them. When every tower flies the array's instrument, `T_max` is every tower's count,
//! `mask` is all true and the padded arrays are S2's, bit for bit. Everything is `f64` (`<f8`),
//! the engine's own precision until the GPU path of S5, counts included (whole numbers, exact);
//! `valid` and `mask` are `bool` (`|b1`).
//!
//! | File | Shape | What |
//! |---|---|---|
//! | `time.npy` | `(T,)` | each shot's readout on the array's clock at the array's own instrument, seconds: the nominal axis of the tracks |
//! | `detector_time.npy` | `(T_max, D)` | each tower's own readout time, seconds; NaN where `mask` is false |
//! | `mask.npy` | `(T_max, D)` | whether the tower has that shot at all |
//! | `n_shots.npy` | `(D,)` | how many shots each tower has |
//! | `valid.npy` | `(T_max, D)` | whether the tower took the shot (false in the padding) |
//! | `signal.npy` | `(T_max, D)` | the gradiometer's ΔΦ, radians: the sum over sources of each source's own difference, so it is `signal_per_ifo`'s difference to rounding, not to the bit; 0 in the padding |
//! | `signal_per_ifo.npy` | `(T_max, D, 2)` | δφ₁ (the lower interferometer), δφ₂ (the upper), radians |
//! | `tower_offsets.npy` | `(D + 1,)` | where each tower's shots begin in the `_flat` arrays, and the end |
//! | `t_flat.npy`, `valid_flat.npy`, `phase_flat.npy` | `(N,)` | each tower's own readout times, taken flags and ΔΦ, tower after tower |
//! | `phase_per_ifo_flat.npy` | `(N, 2)` | δφ₁, δφ₂, tower after tower |
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
//! **What a bundle never holds.** A NaN where there is a shot: its bits are not the same from run
//! to run, so a bundle with one could not be compared with its own regeneration; [`write()`]
//! refuses, naming the place. The one exception is the padding of `detector_time`, which is NaN
//! where `mask` is false (and only there), so that a consumer that forgets the mask fails loudly
//! rather than reading a time of zero; it is written as Rust's one quiet NaN, the same bits every
//! run. The manifest's `instruments` lists each distinct instrument once and `tower_instrument`
//! each tower's row. A value for a shot that was not taken: those hold 0 and `valid` says so. An infinity
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

/// Who made a bundle: the engine's version and the revision of its source. These are the
/// build's, fixed when the engine was compiled (`cav_engine::build`); nothing here is found out
/// when a file is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    /// The engine's version, as `cav doctor` reports it.
    pub version: String,
    /// The git revision that was checked out when the engine was compiled, or `unknown`.
    pub revision: String,
    /// Whether the engine's sources then had changes that were not committed; `None` when it is
    /// not known.
    pub dirty: Option<bool>,
}

/// What a manifest and a figure's provenance say `revision` and `dirty` are.
pub const BUILT_FROM: &str = "revision is the commit checked out when the engine was compiled; dirty is whether \
     the engine's sources (cav-core, cav-sim, cav-engine, the built-in scenes, the workspace's manifest, lock file \
     and toolchain) then differed from that commit: a tracked file changed or an untracked one present. Both are \
     the build's, fixed at compile time: git is not asked when this file is written. Either is `unknown` when \
     the engine was built outside a git repository";

impl Build {
    /// The build as a manifest and a figure's provenance carry it: `version`, `revision`, `dirty`
    /// (`true`, `false` or `"unknown"`) and `built_from`, which says what the two are.
    #[must_use]
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "version": self.version,
            "revision": self.revision,
            "dirty": self.dirty.map_or_else(|| serde_json::json!("unknown"), serde_json::Value::Bool),
            "built_from": BUILT_FROM,
        })
    }
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

/// A per-shot array of the signal (laid out tower after tower, each its own number of shots) in
/// the padded order of a `(T_max, D)` array: shots first, then towers; a tower with fewer shots
/// than `T_max` gives `pad` for each it lacks.
///
/// `value` is asked for tower `d`'s shot `k` and gives what is written there; nothing is gathered
/// first, so writing costs no second copy.
fn padded<'a, T: Copy + 'a>(
    signal: &'a Signal,
    value: impl Fn(usize, usize) -> T + Copy + 'a,
    pad: T,
) -> impl Iterator<Item = T> + 'a {
    let (most, towers) = (signal.most(), signal.towers);
    (0..most).flat_map(move |k| (0..towers).map(move |d| if k < signal.count(d) { value(d, k) } else { pad }))
}

/// Every shot of every tower, tower after tower: `(tower, shot)`, the order of the `_flat` arrays.
fn flat(signal: &Signal) -> impl Iterator<Item = (usize, usize)> + '_ {
    (0..signal.towers).flat_map(move |d| (0..signal.count(d)).map(move |k| (d, k)))
}

/// The first value of the signal that is not a number, as a place a person can find.
fn first_nan(signal: &Signal) -> Option<String> {
    for (name, values) in [("ΔΦ", &signal.gradiometer), ("δφ₁", &signal.lower), ("δφ₂", &signal.upper)]
    {
        if let Some(at) = values.iter().position(|value| value.is_nan()) {
            // The tower whose stretch holds it, and the shot within that tower's.
            let tower = signal.starts.partition_point(|start| *start <= at) - 1;
            let k = at - signal.starts[tower];
            return Some(format!(
                "{name} of tower {} at shot {} (read out at {} s) is not a number",
                tower + 1,
                k + 1,
                signal.readout(tower, k)
            ));
        }
    }
    None
}

/// Writes the bundle of `spec` and its `signal` into `dir` (made if it is not there), and gives
/// the files written. The signal must be the scene's own, whole: the caller (the engine) writes
/// only from a refined one.
///
/// A bundle is of the scene's own instruments and nothing else. `compared_with` names the
/// instrument the session was also reading the scene on, if it was: the manifest then says, under
/// `comparison`, that that reading is not in the bundle (and `null` otherwise).
///
/// # Errors
/// When the signal is not this scene's (another number of towers), holds a NaN, the scene's
/// timeline has too many shots, or a file cannot be written. Files already written stay; the
/// manifest is written last, so a directory with a manifest is a whole bundle.
pub fn write(
    dir: &Path,
    spec: &SceneSpec,
    signal: &Signal,
    build: &Build,
    compared_with: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    let (towers, most, total, bodies) = (signal.towers, signal.most(), signal.clock.len(), spec.bodies.len());
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
    // The nominal clock: the array's own instrument's readouts, on which the tracks are given. A
    // tower that flies the array's cycle and 2T has exactly those times (its clock, before its
    // offset), so they are read from it; only an array with no such tower makes them afresh
    // (at most MAX_SHOTS of them, 16 MB).
    let own = (0..towers).find(|&d| {
        let i = spec.array.instrument_of(d);
        i.cycle.to_bits() == spec.array.instrument.cycle.to_bits()
            && i.half_sequence.to_bits() == spec.array.instrument.half_sequence.to_bits()
    });
    let time: std::borrow::Cow<'_, [f64]> = match own {
        Some(d) => std::borrow::Cow::Borrowed(signal.times(d)),
        None => std::borrow::Cow::Owned(crate::read::readouts(spec)?),
    };
    let nominal = time.len();
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut listed: Vec<Listed> = Vec::new();
    let mut list = |name, shape: &[usize], dtype, unit, about| {
        listed.push(Listed { name, shape: shape.to_vec(), dtype, unit, about });
    };
    // Shots first, then towers: (T_max, D), padded where a tower has fewer shots than T_max. The
    // signal itself is held tower by tower, each its own length: the `_flat` arrays are that.

    let mut file = create(dir, "time.npy")?;
    let written = npy::write_f64(&mut file, &[nominal], time.iter().copied());
    finish(dir, "time.npy", written, file)?;
    list(
        "time",
        &[nominal],
        npy::F64,
        "s",
        "each shot's readout on the array's clock at the array's own instrument: the nominal axis of the tracks",
    );

    let mut file = create(dir, "detector_time.npy")?;
    let readout = |d: usize, k: usize| signal.readout(d, k);
    let written = npy::write_f64(&mut file, &[most, towers], padded(signal, readout, f64::NAN));
    finish(dir, "detector_time.npy", written, file)?;
    list(
        "detector_time",
        &[most, towers],
        npy::F64,
        "s",
        "each tower's own readout time: its clock and its offset; NaN where `mask` is false (no such shot)",
    );

    let mut file = create(dir, "mask.npy")?;
    let written = npy::write_bool(&mut file, &[most, towers], padded(signal, |_, _| true, false));
    finish(dir, "mask.npy", written, file)?;
    list(
        "mask",
        &[most, towers],
        npy::BOOL,
        "",
        "whether the tower has that shot at all: false in the padding of a tower with fewer than T_max shots",
    );

    let mut file = create(dir, "n_shots.npy")?;
    let written = npy::write_f64(&mut file, &[towers], (0..towers).map(|d| signal.count(d) as f64));
    finish(dir, "n_shots.npy", written, file)?;
    list("n_shots", &[towers], npy::F64, "1", "how many shots each tower has (a whole number, as f8)");

    let mut file = create(dir, "valid.npy")?;
    let written = npy::write_bool(&mut file, &[most, towers], padded(signal, |d, k| signal.valid[signal.starts[d] + k], false));
    finish(dir, "valid.npy", written, file)?;
    list(
        "valid",
        &[most, towers],
        npy::BOOL,
        "",
        "whether the tower took the shot; where false the signal holds 0, which is not a measurement",
    );

    let mut file = create(dir, "signal.npy")?;
    let written = npy::write_f64(&mut file, &[most, towers], padded(signal, |d, k| signal.gradiometer[signal.starts[d] + k], 0.0));
    finish(dir, "signal.npy", written, file)?;
    list(
        "signal",
        &[most, towers],
        npy::F64,
        "rad",
        "the gradiometer's differential phase, with each tower's sign as the manifest states; 0 where `valid` is false",
    );

    let mut file = create(dir, "signal_per_ifo.npy")?;
    let pair = |d: usize, k: usize| {
        let at = signal.starts[d] + k;
        [signal.lower[at], signal.upper[at]]
    };
    let both = padded(signal, pair, [0.0, 0.0]).flatten();
    let written = npy::write_f64(&mut file, &[most, towers, 2], both);
    finish(dir, "signal_per_ifo.npy", written, file)?;
    list(
        "signal_per_ifo",
        &[most, towers, 2],
        npy::F64,
        "rad",
        "the two interferometers' own phases: [..., 0] the lower (δφ₁), [..., 1] the upper (δφ₂)",
    );

    let mut file = create(dir, "tower_offsets.npy")?;
    let written = npy::write_f64(&mut file, &[towers + 1], signal.starts.iter().map(|s| *s as f64));
    finish(dir, "tower_offsets.npy", written, file)?;
    list(
        "tower_offsets",
        &[towers + 1],
        npy::F64,
        "1",
        "where each tower's shots begin in the `_flat` arrays, and the end: D + 1 whole numbers, as f8",
    );

    let mut file = create(dir, "t_flat.npy")?;
    let written = npy::write_f64(&mut file, &[total], flat(signal).map(|(d, k)| signal.readout(d, k)));
    finish(dir, "t_flat.npy", written, file)?;
    list("t_flat", &[total], npy::F64, "s", "each tower's own readout times, tower after tower (CSR)");

    let mut file = create(dir, "valid_flat.npy")?;
    let written = npy::write_bool(&mut file, &[total], signal.valid.iter().copied());
    finish(dir, "valid_flat.npy", written, file)?;
    list("valid_flat", &[total], npy::BOOL, "", "whether each shot was taken, tower after tower (CSR)");

    let mut file = create(dir, "phase_flat.npy")?;
    let written = npy::write_f64(&mut file, &[total], signal.gradiometer.iter().copied());
    finish(dir, "phase_flat.npy", written, file)?;
    list("phase_flat", &[total], npy::F64, "rad", "the gradiometer's ΔΦ, tower after tower (CSR)");

    let mut file = create(dir, "phase_per_ifo_flat.npy")?;
    let written = npy::write_f64(&mut file, &[total, 2], signal.lower.iter().zip(&signal.upper).flat_map(|(l, u)| [*l, *u]));
    finish(dir, "phase_per_ifo_flat.npy", written, file)?;
    list("phase_per_ifo_flat", &[total, 2], npy::F64, "rad", "δφ₁ and δφ₂, tower after tower (CSR)");

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
    let positions =
        spec.bodies.iter().flat_map(|body| time.iter().flat_map(move |time| body.motion.position(*time, gravity)));
    let written = npy::write_f64(&mut file, &[bodies, nominal, 3], positions);
    finish(dir, "source_position.npy", written, file)?;
    list(
        "source_position",
        &[bodies, nominal, 3],
        npy::F64,
        "m",
        "each body's position at `time` (the array's nominal clock), whether or not it exists then",
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
    let comparison = compared_with.map(|name| {
        format!(
            "this bundle is of the scene's own instruments alone (`spec.array.instrument` and each tower's \
             override). The session was also reading the scene on {name}: none of that reading is in this bundle"
        )
    });
    let text = serde_json::to_string_pretty(&manifest_of(spec, signal, build, &listed, comparison))
        .expect("a manifest is JSON");
    std::fs::write(&manifest, text + "\n").map_err(|e| format!("{}: {e}", manifest.display()))?;

    let mut files: Vec<PathBuf> =
        listed.iter().map(|array| dir.join(format!("{}.npy", array.name))).collect();
    files.extend([scene, manifest]);
    Ok(files)
}

/// The instruments of the array, each once, and which each tower flies: `(table, column)`. The
/// table lists the distinct resolved instruments in the order of the first tower that flies each
/// (the array's own first, whether or not a tower flies it), as the spec writes an instrument;
/// the column gives each tower's row.
#[must_use]
pub fn instrument_table(spec: &SceneSpec) -> (Vec<crate::InstrumentSpec>, Vec<usize>) {
    let mut table = vec![spec.array.instrument.clone()];
    let mut column = Vec::with_capacity(spec.array.towers.len());
    for own in spec.array.instruments() {
        let key = crate::read::instrument_key(&own.instrument());
        let row = table.iter().position(|row| crate::read::instrument_key(&row.instrument()) == key && row.tower_height.to_bits() == own.tower_height.to_bits() && row.cycle.to_bits() == own.cycle.to_bits());
        let row = row.unwrap_or_else(|| {
            table.push(own.clone());
            table.len() - 1
        });
        column.push(row);
    }
    (table, column)
}
/// The numerics block: what made the numbers, so that they can be made again or doubted.
#[must_use]
pub fn numerics(spec: &SceneSpec, build: &Build) -> serde_json::Value {
    let instrument = spec.array.instrument.instrument();
    serde_json::json!({
        "engine": build.json(),
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

fn manifest_of(
    spec: &SceneSpec,
    signal: &Signal,
    build: &Build,
    listed: &[Listed],
    comparison: Option<String>,
) -> serde_json::Value {
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
    let words = |sign: SignSpec| match sign {
        SignSpec::LowerMinusUpper => {
            "lower minus upper (δφ₁ − δφ₂): George's convention. v1's bundles are upper minus lower: the opposite sign"
        }
        SignSpec::UpperMinusLower => {
            "upper minus lower (δφ₂ − δφ₁): v1's convention, set by this scene's instrument"
        }
    };
    let (table, column) = instrument_table(spec);
    // One sign for the whole array when every tower has it; else each tower's in its row.
    let signs: Vec<SignSpec> = column.iter().map(|row| table[*row].sign).collect();
    let sign = match signs.first() {
        Some(first) if signs.iter().any(|s| s != first) => {
            "each tower's own: see `instruments` and `tower_instrument`".to_owned()
        }
        Some(first) => words(*first).to_owned(),
        None => words(spec.array.instrument.sign).to_owned(),
    };
    serde_json::json!({
        "cavendish_bundle": BUNDLE_VERSION,
        "scene": spec.name,
        "shots": signal.most(),
        "shots_per_tower": (0..signal.towers).map(|d| signal.count(d)).collect::<Vec<_>>(),
        "ragged": "each tower has its own shots: the (T_max, D) arrays are padded where a tower has fewer \
                   (`mask` false: `detector_time` NaN, the values 0, `valid` false), and the `_flat` arrays hold \
                   every tower's shots one after another, tower d's at tower_offsets[d]..tower_offsets[d + 1]. \
                   NaN appears only where `mask` is false",
        "instruments": table,
        "tower_instrument": column,
        "towers": signal.towers,
        "sources": spec.bodies.len(),
        "sign": sign,
        "settled": signal.settled,
        "warnings": signal.warnings.iter().map(|warning| warning.text.clone()).collect::<Vec<_>>(),
        "comparison": comparison,
        "numerics": numerics(spec, build),
        "arrays": arrays,
        "spec": spec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dev, read};
    use cav_core::forward::Channel;
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
            let files = write(&dir, &scene, &signal, &build(), None).unwrap();
            // S2's eleven, and since S3 the mask, the counts and the four flat arrays with their offsets.
            assert_eq!(files.len(), 18);
            let (shots, towers) = (signal.times(0).len(), 5);
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
            check("time", &[shots], signal.times(0).to_vec());
            check("detector_time", &[shots, towers], (0..shots).flat_map(|k| (0..towers).map(move |d| (d, k))).map(|(d, k)| signal.readout(d, k)).collect());
            check("signal", &[shots, towers], by_shot(&signal.gradiometer));
            let per_ifo: Vec<f64> = (0..shots)
                .flat_map(|k| (0..towers).map(move |d| (d, k)))
                .flat_map(|(d, k)| [signal.lower[d * shots + k], signal.upper[d * shots + k]])
                .collect();
            check("signal_per_ifo", &[shots, towers, 2], per_ifo);
            check("error_estimate", &[towers], signal.estimates.clone());
            // Every tower has every shot: the flat arrays are the signal as the engine holds it.
            check("n_shots", &[towers], vec![shots as f64; towers]);
            check("tower_offsets", &[towers + 1], (0..=towers).map(|d| (d * shots) as f64).collect());
            check("phase_flat", &[towers * shots], signal.gradiometer.clone());
            check("t_flat", &[towers * shots], (0..towers).flat_map(|d| signal.readouts(d)).collect());
            let (shape, mask) = array(&dir, "mask").into_bool().unwrap();
            let masked = usize::from(shape != [shots, towers]) + mask.iter().filter(|m| !**m).count();
            check("source_mass", &[2], vec![scene.bodies[0].mass, 3.0]);
            let placement: Vec<f64> = scene.array.towers.iter().flat_map(|t| [t.position[0], t.position[1], t.position[2], 1.0, 0.0, 0.0, 0.0]).collect();
            check("detector_placement", &[towers, 7], placement);
            let positions: Vec<f64> = scene.bodies.iter().flat_map(|body| signal.times(0).iter().flat_map(|t| body.motion.position(*t, 9.81))).collect();
            check("source_position", &[2, shots, 3], positions);
            wrong += masked;
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
        let shots = signal.times(0).len();
        let dir = scratch("not a number");
        // One value that is not a number, in the upper interferometer of tower 3 at shot 5.
        signal.upper[2 * shots + 4] = f64::NAN;
        let why = write(&dir, &scene, &signal, &build(), None).unwrap_err();
        assert!(why.contains("δφ₂ of tower 3 at shot 5") && why.contains("is not a number"), "{why}");
        assert!(!dir.exists(), "nothing was written, not even the directory");
        // An infinity is exact and is allowed: the half-space's own channels.
        let floor = dev::infinite_floor();
        let infinite = read::signal(&floor).unwrap();
        let dir = scratch("half space");
        write(&dir, &floor, &infinite, &build(), None).unwrap();
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
        assert!(write(&dir, &fewer, &other, &build(), None).unwrap_err().contains("not this scene's signal"));
        signal.upper[2 * shots + 4] = 0.0;
        assert!(write(&scratch("whole"), &scene, &signal, &build(), None).is_ok());
    }

    #[test]
    fn the_manifest_says_what_made_it() {
        let scene = scene();
        let signal = read::signal(&scene).unwrap();
        let dir = scratch("manifest");
        write(&dir, &scene, &signal, &build(), None).unwrap();
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
        // With what the two are: the build's, not the tree's at the time of writing.
        assert_eq!(
            numerics["engine"],
            serde_json::json!({"version": "0.0.0", "revision": "0123abc", "dirty": false, "built_from": BUILT_FROM})
        );
        assert!(BUILT_FROM.contains("fixed at compile time") && BUILT_FROM.contains("git is not asked"));
        // A build that does not know says so in a word, not with a null; a dirty one says true.
        let unknown = Build { version: "0.0.0".into(), revision: "unknown".into(), dirty: None };
        assert_eq!(
            (&unknown.json()["revision"], &unknown.json()["dirty"]),
            (&"unknown".into(), &"unknown".into())
        );
        assert_eq!(Build { dirty: Some(true), ..build() }.json()["dirty"], true);
        assert_eq!(numerics["precision"], "f64 on the CPU");
        assert_eq!(numerics["phase_tolerance_rad"], 1e-10);
        assert!(numerics["quadrature"].as_str().unwrap().contains("Gauss"));
        assert_eq!(numerics["constants"]["atom_mass_kg"], cav_core::constants::SR87_MASS);
        assert!(numerics["replay"].as_str().unwrap().contains("same platform"));
        // Every array is listed with its shape, dtype and unit, and the file is there.
        let arrays = manifest["arrays"].as_array().unwrap();
        assert_eq!(arrays.len(), 16);
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
        // No comparison was on: the manifest says so with a null. With one on it says, in
        // words, that the bundle is of the scene's own instrument and what is not in it.
        assert!(manifest["comparison"].is_null());
        write(&dir, &scene, &signal, &build(), Some("AICE")).unwrap();
        let compared = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
        assert!(compared.contains("of the scene's own instruments alone") && compared.contains("on AICE"));
        // Control: v1's sign is said as v1's when the scene asks for it.
        let mut flipped = scene.clone();
        flipped.array.instrument.sign = SignSpec::UpperMinusLower;
        let signal = read::signal(&flipped).unwrap();
        write(&dir, &flipped, &signal, &build(), None).unwrap();
        let again = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
        assert!(again.contains("upper minus lower (δφ₂ − δφ₁): v1's convention"));
    }

    #[test]
    fn a_ragged_bundle_round_trips() {
        // AION-10 and AICE in one array: 2 s against 5 s cycles, 1.46 s against 5 s flights.
        let mut scene = dev::mixed_array();
        scene.duration = 41.0;
        let signal = read::signal(&scene).unwrap();
        let counts: Vec<usize> = (0..signal.towers).map(|d| signal.count(d)).collect();
        assert!(counts.iter().any(|c| *c != counts[0]), "the towers have different numbers of shots: {counts:?}");
        let (most, towers, total) = (signal.most(), signal.towers, signal.clock.len());
        let dir = scratch("ragged");
        write(&dir, &scene, &signal, &build(), None).unwrap();
        let f64s = |name: &str| array(&dir, name).into_f64().unwrap();
        let bools = |name: &str| array(&dir, name).into_bool().unwrap();
        let mut wrong = 0usize;
        // The flat arrays are the engine's, bit for bit, with their offsets.
        let (shape, offsets) = f64s("tower_offsets");
        wrong += usize::from(shape != [towers + 1]);
        wrong += offsets.iter().zip(&signal.starts).filter(|(a, b)| **a != **b as f64).count();
        let (shape, flat) = f64s("phase_flat");
        wrong += usize::from(shape != [total]);
        wrong += flat.iter().zip(&signal.gradiometer).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
        let (_, t) = f64s("t_flat");
        let readouts: Vec<f64> = (0..towers).flat_map(|d| signal.readouts(d)).collect();
        wrong += t.iter().zip(&readouts).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
        let (_, pair) = f64s("phase_per_ifo_flat");
        wrong += (0..total).filter(|n| pair[2 * n].to_bits() != signal.lower[*n].to_bits()).count();
        // The padded view: each tower's values in its column, the padding masked, NaN in the
        // readout times and nowhere else, zeros in the values, and the counts.
        let (shape, padded) = f64s("signal");
        wrong += usize::from(shape != [most, towers]);
        let (_, times) = f64s("detector_time");
        let (_, mask) = bools("mask");
        let (_, valid) = bools("valid");
        let (_, n) = f64s("n_shots");
        for d in 0..towers {
            wrong += usize::from(n[d] != counts[d] as f64);
            for k in 0..most {
                let at = k * towers + d;
                if k < counts[d] {
                    wrong += usize::from(!mask[at] || padded[at].to_bits() != signal.channel(d, Channel::Gradiometer)[k].to_bits());
                    wrong += usize::from(times[at].to_bits() != signal.readout(d, k).to_bits() || valid[at] != signal.taken(d)[k]);
                } else {
                    wrong += usize::from(mask[at] || valid[at] || padded[at].to_bits() != 0 || !times[at].is_nan());
                }
            }
        }
        assert_eq!(wrong, 0);
        assert!(mask.iter().any(|m| !m), "the control: there is padding to check");
        // The instruments are listed once each, and each tower names its row.
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
        let rows = manifest["instruments"].as_array().unwrap();
        let column: Vec<usize> =
            manifest["tower_instrument"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
        assert_eq!(column.len(), towers);
        for (d, row) in column.iter().enumerate() {
            assert_eq!(rows[*row]["half_sequence"], scene.array.instrument_of(d).half_sequence);
        }
        assert!(rows.len() >= 2 && column.iter().any(|r| *r != column[0]));
        assert!(manifest["ragged"].as_str().unwrap().contains("NaN appears only where `mask` is false"));
        // Control: an even array is not ragged: no padding at all.
        let even = read::signal(&dev::comb()).unwrap();
        assert!((0..even.towers).all(|d| even.count(d) == even.most()));
    }

    #[test]
    fn a_bundle_of_no_towers_is_written() {
        let mut scene = dev::no_towers();
        scene.duration = 10.0;
        let signal = read::signal(&scene).unwrap();
        let dir = scratch("no towers");
        let files = write(&dir, &scene, &signal, &build(), None).unwrap();
        assert_eq!(files.len(), 18);
        let (shape, _) = array(&dir, "signal").into_f64().unwrap();
        assert_eq!(shape, [0, 0]);
        let (shape, offsets) = array(&dir, "tower_offsets").into_f64().unwrap();
        assert_eq!((shape, offsets), (vec![1], vec![0.0]));
        // The tracks are still on the array's clock: the bodies move whether or not anyone reads them.
        let (shape, _) = array(&dir, "source_position").into_f64().unwrap();
        assert_eq!(shape[0], scene.bodies.len());
        assert!(shape[1] > 0);
    }}
