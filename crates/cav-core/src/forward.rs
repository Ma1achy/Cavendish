//! The forward model: what one gradiometer reads from the masses around it.
//!
//! A tower holds two atom interferometers, one launched from its base and one `Δr` above it. Each
//! splits its atoms into two arms that fly apart and are brought back together after `2T`. An arm
//! that spends its flight in a different gravitational potential comes back with a different
//! phase: for interferometer `k`,
//!
//! ```text
//! δφ_k = (m_A / ħ) ∫₀^2T [ V(upper arm, t) − V(lower arm, t) ] dt
//! ```
//!
//! and the gradiometer reads the difference of the two, `ΔΦ = δφ₁ − δφ₂`: lower minus upper
//! (George, 21 Nov 2026). **Interferometer 1 is the lower one**, in the reference scripts (they
//! launch it from z = 0 and the other from z = Δr) and in v1 (`build_arms` returns
//! `[make(0.0), make(ifo_sep)]`); v1 reads `δφ₂ − δφ₁`, upper minus lower, which is
//! [`Sign::UpperMinusLower`] here.
//!
//! The atom and the source are evaluated at the same time. The arms never go below the launch
//! height: an arm that falls back to it stops there and waits for the next kick or the next
//! sequence, as the reference scripts code it ([`Arm`]).
//!
//! # How the numbers are kept honest (REBUILD §3.4)
//!
//! - **Local frames.** A source's position is taken relative to the tower's base in `f64` before
//!   anything else is done with it, and arm heights are kept as the rise above the arm's own
//!   launch point. A scene 10 km from the origin reads what it reads at the origin.
//! - **No difference of large numbers.** `1/|a| − 1/|b|` is formed as
//!   `(|b|² − |a|²) / (|a||b|(|a| + |b|))`, and `|b|² − |a|²` from the arms' separation, so the
//!   monopole that cancels between the arms never appears.
//! - **Units of order one.** The sum over the flight is in kg s / m; `G·m_A/ħ` multiplies it once.
//! - **The quadrature is bounded.** The default rule is Gauss–Legendre on pieces cut at every kink
//!   of the arms (the π pulse, each landing) and of the source (a body that appears, a pass that
//!   starts); its error is measured against a much finer rule by the validation cases.

use std::f64::consts::{PI, TAU};

use crate::{Vec3, constants};

/// Which way round the two interferometers are differenced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    /// `ΔΦ = δφ₁ − δφ₂`: the lower interferometer minus the upper. The engine's and the scripts'.
    LowerMinusUpper,
    /// `ΔΦ = δφ₂ − δφ₁`: v1's.
    UpperMinusLower,
}

/// How the integral over the flight is taken.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Quadrature {
    /// Gauss–Legendre with `order` nodes on every panel; each smooth piece of the flight is cut
    /// into equal panels no longer than `panel` seconds. The engine's rule.
    Gauss { order: usize, panel: f64 },
    /// Composite Simpson at about `step` seconds on `[0, T]` and on `[T, 2T]`, with no regard for
    /// landings: v1's rule, node for node, for the oracle tie.
    Simpson { step: f64 },
    /// The reference scripts' sampling, for parity tests only: `nodes` nodes at `j·step` joined by
    /// the trapezoid rule, with the arm taken `lag` steps later than the source. The scripts as
    /// published have `lag = 1`; `lag = 0` is the same rule with the offset corrected.
    ScriptTrapezoid { step: f64, nodes: usize, lag: usize },
}

/// The engine's rule: sixth-order panels of at most T/8 at AION-10's T. Its error on the anchors
/// is in the validation report (`forward.quadrature_*`).
pub const DEFAULT_QUADRATURE: Quadrature = Quadrature::Gauss { order: 6, panel: 0.1 };

/// Everything about the instrument that the phase depends on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instrument {
    /// Vertical separation of the two interferometers, Δr, metres.
    pub ifo_separation: f64,
    /// Half the sequence, T, seconds: the π pulse is at T and the readout at 2T.
    pub half_sequence: f64,
    /// Launch velocity of the lower arm, u₀, metres per second.
    pub launch_velocity: f64,
    /// Local gravity, metres per second squared.
    pub gravity: f64,
    /// The atom's mass, kilograms.
    pub atom_mass: f64,
    /// ħ, joule seconds.
    pub hbar: f64,
    /// G, m³ kg⁻¹ s⁻².
    pub big_g: f64,
    /// The wavelength of the light that kicks the atoms, metres.
    pub wavelength: f64,
    /// How many photon momenta a kick gives.
    pub kicks: f64,
    /// Whether an arm that falls back to its launch height stops there.
    pub floor: bool,
    /// Which way round the interferometers are differenced.
    pub sign: Sign,
    /// How the flight integral is taken.
    pub quadrature: Quadrature,
}

impl Instrument {
    /// AION-10 with George's defaults: ⁸⁷Sr, u₀ = 3.86 m/s, a floor at the launch height, lower
    /// minus upper, CODATA constants.
    #[must_use]
    pub const fn aion10() -> Self {
        Self {
            ifo_separation: crate::defaults::IFO_SEPARATION,
            half_sequence: crate::defaults::HALF_SEQUENCE,
            launch_velocity: crate::defaults::LAUNCH_VELOCITY,
            gravity: constants::LOCAL_GRAVITY,
            atom_mass: constants::SR87_MASS,
            hbar: constants::HBAR,
            big_g: constants::BIG_G,
            wavelength: constants::CLOCK_WAVELENGTH,
            kicks: constants::LMT_KICKS,
            floor: true,
            sign: Sign::LowerMinusUpper,
            quadrature: DEFAULT_QUADRATURE,
        }
    }

    /// The same geometry with v1's conventions (tag `v1-final`): no floor, the rounded mass and ħ,
    /// upper minus lower, Simpson at 0.01 s on each half. For the oracle tie only.
    #[must_use]
    pub const fn with_v1_conventions(self) -> Self {
        Self {
            atom_mass: constants::reference::ATOM_MASS,
            hbar: constants::reference::HBAR,
            big_g: constants::BIG_G,
            floor: false,
            sign: Sign::UpperMinusLower,
            quadrature: Quadrature::Simpson { step: 0.01 },
            ..self
        }
    }

    /// The same geometry with the reference scripts' constants (their mass, ħ and G); the floor,
    /// the sign and the same-time rule are already the engine's. The quadrature is left alone.
    #[must_use]
    pub const fn with_script_constants(self) -> Self {
        Self {
            atom_mass: constants::reference::ATOM_MASS,
            hbar: constants::reference::HBAR,
            big_g: constants::reference::SCRIPT_BIG_G,
            ..self
        }
    }

    /// The AICE 100 m preset of the reference scripts: Δr = 70 m, T = 2.5 s, and the launch that
    /// closes the arms at the launch height at 2T, u₀ = gT − v_kick/2.
    #[must_use]
    pub fn aice() -> Self {
        let mut instrument = Self {
            ifo_separation: 70.0,
            half_sequence: 2.5,
            quadrature: DEFAULT_QUADRATURE,
            ..Self::aion10()
        };
        instrument.launch_velocity = instrument.symmetric_launch();
        instrument
    }

    /// The velocity a kick gives: `n ħ k / m_A`, metres per second.
    #[must_use]
    pub fn kick_velocity(&self) -> f64 {
        self.kicks * self.hbar * (TAU / self.wavelength) / self.atom_mass
    }

    /// The launch velocity that brings both arms back to the launch height exactly at 2T:
    /// `gT − v_kick/2`. Each arm is then its own mirror image about T.
    #[must_use]
    pub fn symmetric_launch(&self) -> f64 {
        self.gravity * self.half_sequence - 0.5 * self.kick_velocity()
    }

    /// The flight time, 2T, seconds.
    #[must_use]
    pub fn flight_time(&self) -> f64 {
        2.0 * self.half_sequence
    }

    /// The lower and the upper arm. Both interferometers fly the same pair.
    #[must_use]
    pub fn arms(&self) -> [Arm; 2] {
        let kick = self.kick_velocity();
        let arm = |first: f64, kick: f64| Arm::new(first, kick, self.half_sequence, self.gravity, self.floor);
        [arm(self.launch_velocity, kick), arm(self.launch_velocity + kick, -kick)]
    }

    /// `G·m_A/ħ`: what turns the flight's sum (kg s / m) into radians.
    #[must_use]
    pub fn phase_scale(&self) -> f64 {
        self.big_g * self.atom_mass / self.hbar
    }

    /// The pieces of the flight the Gauss rule integrates over: `[a, b]` pairs between every kink
    /// (the π pulse, each landing, each of `breaks`), leaving out the pieces in which both arms
    /// are on the floor. There the arms are at one point, the difference of potential between
    /// them is nothing whatever the source, and nothing is left to integrate; for a long sequence
    /// with a modest launch that is most of the flight.
    fn pieces(&self, breaks: &[f64]) -> Vec<(f64, f64)> {
        let [lower, upper] = self.arms();
        let two_t = self.flight_time();
        let mut cuts = vec![0.0, self.half_sequence, two_t];
        cuts.extend(lower.landings().into_iter().chain(upper.landings()).chain(breaks.iter().copied()));
        cuts.retain(|c| (0.0..=two_t).contains(c));
        cuts.sort_by(f64::total_cmp);
        // Cuts closer than a nanosecond are one cut: a sliver of a panel adds nothing.
        cuts.dedup_by(|b, a| *b - *a < 1e-9);
        cuts.windows(2)
            .map(|piece| (piece[0], piece[1]))
            .filter(|&(a, b)| {
                let middle = 0.5 * (a + b);
                !(self.floor && lower.grounded(middle) && upper.grounded(middle))
            })
            .collect()
    }

    /// How many panels a piece of `length` seconds is cut into.
    fn panels(length: f64, panel: f64) -> u64 {
        (length / panel).ceil().max(1.0) as u64
    }

    /// How many nodes [`Self::visit`] would visit: the work of one read, known before it is done.
    #[must_use]
    pub fn node_count(&self, breaks: &[f64]) -> u64 {
        match self.quadrature {
            Quadrature::Gauss { order, panel } => {
                self.pieces(breaks).iter().map(|(a, b)| Self::panels(b - a, panel) * order as u64).sum()
            }
            Quadrature::Simpson { step } => {
                let n = ((self.half_sequence / step).round() as u64).max(2);
                2 * (n + n % 2 + 1)
            }
            Quadrature::ScriptTrapezoid { nodes, .. } => nodes as u64,
        }
    }

    /// Visits the nodes of the flight integral in order of time, one at a time: nothing is stored,
    /// so a read's memory does not grow with the length of the flight. `breaks` are flight times
    /// (seconds after launch) at which the source is not smooth; the Gauss rule cuts its pieces
    /// there, the reference rules do not. `rule` is the Gauss rule for the instrument's order
    /// ([`gauss_legendre`]); the reference rules ignore it.
    ///
    /// # Panics
    /// When the quadrature asks for nothing: no nodes, or a step or panel that is not positive.
    pub fn visit(&self, breaks: &[f64], rule: &[(f64, f64)], visitor: &mut dyn FnMut(Node)) {
        let [lower, upper] = self.arms();
        let node = |tau: f64, arm_time: f64, weight: f64| Node {
            tau,
            weight,
            lower: lower.rise(arm_time),
            upper: upper.rise(arm_time),
        };
        match self.quadrature {
            Quadrature::Gauss { order, panel } => {
                assert!(order >= 1 && panel > 0.0, "a Gauss rule needs nodes and a positive panel");
                assert_eq!(rule.len(), order, "the rule handed in is not the instrument's");
                for (a, b) in self.pieces(breaks) {
                    let panels = Self::panels(b - a, panel);
                    let width = (b - a) / panels as f64;
                    for p in 0..panels {
                        let left = a + p as f64 * width;
                        for &(x, w) in rule {
                            let tau = left + 0.5 * width * (x + 1.0);
                            visitor(node(tau, tau, 0.5 * width * w));
                        }
                    }
                }
            }
            Quadrature::Simpson { step } => {
                assert!(step > 0.0, "Simpson's rule needs a positive step");
                for (a, b) in [(0.0, self.half_sequence), (self.half_sequence, self.flight_time())] {
                    // v1's count: the nearest whole number of steps, at least two, made even.
                    let mut n = ((b - a) / step).round() as usize;
                    n = n.max(2);
                    n += n % 2;
                    let h = (b - a) / n as f64;
                    for i in 0..=n {
                        let weight = if i == 0 || i == n {
                            1.0
                        } else if i % 2 == 1 {
                            4.0
                        } else {
                            2.0
                        };
                        let tau = a + i as f64 * h;
                        visitor(node(tau, tau, weight * h / 3.0));
                    }
                }
            }
            Quadrature::ScriptTrapezoid { step, nodes: count, lag } => {
                assert!(step > 0.0 && count >= 2, "the scripts' rule needs a step and two nodes");
                for j in 0..count {
                    let weight = if j == 0 || j + 1 == count { 0.5 * step } else { step };
                    visitor(node(j as f64 * step, (j + lag) as f64 * step, weight));
                }
            }
        }
    }

    /// The nodes of the flight integral, gathered into a table. For looking at a rule (tests, the
    /// oracle tier); a read does not use it, because the table grows with the flight.
    #[must_use]
    pub fn flight(&self, breaks: &[f64]) -> Flight {
        let rule = match self.quadrature {
            Quadrature::Gauss { order, .. } => gauss_legendre(order),
            _ => Vec::new(),
        };
        let mut nodes = Vec::new();
        self.visit(breaks, &rule, &mut |node| nodes.push(node));
        Flight { nodes, duration: self.flight_time() }
    }
}

/// One arm's flight, as its rise above its own launch point: free fall under `g`, a kick at T,
/// and (with a floor) a stop wherever it falls back to the launch height.
///
/// The floor is the reference scripts': an arm that reaches its launch height while falling stays
/// there with no velocity. If that happens before T, the kick at T starts it again from rest; if
/// it happens after T, it stays there until the next sequence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arm {
    first: f64,
    half: f64,
    gravity: f64,
    /// When, in the first half, the arm lands (with a floor).
    landed_first: Option<f64>,
    rise_at_half: f64,
    /// Velocity just after the kick.
    second: f64,
    /// How long after T the arm lands (with a floor).
    landed_second: Option<f64>,
}

impl Arm {
    /// An arm launched at `first` metres per second and kicked by `kick` at `half`.
    #[must_use]
    pub fn new(first: f64, kick: f64, half: f64, gravity: f64, floor: bool) -> Self {
        let fall = |v: f64, t: f64| v * t - 0.5 * gravity * t * t;
        let landed_first = if floor {
            let back = if first > 0.0 { 2.0 * first / gravity } else { 0.0 };
            (back < half).then_some(back)
        } else {
            None
        };
        let (rise_at_half, before_kick) = match landed_first {
            Some(_) => (0.0, 0.0),
            None => (fall(first, half), first - gravity * half),
        };
        let second = before_kick + kick;
        let landed_second = if floor {
            let back = if rise_at_half <= 0.0 && second <= 0.0 {
                0.0
            } else {
                (second + (second * second + 2.0 * gravity * rise_at_half).sqrt()) / gravity
            };
            (back < half).then_some(back)
        } else {
            None
        };
        Self { first, half, gravity, landed_first, rise_at_half, second, landed_second }
    }

    /// Height above the launch point at `tau` seconds after launch, metres. Past 2T an arm with a
    /// floor is on it: the atoms wait at the launch height for the next sequence.
    #[must_use]
    pub fn rise(&self, tau: f64) -> f64 {
        let fall = |v: f64, t: f64| v * t - 0.5 * self.gravity * t * t;
        if tau <= self.half {
            match self.landed_first {
                Some(back) if tau >= back => 0.0,
                _ => fall(self.first, tau),
            }
        } else {
            let since = tau - self.half;
            match self.landed_second {
                Some(back) if since >= back => 0.0,
                // With a floor, an arm still in the air at 2T has (to rounding) just arrived.
                None if self.landed_first.is_some() && since >= self.half => 0.0,
                _ => self.rise_at_half + fall(self.second, since),
            }
        }
    }

    /// Whether the arm is on the floor at `tau`: landed, and not yet kicked off it again.
    #[must_use]
    pub fn grounded(&self, tau: f64) -> bool {
        if tau <= self.half {
            self.landed_first.is_some_and(|back| tau >= back)
        } else {
            self.landed_second.is_some_and(|back| tau - self.half >= back)
        }
    }

    /// The flight times at which the arm lands, where its path has a kink.
    #[must_use]
    pub fn landings(&self) -> Vec<f64> {
        self.landed_first.into_iter().chain(self.landed_second.map(|back| self.half + back)).collect()
    }
}

/// One node of the flight integral.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    /// When the source is evaluated, seconds after launch.
    pub tau: f64,
    /// The node's weight, seconds.
    pub weight: f64,
    /// The lower and the upper arm's rise above the launch point, metres.
    pub lower: f64,
    pub upper: f64,
}

/// The nodes of one flight. They depend on the instrument alone, so they are built once.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    pub nodes: Vec<Node>,
    /// 2T, seconds: a shot read out at `t` was launched at `t − duration`.
    pub duration: f64,
}

/// The nodes and weights of the `n`-point Gauss–Legendre rule on [−1, 1].
///
/// # Panics
/// When `n` is zero.
#[must_use]
pub fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    assert!(n >= 1, "a rule needs a node");
    // P_n and its derivative by the three-term recurrence.
    let legendre = |x: f64| {
        let (mut previous, mut current) = (1.0, x);
        for k in 2..=n {
            let k = k as f64;
            (previous, current) = (current, ((2.0 * k - 1.0) * x * current - (k - 1.0) * previous) / k);
        }
        let value = if n == 0 { 1.0 } else { current };
        (value, n as f64 * (x * value - previous) / (x * x - 1.0))
    };
    (0..n)
        .map(|i| {
            // Newton from the Chebyshev guess: a fixed number of steps, each of which squares the error.
            let mut x = -(PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
            if n == 1 {
                return (0.0, 2.0);
            }
            for _ in 0..12 {
                let (p, dp) = legendre(x);
                x -= p / dp;
            }
            let (_, dp) = legendre(x);
            (x, 2.0 / ((1.0 - x * x) * dp * dp))
        })
        .collect()
}

/// Something with mass, as the flight integral sees it.
pub trait Source: Sync {
    /// `(V(upper arm) − V(lower arm)) / G` for each interferometer at absolute time `time`,
    /// kilograms per metre. It is nothing when `lower` equals `upper` (the two arms at one point):
    /// the integrator relies on that and does not ask while both arms are on the floor. `tower` is the tower's base (the lower launch point), `heights` the
    /// two launch points' heights above it, and `lower` and `upper` the arms' rise above their
    /// launch point.
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2];

    /// The times in `(from, to)` at which the source is not smooth: it appears, vanishes, starts
    /// or stops. The Gauss rule cuts its pieces there.
    fn breaks(&self, _from: f64, _to: f64, _out: &mut Vec<f64>) {}
}

/// The two interferometers' phases for one shot, radians.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Reading {
    /// δφ₁: the lower interferometer.
    pub lower: f64,
    /// δφ₂: the upper interferometer.
    pub upper: f64,
}

/// A channel of a tower's signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    /// The gradiometer's differential phase, ΔΦ.
    Gradiometer,
    /// The lower interferometer alone, δφ₁.
    Lower,
    /// The upper interferometer alone, δφ₂.
    Upper,
}

impl Channel {
    pub const ALL: [Self; 3] = [Self::Gradiometer, Self::Lower, Self::Upper];
}

impl Reading {
    /// The gradiometer's differential phase with the given sign.
    #[must_use]
    pub fn gradiometer(self, sign: Sign) -> f64 {
        match sign {
            Sign::LowerMinusUpper => self.lower - self.upper,
            Sign::UpperMinusLower => self.upper - self.lower,
        }
    }

    /// One channel.
    #[must_use]
    pub fn channel(self, channel: Channel, sign: Sign) -> f64 {
        match channel {
            Channel::Gradiometer => self.gradiometer(sign),
            Channel::Lower => self.lower,
            Channel::Upper => self.upper,
        }
    }
}

impl std::ops::Add for Reading {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self { lower: self.lower + o.lower, upper: self.upper + o.upper }
    }
}

/// A reading with a measured bound on its quadrature error.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Audited {
    pub reading: Reading,
    /// How far the last two rules disagreed, radians, over the two interferometers and their
    /// difference: an estimate of the error of the coarser of the two, so an overestimate of the
    /// error of this reading. `None` for the reference rules, which are not refined.
    pub estimate: Option<f64>,
    /// How many times the panels were halved.
    pub halvings: u32,
    /// Whether the last two rules agreed to the tolerance. When they did not (the halvings or the
    /// work budget ran out first, or the reading is not a number) the reading is the best there
    /// is and `estimate` says how good: it must not be shown as settled.
    pub settled: bool,
    /// How many nodes were evaluated, over every rule tried: the work this read cost.
    pub nodes: u64,
}

/// The tolerance of the audited read: this many radians …
pub const PHASE_TOLERANCE: f64 = 1e-10;
/// … or this fraction of the phase, whichever is larger (rounding sets a floor on what two sums
/// of the same integral can agree to).
pub const PHASE_TOLERANCE_RELATIVE: f64 = 1e-12;
/// The most times the audited read halves its panels: 0.1 s down to 0.4 ms.
pub const MAX_HALVINGS: u32 = 8;
/// The most nodes one audited read evaluates, over every rule it tries: its budget of work. A
/// read that would go past it stops and reports that it did not settle. An AION-10 read uses
/// about three hundred; four million is some tens of milliseconds.
pub const MAX_NODES: u64 = 4_000_000;

/// An instrument ready to read shots. It holds the instrument and its Gauss rule and nothing
/// that grows with the flight: nodes are made and used one at a time.
#[derive(Clone, Debug)]
pub struct Reader {
    instrument: Instrument,
    /// The Gauss rule of the instrument's order; empty for the reference rules.
    rule: Vec<(f64, f64)>,
}

impl Reader {
    /// A reader for `instrument`.
    #[must_use]
    pub fn new(instrument: Instrument) -> Self {
        let rule = match instrument.quadrature {
            Quadrature::Gauss { order, .. } => gauss_legendre(order),
            _ => Vec::new(),
        };
        Self { instrument, rule }
    }

    #[must_use]
    pub const fn instrument(&self) -> &Instrument {
        &self.instrument
    }

    /// The instrument with its Gauss panels halved `level` times.
    fn halved(&self, level: u32) -> Instrument {
        let quadrature = match self.instrument.quadrature {
            Quadrature::Gauss { order, panel } => {
                Quadrature::Gauss { order, panel: panel / f64::from(1u32 << level) }
            }
            other => other,
        };
        Instrument { quadrature, ..self.instrument }
    }

    /// The breaks of the source inside the flight read out at `readout`, as flight times. The
    /// reference rules do not cut at them.
    fn breaks(&self, source: &dyn Source, readout: f64) -> Vec<f64> {
        let mut breaks = Vec::new();
        if matches!(self.instrument.quadrature, Quadrature::Gauss { .. }) {
            let launch = readout - self.instrument.flight_time();
            source.breaks(launch, readout, &mut breaks);
            for b in &mut breaks {
                *b -= launch;
            }
        }
        breaks
    }

    /// Visits the nodes of the flight with the panels halved `level` times, cut at `breaks`: the
    /// nodes a read at that level uses.
    pub fn visit(&self, level: u32, breaks: &[f64], visitor: &mut dyn FnMut(Node)) {
        self.halved(level).visit(breaks, &self.rule, visitor);
    }

    /// The flight integral at one level of refinement, node by node.
    fn sum(&self, level: u32, breaks: &[f64], tower: Vec3, source: &dyn Source, readout: f64) -> Reading {
        let launch = readout - self.instrument.flight_time();
        let heights = [0.0, self.instrument.ifo_separation];
        let mut sum = [0.0; 2];
        self.visit(level, breaks, &mut |node| {
            let d = source.difference(tower, heights, node.lower, node.upper, launch + node.tau);
            sum[0] += node.weight * d[0];
            sum[1] += node.weight * d[1];
        });
        let scale = self.instrument.phase_scale();
        Reading { lower: scale * sum[0], upper: scale * sum[1] }
    }

    /// What `source` adds to the shot of the tower at `tower` that is read out at `readout`
    /// seconds: launched at `readout − 2T`, integrated over the flight with the rule of the
    /// instrument as it stands. [`Self::read_to`] is the read that checks itself.
    #[must_use]
    pub fn read(&self, tower: Vec3, source: &dyn Source, readout: f64) -> Reading {
        let breaks = self.breaks(source, readout);
        self.sum(0, &breaks, tower, source, readout)
    }

    /// The same shot, read until it stops changing: the Gauss panels are halved until two
    /// successive readings agree to `tolerance` radians (or to [`PHASE_TOLERANCE_RELATIVE`] of
    /// the phase, if that is larger). The finer of the two is returned with their difference,
    /// which estimates the error of the coarser.
    ///
    /// The work is bounded: at most [`MAX_HALVINGS`] halvings and at most [`MAX_NODES`] nodes in
    /// all, counted before each rule is run. A read that stops for either reason says so
    /// (`settled` is false) and still returns its best reading and its estimate.
    #[must_use]
    pub fn read_to(&self, tower: Vec3, source: &dyn Source, readout: f64, tolerance: f64) -> Audited {
        let breaks = self.breaks(source, readout);
        let mut nodes = self.instrument.node_count(&breaks);
        let mut reading = self.sum(0, &breaks, tower, source, readout);
        if !matches!(self.instrument.quadrature, Quadrature::Gauss { .. }) {
            return Audited { reading, estimate: None, halvings: 0, settled: true, nodes };
        }
        let (mut estimate, mut halvings, mut settled) = (f64::INFINITY, 0, false);
        while halvings < MAX_HALVINGS {
            let next = self.halved(halvings + 1).node_count(&breaks);
            if nodes + next > MAX_NODES {
                break;
            }
            halvings += 1;
            nodes += next;
            let finer = self.sum(halvings, &breaks, tower, source, readout);
            let (dl, du) = (finer.lower - reading.lower, finer.upper - reading.upper);
            estimate = dl.abs().max(du.abs()).max((dl - du).abs());
            let size = finer.lower.abs().max(finer.upper.abs());
            reading = finer;
            if estimate <= tolerance.max(PHASE_TOLERANCE_RELATIVE * size) {
                settled = true;
                break;
            }
            // A reading that is not a number will not become one: halving again changes nothing.
            if estimate.is_nan() {
                break;
            }
        }
        Audited { reading, estimate: Some(estimate), halvings, settled, nodes }
    }

    /// The gradiometer's phase from `source` alone, with the sign and the rule of the instrument.
    #[must_use]
    pub fn phase(&self, tower: Vec3, source: &dyn Source, readout: f64) -> f64 {
        self.read(tower, source, readout).gradiometer(self.instrument.sign)
    }
}

/// A point mass, or a uniform sphere, that moves.
///
/// Outside itself a uniform sphere is a point mass; inside, its potential is
/// `−GM(3R² − r²)/(2R³)`, finite at the centre, so a sphere may pass through an atom's path where
/// a point may not.
pub struct Mass<P: Fn(f64) -> Vec3 + Sync> {
    /// Kilograms.
    pub mass: f64,
    /// The sphere's radius, metres; zero for a point.
    pub radius: f64,
    /// Where its centre is at a time, metres, in the world's frame.
    pub position: P,
    /// It exists from the first time up to the second, seconds.
    pub exists: (f64, f64),
    /// Times at which its motion is not smooth.
    pub breaks: Vec<f64>,
}

impl<P: Fn(f64) -> Vec3 + Sync> Mass<P> {
    /// A point mass that always exists and moves smoothly.
    pub const fn point(mass: f64, position: P) -> Self {
        Self { mass, radius: 0.0, position, exists: (f64::NEG_INFINITY, f64::INFINITY), breaks: Vec::new() }
    }
}

impl<P: Fn(f64) -> Vec3 + Sync> Source for Mass<P> {
    fn difference(&self, tower: Vec3, heights: [f64; 2], lower: f64, upper: f64, time: f64) -> [f64; 2] {
        if !(time >= self.exists.0 && time < self.exists.1) {
            return [0.0; 2];
        }
        // The local frame: everything from here on is an offset from the tower's base.
        let offset = (self.position)(time) - tower;
        let across = offset.x * offset.x + offset.y * offset.y;
        heights.map(|height| {
            // Heights of the two arm points above the source.
            let (below, above) = (height + lower - offset.z, height + upper - offset.z);
            let (b, a) = ((across + below * below).sqrt(), (across + above * above).sqrt());
            if a >= self.radius && b >= self.radius {
                // 1/a − 1/b without the cancellation; b² − a² = (lower − upper)(below + above).
                -self.mass * (lower - upper) * (below + above) / (a * b * (a + b))
            } else {
                let radius = self.radius;
                let inverse = |r: f64| {
                    if r >= radius {
                        1.0 / r
                    } else {
                        (3.0 * radius * radius - r * r) / (2.0 * radius.powi(3))
                    }
                };
                -self.mass * (inverse(a) - inverse(b))
            }
        })
    }

    fn breaks(&self, from: f64, to: f64, out: &mut Vec<f64>) {
        let ends = [self.exists.0, self.exists.1];
        out.extend(self.breaks.iter().chain(&ends).copied().filter(|t| *t > from && *t < to));
    }
}

/// A uniform slab that extends to the horizon, with its top at or below every arm.
///
/// Above an infinite slab the field is uniform, `2πGρh`, whatever the height: the potential is
/// linear, each interferometer's arms see the same difference, and the gradiometer reads nothing.
/// The difference is formed from the arms' rise alone, so the two interferometers get the same
/// bits and the null is exact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slab {
    /// Kilograms per cubic metre.
    pub density: f64,
    /// Metres.
    pub thickness: f64,
}

impl Source for Slab {
    fn difference(&self, _: Vec3, _: [f64; 2], lower: f64, upper: f64, _: f64) -> [f64; 2] {
        [TAU * self.density * self.thickness * (upper - lower); 2]
    }
}

/// A uniform disc of finite radius under a tower that stands on its axis: the finite floor whose
/// edge the infinite slab does not have. On the axis the potential has a closed form,
/// `V(z)/G = −2πρ [F(z − bottom) − F(z − top)]` with `F(s) = ½(s√(s² + R²) + R² asinh(s/R)) − s²/2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxialDisc {
    /// Kilograms per cubic metre.
    pub density: f64,
    /// Metres.
    pub thickness: f64,
    /// Metres.
    pub radius: f64,
    /// How far the disc's top surface lies below the tower's base, metres; not negative.
    pub depth: f64,
}

impl Source for AxialDisc {
    fn difference(&self, _: Vec3, heights: [f64; 2], lower: f64, upper: f64, _: f64) -> [f64; 2] {
        let r = self.radius;
        let f = |s: f64| 0.5 * (s * s.hypot(r) + r * r * (s / r).asinh()) - 0.5 * s * s;
        let potential = |z: f64| {
            let above_top = z + self.depth;
            -TAU * self.density * (f(above_top + self.thickness) - f(above_top))
        };
        heights.map(|height| potential(height + upper) - potential(height + lower))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: Vec3 = Vec3::ZERO;

    /// A fixed point mass.
    fn fixed(mass: f64, at: Vec3) -> Mass<impl Fn(f64) -> Vec3 + Sync> {
        Mass::point(mass, move |_| at)
    }

    /// The same integral by brute force: the plain formula `−M(1/a − 1/b)`, absolute coordinates,
    /// a very fine Gauss rule. Slow and simple, to hold the fast one to.
    fn brute(
        instrument: &Instrument,
        tower: Vec3,
        mass: f64,
        path: &dyn Fn(f64) -> Vec3,
        readout: f64,
    ) -> f64 {
        let fine = Instrument { quadrature: Quadrature::Gauss { order: 12, panel: 0.01 }, ..*instrument };
        let flight = fine.flight(&[]);
        let mut sum = [0.0f64; 2];
        for node in &flight.nodes {
            let source = path(readout - flight.duration + node.tau);
            for (k, height) in [0.0, instrument.ifo_separation].into_iter().enumerate() {
                let point = |rise: f64| Vec3::new(tower.x, tower.y, tower.z + height + rise);
                let v = |rise: f64| -mass / (point(rise) - source).length();
                sum[k] += node.weight * (v(node.upper) - v(node.lower));
            }
        }
        instrument.phase_scale() * (sum[0] - sum[1])
    }

    #[test]
    fn gauss_rules_integrate_polynomials_exactly() {
        for n in [1, 2, 3, 6, 12, 24] {
            let rule = gauss_legendre(n);
            assert_eq!(rule.len(), n);
            assert!((rule.iter().map(|r| r.1).sum::<f64>() - 2.0).abs() < 1e-14, "weights of {n}");
            // Exact up to degree 2n − 1: ∫ x^(2n−2) dx = 2/(2n − 1).
            let degree = 2 * n - 2;
            let integral: f64 = rule.iter().map(|&(x, w)| w * x.powi(degree as i32)).sum();
            assert!((integral - 2.0 / (degree as f64 + 1.0)).abs() < 1e-14, "{n} nodes, degree {degree}");
            // Control: degree 2n is beyond the rule, and it shows.
            let beyond: f64 = rule.iter().map(|&(x, w)| w * x.powi(2 * n as i32)).sum();
            assert!((beyond - 2.0 / (2.0 * n as f64 + 1.0)).abs() > 1e-13 || n >= 24, "{n} nodes");
        }
    }

    #[test]
    fn arms_follow_the_parabolas_and_the_kick() {
        let instrument = Instrument { floor: false, ..Instrument::aion10() };
        let [lower, upper] = instrument.arms();
        let (t, g, u, v) = (0.73, 9.81, 3.86, instrument.kick_velocity());
        // ⁸⁷Sr and the exact ħ: the kick is 6.578 m/s, not the 6.505 of the rounded constants.
        assert!((v - 6.5779).abs() < 1e-3, "{v}");
        assert_eq!(lower.rise(0.0), 0.0);
        assert!((lower.rise(0.3) - (u * 0.3 - 0.5 * g * 0.09)).abs() < 1e-15);
        assert!((upper.rise(0.3) - ((u + v) * 0.3 - 0.5 * g * 0.09)).abs() < 1e-15);
        // The arms are furthest apart at the π pulse, by v·T, and meet again at 2T.
        assert!((upper.rise(t) - lower.rise(t) - v * t).abs() < 1e-12);
        assert!((upper.rise(2.0 * t) - lower.rise(2.0 * t)).abs() < 1e-12);
        // With u₀ = 3.86 they meet a little below the launch height: 2T(u₀ − gT + v/2).
        let end = 2.0 * t * (u - g * t + 0.5 * v);
        assert!(end < 0.0 && (lower.rise(2.0 * t) - end).abs() < 1e-12, "{end}");
        assert!(lower.landings().is_empty() && upper.landings().is_empty());
        // Control: the symmetric launch closes them at the launch height.
        let symmetric = Instrument { launch_velocity: instrument.symmetric_launch(), ..instrument };
        assert!(symmetric.arms()[0].rise(2.0 * t).abs() < 1e-12);
        // And then each arm is its own mirror image about T.
        for arm in symmetric.arms() {
            for tau in [0.1, 0.4, 0.7] {
                assert!((arm.rise(tau) - arm.rise(2.0 * t - tau)).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn the_floor_stops_an_arm_at_its_launch_height() {
        let instrument = Instrument::aion10();
        let [lower, upper] = instrument.arms();
        let two_t = instrument.flight_time();
        // Both arms land shortly before 2T and stay down.
        for arm in [lower, upper] {
            let landings = arm.landings();
            assert_eq!(landings.len(), 1);
            let landed = landings[0];
            assert!(landed > 1.4 && landed < two_t, "{landed}");
            // A microsecond earlier it is still in the air, by its landing speed times a microsecond:
            // the upper arm comes down from 5 m at 10.4 m/s, the lower at 3.8 m/s.
            assert!(arm.rise(landed - 1e-6) > 0.0 && arm.rise(landed - 1e-6) < 1.1e-5);
            assert_eq!(arm.rise(landed), 0.0);
            assert_eq!(arm.rise(two_t), 0.0);
            assert_eq!(arm.rise(two_t + 0.3), 0.0, "the atoms wait for the next sequence");
            // No arm is ever below its launch height.
            for step in 0..=1460 {
                assert!(arm.rise(f64::from(step) * 1e-3) >= 0.0);
            }
        }
        // Control: with the floor off the same arms go below it.
        let [bare, _] = Instrument { floor: false, ..instrument }.arms();
        assert!(bare.rise(two_t) < -0.01);
        // An arm that lands before the kick waits for it, then flies again from rest.
        let weak = Arm::new(1.0, 2.0, 0.73, 9.81, true);
        assert_eq!(weak.landings()[0], 2.0 / 9.81);
        assert_eq!(weak.rise(0.5), 0.0);
        assert!((weak.rise(0.73 + 0.1) - (2.0 * 0.1 - 0.5 * 9.81 * 0.01)).abs() < 1e-15);
        // And one kicked downwards on the floor stays on it.
        assert_eq!(Arm::new(1.0, -2.0, 0.73, 9.81, true).rise(1.0), 0.0);
    }

    #[test]
    fn the_rules_place_their_nodes_as_their_references_do() {
        let instrument = Instrument::aion10();
        // The engine's rule: pieces cut at T and at both landings, weights adding up to 2T.
        let flight = instrument.flight(&[]);
        // The weights add up to the time at least one arm is in the air: the flight, less the last
        // few milliseconds in which both are down and nothing is integrated.
        let landed = instrument.arms().iter().flat_map(Arm::landings).fold(0.0f64, f64::max);
        assert!(landed > 1.44 && landed < 1.46, "{landed}");
        assert!((flight.nodes.iter().map(|n| n.weight).sum::<f64>() - landed).abs() < 1e-13);
        assert!(flight.nodes.iter().all(|n| n.tau < landed));
        assert_eq!(instrument.node_count(&[]), flight.nodes.len() as u64, "the count is the work");
        // With the floor off there is no such piece and the weights are the whole flight.
        let bare = Instrument { floor: false, ..instrument }.flight(&[]);
        assert!((bare.nodes.iter().map(|n| n.weight).sum::<f64>() - 1.46).abs() < 1e-13);
        let landings: Vec<f64> = instrument.arms().iter().flat_map(Arm::landings).collect();
        for cut in landings.iter().chain(&[0.73]) {
            // No panel straddles a kink: the nearest nodes on either side are within one panel.
            assert!(flight.nodes.iter().all(|n| (n.tau - cut).abs() > 1e-9));
        }
        assert!(flight.nodes.len() < 147, "fewer nodes than v1's 150: {}", flight.nodes.len());
        // A source's break is a cut as well: the weights before it then add up to it exactly.
        let before =
            |flight: &Flight| flight.nodes.iter().filter(|n| n.tau < 0.4).map(|n| n.weight).sum::<f64>();
        assert!((before(&instrument.flight(&[0.4])) - 0.4).abs() < 1e-14);
        assert!((before(&flight) - 0.4).abs() > 1e-3, "control: without the break no panel ends there");
        // v1's rule: 74 steps on each half, 75 nodes each, the node at T counted on both sides.
        let v1 = instrument.with_v1_conventions().flight(&[0.4]);
        assert_eq!(v1.nodes.len(), 150);
        assert_eq!((v1.nodes[0].tau, v1.nodes[74].tau, v1.nodes[75].tau), (0.0, 0.73, 0.73));
        assert!((v1.nodes.iter().map(|n| n.weight).sum::<f64>() - 1.46).abs() < 1e-13);
        // The scripts' rule: nodes at j·dt, the arm one step ahead of the source as published.
        let script = |lag| Instrument {
            quadrature: Quadrature::ScriptTrapezoid { step: 0.01, nodes: 146, lag },
            ..instrument
        };
        let published = script(1).flight(&[]);
        let [lower, _] = instrument.arms();
        assert_eq!(published.nodes.len(), 146);
        assert_eq!(published.nodes[10].tau, 0.1);
        assert_eq!(published.nodes[10].lower, lower.rise(0.11));
        assert!((published.nodes.iter().map(|n| n.weight).sum::<f64>() - 1.45).abs() < 1e-13);
        // Control: with the offset corrected the arm is where the source's clock says.
        assert_eq!(script(0).flight(&[]).nodes[10].lower, lower.rise(0.1));
    }

    #[test]
    fn a_static_mass_reads_what_the_plain_formula_gives() {
        let instrument = Instrument::aion10();
        let reader = Reader::new(instrument);
        for (mass, at, tower) in [
            (1.0, Vec3::new(5.0, 0.0, 1.0), ORIGIN),
            (10.0, Vec3::new(-3.0, 4.0, 7.5), ORIGIN),
            (70.0, Vec3::new(2.0, 1.0, -1.0), Vec3::new(4.0, 0.0, 0.0)),
        ] {
            let phase = reader.phase(tower, &fixed(mass, at), 3.0);
            let reference = brute(&instrument, tower, mass, &|_| at, 3.0);
            assert!(phase.abs() > 1e-6, "{phase}");
            assert!((phase / reference - 1.0).abs() < 1e-11, "{phase} against {reference}");
        }
        // Control: the comparison sees a millimetre.
        let moved = reader.phase(ORIGIN, &fixed(1.0, Vec3::new(5.001, 0.0, 1.0)), 3.0);
        let reference = brute(&instrument, ORIGIN, 1.0, &|_| Vec3::new(5.0, 0.0, 1.0), 3.0);
        assert!((moved / reference - 1.0).abs() > 1e-5);
    }

    #[test]
    fn the_phase_is_linear_in_mass_and_sources_superpose() {
        let reader = Reader::new(Instrument::aion10());
        let a = fixed(3.0, Vec3::new(5.0, 0.0, 1.0));
        let b = Mass::point(8.0, |t: f64| Vec3::new(-2.0, 6.0, 1.0 + (0.6 * t).cos()));
        let (ra, rb) = (reader.read(ORIGIN, &a, 4.0), reader.read(ORIGIN, &b, 4.0));
        struct Both<'a>(&'a dyn Source, &'a dyn Source);
        impl Source for Both<'_> {
            fn difference(&self, t: Vec3, h: [f64; 2], l: f64, u: f64, time: f64) -> [f64; 2] {
                let (x, y) = (self.0.difference(t, h, l, u, time), self.1.difference(t, h, l, u, time));
                [x[0] + y[0], x[1] + y[1]]
            }
        }
        let both = reader.read(ORIGIN, &Both(&a, &b), 4.0);
        assert!((both.lower - (ra.lower + rb.lower)).abs() < 1e-12 * ra.lower.abs());
        assert!((both.upper - (ra.upper + rb.upper)).abs() < 1e-12 * ra.upper.abs());
        let doubled = reader.read(ORIGIN, &fixed(6.0, Vec3::new(5.0, 0.0, 1.0)), 4.0);
        assert_eq!(doubled.lower, 2.0 * ra.lower, "doubling the mass doubles the phase, to the bit");
        // Control: the two sources are not the same size, so the sum is not twice either.
        assert!((both.lower / (2.0 * ra.lower) - 1.0).abs() > 1e-3);
    }

    #[test]
    fn the_sign_and_the_channels_are_the_stated_ones() {
        let instrument = Instrument::aion10();
        let reader = Reader::new(instrument);
        // A mass near the base is nearer the lower interferometer, whose arms fly from 0 to 5 m,
        // than the upper, whose arms fly from 5 to 10 m: the lower reads the more.
        let source = fixed(100.0, Vec3::new(2.0, 0.0, 1.0));
        let reading = reader.read(ORIGIN, &source, 2.0);
        assert!(reading.lower > 2.0 * reading.upper, "{reading:?}");
        // Above the mass the potential rises with height, so the upper arm's is the greater.
        assert!(reading.upper > 0.0);
        assert_eq!(reading.gradiometer(Sign::LowerMinusUpper), reading.lower - reading.upper);
        assert_eq!(reading.gradiometer(Sign::UpperMinusLower), -reading.gradiometer(Sign::LowerMinusUpper));
        assert_eq!(reader.phase(ORIGIN, &source, 2.0), reading.lower - reading.upper);
        assert_eq!(reading.channel(Channel::Lower, instrument.sign), reading.lower);
        assert_eq!(reading.channel(Channel::Upper, instrument.sign), reading.upper);
        // v1's conventions flip the sign of the same source's reading.
        let v1 = Reader::new(instrument.with_v1_conventions());
        assert!(v1.phase(ORIGIN, &source, 2.0) * reader.phase(ORIGIN, &source, 2.0) < 0.0);
    }

    #[test]
    fn a_uniform_sphere_is_a_point_outside_and_finite_inside() {
        let reader = Reader::new(Instrument::aion10());
        let at = Vec3::new(3.0, 0.0, 2.0);
        let point = reader.read(ORIGIN, &fixed(50.0, at), 2.0);
        let sphere = Mass { radius: 0.5, ..fixed(50.0, at) };
        assert_eq!(reader.read(ORIGIN, &sphere, 2.0), point, "outside, the shell theorem is exact");
        // On the atoms' path a point is singular and a sphere is not.
        let on_axis = Vec3::new(0.0, 0.0, 1.0);
        let through = reader.read(ORIGIN, &Mass { radius: 0.2, ..fixed(50.0, on_axis) }, 2.0);
        assert!(through.lower.is_finite() && through.upper.is_finite(), "{through:?}");
        assert!(through.lower.abs() > point.lower.abs());
        // The potential is continuous across the surface.
        let sphere = Mass { radius: 0.2, ..fixed(1.0, ORIGIN) };
        let h = [0.0, 5.0];
        let inside = sphere.difference(ORIGIN, h, 0.1, 0.2 - 1e-9, 0.0)[0];
        let outside = sphere.difference(ORIGIN, h, 0.1, 0.2 + 1e-9, 0.0)[0];
        assert!((inside - outside).abs() < 1e-7 * inside.abs(), "{inside} {outside}");
        // At the centre: V/G = −3M/(2R); at the surface −M/R.
        let centre_to_surface = sphere.difference(ORIGIN, h, 0.0, 0.2, 0.0)[0];
        assert!((centre_to_surface - (-1.0 / 0.2 + 1.5 / 0.2)).abs() < 1e-12);
    }

    #[test]
    fn a_source_that_appears_or_vanishes_cuts_the_flight_there() {
        let instrument = Instrument::aion10();
        let reader = Reader::new(instrument);
        let at = Vec3::new(4.0, 0.0, 2.0);
        let always = reader.read(ORIGIN, &fixed(20.0, at), 3.0).lower;
        // Present for the whole flight, the first half, the second half.
        let window = |from: f64, until: f64| {
            reader.read(ORIGIN, &Mass { exists: (from, until), ..fixed(20.0, at) }, 3.0).lower
        };
        let (first, second) = (window(0.0, 2.0), window(2.0, 10.0));
        assert_eq!(window(0.0, 10.0), always);
        assert!(
            (first + second - always).abs() < 1e-13 * always.abs(),
            "{first} + {second} against {always}"
        );
        assert!(first != 0.0 && second != 0.0 && (first - second).abs() > 1e-3 * always.abs());
        assert_eq!(window(5.0, 6.0), 0.0, "not yet there");
        assert_eq!(window(-3.0, 1.0), 0.0, "already gone");
        // A flash of one millisecond is its width times the integrand, and it is not missed.
        let flash = window(2.2, 2.201);
        let [lower, upper] = instrument.arms();
        let tau = 2.2005 - (3.0 - 1.46);
        let integrand =
            fixed(20.0, at).difference(ORIGIN, [0.0, 5.0], lower.rise(tau), upper.rise(tau), 0.0)[0];
        let expected = instrument.phase_scale() * integrand * 1e-3;
        assert!(flash != 0.0 && (flash / expected - 1.0).abs() < 1e-6, "{flash} against {expected}");
    }

    #[test]
    fn an_infinite_slab_reads_exactly_nothing_in_the_gradiometer() {
        let instrument = Instrument::aion10();
        let reader = Reader::new(instrument);
        let slab = Slab { density: 2400.0, thickness: 1.0 };
        for tower in [ORIGIN, Vec3::new(4.0, -3.0, 0.7), Vec3::new(1e4, -7e3, 300.0)] {
            let reading = reader.read(tower, &slab, 2.0);
            // Each interferometer feels the slab: k_eff·g·T² with g = 2πGρh, about 5000 rad …
            let k_eff = TAU * instrument.kicks / instrument.wavelength;
            let g = TAU * instrument.big_g * 2400.0;
            assert!((reading.lower / (k_eff * g * 0.73 * 0.73) - 1.0).abs() < 2e-3, "{}", reading.lower);
            // … and both feel exactly the same.
            assert_eq!(reading.lower.to_bits(), reading.upper.to_bits());
            assert_eq!(reading.gradiometer(instrument.sign), 0.0);
        }
        // Control: a floor with an edge is not a null. The same slab cut to a disc reads something.
        let disc = AxialDisc { density: 2400.0, thickness: 1.0, radius: 50.0, depth: 0.0 };
        assert!(reader.phase(ORIGIN, &disc, 2.0).abs() > 1.0);
    }

    #[test]
    fn a_disc_on_the_axis_matches_rings_of_point_masses() {
        // The closed form against the same disc as a sum of point masses, at two heights.
        let disc = AxialDisc { density: 1000.0, thickness: 0.5, radius: 3.0, depth: 1.0 };
        let closed = disc.difference(ORIGIN, [0.0, 5.0], 0.5, 2.5, 0.0);
        let (rings, layers, spokes) = (400, 40, 1);
        let mut summed = [0.0f64; 2];
        for ring in 0..rings {
            let rho = (f64::from(ring) + 0.5) * 3.0 / f64::from(rings);
            for layer in 0..layers {
                let z = -1.0 - (f64::from(layer) + 0.5) * 0.5 / f64::from(layers);
                // On the axis a ring is one point at its radius.
                let mass = 1000.0 * TAU * rho * (3.0 / f64::from(rings)) * (0.5 / f64::from(layers))
                    / f64::from(spokes);
                let d = fixed(mass, Vec3::new(rho, 0.0, z)).difference(ORIGIN, [0.0, 5.0], 0.5, 2.5, 0.0);
                summed[0] += d[0];
                summed[1] += d[1];
            }
        }
        for k in 0..2 {
            assert!(
                (closed[k] / summed[k] - 1.0).abs() < 1e-5,
                "ifo {k}: {} against {}",
                closed[k],
                summed[k]
            );
        }
        // Control: a disc of another radius is told apart.
        let wider = AxialDisc { radius: 3.1, ..disc }.difference(ORIGIN, [0.0, 5.0], 0.5, 2.5, 0.0);
        assert!((wider[0] / summed[0] - 1.0).abs() > 1e-3);
    }

    #[test]
    fn the_audited_read_stops_and_says_what_it_did() {
        let reader = Reader::new(Instrument::aion10());
        let at = Vec3::new(5.0, 0.0, 1.0);
        // A smooth, distant source: one halving confirms the fixed rule, and the estimate is tiny.
        let far = reader.read_to(ORIGIN, &fixed(1.0, at), 2.0, PHASE_TOLERANCE);
        assert_eq!(far.halvings, 1);
        assert!(far.estimate.unwrap() < 1e-13, "{:?}", far.estimate);
        assert!((far.reading.lower / reader.read(ORIGIN, &fixed(1.0, at), 2.0).lower - 1.0).abs() < 1e-12);
        // The reference rules are what they are: they are not refined, and no estimate is claimed.
        let v1 = Reader::new(Instrument::aion10().with_v1_conventions());
        let reference = v1.read_to(ORIGIN, &fixed(1.0, at), 2.0, PHASE_TOLERANCE);
        assert_eq!((reference.estimate, reference.halvings), (None, 0));
        assert_eq!(reference.reading, v1.read(ORIGIN, &fixed(1.0, at), 2.0));
        // Control: a point mass on the atoms' own path has no converged integral. The audit runs
        // out of halvings and says so with an estimate far above the tolerance; it does not
        // pretend to an answer.
        let on_path = reader.read_to(ORIGIN, &fixed(1.0, Vec3::new(0.0, 0.0, 1.0)), 2.0, PHASE_TOLERANCE);
        assert_eq!(on_path.halvings, MAX_HALVINGS);
        assert!(on_path.estimate.unwrap() > 1e3 * PHASE_TOLERANCE, "{:?}", on_path.estimate);
        // And a source that is not a number stops the audit at once.
        struct Broken;
        impl Source for Broken {
            fn difference(&self, _: Vec3, _: [f64; 2], _: f64, _: f64, _: f64) -> [f64; 2] {
                [f64::NAN; 2]
            }
        }
        let broken = reader.read_to(ORIGIN, &Broken, 2.0, PHASE_TOLERANCE);
        assert_eq!(broken.halvings, 1);
        assert!(broken.estimate.unwrap().is_nan() && broken.reading.lower.is_nan());
    }

    // The published anchors (reference/design/instrument.md §12), as paths.

    /// 10 kg at D = 10 m passing vertically at 2.5 m/s: a peak of 13.7 mrad.
    fn anchor_pass(t: f64) -> Vec3 {
        Vec3::new(10.0, 0.0, -10.0 + 2.5 * t)
    }

    /// 1 kg at D = 5 m, h = 1 + cos(2π·0.1·t): about 4 mrad peak to peak.
    fn anchor_oscillation(t: f64) -> Vec3 {
        Vec3::new(5.0, 0.0, 1.0 + (TAU * 0.1 * t).cos())
    }

    /// The gradiometer series of a point mass on a path, at readouts `first + n·step`.
    fn series(
        reader: &Reader,
        tower: Vec3,
        mass: f64,
        path: fn(f64) -> Vec3,
        (first, step, count): (f64, f64, usize),
    ) -> Vec<f64> {
        let source = Mass::point(mass, path);
        (0..count).map(|n| reader.phase(tower, &source, first + n as f64 * step)).collect()
    }

    /// The largest difference between two series, as a fraction of the largest value of the second.
    fn worst_of_peak(a: &[f64], b: &[f64]) -> f64 {
        assert_eq!(a.len(), b.len());
        let peak = b.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs())) / peak
    }

    fn fine(instrument: Instrument, order: usize, panel: f64) -> Reader {
        Reader::new(Instrument { quadrature: Quadrature::Gauss { order, panel }, ..instrument })
    }

    fn quadrature_error_on_the_anchors(reader: &Reader) -> f64 {
        let reference = fine(*reader.instrument(), 12, 0.025);
        let still: fn(f64) -> Vec3 = |_| Vec3::new(5.0, 0.0, 1.0);
        /// A mass, its path, and the readouts: first, step, count.
        type Anchor = (f64, fn(f64) -> Vec3, (f64, f64, usize));
        let cases: [Anchor; 3] = [
            (1.0, still, (2.0, 1.0, 1)),
            (10.0, anchor_pass, (0.0, 0.5, 21)),
            (1.0, anchor_oscillation, (0.0, 0.25, 41)),
        ];
        cases.into_iter().fold(0.0f64, |worst, (mass, path, readouts)| {
            let (ours, theirs) = (
                series(reader, ORIGIN, mass, path, readouts),
                series(&reference, ORIGIN, mass, path, readouts),
            );
            worst.max(worst_of_peak(&ours, &theirs))
        })
    }

    cav_validate::case! {
        test: validation_quadrature_on_the_anchors,
        name: "forward.quadrature_on_the_anchors",
        subsystem: "forward",
        slice: "S1",
        reference: Exact,
        against: "the engine's rule (Gauss, 6 nodes, 0.1 s panels) against 12 nodes on 0.025 s panels, on a static mass and the two published anchors; largest difference as a fraction of each series' peak",
        unit: Relative,
        tolerance: 1e-11,
        measure: {
            let reader = Reader::new(Instrument::aion10());
            // Control: a rule with two nodes a panel is not converged, and the measure says so.
            let coarse = fine(Instrument::aion10(), 2, 0.1);
            assert!(quadrature_error_on_the_anchors(&coarse) > 1e-8);
            cav_validate::Measured::Error(quadrature_error_on_the_anchors(&reader))
        },
    }

    cav_validate::case! {
        test: validation_quadrature_order,
        name: "forward.quadrature_order",
        subsystem: "forward",
        slice: "S1",
        reference: ClosedForm,
        against: "Gauss-Legendre with 3 nodes a panel converges as h^6: the oscillation anchor's lower-interferometer phase at panels of T/2, T/4, T/8, T/16 against 12 nodes on T/64 (floor off, so the pieces are the two halves). The case is about the order; the 1e-9 on the finest step is a ceiling with no prediction behind it",
        unit: Relative,
        tolerance: 1e-9,
        order: (6.0, 0.5),
        measure: {
            let instrument = Instrument { floor: false, ..Instrument::aion10() };
            let source = Mass::point(1.0, anchor_oscillation);
            let reference = fine(instrument, 12, 0.73 / 64.0).read(ORIGIN, &source, 3.0).lower;
            // One panel for each half of the flight (h = T) is not yet in the asymptotic range: with
            // it the fit was 6.65 (docs/slices/S1.md §7.1). The order is fitted from T/2 down.
            let points = [2.0, 4.0, 8.0, 16.0]
                .iter()
                .map(|panels| {
                    let h = 0.73 / panels;
                    (h, (fine(instrument, 3, h).read(ORIGIN, &source, 3.0).lower / reference - 1.0).abs())
                })
                .collect();
            cav_validate::Measured::Convergence(points)
        },
    }

    /// 10 kg passing 0.5 m from the tower at 2.5 m/s, level with its base at t = 4 s.
    fn close_pass(t: f64) -> Vec3 {
        Vec3::new(0.5, 0.0, -10.0 + 2.5 * t)
    }

    cav_validate::case! {
        test: validation_audited_read_on_a_close_pass,
        name: "forward.audited_read_close_pass",
        subsystem: "forward",
        slice: "S1",
        reference: Exact,
        against: "10 kg passing 0.5 m from a tower at 2.5 m/s, read until two successive rules agree to 1e-10 rad, against 12 nodes on 1.6 ms panels; largest error over 25 shots and both interferometers",
        unit: Radians,
        tolerance: 1e-10,
        measure: {
            let instrument = Instrument::aion10();
            let (reader, reference) = (Reader::new(instrument), fine(instrument, 12, 0.1 / 64.0));
            let source = Mass::point(10.0, close_pass);
            let apart = |a: Reading, b: Reading| (a.lower - b.lower).abs().max((a.upper - b.upper).abs());
            let (mut audited_error, mut fixed_error, mut halvings) = (0.0f64, 0.0f64, 0);
            for shot in 0..25 {
                let readout = 3.0 + 0.25 * f64::from(shot);
                let truth = reference.read(ORIGIN, &source, readout);
                let audited = reader.read_to(ORIGIN, &source, readout, PHASE_TOLERANCE);
                let error = apart(audited.reading, truth);
                // The estimate is the coarser rule's error: it must not be below the answer's.
                let estimate = audited.estimate.expect("a Gauss rule is audited");
                assert!(estimate >= error, "shot {shot}: estimate {estimate:e} below the error {error:e}");
                assert!(audited.halvings < MAX_HALVINGS, "shot {shot} ran out of halvings");
                audited_error = audited_error.max(error);
                halvings = halvings.max(audited.halvings);
                fixed_error = fixed_error.max(apart(reader.read(ORIGIN, &source, readout), truth));
            }
            // Control: the fixed rule, made for the anchors, is not good enough here.
            assert!(fixed_error > PHASE_TOLERANCE, "the fixed rule is off by only {fixed_error:e} rad");
            assert!(halvings >= 2, "the audit refined {halvings} time(s)");
            eprintln!("close pass: fixed rule off by {fixed_error:e} rad, audited by {audited_error:e} rad, {halvings} halvings");
            cav_validate::Measured::Error(audited_error)
        },
    }

    /// The oscillation anchor read by a tower at (3.7, 0.4, 0), everything moved by `shift`; with
    /// `single`, every world coordinate is first rounded to `f32`, as a GPU given world
    /// coordinates would hold it. The tower is not a whole number of metres from the source: two
    /// coordinates that differ by a multiple of the format's spacing round alike, and the rounding
    /// would cancel in their difference without any local frame (docs/slices/S1.md §7.1).
    fn shifted_series(shift: Vec3, single: bool) -> Vec<f64> {
        let reader = Reader::new(Instrument::aion10());
        let round = move |v: Vec3| {
            if single {
                Vec3::new(f64::from(v.x as f32), f64::from(v.y as f32), f64::from(v.z as f32))
            } else {
                v
            }
        };
        let tower = round(Vec3::new(3.7, 0.4, 0.0) + shift);
        let source = Mass::point(1.0, move |t| round(anchor_oscillation(t) + shift));
        (0..41).map(|n| reader.phase(tower, &source, 0.25 * f64::from(n))).collect()
    }

    cav_validate::case! {
        test: validation_precision_far,
        name: "forward.precision_far",
        subsystem: "forward",
        slice: "S1",
        reference: Exact,
        against: "the oscillation anchor and a tower 1.36 m from it, both moved by (10000.3, -7000.7, 300.1) m, read the same series as at the origin; largest difference as a fraction of the peak",
        unit: Relative,
        tolerance: 1e-11,
        measure: {
            // Not whole numbers: those are exact in f32 and subtract exactly in f64, and a shift made
            // of them tests neither arm (docs/slices/S1.md §7.1).
            let far = Vec3::new(10_000.3, -7_000.7, 300.1);
            let near = shifted_series(ORIGIN, false);
            // Control: world coordinates held in f32 are fine at the origin and useless 10 km out.
            let single_near = worst_of_peak(&shifted_series(ORIGIN, true), &near);
            let single_far = worst_of_peak(&shifted_series(far, true), &near);
            eprintln!("precision-far: f32 world coordinates are off by {single_near:e} at the origin, {single_far:e} at 10 km");
            // Measured 5.8e-8 and 9.4e-5. The brief first said "more than 1e-4" for the second; that
            // was a prediction and it was 6% out (docs/slices/S1.md §7.1). What the control is for:
            // the f32 answer is far outside this case's tolerance, and it is the distance that does it.
            assert!(single_near < 1e-6, "{single_near:e}");
            assert!(single_far > 1e-8 && single_far > 100.0 * single_near, "{single_near:e} {single_far:e}");
            cav_validate::Measured::Error(worst_of_peak(&shifted_series(far, false), &near))
        },
    }
    cav_validate::case! {
        test: validation_finite_floor_converges,
        name: "nulls.finite_floor_edge",
        subsystem: "nulls",
        slice: "S1",
        reference: ClosedForm,
        against: "a concrete disc 1 m thick under a tower on its axis, radius 400 m to 6.4 km: the gradiometer's reading as a fraction of what one interferometer reads from the infinite slab; it falls as 1/R, the edge's uniform gradient, towards the slab's zero",
        unit: Relative,
        tolerance: 1e-3,
        order: (1.0, 0.05),
        measure: {
            let instrument = Instrument::aion10();
            let reader = Reader::new(instrument);
            let slab = reader.read(ORIGIN, &Slab { density: 2400.0, thickness: 1.0 }, 2.0).lower;
            let points: Vec<(f64, f64)> = [400.0, 800.0, 1600.0, 3200.0, 6400.0]
                .iter()
                .map(|&radius| {
                    let disc = AxialDisc { density: 2400.0, thickness: 1.0, radius, depth: 0.0 };
                    (1.0 / radius, (reader.phase(ORIGIN, &disc, 2.0) / slab).abs())
                })
                .collect();
            // The leading term is Δr/R: 7.8e-4 at 6.4 km.
            let (step, error) = points[4];
            assert!((error / (instrument.ifo_separation * step) - 1.0).abs() < 0.05, "{error:e}");
            cav_validate::Measured::Convergence(points)
        },
    }
}
