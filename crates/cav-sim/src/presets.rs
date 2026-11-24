//! Instrument presets: AION-10, AICE 140 m, and any instrument of one's own kept in a file.
//!
//! A preset is an [`InstrumentSpec`] and nothing more: applying one is a `set` of
//! `array.instrument`, so it is checked, journalled and undone like any other edit.
//!
//! | Preset | Tower | Δr | T | Launch u₀ | Cycle | Kick number n | From |
//! |---|---|---|---|---|---|---|---|
//! | `aion10` | 10.1 m | 5 m | 0.73 s | 3.86 m/s | 2 s | 1000 | George (21 and 23 Nov 2026) |
//! | `aice` | 100 m | 70 m | 2.5 s | derived: `g·T − v_kick/2` | 5 s | 1000, a parameter | George (23 Nov 2026) |
//!
//! AICE's separation, T and cycle are fixed (George, 23 Nov 2026). Its launch speed is not a
//! number that is kept: it is derived, when the preset is built, from the instrument's own
//! gravity, T, atom and kick number, by the rule of George's `fig2_final.py`
//! (`u0 = g * T_fly / 2 - v_kick / 2`): the launch that brings each arm back to the height it
//! started from exactly at 2T. With ⁸⁷Sr and the exact ħ that is 21.236 m/s; the plan's 21.27 is
//! the same rule with the scripts' rounded constants. AICE would support a larger kick number
//! than AION-10's thousand: [`aice_with_kicks`] builds the preset at another n, and in a scene
//! `set array.instrument.kicks <n>` followed by `launch symmetric` does the same.

use crate::{InstrumentSpec, SpecError};

/// The built-in presets: the name used on the command line, what it is called, and the instrument.
#[must_use]
pub fn built_in() -> Vec<(&'static str, &'static str, InstrumentSpec)> {
    vec![("aion10", "AION-10", InstrumentSpec::default()), ("aice", "AICE 140 m", aice())]
}

/// The AICE instrument: a 140 m tower (George, 24 Nov 2026: the baseline is over 140 m, as his
/// `fig2_final.py` has it, `L_base = 140.0`), interferometers launched from its base and 70 m
/// above it (`delta_r`), T = 2.5 s, a 5 s cycle, AION-10's kick number (1000), and the launch
/// derived from those by [`symmetric_launch`]. Every arm stays inside the tower: the highest
/// reaches 108.9 m. (It was a 100 m tower until 24 Nov 2026, which the arms did not fit.)
#[must_use]
pub fn aice() -> InstrumentSpec {
    aice_with_kicks(1000)
}

/// The AICE 140 m instrument with kick number `kicks` (a whole number: how many photon momenta a
/// kick gives): [`aice`] is this at 1000. The launch velocity is derived for that kick number,
/// so the arms still finish where they started. With ⁸⁷Sr the rule asks for a launch of zero at
/// n ≈ 7457 and a downward one beyond: such an instrument is refused when it is set into a
/// scene, like any other that cannot exist.
#[must_use]
pub fn aice_with_kicks(kicks: u32) -> InstrumentSpec {
    let mut spec = InstrumentSpec {
        tower_height: 140.0,
        ifo_separation: 70.0,
        half_sequence: 2.5,
        cycle: 5.0,
        kicks,
        ..InstrumentSpec::default()
    };
    spec.launch_velocity = symmetric_launch(&spec);
    spec
}

/// The launch velocity that brings each arm of an instrument back to the height it was launched
/// from exactly at 2T, metres per second: `g·T − v_kick/2`, with the instrument's own gravity,
/// T, atom and kick number. It is the rule of George's `fig2_final.py`. Derived each time it is
/// asked for: after a change of any of those four, ask again (`launch symmetric`).
#[must_use]
pub fn symmetric_launch(spec: &InstrumentSpec) -> f64 {
    spec.instrument().symmetric_launch()
}

/// The largest preset file [`load`] reads, bytes.
pub const MAX_PRESET_FILE_BYTES: u64 = 64 << 10;

/// A preset by name (`aion10`, `aice`; `aion-10`, `aice-140m` and the older `aice-100m` are understood too), or else an
/// instrument of one's own from a `.ron` file of an `InstrumentSpec`.
///
/// # Errors
/// When the name is neither a preset nor a readable file of an instrument; the message lists the
/// presets. The instrument is checked when it is set into a scene, not here.
pub fn load(name_or_path: &str) -> Result<InstrumentSpec, SpecError> {
    let plain = name_or_path.to_lowercase().replace(['-', '_', ' '], "");
    if let Some((_, _, spec)) = built_in().into_iter().find(|(name, _, _)| {
        *name == plain || [format!("{name}100m"), format!("{name}140m")].contains(&plain)
    }) {
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

    /// Whether both interferometers' both arms stay between the tower's base and top: at the
    /// three pulses (0, T, 2T), at each arm's own highest point, and at 1000 instants between.
    fn arms_inside(spec: &InstrumentSpec) -> bool {
        let instrument = spec.instrument();
        let (two_t, arms) = (instrument.flight_time(), instrument.arms());
        let mut instants: Vec<f64> = vec![0.0, instrument.half_sequence, two_t];
        instants.extend((0..=1000).map(|k| two_t * f64::from(k) / 1000.0));
        let highest = instrument.ifo_separation + arms.iter().map(|a| a.apex()).fold(0.0, f64::max);
        highest <= spec.tower_height
            && [0.0, instrument.ifo_separation].iter().all(|launch| {
                arms.iter().all(|arm| {
                    instants.iter().all(|&tau| {
                        let z = launch + arm.rise(tau);
                        (0.0..=spec.tower_height).contains(&z)
                    })
                })
            })
    }

    #[test]
    fn presets_are_the_plans() {
        // AION-10: George's numbers, which are the spec's defaults.
        let aion = load("aion10").unwrap();
        assert_eq!(aion, InstrumentSpec::default());
        assert_eq!((aion.tower_height, aion.ifo_separation, aion.half_sequence), (10.1, 5.0, 0.73));
        assert_eq!((aion.launch_velocity, aion.cycle, aion.kicks), (3.86, 2.0, 1000));
        // AICE: the plan's table, and the launch rule u₀ = gT − v_kick/2 with the engine's atom.
        let aice = load("aice").unwrap();
        assert_eq!(
            (aice.tower_height, aice.ifo_separation, aice.half_sequence, aice.cycle, aice.kicks),
            (140.0, 70.0, 2.5, 5.0, 1000)
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
        // Every arm of every preset inside its tower at every pulse and between (George, 24 Nov
        // 2026: AICE's baseline is over 140 m, as `fig2_final.py` has it): above the base, and
        // below the top at the highest point of any arm.
        for (name, _, spec) in built_in() {
            assert!(arms_inside(&spec), "{name}: an arm leaves the tower");
        }
        let reach = instrument.reach();
        assert!((reach - 108.88).abs() < 0.01, "{reach}");
        // Control: in the 100 m the plan once gave it, the upper arm leaves the tower.
        assert!(!arms_inside(&InstrumentSpec { tower_height: 100.0, ..aice.clone() }));
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
    fn the_aice_launch_is_derived_and_the_atoms_finish_where_they_started() {
        use cav_core::forward::Instrument;
        // Each arm's height above its launch point at 2T, metres, with the floor off so that an
        // arm that would end below is seen to: `[lower arm, upper arm]`.
        let ends = |spec: &InstrumentSpec| {
            let instrument = Instrument { floor: false, ..spec.instrument() };
            instrument.arms().map(|arm| arm.rise(instrument.flight_time()))
        };
        let closes = |spec: &InstrumentSpec| ends(spec).iter().all(|end| end.abs() < 1e-9);
        // "The atoms finish at the height they started": each arm does, not only their mean.
        let aice = aice();
        assert_eq!(aice.launch_velocity.to_bits(), aice.instrument().symmetric_launch().to_bits());
        assert!(closes(&aice), "{:?}", ends(&aice));
        // Control: the plan's 21.27 m/s with ⁸⁷Sr ends both arms 17 cm above where they started,
        // 2T·(21.27 − u₀).
        let typed = InstrumentSpec { launch_velocity: 21.27, ..aice.clone() };
        let over = 5.0 * (21.27 - aice.launch_velocity);
        assert!(ends(&typed).iter().all(|end| (end - over).abs() < 1e-9), "{:?} {over}", ends(&typed));
        assert!(over > 0.16 && over < 0.18 && !closes(&typed), "{over}");
        // Nothing is kept: change T, g, the atom or the kick number and the rule is asked again.
        // Each change alone leaves the old launch wrong (the control) and the new one right.
        type Change = fn(&mut InstrumentSpec);
        let changes: [(&str, Change); 4] = [
            ("T", |spec| spec.half_sequence = 2.0),
            ("g", |spec| spec.gravity = 9.78),
            ("the atom's mass", |spec| spec.atom_mass = cav_core::constants::reference::ATOM_MASS),
            ("the kick number", |spec| spec.kicks = 4000),
        ];
        for (what, change) in changes {
            let mut changed = aice.clone();
            change(&mut changed);
            assert!(!closes(&changed), "{what}: the old launch still closes the arms");
            changed.launch_velocity = symmetric_launch(&changed);
            assert!(closes(&changed), "{what}: {:?}", ends(&changed));
            assert_ne!(changed.launch_velocity, aice.launch_velocity, "{what}");
        }
        // The kick number is a parameter of the preset: at another n it is the same instrument
        // with the launch derived for that n, and a valid scene.
        let strong = aice_with_kicks(4000);
        assert_eq!(
            strong,
            InstrumentSpec { kicks: 4000, launch_velocity: strong.launch_velocity, ..aice.clone() }
        );
        let kick = strong.instrument().kick_velocity();
        assert_eq!(kick, 4.0 * aice.instrument().kick_velocity());
        assert_eq!(strong.launch_velocity, 9.81 * 2.5 - 0.5 * kick);
        assert!(closes(&strong), "{:?}", ends(&strong));
        let scene = |instrument: InstrumentSpec| SceneSpec {
            array: crate::ArraySpec { instrument, ..crate::ArraySpec::default() },
            ..SceneSpec::room()
        };
        assert_eq!(scene(strong).validate(), Ok(()));
        assert_eq!(aice_with_kicks(1000), aice);
        // Control: past the n at which the rule asks for a launch of zero there is no such
        // instrument, and the scene says so.
        let why = scene(aice_with_kicks(8000)).validate().unwrap_err().0;
        assert!(why.starts_with("array.instrument.launch_velocity"), "{why}");
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
