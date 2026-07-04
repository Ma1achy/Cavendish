//! The field view's on-demand slice: sample `gravity::field` on one plane at the scrubbed pose. The
//! cheap default — one plane, not a volume — so inspecting the field never forces the storage-dominant
//! stored grid (deferred; `design/viewer.md` §4).

use gravity::Cloud;
use state::{Isometry3, Vec3};

/// A rectangular sampling plane: `origin` with in-plane spans `u`, `v`, over `nx`×`ny` nodes.
pub struct Plane {
    pub origin: [f64; 3],
    pub u: [f64; 3],
    pub v: [f64; 3],
    pub nx: usize,
    pub ny: usize,
}

impl Plane {
    /// World position of node `(i, j)`: `origin + (i/(nx−1))·u + (j/(ny−1))·v`.
    pub fn node(&self, i: usize, j: usize) -> Vec3<f64> {
        let fx = if self.nx > 1 {
            i as f64 / (self.nx - 1) as f64
        } else {
            0.0
        };
        let fy = if self.ny > 1 {
            j as f64 / (self.ny - 1) as f64
        } else {
            0.0
        };
        Vec3::new(
            self.origin[0] + fx * self.u[0] + fy * self.v[0],
            self.origin[1] + fx * self.u[1] + fy * self.v[1],
            self.origin[2] + fx * self.u[2] + fy * self.v[2],
        )
    }
}

/// The world cloud from a bundle body cloud `body` (`[x, y, z, m]` per element) posed by `pose`,
/// assembled through the canonical `Cloud::transformed` path.
pub fn world_cloud(body: &[[f64; 4]], pose: &Isometry3) -> Cloud {
    let mut b = Cloud::default();
    for e in body {
        b.xs.push(e[0]);
        b.ys.push(e[1]);
        b.zs.push(e[2]);
        b.ms.push(e[3]);
    }
    b.transformed(pose)
}

/// Sample the gravitational field `g = −∇V` at every plane node of the posed body cloud (row-major,
/// `j` outer, `i` inner).
pub fn sample_slice(body: &[[f64; 4]], pose: &Isometry3, plane: &Plane) -> Vec<Vec3<f64>> {
    let world = world_cloud(body, pose);
    let mut out = Vec::with_capacity(plane.nx * plane.ny);
    for j in 0..plane.ny {
        for i in 0..plane.nx {
            out.push(gravity::field(&world, plane.node(i, j)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::Quat;

    #[test]
    fn field_two_modes() {
        // The on-demand slice equals an independent `gravity::field` reference at the same nodes (≤1e-6).
        // The slice sampler assembles the world cloud via `Cloud::transformed`; the reference poses each
        // element by hand. Same kernel (the ground truth), independent assembly — not a tautology.
        let body = vec![[0.2, 0.0, 0.0, 800.0], [-0.2, 0.0, 0.1, 400.0]];
        let angle = 0.6_f64;
        let pose = Isometry3::new(
            Quat::new((angle * 0.5).cos(), 0.0, (angle * 0.5).sin(), 0.0), // about y
            Vec3::new(1.0, 0.5, -0.3),
        );
        let plane = Plane {
            origin: [0.0, 0.0, 2.0],
            u: [2.0, 0.0, 0.0],
            v: [0.0, 2.0, 0.0],
            nx: 5,
            ny: 4,
        };
        let slice = sample_slice(&body, &pose, &plane);

        // Independent reference: pose each element manually, then sample the kernel.
        let mut world = Cloud::default();
        for e in &body {
            let w = pose.apply(Vec3::new(e[0], e[1], e[2]));
            world.xs.push(w.x);
            world.ys.push(w.y);
            world.zs.push(w.z);
            world.ms.push(e[3]);
        }

        let mut k = 0;
        for j in 0..plane.ny {
            for i in 0..plane.nx {
                let r = gravity::field(&world, plane.node(i, j));
                let g = slice[k];
                k += 1;
                assert!(
                    (g.x - r.x).abs() <= 1e-6
                        && (g.y - r.y).abs() <= 1e-6
                        && (g.z - r.z).abs() <= 1e-6,
                    "slice vs reference disagree at node ({i},{j}): {g:?} vs {r:?}"
                );
            }
        }
    }
}
