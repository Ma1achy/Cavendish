//! Cavendish simulation: scenes as data, and what the instrument reads from them. The scene spec
//! with its schema and paths, motions, presets, the schedule and its predicted spectral window,
//! the read of a scene into a signal, bundles, the view spec and the built-in scene library.
//! Voxel bodies (S3) and the world volume of static matter (S4, [`world`]); dynamics and fluids
//! arrive with their slices.
//!
//! ## What this crate is for
//! What a scene *is* and what the instrument reads from it: the scene spec, its validation and its
//! built-in library; motions; edits by path with their units and stages; the schedule of every
//! tower's shots; presets; the read (the forward model applied to a scene); bundles; and the view
//! spec that says how a scene is looked at.
//!
//! ## What it may depend on
//! Of the workspace, `cav-core` only (`cav-validate` in its tests).
//! It must not hold commands, queries, history or caching (those are the engine's), nor anything
//! that draws or touches a GPU.

#![warn(missing_docs)]

pub mod bundle;
pub mod dev;
pub mod json_f64;
pub mod library;
pub mod motion;
pub mod paths;
pub mod presets;
pub mod read;
pub mod schedule;
pub mod shape;
pub mod spec;
pub mod starters;
pub mod view;
pub mod voxel_view;
pub mod world;

pub use library::{LIBRARY, LibraryScene, inline, load};
pub use motion::{ModeSpec, Motion};
pub use shape::{Axis, PlatonicSolid, ShapeKind, ShapeSpec};
pub use spec::{
    ArraySpec, BodySpec, FloorKind, FloorSpec, GapSpec, InstrumentOverride, InstrumentSpec, JitterKind,
    JitterModel, JitterSpec, RoomSpec, SPEC_VERSION, SceneSpec, ScheduleSpec, SignSpec, SpecError, Target,
    TimingSpec, TowerSpec, WindowSpec,
};
pub use view::{
    DrawStyle, FOV_Y_DEGREES, FRAMING_ASPECT, FRAMING_MARGIN, NamedView, PaneContent, Projection, SceneView,
    ViewSpec,
};
