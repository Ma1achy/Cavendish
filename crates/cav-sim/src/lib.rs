//! Cavendish simulation: in S0, the scene spec, the view spec and the built-in scene library.
//! Bodies, paths, dynamics, fluids and bundles arrive with their slices.

pub mod dev;
pub mod json_f64;
pub mod library;
pub mod motion;
pub mod paths;
pub mod read;
pub mod spec;
pub mod view;

pub use library::{LIBRARY, LibraryScene, inline, load};
pub use motion::Motion;
pub use spec::{
    ArraySpec, BodySpec, FloorKind, FloorSpec, InstrumentSpec, RoomSpec, SPEC_VERSION, SceneSpec, SignSpec,
    SpecError, Target, TimingSpec, TowerSpec,
};
pub use view::{
    DrawStyle, FOV_Y_DEGREES, FRAMING_ASPECT, FRAMING_MARGIN, NamedView, PaneContent, Projection, SceneView,
    ViewSpec,
};
