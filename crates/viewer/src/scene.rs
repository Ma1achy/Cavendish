//! Assemble the voxel body for scrub index `ℓ`: each cloud element posed into the world frame and
//! drawn as a shaded cube. The detectors, world axes, body box and spin-axis arrow are gizmos
//! ([`crate::gizmo`]); the on-demand field slice is below. One place maps the bundle's tensors to
//! drawables, so the live App and the headless test draw the same thing from the same data.

use state::StateBundle;

use crate::field_slice::{sample_slice, Plane};
use crate::pose::{pose_of, world_vertices};
use crate::render::SceneData;

const CLOUD_COL: [f32; 3] = [0.55, 0.72, 1.0];

/// The voxel half-extent for a cloud: about half a lattice pitch, `pitch ≈ (bbox_volume / n)^⅓`, so a
/// filled voxel grid reads as a solid body. A degenerate/tiny cloud falls back to a small default.
pub fn voxel_half(cloud: &[[f64; 4]]) -> f32 {
    let n = cloud.len();
    if n == 0 {
        return 0.02;
    }
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for e in cloud {
        for k in 0..3 {
            min[k] = min[k].min(e[k]);
            max[k] = max[k].max(e[k]);
        }
    }
    let extent = |k: usize| (max[k] - min[k]).max(1e-6);
    let pitch = (extent(0) * extent(1) * extent(2) / n as f64).cbrt();
    (0.48 * pitch).max(1e-3) as f32
}

/// A camera target and orbit radius that frame the whole scene — the detectors and the posed body at
/// `ℓ = 0`. Centres on their centroid and fits the extent (with margin) to the vertical field of view,
/// so both the body and the array are visible whatever the scenario's standoff. Empty ⇒ a default.
pub fn frame_scene(bundle: &StateBundle) -> ([f32; 3], f32) {
    let mut pts: Vec<[f64; 3]> = bundle
        .detector_placement
        .iter()
        .map(|d| [d[0], d[1], d[2]])
        .collect();
    for (s, cloud) in bundle.source_cloud.iter().enumerate() {
        if bundle.source_position.get(s).map_or(0, |t| t.len()) > 0 {
            pts.extend(world_vertices(cloud, &pose_of(bundle, s, 0)));
        }
    }
    if pts.is_empty() {
        return ([0.0, 0.0, 0.0], 12.0);
    }
    let mut c = [0.0; 3];
    for p in &pts {
        for k in 0..3 {
            c[k] += p[k];
        }
    }
    for v in &mut c {
        *v /= pts.len() as f64;
    }
    let mut r2 = 0.0f64;
    for p in &pts {
        let d = (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2);
        r2 = r2.max(d);
    }
    let radius = (r2.sqrt() * 2.2).max(2.0);
    ([c[0] as f32, c[1] as f32, c[2] as f32], radius as f32)
}

/// The posed voxel body at `ℓ`: each source cloud posed by `pose_of` and drawn as cubes.
pub fn scene_at(bundle: &StateBundle, l: usize) -> SceneData {
    let mut scene = SceneData::new();
    for (s, cloud) in bundle.source_cloud.iter().enumerate() {
        let poses = bundle.source_position.get(s).map_or(0, |t| t.len());
        if l >= poses {
            continue;
        }
        let pose = pose_of(bundle, s, l);
        let verts = world_vertices(cloud, &pose);
        scene.push_cubes(&verts, voxel_half(cloud), CLOUD_COL);
    }
    scene
}

/// Overlay the on-demand field slice for source 0 at `ℓ`: a plane through the CoM, each node a dim dot
/// (tiny cube) with a bright line along `ĝ` — a crude arrow field, sampling `gravity::field` on demand
/// (never the stored grid). Off by default in the App.
pub fn push_field_slice(scene: &mut SceneData, bundle: &StateBundle, l: usize) {
    let Some(cloud) = bundle.source_cloud.first() else {
        return;
    };
    if bundle.source_position.first().map_or(0, |t| t.len()) <= l {
        return;
    }
    let pose = pose_of(bundle, 0, l);
    let com = bundle.source_position[0][l];
    let span = 2.0;
    let plane = Plane {
        origin: [com[0] - span, com[1], com[2] - span],
        u: [2.0 * span, 0.0, 0.0],
        v: [0.0, 0.0, 2.0 * span],
        nx: 7,
        ny: 7,
    };
    let field = sample_slice(cloud, &pose, &plane);
    let mut k = 0;
    for j in 0..plane.ny {
        for i in 0..plane.nx {
            let node = plane.node(i, j);
            let g = field[k];
            k += 1;
            scene.push_cube([node.x, node.y, node.z], 0.02, [0.25, 0.55, 0.4]);
            let mag = (g.x * g.x + g.y * g.y + g.z * g.z).sqrt();
            if mag > 0.0 {
                let a = 0.28;
                let tip = [
                    node.x + g.x / mag * a,
                    node.y + g.y / mag * a,
                    node.z + g.z / mag * a,
                ];
                scene.push_line([node.x, node.y, node.z], tip, [0.2, 1.0, 0.45]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_draws_a_cube_per_voxel() {
        let bundle = StateBundle {
            source_cloud: vec![vec![
                [0.0, 0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 1.0, 0.0, 1.0],
            ]],
            source_orientation: vec![vec![[1.0, 0.0, 0.0, 0.0]]],
            source_position: vec![vec![[0.0, 0.0, 0.0]]],
            ..Default::default()
        };
        let scene = scene_at(&bundle, 0);
        assert_eq!(scene.cubes.len(), 3, "one cube per cloud element");
    }

    #[test]
    fn frame_centres_on_the_content() {
        // Body at x=10, a detector at the origin ⇒ target near their midpoint, radius covering both.
        let bundle = StateBundle {
            source_cloud: vec![vec![[0.0, 0.0, 0.0, 1.0]]],
            source_orientation: vec![vec![[1.0, 0.0, 0.0, 0.0]]],
            source_position: vec![vec![[10.0, 0.0, 0.0]]],
            detector_placement: vec![[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]],
            ..Default::default()
        };
        let (target, radius) = frame_scene(&bundle);
        assert!(
            (target[0] - 5.0).abs() < 1e-4,
            "centred between body and detector"
        );
        assert!(radius >= 5.0 * 2.2, "radius covers the extent: {radius}");
    }
}
