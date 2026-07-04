//! `scenario` — the runnable `Scenario` and the measurement `Schedule` (minimal M1).
//!
//! Design: `design/scenario.md`. Milestone: `milestones/M1-physics-spine.md`.
//!
//! Re-exports the seam names it consumes so the reachability edges hold up to `generate`.

pub use config::{ConfigError, Dist, FieldSet, RunConfig};
pub use instrument::{Detector, DetectorArray, PhaseModel, PhaseModelKind};
pub use noise::{
    AtmoConfig, AtmoField, KeyRng, NoiseSource, NoiseStack, ShotNoise, VibrationResidual,
};
pub use source::{
    BodyMotion, Orient, Path, Prescribed, Source, SourceDynamics, Timing, Trajectory,
};
pub use uldm::{uldm_phase, UldmConfig};

use gravity::Cloud;
use math::{Isometry3, Quat, Vec3};

/// The measurement times. Uniform in M1; jitter/gaps arrive with M6's schedules.
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    pub times: Vec<f64>,
}

impl Schedule {
    /// `n` measurements spaced `cadence` seconds apart, starting at `t = 0`.
    pub fn uniform(cadence: f64, n: usize) -> Self {
        Schedule {
            times: (0..n).map(|i| i as f64 * cadence).collect(),
        }
    }
}

/// One runnable scene: a source, a detector array, a schedule, the seed, and the phase model.
pub struct Scenario {
    pub source: Box<dyn SourceDynamics>,
    pub array: DetectorArray,
    pub schedule: Schedule,
    pub seed: u64,
    /// Which `PhaseModel` `generate` uses (default `PropagationIntegral`, the reference).
    // DEFERRED (M6b): `phase_model` is a forward-model selector; its designed home is `config`, but
    // seating it there edits the validated forward-model crates. Left on `Scenario` so M6b's diff stays
    // off the forward model — to be moved in a dedicated "seat forward-model config" PR.
    pub phase_model: PhaseModelKind,
    /// Which optional bundle field groups to compute (default: none).
    pub field_set: FieldSet,
    /// The ordered post-hoc noise stack (default: empty).
    pub noise: NoiseStack,
    /// The ULDM common-mode channel (default: off).
    pub uldm: Option<UldmConfig>,
    /// The atmospheric-GGN field source (default: off).
    pub atmo: Option<AtmoConfig>,
}

impl Scenario {
    pub fn new(
        source: Box<dyn SourceDynamics>,
        array: DetectorArray,
        schedule: Schedule,
        seed: u64,
    ) -> Self {
        Scenario {
            source,
            array,
            schedule,
            seed,
            phase_model: PhaseModelKind::default(),
            field_set: FieldSet::default(),
            noise: NoiseStack::default(),
            uldm: None,
            atmo: None,
        }
    }

    /// Attach the post-hoc noise stack (builder style).
    pub fn with_noise(mut self, noise: NoiseStack) -> Self {
        self.noise = noise;
        self
    }

    /// Attach the ULDM common-mode channel (builder style).
    pub fn with_uldm(mut self, uldm: UldmConfig) -> Self {
        self.uldm = Some(uldm);
        self
    }

    /// Attach the atmospheric-GGN field source (builder style).
    pub fn with_atmo(mut self, atmo: AtmoConfig) -> Self {
        self.atmo = Some(atmo);
        self
    }

    /// Select the phase model (builder style).
    pub fn with_phase_model(mut self, kind: PhaseModelKind) -> Self {
        self.phase_model = kind;
        self
    }

    /// Select which optional field groups to compute (builder style).
    pub fn with_field_set(mut self, field_set: FieldSet) -> Self {
        self.field_set = field_set;
        self
    }
}

/// A distribution over `Scenario`s — **optional batch sugar** over direct `Scenario` construction,
/// which stays the primary path. A fixed body template plus a set of **named** scalar distributions;
/// `sample(seed)` is total on a validated `Prior`.
///
/// Each field is keyed by its NAME through the RNG key tree (`prior/<name>`), independently of the
/// others — so adding a field gives it a fresh path and leaves every existing field's draw
/// bit-identical (extension-stability). Draws never depend on field order or on batch position, only
/// on `(seed, name)`.
pub struct Prior {
    /// The body shape (fixed); a sampled `mass` field rescales its total mass.
    pub cloud: Cloud,
    /// Named scalar distributions. Recognised names: `mass` (kg), `standoff` (m, placement x),
    /// `uldm_amp`, `uldm_freq` (Hz). Unknown names are drawn (keeping keys stable) but unused.
    pub fields: Vec<(String, Dist)>,
    /// Shared across every sample.
    pub array: DetectorArray,
    pub schedule: Schedule,
    pub field_set: FieldSet,
}

impl Prior {
    /// Validate every field's distribution (so `sample` is total).
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.fields.iter().try_for_each(|(_, d)| d.validate())
    }

    /// Draw every named field independently — each keyed `prior/<name>` off `seed`, so the draw is a
    /// pure function of `(seed, name)` and adding a field cannot perturb existing draws.
    pub fn draw_fields(&self, seed: u64) -> Vec<(String, f64)> {
        self.fields
            .iter()
            .map(|(name, dist)| {
                let mut rng = KeyRng::stream(seed, &format!("prior/{name}"));
                (name.clone(), dist.sample(&mut || rng.next_unit()))
            })
            .collect()
    }

    /// A runnable `Scenario` for this seed. Total on a validated `Prior`.
    pub fn sample(&self, seed: u64) -> Scenario {
        let vals: std::collections::HashMap<String, f64> =
            self.draw_fields(seed).into_iter().collect();
        let get = |name: &str, default: f64| vals.get(name).copied().unwrap_or(default);

        // Body: rescale the template cloud's total mass to the sampled `mass`.
        let mut cloud = self.cloud.clone();
        let total: f64 = cloud.ms.iter().sum();
        if total > 0.0 {
            let k = get("mass", total) / total;
            cloud.ms.iter_mut().for_each(|m| *m *= k);
        }
        let place = Isometry3::new(Quat::identity(), Vec3::new(get("standoff", 3.0), 0.0, 0.0));
        let traj = Trajectory::new(place, Path::Static, Timing::Uniform { rate: 0.0 })
            .with_orient(Orient::Fixed(Quat::identity()));
        let source = Source::new(cloud, traj);

        let mut scn = Scenario::new(
            Box::new(source),
            self.array.clone(),
            self.schedule.clone(),
            seed,
        )
        .with_field_set(self.field_set);
        if vals.contains_key("uldm_amp") || vals.contains_key("uldm_freq") {
            scn = scn.with_uldm(UldmConfig {
                amplitude: get("uldm_amp", 0.0),
                frequency: get("uldm_freq", 0.1),
                phase: 0.0,
            });
        }
        scn
    }
}
