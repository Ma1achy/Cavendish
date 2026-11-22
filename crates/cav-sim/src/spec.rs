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

use crate::motion::Motion;

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
    /// The longest half-sequence, seconds. AION-10's is 0.73 s and AICE's 2.5 s; proposed space
    /// instruments interrogate for hundreds of seconds. A thousand holds all of them, and keeps the
    /// work of one read (which grows with the time the atoms are in the air) inside its budget.
    pub const HALF_SEQUENCE_MAX: f64 = 1.0e3;
    /// The longest timeline, seconds.
    pub const DURATION: f64 = 1.0e6;
    /// The fastest launch, metres per second.
    pub const LAUNCH_VELOCITY: f64 = 1.0e4;
    /// The densest floor, kilograms per cubic metre.
    pub const DENSITY: f64 = 1.0e5;
    /// The most bodies in a scene.
    pub const BODIES: usize = 1000;
    /// The heaviest body, kilograms.
    pub const MASS: f64 = 1.0e12;
    /// The largest sphere, metres.
    pub const RADIUS: f64 = 1.0e3;
    /// The fastest pass, metres per second.
    pub const SPEED: f64 = 1.0e4;
    /// The highest frequency of an oscillator, hertz.
    pub const FREQUENCY: f64 = 1.0e3;
    /// The largest angle written in a spec, degrees either way.
    pub const ANGLE: f64 = 3.6e5;
    /// The atom's mass, kilograms: from a hydrogen atom to well past any atom there is.
    pub const ATOM_MASS: (f64, f64) = (1.0e-27, 1.0e-24);
    /// Local gravity, metres per second squared.
    pub const GRAVITY: (f64, f64) = (0.1, 100.0);
    /// The longest name or label, characters.
    pub const LABEL: usize = 200;
    /// The longest description, characters.
    pub const DESCRIPTION: usize = 4000;
}

/// The version of the spec format this build reads and writes.
pub const SPEC_VERSION: u32 = 1;

/// A scene: the array, and what surrounds it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneSpec {
    /// Version of the spec format; this build reads version 1.
    #[schemars(extend("x-stage" = "nothing"))]
    pub version: u32,
    /// Short name, as shown in the library and the title bar. At most 200 characters.
    #[schemars(length(min = 1, max = 200))]
    #[schemars(extend("x-stage" = "nothing"))]
    pub name: String,
    /// One or two sentences on what the scene is and what it checks. At most 4000 characters.
    #[schemars(length(max = 4000))]
    #[schemars(extend("x-stage" = "nothing"))]
    pub description: String,
    /// Length of the timeline, seconds.
    #[schemars(range(max = 1.0e6), extend("exclusiveMinimum" = 0.0))]
    #[schemars(extend("x-stage" = "forward", "x-unit" = "s"))]
    pub duration: f64,
    /// The towers and the instrument they share.
    #[schemars(extend("x-stage" = "forward"))]
    pub array: ArraySpec,
    /// The room around the array, or none for an open scene.
    #[schemars(extend("x-stage" = "view"))]
    pub room: Option<RoomSpec>,
    /// A floor under the array that extends to the horizon, or none. Its top lies at or below
    /// every tower's base: in S1 the floor is an analytic slab, and the atoms fly above it.
    #[schemars(extend("x-stage" = "forward"))]
    pub floor: Option<FloorSpec>,
    /// The bodies: point masses and uniform spheres, each on its path. At most 1000.
    #[serde(default)]
    #[schemars(extend("x-stage" = "forward"))]
    pub bodies: Vec<BodySpec>,
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
    #[schemars(range(max = 1.0e3), extend("exclusiveMinimum" = 0.0))]
    #[schemars(extend("x-unit" = "m"))]
    pub tower_height: f64,
    /// Vertical separation of the two interferometers (Δr), metres. Less than `tower_height`.
    #[schemars(range(max = 1.0e3), extend("exclusiveMinimum" = 0.0))]
    #[schemars(extend("x-unit" = "m"))]
    pub ifo_separation: f64,
    /// Half the interrogation sequence, T, seconds; a shot lasts 2T from launch to readout.
    /// 2T is at most `cycle`. At most 1000 s.
    #[schemars(range(min = 1.0e-3, max = 1.0e3))]
    #[schemars(extend("x-unit" = "s"))]
    pub half_sequence: f64,
    /// Launch velocity u₀, metres per second.
    #[schemars(range(max = 1.0e4), extend("exclusiveMinimum" = 0.0))]
    #[schemars(extend("x-unit" = "m/s"))]
    pub launch_velocity: f64,
    /// Time between shots of one tower, seconds. At least a shot: `cycle` ≥ 2 × `half_sequence`
    /// (AICE's 5 s cycle is exactly its 2T: each shot is launched as the last is read out).
    #[schemars(range(max = 1.0e4), extend("exclusiveMinimum" = 0.0))]
    #[schemars(extend("x-unit" = "s"))]
    pub cycle: f64,
    /// The atom's mass, kilograms. The default is ⁸⁷Sr, 1.443 155 87e-25 kg.
    #[serde(default = "default_atom_mass")]
    #[schemars(range(min = 1.0e-27, max = 1.0e-24))]
    #[schemars(extend("x-unit" = "kg"))]
    pub atom_mass: f64,
    /// Local gravity, metres per second squared. The default is 9.81.
    #[serde(default = "default_gravity")]
    #[schemars(range(min = 0.1, max = 100.0))]
    #[schemars(extend("x-unit" = "m/s²"))]
    pub gravity: f64,
    /// Whether an arm that falls back to its launch height stops there and waits (for the kick at
    /// T, or for the next sequence). The default is true.
    #[serde(default = "default_launch_floor")]
    pub launch_floor: bool,
    /// Which way round the two interferometers are differenced. The default is lower minus upper.
    #[serde(default)]
    pub sign: SignSpec,
}

fn default_atom_mass() -> f64 {
    cav_core::constants::SR87_MASS
}

fn default_gravity() -> f64 {
    cav_core::constants::LOCAL_GRAVITY
}

const fn default_launch_floor() -> bool {
    true
}

/// Which way round a tower's two interferometers are differenced. Interferometer 1 is the lower one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum SignSpec {
    /// ΔΦ = δφ₁ − δφ₂: the lower interferometer minus the upper.
    #[default]
    LowerMinusUpper,
    /// ΔΦ = δφ₂ − δφ₁: the upper minus the lower, as v1 had it.
    UpperMinusLower,
}

/// A body: a point mass, or a uniform sphere, on a path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BodySpec {
    /// Name shown in the outliner; no two bodies share one. At most 200 characters.
    #[schemars(length(min = 1, max = 200))]
    pub label: String,
    /// Kilograms.
    #[schemars(range(max = 1.0e12), extend("exclusiveMinimum" = 0.0))]
    #[schemars(extend("x-unit" = "kg"))]
    pub mass: f64,
    /// The radius of the sphere, metres; zero (the default) for a point. A sphere is a point
    /// outside itself and may cross an atom's path, where a point may not.
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1.0e3))]
    #[schemars(extend("x-unit" = "m"))]
    pub radius: f64,
    /// How it moves.
    pub motion: Motion,
    /// When it comes into being, seconds; from the start if absent.
    #[serde(default)]
    #[schemars(extend("x-unit" = "s"))]
    pub appears: Option<f64>,
    /// When it ceases to be, seconds; never if absent. Later than `appears`.
    #[serde(default)]
    #[schemars(extend("x-unit" = "s"))]
    pub vanishes: Option<f64>,
}

/// One tower's placement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TowerSpec {
    /// Name shown in the outliner and on the timeline lane. At most 200 characters.
    #[schemars(length(min = 1, max = 200))]
    pub label: String,
    /// The base of the tower (the lower launch point): x east, y north, z up, metres; each within ±1e6.
    #[schemars(extend("x-unit" = "m"))]
    pub position: [f64; 3],
    /// The tower's own clock: when it fires relative to the array's schedule.
    #[serde(default)]
    pub timing: TimingSpec,
}

/// One tower's clock. Every tower takes the same shots as the array's schedule says; a tower's
/// timing shifts all of its shots together.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimingSpec {
    /// How much later than the array's schedule this tower launches (and reads out) each shot,
    /// seconds; negative for earlier. Within ± the longest cycle, 1e4 s. The default is 0: every
    /// tower fires together.
    #[serde(default)]
    #[schemars(range(min = -1.0e4, max = 1.0e4), extend("x-unit" = "s"))]
    pub offset: f64,
}

impl TowerSpec {
    /// A tower at `position` (its base, metres) that fires with the array's schedule.
    #[must_use]
    pub fn at(label: &str, position: [f64; 3]) -> Self {
        Self { label: label.to_owned(), position, timing: TimingSpec::default() }
    }
}

/// An axis-aligned room.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoomSpec {
    /// The corner with the smallest x, y and z, metres; each within ±1e6.
    #[schemars(extend("x-unit" = "m"))]
    pub origin: [f64; 3],
    /// Extent along x, y and z, metres; each above 0 and at most 1e4.
    #[schemars(extend("x-unit" = "m"))]
    pub size: [f64; 3],
}

/// A horizontal floor that extends to the horizon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FloorSpec {
    /// Height of the floor's top surface, metres.
    #[schemars(range(min = -1.0e6, max = 1.0e6))]
    #[schemars(extend("x-unit" = "m"))]
    pub height: f64,
    /// What lies below the surface.
    pub kind: FloorKind,
    /// Density of the floor's matter, kilograms per cubic metre.
    #[schemars(range(max = 1.0e5), extend("exclusiveMinimum" = 0.0))]
    #[schemars(extend("x-unit" = "kg/m³"))]
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
        #[schemars(range(max = 1.0e6), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
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
    /// A body, counted from 1 as on screen.
    Body(usize),
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
            atom_mass: default_atom_mass(),
            gravity: default_gravity(),
            launch_floor: default_launch_floor(),
            sign: SignSpec::default(),
        }
    }
}

impl InstrumentSpec {
    /// The instrument as the forward model takes it: this spec's geometry, atom, gravity, floor
    /// and sign, with the engine's constants and quadrature.
    #[must_use]
    pub fn instrument(&self) -> cav_core::forward::Instrument {
        cav_core::forward::Instrument {
            ifo_separation: self.ifo_separation,
            half_sequence: self.half_sequence,
            launch_velocity: self.launch_velocity,
            gravity: self.gravity,
            atom_mass: self.atom_mass,
            floor: self.launch_floor,
            sign: match self.sign {
                SignSpec::LowerMinusUpper => cav_core::forward::Sign::LowerMinusUpper,
                SignSpec::UpperMinusLower => cav_core::forward::Sign::UpperMinusLower,
            },
            ..cav_core::forward::Instrument::aion10()
        }
    }
}

impl BodySpec {
    /// A point mass at rest.
    #[must_use]
    pub fn point(label: &str, mass: f64, position: [f64; 3]) -> Self {
        Self::moving(label, mass, Motion::Static { position })
    }

    /// A point mass on a path, there for the whole timeline.
    #[must_use]
    pub fn moving(label: &str, mass: f64, motion: Motion) -> Self {
        Self { label: label.to_owned(), mass, radius: 0.0, motion, appears: None, vanishes: None }
    }

    /// When the body exists: from the first time up to the second, seconds.
    #[must_use]
    pub fn exists(&self) -> (f64, f64) {
        (self.appears.unwrap_or(f64::NEG_INFINITY), self.vanishes.unwrap_or(f64::INFINITY))
    }
}

impl Default for ArraySpec {
    /// Five AION-10 towers: one at the centre and four on a cross with 4 m arms.
    fn default() -> Self {
        let towers = defaults::TOWER_POSITIONS
            .iter()
            .enumerate()
            .map(|(i, p)| TowerSpec::at(&format!("tower {}", i + 1), [p[0], p[1], 0.0]))
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
            bodies: Vec::new(),
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

    /// Writes the scene to a `.ron` file, replacing what is there. The scene is checked first, and
    /// the text is written beside the file and moved into place, so a refusal or a failure half
    /// way leaves the file as it was. Comments in a file that is saved over are not kept.
    ///
    /// # Errors
    /// The first failed check of [`Self::validate`], or why the file could not be written.
    pub fn save(&self, path: &str) -> Result<(), SpecError> {
        self.validate()?;
        let failed = |e: std::io::Error| SpecError(format!("{path}: the scene was not saved: {e}"));
        let beside = format!("{path}.saving");
        std::fs::write(&beside, self.to_ron()).map_err(failed)?;
        std::fs::rename(&beside, path).map_err(|e| {
            let _ = std::fs::remove_file(&beside);
            failed(e)
        })
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
        Self::schema_ref().clone()
    }

    /// The same schema, made once and kept: it is derived from the types, so it cannot change
    /// while the program runs. Paths read it on every look-up.
    #[must_use]
    pub fn schema_ref() -> &'static serde_json::Value {
        static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| serde_json::to_value(schemars::schema_for!(Self)).expect("a schema is JSON"))
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
        short("name", &self.name, limits::LABEL)?;
        short("description", &self.description, limits::DESCRIPTION)?;
        positive("duration", self.duration, limits::DURATION)?;
        let i = &self.array.instrument;
        positive("array.instrument.tower_height", i.tower_height, limits::TOWER_HEIGHT)?;
        positive("array.instrument.ifo_separation", i.ifo_separation, limits::TOWER_HEIGHT)?;
        positive("array.instrument.half_sequence", i.half_sequence, limits::HALF_SEQUENCE_MAX)?;
        positive("array.instrument.launch_velocity", i.launch_velocity, limits::LAUNCH_VELOCITY)?;
        positive("array.instrument.cycle", i.cycle, limits::CYCLE)?;
        if i.ifo_separation >= i.tower_height {
            return fail(format!(
                "array.instrument.ifo_separation: {} m does not fit in a tower of {} m",
                i.ifo_separation, i.tower_height
            ));
        }
        if 2.0 * i.half_sequence > i.cycle {
            return fail(format!(
                "array.instrument.cycle: {} s is shorter than a shot, 2T = {} s",
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
        // Labels seen so far, for the check that no two are the same: a set, so that a thousand
        // towers are a thousand look-ups and not a million comparisons.
        let mut seen = std::collections::HashSet::with_capacity(self.array.towers.len());
        for (n, tower) in self.array.towers.iter().enumerate() {
            finite(&format!("array.towers[{n}].position"), &tower.position)?;
            if tower.label.trim().is_empty() {
                return fail(format!("array.towers[{n}].label: must not be empty"));
            }
            short(&format!("array.towers[{n}].label"), &tower.label, limits::LABEL)?;
            if !seen.insert(tower.label.as_str()) {
                return fail(format!("array.towers[{n}].label: `{}` is used twice", tower.label));
            }
            within(
                &format!("array.towers[{n}].timing.offset"),
                tower.timing.offset,
                (-limits::CYCLE, limits::CYCLE),
            )?;
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
            // The floor is an analytic slab whose field is uniform above it and only there.
            if let Some(tower) = self.array.towers.iter().find(|t| t.position[2] < floor.height) {
                return fail(format!(
                    "floor.height: {} m is above the base of `{}` ({} m); the floor lies at or below every \
                     tower's base",
                    floor.height, tower.label, tower.position[2]
                ));
            }
        }
        within("array.instrument.atom_mass", i.atom_mass, limits::ATOM_MASS)?;
        within("array.instrument.gravity", i.gravity, limits::GRAVITY)?;
        if self.bodies.len() > limits::BODIES {
            return fail(format!("bodies: {} bodies is more than {}", self.bodies.len(), limits::BODIES));
        }
        let mut seen = std::collections::HashSet::with_capacity(self.bodies.len());
        for (n, body) in self.bodies.iter().enumerate() {
            let at = format!("bodies[{n}]");
            if body.label.trim().is_empty() {
                return fail(format!("{at}.label: must not be empty"));
            }
            short(&format!("{at}.label"), &body.label, limits::LABEL)?;
            if !seen.insert(body.label.as_str()) {
                return fail(format!("{at}.label: `{}` is used twice", body.label));
            }
            positive(&format!("{at}.mass"), body.mass, limits::MASS)?;
            within(&format!("{at}.radius"), body.radius, (0.0, limits::RADIUS))?;
            body.motion.validate(&format!("{at}.motion")).map_err(SpecError)?;
            for (name, time) in [("appears", body.appears), ("vanishes", body.vanishes)] {
                if let Some(time) = time {
                    within(&format!("{at}.{name}"), time, (-limits::DURATION, limits::DURATION))?;
                }
            }
            if let (Some(appears), Some(vanishes)) = (body.appears, body.vanishes)
                && vanishes <= appears
            {
                return fail(format!("{at}.vanishes: {vanishes} s is not later than `appears`, {appears} s"));
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
        // Each body where it is at nine times along the timeline: enough to hold a pass or an orbit.
        for body in &self.bodies {
            for step in 0..=8 {
                let time = self.duration * f64::from(step) / 8.0;
                include(body.motion.position(time, self.array.instrument.gravity));
            }
        }
        (lo, hi)
    }
}

/// Text in a spec has a length: a label of a gigabyte is a valid string and not a valid label.
fn short(path: &str, text: &str, most: usize) -> Result<(), SpecError> {
    let length = text.chars().count();
    if length <= most {
        Ok(())
    } else {
        Err(SpecError(format!("{path}: {length} characters is longer than {most}")))
    }
}

fn positive(path: &str, value: f64, most: f64) -> Result<(), SpecError> {
    if value.is_finite() && value > 0.0 && value <= most {
        Ok(())
    } else {
        Err(SpecError(format!("{path}: {value} must be a positive number, at most {most}")))
    }
}

fn within(path: &str, value: f64, (least, most): (f64, f64)) -> Result<(), SpecError> {
    if value.is_finite() && value >= least && value <= most {
        Ok(())
    } else {
        Err(SpecError(format!("{path}: {value} must be at least {least} and at most {most}")))
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
    fn bodies_round_trip_and_are_validated() {
        use crate::dev;
        // Every kind of motion, a sphere and a body that comes and goes, through both forms.
        let mut spec = dev::oscillator_zoo();
        spec.bodies.extend(dev::pass().bodies);
        spec.bodies.extend(dev::flash().bodies);
        spec.bodies[5].label = "flash".into();
        spec.bodies[5].radius = 1.0 / 3.0;
        spec.array.instrument.sign = SignSpec::UpperMinusLower;
        spec.array.instrument.launch_floor = false;
        assert_eq!(spec.validate(), Ok(()));
        assert_eq!(SceneSpec::from_ron(&spec.to_ron()).unwrap(), spec);
        assert_eq!(SceneSpec::from_json(&spec.to_json()).unwrap(), spec);
        // A file from S0, with no bodies and no word about the atom, loads with George's defaults.
        let old = SceneSpec::room()
            .to_ron()
            .lines()
            .filter(|l| {
                !["atom_mass", "gravity", "launch_floor", "sign", "bodies"].iter().any(|f| l.contains(f))
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!old.contains("bodies") && !old.contains("atom_mass"));
        let loaded = SceneSpec::from_ron(&old).unwrap();
        assert_eq!(loaded, SceneSpec::room());
        assert_eq!(loaded.array.instrument.atom_mass, cav_core::constants::SR87_MASS);
        assert!(loaded.array.instrument.launch_floor && loaded.bodies.is_empty());
        // Refusals name the field.
        type Break = fn(&mut SceneSpec);
        let cases: [(&str, Break); 9] = [
            ("bodies[1].label", |s| s.bodies[1].label = "linear".into()),
            ("bodies[0].label", |s| s.bodies[0].label = " ".into()),
            ("bodies[0].mass", |s| s.bodies[0].mass = 0.0),
            ("bodies[2].radius", |s| s.bodies[2].radius = -0.1),
            ("bodies[5].vanishes", |s| s.bodies[5].vanishes = Some(4.5)),
            ("bodies[5].appears", |s| s.bodies[5].appears = Some(f64::NAN)),
            ("bodies[0].motion.Linear.frequency", |s| {
                if let Motion::Linear { frequency, .. } = &mut s.bodies[0].motion {
                    *frequency = 1e9;
                }
            }),
            ("array.instrument.atom_mass", |s| s.array.instrument.atom_mass = 1.0),
            ("floor.height", |s| {
                s.floor = Some(FloorSpec { height: 0.5, kind: FloorKind::HalfSpace, density: 2400.0 });
            }),
        ];
        for (field, break_it) in cases {
            let mut broken = spec.clone();
            break_it(&mut broken);
            let why = broken.validate().expect_err(field).0;
            assert!(why.starts_with(field), "`{why}` should name `{field}`");
        }
        // The schema describes a body and the atom.
        let schema = SceneSpec::schema().to_string();
        for name in ["bodies", "BodySpec", "Motion", "Pendulum", "atom_mass", "launch_floor", "SignSpec"] {
            assert!(schema.contains(&format!("\"{name}\"")), "the schema does not mention `{name}`");
        }
        // The bounds hold a body's path: the pass ends 15 m up, well above the towers.
        assert_eq!(dev::pass().bounds().1[2], 15.0);
    }

    #[test]
    fn the_spec_gives_the_forward_model_its_instrument() {
        use cav_core::forward::{Instrument, Sign};
        // The default spec is the engine's default instrument: George's answers.
        assert_eq!(InstrumentSpec::default().instrument(), Instrument::aion10());
        let spec = InstrumentSpec {
            ifo_separation: 70.0,
            half_sequence: 2.5,
            launch_velocity: 21.0,
            atom_mass: 1.46e-25,
            gravity: 9.8,
            launch_floor: false,
            sign: SignSpec::UpperMinusLower,
            ..InstrumentSpec::default()
        };
        let instrument = spec.instrument();
        assert_eq!(
            (instrument.ifo_separation, instrument.half_sequence, instrument.launch_velocity),
            (70.0, 2.5, 21.0)
        );
        assert_eq!((instrument.atom_mass, instrument.gravity, instrument.floor), (1.46e-25, 9.8, false));
        assert_eq!(instrument.sign, Sign::UpperMinusLower);
        // Control: what the spec does not set is the engine's, not changed on the way.
        assert_eq!(instrument.hbar, Instrument::aion10().hbar);
        assert_eq!(instrument.quadrature, Instrument::aion10().quadrature);
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
            ("array.instrument.cycle", |s| s.array.instrument.cycle = 1.45),
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
        against: "tower x, y and base z, the room and 2T of docs/viewer-mockup/replay.json (the v2 recording); largest difference",
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
            // The towers stand on the recording's floor: their base is at the grid's lowest z. (The
            // recording has no tower height; `instrument_defaults_are_the_aion_10_preset` pins that.)
            for tower in &scene.array.towers {
                worst = worst.max((number(&grid["origin"][2]) - tower.position[2]).abs());
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
        let cases: [(&str, Break); 18] = [
            ("name", |s| s.name = "n".repeat(limits::LABEL + 1)),
            ("description", |s| s.description = "d".repeat(limits::DESCRIPTION + 1)),
            ("array.towers[2].label", |s| s.array.towers[2].label = "t".repeat(limits::LABEL + 1)),
            ("bodies[0].label", |s| s.bodies[0].label = "b".repeat(limits::LABEL + 1)),
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
            ("array.towers[1].timing.offset", |s| s.array.towers[1].timing.offset = 2.0e4),
            ("array.towers[4].timing.offset", |s| {
                s.array.towers[4].timing.offset = f64::NAN;
            }),
            ("floor.height", |s| s.floor.as_mut().unwrap().height = 1e7),
            ("floor.density", |s| s.floor.as_mut().unwrap().density = 1e6),
            ("floor.kind.Slab.thickness", |s| {
                s.floor.as_mut().unwrap().kind = FloorKind::Slab { thickness: 1e7 }
            }),
        ];
        let mut base = SceneSpec::room();
        base.floor = Some(FloorSpec { height: 0.0, kind: FloorKind::HalfSpace, density: 2400.0 });
        base.bodies.push(BodySpec::point("mass", 1.0, [3.0, 0.0, 1.0]));
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
        edge.name = "é".repeat(limits::LABEL);
        edge.description = "d".repeat(limits::DESCRIPTION);
        edge.bodies[0].label = "b".repeat(limits::LABEL);
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
        assert_eq!(schema["properties"]["name"]["maxLength"], limits::LABEL);
        assert_eq!(schema["properties"]["description"]["maxLength"], limits::DESCRIPTION);
        assert_eq!(schema["$defs"]["BodySpec"]["properties"]["label"]["maxLength"], limits::LABEL);
    }

    /// The schema of the field at `path`: `["InstrumentSpec", "cycle"]`, or `["duration"]` at the root.
    fn field_schema(schema: &serde_json::Value, path: &[&str]) -> serde_json::Value {
        match path {
            [field] => schema["properties"][field].clone(),
            [owner, field] => schema["$defs"][owner]["properties"][field].clone(),
            _ => panic!("{path:?}"),
        }
    }

    #[test]
    fn the_schema_states_the_ranges_validate_enforces() {
        type Set = fn(&mut SceneSpec, f64);
        let schema = SceneSpec::schema();
        let mut base = SceneSpec::room();
        base.floor =
            Some(FloorSpec { height: 0.0, kind: FloorKind::Slab { thickness: 1.0 }, density: 2400.0 });
        assert_eq!(base.validate(), Ok(()));
        // Every number `validate` refuses at zero: the schema must exclude zero too, and agree on
        // the largest value.
        let positive: [(&[&str], Set, f64); 7] = [
            (&["duration"], |s, v| s.duration = v, limits::DURATION),
            (&["InstrumentSpec", "tower_height"], |s, v| s.array.instrument.tower_height = v, 1.0e3),
            (&["InstrumentSpec", "ifo_separation"], |s, v| s.array.instrument.ifo_separation = v, 1.0e3),
            (&["InstrumentSpec", "launch_velocity"], |s, v| s.array.instrument.launch_velocity = v, 1.0e4),
            (&["InstrumentSpec", "cycle"], |s, v| s.array.instrument.cycle = v, limits::CYCLE),
            (&["FloorSpec", "density"], |s, v| s.floor.as_mut().unwrap().density = v, limits::DENSITY),
            (
                &["thickness"],
                |s, v| s.floor.as_mut().unwrap().kind = FloorKind::Slab { thickness: v },
                limits::POSITION,
            ),
        ];
        for (path, set, most) in positive {
            let field = if path == ["thickness"] {
                // The slab is a variant of `FloorKind`, wherever schemars puts its object.
                let kinds = schema["$defs"]["FloorKind"]["oneOf"].as_array().expect("variants").clone();
                let slab = kinds.iter().find(|k| !k["properties"]["Slab"].is_null()).expect("Slab");
                slab["properties"]["Slab"]["properties"]["thickness"].clone()
            } else {
                field_schema(&schema, path)
            };
            assert_eq!(field["exclusiveMinimum"], 0.0, "{path:?}: {field}");
            assert!(field.get("minimum").is_none(), "{path:?} must not admit 0: {field}");
            assert_eq!(field["maximum"], most, "{path:?}");
            let mut zero = base.clone();
            set(&mut zero, 0.0);
            assert!(zero.validate().is_err(), "{path:?}: validate refuses 0, as the schema says");
        }
        // Control: a bound that is inclusive is stated as inclusive, and validate accepts it.
        let half = field_schema(&schema, &["InstrumentSpec", "half_sequence"]);
        assert_eq!(half["minimum"], limits::HALF_SEQUENCE_MIN);
        assert!(half.get("exclusiveMinimum").is_none());
        let mut edge = base.clone();
        edge.array.instrument.half_sequence = limits::HALF_SEQUENCE_MIN;
        assert_eq!(edge.validate(), Ok(()));
        let height = field_schema(&schema, &["FloorSpec", "height"]);
        assert_eq!(
            (&height["minimum"], &height["maximum"]),
            (&serde_json::json!(-1.0e6), &serde_json::json!(1.0e6))
        );
        // The rules between fields are in the words, since a schema range cannot hold them.
        let text = |path: &[&str]| field_schema(&schema, path)["description"].as_str().unwrap().to_owned();
        assert!(text(&["InstrumentSpec", "ifo_separation"]).contains("Less than `tower_height`"));
        assert!(text(&["InstrumentSpec", "cycle"]).contains("`cycle` ≥ 2 × `half_sequence`"));
        assert!(text(&["InstrumentSpec", "half_sequence"]).contains("2T is at most `cycle`"));
        // And they are the rules: at equality each is refused, just inside each is accepted.
        let mut spec = base.clone();
        spec.array.instrument.ifo_separation = spec.array.instrument.tower_height;
        assert!(spec.validate().is_err());
        spec.array.instrument.ifo_separation = spec.array.instrument.tower_height - 0.01;
        assert_eq!(spec.validate(), Ok(()));
        // A cycle may be exactly a shot (the plan's AICE: 2T = 5 s, one shot every 5 s); it may
        // not be shorter.
        spec.array.instrument.cycle = 2.0 * spec.array.instrument.half_sequence;
        assert_eq!(spec.validate(), Ok(()));
        spec.array.instrument.cycle = 2.0 * spec.array.instrument.half_sequence - 0.01;
        assert!(spec.validate().is_err());
    }
}
