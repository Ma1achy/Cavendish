//! Cavendish simulation: in S0, the scene spec, the view spec and the built-in scene library.
//! Bodies, paths, dynamics, fluids and bundles arrive with their slices.

pub mod dev;
pub mod library;
pub mod motion;
pub mod spec;
pub mod view;

pub use library::{LIBRARY, LibraryScene, load};
pub use motion::Motion;
pub use spec::{
    ArraySpec, BodySpec, FloorKind, FloorSpec, InstrumentSpec, RoomSpec, SPEC_VERSION, SceneSpec, SignSpec,
    SpecError, Target, TowerSpec,
};
pub use view::{
    DrawStyle, FOV_Y_DEGREES, FRAMING_ASPECT, FRAMING_MARGIN, NamedView, Projection, SceneView, ViewSpec,
};
