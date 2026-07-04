//! Posing the body cloud into the world frame. The geometry the 3D scene draws MUST equal what the
//! bundle holds: `vertex_world = orientation[ℓ] · body + position[ℓ]`. A rendered pose the bundle does
//! not contain is a visual lie — the viewer's marshalling bug — so this is tested headlessly, not eyed.

use state::{Isometry3, Quat, StateBundle, Vec3};

/// The world pose of source `s` at scrub index `ℓ`, reconstructed from the bundle's tensors.
pub fn pose_of(bundle: &StateBundle, s: usize, l: usize) -> Isometry3 {
    let q = bundle.source_orientation[s][l]; // [w, x, y, z], world ← body
    let p = bundle.source_position[s][l]; // [x, y, z]
    Isometry3::new(
        Quat::new(q[0], q[1], q[2], q[3]),
        Vec3::new(p[0], p[1], p[2]),
    )
}

/// The world-frame vertices of body cloud `body` (`[x, y, z, m]` per element) under `pose`.
pub fn world_vertices(body: &[[f64; 4]], pose: &Isometry3) -> Vec<[f64; 3]> {
    body.iter()
        .map(|e| {
            let w = pose.apply(Vec3::new(e[0], e[1], e[2]));
            [w.x, w.y, w.z]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pose_placement() {
        // The rendered vertices equal orientation[ℓ]·body + position[ℓ] to ≤1e-6, checked against an
        // INDEPENDENT rotation-matrix reference (a 45° turn about z, then a translation). The view shows
        // what the bundle holds — not what a second copy of the same code computes.
        let body = vec![
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 2.0, 0.0, 5.0],
            [0.3, -0.4, 1.0, 5.0],
        ];
        let angle = std::f64::consts::FRAC_PI_4;
        let q = [(angle * 0.5).cos(), 0.0, 0.0, (angle * 0.5).sin()]; // rotation about z
        let p = [3.0, -1.0, 2.0];

        let bundle = StateBundle {
            source_orientation: vec![vec![q]],
            source_position: vec![vec![p]],
            ..Default::default()
        };
        let got = world_vertices(&body, &pose_of(&bundle, 0, 0));

        let (ca, sa) = (angle.cos(), angle.sin());
        for (v, w) in body.iter().zip(&got) {
            let want = [
                ca * v[0] - sa * v[1] + p[0],
                sa * v[0] + ca * v[1] + p[1],
                v[2] + p[2],
            ];
            for k in 0..3 {
                assert!(
                    (w[k] - want[k]).abs() <= 1e-6,
                    "vertex {v:?} axis {k}: got {}, want {}",
                    w[k],
                    want[k]
                );
            }
        }
    }
}
