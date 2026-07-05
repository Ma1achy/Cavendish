//! Scene gizmos: the world axes (coloured arrows + ticks + labels), detector wireframe cages, the
//! body's oriented bounding box, and the spin-axis arrow. Pure assembly into [`SceneData`] — the
//! drawing is the renderer's, the numbers are the bundle's. Each is a toggle the App composes in.

use state::{StateBundle, Vec3};

use crate::pose::pose_of;
use crate::render::SceneData;

pub const X_COL: [f32; 3] = [0.9, 0.3, 0.3];
pub const Y_COL: [f32; 3] = [0.35, 0.85, 0.4];
pub const Z_COL: [f32; 3] = [0.4, 0.55, 1.0];
pub const DETECTOR_COL: [f32; 3] = [1.0, 0.62, 0.2];
pub const BODY_COL: [f32; 3] = [0.85, 0.87, 0.95];
pub const SPIN_COL: [f32; 3] = [1.0, 0.2, 0.2];

/// The 8 corners of the box `[min, max]` in the order `i = x + 2y + 4z` (see `render::BOX_EDGES`).
fn box_corners(min: [f64; 3], max: [f64; 3]) -> [[f64; 3]; 8] {
    std::array::from_fn(|i| {
        [
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        ]
    })
}

/// The two axis indices perpendicular to axis `k`.
fn perp(k: usize) -> (usize, usize) {
    match k {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

fn tick_label(v: f64) -> String {
    if (v - v.round()).abs() < 1e-6 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

/// World axes: an arrow per axis (X red, Y green, Z blue) to `len`, tick cross-marks every `step`, and
/// the axis letter at each tip plus a numeric label at each tick.
pub fn push_axes(scene: &mut SceneData, len: f64, step: f64) {
    for (k, col, name) in [(0usize, X_COL, "X"), (1, Y_COL, "Y"), (2, Z_COL, "Z")] {
        let mut tip = [0.0; 3];
        tip[k] = len;
        scene.push_arrow([0.0; 3], tip, col);
        scene.push_label(tip, name, col);

        let (a, b) = perp(k);
        let t = 0.03 * len;
        let mut d = step;
        while d <= len + 1e-9 {
            let mut at = [0.0; 3];
            at[k] = d;
            let (mut p0, mut p1) = (at, at);
            p0[a] -= t;
            p1[a] += t;
            scene.push_line(p0, p1, col);
            let (mut q0, mut q1) = (at, at);
            q0[b] -= t;
            q1[b] += t;
            scene.push_line(q0, q1, col);
            scene.push_label(at, tick_label(d), col);
            d += step;
        }
    }
}

/// A small wireframe cube at each detector placement (`[x, y, z, …]` — only the position is used).
pub fn push_detector_cages(scene: &mut SceneData, placements: &[[f64; 7]], half: f64) {
    for c in placements {
        let corners = box_corners(
            [c[0] - half, c[1] - half, c[2] - half],
            [c[0] + half, c[1] + half, c[2] + half],
        );
        scene.push_box_wireframe(&corners, DETECTOR_COL);
    }
}

/// The body's **oriented bounding box** at scrub `ℓ`: the AABB of body cloud `s` in its own frame,
/// posed into the world by `pose_of` — so the wireframe tracks the tumble. No-op if out of range.
pub fn push_body_wireframe(scene: &mut SceneData, bundle: &StateBundle, s: usize, l: usize) {
    let Some(cloud) = bundle.source_cloud.get(s) else {
        return;
    };
    if cloud.is_empty() || bundle.source_position.get(s).map_or(0, |t| t.len()) <= l {
        return;
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for e in cloud {
        for k in 0..3 {
            min[k] = min[k].min(e[k]);
            max[k] = max[k].max(e[k]);
        }
    }
    let pose = pose_of(bundle, s, l);
    let world: [[f64; 3]; 8] = box_corners(min, max).map(|c| {
        let w = pose.apply(Vec3::new(c[0], c[1], c[2]));
        [w.x, w.y, w.z]
    });
    scene.push_box_wireframe(&world, BODY_COL);
}

/// A red arrow from the body's CoM along its angular velocity at `ℓ` (length `len`). No-op if the body
/// is not rotating (‖ω‖ = 0) or the index is out of range.
pub fn push_spin_axis(scene: &mut SceneData, bundle: &StateBundle, s: usize, l: usize, len: f64) {
    let Some(com) = bundle.source_position.get(s).and_then(|t| t.get(l)) else {
        return;
    };
    let Some(w) = bundle.source_angular_velocity.get(s).and_then(|t| t.get(l)) else {
        return;
    };
    let mag = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    if mag <= 0.0 {
        return;
    }
    let tip = [
        com[0] + w[0] / mag * len,
        com[1] + w[1] / mag * len,
        com[2] + w[2] / mag * len,
    ];
    scene.push_arrow(*com, tip, SPIN_COL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes_have_three_labelled_arrows() {
        let mut scene = SceneData::new();
        push_axes(&mut scene, 5.0, 1.0);
        let names: Vec<&str> = scene.labels.iter().map(|l| l.text.as_str()).collect();
        assert!(names.contains(&"X") && names.contains(&"Y") && names.contains(&"Z"));
        assert!(!scene.lines.is_empty(), "arrows + ticks drawn");
    }

    #[test]
    fn detector_cages_are_boxes() {
        let mut scene = SceneData::new();
        let dets = [
            [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0],
        ];
        push_detector_cages(&mut scene, &dets, 0.1);
        assert_eq!(scene.lines.len(), 2 * 24, "two 12-edge cages");
    }

    #[test]
    fn body_wireframe_posed() {
        // The wireframe corners equal the AABB corners under the bundle pose (a 90° turn about z).
        let angle = std::f64::consts::FRAC_PI_2;
        let q = [(angle * 0.5).cos(), 0.0, 0.0, (angle * 0.5).sin()];
        let bundle = StateBundle {
            source_cloud: vec![vec![[1.0, 0.5, 0.2, 1.0], [-1.0, -0.5, -0.2, 1.0]]],
            source_orientation: vec![vec![q]],
            source_position: vec![vec![[10.0, 0.0, 0.0]]],
            ..Default::default()
        };
        let mut scene = SceneData::new();
        push_body_wireframe(&mut scene, &bundle, 0, 0);
        assert_eq!(scene.lines.len(), 24, "12 edges");
        // Corner i=7 is body-max (1, 0.5, 0.2); a +90° z-turn sends (x,y)→(−y,x), then +position.
        let want = [10.0 - 0.5, 0.0 + 1.0, 0.2];
        // The first vertex of edge (5,7) is corner 5; find any line vertex matching the posed max.
        let got = scene.lines.iter().any(|v| {
            (v.pos[0] as f64 - want[0]).abs() < 1e-5 && (v.pos[1] as f64 - want[1]).abs() < 1e-5
        });
        assert!(got, "posed max corner present");
    }

    #[test]
    fn spin_axis_only_when_rotating() {
        let mut still = StateBundle {
            source_position: vec![vec![[0.0, 0.0, 0.0]]],
            source_angular_velocity: vec![vec![[0.0, 0.0, 0.0]]],
            ..Default::default()
        };
        let mut scene = SceneData::new();
        push_spin_axis(&mut scene, &still, 0, 0, 1.0);
        assert!(scene.lines.is_empty(), "no arrow when ω = 0");

        still.source_angular_velocity = vec![vec![[0.0, 0.0, 2.0]]];
        push_spin_axis(&mut scene, &still, 0, 0, 1.0);
        assert!(!scene.lines.is_empty(), "arrow when spinning");
    }
}
