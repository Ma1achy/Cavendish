//! From a [`ViewSpec`] to matrices, and the layout of panes in a figure.
//!
//! The camera works in a local frame whose origin is the view's target (REBUILD §3.4): the matrix
//! it hands the GPU takes offsets from that origin, never world positions, so a scene 10 km from
//! the world origin is drawn with the same `f32` numbers as one at it.

use cav_core::{Mat4, Vec3};
use cav_sim::{FOV_Y_DEGREES, Projection, ViewSpec};

/// A camera for one view at one aspect ratio.
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    /// The local frame's origin in the world: the view's target.
    pub origin: Vec3,
    /// From the target to the camera, in the local frame.
    pub eye: Vec3,
    /// Unit vector the camera looks along.
    pub forward: Vec3,
    /// Unit vector to the right of the picture.
    pub right: Vec3,
    /// Unit vector up the picture.
    pub up: Vec3,
    /// Local offsets → clip space.
    pub view_proj: Mat4,
    /// Distance to the far plane, metres.
    pub far: f64,
}

impl Camera {
    /// The camera of `view` for a picture `aspect` times as wide as it is high.
    #[must_use]
    pub fn new(view: &ViewSpec, aspect: f64) -> Self {
        let (yaw, pitch) = (view.yaw.to_radians(), view.pitch.to_radians());
        // The basis is built from the angles, not from a look-at with a fixed "up", so that the
        // views from straight above and below are well defined: yaw then says which way is up.
        let to_eye = Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin());
        let right = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
        let up = to_eye.cross(right);
        let forward = -to_eye;
        let eye = to_eye * view.distance;
        let view_matrix = Mat4 {
            m: [
                [right.x, up.x, to_eye.x, 0.0],
                [right.y, up.y, to_eye.y, 0.0],
                [right.z, up.z, to_eye.z, 0.0],
                [-right.dot(eye), -up.dot(eye), -to_eye.dot(eye), 1.0],
            ],
        };
        let far = (view.distance * 100.0).max(5000.0);
        let projection = match view.projection {
            Projection::Perspective => {
                let near = (view.distance * 0.01).max(0.02);
                Mat4::perspective(FOV_Y_DEGREES.to_radians(), aspect, near, far)
            }
            // The same apparent size at the target as the perspective camera, so switching
            // projection does not jump. The slab reaches behind the camera too: nothing in an
            // orthographic drawing should vanish because the camera stands inside it.
            Projection::Orthographic => {
                let half_height = view.distance * (0.5 * FOV_Y_DEGREES).to_radians().tan();
                Mat4::orthographic(half_height, aspect, -far, far)
            }
        };
        Self {
            origin: Vec3::from_array(view.target),
            eye,
            forward,
            right,
            up,
            view_proj: projection * view_matrix,
            far,
        }
    }

    /// Where a world point lands in a picture of `size` pixels: `(x, y)` from the top-left corner,
    /// or `None` when the point is behind the camera.
    #[must_use]
    pub fn pixel(&self, world: Vec3, size: [u32; 2]) -> Option<[f64; 2]> {
        let ndc = self.view_proj.project(world - self.origin)?;
        Some([(0.5 + 0.5 * ndc.x) * f64::from(size[0]), (0.5 - 0.5 * ndc.y) * f64::from(size[1])])
    }
}

/// A rectangle of whole pixels inside a figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaneRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// How a figure of `size` pixels is split into `panes` panes (1, 2 or 4), with `gap` pixels between
/// neighbours: two side by side, four as a 2 × 2 grid in reading order.
///
/// # Panics
/// When `panes` is not 1, 2 or 4.
#[must_use]
pub fn pane_rects(panes: usize, size: [u32; 2], gap: u32) -> Vec<PaneRect> {
    let [width, height] = size;
    // An extent cut into two parts with the gap between them: `(first, where the second starts,
    // second)`. An extent smaller than the gap gives two empty parts that still lie inside it.
    let split = |extent: u32| {
        let gap = gap.min(extent);
        let first = (extent - gap) / 2;
        (first, first + gap, extent - gap - first)
    };
    match panes {
        1 => vec![PaneRect { x: 0, y: 0, width, height }],
        2 => {
            let (left, x1, right) = split(width);
            vec![PaneRect { x: 0, y: 0, width: left, height }, PaneRect { x: x1, y: 0, width: right, height }]
        }
        4 => {
            let (left, x1, right) = split(width);
            let (top, y1, bottom) = split(height);
            vec![
                PaneRect { x: 0, y: 0, width: left, height: top },
                PaneRect { x: x1, y: 0, width: right, height: top },
                PaneRect { x: 0, y: y1, width: left, height: bottom },
                PaneRect { x: x1, y: y1, width: right, height: bottom },
            ]
        }
        other => panic!("a figure has 1, 2 or 4 panes, not {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cav_sim::{DrawStyle, NamedView, SceneSpec};

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() <= 1e-12
    }

    #[test]
    fn named_views_look_along_their_axes() {
        let scene = SceneSpec::room();
        // (view, the way the camera looks, what is to the right of the picture, what is up it)
        let rows = [
            (NamedView::Top, -Vec3::Z, Vec3::X, Vec3::Y),
            (NamedView::Bottom, Vec3::Z, -Vec3::X, Vec3::Y),
            (NamedView::North, -Vec3::Y, -Vec3::X, Vec3::Z),
            (NamedView::South, Vec3::Y, Vec3::X, Vec3::Z),
            (NamedView::East, -Vec3::X, Vec3::Y, Vec3::Z),
            (NamedView::West, Vec3::X, -Vec3::Y, Vec3::Z),
        ];
        for projection in [Projection::Perspective, Projection::Orthographic] {
            for (named, forward, right, up) in rows {
                let view = ViewSpec::framing(&scene, named, projection, DrawStyle::Shaded);
                let camera = Camera::new(&view, 1.5);
                assert!(close(camera.forward, forward), "{named:?} looks along {:?}", camera.forward);
                assert!(close(camera.right, right), "{named:?} has {:?} on the right", camera.right);
                assert!(close(camera.up, up), "{named:?} has {:?} up", camera.up);
                // The picture agrees with the basis: the target is at the centre, and a step along
                // `right` and `up` moves right and up on screen (y grows downwards in pixels).
                let size = [900, 600];
                let centre = camera.pixel(camera.origin, size).unwrap();
                assert!((centre[0] - 450.0).abs() < 1e-9 && (centre[1] - 300.0).abs() < 1e-9);
                let stepped = camera.pixel(camera.origin + right + up, size).unwrap();
                assert!(stepped[0] > centre[0] + 1.0 && stepped[1] < centre[1] - 1.0, "{named:?}");
            }
        }
        // The isometric view sees three faces equally: its direction has equal parts along each axis.
        let iso = Camera::new(&ViewSpec::home(&scene), 1.0);
        let third = 1.0 / 3f64.sqrt();
        assert!(close(iso.forward, Vec3::new(-third, third, -third)), "{:?}", iso.forward);
        // Control: the rows are not interchangeable. East's basis is wrong for North.
        let north = Camera::new(
            &ViewSpec::framing(&scene, NamedView::North, Projection::Perspective, DrawStyle::Shaded),
            1.0,
        );
        assert!(!close(north.forward, -Vec3::X));
    }

    #[test]
    fn the_basis_is_orthonormal_at_any_angle() {
        let mut view = ViewSpec::home(&SceneSpec::room());
        for (yaw, pitch) in [(0.0, 0.0), (37.0, 12.5), (-170.0, -89.0), (90.0, 90.0), (123.0, -90.0)] {
            (view.yaw, view.pitch) = (yaw, pitch);
            let c = Camera::new(&view, 1.0);
            for v in [c.forward, c.right, c.up] {
                assert!((v.length() - 1.0).abs() < 1e-12);
            }
            assert!(c.forward.dot(c.right).abs() < 1e-12 && c.forward.dot(c.up).abs() < 1e-12);
            // Right-handed: right × up points back at the camera.
            assert!(close(c.right.cross(c.up), -c.forward));
            assert!((c.eye.length() - view.distance).abs() < 1e-9);
        }
    }

    #[test]
    fn switching_projection_keeps_the_size_at_the_target() {
        let scene = SceneSpec::room();
        let persp = ViewSpec::framing(&scene, NamedView::South, Projection::Perspective, DrawStyle::Flat);
        let ortho = ViewSpec { projection: Projection::Orthographic, ..persp.clone() };
        let size = [800, 600];
        let (p, o) = (Camera::new(&persp, 4.0 / 3.0), Camera::new(&ortho, 4.0 / 3.0));
        // A point in the plane of the target, 3 m to the right and 2 m up, lands on the same pixel.
        let at_target = p.origin + p.right * 3.0 + p.up * 2.0;
        let (a, b) = (p.pixel(at_target, size).unwrap(), o.pixel(at_target, size).unwrap());
        assert!((a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9, "{a:?} vs {b:?}");
        // Control: a point nearer the camera differs, as perspective must.
        let nearer = at_target - p.forward * 4.0;
        let (a, b) = (p.pixel(nearer, size).unwrap(), o.pixel(nearer, size).unwrap());
        assert!(a[0] - b[0] > 5.0, "perspective enlarges what is nearer: {a:?} vs {b:?}");
    }

    #[test]
    fn pane_layouts_tile_the_figure() {
        for size in [[1440, 900], [1023, 701], [3, 3], [640, 1]] {
            for gap in [0, 1, 2] {
                for panes in [1, 2, 4] {
                    let rects = pane_rects(panes, size, gap);
                    assert_eq!(rects.len(), panes);
                    // Every pixel of the figure is in at most one pane; with no gap, in exactly one.
                    let mut cover = vec![0u8; (size[0] * size[1]) as usize];
                    for r in &rects {
                        assert!(r.x + r.width <= size[0] && r.y + r.height <= size[1], "{r:?} in {size:?}");
                        for y in r.y..r.y + r.height {
                            for x in r.x..r.x + r.width {
                                cover[(y * size[0] + x) as usize] += 1;
                            }
                        }
                    }
                    assert!(cover.iter().all(|&c| c <= 1), "panes overlap: {panes} in {size:?}, gap {gap}");
                    let uncovered = cover.iter().filter(|&&c| c == 0).count() as u32;
                    if gap == 0 {
                        assert_eq!(uncovered, 0, "{panes} panes leave a hole in {size:?}");
                    } else if size[0] > 2 * gap && size[1] > 2 * gap {
                        // Only the gaps are uncovered: one column for two panes, a cross for four.
                        let expected = match panes {
                            1 => 0,
                            2 => gap * size[1],
                            _ => gap * size[1] + gap * size[0] - gap * gap,
                        };
                        assert_eq!(uncovered, expected, "{panes} panes in {size:?}, gap {gap}");
                    }
                }
            }
        }
        // Reading order: the second of four panes is top right.
        let four = pane_rects(4, [100, 80], 0);
        assert_eq!(four[1], PaneRect { x: 50, y: 0, width: 50, height: 40 });
        assert_eq!(four[2], PaneRect { x: 0, y: 40, width: 50, height: 40 });
    }

    #[test]
    #[should_panic(expected = "1, 2 or 4 panes")]
    fn three_panes_is_not_a_layout() {
        let _ = pane_rects(3, [100, 100], 0);
    }
}
