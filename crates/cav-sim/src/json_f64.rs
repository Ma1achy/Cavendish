//! Floats that are not finite, through JSON.
//!
//! JSON has no word for infinity or for "not a number": `serde_json` writes both as `null` and
//! then refuses to read `null` back as a float. A scene never holds one (`validate` refuses them),
//! but an answer may: over a half-space each interferometer's own phase is infinite, and a read
//! that failed is not a number. Fields that may hold such a value are written through this
//! module: a finite value as a JSON number (exact: the workspace builds `serde_json` with
//! `float_roundtrip`), `+∞` and `−∞` as the strings `"inf"` and `"-inf"`, and NaN as `"nan"`.
//!
//! What is kept: every bit of a finite value (negative zero and subnormals included) and the
//! sign of an infinity. What is not: a NaN's sign and payload, which Rust does not promise to be
//! the same from one run to the next either; a bundle refuses NaN for that reason.
//!
//! Use: `#[serde(with = "cav_sim::json_f64")]` on an `f64`, `json_f64::list` on a `Vec<f64>`, and
//! `#[schemars(schema_with = "cav_sim::json_f64::schema")]` (or `list_schema`) beside it.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A float as it travels: a number, or one of three words.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Wire {
    Number(f64),
    Word(String),
}

fn to_wire(value: f64) -> Wire {
    if value.is_finite() {
        Wire::Number(value)
    } else if value.is_nan() {
        Wire::Word("nan".into())
    } else if value > 0.0 {
        Wire::Word("inf".into())
    } else {
        Wire::Word("-inf".into())
    }
}

fn from_wire<E: serde::de::Error>(wire: Wire) -> Result<f64, E> {
    match wire {
        Wire::Number(value) => Ok(value),
        Wire::Word(word) => match word.as_str() {
            "inf" => Ok(f64::INFINITY),
            "-inf" => Ok(f64::NEG_INFINITY),
            "nan" => Ok(f64::NAN),
            other => Err(E::custom(format!("`{other}` is not a number, `inf`, `-inf` or `nan`"))),
        },
    }
}

/// Writes one float (see the module's note).
///
/// # Errors
/// The serialiser's own.
pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
    to_wire(*value).serialize(serializer)
}

/// Reads one float written by [`serialize`].
///
/// # Errors
/// A string that is not one of the three words, or anything that is not a number.
pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    from_wire(Wire::deserialize(deserializer)?)
}

/// The JSON Schema of a float written by this module: a number or one of the three words.
#[must_use]
pub fn schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "description": "A number; or \"inf\", \"-inf\" or \"nan\", which JSON has no number for.",
        "anyOf": [{ "type": "number" }, { "enum": ["inf", "-inf", "nan"] }]
    })
}

/// The JSON Schema of a list of such floats.
#[must_use]
pub fn list_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": "array", "items": schema(generator) })
}

/// The same for a `Vec<f64>`: `#[serde(with = "cav_sim::json_f64::list")]`.
pub mod list {
    use super::{Deserialize, Deserializer, Serializer, Wire, from_wire, to_wire};

    /// Writes a list of floats, each as [`super::serialize`] writes one.
    ///
    /// # Errors
    /// The serialiser's own.
    pub fn serialize<S: Serializer>(values: &[f64], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(values.iter().map(|value| to_wire(*value)))
    }

    /// Reads a list written by [`serialize`].
    ///
    /// # Errors
    /// As [`super::deserialize`], for any item.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<f64>, D::Error> {
        Vec::<Wire>::deserialize(deserializer)?.into_iter().map(from_wire).collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{FloorKind, FloorSpec, SceneSpec, dev};
    use cav_validate::Measured;

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct Answer {
        #[serde(with = "super")]
        one: f64,
        #[serde(with = "super::list")]
        many: Vec<f64>,
    }

    /// SplitMix64: the bit patterns below are the same on every run.
    fn patterns(count: usize, seed: u64) -> Vec<f64> {
        let mut state = seed;
        let mut out = Vec::with_capacity(count + 8);
        while out.len() < count {
            state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            let value = f64::from_bits(z ^ (z >> 31));
            // Every exponent, both signs, subnormals among them; the scene's own floats are finite.
            if value.is_finite() {
                out.push(value);
            }
        }
        out.extend([-0.0, 0.0, 5e-324, -5e-324, f64::MAX, f64::MIN, f64::MIN_POSITIVE, 0.1 + 0.2]);
        out
    }

    fn differing(a: &[f64], b: &[f64]) -> f64 {
        if a.len() != b.len() {
            return f64::INFINITY;
        }
        a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count() as f64
    }

    /// A number in `{:e}` form read the quick way: its digits as an integer, times a power of
    /// ten, in two multiplications when the power is out of an f64's range. Correct to within a
    /// few units in the last place, which is not the same number.
    fn two_step(text: &str) -> f64 {
        let (mantissa, exponent) = text.split_once('e').expect("an exponent");
        let exponent: i32 = exponent.parse().expect("a whole exponent");
        let negative = mantissa.starts_with('-');
        let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
        let places = mantissa.split_once('.').map_or(0, |(_, after)| after.len()) as i32;
        let whole = digits.parse::<u64>().expect("at most seventeen digits") as f64;
        let power = exponent - places;
        let value = if power < -300 {
            whole * 10f64.powi(-300) * 10f64.powi(power + 300)
        } else {
            whole * 10f64.powi(power)
        };
        if negative { -value } else { value }
    }

    cav_validate::case! {
        test: validation_json_round_trip_is_exact,
        name: "spec.json_f64_exact",
        subsystem: "spec",
        slice: "S2",
        reference: Exact,
        against: "10^6 random finite f64 bit patterns and the awkward values (negative zero, subnormals, the largest), written as JSON and read back, as text and through serde_json::Value; values whose bits differ",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let values = patterns(1_000_000, 7);
            let text = serde_json::to_string(&values).unwrap();
            let typed: Vec<f64> = serde_json::from_str(&text).unwrap();
            let through: Vec<f64> = serde_json::from_value(serde_json::to_value(&values).unwrap()).unwrap();
            // Control: the same values written with fifteen digits after the point, which looks
            // exact and is not, come back changed by the thousand.
            let lossy: Vec<f64> = values[..10_000].iter().map(|x| format!("{x:.15e}").parse().unwrap()).collect();
            let lost = differing(&values[..10_000], &lossy);
            assert!(lost > 1000.0, "fifteen digits lost only {lost} of 10000");
            // Control: the right digits read the quick way (the digits as an integer, times a
            // power of ten in one or two multiplications, which is what serde_json does without
            // `float_roundtrip`) do not all come back: the feature is what makes the count 0.
            let quick: Vec<f64> = values[..10_000].iter().map(|x| two_step(&format!("{x:e}"))).collect();
            let lost = differing(&values[..10_000], &quick);
            assert!(lost > 100.0, "the quick reading lost only {lost} of 10000");
            Measured::Error(differing(&values, &typed) + differing(&values, &through))
        },
    }

    cav_validate::case! {
        test: validation_ron_round_trip_is_exact,
        name: "spec.ron_f64_exact",
        subsystem: "spec",
        slice: "S2",
        reference: Exact,
        against: "10^6 random finite f64 bit patterns and the awkward values, written as RON and read back; values whose bits differ",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let values = patterns(1_000_000, 11);
            let text = ron::to_string(&values).unwrap();
            let back: Vec<f64> = ron::from_str(&text).unwrap();
            // Control: the same values as RON text with fifteen digits after the point, which
            // looks exact and is not, come back changed by the thousand (S2's review, 37: this
            // case had no control that text can lose a value).
            let short: Vec<String> = values[..10_000].iter().map(|x| format!("{x:.15e}")).collect();
            let lossy: Vec<f64> = ron::from_str(&format!("[{}]", short.join(","))).unwrap();
            let lost = differing(&values[..10_000], &lossy);
            assert!(lost > 1000.0, "fifteen digits lost only {lost} of 10000");
            // Control: one unit in the last place of one value is seen.
            let mut nudged = back.clone();
            nudged[123_456] = f64::from_bits(nudged[123_456].to_bits() ^ 1);
            assert_eq!(differing(&values, &nudged), 1.0);
            Measured::Error(differing(&values, &back))
        },
    }

    #[test]
    fn non_finite_numbers_survive_json() {
        let answer = Answer {
            one: f64::NEG_INFINITY,
            many: vec![f64::INFINITY, -0.0, 5e-324, 1.5, f64::NAN, f64::NEG_INFINITY],
        };
        let text = serde_json::to_string(&answer).unwrap();
        assert_eq!(text, r#"{"one":"-inf","many":["inf",-0.0,5e-324,1.5,"nan","-inf"]}"#);
        let back: Answer = serde_json::from_str(&text).unwrap();
        assert_eq!(back.one, f64::NEG_INFINITY);
        assert_eq!(back.many[0], f64::INFINITY);
        assert_eq!(back.many[1].to_bits(), (-0.0f64).to_bits(), "negative zero keeps its sign");
        assert_eq!(back.many[2].to_bits(), 5e-324f64.to_bits());
        assert!(back.many[4].is_nan());
        assert_eq!(back.many[5], f64::NEG_INFINITY);
        // Through a `Value` as well, which is what MCP sends.
        let value = serde_json::to_value(&answer).unwrap();
        let again: Answer = serde_json::from_value(value).unwrap();
        assert_eq!(again.many[0], f64::INFINITY);
        // Control: without this module the same values cannot make the trip. Infinity is
        // written as `null`, and `null` is not a float.
        let bare = serde_json::to_string(&[f64::INFINITY]).unwrap();
        assert_eq!(bare, "[null]");
        assert!(serde_json::from_str::<Vec<f64>>(&bare).is_err());
        // A word that is not one of the three is refused, with the word.
        let wrong = serde_json::from_str::<Answer>(r#"{"one":"big","many":[]}"#).unwrap_err().to_string();
        assert!(wrong.contains("`big` is not a number"), "{wrong}");
    }

    #[test]
    fn awkward_numbers_in_a_file_are_read_or_refused_as_stated() {
        let good = dev::point().to_ron();
        let with = |from: &str, to: &str| {
            let text = good.replacen(from, to, 1);
            assert_ne!(text, good, "the edit `{from}` → `{to}` changed nothing");
            SceneSpec::from_ron(&text)
        };
        // A whole number, an exponent and an underscore are floats.
        assert_eq!(with("duration: 12.0", "duration: 12").unwrap().duration, 12.0);
        assert_eq!(with("duration: 12.0", "duration: 1.2e1").unwrap().duration, 12.0);
        assert_eq!(with("duration: 12.0", "duration: 1_2.0").unwrap().duration, 12.0);
        // Negative zero and a subnormal are kept to the bit, where a coordinate may be either.
        let spec = with("position: (0.0, 0.0, 0.0)", "position: (-0.0, 5e-324, 0.0)").unwrap();
        assert_eq!(spec.array.towers[0].position[0].to_bits(), (-0.0f64).to_bits());
        assert_eq!(spec.array.towers[0].position[1].to_bits(), 5e-324f64.to_bits());
        let again = SceneSpec::from_ron(&spec.to_ron()).unwrap();
        assert_eq!(again.array.towers[0].position[0].to_bits(), (-0.0f64).to_bits());
        let json = SceneSpec::from_json(&spec.to_json()).unwrap();
        assert_eq!(json.array.towers[0].position[0].to_bits(), (-0.0f64).to_bits());
        assert_eq!(json.array.towers[0].position[1].to_bits(), 5e-324f64.to_bits());
        // What RON reads as infinity or as not a number never becomes a scene: each is refused
        // by name. (RON takes `1e400` as `inf` in silence; JSON refuses it itself.)
        for (text, field) in [
            ("duration: 1e400", "duration"),
            ("duration: inf", "duration"),
            ("duration: NaN", "duration"),
            ("duration: -0.0", "duration"),
        ] {
            let why = with("duration: 12.0", text).unwrap_err().0;
            assert!(why.contains(field), "`{text}`: {why}");
        }
        let why = with("position: (0.0, 0.0, 0.0)", "position: (inf, 0.0, 0.0)").unwrap_err().0;
        assert!(why.contains("array.towers[0].position"), "{why}");
        let json = dev::point().to_json().replacen("12.0", "1e999", 1);
        assert!(SceneSpec::from_json(&json).is_err());
        // Control: the unedited file loads.
        assert_eq!(SceneSpec::from_ron(&good).unwrap(), dev::point());
    }

    #[test]
    fn a_saved_scene_loads_to_the_same_scene() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tmp/saved scenes");
        std::fs::create_dir_all(&dir).unwrap();
        let mut awkward = dev::oscillator_zoo();
        awkward.name = "awkward".into();
        awkward.duration = 0.1 + 0.2;
        awkward.bodies[0].mass = 1.0 / 3.0;
        awkward.array.towers[2].position = [std::f64::consts::PI, -0.0, 5e-324];
        awkward.floor =
            Some(FloorSpec { height: -1e-17, kind: FloorKind::Slab { thickness: 0.7 }, density: 2.4e3 });
        let scenes: Vec<SceneSpec> = crate::LIBRARY
            .iter()
            .map(|scene| SceneSpec::from_ron(scene.ron).unwrap())
            .chain([awkward])
            .collect();
        for scene in &scenes {
            let path = dir.join(format!("{}.ron", scene.name));
            scene.save(path.to_str().unwrap()).unwrap();
            let loaded = crate::load(path.to_str().unwrap()).unwrap();
            assert_eq!(&loaded, scene, "{}", scene.name);
            // Equality of floats does not tell 0.0 from −0.0; the text does.
            let text = std::fs::read_to_string(&path).unwrap();
            assert_eq!(text, scene.to_ron(), "{}: the file is the scene's RON", scene.name);
            assert_eq!(loaded.to_ron(), text, "{}: text → scene → text is the same text", scene.name);
        }
        // Saving over a file replaces it whole; a scene that is not valid is not written at all.
        let path = dir.join("room.ron");
        let mut broken = SceneSpec::room();
        broken.duration = -1.0;
        let why = broken.save(path.to_str().unwrap()).unwrap_err().0;
        assert!(why.starts_with("duration"), "{why}");
        assert_eq!(crate::load(path.to_str().unwrap()).unwrap(), SceneSpec::room(), "the file is untouched");
        // Control: a file that differs in one digit loads to a scene that differs.
        let text = std::fs::read_to_string(&path).unwrap().replacen("duration: 12.0", "duration: 12.5", 1);
        std::fs::write(&path, text).unwrap();
        assert_ne!(crate::load(path.to_str().unwrap()).unwrap(), SceneSpec::room());
        // A directory that is not there is a refusal with the path, not a panic.
        let nowhere = dir.join("no such folder/x.ron");
        assert!(SceneSpec::room().save(nowhere.to_str().unwrap()).unwrap_err().0.contains("no such folder"));
    }
}
