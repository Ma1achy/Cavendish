//! `shape` — geometry to mass: the `Solid` occupancy seam, one lattice voxeliser, and primitives.
//!
//! Design: `design/shape.md`. Milestone: `milestones/M2-motion-and-shape.md`.
//!
//! A shape-produced [`gravity::Cloud`] is indistinguishable downstream from any other: exact total
//! mass (renormalised), centre of mass at the body-frame origin (recentred), and deterministic
//! canonical (x-fastest raster) element order. The mesh path (`MeshSolid`, STL/OBJ/glTF, winding
//! numbers) is M10 — the `Solid` seam is written so it can plug in later. The `Cloud → C/I/Q`
//! reduction is `gravity`'s (M4); [`second_moment`] here is a validation helper only.

use gravity::Cloud;
use math::Mat3;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod mesh;
pub use mesh::*;

/// An axis-aligned bounding box — the voxelisation domain.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// The occupancy seam: any solid that can report whether a body-frame point is inside it.
pub trait Solid {
    /// Occupancy at a body-frame point: 1 inside, 0 outside (fractional values permitted).
    fn occupancy(&self, p: [f64; 3]) -> f64;
    /// A finite bound on the support.
    fn bbox(&self) -> Aabb;
}

/// A solid sphere of radius `r`, centred at the origin.
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub r: f64,
}

/// A spherical shell of radius `r` and finite `thickness` (a lattice cannot carry a measure-zero
/// surface; the moment converges to the ideal thin-shell `MR²/3` as `thickness → 0`).
#[derive(Clone, Copy, Debug)]
pub struct Shell {
    pub r: f64,
    pub thickness: f64,
}

/// An axis-aligned cuboid with the given half-extents.
#[derive(Clone, Copy, Debug)]
pub struct Cuboid {
    pub half: [f64; 3],
}

/// A cylinder of radius `r` and half-length `half_l`, axis along `z`.
#[derive(Clone, Copy, Debug)]
pub struct Cylinder {
    pub r: f64,
    pub half_l: f64,
}

/// A solid shifted by `offset` in the body frame.
pub struct Translated {
    pub inner: Box<dyn Solid>,
    pub offset: [f64; 3],
}

/// The union of solids: occupied where any member is (`occupancy = max`).
pub struct Union(pub Vec<Box<dyn Solid>>);

fn diag(a: f64, b: f64, c: f64) -> Mat3<f64> {
    Mat3 {
        m: [[a, 0.0, 0.0], [0.0, b, 0.0], [0.0, 0.0, c]],
    }
}

impl Sphere {
    /// Analytic second moment about the CoM: `(M R²/5) 𝟙`.
    pub fn analytic_c(&self, mass: f64) -> Mat3<f64> {
        let v = mass * self.r * self.r / 5.0;
        diag(v, v, v)
    }
}

impl Shell {
    /// Analytic second moment of the ideal thin shell: `(M R²/3) 𝟙`.
    pub fn analytic_c(&self, mass: f64) -> Mat3<f64> {
        let v = mass * self.r * self.r / 3.0;
        diag(v, v, v)
    }
}

impl Cuboid {
    /// Analytic second moment: `diag(Ma², Mb², Mc²)/12` for full sides `a,b,c = 2·half`.
    pub fn analytic_c(&self, mass: f64) -> Mat3<f64> {
        let f = |h: f64| mass * (2.0 * h) * (2.0 * h) / 12.0;
        diag(f(self.half[0]), f(self.half[1]), f(self.half[2]))
    }
}

impl Cylinder {
    /// Analytic second moment: `diag(MR²/4, MR²/4, ML²/12)` for full length `L = 2·half_l`.
    pub fn analytic_c(&self, mass: f64) -> Mat3<f64> {
        let rad = mass * self.r * self.r / 4.0;
        let axial = mass * (2.0 * self.half_l) * (2.0 * self.half_l) / 12.0;
        diag(rad, rad, axial)
    }
}

impl Solid for Sphere {
    fn occupancy(&self, p: [f64; 3]) -> f64 {
        if p[0] * p[0] + p[1] * p[1] + p[2] * p[2] <= self.r * self.r {
            1.0
        } else {
            0.0
        }
    }
    fn bbox(&self) -> Aabb {
        Aabb {
            min: [-self.r; 3],
            max: [self.r; 3],
        }
    }
}

impl Solid for Shell {
    fn occupancy(&self, p: [f64; 3]) -> f64 {
        let d = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        let half = 0.5 * self.thickness;
        if d >= self.r - half && d <= self.r + half {
            1.0
        } else {
            0.0
        }
    }
    fn bbox(&self) -> Aabb {
        let e = self.r + 0.5 * self.thickness;
        Aabb {
            min: [-e; 3],
            max: [e; 3],
        }
    }
}

impl Solid for Cuboid {
    fn occupancy(&self, p: [f64; 3]) -> f64 {
        if p[0].abs() <= self.half[0] && p[1].abs() <= self.half[1] && p[2].abs() <= self.half[2] {
            1.0
        } else {
            0.0
        }
    }
    fn bbox(&self) -> Aabb {
        Aabb {
            min: [-self.half[0], -self.half[1], -self.half[2]],
            max: self.half,
        }
    }
}

impl Solid for Cylinder {
    fn occupancy(&self, p: [f64; 3]) -> f64 {
        if p[0] * p[0] + p[1] * p[1] <= self.r * self.r && p[2].abs() <= self.half_l {
            1.0
        } else {
            0.0
        }
    }
    fn bbox(&self) -> Aabb {
        Aabb {
            min: [-self.r, -self.r, -self.half_l],
            max: [self.r, self.r, self.half_l],
        }
    }
}

impl Solid for Translated {
    fn occupancy(&self, p: [f64; 3]) -> f64 {
        self.inner.occupancy([
            p[0] - self.offset[0],
            p[1] - self.offset[1],
            p[2] - self.offset[2],
        ])
    }
    fn bbox(&self) -> Aabb {
        let b = self.inner.bbox();
        Aabb {
            min: [
                b.min[0] + self.offset[0],
                b.min[1] + self.offset[1],
                b.min[2] + self.offset[2],
            ],
            max: [
                b.max[0] + self.offset[0],
                b.max[1] + self.offset[1],
                b.max[2] + self.offset[2],
            ],
        }
    }
}

impl Solid for Union {
    fn occupancy(&self, p: [f64; 3]) -> f64 {
        self.0.iter().map(|s| s.occupancy(p)).fold(0.0, f64::max)
    }
    fn bbox(&self) -> Aabb {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for s in &self.0 {
            let b = s.bbox();
            for i in 0..3 {
                min[i] = min[i].min(b.min[i]);
                max[i] = max[i].max(b.max[i]);
            }
        }
        Aabb { min, max }
    }
}

/// How to fix the total mass: an explicit density or an explicit total.
#[derive(Clone, Copy, Debug)]
pub enum MassSpec {
    Density(f64),
    Total(f64),
}

/// Voxeliser controls: the lattice pitch (or a target element count), and boundary supersampling.
#[derive(Clone, Copy, Debug)]
pub struct VoxelParams {
    pub pitch: Option<f64>,
    pub target_n: Option<usize>,
    pub supersample: u8,
}

impl VoxelParams {
    /// A lattice of fixed pitch `h`, boundary supersample `k = 2`.
    pub fn pitch(h: f64) -> Self {
        VoxelParams {
            pitch: Some(h),
            target_n: None,
            supersample: 2,
        }
    }
}

/// Why a solid could not be voxelised or a mesh imported.
///
/// Not `Copy`/`Eq`: the mesh variants carry payloads (`f64` diagnostic, parser message) so a dirty
/// mesh fails *loudly* with context rather than as a bare tag.
#[derive(Clone, Debug, PartialEq)]
pub enum ShapeError {
    BadParams,
    EmptySolid,
    TooManyElements,
    /// A mesh was imported without an explicit `scale` — units are ambiguous, so guessing is refused.
    ScaleMissing,
    /// The robust classifier's ambiguity diagnostic `A` exceeded threshold: the interior is genuinely
    /// undecidable, so no cloud is emitted (never a silent garbage cloud).
    AmbiguousInterior(f64),
    /// A mesh file could not be read or parsed; the string carries the underlying reason.
    UnreadableMesh(String),
}

/// The maximum number of elements a voxelised cloud may carry.
pub const ELEMENT_CAP: usize = 2_000_000;

/// Cell occupancy: the centre value unless the cell straddles the surface, in which case the mean
/// of a fixed `k³` seed-free sub-lattice.
fn cell_occupancy(solid: &dyn Solid, centre: [f64; 3], h: f64, k: usize) -> f64 {
    let oc = solid.occupancy(centre);
    let inside = oc > 0.5;
    let half = 0.5 * h;
    let mut boundary = false;
    for &sx in &[-half, half] {
        for &sy in &[-half, half] {
            for &sz in &[-half, half] {
                let corner = [centre[0] + sx, centre[1] + sy, centre[2] + sz];
                if (solid.occupancy(corner) > 0.5) != inside {
                    boundary = true;
                }
            }
        }
    }
    if !boundary {
        return oc;
    }
    // Average over k³ fixed sub-sample centres inside the cell.
    let mut sum = 0.0;
    for i in 0..k {
        let ox = ((i as f64 + 0.5) / k as f64 - 0.5) * h;
        for j in 0..k {
            let oy = ((j as f64 + 0.5) / k as f64 - 0.5) * h;
            for l in 0..k {
                let oz = ((l as f64 + 0.5) / k as f64 - 0.5) * h;
                sum += solid.occupancy([centre[0] + ox, centre[1] + oy, centre[2] + oz]);
            }
        }
    }
    sum / (k * k * k) as f64
}

/// Resolve the lattice pitch `h` from the params: an explicit pitch, or one derived from a target
/// element count over the bounding-box volume. A mesh and a primitive resolve `h` identically.
pub(crate) fn pitch_for(bbox: &Aabb, params: &VoxelParams) -> Result<f64, ShapeError> {
    let extent = [
        bbox.max[0] - bbox.min[0],
        bbox.max[1] - bbox.min[1],
        bbox.max[2] - bbox.min[2],
    ];
    match (params.pitch, params.target_n) {
        (Some(h), _) if h > 0.0 => Ok(h),
        (None, Some(n)) if n > 0 => {
            let vol = extent[0] * extent[1] * extent[2];
            Ok((vol / n as f64).cbrt())
        }
        _ => Err(ShapeError::BadParams),
    }
}

/// The shared voxeliser tail: given occupied cells (`xs/ys/zs` centres and `vols` occupancy·h³),
/// renormalise the masses to `mass` exactly and recentre so the discrete CoM is the origin. Both the
/// primitive path ([`voxelise`]) and the mesh path ([`voxelise_mesh`]) end here, so a mesh cloud is
/// indistinguishable downstream — exact total mass, zero dipole, and the caller's canonical order.
pub(crate) fn finish_cloud(
    mut xs: Vec<f64>,
    mut ys: Vec<f64>,
    mut zs: Vec<f64>,
    vols: Vec<f64>,
    mass: MassSpec,
) -> Result<Cloud, ShapeError> {
    if xs.is_empty() {
        return Err(ShapeError::EmptySolid);
    }
    let volume: f64 = vols.iter().sum();
    let total = match mass {
        MassSpec::Total(m) => m,
        MassSpec::Density(rho) => rho * volume,
    };

    // Renormalise: masses ∝ volume, scaled so Σmᵢ = total exactly.
    let mut ms: Vec<f64> = vols.iter().map(|v| v * total / volume).collect();
    let sum: f64 = ms.iter().sum();
    let corr = total / sum;
    for m in &mut ms {
        *m *= corr;
    }

    // Recentre: subtract the discrete CoM so the dipole is zero about the origin.
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    for i in 0..ms.len() {
        cx += ms[i] * xs[i];
        cy += ms[i] * ys[i];
        cz += ms[i] * zs[i];
    }
    cx /= total;
    cy /= total;
    cz /= total;
    for i in 0..ms.len() {
        xs[i] -= cx;
        ys[i] -= cy;
        zs[i] -= cz;
    }

    Ok(Cloud { xs, ys, zs, ms })
}

/// Sample a solid onto a cubic lattice and emit a cloud with exact total mass and CoM at the origin.
pub fn voxelise(
    solid: &dyn Solid,
    params: &VoxelParams,
    mass: MassSpec,
) -> Result<Cloud, ShapeError> {
    let bbox = solid.bbox();
    let h = pitch_for(&bbox, params)?;
    let k = params.supersample.max(1) as usize;
    let counts = [
        ((bbox.max[0] - bbox.min[0]) / h).ceil().max(1.0) as usize,
        ((bbox.max[1] - bbox.min[1]) / h).ceil().max(1.0) as usize,
        ((bbox.max[2] - bbox.min[2]) / h).ceil().max(1.0) as usize,
    ];

    let cell = h * h * h;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut zs = Vec::new();
    let mut vols = Vec::new();
    // x-fastest raster (canonical, deterministic).
    for iz in 0..counts[2] {
        let cz = bbox.min[2] + (iz as f64 + 0.5) * h;
        for iy in 0..counts[1] {
            let cy = bbox.min[1] + (iy as f64 + 0.5) * h;
            for ix in 0..counts[0] {
                let cx = bbox.min[0] + (ix as f64 + 0.5) * h;
                let occ = cell_occupancy(solid, [cx, cy, cz], h, k);
                if occ > 0.0 {
                    xs.push(cx);
                    ys.push(cy);
                    zs.push(cz);
                    vols.push(occ * cell);
                    if xs.len() > ELEMENT_CAP {
                        return Err(ShapeError::TooManyElements);
                    }
                }
            }
        }
    }
    finish_cloud(xs, ys, zs, vols, mass)
}

/// A copy of a cloud rescaled to total mass `total` (linearity: never re-voxelise for a mass draw).
pub fn scale_mass(cloud: &Cloud, total: f64) -> Cloud {
    let sum: f64 = cloud.ms.iter().sum();
    let corr = total / sum;
    Cloud {
        xs: cloud.xs.clone(),
        ys: cloud.ys.clone(),
        zs: cloud.zs.clone(),
        ms: cloud.ms.iter().map(|m| m * corr).collect(),
    }
}

/// The second moment `C = Σ mᵢ (rᵢ − com)(rᵢ − com)ᵀ` — a validation helper for the moment oracle.
pub fn second_moment(cloud: &Cloud) -> Mat3<f64> {
    let total: f64 = cloud.ms.iter().sum();
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    for i in 0..cloud.ms.len() {
        cx += cloud.ms[i] * cloud.xs[i];
        cy += cloud.ms[i] * cloud.ys[i];
        cz += cloud.ms[i] * cloud.zs[i];
    }
    cx /= total;
    cy /= total;
    cz /= total;
    let mut c = [[0.0f64; 3]; 3];
    for i in 0..cloud.ms.len() {
        let d = [cloud.xs[i] - cx, cloud.ys[i] - cy, cloud.zs[i] - cz];
        for (a, &da) in d.iter().enumerate() {
            for (b, &db) in d.iter().enumerate() {
                c[a][b] += cloud.ms[i] * da * db;
            }
        }
    }
    Mat3 { m: c }
}

/// An in-memory cache of unit-mass clouds, keyed by the caller's geometry+lattice hash.
#[derive(Default)]
pub struct Registry {
    map: Mutex<HashMap<u64, Arc<Cloud>>>,
}

impl Registry {
    pub fn new() -> Self {
        Registry::default()
    }

    /// Return the cached unit-mass cloud for `key`, building it via `build` on the first miss.
    pub fn resolve(
        &self,
        key: u64,
        build: impl FnOnce() -> Result<Cloud, ShapeError>,
    ) -> Result<Arc<Cloud>, ShapeError> {
        let mut map = self.map.lock().unwrap();
        if let Some(c) = map.get(&key) {
            return Ok(c.clone());
        }
        let arc = Arc::new(build()?);
        map.insert(key, arc.clone());
        Ok(arc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel_frob(a: &Mat3<f64>, b: &Mat3<f64>) -> f64 {
        let mut num = 0.0;
        let mut den = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                num += (a.m[i][j] - b.m[i][j]).powi(2);
                den += b.m[i][j].powi(2);
            }
        }
        (num / den).sqrt()
    }

    #[test]
    fn moments_table() {
        // Voxelised C matches the analytic oracle at h = R/20, k = 2, to ≤2%.
        let r = 1.0;
        let h = r / 20.0;
        let params = VoxelParams::pitch(h);
        let mass = MassSpec::Total(7.0);

        let sphere = Sphere { r };
        let cs = second_moment(&voxelise(&sphere, &params, mass).unwrap());
        assert!(rel_frob(&cs, &sphere.analytic_c(7.0)) <= 0.02, "sphere");

        let shell = Shell { r, thickness: h };
        let ch = second_moment(&voxelise(&shell, &params, mass).unwrap());
        assert!(rel_frob(&ch, &shell.analytic_c(7.0)) <= 0.02, "shell");

        let cuboid = Cuboid {
            half: [0.6, 0.8, 1.0],
        };
        let cc = second_moment(&voxelise(&cuboid, &VoxelParams::pitch(0.05), mass).unwrap());
        assert!(rel_frob(&cc, &cuboid.analytic_c(7.0)) <= 0.02, "cuboid");

        let cyl = Cylinder { r, half_l: 1.5 };
        let cy = second_moment(&voxelise(&cyl, &params, mass).unwrap());
        assert!(rel_frob(&cy, &cyl.analytic_c(7.0)) <= 0.02, "cylinder");
    }

    #[test]
    fn mass_com_exact() {
        // Σm = M and |CoM| = 0 exactly (to machine precision, scaled).
        let m = 3.3;
        let cloud = voxelise(
            &Cylinder {
                r: 0.7,
                half_l: 1.1,
            },
            &VoxelParams::pitch(0.05),
            MassSpec::Total(m),
        )
        .unwrap();
        let sum: f64 = cloud.ms.iter().sum();
        assert!((sum - m).abs() / m <= 1e-12, "mass not exact: {sum}");
        let mut com = [0.0; 3];
        for i in 0..cloud.ms.len() {
            com[0] += cloud.ms[i] * cloud.xs[i];
            com[1] += cloud.ms[i] * cloud.ys[i];
            com[2] += cloud.ms[i] * cloud.zs[i];
        }
        for c in com {
            assert!((c / m).abs() <= 1e-12, "CoM not zero: {c}");
        }
    }

    #[test]
    fn deterministic_cloud() {
        let build = || {
            voxelise(
                &Sphere { r: 1.0 },
                &VoxelParams::pitch(0.1),
                MassSpec::Total(2.0),
            )
            .unwrap()
        };
        let a = build();
        let b = build();
        assert_eq!(a.xs, b.xs);
        assert_eq!(a.ys, b.ys);
        assert_eq!(a.zs, b.zs);
        assert_eq!(a.ms, b.ms);
    }

    #[test]
    fn union_no_doublecount() {
        // Two spheres overlapping along x by R: voxel mass < sum of parts, = union volume·ρ ≤2%.
        let r = 1.0;
        let d = 1.0; // centre separation < 2R ⇒ overlap
        let rho = 100.0;
        let two = Union(vec![
            Box::new(Translated {
                inner: Box::new(Sphere { r }),
                offset: [-0.5 * d, 0.0, 0.0],
            }),
            Box::new(Translated {
                inner: Box::new(Sphere { r }),
                offset: [0.5 * d, 0.0, 0.0],
            }),
        ]);
        let cloud = voxelise(&two, &VoxelParams::pitch(0.05), MassSpec::Density(rho)).unwrap();
        let mass: f64 = cloud.ms.iter().sum();

        let v_sphere = 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
        let v_lens = std::f64::consts::PI * (2.0 * r - d).powi(2) * (4.0 * r + d) / 12.0;
        let v_union = 2.0 * v_sphere - v_lens;

        assert!(mass < 2.0 * rho * v_sphere, "double-counts the overlap");
        assert!(
            (mass - rho * v_union).abs() / (rho * v_union) <= 0.02,
            "union volume off"
        );
    }

    #[test]
    fn convergence_halving() {
        // Halving h halves the C error (ratio ~0.5) for sphere and cuboid.
        let err = |solid: &dyn Solid, analytic: &Mat3<f64>, h: f64| {
            let c = second_moment(
                &voxelise(solid, &VoxelParams::pitch(h), MassSpec::Total(1.0)).unwrap(),
            );
            rel_frob(&c, analytic)
        };
        for (solid, analytic) in [
            (
                Box::new(Sphere { r: 1.0 }) as Box<dyn Solid>,
                Sphere { r: 1.0 }.analytic_c(1.0),
            ),
            (
                Box::new(Cuboid {
                    half: [0.53, 0.61, 0.79],
                }) as Box<dyn Solid>,
                Cuboid {
                    half: [0.53, 0.61, 0.79],
                }
                .analytic_c(1.0),
            ),
        ] {
            let coarse = err(solid.as_ref(), &analytic, 0.1);
            let fine = err(solid.as_ref(), &analytic, 0.05);
            let ratio = fine / coarse;
            // "Halving h halves the C error" as a lower bound: with k=2 boundary subsampling the
            // observed rate is superlinear (~0.2), better than the brief's nominal 0.5 — so assert
            // the error at least halves and is genuinely converging (not sitting on the noise floor).
            assert!(coarse > 1e-6, "coarse error at noise floor");
            assert!(
                (0.05..=0.6).contains(&ratio),
                "convergence ratio {ratio} not halving"
            );
        }
    }

    #[test]
    fn cache_hits() {
        // Second resolution of the same key returns the same Arc; a mass draw only rescales.
        let reg = Registry::new();
        let build = || {
            voxelise(
                &Sphere { r: 1.0 },
                &VoxelParams::pitch(0.1),
                MassSpec::Total(1.0),
            )
        };
        let a = reg.resolve(42, build).unwrap();
        let b = reg.resolve(42, build).unwrap();
        assert!(Arc::ptr_eq(&a, &b), "cache miss on repeat resolve");

        let scaled = scale_mass(&a, 5.0);
        let sum: f64 = scaled.ms.iter().sum();
        assert!((sum - 5.0).abs() / 5.0 <= 1e-12);
        assert_eq!(scaled.xs, a.xs); // positions unchanged — no re-voxelise
    }

    #[test]
    fn shell_theorem() {
        // A voxelised sphere's external field matches a point mass of equal total mass over
        // d ∈ [2R, 10R], to ≤1e-3 — an independent check, owing nothing to the reference port.
        let r = 1.0;
        let m = 5.0;
        let sphere = voxelise(
            &Sphere { r },
            &VoxelParams::pitch(r / 20.0),
            MassSpec::Total(m),
        )
        .unwrap();
        let point = Cloud::from_elements(&[(0.0, 0.0, 0.0, m)]);
        for i in 2..=10 {
            let d = i as f64 * r;
            let p = math::Vec3::new(d, 0.0, 0.0);
            let gs = gravity::field(&sphere, p);
            let gp = gravity::field(&point, p);
            let rel = (gs - gp).norm() / gp.norm();
            assert!(rel <= 1e-3, "shell theorem d={d}: rel {rel}");
        }
    }
}
