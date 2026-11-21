//! The fluid: incompressible Boussinesq flow on a staggered (MAC) grid.
//!
//! `∂u/∂t + (u·∇)u = −∇p/ρ₀ + ν∇²u + gβ(T − T₀)ẑ`, `∂T/∂t + (u·∇)T = α∇²T + S`, `∇·u = 0`.
//!
//! Velocities live on cell faces and scalars at cell centres, so the pressure projection is exact
//! to the solver tolerance (no checkerboard mode). One step is: advect (semi-Lagrangian with a
//! MacCormack correction — plain semi-Lagrangian smears the temperature field, and the temperature
//! field *is* the density signal the gradiometer reads), diffuse (explicit, sub-stepped to its
//! stability limit), add buoyancy, then project onto divergence-free with a MIC(0)-preconditioned
//! conjugate gradient.
//!
//! The gravity signal is **not** the solver's working density (Boussinesq holds that constant —
//! reading it would give exactly zero signal). It is reconstructed from temperature through the
//! equation of state: [`Air::density`], and [`Fluid::mass_anomaly`].

use crate::grid::Grid3;

/// A thermal boundary condition, on a domain wall or a solid surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Heat {
    /// No heat flux.
    Adiabatic,
    /// A resolved isothermal surface at this temperature: heat enters by conduction through the
    /// half-cell next to it. Correct when the thermal boundary layer is resolved by the grid.
    Isothermal(f64),
    /// An under-resolved surface at `t_surface` exchanging heat by a film coefficient `h_c`
    /// [W m⁻² K⁻¹] over ρc_p: `∂T/∂t = h_c (T_s − T) / (ρ c_p h)` in the adjacent cell. The right
    /// model at room scale, where the real boundary layer is millimetres thick and the cell is not.
    /// `h_over_rho_cp` is `h_c/(ρ c_p)` [m s⁻¹].
    Film { t_surface: f64, h_over_rho_cp: f64 },
}

/// Dry air as an ideal gas at constant thermodynamic pressure — the equation of state the gravity
/// read uses.
#[derive(Clone, Copy, Debug)]
pub struct Air {
    /// Ambient pressure [Pa].
    pub pressure: f64,
    /// Specific gas constant [J kg⁻¹ K⁻¹].
    pub r_specific: f64,
    /// Specific heat at constant pressure [J kg⁻¹ K⁻¹].
    pub c_p: f64,
}

impl Default for Air {
    fn default() -> Self {
        Air {
            pressure: 101_325.0,
            r_specific: 287.05,
            c_p: 1005.0,
        }
    }
}

impl Air {
    /// `ρ = p/(R T)` [kg m⁻³], `T` in kelvin.
    pub fn density(&self, t: f64) -> f64 {
        self.pressure / (self.r_specific * t)
    }
}

/// Fluid properties and forcing.
#[derive(Clone, Copy, Debug)]
pub struct FluidParams {
    /// Kinematic viscosity ν [m² s⁻¹].
    pub nu: f64,
    /// Thermal diffusivity α [m² s⁻¹].
    pub alpha: f64,
    /// Gravitational acceleration magnitude (acting along −z) [m s⁻²].
    pub g: f64,
    /// Thermal expansion coefficient β [K⁻¹] (`1/T₀` for an ideal gas).
    pub beta: f64,
    /// Reference temperature T₀ of the Boussinesq expansion.
    pub t_ref: f64,
    /// Thermal condition on the six domain walls: `[−x, +x, −y, +y, −z, +z]`.
    pub walls: [Heat; 6],
    /// Whether the pair of walls normal to each axis is free-slip (`true`) or no-slip (`false`).
    /// Free-slip on one axis with a single cell across it gives a two-dimensional flow.
    pub free_slip: [bool; 3],
    /// Relative residual at which the pressure solve stops.
    pub tol: f64,
}

impl FluidParams {
    /// Room air at `t_ref` kelvin with adiabatic walls.
    pub fn air(t_ref: f64) -> Self {
        FluidParams {
            nu: 1.5e-5,
            alpha: 2.1e-5,
            g: 9.81,
            beta: 1.0 / t_ref,
            t_ref,
            walls: [Heat::Adiabatic; 6],
            free_slip: [false; 3],
            tol: 1e-6,
        }
    }
}

/// The fluid state on a [`Grid3`].
pub struct Fluid {
    pub grid: Grid3,
    pub params: FluidParams,
    /// x-velocity on x-faces, `(nx+1)·ny·nz`.
    pub u: Vec<f64>,
    /// y-velocity on y-faces, `nx·(ny+1)·nz`.
    pub v: Vec<f64>,
    /// z-velocity on z-faces, `nx·ny·(nz+1)`.
    pub w: Vec<f64>,
    /// Temperature at cell centres.
    pub t: Vec<f64>,
    /// Which cells are solid.
    pub solid: Vec<bool>,
    /// The velocity of the solid occupying each solid cell.
    pub solid_vel: Vec<[f64; 3]>,
    /// The thermal condition on each solid cell's surface.
    pub solid_heat: Vec<Heat>,
    /// Pressure-solve iterations taken by the last step.
    pub last_iterations: usize,
    /// Cumulative wall-clock seconds spent in `[advection, diffusion, projection]` — the profile.
    pub timing: [f64; 3],
    /// The previous step's pressure solution, the initial guess for the next solve.
    q: Vec<f64>,
}

/// Run `work(start, a_chunk, b_chunk)` over matching chunks of two equal-length slices, one chunk per
/// available core. Each element is computed independently of the others, so the result is identical
/// for any thread count.
fn par_chunks<A: Send, B: Send>(
    a: &mut [A],
    b: &mut [B],
    work: impl Fn(usize, &mut [A], &mut [B]) + Sync,
) {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = a.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        for (n, (ca, cb)) in a.chunks_mut(chunk).zip(b.chunks_mut(chunk)).enumerate() {
            let work = &work;
            scope.spawn(move || work(n * chunk, ca, cb));
        }
    });
}

/// The three staggered components and the scalar, for the shared sampling code.
#[derive(Clone, Copy, PartialEq)]
enum Field {
    U = 0,
    V = 1,
    W = 2,
    T = 3,
}

/// Where each field's samples sit within a cell, in cell units (indexed by `Field as usize`).
const OFFSET: [[f64; 3]; 4] = [
    [0.0, 0.5, 0.5],
    [0.5, 0.0, 0.5],
    [0.5, 0.5, 0.0],
    [0.5, 0.5, 0.5],
];

impl Fluid {
    pub fn new(grid: Grid3, params: FluidParams) -> Self {
        let [nx, ny, nz] = grid.n;
        let n = grid.len();
        Fluid {
            grid,
            params,
            u: vec![0.0; (nx + 1) * ny * nz],
            v: vec![0.0; nx * (ny + 1) * nz],
            w: vec![0.0; nx * ny * (nz + 1)],
            t: vec![params.t_ref; n],
            solid: vec![false; n],
            solid_vel: vec![[0.0; 3]; n],
            solid_heat: vec![Heat::Adiabatic; n],
            last_iterations: 0,
            timing: [0.0; 3],
            q: vec![0.0; n],
        }
    }

    fn iu(&self, i: usize, j: usize, k: usize) -> usize {
        i + (self.grid.n[0] + 1) * (j + self.grid.n[1] * k)
    }
    fn iv(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.grid.n[0] * (j + (self.grid.n[1] + 1) * k)
    }
    fn iw(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.grid.n[0] * (j + self.grid.n[1] * k)
    }
    fn ic(&self, i: usize, j: usize, k: usize) -> usize {
        self.grid.idx(i, j, k)
    }

    /// Is the cell at signed index `(i, j, k)` solid? Outside the domain counts as solid (walls).
    fn is_solid(&self, i: i64, j: i64, k: i64) -> bool {
        let [nx, ny, nz] = self.grid.n;
        if i < 0 || j < 0 || k < 0 || i >= nx as i64 || j >= ny as i64 || k >= nz as i64 {
            return true;
        }
        self.solid[self.ic(i as usize, j as usize, k as usize)]
    }

    /// The velocity of the solid at a cell (zero for the domain walls).
    fn solid_velocity(&self, i: i64, j: i64, k: i64) -> [f64; 3] {
        let [nx, ny, nz] = self.grid.n;
        if i < 0 || j < 0 || k < 0 || i >= nx as i64 || j >= ny as i64 || k >= nz as i64 {
            return [0.0; 3];
        }
        self.solid_vel[self.ic(i as usize, j as usize, k as usize)]
    }

    /// The prescribed normal velocity of a face along `axis` between cell `c − ê` and cell `c`, if
    /// either side is solid; `None` for a free fluid face.
    fn face_fixed(&self, axis: usize, c: [i64; 3]) -> Option<f64> {
        let mut lo = c;
        lo[axis] -= 1;
        let (a, b) = (
            self.is_solid(lo[0], lo[1], lo[2]),
            self.is_solid(c[0], c[1], c[2]),
        );
        match (a, b) {
            (false, false) => None,
            (true, false) => Some(self.solid_velocity(lo[0], lo[1], lo[2])[axis]),
            (false, true) => Some(self.solid_velocity(c[0], c[1], c[2])[axis]),
            (true, true) => Some(
                0.5 * (self.solid_velocity(lo[0], lo[1], lo[2])[axis]
                    + self.solid_velocity(c[0], c[1], c[2])[axis]),
            ),
        }
    }

    /// Write the prescribed velocity onto every face that touches a solid or a wall.
    fn enforce_solid_faces(&mut self) {
        let [nx, ny, nz] = self.grid.n;
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..=nx {
                    if let Some(x) = self.face_fixed(0, [i as i64, j as i64, k as i64]) {
                        let id = self.iu(i, j, k);
                        self.u[id] = x;
                    }
                }
            }
        }
        for k in 0..nz {
            for j in 0..=ny {
                for i in 0..nx {
                    if let Some(x) = self.face_fixed(1, [i as i64, j as i64, k as i64]) {
                        let id = self.iv(i, j, k);
                        self.v[id] = x;
                    }
                }
            }
        }
        for k in 0..=nz {
            for j in 0..ny {
                for i in 0..nx {
                    if let Some(x) = self.face_fixed(2, [i as i64, j as i64, k as i64]) {
                        let id = self.iw(i, j, k);
                        self.w[id] = x;
                    }
                }
            }
        }
    }

    /// A field value at signed lattice index, with the boundary conditions applied as ghost values:
    /// a velocity component is clamped along its own axis and mirrored with a sign flip across the
    /// walls it is tangential to (no-slip at the wall plane); temperature mirrors across an
    /// adiabatic or film wall and reflects about the wall value across an isothermal one.
    fn at(&self, f: Field, data: &[f64], i: i64, j: i64, k: i64) -> f64 {
        let [nx, ny, nz] = [
            self.grid.n[0] as i64,
            self.grid.n[1] as i64,
            self.grid.n[2] as i64,
        ];
        let dims = match f {
            Field::U => [nx + 1, ny, nz],
            Field::V => [nx, ny + 1, nz],
            Field::W => [nx, ny, nz + 1],
            Field::T => [nx, ny, nz],
        };
        let normal = match f {
            Field::U => 0,
            Field::V => 1,
            Field::W => 2,
            Field::T => 3,
        };
        let mut idx = [i, j, k];
        let mut sign = 1.0;
        let mut wall_t: Option<f64> = None;
        for a in 0..3 {
            if idx[a] >= 0 && idx[a] < dims[a] {
                continue;
            }
            let low = idx[a] < 0;
            if a == normal {
                idx[a] = idx[a].clamp(0, dims[a] - 1);
                continue;
            }
            idx[a] = if low {
                -1 - idx[a]
            } else {
                2 * dims[a] - 1 - idx[a]
            };
            idx[a] = idx[a].clamp(0, dims[a] - 1);
            if f == Field::T {
                if let Heat::Isothermal(tw) = self.params.walls[2 * a + usize::from(!low)] {
                    wall_t = Some(tw);
                }
            } else if !self.params.free_slip[a] {
                sign = -sign;
            }
        }
        let stride = [1, dims[0], dims[0] * dims[1]];
        let v = data[(idx[0] + stride[1] * idx[1] + stride[2] * idx[2]) as usize];
        match wall_t {
            Some(tw) => 2.0 * tw - v,
            None => sign * v,
        }
    }

    fn data(&self, f: Field) -> &[f64] {
        match f {
            Field::U => &self.u,
            Field::V => &self.v,
            Field::W => &self.w,
            Field::T => &self.t,
        }
    }

    /// Trilinear sample of a field at a position in cell units (`x/h` from the grid origin).
    /// Returns the value and the min/max of the eight values it interpolates (the MacCormack limiter).
    fn sample(&self, f: Field, data: &[f64], p: [f64; 3]) -> (f64, f64, f64) {
        let off = OFFSET[f as usize];
        let q = [p[0] - off[0], p[1] - off[1], p[2] - off[2]];
        let b = [q[0].floor(), q[1].floor(), q[2].floor()];
        let fr = [q[0] - b[0], q[1] - b[1], q[2] - b[2]];
        let (i0, j0, k0) = (b[0] as i64, b[1] as i64, b[2] as i64);
        // Interior fast path: all eight samples are real lattice values, no ghosts to resolve.
        let [nx, ny, nz] = [
            self.grid.n[0] as i64,
            self.grid.n[1] as i64,
            self.grid.n[2] as i64,
        ];
        let dims = match f {
            Field::U => [nx + 1, ny, nz],
            Field::V => [nx, ny + 1, nz],
            Field::W => [nx, ny, nz + 1],
            Field::T => [nx, ny, nz],
        };
        if i0 >= 0 && j0 >= 0 && k0 >= 0 && i0 + 1 < dims[0] && j0 + 1 < dims[1] && k0 + 1 < dims[2]
        {
            let (sy, sz) = (dims[0] as usize, (dims[0] * dims[1]) as usize);
            let o = i0 as usize + sy * j0 as usize + sz * k0 as usize;
            let c = [
                data[o],
                data[o + 1],
                data[o + sy],
                data[o + sy + 1],
                data[o + sz],
                data[o + sz + 1],
                data[o + sz + sy],
                data[o + sz + sy + 1],
            ];
            let (gx, gy, gz) = (1.0 - fr[0], 1.0 - fr[1], 1.0 - fr[2]);
            let acc = gz * (gy * (gx * c[0] + fr[0] * c[1]) + fr[1] * (gx * c[2] + fr[0] * c[3]))
                + fr[2] * (gy * (gx * c[4] + fr[0] * c[5]) + fr[1] * (gx * c[6] + fr[0] * c[7]));
            let lo = c.iter().fold(f64::INFINITY, |m, &x| m.min(x));
            let hi = c.iter().fold(f64::NEG_INFINITY, |m, &x| m.max(x));
            return (acc, lo, hi);
        }
        let (mut acc, mut lo, mut hi) = (0.0, f64::INFINITY, f64::NEG_INFINITY);
        for (dk, wz) in [(0, 1.0 - fr[2]), (1, fr[2])] {
            for (dj, wy) in [(0, 1.0 - fr[1]), (1, fr[1])] {
                for (di, wx) in [(0, 1.0 - fr[0]), (1, fr[0])] {
                    let val = self.at(f, data, i0 + di, j0 + dj, k0 + dk);
                    acc += wx * wy * wz * val;
                    lo = lo.min(val);
                    hi = hi.max(val);
                }
            }
        }
        (acc, lo, hi)
    }

    /// The velocity at a position in cell units, in cells per second.
    fn velocity_cells(&self, p: [f64; 3]) -> [f64; 3] {
        let inv = 1.0 / self.grid.h;
        [
            self.sample(Field::U, &self.u, p).0 * inv,
            self.sample(Field::V, &self.v, p).0 * inv,
            self.sample(Field::W, &self.w, p).0 * inv,
        ]
    }

    /// Trace a point through the velocity field for `dt` (one Euler step), clamped to the domain.
    /// First-order on its own; the MacCormack pair it is used in is second-order in space and time.
    fn trace(&self, p: [f64; 3], dt: f64) -> [f64; 3] {
        let v = self.velocity_cells(p);
        self.clamp([p[0] + dt * v[0], p[1] + dt * v[1], p[2] + dt * v[2]])
    }

    fn clamp(&self, p: [f64; 3]) -> [f64; 3] {
        [
            p[0].clamp(0.0, self.grid.n[0] as f64),
            p[1].clamp(0.0, self.grid.n[1] as f64),
            p[2].clamp(0.0, self.grid.n[2] as f64),
        ]
    }

    /// The lattice dimensions of a field's samples.
    fn dims(&self, f: Field) -> [usize; 3] {
        let [nx, ny, nz] = self.grid.n;
        match f {
            Field::U => [nx + 1, ny, nz],
            Field::V => [nx, ny + 1, nz],
            Field::W => [nx, ny, nz + 1],
            Field::T => [nx, ny, nz],
        }
    }

    /// Which samples of a field are free unknowns (not pinned by a wall or a solid), in flat order.
    fn free_mask(&self, f: Field) -> Vec<bool> {
        let d = self.dims(f);
        let mut out = Vec::with_capacity(d[0] * d[1] * d[2]);
        for k in 0..d[2] {
            for j in 0..d[1] {
                for i in 0..d[0] {
                    let c = [i as i64, j as i64, k as i64];
                    out.push(match f {
                        Field::U => self.face_fixed(0, c).is_none(),
                        Field::V => self.face_fixed(1, c).is_none(),
                        Field::W => self.face_fixed(2, c).is_none(),
                        Field::T => !self.solid[self.ic(i, j, k)],
                    });
                }
            }
        }
        out
    }

    /// MacCormack advection of one field over `dt`: a semi-Lagrangian step, the same step run
    /// backwards to estimate its error, and half that error taken back out — second-order where the
    /// field is smooth. Where the correction would overshoot the values the forward step
    /// interpolated between, it is dropped (plain semi-Lagrangian), which keeps the scheme monotone.
    fn advect(&self, f: Field, free: &[bool], dt: f64) -> Vec<f64> {
        let src = self.data(f);
        let d = self.dims(f);
        let off = OFFSET[f as usize];
        let site = |n: usize| {
            [
                (n % d[0]) as f64 + off[0],
                ((n / d[0]) % d[1]) as f64 + off[1],
                (n / (d[0] * d[1])) as f64 + off[2],
            ]
        };
        let mut forward = src.to_vec();
        let mut bounds = vec![(0.0, 0.0); src.len()];
        par_chunks(&mut forward, &mut bounds, |start, fwd, bnd| {
            for (n, (val, bound)) in fwd.iter_mut().zip(bnd.iter_mut()).enumerate() {
                if free[start + n] {
                    let (v, lo, hi) = self.sample(f, src, self.trace(site(start + n), -dt));
                    *val = v;
                    *bound = (lo, hi);
                }
            }
        });
        let mut out = forward.clone();
        par_chunks(&mut out, &mut bounds, |start, o, bnd| {
            for (n, (val, &mut (lo, hi))) in o.iter_mut().zip(bnd.iter_mut()).enumerate() {
                if free[start + n] {
                    let (back, _, _) = self.sample(f, &forward, self.trace(site(start + n), dt));
                    let fwd = forward[start + n];
                    let corrected = fwd + 0.5 * (src[start + n] - back);
                    *val = if corrected < lo || corrected > hi {
                        fwd
                    } else {
                        corrected
                    };
                }
            }
        });
        out
    }

    /// One explicit diffusion sub-step of a velocity component with coefficient `c = ν·dt/h²`.
    fn diffuse_velocity(&self, f: Field, free: &[bool], c: f64) -> Vec<f64> {
        let src = self.data(f);
        let d = self.dims(f);
        let (sy, sz) = (d[0], d[0] * d[1]);
        let mut out = src.to_vec();
        for k in 0..d[2] {
            for j in 0..d[1] {
                for i in 0..d[0] {
                    let n = i + sy * j + sz * k;
                    if !free[n] {
                        continue;
                    }
                    let interior =
                        i > 0 && j > 0 && k > 0 && i + 1 < d[0] && j + 1 < d[1] && k + 1 < d[2];
                    let sum = if interior {
                        src[n - 1]
                            + src[n + 1]
                            + src[n - sy]
                            + src[n + sy]
                            + src[n - sz]
                            + src[n + sz]
                    } else {
                        let (i, j, k) = (i as i64, j as i64, k as i64);
                        self.at(f, src, i - 1, j, k)
                            + self.at(f, src, i + 1, j, k)
                            + self.at(f, src, i, j - 1, k)
                            + self.at(f, src, i, j + 1, k)
                            + self.at(f, src, i, j, k - 1)
                            + self.at(f, src, i, j, k + 1)
                    };
                    out[n] = src[n] + c * (sum - 6.0 * src[n]);
                }
            }
        }
        out
    }

    /// One explicit diffusion sub-step of temperature over `dt`, with the thermal conditions on
    /// walls and solid surfaces.
    fn diffuse_temperature(&self, dt: f64) -> Vec<f64> {
        let [nx, ny, nz] = self.grid.n;
        let h = self.grid.h;
        let c = self.params.alpha * dt / (h * h);
        let mut out = self.t.clone();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let id = self.ic(i, j, k);
                    if self.solid[id] {
                        continue;
                    }
                    let tc = self.t[id];
                    let (mut lap, mut film) = (0.0, 0.0);
                    let nbrs: [(i64, i64, i64, usize); 6] = [
                        (-1, 0, 0, 0),
                        (1, 0, 0, 1),
                        (0, -1, 0, 2),
                        (0, 1, 0, 3),
                        (0, 0, -1, 4),
                        (0, 0, 1, 5),
                    ];
                    for (di, dj, dk, wall) in nbrs {
                        let (a, b, cc) = (i as i64 + di, j as i64 + dj, k as i64 + dk);
                        let outside = a < 0
                            || b < 0
                            || cc < 0
                            || a >= nx as i64
                            || b >= ny as i64
                            || cc >= nz as i64;
                        let surface = if outside {
                            Some(self.params.walls[wall])
                        } else {
                            let nid = self.ic(a as usize, b as usize, cc as usize);
                            self.solid[nid].then(|| self.solid_heat[nid])
                        };
                        match surface {
                            None => {
                                lap += self.t[self.ic(a as usize, b as usize, cc as usize)] - tc
                            }
                            Some(Heat::Adiabatic) => {}
                            // The surface sits half a cell away: flux (T_s − T)/(h/2).
                            Some(Heat::Isothermal(ts)) => lap += 2.0 * (ts - tc),
                            Some(Heat::Film {
                                t_surface,
                                h_over_rho_cp,
                            }) => {
                                film += h_over_rho_cp * (t_surface - tc) / h;
                            }
                        }
                    }
                    out[id] = tc + c * lap + dt * film;
                }
            }
        }
        out
    }

    /// The divergence of the velocity field in each fluid cell [s⁻¹].
    pub fn divergence(&self) -> Vec<f64> {
        let [nx, ny, nz] = self.grid.n;
        let inv = 1.0 / self.grid.h;
        let mut div = vec![0.0; self.grid.len()];
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let id = self.ic(i, j, k);
                    if !self.solid[id] {
                        div[id] = inv
                            * (self.u[self.iu(i + 1, j, k)] - self.u[self.iu(i, j, k)]
                                + self.v[self.iv(i, j + 1, k)]
                                - self.v[self.iv(i, j, k)]
                                + self.w[self.iw(i, j, k + 1)]
                                - self.w[self.iw(i, j, k)]);
                    }
                }
            }
        }
        div
    }

    /// Project the velocity onto its divergence-free part: solve `A q = −h·∇·u` over the fluid
    /// cells (Neumann at every solid face) with MIC(0)-preconditioned conjugate gradients, then
    /// subtract the gradient of `q` from the free faces.
    fn project(&mut self) {
        let [nx, ny, nz] = self.grid.n;
        let n = self.grid.len();
        let h = self.grid.h;
        // Coupling to the +x/+y/+z neighbour (1 if both cells are fluid) and the diagonal.
        let (mut ax, mut ay, mut az, mut diag): (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) =
            (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let id = self.ic(i, j, k);
                    if self.solid[id] {
                        continue;
                    }
                    if i + 1 < nx && !self.solid[self.ic(i + 1, j, k)] {
                        ax[id] = 1.0;
                        diag[id] += 1.0;
                        diag[self.ic(i + 1, j, k)] += 1.0;
                    }
                    if j + 1 < ny && !self.solid[self.ic(i, j + 1, k)] {
                        ay[id] = 1.0;
                        diag[id] += 1.0;
                        diag[self.ic(i, j + 1, k)] += 1.0;
                    }
                    if k + 1 < nz && !self.solid[self.ic(i, j, k + 1)] {
                        az[id] = 1.0;
                        diag[id] += 1.0;
                        diag[self.ic(i, j, k + 1)] += 1.0;
                    }
                }
            }
        }
        let (sx, sy, sz) = (1usize, nx, nx * ny);

        let mut b: Vec<f64> = self.divergence().iter().map(|d| -h * d).collect();
        // The all-Neumann system is singular; make the right-hand side compatible (zero mean over the
        // fluid) so the residual can actually reach the tolerance.
        let active: Vec<usize> = (0..n)
            .filter(|&i| !self.solid[i] && diag[i] > 0.0)
            .collect();
        if active.is_empty() {
            return;
        }
        let mean = active.iter().map(|&i| b[i]).sum::<f64>() / active.len() as f64;
        for &i in &active {
            b[i] -= mean;
        }
        let bmax = active.iter().fold(0.0f64, |m, &i| m.max(b[i].abs()));
        if bmax == 0.0 {
            self.last_iterations = 0;
            return;
        }

        // MIC(0) preconditioner (Bridson, Fluid Simulation for Computer Graphics §5).
        let (tau, sigma) = (0.97, 0.25);
        let mut pre: Vec<f64> = vec![0.0; n];
        for &id in &active {
            let (mut e, d): (f64, f64) = (diag[id], diag[id]);
            if id >= sx && ax[id - sx] != 0.0 {
                let p = pre[id - sx];
                e -= (ax[id - sx] * p).powi(2)
                    + tau * ax[id - sx] * (ay[id - sx] + az[id - sx]) * p * p;
            }
            if id >= sy && ay[id - sy] != 0.0 {
                let p = pre[id - sy];
                e -= (ay[id - sy] * p).powi(2)
                    + tau * ay[id - sy] * (ax[id - sy] + az[id - sy]) * p * p;
            }
            if id >= sz && az[id - sz] != 0.0 {
                let p = pre[id - sz];
                e -= (az[id - sz] * p).powi(2)
                    + tau * az[id - sz] * (ax[id - sz] + ay[id - sz]) * p * p;
            }
            if e < sigma * d {
                e = d;
            }
            pre[id] = 1.0 / e.sqrt();
        }
        let apply_pre = |r: &[f64], z: &mut [f64], tmp: &mut [f64]| {
            for &id in &active {
                let mut t = r[id];
                if id >= sx && ax[id - sx] != 0.0 {
                    t += ax[id - sx] * pre[id - sx] * tmp[id - sx];
                }
                if id >= sy && ay[id - sy] != 0.0 {
                    t += ay[id - sy] * pre[id - sy] * tmp[id - sy];
                }
                if id >= sz && az[id - sz] != 0.0 {
                    t += az[id - sz] * pre[id - sz] * tmp[id - sz];
                }
                tmp[id] = t * pre[id];
            }
            for &id in active.iter().rev() {
                let mut t = tmp[id];
                if ax[id] != 0.0 {
                    t += ax[id] * pre[id] * z[id + sx];
                }
                if ay[id] != 0.0 {
                    t += ay[id] * pre[id] * z[id + sy];
                }
                if az[id] != 0.0 {
                    t += az[id] * pre[id] * z[id + sz];
                }
                z[id] = t * pre[id];
            }
        };
        let apply_a = |x: &[f64], y: &mut [f64]| {
            for &id in &active {
                let mut acc = diag[id] * x[id];
                if ax[id] != 0.0 {
                    acc -= x[id + sx];
                }
                if ay[id] != 0.0 {
                    acc -= x[id + sy];
                }
                if az[id] != 0.0 {
                    acc -= x[id + sz];
                }
                if id >= sx && ax[id - sx] != 0.0 {
                    acc -= x[id - sx];
                }
                if id >= sy && ay[id - sy] != 0.0 {
                    acc -= x[id - sy];
                }
                if id >= sz && az[id - sz] != 0.0 {
                    acc -= x[id - sz];
                }
                y[id] = acc;
            }
        };
        let dot = |a: &[f64], c: &[f64]| active.iter().map(|&i| a[i] * c[i]).sum::<f64>();

        // Warm start from the previous solution (the pressure field changes slowly step to step).
        let mut q = std::mem::take(&mut self.q);
        for (i, x) in q.iter_mut().enumerate() {
            if self.solid[i] || diag[i] == 0.0 {
                *x = 0.0;
            }
        }
        let (mut z, mut tmp, mut s, mut a_s) =
            (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        apply_a(&q, &mut a_s);
        let mut r = b;
        for &i in &active {
            r[i] -= a_s[i];
        }
        let limit = self.params.tol * bmax;
        self.last_iterations = 0;
        if active.iter().fold(0.0f64, |m, &i| m.max(r[i].abs())) > limit {
            apply_pre(&r, &mut z, &mut tmp);
            s.copy_from_slice(&z);
            let mut rho = dot(&z, &r);
            for it in 1..=4 * n.max(100) {
                apply_a(&s, &mut a_s);
                let alpha = rho / dot(&s, &a_s);
                if !alpha.is_finite() {
                    break;
                }
                for &i in &active {
                    q[i] += alpha * s[i];
                    r[i] -= alpha * a_s[i];
                }
                self.last_iterations = it;
                if active.iter().fold(0.0f64, |m, &i| m.max(r[i].abs())) <= limit {
                    break;
                }
                apply_pre(&r, &mut z, &mut tmp);
                let rho_new = dot(&z, &r);
                let beta = rho_new / rho;
                rho = rho_new;
                for &i in &active {
                    s[i] = z[i] + beta * s[i];
                }
            }
        }

        // u ← u − ∇q on every free face.
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let id = self.ic(i, j, k);
                    if ax[id] != 0.0 {
                        let f = self.iu(i + 1, j, k);
                        self.u[f] -= q[id + sx] - q[id];
                    }
                    if ay[id] != 0.0 {
                        let f = self.iv(i, j + 1, k);
                        self.v[f] -= q[id + sy] - q[id];
                    }
                    if az[id] != 0.0 {
                        let f = self.iw(i, j, k + 1);
                        self.w[f] -= q[id + sz] - q[id];
                    }
                }
            }
        }
        self.q = q;
    }

    /// Give solid cells the mean temperature of their fluid neighbours, so interpolation next to a
    /// surface sees no spurious gradient (the surface's heat enters through `diffuse_temperature`).
    fn fill_solid_temperature(&mut self) {
        let [nx, ny, nz] = self.grid.n;
        let mut fill = Vec::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let id = self.ic(i, j, k);
                    if !self.solid[id] {
                        continue;
                    }
                    let (mut sum, mut count) = (0.0, 0.0);
                    for (di, dj, dk) in [
                        (-1, 0, 0),
                        (1, 0, 0),
                        (0, -1, 0),
                        (0, 1, 0),
                        (0, 0, -1),
                        (0, 0, 1),
                    ] {
                        let (a, b, c) = (i as i64 + di, j as i64 + dj, k as i64 + dk);
                        if !self.is_solid(a, b, c) {
                            sum += self.t[self.ic(a as usize, b as usize, c as usize)];
                            count += 1.0;
                        }
                    }
                    if count > 0.0 {
                        fill.push((id, sum / count));
                    }
                }
            }
        }
        for (id, t) in fill {
            self.t[id] = t;
        }
    }

    /// Advance the fluid by `dt`.
    pub fn step(&mut self, dt: f64) {
        let h = self.grid.h;
        self.enforce_solid_faces();
        self.fill_solid_temperature();

        // Advection — every field is carried by the same start-of-step velocity.
        let clock = std::time::Instant::now();
        let free = [
            self.free_mask(Field::U),
            self.free_mask(Field::V),
            self.free_mask(Field::W),
            self.free_mask(Field::T),
        ];
        let (u, v, w, t) = (
            self.advect(Field::U, &free[0], dt),
            self.advect(Field::V, &free[1], dt),
            self.advect(Field::W, &free[2], dt),
            self.advect(Field::T, &free[3], dt),
        );
        self.u = u;
        self.v = v;
        self.w = w;
        self.t = t;

        self.timing[0] += clock.elapsed().as_secs_f64();
        let clock = std::time::Instant::now();

        // Diffusion, explicit, sub-stepped to stay inside dt ≤ h²/(6·max(ν, α)).
        let kappa = self.params.nu.max(self.params.alpha);
        let sub = ((dt * 6.0 * kappa / (h * h)) / 0.9).ceil().max(1.0) as usize;
        let ddt = dt / sub as f64;
        for _ in 0..sub {
            let c = self.params.nu * ddt / (h * h);
            let (u, v, w) = (
                self.diffuse_velocity(Field::U, &free[0], c),
                self.diffuse_velocity(Field::V, &free[1], c),
                self.diffuse_velocity(Field::W, &free[2], c),
            );
            let t = self.diffuse_temperature(ddt);
            self.u = u;
            self.v = v;
            self.w = w;
            self.t = t;
        }

        // Buoyancy on the free z-faces: gβ(T − T₀), T averaged from the two cells the face joins.
        let [nx, ny, nz] = self.grid.n;
        let gb = self.params.g * self.params.beta;
        for k in 1..nz {
            for j in 0..ny {
                for i in 0..nx {
                    if free[2][self.iw(i, j, k)] {
                        let tf = 0.5 * (self.t[self.ic(i, j, k - 1)] + self.t[self.ic(i, j, k)]);
                        let id = self.iw(i, j, k);
                        self.w[id] += dt * gb * (tf - self.params.t_ref);
                    }
                }
            }
        }

        self.enforce_solid_faces();
        self.timing[1] += clock.elapsed().as_secs_f64();
        let clock = std::time::Instant::now();
        self.project();
        self.timing[2] += clock.elapsed().as_secs_f64();
    }

    /// The largest stable advective time step for a Courant number `cfl`, or `fallback` at rest.
    pub fn cfl_dt(&self, cfl: f64, fallback: f64) -> f64 {
        let vmax = self
            .u
            .iter()
            .chain(&self.v)
            .chain(&self.w)
            .fold(0.0f64, |m, x| m.max(x.abs()));
        if vmax > 0.0 {
            (cfl * self.grid.h / vmax).min(fallback)
        } else {
            fallback
        }
    }

    /// The mass anomaly of each cell [kg]: `(ρ(T) − ρ(T₀))·h³` through the equation of state, zero
    /// in solid cells. This — not the solver's constant working density — is what gravitates.
    pub fn mass_anomaly(&self, air: &Air) -> Vec<f64> {
        let rho0 = air.density(self.params.t_ref);
        let vol = self.grid.cell_volume();
        self.t
            .iter()
            .zip(&self.solid)
            .map(|(&t, &solid)| {
                if solid {
                    0.0
                } else {
                    (air.density(t) - rho0) * vol
                }
            })
            .collect()
    }

    /// The total kinetic energy per unit density, `½ Σ |u|² h³`.
    pub fn kinetic_energy(&self) -> f64 {
        let sq = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>();
        0.5 * (sq(&self.u) + sq(&self.v) + sq(&self.w)) * self.grid.cell_volume()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(state: &mut u64) -> f64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (*state >> 11) as f64 / 9007199254740992.0
    }

    /// Room air with the pressure solve driven to rounding — for tests of the projection itself.
    fn tight(t_ref: f64) -> FluidParams {
        FluidParams {
            tol: 1e-12,
            ..FluidParams::air(t_ref)
        }
    }

    #[test]
    fn projection_removes_divergence() {
        // A random velocity field around a solid block: after projection the divergence in every
        // fluid cell is at the solver tolerance, and no face touching a solid has moved.
        let grid = Grid3::new([0.0; 3], 0.1, [12, 10, 14]);
        let mut f = Fluid::new(grid, tight(293.0));
        for k in 5..8 {
            for j in 3..6 {
                for i in 4..9 {
                    let id = f.ic(i, j, k);
                    f.solid[id] = true;
                }
            }
        }
        let mut s = 99u64;
        for x in f.u.iter_mut().chain(f.v.iter_mut()).chain(f.w.iter_mut()) {
            *x = lcg(&mut s) - 0.5;
        }
        f.enforce_solid_faces();
        let before = f.divergence().iter().fold(0.0f64, |m, d| m.max(d.abs()));
        f.project();
        let after = f.divergence().iter().fold(0.0f64, |m, d| m.max(d.abs()));
        assert!(after <= 1e-8 * before, "divergence {before:e} → {after:e}");
        assert_eq!(f.face_fixed(0, [4, 4, 6]), Some(0.0));
        assert_eq!(f.u[f.iu(4, 4, 6)], 0.0);
    }

    #[test]
    fn stratified_fluid_at_rest_stays_at_rest() {
        // A stable linear stratification is in hydrostatic balance: buoyancy is a pure gradient, so
        // the projection must absorb it exactly and no flow may appear.
        let grid = Grid3::new([0.0; 3], 0.1, [8, 8, 12]);
        let mut f = Fluid::new(grid, tight(293.0));
        for k in 0..12 {
            for j in 0..8 {
                for i in 0..8 {
                    let id = f.ic(i, j, k);
                    f.t[id] = 293.0 + 2.0 * (k as f64 + 0.5) * 0.1;
                }
            }
        }
        for _ in 0..20 {
            f.step(0.05);
        }
        let vmax =
            f.u.iter()
                .chain(&f.v)
                .chain(&f.w)
                .fold(0.0f64, |m, x| m.max(x.abs()));
        assert!(vmax <= 1e-9, "spurious current {vmax:e} m/s");
    }

    /// The differentially heated square cavity (de Vahl Davis 1983): hot wall at −x, cold at +x,
    /// adiabatic floor and ceiling, Pr = 0.71. Returns the mean Nusselt number on the hot and cold
    /// walls at steady state.
    fn heated_cavity(rayleigh: f64, n: usize) -> (f64, f64) {
        let pr = 0.71;
        let h = 1.0 / n as f64;
        let grid = Grid3::new([0.0; 3], h, [n, 1, n]);
        // Non-dimensional: L = 1, α = 1, ΔT = 1 ⇒ ν = Pr, gβ = Ra·Pr.
        let params = FluidParams {
            nu: pr,
            alpha: 1.0,
            g: rayleigh * pr,
            beta: 1.0,
            t_ref: 0.0,
            walls: [
                Heat::Isothermal(0.5),
                Heat::Isothermal(-0.5),
                Heat::Adiabatic,
                Heat::Adiabatic,
                Heat::Adiabatic,
                Heat::Adiabatic,
            ],
            free_slip: [false, true, false],
            tol: 1e-10,
        };
        let mut f = Fluid::new(grid, params);
        let dt = 2e-4;
        for _ in 0..(0.5 / dt) as usize {
            f.step(dt);
        }
        // Second-order one-sided wall gradient from the wall value and the first two cells.
        let nu = |wall_t: f64, first: usize, second: usize| {
            (0..n)
                .map(|k| {
                    let (t0, t1) = (f.t[f.ic(first, 0, k)], f.t[f.ic(second, 0, k)]);
                    (-8.0 * wall_t + 9.0 * t0 - t1) / (3.0 * h)
                })
                .sum::<f64>()
                / n as f64
        };
        (-nu(0.5, 0, 1), nu(-0.5, n - 1, n - 2))
    }

    #[test]
    fn heated_cavity_matches_de_vahl_davis() {
        // The benchmark mean Nusselt numbers: 1.118 at Ra = 10³, 2.243 at Ra = 10⁴.
        for (ra, want) in [(1e3, 1.118), (1e4, 2.243)] {
            let (hot, cold) = heated_cavity(ra, 32);
            eprintln!("heated cavity Ra={ra:e}: Nu hot {hot:.4} cold {cold:.4} (benchmark {want})");
            assert!(
                (hot - cold).abs() / want <= 0.01,
                "heat in ≠ heat out: {hot} vs {cold}"
            );
            assert!(
                (hot - want).abs() / want <= 0.02,
                "Ra={ra:e}: Nu {hot} vs {want}"
            );
        }
    }

    #[test]
    fn equation_of_state_reads_temperature() {
        // The Boussinesq guard: a warm cell is lighter than ambient by the ideal-gas amount, even
        // though the solver's own working density never changes.
        let grid = Grid3::new([0.0; 3], 0.1, [4, 4, 4]);
        let mut f = Fluid::new(grid, FluidParams::air(293.15));
        let id = f.ic(2, 2, 2);
        f.t[id] = 303.15;
        let air = Air::default();
        let dm = f.mass_anomaly(&air);
        let want = (air.density(303.15) - air.density(293.15)) * 1e-3;
        assert!(
            want < 0.0 && (dm[id] - want).abs() <= 1e-15,
            "δm {} vs {want}",
            dm[id]
        );
        assert!(
            (air.density(293.15) - 1.204).abs() <= 2e-3,
            "ρ_air(20 °C) = {}",
            air.density(293.15)
        );
        assert!(dm.iter().enumerate().all(|(i, &m)| i == id || m == 0.0));
    }
}
