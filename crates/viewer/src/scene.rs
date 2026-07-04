//! Assemble the 3D scene for scrub index `ℓ` from a bundle: the posed source cloud(s), the detector
//! array markers, and the spin-axis vector. One place maps the bundle's tensors to drawable instances,
//! so the live App and the headless render test draw the same thing from the same data.

use state::StateBundle;

use crate::field_slice::{sample_slice, Plane};
use crate::pose::{pose_of, world_vertices};
use crate::render::SceneData;

/// The drawable scene at scrub index `ℓ`: clouds posed by `pose_of`, array markers from
/// `detector_placement`, and a short spin-axis vector along `source_angular_velocity`.
pub fn scene_at(bundle: &StateBundle, l: usize) -> SceneData {
    let mut scene = SceneData::new();

    for (s, cloud) in bundle.source_cloud.iter().enumerate() {
        let poses = bundle.source_position.get(s).map_or(0, |t| t.len());
        if l >= poses {
            continue;
        }
        let pose = pose_of(bundle, s, l);
        scene.push_points(&world_vertices(cloud, &pose), 0.012, [0.65, 0.78, 1.0]);

        // Spin axis: a run of markers from the COM along ω̂ (a crude vector; the tumble is judged live).
        let com = bundle.source_position[s][l];
        if let Some(w) = bundle.source_angular_velocity.get(s).and_then(|t| t.get(l)) {
            let mag = (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
            if mag > 0.0 {
                let axis = [w[0] / mag, w[1] / mag, w[2] / mag];
                for k in 1..=6 {
                    let r = 0.4 * k as f64;
                    scene.push_marker(
                        [
                            com[0] + axis[0] * r,
                            com[1] + axis[1] * r,
                            com[2] + axis[2] * r,
                        ],
                        0.008,
                        [1.0, 0.35, 0.35],
                    );
                }
            }
        }
    }

    for d in &bundle.detector_placement {
        scene.push_marker([d[0], d[1], d[2]], 0.03, [1.0, 0.62, 0.2]);
    }

    scene
}

/// Overlay the on-demand field slice for source 0 at `ℓ`: a plane through the COM, each node drawn as a
/// dim dot with a bright tip along `ĝ` — a crude arrow field, sampled `gravity::field` on demand (never
/// the stored grid). Off by default in the App.
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
            let mag = (g.x * g.x + g.y * g.y + g.z * g.z).sqrt();
            scene.push_marker([node.x, node.y, node.z], 0.006, [0.25, 0.55, 0.4]);
            if mag > 0.0 {
                let a = 0.28;
                scene.push_marker(
                    [
                        node.x + g.x / mag * a,
                        node.y + g.y / mag * a,
                        node.z + g.z / mag * a,
                    ],
                    0.009,
                    [0.2, 1.0, 0.45],
                );
            }
        }
    }
}
