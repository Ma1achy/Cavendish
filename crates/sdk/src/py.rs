//! The PyO3 extension module `cavendish` (gated behind `extension-module`).

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::marker::Ungil;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::IntoPyObjectExt;

use dlpark::ffi::{DataType, Device};
use dlpark::traits::{InferDataType, RowMajorCompactLayout, TensorLike};
use dlpark::SafeManagedTensorVersioned;

use std::panic::{catch_unwind, AssertUnwindSafe};

use generate::{
    scenario_key, Detector as RDetector, DetectorArray as RArray, FieldSet as RFieldSet, Isometry3,
    Orient as ROrient, Path as RPath, PhaseModelKind as RPhase, Prior as RPrior, Quat,
    Scenario as RScenario, Schedule as RSchedule, Source as RSource, Timing as RTiming,
    Trajectory as RTrajectory, UldmConfig as RUldm, Vec3,
};
use gravity::Cloud;
use shape::{voxelise, Cuboid, MassSpec, Sphere, VoxelParams};
use state::StateBundle;

// ── DLPack hand-off ─────────────────────────────────────────────────────────────────────────────

/// An owned, contiguous, row-major buffer handed to torch via DLPack. The `ManagedTensor` boxes and
/// owns it; the DLPack deleter frees it when torch releases the tensor — so the tensor outlives the
/// bundle. `T` is `f64` (every numeric field) or `bool` (`mask`).
struct Owned<T> {
    data: Vec<T>,
    shape: Vec<i64>,
}

impl<T: InferDataType + 'static> TensorLike<RowMajorCompactLayout> for Owned<T> {
    type Error = dlpark::Error;
    fn data_ptr(&self) -> *mut std::ffi::c_void {
        self.data.as_ptr() as *mut _
    }
    fn memory_layout(&self) -> RowMajorCompactLayout {
        RowMajorCompactLayout::new(self.shape.clone())
    }
    fn device(&self) -> Result<Device, Self::Error> {
        Ok(Device::CPU)
    }
    fn data_type(&self) -> Result<DataType, Self::Error> {
        Ok(T::data_type())
    }
    fn byte_offset(&self) -> u64 {
        0
    }
}

/// The raw host address of `data`'s buffer — for the zero-copy probe (torch must wrap this exact
/// allocation, not a copy).
fn buffer_addr<T>(data: &[T]) -> usize {
    data.as_ptr() as usize
}

/// Wrap an owned buffer in a DLPack capsule (device `kDLCPU`).
fn to_capsule<'py, T: InferDataType + 'static>(
    py: Python<'py>,
    data: Vec<T>,
    shape: Vec<i64>,
) -> PyResult<Bound<'py, PyAny>> {
    let mt = SafeManagedTensorVersioned::new(Owned { data, shape })
        .map_err(|e| PyRuntimeError::new_err(format!("dlpack build failed: {e}")))?;
    mt.into_pyobject(py)
}

/// Hand an owned buffer to torch (`torch.from_dlpack` — zero-copy on CPU).
fn to_torch<T: InferDataType + 'static>(
    py: Python<'_>,
    data: Vec<T>,
    shape: Vec<i64>,
) -> PyResult<Py<PyAny>> {
    let capsule = to_capsule(py, data, shape)?;
    let torch = py.import("torch")?;
    let tensor = torch.getattr("from_dlpack")?.call1((capsule,))?;
    Ok(tensor.unbind())
}

/// Run heavy Rust work with the **GIL released** (`detach`, so Python threads progress) and any
/// **panic caught and converted** to a `RuntimeError` — a Rust panic must never unwind across the FFI
/// boundary as a panic (that is undefined behaviour / an abort). `f` touches no Python objects
/// (`Ungil`). The panic payload is dropped to `()` inside the closure so the result stays `Ungil`.
fn guarded<T: Ungil + Send>(py: Python<'_>, f: impl FnOnce() -> T + Ungil + Send) -> PyResult<T> {
    py.detach(|| catch_unwind(AssertUnwindSafe(f)).map_err(|_| ()))
        .map_err(|()| {
            PyRuntimeError::new_err(
                "cavendish: a Rust panic was caught at the boundary and converted",
            )
        })
}

// ── flatten helpers: a bundle field → (contiguous buffer, shape) ─────────────────────────────────

fn flat2(v: &[Vec<f64>]) -> (Vec<f64>, Vec<i64>) {
    let (d0, d1) = (v.len(), v.first().map_or(0, Vec::len));
    (
        v.iter().flatten().copied().collect(),
        vec![d0 as i64, d1 as i64],
    )
}

/// `(d0, d1, K)` from a nested `Vec<Vec<[f64;K]>>` — serves both `(S,T,·)` kinematics and `(T,D,2)`.
fn flat3<const K: usize>(v: &[Vec<[f64; K]>]) -> (Vec<f64>, Vec<i64>) {
    let (d0, d1) = (v.len(), v.first().map_or(0, Vec::len));
    (
        v.iter().flatten().flatten().copied().collect(),
        vec![d0 as i64, d1 as i64, K as i64],
    )
}

fn flat_vk<const K: usize>(v: &[[f64; K]]) -> (Vec<f64>, Vec<i64>) {
    (
        v.iter().flatten().copied().collect(),
        vec![v.len() as i64, K as i64],
    )
}

fn flat_v33(v: &[[[f64; 3]; 3]]) -> (Vec<f64>, Vec<i64>) {
    (
        v.iter().flatten().flatten().copied().collect(),
        vec![v.len() as i64, 3, 3],
    )
}

// ── small vector helpers ─────────────────────────────────────────────────────────────────────────

fn vec3(v: &[f64]) -> PyResult<Vec3<f64>> {
    match v {
        [x, y, z] => Ok(Vec3::new(*x, *y, *z)),
        _ => Err(PyValueError::new_err("expected a 3-vector [x, y, z]")),
    }
}

fn iso_from(placement: &[f64]) -> PyResult<Isometry3> {
    Ok(Isometry3::new(Quat::identity(), vec3(placement)?))
}

// ── constructor surface ──────────────────────────────────────────────────────────────────────────

/// A voxelised body cloud (built from a primitive solid).
#[pyclass(skip_from_py_object)]
#[derive(Clone)]
struct Body {
    cloud: Cloud,
}

#[pyfunction]
fn cuboid(half: Vec<f64>, pitch: f64, mass: f64) -> PyResult<Body> {
    let half = vec3(&half)?;
    let cloud = voxelise(
        &Cuboid {
            half: [half.x, half.y, half.z],
        },
        &VoxelParams::pitch(pitch),
        MassSpec::Total(mass),
    )
    .map_err(|e| PyValueError::new_err(format!("voxelise: {e:?}")))?;
    Ok(Body { cloud })
}

#[pyfunction]
fn sphere(r: f64, pitch: f64, mass: f64) -> PyResult<Body> {
    let cloud = voxelise(
        &Sphere { r },
        &VoxelParams::pitch(pitch),
        MassSpec::Total(mass),
    )
    .map_err(|e| PyValueError::new_err(format!("voxelise: {e:?}")))?;
    Ok(Body { cloud })
}

#[pyclass(from_py_object)]
#[derive(Clone)]
struct Path {
    inner: RPath,
}

#[pymethods]
impl Path {
    #[staticmethod]
    #[pyo3(name = "static")]
    fn r#static() -> Self {
        Path {
            inner: RPath::Static,
        }
    }
    #[staticmethod]
    fn linear_pass(a: Vec<f64>, b: Vec<f64>) -> PyResult<Self> {
        Ok(Path {
            inner: RPath::LinearPass {
                a: vec3(&a)?,
                b: vec3(&b)?,
            },
        })
    }
    #[staticmethod]
    fn oscillation(axis: Vec<f64>, amp: f64, freq: f64, phase: f64) -> PyResult<Self> {
        Ok(Path {
            inner: RPath::Oscillation {
                axis: vec3(&axis)?,
                amp,
                freq,
                phase,
            },
        })
    }
    #[staticmethod]
    fn circular(radius: f64, freq: f64) -> Self {
        Path {
            inner: RPath::Circular { radius, freq },
        }
    }
}

#[pyclass(from_py_object)]
#[derive(Clone)]
struct Timing {
    inner: RTiming,
}

#[pymethods]
impl Timing {
    #[staticmethod]
    fn uniform(rate: f64) -> Self {
        Timing {
            inner: RTiming::Uniform { rate },
        }
    }
    #[staticmethod]
    fn eased(rate: f64, accel: f64) -> Self {
        Timing {
            inner: RTiming::Eased { rate, accel },
        }
    }
}

#[pyclass(from_py_object)]
#[derive(Clone)]
struct Orient {
    inner: ROrient,
}

#[pymethods]
impl Orient {
    #[staticmethod]
    #[pyo3(signature = (quat=None))]
    fn fixed(quat: Option<Vec<f64>>) -> PyResult<Self> {
        let q = match quat {
            None => Quat::identity(),
            Some(v) => match v.as_slice() {
                [w, x, y, z] => Quat::new(*w, *x, *y, *z),
                _ => return Err(PyValueError::new_err("expected a quaternion [w, x, y, z]")),
            },
        };
        Ok(Orient {
            inner: ROrient::Fixed(q),
        })
    }
    #[staticmethod]
    fn free_rotation(omega0: Vec<f64>) -> PyResult<Self> {
        Ok(Orient {
            inner: ROrient::FreeRotation {
                omega0: vec3(&omega0)?,
            },
        })
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
struct Trajectory {
    inner: RTrajectory,
}

#[pymethods]
impl Trajectory {
    #[new]
    #[pyo3(signature = (placement, path=None, timing=None, orient=None))]
    fn new(
        placement: Vec<f64>,
        path: Option<Path>,
        timing: Option<Timing>,
        orient: Option<Orient>,
    ) -> PyResult<Self> {
        let p = path.map_or(RPath::Static, |p| p.inner);
        let t = timing.map_or(RTiming::Uniform { rate: 0.0 }, |t| t.inner);
        let mut tr = RTrajectory::new(iso_from(&placement)?, p, t);
        if let Some(o) = orient {
            tr = tr.with_orient(o.inner);
        }
        Ok(Trajectory { inner: tr })
    }
}

#[pyclass(from_py_object)]
#[derive(Clone)]
struct Detector {
    inner: RDetector,
}

#[pymethods]
impl Detector {
    #[new]
    fn new(base_z: f64) -> Self {
        Detector {
            inner: RDetector::new(base_z),
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
struct DetectorArray {
    inner: RArray,
}

#[pymethods]
impl DetectorArray {
    #[new]
    fn new(detectors: Vec<Detector>) -> Self {
        DetectorArray {
            inner: RArray::new(detectors.into_iter().map(|d| d.inner).collect()),
        }
    }
    /// A line of vertically-oriented detectors at the given base heights.
    #[staticmethod]
    fn line(base_zs: Vec<f64>) -> Self {
        DetectorArray {
            inner: RArray::new(base_zs.into_iter().map(RDetector::new).collect()),
        }
    }
}

#[pyclass(skip_from_py_object)]
#[derive(Clone)]
struct Schedule {
    inner: RSchedule,
}

#[pymethods]
impl Schedule {
    #[staticmethod]
    fn uniform(cadence: f64, n: usize) -> Self {
        Schedule {
            inner: RSchedule::uniform(cadence, n),
        }
    }
    #[staticmethod]
    fn gappy(cadence: f64, n: usize, p_drop: f64, seed: u64) -> Self {
        Schedule {
            inner: RSchedule::gappy(cadence, n, p_drop, seed),
        }
    }
    #[staticmethod]
    fn jittered(cadence: f64, n: usize, jitter: f64, seed: u64) -> Self {
        Schedule {
            inner: RSchedule::jittered(cadence, n, jitter, seed),
        }
    }
    fn with_contamination(&self, fraction: f64, seed: u64) -> Self {
        Schedule {
            inner: self.inner.clone().with_contamination(fraction, seed),
        }
    }
}

#[pyclass(from_py_object)]
#[derive(Clone)]
struct FieldSet {
    inner: RFieldSet,
}

#[pymethods]
impl FieldSet {
    #[new]
    #[pyo3(signature = (shape=false, decomposition=false, periodogram=false))]
    fn new(shape: bool, decomposition: bool, periodogram: bool) -> Self {
        FieldSet {
            inner: RFieldSet {
                shape,
                decomposition,
                periodogram,
            },
        }
    }
}

#[pyclass(from_py_object)]
#[derive(Clone)]
struct UldmConfig {
    inner: RUldm,
}

#[pymethods]
impl UldmConfig {
    #[new]
    #[pyo3(signature = (amplitude=1e-3, frequency=0.1, phase=0.0))]
    fn new(amplitude: f64, frequency: f64, phase: f64) -> Self {
        UldmConfig {
            inner: RUldm {
                amplitude,
                frequency,
                phase,
            },
        }
    }
}

#[pyclass(from_py_object)]
#[derive(Clone)]
struct AtmoConfig {
    inner: generate::AtmoConfig,
}

#[pymethods]
impl AtmoConfig {
    #[new]
    #[pyo3(signature = (n_modes=32, correlation_length=50.0, amplitude=1e-6, sound_speed=343.0))]
    fn new(n_modes: usize, correlation_length: f64, amplitude: f64, sound_speed: f64) -> Self {
        AtmoConfig {
            inner: generate::AtmoConfig {
                n_modes,
                correlation_length,
                amplitude,
                sound_speed,
            },
        }
    }
}

/// A scalar prior distribution (for `Prior`'s named fields).
#[pyclass(from_py_object)]
#[derive(Clone)]
struct Dist {
    inner: config::Dist,
}

#[pymethods]
impl Dist {
    #[staticmethod]
    #[pyo3(name = "const")]
    fn r#const(value: f64) -> Self {
        Dist {
            inner: config::Dist::Const(value),
        }
    }
    #[staticmethod]
    fn uniform(lo: f64, hi: f64) -> Self {
        Dist {
            inner: config::Dist::Uniform { lo, hi },
        }
    }
    #[staticmethod]
    fn log_uniform(lo: f64, hi: f64) -> Self {
        Dist {
            inner: config::Dist::LogUniform { lo, hi },
        }
    }
    #[staticmethod]
    fn normal(mean: f64, sigma: f64) -> Self {
        Dist {
            inner: config::Dist::Normal { mean, sigma },
        }
    }
}

/// The forward-model selector (exposed by value — no forward-model edit; deferred seating into config).
#[pyclass(eq, eq_int, from_py_object)]
#[derive(Clone, PartialEq)]
enum PhaseModelKind {
    PropagationIntegral,
    QuasiStatic,
}

impl From<PhaseModelKind> for RPhase {
    fn from(k: PhaseModelKind) -> Self {
        match k {
            PhaseModelKind::PropagationIntegral => RPhase::PropagationIntegral,
            PhaseModelKind::QuasiStatic => RPhase::QuasiStatic,
        }
    }
}

/// The scenario as **Send + Sync components** — never the built `RScenario`, which holds
/// `Box<dyn SourceDynamics>` / `NoiseStack` (neither `Send` nor `Sync`, so it cannot back a
/// `#[pyclass]` nor cross a GIL-released boundary). `build()` assembles it on demand, inside the
/// GIL-free region, where the trait objects never leave the thread.
#[pyclass]
struct Scenario {
    cloud: Cloud,
    trajectory: RTrajectory,
    array: RArray,
    schedule: RSchedule,
    seed: u64,
    field_set: Option<RFieldSet>,
    uldm: Option<RUldm>,
    atmo: Option<generate::AtmoConfig>,
    phase_model: Option<RPhase>,
    fine_dt: Option<f64>,
}

impl Scenario {
    fn build(&self) -> RScenario {
        let mut src = RSource::new(self.cloud.clone(), self.trajectory);
        if let Some(dt) = self.fine_dt {
            src = src.with_fine_dt(dt);
        }
        let mut scn = RScenario::new(
            Box::new(src),
            self.array.clone(),
            self.schedule.clone(),
            self.seed,
        );
        if let Some(f) = self.field_set {
            scn = scn.with_field_set(f);
        }
        if let Some(u) = self.uldm {
            scn = scn.with_uldm(u);
        }
        if let Some(a) = self.atmo {
            scn = scn.with_atmo(a);
        }
        if let Some(pm) = self.phase_model {
            scn = scn.with_phase_model(pm);
        }
        scn
    }
}

#[pymethods]
impl Scenario {
    #[new]
    #[pyo3(signature = (body, trajectory, array, schedule, seed=0, field_set=None, uldm=None, atmo=None, phase_model=None, fine_dt=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        body: &Body,
        trajectory: &Trajectory,
        array: &DetectorArray,
        schedule: &Schedule,
        seed: u64,
        field_set: Option<FieldSet>,
        uldm: Option<UldmConfig>,
        atmo: Option<AtmoConfig>,
        phase_model: Option<PhaseModelKind>,
        fine_dt: Option<f64>,
    ) -> Self {
        Scenario {
            cloud: body.cloud.clone(),
            trajectory: trajectory.inner,
            array: array.inner.clone(),
            schedule: schedule.inner.clone(),
            seed,
            field_set: field_set.map(|f| f.inner),
            uldm: uldm.map(|u| u.inner),
            atmo: atmo.map(|a| a.inner),
            phase_model: phase_model.map(Into::into),
            fine_dt,
        }
    }
}

// ── the bundle ───────────────────────────────────────────────────────────────────────────────────

// `weakref` so a stream consumer can observe that only one bundle stays resident at a time.
#[pyclass(weakref)]
struct Bundle {
    inner: StateBundle,
}

impl Bundle {
    fn opt<'py, T: InferDataType + 'static>(
        py: Python<'py>,
        field: Option<(Vec<T>, Vec<i64>)>,
    ) -> PyResult<Option<Py<PyAny>>> {
        match field {
            Some((data, shape)) => Ok(Some(to_torch(py, data, shape)?)),
            None => Ok(None),
        }
    }
}

#[pymethods]
impl Bundle {
    // Always-present tensors.
    #[getter]
    fn time(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let t = &self.inner.time;
        to_torch(py, t.clone(), vec![t.len() as i64])
    }
    #[getter]
    fn signal(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (d, s) = flat2(&self.inner.signal);
        to_torch(py, d, s)
    }
    #[getter]
    fn signal_noise(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (d, s) = flat2(&self.inner.signal_noise);
        to_torch(py, d, s)
    }
    #[getter]
    fn mask(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let m = &self.inner.mask;
        to_torch(py, m.clone(), vec![m.len() as i64])
    }
    #[getter]
    fn source_position(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (d, s) = flat3::<3>(&self.inner.source_position);
        to_torch(py, d, s)
    }
    #[getter]
    fn source_velocity(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (d, s) = flat3::<3>(&self.inner.source_velocity);
        to_torch(py, d, s)
    }
    #[getter]
    fn source_accel(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (d, s) = flat3::<3>(&self.inner.source_accel);
        to_torch(py, d, s)
    }
    #[getter]
    fn source_orientation(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (d, s) = flat3::<4>(&self.inner.source_orientation);
        to_torch(py, d, s)
    }
    #[getter]
    fn source_angular_velocity(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (d, s) = flat3::<3>(&self.inner.source_angular_velocity);
        to_torch(py, d, s)
    }
    #[getter]
    fn source_angular_accel(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (d, s) = flat3::<3>(&self.inner.source_angular_accel);
        to_torch(py, d, s)
    }
    #[getter]
    fn detector_placement(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let (d, s) = flat_vk::<7>(&self.inner.detector_placement);
        to_torch(py, d, s)
    }

    // Shape descriptors (Some iff FieldSet.shape).
    #[getter]
    fn source_mass(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Self::opt(
            py,
            self.inner
                .source_mass
                .as_ref()
                .map(|v| (v.clone(), vec![v.len() as i64])),
        )
    }
    #[getter]
    fn source_inertia(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Self::opt(py, self.inner.source_inertia.as_ref().map(|v| flat_v33(v)))
    }
    #[getter]
    fn source_moments(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Self::opt(
            py,
            self.inner.source_moments.as_ref().map(|v| flat_vk::<3>(v)),
        )
    }
    #[getter]
    fn source_axes(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Self::opt(py, self.inner.source_axes.as_ref().map(|v| flat_v33(v)))
    }
    #[getter]
    fn source_quadrupole(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Self::opt(
            py,
            self.inner.source_quadrupole.as_ref().map(|v| flat_v33(v)),
        )
    }

    // Decomposition channels (Some iff FieldSet.decomposition).
    #[getter]
    fn signal_uldm(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Self::opt(
            py,
            self.inner
                .signal_uldm
                .as_ref()
                .map(|v| (v.clone(), vec![v.len() as i64])),
        )
    }
    #[getter]
    fn signal_targets(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Self::opt(py, self.inner.signal_targets.as_ref().map(|v| flat2(v)))
    }
    #[getter]
    fn signal_atmospheric(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Self::opt(py, self.inner.signal_atmospheric.as_ref().map(|v| flat2(v)))
    }
    #[getter]
    fn signal_per_ifo(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        Self::opt(
            py,
            self.inner.signal_per_ifo.as_ref().map(|v| flat3::<2>(v)),
        )
    }

    // Periodogram (Some iff FieldSet.periodogram) → {freqs (F), power (D,F)} or None.
    #[getter]
    fn periodogram(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        match &self.inner.periodogram {
            None => Ok(None),
            Some(p) => {
                let freqs = to_torch(py, p.freqs.clone(), vec![p.freqs.len() as i64])?;
                let (pd, ps) = flat2(&p.power);
                let power = to_torch(py, pd, ps)?;
                let d = PyDict::new(py);
                d.set_item("freqs", freqs)?;
                d.set_item("power", power)?;
                Ok(Some(d.into_py_any(py)?))
            }
        }
    }

    /// Resolved config + seed, as a plain dict.
    #[getter]
    fn meta(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let d = PyDict::new(py);
        d.set_item("seed", self.inner.meta.seed)?;
        d.set_item("description", &self.inner.meta.description)?;
        d.into_py_any(py)
    }

    /// Test hook: a field as native Python lists, built straight from the Rust `Vec` (bypassing
    /// DLPack) — the independent reference for `fidelity`. Returns `None` for an unset optional field.
    fn _raw(&self, py: Python<'_>, name: &str) -> PyResult<Py<PyAny>> {
        let b = &self.inner;
        Ok(match name {
            "time" => b.time.clone().into_py_any(py)?,
            "signal" => b.signal.clone().into_py_any(py)?,
            "signal_noise" => b.signal_noise.clone().into_py_any(py)?,
            "mask" => b.mask.clone().into_py_any(py)?,
            "source_position" => rows3(&b.source_position).into_py_any(py)?,
            "detector_placement" => b
                .detector_placement
                .iter()
                .map(|a| a.to_vec())
                .collect::<Vec<_>>()
                .into_py_any(py)?,
            "signal_targets" => match &b.signal_targets {
                Some(v) => v.clone().into_py_any(py)?,
                None => py.None(),
            },
            "source_mass" => match &b.source_mass {
                Some(v) => v.clone().into_py_any(py)?,
                None => py.None(),
            },
            _ => return Err(PyValueError::new_err(format!("_raw: unknown field {name}"))),
        })
    }

    /// Test hook (zero-copy probe): build `signal`'s buffer, return `(tensor, host_addr)`. A genuine
    /// zero-copy hand-off has `tensor.data_ptr() == host_addr` (torch wrapped the exact allocation).
    fn _zero_copy_probe(&self, py: Python<'_>) -> PyResult<(Py<PyAny>, usize)> {
        let (data, shape) = flat2(&self.inner.signal);
        let addr = buffer_addr(&data);
        let tensor = to_torch(py, data, shape)?;
        Ok((tensor, addr))
    }
}

/// `(S,T,3)` as nested Python lists.
fn rows3(v: &[Vec<[f64; 3]>]) -> Vec<Vec<Vec<f64>>> {
    v.iter()
        .map(|src| src.iter().map(|a| a.to_vec()).collect())
        .collect()
}

// ── the prior + the stream ───────────────────────────────────────────────────────────────────────

/// Optional batch sugar: a body template + named scalar `Dist`s. Held as Send + Sync components;
/// `build()` assembles the (Send + Sync) `RPrior`.
#[pyclass(skip_from_py_object)]
struct Prior {
    cloud: Cloud,
    fields: Vec<(String, config::Dist)>,
    array: RArray,
    schedule: RSchedule,
    field_set: RFieldSet,
    atmo: Option<generate::AtmoConfig>,
}

impl Prior {
    fn build(&self) -> RPrior {
        RPrior {
            cloud: self.cloud.clone(),
            fields: self.fields.clone(),
            array: self.array.clone(),
            schedule: self.schedule.clone(),
            field_set: self.field_set,
            atmo: self.atmo,
        }
    }
}

#[pymethods]
impl Prior {
    #[new]
    #[pyo3(signature = (body, fields, array, schedule, field_set=None, atmo=None))]
    fn new(
        body: &Body,
        fields: Vec<(String, Dist)>,
        array: &DetectorArray,
        schedule: &Schedule,
        field_set: Option<FieldSet>,
        atmo: Option<AtmoConfig>,
    ) -> PyResult<Self> {
        let prior = Prior {
            cloud: body.cloud.clone(),
            fields: fields.into_iter().map(|(n, d)| (n, d.inner)).collect(),
            array: array.inner.clone(),
            schedule: schedule.inner.clone(),
            field_set: field_set.map_or_else(RFieldSet::default, |f| f.inner),
            atmo: atmo.map(|a| a.inner),
        };
        // Validate at construction: an invalid prior raises on build, not mid-stream.
        prior
            .build()
            .validate()
            .map_err(|e| PyValueError::new_err(format!("invalid prior: {e}")))?;
        Ok(prior)
    }
}

/// A memory-bounded Python iterator over sampled scenarios. Each `__next__` samples scenario `i`
/// (keyed `scenario[i]` off `seed`, matching `generate::stream`) and runs it **with the GIL
/// released** — so a training loop overlaps compute — holding just one bundle resident at a time.
#[pyclass]
struct Stream {
    prior: RPrior,
    n: usize,
    root: u64,
    i: usize,
}

#[pymethods]
impl Stream {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }
    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Bundle>> {
        if self.i >= self.n {
            return Ok(None); // → StopIteration
        }
        let i = self.i;
        self.i += 1;
        let (prior, root) = (&self.prior, self.root);
        let inner = guarded(py, || generate::run(&prior.sample(scenario_key(root, i))))?;
        Ok(Some(Bundle { inner }))
    }
}

// ── verbs ────────────────────────────────────────────────────────────────────────────────────────

#[pyfunction]
fn run(py: Python<'_>, scenario: &Scenario) -> PyResult<Bundle> {
    let inner = guarded(py, || generate::run(&scenario.build()))?;
    Ok(Bundle { inner })
}

#[pyfunction]
#[pyo3(signature = (prior, n, seed=0))]
fn stream(prior: &Prior, n: usize, seed: u64) -> Stream {
    Stream {
        prior: prior.build(),
        n,
        root: seed,
        i: 0,
    }
}

/// Test hook: panic inside the guard, proving a Rust panic converts to a Python exception (never an
/// abort). Because `generate::run` is infallible, this is how the `RuntimeError` path is exercised.
#[pyfunction]
fn _panic_probe(py: Python<'_>) -> PyResult<()> {
    guarded(py, || panic!("intentional panic probe"))
}

// ── module ───────────────────────────────────────────────────────────────────────────────────────

#[pymodule]
fn cavendish(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Body>()?;
    m.add_class::<Path>()?;
    m.add_class::<Timing>()?;
    m.add_class::<Orient>()?;
    m.add_class::<Trajectory>()?;
    m.add_class::<Detector>()?;
    m.add_class::<DetectorArray>()?;
    m.add_class::<Schedule>()?;
    m.add_class::<FieldSet>()?;
    m.add_class::<UldmConfig>()?;
    m.add_class::<AtmoConfig>()?;
    m.add_class::<Dist>()?;
    m.add_class::<PhaseModelKind>()?;
    m.add_class::<Scenario>()?;
    m.add_class::<Prior>()?;
    m.add_class::<Stream>()?;
    m.add_class::<Bundle>()?;
    m.add_function(wrap_pyfunction!(cuboid, m)?)?;
    m.add_function(wrap_pyfunction!(sphere, m)?)?;
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_function(wrap_pyfunction!(stream, m)?)?;
    m.add_function(wrap_pyfunction!(_panic_probe, m)?)?;
    Ok(())
}
