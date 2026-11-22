//! Instrument presets: AION-10, AICE 100 m, and any instrument of one's own kept in a file.
//!
//! A preset is an [`InstrumentSpec`] and nothing more: applying one is a `set` of
//! `array.instrument`, so it is checked, journalled and undone like any other edit.
//!
//! | Preset | Tower | Δr | T | Launch u₀ | Cycle | From |
//! |---|---|---|---|---|---|---|
//! | `aion10` | 10 m | 5 m | 0.73 s | 3.86 m/s | 2 s | George (21 Nov 2026) |
//! | `aice` | 100 m | 70 m | 2.5 s | `g·T − v_kick/2` | 5 s | `plan/VALIDATION-AND-RESEARCH.md` §1 |
//!
//! AICE's launch is the rule, not the plan's rounded 21.27 m/s: that number is the rule with the
//! reference scripts' constants; with ⁸⁷Sr and the exact ħ it is 21.236 m/s. AICE's separation
//! (70 m) and its kick number (1000, as AION-10) are the plan's values and are open questions
//! for George (`plan/PROGRESS.md`, "For George").

use crate::{InstrumentSpec, SpecError};

/// The built-in presets: the name used on the command line, what it is called, and the instrument.
#[must_use]
pub fn built_in() -> Vec<(&'static str, &'static str, InstrumentSpec)> {
    vec![("aion10", "AION-10", InstrumentSpec::default()), ("aice", "AICE 100 m", aice())]
}

/// The AICE 100 m instrument: a 100 m tower, interferometers 70 m apart, T = 2.5 s, a 5 s cycle,
/// and the launch that brings both arms back to the launch height at 2T, `u₀ = g·T − v_kick/2`.
#[must_use]
pub fn aice() -> InstrumentSpec {
    let mut spec = InstrumentSpec {
        tower_height: 100.0,
        ifo_separation: 70.0,
        half_sequence: 2.5,
        cycle: 5.0,
        ..InstrumentSpec::default()
    };
    spec.launch_velocity = symmetric_launch(&spec);
    spec
}

/// The launch velocity that closes an instrument's arms at the launch height at 2T, metres per
/// second: `g·T − v_kick/2`, with the instrument's own gravity, T and atom.
#[must_use]
pub fn symmetric_launch(spec: &InstrumentSpec) -> f64 {
    spec.instrument().symmetric_launch()
}

/// The largest preset file [`load`] reads, bytes.
pub const MAX_PRESET_FILE_BYTES: u64 = 64 << 10;

/// A preset by name (`aion10`, `aice`; `aion-10` and `aice-100m` are understood too), or else an
/// instrument of one's own from a `.ron` file of an `InstrumentSpec`.
///
/// # Errors
/// When the name is neither a preset nor a readable file of an instrument; the message lists the
/// presets. The instrument is checked when it is set into a scene, not here.
pub fn load(name_or_path: &str) -> Result<InstrumentSpec, SpecError> {
    let plain = name_or_path.to_lowercase().replace(['-', '_', ' '], "");
    if let Some((_, _, spec)) =
        built_in().into_iter().find(|(name, _, _)| *name == plain || format!("{name}100m") == plain)
    {
        return Ok(spec);
    }
    let unreadable = |e: std::io::Error| {
        SpecError(format!("`{name_or_path}` is not a preset (aion10, aice) and not a readable file: {e}"))
    };
    let file = std::fs::File::open(name_or_path).map_err(unreadable)?;
    let mut bytes = Vec::with_capacity(MAX_PRESET_FILE_BYTES as usize + 1);
    std::io::Read::read_to_end(&mut std::io::Read::take(file, MAX_PRESET_FILE_BYTES + 1), &mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > MAX_PRESET_FILE_BYTES {
        return Err(SpecError(format!(
            "{name_or_path}: the file is larger than {MAX_PRESET_FILE_BYTES} bytes, the most a preset may be"
        )));
    }
    let text = String::from_utf8(bytes)
        .map_err(|e| unreadable(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    ron::from_str(&text).map_err(|e| SpecError(format!("{name_or_path}: not an instrument (RON): {e}")))
}

/// An instrument as the text of a preset file.
#[must_use]
pub fn to_ron(spec: &InstrumentSpec) -> String {
    let pretty = ron::ser::PrettyConfig::new().struct_names(false);
    ron::ser::to_string_pretty(spec, pretty).expect("an instrument always serialises")
}

/// What a preset is called, if `spec` is one of the built-in presets to the bit.
#[must_use]
pub fn name_of(spec: &InstrumentSpec) -> Option<&'static str> {
    built_in().into_iter().find(|(_, _, preset)| preset == spec).map(|(_, title, _)| title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SceneSpec;

    #[test]
    fn presets_are_the_plans() {
        // AION-10: George's numbers, which are the spec's defaults.
        let aion = load("aion10").unwrap();
        assert_eq!(aion, InstrumentSpec::default());
        assert_eq!((aion.tower_height, aion.ifo_separation, aion.half_sequence), (10.0, 5.0, 0.73));
        assert_eq!((aion.launch_velocity, aion.cycle), (3.86, 2.0));
        // AICE: the plan's table, and the launch rule u₀ = gT − v_kick/2 with the engine's atom.
        let aice = load("aice").unwrap();
        assert_eq!(
            (aice.tower_height, aice.ifo_separation, aice.half_sequence, aice.cycle),
            (100.0, 70.0, 2.5, 5.0)
        );
        let instrument = aice.instrument();
        let rule = 9.81 * 2.5 - instrument.kick_velocity() / 2.0;
        assert_eq!(aice.launch_velocity, rule);
        assert!((rule - 21.236).abs() < 5e-4, "{rule}");
        // With the reference scripts' constants the same rule is the plan's 21.27 m/s.
        let scripts = instrument.with_script_constants();
        assert!((scripts.symmetric_launch() - 21.27).abs() < 5e-3, "{}", scripts.symmetric_launch());
        // The rule does what it says: both arms are back at the launch height at 2T, to rounding.
        let [lower, upper] = cav_core::forward::Instrument { floor: false, ..instrument }.arms();
        assert!(
            lower.rise(5.0).abs() < 1e-12 && upper.rise(5.0).abs() < 1e-12,
            "{} {}",
            lower.rise(5.0),
            upper.rise(5.0)
        );
        // It is the forward model's own AICE in every number the two share.
        let core = cav_core::forward::Instrument::aice();
        assert_eq!(
            (instrument.ifo_separation, instrument.half_sequence, instrument.launch_velocity),
            (core.ifo_separation, core.half_sequence, core.launch_velocity)
        );
        // Each makes a valid scene, is named, and other spellings find it.
        for (name, title, spec) in built_in() {
            let scene = SceneSpec {
                array: crate::ArraySpec { instrument: spec.clone(), ..crate::ArraySpec::default() },
                ..SceneSpec::room()
            };
            assert_eq!(scene.validate(), Ok(()), "{name}");
            assert_eq!(name_of(&spec), Some(title));
        }
        assert_eq!(load("AICE-100m").unwrap(), aice);
        assert_eq!(load("aion-10").unwrap(), aion);
        // Control: AION-10's launch is not the rule (it is George's 3.86 m/s), and an instrument
        // that is nearly a preset is not called one.
        assert!((symmetric_launch(&aion) - 3.86).abs() > 0.01, "{}", symmetric_launch(&aion));
        assert_eq!(name_of(&InstrumentSpec { cycle: 2.5, ..aion }), None);
    }

    #[test]
    fn a_user_preset_is_a_file() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/presets");
        std::fs::create_dir_all(&dir).unwrap();
        let mine = InstrumentSpec {
            tower_height: 30.0,
            ifo_separation: 20.0,
            half_sequence: 1.2,
            cycle: 3.0,
            ..InstrumentSpec::default()
        };
        let path = dir.join("my tower.ron");
        std::fs::write(&path, to_ron(&mine)).unwrap();
        assert_eq!(load(path.to_str().unwrap()).unwrap(), mine);
        // Refusals say what was wrong: not there, not an instrument, too large.
        let why = load("no-such-preset").unwrap_err().0;
        assert!(why.contains("not a preset (aion10, aice)"), "{why}");
        std::fs::write(dir.join("scene.ron"), SceneSpec::room().to_ron()).unwrap();
        assert!(load(dir.join("scene.ron").to_str().unwrap()).unwrap_err().0.contains("not an instrument"));
        std::fs::write(dir.join("big.ron"), vec![b' '; MAX_PRESET_FILE_BYTES as usize + 1]).unwrap();
        assert!(load(dir.join("big.ron").to_str().unwrap()).unwrap_err().0.contains("larger than"));
        // Control: a preset file is the instrument's text and reads back to the bit.
        let odd = InstrumentSpec { launch_velocity: 0.1 + 0.2, ..mine };
        assert_eq!(ron::from_str::<InstrumentSpec>(&to_ron(&odd)).unwrap(), odd);
    }
}
