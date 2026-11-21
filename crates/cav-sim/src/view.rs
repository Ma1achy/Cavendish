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
    /// The point the camera looks at and orbits, metres.
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
}

impl ViewSpec {
    /// The view from `named` that frames the whole scene.
    #[must_use]
    pub fn framing(scene: &SceneSpec, named: NamedView, projection: Projection, style: DrawStyle) -> Self {
        let (lo, hi) = scene.bounds();
        let target = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1]), 0.5 * (lo[2] + hi[2])];
        let radius = 0.5 * (0..3).map(|a| (hi[a] - lo[a]).powi(2)).sum::<f64>().sqrt();
        // The bounding sphere just fits the vertical field of view, with a margin.
        let distance = 1.02 * radius.max(0.5) / (0.5 * FOV_Y_DEGREES).to_radians().sin();
        let (yaw, pitch) = named.angles();
        Self { target, yaw, pitch, distance, projection, style }
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

    #[test]
    fn framing_centres_on_the_scene_and_stands_back_far_enough() {
        let scene = SceneSpec::room();
        let home = ViewSpec::home(&scene);
        assert_eq!(home.target, [0.0, 0.0, 5.0]);
        assert_eq!(home.named(), Some(NamedView::Iso));
        // The bounding sphere (radius √75 m) subtends less than the field of view.
        let radius = 75f64.sqrt();
        let half_angle = (radius / home.distance).asin().to_degrees();
        assert!(half_angle < 0.5 * FOV_Y_DEGREES, "half angle {half_angle}");
        assert!(half_angle > 0.45 * FOV_Y_DEGREES, "and it is not lost in the distance: {half_angle}");
        // Control: a camera nudged off a named direction is no longer at a named view.
        let nudged = ViewSpec { yaw: home.yaw + 1.0, ..home };
        assert_eq!(nudged.named(), None);
    }
}
