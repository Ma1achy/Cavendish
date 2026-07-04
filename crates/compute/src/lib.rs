//! `compute` — the `ComputeBackend` seam: the CPU reference and the wgpu fast path (two-pass).
//!
//! Design: `design/compute.md`. Milestone: `milestones/M6-compute-gpu-and-batch.md`.
//!
//! M6a formalises the seam: `CpuBackend` (f64, the bit-exact oracle — it reconstructs the validated
//! forward model verbatim) and `WgpuBackend` (WGSL, f32, differential-first). Both consume an
//! `EvalBatch` whose canonical parameters are **f64**; the GPU downcasts to f32 only at the upload
//! boundary, so `cpu_equals_gpu` is the honest f64-oracle-vs-f32-GPU test on the same scenario. Batch
//! and stream orchestration, `config`, `Prior`, `Schedule` realism, and Lomb–Scargle are M6b.

use gravity::{Cloud, FieldContribution, Inertia};
use instrument::{
    per_ifo_generic, Detector, InstrumentConfig, PhaseModel, PhaseModelKind, PosedSource,
    PropagationIntegral, QuasiStaticGradient,
};
use math::{Dual, Isometry3, Mat3, Quat, Scalar, Vec3};
use source::{world_pose, Orient, Path, Source, SourceDynamics, Timing, Trajectory, ROT_FINE_DT};

mod gpu;
pub use gpu::Gpu;

/// Failure modes of a backend evaluation.
#[derive(Debug)]
pub enum ComputeError {
    /// No compatible GPU adapter / device could be acquired.
    DeviceUnavailable(String),
    /// The batch exceeds a device limit (buffer size, workgroup count).
    BatchTooLarge(String),
}

impl std::fmt::Display for ComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputeError::DeviceUnavailable(s) => write!(f, "compute device unavailable: {s}"),
            ComputeError::BatchTooLarge(s) => write!(f, "batch too large: {s}"),
        }
    }
}

impl std::error::Error for ComputeError {}

/// Backend identity — device name and precision, for provenance in the bundle.
#[derive(Clone, Debug)]
pub struct BackendInfo {
    pub name: String,
    pub precision: Precision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    F64,
    F32,
}

/// One atmospheric mode (POD): the realised `(k, ω, ψ)` and the precomputed potential coefficient
/// `−4πG a/|k|²`. The canonical form both backends read (`noise::AtmoField`'s private modes are
/// extracted into this); the GPU uploads it as f32.
#[derive(Clone, Copy, Debug)]
pub struct AtmoMode {
    pub k: [f64; 3],
    pub omega: f64,
    pub psi: f64,
    pub coeff: f64,
}

/// The atmospheric field as a [`gravity::FieldContribution`] re-expressed from POD modes — the CPU
/// oracle's view of the atmosphere. Mirrors `noise::AtmoField` statement-for-statement (a `generate`
/// test pins the two equal), so the reconstruction inherits M5's validation.
pub struct AtmoContribution {
    pub modes: Vec<AtmoMode>,
}

impl<S: Scalar> FieldContribution<S> for AtmoContribution {
    fn potential(&self, p: Vec3<S>, t: f64) -> S {
        let mut acc = S::from_f64(0.0);
        for m in &self.modes {
            let kp =
                p.x * S::from_f64(m.k[0]) + p.y * S::from_f64(m.k[1]) + p.z * S::from_f64(m.k[2]);
            let arg = kp + S::from_f64(m.psi - m.omega * t);
            acc = acc + S::from_f64(m.coeff) * arg.cos();
        }
        acc
    }

    fn gradient_tensor(&self, p: Vec3<S>, t: f64) -> Mat3<S> {
        let mut m = [[S::from_f64(0.0); 3]; 3];
        for mode in &self.modes {
            let kp = p.x * S::from_f64(mode.k[0])
                + p.y * S::from_f64(mode.k[1])
                + p.z * S::from_f64(mode.k[2]);
            let arg = kp + S::from_f64(mode.psi - mode.omega * t);
            let factor = S::from_f64(mode.coeff) * arg.cos();
            for (i, row) in m.iter_mut().enumerate() {
                for (j, e) in row.iter_mut().enumerate() {
                    *e = *e + factor * S::from_f64(mode.k[i] * mode.k[j]);
                }
            }
        }
        Mat3 { m }
    }
}

/// One source's parameters — the exact inputs to `Source::new`, so the CPU backend reconstructs the
/// validated forward model verbatim (and the GPU reads the same numbers).
#[derive(Clone)]
pub struct SourceBatch {
    pub cloud: Cloud,
    pub placement: Isometry3,
    pub path: Path,
    pub timing: Timing,
    pub orient: Orient,
}

/// One scenario's evaluable parameters.
#[derive(Clone, Default)]
pub struct ScenarioBatch {
    pub sources: Vec<SourceBatch>,
    pub atmo: Vec<AtmoMode>,
    pub detectors: Vec<Isometry3>,
    pub times: Vec<f64>,
}

/// An evaluation batch — canonical **f64** parameters (not poses) for one or more scenarios.
#[derive(Clone, Default)]
pub struct EvalBatch {
    pub scenarios: Vec<ScenarioBatch>,
    pub instrument: InstrumentConfig,
    pub phase_model: PhaseModelKind,
}

/// Per-detector ΔΦ signals: `dphi[scenario][detector][measurement]`.
#[derive(Clone, Debug, Default)]
pub struct SignalBatch {
    pub dphi: Vec<Vec<Vec<f64>>>,
}

/// Executes the forward model — the only crate that knows about devices.
///
/// # Contract (`DESIGN.md` §3.4, `design/compute.md`)
/// - **Method.** `evaluate(batch) -> SignalBatch` — take an `EvalBatch` (f64 parameters, not poses)
///   and return per-detector ΔΦ.
/// - **Post.** `CpuBackend` (f64) is the bit-exact oracle; `WgpuBackend` (WGSL, f32) is
///   differential-first and reproduces the CPU path to the validation tolerance, never worse.
pub trait ComputeBackend {
    fn evaluate(&self, batch: &EvalBatch) -> Result<SignalBatch, ComputeError>;
    fn info(&self) -> BackendInfo;
}

/// The reference backend — pure f64, reconstructing the validated forward model. The oracle the GPU
/// is checked against.
#[derive(Clone, Copy, Debug, Default)]
pub struct CpuBackend;

impl CpuBackend {
    /// Reconstruct one scenario's `(detector, measurement)` ΔΦ grid via the canonical `PhaseModel`.
    fn evaluate_scenario(&self, scn: &ScenarioBatch, batch: &EvalBatch) -> Vec<Vec<f64>> {
        let model: Box<dyn PhaseModel> = match batch.phase_model {
            PhaseModelKind::PropagationIntegral => {
                Box::new(PropagationIntegral::new(batch.instrument))
            }
            PhaseModelKind::QuasiStatic => Box::new(QuasiStaticGradient::new(batch.instrument)),
        };
        // Reconstruct the exact inputs to Source::new — bit-identical to generate::run's forward model.
        let sources: Vec<Source> = scn
            .sources
            .iter()
            .map(|s| {
                let traj = Trajectory::new(s.placement, s.path, s.timing).with_orient(s.orient);
                Source::new(s.cloud.clone(), traj)
            })
            .collect();
        let src_refs: Vec<&dyn SourceDynamics> =
            sources.iter().map(|s| s as &dyn SourceDynamics).collect();
        let atmo = AtmoContribution {
            modes: scn.atmo.clone(),
        };
        let fields: Vec<&dyn FieldContribution<f64>> = if scn.atmo.is_empty() {
            Vec::new()
        } else {
            vec![&atmo]
        };
        scn.detectors
            .iter()
            .map(|&placement| {
                let det = Detector::placed(placement);
                scn.times
                    .iter()
                    .map(|&t| model.delta_phi(&src_refs, &fields, &det, t))
                    .collect()
            })
            .collect()
    }
}

impl ComputeBackend for CpuBackend {
    fn evaluate(&self, batch: &EvalBatch) -> Result<SignalBatch, ComputeError> {
        let dphi = batch
            .scenarios
            .iter()
            .map(|scn| self.evaluate_scenario(scn, batch))
            .collect();
        Ok(SignalBatch { dphi })
    }

    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: "cpu-f64".into(),
            precision: Precision::F64,
        }
    }
}

// ── The differentiable forward path (the `Dual` sweep for the CRB Jacobian, `analysis`/M8) ──────────
//
// A generic `forward::<S>` over the reference `PropagationIntegral` core (`per_ifo_generic`): `f64`
// reproduces `PropagationIntegral::delta_phi` bit-for-bit (`forward_matches_reference`), and `Dual`
// carries one seeded tangent (`value_channel_identity`). The seam `analysis` drives is `forward_dual`
// (one column per `ParamSeed`) against the `forward_f64` base.

/// A Cartesian axis of a vector-valued source parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// A differentiable source parameter — the θ a Jacobian column is taken with respect to.
///
/// `Mass` seeds a fractional multiplier (the signal is linear in mass, so `∂signal/∂(fractional mass)`
/// = signal; `analysis` divides by total mass for the absolute derivative). `Velocity` seeds the
/// `LinearPass` end-point (the constant-velocity parameterisation: velocity `= (b − a)·rate`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    Position(Axis),
    Velocity(Axis),
    Mass,
    Omega0(Axis),
}

/// Which source's which parameter a `Dual` sweep seeds. The PR1↔`analysis` seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamSeed {
    pub source: usize,
    pub param: Param,
}

/// Lifts an `f64` forward-model parameter into the scalar type, seeding one chosen parameter's
/// tangent. `Plain` (`f64`) is the identity; `Seed` (`Dual`) sets tangent 1 on the matching entry.
trait Lift<S: Scalar>: Copy {
    fn at(&self, source: usize, param: Param, value: f64) -> S;
}

#[derive(Clone, Copy)]
struct Plain;
impl Lift<f64> for Plain {
    fn at(&self, _: usize, _: Param, value: f64) -> f64 {
        value
    }
}

#[derive(Clone, Copy)]
struct Seed(ParamSeed);
impl Lift<Dual> for Seed {
    fn at(&self, source: usize, param: Param, value: f64) -> Dual {
        if self.0.source == source && self.0.param == param {
            Dual::var(value)
        } else {
            Dual::from_f64(value)
        }
    }
}

/// One source lifted into the scalar type, ready for the generic phase core.
struct SeededSource<S: Scalar> {
    cloud: Cloud,
    trajectory: Trajectory<S>,
    inertia: Inertia,
    mass_scale: S,
}

impl<S: Scalar> SeededSource<S> {
    fn build(i: usize, s: &SourceBatch, lift: &impl Lift<S>) -> Self {
        let q = |v: f64| S::from_f64(v);
        let quat = |r: Quat| Quat::new(q(r.w), q(r.x), q(r.y), q(r.z));
        let placement = {
            let t = s.placement.translation;
            Isometry3::new(
                quat(s.placement.rotation),
                Vec3::new(
                    lift.at(i, Param::Position(Axis::X), t.x),
                    lift.at(i, Param::Position(Axis::Y), t.y),
                    lift.at(i, Param::Position(Axis::Z), t.z),
                ),
            )
        };
        let path = match s.path {
            Path::Static => Path::Static,
            Path::LinearPass { a, b } => Path::LinearPass {
                a: Vec3::new(q(a.x), q(a.y), q(a.z)),
                b: Vec3::new(
                    lift.at(i, Param::Velocity(Axis::X), b.x),
                    lift.at(i, Param::Velocity(Axis::Y), b.y),
                    lift.at(i, Param::Velocity(Axis::Z), b.z),
                ),
            },
            Path::Oscillation {
                axis,
                amp,
                freq,
                phase,
            } => Path::Oscillation {
                axis: Vec3::new(q(axis.x), q(axis.y), q(axis.z)),
                amp: q(amp),
                freq: q(freq),
                phase: q(phase),
            },
            Path::Circular { radius, freq } => Path::Circular {
                radius: q(radius),
                freq: q(freq),
            },
        };
        let timing = match s.timing {
            Timing::Uniform { rate } => Timing::Uniform { rate: q(rate) },
            Timing::Eased { rate, accel } => Timing::Eased {
                rate: q(rate),
                accel: q(accel),
            },
        };
        let orient = match s.orient {
            Orient::Fixed(r) => Orient::Fixed(quat(r)),
            Orient::FreeRotation { omega0 } => Orient::FreeRotation {
                omega0: Vec3::new(
                    lift.at(i, Param::Omega0(Axis::X), omega0.x),
                    lift.at(i, Param::Omega0(Axis::Y), omega0.y),
                    lift.at(i, Param::Omega0(Axis::Z), omega0.z),
                ),
            },
            Orient::Libration {
                axis,
                pivot_distance,
                theta0,
                thetadot0,
            } => Orient::Libration {
                axis: Vec3::new(q(axis.x), q(axis.y), q(axis.z)),
                pivot_distance: q(pivot_distance),
                theta0: q(theta0),
                thetadot0: q(thetadot0),
            },
        };
        SeededSource {
            cloud: s.cloud.clone(),
            trajectory: Trajectory {
                placement,
                path,
                timing,
                orient,
            },
            inertia: gravity::inertia(&s.cloud),
            mass_scale: lift.at(i, Param::Mass, 1.0),
        }
    }
}

impl<S: Scalar> PosedSource<S> for SeededSource<S> {
    fn body(&self) -> &Cloud {
        &self.cloud
    }
    fn mass_scale(&self) -> S {
        self.mass_scale
    }
    fn inverse_pose_at(&self, t_abs: f64) -> Isometry3<S> {
        world_pose(&self.trajectory, &self.inertia, ROT_FINE_DT, t_abs).inverse()
    }
}

/// The generic forward evaluation over the reference `PropagationIntegral` core: `(detector,
/// measurement)` ΔΦ for one scenario, lifting each source into `S` and seeding via `lift`.
fn forward<S: Scalar>(
    scn: &ScenarioBatch,
    cfg: &InstrumentConfig,
    lift: impl Lift<S>,
) -> Vec<Vec<S>> {
    let sources: Vec<SeededSource<S>> = scn
        .sources
        .iter()
        .enumerate()
        .map(|(i, s)| SeededSource::build(i, s, &lift))
        .collect();
    let posed: Vec<&dyn PosedSource<S>> =
        sources.iter().map(|s| s as &dyn PosedSource<S>).collect();
    let atmo = AtmoContribution {
        modes: scn.atmo.clone(),
    };
    let fields: Vec<&dyn FieldContribution<S>> = if scn.atmo.is_empty() {
        Vec::new()
    } else {
        vec![&atmo]
    };
    scn.detectors
        .iter()
        .map(|&placement| {
            let det = Detector::placed(placement);
            scn.times
                .iter()
                .map(|&t| {
                    let [d1, d2] = per_ifo_generic::<S>(cfg, &det, &posed, &fields, t);
                    d2 - d1 // ΔΦ = δφ₂ − δφ₁ (spec sign)
                })
                .collect()
        })
        .collect()
}

/// The `f64` forward over the reference phase model — the value base for `analysis`'s finite-difference
/// check. Bit-exact to `PropagationIntegral::delta_phi` (`forward_matches_reference`).
pub fn forward_f64(scn: &ScenarioBatch, cfg: &InstrumentConfig) -> Vec<Vec<f64>> {
    forward(scn, cfg, Plain)
}

/// One `Dual` forward sweep with `seed`'s tangent hot — the value channel is the signal, the tangent
/// channel is `∂signal/∂θ` (one Jacobian column, `(detector, measurement)`).
pub fn forward_dual(
    scn: &ScenarioBatch,
    cfg: &InstrumentConfig,
    seed: ParamSeed,
) -> Vec<Vec<Dual>> {
    forward(scn, cfg, Seed(seed))
}

/// The hot-path backend — the two passes as WGSL compute shaders, f32, differential-first. Holds the
/// wgpu device (shared with the viewer at M9). The WGSL kernels land in Commits 2–3.
pub struct WgpuBackend {
    gpu: Gpu,
}

impl WgpuBackend {
    pub fn new() -> Result<Self, ComputeError> {
        Ok(WgpuBackend { gpu: Gpu::new()? })
    }

    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }
}

/// Encode one `(source, detector, measurement)` into the `k_phase` f32 param buffer (the WGSL layout).
fn encode_phase_params(
    cfg: &InstrumentConfig,
    src: &SourceBatch,
    moments: [f64; 3],
    atmo: &[AtmoMode],
    det: Isometry3,
    t: f64,
) -> Vec<f32> {
    let mut p = vec![0.0f32; 46];
    let set = |p: &mut Vec<f32>, i: usize, v: f64| p[i] = v as f32;
    set(&mut p, 0, cfg.m_a);
    set(&mut p, 1, cfg.hbar);
    set(&mut p, 2, cfg.g);
    set(&mut p, 3, cfg.t_half);
    set(&mut p, 4, cfg.v_rec);
    set(&mut p, 5, cfg.u0);
    set(&mut p, 6, cfg.ifo_sep);
    set(&mut p, 7, cfg.fine_dt);
    set(&mut p, 8, t);
    let quat = |p: &mut Vec<f32>, i: usize, q: math::Quat| {
        p[i] = q.w as f32;
        p[i + 1] = q.x as f32;
        p[i + 2] = q.y as f32;
        p[i + 3] = q.z as f32;
    };
    let vec3 = |p: &mut Vec<f32>, i: usize, v: Vec3<f64>| {
        p[i] = v.x as f32;
        p[i + 1] = v.y as f32;
        p[i + 2] = v.z as f32;
    };
    quat(&mut p, 9, det.rotation);
    vec3(&mut p, 13, det.translation);
    quat(&mut p, 16, src.placement.rotation);
    vec3(&mut p, 20, src.placement.translation);
    match src.path {
        Path::Static => set(&mut p, 23, 0.0),
        Path::LinearPass { a, b } => {
            set(&mut p, 23, 1.0);
            vec3(&mut p, 24, a);
            vec3(&mut p, 27, b);
        }
        Path::Oscillation {
            axis,
            amp,
            freq,
            phase,
        } => {
            set(&mut p, 23, 2.0);
            vec3(&mut p, 24, axis);
            set(&mut p, 30, amp);
            set(&mut p, 31, freq);
            set(&mut p, 32, phase);
        }
        Path::Circular { .. } => panic!("M6a GPU path: Circular unsupported (M6b/later)"),
    }
    match src.timing {
        Timing::Uniform { rate } => set(&mut p, 33, rate),
        Timing::Eased { .. } => panic!("M6a GPU path: Eased timing unsupported (M6b/later)"),
    }
    match src.orient {
        Orient::Fixed(q) => {
            set(&mut p, 34, 0.0);
            quat(&mut p, 35, q);
        }
        Orient::FreeRotation { omega0 } => {
            // The on-device f32 integrator (forward.wgsl::free_rotation_quat) accumulates ~1e-5 by
            // ~200 substeps, ~1.7e-5 by 300 — within cpu_equals_gpu's ≤1e-4; keep rotating horizons short.
            set(&mut p, 34, 1.0);
            vec3(&mut p, 39, omega0);
            p[42] = moments[0] as f32;
            p[43] = moments[1] as f32;
            p[44] = moments[2] as f32;
        }
        Orient::Libration { .. } => panic!("M6a GPU path: Libration unsupported (M6b/later)"),
    }
    p[45] = atmo.len() as f32;
    for m in atmo {
        p.extend_from_slice(&[
            m.k[0] as f32,
            m.k[1] as f32,
            m.k[2] as f32,
            m.omega as f32,
            m.psi as f32,
            m.coeff as f32,
        ]);
    }
    p
}

impl ComputeBackend for WgpuBackend {
    fn evaluate(&self, batch: &EvalBatch) -> Result<SignalBatch, ComputeError> {
        let cfg = batch.instrument;
        let mut dphi = Vec::with_capacity(batch.scenarios.len());
        for scn in &batch.scenarios {
            // M6a's GPU path handles one source per scenario (the anchors); multi-source is M6b.
            let src = scn
                .sources
                .first()
                .ok_or_else(|| ComputeError::BatchTooLarge("empty scenario".into()))?;
            let cloud_f32: Vec<[f32; 4]> = (0..src.cloud.len())
                .map(|i| {
                    [
                        src.cloud.xs[i] as f32,
                        src.cloud.ys[i] as f32,
                        src.cloud.zs[i] as f32,
                        src.cloud.ms[i] as f32,
                    ]
                })
                .collect();
            let moments = {
                let m = &gravity::inertia(&src.cloud).i.m;
                [m[0][0], m[1][1], m[2][2]]
            };
            let sig = scn
                .detectors
                .iter()
                .map(|&det| {
                    scn.times
                        .iter()
                        .map(|&t| {
                            let params = encode_phase_params(&cfg, src, moments, &scn.atmo, det, t);
                            self.gpu
                                .run_kernel(FORWARD_WGSL, "k_phase", &cloud_f32, &params, 1)[0]
                                as f64
                        })
                        .collect()
                })
                .collect();
            dphi.push(sig);
        }
        Ok(SignalBatch { dphi })
    }

    fn info(&self) -> BackendInfo {
        BackendInfo {
            name: self.gpu.adapter_name.clone(),
            precision: Precision::F32,
        }
    }
}

/// The forward-model WGSL kernels — mirrors `gravity`'s Rust functions statement-for-statement.
pub const FORWARD_WGSL: &str = include_str!("forward.wgsl");

#[cfg(test)]
mod tests {
    use super::*;
    use gravity::Cloud;

    #[test]
    #[allow(clippy::float_cmp)] // forward::<f64> must reproduce the reference bit-for-bit
    fn forward_matches_reference() {
        // forward_f64 reproduces the canonical PropagationIntegral::delta_phi bit-for-bit (source +
        // atmosphere), anchoring the analysis path to the validated forward model.
        let cloud = Cloud::from_elements(&[(3.0, 0.0, 2.5, 500.0), (2.6, 0.1, 2.4, 300.0)]);
        let placement = Isometry3::new(Quat::identity(), Vec3::new(0.0, 0.0, 0.0));
        let (path, timing, orient) = (
            Path::Static,
            Timing::Uniform { rate: 0.0 },
            Orient::Fixed(Quat::identity()),
        );
        let atmo = vec![AtmoMode {
            k: [0.3, -0.2, 0.1],
            omega: 0.5,
            psi: 0.4,
            coeff: 1e-9,
        }];
        let detectors = vec![
            Isometry3::new(Quat::identity(), Vec3::new(0.0, 0.0, 0.0)),
            Isometry3::new(Quat::identity(), Vec3::new(1.0, 0.0, 0.0)),
        ];
        let times = vec![0.0, 1.5, 3.0];
        let scn = ScenarioBatch {
            sources: vec![SourceBatch {
                cloud: cloud.clone(),
                placement,
                path,
                timing,
                orient,
            }],
            atmo: atmo.clone(),
            detectors: detectors.clone(),
            times: times.clone(),
        };
        let cfg = InstrumentConfig::default();
        let got = forward_f64(&scn, &cfg);

        let source = Source::new(
            cloud,
            Trajectory::new(placement, path, timing).with_orient(orient),
        );
        let atmo_c = AtmoContribution { modes: atmo };
        let fields: Vec<&dyn FieldContribution<f64>> = vec![&atmo_c];
        let model = PropagationIntegral::new(cfg);
        for (di, &det_iso) in detectors.iter().enumerate() {
            let det = Detector::placed(det_iso);
            for (ti, &t) in times.iter().enumerate() {
                let want = model.delta_phi(&[&source], &fields, &det, t);
                assert_eq!(got[di][ti], want, "det {di} time {ti}");
            }
        }
    }

    #[test]
    #[allow(clippy::float_cmp)] // the value channel must match the f64 forward bit-for-bit
    fn value_channel_identity() {
        // A free-rotation source with atmosphere: forward_dual (ω₀ₓ seeded) has a value channel equal
        // to forward_f64 — the whole-model guard that the Scalar lift preserves the forward value while
        // carrying a tangent — and the ω₀ tangent is non-trivial.
        let cloud = Cloud::from_elements(&[
            (0.4, 0.0, 0.0, 200.0),
            (-0.4, 0.0, 0.0, 200.0),
            (0.0, 0.3, 0.1, 150.0),
        ]);
        let scn = ScenarioBatch {
            sources: vec![SourceBatch {
                cloud,
                placement: Isometry3::new(Quat::identity(), Vec3::new(2.5, 0.2, 2.0)),
                path: Path::Static,
                timing: Timing::Uniform { rate: 0.0 },
                orient: Orient::FreeRotation {
                    omega0: Vec3::new(0.6, 0.2, 0.4),
                },
            }],
            atmo: vec![AtmoMode {
                k: [0.2, 0.1, -0.3],
                omega: 0.7,
                psi: 0.1,
                coeff: 2e-9,
            }],
            detectors: vec![Isometry3::new(Quat::identity(), Vec3::new(0.0, 0.0, 0.0))],
            times: vec![0.0, 1.0],
        };
        let cfg = InstrumentConfig::default();
        let base = forward_f64(&scn, &cfg);
        let dual = forward_dual(
            &scn,
            &cfg,
            ParamSeed {
                source: 0,
                param: Param::Omega0(Axis::X),
            },
        );
        for (rb, rd) in base.iter().zip(&dual) {
            for (&vb, &vd) in rb.iter().zip(rd) {
                assert_eq!(vb, vd.v, "value channel drift");
            }
        }
        let max_tan = dual.iter().flatten().map(|d| d.d.abs()).fold(0.0, f64::max);
        assert!(
            max_tan > 0.0,
            "ω₀ tangent identically zero — seeding is dead"
        );
    }

    /// Smoke test: wgpu initialises and a trivial compute shader runs and reads back. `#[ignore]` so
    /// it runs only in the GPU CI job (lavapipe) and locally on Metal, never in the GPU-less gate.
    #[test]
    #[ignore = "requires a GPU device (run in the gpu CI job / locally on Metal)"]
    fn gpu_smoke() {
        let gpu = Gpu::new().expect("acquire device");
        eprintln!("gpu_smoke: adapter = {}", gpu.adapter_name);
        let input: Vec<f32> = (0..64).map(|i| i as f32).collect();
        let out = gpu.run_double(&input);
        for (i, (&o, &inp)) in out.iter().zip(&input).enumerate() {
            assert_eq!(o, inp * 2.0, "gpu doubled wrong at {i}");
        }
    }

    fn rel(a: f64, b: f64) -> f64 {
        (a - b).abs() / a.abs().max(1e-30)
    }

    /// Each WGSL kernel (V, g, Γ, arm sample) vs its Rust reference on small fixed inputs — a handful
    /// of elements, so f32 accumulation stays ≤1e-6 rel (the full-cloud accumulation is `cpu_equals_gpu`
    /// at ≤1e-4). `#[ignore]`: GPU CI job / local Metal.
    #[test]
    #[ignore = "requires a GPU device (run in the gpu CI job / locally on Metal)"]
    fn wgsl_kernel_parity() {
        let gpu = Gpu::new().expect("acquire device");
        let cloud = Cloud::from_elements(&[
            (0.1, 0.2, 0.3, 5.0),
            (1.0, -0.5, 0.8, 12.0),
            (-0.7, 0.4, -1.1, 3.0),
        ]);
        let cloud_f32: Vec<[f32; 4]> = (0..cloud.len())
            .map(|i| {
                [
                    cloud.xs[i] as f32,
                    cloud.ys[i] as f32,
                    cloud.zs[i] as f32,
                    cloud.ms[i] as f32,
                ]
            })
            .collect();
        let p = Vec3::new(2.0, 1.5, 3.0);
        let pf = [p.x as f32, p.y as f32, p.z as f32];

        // V
        let v_ref = gravity::potential(&cloud, p);
        let v_gpu = gpu.run_kernel(FORWARD_WGSL, "k_potential", &cloud_f32, &pf, 1)[0] as f64;
        assert!(rel(v_ref, v_gpu) <= 1e-6, "V: {v_ref:e} vs {v_gpu:e}");

        // g — each component to 1e-6 of the field magnitude (a coincidentally-small component need
        // not match to 1e-6 of its own tiny value; the vector agrees to 1e-6 of its scale).
        let g_ref = gravity::field(&cloud, p);
        let g_gpu = gpu.run_kernel(FORWARD_WGSL, "k_field", &cloud_f32, &pf, 3);
        let g_scale = g_ref.norm();
        for (r, &gg) in [g_ref.x, g_ref.y, g_ref.z].iter().zip(&g_gpu) {
            assert!(
                (r - gg as f64).abs() <= 1e-6 * g_scale,
                "g: {r:e} vs {gg:e}"
            );
        }

        // Γ (symmetric, row-major) — each component to 1e-6 of the tensor scale (largest |component|).
        let gamma_ref = gravity::gradient_tensor(&cloud, p);
        let gamma_gpu = gpu.run_kernel(FORWARD_WGSL, "k_gamma", &cloud_f32, &pf, 9);
        let gamma_scale = gamma_ref
            .m
            .iter()
            .flatten()
            .fold(0.0f64, |acc, &x| acc.max(x.abs()));
        for a in 0..3 {
            for b in 0..3 {
                assert!(
                    (gamma_ref.m[a][b] - gamma_gpu[a * 3 + b] as f64).abs() <= 1e-6 * gamma_scale,
                    "Γ[{a}][{b}]: {:e} vs {:e}",
                    gamma_ref.m[a][b],
                    gamma_gpu[a * 3 + b]
                );
            }
        }

        // arm sample (π-pulse kick reproduced): lower arm of IFO 0. z = vt − ½gt² cancels two large
        // ballistic terms near the apex, so parity is to 1e-6 of the flight scale (v_first·2T), not of
        // the coincidentally-small apex height.
        let cfg = InstrumentConfig::default();
        let arm = instrument::build_arms(&cfg)[0].lower;
        let arm_scale = cfg.u0 * 2.0 * cfg.t_half;
        for &tau in &[0.1, cfg.t_half, cfg.t_half + 0.3, 2.0 * cfg.t_half] {
            let z_ref = arm.z_at(tau);
            let ap = [
                0.0,
                cfg.u0 as f32,
                cfg.v_rec as f32,
                cfg.t_half as f32,
                cfg.g as f32,
                tau as f32,
            ];
            let z_gpu = gpu.run_kernel(FORWARD_WGSL, "k_arm", &[], &ap, 1)[0] as f64;
            assert!(
                (z_ref - z_gpu).abs() <= 1e-6 * arm_scale,
                "arm z_at({tau}): {z_ref} vs {z_gpu}"
            );
        }
        // phase (static source): the full arm integral — π-pulse Simpson split + differential-first
        // per-element differencing. A few-element source, so f32 accumulation stays ≤1e-6 rel (the
        // full-cloud phase is cpu_equals_gpu at ≤1e-4).
        let place = Isometry3::new(math::Quat::identity(), Vec3::new(3.0, 0.0, 2.5));
        let traj = Trajectory::new(place, Path::Static, Timing::Uniform { rate: 0.0 })
            .with_orient(Orient::Fixed(math::Quat::identity()));
        let source = Source::new(cloud.clone(), traj);
        let det = Detector::new(0.0);
        let phi_ref = PropagationIntegral::new(cfg).delta_phi(&[&source], &[], &det, 0.0);
        let pose = source.pose_at(0.0);
        let (sq, st) = (pose.rotation, pose.translation);
        let (dq, dt) = (det.placement.rotation, det.placement.translation);
        let pp = [
            cfg.m_a as f32,
            cfg.hbar as f32,
            cfg.g as f32,
            cfg.t_half as f32,
            cfg.v_rec as f32,
            cfg.u0 as f32,
            cfg.ifo_sep as f32,
            cfg.fine_dt as f32,
            sq.w as f32,
            sq.x as f32,
            sq.y as f32,
            sq.z as f32,
            st.x as f32,
            st.y as f32,
            st.z as f32,
            dq.w as f32,
            dq.x as f32,
            dq.y as f32,
            dq.z as f32,
            dt.x as f32,
            dt.y as f32,
            dt.z as f32,
        ];
        let phi_gpu = gpu.run_kernel(FORWARD_WGSL, "k_phase_static", &cloud_f32, &pp, 1)[0] as f64;
        eprintln!(
            "wgsl_kernel_parity: phase {phi_ref:e} vs {phi_gpu:e} (rel {:.2e})",
            rel(phi_ref, phi_gpu)
        );
        assert!(
            rel(phi_ref, phi_gpu) <= 1e-6,
            "phase: {phi_ref:e} vs {phi_gpu:e}"
        );

        eprintln!(
            "wgsl_kernel_parity: V/g/Γ/arm/phase ≤1e-6 on {} elements",
            cloud.len()
        );
    }
}
