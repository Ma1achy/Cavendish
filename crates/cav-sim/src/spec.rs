//! The scene spec: everything that can be edited about a scene, as plain data.
//!
//! A scene file is this type in RON; MCP, the CLI and (later) Python speak the same type as JSON, and
//! `scene_schema` is its JSON Schema. Only plain structs and externally tagged enums are used, because
//! RON supports nothing else reliably (research note §4).
//!
//! Frame: metres, x east, y north, z up. Times are seconds.

use cav_core::defaults;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The ranges a spec must stay within. They are wide (a thousand kilometres, a kilometre of tower)
/// and they are finite, so that nothing downstream has to cope with a number chosen to break it:
/// a renderer's loop over a room's metres, a timeline's loop over shots.
pub mod limits {
    /// Every coordinate lies within this distance of the origin, metres.
    pub const POSITION: f64 = 1.0e6;
    /// The longest side of a room, metres.
    pub const ROOM_SIZE: f64 = 1.0e4;
    /// The tallest tower, metres.
    pub const TOWER_HEIGHT: f64 = 1.0e3;
    /// The most towers in an array.
    pub const TOWERS: usize = 1000;
    /// The longest cycle (and half-sequence), seconds.
    pub const CYCLE: f64 = 1.0e4;
    /// The shortest half-sequence, seconds.
    pub const HALF_SEQUENCE_MIN: f64 = 1.0e-3;
    /// The longest timeline, seconds.
    pub const DURATION: f64 = 1.0e6;
    /// The fastest launch, metres per second.
    pub const LAUNCH_VELOCITY: f64 = 1.0e4;
    /// The densest floor, kilograms per cubic metre.
    pub const DENSITY: f64 = 1.0e5;
}

/// The version of the spec format this build reads and writes.
pub const SPEC_VERSION: u32 = 1;

/// A scene: the array, and what surrounds it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneSpec {
    /// Version of the spec format; this build reads version 1.
    pub version: u32,
    /// Short name, as shown in the library and the title bar.
    pub name: String,
    /// One or two sentences on what the scene is and what it checks.
    pub description: String,
    /// Length of the timeline, seconds.
    #[schemars(range(min = 0.0, max = 1.0e6))]
    pub duration: f64,
    /// The towers and the instrument they share.
    pub array: ArraySpec,
    /// The room around the array, or none for an open scene.
    pub room: Option<RoomSpec>,
    /// A floor under the array that extends to the horizon, or none.
    pub floor: Option<FloorSpec>,
}

/// The array of gradiometer towers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArraySpec {
    /// The instrument every tower shares.
    pub instrument: InstrumentSpec,
    /// The towers, in display order; tower 1 is the first. From 1 to 1000 of them.
    pub towers: Vec<TowerSpec>,
}

/// One gradiometer: two atom interferometers stacked in a vertical tower.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstrumentSpec {
    /// Height of the tower, metres.
    #[schemars(range(min = 0.0, max = 1.0e3))]
    pub tower_height: f64,
    /// Vertical separation of the two interferometers (Δr), metres.
    #[schemars(range(min = 0.0, max = 1.0e3))]
    pub ifo_separation: f64,
    /// Half the interrogation sequence, T, seconds; a shot lasts 2T from launch to readout.
    #[schemars(range(min = 1.0e-3, max = 1.0e4))]
    pub half_sequence: f64,
    /// Launch velocity u₀, metres per second.
    #[schemars(range(min = 0.0, max = 1.0e4))]
    pub launch_velocity: f64,
    /// Time between shots of one tower, seconds.
    #[schemars(range(min = 0.0, max = 1.0e4))]
    pub cycle: f64,
}

/// One tower's placement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TowerSpec {
    /// Name shown in the outliner and on the timeline lane.
    pub label: String,
    /// The base of the tower (the lower launch point): x east, y north, z up, metres; each within ±1e6.
    pub position: [f64; 3],
}

/// An axis-aligned room.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoomSpec {
    /// The corner with the smallest x, y and z, metres; each within ±1e6.
    pub origin: [f64; 3],
    /// Extent along x, y and z, metres; each above 0 and at most 1e4.
    pub size: [f64; 3],
}

/// A horizontal floor that extends to the horizon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FloorSpec {
    /// Height of the floor's top surface, metres.
    #[schemars(range(min = -1.0e6, max = 1.0e6))]
    pub height: f64,
    /// What lies below the surface.
    pub kind: FloorKind,
    /// Density of the floor's matter, kilograms per cubic metre.
    #[schemars(range(min = 0.0, max = 1.0e5))]
    pub density: f64,
}

/// What an infinite floor is made of, in depth.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum FloorKind {
    /// Matter all the way down.
    HalfSpace,
    /// A slab of the given thickness, metres, with nothing below it.
    Slab {
        /// Thickness of the slab, metres.
        thickness: f64,
    },
}

/// Something in a scene that can be selected and highlighted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Target {
    /// A tower, counted from 1 as on screen.
    Tower(usize),
    /// The room.
    Room,
    /// The floor.
    Floor,
}

/// Why a spec was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecError(pub String);

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SpecError {}

impl Default for InstrumentSpec {
    /// The AION-10 preset.
    fn default() -> Self {
        Self {
            tower_height: defaults::TOWER_HEIGHT,
            ifo_separation: defaults::IFO_SEPARATION,
            half_sequence: defaults::HALF_SEQUENCE,
            launch_velocity: defaults::LAUNCH_VELOCITY,
            cycle: defaults::CYCLE,
        }
    }
}

impl Default for ArraySpec {
    /// Five AION-10 towers: one at the centre and four on a cross with 4 m arms.
    fn default() -> Self {
        let towers = defaults::TOWER_POSITIONS
            .iter()
            .enumerate()
            .map(|(i, p)| TowerSpec { label: format!("tower {}", i + 1), position: [p[0], p[1], 0.0] })
            .collect();
        Self { instrument: InstrumentSpec::default(), towers }
    }
}

impl SceneSpec {
    /// The default scene: the five towers in their room.
    #[must_use]
    pub fn room() -> Self {
        Self {
            version: SPEC_VERSION,
            name: "room".into(),
            description: "The five-tower array in its room. Nothing moves yet.".into(),
            duration: defaults::DURATION,
            array: ArraySpec::default(),
            room: Some(RoomSpec { origin: defaults::ROOM_ORIGIN, size: defaults::ROOM_SIZE }),
            floor: None,
        }
    }

    /// Reads a spec from RON, refusing unknown fields and specs that make no sense.
    ///
    /// # Errors
    /// The parse error with its line and column, or the first failed check of [`Self::validate`].
    pub fn from_ron(text: &str) -> Result<Self, SpecError> {
        let spec: Self = ron::from_str(text).map_err(|e| SpecError(format!("scene spec (RON): {e}")))?;
        spec.validate()?;
        Ok(spec)
    }

    /// The spec as RON, laid out for people to read.
    #[must_use]
    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new().struct_names(false);
        ron::ser::to_string_pretty(self, pretty).expect("a scene spec always serialises")
    }

    /// Reads a spec from JSON, refusing unknown fields and specs that make no sense.
    ///
    /// # Errors
    /// The parse error, or the first failed check of [`Self::validate`].
    pub fn from_json(text: &str) -> Result<Self, SpecError> {
        let spec: Self =
            serde_json::from_str(text).map_err(|e| SpecError(format!("scene spec (JSON): {e}")))?;
        spec.validate()?;
        Ok(spec)
    }

    /// The spec as JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a scene spec always serialises")
    }

    /// The JSON Schema of the scene spec (draft 2020-12), with every field's doc string.
    #[must_use]
    pub fn schema() -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(Self)).expect("a schema is JSON")
    }

    /// Checks that the spec describes something that can exist.
    ///
    /// # Errors
    /// The first check that fails, naming the field and its value.
    pub fn validate(&self) -> Result<(), SpecError> {
        let fail = |why: String| Err(SpecError(why));
        if self.version != SPEC_VERSION {
            return fail(format!(
                "version: {} is not supported; this build reads {SPEC_VERSION}",
                self.version
            ));
        }
        if self.name.trim().is_empty() {
            return fail("name: must not be empty".into());
        }
        positive("duration", self.duration, limits::DURATION)?;
        let i = &self.array.instrument;
        positive("array.instrument.tower_height", i.tower_height, limits::TOWER_HEIGHT)?;
        positive("array.instrument.ifo_separation", i.ifo_separation, limits::TOWER_HEIGHT)?;
        positive("array.instrument.half_sequence", i.half_sequence, limits::CYCLE)?;
        positive("array.instrument.launch_velocity", i.launch_velocity, limits::LAUNCH_VELOCITY)?;
        positive("array.instrument.cycle", i.cycle, limits::CYCLE)?;
        if i.ifo_separation >= i.tower_height {
            return fail(format!(
                "array.instrument.ifo_separation: {} m does not fit in a tower of {} m",
                i.ifo_separation, i.tower_height
            ));
        }
        if 2.0 * i.half_sequence >= i.cycle {
            return fail(format!(
                "array.instrument.cycle: {} s is not longer than a shot, 2T = {} s",
                i.cycle,
                2.0 * i.half_sequence
            ));
        }
        if i.half_sequence < limits::HALF_SEQUENCE_MIN {
            return fail(format!(
                "array.instrument.half_sequence: {} s is shorter than {} s",
                i.half_sequence,
                limits::HALF_SEQUENCE_MIN
            ));
        }
        if self.array.towers.is_empty() {
            return fail("array.towers: an array needs at least one tower".into());
        }
        if self.array.towers.len() > limits::TOWERS {
            return fail(format!(
                "array.towers: {} towers is more than {}",
                self.array.towers.len(),
                limits::TOWERS
            ));
        }
        for (n, tower) in self.array.towers.iter().enumerate() {
            finite(&format!("array.towers[{n}].position"), &tower.position)?;
            if tower.label.trim().is_empty() {
                return fail(format!("array.towers[{n}].label: must not be empty"));
            }
            if self.array.towers[..n].iter().any(|t| t.label == tower.label) {
                return fail(format!("array.towers[{n}].label: `{}` is used twice", tower.label));
            }
        }
        if let Some(room) = &self.room {
            finite("room.origin", &room.origin)?;
            for (axis, &s) in room.size.iter().enumerate() {
                positive(&format!("room.size[{axis}]"), s, limits::ROOM_SIZE)?;
            }
        }
        if let Some(floor) = &self.floor {
            finite("floor.height", &[floor.height])?;
            positive("floor.density", floor.density, limits::DENSITY)?;
            if let FloorKind::Slab { thickness } = floor.kind {
                positive("floor.kind.Slab.thickness", thickness, limits::POSITION)?;
            }
        }
        Ok(())
    }

    /// The box that holds everything the spec places: `(minimum corner, maximum corner)`, metres.
    #[must_use]
    pub fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        let mut include = |p: [f64; 3]| {
            for axis in 0..3 {
                lo[axis] = lo[axis].min(p[axis]);
                hi[axis] = hi[axis].max(p[axis]);
            }
        };
        for tower in &self.array.towers {
            let [x, y, z] = tower.position;
            include([x, y, z]);
            include([x, y, z + self.array.instrument.tower_height]);
        }
        if let Some(room) = &self.room {
            include(room.origin);
            include([
                room.origin[0] + room.size[0],
                room.origin[1] + room.size[1],
                room.origin[2] + room.size[2],
            ]);
        }
        (lo, hi)
    }
}

fn positive(path: &str, value: f64, most: f64) -> Result<(), SpecError> {
    if value.is_finite() && value > 0.0 && value <= most {
        Ok(())
    } else {
        Err(SpecError(format!("{path}: {value} must be a positive number, at most {most}")))
    }
}

fn finite(path: &str, values: &[f64]) -> Result<(), SpecError> {
    match values.iter().find(|v| !(v.is_finite() && v.abs() <= limits::POSITION)) {
        None => Ok(()),
        Some(v) => Err(SpecError(format!("{path}: {v} must be within ±{} m", limits::POSITION))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A spec whose numbers have no short decimal form, to make a lossy round trip visible.
    fn awkward() -> SceneSpec {
        let mut spec = SceneSpec::room();
        spec.duration = 0.1 + 0.2; // 0.30000000000000004
        spec.array.towers[1].position = [std::f64::consts::PI, -1.0 / 3.0, 5e-324];
        spec.array.instrument.launch_velocity = 3.86 * (1.0 + f64::EPSILON);
        spec.floor =
            Some(FloorSpec { height: -1e-17, kind: FloorKind::Slab { thickness: 0.7 }, density: 2.4e3 });
        spec.room = Some(RoomSpec {
            origin: [-5.0, -5.0, 0.0],
            size: [9_999.999_999_999_9, 10.0, 2.999_999_999_999_999_6],
        });
        spec
    }

    #[test]
    fn spec_round_trips_through_ron_and_json() {
        for spec in [SceneSpec::room(), awkward()] {
            // `PartialEq` on f64 is bitwise here apart from ±0 and NaN, which `validate` and the
            // values above exclude, so equality is the bit-for-bit claim.
            assert_eq!(SceneSpec::from_ron(&spec.to_ron()).unwrap(), spec, "RON: {}", spec.name);
            assert_eq!(SceneSpec::from_json(&spec.to_json()).unwrap(), spec, "JSON: {}", spec.name);
        }
        // Control arm: a change of one unit in the last place survives the trip, so the equality
        // above is not blind to small differences.
        let spec = awkward();
        let mut nudged = spec.clone();
        nudged.duration = f64::from_bits(spec.duration.to_bits() + 1);
        assert_ne!(SceneSpec::from_ron(&nudged.to_ron()).unwrap(), spec);
        assert_ne!(SceneSpec::from_json(&nudged.to_json()).unwrap(), spec);
        assert_eq!(
            SceneSpec::from_ron(&nudged.to_ron()).unwrap().duration.to_bits(),
            nudged.duration.to_bits()
        );
    }

    #[test]
    fn unknown_fields_are_refused() {
        let good = SceneSpec::room().to_ron();
        assert!(SceneSpec::from_ron(&good).is_ok());
        // A misspelt or stale field must not be dropped in silence.
        let extra = good.replacen("duration:", "colour: \"red\",\n    duration:", 1);
        assert_ne!(extra, good);
        let why = SceneSpec::from_ron(&extra).unwrap_err().0;
        assert!(why.contains("colour"), "{why}");
        let nested = good.replacen("cycle:", "cadence: 2.0,\n            cycle:", 1);
        assert_ne!(nested, good);
        assert!(SceneSpec::from_ron(&nested).unwrap_err().0.contains("cadence"));
        let json = SceneSpec::room().to_json().replacen("\"duration\"", "\"extra\": 1, \"duration\"", 1);
        assert!(SceneSpec::from_json(&json).unwrap_err().0.contains("extra"));
    }

    #[test]
    fn invalid_specs_are_refused_with_the_reason() {
        type Break = fn(&mut SceneSpec);
        let cases: [(&str, Break); 10] = [
            ("version", |s| s.version = 2),
            ("name", |s| s.name = "  ".into()),
            ("duration", |s| s.duration = 0.0),
            ("array.towers", |s| s.array.towers.clear()),
            ("array.towers[1].label", |s| s.array.towers[1].label = "tower 1".into()),
            ("array.towers[2].position", |s| s.array.towers[2].position[1] = f64::NAN),
            ("array.instrument.ifo_separation", |s| s.array.instrument.ifo_separation = 10.0),
            ("array.instrument.cycle", |s| s.array.instrument.cycle = 1.46),
            ("room.size[2]", |s| s.room.as_mut().unwrap().size[2] = -3.0),
            ("floor.density", |s| s.floor.as_mut().unwrap().density = f64::INFINITY),
        ];
        let mut base = SceneSpec::room();
        base.floor = Some(FloorSpec { height: 0.0, kind: FloorKind::HalfSpace, density: 2400.0 });
        assert_eq!(base.validate(), Ok(()), "the control: the unbroken spec is valid");
        for (field, break_it) in cases {
            let mut spec = base.clone();
            break_it(&mut spec);
            let why = spec.validate().expect_err(field).0;
            assert!(why.starts_with(field), "`{why}` should name `{field}`");
            // The same refusal reaches a file on the way in.
            assert!(SceneSpec::from_json(&spec.to_json()).is_err());
        }
    }

    #[test]
    fn schema_names_every_field_with_its_description() {
        let schema = SceneSpec::schema();
        let text = schema.to_string();
        // Every field of every struct, by reading the serialised default scene.
        fn fields(value: &serde_json::Value, out: &mut Vec<String>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, v) in map {
                        out.push(key.clone());
                        fields(v, out);
                    }
                }
                serde_json::Value::Array(items) => items.iter().for_each(|v| fields(v, out)),
                _ => {}
            }
        }
        let mut names = Vec::new();
        let mut spec = SceneSpec::room();
        spec.floor = Some(FloorSpec { height: 0.0, kind: FloorKind::Slab { thickness: 1.0 }, density: 1.0 });
        fields(&serde_json::to_value(&spec).unwrap(), &mut names);
        names.sort();
        names.dedup();
        assert!(names.len() >= 22, "the walk found the nested fields: {names:?}");
        for name in &names {
            assert!(text.contains(&format!("\"{name}\"")), "the schema does not mention `{name}`");
        }
        // Doc strings become descriptions.
        let duration = &schema["properties"]["duration"];
        assert_eq!(duration["description"], "Length of the timeline, seconds.");
        assert_eq!(duration["type"], "number");
        let required: Vec<&str> =
            schema["required"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert!(required.contains(&"array") && required.contains(&"version"));
        // Control: a name that is not a field is not in the schema.
        assert!(!text.contains("\"cadence\""));
    }

    #[test]
    fn bounds_hold_the_towers_and_the_room() {
        let (lo, hi) = SceneSpec::room().bounds();
        assert_eq!(lo, [-5.0, -5.0, 0.0]);
        assert_eq!(hi, [5.0, 5.0, 10.0]);
        // Control: without the room the box shrinks to the towers.
        let (lo, hi) = SceneSpec { room: None, ..SceneSpec::room() }.bounds();
        assert_eq!(lo, [-4.0, -4.0, 0.0]);
        assert_eq!(hi, [4.0, 4.0, 10.0]);
    }

    /// How many numbers of `spec` come back with different bits from `trip`.
    fn changed_numbers(spec: &SceneSpec, back: &SceneSpec) -> f64 {
        fn numbers(value: &serde_json::Value, out: &mut Vec<u64>) {
            match value {
                serde_json::Value::Number(n) => out.push(n.as_f64().map_or(u64::MAX, f64::to_bits)),
                serde_json::Value::Array(items) => items.iter().for_each(|v| numbers(v, out)),
                serde_json::Value::Object(map) => map.values().for_each(|v| numbers(v, out)),
                _ => {}
            }
        }
        let (mut before, mut after) = (Vec::new(), Vec::new());
        numbers(&serde_json::to_value(spec).unwrap(), &mut before);
        numbers(&serde_json::to_value(back).unwrap(), &mut after);
        assert!(before.len() >= 25, "the awkward spec has {} numbers", before.len());
        if before.len() != after.len() {
            return f64::INFINITY;
        }
        before.iter().zip(&after).filter(|(a, b)| a != b).count() as f64
    }

    cav_validate::case! {
        test: validation_spec_ron_round_trip,
        name: "spec.ron_round_trip",
        subsystem: "spec",
        slice: "S0",
        reference: Exact,
        against: "a spec of awkward numbers written to RON and read back; numbers whose bits differ",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let spec = awkward();
            cav_validate::Measured::Error(changed_numbers(&spec, &SceneSpec::from_ron(&spec.to_ron()).unwrap()))
        },
    }

    cav_validate::case! {
        test: validation_spec_json_round_trip,
        name: "spec.json_round_trip",
        subsystem: "spec",
        slice: "S0",
        reference: Exact,
        against: "a spec of awkward numbers written to JSON and read back; numbers whose bits differ",
        unit: Count,
        tolerance: 0.0,
        measure: {
            let spec = awkward();
            cav_validate::Measured::Error(changed_numbers(&spec, &SceneSpec::from_json(&spec.to_json()).unwrap()))
        },
    }

    #[test]
    fn the_round_trip_measure_sees_one_changed_number() {
        // The control arm of the two cases above: nudge one number by one unit in the last place.
        let spec = awkward();
        let mut nudged = spec.clone();
        nudged.duration = f64::from_bits(spec.duration.to_bits() + 1);
        assert_eq!(changed_numbers(&spec, &nudged), 1.0);
        assert_eq!(changed_numbers(&spec, &spec), 0.0);
    }

    cav_validate::case! {
        test: validation_array_geometry_matches_the_v2_recording,
        name: "array.geometry_vs_v2_recording",
        subsystem: "array",
        slice: "S0",
        reference: Recording,
        against: "tower positions and room of docs/viewer-mockup/replay.json (the v2 recording); largest difference",
        unit: Metres,
        tolerance: 0.0,
        measure: {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/viewer-mockup/replay.json");
            let recording: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let number = |v: &serde_json::Value| v.as_f64().expect("a number");
            let scene = SceneSpec::room();
            let mut worst = 0f64;
            let towers = recording["towers"].as_array().expect("towers");
            assert_eq!(towers.len(), scene.array.towers.len(), "the recording has five towers");
            for (recorded, tower) in towers.iter().zip(&scene.array.towers) {
                for axis in 0..2 {
                    worst = worst.max((number(&recorded[axis]) - tower.position[axis]).abs());
                }
            }
            // The recording's air grid is the room: origin, and cells × pitch along each axis.
            let room = scene.room.as_ref().expect("the default scene has a room");
            let grid = &recording["grid"];
            for axis in 0..3 {
                worst = worst.max((number(&grid["origin"][axis]) - room.origin[axis]).abs());
                worst = worst.max((number(&grid["n"][axis]) * number(&grid["h"]) - room.size[axis]).abs());
            }
            // And the flight is the recording's: 2T.
            worst = worst.max((number(&recording["two_t"]) - 2.0 * scene.array.instrument.half_sequence).abs());
            cav_validate::Measured::Error(worst)
        },
    }

    #[test]
    fn specs_outside_the_stated_ranges_are_refused() {
        type Break = fn(&mut SceneSpec);
        // The first three are the specs that hung the renderer or exhausted memory before the
        // ranges existed (S0 review, finding 1).
        let cases: [(&str, Break); 12] = [
            ("room.origin", |s| s.room.as_mut().unwrap().origin[0] = 1e17),
            ("room.size[0]", |s| s.room.as_mut().unwrap().size[0] = 1e10),
            ("array.instrument.tower_height", |s| {
                s.array.instrument.tower_height = 3e6;
                s.array.instrument.ifo_separation = 5.0;
            }),
            ("duration", |s| s.duration = 1e9),
            ("array.instrument.cycle", |s| s.array.instrument.cycle = 1e5),
            ("array.instrument.half_sequence", |s| s.array.instrument.half_sequence = 1e-6),
            ("array.instrument.launch_velocity", |s| s.array.instrument.launch_velocity = 1e9),
            ("array.towers[3].position", |s| s.array.towers[3].position[2] = -2e6),
            ("array.towers", |s| {
                let tower = s.array.towers[0].clone();
                s.array.towers =
                    (0..1001).map(|n| TowerSpec { label: format!("t{n}"), ..tower.clone() }).collect();
            }),
            ("floor.height", |s| s.floor.as_mut().unwrap().height = 1e7),
            ("floor.density", |s| s.floor.as_mut().unwrap().density = 1e6),
            ("floor.kind.Slab.thickness", |s| {
                s.floor.as_mut().unwrap().kind = FloorKind::Slab { thickness: 1e7 }
            }),
        ];
        let mut base = SceneSpec::room();
        base.floor = Some(FloorSpec { height: 0.0, kind: FloorKind::HalfSpace, density: 2400.0 });
        for (field, break_it) in cases {
            let mut spec = base.clone();
            break_it(&mut spec);
            let why = spec.validate().expect_err(field).0;
            assert!(why.starts_with(field), "`{why}` should name `{field}`");
            assert!(SceneSpec::from_ron(&spec.to_ron()).is_err(), "{field}: a file with it is refused too");
        }
        // Control: a spec at the edge of every range is accepted, so the ranges are the stated ones.
        let mut edge = base;
        edge.duration = limits::DURATION;
        edge.array.instrument.tower_height = limits::TOWER_HEIGHT;
        edge.array.instrument.ifo_separation = 500.0;
        edge.array.instrument.half_sequence = limits::HALF_SEQUENCE_MIN;
        edge.array.instrument.cycle = limits::CYCLE;
        edge.array.towers[0].position = [limits::POSITION, -limits::POSITION, 0.0];
        let room = edge.room.as_mut().unwrap();
        (room.origin, room.size) = ([-limits::POSITION; 3], [limits::ROOM_SIZE; 3]);
        assert_eq!(edge.validate(), Ok(()));
        // The schema states the ranges.
        let schema = SceneSpec::schema();
        assert_eq!(schema["properties"]["duration"]["maximum"], 1.0e6);
        assert_eq!(schema["$defs"]["InstrumentSpec"]["properties"]["tower_height"]["maximum"], 1.0e3);
        assert!(schema.to_string().contains("each above 0 and at most 1e4"));
    }
}
