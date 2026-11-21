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
    /// The towers, in display order; tower 1 is the first.
    pub towers: Vec<TowerSpec>,
}

/// One gradiometer: two atom interferometers stacked in a vertical tower.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InstrumentSpec {
    /// Height of the tower, metres.
    pub tower_height: f64,
    /// Vertical separation of the two interferometers (Δr), metres.
    pub ifo_separation: f64,
    /// Half the interrogation sequence, T, seconds; a shot lasts 2T from launch to readout.
    pub half_sequence: f64,
    /// Launch velocity u₀, metres per second.
    pub launch_velocity: f64,
    /// Time between shots of one tower, seconds.
    pub cycle: f64,
}

/// One tower's placement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TowerSpec {
    /// Name shown in the outliner and on the timeline lane.
    pub label: String,
    /// The base of the tower (the lower launch point): x east, y north, z up, metres.
    pub position: [f64; 3],
}

/// An axis-aligned room.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoomSpec {
    /// The corner with the smallest x, y and z, metres.
    pub origin: [f64; 3],
    /// Extent along x, y and z, metres.
    pub size: [f64; 3],
}

/// A horizontal floor that extends to the horizon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FloorSpec {
    /// Height of the floor's top surface, metres.
    pub height: f64,
    /// What lies below the surface.
    pub kind: FloorKind,
    /// Density of the floor's matter, kilograms per cubic metre.
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
        positive("duration", self.duration)?;
        let i = &self.array.instrument;
        positive("array.instrument.tower_height", i.tower_height)?;
        positive("array.instrument.ifo_separation", i.ifo_separation)?;
        positive("array.instrument.half_sequence", i.half_sequence)?;
        positive("array.instrument.launch_velocity", i.launch_velocity)?;
        positive("array.instrument.cycle", i.cycle)?;
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
        if self.array.towers.is_empty() {
            return fail("array.towers: an array needs at least one tower".into());
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
                positive(&format!("room.size[{axis}]"), s)?;
            }
        }
        if let Some(floor) = &self.floor {
            finite("floor.height", &[floor.height])?;
            positive("floor.density", floor.density)?;
            if let FloorKind::Slab { thickness } = floor.kind {
                positive("floor.kind.Slab.thickness", thickness)?;
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

fn positive(path: &str, value: f64) -> Result<(), SpecError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(SpecError(format!("{path}: {value} must be a positive, finite number")))
    }
}

fn finite(path: &str, values: &[f64]) -> Result<(), SpecError> {
    match values.iter().find(|v| !v.is_finite()) {
        None => Ok(()),
        Some(v) => Err(SpecError(format!("{path}: {v} must be finite"))),
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
        spec.room = Some(RoomSpec { origin: [-5.0, -5.0, 0.0], size: [1e10, 10.0, 2.999_999_999_999_999_6] });
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
}
