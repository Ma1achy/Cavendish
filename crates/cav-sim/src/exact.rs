//! A body read exactly, with no voxels (TASK-S5-06; `docs/slices/S5.md` §2 "Reading a cut cell",
//! §7.4).
//!
//! A body whose shape is a polyhedron (a cuboid, a Platonic solid, the convex hull of given
//! corners) or a closed mesh, of one material and with no parts, may set its shape's
//! `representation` to `Exact` ([`crate::shape::Representation`]). It is then read as the solid
//! itself, by [`cav_core::polyhedral::Faceted`]:
//!
//! - **near**, by the closed form: `V(upper) − V(lower)` of Werner & Scheeres' face form. Its two
//!   potentials are of the same size, so the difference loses digits as the arm points go farther:
//!   measured (§7.1), its error is about `100 ε (R/s)² R/gap` of the difference, `R` the distance
//!   from the solid's centroid, `s` its size ([`Faceted::size`]);
//! - **far**, by its exact moments ([`Faceted::moments`]) through the expansion to the octupole
//!   about its centroid ([`Summary::difference`]), which keeps its digits at any distance, and S4's
//!   bound on what that leaves out added to the read's estimate ([`Summary::bound`], with
//!   `A_l = M r^l`, `r` the farthest vertex from the centroid: every bit of the solid is within `r`,
//!   so `Σ|m| s^l ≤ M r^l` holds exactly and the bound is rigorous).
//!
//! **The switch.** The closed form is taken while `(R/s)² · R/G ≤` [`CLOSED_WITHIN`], `G` the
//! instrument's widest arm separation (`v_kick · T`): there its rounding is at most about 1e-9 of
//! the largest difference of the flight. The closed form's rounding does not shrink with the gap
//! (it is `ε` of the potential's terms), so `G`, not each node's gap, is the scale it is held to.
//! It is also taken wherever the expansion would not converge well (`R < 2r`).
//!
//! Frames: the solid's own frame is the body's, centred on the body's position (the world's axes:
//! bodies do not turn), metres; masses kg; densities kg/m³; the source's results kg/m (`V/G`).

use cav_core::Vec3;
use cav_core::forward::Source;
use cav_core::polyhedral::Faceted;
use cav_core::voxel::summary::{ABSOLUTE_ORDERS, Summary};

pub use crate::shape::MAX_EXACT_FACES;

/// The closed form is the read while `(R/s)² · R/G` is at most this (see the module's
/// documentation): `1e-9 / (100 ε)`, from §7.1's measured `100 ε (R/s)² R/gap`, which put the
/// 1e-9 line at about 30 sizes for a gap of one size and 100 for ten. Beyond it, the moments.
pub const CLOSED_WITHIN: f64 = 1e-9 / (100.0 * f64::EPSILON);

/// A solid of uniform density read exactly as a source of the forward model: the closed form
/// near, the moments far (the module's documentation).
pub struct ExactBody<P: Fn(f64) -> Vec3 + Sync> {
    solid: Faceted,
    /// kg/m³.
    density: f64,
    /// The moments about the centroid, with `A_l = M r^l` and reach `r`.
    summary: Summary,
    /// The solid's size `s`, metres ([`Faceted::size`]).
    size: f64,
    /// The arm separation `G` the closed form's rounding is held to, metres.
    separation: f64,
    /// Where the body's frame is at a time: seconds on the scene's clock in, metres in the world's
    /// frame out. The body does not turn.
    pub position: P,
    /// `(from, until)`, seconds on the scene's clock: it exists from the first up to the second.
    pub exists: (f64, f64),
    /// Times at which its motion is not smooth, seconds on the scene's clock.
    pub breaks: Vec<f64>,
}

impl<P: Fn(f64) -> Vec3 + Sync> ExactBody<P> {
    /// `solid` (the body's frame, metres) at uniform `density` kg/m³, its closed form held to
    /// arms `separation` metres apart (the widest of the flight; the smallest such over the
    /// instruments that read it), at `position`, always there.
    #[must_use]
    pub fn new(solid: Faceted, density: f64, separation: f64, position: P) -> Self {
        let moments = solid.moments(density);
        let reach = 0.5 * solid.size();
        let mut summary = Summary::empty(moments.centre);
        summary.mass = moments.mass;
        summary.second = moments.second;
        summary.third = moments.third;
        let weight = moments.mass.abs();
        let mut power = weight;
        for a in summary.absolute.iter_mut().take(ABSOLUTE_ORDERS) {
            *a = power;
            power *= reach;
        }
        summary.reach = reach;
        let size = solid.size();
        Self {
            solid,
            density,
            summary,
            size,
            separation,
            position,
            exists: (f64::NEG_INFINITY, f64::INFINITY),
            breaks: Vec::new(),
        }
    }

    /// How many triangles each arm point costs: the read's work per point.
    #[must_use]
    pub fn faces(&self) -> usize {
        self.solid.faces().len()
    }

    /// The solid's mass, kg.
    #[must_use]
    pub fn mass(&self) -> f64 {
        self.summary.mass
    }

    /// Whether the closed form is the read at `distance` metres from the centroid.
    fn closed(&self, distance: f64) -> bool {
        let ratio = distance / self.size;
        // With no arm separation (no kick) the arms are at one point and nothing cancels.
        self.separation <= 0.0
            || distance < 2.0 * self.summary.reach
            || ratio * ratio * distance / self.separation <= CLOSED_WITHIN
    }

    /// The arm points of one interferometer relative to the centroid: `(across, below, above)` as
    /// `cube_difference` takes them, and the nearer point's distance.
    fn points(&self, offset: Vec3, height: f64, lower: f64, upper: f64) -> ([f64; 2], f64, f64, f64) {
        let at = offset + self.summary.centre;
        let across = [-at.x, -at.y];
        let (below, above) = (height + lower - at.z, height + upper - at.z);
        let horizontal = across[0] * across[0] + across[1] * across[1];
        let nearer = (horizontal + below * below).sqrt().min((horizontal + above * above).sqrt());
        (across, below, above, nearer)
    }

    /// What reading the far side through the moments leaves out at one instant, for each
    /// interferometer, kg/m: S4's bound between the two arm points, 0 where the closed form is
    /// the read. Read through the flight as a source ([`ExactBound`]).
    fn bound(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2] {
        if !(time >= self.exists.0 && time < self.exists.1) {
            return [0.0; 2];
        }
        let offset = (self.position)(time) - tower;
        heights.map(|height| {
            let (across, below, above, nearer) = self.points(offset, height, lower, upper);
            if self.closed(nearer) {
                return 0.0;
            }
            // The least distance from the centroid to the segment between the two points.
            let horizontal = across[0].hypot(across[1]);
            let (low, high) = (below.min(above), below.max(above));
            let vertical = if low > 0.0 {
                low
            } else if high < 0.0 {
                -high
            } else {
                0.0
            };
            self.summary.bound(horizontal.hypot(vertical), upper - lower)
        })
    }
}

impl<P: Fn(f64) -> Vec3 + Sync> Source for ExactBody<P> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2] {
        if !(time >= self.exists.0 && time < self.exists.1) {
            return [0.0; 2];
        }
        // The local frame first: the body's frame relative to the tower's base.
        let offset = (self.position)(time) - tower;
        let gap = lower - upper;
        heights.map(|height| {
            let (across, below, above, nearer) = self.points(offset, height, lower, upper);
            if self.closed(nearer) {
                // The solid's own frame: its origin is the body's position, not the centroid.
                let c = self.summary.centre;
                self.solid.difference(
                    self.density,
                    [across[0] + c.x, across[1] + c.y],
                    below + c.z,
                    above + c.z,
                    gap,
                )
            } else {
                self.summary.difference(across, below, above, gap)
            }
        })
    }

    fn breaks(&self, from: f64, to: f64, out: &mut Vec<f64>) {
        let ends = [self.exists.0, self.exists.1];
        out.extend(self.breaks.iter().chain(&ends).copied().filter(|t| *t > from && *t < to));
    }
}

/// The bound on what an [`ExactBody`]'s far reading leaves out, as a source: read through the
/// flight (the quadrature's weights are positive), it bounds that error in radians.
pub struct ExactBound<'a, P: Fn(f64) -> Vec3 + Sync>(pub &'a ExactBody<P>);

impl<P: Fn(f64) -> Vec3 + Sync> Source for ExactBound<'_, P> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2] {
        self.0.bound(tower, heights, lower, upper, time)
    }

    fn breaks(&self, from: f64, to: f64, out: &mut Vec<f64>) {
        self.0.breaks(from, to, out);
    }
}
