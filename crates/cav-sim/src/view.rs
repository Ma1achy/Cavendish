//! How a scene is looked at: camera, projection and draw style, as plain data.
//!
//! Views live beside the scene spec because saved views belong to scenes, and because the engine
//! (which owns the current views) and the renderer (which draws them) both need the type. The
//! camera matrices are the renderer's business; this is only the description.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::SceneSpec;

/// The full vertical field of view of the perspective camera, degrees.
pub const FOV_Y_DEGREES: f64 = 35.0;

/// Framing assumes the picture is at least this many times as wide as it is high.
pub const FRAMING_ASPECT: f64 = 1.25;
/// Framing stands back this much further than the tightest fit.
pub const FRAMING_MARGIN: f64 = 1.12;

/// A named, square-on or isometric direction to look from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum NamedView {
    /// From the south-east and above, at the isometric angle.
    Iso,
    /// From above, north up.
    Top,
    /// From below, north up.
    Bottom,
    /// From the north, looking south.
    North,
    /// From the south, looking north.
    South,
    /// From the east, looking west.
    East,
    /// From the west, looking east.
    West,
}

impl NamedView {
    /// Every named view, in the order menus list them.
    pub const ALL: [Self; 7] =
        [Self::Iso, Self::Top, Self::Bottom, Self::North, Self::South, Self::East, Self::West];

    /// The word used in commands and file names.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Iso => "iso",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::North => "north",
            Self::South => "south",
            Self::East => "east",
            Self::West => "west",
        }
    }

    /// The view with this name, if there is one.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.name() == word)
    }

    /// `(yaw, pitch)` in degrees. Yaw is the compass direction from the target to the camera,
    /// anticlockwise from east; pitch is the camera's elevation above the horizontal.
    #[must_use]
    pub fn angles(self) -> (f64, f64) {
        match self {
            // atan(1/√2): the angle at which the three axes foreshorten equally.
            Self::Iso => (-45.0, 35.264_389_682_754_654),
            // At ±90° of pitch the yaw decides which way is up on screen; these put north up.
            Self::Top => (-90.0, 90.0),
            Self::Bottom => (90.0, -90.0),
            Self::North => (90.0, 0.0),
            Self::South => (-90.0, 0.0),
            Self::East => (0.0, 0.0),
            Self::West => (180.0, 0.0),
        }
    }
}

/// How depth is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Projection {
    /// Things further away are smaller.
    Perspective,
    /// Parallel lines stay parallel; sizes can be measured off the picture.
    Orthographic,
}

/// How surfaces and lines are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DrawStyle {
    /// Lit surfaces.
    Shaded,
    /// Unlit, flat colour.
    Flat,
    /// Additive and see-through, depth test off.
    XRay,
    /// Edges only.
    Wireframe,
    /// Ink on cream paper.
    Paper,
}

impl DrawStyle {
    /// Every draw style, in the order menus list them.
    pub const ALL: [Self; 5] = [Self::Shaded, Self::Flat, Self::XRay, Self::Wireframe, Self::Paper];

    /// The word used in commands and file names.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shaded => "shaded",
            Self::Flat => "flat",
            Self::XRay => "xray",
            Self::Wireframe => "wireframe",
            Self::Paper => "paper",
        }
    }

    /// The style with this name, if there is one.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == word)
    }
}

/// One view of the scene: an orbit camera about a target, a projection and a draw style.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ViewSpec {
    /// The point the camera looks at and orbits: `[x, y, z]` (east, north, up), metres, in the
    /// world's frame.
    pub target: [f64; 3],
    /// Compass direction from the target to the camera, degrees anticlockwise from east.
    pub yaw: f64,
    /// Elevation of the camera above the horizontal, degrees, from −90 to 90.
    pub pitch: f64,
    /// Distance from the target to the camera, metres. In an orthographic view it sets the scale.
    pub distance: f64,
    /// Perspective or orthographic.
    pub projection: Projection,
    /// How surfaces and lines are drawn.
    pub style: DrawStyle,
    /// What colours the leaves of voxel bodies. The default is their level.
    #[serde(default)]
    pub colour: crate::voxel_view::Colouring,
    /// What is drawn over voxel bodies: their leaves' bounds, a cut. The default is nothing.
    #[serde(default)]
    pub overlay: crate::voxel_view::Overlay,
    /// A section plane through the voxel bodies: a 2-D section when the view looks square-on
    /// along its axis, a cut-away otherwise. The default is none.
    #[serde(default)]
    pub section: Option<crate::voxel_view::Section>,
    /// The labels of the bodies this view does not draw (the scene tree's eye, S19). A hidden
    /// body is still read by the towers: hiding is a view's, not the scene's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<String>,
    /// How bodies with a shape are drawn: their voxels (the default, not written), shaded by their
    /// exact fill, their own triangles, or the triangles inside the voxels' outlines
    /// ([`crate::body_draw::BodyDraw`]). A view's, like the colouring: never undone.
    #[serde(default, skip_serializing_if = "crate::body_draw::BodyDraw::is_voxels")]
    pub body_draw: crate::body_draw::BodyDraw,
    /// Which levels of detail the world is drawn at: the camera's clipmap (the default, not
    /// written), the cut tower `n` reads, or the finest any of several towers reads
    /// ([`crate::world::lod_view::LodView`]). A view's: never undone.
    #[serde(default, skip_serializing_if = "crate::world::lod_view::LodView::is_camera")]
    pub lod_view: crate::world::lod_view::LodView,
}

/// What a pane shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum PaneContent {
    /// The scene in three dimensions.
    #[default]
    Scene,
    /// Every tower's signal against time, with the playhead.
    Signal,
    /// The periodogram of a tower's signal, with each oscillating body's comb of harmonics drawn
    /// where they fold to.
    Periodogram,
    /// The atoms' flights: each tower's arms as height against absolute time, one row a tower,
    /// so that towers on their own clocks are seen side by side.
    Flight,
    /// The cadence designer: the spectral window of the schedule, one realisation with the
    /// lines and the floor the closed forms predict.
    Cadence,
}

impl PaneContent {
    /// The word the console uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Signal => "signal",
            Self::Periodogram => "periodogram",
            Self::Flight => "flight",
            Self::Cadence => "cadence",
        }
    }

    /// Every content, in the order the application offers them.
    pub const ALL: [Self; 5] = [Self::Scene, Self::Signal, Self::Periodogram, Self::Flight, Self::Cadence];

    /// The content a word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == word)
    }
}

/// What to draw: a scene at a time, how to look at it, and what is selected. The engine makes it
/// ([`cav-engine`'s `Engine::frame`]) and the renderer draws it, so every client's picture is of
/// the same thing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneView<'a> {
    /// The scene to draw: its array, room, floor and bodies, in the world's frame.
    pub scene: &'a SceneSpec,
    /// The camera, projection and style to draw it with.
    pub view: &'a ViewSpec,
    /// What is selected, drawn highlighted.
    pub highlight: Option<crate::Target>,
    /// The playhead, seconds on the scene's clock: bodies are drawn where they are at this time.
    pub time: f64,
    /// What the pane shows. A renderer of scenes draws [`PaneContent::Scene`] and must refuse the
    /// others rather than draw the scene in their place.
    pub content: PaneContent,
    /// The stretch of a plot's horizontal axis the pane shows, `[from, to]` in the axis's own
    /// unit: seconds on the scene's clock for a signal or a flight diagram, hertz for a
    /// periodogram or a cadence window. `None` for all of it. A scene pane has no span.
    pub span: Option<[f64; 2]>,
    /// The voxel bodies as this pane draws them (their cubes, overlays and legend), when the
    /// scene has any; the renderer draws other bodies as before.
    pub voxels: Option<&'a crate::voxel_view::VoxelFrame>,
    /// The resident world (S4) the pane draws its clipmap from, when the scene has a world.
    pub world: Option<&'a crate::world::clipmap::WorldFrame>,
    /// What the pane draws of the world's levels: its LOD view's draw list and its world
    /// overlay's boxes ([`crate::world::lod_view::LodFrame`]), when the scene has a world.
    pub lod: Option<&'a crate::world::lod_view::LodFrame>,
}

impl ViewSpec {
    /// The view from `named` that frames the whole scene.
    #[must_use]
    pub fn framing(scene: &SceneSpec, named: NamedView, projection: Projection, style: DrawStyle) -> Self {
        let (yaw, pitch) = named.angles();
        let mut view = Self {
            target: [0.0; 3],
            yaw,
            pitch,
            distance: 1.0,
            projection,
            style,
            colour: crate::voxel_view::Colouring::default(),
            overlay: crate::voxel_view::Overlay::default(),
            section: None,
            hidden: Vec::new(),
            body_draw: crate::body_draw::BodyDraw::Voxels,
            lod_view: crate::world::lod_view::LodView::Camera,
        };
        view.frame(scene);
        view
    }

    /// Moves the target to the middle of the scene and stands back just far enough to see all of
    /// it from the present direction, in the present projection, with a margin. The picture is
    /// taken to be at least [`FRAMING_ASPECT`] times as wide as it is high.
    pub fn frame(&mut self, scene: &SceneSpec) {
        let (lo, hi) = scene.bounds();
        let half = [0.5 * (hi[0] - lo[0]), 0.5 * (hi[1] - lo[1]), 0.5 * (hi[2] - lo[2])];
        self.target = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])];
        let (to_eye, right, up) = self.basis();
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let slope = (0.5 * FOV_Y_DEGREES).to_radians().tan();
        // Each corner of the bounding box must be inside the picture. A corner `across` metres off
        // the axis and `nearer` metres towards the camera needs across ≤ (distance − nearer)·slope
        // in perspective; in an orthographic view its depth does not matter.
        let mut distance = 0.5 / slope;
        for corner in 0..8 {
            let sign = |bit: u32| if corner & bit == 0 { -1.0 } else { 1.0 };
            let offset = [sign(1) * half[0], sign(2) * half[1], sign(4) * half[2]];
            let across = dot(offset, up).abs().max(dot(offset, right).abs() / FRAMING_ASPECT);
            let nearer = if self.projection == Projection::Perspective { dot(offset, to_eye) } else { 0.0 };
            distance = f64::max(distance, across / slope + nearer);
        }
        self.distance = FRAMING_MARGIN * distance;
    }

    /// The basis of the camera from its angles, `(to_eye, right, up)`: the unit vectors from the
    /// target towards the camera, to the right of the picture and up it. Each is `[x, y, z]`
    /// (east, north, up) in the world's frame, dimensionless. Defined at the poles too, where yaw
    /// says which way is up.
    #[must_use]
    pub fn basis(&self) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let (yaw, pitch) = (self.yaw.to_radians(), self.pitch.to_radians());
        let to_eye = [pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()];
        let right = [-yaw.sin(), yaw.cos(), 0.0];
        let up = [
            to_eye[1] * right[2] - to_eye[2] * right[1],
            to_eye[2] * right[0] - to_eye[0] * right[2],
            to_eye[0] * right[1] - to_eye[1] * right[0],
        ];
        (to_eye, right, up)
    }

    /// The scene's home view: isometric, perspective, shaded.
    #[must_use]
    pub fn home(scene: &SceneSpec) -> Self {
        Self::framing(scene, NamedView::Iso, Projection::Perspective, DrawStyle::Shaded)
    }

    /// The named view this camera is at, if it is exactly at one.
    #[must_use]
    pub fn named(&self) -> Option<NamedView> {
        NamedView::ALL.into_iter().find(|v| v.angles() == (self.yaw, self.pitch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_parse_back_to_their_views_and_styles() {
        for view in NamedView::ALL {
            assert_eq!(NamedView::parse(view.name()), Some(view));
        }
        for style in DrawStyle::ALL {
            assert_eq!(DrawStyle::parse(style.name()), Some(style));
        }
        // Control: near misses are not accepted.
        assert_eq!(NamedView::parse("Top"), None);
        assert_eq!(NamedView::parse("up"), None);
        assert_eq!(DrawStyle::parse("x-ray"), None);
    }

    /// How much of the half-height and half-width of the picture (at `FRAMING_ASPECT`) the
    /// bounding box of the scene takes up in `view`: the largest over its corners, 1 being the edge.
    fn fill(scene: &SceneSpec, view: &ViewSpec) -> f64 {
        let (lo, hi) = scene.bounds();
        let (to_eye, right, up) = view.basis();
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let slope = (0.5 * FOV_Y_DEGREES).to_radians().tan();
        let mut worst = 0f64;
        for corner in 0..8u32 {
            let pick = |axis: usize| if corner & (1 << axis) == 0 { lo[axis] } else { hi[axis] };
            let offset = [pick(0) - view.target[0], pick(1) - view.target[1], pick(2) - view.target[2]];
            let depth = match view.projection {
                Projection::Perspective => view.distance - dot(offset, to_eye),
                Projection::Orthographic => view.distance,
            };
            assert!(depth > 0.0, "a corner is behind the camera");
            let reach = depth * slope;
            worst = worst
                .max(dot(offset, up).abs() / reach)
                .max(dot(offset, right).abs() / (reach * FRAMING_ASPECT));
        }
        worst
    }

    #[test]
    fn framing_shows_the_whole_scene_and_fills_the_picture() {
        let room = SceneSpec::room();
        let void = SceneSpec { room: None, ..SceneSpec::room() };
        for scene in [&room, &void] {
            for named in NamedView::ALL {
                for projection in [Projection::Perspective, Projection::Orthographic] {
                    let view = ViewSpec::framing(scene, named, projection, DrawStyle::Shaded);
                    let (lo, hi) = scene.bounds();
                    assert_eq!(
                        view.target,
                        [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])]
                    );
                    assert_eq!(view.named(), Some(named));
                    // Everything is in view with at least the margin to spare. In an orthographic
                    // view the furthest-out corner sits exactly at 1/margin of the way to the edge;
                    // in perspective the margin also scales the corner's depth, so it sits a
                    // little inside that, and never so far in that the scene is lost.
                    let filled = fill(scene, &view);
                    let limit = 1.0 / FRAMING_MARGIN;
                    match projection {
                        Projection::Orthographic => {
                            assert!((filled - limit).abs() < 1e-9, "{named:?} orthographic: fills {filled}");
                        }
                        Projection::Perspective => {
                            assert!(
                                filled <= limit + 1e-9 && filled > 0.8,
                                "{named:?} perspective: fills {filled}"
                            );
                        }
                    }
                }
            }
        }
        // Control arms: the measure sees a camera that stands too close or too far.
        let home = ViewSpec::home(&room);
        assert!(
            fill(&room, &ViewSpec { distance: 0.7 * home.distance, ..home.clone() }) > 1.0,
            "too close clips the scene"
        );
        assert!(fill(&room, &ViewSpec { distance: 3.0 * home.distance, ..home.clone() }) < 0.4);
        // And one nudged off a named direction is no longer at a named view.
        assert_eq!(ViewSpec { yaw: home.yaw + 1.0, ..home }.named(), None);
    }

    #[test]
    fn the_basis_is_orthonormal_and_north_is_up_in_plan() {
        let mut view = ViewSpec::home(&SceneSpec::room());
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        for (yaw, pitch) in [(0.0, 0.0), (37.0, 12.5), (-170.0, -89.0), (-90.0, 90.0), (90.0, -90.0)] {
            (view.yaw, view.pitch) = (yaw, pitch);
            let (e, r, u) = view.basis();
            for v in [e, r, u] {
                assert!((dot(v, v) - 1.0).abs() < 1e-12);
            }
            assert!(dot(e, r).abs() < 1e-12 && dot(e, u).abs() < 1e-12 && dot(r, u).abs() < 1e-12);
        }
        // From above with north up: east is to the right and north is up the picture.
        (view.yaw, view.pitch) = NamedView::Top.angles();
        let (e, r, u) = view.basis();
        let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-12);
        assert!(
            near(e, [0.0, 0.0, 1.0]) && near(r, [1.0, 0.0, 0.0]) && near(u, [0.0, 1.0, 0.0]),
            "{e:?} {r:?} {u:?}"
        );
        // Control: from below, east is to the left.
        (view.yaw, view.pitch) = NamedView::Bottom.angles();
        assert!(near(view.basis().1, [-1.0, 0.0, 0.0]));
    }
}
