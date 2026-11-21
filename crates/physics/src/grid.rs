//! The world grid: cloud-in-cell deposition and the time-varying grid mass field.
//!
//! [`GridField`] is how a fluid (or anything else that lives on a fixed lattice) enters the forward
//! model: cell centres are fixed, only the cell masses change, sampled as frames and interpolated
//! linearly in time. It is a [`gravity::FieldContribution`] — that is the reference path, quadrature
//! through the propagation integral exactly like any other field.
//!
//! Because the cells do not move and the detector does not move, the propagation integral of a grid
//! field is a **fixed linear functional of the frames**: `δφ(t) = Σⱼ Σᵢ W[j][i]·m[k₀+j][i]`. The
//! weights `W` (the detector's response to unit mass in cell `i`, `j` frames after the start of the
//! flight) are built once per detector and then every measurement is a dot product —
//! [`GridField::per_ifo_fast`]. Same quadrature nodes, same weights, same kernel as the reference
//! path, summed in a different order; the two agree to rounding (`fast_path_equals_reference`).

use std::sync::{Arc, Mutex};

use gravity::{Cloud, FieldContribution};
use instrument::{build_arms, Detector, InstrumentConfig};
use math::{Mat3, Vec3};
use world::{Channel, Eval, WorldSource};

use crate::kernel::{cube_gradient_tensor, cube_potential};

/// A uniform cubic lattice: `n` cells of side `h` from the `origin` (minimum) corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid3 {
    pub origin: [f64; 3],
    pub h: f64,
    pub n: [usize; 3],
}

impl Grid3 {
    pub fn new(origin: [f64; 3], h: f64, n: [usize; 3]) -> Self {
        assert!(h > 0.0 && n.iter().all(|&c| c > 0), "degenerate grid");
        Grid3 { origin, h, n }
    }

    pub fn len(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// The flat index of cell `(i, j, k)` — x-fastest, the voxeliser's raster order.
    pub fn idx(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.n[0] * (j + self.n[1] * k)
    }

    /// The centre of cell `(i, j, k)`.
    pub fn centre(&self, i: usize, j: usize, k: usize) -> [f64; 3] {
        [
            self.origin[0] + (i as f64 + 0.5) * self.h,
            self.origin[1] + (j as f64 + 0.5) * self.h,
            self.origin[2] + (k as f64 + 0.5) * self.h,
        ]
    }

    pub fn cell_volume(&self) -> f64 {
        self.h * self.h * self.h
    }

    /// Every cell centre, in flat-index order.
    pub fn centres(&self) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let (mut xs, mut ys, mut zs) = (Vec::new(), Vec::new(), Vec::new());
        for k in 0..self.n[2] {
            for j in 0..self.n[1] {
                for i in 0..self.n[0] {
                    let c = self.centre(i, j, k);
                    xs.push(c[0]);
                    ys.push(c[1]);
                    zs.push(c[2]);
                }
            }
        }
        (xs, ys, zs)
    }

    /// Cloud-in-cell (trilinear) deposition of point quantities onto the cell centres.
    ///
    /// Each point's weights over its eight surrounding cells sum to one (a partition of unity), so
    /// the deposited total equals the input total **whatever the positions are** — conservation by
    /// construction. A point on a cell centre deposits wholly into that cell, so depositing a
    /// lattice's own points is the identity, bit for bit: a coordinate within `1e-9` of a cell of a
    /// centre is snapped onto it, so the rounding in `origin + (i + ½)h` cannot leak a part in 10¹⁵
    /// into the neighbour (the weights still sum to one). Returns the amount that fell outside the grid, which
    /// is the only way anything can go missing; callers that need conservation assert it is zero.
    pub fn splat(&self, cloud: &Cloud, values: &[f64], out: &mut [f64]) -> f64 {
        assert_eq!(out.len(), self.len());
        assert_eq!(values.len(), cloud.len());
        let mut lost = 0.0;
        for (p, &value) in values.iter().enumerate() {
            let u = [
                snap((cloud.xs[p] - self.origin[0]) / self.h - 0.5),
                snap((cloud.ys[p] - self.origin[1]) / self.h - 0.5),
                snap((cloud.zs[p] - self.origin[2]) / self.h - 0.5),
            ];
            let base = [u[0].floor(), u[1].floor(), u[2].floor()];
            let f = [u[0] - base[0], u[1] - base[1], u[2] - base[2]];
            for (dz, wz) in [(0i64, 1.0 - f[2]), (1, f[2])] {
                for (dy, wy) in [(0i64, 1.0 - f[1]), (1, f[1])] {
                    for (dx, wx) in [(0i64, 1.0 - f[0]), (1, f[0])] {
                        let w = wx * wy * wz;
                        if w == 0.0 {
                            continue;
                        }
                        let c = [
                            base[0] as i64 + dx,
                            base[1] as i64 + dy,
                            base[2] as i64 + dz,
                        ];
                        let inside = (0..3).all(|a| c[a] >= 0 && (c[a] as usize) < self.n[a]);
                        if inside {
                            out[self.idx(c[0] as usize, c[1] as usize, c[2] as usize)] +=
                                w * values[p];
                        } else {
                            lost += w * value;
                        }
                    }
                }
            }
        }
        lost
    }
}

/// Snap a lattice coordinate onto the nearest integer when it is within `1e-9` of it.
fn snap(u: f64) -> f64 {
    let r = u.round();
    if (u - r).abs() < 1e-9 {
        r
    } else {
        u
    }
}

/// The propagation integral's quadrature — `(flight time, weight)` over `[0, 2T]`, the two composite
/// Simpson halves split at the π-pulse. Reproduces `instrument`'s rule node for node.
pub fn flight_nodes(cfg: &InstrumentConfig) -> Vec<(f64, f64)> {
    let mut nodes = Vec::new();
    for (a, b) in [(0.0, cfg.t_half), (cfg.t_half, 2.0 * cfg.t_half)] {
        let mut n = ((b - a) / cfg.fine_dt).round() as usize;
        n = n.max(2);
        n += n % 2;
        let h = (b - a) / n as f64;
        for i in 0..=n {
            let w = if i == 0 || i == n {
                1.0
            } else if i % 2 == 1 {
                4.0
            } else {
                2.0
            };
            nodes.push((a + i as f64 * h, w * h / 3.0));
        }
    }
    nodes
}

/// A detector's linear response to a grid field: `w[ifo][j·N + i]` is the single-interferometer
/// phase per unit mass in cell `i`, `j` frames after the frame containing the start of the flight.
struct Response {
    key: [u64; 9],
    span: usize,
    w: [Vec<f64>; 2],
}

/// Frames of cell masses on a fixed lattice — a time-varying world-frame mass field.
pub struct GridField {
    xs: Vec<f64>,
    ys: Vec<f64>,
    zs: Vec<f64>,
    h: f64,
    t0: f64,
    dt: f64,
    frames: Vec<Vec<f64>>,
    channel: Channel,
    responses: Mutex<Vec<Arc<Response>>>,
}

/// How many detector responses a field keeps. A uniform schedule needs one per detector; a jittered
/// one needs a fresh response per measurement, so the oldest are dropped.
const RESPONSE_CACHE: usize = 64;

impl GridField {
    /// A field on `grid` whose cell masses [kg] are `frames[f]` at time `t0 + f·dt`.
    pub fn new(grid: &Grid3, t0: f64, dt: f64, frames: Vec<Vec<f64>>, channel: Channel) -> Self {
        assert!(
            dt > 0.0 && frames.len() >= 2,
            "a field needs at least two frames"
        );
        assert!(
            frames.iter().all(|f| f.len() == grid.len()),
            "frame size ≠ grid size"
        );
        let (xs, ys, zs) = grid.centres();
        GridField {
            xs,
            ys,
            zs,
            h: grid.h,
            t0,
            dt,
            frames,
            channel,
            responses: Mutex::new(Vec::new()),
        }
    }

    /// The time span the field is defined on.
    pub fn span(&self) -> (f64, f64) {
        (self.t0, self.t0 + (self.frames.len() - 1) as f64 * self.dt)
    }

    pub fn frames(&self) -> &[Vec<f64>] {
        &self.frames
    }

    pub fn frame_dt(&self) -> f64 {
        self.dt
    }

    /// The bracketing frame and interpolation weight at time `t`. Panics outside the span: a field
    /// queried where it was never simulated has no right answer, and a silent clamp would be a
    /// plausible-looking wrong one.
    fn bracket(&self, t: f64) -> (usize, f64) {
        let s = (t - self.t0) / self.dt;
        let last = (self.frames.len() - 1) as f64;
        assert!(
            s >= -1e-9 && s <= last + 1e-9,
            "grid field queried at t = {t} outside its span {:?}",
            self.span()
        );
        let k = (s.floor().max(0.0) as usize).min(self.frames.len() - 2);
        (k, s - k as f64)
    }

    /// The cell masses at time `t` (linear interpolation between frames).
    pub fn masses_at(&self, t: f64) -> Vec<f64> {
        let (k, a) = self.bracket(t);
        self.frames[k]
            .iter()
            .zip(&self.frames[k + 1])
            .map(|(m0, m1)| (1.0 - a) * m0 + a * m1)
            .collect()
    }

    fn cell(&self, i: usize) -> Vec3<f64> {
        Vec3::new(self.xs[i], self.ys[i], self.zs[i])
    }

    fn response(&self, cfg: &InstrumentConfig, det: &Detector, phase: f64) -> Arc<Response> {
        // Keyed by the detector placement and the flight's phase against the frame grid, quantised
        // to 2⁻³⁰ of a frame (≈1e-10 s): measurements that differ by less share one response.
        let row = det.placement_row();
        let mut key = [0u64; 9];
        for (k, v) in key.iter_mut().zip(row) {
            *k = v.to_bits();
        }
        key[7] = (phase * (1u64 << 30) as f64).round() as u64;
        key[8] = cfg.fine_dt.to_bits() ^ cfg.t_half.to_bits().rotate_left(17);
        let mut cache = self.responses.lock().unwrap();
        if let Some(r) = cache.iter().find(|r| r.key == key) {
            return r.clone();
        }

        let n = self.xs.len();
        let nodes = flight_nodes(cfg);
        let two_t = 2.0 * cfg.t_half;
        let span = ((phase + two_t / self.dt).floor() as usize) + 2;
        let scale = cfg.m_a / cfg.hbar;
        let mut w = [vec![0.0; span * n], vec![0.0; span * n]];
        for (ifo, wi) in build_arms(cfg).iter().zip(w.iter_mut()) {
            for &(tau, weight) in &nodes {
                let s = phase + tau / self.dt;
                let j = (s.floor() as usize).min(span - 2);
                let a = s - j as f64;
                let pu = det
                    .placement
                    .apply(Vec3::new(0.0, 0.0, ifo.upper.z_at(tau)));
                let pl = det
                    .placement
                    .apply(Vec3::new(0.0, 0.0, ifo.lower.z_at(tau)));
                let c = scale * weight;
                for i in 0..n {
                    let x = self.cell(i);
                    let diff = cube_potential(pu - x, self.h) - cube_potential(pl - x, self.h);
                    wi[j * n + i] += c * (1.0 - a) * diff;
                    wi[(j + 1) * n + i] += c * a * diff;
                }
            }
        }
        let r = Arc::new(Response { key, span, w });
        if cache.len() >= RESPONSE_CACHE {
            cache.remove(0);
        }
        cache.push(r.clone());
        r
    }
}

impl FieldContribution<f64> for GridField {
    fn potential(&self, p: Vec3<f64>, t: f64) -> f64 {
        let (k, a) = self.bracket(t);
        let (f0, f1) = (&self.frames[k], &self.frames[k + 1]);
        let mut acc = 0.0;
        for i in 0..self.xs.len() {
            let m = (1.0 - a) * f0[i] + a * f1[i];
            if m != 0.0 {
                acc += m * cube_potential(p - self.cell(i), self.h);
            }
        }
        acc
    }

    fn gradient_tensor(&self, p: Vec3<f64>, t: f64) -> Mat3<f64> {
        let (k, a) = self.bracket(t);
        let (f0, f1) = (&self.frames[k], &self.frames[k + 1]);
        let mut m = [[0.0; 3]; 3];
        for i in 0..self.xs.len() {
            let mass = (1.0 - a) * f0[i] + a * f1[i];
            if mass != 0.0 {
                let g = cube_gradient_tensor(p - self.cell(i), self.h);
                for (row, grow) in m.iter_mut().zip(&g.m) {
                    for (e, ge) in row.iter_mut().zip(grow) {
                        *e += mass * ge;
                    }
                }
            }
        }
        Mat3 { m }
    }
}

impl WorldSource for GridField {
    fn eval(&self) -> Eval<'_> {
        Eval::Field(self)
    }

    fn world_cloud_at(&self, t: f64) -> Cloud {
        Cloud {
            xs: self.xs.clone(),
            ys: self.ys.clone(),
            zs: self.zs.clone(),
            ms: self.masses_at(t),
        }
    }

    fn channel(&self) -> Channel {
        self.channel
    }

    fn per_ifo_fast(&self, cfg: &InstrumentConfig, det: &Detector, t: f64) -> Option<[f64; 2]> {
        // Snapped, so a schedule aligned with the frame grid lands on one phase despite rounding.
        let s = snap((t - 2.0 * cfg.t_half - self.t0) / self.dt);
        let (lo, hi) = self.span();
        assert!(
            s >= -1e-9 && t <= hi + 1e-9,
            "grid field needs [t − 2T, t] = [{}, {t}] inside its span [{lo}, {hi}]",
            t - 2.0 * cfg.t_half
        );
        let k0 = s.floor().max(0.0) as usize;
        let r = self.response(cfg, det, s - k0 as f64);
        let n = self.xs.len();
        let mut out = [0.0; 2];
        for (o, w) in out.iter_mut().zip(&r.w) {
            for j in 0..r.span {
                // The last response row carries weight only when the flight ends strictly inside a
                // frame interval; a schedule that ends on the final frame never reads past it.
                let Some(frame) = self.frames.get(k0 + j) else {
                    debug_assert!(w[j * n..(j + 1) * n].iter().all(|&x| x == 0.0));
                    continue;
                };
                let row = &w[j * n..(j + 1) * n];
                *o += row.iter().zip(frame).map(|(a, b)| a * b).sum::<f64>();
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use instrument::PropagationIntegral;

    fn lcg(state: &mut u64) -> f64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*state >> 11) as f64 / 9007199254740992.0
    }

    #[test]
    fn splat_is_a_partition_of_unity() {
        // Arbitrary positions inside the grid: the deposited total equals the input total to
        // rounding, and nothing is lost.
        let grid = Grid3::new([-1.0, -2.0, 0.0], 0.25, [16, 20, 12]);
        let mut s = 7u64;
        let mut elems = Vec::new();
        for _ in 0..500 {
            elems.push((
                -0.8 + 3.5 * lcg(&mut s),
                -1.8 + 4.5 * lcg(&mut s),
                0.2 + 2.5 * lcg(&mut s),
                0.1 + lcg(&mut s),
            ));
        }
        let cloud = Cloud::from_elements(&elems);
        let mut out = vec![0.0; grid.len()];
        let lost = grid.splat(&cloud, &cloud.ms, &mut out);
        assert_eq!(lost, 0.0);
        let (total, deposited): (f64, f64) = (cloud.total_mass(), out.iter().sum());
        assert!(
            (deposited - total).abs() / total <= 1e-14,
            "{deposited} vs {total}"
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // the null limit is exact, not approximate
    fn splat_at_cell_centres_is_the_identity() {
        // The null limit: a lattice's own cell-centre points deposit wholly into their home cells,
        // bit for bit — depositing the rest voxelisation recovers the rest voxelisation.
        let grid = Grid3::new([0.3, -0.7, 1.1], 0.125, [9, 7, 5]);
        let (xs, ys, zs) = grid.centres();
        let ms: Vec<f64> = (0..grid.len())
            .map(|i| 1.0 + (i % 13) as f64 * 0.37)
            .collect();
        let cloud = Cloud {
            xs,
            ys,
            zs,
            ms: ms.clone(),
        };
        let mut out = vec![0.0; grid.len()];
        assert_eq!(grid.splat(&cloud, &cloud.ms, &mut out), 0.0);
        assert_eq!(out, ms);
    }

    #[test]
    fn splat_reports_what_leaves_the_grid() {
        let grid = Grid3::new([0.0; 3], 1.0, [2, 2, 2]);
        let cloud = Cloud::from_elements(&[(2.4, 1.0, 1.0, 3.0)]);
        let mut out = vec![0.0; grid.len()];
        let lost = grid.splat(&cloud, &cloud.ms, &mut out);
        assert!(lost > 0.0);
        assert!((lost + out.iter().sum::<f64>() - 3.0).abs() <= 1e-14);
    }

    fn drifting_field() -> GridField {
        // A blob of positive and negative density anomaly drifting upward beside the tower.
        let grid = Grid3::new([0.6, -0.5, 0.5], 0.25, [6, 4, 14]);
        let (dt, n_frames) = (0.2, 40);
        let frames = (0..n_frames)
            .map(|f| {
                let t = f as f64 * dt;
                let zc = 1.0 + 0.3 * t;
                let mut m = vec![0.0; grid.len()];
                for k in 0..grid.n[2] {
                    for j in 0..grid.n[1] {
                        for i in 0..grid.n[0] {
                            let c = grid.centre(i, j, k);
                            let r2 = (c[0] - 1.2).powi(2) + c[1] * c[1] + (c[2] - zc).powi(2);
                            m[grid.idx(i, j, k)] =
                                2e-3 * ((-r2 / 0.3).exp() - 0.4 * (-r2 / 0.9).exp());
                        }
                    }
                }
                m
            })
            .collect();
        GridField::new(&grid, -1.0, dt, frames, Channel::Ambient)
    }

    #[test]
    fn fast_path_equals_reference() {
        // The linear-response reformulation against quadrature through the propagation integral —
        // for an upright and a tilted, displaced detector, at measurement times on and off the
        // frame grid.
        let field = drifting_field();
        let model = PropagationIntegral::default();
        let tilted = Detector::placed(math::Isometry3::new(
            math::Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0), 0.15),
            Vec3::new(-0.4, 0.2, 0.3),
        ));
        for det in [Detector::new(0.0), tilted] {
            for &t in &[0.46, 1.0, 2.537, 5.2, 6.8] {
                let want = model.per_ifo(&[], &[&field], &det, t);
                let got = field.per_ifo_fast(&model.cfg, &det, t).unwrap();
                for (g, w) in got.iter().zip(want) {
                    assert!(
                        (g - w).abs() <= 1e-9 * w.abs(),
                        "t={t}: fast {g:e} vs reference {w:e}"
                    );
                }
            }
        }
    }

    #[test]
    fn uniform_schedule_reuses_one_response() {
        let field = drifting_field();
        let cfg = InstrumentConfig::default();
        let det = Detector::new(0.0);
        for i in 0..10 {
            field.per_ifo_fast(&cfg, &det, 0.46 + 0.6 * i as f64);
        }
        assert_eq!(field.responses.lock().unwrap().len(), 1);
    }

    #[test]
    fn static_field_is_time_independent() {
        // Identical frames: the phase is the same at every measurement time.
        let grid = Grid3::new([1.0, 0.0, 1.0], 0.5, [2, 2, 2]);
        let frame: Vec<f64> = (0..8).map(|i| 1.0 + i as f64).collect();
        let field = GridField::new(&grid, 0.0, 0.5, vec![frame; 12], Channel::Target);
        let (cfg, det) = (InstrumentConfig::default(), Detector::new(0.0));
        let a = field.per_ifo_fast(&cfg, &det, 2.0).unwrap();
        let b = field.per_ifo_fast(&cfg, &det, 4.13).unwrap();
        assert!(
            (a[0] - b[0]).abs() <= 1e-12 * a[0].abs() && (a[1] - b[1]).abs() <= 1e-12 * a[1].abs()
        );
    }

    #[test]
    #[should_panic(expected = "outside its span")]
    fn query_outside_the_span_is_loud() {
        drifting_field().potential(Vec3::new(0.0, 0.0, 1.0), 99.0);
    }
}
